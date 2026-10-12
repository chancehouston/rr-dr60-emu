//! Spec 006 User Story 2: a memory that fills and empties like the device's (FR-010 – FR-013,
//! FR-017; SC-003, SC-007; task T028). The record path only, at a 10 s / 3-recording capacity;
//! the default 60-minute case and the recording past 60 minutes run in the release job only
//! (`--ignored`; plan review P9).
//!
//! `cargo test -p rr_dr60_harness --test us2_memory -- --nocapture` prints the report.

use rr_dr60_harness::checks::assert_all;
use rr_dr60_harness::{configs, report, split_checks};

fn at_all_rates(check: fn(u32) -> Vec<rr_dr60_harness::checks::MeasurementResult>) {
    let mut results = Vec::new();
    for rate in configs::all_rates() {
        results.extend(check(rate));
    }
    report::print(&results);
    assert_all(&results);
}

/// US2 scenarios 2 and 6 (FR-012, SC-003): the time limit at the exact sample.
#[test]
fn capacity_time() {
    at_all_rates(split_checks::capacity_time);
}

/// US2 scenario 2 at the device's own capacity (S-001: 60 minutes), 6 kHz; release only.
#[test]
#[ignore = "60 minutes of audio; run with --release -- --ignored"]
fn capacity_time_default_6k() {
    let results = split_checks::capacity_time_at(6000, 3600, 99, 3601.0);
    report::print(&results);
    assert_all(&results);
}

/// US2 scenario 3 (FR-011, FR-012): paused time costs nothing in drop mode and memory in mute
/// mode.
#[test]
fn capacity_vas() {
    at_all_rates(split_checks::capacity_vas);
}

/// US2 scenario 4 (FR-012): the count limit.
#[test]
fn capacity_count() {
    at_all_rates(split_checks::capacity_count);
}

/// US2 scenario 5 (FR-013): erase and renumbering.
#[test]
fn erase_renumber() {
    at_all_rates(split_checks::erase_renumber);
}

/// US2 scenario 7 (FR-012, A-039): the empty recording and the silent one.
#[test]
fn empty_discarded() {
    at_all_rates(split_checks::empty_discarded);
}

/// US2 scenario 8 (FR-010): a capacity above the device's.
#[test]
fn expanded_capacity() {
    at_all_rates(split_checks::expanded_capacity);
}

/// US2 scenario 8 (FR-010): recording past 60 minutes; release only.
#[test]
#[ignore = "61 minutes of audio; run with --release -- --ignored"]
fn expanded_capacity_past_device() {
    let results = split_checks::expanded_capacity_past_device();
    report::print(&results);
    assert_all(&results);
}

/// SC-007 (FR-017): the footprint and its invariance.
#[test]
fn footprint() {
    let results = split_checks::footprint();
    report::print(&results);
    assert_all(&results);
}
