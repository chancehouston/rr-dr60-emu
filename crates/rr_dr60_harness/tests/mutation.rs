//! SC-008: a deliberate 100 Hz shift of the voice-band edges is caught by both the response
//! check and the golden check (tasks.md T057).

#[path = "fixtures/shifted_100hz_sos.rs"]
mod shifted;

use rr_dr60::{Pipeline, Settings};
use rr_dr60_harness::checks::{self, Stage};
use rr_dr60_harness::golden;

fn mutant(s: Settings) -> Pipeline {
    Pipeline::__with_stage_sos(s, shifted::SHIFTED_SOS).unwrap()
}

#[test]
fn response_check_catches_shifted_band_edges() {
    let results = checks::check_fr010_stage(48_000, Stage::Record, &mutant);
    let failed: Vec<_> = results
        .iter()
        .filter(|r| !r.passed)
        .map(|r| r.property)
        .collect();
    assert!(
        failed.contains(&"upper -3 dB point"),
        "upper edge not caught; failures: {failed:?}"
    );
    assert!(
        failed.contains(&"lower -3 dB point"),
        "lower edge not caught; failures: {failed:?}"
    );
}

#[test]
fn golden_check_catches_shifted_band_edges() {
    let expected = golden::committed();
    let actual = golden::generate(&mutant);
    let report = golden::compare(&expected, &actual).expect_err("mutant matched the golden file");
    assert!(
        report.contains("sweep_log/default/48000 Hz differs"),
        "{report}"
    );
}
