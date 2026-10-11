//! Spec 002 AGC checks: one function per requirement (research.md R-11; tasks.md T034).
//!
//! Every check runs the AGC-isolated configuration (`agc_only`: stages 4 and 10 bypassed,
//! tap after the AGC) unless stated otherwise, and reports `002/FR-0xx` with its trace IDs.
//! Gain trajectories use the **reference ratio**: output ÷ the same stimulus through the
//! same configuration with the AGC bypassed, sample by sample, where the reference is at least
//! 0.1 × the tone amplitude (R-11, corrected in T026). Tolerances that are not device
//! properties are engineering targets (spec 002 FR-015, R-15).

use rr_dr60::{AgcSettings, Pipeline, Settings, Tap};
use rr_dr60_detmath::{TAU, sin};

use crate::analysis::{db, gain_trajectory_exact, level_dbfs_aes17, single_bin, thd_n_db};
use crate::checks::{Make, MeasurementResult, Tolerance};
use crate::{configs, stimulus};

const CONFIG: &str = "agc_only";
const T_A017: &[&str] = &["A-017", "engineering target"];
const T_A018: &[&str] = &["A-018", "engineering target"];
const T_A019: &[&str] = &["A-019"];
const T_ENG: &[&str] = &["engineering target"];
/// The AGC detector's delay at the device rate: 31 samples at 8 kHz (research.md R-03).
const DETECTOR_DELAY_S: f64 = 31.0 / 8000.0;

/// A named AGC setting for the matrix (spec 002 FR-012).
#[derive(Clone, Copy, Debug)]
pub struct Case {
    /// Label for reports, e.g. `"attack min"`.
    pub label: &'static str,
    /// The AGC settings.
    pub agc: AgcSettings,
}

impl Case {
    /// Whether these are the device defaults (default tolerances apply).
    pub fn is_default(&self) -> bool {
        self.agc == AgcSettings::DEVICE
    }
}

/// The FR-012 settings matrix: the defaults, the two US2 examples, and the minimum and
/// maximum of each setting with the others at default.
pub fn matrix() -> Vec<Case> {
    let with = |label, f: fn(&mut AgcSettings)| {
        let mut agc = AgcSettings::DEVICE;
        f(&mut agc);
        Case { label, agc }
    };
    vec![
        with("defaults", |_| {}),
        with("release 3 s (US2 AS3)", |a| a.release_ms = 3000.0),
        with("target -20 dBFS (US2 AS4)", |a| a.target_dbfs = -20.0),
        with("target min", |a| a.target_dbfs = -30.0),
        with("target max", |a| a.target_dbfs = 0.0),
        with("max gain min", |a| a.max_gain_db = 0.0),
        with("max gain max", |a| a.max_gain_db = 60.0),
        with("max attenuation min", |a| a.max_attenuation_db = 0.0),
        with("max attenuation max", |a| a.max_attenuation_db = 40.0),
        with("attack min", |a| a.attack_ms = 1.0),
        with("attack max", |a| a.attack_ms = 100.0),
        with("release min", |a| a.release_ms = 50.0),
        with("release max", |a| a.release_ms = 10_000.0),
    ]
}

/// The regulated input range (spec 002 Overview): (T − G_max/0.9, T + A_max/0.9).
pub fn regulated_range(agc: &AgcSettings) -> (f64, f64) {
    let t = f64::from(agc.target_dbfs);
    (
        t - f64::from(agc.max_gain_db) / 0.9,
        t + f64::from(agc.max_attenuation_db) / 0.9,
    )
}

fn agc_only(rate: u32, agc: &AgcSettings) -> Settings {
    let mut s = configs::settings(CONFIG, rate);
    s.agc = *agc;
    s
}

fn run(make: Make<'_>, settings: Settings, x: &[f32]) -> Vec<f32> {
    let mut y = vec![0.0; x.len()];
    let produced = make(settings)
        .process(x, &mut y)
        .expect("equal lengths")
        .produced;
    y.truncate(produced);
    y
}

/// Seconds for a steady tone to settle from maximum gain: 20 × attack + detector delay +
/// 50 ms margin (R-11; engineering target).
fn settle_s(agc: &AgcSettings) -> f64 {
    20.0 * f64::from(agc.attack_ms) / 1000.0 + DETECTOR_DELAY_S + 0.05
}

/// Steady 1 kHz output level (dBFS, AES17) for a steady input level, over the last 0.25 s.
fn steady_level(make: Make<'_>, rate: u32, agc: &AgcSettings, input_dbfs: f64) -> f64 {
    let fs = f64::from(rate);
    let x = stimulus::step(1000.0, &[input_dbfs], &[settle_s(agc) + 0.25], fs);
    let y = run(make, agc_only(rate, agc), &x);
    let len = (0.25 * fs) as usize;
    db(single_bin(&y, 1000.0, fs, y.len() - len, len).norm())
}

/// Reference-ratio gain trajectory in dB, indexed by *input* time (latency removed).
fn ratio_trajectory(
    make: Make<'_>,
    settings: Settings,
    x: &[f32],
    gate_amp: f64,
) -> Vec<(usize, f64)> {
    let y = run(make, settings, x);
    let mut off = settings;
    off.agc.enabled = false;
    let y_ref = run(&|s| Pipeline::new(s).unwrap(), off, x);
    let lat = Pipeline::new(settings).unwrap().latency_samples() as usize;
    gain_trajectory_exact(&y_ref, &y, gate_amp)
        .into_iter()
        .filter(|&(n, _)| n >= lat)
        .map(|(n, g)| (n - lat, g))
        .collect()
}

/// Points of `traj` with index in `from..to`, re-indexed from `from`.
fn window(traj: &[(usize, f64)], from: usize, to: usize) -> Vec<(usize, f64)> {
    traj.iter()
        .filter(|&&(n, _)| n >= from && n < to)
        .map(|&(n, g)| (n - from, g))
        .collect()
}

fn mean(points: &[(usize, f64)]) -> f64 {
    points.iter().map(|p| p.1).sum::<f64>() / points.len() as f64
}

/// Samples from the start of `traj` to just after its last point outside the settling band
/// (2/27 of the excursion, spec 002 Overview).
fn settle(traj: &[(usize, f64)], final_db: f64, excursion_db: f64) -> usize {
    let band = excursion_db.abs() * 2.0 / 27.0;
    traj.iter()
        .rev()
        .find(|(_, g)| (g - final_db).abs() > band)
        .map_or(0, |&(n, _)| n + 1)
}

/// Step levels for timing (spec 002 FR-012): −40 ↔ −10 dBFS when both are ≥ 3 dB inside the
/// regulated range; otherwise centred, 6 dB narrower than the range, at most 30 dB. `None`
/// when the range is narrower than 12 dB.
pub fn step_levels(agc: &AgcSettings) -> Option<(f64, f64)> {
    let (lo, hi) = regulated_range(agc);
    if lo + 3.0 <= -40.0 && -10.0 <= hi - 3.0 {
        return Some((-40.0, -10.0));
    }
    let width = hi - lo;
    if width < 12.0 {
        return None;
    }
    let step = (width - 6.0).min(30.0);
    let centre = (lo + hi) / 2.0;
    Some((centre - step / 2.0, centre + step / 2.0))
}

/// Measured timing of one step run.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    /// Attack time in seconds.
    pub attack_s: f64,
    /// Release time in seconds.
    pub release_s: f64,
    /// Fraction of the release excursion reached at 25 % of the release time.
    pub midpoint: f64,
    /// Steady gain at the high level, dB.
    pub high_gain_db: f64,
}

/// Runs the timing step and measures it. `exact` uses y/x directly (valid only at 8 kHz,
/// where the rate conversion is the identity); otherwise the reference ratio.
pub fn measure_timing(make: Make<'_>, rate: u32, agc: &AgcSettings, exact: bool) -> Option<Timing> {
    let (low, high) = step_levels(agc)?;
    let fs = f64::from(rate);
    let attack = f64::from(agc.attack_ms) / 1000.0;
    let release = f64::from(agc.release_ms) / 1000.0;
    // Low hold lets the gain settle from maximum; the final low hold is 3 × release + 0.5 s
    // (spec minimum 1.5 ×), so the "final" value is truly settled (engineering target, R-11).
    let d = [
        (20.0 * attack + 0.1).max(0.5),
        (20.0 * attack).max(0.2),
        3.0 * release + 0.5,
    ];
    let x = stimulus::step(1000.0, &[low, high, low], &d, fs);
    let settings = agc_only(rate, agc);
    // Gate each segment at 0.5 × its own amplitude (engineering target, R-15). When the gain
    // changes within the rate converter's kernel span (fast attacks), output ÷ reference has a
    // small absolute error that becomes a ±2–4 dB spike near the reference's zero crossings.
    // Keeping only |reference| ≥ 0.5·A removes them and still leaves a sample at least every
    // 1/6 cycle (0.17 ms at 1 kHz), finer than FR-006's attack/4 resolution (T035).
    let trajectory = |level: f64| {
        // `gain_trajectory_exact` keeps |reference| ≥ 0.1 × its `amp` argument.
        let gate = 5.0 * stimulus::amplitude(level);
        if exact {
            assert_eq!(rate, 8000, "exact y/x is valid only at the 8 kHz host rate");
            gain_trajectory_exact(&x, &run(make, settings, &x), gate)
        } else {
            ratio_trajectory(make, settings, &x, gate)
        }
    };
    let (traj, traj_high) = (trajectory(low), trajectory(high));
    let s = |secs: f64| (secs * fs) as usize;
    let (s1, s2, end) = (s(d[0]), s(d[0] + d[1]), x.len());
    let margin = s(0.01).max(2);

    let before = mean(&window(&traj, s1 - s(0.04) - margin, s1 - margin));
    let high_gain = mean(&window(&traj_high, s2 - s(0.04) - margin, s2 - margin));
    let up = window(&traj_high, s1, s2 - margin);
    let attack_s = settle(&up, high_gain, before - high_gain) as f64 / fs;

    let tail = s(0.1);
    let fin = mean(&window(&traj, end - tail - s(0.05), end - s(0.05)));
    let down = window(&traj, s2, end - s(0.05));
    let release_n = settle(&down, fin, fin - high_gain);
    let at = down
        .iter()
        .find(|&&(n, _)| n >= release_n / 4)
        .map_or(fin, |p| p.1);
    Some(Timing {
        attack_s,
        release_s: release_n as f64 / fs,
        midpoint: (at - high_gain) / (fin - high_gain),
        high_gain_db: high_gain,
    })
}

/// FR-006 / FR-012 tolerance on a time: defaults use the spec's absolute tolerance; other
/// settings ±20 % or ±1 ms, whichever is larger (engineering targets).
fn time_tolerance(target_s: f64, default_tol_s: f64, is_default: bool) -> Tolerance {
    let tol = if is_default {
        default_tol_s
    } else {
        (0.2 * target_s).max(0.001)
    };
    Tolerance::Range(target_s - tol, target_s + tol)
}

/// 002 FR-004: on the regulation line ± 1 dB, at points ≥ 3 dB inside the regulated range.
pub fn check_fr004_regulation(rate: u32, case: &Case, make: Make<'_>) -> Vec<MeasurementResult> {
    let (lo, hi) = regulated_range(&case.agc);
    let t = f64::from(case.agc.target_dbfs);
    let mut points = Vec::new();
    let mut l = lo + 3.0;
    while l <= hi - 3.0 {
        points.push(l);
        l += 5.0;
    }
    if hi - 3.0 > lo + 3.0 {
        points.push(hi - 3.0);
    }
    points
        .into_iter()
        .map(|input| {
            let want = t + (input - t) / 10.0;
            let got = steady_level(make, rate, &case.agc, input);
            MeasurementResult::new(
                "002/FR-004",
                "output level on the regulation line",
                T_A017,
                rate,
                CONFIG,
                format!("{}: 1 kHz {input:.1} dBFS", case.label),
                got - want,
                "dB",
                Tolerance::Range(-1.0, 1.0), // engineering target (002 FR-015); A-017
            )
        })
        .collect()
}

/// 002 FR-005: gain at the limits (± 1 dB) at points ≥ 3 dB outside the regulated range, on
/// a sweep from 10 dB below the range to 10 dB above it in 5 dB steps.
pub fn check_fr005_limits(rate: u32, case: &Case, make: Make<'_>) -> Vec<MeasurementResult> {
    let (lo, hi) = regulated_range(&case.agc);
    let (g_max, a_max) = (
        f64::from(case.agc.max_gain_db),
        f64::from(case.agc.max_attenuation_db),
    );
    let mut out = Vec::new();
    let mut l = lo - 10.0;
    while l <= hi + 10.0 + 1e-9 {
        let (property, want) = if l <= lo - 3.0 {
            ("gain at maximum gain", g_max)
        } else if l >= hi + 3.0 {
            ("gain at maximum attenuation", -a_max)
        } else {
            l += 5.0;
            continue;
        };
        let gain = steady_level(make, rate, &case.agc, l) - l;
        out.push(MeasurementResult::new(
            "002/FR-005",
            property,
            T_A017,
            rate,
            CONFIG,
            format!("{}: 1 kHz {l:.1} dBFS", case.label),
            gain - want,
            "dB",
            Tolerance::Range(-1.0, 1.0), // engineering target (002 FR-015); A-017
        ));
        l += 5.0;
    }
    out
}

/// 002 FR-006 / FR-012: attack and release times and the release shape. Empty when the
/// regulated range is narrower than 12 dB (FR-012).
pub fn check_fr006_timing(rate: u32, case: &Case, make: Make<'_>) -> Vec<MeasurementResult> {
    let Some(t) = measure_timing(make, rate, &case.agc, false) else {
        return Vec::new();
    };
    let attack = f64::from(case.agc.attack_ms) / 1000.0;
    let release = f64::from(case.agc.release_ms) / 1000.0;
    let stim = || format!("{}: step", case.label);
    vec![
        MeasurementResult::new(
            "002/FR-006",
            "attack time",
            T_A018,
            rate,
            CONFIG,
            stim(),
            t.attack_s * 1000.0,
            "ms",
            scale(time_tolerance(attack, 0.002, case.is_default()), 1000.0),
        ),
        MeasurementResult::new(
            "002/FR-006",
            "release time",
            T_A018,
            rate,
            CONFIG,
            stim(),
            t.release_s,
            "s",
            time_tolerance(release, 0.2, case.is_default()),
        ),
        MeasurementResult::new(
            "002/FR-006",
            "release recovered at 25 % of release time",
            T_A018,
            rate,
            CONFIG,
            stim(),
            t.midpoint,
            "fraction",
            Tolerance::Range(0.35, 0.65), // engineering target (002 FR-006 shape check)
        ),
    ]
}

fn scale(t: Tolerance, k: f64) -> Tolerance {
    match t {
        Tolerance::Range(a, b) => Tolerance::Range(a * k, b * k),
        other => other,
    }
}

/// 002 R-11 self-check: the reference-ratio method at `rate` agrees with the exact y/x at
/// 8 kHz: attack within 0.25 ms, release within 1 %, steady gain within 0.1 dB (engineering
/// targets; release uses a relative tolerance because the gain approaches its final value
/// too slowly for a 0.25 ms bound to be measurable, T034).
pub fn check_ratio_self_test(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let agc = AgcSettings::DEVICE;
    let exact = measure_timing(make, 8000, &agc, true).expect("default step");
    let ratio = measure_timing(make, rate, &agc, false).expect("default step");
    let r = |property, measured, unit, tol| {
        MeasurementResult::new(
            "002/R-11",
            property,
            T_ENG,
            rate,
            CONFIG,
            "defaults: step, reference ratio vs exact 8 kHz",
            measured,
            unit,
            tol,
        )
    };
    vec![
        r(
            "attack: ratio − exact",
            (ratio.attack_s - exact.attack_s) * 1000.0,
            "ms",
            Tolerance::Range(-0.25, 0.25), // engineering target (002 R-15)
        ),
        r(
            "release: ratio / exact − 1",
            ratio.release_s / exact.release_s - 1.0,
            "fraction",
            Tolerance::Range(-0.01, 0.01), // engineering target (002 R-15)
        ),
        r(
            "steady gain: ratio − exact",
            ratio.high_gain_db - exact.high_gain_db,
            "dB",
            Tolerance::Range(-0.1, 0.1), // engineering target (002 R-15)
        ),
    ]
}

/// 002 FR-007: digital silence gives exact zeros, and after a loud tone the gain returns to
/// maximum during silence (no hold, no gate; A-019). Probed with a tone 10 dB below the
/// regulated range, where the target gain is the maximum, so the probe doesn't move it.
pub fn check_fr007_silence(rate: u32, case: &Case, make: Make<'_>) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let settings = agc_only(rate, &case.agc);
    let silence = stimulus::silence((0.5 * fs) as usize);
    let y = run(make, settings, &silence);
    let nonzero = y.iter().filter(|v| v.to_bits() != 0).count();

    let (lo, _) = regulated_range(&case.agc);
    let probe = lo - 10.0;
    let release = f64::from(case.agc.release_ms) / 1000.0;
    let quiet = 4.0 * release + 0.5;
    let mut x = stimulus::step(1000.0, &[0.0], &[0.5], fs);
    x.extend(stimulus::silence((quiet * fs) as usize));
    let probe_start = x.len();
    x.extend(stimulus::step(1000.0, &[probe], &[0.05], fs));
    let traj = ratio_trajectory(make, settings, &x, stimulus::amplitude(probe));
    let probe_gain = mean(&window(
        &traj,
        probe_start,
        probe_start + (0.01 * fs) as usize,
    ));
    vec![
        MeasurementResult::new(
            "002/FR-007",
            "non-zero output samples for digital silence",
            T_A019,
            rate,
            CONFIG,
            format!("{}: 0.5 s silence", case.label),
            nonzero as f64,
            "samples",
            Tolerance::Exact(0.0), // 002 FR-007: exact digital silence
        ),
        MeasurementResult::new(
            "002/FR-007",
            "gain after silence − maximum gain",
            &["A-019", "engineering target"],
            rate,
            CONFIG,
            format!(
                "{}: 0 dBFS, {quiet:.1} s silence, probe {probe:.1} dBFS",
                case.label
            ),
            probe_gain - f64::from(case.agc.max_gain_db),
            "dB",
            Tolerance::Range(-0.1, 0.1), // engineering target (002 R-15)
        ),
    ]
}

/// A tone with a starting phase, bit-reproducible (detmath).
pub fn tone_with_phase(freq_hz: f64, level_dbfs: f64, phase: f64, fs: f64, len: usize) -> Vec<f32> {
    let amp = stimulus::amplitude(level_dbfs);
    (0..len)
        .map(|n| {
            let cycles = freq_hz * n as f64 / fs;
            (amp * sin(TAU * (cycles - (cycles as u64) as f64) + phase)) as f32
        })
        .collect()
}

/// 002 FR-008: steady level within ±0.5 dB of the 1 kHz value and THD+N ≤ 1 % (−40 dB), at
/// 300, 500, 1000, 2000, 8000/3 and 3400 Hz and −30, −10, 0 dBFS, with 4 starting phases at
/// 2000 and 8000/3 Hz. Defaults only.
pub fn check_fr008_frequency_thdn(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let agc = AgcSettings::DEVICE;
    let settings = agc_only(rate, &agc);
    let len = ((settle_s(&agc) + 0.25) * fs) as usize;
    let seg = (0.25 * fs) as usize;
    let measure = |f: f64, level: f64, phase: f64| {
        let y = run(make, settings, &tone_with_phase(f, level, phase, fs, len));
        let lvl = db(single_bin(&y, f, fs, y.len() - seg, seg).norm());
        (lvl, thd_n_db(&y, f, fs, y.len() - seg, seg))
    };
    let mut out = Vec::new();
    for level in [-30.0, -10.0, 0.0] {
        let (ref_level, _) = measure(1000.0, level, 0.0);
        for f in [300.0, 500.0, 1000.0, 2000.0, 8000.0 / 3.0, 3400.0] {
            let phases: &[f64] = if f == 2000.0 || f == 8000.0 / 3.0 {
                &[0.0, TAU / 8.0, TAU / 4.0, 3.0 * TAU / 8.0]
            } else {
                &[0.0]
            };
            for &phase in phases {
                let (lvl, thdn) = measure(f, level, phase);
                let stim = format!("{f:.1} Hz {level} dBFS phase {phase:.3}");
                out.push(MeasurementResult::new(
                    "002/FR-008",
                    "level relative to 1 kHz",
                    T_ENG,
                    rate,
                    CONFIG,
                    stim.clone(),
                    lvl - ref_level,
                    "dB",
                    Tolerance::Range(-0.5, 0.5), // engineering target (002 FR-015)
                ));
                out.push(MeasurementResult::new(
                    "002/FR-008",
                    "THD+N",
                    T_ENG,
                    rate,
                    CONFIG,
                    stim,
                    thdn,
                    "dB",
                    Tolerance::AtMost(-40.0), // engineering target (002 FR-015): 1 %
                ));
            }
        }
    }
    out
}

/// The FR-009 stimulus at `fs`: band-limited noise at −70 dBFS (AES17) plus 1 kHz bursts at
/// −10 dBFS, 1 s on / 4 s off, 2 cycles (10 s). Bit-reproducible.
pub fn noise_burst_stimulus(fs: f64) -> Vec<f32> {
    let bursts = stimulus::tone_bursts(1000.0, -10.0, 1.0, 4.0, 2, fs);
    let noise = stimulus::bandlimited_noise(0x0D60, bursts.len(), fs, -70.0);
    bursts.iter().zip(&noise).map(|(b, n)| b + n).collect()
}

/// 002 FR-009: in the last 1 s of each pause the output noise is −30 dBFS ± 2 dB (input noise
/// plus maximum gain); in the first 50 ms after each burst ends (after the boundary latency)
/// it is at least 20 dB lower than that. Defaults only. Levels are AES17.
pub fn check_fr009_noise_rise(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let settings = agc_only(rate, &AgcSettings::DEVICE);
    let x = noise_burst_stimulus(fs);
    let y = run(make, settings, &x);
    let lat = Pipeline::new(settings).unwrap().latency_samples() as usize;
    let at = |secs: f64| (secs * fs) as usize + lat;
    let mut out = Vec::new();
    for cycle in 0..2 {
        let t0 = cycle as f64 * 5.0;
        // The last 1 s of the pause, ending 10 ms early: the rate conversion is linear-phase,
        // so the next burst's onset (overshooting by up to the maximum gain) appears a few ms
        // before its nominal latency (engineering target, R-15).
        // The second pause ends with the stimulus, so its window ends there instead.
        let end = at(t0 + 4.99).min(y.len());
        let pause_end = level_dbfs_aes17(&y[end - fs as usize..end]);
        let after_burst = level_dbfs_aes17(&y[at(t0 + 1.0)..at(t0 + 1.05)]);
        out.push(MeasurementResult::new(
            "002/FR-009",
            "noise level, last 1 s of pause",
            &["A-017", "A-019", "engineering target"],
            rate,
            CONFIG,
            format!("noise -70 dBFS + bursts, pause {}", cycle + 1),
            pause_end,
            "dBFS",
            Tolerance::Range(-32.0, -28.0), // engineering target (002 FR-015); A-017
        ));
        out.push(MeasurementResult::new(
            "002/FR-009",
            "pause-end level − level in first 50 ms after burst",
            &["A-018", "engineering target"],
            rate,
            CONFIG,
            format!("noise -70 dBFS + bursts, pause {}", cycle + 1),
            pause_end - after_burst,
            "dB",
            Tolerance::AtLeast(20.0), // engineering target (002 FR-015)
        ));
    }
    out
}

/// 002 FR-010: the AGC adds no latency, for every tap.
pub fn check_fr010_latency(rate: u32) -> Vec<MeasurementResult> {
    [Tap::AfterAgc, Tap::AfterRecord, Tap::AfterPlayback]
        .into_iter()
        .map(|tap| {
            let mut on = Settings::new(rate);
            on.vas.enabled = false; // the spec 002 default (spec 003 FR-020)
            on.tap = tap;
            let mut off = on;
            off.agc.enabled = false;
            let diff = f64::from(Pipeline::new(on).unwrap().latency_samples())
                - f64::from(Pipeline::new(off).unwrap().latency_samples());
            MeasurementResult::new(
                "002/FR-010",
                "latency with AGC − latency without",
                T_ENG,
                rate,
                "default_agc",
                format!("{tap:?}"),
                diff,
                "samples",
                Tolerance::Exact(0.0), // 002 FR-010: no added latency
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regulated_range_and_step_levels() {
        let (lo, hi) = regulated_range(&AgcSettings::DEVICE);
        assert!((lo + 54.444).abs() < 0.01 && (hi - 12.222).abs() < 0.01);
        assert_eq!(step_levels(&AgcSettings::DEVICE), Some((-40.0, -10.0)));
        let mut zero = AgcSettings::DEVICE;
        zero.max_gain_db = 0.0;
        zero.max_attenuation_db = 0.0;
        assert_eq!(step_levels(&zero), None);
        let mut narrow = AgcSettings::DEVICE;
        narrow.max_gain_db = 0.0; // range −10 … +12.2: 22.2 dB wide
        let (a, b) = step_levels(&narrow).unwrap();
        assert!((b - a - 16.222).abs() < 0.01 && ((a + b) / 2.0 - 1.111).abs() < 0.01);
        assert_eq!(matrix().len(), 13);
        assert!(matrix()[0].is_default() && !matrix()[1].is_default());
    }
}
