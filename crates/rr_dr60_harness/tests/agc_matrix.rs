//! Spec 005 limiter measurement matrix (tasks.md T018; research R-08; SC-002).
//!
//! - `defaults_all_rates`: every limiter check at the device defaults, at all seven host rates
//!   (FR-001, FR-003 – FR-009), the static-curve set included (only the limiter runs at the
//!   host-rate tap, so the 46 five-second tones per rate are cheap; 1 s tones under
//!   `cfg(coverage)`).
//! - `settings_matrix_6k_48k`: the FR-011 settings matrix (knee −10 / +3 dBFS, attack 1 / 10 /
//!   50 ms, release 200 / 4000 / 10 000 ms, one setting moved at a time) at the 6 kHz identity
//!   rate and at 48 kHz (SC-002).
//! - `full_matrix_all_rates` (ignored; release-mode CI): the defaults plus the FR-011 settings
//!   matrix at all seven rates.
//!
//! Report: `cargo test -p rr_dr60_harness --test agc_matrix -- --nocapture`.

use rr_dr60_harness::agc_checks::{self, Case};
use rr_dr60_harness::checks::{MeasurementResult, assert_all, standard};
use rr_dr60_harness::{configs, report};

/// The per-setting checks (005 FR-011).
fn per_case(rate: u32, case: &Case) -> Vec<MeasurementResult> {
    agc_checks::check_fr011_settings(rate, case, &standard)
}

/// The default-settings checks (005 FR-001, FR-003 – FR-009).
fn defaults_at(rate: u32) -> Vec<MeasurementResult> {
    let mut r = agc_checks::check_fr003_passthrough(rate, &standard);
    r.extend(agc_checks::check_fr004_static(rate, &standard));
    r.extend(agc_checks::check_fr005_attack(rate, &standard));
    r.extend(agc_checks::check_fr006_release(rate, &standard));
    r.extend(agc_checks::check_fr007_polarity(rate, &standard));
    r.extend(agc_checks::check_fr007_bandwidth(rate, &standard));
    r.extend(agc_checks::check_fr008_clamp(rate, &standard));
    r.extend(agc_checks::check_fr009_thdn(rate, &standard));
    r.extend(agc_checks::check_fr001_latency(rate));
    r
}

fn finish(results: Vec<MeasurementResult>) {
    report::print(&results);
    assert!(!results.is_empty(), "no measurements");
    assert_all(&results);
}

#[test]
fn defaults_all_rates() {
    assert!(agc_checks::matrix()[0].is_default());
    let mut r = Vec::new();
    for rate in configs::all_rates() {
        r.extend(defaults_at(rate));
    }
    finish(r);
}

/// 005 FR-011, SC-002: the settings matrix at the identity rate and at 48 kHz.
#[test]
fn settings_matrix_6k_48k() {
    let mut r = Vec::new();
    for rate in [6000, 48_000] {
        for case in &agc_checks::matrix()[1..] {
            r.extend(per_case(rate, case));
        }
    }
    assert!(r.iter().all(|row| row.requirement == "005/FR-011"));
    finish(r);
}

#[test]
#[ignore = "slow: defaults plus the settings matrix at all seven rates; run in release mode (CI check job)"]
fn full_matrix_all_rates() {
    let mut r = Vec::new();
    for rate in configs::all_rates() {
        r.extend(defaults_at(rate));
        for case in &agc_checks::matrix()[1..] {
            r.extend(per_case(rate, case));
        }
    }
    finish(r);
}
