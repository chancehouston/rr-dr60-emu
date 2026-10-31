//! Spec 019 FR-009 (tasks.md T018; research R-08): the playback's cost per host sample at a
//! constant level 3 and at a constant level 5 (the general path, where the crossfade's test
//! and the re-signatured dot live), written against **version 0.18's public API only**, so
//! `scripts/ab-timing.sh <rev> crossfade` can copy this file into a 0.18 worktree and compare
//! the two builds (both within 3 %; `RR_DR60_CROSSFADE_COST_CEILING`, 0.05 on the CI runner).
//! Ignored by default (a release-mode timing; never under coverage). Run:
//! `cargo test -p rr_dr60_harness --release --test crossfade_timing -- --ignored --nocapture`

use std::time::Instant;

use rr_dr60::{Pipeline, Route, Settings};

fn noise(rate: u32, seconds: usize) -> Vec<f32> {
    let mut state = 0x019u64;
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

/// The figures the A/B script reads: `level3_ns=` and `level5_ns=` (the codec-bypassed playback
/// of 20 s of noise at 48 kHz on the speaker route in 4096-sample blocks, the fastest of 45).
#[test]
#[ignore = "release-mode timing; run with --ignored"]
fn crossfade_cost() {
    if cfg!(debug_assertions) {
        eprintln!("timing tests need --release; skipped");
        return;
    }
    let rate = 48_000u32;
    let x = noise(rate, 20);
    let mut s = Settings::new(rate);
    s.speech_codec_enabled = false;
    let mut p = Pipeline::new(s).unwrap();
    p.set_route(Route::Speaker);
    p.start_recording().unwrap();
    for chunk in x.chunks(4096) {
        let _ = p.record(chunk, &mut []).unwrap();
    }
    let _ = p.stop_recording(&mut []).unwrap();
    let mut out = vec![0.0f32; 4096];
    for level in [3u8, 5] {
        p.set_playback_speed(level).unwrap();
        let mut best = f64::MAX;
        for _ in 0..45 {
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
        println!("level{level}_ns={best:.3}");
    }
}
