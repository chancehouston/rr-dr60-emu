//! Spec 029 US3 [TR: D4, D6 i, ii, v, vi; FR-003, FR-006]: the measures respond. The reference
//! reads high, the pass-through row reads the input's own figures, silent cells read empty, the
//! teeth rows hold, and the reader is self-contained under the deterministic-math rule.

use rr_dr60_harness::checks::{MeasurementResult, Tolerance, assert_all};
use rr_dr60_harness::nonspeech_checks::{self as ns, Configuration as C, Stimulus as S};
use rr_dr60_harness::{configs, report};

/// The bounds of D6 (i), restated 2026-11-04 from the harness print (plan P1; spec Clarifications).
const REFERENCE_FRACTION_MIN: f64 = 0.9;
const REFERENCE_PEAKS_MIN: f64 = 1.8;
const NOISE_FRACTION_MAX: f64 = 0.1;
/// The noise bound applies to cells with at least this many active frames (the bursts row
/// through the coder alone has about 85 and reads the decoder's continuation; restated at the
/// first print, 2026-11-04).
const NOISE_FRACTION_MIN_ACTIVE: usize = 500;
/// D6 (v): the measures' own teeth.
const SHARE_B_MIN: f64 = 0.8;
const SHARE_STEADY_MAX: f64 = 0.35;
const NOISE_PEAKS_MAX: f64 = 1.5;
/// The resonator row's rise over a35 and its median band (plan P2; restated at the first print:
/// the rise reads 0.20 – 0.40 by rate, the median 700 – 720 Hz).
const RESONATOR_RISE_MIN: f64 = 0.15;
const RESONATOR_MEDIAN_HZ: (f64, f64) = (650.0, 800.0);
const REFERENCE_MEDIAN_HZ: (f64, f64) = (250.0, 800.0);
/// D6 (ii) at the rates other than 6 kHz (plan R-05b; recorded 2026-11-04 from the first harness
/// print: 2 × the largest |C0 − input-own| over the stimuli at seven rates, with the floors):
/// level 0.032 → 0.07 dB, peaks 0.067 → 0.14, periodicity 0.001 → 0.002, periodic fraction
/// 0.013 → 0.03, share 0.003 → 0.01, flux 0.015 → 0.03 dB, active frames 0 → ±1.
const PASS_THROUGH_TOL_LEVEL_DB: f64 = 0.07;
const PASS_THROUGH_TOL_PEAKS: f64 = 0.14;
const PASS_THROUGH_TOL_PERIODICITY: f64 = 0.002;
const PASS_THROUGH_TOL_FRACTION: f64 = 0.03;
const PASS_THROUGH_TOL_SHARE: f64 = 0.01;
const PASS_THROUGH_TOL_FLUX_DB: f64 = 0.03;

const NOISE: [S; 5] = [S::A35, S::A50, S::A55, S::B, S::C];
const TRACE: &[&str] = &["029 D6"];

#[allow(clippy::too_many_arguments)]
fn r(
    req: &'static str,
    prop: &'static str,
    rate: u32,
    cfg: &'static str,
    stim: String,
    v: f64,
    unit: &'static str,
    t: Tolerance,
) -> MeasurementResult {
    MeasurementResult::new(req, prop, TRACE, rate, cfg, stim, v, unit, t)
}

/// D6 (i): the reference separates; every noise cell with ≥ 500 active frames reads low.
#[test]
fn nonspeech_separation() {
    let mut out = Vec::new();
    for &rate in configs::measurement_rates() {
        for cfg in [C::C0, C::C1, C::C2, C::C3] {
            let m = ns::cell(S::G, cfg, rate).measures;
            out.push(r(
                "029 D6 i",
                "reference periodic-frame fraction",
                rate,
                cfg.id(),
                "g".into(),
                m.periodic_fraction.unwrap(),
                "",
                Tolerance::AtLeast(REFERENCE_FRACTION_MIN),
            ));
            out.push(r(
                "029 D6 i",
                "reference LPC peaks per frame",
                rate,
                cfg.id(),
                "g".into(),
                m.peaks.unwrap(),
                "",
                Tolerance::AtLeast(REFERENCE_PEAKS_MIN),
            ));
        }
        for cfg in [C::C4, C::C5, C::C6, C::C7] {
            let m = ns::cell(S::G, cfg, rate).measures;
            out.push(r(
                "029 D6 i",
                "reference periodic-frame fraction (reported)",
                rate,
                cfg.id(),
                "g".into(),
                m.periodic_fraction.unwrap(),
                "",
                Tolerance::Report,
            ));
        }
        let mut cells: Vec<(S, C)> = NOISE
            .iter()
            .chain([S::D, S::E].iter())
            .flat_map(|&s| ns::CONFIGURATIONS.iter().map(move |&c| (s, c)))
            .collect();
        cells.push((S::F, C::C6));
        for (s, c) in cells {
            let m = ns::cell(s, c, rate).measures;
            let Some(f) = m.periodic_fraction else {
                continue;
            };
            let t = if m.active >= NOISE_FRACTION_MIN_ACTIVE {
                Tolerance::AtMost(NOISE_FRACTION_MAX)
            } else {
                Tolerance::Report
            };
            out.push(r(
                "029 D6 i",
                "noise periodic-frame fraction",
                rate,
                c.id(),
                format!("{} ({} active)", s.id(), m.active),
                f,
                "",
                t,
            ));
        }
    }
    report::print(&out);
    assert_all(&out);
}

/// D6 (ii): C0 equals the input's own reading, bit for bit at 6 kHz; the other rates reported
/// until the tolerances are recorded (plan R-05b).
#[test]
fn nonspeech_pass_through() {
    let mut out = Vec::new();
    for &rate in configs::measurement_rates() {
        for s in ns::STIMULI {
            let c = ns::cell(s, C::C0, rate);
            let own = c.input_own.unwrap();
            if rate == ns::DEVICE_HZ {
                assert_eq!(
                    c.measures,
                    own,
                    "{} at 6 kHz: C0 is the input bit for bit",
                    s.id()
                );
                out.push(r(
                    "029 D6 ii",
                    "C0 equals input-own (bit for bit)",
                    rate,
                    "C0",
                    s.id().into(),
                    0.0,
                    "",
                    Tolerance::Exact(0.0),
                ));
            } else {
                let d = |a: Option<f64>, b: Option<f64>| match (a, b) {
                    (Some(a), Some(b)) => (a - b).abs(),
                    _ => 0.0,
                };
                let level = if c.measures.level_dbfs.is_finite() {
                    (c.measures.level_dbfs - own.level_dbfs).abs()
                } else {
                    0.0
                };
                out.push(r(
                    "029 D6 ii",
                    "|C0 − own| level",
                    rate,
                    "C0",
                    s.id().into(),
                    level,
                    "dB",
                    Tolerance::AtMost(PASS_THROUGH_TOL_LEVEL_DB),
                ));
                out.push(r(
                    "029 D6 ii",
                    "|C0 − own| peaks",
                    rate,
                    "C0",
                    s.id().into(),
                    d(c.measures.peaks, own.peaks),
                    "",
                    Tolerance::AtMost(PASS_THROUGH_TOL_PEAKS),
                ));
                out.push(r(
                    "029 D6 ii",
                    "|C0 − own| periodicity",
                    rate,
                    "C0",
                    s.id().into(),
                    d(c.measures.periodicity, own.periodicity),
                    "",
                    Tolerance::AtMost(PASS_THROUGH_TOL_PERIODICITY),
                ));
                out.push(r(
                    "029 D6 ii",
                    "|C0 − own| periodic fraction",
                    rate,
                    "C0",
                    s.id().into(),
                    d(c.measures.periodic_fraction, own.periodic_fraction),
                    "",
                    Tolerance::AtMost(PASS_THROUGH_TOL_FRACTION),
                ));
                out.push(r(
                    "029 D6 ii",
                    "|C0 − own| share",
                    rate,
                    "C0",
                    s.id().into(),
                    d(c.measures.mod_share, own.mod_share),
                    "",
                    Tolerance::AtMost(PASS_THROUGH_TOL_SHARE),
                ));
                out.push(r(
                    "029 D6 ii",
                    "|C0 − own| flux",
                    rate,
                    "C0",
                    s.id().into(),
                    d(c.measures.flux_db, own.flux_db),
                    "dB",
                    Tolerance::AtMost(PASS_THROUGH_TOL_FLUX_DB),
                ));
                out.push(r(
                    "029 D6 ii",
                    "|C0 − own| active frames",
                    rate,
                    "C0",
                    s.id().into(),
                    (c.measures.active as f64 - own.active as f64).abs(),
                    "frames",
                    Tolerance::AtMost(1.0),
                ));
            }
        }
    }
    report::print(&out);
    assert_all(&out);
}

/// D6 (v): the measures' own teeth on pass-through rows.
#[test]
fn nonspeech_teeth() {
    let mut out = Vec::new();
    for &rate in configs::measurement_rates() {
        let b = ns::cell(S::B, C::C0, rate).measures;
        out.push(r(
            "029 D6 v",
            "2-8 Hz share on (b) in C0",
            rate,
            "C0",
            "b".into(),
            b.mod_share.unwrap(),
            "",
            Tolerance::AtLeast(SHARE_B_MIN),
        ));
        let a = ns::cell(S::A35, C::C0, rate).measures;
        out.push(r(
            "029 D6 v",
            "2-8 Hz share on steady noise in C0",
            rate,
            "C0",
            "a35".into(),
            a.mod_share.unwrap(),
            "",
            Tolerance::AtMost(SHARE_STEADY_MAX),
        ));
        for s in NOISE.iter().chain([S::D, S::E].iter()) {
            for c in [C::C0, C::C1] {
                let m = ns::cell(*s, c, rate).measures;
                out.push(r(
                    "029 D6 v",
                    "noise LPC peaks per frame",
                    rate,
                    c.id(),
                    s.id().into(),
                    m.peaks.unwrap(),
                    "",
                    Tolerance::AtMost(NOISE_PEAKS_MAX),
                ));
            }
        }
        let h = ns::cell(S::H, C::C0, rate).measures;
        out.push(r(
            "029 D6 v",
            "resonator row's peak rise over a35 in C0",
            rate,
            "C0",
            "h".into(),
            h.peaks.unwrap() - a.peaks.unwrap(),
            "",
            Tolerance::AtLeast(RESONATOR_RISE_MIN),
        ));
        out.push(r(
            "029 D6 v",
            "resonator row's lowest-peak median",
            rate,
            "C0",
            "h".into(),
            h.lowest_peak_median_hz.unwrap(),
            "Hz",
            Tolerance::Range(RESONATOR_MEDIAN_HZ.0, RESONATOR_MEDIAN_HZ.1),
        ));
        let g = ns::cell(S::G, C::C0, rate).measures;
        out.push(r(
            "029 D6 v",
            "reference's lowest-peak median",
            rate,
            "C0",
            "g".into(),
            g.lowest_peak_median_hz.unwrap(),
            "Hz",
            Tolerance::Range(REFERENCE_MEDIAN_HZ.0, REFERENCE_MEDIAN_HZ.1),
        ));
    }
    report::print(&out);
    assert_all(&out);
}

/// D6 (vi): every figure finite where the cell has active frames, "—" where it has none; the
/// pair count on (d) in C0 and C5 equals the structural oracle.
#[test]
fn nonspeech_finite_and_pairs() {
    let mut out = Vec::new();
    for &rate in configs::measurement_rates() {
        for c in ns::table(rate) {
            let m = c.measures;
            let finite = [m.peaks, m.periodicity, m.periodic_fraction]
                .iter()
                .all(|v| match v {
                    Some(x) => x.is_finite(),
                    None => m.active == 0,
                });
            out.push(r(
                "029 D6 vi",
                "figures finite where active, absent where not",
                rate,
                c.configuration.id(),
                c.stimulus.id().into(),
                f64::from(u8::from(finite)),
                "",
                Tolerance::Exact(1.0),
            ));
            let empty_ok = m.active > 0
                || (m.peaks.is_none() && m.periodicity.is_none() && m.mod_share.is_none());
            out.push(r(
                "029 D6 vi",
                "no figure on an inactive cell",
                rate,
                c.configuration.id(),
                c.stimulus.id().into(),
                f64::from(u8::from(empty_ok)),
                "",
                Tolerance::Exact(1.0),
            ));
        }
        for cfg in [C::C0, C::C5] {
            let sp = ns::process(S::D, cfg, rate, ns::BLOCK);
            let m = ns::read(&sp.samples, &sp.splices, ns::PRODUCTION);
            let want = ns::pairs_oracle(&sp.samples, &sp.splices, ns::ACTIVE_DBFS);
            out.push(r(
                "029 D6 vi",
                "pair count equals Σ(run − 1) split at splices",
                rate,
                cfg.id(),
                format!("d ({} splices)", sp.splices.len()),
                m.pairs as f64 - want as f64,
                "pairs",
                Tolerance::Exact(0.0),
            ));
        }
    }
    report::print(&out);
    assert_all(&out);
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

/// FR-003 (plan R-01, coach 13): the reader calls no `analysis::` item and no `std`
/// transcendental method; the module carries no `allow(clippy::disallowed_methods)`.
#[test]
fn nonspeech_reader_uses_detmath_only() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/nonspeech_checks.rs"
    ))
    .unwrap();
    // Code lines only (comments describe the rule).
    let src: String = src
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !src.contains("analysis::"),
        "nonspeech_checks must not call analysis::"
    );
    assert!(!src.contains("disallowed_methods"));
    for m in [
        ".sin(", ".cos(", ".tan(", ".exp(", ".ln(", ".log10(", ".powf(", ".powi(", ".sqrt(",
        "mul_add",
    ] {
        assert!(!src.contains(m), "std method {m} in the reader");
    }
}
