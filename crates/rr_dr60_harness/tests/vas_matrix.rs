//! Spec 003 VAS measurement matrix (tasks.md T035; research.md R-11, R-12; SC-002).
//!
//! - `defaults_all_rates`: every VAS check at the device defaults, at all seven host rates
//!   (spec 004 FR-012, FR-016).
//! - `settings_matrix_6k_48k`: the FR-013 settings matrix (every sensitivity level, and the
//!   minimum and maximum of each other setting) at the 6 kHz identity rate and at 48 kHz.
//! - `full_matrix_all_rates` (ignored; release-mode CI): the whole matrix at all seven rates.
//! - `interplay_report` (US3 AS3): the floor sweep through the default pipeline, report only.
//!
//! Report: `cargo test -p rr_dr60_harness --test vas_matrix -- --nocapture`.
//! Runtime (T035, measured on an Apple-silicon laptop, test profile opt-level 3): the normal
//! run (`defaults_all_rates`, 228 checks, plus `settings_matrix_6k_48k`, 554 checks) takes
//! about 7 s; the ignored full matrix (1896 checks) about 55 s in release mode, most of it in
//! the 10 s-hang cases (R-12 budget: 60 s).

use rr_dr60::DEVICE_RATE_HZ;
use rr_dr60_harness::checks::{MeasurementResult, assert_all, standard};
use rr_dr60_harness::vas_checks::{self, Case};
use rr_dr60_harness::{configs, report};

/// The per-setting checks (003 FR-004 – FR-010, FR-013).
fn per_case(rate: u32, case: &Case) -> Vec<MeasurementResult> {
    let mut r = vas_checks::check_fr004_length(rate, case, &standard);
    r.extend(vas_checks::check_fr005_events(rate, case, &standard));
    r.extend(vas_checks::check_fr006_threshold(rate, case, &standard));
    r.extend(vas_checks::check_fr008_hang(rate, case, &standard));
    r.extend(vas_checks::check_fr009_onset(rate, case, &standard));
    r.extend(vas_checks::check_fr010_splices(rate, case, &standard));
    r
}

fn finish(results: Vec<MeasurementResult>) {
    report::print(&results);
    assert_all(&results);
}

#[test]
fn defaults_all_rates() {
    let defaults = vas_checks::matrix()[0];
    assert!(defaults.is_default());
    let mut r = Vec::new();
    for rate in configs::all_rates() {
        r.extend(per_case(rate, &defaults));
        r.extend(vas_checks::check_fr007_sensitivity(
            rate, &defaults, &standard,
        ));
        r.extend(vas_checks::check_fr011_latency(rate));
        if rate != DEVICE_RATE_HZ {
            r.extend(vas_checks::check_rate_self_test(rate, &defaults, &standard));
        }
    }
    finish(r);
}

#[test]
fn settings_matrix_6k_48k() {
    let mut r = Vec::new();
    for rate in [DEVICE_RATE_HZ, 48_000] {
        for case in &vas_checks::matrix()[1..] {
            r.extend(per_case(rate, case));
        }
    }
    finish(r);
}

#[test]
#[ignore = "slow: full settings matrix at all seven rates; run in release mode (CI check job)"]
fn full_matrix_all_rates() {
    let mut r = Vec::new();
    for rate in configs::all_rates() {
        for case in &vas_checks::matrix() {
            r.extend(per_case(rate, case));
            if rate != DEVICE_RATE_HZ {
                r.extend(vas_checks::check_rate_self_test(rate, case, &standard));
            }
        }
    }
    finish(r);
}

/// US3 AS3 (rewritten by spec 005 FR-012): how much of each noise gap the default pipeline keeps
/// at each floor with the speech-like bursts on top, and the threshold table. Report only,
/// except the identity statement: below the limiter's knee the threshold is input-referred one
/// to one (the spec 003 "about −58 dBFS" statement is withdrawn).
#[test]
fn interplay_report() {
    for rate in [DEVICE_RATE_HZ, 48_000] {
        for line in vas_checks::interplay_report(rate, &standard) {
            println!("{line}");
        }
    }
    for level in 1..=5u8 {
        let mut s = configs::settings("default_vas", 48_000);
        s.vas.sensitivity = level;
        assert_eq!(
            vas_checks::input_referred_threshold_dbfs(&s),
            vas_checks::effective_threshold_dbfs(&s.vas),
            "level {level}: input-referred == configured below the knee (005 FR-012)"
        );
    }
}
