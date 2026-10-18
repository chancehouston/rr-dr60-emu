//! Spec 013 FR-010 (tasks.md T022; research R-07): the playback's cost per host sample at the
//! default speed level, written against **version 0.12's public API only**, so
//! `scripts/ab-timing.sh <rev> playback` can copy this file into a 0.12 worktree and compare the
//! two builds (level 3 against 0.12: at most 3 % on the development machine, 5 % on the CI
//! runner). Ignored by default (a release-mode timing; never under coverage).
//! Run: `cargo test -p rr_dr60_harness --release --test playback_timing -- --ignored --nocapture`

use std::time::Instant;

use rr_dr60::{Pipeline, Route, Settings};

/// The codec-bypassed playback of 20 s of −20 dB noise at 48 kHz on the speaker route in
/// 4096-sample blocks, the fastest of 5 runs, printed in the A/B script's form.
#[test]
#[ignore = "release-mode timing; run with --ignored"]
fn playback_cost() {
    if cfg!(debug_assertions) {
        eprintln!("timing tests need --release; skipped");
        return;
    }
    let rate = 48_000u32;
    let mut state = 0x013u64;
    let x: Vec<f32> = (0..20 * rate as usize)
        .map(|_| {
            // A deterministic white noise at −20 dB (xorshift; no platform math).
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            ((state >> 40) as f32 / (1u64 << 23) as f32 - 1.0) * 0.1
        })
        .collect();
    let mut s = Settings::new(rate);
    s.speech_codec_enabled = false; // the codec-bypassed playback (013 FR-010)
    let mut p = Pipeline::new(s).unwrap();
    p.set_route(Route::Speaker);
    p.start_recording().unwrap();
    for chunk in x.chunks(4096) {
        let _ = p.record(chunk, &mut []).unwrap();
    }
    let _ = p.stop_recording(&mut []).unwrap();
    let mut out = vec![0.0f32; 4096];
    let mut best = f64::MAX;
    for _ in 0..5 {
        p.start_playback(1).unwrap();
        let mut n = 0usize;
        let t = Instant::now();
        loop {
            let info = p.play(&mut out).unwrap();
            n += info.produced;
            if info.finished {
                break;
            }
        }
        best = best.min(t.elapsed().as_nanos() as f64 / n as f64);
    }
    println!("playback, Speaker route, 48 kHz, 4096 blocks: {best:.2} ns per host sample");
}
