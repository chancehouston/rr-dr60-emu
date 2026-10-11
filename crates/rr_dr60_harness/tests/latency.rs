//! Latency (spec 004 FR-004, SC-005; tasks.md T026): at all seven host rates the reported
//! latency equals the measured 1 kHz group delay within ±1 sample for the five spec 001
//! configurations, and the default configuration stays within the 20 ms budget. Prints a
//! `rate → samples (ms)` table for the CHANGELOG (T037).

use rr_dr60::Pipeline;
use rr_dr60_harness::checks::{self, assert_all, standard};
use rr_dr60_harness::{configs, report};

#[test]
fn latency_matches_measurement_and_budget() {
    let mut results = Vec::new();
    eprintln!("default latency (spec 004 FR-004):");
    for rate in configs::all_rates() {
        let samples = Pipeline::new(configs::settings("default", rate))
            .unwrap()
            .latency_samples();
        eprintln!(
            "  {rate:>6} Hz -> {samples:>5} samples ({:.2} ms)",
            f64::from(samples) / f64::from(rate) * 1000.0
        );
        results.extend(checks::check_fr004_latency(rate, &standard));
    }
    report::print(&results);
    assert_all(&results);
    // SC-005: every configuration at every rate was measured (5 + 1 rows per rate).
    assert_eq!(
        results.len(),
        configs::all_rates().len() * (configs::CONFIGS.len() + 1)
    );
}
