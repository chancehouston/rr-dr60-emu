//! Spec 017 FR-009 (tasks.md T019; research R-11): the record path's per-sample cost with one
//! knee change scheduled per 4096-sample block, and the worst block, head-only (it uses the
//! record-side schedule, so it is never copied into the baseline worktree); reported by
//! `scripts/ab-timing.sh <rev> record`, not budgeted. Run:
//! `cargo test -p rr_dr60_harness --release --test record_pending_timing -- --ignored --nocapture`

use std::time::Instant;

use rr_dr60::{MicSettings, Pipeline, Settings, Stream};

fn noise(rate: u32, seconds: usize) -> Vec<f32> {
    let mut state = 0x017u64;
    (0..seconds * rate as usize)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            ((state >> 40) as f32 / (1u64 << 23) as f32 - 1.0) * 0.1
        })
        .collect()
}

/// The figures the A/B script reports: `record_pending_ns=` and `worst_block_us=`.
#[test]
#[ignore = "release-mode timing; run with --ignored"]
fn record_pending_cost() {
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
    let knee = rr_dr60::controls()
        .iter()
        .position(|e| e.identifier == "agc.knee_dbfs")
        .unwrap() as u32;
    let mut best = f64::MAX;
    let mut worst_block_us = 0.0f64;
    for _ in 0..45 {
        p.start_recording().unwrap();
        let t = Instant::now();
        for (k, chunk) in x.chunks(4096).enumerate() {
            // One change inside every block (the pending compare is live on every sample and
            // the limiter's derivation runs once per block).
            let pos = p.input_position(Stream::Record).unwrap() + 2000;
            let v = if k % 2 == 0 { -5.0 } else { -1.0 };
            p.schedule_change(Stream::Record, pos, knee, v, k as u64 + 1)
                .unwrap();
            let tb = Instant::now();
            let _ = p.record(chunk, &mut []).unwrap();
            worst_block_us = worst_block_us.max(tb.elapsed().as_nanos() as f64 / 1000.0);
        }
        best = best.min(t.elapsed().as_nanos() as f64 / x.len() as f64);
        let _ = p.stop_recording(&mut []).unwrap();
        p.erase_all().unwrap();
    }
    println!("record_pending_ns={best:.3}");
    println!("worst_block_us={worst_block_us:.1}");
}
