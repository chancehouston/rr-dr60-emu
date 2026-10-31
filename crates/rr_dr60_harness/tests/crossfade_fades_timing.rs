//! Spec 019 FR-009 (tasks.md T018; research R-08): the playback's cost with a level change every
//! 200 ms (3 → 5 → 1 → 3, every change a 120-sample crossfade of two dots), head-only (it is
//! never copied into the baseline worktree); the per-sample cost reported, the worst
//! 4096-sample block asserted against the real-time budget (1000 × 4096 ÷ 48 = 85 333 µs). Run:
//! `cargo test -p rr_dr60_harness --release --test crossfade_fades_timing -- --ignored --nocapture`

use std::time::Instant;

use rr_dr60::{Pipeline, Route, Settings};

fn noise(rate: u32, seconds: usize) -> Vec<f32> {
    let mut state = 0x019u64;
    (0..seconds * rate as usize)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            ((state >> 40) as f32 / (1u64 << 23) as f32 - 1.0) * 0.1
        })
        .collect()
}

#[test]
#[ignore = "release-mode timing; run with --ignored"]
fn crossfade_fades_cost() {
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
    // The constant level 3's worst block in the same run (the relative bound; coach review 9).
    let mut worst_constant_us = 0.0f64;
    for _ in 0..5 {
        p.set_playback_speed(3).unwrap();
        p.start_playback(1).unwrap();
        loop {
            let tb = Instant::now();
            let info = p.play(&mut out).unwrap();
            worst_constant_us = worst_constant_us.max(tb.elapsed().as_nanos() as f64 / 1000.0);
            if info.finished {
                break;
            }
        }
    }
    let cycle = [3u8, 5, 1];
    let per_change = 200 * rate as usize / 1000; // 200 ms of host samples
    let mut best = f64::MAX;
    let mut worst_block_us = 0.0f64;
    for _ in 0..45 {
        p.set_playback_speed(3).unwrap();
        p.start_playback(1).unwrap();
        let (mut n, mut next_change, mut k) = (0usize, per_change, 1usize);
        let t = Instant::now();
        loop {
            if n >= next_change {
                p.set_playback_speed(cycle[k % 3]).unwrap();
                k += 1;
                next_change += per_change;
            }
            let tb = Instant::now();
            let info = p.play(&mut out).unwrap();
            worst_block_us = worst_block_us.max(tb.elapsed().as_nanos() as f64 / 1000.0);
            n += info.produced;
            if info.finished {
                break;
            }
        }
        best = best.min(t.elapsed().as_nanos() as f64 / n as f64);
    }
    println!("fades_ns={best:.3}");
    println!("worst_block_us={worst_block_us:.1}");
    println!("worst_constant_block_us={worst_constant_us:.1}");
    assert!(
        worst_block_us <= 85_333.0,
        "the worst block {worst_block_us:.1} µs exceeds the real-time budget (019 FR-009)"
    );
    assert!(
        worst_block_us <= 4.0 * worst_constant_us,
        "the worst block with fades {worst_block_us:.1} µs is more than four times the constant level's {worst_constant_us:.1} µs (019 FR-009, an engineering target)"
    );
}
