//! Spec 029 [TR: D3, D5; FR-002, FR-005]: the configurations' identities, the partitions, the
//! seven-rate table and the cross-rate rows (bounded on the open-gate cells; the rest reported).

use rr_dr60_harness::checks::{MeasurementResult, Tolerance, assert_all};
use rr_dr60_harness::nonspeech_checks::{self as ns, Configuration as C, Stimulus as S};
use rr_dr60_harness::{configs, report};

const TRACE: &[&str] = &["029 D3", "029 D5"];

/// D3: C4 with the limiter off is C4 (the stimuli stay below the knee), and C4 with the
/// microphone and the limiter off is C3 (one stage group per rung), bit for bit on every stimulus.
#[test]
fn nonspeech_configs() {
    for &rate in configs::measurement_rates() {
        for s in ns::STIMULI {
            let x = s.samples(rate);
            let c4 = ns::process_raw(C::C4.settings(rate), &x, &[ns::BLOCK]).0;
            let mut no_limiter = C::C4.settings(rate);
            no_limiter.agc.enabled = false;
            assert_eq!(
                ns::process_raw(no_limiter, &x, &[ns::BLOCK]).0,
                c4,
                "{} at {rate}: the limiter is unity below the knee",
                s.id()
            );
            let mut like_c3 = C::C4.settings(rate);
            like_c3.agc.enabled = false;
            like_c3.mic = rr_dr60::MicSettings::BYPASSED;
            let c3 = ns::process_raw(C::C3.settings(rate), &x, &[ns::BLOCK]).0;
            assert_eq!(
                ns::process_raw(like_c3, &x, &[ns::BLOCK]).0,
                c3,
                "{} at {rate}: C4 minus the microphone and limiter is C3",
                s.id()
            );
        }
    }
}

/// D5: the partition identity on C2 and C5 with a35 and g at 6 and 48 kHz.
#[test]
#[cfg_attr(coverage, ignore)]
fn nonspeech_partitions() {
    for rate in [6000, 48_000] {
        for (s, c) in [(S::A35, C::C2), (S::G, C::C5)] {
            let x = s.samples(rate);
            let base = ns::process_raw(c.settings(rate), &x, &[4096]).0;
            // Block size 1 at 6 kHz only (the plain-build cost; the irregular partition covers
            // the other rates' call boundaries).
            let parts: Vec<Vec<usize>> = if rate == 6000 {
                vec![vec![1usize], vec![64], vec![1, 7, 64, 1000, 4096, 3]]
            } else {
                vec![vec![64], vec![1, 7, 64, 1000, 4096, 3]]
            };
            for part in parts {
                assert_eq!(
                    ns::process_raw(c.settings(rate), &x, &part).0,
                    base,
                    "{} {} at {rate}: partition {part:?}",
                    s.id(),
                    c.id()
                );
            }
        }
    }
}

/// D5's cross-rate bound (plan R-05, P6; recorded 2026-11-04 from the first harness print as
/// 2 × the largest |rate − 6 kHz| over the open-gate rungs C0 – C4 and C6 on the steady,
/// modulated and pink rows a35, a50, a55, b, c, e, with the floors): level 0.89 → 1.8 dB, peaks
/// 0.314 → 0.7, periodicity 0.026 → 0.06, periodic fraction 0.031 → 0.07, share 0.097 → 0.2,
/// flux 0.27 → 0.6 dB, active frames ±1. A regression guard, not a precision figure. The bursts
/// row (few active frames; the decoder's continuation differs by rate), silence (the residue's
/// level moves with the engine's resampler), the reference (generated per rate), the resonator
/// row and the pausing rungs C5 / C7 are reported only.
const CROSS_RATE_TOL: [(&str, f64); 7] = [
    ("level", 1.8),
    ("peaks", 0.7),
    ("periodicity", 0.06),
    ("periodic fraction", 0.07),
    ("share", 0.2),
    ("flux", 0.6),
    ("active frames", 1.0),
];

/// D5: the seven-rate table printed, and every cell against its 6 kHz twin: bounded on the
/// open-gate cells, reported elsewhere (plan R-05, P6; the spec's first-print session).
#[test]
#[cfg_attr(coverage, ignore)]
fn nonspeech_rates() {
    let mut out = Vec::new();
    let base: Vec<ns::Cell> = ns::table(ns::DEVICE_HZ);
    for rate in configs::all_rates() {
        let t = ns::table(rate);
        print!("{}", ns::rate_text(rate));
        if rate == ns::DEVICE_HZ {
            continue;
        }
        for (c, b) in t.iter().zip(&base) {
            let bounded = matches!(
                c.configuration,
                C::C0 | C::C1 | C::C2 | C::C3 | C::C4 | C::C6
            ) && matches!(c.stimulus, S::A35 | S::A50 | S::A55 | S::B | S::C | S::E);
            let d = |a: Option<f64>, b: Option<f64>| match (a, b) {
                (Some(a), Some(b)) => Some((a - b).abs()),
                _ => None,
            };
            let level = if c.measures.level_dbfs.is_finite() && b.measures.level_dbfs.is_finite() {
                Some((c.measures.level_dbfs - b.measures.level_dbfs).abs())
            } else {
                None
            };
            let rows = [
                ("level", level, "dB"),
                ("peaks", d(c.measures.peaks, b.measures.peaks), ""),
                (
                    "periodicity",
                    d(c.measures.periodicity, b.measures.periodicity),
                    "",
                ),
                (
                    "periodic fraction",
                    d(c.measures.periodic_fraction, b.measures.periodic_fraction),
                    "",
                ),
                ("share", d(c.measures.mod_share, b.measures.mod_share), ""),
                ("flux", d(c.measures.flux_db, b.measures.flux_db), "dB"),
                (
                    "active frames",
                    Some((c.measures.active as f64 - b.measures.active as f64).abs()),
                    "frames",
                ),
            ];
            for (name, v, unit) in rows {
                let Some(v) = v else { continue };
                let tol = CROSS_RATE_TOL
                    .iter()
                    .find(|(n, _)| *n == name)
                    .map(|&(_, t)| t)
                    .unwrap();
                let (prop, t) = if bounded {
                    (
                        "cross-rate |rate − 6 kHz| (bounded)",
                        Tolerance::AtMost(tol),
                    )
                } else {
                    ("cross-rate |rate − 6 kHz| (reported)", Tolerance::Report)
                };
                out.push(MeasurementResult::new(
                    "029 D5",
                    prop,
                    TRACE,
                    rate,
                    c.configuration.id(),
                    format!("{} {name}", c.stimulus.id()),
                    v,
                    unit,
                    t,
                ));
            }
        }
    }
    report::print(&out);
    assert_all(&out);
}
