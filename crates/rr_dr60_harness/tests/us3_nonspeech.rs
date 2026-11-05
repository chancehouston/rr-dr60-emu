//! Spec 029 US3 [TR: D4, D6 i, ii, v, vi; FR-003, FR-006]: the measures respond. The check bodies
//! live in `nonspeech_checks::d6_*` so the bless gate asserts the same rows (D1).

use rr_dr60_harness::checks::assert_all;
use rr_dr60_harness::nonspeech_checks as ns;
use rr_dr60_harness::{configs, report};

fn run(results: Vec<rr_dr60_harness::checks::MeasurementResult>) {
    report::print(&results);
    for l in ns::summary(&results) {
        println!("{l}");
    }
    assert_all(&results);
}

/// D6 (i): the reference separates; every noise cell with ≥ 500 active frames reads low.
#[test]
fn nonspeech_separation() {
    run(ns::d6_separation(configs::measurement_rates()));
}

/// D6 (ii): C0 equals the input's own reading, bit for bit at 6 kHz, within the recorded
/// tolerances elsewhere; a presence mismatch fails.
#[test]
fn nonspeech_pass_through() {
    run(ns::d6_pass_through(configs::measurement_rates()));
}

/// D6 (v): the measures' own teeth on pass-through rows (incl. the pink row's median, the
/// pre-emphasis mutant's row).
#[test]
fn nonspeech_teeth() {
    run(ns::d6_teeth(configs::measurement_rates()));
}

/// D6 (vi): every figure finite where present (the golden's rates), the pair oracle (the
/// measurement rates).
#[test]
fn nonspeech_finite_and_pairs() {
    run(ns::d6_finite_and_pairs(&ns::GOLDEN_RATES));
}

/// FR-003: the header's definitions equal a hand-written block (coach 16).
#[test]
fn nonspeech_reader_definitions() {
    let want = "\
def frame=120
def settle_s=0.5
def active_dbfs=-75
def lpc_order=8
def preemphasis=0.9
def grid_hz=10
def search_hz=200-2300
def prominence_db=3
def prominence_span_hz=300
def width_hz=300
def lag=20-100
def periodic_r=0.5
def stable_r=0.35
def stable_dlag=2
def dft_hz=0.5-25
def share_hz=2-8
def share_active=0.9
def flux_grid_hz=200-2300
def window=hann
def resample_cutoff_hz=2700
def splice_reset=at each splice (the pause reports)";
    assert_eq!(ns::definitions().join("\n"), want);
}

/// FR-003 (plan R-01, coach 13): the reader calls nothing from the analysis module and no `std`
/// transcendental method; the module carries no `allow(clippy::disallowed_methods)`. Code lines
/// only (comments describe the rule).
#[test]
fn nonspeech_reader_uses_detmath_only() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/nonspeech_checks.rs"
    ))
    .unwrap();
    let src: String = src
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !src.contains("analysis::"),
        "nonspeech_checks must not call the analysis module"
    );
    assert!(!src.contains("disallowed_methods"));
    for m in [
        ".sin(", ".cos(", ".tan(", ".exp(", ".ln(", ".log10(", ".powf(", ".powi(", ".sqrt(",
        "mul_add",
    ] {
        assert!(!src.contains(m), "std method {m} in the reader");
    }
}
