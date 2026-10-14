//! Spec 008 User Story 2: a memory of frames: capacity, limits and the stored form (FR-010 –
//! FR-014; SC-007, SC-009; tasks T026, T028). Small configured capacities and the harness's
//! stimuli only: no C, no registers (Principle VII).
//!
//! `cargo test -p rr_dr60_harness --test us2_frames_memory -- --nocapture` prints every figure.

use rr_dr60_harness::checks::{MeasurementResult, assert_all};
use rr_dr60_harness::{codec_checks, configs, report};

fn at_all_rates(check: fn(u32) -> Vec<MeasurementResult>) -> Vec<MeasurementResult> {
    let mut results = Vec::new();
    for rate in configs::all_rates() {
        results.extend(check(rate));
    }
    report::print(&results);
    results
}

/// US2 scenario 1 (SC-007): the footprint with the codec on, its invariance, and spec 006's
/// figure when bypassed.
#[test]
fn footprint() {
    assert_all(&at_all_rates(codec_checks::footprint));
}

/// US2 scenario 2 (FR-013): the self-stop on a frame boundary at a 2 s capacity, the remaining
/// time exactly zero.
#[test]
fn capacity_frames() {
    assert_all(&at_all_rates(codec_checks::capacity_frames));
}

/// US2 scenario 3 (FR-013): the host's stop mid-frame pads to a whole frame and reports the
/// silence fed.
#[test]
fn mid_frame_stop() {
    assert_all(&at_all_rates(codec_checks::mid_frame_stop));
}

/// US2 scenario 4 (FR-010, SC-009): export → import into a fresh engine at each of the seven
/// rates, bit-identical playback, the form's fields read back; 7 × 7 round trips.
#[test]
fn round_trip() {
    let results = at_all_rates(codec_checks::round_trip);
    assert_eq!(
        results
            .iter()
            .filter(|r| r.property == "imported playback bit-identical to the original's")
            .count(),
        49
    );
    assert_all(&results);
}

/// US2 scenarios 5 and 6 (FR-010 – FR-012, SC-009): the corruption set, the version 1
/// sequence, the unsupported model version and the encoding mismatch, each refused with the
/// stated reason and the memory unchanged.
#[test]
fn corruption_set() {
    assert_all(&at_all_rates(codec_checks::corruption_set));
}

/// US2 scenario 7 (FR-014): VAS splice positions are unchanged by the codec and fall inside
/// frames.
#[test]
fn vas_splices_inside_frames() {
    assert_all(&at_all_rates(codec_checks::vas_splices_inside_frames));
}
