//! Spec 019 seven-rate report (tasks.md T009): the blend identity and the splice per pair, the
//! partitions and the lengths at `configs::all_rates()`; at 6 kHz the click table under both
//! readers with the floor, the voiced energies and the end cut. Report:
//! `cargo test -p rr_dr60_harness --test crossfade_matrix -- --nocapture`.

use rr_dr60_harness::checks::{MeasurementResult, assert_all, standard};
use rr_dr60_harness::crossfade_checks as xf;
use rr_dr60_harness::{configs, report};

fn finish(results: Vec<MeasurementResult>) {
    report::print(&results);
    assert!(!results.is_empty(), "no measurements");
    assert_all(&results);
}

/// FR-001 at every rate and pair.
#[test]
fn blend_and_splice_at_every_rate() {
    let mut rows = Vec::new();
    for rate in configs::all_rates() {
        for pair in xf::PAIRS {
            rows.extend(xf::blend_identity(&standard, rate, pair));
        }
    }
    finish(rows);
}

/// FR-006, FR-008 at every rate.
#[test]
fn partitions_and_lengths_at_every_rate() {
    let mut rows = Vec::new();
    for rate in configs::all_rates() {
        rows.extend(xf::partitions(&standard, rate));
        rows.extend(xf::lengths_unchanged(&standard, rate));
        rows.extend(xf::wait_cases(&standard, rate));
    }
    finish(rows);
}

/// FR-007 at 6 kHz: both readers, the floor, the script, the voiced energies.
#[test]
fn click_table_at_6k() {
    let mut rows = xf::click_table(&standard);
    rows.extend(xf::click_table_against_the_script(&standard));
    rows.extend(xf::voiced_click(&standard));
    finish(rows);
}

/// FR-006 at 6 kHz: the end cut in both configurations.
#[test]
fn end_cut_at_6k() {
    finish(xf::end_cut(&standard));
}
