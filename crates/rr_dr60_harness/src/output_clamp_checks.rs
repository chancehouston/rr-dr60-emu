//! Spec 010 output-clamp checks: one function per measurable property (research R-06; tasks.md
//! T007, T008, T013).
//!
//! The only oracle is the **identity** of spec 010 FR-005: for every stimulus, configuration,
//! route, position and host rate, the route output with the option on equals, sample for sample
//! and bit for bit, the route output with the option off with each sample above +1.0 replaced by
//! +1.0 and each below −1.0 by −1.0 (`f32::clamp`). Research R-01 proves the identity holds
//! exactly on the narrowed `f32` output, so the harness computes the expected output from the
//! option-off twin ([`configs::output_clamp_twin`]). The option-off peaks and the counts of
//! samples beyond the bound are **reported**, never toleranced (R-04), except where the spec
//! says a stimulus must exceed the bound (the four overshooting cases) or must not (the
//! reference-position configuration), and the speaker route's peaks must never exceed the
//! speaker template's impulse-response L1 norm (the provable bound, FR-011).

use rr_dr60::{OutputSettings, Route, Settings, Tap};

use crate::analysis::{self, OutputTemplate};
use crate::checks::{Make, MeasurementResult, Tolerance};
use crate::{configs, output_checks, stimulus};

/// Trace: the option (A-042's full scale; an engineering convention).
const T_CLAMP: &[&str] = &["A-042", "engineering target"];
/// Trace: the speaker template's bound (A-010's template, A-031's clipper).
const T_BOUND: &[&str] = &["A-010", "A-031", "A-042", "engineering target"];

/// The spec's stimuli by name (data-model.md › Stimuli), `len_s` seconds long at `rate`.
pub fn stimulus_by_name(name: &str, rate: u32, len_s: f64) -> Vec<f32> {
    let fs = f64::from(rate);
    let n = (fs * len_s) as usize;
    match name {
        "sine_1k_1p5" => stimulus::tone(1000.0, 1.5, fs, n),
        "square_1k_fs" => stimulus::square(1000.0, 1.0, fs, n),
        "tone_1k_0" => stimulus::tone(1000.0, 1.0, fs, n),
        "voiced_m1" => stimulus::voiced(fs, len_s, -1.0, 0x0D60),
        "voiced_limited" => stimulus::voiced(fs, len_s, 10.0, 0x0D60),
        "noise_m3" => stimulus::bandlimited_noise(0x0D60, n, fs, -3.0),
        other => panic!("output_clamp_checks::stimulus_by_name: unknown stimulus {other:?}"),
    }
}

/// What the option-off run is expected to do at the bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expect {
    /// At least one sample beyond ±1.0 (the overshooting stimuli, FR-011).
    Exceeds,
    /// No sample beyond ±1.0 (the reference-position configuration; US1 AS4).
    Within,
    /// Reported only.
    Report,
}

/// The identity of FR-005 on `config` (its route and position from
/// [`configs::output_clamp_route_volume`]) with the named stimulus of `len_s` seconds: the
/// option-on output must equal the option-off twin's output bounded sample by sample. Returns
/// rows with the mismatch count (must be 0), the option-off peak, the count of option-off
/// samples beyond ±1.0 (against `expect`), the share of samples bounded, and whether the
/// option-on run holds a sample exactly at the bound wherever the count is above 0.
pub fn identity(
    make: Make<'_>,
    rate: u32,
    config: &'static str,
    stimulus: &'static str,
    len_s: f64,
    expect: Expect,
) -> Vec<MeasurementResult> {
    let (route, volume) = configs::output_clamp_route_volume(config);
    identity_at(make, rate, config, route, volume, stimulus, len_s, expect)
}

/// [`identity`] at an explicit route and position.
#[allow(clippy::too_many_arguments)]
pub fn identity_at(
    make: Make<'_>,
    rate: u32,
    config: &'static str,
    route: Route,
    volume: f32,
    stimulus: &'static str,
    len_s: f64,
    expect: Expect,
) -> Vec<MeasurementResult> {
    let x = stimulus_by_name(stimulus, rate, len_s);
    let on = output_checks::run(make, configs::settings(config, rate), route, volume, &x);
    let twin = configs::settings(configs::output_clamp_twin(config), rate);
    let off = output_checks::run(make, twin, route, volume, &x);
    rows_for(rate, config, route, volume, stimulus, &on, &off, expect)
}

/// The identity rows for an option-on output and its option-off twin.
#[allow(clippy::too_many_arguments)]
fn rows_for(
    rate: u32,
    config: &'static str,
    route: Route,
    volume: f32,
    stimulus: &'static str,
    on: &[f32],
    off: &[f32],
    expect: Expect,
) -> Vec<MeasurementResult> {
    let label = format!("{stimulus}, {route:?}, position {volume}");
    let length_ok = on.len() == off.len();
    let mismatches = if length_ok {
        on.iter()
            .zip(off)
            .filter(|(a, b)| a.to_bits() != b.clamp(-1.0, 1.0).to_bits())
            .count()
    } else {
        usize::MAX
    };
    let peak = off.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let beyond = off.iter().filter(|v| v.abs() > 1.0).count();
    let at_bound = on.iter().filter(|v| v.abs() == 1.0).count();
    let share = if off.is_empty() {
        0.0
    } else {
        100.0 * beyond as f64 / off.len() as f64
    };
    let expect_tol = match expect {
        Expect::Exceeds => Tolerance::AtLeast(1.0),
        Expect::Within => Tolerance::Exact(0.0),
        Expect::Report => Tolerance::Report,
    };
    vec![
        MeasurementResult::new(
            "FR-005",
            "identity: samples where on != clamp(off), or usize::MAX on a length mismatch",
            T_CLAMP,
            rate,
            config,
            label.clone(),
            mismatches as f64,
            "samples",
            Tolerance::Exact(0.0),
        ),
        MeasurementResult::new(
            "FR-011",
            "option-off peak at the route output (reported)",
            T_CLAMP,
            rate,
            config,
            label.clone(),
            f64::from(peak),
            "full scale",
            Tolerance::Report,
        ),
        MeasurementResult::new(
            "FR-011",
            "option-off samples beyond the bound",
            T_CLAMP,
            rate,
            config,
            label.clone(),
            beyond as f64,
            "samples",
            expect_tol,
        ),
        MeasurementResult::new(
            "FR-011",
            "share of samples bounded (reported)",
            T_CLAMP,
            rate,
            config,
            label.clone(),
            share,
            "%",
            Tolerance::Report,
        ),
        MeasurementResult::new(
            "FR-011",
            "option-on samples exactly at the bound (at least 1 wherever the off run exceeds)",
            T_CLAMP,
            rate,
            config,
            label,
            at_bound as f64,
            "samples",
            if beyond > 0 {
                Tolerance::AtLeast(1.0)
            } else {
                Tolerance::Report
            },
        ),
    ]
}

/// FR-003 (US1 AS5, SC-002): the per-stage taps are bit-identical with the option on and off
/// on all three golden stimuli at position 1 (`AfterVolumeAmp` and `AfterSpeaker` on the
/// speaker route, `AfterVolumeAmp` and `AfterEarphone` on the earphone route); `AfterPlayback`
/// with the chain bypassed likewise; and with every stage bypassed plus the option on, every
/// earlier tap on `sine_1k_1p5` is bit-identical to the option off with a peak above 1.0.
pub fn per_stage_taps_unclamped(make: Make<'_>, rate: u32, route: Route) -> Vec<MeasurementResult> {
    let mut rows = Vec::new();
    let (config, taps): (&'static str, &[Tap]) = match route {
        Route::Speaker => (
            "output_only_speaker_clamp",
            &[Tap::AfterVolumeAmp, Tap::AfterSpeaker],
        ),
        Route::Earphone => (
            "output_only_earphone_clamp",
            &[Tap::AfterVolumeAmp, Tap::AfterEarphone],
        ),
        _ => unreachable!("the two routes of spec 006"),
    };
    for stim in ["sine_1k_1p5", "square_1k_fs", "tone_1k_0"] {
        let x = stimulus_by_name(stim, rate, 1.0);
        for &tap in taps {
            let mut on = configs::settings(config, rate);
            on.tap = tap;
            let mut off = on;
            off.output.clamp_enabled = false;
            let y_on = output_checks::run(make, on, route, 1.0, &x);
            let y_off = output_checks::run(make, off, route, 1.0, &x);
            rows.push(tap_row(
                rate,
                config,
                format!("{stim}, {tap:?}"),
                &y_on,
                &y_off,
            ));
        }
    }
    // The chain bypassed: `AfterPlayback` is never bounded.
    let x = stimulus_by_name("sine_1k_1p5", rate, 1.0);
    let mut on = configs::settings("output_clamp_only", rate);
    on.tap = Tap::AfterPlayback;
    let mut off = on;
    off.output.clamp_enabled = false;
    let y_on = output_checks::run(make, on, route, 1.0, &x);
    let y_off = output_checks::run(make, off, route, 1.0, &x);
    rows.push(tap_row(
        rate,
        "output_clamp_only",
        "sine_1k_1p5, AfterPlayback".to_string(),
        &y_on,
        &y_off,
    ));
    rows.push(MeasurementResult::new(
        "FR-003",
        "AfterPlayback peak with the option on (never bounded)",
        T_CLAMP,
        rate,
        "output_clamp_only",
        "sine_1k_1p5",
        f64::from(y_on.iter().fold(0.0f32, |m, v| m.max(v.abs()))),
        "full scale",
        Tolerance::AtLeast(1.0 + 1e-6),
    ));
    // Every earlier tap, with every stage bypassed plus the option on.
    for tap in [
        Tap::AfterRecord,
        Tap::AfterAgc,
        Tap::AfterVas,
        Tap::AfterDecoder,
    ] {
        let mut on = configs::settings("bypass_all", rate);
        on.output.clamp_enabled = true;
        on.tap = tap;
        let mut off = on;
        off.output.clamp_enabled = false;
        let y_on = output_checks::run(make, on, route, 1.0, &x);
        let y_off = output_checks::run(make, off, route, 1.0, &x);
        rows.push(tap_row(
            rate,
            "bypass_all + clamp",
            format!("sine_1k_1p5, {tap:?}"),
            &y_on,
            &y_off,
        ));
        rows.push(MeasurementResult::new(
            "FR-003",
            "earlier tap's peak with the option on (never bounded)",
            T_CLAMP,
            rate,
            "bypass_all + clamp",
            format!("sine_1k_1p5, {tap:?}"),
            f64::from(y_on.iter().fold(0.0f32, |m, v| m.max(v.abs()))),
            "full scale",
            Tolerance::AtLeast(1.0 + 1e-6),
        ));
    }
    rows
}

fn tap_row(
    rate: u32,
    config: &'static str,
    label: String,
    on: &[f32],
    off: &[f32],
) -> MeasurementResult {
    let same = on.len() == off.len() && on.iter().zip(off).all(|(a, b)| a.to_bits() == b.to_bits());
    MeasurementResult::new(
        "FR-003",
        "tap bit-identical with the option on and off (1 = yes)",
        T_CLAMP,
        rate,
        config,
        label,
        f64::from(u8::from(same)),
        "",
        Tolerance::Exact(1.0),
    )
}

/// Spec Edge Cases "Output chain bypassed" (US1 AS5): `output_clamp_only` (every stage
/// bypassed, the option on) bounds the route output by the identity, while `AfterPlayback`
/// reads the unbounded signal.
pub fn bypassed_chain_bounded(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let mut rows = identity(
        make,
        rate,
        "output_clamp_only",
        "sine_1k_1p5",
        1.0,
        Expect::Exceeds,
    );
    let x = stimulus_by_name("sine_1k_1p5", rate, 1.0);
    let mut s = configs::settings("output_clamp_only", rate);
    s.tap = Tap::AfterPlayback;
    let y = output_checks::run(make, s, Route::Speaker, 1.0, &x);
    rows.push(MeasurementResult::new(
        "FR-003",
        "bypassed chain: AfterPlayback peak with the option on (unbounded)",
        T_CLAMP,
        rate,
        "output_clamp_only",
        "sine_1k_1p5",
        f64::from(y.iter().fold(0.0f32, |m, v| m.max(v.abs()))),
        "full scale",
        Tolerance::AtLeast(1.0 + 1e-6),
    ));
    rows
}

/// The hardware reviewer's L1 figures (research R-04), by `configs::all_rates()` index.
const L1_TABLE: [f64; 7] = [1.75, 1.88, 2.09, 2.24, 2.25, 2.28, 2.29];

/// FR-011 (research R-04): the speaker template's impulse-response L1 norm at `rate`,
/// recomputed from the design's discrete realization and pinned within ±0.02 of the reviewed
/// table; every option-off speaker-route peak in `peaks` must be at or below it.
pub fn l1_bound(rate: u32, peaks: &[(String, f32)]) -> (f64, Vec<MeasurementResult>) {
    let l1 = analysis::impulse_l1(OutputTemplate::Speaker, f64::from(rate));
    let want = configs::all_rates()
        .iter()
        .position(|&r| r == rate)
        .map_or(f64::NAN, |i| L1_TABLE[i]);
    let mut rows = vec![MeasurementResult::new(
        "FR-011",
        "speaker template impulse-response L1 norm (the speaker route's bound)",
        T_BOUND,
        rate,
        "design realization",
        "impulse, 2 s",
        l1,
        "full scale",
        Tolerance::Range(want - 0.02, want + 0.02),
    )];
    for (label, peak) in peaks {
        rows.push(MeasurementResult::new(
            "FR-011",
            "speaker-route option-off peak at or below the L1 bound",
            T_BOUND,
            rate,
            "speaker route",
            label.clone(),
            f64::from(*peak),
            "full scale",
            Tolerance::AtMost(l1),
        ));
    }
    (l1, rows)
}

/// The option-off peak of `stimulus` on the twin of `config` (a helper for the L1 rows).
pub fn off_peak(make: Make<'_>, rate: u32, config: &str, stimulus: &str, len_s: f64) -> f32 {
    let (route, volume) = configs::output_clamp_route_volume(config);
    let x = stimulus_by_name(stimulus, rate, len_s);
    let twin = configs::settings(configs::output_clamp_twin(config), rate);
    output_checks::run(make, twin, route, volume, &x)
        .iter()
        .fold(0.0f32, |m, v| m.max(v.abs()))
}

/// The settings of `config` with the option forced to `on` (a helper for the edge cases).
pub fn with_clamp(config: &str, rate: u32, on: bool) -> Settings {
    let mut s = configs::settings(config, rate);
    s.output.clamp_enabled = on;
    s
}

/// Every output stage bypassed with the option `on`, at the route-output tap.
pub fn bypassed_with_clamp(rate: u32, on: bool) -> Settings {
    let mut s = configs::settings("bypass_route_output", rate);
    s.output = OutputSettings::BYPASSED;
    s.output.clamp_enabled = on;
    s.tap = Tap::RouteOutput;
    s
}
