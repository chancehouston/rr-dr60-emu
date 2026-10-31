//! Spec 018 FR-007 (tasks.md T022; research R-08): the per-sample cost of the output chain at a
//! non-default battery value on clipped material, head-only (it uses the battery call, so it is
//! never copied into the baseline worktree); reported by `scripts/ab-timing.sh <rev> battery`,
//! not budgeted. Run:
//! `cargo test -p rr_dr60_harness --release --test battery_clipped_timing -- --ignored --nocapture`

use std::time::Instant;

use rr_dr60::{Pipeline, Route};
use rr_dr60_harness::{configs, stimulus};

/// The figures the A/B script reports: `clipped_ns=` at 2.4 V and `clipped_default_ns=` at 3.0 V
/// (the same stimulus: a full-scale square at position 1 through the default engine, the codec
/// bypassed, the speaker route).
#[test]
#[ignore = "release-mode timing; run with --ignored"]
fn battery_clipped_cost() {
    if cfg!(debug_assertions) {
        eprintln!("timing tests need --release; skipped");
        return;
    }
    let rate = 48_000;
    let x = stimulus::square(100.0, 1.0, f64::from(rate), 30 * rate as usize);
    let mut s = configs::settings("default_output", rate);
    s.speech_codec_enabled = false;
    let fastest = |volts: f32| {
        let mut best = f64::MAX;
        for _ in 0..45 {
            let mut p = Pipeline::new(s).unwrap();
            p.set_route(Route::Speaker);
            p.set_volume(1.0).unwrap();
            p.set_battery_volts(volts).unwrap();
            let mut buf = x.clone();
            let t = Instant::now();
            for chunk in buf.chunks_mut(4096) {
                let _ = p.process_in_place(chunk);
            }
            best = best.min(t.elapsed().as_nanos() as f64 / x.len() as f64);
        }
        best
    };
    println!("clipped_default_ns={:.3}", fastest(3.0));
    println!("clipped_ns={:.3}", fastest(2.4));
}
