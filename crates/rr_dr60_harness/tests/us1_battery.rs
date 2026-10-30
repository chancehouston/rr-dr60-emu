//! Spec 018 User Story 1: the speaker's clip ceiling follows the battery value (tasks.md
//! T007; spec › US1 AS1 – AS5). One test per acceptance scenario at the measurement rates,
//! each asserting every row passes. Report:
//! `cargo test -p rr_dr60_harness --test us1_battery -- --nocapture`.

use rr_dr60_harness::battery_checks as bat;
use rr_dr60_harness::checks::{MeasurementResult, assert_all, standard};
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

/// US1 AS1 (FR-005): at the default nothing moves, on both routes, over the sampled
/// configurations; the scheduled no-op is logged.
#[test]
fn as1_default_is_bit_identical() {
    finish(each(|r| bat::default_identity(&standard, r)));
}

/// US1 AS2 (FR-002): below the knee the output is bit-identical at every law point, the
/// premise asserted.
#[test]
fn as2_below_knee_identity() {
    finish(each(|r| bat::below_knee_identity(&standard, r)));
}

/// US1 AS3 (FR-002, FR-006): the hard-clipped tone's settled peak equals the factor to the bit
/// at the seven trajectory rows and the six law points; the knee table shifts by 20·log10(b)
/// and the 10 % THD point with it at 44.1 kHz and above.
#[test]
fn as3_trajectory_and_knee_shift() {
    finish(each(|r| {
        let mut rows = bat::trajectory_rows(&standard, r);
        rows.extend(bat::law_point_peaks(&standard, r));
        rows.extend(bat::reference_position_rows(&standard, r));
        if r >= 44_100 {
            for volts in [2.6f32, 2.2] {
                rows.extend(bat::knee_table_shift(&standard, r, volts));
            }
        }
        rows
    }));
}

/// US1 AS4 (FR-003, FR-001): a scheduled change is one transition at any partition (block sizes
/// 1, 64 and 4096 and the irregular partitions): the splice of the two constant runs, the click
/// energy at least 30 dB below the instantaneous step, the factor trace along the smoothstep,
/// the log entry; the earphone route logs it without effect.
#[test]
fn as4_transition_and_partitions() {
    finish(each(|r| {
        let mut rows = bat::transition_rows(&standard, r, 2.4);
        rows.extend(bat::trace_rows(&standard, r, 2.4));
        rows.extend(bat::trace_rows(&standard, r, 2.2));
        rows
    }));
}

/// US1 AS5 (FR-004): the earphone route and a recording's stored samples are unmoved at every
/// law point.
#[test]
fn as5_other_paths_unmoved() {
    finish(each(|r| bat::other_paths(&standard, r)));
}
