//! Spec 015 FR-010 (tasks.md T018; analyze A1): the player's per-sample cost with one volume
//! change scheduled per 4096-sample block, head-only (it uses the schedule call, so it is never
//! copied into the baseline worktree); reported by `scripts/ab-timing.sh <rev> changes`, not
//! budgeted. Run:
//! `cargo test -p rr_dr60_harness --release --test changes_pending_timing -- --ignored --nocapture`

use std::time::Instant;

use rr_dr60::{MicSettings, Pipeline, Route, Settings, Stream};

fn noise(rate: u32, seconds: usize) -> Vec<f32> {
    let mut state = 0x015u64;
    (0..seconds * rate as usize)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            ((state >> 40) as f32 / (1u64 << 23) as f32 - 1.0) * 0.1
        })
        .collect()
}

/// The figure the A/B script reports: `playback_pending_ns=`.
#[test]
#[ignore = "release-mode timing; run with --ignored"]
fn changes_pending_cost() {
    if cfg!(debug_assertions) {
        eprintln!("timing tests need --release; skipped");
        return;
    }
    let rate = 48_000;
    let x = noise(rate, 10);
    let mut s = Settings::new(rate);
    s.mic = MicSettings::DEVICE;
    s.speech_codec_enabled = false;
    let mut p = Pipeline::new(s).unwrap();
    p.set_route(Route::Speaker);
    p.set_volume(1.0).unwrap();
    p.start_recording().unwrap();
    let _ = p.record(&x, &mut []).unwrap();
    let _ = p.stop_recording(&mut []).unwrap();
    let volume = rr_dr60::controls()
        .iter()
        .position(|e| e.identifier == "volume")
        .unwrap() as u32;
    let mut best = f64::MAX;
    let mut buf = vec![0.0f32; 4096];
    for _ in 0..45 {
        p.start_playback(1).unwrap();
        let mut produced = 0usize;
        let mut k = 0u64;
        let t = Instant::now();
        loop {
            // One change inside every block (the pending compare is live on every sample).
            let pos = p.stream_position(Stream::Playback).unwrap() + 2000;
            let v = if k % 2 == 0 { 0.95 } else { 1.0 };
            p.schedule_change(Stream::Playback, pos, volume, v, k + 1)
                .unwrap();
            k += 1;
            let info = p.play(&mut buf).unwrap();
            produced += info.produced;
            if info.finished {
                break;
            }
        }
        best = best.min(t.elapsed().as_nanos() as f64 / produced as f64);
    }
    println!("playback_pending_ns={best:.3}");
}
