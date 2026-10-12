//! Spec 006 FR-022: the record and playback paths' measurement matrix at the seven host rates
//! (SC-002 – SC-008; task T038), every `split_checks` check in one report. The story tests
//! (`us1_record_play`, `us2_memory`, `us3_export_import`) run the same checks per story; this
//! file is the single `-- --nocapture` report. The 7 × 7 cross-rate grid runs in the release
//! job only (`--ignored`); the default-capacity case lives in `us2_memory` (`--ignored`).
//!
//! `cargo test -p rr_dr60_harness --test split_matrix -- --nocapture`

use rr_dr60_harness::checks::{MeasurementResult, assert_all};
use rr_dr60_harness::{configs, report, split_checks};

type Check = (&'static str, fn(u32) -> Vec<MeasurementResult>);

/// Every per-rate check of research R-08.
const CHECKS: &[Check] = &[
    ("golden_equivalence", split_checks::golden_equivalence),
    ("live_equivalence", split_checks::live_equivalence),
    ("length_rule", split_checks::length_rule),
    ("route_identity", split_checks::route_identity),
    ("two_items", split_checks::two_items),
    ("state_table", split_checks::state_table),
    ("drain_level", split_checks::drain_level),
    ("latency_sum", split_checks::latency_sum),
    ("capacity_time", split_checks::capacity_time),
    ("capacity_vas", split_checks::capacity_vas),
    ("capacity_count", split_checks::capacity_count),
    ("erase_renumber", split_checks::erase_renumber),
    ("empty_discarded", split_checks::empty_discarded),
    ("expanded_capacity", split_checks::expanded_capacity),
    ("round_trip", split_checks::round_trip),
    ("corruption_set", split_checks::corruption_set),
];

/// The full matrix: every check at the seven rates, the 48 → 44.1 kHz cross-rate case and
/// the footprint; one report, every row within tolerance.
#[test]
fn split_matrix() {
    let mut results = Vec::new();
    let mut counts = Vec::new();
    for (name, check) in CHECKS {
        let before = results.len();
        for rate in configs::all_rates() {
            results.extend(check(rate));
        }
        counts.push((*name, results.len() - before));
    }
    let before = results.len();
    results.extend(split_checks::cross_rate_tone(48_000, 44_100));
    counts.push(("cross_rate_tone 48k -> 44.1k", results.len() - before));
    let before = results.len();
    results.extend(split_checks::footprint());
    counts.push(("footprint", results.len() - before));
    report::print(&results);
    for (name, n) in &counts {
        println!("{name:>32}: {n} rows");
    }
    println!("split matrix: {} rows at seven rates", results.len());
    assert_all(&results);
}

/// US1 scenario 5 at every (record rate, playback rate) pair; release only.
#[test]
#[ignore = "49 five-second recordings; run with --release -- --ignored"]
fn cross_rate_grid() {
    let mut results = Vec::new();
    for record_rate in configs::all_rates() {
        for play_rate in configs::all_rates() {
            results.extend(split_checks::cross_rate_tone(record_rate, play_rate));
        }
    }
    report::print(&results);
    assert_eq!(results.len(), 49 * 3);
    assert_all(&results);
}
