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

/// Spec 002 SC-008: a deliberate change of the default release time by 50 %, or of the target
/// level by 3 dB, is caught by both a tolerance check and the AGC golden check. The mutants are
/// built through the harness's `Make` factory with altered public settings (research.md R-12).
mod agc {
    use rr_dr60::{Pipeline, Settings};
    use rr_dr60_harness::agc_checks;
    use rr_dr60_harness::golden;

    fn release_x1_5(mut s: Settings) -> Pipeline {
        s.agc.release_ms *= 1.5;
        Pipeline::new(s).unwrap()
    }

    fn target_plus_3(mut s: Settings) -> Pipeline {
        s.agc.target_dbfs += 3.0;
        Pipeline::new(s).unwrap()
    }

    fn failed_properties(
        results: &[rr_dr60_harness::checks::MeasurementResult],
    ) -> Vec<&'static str> {
        results
            .iter()
            .filter(|r| !r.passed)
            .map(|r| r.property)
            .collect()
    }

    #[test]
    fn release_change_is_caught() {
        let defaults = agc_checks::matrix()[0];
        let failed = failed_properties(&agc_checks::check_fr006_timing(
            48_000,
            &defaults,
            &release_x1_5,
        ));
        assert!(
            failed.contains(&"release time"),
            "not caught; failures: {failed:?}"
        );
        let diff = golden::compare(
            &golden::committed_agc(),
            &golden::generate_agc(&release_x1_5),
        );
        assert!(diff.is_err(), "AGC golden check missed the release change");
    }

    #[test]
    fn target_change_is_caught() {
        let defaults = agc_checks::matrix()[0];
        let failed = failed_properties(&agc_checks::check_fr004_regulation(
            48_000,
            &defaults,
            &target_plus_3,
        ));
        assert!(
            failed.contains(&"output level on the regulation line"),
            "not caught; failures: {failed:?}"
        );
        let diff = golden::compare(
            &golden::committed_agc(),
            &golden::generate_agc(&target_plus_3),
        );
        assert!(diff.is_err(), "AGC golden check missed the target change");
    }
}

/// Spec 003 SC-007: a deliberate change of the default hang time by 20 %, or of the threshold
/// by 3 dB, is caught by both a tolerance check and the VAS golden check. The mutants are built
/// through the harness's `Make` factory with altered public settings, with no hidden hooks.
mod vas {
    use rr_dr60::{Pipeline, Settings};
    use rr_dr60_harness::golden;
    use rr_dr60_harness::vas_checks;

    fn hang_x1_2(mut s: Settings) -> Pipeline {
        s.vas.hang_ms *= 1.2; // 20 %, engineering target (003 FR-017; SC-007)
        Pipeline::new(s).unwrap()
    }

    fn threshold_plus_3(mut s: Settings) -> Pipeline {
        s.vas.threshold_dbfs += 3.0; // 3 dB, engineering target (003 FR-017; SC-007)
        Pipeline::new(s).unwrap()
    }

    fn failed_properties(
        results: &[rr_dr60_harness::checks::MeasurementResult],
    ) -> Vec<&'static str> {
        results
            .iter()
            .filter(|r| !r.passed)
            .map(|r| r.property)
            .collect()
    }

    #[test]
    fn hang_change_is_caught() {
        let defaults = vas_checks::matrix()[0];
        let failed =
            failed_properties(&vas_checks::check_fr008_hang(48_000, &defaults, &hang_x1_2));
        assert!(
            failed.contains(&"kept part of the gap"),
            "not caught; failures: {failed:?}"
        );
        let diff = golden::compare(&golden::committed_vas(), &golden::generate_vas(&hang_x1_2));
        assert!(diff.is_err(), "VAS golden check missed the hang change");
    }

    #[test]
    fn threshold_change_is_caught() {
        let defaults = vas_checks::matrix()[0];
        let failed = failed_properties(&vas_checks::check_fr006_threshold(
            48_000,
            &defaults,
            &threshold_plus_3,
        ));
        assert!(
            failed.contains(&"threshold at 1 kHz − nominal"),
            "not caught; failures: {failed:?}"
        );
        let diff = golden::compare(
            &golden::committed_vas(),
            &golden::generate_vas(&threshold_plus_3),
        );
        assert!(
            diff.is_err(),
            "VAS golden check missed the threshold change"
        );
    }
}
