//! Spec 011 User Story 1: click-free volume changes (tasks.md T013; spec › US1 AS1–AS8). One
//! test per acceptance scenario at the measurement rates, on both routes and in both
//! directions, each asserting every row passes. Report:
//! `cargo test -p rr_dr60_harness --test us1_volume_ramp -- --nocapture`.

use rr_dr60::__test_hooks::OutputMutation;
use rr_dr60::Pipeline;
use rr_dr60::Route;
use rr_dr60_harness::checks::{MeasurementResult, assert_all, standard};
use rr_dr60_harness::volume_ramp_checks as ramp;
use rr_dr60_harness::{configs, report};

fn finish(results: Vec<MeasurementResult>) {
    report::print(&results);
    assert!(!results.is_empty(), "no measurements");
    assert_all(&results);
}

/// Every measurement rate × stage-11-only configuration.
fn each(check: impl Fn(u32, &'static str) -> Vec<MeasurementResult>) -> Vec<MeasurementResult> {
    let mut rows = Vec::new();
    for &r in configs::measurement_rates() {
        for c in configs::STAGE11_CONFIGS {
            rows.extend(check(r, c));
        }
    }
    rows
}

const DIRECTIONS: [(f32, f32); 2] = [(0.1, 1.0), (1.0, 0.1)];

/// US1 AS1 (a), SC-001: the splice identity, the first and last differing samples.
#[test]
fn splice_identity() {
    finish(each(|r, c| {
        DIRECTIONS
            .iter()
            .flat_map(|&(a, b)| ramp::splice(&standard, r, c, a, b))
            .collect()
    }));
}

/// US1 AS1 (c): the ratio reader is monotonic within 4 ULP.
#[test]
fn ratio_reader() {
    finish(each(|r, c| {
        DIRECTIONS
            .iter()
            .flat_map(|&(a, b)| ramp::ratio_reader(&standard, r, c, a, b))
            .collect()
    }));
}

/// US1 AS2: the transition does not depend on the partition.
#[test]
fn partition_independence() {
    finish(each(|r, c| ramp::partitions(&standard, r, c)));
}

/// US1 AS3, SC-002 (FR-004): ≥ 30 dB below the instantaneous step and within ±0.25 dB of the
/// reference script.
#[test]
fn click_reduction() {
    finish(each(|r, c| {
        DIRECTIONS
            .iter()
            .flat_map(|&(a, b)| ramp::click_reduction(&standard, r, c, a, b))
            .collect()
    }));
}

/// US1 AS4: a full-scale tone on the speaker route never exceeds the ceiling.
#[test]
fn ceiling_full_scale() {
    finish(
        configs::measurement_rates()
            .iter()
            .flat_map(|&r| ramp::ceiling(&standard, r))
            .collect(),
    );
}

/// US1 AS5, SC-003: a retarget starts from the knob without a jump; the same target continues.
#[test]
fn retarget_and_same_target() {
    finish(each(|r, c| ramp::retarget(&standard, r, c)));
}

/// US1 AS6, FR-008: exact zeros from sample N; the route output decays below −120 dBFS.
#[test]
fn transition_to_zero() {
    finish(each(|r, c| ramp::zeros(&standard, r, c)));
}

/// US1 AS8: a ten-update drag is monotonic and ≥ 30 dB below the stepped drag.
#[test]
fn drag() {
    finish(each(|r, c| ramp::drag(&standard, r, c)));
}

/// US1 AS7 (reported): the route output against the test hooks' instantaneous step.
#[test]
fn route_output_reported() {
    let m = OutputMutation {
        volume_step: true,
        ..OutputMutation::default()
    };
    let step = move |s| Pipeline::__with_output_mutation(s, &m).unwrap();
    finish(
        configs::measurement_rates()
            .iter()
            .flat_map(|&r| {
                [Route::Speaker, Route::Earphone]
                    .into_iter()
                    .flat_map(move |route| ramp::route_output(&standard, &step, r, route))
                    .collect::<Vec<_>>()
            })
            .collect(),
    );
}
