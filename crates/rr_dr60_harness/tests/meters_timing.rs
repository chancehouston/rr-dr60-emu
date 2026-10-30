//! Spec 016 FR-013 (tasks.md T015; research R-09): the per-sample cost of the default front
//! (stages 1 – 2 on, the speech codec bypassed, as specs 009 – 013 measure "the default") and
//! of every stage off, written against **version 0.14's public API only** (no `Meters`), so
//! `scripts/ab-timing.sh <rev> meters` can copy this file into a 0.14 worktree and compare the
//! two builds (budget 5 % on the default front, the same absolute nanoseconds with every stage
//! off; 12 % on the CI runner). Ignored by default (a release-mode timing; never under
//! coverage). Run: `cargo test -p rr_dr60_harness --release --test meters_timing -- --ignored
//! --nocapture`

use std::time::Instant;

use rr_dr60::{__test_hooks, MicSettings, Pipeline, Route, Settings, Tap};

fn noise(rate: u32, seconds: usize) -> Vec<f32> {
    let mut state = 0x016u64;
    (0..seconds * rate as usize)
        .map(|_| {
            // A deterministic white noise at −20 dB (xorshift; no platform math).
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            ((state >> 40) as f32 / (1u64 << 23) as f32 - 1.0) * 0.1
        })
        .collect()
}

fn fastest(settings: Settings, x: &[f32]) -> (f64, Pipeline) {
    let mut best = f64::MAX;
    let mut last = None;
    for _ in 0..45 {
        let mut p = Pipeline::new(settings).unwrap();
        p.set_route(Route::Speaker);
        p.set_volume(1.0).unwrap();
        let mut buf = x.to_vec();
        let t = Instant::now();
        for chunk in buf.chunks_mut(4096) {
            let _ = p.process_in_place(chunk);
        }
        best = best.min(t.elapsed().as_nanos() as f64 / x.len() as f64);
        last = Some(p);
    }
    (best, last.unwrap())
}

/// The two figures the A/B script reads: `default_front_ns=` and `every_stage_off_ns=`.
#[test]
#[ignore = "release-mode timing; run with --ignored"]
fn meters_cost() {
    if cfg!(debug_assertions) {
        eprintln!("timing tests need --release; skipped");
        return;
    }
    let rate = 48_000;
    let x = noise(rate, 30);
    // The default front: 0.14's default with the speech codec bypassed.
    let mut front = Settings::new(rate);
    front.mic = MicSettings::DEVICE;
    front.speech_codec_enabled = false;
    let (front_ns, p) = fastest(front, &x);
    assert_eq!(
        __test_hooks::limiter_state(&p),
        0.0,
        "the −20 dB noise must leave the limiter uncharged"
    );
    // Every stage off at the route output.
    let mut off = Settings::new(rate);
    off.mic = MicSettings::BYPASSED;
    off.agc.enabled = false;
    off.record_stage_enabled = false;
    off.adc_clamp_enabled = false;
    off.vas.enabled = false;
    off.speech_codec_enabled = false;
    off.playback_stage_enabled = false;
    off.output = rr_dr60::OutputSettings::BYPASSED;
    off.tap = Tap::RouteOutput;
    let (off_ns, _) = fastest(off, &x);
    println!("default_front_ns={front_ns:.3}");
    println!("every_stage_off_ns={off_ns:.3}");
}
