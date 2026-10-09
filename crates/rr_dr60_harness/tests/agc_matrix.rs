//! Spec 002 AGC measurement matrix (tasks.md T035; research.md R-11; SC-002).
//!
//! - `defaults_all_rates`: every AGC check at the device defaults, at all six host rates.
//! - `settings_matrix_8k_48k`: the FR-012 settings matrix (US2 examples plus the minimum and
//!   maximum of each setting) at 8 and 48 kHz.
//! - `full_matrix_all_rates` (ignored; release-mode CI): the whole matrix at all six rates.
//!
//! Report: `cargo test -p rr_dr60_harness --test agc_matrix -- --nocapture`.
//! Runtime (T035, measured on an Apple-silicon laptop, test profile opt-level 3): the normal
//! run (1162 checks) takes about 5 s; the ignored full matrix (1734 checks) about 25 s in
//! release mode, most of it in the 10 s-release cases.

use rr_dr60::SUPPORTED_HOST_RATES;
use rr_dr60_harness::agc_checks::{self, Case};
use rr_dr60_harness::checks::{MeasurementResult, assert_all, standard};
use rr_dr60_harness::report;

/// The per-setting checks (002 FR-004 – FR-007, FR-012).
fn per_case(rate: u32, case: &Case) -> Vec<MeasurementResult> {
    let mut r = agc_checks::check_fr004_regulation(rate, case, &standard);
    r.extend(agc_checks::check_fr005_limits(rate, case, &standard));
    r.extend(agc_checks::check_fr006_timing(rate, case, &standard));
    r.extend(agc_checks::check_fr007_silence(rate, case, &standard));
    r
}

fn finish(results: Vec<MeasurementResult>) {
    report::print(&results);
    assert_all(&results);
}

#[test]
fn defaults_all_rates() {
    let defaults = agc_checks::matrix()[0];
    assert!(defaults.is_default());
    let mut r = Vec::new();
    for rate in SUPPORTED_HOST_RATES {
        r.extend(per_case(rate, &defaults));
        r.extend(agc_checks::check_ratio_self_test(rate, &standard));
        r.extend(agc_checks::check_fr008_frequency_thdn(rate, &standard));
        r.extend(agc_checks::check_fr009_noise_rise(rate, &standard));
        r.extend(agc_checks::check_fr010_latency(rate));
    }
    finish(r);
}

#[test]
fn settings_matrix_8k_48k() {
    let mut r = Vec::new();
    for rate in [8000, 48_000] {
        for case in &agc_checks::matrix()[1..] {
            r.extend(per_case(rate, case));
        }
    }
    finish(r);
}

#[test]
#[ignore = "slow: full settings matrix at all six rates; run in release mode (CI check job)"]
fn full_matrix_all_rates() {
    let mut r = Vec::new();
    for rate in SUPPORTED_HOST_RATES {
        for case in &agc_checks::matrix() {
            r.extend(per_case(rate, case));
        }
    }
    finish(r);
}
