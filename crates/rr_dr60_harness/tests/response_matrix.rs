//! Full response matrix (spec 004 SC-002; tasks.md T023, T031): both stage templates (FR-005,
//! FR-006, FR-008), the anti-alias requirement (FR-007) and the cascade (FR-009) at all seven
//! host rates, plus the minimum-phase check at the 6 kHz identity rate. Run with `--nocapture`
//! to see the report.

use rr_dr60_harness::checks::{self, Stage, assert_all, standard};
use rr_dr60_harness::{configs, report};

#[test]
fn response_matrix_meets_spec() {
    let mut results = Vec::new();
    for rate in configs::all_rates() {
        results.extend(checks::check_stage_template(rate, Stage::Record, &standard));
        results.extend(checks::check_stage_template(
            rate,
            Stage::Playback,
            &standard,
        ));
        results.extend(checks::check_fr007_antialias(rate, &standard));
        results.extend(checks::check_fr009_cascade(rate, &standard));
    }
    results.extend(checks::check_minphase(&standard));
    report::print(&results);
    eprintln!("response matrix: {} rows", results.len());
    assert_all(&results);
}
