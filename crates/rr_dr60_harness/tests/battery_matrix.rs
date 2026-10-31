//! Spec 018 seven-rate report (tasks.md T008): the trajectory rows (the peak bit-exact, the dB
//! printed), the law points, the knee-table shift at 44.1 kHz and above, the clamp row on a
//! full-scale square at three values, the setter oracle and the below-knee identity. One table
//! at `configs::all_rates()`. Report:
//! `cargo test -p rr_dr60_harness --test battery_matrix -- --nocapture`.

use rr_dr60_harness::battery_checks as bat;
use rr_dr60_harness::checks::{MeasurementResult, assert_all, standard};
use rr_dr60_harness::{configs, report};

fn finish(results: Vec<MeasurementResult>) {
    report::print(&results);
    assert!(!results.is_empty(), "no measurements");
    assert_all(&results);
}

/// FR-003, FR-006: the trajectory and the law points at every rate.
#[test]
fn trajectory_at_every_rate() {
    let mut rows = Vec::new();
    for rate in configs::all_rates() {
        rows.extend(bat::trajectory_rows(&standard, rate));
        rows.extend(bat::law_point_peaks(&standard, rate));
    }
    finish(rows);
}

/// FR-003: the knee table shifted by 20·log10(b) at 44.1 kHz and above, at 2.8 and 2.2 V.
#[test]
fn knee_shift_at_the_high_rates() {
    let mut rows = Vec::new();
    for rate in configs::all_rates().into_iter().filter(|&r| r >= 44_100) {
        for volts in [2.8f32, 2.2] {
            rows.extend(bat::knee_table_shift(&standard, rate, volts));
        }
    }
    finish(rows);
}

/// D5: the clamp row on a full-scale square at position 1 at 3.0, 2.6 and 2.2 V.
#[test]
fn clamp_row_at_every_rate() {
    let mut rows = Vec::new();
    for rate in configs::all_rates() {
        rows.extend(bat::clamp_row(&standard, rate));
    }
    finish(rows);
}

/// FR-002, FR-001: the below-knee identity and the setter oracle at every rate.
#[test]
fn identity_and_setter_oracle_at_every_rate() {
    let mut rows = Vec::new();
    for rate in configs::all_rates() {
        rows.extend(bat::below_knee_identity(&standard, rate));
        rows.extend(bat::setter_oracle(&standard, rate, 2.4));
    }
    finish(rows);
}

/// FR-001 – FR-003 on the playback stream (coach review 3): the recorded clipped tone played at
/// a tired value, and a change scheduled on the playback stream, at every rate.
#[test]
fn playback_stream_at_every_rate() {
    let mut rows = Vec::new();
    for rate in configs::all_rates() {
        rows.extend(bat::playback_rows(&standard, rate, 2.4));
    }
    finish(rows);
}

/// Convergence T027, T028: the playback stream's below-knee identity (SC-001 "both streams")
/// and the clip count against its twin at the five law points, at every rate (FR-010, SC-004).
#[test]
fn playback_identity_and_clip_count_at_every_rate() {
    let mut rows = Vec::new();
    for rate in configs::all_rates() {
        rows.extend(bat::playback_below_knee_identity(&standard, rate));
        rows.extend(bat::clip_count_twin(&standard, rate));
    }
    finish(rows);
}
