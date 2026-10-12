//! Spec 006 User Story 1: record, stop, play (FR-003 – FR-008, FR-015, FR-016; SC-002, SC-005,
//! SC-008; tasks T021, T025). One engine per case; no import; the memory at its default
//! capacity.
//!
//! `cargo test -p rr_dr60_harness --test us1_record_play -- --nocapture` prints the report.

use rr_dr60_harness::checks::assert_all;
use rr_dr60_harness::{configs, golden, report, split_checks};

/// SC-002: 196 of 217 golden entries reproduced bit-exactly by record-then-play at the seven
/// rates, with the VAS events and the paused state.
#[test]
fn golden_entries_are_reproduced_by_record_then_play() {
    let applicable = golden::applicable_entries();
    assert_eq!(applicable.len(), 196);
    let mut results = Vec::new();
    for rate in configs::all_rates() {
        results.extend(split_checks::golden_equivalence(rate));
    }
    report::print(&results);
    let entries = results
        .iter()
        .filter(|r| r.property == "golden entry reproduced by record-then-play")
        .count();
    println!(
        "golden entries reproduced by record-then-play: {entries} of 217 (21 agc_only excluded)"
    );
    assert_eq!(entries, 196);
    assert_all(&results);
}

/// FR-007, FR-018: record-then-play equals the one-pass prefix for the live stimulus set in two
/// partitions, with the playback length exactly ⌈K·m/l⌉.
#[test]
fn live_equivalence_at_all_rates() {
    let mut results = Vec::new();
    for rate in configs::all_rates() {
        results.extend(split_checks::live_equivalence(rate));
    }
    report::print(&results);
    assert_all(&results);
}

/// FR-007 (B3): K = 1 … 50 at the seven rates.
#[test]
fn length_rule_at_all_rates() {
    let mut results = Vec::new();
    for rate in configs::all_rates() {
        results.extend(split_checks::length_rule(rate));
    }
    report::print(&results);
    assert_eq!(results.len(), 7 * 50);
    assert_all(&results);
}

/// FR-015, FR-004: the route does not change the sound; item 2 after a loud item 1 starts from
/// the reset state.
#[test]
fn route_identity_and_two_items() {
    let mut results = Vec::new();
    for rate in configs::all_rates() {
        results.extend(split_checks::route_identity(rate));
        results.extend(split_checks::two_items(rate));
    }
    report::print(&results);
    assert_all(&results);
}

/// US1 scenario 5: recorded at 48 kHz, played at 44.1 kHz, within ±0.1 dB and ±0.1 Hz of the
/// one-pass at 44.1 kHz.
#[test]
fn cross_rate_tone_48k_to_44k1() {
    let results = split_checks::cross_rate_tone(48_000, 44_100);
    report::print(&results);
    assert_all(&results);
}

/// FR-008: the drain's length, its end and its level at the seven rates.
#[test]
fn drain_level_at_all_rates() {
    let mut results = Vec::new();
    for rate in configs::all_rates() {
        results.extend(split_checks::drain_level(rate));
    }
    report::print(&results);
    assert_all(&results);
}

/// SC-008: the latency sum and the playback onset at the seven rates.
#[test]
fn latency_sum_at_all_rates() {
    let mut results = Vec::new();
    for rate in configs::all_rates() {
        results.extend(split_checks::latency_sum(rate));
    }
    report::print(&results);
    assert_all(&results);
}

/// SC-005: the state table at the seven rates.
#[test]
fn state_table_at_all_rates() {
    let mut results = Vec::new();
    for rate in configs::all_rates() {
        results.extend(split_checks::state_table(rate));
    }
    report::print(&results);
    assert_all(&results);
}
