//! Measurement results and one check function per spec requirement (001 tasks.md T022, T048;
//! spec 004 tasks.md T019–T022, T030).
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
    /// Report only: always passes. For values no source constrains (spec 004 FR-005, FR-008:
    /// the stage loss at 2700 and 2900 Hz).
    Report,
}

impl Tolerance {
    /// Whether `measured` satisfies this tolerance. NaN never does.
    pub fn contains(self, measured: f64) -> bool {
        match self {
            Tolerance::Range(lo, hi) => (lo..=hi).contains(&measured),
            Tolerance::AtLeast(min) => measured >= min,
            Tolerance::AtMost(max) => measured <= max,
            Tolerance::Exact(value) => measured == value,
            Tolerance::Report => !measured.is_nan(),
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
            Tolerance::Report => f.write_str("report"),
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

/// Trace: the codec templates, S-005 scaled by 0.75 (A-027; spec 004 FR-005, FR-006, FR-008).
const TEMPLATE: &[&str] = &["A-027", "S-005"];
/// Trace: the record stage's DC attenuation (engineering target: ITU-T G.712, S-007, states no
/// DC requirement; spec 004 FR-005, Clarifications).
const DC_TARGET: &[&str] = &["engineering target", "S-007"];
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
    let produced = p.process(input, &mut out).expect("equal lengths").produced;
    out.truncate(produced);
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

/// Trace: the rate conversion boundary's engineering targets (spec 004 FR-003).
const BOUNDARY_TARGET: &[&str] = &["engineering target (004 FR-003)"];
/// Rejection frequencies for the boundary (spec 004 FR-003), used where below host Nyquist.
const BOUNDARY_REJECT_HZ: [f64; 4] = [3000.0, 3450.0, 6000.0, 20_000.0];

/// Spec 004 FR-003 (engineering targets, replacing 001 FR-005): the rate conversion boundary
/// (`bypass_all`) is flat from 50 to 2550 Hz within ±0.1 dB (minimum and maximum reported
/// separately), rejects 3000 Hz, 3450 Hz, 6000 Hz and 20 kHz by ≥ 60 dB where they lie below
/// the host Nyquist frequency, and keeps alias and image products ≥ 60 dB below a 1 kHz tone.
/// At the 6 kHz host rate the boundary is the identity (FR-002), so the only row is an exact
/// pass-through assertion on a noise stimulus.
pub fn check_fr003_boundary(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let base = configs::settings("bypass_all", rate);
    let fs = f64::from(rate);
    let mr = |property, stim: &str, measured, unit, tol| {
        MeasurementResult::new(
            "004/FR-003",
            property,
            BOUNDARY_TARGET,
            rate,
            "bypass_all",
            stim,
            measured,
            unit,
            tol,
        )
    };
    if rate == rr_dr60::DEVICE_RATE_HZ {
        // Identity (FR-002): N in, N out, every output bit equals its input bit.
        let x = stimulus::noise(0x0D60, rate as usize);
        let y = run(make, base, &x);
        let differing = if y.len() == x.len() {
            x.iter()
                .zip(&y)
                .filter(|(a, b)| a.to_bits() != b.to_bits())
                .count()
        } else {
            x.len().max(y.len())
        };
        return vec![mr(
            "identity: samples differing from input",
            "noise 1 s",
            differing as f64,
            "samples",
            Tolerance::Exact(0.0), // 004 FR-002
        )];
    }
    let sb = Spectrum::from_impulse_response(&impulse_response(make, base), fs, IR_LEN);
    let (mut g_min, mut g_max) = (f64::INFINITY, f64::NEG_INFINITY);
    for k in sb.range(50.0, 2550.0) {
        let g = analysis::db(sb.bins[k].norm());
        g_min = g_min.min(g);
        g_max = g_max.max(g);
    }
    let mut out = vec![
        mr(
            "flatness 50-2550 Hz (min)",
            "impulse",
            g_min,
            "dB",
            Tolerance::AtLeast(-0.1), // 004 FR-003, engineering target
        ),
        mr(
            "flatness 50-2550 Hz (max)",
            "impulse",
            g_max,
            "dB",
            Tolerance::AtMost(0.1), // 004 FR-003, engineering target
        ),
    ];
    for f in BOUNDARY_REJECT_HZ {
        if f < fs / 2.0 {
            out.push(mr(
                "rejection >= 3000 Hz",
                &format!("tone {f} Hz"),
                -tone_power_db(make, base, f),
                "dB",
                Tolerance::AtLeast(60.0), // 004 FR-003, engineering target
            ));
        }
    }
    let x = stimulus::tone(1000.0, 0.25, fs, rate as usize);
    let r = -analysis::residual_power_db(&x, &run(make, base, &x), 1000.0, fs, rate as usize / 5);
    out.push(mr(
        "alias/image products below tone",
        "tone 1000 Hz",
        r,
        "dB",
        Tolerance::AtLeast(60.0), // 004 FR-003, engineering target
    ));
    out
}

/// The reference frequency for stage loss: 0.75 × the datasheet's 1020 Hz (A-027).
pub const REF_HZ: f64 = 765.0;
/// The templates' top edge: 0.75 × 3400 Hz (A-027).
pub const EDGE_HZ: f64 = 2550.0;
/// Group-delay distortion is relative to the stage's minimum group delay over this band (S-005).
const GDD_BAND: (f64, f64) = (225.0, 2550.0);
/// Report-only frequencies: no source constrains the response between 2550 Hz and the 3 kHz
/// Nyquist frequency (spec 004 FR-005, FR-008, Assumptions).
const REPORT_HZ: [f64; 2] = [2700.0, 2900.0];
/// The anti-alias set (spec 004 definitions; engineering target), used below the host Nyquist.
pub const ANTI_ALIAS_HZ: [f64; 8] = [
    3450.0, 4000.0, 5000.0, 7000.0, 10_000.0, 15_000.0, 20_000.0, 40_000.0,
];

/// One stage's template: the datasheet limits scaled by 0.75 (A-027, S-005), as spec 004
/// FR-005/FR-006 (record) and FR-008 (playback) state them (data-model.md › stage templates).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Template {
    /// Spec requirement the loss rows verify.
    pub requirement: &'static str,
    /// Spec requirement the group-delay-distortion rows verify.
    pub gdd_requirement: &'static str,
    /// Lower in-band loss limit in dB relative to 765 Hz (negative: gain allowed).
    pub loss_lo_db: f64,
    /// Upper in-band loss limit in dB relative to 765 Hz.
    pub loss_hi_db: f64,
    /// Where the in-band limits start: 225 Hz (record) or DC (playback).
    pub band_lo_hz: f64,
    /// The stage rejects low frequencies (record) or passes DC (playback).
    pub low_rejection: bool,
    /// Group-delay-distortion limits in ms at the measured frequencies.
    pub gdd_limits_ms: [(f64, f64); 7],
}

impl Template {
    /// Stage 4: S-005 transmit template (spec 004 FR-005, FR-006).
    pub fn record() -> Self {
        Self {
            requirement: "004/FR-005",
            gdd_requirement: "004/FR-006",
            loss_lo_db: -0.15,
            loss_hi_db: 0.2,
            band_lo_hz: 225.0,
            low_rejection: true,
            gdd_limits_ms: [
                (375.0, 1.0),
                (450.0, 0.47),
                (750.0, 0.17),
                (1000.0, 0.17),
                (1500.0, 0.17),
                (1950.0, 0.17),
                (2100.0, 1.0),
            ],
        }
    }

    /// Stage 10: S-005 receive template, a low-pass that passes DC (spec 004 FR-008).
    pub fn playback() -> Self {
        Self {
            requirement: "004/FR-008",
            gdd_requirement: "004/FR-008",
            loss_lo_db: -0.15, // carried from the transmit row (engineering target, FR-015)
            loss_hi_db: 0.25,
            band_lo_hz: 0.0,
            low_rejection: false,
            gdd_limits_ms: [
                (375.0, 0.17),
                (450.0, 0.17),
                (750.0, 0.17),
                (1000.0, 0.17),
                (1500.0, 0.17),
                (1950.0, 0.17),
                (2100.0, 1.0),
            ],
        }
    }

    /// The template of a stage.
    pub fn of(stage: Stage) -> Self {
        match stage {
            Stage::Record => Self::record(),
            Stage::Playback => Self::playback(),
        }
    }
}

/// Steady-state mean of the last fifth of `y`.
fn steady_mean(y: &[f32]) -> f64 {
    let tail = &y[y.len() - y.len() / 5..];
    tail.iter().map(|&v| f64::from(v)).sum::<f64>() / tail.len() as f64
}

/// Spec 004 FR-005/FR-006 (record) and FR-008 (playback): one stage against its template,
/// measured as the configuration's response divided by the fully bypassed response. Loss is
/// relative to the 765 Hz gain, which is absolute. Group-delay distortion is relative to the
/// minimum group delay over 225–2550 Hz; the group delay at the band edge is evaluated at exact
/// frequencies with the DTFT, not from adjacent FFT bins (research R-07).
pub fn check_stage_template(rate: u32, stage: Stage, make: Make<'_>) -> Vec<MeasurementResult> {
    let t = Template::of(stage);
    let name = stage.config();
    let fs = f64::from(rate);
    let ir_c = impulse_response(make, configs::settings(name, rate));
    let ir_b = impulse_response(make, configs::settings("bypass_all", rate));
    let st = Spectrum::from_impulse_response(&ir_c, fs, IR_LEN)
        .divide(&Spectrum::from_impulse_response(&ir_b, fs, IR_LEN));
    let exact = |f: f64| analysis::dtft(&ir_c, f, fs) / analysis::dtft(&ir_b, f, fs);
    let g_ref = analysis::db(exact(REF_HZ).norm());
    let loss_exact = |f: f64| g_ref - analysis::db(exact(f).norm());
    let loss_bin = |k: usize| g_ref - analysis::db(st.bins[k].norm());
    let ms = |samples: f64| samples / fs * 1000.0;
    // Group delay at an exact frequency, from the phase slope over ±1 Hz.
    let gd_exact_ms = |f: f64| {
        let dphi = (exact(f + 1.0) / exact(f - 1.0)).arg();
        ms(-dphi / (std::f64::consts::TAU * 2.0 / fs))
    };
    let mr = |req, property, trace, stim: &str, measured, unit, tol| {
        MeasurementResult::new(req, property, trace, rate, name, stim, measured, unit, tol)
    };
    let mut out = vec![mr(
        t.requirement,
        "gain at 765 Hz",
        GAIN,
        "impulse",
        g_ref,
        "dB",
        Tolerance::Range(-0.1, 0.1), // A-015; ±0.1 dB engineering target
    )];

    // In-band loss, minimum and maximum tracked separately (asymmetric limits).
    let lo = if t.low_rejection {
        t.band_lo_hz
    } else {
        st.resolution()
    };
    let (mut l_min, mut l_max) = (f64::INFINITY, f64::NEG_INFINITY);
    for k in st.range(lo, 2250.0) {
        let l = loss_bin(k);
        l_min = l_min.min(l);
        l_max = l_max.max(l);
    }
    let band_label = if t.low_rejection {
        ("loss 225-2250 Hz (min)", "loss 225-2250 Hz (max)")
    } else {
        ("loss DC-2250 Hz (min)", "loss DC-2250 Hz (max)")
    };
    out.push(mr(
        t.requirement,
        band_label.0,
        TEMPLATE,
        "impulse",
        l_min,
        "dB",
        Tolerance::AtLeast(t.loss_lo_db),
    ));
    out.push(mr(
        t.requirement,
        band_label.1,
        TEMPLATE,
        "impulse",
        l_max,
        "dB",
        Tolerance::AtMost(t.loss_hi_db),
    ));
    out.push(mr(
        t.requirement,
        "loss at 2550 Hz",
        TEMPLATE,
        "impulse",
        loss_exact(EDGE_HZ),
        "dB",
        Tolerance::Range(0.0, 0.8), // S-005 3400 Hz row scaled (A-027)
    ));

    // Low end: steady DC through the configuration and the baseline (the spectrum's DC bin is
    // unreliable for a high-pass with zeros at z = 1, research R-07).
    let dc_in = stimulus::dc(0.5, rate as usize);
    let dc_c = steady_mean(&run(make, configs::settings(name, rate), &dc_in));
    let dc_b = steady_mean(&run(make, configs::settings("bypass_all", rate), &dc_in));
    if t.low_rejection {
        for f in [45.0, 20.0] {
            out.push(mr(
                t.requirement,
                if f == 45.0 {
                    "attenuation at 45 Hz"
                } else {
                    "attenuation at 20 Hz"
                },
                TEMPLATE,
                "impulse",
                loss_exact(f),
                "dB",
                Tolerance::AtLeast(20.0), // S-005 60 Hz row scaled (A-027)
            ));
        }
        // Amplitude ratio floored at 1e-15 (power 1e-30) and the result clamped to 200 dB
        // (engineering target, spec 004 FR-015): the record stage's DC output is exactly 0.0.
        let atten = (-analysis::db((dc_c.abs() / dc_b.abs()).max(1e-15))).min(200.0);
        out.push(mr(
            t.requirement,
            "attenuation at DC",
            DC_TARGET,
            "dc 0.5, steady state",
            atten,
            "dB",
            Tolerance::AtLeast(40.0), // engineering target (G.712 states no DC requirement)
        ));
    } else {
        out.push(mr(
            t.requirement,
            "loss at DC (passes DC)",
            TEMPLATE,
            "dc 0.5, steady state",
            -analysis::db(dc_c / dc_b),
            "dB",
            Tolerance::Range(t.loss_lo_db, t.loss_hi_db), // S-005 receive: no low-frequency rejection
        ));
        for f in [20.0, 45.0, 100.0] {
            out.push(mr(
                t.requirement,
                "loss at 20/45/100 Hz (passes DC)",
                TEMPLATE,
                "impulse",
                loss_exact(f),
                "dB",
                Tolerance::Range(t.loss_lo_db, t.loss_hi_db),
            ));
        }
    }

    for f in REPORT_HZ {
        out.push(mr(
            t.requirement,
            if f == 2700.0 {
                "loss at 2700 Hz"
            } else {
                "loss at 2900 Hz"
            },
            TARGET,
            "impulse",
            loss_exact(f),
            "dB",
            Tolerance::Report, // no source constrains 2550–3000 Hz (spec 004 Assumptions)
        ));
    }

    // Group delay: the band minimum from the FFT grid (smooth mid-band, bias ≤ 0.001 ms), the
    // rows at exact frequencies.
    let gd_min = st
        .range(GDD_BAND.0, GDD_BAND.1)
        .map(|k| ms(st.group_delay_samples(st.freq(k))))
        .fold(f64::INFINITY, f64::min);
    for (f, limit) in t.gdd_limits_ms {
        let property = match f as u32 {
            375 => "group delay distortion at 375 Hz",
            450 => "group delay distortion at 450 Hz",
            750 => "group delay distortion at 750 Hz",
            1000 => "group delay distortion at 1000 Hz",
            1500 => "group delay distortion at 1500 Hz",
            1950 => "group delay distortion at 1950 Hz",
            _ => "group delay distortion at 2100 Hz",
        };
        out.push(mr(
            t.gdd_requirement,
            property,
            TEMPLATE,
            "impulse",
            gd_exact_ms(f) - gd_min,
            "ms",
            Tolerance::AtMost(limit), // S-005 group-delay rows ÷ 0.75 (A-027)
        ));
    }
    let gd1k = gd_exact_ms(1000.0);
    out.push(mr(
        t.requirement,
        "group delay at 1 kHz",
        PHASE,
        "impulse",
        gd1k,
        "ms",
        Tolerance::AtMost(2.0),
    ));
    if t.low_rejection {
        out.push(mr(
            t.requirement,
            "group delay 225 Hz minus 1 kHz",
            PHASE,
            "impulse",
            gd_exact_ms(225.0) - gd1k,
            "ms",
            Tolerance::AtLeast(f64::MIN_POSITIVE), // A-016: delay rises toward the edges
        ));
    }
    let gd_edge = gd_exact_ms(EDGE_HZ);
    out.push(mr(
        t.requirement,
        "group delay 2550 Hz minus 1 kHz",
        PHASE,
        "impulse",
        gd_edge - gd1k,
        "ms",
        Tolerance::AtLeast(f64::MIN_POSITIVE), // A-016
    ));
    out.push(mr(
        t.requirement,
        "group delay at 2550 Hz",
        PHASE_TARGET,
        "impulse (DTFT at 2549/2551 Hz)",
        gd_edge,
        "ms",
        Tolerance::AtMost(2.0), // engineering target in A-016's spirit (spec 004 Clarifications)
    ));
    out
}

/// Spec 004 FR-005/FR-008 phase (A-016): at the 6 kHz host rate, where the boundary is the
/// identity, each stage's phase is within ±5° of the minimum-phase phase computed from its
/// magnitude over 225–2550 Hz. If the cepstral reconstruction misbehaves at the band edge, the
/// check falls back to 225–2400 Hz and says so (research R-07; the coefficient zero-radius unit
/// tests cover the edge).
pub fn check_minphase(make: Make<'_>) -> Vec<MeasurementResult> {
    let identity = rr_dr60::DEVICE_RATE_HZ;
    [Stage::Record, Stage::Playback]
        .into_iter()
        .map(|stage| {
            let ir = impulse_response(make, configs::settings(stage.config(), identity));
            let sp = Spectrum::from_impulse_response(&ir, f64::from(identity), IR_LEN);
            let mut d = analysis::phase_deviation_deg(&sp, 225.0, 2550.0);
            let mut property = "phase vs minimum phase 225-2550 Hz";
            if d > 5.0 {
                d = analysis::phase_deviation_deg(&sp, 225.0, 2400.0);
                property = "phase vs minimum phase 225-2400 Hz (edge excluded, R-07 fallback)";
            }
            MeasurementResult::new(
                Template::of(stage).requirement,
                property,
                PHASE_TARGET,
                identity,
                stage.config(),
                "impulse",
                d,
                "deg",
                Tolerance::AtMost(5.0), // A-016; ±5° is an engineering target
            )
        })
        .collect()
}

/// Spec 004 FR-007 (S-005 discrimination row scaled, A-027): through the record stage alone at
/// a host rate above 6 kHz, each anti-alias tone below the host Nyquist frequency is at least
/// 30 dB down in total output power (aliases included) relative to the 765 Hz tone. The rate
/// conversion boundary gives ≥ 60 dB; the check exists because this is the device requirement
/// the boundary stands in for.
pub fn check_fr007_antialias(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    if rate <= rr_dr60::DEVICE_RATE_HZ {
        return Vec::new();
    }
    let fs = f64::from(rate);
    let s = configs::settings("record_only", rate);
    let reference = tone_power_db(make, s, REF_HZ);
    ANTI_ALIAS_HZ
        .into_iter()
        .filter(|&f| f < fs / 2.0)
        .map(|f| {
            MeasurementResult::new(
                "004/FR-007",
                "anti-alias rejection (total power incl. aliases)",
                TEMPLATE,
                rate,
                "record_only",
                format!("tone {f} Hz vs 765 Hz"),
                reference - tone_power_db(make, s, f),
                "dB",
                Tolerance::AtLeast(30.0), // S-005 "4.6–72 kHz ≥ 30 dB" scaled to 3450 Hz
            )
        })
        .collect()
}

/// Spec 004 FR-009: with both stages on, the response equals R_rec + R_play − R_base within
/// ±0.3 dB from 50 to 2900 Hz wherever that is above −40 dB, and the 765 Hz gain is 0 ± 0.2 dB.
pub fn check_fr009_cascade(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let [d, r, p, b] = ["default", "record_only", "playback_only", "bypass_all"]
        .map(|c| impulse_response(make, configs::settings(c, rate)));
    let at = |ir: &[f32], f: f64| analysis::db(analysis::dtft(ir, f, fs).norm());
    let (points, f_lo, f_hi) = (48, 50.0f64, 2900.0f64);
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
            "004/FR-009",
            "cascade vs R_rec+R_play-R_base (max |dev|)",
            TARGET,
            rate,
            "default",
            "impulse, 48 log points 50-2900 Hz",
            worst,
            "dB",
            Tolerance::AtMost(0.3), // engineering target
        ),
        MeasurementResult::new(
            "004/FR-009",
            "gain at 765 Hz",
            GAIN,
            rate,
            "default",
            "impulse",
            at(&d, REF_HZ),
            "dB",
            Tolerance::Range(-0.2, 0.2), // A-015
        ),
    ]
}

/// Trace: the latency requirement (spec 004 FR-004, 001 FR-012/FR-013 unchanged in form).
const LATENCY_TARGET: &[&str] = &["engineering target (004 FR-004)"];

/// Spec 004 FR-004 (001 FR-012 and FR-013 unchanged in form; SC-005): for each spec 001
/// configuration the reported latency equals the measured 1 kHz group delay within ±1 host
/// sample, and the default configuration's latency is at most 20 ms.
pub fn check_fr004_latency(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let mut out: Vec<MeasurementResult> = configs::CONFIGS
        .into_iter()
        .map(|config| {
            let s = configs::settings(config, rate);
            let reported = f64::from(make(s).latency_samples());
            let measured = analysis::group_delay_samples(
                |f| tone_response(make, s, f),
                1000.0,
                f64::from(rate),
            );
            MeasurementResult::new(
                "004/FR-004",
                "reported minus measured latency",
                LATENCY_TARGET,
                rate,
                config,
                format!("tones 999/1001 Hz; reported {reported}"),
                reported - measured,
                "samples",
                Tolerance::Range(-1.0, 1.0), // 004 FR-004, ±1 sample engineering target
            )
        })
        .collect();
    let ms = f64::from(make(configs::settings("default", rate)).latency_samples())
        / f64::from(rate)
        * 1000.0;
    out.push(MeasurementResult::new(
        "004/FR-004",
        "default latency",
        LATENCY_TARGET,
        rate,
        "default",
        "reported",
        ms,
        "ms",
        Tolerance::AtMost(20.0), // 004 FR-004, 20 ms budget (001 FR-013)
    ));
    out
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
