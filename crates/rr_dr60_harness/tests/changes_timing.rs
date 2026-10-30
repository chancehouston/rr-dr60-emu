//! Spec 015 FR-010 (tasks.md T018; research R-09): the per-sample cost of the player and of the
//! one-pass with the schedule empty, written against **version 0.15's public API only** (no
//! schedule call), so `scripts/ab-timing.sh <rev> changes` can copy this file into a 0.15
//! worktree and compare the two builds (budget 3 %; 5 % on the CI runner). Ignored by default
//! (a release-mode timing; never under coverage). Run:
//! `cargo test -p rr_dr60_harness --release --test changes_timing -- --ignored --nocapture`

use std::time::Instant;

use rr_dr60::{MicSettings, Pipeline, Route, Settings};

fn noise(rate: u32, seconds: usize) -> Vec<f32> {
    let mut state = 0x015u64;
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

/// The 0.15 default front with the speech codec bypassed (specs 009 – 013's "default").
fn front(rate: u32) -> Settings {
    let mut s = Settings::new(rate);
    s.mic = MicSettings::DEVICE;
    s.speech_codec_enabled = false;
    s
}

fn fastest_one_pass(settings: Settings, x: &[f32]) -> f64 {
    let mut best = f64::MAX;
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
    }
    best
}

fn fastest_playback(settings: Settings, x: &[f32]) -> f64 {
    let mut p = Pipeline::new(settings).unwrap();
    p.set_route(Route::Speaker);
    p.set_volume(1.0).unwrap();
    p.start_recording().unwrap();
    let _ = p.record(x, &mut []).unwrap();
    let _ = p.stop_recording(&mut []).unwrap();
    let mut best = f64::MAX;
    let mut buf = vec![0.0f32; 4096];
    for _ in 0..45 {
        p.start_playback(1).unwrap();
        let mut produced = 0usize;
        let t = Instant::now();
        loop {
            let info = p.play(&mut buf).unwrap();
            produced += info.produced;
            if info.finished {
                break;
            }
        }
        best = best.min(t.elapsed().as_nanos() as f64 / produced as f64);
    }
    best
}

/// The two figures the A/B script reads: `playback_ns=` and `one_pass_ns=`.
#[test]
#[ignore = "release-mode timing; run with --ignored"]
fn changes_cost() {
    if cfg!(debug_assertions) {
        eprintln!("timing tests need --release; skipped");
        return;
    }
    let rate = 48_000;
    let x = noise(rate, 10);
    let playback_ns = fastest_playback(front(rate), &x);
    let one_pass_ns = fastest_one_pass(front(rate), &noise(rate, 30));
    println!("playback_ns={playback_ns:.3}");
    println!("one_pass_ns={one_pass_ns:.3}");
}
