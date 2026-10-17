//! Spec 012 User Story 1: the device's microphone sound, its band and its hiss (tasks.md T022;
//! spec › US1 AS1 – AS4). One test per acceptance scenario at the measurement rates in the
//! stages-only configurations, each asserting every row passes. Report:
//! `cargo test -p rr_dr60_harness --test us1_mic -- --nocapture`.

use rr_dr60::{Error, MicSettings, Pipeline, Setting};
use rr_dr60_harness::checks::{MeasurementResult, assert_all, standard};
use rr_dr60_harness::mic_checks as mic;
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

/// US1 AS1, SC-002: the band after stage 2 matches the prototype within ±0.1 dB; the group
/// delay and the −3 dB points are reported.
#[test]
fn band_matches_the_prototype() {
    finish(each(|r| mic::band(&standard, r)));
}

/// US1 AS2, SC-001: the hiss at offset 0, level and shape.
#[test]
fn hiss_level_and_shape() {
    finish(each(|r| mic::hiss_level(&standard, r, 0.0)));
}

/// US1 AS3, SC-001: each offset moves the hiss by its value (−18, −6, +12 dB; 0 above).
#[test]
fn hiss_offsets() {
    finish(each(|r| {
        [-18.0, -6.0, 12.0]
            .iter()
            .flat_map(|&o| mic::hiss_level(&standard, r, o))
            .collect()
    }));
}

/// US1 AS3 (FR-003): an offset outside −18 … +12 dB, or not finite, is refused with the
/// invalid-setting status whether or not the capsule is on; the ends are accepted.
#[test]
fn out_of_range_offsets_are_refused() {
    for capsule in [true, false] {
        for (v, ok) in [
            (-18.0, true),
            (12.0, true),
            (-18.01, false),
            (12.01, false),
            (f32::NAN, false),
            (f32::INFINITY, false),
            (f32::NEG_INFINITY, false),
        ] {
            let mut s = mic::mic_only(48_000, 0.0);
            s.mic.capsule_enabled = capsule;
            s.mic.hiss_offset_db = v;
            let got = Pipeline::new(s).map(|_| ());
            if ok {
                assert_eq!(got, Ok(()), "{v} (capsule {capsule})");
            } else {
                assert_eq!(
                    got,
                    Err(Error::InvalidSetting {
                        setting: Setting::MicHissOffsetDb
                    }),
                    "{v} (capsule {capsule})"
                );
            }
        }
    }
    assert_eq!(MicSettings::DEVICE.hiss_offset_db, 0.0);
}

/// US1 AS4, SC-005: independent streams for two seeds and two consecutive counters.
#[test]
fn streams_are_independent() {
    finish(each(|r| mic::streams(&standard, r)));
}

/// US1 AS4, SC-005: four block partitions are bit-identical.
#[test]
fn partitions_are_bit_identical() {
    finish(each(|r| mic::determinism(&standard, r)));
}

/// US3 AS1, SC-008 (at the P1 checkpoint): the first recording after construction, reset or
/// reconfigure equals the one-pass, the stop's flush carrying the hiss.
#[test]
fn whole_engine_equivalence() {
    finish(each(|r| mic::equivalence(&standard, r)));
}

/// US3 AS2 (FR-009): rest and rekeying after reset; the k-th recordings of two engines.
#[test]
fn whole_engine_rest_and_rekey() {
    finish(each(|r| mic::rest_and_rekey(&standard, r)));
}

/// US3 AS3 (FR-008): the two taps and the earlier taps' latencies.
#[test]
fn whole_engine_taps() {
    finish(each(|r| mic::taps(&standard, r)));
}

/// US3 AS6, SC-010: the voice activation pauses once and never resumes on the hiss at every
/// level; the level-5 margin is at least 10 dB (60 s runs).
#[test]
fn whole_engine_vas_margin() {
    finish(each(|r| mic::vas_margin(&standard, r, mic::VAS_SECONDS)));
}

/// US3 AS6 (reported): the offsets at which the gate stops pausing at levels 4 and 5 (6 kHz
/// only under coverage: the hiss level at the voice-activation input does not depend on the
/// rate).
#[test]
fn whole_engine_vas_offset_bisection() {
    let rates: &[u32] = if cfg!(coverage) {
        &[6000]
    } else {
        configs::measurement_rates()
    };
    finish(
        rates
            .iter()
            .flat_map(|&r| {
                [4u8, 5]
                    .iter()
                    .flat_map(move |&l| mic::vas_offset_bisection(&standard, r, l, 10.0))
                    .collect::<Vec<_>>()
            })
            .collect(),
    );
}

/// US3 AS8: the limiter only scales the hiss.
#[test]
fn whole_engine_limiter_ratio() {
    finish(each(|r| mic::limiter_ratio(&standard, r)));
}

/// US3 AS7 (reported): the decoded hiss and the recording length.
#[test]
fn whole_engine_decoded_hiss() {
    finish(each(|r| mic::decoded_hiss(&standard, r)));
}
