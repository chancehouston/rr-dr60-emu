//! Spec 020 FR-008 (tasks.md T024): the per-sample cost of the record path at the default, of
//! the one-pass and of playback at constant levels 3 and 5, written against **version 0.19's
//! public API only**, so `scripts/ab-timing.sh <rev> record_contract` can copy this file into a
//! 0.19 worktree and compare the two builds (each within 3 %;
//! `RR_DR60_RECORD_CONTRACT_COST_CEILING`, 0.05 on the CI runner). Ignored by default (a
//! release-mode timing; never under coverage). Run:
//! `cargo test -p rr_dr60_harness --release --test record_contract_timing -- --ignored --nocapture`

use std::time::Instant;

use rr_dr60::{Pipeline, Route, Settings};

fn noise(rate: u32, seconds: usize) -> Vec<f32> {
    let mut state = 0x020u64;
    (0..seconds * rate as usize)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            ((state >> 40) as f32 / (1u64 << 23) as f32 - 1.0) * 0.1
        })
        .collect()
}

/// The figures the A/B script reads: `record_ns=`, `onepass_ns=`, `level3_ns=` and
/// `level5_ns=` (20 s of −20 dB noise at 48 kHz, the codec bypassed, 4096-sample blocks, the
/// fastest of 45).
#[test]
#[ignore = "release-mode timing; run with --ignored"]
fn record_contract_cost() {
    if cfg!(debug_assertions) {
        eprintln!("timing tests need --release; skipped");
        return;
    }
    let rate = 48_000u32;
    let x = noise(rate, 20);
    let mut s = Settings::new(rate);
    s.speech_codec_enabled = false;
    s.memory.capacity_s = 30;
    let mut p = Pipeline::new(s).unwrap();
    p.set_route(Route::Speaker);
    let mut best = f64::MAX;
    for _ in 0..45 {
        p.start_recording().unwrap();
        let t = Instant::now();
        for chunk in x.chunks(4096) {
            let _ = p.record(chunk, &mut []).unwrap();
        }
        let _ = p.stop_recording(&mut []).unwrap();
        best = best.min(t.elapsed().as_secs_f64() * 1e9 / x.len() as f64);
        let _ = p.erase_all();
    }
    println!("record_ns={best:.3}");
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
            best = best.min(t.elapsed().as_secs_f64() * 1e9 / n as f64);
        }
        println!("level{level}_ns={best:.3}");
    }
}
