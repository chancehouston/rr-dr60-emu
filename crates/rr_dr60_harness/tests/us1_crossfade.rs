//! Spec 019 User Story 1: change the speed without a tick (tasks.md T008; spec › US1 AS1 – AS6).
//! One test per acceptance scenario at the measurement rates, each asserting every row passes.
//! Report: `cargo test -p rr_dr60_harness --test us1_crossfade -- --nocapture`.

use rr_dr60_harness::checks::{MeasurementResult, assert_all, standard};
use rr_dr60_harness::crossfade_checks as xf;
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

/// US1 AS1 (FR-003): every constant level is bit-identical to the stepped build.
#[test]
fn as1_constant_levels_bit_identical() {
    finish(each(|r| xf::constant_levels(&standard, r)));
}

/// US1 AS2 (FR-001, FR-011): the blend identity and the splice per pair; the weights hook.
#[test]
fn as2_blend_identity_and_splice() {
    let mut rows = xf::weights_agree();
    rows.extend(each(|r| {
        xf::PAIRS
            .iter()
            .flat_map(|&pair| xf::blend_identity(&standard, r, pair))
            .collect()
    }));
    finish(rows);
}

/// US1 AS3 (FR-007): the click table at 6 kHz against the stepped build and the script; the
/// voiced energies.
#[test]
fn as3_click_table_and_voiced() {
    let mut rows = xf::click_table(&standard);
    rows.extend(xf::click_table_against_the_script(&standard));
    rows.extend(xf::voiced_click(&standard));
    finish(rows);
}

/// US1 AS4 (FR-008): the partitions and spec 015's setter oracle.
#[test]
fn as4_partitions() {
    finish(each(|r| xf::partitions(&standard, r)));
}

/// US1 AS5 (FR-004): level 3 without a branch: the blend into 3 starts at a whole place and a
/// constant level 3 is the stored samples (through the identity and the lengths).
#[test]
fn as5_level_3() {
    finish(each(|r| {
        let mut rows = xf::blend_identity(&standard, r, (1, 3));
        rows.extend(xf::blend_identity(&standard, r, (3, 1)));
        rows.extend(xf::lengths_unchanged(&standard, r));
        rows
    }));
}

/// US1 AS6 (FR-006): the end cut.
#[test]
fn as6_end_cut() {
    finish(xf::end_cut(&standard));
}
