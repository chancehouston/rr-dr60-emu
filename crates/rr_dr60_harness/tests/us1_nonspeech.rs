//! Spec 029 US1 [TR: D6 iii, iv; FR-006]: the silence rows and the coder's periodicity. The
//! check bodies live in `nonspeech_checks::d6_*` so the bless gate asserts the same rows (D1).

use rr_dr60_harness::checks::assert_all;
use rr_dr60_harness::nonspeech_checks as ns;
use rr_dr60_harness::report;

fn run(results: Vec<rr_dr60_harness::checks::MeasurementResult>) {
    report::print(&results);
    for l in ns::summary(&results) {
        println!("{l}");
    }
    assert_all(&results);
}

/// D6 (iii): digital silence through the ladder (C6's open gate against the nominal 575).
#[test]
fn nonspeech_silence() {
    run(ns::d6_silence(ns::check_rates()));
}

/// D6 (iv): the coder's periodicity, C3 against C1, on the steady and modulated rows; the bursts
/// and pink rows, the level change and the share ratio reported.
#[test]
fn nonspeech_coder_periodicity() {
    run(ns::d6_coder_periodicity(ns::check_rates()));
}
