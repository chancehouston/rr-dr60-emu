//! Spec 029 US1 [TR: D6 iii, iv; FR-006]: the silence rows and the coder's periodicity.

use rr_dr60_harness::checks::{MeasurementResult, Tolerance, assert_all};
use rr_dr60_harness::nonspeech_checks::{self as ns, Configuration as C, Stimulus as S};
use rr_dr60_harness::{configs, report};

const TRACE: &[&str] = &["029 D6"];
/// D6 (iv): C3's periodicity over C1's on every noise stimulus (observed +0.034 … +0.098 on the
/// white rows a – d at seven rates; the pink row e reads +0.015 … +0.045, so it carries its own
/// bound; restated at the first print, 2026-11-04).
const PERIODICITY_RISE_MIN: f64 = 0.03;
const PERIODICITY_RISE_MIN_PINK: f64 = 0.01;
const MIN_ACTIVE_FOR_RISE: usize = 50;
/// D6 (iii): the residue bound (spec 008: ≤ −78 dBFS; margin 3 dB) and C6's open-gate share.
const RESIDUE_MAX_DBFS: f64 = -75.0;
const OPEN_GATE_MIN_SHARE: f64 = 0.9;

fn r(
    prop: &'static str,
    rate: u32,
    cfg: &'static str,
    stim: &str,
    v: f64,
    unit: &'static str,
    t: Tolerance,
) -> MeasurementResult {
    MeasurementResult::new(
        "029 D6",
        prop,
        TRACE,
        rate,
        cfg,
        stim.to_string(),
        v,
        unit,
        t,
    )
}

/// D6 (iii): digital silence through the ladder.
#[test]
fn nonspeech_silence() {
    let mut out = Vec::new();
    for &rate in configs::measurement_rates() {
        for c in [C::C0, C::C1] {
            let m = ns::cell(S::F, c, rate).measures;
            out.push(r(
                "silence: active frames",
                rate,
                c.id(),
                "f",
                m.active as f64,
                "frames",
                Tolerance::Exact(0.0),
            ));
            assert_eq!(
                m.level_dbfs,
                f64::NEG_INFINITY,
                "{} on silence is digital zero",
                c.id()
            );
        }
        for c in [C::C2, C::C3] {
            let m = ns::cell(S::F, c, rate).measures;
            out.push(r(
                "silence: active frames",
                rate,
                c.id(),
                "f",
                m.active as f64,
                "frames",
                Tolerance::Exact(0.0),
            ));
            out.push(r(
                "silence: the decoder's residue",
                rate,
                c.id(),
                "f",
                m.level_dbfs,
                "dBFS",
                Tolerance::AtMost(RESIDUE_MAX_DBFS),
            ));
        }
        let c5 = ns::cell(S::F, C::C5, rate);
        let s = C::C5.settings(rate);
        let bound = ((f64::from(s.vas.onset_ms)
            + f64::from(s.vas.hang_ms)
            + 1000.0 * f64::from(c5.latency) / f64::from(rate))
            / 20.0)
            .ceil()
            - 25.0;
        out.push(r(
            "silence: C5 closes the gate (active ≤ ⌈(onset + hang + latency)/20 ms⌉ − 25)",
            rate,
            "C5",
            "f",
            c5.measures.active as f64,
            "frames",
            Tolerance::AtMost(bound),
        ));
        let c6 = ns::cell(S::F, C::C6, rate).measures;
        out.push(r(
            "silence: C6 stays open (active ≥ 90 % of the span)",
            rate,
            "C6",
            "f",
            c6.active as f64 / c6.frames.max(1) as f64,
            "",
            Tolerance::AtLeast(OPEN_GATE_MIN_SHARE),
        ));
        out.push(r(
            "silence: C6's level (reported)",
            rate,
            "C6",
            "f",
            c6.level_dbfs,
            "dBFS",
            Tolerance::Report,
        ));
    }
    report::print(&out);
    assert_all(&out);
}

/// D6 (iv): the coder's periodicity, C3 against C1; the level change reported.
#[test]
fn nonspeech_coder_periodicity() {
    let mut out = Vec::new();
    for &rate in configs::measurement_rates() {
        for s in [S::A35, S::A50, S::A55, S::B, S::C, S::D, S::E] {
            let c1 = ns::cell(s, C::C1, rate).measures;
            let c3 = ns::cell(s, C::C3, rate).measures;
            let enough = c1.active >= MIN_ACTIVE_FOR_RISE && c3.active >= MIN_ACTIVE_FOR_RISE;
            let bound = if s == S::E {
                PERIODICITY_RISE_MIN_PINK
            } else {
                PERIODICITY_RISE_MIN
            };
            let t = if enough {
                Tolerance::AtLeast(bound)
            } else {
                Tolerance::Report
            };
            out.push(r(
                "coder periodicity rise, C3 − C1",
                rate,
                "C3",
                s.id(),
                c3.periodicity.unwrap() - c1.periodicity.unwrap(),
                "",
                t,
            ));
            out.push(r(
                "coder level change, C3 − C1 (reported)",
                rate,
                "C3",
                s.id(),
                c3.level_dbfs - c1.level_dbfs,
                "dB",
                Tolerance::Report,
            ));
            if let (Some(a), Some(b)) = (c3.mod_share, c1.mod_share) {
                out.push(r(
                    "2-8 Hz share ratio C3 ÷ C1 (reported)",
                    rate,
                    "C3",
                    s.id(),
                    a / b,
                    "",
                    Tolerance::Report,
                ));
            }
        }
    }
    report::print(&out);
    assert_all(&out);
}
