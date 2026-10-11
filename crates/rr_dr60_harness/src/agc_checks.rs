//! Spec 005 limiter checks: one function per requirement (research R-08; tasks.md T021–T024,
//! T033).
//!
//! Every limiter check runs the AGC-isolated configuration (`agc_only`: the "after AGC" tap at
//! the host rate, everything else off) and measures the **exact per-sample gain** `y[n] / x[n]`
//! at every host rate: no reference run and no boundary latency (the spec 002 reference-ratio
//! method and its Hilbert detector are retired, 005 R-08). The limiter is a pure multiplier at
//! that tap, so the trace is exact to f32 rounding (about 5e-7 dB) wherever `|x[n]|` is above
//! the gate. Results report `005/FR-0xx` with their trace IDs; tolerances that are not device
//! properties are engineering targets (spec 005 FR-017, research R-13).
//!
//! Stimuli that feed the limiter are 1 kHz tones at the host-grid peak phase
//! ([`stimulus::peak_phase`], R-08); the checks at the default output (FR-007 bandwidth,
//! FR-009) use spec 004's device-grid [`stimulus::aligned_phase`].

use rr_dr60::{AgcSettings, Pipeline, Settings, Tap};

use crate::analysis::{db, single_bin, thd_n_db};
use crate::checks::{Make, MeasurementResult, Tolerance};
use crate::{configs, stimulus};

const CONFIG: &str = "agc_only";
/// Trace: the knee and the ceiling (A-028) with engineering-target tolerances.
const T_KNEE: &[&str] = &["A-028", "engineering target"];
/// Trace: the dead time (A-034) and the arming (A-028).
const T_ATTACK: &[&str] = &["A-034", "A-028", "engineering target"];
/// Trace: the release (A-035).
const T_RELEASE: &[&str] = &["A-035", "engineering target"];
/// Trace: the sensed polarity (A-036).
const T_POLARITY: &[&str] = &["A-036", "A-028", "engineering target"];
/// Trace: the codec's full scale (A-030).
const T_CLAMP: &[&str] = &["A-030", "engineering target"];
const T_ENG: &[&str] = &["engineering target"];

/// Static-curve tone length, s (FR-004: 5 s, the last second measured); 1 s under coverage
/// instrumentation (003 T050 pattern).
const STATIC_TONE_S: f64 = if cfg!(coverage) { 1.0 } else { 5.0 }; // engineering target (005 FR-004)

/// A named AGC setting for the matrix (spec 005 FR-011).
#[derive(Clone, Copy, Debug)]
pub struct Case {
    /// Label for reports, e.g. `"knee -10 dBFS"`.
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

/// The FR-011 settings matrix (005 research R-08): the defaults, then one setting moved at a
/// time with the others at default: knee −10 and +3 dBFS, attack 1, 10 and 50 ms, release 200,
/// 4000 and 10 000 ms.
pub fn matrix() -> Vec<Case> {
    let with = |label, f: fn(&mut AgcSettings)| {
        let mut agc = AgcSettings::DEVICE;
        f(&mut agc);
        Case { label, agc }
    };
    vec![
        with("defaults", |_| {}),
        with("knee -10 dBFS", |a| a.knee_dbfs = -10.0),
        with("knee +3 dBFS", |a| a.knee_dbfs = 3.0),
        with("attack 1 ms", |a| a.attack_ms = 1.0),
        with("attack 10 ms", |a| a.attack_ms = 10.0),
        with("attack 50 ms", |a| a.attack_ms = 50.0),
        with("release 200 ms", |a| a.release_ms = 200.0),
        with("release 4000 ms", |a| a.release_ms = 4000.0),
        with("release 10000 ms", |a| a.release_ms = 10_000.0),
    ]
}

// ---------------------------------------------------------------------------------------------
// Stimuli and measurement (T021; research R-08)
// ---------------------------------------------------------------------------------------------

fn run(make: Make<'_>, settings: Settings, x: &[f32]) -> Vec<f32> {
    let mut y = vec![0.0; x.len()];
    let produced = make(settings)
        .process(x, &mut y)
        .expect("equal lengths")
        .produced;
    y.truncate(produced);
    y
}

/// A 1 kHz tone at `level_dbfs` for `secs` at the host-grid peak phase (R-08).
fn tone(level_dbfs: f64, secs: f64, rate: u32) -> Vec<f32> {
    let fs = f64::from(rate);
    stimulus::tone_at_phase(
        1000.0,
        level_dbfs,
        stimulus::peak_phase(1000.0, rate),
        fs,
        (secs * fs) as usize,
    )
}

/// A 1 kHz level step at the host-grid peak phase (R-08).
fn step(levels_dbfs: &[f64], durations_s: &[f64], rate: u32) -> Vec<f32> {
    stimulus::step_at_phase(
        1000.0,
        levels_dbfs,
        durations_s,
        stimulus::peak_phase(1000.0, rate),
        f64::from(rate),
    )
}

/// The exact per-sample gain in dB of `x` through `settings`: `20·log10(y[n] / x[n])` where
/// `|x[n]| ≥ 0.1 · gate_amp` (`// 002 R-15 gate, kept by 005 R-08`), `NaN` elsewhere. Valid
/// only where the pipeline is a pure multiplier with no delay: the "after AGC" tap (the output
/// has the input's length and alignment). No reference run and no boundary (002 R-11 retired).
pub fn gain_trace_settings(
    make: Make<'_>,
    settings: Settings,
    x: &[f32],
    gate_amp: f64,
) -> Vec<f64> {
    assert_eq!(
        settings.tap,
        Tap::AfterAgc,
        "the exact gain is observable only at the tap"
    );
    let y = run(make, settings, x);
    assert_eq!(y.len(), x.len());
    let gate = 0.1 * gate_amp; // 002 R-15 gate, kept by 005 R-08 (engineering target)
    x.iter()
        .zip(&y)
        .map(|(&xi, &yi)| {
            if f64::from(xi).abs() >= gate {
                db(f64::from(yi) / f64::from(xi))
            } else {
                f64::NAN
            }
        })
        .collect()
}

/// [`gain_trace_settings`] for a named configuration (normally `agc_only`).
pub fn gain_trace(
    make: Make<'_>,
    rate: u32,
    settings_name: &str,
    x: &[f32],
    gate_amp: f64,
) -> Vec<f64> {
    gain_trace_settings(make, configs::settings(settings_name, rate), x, gate_amp)
}

fn nan_free(trace: &[f64]) -> impl Iterator<Item = f64> + '_ {
    trace.iter().copied().filter(|g| !g.is_nan())
}

/// Mean gain over `[from_s, to_s)`, ignoring gated samples.
pub fn steady_db(trace: &[f64], fs: f64, from_s: f64, to_s: f64) -> f64 {
    let seg = &trace[(from_s * fs) as usize..((to_s * fs) as usize).min(trace.len())];
    let (sum, n) = nan_free(seg).fold((0.0, 0usize), |(s, n), g| (s + g, n + 1));
    assert!(n > 0, "no gain samples in {from_s}..{to_s} s");
    sum / n as f64
}

/// Mean gain over a 2 ms window centred on `t_s` (the trace is gated near zero crossings).
fn gain_at(trace: &[f64], fs: f64, t_s: f64) -> f64 {
    steady_db(trace, fs, t_s - 0.001, t_s + 0.001)
}

/// Index of the first sample at or after `t0_s` whose attenuation exceeds half the steady
/// attenuation (`// 005 FR-005: half the steady attenuation`).
fn dead_index(trace: &[f64], fs: f64, t0_s: f64, steady_db: f64) -> Option<usize> {
    let half = 0.5 * steady_db;
    ((t0_s * fs) as usize..trace.len()).find(|&n| trace[n] < half)
}

/// FR-005 dead time in seconds: from `t0_s` to the first sample whose attenuation exceeds half
/// `steady_db` (the same relative rule for every step size, plan review P1). `None` if never.
pub fn dead_time_s(trace: &[f64], fs: f64, t0_s: f64, steady_db: f64) -> Option<f64> {
    dead_index(trace, fs, t0_s, steady_db).map(|n| n as f64 / fs - t0_s)
}

/// FR-005 ramp in seconds: from the dead-time sample to the first sample within 1 dB of
/// `steady_db`.
pub fn ramp_s(trace: &[f64], fs: f64, t0_s: f64, steady_db: f64) -> Option<f64> {
    let n_dead = dead_index(trace, fs, t0_s, steady_db)?;
    let n_done = (n_dead..trace.len()).find(|&n| trace[n] < steady_db + 1.0)?;
    Some((n_done - n_dead) as f64 / fs)
}

/// FR-005 over-attenuation guard: how far below `steady_db` the gain ever went in
/// `[from_s, to_s)`, as a positive dB figure (0 if it never did).
pub fn max_over_attenuation_db(
    trace: &[f64],
    fs: f64,
    from_s: f64,
    to_s: f64,
    steady_db: f64,
) -> f64 {
    let seg = &trace[(from_s * fs) as usize..((to_s * fs) as usize).min(trace.len())];
    let gmin = nan_free(seg).fold(f64::INFINITY, f64::min);
    (steady_db - gmin).max(0.0)
}

/// FR-006 release in seconds: from `t0_s` to the first sample whose gain exceeds `to_db`.
pub fn release_s(trace: &[f64], fs: f64, t0_s: f64, to_db: f64) -> Option<f64> {
    ((t0_s * fs) as usize..trace.len())
        .find(|&n| trace[n] > to_db)
        .map(|n| n as f64 / fs - t0_s)
}

fn peak(y: &[f32]) -> f64 {
    f64::from(y.iter().fold(0.0f32, |m, v| m.max(v.abs())))
}

/// Steady gain of a static tone through `settings`: the output peak over the last second of a
/// `STATIC_TONE_S` tone at `level_dbfs` (plus `dc_offset` on every sample), in dB relative to
/// the input amplitude.
fn static_gain_db(make: Make<'_>, settings: Settings, level_dbfs: f64, dc_offset: f64) -> f64 {
    let rate = settings.host_rate_hz;
    let mut x = tone(level_dbfs, STATIC_TONE_S, rate);
    if dc_offset != 0.0 {
        for v in &mut x {
            *v += dc_offset as f32;
        }
    }
    let y = run(make, settings, &x);
    let tail = &y[y.len() - rate as usize..];
    db(peak(tail) / stimulus::amplitude(level_dbfs))
}

/// FR-004: the knee, as the input level at which a steady 1 kHz tone's gain reaches −1 dB, by
/// bisection to 0.1 dB between −4 and +8 dBFS (`// engineering target (005 FR-004)`). With a
/// `dc_offset` added to the tone for the DC-offset edge case.
pub fn knee_bisection(rate: u32, make: Make<'_>, dc_offset: f64) -> f64 {
    knee_bisection_settings(configs::settings(CONFIG, rate), make, dc_offset)
}

/// [`knee_bisection`] for explicit settings (the FR-011 matrix): the search spans
/// knee − 5 … knee + 7 dBFS around the configured knee.
pub fn knee_bisection_settings(settings: Settings, make: Make<'_>, dc_offset: f64) -> f64 {
    let knee = f64::from(settings.agc.knee_dbfs);
    let (mut lo, mut hi) = (knee - 5.0, knee + 7.0); // engineering target (005 FR-004): −4 … +8 at the default
    while hi - lo > 0.1 {
        let mid = 0.5 * (lo + hi);
        if static_gain_db(make, settings, mid, dc_offset) < -1.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    0.5 * (lo + hi)
}

/// The static-curve set (data-model.md › Harness stimuli): −60 … +20 dBFS in 2 dB steps plus
/// 1 dB steps from −4 to +6 (engineering target, 005 FR-004).
pub fn static_curve_levels() -> Vec<f64> {
    let mut levels: Vec<f64> = (-30..=10).map(|k| f64::from(k) * 2.0).collect();
    levels.extend([-3.0, -1.0, 1.0, 3.0, 5.0]);
    levels.sort_by(f64::total_cmp);
    levels
}

/// One FR-004 row per level through `settings` (`static_curve_rows_settings`); see
/// [`static_curve_rows`].
pub fn static_curve_rows_settings(
    settings: Settings,
    config: &'static str,
    label: &str,
    make: Make<'_>,
    levels: &[f64],
    knee_dbfs: f64,
) -> Vec<MeasurementResult> {
    let rate = settings.host_rate_hz;
    // The ceiling is the output level at the measured knee: the input level whose gain is −1 dB,
    // minus that 1 dB (spec Terms: the ceiling equals the knee within the knee's width).
    let ceiling = knee_dbfs - 1.0;
    levels
        .iter()
        .map(|&level| {
            let gain = static_gain_db(make, settings, level, 0.0);
            let peak_dbfs = level + gain;
            let stim = format!("{label}1 kHz {level:+.0} dBFS, {STATIC_TONE_S} s");
            if level >= knee_dbfs + 1.5 {
                // 005 FR-004: ±0.5 dB of the ceiling above knee + 1.5 dB (engineering target).
                MeasurementResult::new(
                    "005/FR-004",
                    "output peak vs ceiling",
                    T_KNEE,
                    rate,
                    config,
                    stim,
                    peak_dbfs - ceiling,
                    "dB",
                    Tolerance::Range(-0.5, 0.5), // engineering target (005 FR-004); A-028
                )
            } else if level > knee_dbfs - 1.5 {
                MeasurementResult::new(
                    "005/FR-004",
                    "output peak vs ceiling (knee band, reported)",
                    T_KNEE,
                    rate,
                    config,
                    stim,
                    peak_dbfs - ceiling,
                    "dB",
                    Tolerance::Report,
                )
            } else {
                // Below the knee FR-003 applies: bit-identity, asserted exactly (coach review
                // after US1; a reported peak ratio asserted nothing).
                let x = tone(level, STATIC_TONE_S, rate);
                let y = run(make, settings, &x);
                let differing = x
                    .iter()
                    .zip(&y)
                    .filter(|(a, b)| a.to_bits() != b.to_bits())
                    .count() as f64;
                MeasurementResult::new(
                    "005/FR-003",
                    "differing samples below the knee (static-curve set)",
                    T_KNEE,
                    rate,
                    config,
                    format!("{stim}; gain {gain:+.3} dB"),
                    differing,
                    "samples",
                    Tolerance::Exact(0.0), // 005 FR-003: bit-identical below the knee
                )
            }
        })
        .collect()
}

/// One FR-004 row per level of the static-curve set through `agc_only`: above knee + 1.5 dB
/// the output peak within ±0.5 dB of the ceiling; within ±1.5 dB of the knee a report row;
/// below, the gain reported (FR-003 makes it 0).
pub fn static_curve_rows(
    rate: u32,
    make: Make<'_>,
    levels: &[f64],
    knee_dbfs: f64,
) -> Vec<MeasurementResult> {
    static_curve_rows_settings(
        configs::settings(CONFIG, rate),
        CONFIG,
        "",
        make,
        levels,
        knee_dbfs,
    )
}

// ---------------------------------------------------------------------------------------------
// Checks (T022 – T024)
// ---------------------------------------------------------------------------------------------

/// 005 FR-003 (A-028): input whose peaks of the sensed polarity stay 1.5 dB below the knee is
/// returned bit for bit: a −20 dBFS tone, speech-like bursts at −30 dBFS, white noise at ±0.5
/// and a 5 s tone at knee − 1.5 dB (−0.5 dBFS); and FR-002: a bypassed limiter passes a
/// +11 dBFS tone bit for bit.
pub fn check_fr003_passthrough(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let settings = configs::settings(CONFIG, rate);
    let differing = |settings: Settings, x: &[f32]| {
        let y = run(make, settings, x);
        assert_eq!(y.len(), x.len());
        x.iter()
            .zip(&y)
            .filter(|(a, b)| a.to_bits() != b.to_bits())
            .count() as f64
    };
    let stimuli: [(&str, Vec<f32>); 4] = [
        ("1 kHz -20 dBFS, 2 s", tone(-20.0, 2.0, rate)),
        (
            "speech-like bursts -30 dBFS (300 ms on / 150 ms off x 4)",
            stimulus::tone_bursts(1000.0, -30.0, 0.3, 0.15, 4, fs),
        ),
        (
            "white noise +-0.5 (seed 60), 2 s",
            stimulus::noise(60, (2.0 * fs) as usize),
        ),
        (
            "1 kHz -0.5 dBFS (knee - 1.5 dB), 5 s",
            tone(-0.5, 5.0, rate), // engineering target (005 FR-003): the 1.5 dB margin
        ),
    ];
    let mut out: Vec<MeasurementResult> = stimuli
        .iter()
        .map(|(label, x)| {
            MeasurementResult::new(
                "005/FR-003",
                "differing samples below the knee",
                T_KNEE,
                rate,
                CONFIG,
                *label,
                differing(settings, x),
                "samples",
                Tolerance::Exact(0.0), // 005 FR-003: bit-identical
            )
        })
        .collect();
    let mut bypassed = settings;
    bypassed.agc.enabled = false;
    out.push(MeasurementResult::new(
        "005/FR-002",
        "differing samples with the limiter bypassed",
        &["A-020", "engineering target"],
        rate,
        CONFIG,
        "1 kHz +11 dBFS, 1 s, agc.enabled = false",
        differing(bypassed, &tone(11.0, 1.0, rate)),
        "samples",
        Tolerance::Exact(0.0), // 005 FR-002: bypass is bit-exact
    ));
    out
}

/// 005 FR-004 (A-028): the knee by bisection at +2 ± 2 dBFS, the static-curve set against the
/// ceiling, and the 40 dB attenuation limit never exceeded.
pub fn check_fr004_static(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let knee = knee_bisection(rate, make, 0.0);
    let mut out = vec![MeasurementResult::new(
        "005/FR-004",
        "knee (-1 dB gain point) by bisection",
        T_KNEE,
        rate,
        CONFIG,
        format!("1 kHz {STATIC_TONE_S} s tones, bisection to 0.1 dB"),
        knee,
        "dBFS",
        Tolerance::Range(0.0, 4.0), // A-028: +2 ± 2 dBFS (engineering target, 005 FR-004)
    )];
    let rows = static_curve_rows(rate, make, &static_curve_levels(), knee);
    // The deepest attenuation in the set is at +20 dBFS: gain = (peak − ceiling) + ceiling − 20.
    let deepest = rows
        .iter()
        .find(|r| r.stimulus.contains("+20 dBFS"))
        .map_or(f64::NAN, |r| r.measured + (knee - 1.0) - 20.0);
    out.extend(rows);
    out.push(MeasurementResult::new(
        "005/FR-004",
        "gain at +20 dBFS (attenuation limit 40 dB)",
        T_ENG,
        rate,
        CONFIG,
        format!("1 kHz +20 dBFS, {STATIC_TONE_S} s"),
        deepest,
        "dB",
        Tolerance::AtLeast(-40.0), // engineering target (005 FR-004): the 40 dB limit
    ));
    out
}

/// 005 FR-005 (A-034, A-028): the dead time of a −20 → +11 dBFS step from the discharged state
/// (a fresh pipeline equals 20 s of sub-knee input, R-03), the ramp and the over-attenuation
/// guard, the 2 dB-over step's dead-time ratio, and the armed dead time.
pub fn check_fr005_attack(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    check_fr005_attack_settings(configs::settings(CONFIG, rate), CONFIG, "", make, 3.0)
}

/// [`check_fr005_attack`] for explicit settings: `expected_ms` is the configured dead time at
/// 10 dB over the configured knee; the default tolerance is ±1.5 ms (FR-005), the matrix's
/// ± max(50 %, 0.5 ms) (FR-011). With a non-default attack setting the ramp (≤ 1 ms) and the
/// armed dead time (≤ 0.5 ms) are **reported**, not toleranced: FR-011 restates the dead time
/// only, and both scale with the detector's charge rate (a 50 ms setting charges 17× slower
/// than the default, so its ramp is about 7 ms and its armed dead time about 8–9 ms; T033).
/// The over-attenuation guard and the 2 dB-over ratio are relative rules and stay pass/fail.
pub fn check_fr005_attack_settings(
    settings: Settings,
    config: &'static str,
    label: &str,
    make: Make<'_>,
    expected_ms: f64,
) -> Vec<MeasurementResult> {
    let rate = settings.host_rate_hz;
    let fs = f64::from(rate);
    let knee = f64::from(settings.agc.knee_dbfs);
    let (quiet, loud, near) = (knee - 21.0, knee + 10.0, knee + 2.0);
    let gate = stimulus::amplitude(quiet);
    let tol = if settings.agc == AgcSettings::DEVICE {
        1.5 // A-034: 3 ± 1.5 ms (engineering target, 005 FR-005)
    } else {
        (0.5 * expected_ms).max(0.5) // engineering target (005 FR-011): ± max(50 %, 0.5 ms)
    };
    // The ramp and armed figures are FR-005's default-attack figures (T033: reported for a
    // non-default attack setting, which scales both with the charge rate).
    let default_attack = settings.agc.attack_ms == AgcSettings::DEVICE.attack_ms;
    let ramp_tol = if default_attack {
        Tolerance::AtMost(1.0) // engineering target (005 FR-005): ≤ 1 ms
    } else {
        Tolerance::Report
    };
    let armed_tol = if default_attack {
        Tolerance::AtMost(0.5) // A-028; engineering target (005 FR-005): ≤ 0.5 ms
    } else {
        Tolerance::Report
    };

    // The 10 dB step: 0.1 s quiet, 0.4 s loud; steady = the last 50 ms.
    let x = step(&[quiet, loud], &[0.1, 0.4], rate);
    let g = gain_trace_settings(make, settings, &x, gate);
    let steady = steady_db(&g, fs, 0.45, 0.5);
    let dead_10 = dead_time_s(&g, fs, 0.1, steady);
    let ramp = ramp_s(&g, fs, 0.1, steady);
    let over = max_over_attenuation_db(&g, fs, 0.1, 0.5, steady);
    let stim = |what: &str| format!("{label}1 kHz {quiet:+.0} -> {loud:+.0} dBFS{what}");
    let mut out = vec![
        MeasurementResult::new(
            "005/FR-005",
            "steady attenuation, 10 dB over the knee (reported)",
            T_KNEE,
            rate,
            config,
            stim(""),
            steady,
            "dB",
            Tolerance::Report,
        ),
        MeasurementResult::new(
            "005/FR-005",
            "dead time, 10 dB over the knee",
            T_ATTACK,
            rate,
            config,
            stim(", fresh pipeline"),
            dead_10.map_or(f64::NAN, |s| s * 1e3),
            "ms",
            Tolerance::Range(expected_ms - tol, expected_ms + tol), // A-034; engineering target (005 FR-005)
        ),
        MeasurementResult::new(
            "005/FR-005",
            "ramp from the dead-time point to within 1 dB of steady",
            T_ATTACK,
            rate,
            config,
            stim(""),
            ramp.map_or(f64::NAN, |s| s * 1e3),
            "ms",
            ramp_tol,
        ),
        MeasurementResult::new(
            "005/FR-005",
            "over-attenuation vs the steady value",
            T_ATTACK,
            rate,
            config,
            stim(""),
            over,
            "dB",
            Tolerance::AtMost(1.0), // engineering target (005 FR-005): the guard
        ),
    ];

    // The 2 dB-over step: 0.1 s quiet, 1.5 s at knee + 2 dB; steady = the last 50 ms.
    let x = step(&[quiet, near], &[0.1, 1.5], rate);
    let g = gain_trace_settings(make, settings, &x, gate);
    let steady_near = steady_db(&g, fs, 1.55, 1.6);
    let dead_2 = dead_time_s(&g, fs, 0.1, steady_near);
    out.push(MeasurementResult::new(
        "005/FR-005",
        "dead-time ratio, 2 dB over / 10 dB over",
        T_ATTACK,
        rate,
        config,
        format!("{label}1 kHz {quiet:+.0} -> {near:+.0} dBFS vs -> {loud:+.0} dBFS"),
        match (dead_2, dead_10) {
            (Some(a), Some(b)) => a / b,
            _ => f64::NAN,
        },
        "ratio",
        Tolerance::AtLeast(3.0), // A-034; engineering target (005 FR-005): ≥ 3× (model 5×)
    ));

    // Armed: a 0.2 s burst, 2 s quiet, the step again; steady = the last 50 ms of the second burst.
    let x = step(&[loud, quiet, loud], &[0.2, 2.0, 0.2], rate);
    let g = gain_trace_settings(make, settings, &x, gate);
    let steady_armed = steady_db(&g, fs, 2.35, 2.4);
    out.push(MeasurementResult::new(
        "005/FR-005",
        "dead time, armed (2 s after a 0.2 s burst)",
        T_ATTACK,
        rate,
        config,
        format!("{label}1 kHz {loud:+.0} 0.2 s, {quiet:+.0} 2 s, {loud:+.0} dBFS"),
        dead_time_s(&g, fs, 2.2, steady_armed).map_or(f64::NAN, |s| s * 1e3),
        "ms",
        armed_tol,
    ));
    out
}

/// 005 FR-006 (A-035): after a 1 s burst 10 dB over the knee, the gain is back within 1 dB of
/// unity 1.37 s ± 40 % later, reading −6 ± 2 dB at 0.3 s and −3 ± 1.5 dB at 0.75 s; the
/// 17 dB case (+18 dBFS) is reported.
pub fn check_fr006_release(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    check_fr006_release_settings(configs::settings(CONFIG, rate), CONFIG, "", make, 1.37)
}

/// [`check_fr006_release`] for explicit settings: `expected_s` is the configured release; the
/// −6 / −3 dB points scale in time by `expected_s / 1.37` (FR-011). With a non-default release
/// the arming is checked too, as FR-011 asks ("the arming MUST scale with the release
/// setting"): the armed step's 2 s gap is scaled by the same factor and the armed dead time
/// must still be ≤ 0.5 ms (the discharge sets how long the control stays near its turn-on
/// point, R-02; T033).
pub fn check_fr006_release_settings(
    settings: Settings,
    config: &'static str,
    label: &str,
    make: Make<'_>,
    expected_s: f64,
) -> Vec<MeasurementResult> {
    let rate = settings.host_rate_hz;
    let fs = f64::from(rate);
    let knee = f64::from(settings.agc.knee_dbfs);
    let (quiet, loud) = (knee - 21.0, knee + 10.0);
    let gate = stimulus::amplitude(quiet);
    let tail_s = (3.0 * expected_s / 1.37).max(3.0);
    let x = step(&[loud, quiet], &[1.0, tail_s], rate);
    let g = gain_trace_settings(make, settings, &x, gate);
    let scale = expected_s / 1.37; // 005 FR-011: the shape scales with the release setting
    let stim = format!("{label}1 kHz {loud:+.0} dBFS 1 s, then {quiet:+.0} dBFS");
    let mut out = vec![
        MeasurementResult::new(
            "005/FR-006",
            "release time to -1 dB",
            T_RELEASE,
            rate,
            config,
            stim.clone(),
            release_s(&g, fs, 1.0, -1.0).unwrap_or(f64::NAN),
            "s",
            Tolerance::Range(0.6 * expected_s, 1.4 * expected_s), // A-035: ± 40 % (engineering target, 005 FR-006)
        ),
        MeasurementResult::new(
            "005/FR-006",
            "gain 0.3 s (scaled) after the burst",
            T_RELEASE,
            rate,
            config,
            stim.clone(),
            gain_at(&g, fs, 1.0 + 0.3 * scale),
            "dB",
            Tolerance::Range(-8.0, -4.0), // A-035: −6 ± 2 dB (engineering target, 005 FR-006)
        ),
        MeasurementResult::new(
            "005/FR-006",
            "gain 0.75 s (scaled) after the burst",
            T_RELEASE,
            rate,
            config,
            stim,
            gain_at(&g, fs, 1.0 + 0.75 * scale),
            "dB",
            Tolerance::Range(-4.5, -1.5), // A-035: −3 ± 1.5 dB (engineering target, 005 FR-006)
        ),
    ];
    if settings.agc.release_ms != AgcSettings::DEVICE.release_ms {
        // Arming scales with the release: the 2 s gap of the FR-005 armed step, scaled and
        // rounded to whole milliseconds (whole 1 kHz cycles), so the second burst still starts
        // on a positive peak (R-08) and the dead time is measured from the same phase.
        let gap = (2.0 * scale * 1000.0).round() / 1000.0; // engineering target (005 FR-011)
        let x = step(&[loud, quiet, loud], &[0.2, gap, 0.2], rate);
        let g = gain_trace_settings(make, settings, &x, gate);
        let t_on = 0.2 + gap;
        let steady_armed = steady_db(&g, fs, t_on + 0.15, t_on + 0.2);
        out.push(MeasurementResult::new(
            "005/FR-011",
            "dead time, armed (gap scaled with the release)",
            &["A-028", "A-035", "engineering target"],
            rate,
            config,
            format!("{label}1 kHz {loud:+.0} 0.2 s, {quiet:+.0} {gap:.2} s, {loud:+.0} dBFS"),
            dead_time_s(&g, fs, t_on, steady_armed).map_or(f64::NAN, |s| s * 1e3),
            "ms",
            Tolerance::AtMost(0.5), // A-028; engineering target (005 FR-005, FR-011): ≤ 0.5 ms
        ));
    }
    if settings.agc == AgcSettings::DEVICE {
        let x = step(&[knee + 17.0, quiet], &[1.0, 3.0], rate);
        let g = gain_trace_settings(make, settings, &x, gate);
        out.push(MeasurementResult::new(
            "005/FR-006",
            "release time to -1 dB from 17 dB (reported)",
            T_RELEASE,
            rate,
            config,
            format!("1 kHz {:+.0} dBFS 1 s, then {quiet:+.0} dBFS", knee + 17.0),
            release_s(&g, fs, 1.0, -1.0).unwrap_or(f64::NAN),
            "s",
            Tolerance::Report,
        ));
    }
    out
}

/// 005 FR-007 (A-036): the detector senses positive input samples. The asymmetric stimulus
/// (peaks +0.75 / −1.5, the larger at +4 dBFS) and its negation differ in steady gain by
/// 3 ± 0.5 dB, the attenuated one being the case whose larger peak is positive; through the
/// record path with the clamp (6 kHz, where the device-rate output is observable) the
/// un-sensed loud polarity reaches the clamp and the quiet polarity never does.
pub fn check_fr007_polarity(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let settings = configs::settings(CONFIG, rate);
    let len = (2.0 * fs) as usize;
    let gate = stimulus::amplitude(4.0) / 1.5 * 0.75; // the smaller peak
    let phase = stimulus::peak_phase(1000.0, rate); // R-08: a host sample on the 270° peak
    let steady = |sign: f64| {
        let x = stimulus::asymmetric(4.0, sign, fs, len, phase);
        let g = gain_trace_settings(make, settings, &x, gate);
        steady_db(&g, fs, 1.9, 2.0)
    };
    let (unsensed, sensed) = (steady(1.0), steady(-1.0));
    let mut out = vec![
        MeasurementResult::new(
            "005/FR-007",
            "steady gain, larger peak on the sensed (positive) side (reported)",
            T_POLARITY,
            rate,
            CONFIG,
            "asymmetric sin + 0.5 cos 2wt, larger peak +4 dBFS positive",
            sensed,
            "dB",
            Tolerance::Report,
        ),
        MeasurementResult::new(
            "005/FR-007",
            "polarity gain difference (un-sensed - sensed)",
            T_POLARITY,
            rate,
            CONFIG,
            "asymmetric stimulus vs its negation, larger peak +4 dBFS",
            unsensed - sensed,
            "dB",
            Tolerance::Range(2.5, 3.5), // A-036; engineering target (005 FR-007): 3 ± 0.5 dB
        ),
    ];
    // The record path with the clamp, at the device-rate tap.
    let path = configs::settings("record_path_agc", rate);
    let extremes = |sign: f64| {
        let y = run(make, path, &stimulus::asymmetric(4.0, sign, fs, len, phase));
        let tail = &y[y.len() - rate as usize..];
        let max = f64::from(tail.iter().fold(f32::MIN, |m, &v| m.max(v)));
        let min = f64::from(tail.iter().fold(f32::MAX, |m, &v| m.min(v)));
        let clipped = tail.iter().filter(|v| v.abs() == 1.0).count() as f64 / tail.len() as f64;
        (max, min, clipped)
    };
    let (max_u, min_u, _) = extremes(1.0);
    let (max_s, min_s, duty_s) = extremes(-1.0);
    if rate == rr_dr60::DEVICE_RATE_HZ {
        out.push(MeasurementResult::new(
            "005/FR-007",
            "un-sensed loud polarity reaches the clamp (min output)",
            T_POLARITY,
            rate,
            "record_path_agc",
            "larger peak +4 dBFS negative, last 1 s",
            min_u,
            "FS",
            Tolerance::Exact(-1.0), // 005 FR-007: the un-sensed polarity reaches the clamp
        ));
        out.push(MeasurementResult::new(
            "005/FR-007",
            "sensed loud polarity: the quiet side stays inside full scale (min output)",
            T_POLARITY,
            rate,
            "record_path_agc",
            "larger peak +4 dBFS positive, last 1 s",
            min_s,
            "FS",
            Tolerance::Range(-0.999, 0.0), // 005 FR-007 (engineering target): never −1.0
        ));
    }
    out.push(MeasurementResult::new(
        "005/FR-007",
        "un-sensed loud polarity: quiet side max output (reported)",
        T_POLARITY,
        rate,
        "record_path_agc",
        "larger peak +4 dBFS negative, last 1 s",
        max_u,
        "FS",
        Tolerance::Report,
    ));
    out.push(MeasurementResult::new(
        "005/FR-007",
        "sensed loud polarity: max output and clipped duty (reported)",
        T_POLARITY,
        rate,
        "record_path_agc",
        format!(
            "larger peak +4 dBFS positive, last 1 s; clipped duty {:.1} %",
            100.0 * duty_s
        ),
        max_s,
        "FS",
        Tolerance::Report,
    ));
    out
}

/// 005 FR-007 bandwidth (A-019 revised): at hosts of 16 kHz and above, a 6 kHz tone at +6 dBFS
/// added to a −20 dBFS 1 kHz probe pulls the probe's level at the default output down by
/// 5 ± 1 dB (the sum peak +6.41 dBFS against the +1.00 dBFS ceiling, 005 R-04: 5.41 dB), while
/// the default output's 6 kHz content is at least 60 dB down (spec 004 FR-003 restated).
pub fn check_fr007_bandwidth(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    if rate < 16_000 {
        return Vec::new();
    }
    let fs = f64::from(rate);
    let settings = configs::settings("default_agc", rate);
    let len = (3.0 * fs) as usize;
    let last = rate as usize;
    let level_1k = |x: &[f32]| {
        let y = run(make, settings, x);
        db(single_bin(&y, 1000.0, fs, y.len() - last, last).norm())
    };
    let probe = stimulus::tone(1000.0, stimulus::amplitude(-20.0), fs, len);
    let both = stimulus::two_tone(1000.0, -20.0, 6000.0, 6.0, fs, len);
    let drop = level_1k(&probe) - level_1k(&both);
    let y = run(make, settings, &both);
    let content_6k =
        db(single_bin(&y, 6000.0, fs, y.len() - last, last).norm() / stimulus::amplitude(6.0));
    vec![
        MeasurementResult::new(
            "005/FR-007",
            "probe level drop with a 6 kHz tone at +6 dBFS",
            &["A-019", "A-028", "engineering target"],
            rate,
            "default_agc",
            "1 kHz -20 dBFS + 6 kHz +6 dBFS, 3 s, last 1 s",
            drop,
            "dB",
            Tolerance::Range(4.0, 6.0), // engineering target (005 FR-007): 5 ± 1 dB; 005 R-04 5.41
        ),
        MeasurementResult::new(
            "005/FR-007",
            "6 kHz content at the default output",
            &["A-026", "A-027", "engineering target"],
            rate,
            "default_agc",
            "1 kHz -20 dBFS + 6 kHz +6 dBFS, 3 s, last 1 s",
            content_6k,
            "dB",
            Tolerance::AtMost(-60.0), // spec 004 FR-003 restated by 005 FR-007
        ),
    ]
}

/// Fraction of `y[from..to]` at exactly ±1.0, in percent.
fn clipped_duty_percent(y: &[f32], from: usize, to: usize) -> f64 {
    let seg = &y[from.min(y.len())..to.min(y.len())];
    if seg.is_empty() {
        return f64::NAN;
    }
    100.0 * seg.iter().filter(|v| v.abs() == 1.0).count() as f64 / seg.len() as f64
}

/// 005 FR-008 (A-030): at the 6 kHz identity rate a +11 dBFS tone through the record path peaks
/// at exactly 1.0 (and above 1.0 with the clamp off); the onset of the same tone on a fresh
/// pipeline (20 s of sub-knee input, R-03) has ≥ 60 % of its first 2 ms at ±1.0 and ≤ 40 % of
/// the 20–100 ms window with the limiter on, and ≥ 60 % in both with it off. At the other rates
/// the record-path peak is reported (the up-converter interpolates between the clamped samples).
pub fn check_fr008_clamp(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let x = tone(11.0, 1.0, rate);
    let record_only = run(make, configs::settings("record_only", rate), &x);
    let unclamped = run(make, configs::settings("record_only_clamp_off", rate), &x);
    if rate != rr_dr60::DEVICE_RATE_HZ {
        return vec![MeasurementResult::new(
            "005/FR-008",
            "record path peak, +11 dBFS (interpolated; reported)",
            T_CLAMP,
            rate,
            "record_only",
            format!("1 kHz +11 dBFS 1 s; clamp off peak {:.3}", peak(&unclamped)),
            peak(&record_only),
            "FS",
            Tolerance::Report,
        )];
    }
    let mut out = vec![
        MeasurementResult::new(
            "005/FR-008",
            "largest output magnitude, +11 dBFS through the record stage",
            T_CLAMP,
            rate,
            "record_only",
            "1 kHz +11 dBFS 1 s",
            peak(&record_only),
            "FS",
            Tolerance::Exact(1.0), // A-030: full scale
        ),
        MeasurementResult::new(
            "005/FR-008",
            "largest output magnitude with the clamp off",
            T_CLAMP,
            rate,
            "record_only_clamp_off",
            "1 kHz +11 dBFS 1 s",
            peak(&unclamped),
            "FS",
            Tolerance::AtLeast(1.5), // 005 FR-008 (engineering target): exceeds full scale
        ),
    ];
    // The clipped onset: −20 dBFS 0.1 s then +11 dBFS 0.1 s, at the device-rate tap after the
    // record stage and the clamp, with the limiter on and off.
    let onset = step(&[-20.0, 11.0], &[0.1, 0.1], rate);
    let n_on = (0.1 * fs) as usize;
    for (config, limiter_on) in [("record_path_agc", true), ("tap_after_record", false)] {
        let settings = configs::settings(config, rate);
        let lat = Pipeline::new(settings).unwrap().latency_samples() as usize;
        let y = run(make, settings, &onset);
        let at = |ms: f64| n_on + lat + (ms * fs / 1000.0) as usize;
        let early = clipped_duty_percent(&y, at(0.0), at(2.0));
        let late = clipped_duty_percent(&y, at(20.0), at(100.0));
        let label = if limiter_on {
            "limiter on"
        } else {
            "limiter off"
        };
        out.push(MeasurementResult::new(
            "005/FR-008",
            if limiter_on {
                "clipped-sample duty, first 2 ms of the onset, limiter on"
            } else {
                "clipped-sample duty, first 2 ms of the onset, limiter off"
            },
            T_CLAMP,
            rate,
            config,
            format!("1 kHz -20 -> +11 dBFS, fresh pipeline, {label}"),
            early,
            "%",
            Tolerance::AtLeast(60.0), // engineering target (005 FR-008); 005 R-04: 79 % before the filter
        ));
        out.push(MeasurementResult::new(
            "005/FR-008",
            if limiter_on {
                "clipped-sample duty, 20-100 ms after the onset, limiter on"
            } else {
                "clipped-sample duty, 20-100 ms after the onset, limiter off"
            },
            T_CLAMP,
            rate,
            config,
            format!("1 kHz -20 -> +11 dBFS, fresh pipeline, {label}"),
            late,
            "%",
            if limiter_on {
                Tolerance::AtMost(40.0) // engineering target (005 FR-008); 005 R-04: 29 % steady
            } else {
                Tolerance::AtLeast(60.0) // 005 FR-008: the clamp alone clips the whole tone
            },
        ));
    }
    out
}

/// 005 FR-009: THD+N of +6 and +12 dBFS tones at the default output, reported (dominated by
/// the clamp and the knee; below the knee FR-003 makes it zero).
pub fn check_fr009_thdn(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let settings = configs::settings("default_agc", rate);
    let bypass = Pipeline::new(configs::settings("bypass_all", rate))
        .expect("supported rate")
        .latency_samples();
    let phase = stimulus::aligned_phase(1000.0, rate, bypass);
    [6.0, 12.0]
        .into_iter()
        .map(|level| {
            let x = stimulus::tone_at_phase(1000.0, level, phase, fs, (3.0 * fs) as usize);
            let y = run(make, settings, &x);
            let last = rate as usize;
            MeasurementResult::new(
                "005/FR-009",
                "THD+N at the default output (reported)",
                &["A-030", "A-028"],
                rate,
                "default_agc",
                format!("1 kHz {level:+.0} dBFS, 3 s, last 1 s"),
                thd_n_db(&y, 1000.0, fs, y.len() - last, last),
                "dB",
                Tolerance::Report,
            )
        })
        .collect()
}

/// 005 FR-001 (SC-006): the "after AGC" tap has latency 0; every other tap's latency is the
/// same with the limiter on and off.
pub fn check_fr001_latency(rate: u32) -> Vec<MeasurementResult> {
    let mut out = vec![MeasurementResult::new(
        "005/FR-001",
        "latency at the after-AGC tap",
        T_ENG,
        rate,
        CONFIG,
        "agc_only",
        f64::from(
            Pipeline::new(configs::settings(CONFIG, rate))
                .unwrap()
                .latency_samples(),
        ),
        "samples",
        Tolerance::Exact(0.0), // 005 FR-001: host-rate output, latency 0
    )];
    for tap in [Tap::AfterRecord, Tap::AfterVas, Tap::AfterPlayback] {
        let mut on = configs::settings("default_agc", rate);
        on.tap = tap;
        let mut off = on;
        off.agc.enabled = false;
        let diff = f64::from(Pipeline::new(on).unwrap().latency_samples())
            - f64::from(Pipeline::new(off).unwrap().latency_samples());
        out.push(MeasurementResult::new(
            "005/FR-001",
            "latency with the limiter - latency without",
            T_ENG,
            rate,
            "default_agc",
            format!("{tap:?}"),
            diff,
            "samples",
            Tolerance::Exact(0.0), // 005 FR-001, SC-006: no added latency
        ));
    }
    out
}

/// 005 FR-011 (T033): the settings matrix, one non-default setting per case with the others at
/// the defaults, through `agc_only`. For a knee case: the FR-004 ceiling-and-knee check with
/// the measured knee expected at the configured knee + 1 ± 2 dB and the static-curve set
/// shifted with the knee. For an attack case: the FR-005 dead-time check with the expected
/// dead time at the configured value ± max(50 %, 0.5 ms), the step relative to the configured
/// knee. For a release case: the FR-006 release check at the configured value ± 40 %, with the
/// −6 ± 2 dB and −3 ± 1.5 dB points scaled in time by `release / 1370`. Every row reports
/// `005/FR-011` (the tolerances are engineering targets, 005 FR-011). The defaults case returns
/// nothing: the default checks cover it.
pub fn check_fr011_settings(rate: u32, case: &Case, make: Make<'_>) -> Vec<MeasurementResult> {
    let d = AgcSettings::DEVICE;
    let mut settings = configs::settings(CONFIG, rate);
    settings.agc = case.agc;
    let label = format!("{}: ", case.label);
    let mut out = Vec::new();
    if case.agc.knee_dbfs != d.knee_dbfs {
        let configured = f64::from(case.agc.knee_dbfs);
        let knee = knee_bisection_settings(settings, make, 0.0);
        out.push(MeasurementResult::new(
            "005/FR-011",
            "knee (-1 dB gain point) by bisection",
            T_KNEE,
            rate,
            CONFIG,
            format!("{label}1 kHz {STATIC_TONE_S} s tones, bisection to 0.1 dB"),
            knee - configured,
            "dB re configured",
            Tolerance::Range(-1.0, 3.0), // engineering target (005 FR-011): configured + 1 ± 2 dB
        ));
        // The static-curve set shifted with the knee (the default set is relative to +1 dBFS).
        let shift = configured - f64::from(d.knee_dbfs);
        let levels: Vec<f64> = static_curve_levels().iter().map(|l| l + shift).collect();
        out.extend(static_curve_rows_settings(
            settings, CONFIG, &label, make, &levels, knee,
        ));
    }
    if case.agc.attack_ms != d.attack_ms {
        out.extend(check_fr005_attack_settings(
            settings,
            CONFIG,
            &label,
            make,
            f64::from(case.agc.attack_ms),
        ));
    }
    if case.agc.release_ms != d.release_ms {
        out.extend(check_fr006_release_settings(
            settings,
            CONFIG,
            &label,
            make,
            f64::from(case.agc.release_ms) / 1000.0,
        ));
    }
    for r in &mut out {
        r.requirement = "005/FR-011";
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_has_the_fr011_cases() {
        let m = matrix();
        assert_eq!(m.len(), 9);
        assert!(m[0].is_default() && m[1..].iter().all(|c| !c.is_default()));
        for c in &m {
            let mut s = rr_dr60::Settings::new(48_000);
            s.agc = c.agc;
            assert_eq!(s.validate(), Ok(()), "{}", c.label);
        }
    }

    #[test]
    fn static_curve_levels_are_the_fr004_set() {
        let l = static_curve_levels();
        assert_eq!(l.len(), 46);
        assert_eq!((l[0], l[l.len() - 1]), (-60.0, 20.0));
        for want in [-4.0, -3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0] {
            assert!(l.contains(&want), "{want}");
        }
        assert!(!l.contains(&7.0));
    }

    /// The trace helpers on a synthetic gain curve.
    #[test]
    fn trace_helpers() {
        let fs = 1000.0;
        // 100 ms at 0 dB, then −10 dB from t = 0.1 s, gated every 10th sample.
        let g: Vec<f64> = (0..300)
            .map(|n| {
                if n % 10 == 0 {
                    f64::NAN
                } else if n < 100 {
                    0.0
                } else {
                    -10.0
                }
            })
            .collect();
        assert_eq!(steady_db(&g, fs, 0.2, 0.3), -10.0);
        let near = |a: Option<f64>, b: f64| (a.unwrap() - b).abs() < 1e-12;
        assert!(near(dead_time_s(&g, fs, 0.1, -10.0), 0.001)); // sample 100 is gated
        assert!(near(ramp_s(&g, fs, 0.1, -10.0), 0.0));
        assert_eq!(max_over_attenuation_db(&g, fs, 0.1, 0.3, -10.0), 0.0);
        let r: Vec<f64> = (0..300).map(|n| -10.0 + 0.05 * n as f64).collect();
        assert!(near(release_s(&r, fs, 0.0, -1.0), 0.181));
    }
}
