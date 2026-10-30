//! Spec 016: the seven-rate report (tasks.md T008): every US1 oracle at `all_rates()`, the
//! per-rate steady pins, the stop flush's and the drain's rows. Report:
//! `cargo test -p rr_dr60_harness --test meters_matrix -- --nocapture`.
//!
//! Under coverage instrumentation the audio-heavy loops run at three rates
//! (`configs::measurement_rates`); the row counts here are per rate.

use rr_dr60_harness::checks::{MeasurementResult, assert_all};
use rr_dr60_harness::{configs, meters_checks as mc, report};

fn finish(results: Vec<MeasurementResult>) {
    report::print(&results);
    assert!(!results.is_empty(), "no measurements");
    assert_all(&results);
}

fn rates() -> Vec<u32> {
    if cfg!(coverage) {
        configs::measurement_rates().to_vec()
    } else {
        configs::all_rates().to_vec()
    }
}

#[test]
fn peaks_at_seven_rates() {
    finish(rates().into_iter().flat_map(mc::host_rate_peaks).collect());
}

/// The limiter figures with 1 s tones in 120-sample blocks (a second block length that obeys
/// the block-length rule); the steady pins per rate.
#[test]
fn limiter_at_seven_rates() {
    finish(
        rates()
            .into_iter()
            .flat_map(|r| mc::limiter_figures(r, 5.0, 120))
            .collect(),
    );
}

#[test]
fn clamps_at_seven_rates() {
    finish(
        rates()
            .into_iter()
            .flat_map(|r| {
                let mut rows = mc::adc_clamp_counts(r);
                rows.extend(mc::output_clamp_counts(r));
                rows
            })
            .collect(),
    );
}

#[test]
fn vas_at_seven_rates() {
    finish(
        rates()
            .into_iter()
            .flat_map(|r| {
                let mut rows = mc::vas_figures(r, false);
                rows.extend(mc::vas_figures(r, true));
                rows
            })
            .collect(),
    );
}

#[test]
fn transport_at_seven_rates() {
    finish(rates().into_iter().flat_map(mc::transport_groups).collect());
}
