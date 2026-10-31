//! Spec 019 User Story 2: the change is bounded and described (tasks.md T011; spec › US2 AS1 –
//! AS2; AS3 is the A/B script, AS4 `ffi_parity speed`). Report:
//! `cargo test -p rr_dr60_harness --test us2_crossfade -- --nocapture`.

use rr_dr60::manifest;
use rr_dr60_harness::checks::{MeasurementResult, assert_all, standard};
use rr_dr60_harness::crossfade_checks as xf;
use rr_dr60_harness::{configs, manifest_checks, report};

fn finish(results: Vec<MeasurementResult>) {
    report::print(&results);
    assert!(!results.is_empty(), "no measurements");
    assert_all(&results);
}

/// US2 AS1 (FR-005): the wait cases against the reference at the measurement rates.
#[test]
fn as1_wait_cases() {
    finish(
        configs::measurement_rates()
            .iter()
            .flat_map(|&r| xf::wait_cases(&standard, r))
            .collect(),
    );
}

/// US2 AS2 (FR-010): stage 9's description names the blend and its coverage note the sound of
/// a level change with A-054, at model version 0.19.0; the control manifest is unchanged (its
/// canonical golden holds); A-054's register row carries the pinned phrases.
#[test]
fn as2_manifest_and_register() {
    let m = manifest();
    let s9 = m.stage(9).unwrap();
    assert!(
        s9.description.contains("blends over 20 ms"),
        "stage 9's description: {}",
        s9.description
    );
    assert!(s9.description.contains("engine convention"));
    assert!(
        s9.coverage.contains("the sound of a level change") && s9.coverage.contains("A-054"),
        "stage 9's coverage: {}",
        s9.coverage
    );
    assert_eq!(s9.model_version, "0.19.0");
    assert_eq!(
        rr_dr60::controls().len(),
        30,
        "the control manifest is unchanged"
    );
    let text =
        std::fs::read_to_string(manifest_checks::repo_root().join("docs/hardware/assumptions.md"))
            .unwrap();
    let row = text
        .lines()
        .find(|l| l.starts_with("| A-054 |"))
        .expect("A-054's row");
    let lower = row.to_lowercase();
    for phrase in ["engine convention", "capture p1"] {
        assert!(lower.contains(phrase), "A-054 lacks {phrase:?}");
    }
    assert_eq!(rr_dr60::speed::CROSSFADE_SAMPLES, 120);
    assert_eq!(rr_dr60::speed::CROSSFADE_MS, 20);
}
