//! Latency: FR-012 for 6 rates × 5 configurations, FR-013 for 6 rates (SC-005; tasks.md T050).

use rr_dr60_harness::checks::{self, assert_all, standard};
use rr_dr60_harness::{configs, report};

#[test]
fn latency_matches_measurement_and_budget() {
    let mut results = Vec::new();
    for rate in configs::all_rates() {
        for config in configs::CONFIGS {
            results.extend(checks::check_fr012_latency(rate, config, &standard));
        }
        results.extend(checks::check_fr013_latency(rate, &standard));
    }
    report::print(&results);
    assert_all(&results);
}
