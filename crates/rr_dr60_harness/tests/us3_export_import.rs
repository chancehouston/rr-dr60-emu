//! Spec 006 User Story 3: recordings that survive the session and can be shared (FR-014;
//! SC-004; task T034). Export → import into a fresh engine at each of the seven rates, the
//! corruption set, and the two mutants that prove the checks have teeth.
//!
//! `cargo test -p rr_dr60_harness --test us3_export_import -- --nocapture` prints the report.

use rr_dr60_harness::checks::{MeasurementResult, assert_all};
use rr_dr60_harness::{configs, report, split_checks};

fn at_all_rates(check: fn(u32) -> Vec<MeasurementResult>) -> Vec<MeasurementResult> {
    let mut results = Vec::new();
    for rate in configs::all_rates() {
        results.extend(check(rate));
    }
    report::print(&results);
    results
}

/// US3 scenarios 1 and 2 (SC-004): the round trip at 7 × 7 rates.
#[test]
fn round_trip() {
    let results = at_all_rates(split_checks::round_trip);
    assert_eq!(
        results
            .iter()
            .filter(|r| r.property == "imported playback bit-identical to the original's")
            .count(),
        49
    );
    assert_all(&results);
}

/// US3 scenarios 3 and 4 (SC-004): the corruption set.
#[test]
fn corruption_set() {
    let results = at_all_rates(split_checks::corruption_set);
    assert_all(&results);
}

fn failed(results: &[MeasurementResult]) -> Vec<&'static str> {
    let mut v: Vec<_> = results
        .iter()
        .filter(|r| !r.passed)
        .map(|r| r.property)
        .collect();
    v.dedup();
    v
}

/// SC-004's teeth: a flipped CRC bit is refused at import, so the round trip fails.
#[test]
fn crc_mutant_is_caught() {
    let results = split_checks::round_trip_with(48_000, split_checks::mutants::flip_crc_bit);
    let failed = failed(&results);
    assert!(
        failed.contains(&"import accepted into a fresh engine as number 1"),
        "{failed:?}"
    );
}

/// SC-002's teeth: a recording missing its first sample imports but fails the length and the
/// bit-identical checks.
#[test]
fn drop_first_sample_mutant_is_caught() {
    let results = split_checks::round_trip_with(48_000, split_checks::mutants::drop_first_sample);
    let failed = failed(&results);
    assert!(
        failed.contains(&"imported length minus the original's (unchanged by the rate)")
            && failed.contains(&"imported playback bit-identical to the original's"),
        "{failed:?}"
    );
}
