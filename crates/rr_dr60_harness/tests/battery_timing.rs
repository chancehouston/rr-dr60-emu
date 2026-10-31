//! Spec 018 FR-007 (tasks.md T022; research R-08): the per-sample cost of the default front
//! (stages 1 – 2 on, the speech codec bypassed) on the speaker route at the default battery
//! value, written against **version 0.17's public API only** (no battery call), so
//! `scripts/ab-timing.sh <rev> battery` can copy this file into a 0.17 worktree and compare
//! the two builds (budget 3 %; `RR_DR60_BATTERY_COST_CEILING`, 0.05 on the CI runner). Ignored
//! by default (a release-mode timing; never under coverage). Run:
//! `cargo test -p rr_dr60_harness --release --test battery_timing -- --ignored --nocapture`

use std::time::Instant;

use rr_dr60::{MicSettings, Pipeline, Route, Settings};

fn noise(rate: u32, seconds: usize) -> Vec<f32> {
    let mut state = 0x018u64;
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

/// The figure the A/B script reads: `speaker_ns=`.
#[test]
#[ignore = "release-mode timing; run with --ignored"]
fn battery_cost() {
    if cfg!(debug_assertions) {
        eprintln!("timing tests need --release; skipped");
        return;
    }
    let rate = 48_000;
    let x = noise(rate, 30);
    let mut front = Settings::new(rate);
    front.mic = MicSettings::DEVICE;
    front.speech_codec_enabled = false;
    let mut best = f64::MAX;
    for _ in 0..45 {
        let mut p = Pipeline::new(front).unwrap();
        p.set_route(Route::Speaker);
        p.set_volume(1.0).unwrap();
        let mut buf = x.to_vec();
        let t = Instant::now();
        for chunk in buf.chunks_mut(4096) {
            let _ = p.process_in_place(chunk);
        }
        best = best.min(t.elapsed().as_nanos() as f64 / x.len() as f64);
    }
    println!("speaker_ns={best:.3}");
}
