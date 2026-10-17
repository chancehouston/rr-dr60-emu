//! Spec 012 User Story 2: very loud sounds overload the microphone amplifier (tasks.md T035;
//! spec › US2 AS1 – AS4, SC-003). One test per acceptance scenario at the measurement rates in
//! `mic_only_quiet` (stages 1 – 2 alone, the hiss off), each asserting every row passes.
//! Report: `cargo test -p rr_dr60_harness --test us2_mic -- --nocapture`.

use rr_dr60_harness::checks::{MeasurementResult, assert_all, standard};
use rr_dr60_harness::mic_checks as mic;
use rr_dr60_harness::{configs, report};

fn finish(results: Vec<MeasurementResult>) {
    report::print(&results);
    assert!(!results.is_empty(), "no measurements");
    assert_all(&results);
}

fn each(check: impl Fn(u32) -> Vec<MeasurementResult>) -> Vec<MeasurementResult> {
    configs::measurement_rates()
        .iter()
        .flat_map(|&r| check(r))
        .collect()
}

/// US2 AS1, SC-003: the static curve through the clipper probe, in clipper units.
#[test]
fn static_curve() {
    finish(each(|r| mic::static_curve(&standard, r)));
}

/// US2 AS1, SC-003: the onsets and limits at the engine input; the negative polarity first.
#[test]
fn engine_onsets_and_limits() {
    finish(each(|r| mic::engine_onsets(&standard, r)));
}

/// US2 AS2, SC-003: nothing after stage 2 exceeds L+ + L-.
#[test]
fn bound_after_stage_2() {
    finish(each(|r| mic::bound(&standard, r)));
}

/// US2 AS3: H2 and H3 against the script's discrete model.
#[test]
fn harmonics() {
    finish(each(|r| mic::harmonics(&standard, r)));
}

/// US2 AS4: memoryless recovery and the settle time.
#[test]
fn recovery() {
    finish(each(|r| mic::recovery(&standard, r)));
}
