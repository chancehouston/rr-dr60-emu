//! Measurement results and one check function per spec requirement (tasks.md T022, T048).
//!
//! Every result carries the requirement it verifies and its trace (A-/S- IDs, or
//! "engineering target"), as FR-020 requires.

use std::fmt;

/// The pass condition for a measured value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tolerance {
    /// `lo ≤ measured ≤ hi`.
    Range(f64, f64),
    /// `measured ≥ min`.
    AtLeast(f64),
    /// `measured ≤ max`.
    AtMost(f64),
    /// `measured == value`.
    Exact(f64),
}

impl Tolerance {
    /// Whether `measured` satisfies this tolerance. NaN never does.
    pub fn contains(self, measured: f64) -> bool {
        match self {
            Tolerance::Range(lo, hi) => (lo..=hi).contains(&measured),
            Tolerance::AtLeast(min) => measured >= min,
            Tolerance::AtMost(max) => measured <= max,
            Tolerance::Exact(value) => measured == value,
        }
    }
}

/// Compact number formatting: scientific notation for very small or very large values.
fn num(v: f64) -> String {
    if v != 0.0 && (v.abs() < 1e-4 || v.abs() >= 1e7) {
        format!("{v:e}")
    } else {
        format!("{v}")
    }
}

impl fmt::Display for Tolerance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Tolerance::Range(lo, hi) => write!(f, "{}..={}", num(lo), num(hi)),
            Tolerance::AtLeast(min) => write!(f, ">= {}", num(min)),
            Tolerance::AtMost(max) => write!(f, "<= {}", num(max)),
            Tolerance::Exact(value) => write!(f, "== {}", num(value)),
        }
    }
}

/// One harness check (data-model.md › Measurement result).
#[derive(Clone, Debug, PartialEq)]
pub struct MeasurementResult {
    /// Spec requirement verified, e.g. `"FR-010"`.
    pub requirement: &'static str,
    /// What was measured, e.g. `"upper -3 dB point"`.
    pub property: &'static str,
    /// Trace IDs, e.g. `["A-002", "A-014"]`, or `["engineering target"]`.
    pub trace: &'static [&'static str],
    /// Host rate of the run, in Hz.
    pub host_rate_hz: u32,
    /// Named configuration, e.g. `"record_only"`.
    pub config: &'static str,
    /// Stimulus description, e.g. `"tone 3400 Hz -20 dBFS"`.
    pub stimulus: String,
    /// Measured value.
    pub measured: f64,
    /// Unit of `measured` and of the tolerance, e.g. `"Hz"`.
    pub unit: &'static str,
    /// Pass condition.
    pub tolerance: Tolerance,
    /// Whether `measured` satisfies `tolerance`.
    pub passed: bool,
}

impl MeasurementResult {
    /// Builds a result and evaluates `passed` from the tolerance.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        requirement: &'static str,
        property: &'static str,
        trace: &'static [&'static str],
        host_rate_hz: u32,
        config: &'static str,
        stimulus: impl Into<String>,
        measured: f64,
        unit: &'static str,
        tolerance: Tolerance,
    ) -> Self {
        Self {
            requirement,
            property,
            trace,
            host_rate_hz,
            config,
            stimulus: stimulus.into(),
            measured,
            unit,
            tolerance,
            passed: tolerance.contains(measured),
        }
    }
}

impl fmt::Display for MeasurementResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} [{}] {} Hz {} ({}): measured {:.4} {}, tolerance {} {} — {}",
            self.requirement,
            self.property,
            self.trace.join(","),
            self.host_rate_hz,
            self.config,
            self.stimulus,
            self.measured,
            self.unit,
            self.tolerance,
            self.unit,
            if self.passed { "PASS" } else { "FAIL" }
        )
    }
}

/// Panics with a table of every failed result if any check failed.
pub fn assert_all(results: &[MeasurementResult]) {
    let failures: Vec<String> = results
        .iter()
        .filter(|r| !r.passed)
        .map(ToString::to_string)
        .collect();
    assert!(
        failures.is_empty(),
        "{} of {} checks failed:\n  {}",
        failures.len(),
        results.len(),
        failures.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// Check functions (tasks.md T048). Each returns results for one spec requirement.
// ---------------------------------------------------------------------------

use rr_dr60::{Pipeline, Settings};

use crate::analysis::{self, Spectrum};
use crate::{configs, stimulus};

/// Builds a pipeline for some settings. The checks take one, so a deliberately broken
/// pipeline can be measured too (SC-008 mutation test).
pub type Make<'a> = &'a dyn Fn(Settings) -> Pipeline;

/// The standard factory: `Pipeline::new`.
///
/// # Panics
///
/// If the settings are rejected.
pub fn standard(settings: Settings) -> Pipeline {
    Pipeline::new(settings).expect("supported settings")
}

/// Impulse-response length (and FFT size): 0.68 s even at 96 kHz, far longer than any tail.
const IR_LEN: usize = 65_536;

/// Trace: the voice-band band shape (A-002, A-014).
const BAND: &[&str] = &["A-002", "A-014"];
/// Trace: unity passband gain (A-015).
const GAIN: &[&str] = &["A-015"];
/// Trace: minimum-phase codec filters (A-016).
const PHASE: &[&str] = &["A-016"];
/// Trace: emulator engineering targets (FR-018).
const TARGET: &[&str] = &["engineering target"];
/// Trace: minimum-phase with an engineering-target tolerance.
const PHASE_TARGET: &[&str] = &["A-016", "engineering target"];

fn run(make: Make<'_>, settings: Settings, input: &[f32]) -> Vec<f32> {
    let mut p = make(settings);
    let mut out = vec![0.0; input.len()];
    p.process(input, &mut out).expect("equal lengths");
    out
}

/// Impulse response, with trailing exact zeros trimmed (the tails flush to 0.0, R-05).
fn impulse_response(make: Make<'_>, settings: Settings) -> Vec<f32> {
    let mut ir = run(make, settings, &stimulus::impulse(IR_LEN));
    while ir.last() == Some(&0.0) {
        ir.pop();
    }
    ir
}

/// Output power relative to input for a 1 s tone at `freq_hz` (first 0.2 s discarded), in dB.
fn tone_power_db(make: Make<'_>, settings: Settings, freq_hz: f64) -> f64 {
    let fs = settings.host_rate_hz;
    let x = stimulus::tone(freq_hz, 0.25, f64::from(fs), fs as usize);
    analysis::power_ratio_db(&x, &run(make, settings, &x), fs as usize / 5)
}

/// Complex gain for a 1 s tone at `freq_hz` (first 0.2 s discarded).
fn tone_response(make: Make<'_>, settings: Settings, freq_hz: f64) -> analysis::Complex64 {
    let fs = settings.host_rate_hz;
    let x = stimulus::tone(freq_hz, 0.25, f64::from(fs), fs as usize);
    analysis::gain_and_phase(
        &x,
        &run(make, settings, &x),
        freq_hz,
        f64::from(fs),
        fs as usize / 5,
    )
}

/// A voice-band stage, measured with the other stage bypassed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Signal-chain stage 4 (`record_only` configuration).
    Record,
    /// Signal-chain stage 10 (`playback_only` configuration).
    Playback,
}

impl Stage {
    /// The configuration that isolates this stage.
    pub fn config(self) -> &'static str {
        match self {
            Stage::Record => "record_only",
            Stage::Playback => "playback_only",
        }
    }
}

/// FR-005: the rate conversion boundary (`bypass_all`) is flat from 50 to 3600 Hz within
/// ±0.1 dB, rejects 4000 Hz up to Nyquist by ≥ 60 dB, and keeps alias and image products
/// ≥ 60 dB down. All engineering targets. Checks at or above host Nyquist are skipped.
pub fn check_fr005_boundary(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let base = configs::settings("bypass_all", rate);
    let fs = f64::from(rate);
    let sb = Spectrum::from_impulse_response(&impulse_response(make, base), fs, IR_LEN);
    let flat = sb
        .range(50.0, 3600.0)
        .map(|k| analysis::db(sb.bins[k].norm()).abs())
        .fold(0.0, f64::max);
    let mut out = vec![MeasurementResult::new(
        "FR-005",
        "flatness 50-3600 Hz (max |dev|)",
        TARGET,
        rate,
        "bypass_all",
        "impulse",
        flat,
        "dB",
        Tolerance::AtMost(0.1), // FR-005, engineering target
    )];
    for f in [
        4000.0,
        4600.0,
        6000.0,
        8000.0,
        12_000.0,
        20_000.0,
        0.45 * fs,
    ] {
        if f >= 4000.0 && f < fs / 2.0 {
            let a = -tone_power_db(make, base, f);
            out.push(MeasurementResult::new(
                "FR-005",
                "rejection >= 4000 Hz",
                TARGET,
                rate,
                "bypass_all",
                format!("tone {f} Hz"),
                a,
                "dB",
                Tolerance::AtLeast(60.0), // FR-005, engineering target
            ));
        }
    }
    let x = stimulus::tone(1000.0, 0.25, fs, rate as usize);
    let r = -analysis::residual_power_db(&x, &run(make, base, &x), 1000.0, fs, rate as usize / 5);
    out.push(MeasurementResult::new(
        "FR-005",
        "alias/image products below tone",
        TARGET,
        rate,
        "bypass_all",
        "tone 1000 Hz",
        r,
        "dB",
        Tolerance::AtLeast(60.0), // FR-005, engineering target
    ));
    out
}

/// FR-010: one voice-band stage, measured as the configuration's response divided by the
/// fully bypassed response. The 1 kHz gain is absolute; other magnitudes are relative to it.
pub fn check_fr010_stage(rate: u32, stage: Stage, make: Make<'_>) -> Vec<MeasurementResult> {
    let name = stage.config();
    let fs = f64::from(rate);
    let ir_c = impulse_response(make, configs::settings(name, rate));
    let ir_b = impulse_response(make, configs::settings("bypass_all", rate));
    let st = Spectrum::from_impulse_response(&ir_c, fs, IR_LEN)
        .divide(&Spectrum::from_impulse_response(&ir_b, fs, IR_LEN));
    let exact =
        |f: f64| analysis::db((analysis::dtft(&ir_c, f, fs) / analysis::dtft(&ir_b, f, fs)).norm());
    let g1k = exact(1000.0);
    let rel = |k: usize| analysis::db(st.bins[k].norm()) - g1k;
    let mr = |property, trace, stim: &str, measured, unit, tol| {
        MeasurementResult::new(
            "FR-010", property, trace, rate, name, stim, measured, unit, tol,
        )
    };

    let lo3 = st
        .range(1.0, 1000.0)
        .filter(|&k| rel(k) < -3.0)
        .map(|k| st.freq(k))
        .fold(f64::NAN, f64::max);
    let hi3 = st
        .range(1000.0, 3990.0)
        .filter(|&k| rel(k) < -3.0)
        .map(|k| st.freq(k))
        .fold(f64::NAN, f64::min);
    let ripple = st
        .range(400.0, 3200.0)
        .map(|k| rel(k).abs())
        .fold(0.0, f64::max);
    let a60 = -st
        .range(1.0, 60.0)
        .map(rel)
        .fold(f64::NEG_INFINITY, f64::max);
    let adc = -(analysis::db(st.bins[0].norm()) - g1k);
    let ms = |samples: f64| samples / fs * 1000.0;
    let gd1k = ms(st.group_delay_samples(1000.0));

    let mut out = vec![
        mr(
            "gain at 1 kHz",
            GAIN,
            "impulse",
            g1k,
            "dB",
            Tolerance::Range(-0.1, 0.1), // FR-010, A-015
        ),
        mr(
            "lower -3 dB point",
            BAND,
            "impulse",
            lo3,
            "Hz",
            Tolerance::Range(250.0, 350.0), // FR-010, A-002, A-014
        ),
        mr(
            "upper -3 dB point",
            BAND,
            "impulse",
            hi3,
            "Hz",
            Tolerance::Range(3350.0, 3450.0), // FR-010, A-002, A-014
        ),
        mr(
            "ripple 400-3200 Hz (max |dev|)",
            BAND,
            "impulse",
            ripple,
            "dB",
            Tolerance::AtMost(0.5), // FR-010, A-014
        ),
        mr(
            "attenuation <= 60 Hz",
            BAND,
            "impulse",
            a60,
            "dB",
            Tolerance::AtLeast(20.0), // FR-010, A-014
        ),
        mr(
            "attenuation at DC",
            BAND,
            "impulse",
            adc,
            "dB",
            Tolerance::AtLeast(40.0), // FR-010, A-014
        ),
        mr(
            "group delay at 1 kHz",
            PHASE,
            "impulse",
            gd1k,
            "ms",
            Tolerance::AtMost(2.0), // FR-010, A-016
        ),
        mr(
            "group delay 400 Hz minus 1 kHz",
            PHASE,
            "impulse",
            ms(st.group_delay_samples(400.0)) - gd1k,
            "ms",
            Tolerance::AtLeast(f64::MIN_POSITIVE),
        ),
        mr(
            "group delay 3200 Hz minus 1 kHz",
            PHASE,
            "impulse",
            ms(st.group_delay_samples(3200.0)) - gd1k,
            "ms",
            Tolerance::AtLeast(f64::MIN_POSITIVE),
        ),
    ];
    if rate > 8000 {
        // At 8 kHz, 4000 Hz is Nyquist, so the check is skipped (FR-005). Measured with tones:
        // the boundary rejects 4000 Hz by ~140 dB, which leaves an impulse response with only
        // numerical residue at that frequency, so a tone ratio is the accurate method here.
        let cfg = tone_response(make, configs::settings(name, rate), 4000.0);
        let base = tone_response(make, configs::settings("bypass_all", rate), 4000.0);
        let a4k = -(analysis::db((cfg / base).norm()) - g1k);
        out.push(mr(
            "attenuation at 4000 Hz",
            BAND,
            "tone 4000 Hz",
            a4k,
            "dB",
            Tolerance::AtLeast(14.0), // FR-010, A-014
        ));
    }
    for f in [4600.0, 6000.0, 10_000.0, 20_000.0] {
        if f < fs / 2.0 {
            let a = -tone_power_db(make, configs::settings(name, rate), f);
            out.push(mr(
                "total power >= 4600 Hz",
                BAND,
                &format!("tone {f} Hz"),
                a,
                "dB",
                Tolerance::AtLeast(25.0), // FR-010, A-014
            ));
        }
    }
    out
}

/// FR-010 phase: at the 8 kHz host rate (where the boundary is the identity), each stage's
/// phase is within ±5° of the minimum-phase phase computed from its magnitude, 400–3200 Hz.
pub fn check_fr010_minphase(make: Make<'_>) -> Vec<MeasurementResult> {
    [Stage::Record, Stage::Playback]
        .into_iter()
        .map(|stage| {
            let ir = impulse_response(make, configs::settings(stage.config(), 8000));
            let d = analysis::phase_deviation_deg(
                &Spectrum::from_impulse_response(&ir, 8000.0, IR_LEN),
                400.0,
                3200.0,
            );
            MeasurementResult::new(
                "FR-010",
                "phase vs minimum phase 400-3200 Hz",
                PHASE_TARGET,
                8000,
                stage.config(),
                "impulse",
                d,
                "deg",
                Tolerance::AtMost(5.0), // FR-010, A-016; ±5° is an engineering target
            )
        })
        .collect()
}

/// FR-011: with both stages on, the response equals R_rec + R_play − R_base within ±0.3 dB
/// from 100 to 3900 Hz wherever that is above −40 dB, and the 1 kHz gain is 0 ± 0.2 dB.
pub fn check_fr011_cascade(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let [d, r, p, b] = ["default", "record_only", "playback_only", "bypass_all"]
        .map(|c| impulse_response(make, configs::settings(c, rate)));
    let at = |ir: &[f32], f: f64| analysis::db(analysis::dtft(ir, f, fs).norm());
    let (points, f_lo, f_hi) = (48, 100.0f64, 3900.0f64);
    let mut worst = 0.0f64;
    for i in 0..points {
        let f = f_lo
            * rr_dr60_detmath::exp(
                rr_dr60_detmath::ln(f_hi / f_lo) * i as f64 / (points - 1) as f64,
            );
        let predicted = at(&r, f) + at(&p, f) - at(&b, f);
        if predicted > -40.0 {
            worst = worst.max((at(&d, f) - predicted).abs());
        }
    }
    vec![
        MeasurementResult::new(
            "FR-011",
            "cascade vs R_rec+R_play-R_base (max |dev|)",
            TARGET,
            rate,
            "default",
            "impulse, 48 log points 100-3900 Hz",
            worst,
            "dB",
            Tolerance::AtMost(0.3), // FR-011, engineering target
        ),
        MeasurementResult::new(
            "FR-011",
            "gain at 1 kHz",
            GAIN,
            rate,
            "default",
            "impulse",
            at(&d, 1000.0),
            "dB",
            Tolerance::Range(-0.2, 0.2), // FR-011, A-015
        ),
    ]
}

/// FR-012: reported latency equals the measured group delay at 1 kHz within ±1 sample.
pub fn check_fr012_latency(
    rate: u32,
    config: &'static str,
    make: Make<'_>,
) -> Vec<MeasurementResult> {
    let s = configs::settings(config, rate);
    let reported = f64::from(make(s).latency_samples());
    let measured =
        analysis::group_delay_samples(|f| tone_response(make, s, f), 1000.0, f64::from(rate));
    vec![MeasurementResult::new(
        "FR-012",
        "reported minus measured latency",
        TARGET,
        rate,
        config,
        format!("tones 999/1001 Hz; reported {reported}"),
        reported - measured,
        "samples",
        Tolerance::Range(-1.0, 1.0), // FR-012, engineering target
    )]
}

/// FR-013: default-configuration latency is at most 20 ms.
pub fn check_fr013_latency(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let ms = f64::from(make(Settings::new(rate)).latency_samples()) / f64::from(rate) * 1000.0;
    vec![MeasurementResult::new(
        "FR-013",
        "default latency",
        TARGET,
        rate,
        "default",
        "reported",
        ms,
        "ms",
        Tolerance::AtMost(20.0), // FR-013, engineering target
    )]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(measured: f64, tolerance: Tolerance) -> MeasurementResult {
        MeasurementResult::new(
            "FR-010",
            "upper -3 dB point",
            &["A-002", "A-014"],
            48_000,
            "record_only",
            "sweep",
            measured,
            "Hz",
            tolerance,
        )
    }

    #[test]
    fn tolerance_contains() {
        assert!(Tolerance::Range(3350.0, 3450.0).contains(3406.0));
        assert!(!Tolerance::Range(3350.0, 3450.0).contains(3300.0));
        assert!(
            Tolerance::AtLeast(20.0).contains(20.0) && !Tolerance::AtLeast(20.0).contains(19.9)
        );
        assert!(Tolerance::AtMost(2.0).contains(1.9) && !Tolerance::AtMost(2.0).contains(2.1));
        assert!(Tolerance::Exact(0.0).contains(0.0) && !Tolerance::Exact(0.0).contains(1e-300));
        assert!(!Tolerance::AtLeast(0.0).contains(f64::NAN));
    }

    #[test]
    fn result_evaluates_passed_and_formats() {
        let ok = result(3406.0, Tolerance::Range(3350.0, 3450.0));
        assert!(ok.passed);
        let text = ok.to_string();
        for part in [
            "FR-010",
            "A-002,A-014",
            "48000",
            "record_only",
            "3406",
            "3350..=3450",
            "PASS",
        ] {
            assert!(text.contains(part), "{text} lacks {part}");
        }
        assert!(
            result(1.0, Tolerance::AtLeast(2.0))
                .to_string()
                .contains("FAIL")
        );
        assert!(Tolerance::AtMost(2.0).to_string().contains("<= 2"));
        assert!(Tolerance::Exact(1.0).to_string().contains("== 1"));
        assert!(Tolerance::AtLeast(1.0).to_string().contains(">= 1"));
        assert_eq!(
            Tolerance::AtLeast(f64::MIN_POSITIVE).to_string(),
            ">= 2.2250738585072014e-308"
        );
    }

    #[test]
    fn assert_all_passes_when_all_pass() {
        assert_all(&[result(3400.0, Tolerance::Range(3350.0, 3450.0))]);
        assert_all(&[]);
    }

    #[test]
    #[should_panic(expected = "1 of 2 checks failed")]
    fn assert_all_panics_on_failure() {
        assert_all(&[
            result(3400.0, Tolerance::AtMost(3450.0)),
            result(3500.0, Tolerance::AtMost(3450.0)),
        ]);
    }
}
