//! Full response matrix: FR-005, FR-010 (both stages, plus phase) and FR-011 at all six host
//! rates (FR-019, FR-020, SC-002; tasks.md T049). Run with `--nocapture` to see the report.

use rr_dr60_harness::checks::{self, Stage, assert_all, standard};
use rr_dr60_harness::{configs, report};

#[test]
fn response_matrix_meets_spec() {
    let mut results = Vec::new();
    for rate in configs::all_rates() {
        results.extend(checks::check_fr005_boundary(rate, &standard));
        results.extend(checks::check_fr010_stage(rate, Stage::Record, &standard));
        results.extend(checks::check_fr010_stage(rate, Stage::Playback, &standard));
        results.extend(checks::check_fr011_cascade(rate, &standard));
    }
    results.extend(checks::check_fr010_minphase(&standard));
    report::print(&results);
    assert_all(&results);
}
