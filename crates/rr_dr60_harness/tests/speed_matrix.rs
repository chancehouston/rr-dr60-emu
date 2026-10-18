//! Spec 013: the seven-rate report (tasks.md T015; data-model.md › Report rows).
//!
//! Per host rate and level: the playback's length against ⌈(⌈5K ÷ k⌉ + T) · m ÷ l⌉ + the drain
//! and the length helper (FR-005, FR-009), in the speed-only configuration and the default
//! engine; and FR-004's other-rate level comparison. Asserted where the spec asserts.

use rr_dr60_harness::checks::{assert_all, standard};
use rr_dr60_harness::configs::all_rates;
use rr_dr60_harness::report;
use rr_dr60_harness::speed_checks as sc;

#[test]
fn speed_matrix_lengths() {
    let mut results = Vec::new();
    for rate in all_rates() {
        results.extend(sc::lengths(&standard, rate, "speed_only"));
        results.extend(sc::lengths(&standard, rate, "speed_default"));
        results.extend(sc::lengths(&standard, rate, "speed_default_no_codec"));
    }
    report::print(&results);
    assert_all(&results);
}

#[test]
fn speed_matrix_other_rate_levels() {
    let mut results = Vec::new();
    for rate in all_rates() {
        if rate != 6000 {
            results.extend(sc::band_other_rate(&standard, rate));
        }
    }
    report::print(&results);
    assert_all(&results);
}
