//! Spec 021 FR-010 (tasks.md T020): the per-sample cost of the all-on state with no landing on
//! the player (level 3) and on the one-pass, written against **version 0.20's public API only**,
//! so `scripts/ab-timing.sh <rev> bypass` can copy this file into a 0.20 worktree and compare
//! the two builds (each within 3 %; `RR_DR60_BYPASS_COST_CEILING`, 0.05 on the CI runner). The
//! head-only figures (the bypassed steady state, a blend every 200 ms, the worst block) are
//! printed by `bypass_states_timing` in the working tree. Ignored by default (a release-mode
//! timing; never under coverage). Run:
//! `cargo test -p rr_dr60_harness --release --test bypass_timing -- --ignored --nocapture --test-threads=1`

use std::time::Instant;

use rr_dr60::{Pipeline, Route, Settings};

fn noise(rate: u32, seconds: usize) -> Vec<f32> {
    let mut state = 0x021u64;
    (0..seconds * rate as usize)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            ((state >> 40) as f32 / (1u64 << 23) as f32 - 1.0) * 0.1
        })
        .collect()
}

fn settings(rate: u32) -> Settings {
    let mut s = Settings::new(rate);
    s.speech_codec_enabled = false;
    s.memory.capacity_s = 30;
    s
}

/// The figures the A/B script reads: `playback_ns=` and `onepass_ns=` (20 s of −20 dB noise at
/// 48 kHz, the codec bypassed, the speaker route, 4096-sample blocks, the fastest of 45).
#[test]
#[ignore = "release-mode timing; run with --ignored"]
fn bypass_cost() {
    if cfg!(debug_assertions) {
        eprintln!("timing tests need --release; skipped");
        return;
    }
    let rate = 48_000u32;
    let x = noise(rate, 20);
    let mut p = Pipeline::new(settings(rate)).unwrap();
    p.set_route(Route::Speaker);
    let mut out = vec![0.0f32; 4096];
    let mut best = f64::MAX;
    for _ in 0..45 {
        p.reset();
        let t = Instant::now();
        for chunk in x.chunks(4096) {
            let _ = p.process(chunk, &mut out[..chunk.len()]).unwrap();
        }
        best = best.min(t.elapsed().as_secs_f64() * 1e9 / x.len() as f64);
    }
    println!("onepass_ns={best:.3}");
    p.start_recording().unwrap();
    for chunk in x.chunks(4096) {
        let _ = p.record(chunk, &mut []).unwrap();
    }
    let _ = p.stop_recording(&mut []).unwrap();
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
        best = best.min(t.elapsed().as_secs_f64() * 1e9 / n as f64);
    }
    println!("playback_ns={best:.3}");
}
