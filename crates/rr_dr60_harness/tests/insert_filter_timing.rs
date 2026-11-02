//! Spec 024 FR-010 (research R-10): the band filter's cost, reported: ns per host sample with the
//! filter at rest at 48 kHz, a transition every 200 ms, and the worst 64-sample block with three
//! parameters ramping at 96 kHz asserted within real time. Ignored by default (release-mode
//! timing). Run:
//! `cargo test -p rr_dr60_harness --release --test insert_filter_timing -- --ignored --nocapture --test-threads=1`

// The default Q is the spec's literal 0.7071 (024 D4), not 1/√2.
#![allow(clippy::approx_constant)]

use std::time::Instant;

use rr_dr60::{Pipeline, Settings, Stream};
use rr_dr60_harness::insert_checks as ic;

fn noise(rate: u32, seconds: usize) -> Vec<f32> {
    let mut state = 0x024u64;
    (0..seconds * rate as usize)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            ((state >> 40) as f32 / (1u64 << 23) as f32 - 1.0) * 0.1
        })
        .collect()
}

fn best_of(n: usize, mut f: impl FnMut() -> f64) -> f64 {
    (0..n).map(|_| f()).fold(f64::MAX, f64::min)
}

#[test]
#[ignore = "release-mode timing; run with --ignored"]
fn insert_filter_cost() {
    if cfg!(debug_assertions) {
        eprintln!("timing tests need --release; skipped");
        return;
    }
    let rate = 48_000u32;
    let x = noise(rate, 10);
    let mut s = Settings::new(rate);
    s.speech_codec_enabled = false;
    let mut out = vec![0.0f32; 4096];
    let run = |p: &mut Pipeline, out: &mut [f32]| {
        p.reset();
        let t = Instant::now();
        for chunk in x.chunks(4096) {
            let _ = p.process(chunk, &mut out[..chunk.len()]).unwrap();
        }
        t.elapsed().as_secs_f64() * 1e9 / x.len() as f64
    };
    let mut empty = Pipeline::new(s).unwrap();
    let e = best_of(25, || run(&mut empty, &mut out));
    let mut filt = ic::engine(s, [0.0, 1000.0, 0.7071, 0.0]);
    let f = best_of(25, || run(&mut filt, &mut out));
    println!("one-pass at 48 kHz: empty {e:.2} ns, the band filter at rest {f:.2} ns per host sample ({:+.2} ns)", f - e);
    let mut ramp = ic::engine(s, [0.0, 1000.0, 0.7071, 0.0]);
    let r = best_of(15, || {
        ramp.reset();
        let step = u64::from(rate) / 5;
        for k in 0..(x.len() as u64 / step) {
            let v = if k % 2 == 0 { 2000.0 } else { 1000.0 };
            ramp.schedule_change(Stream::OnePass, k * step + 1, 33, v, k + 1).unwrap();
        }
        let t = Instant::now();
        for chunk in x.chunks(4096) {
            let _ = ramp.process(chunk, &mut out[..chunk.len()]).unwrap();
        }
        t.elapsed().as_secs_f64() * 1e9 / x.len() as f64
    });
    println!("a frequency transition every 200 ms: {r:.2} ns per host sample ({:+.2} ns over the filter at rest)", r - f);
    // The worst 64-sample block at 96 kHz with three parameters ramping.
    let rate = 96_000u32;
    let mut s96 = Settings::new(rate);
    s96.speech_codec_enabled = false;
    let x96 = noise(rate, 1);
    let mut worst = 0.0f64;
    for _ in 0..15 {
        let mut p = ic::engine(s96, [0.0, 1000.0, 0.7071, 0.0]);
        for (c, v) in [(33u32, 3000.0), (34, 4.0), (35, 6.0)] {
            p.schedule_change(Stream::OnePass, 6400, c, v, 1).unwrap();
        }
        let mut y = [0.0f32; 64];
        for chunk in x96.chunks(64) {
            let t = Instant::now();
            let _ = p.process(chunk, &mut y[..chunk.len()]).unwrap();
            worst = worst.max(t.elapsed().as_secs_f64() * 1e6);
        }
    }
    let budget = 1000.0 * 64.0 / 96.0;
    println!("the worst 64-sample block at 96 kHz with three ramps: {worst:.1} µs of {budget:.1}");
    assert!(worst < budget, "the block within real time (FR-010)");
}
