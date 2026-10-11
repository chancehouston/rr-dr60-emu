//! Spec 005 User Story 1: hear the RR-DR60's limiter (tasks.md T017; spec › US1 AS1–AS7).
//!
//! One test per acceptance scenario, each running the Phase 3 check functions at the 6 kHz
//! identity rate and at 48 kHz (AS6 at 48 kHz only, AS7 at 6 kHz only) and asserting every row
//! passes. Every limiter row is measured at the host-rate "after AGC" tap of the AGC-isolated
//! configuration (`agc_only`), with the exact per-sample gain (005 research R-08).
//! Report: `cargo test -p rr_dr60_harness --test us1_agc -- --nocapture`.

use rr_dr60_harness::agc_checks;
use rr_dr60_harness::checks::{MeasurementResult, assert_all, standard};
use rr_dr60_harness::report;

const RATES: [u32; 2] = [6000, 48_000];

fn finish(results: Vec<MeasurementResult>) {
    report::print(&results);
    assert!(!results.is_empty(), "no measurements");
    assert_all(&results);
}

fn at(rates: &[u32], check: impl Fn(u32) -> Vec<MeasurementResult>) -> Vec<MeasurementResult> {
    rates.iter().flat_map(|&r| check(r)).collect()
}

/// US1 AS1 (FR-003): input whose peaks stay 1.5 dB below the knee is returned bit for bit; a
/// bypassed limiter too.
#[test]
fn as1_passthrough_below_the_knee() {
    finish(at(&RATES, |r| {
        agc_checks::check_fr003_passthrough(r, &standard)
    }));
}

/// US1 AS2 (FR-004): the static curve from −60 to +20 dBFS, the ceiling ±0.5 dB above the
/// knee + 1.5 dB, the knee at 0 ± 2 dBFS by bisection, the knee band reported.
#[test]
fn as2_static_curve_and_knee() {
    finish(at(&RATES, |r| agc_checks::check_fr004_static(r, &standard)));
}

/// US1 AS3 (FR-005): the dead time of a −22 → +9 dBFS step (10 dB over the knee) is 3 ± 1.5 ms, the gain then
/// settles within 1 ms without over-attenuating by more than 1 dB; the 2 dB-over step takes
/// at least 3× longer.
#[test]
fn as3_dead_time_then_limiting() {
    finish(at(&RATES, |r| {
        agc_checks::check_fr005_attack(r, &standard)
            .into_iter()
            .filter(|row| !row.property.contains("armed"))
            .collect()
    }));
}

/// US1 AS4 (FR-005): 2 s after a loud burst the limiter is armed and the dead time is ≤ 0.5 ms.
#[test]
fn as4_armed_dead_time() {
    finish(at(&RATES, |r| {
        agc_checks::check_fr005_attack(r, &standard)
            .into_iter()
            .filter(|row| row.property.contains("armed"))
            .collect()
    }));
}

/// US1 AS5 (FR-006): after a 1 s burst 10 dB over the knee the gain is back within 1 dB of
/// unity after 1.37 s ± 40 %, reading about −6 dB at 0.3 s and −3 dB at 0.75 s.
#[test]
fn as5_release_trajectory() {
    finish(at(&RATES, |r| {
        agc_checks::check_fr006_release(r, &standard)
    }));
}

/// US1 AS6 (FR-007): at 48 kHz a 6 kHz tone at +6 dBFS pulls a −20 dBFS probe down by 5 ± 1 dB
/// although the default output carries no 6 kHz content.
#[test]
fn as6_bandwidth_at_48k() {
    finish(agc_checks::check_fr007_bandwidth(48_000, &standard));
}

/// FR-007 (A-036): the sensed polarity, with the asymmetric stimulus at 6 and 48 kHz.
#[test]
fn fr007_polarity() {
    finish(at(&RATES, |r| {
        agc_checks::check_fr007_polarity(r, &standard)
    }));
}

/// US1 AS7 (FR-008): at the 6 kHz identity rate a +11 dBFS tone through the record path peaks
/// at exactly 1.0, with ≥ 60 % of the first 2 ms of its onset at ±1.0 and ≤ 40 % after 20 ms.
#[test]
fn as7_clipped_onset_duty_at_6k() {
    finish(agc_checks::check_fr008_clamp(6000, &standard));
}

/// FR-009 (report rows) and FR-001 (latency) at 6 and 48 kHz.
#[test]
fn fr009_thdn_reports_and_fr001_latency() {
    let mut r = at(&RATES, |r| agc_checks::check_fr009_thdn(r, &standard));
    r.extend(at(&RATES, agc_checks::check_fr001_latency));
    finish(r);
}
