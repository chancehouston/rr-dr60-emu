//! Spec 009 output-chain checks: one function per measurable property (research R-10; tasks.md
//! T010, T016).
//!
//! Every chain figure is measured on the **output-chain-only configurations** (every earlier
//! stage bypassed, the route output tap): `output_only_earphone` (stages 11 and 13),
//! `output_only_speaker` (11 and 12) and `output_only_amp` (stage 11 alone, the speaker
//! template off). The chain runs at the host rate after the resampler, so a response is the
//! DTFT of the chain-on output **divided by the same path with the chain bypassed** (the
//! resampler pair cancels), evaluated at exact frequencies (spec 004's lesson) and compared
//! with the design's discrete realization computed in [`analysis`] from the registered
//! figures (R-10). Tolerances are engineering targets of spec 009 FR-003 – FR-009 and bound
//! the model, not the device (FR-018).
//!
//! Levels: tones that feed the knee start on the host grid's peak phase
//! ([`stimulus::peak_phase`], 005 R-03); distortion is measured at 44.1 kHz and above only
//! (the clipper's harmonics alias within the host band at the three low rates, D8).

use rr_dr60::output::{VOLUME_6DB_OVER, VOLUME_REFERENCE, earphone_gain_db, speaker_gain_db};
use rr_dr60::{OutputSettings, Route, Settings, Tap};

use crate::analysis::{self, OutputTemplate, db, dtft, group_delay_samples, thd_n_db};
use crate::checks::{Make, MeasurementResult, Tolerance};
use crate::{configs, stimulus};

/// Trace: stage 11 (A-031 with the loaded law A-044 and the scale A-042).
const T_AMP: &[&str] = &["A-031", "A-044", "engineering target"];
/// Trace: stage 12 (A-010, the template).
const T_SPK: &[&str] = &["A-010", "engineering target"];
/// Trace: stage 13 (A-032).
const T_EAR: &[&str] = &["A-032", "engineering target"];
/// Trace: the output scale and the default position (A-042, A-043).
const T_SCALE: &[&str] = &["A-042", "A-043", "A-044", "engineering target"];
const T_ENG: &[&str] = &["engineering target"];

/// Impulse-response length: 1.37 s at 48 kHz, far past the 32 Hz corner's 58 ms settle.
const IR_LEN: usize = 65_536;
/// The response grid: 40 log-spaced points from 20 Hz to 2.5 kHz (FR-005 – FR-007).
const GRID: usize = 40;

/// Runs `x` through `settings` at `route` and `volume` in one block; the produced output.
pub fn run(make: Make<'_>, settings: Settings, route: Route, volume: f32, x: &[f32]) -> Vec<f32> {
    let mut p = make(settings);
    p.set_route(route);
    p.set_volume(volume).expect("a valid position");
    let mut y = vec![0.0; x.len()];
    let n = p.process(x, &mut y).expect("equal lengths").produced;
    y.truncate(n);
    y
}

/// The same configuration with the chain bypassed at 0.8's tap: the response reference.
fn bypassed(mut settings: Settings) -> Settings {
    settings.output = OutputSettings::BYPASSED;
    settings.tap = Tap::AfterPlayback;
    settings
}

fn impulse(rate: u32) -> Vec<f32> {
    let mut x = stimulus::impulse(IR_LEN.min(rate as usize * 2));
    x[0] = 0.01; // −40 dBFS: below the knee at every position ≤ 1 (R-10)
    x
}

/// A 1 kHz tone at `level_dbfs` for `secs` at the host-grid peak phase.
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

fn grid() -> Vec<f64> {
    (0..GRID)
        .map(|i| 20.0 * analysis::from_db(db(2500.0 / 20.0) * i as f64 / (GRID - 1) as f64))
        .collect()
}

/// The chain's complex response at `f`: the chain-on impulse response over the chain-off one.
fn chain_response(y: &[f32], y_ref: &[f32], f: f64, fs: f64) -> analysis::Complex64 {
    dtft(y, f, fs) / dtft(y_ref, f, fs)
}

/// The loaded volume law in dB relative to position 1 on `route` (A-044).
fn law_db(route: Route, p: f32) -> f64 {
    if route == Route::Speaker {
        f64::from(speaker_gain_db(p) - speaker_gain_db(1.0))
    } else {
        f64::from(earphone_gain_db(p) - earphone_gain_db(1.0))
    }
}

/// The level in dB **re the clip ceiling** at which a full-scale 1 kHz tone reaches the knee at
/// position `p` (A-042, A-043): the loaded law relative to the reference position, less the
/// amplifier path's two corners' residual at 1 kHz (−0.11 dB), so the axis is the knee's own
/// level and the rows match the design script's figures (coach review T015, N4). 0 at the
/// reference position would be the law alone; the knee sees −0.11 dB there.
fn over_ceiling_db(p: f32, fs: f64) -> f64 {
    f64::from(speaker_gain_db(p) - speaker_gain_db(VOLUME_REFERENCE))
        + analysis::discrete_template_response(OutputTemplate::Amplifier, 1000.0, fs)
}

/// The position at which full-scale material sits `over_db` re the ceiling (bisection on the
/// loaded law; the "6 dB over" position equals `VOLUME_6DB_OVER`).
pub fn position_over_ceiling(over_db: f64, fs: f64) -> f32 {
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if over_ceiling_db(mid, fs) < over_db {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

fn peak_dbfs(y: &[f32]) -> f64 {
    db(y.iter().fold(0.0f64, |m, &v| m.max(f64::from(v).abs())))
}

fn peak(y: &[f32]) -> f64 {
    y.iter().fold(0.0f64, |m, &v| m.max(f64::from(v).abs()))
}

/// THD+N in percent of a 1 kHz tone, over whole cycles after `settle_s`.
fn thd_percent(y: &[f32], rate: u32, settle_s: f64) -> f64 {
    let fs = f64::from(rate);
    let start = (settle_s * fs) as usize;
    let cycles = ((y.len() - start) as f64 / fs * 1000.0).floor() as usize - 1;
    let len = (cycles as f64 * fs / 1000.0).round() as usize;
    100.0 * analysis::from_db(thd_n_db(y, 1000.0, fs, start, len))
}

/// Response rows of `template` on `route` through `config` at `volume`: the composite against
/// the design's discrete realization at every grid point within `band`, the maximum deviation
/// as one row, plus the flatness and peak rows the caller asks for.
#[allow(clippy::too_many_arguments)]
fn response_rows(
    rate: u32,
    make: Make<'_>,
    config: &'static str,
    route: Route,
    volume: f32,
    template: OutputTemplate,
    band: (f64, f64),
    requirement: &'static str,
    trace: &'static [&'static str],
) -> (Vec<MeasurementResult>, Vec<f32>, Vec<f32>) {
    let fs = f64::from(rate);
    let settings = configs::settings(config, rate);
    let x = impulse(rate);
    let y = run(make, settings, route, volume, &x);
    let y_ref = run(make, bypassed(settings), route, volume, &x);
    let mut worst = 0.0f64;
    for f in grid().into_iter().filter(|f| *f >= band.0 && *f <= band.1) {
        let got = db(chain_response(&y, &y_ref, f, fs).norm());
        let want = analysis::discrete_template_response(template, f, fs);
        worst = worst.max((got - want).abs());
    }
    // An identity check: the Rust realizes the design's own coefficients, so the residual is
    // numerical (about 1e-4 dB) and the bound is 0.01 dB, fifty times tighter than the spec's
    // ±0.5 dB, which bounds the template against its registered figures and is checked by the
    // design script (hardware review T043, B2: a wrong-rate table would pass ±0.5 dB).
    let rows = vec![MeasurementResult::new(
        requirement,
        "composite response vs the design's discrete realization, max |dev| (identity)",
        trace,
        rate,
        config,
        format!(
            "impulse -40 dBFS, {:.0}-{:.0} Hz, position {volume}",
            band.0, band.1
        ),
        worst,
        "dB",
        Tolerance::AtMost(0.01),
    )];
    (rows, y, y_ref)
}

fn flatness_db(y: &[f32], y_ref: &[f32], fs: f64, lo: f64, hi: f64) -> f64 {
    let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut f = lo;
    while f <= hi {
        let g = db(chain_response(y, y_ref, f, fs).norm());
        min = min.min(g);
        max = max.max(g);
        f += 10.0;
    }
    max - min
}

// ---------------------------------------------------------------------------------------------
// FR-007 / SC-001: the earphone route
// ---------------------------------------------------------------------------------------------

/// FR-007 (SC-001): the earphone composite (the 32 Hz corner and the follower's two) against the
/// design within ±0.5 dB from 20 Hz to 2.5 kHz; the passband above 1 kHz flat within 0.4 dB;
/// the 1 kHz level at position 1 within −0.5 … 0 dB.
pub fn earphone_response(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let (mut rows, y, y_ref) = response_rows(
        rate,
        make,
        "output_only_earphone",
        Route::Earphone,
        1.0,
        OutputTemplate::Earphone,
        (20.0, 2500.0),
        "FR-007",
        T_EAR,
    );
    rows.push(MeasurementResult::new(
        "FR-007",
        "earphone passband flatness 1-2.5 kHz",
        T_EAR,
        rate,
        "output_only_earphone",
        "impulse -40 dBFS",
        flatness_db(&y, &y_ref, fs, 1000.0, 2500.0),
        "dB",
        Tolerance::AtMost(0.4), // amended from 0.3 at implementation (the three-corner composite is 0.33)
    ));
    rows.push(MeasurementResult::new(
        "FR-007",
        "earphone level at 1 kHz, position 1",
        T_EAR,
        rate,
        "output_only_earphone",
        "impulse -40 dBFS",
        db(chain_response(&y, &y_ref, 1000.0, fs).norm()),
        "dB",
        Tolerance::Range(-0.5, 0.0),
    ));
    rows
}

// ---------------------------------------------------------------------------------------------
// FR-006 / SC-002: the amplifier path
// ---------------------------------------------------------------------------------------------

/// FR-006 (SC-002): the amplifier path's composite (32 and 159 Hz) against the design within
/// ±0.5 dB from 20 Hz to 2.5 kHz; flat within 0.2 dB from 800 Hz to 2.5 kHz. At the reference
/// position (unity before the knee).
pub fn amplifier_response(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let (mut rows, y, y_ref) = response_rows(
        rate,
        make,
        "output_only_amp",
        Route::Speaker,
        VOLUME_REFERENCE,
        OutputTemplate::Amplifier,
        (20.0, 2500.0),
        "FR-006",
        T_AMP,
    );
    rows.push(MeasurementResult::new(
        "FR-006",
        "amplifier passband flatness 800 Hz-2.5 kHz",
        T_AMP,
        rate,
        "output_only_amp",
        "impulse -40 dBFS",
        flatness_db(&y, &y_ref, fs, 800.0, 2500.0),
        "dB",
        Tolerance::AtMost(0.2),
    ));
    rows
}

/// FR-006 (SC-002): the stage-11 gain at 1 kHz at position 1, **observed in the route's scale**
/// (21.5 ± 0.5 dB: the loaded gain over the ceiling), with the derived gain in volts (22.7 dB)
/// reported beside it (coach Q4). A −40 dBFS tone keeps the output under the knee.
pub fn amplifier_gain(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let settings = configs::settings("output_only_amp", rate);
    let x = tone(-40.0, 1.0, rate);
    let y = run(make, settings, Route::Speaker, 1.0, &x);
    let y_ref = run(make, bypassed(settings), Route::Speaker, 1.0, &x);
    let settle = (0.2 * fs) as usize;
    let (g, _) = analysis::gain_db_and_phase(&y_ref, &y, 1000.0, fs, settle);
    vec![
        MeasurementResult::new(
            "FR-006",
            "stage 11 gain at 1 kHz, position 1, observed in the route's scale",
            T_AMP,
            rate,
            "output_only_amp",
            "tone 1 kHz -40 dBFS",
            g,
            "dB",
            Tolerance::Range(21.0, 22.0),
        ),
        MeasurementResult::new(
            "FR-006",
            "amplifier gain in volts, 2*68/10 (derived, A-031)",
            T_AMP,
            rate,
            "output_only_amp",
            "the loaded law at position 1",
            f64::from(speaker_gain_db(1.0)),
            "dB",
            Tolerance::Range(22.17, 23.17),
        ),
    ]
}

// ---------------------------------------------------------------------------------------------
// FR-005 / SC-003: the speaker template
// ---------------------------------------------------------------------------------------------

/// FR-005 (SC-003): the speaker route's composite (stage 11 at the reference position plus the
/// template) against the design within ±0.5 dB from 100 Hz to 2.5 kHz; the template's gain
/// never above 1.0 (the chain's response over 20 Hz – 2.5 kHz at most 0 dB within 1e-5); the
/// 1 kHz reading reported; bypassing the speaker stage leaves the amplifier path (its own
/// check, [`amplifier_response`]).
pub fn speaker_response(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let (mut rows, y, y_ref) = response_rows(
        rate,
        make,
        "output_only_speaker",
        Route::Speaker,
        VOLUME_REFERENCE,
        OutputTemplate::Speaker,
        (100.0, 2500.0),
        "FR-005",
        T_SPK,
    );
    let mut peak = f64::NEG_INFINITY;
    let mut f = 20.0;
    while f <= 2500.0 {
        peak = peak.max(db(chain_response(&y, &y_ref, f, fs).norm()));
        f += 1.0;
    }
    rows.push(MeasurementResult::new(
        "FR-005",
        "speaker route peak gain 20 Hz-2.5 kHz (never above 1.0)",
        T_SPK,
        rate,
        "output_only_speaker",
        "impulse -40 dBFS",
        peak,
        "dB",
        Tolerance::AtMost(1e-5),
    ));
    rows.push(MeasurementResult::new(
        "FR-005",
        "speaker template reading at 1 kHz re its peak",
        T_SPK,
        rate,
        "output_only_speaker",
        "impulse -40 dBFS",
        db(chain_response(&y, &y_ref, 1000.0, fs).norm())
            - analysis::discrete_template_response(OutputTemplate::Amplifier, 1000.0, fs),
        "dB",
        Tolerance::Range(-1.0, 0.0),
    ));
    rows
}

// ---------------------------------------------------------------------------------------------
// FR-008: group delay
// ---------------------------------------------------------------------------------------------

/// FR-008 (SC-007): the chain's group delay at 1 kHz on `route`, from the chain's own response
/// (the resampler pair cancels), at most 0.5 ms.
pub fn group_delay_1k(rate: u32, route: Route, make: Make<'_>) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let speaker = route == Route::Speaker;
    let config = if speaker {
        "output_only_speaker"
    } else {
        "output_only_earphone"
    };
    let volume = if speaker { VOLUME_REFERENCE } else { 1.0 };
    let settings = configs::settings(config, rate);
    let x = impulse(rate);
    let y = run(make, settings, route, volume, &x);
    let y_ref = run(make, bypassed(settings), route, volume, &x);
    let gd = group_delay_samples(|f| chain_response(&y, &y_ref, f, fs), 1000.0, fs) / fs * 1e3;
    vec![MeasurementResult::new(
        "FR-008",
        "output chain group delay at 1 kHz",
        if route == Route::Speaker {
            T_SPK
        } else {
            T_EAR
        },
        rate,
        config,
        "impulse -40 dBFS",
        gd,
        "ms",
        Tolerance::AtMost(0.5),
    )]
}

// ---------------------------------------------------------------------------------------------
// FR-003, FR-004 / SC-004: the volume law and the output scale
// ---------------------------------------------------------------------------------------------

/// FR-003 (SC-004): the loaded law within 0.1 dB at positions 1, 0.5, 0.25, the reference,
/// 0.1 and 0.01 on `route` (the level at each position relative to the level at position 1,
/// so the corners' residual cancels; a −40 dBFS tone stays under the knee on the speaker
/// route).
pub fn volume_law(rate: u32, route: Route, make: Make<'_>) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let config = if route == Route::Speaker {
        "output_only_amp"
    } else {
        "output_only_earphone"
    };
    let settings = configs::settings(config, rate);
    let x = tone(-40.0, 1.0, rate);
    let settle = (0.2 * fs) as usize;
    let at = |p: f32| {
        let y = run(make, settings, route, p, &x);
        analysis::gain_db_and_phase(&x, &y, 1000.0, fs, settle).0
    };
    let ref_db = at(1.0);
    [1.0, 0.5, 0.25, VOLUME_REFERENCE, 0.1, 0.01]
        .into_iter()
        .map(|p| {
            MeasurementResult::new(
                "FR-003",
                "loaded volume law: level re position 1 minus the law",
                T_AMP,
                rate,
                config,
                format!("tone 1 kHz -40 dBFS, position {p}"),
                (at(p) - ref_db) - law_db(route, p),
                "dB",
                Tolerance::Range(-0.1, 0.1),
            )
        })
        .collect()
}

/// FR-009 (SC-004): at position 0, every sample at the after-stage-11 tap is exactly 0.0 and
/// the route output decays below −120 dBFS within 100 ms on both routes (the time to cross
/// reported), both counted from **transition sample N** since spec 011 (FR-008; the position
/// moves over N samples first; tasks.md T019).
pub fn volume_zero(rate: u32, route: Route, make: Make<'_>) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let config = if route == Route::Speaker {
        "output_only_speaker"
    } else {
        "output_only_earphone"
    };
    let x = tone(-20.0, 0.5, rate);
    let after = tone(-20.0, 0.3, rate);
    let run_then_zero = |settings: Settings| {
        let mut p = make(settings);
        p.set_route(route);
        p.set_volume(1.0).unwrap();
        let mut y = vec![0.0f32; x.len()];
        let _ = p.process(&x, &mut y).unwrap();
        p.set_volume(0.0).unwrap();
        let mut z = vec![0.0f32; after.len()];
        let n = p.process(&after, &mut z).unwrap().produced;
        z.truncate(n);
        z
    };
    // Spec 011 FR-008: the transition's last sample, N − 1 samples into the second call.
    let n = rr_dr60::output::volume_transition_samples(rate).expect("a supported rate") as usize;
    let mut rows = Vec::new();
    let z = run_then_zero(configs::settings(config, rate));
    let last_loud = z
        .iter()
        .rposition(|v| f64::from(v.abs()) >= 1e-6)
        .map_or(0, |i| (i + 1).saturating_sub(n - 1));
    rows.push(MeasurementResult::new(
        "FR-009",
        "route output below -120 dBFS after position 0, time to cross from sample N",
        T_ENG,
        rate,
        config,
        "tone 1 kHz -20 dBFS then position 0",
        last_loud as f64 / fs * 1e3,
        "ms",
        Tolerance::AtMost(100.0),
    ));
    // Both routes (AS4 "either route"): on the earphone route the after-stage-11 tap is the
    // volume control's output, on the speaker route the knee of 0 · x + 0.0 (the 159 Hz corner
    // sits before the multiply); exactly +0.0 either way.
    let mut s = configs::settings(config, rate);
    s.tap = Tap::AfterVolumeAmp;
    let z = run_then_zero(s);
    let nonzero = z[n - 1..].iter().filter(|v| v.to_bits() != 0).count();
    rows.push(MeasurementResult::new(
        "FR-009",
        "samples not exactly +0.0 at the after-stage-11 tap from sample N after position 0",
        T_ENG,
        rate,
        config,
        "tone 1 kHz -20 dBFS then position 0",
        nonzero as f64,
        "samples",
        Tolerance::Exact(0.0),
    ));
    rows
}

/// FR-004 (SC-004): a full-scale 1 kHz sine at the reference position peaks within −1.0 … 0 dB
/// of full scale on the speaker route with the speaker bypassed (just at the ceiling; the
/// knee's compression), with THD at most 1.5 % at 44.1 kHz and above (US1 scenario 2), and
/// reads the loaded law's earphone figure (about −19.2 dB) within ±1.5 dB on the earphone
/// route.
pub fn default_position_level(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let x = tone(0.0, 1.0, rate);
    let spk = run(
        make,
        configs::settings("output_only_amp", rate),
        Route::Speaker,
        VOLUME_REFERENCE,
        &x,
    );
    let ear = run(
        make,
        configs::settings("output_only_earphone", rate),
        Route::Earphone,
        VOLUME_REFERENCE,
        &x,
    );
    let mut rows = vec![
        MeasurementResult::new(
            "FR-004",
            "speaker route peak at the reference position (speaker bypassed)",
            T_SCALE,
            rate,
            "output_only_amp",
            "tone 1 kHz 0 dBFS",
            peak_dbfs(&spk),
            "dBFS",
            Tolerance::Range(-1.0, 0.0),
        ),
        MeasurementResult::new(
            "FR-004",
            "earphone route level at the reference position minus the loaded law's figure",
            T_SCALE,
            rate,
            "output_only_earphone",
            "tone 1 kHz 0 dBFS",
            analysis::level_dbfs_aes17(&ear[ear.len() / 2..])
                - f64::from(earphone_gain_db(VOLUME_REFERENCE)),
            "dB",
            Tolerance::Range(-1.5, 1.5),
        ),
        MeasurementResult::new(
            "FR-004",
            "earphone route level at the reference position",
            T_SCALE,
            rate,
            "output_only_earphone",
            "tone 1 kHz 0 dBFS",
            analysis::level_dbfs_aes17(&ear[ear.len() / 2..]),
            "dBFS",
            Tolerance::Report,
        ),
    ];
    if rate >= 44_100 {
        rows.push(MeasurementResult::new(
            "FR-006",
            "THD at the reference position (a sine at the ceiling)",
            T_AMP,
            rate,
            "output_only_amp",
            "tone 1 kHz 0 dBFS",
            thd_percent(&spk, rate, 0.2),
            "%",
            Tolerance::AtMost(1.5),
        ));
    }
    rows
}

/// FR-007 (SC-001, as amended 2026-10-15): a full-scale sine at position 1 on the earphone
/// route reads a steady-state amplitude (AES17, after 0.2 s) within −0.5 … 0 dB of full scale,
/// no steady-state sample above 1.0 (the whole-signal peak, which carries the resampler's
/// onset ring, is reported), and at 44.1 kHz and above THD+N below 0.1 % (no clipper on the
/// earphone path).
pub fn earphone_max_volume(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let x = tone(0.0, 1.0, rate);
    let settings = configs::settings("output_only_earphone", rate);
    let y = run(make, settings, Route::Earphone, 1.0, &x);
    // The steady state: a tone that starts abruptly rings the resampler pair (3 % at the onset
    // at 16 kHz and above, in the chain's input, not the chain), so the peak is read after the
    // settle, as the THD is.
    let steady = &y[(0.2 * f64::from(rate)) as usize..];
    let mut rows = vec![
        MeasurementResult::new(
            "FR-007",
            "earphone route whole-signal peak at position 1 (the onset included; the resampler's transient)",
            T_EAR,
            rate,
            "output_only_earphone",
            "tone 1 kHz 0 dBFS",
            peak_dbfs(&y),
            "dBFS",
            Tolerance::Report,
        ),
        // The sine's amplitude (AES17), not the largest sample: at 6 and 8 kHz a 1 kHz tone has
        // 6 or 8 samples per cycle, so after the corners' 24° of phase no sample sits on the
        // peak and a sample reading measures phase, not level (the spec 004 lesson).
        MeasurementResult::new(
            "FR-007",
            "earphone route level at position 1 (steady state, AES17 amplitude)",
            T_EAR,
            rate,
            "output_only_earphone",
            "tone 1 kHz 0 dBFS, after 0.2 s",
            analysis::level_dbfs_aes17(steady),
            "dBFS",
            Tolerance::Range(-0.5, 0.0),
        ),
        MeasurementResult::new(
            "FR-007",
            "earphone route peak sample at position 1 (steady state)",
            T_EAR,
            rate,
            "output_only_earphone",
            "tone 1 kHz 0 dBFS, after 0.2 s",
            peak(steady),
            "",
            Tolerance::AtMost(1.0),
        ),
    ];
    if rate >= 44_100 {
        rows.push(MeasurementResult::new(
            "FR-007",
            "earphone route THD+N at position 1",
            T_EAR,
            rate,
            "output_only_earphone",
            "tone 1 kHz 0 dBFS",
            thd_percent(&y, rate, 0.2),
            "%",
            Tolerance::AtMost(0.1),
        ));
    }
    rows
}

// ---------------------------------------------------------------------------------------------
// FR-006 / SC-002: the clipper
// ---------------------------------------------------------------------------------------------

/// FR-006 (SC-002), 44.1 kHz and above: full-scale 1 kHz sines with the **volume** set so the
/// level sits −6, 0 and +6 dB re the ceiling (the 0.8 chain would hold the input at full
/// scale, so the volume, not the input, drives the clipper; coach Q6), on the amplifier path
/// alone: the output never exceeds 1.0; 6 dB over peaks at ≥ −0.1 dB with 15 – 30 % THD; 10 %
/// THD is reached between +1 and +3 dB over (bisection on the position); 6 dB under has at most
/// 1 %; a sine at the ceiling is compressed by at most 1 dB.
pub fn clip(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    assert!(
        rate >= 44_100,
        "distortion is measured at 44.1 kHz and above (D8)"
    );
    let fs = f64::from(rate);
    let settings = configs::settings("output_only_amp", rate);
    let x = tone(0.0, 1.0, rate);
    let at = |p: f32| run(make, settings, Route::Speaker, p, &x);
    // The levels are re the knee's own level (the corners' 1 kHz residual removed, N4).
    let (under, at_ceiling, over) = (
        at(position_over_ceiling(-6.0, fs)),
        at(position_over_ceiling(0.0, fs)),
        at(position_over_ceiling(6.0, fs)),
    );
    // The 10 % crossing: bisection on the position between the ceiling and 6 dB over.
    let (mut lo, mut hi) = (
        position_over_ceiling(0.0, fs),
        position_over_ceiling(6.0, fs),
    );
    for _ in 0..24 {
        let mid = 0.5 * (lo + hi);
        if thd_percent(&at(mid), rate, 0.2) < 10.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let ten_pct_db = over_ceiling_db(0.5 * (lo + hi), fs);
    let all_peak = [&under, &at_ceiling, &over]
        .iter()
        .map(|y| peak(y))
        .fold(0.0f64, f64::max);
    let stim = |l: f64| format!("tone 1 kHz 0 dBFS, volume for {l:+.0} dB re the ceiling");
    vec![
        MeasurementResult::new(
            "FR-006",
            "peak sample over the three levels (never above 1.0)",
            T_AMP,
            rate,
            "output_only_amp",
            "three volumes",
            all_peak,
            "",
            Tolerance::AtMost(1.0),
        ),
        MeasurementResult::new(
            "FR-006",
            "THD at 6 dB under the ceiling",
            T_AMP,
            rate,
            "output_only_amp",
            stim(-6.0),
            thd_percent(&under, rate, 0.2),
            "%",
            Tolerance::AtMost(1.0),
        ),
        MeasurementResult::new(
            "FR-006",
            "compression at the ceiling (steady state, after the resampler's onset transient)",
            T_AMP,
            rate,
            "output_only_amp",
            stim(0.0),
            -peak_dbfs(&at_ceiling[(0.2 * fs) as usize..]),
            "dB",
            Tolerance::Range(0.0, 1.0),
        ),
        MeasurementResult::new(
            "FR-006",
            "peak at 6 dB over the ceiling",
            T_AMP,
            rate,
            "output_only_amp",
            stim(6.0),
            peak_dbfs(&over),
            "dBFS",
            Tolerance::Range(-0.1, 0.0),
        ),
        MeasurementResult::new(
            "FR-006",
            "THD at 6 dB over the ceiling",
            T_AMP,
            rate,
            "output_only_amp",
            stim(6.0),
            thd_percent(&over, rate, 0.2),
            "%",
            Tolerance::Range(15.0, 30.0),
        ),
        MeasurementResult::new(
            "FR-006",
            "level re the ceiling at which THD reaches 10 %",
            T_AMP,
            rate,
            "output_only_amp",
            "bisection on the position",
            ten_pct_db,
            "dB",
            Tolerance::Range(1.0, 3.0),
        ),
    ]
}

/// FR-006 at 6, 8 and 16 kHz: a sine 6 dB over the ceiling never exceeds 1.0 (the peak ceiling
/// only; distortion aliases within the host band there, D8).
pub fn peak_ceiling(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let settings = configs::settings("output_only_amp", rate);
    let x = tone(0.0, 1.0, rate);
    let y = run(make, settings, Route::Speaker, VOLUME_6DB_OVER, &x);
    vec![MeasurementResult::new(
        "FR-006",
        "peak sample at 6 dB over the ceiling",
        T_AMP,
        rate,
        "output_only_amp",
        "tone 1 kHz 0 dBFS, volume for +6 dB",
        peak(&y),
        "",
        Tolerance::AtMost(1.0),
    )]
}

/// Every US1 check at `rate` (the matrix report and the story test).
pub fn all_us1(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let mut rows = Vec::new();
    rows.extend(earphone_response(rate, make));
    rows.extend(amplifier_response(rate, make));
    rows.extend(amplifier_gain(rate, make));
    rows.extend(speaker_response(rate, make));
    for route in [Route::Speaker, Route::Earphone] {
        rows.extend(group_delay_1k(rate, route, make));
        rows.extend(volume_law(rate, route, make));
        rows.extend(volume_zero(rate, route, make));
    }
    rows.extend(default_position_level(rate, make));
    rows.extend(earphone_max_volume(rate, make));
    if rate >= 44_100 {
        rows.extend(clip(rate, make));
    } else {
        rows.extend(peak_ceiling(rate, make));
    }
    rows
}

// ---------------------------------------------------------------------------------------------
// User Story 2: the whole chain (tasks.md T016; FR-008 – FR-010, FR-013; SC-005)
// ---------------------------------------------------------------------------------------------

use rr_dr60::{Pipeline, StopResult, speech_codec};

use crate::{codec_checks, golden, split_checks};

/// Trace: the equivalence rule (spec 008 FR-009; spec 006 FR-007) with the chain on.
const T_RULE: &[&str] = &["engineering target"];

fn pass(ok: bool) -> f64 {
    f64::from(u8::from(ok))
}

/// The one-pass of `x` through `settings` at `route` and `volume` in blocks of `block`.
fn one_pass_blocks(
    make: Make<'_>,
    settings: Settings,
    route: Route,
    volume: f32,
    x: &[f32],
    block: usize,
) -> Vec<f32> {
    let mut p = make(settings);
    p.set_route(route);
    p.set_volume(volume).unwrap();
    let mut out = Vec::with_capacity(x.len());
    for chunk in x.chunks(block.max(1)) {
        let mut y = vec![0.0f32; chunk.len()];
        let n = p.process(chunk, &mut y).expect("equal lengths").produced;
        out.extend_from_slice(&y[..n]);
    }
    out
}

/// SC-005 (FR-010): record-then-play equals the one-pass bit for bit under spec 008 FR-009's
/// rule with the chain on, on `route` at `volume` in the 0.9 default with the VAS off (and,
/// on the speaker route at the reference position, with the VAS on and a stimulus that ends
/// in sound); the skipped prefix asserted exactly 0.0 first (coach C8).
pub fn chain_equivalence(
    rate: u32,
    route: Route,
    volume: f32,
    make: Make<'_>,
) -> Vec<MeasurementResult> {
    chain_equivalence_in(rate, "default_output", route, volume, make)
}

/// The "(VAS off)" label of a configuration name, as a static string (the row's `config`).
fn vas_off_label(config: &str) -> &'static str {
    match config {
        "default_output" => "default_output (VAS off)",
        "default_output_clamp" => "default_output_clamp (VAS off)",
        "default_earphone_clamp" => "default_earphone_clamp (VAS off)",
        _ => "(VAS off)",
    }
}

/// [`chain_equivalence`] in the named configuration (spec 010 T013: the clamp configurations
/// reuse the check; the 009 callers pass `default_output`).
pub fn chain_equivalence_in(
    rate: u32,
    config: &'static str,
    route: Route,
    volume: f32,
    make: Make<'_>,
) -> Vec<MeasurementResult> {
    let x = stimulus::voiced(f64::from(rate), 2.0, -20.0, 0x0D60);
    chain_equivalence_with(rate, config, route, volume, &x, "voiced -20 dBFS", make)
}

/// [`chain_equivalence_in`] on an explicit stimulus `x` (which must end in sound, for the
/// VAS-on case) labelled `stim` (spec 010 T013: the full-scale square engages the bound).
#[allow(clippy::too_many_arguments)]
pub fn chain_equivalence_with(
    rate: u32,
    config: &'static str,
    route: Route,
    volume: f32,
    x: &[f32],
    stim: &str,
    make: Make<'_>,
) -> Vec<MeasurementResult> {
    let mut rows = Vec::new();
    let d = codec_checks::delay_host(rate);
    let per_device = split_checks::host_per_device(rate);
    let x = x.to_vec();
    let mut no_vas = configs::settings(config, rate);
    no_vas.vas.enabled = false;
    // Both with the VAS off and with it on (the voiced stimulus ends in sound, which the
    // VAS-on rule needs; coach review T019, N6).
    let cases = [
        (vas_off_label(config), no_vas),
        (config, configs::settings(config, rate)),
    ];
    for (config, settings) in cases {
        let mut p = make(settings);
        p.set_route(route);
        p.set_volume(volume).unwrap();
        let run = split_checks::record_then_play(
            &mut p,
            &x,
            split_checks::RECORD_BLOCK,
            split_checks::PLAY_BLOCK,
        );
        let StopResult::Kept { .. } = run.stop.result else {
            unreachable!()
        };
        let pre_encoder = p.record_latency_samples() as usize - d;
        let drain = (speech_codec::DELAY_SAMPLES - speech_codec::LOOKAHEAD_SAMPLES) as usize;
        let tail = pre_encoder + (run.stop.silence_fed_samples as usize + drain + 1) * per_device;
        let mut y = x.clone();
        y.extend(std::iter::repeat_n(0.0f32, tail));
        let want = one_pass_blocks(make, settings, route, volume, &y, 4096);
        // FR-010's precondition where it states it: the decoder's leading-delay output at the
        // chain's input (`Tap::AfterPlayback`), and at the route output too (zeros out of a
        // linear chain at rest).
        let mut at_input = settings;
        at_input.tap = Tap::AfterPlayback;
        let input = one_pass_blocks(make, at_input, route, volume, &y[..d + 1], 4096);
        let prefix_zero = want[..d.min(want.len())].iter().all(|v| v.to_bits() == 0)
            && input[..d.min(input.len())].iter().all(|v| v.to_bits() == 0);
        let got = &run.pre_drain;
        let n = got.len();
        let same = want.len() >= d + n
            && got
                .iter()
                .zip(&want[d..d + n])
                .all(|(a, b)| a.to_bits() == b.to_bits());
        let label = format!("{stim}, {route:?}, position {volume}");
        rows.push(MeasurementResult::new(
            "FR-010",
            "the one-pass's skipped prefix (the codec delay) is exactly 0.0",
            T_RULE,
            rate,
            config,
            label.clone(),
            pass(prefix_zero),
            "",
            Tolerance::Exact(1.0),
        ));
        rows.push(MeasurementResult::new(
            "FR-010",
            "record-then-play equals the one-pass skipped by the codec delay, chain on",
            T_RULE,
            rate,
            config,
            label,
            pass(same),
            "",
            Tolerance::Exact(1.0),
        ));
    }
    rows
}

/// SC-005 (FR-011): the one-pass at `Tap::AfterPlayback` with the chain on equals version 0.8's
/// default output (the chain bypassed at that tap) bit for bit on the four spec 001 stimuli.
pub fn chain_after_playback_equals_0_8(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let mut on = configs::settings_011(rate);
    on.tap = Tap::AfterPlayback;
    let old = configs::settings_008(rate);
    golden::STIMULI
        .iter()
        .map(|stim| {
            let x = golden::stimulus(stim, rate);
            let a = one_pass_blocks(make, on, Route::Speaker, 1.0, &x, 4096);
            let b = one_pass_blocks(make, old, Route::Speaker, 1.0, &x, 4096);
            let same =
                a.len() == b.len() && a.iter().zip(&b).all(|(p, q)| p.to_bits() == q.to_bits());
            MeasurementResult::new(
                "FR-011",
                "one-pass at AfterPlayback with the chain on equals 0.8's output",
                T_RULE,
                rate,
                "default_output",
                *stim,
                pass(same),
                "",
                Tolerance::Exact(1.0),
            )
        })
        .collect()
}

/// The sixteen change points (blocks of 64 in a 4096-sample input) the timing checks use.
pub(crate) const CHANGE_POINTS: [usize; 16] =
    [1, 2, 3, 5, 8, 13, 21, 34, 40, 45, 50, 55, 58, 60, 62, 63];

/// Runs `x` in 64-sample blocks, calling `between(p, block_index)` before each block.
pub(crate) fn in_blocks(
    p: &mut Pipeline,
    x: &[f32],
    mut between: impl FnMut(&mut Pipeline, usize),
) -> Vec<f32> {
    let mut out = Vec::with_capacity(x.len());
    for (i, chunk) in x.chunks(64).enumerate() {
        between(p, i);
        let mut y = vec![0.0f32; chunk.len()];
        let n = p.process(chunk, &mut y).expect("ok").produced;
        out.extend_from_slice(&y[..n]);
    }
    out
}

/// The chain alone with both branches on (every earlier stage bypassed), so the output has the
/// input's length and a route change compares block for block.
fn chain_only_both(rate: u32) -> Settings {
    let mut s = configs::settings("output_only_speaker", rate);
    s.output = OutputSettings::DEVICE;
    s
}

/// SC-005 (FR-009): a route change between blocks lands exactly at the next block at every
/// one of sixteen change points: the blocks before equal the unchanged run bit for bit and the
/// first block after differs from it.
pub fn chain_route_change_timing(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let s = chain_only_both(rate);
    let x = tone(-20.0, 4096.0 / f64::from(rate), rate);
    let mut speaker = make(s);
    speaker.set_volume(1.0).unwrap();
    let speaker = in_blocks(&mut speaker, &x, |p, _| p.set_route(Route::Speaker));
    let mut landed = 0;
    for &k in &CHANGE_POINTS {
        let mut p = make(s);
        p.set_volume(1.0).unwrap();
        let y = in_blocks(&mut p, &x, |p, i| {
            p.set_route(if i < k {
                Route::Speaker
            } else {
                Route::Earphone
            });
        });
        let before = y[..64 * k] == speaker[..64 * k];
        let after = y[64 * k..64 * (k + 1)] != speaker[64 * k..64 * (k + 1)];
        if before && after {
            landed += 1;
        }
    }
    let mut rows = vec![MeasurementResult::new(
        "FR-009",
        "route changes landing exactly at the next block (one-pass)",
        T_RULE,
        rate,
        "chain only, both branches",
        "tone 1 kHz -20 dBFS, 64-sample blocks, 16 change points",
        f64::from(landed),
        "of 16",
        Tolerance::Exact(16.0),
    )];
    // AS3's Given is a playback in progress (coach review T019, S3): the same through `play`,
    // the tone recorded first (the chain alone stores the samples), four change points.
    let play_points = [1usize, 8, 32, 63];
    let mut p = make(s);
    p.set_volume(1.0).unwrap();
    p.start_recording().unwrap();
    let _ = p.record(&x, &mut []).unwrap();
    let _ = p.stop_recording(&mut []).unwrap();
    let unswitched = play_in_blocks(&mut p, 1, 64, |p, _| p.set_route(Route::Speaker));
    let mut played_landed = 0;
    for &k in &play_points {
        let y = play_in_blocks(&mut p, 1, 64, |p, i| {
            p.set_route(if i < k {
                Route::Speaker
            } else {
                Route::Earphone
            });
        });
        let before = y[..64 * k] == unswitched[..64 * k];
        let after = y[64 * k..64 * (k + 1)] != unswitched[64 * k..64 * (k + 1)];
        if before && after {
            played_landed += 1;
        }
    }
    rows.push(MeasurementResult::new(
        "FR-009",
        "route changes landing exactly at the next block (playback in progress)",
        T_RULE,
        rate,
        "chain only, both branches",
        "recorded tone 1 kHz -20 dBFS, 64-sample play blocks, 4 change points",
        f64::from(played_landed),
        "of 4",
        Tolerance::Exact(4.0),
    ));
    // The rest state after a real signal (coach review T019, S4): at the identity rate with
    // stage 11 bypassed (no shared state between the branches and no resampler), the output
    // after the switch equals a fresh engine's on the earphone route fed the remaining input,
    // bit for bit; at the other rates the resampler's state makes that comparison impossible
    // and the reset is the core unit test's (`route_change_resets_sections…`).
    if rate == rr_dr60::DEVICE_RATE_HZ {
        let mut t = s;
        t.output.volume_amp_enabled = false;
        let mut rest = 0;
        for &k in &CHANGE_POINTS {
            let mut p = make(t);
            let y = in_blocks(&mut p, &x, |p, i| {
                p.set_route(if i < k {
                    Route::Speaker
                } else {
                    Route::Earphone
                });
            });
            let mut fresh = make(t);
            let want = in_blocks(&mut fresh, &x[64 * k..], |p, _| {
                p.set_route(Route::Earphone)
            });
            if y[64 * k..] == want[..] {
                rest += 1;
            }
        }
        rows.push(MeasurementResult::new(
            "FR-009",
            "the newly selected route starts from rest after a real signal (6 kHz, stage 11 off)",
            T_RULE,
            rate,
            "chain only, stages 12 and 13",
            "tone 1 kHz -20 dBFS, 64-sample blocks, 16 change points",
            f64::from(rest),
            "of 16",
            Tolerance::Exact(16.0),
        ));
    }
    rows
}

/// SC-005 (FR-009): a volume change (1.0 → 0.5) between blocks lands at the next block at every
/// one of sixteen change points: at the earphone's after-stage-11 tap the output is
/// `K_EAR(p) · u`, where `u` (the 32 Hz corner's output) does not depend on the position, so
/// the blocks before equal the position-1 run bit for bit, and since spec 011 the output from
/// **transition sample N** equals the position-0.5 run bit for bit (the 20 ms transition between
/// them is checked by `volume_ramp_checks`; tasks.md T019). The stimulus is 8192 samples so at
/// least 64 samples are compared after sample N at every change point and rate (plan review Q1).
pub fn chain_volume_change_timing(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    // Both routes at the after-stage-11 tap (coach review T019, N5): on the speaker route
    // `a = knee(k_spk · amp_hpf(u))`, the corner position-independent and the knee memoryless,
    // so the same block-exact equalities hold.
    let x = tone(-20.0, 8192.0 / f64::from(rate), rate);
    let n = rr_dr60::output::volume_transition_samples(rate).expect("a supported rate") as usize;
    let mut rows = Vec::new();
    for (config, route) in [
        ("output_only_earphone", Route::Earphone),
        ("output_only_amp", Route::Speaker),
    ] {
        let mut s = configs::settings(config, rate);
        s.tap = Tap::AfterVolumeAmp;
        let at = |volume: f32| {
            let mut p = make(s);
            p.set_route(route);
            in_blocks(&mut p, &x, |p, _| p.set_volume(volume).unwrap())
        };
        let (full, half) = (at(1.0), at(0.5));
        let mut landed = 0;
        for &k in &CHANGE_POINTS {
            let mut p = make(s);
            p.set_route(route);
            let y = in_blocks(&mut p, &x, |p, i| {
                p.set_volume(if i < k { 1.0 } else { 0.5 }).unwrap()
            });
            let from_n = 64 * k + n - 1;
            assert!(
                y.len().saturating_sub(from_n) >= 64,
                "{rate} Hz: at least 64 samples compared after sample N (change point {k})"
            );
            if y[..64 * k] == full[..64 * k] && y[from_n..] == half[from_n..] {
                landed += 1;
            }
        }
        rows.push(MeasurementResult::new(
            "FR-009",
            "volume changes landing exactly at the next block with no overshoot",
            T_RULE,
            rate,
            config,
            "tone 1 kHz -20 dBFS, 8192 samples, 64-sample blocks, 16 change points; after from sample N",
            f64::from(landed),
            "of 16",
            Tolerance::Exact(16.0),
        ));
    }
    rows
}

/// FR-013: a recording made and exported with the chain bypassed (a 0.8 engine), imported into
/// an engine with the chain on and played, equals the one-pass of the same stimulus with the
/// chain on under the equivalence rule; the stored bytes are byte-identical before and after.
pub fn chain_legacy_recording(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let d = codec_checks::delay_host(rate);
    let per_device = split_checks::host_per_device(rate);
    let x = stimulus::voiced(f64::from(rate), 2.0, -20.0, 0x0D60);
    let mut old = configs::settings_008(rate);
    old.vas.enabled = false;
    let mut p_old = make(old);
    let run = split_checks::record_then_play(
        &mut p_old,
        &x,
        split_checks::RECORD_BLOCK,
        split_checks::PLAY_BLOCK,
    );
    let mut bytes = vec![0u8; p_old.export_len(run.number).unwrap()];
    let n = p_old.export(run.number, &mut bytes).unwrap();
    bytes.truncate(n);
    let mut new = configs::settings("default_output", rate);
    new.vas.enabled = false;
    let mut p_new = make(new);
    let number = p_new.import(&bytes).unwrap();
    let mut again = vec![0u8; p_new.export_len(number).unwrap()];
    let m = p_new.export(number, &mut again).unwrap();
    again.truncate(m);
    let mut rows = vec![MeasurementResult::new(
        "FR-013",
        "stored bytes byte-identical after import into a chain-on engine",
        T_RULE,
        rate,
        "default_output",
        "voiced -20 dBFS recorded by a 0.8 engine",
        pass(again == bytes),
        "",
        Tolerance::Exact(1.0),
    )];
    for (route, volume) in [(Route::Speaker, VOLUME_REFERENCE), (Route::Earphone, 1.0)] {
        p_new.set_route(route);
        p_new.set_volume(volume).unwrap();
        p_new.start_playback(number).unwrap();
        let mut played = Vec::new();
        let mut buf = vec![0.0f32; split_checks::PLAY_BLOCK];
        loop {
            let info = p_new.play(&mut buf).unwrap();
            played.extend_from_slice(&buf[..info.produced]);
            if info.finished {
                break;
            }
        }
        let pre_drain = played.len() - p_new.drain_samples() as usize;
        let got = &played[..pre_drain];
        let pre_encoder = p_new.record_latency_samples() as usize - d;
        let drain = (speech_codec::DELAY_SAMPLES - speech_codec::LOOKAHEAD_SAMPLES) as usize;
        let tail = pre_encoder + (run.stop.silence_fed_samples as usize + drain + 1) * per_device;
        let mut y = x.clone();
        y.extend(std::iter::repeat_n(0.0f32, tail));
        let want = one_pass_blocks(make, new, route, volume, &y, 4096);
        let same = want.len() >= d + got.len()
            && got
                .iter()
                .zip(&want[d..d + got.len()])
                .all(|(a, b)| a.to_bits() == b.to_bits());
        rows.push(MeasurementResult::new(
            "FR-013",
            "a 0.8 recording played through the chain equals the one-pass with the chain on",
            T_RULE,
            rate,
            "default_output",
            format!("{route:?}, position {volume}"),
            pass(same),
            "",
            Tolerance::Exact(1.0),
        ));
    }
    rows
}

/// Spec 006's drain table, ⌈N₁₀·m/l⌉ + 2D, per rate (006 research R-06).
const DRAIN_006: [u32; 7] = [121, 242, 483, 1328, 1444, 2653, 2888];
/// The chain's settle `N_OUT` per rate, the design script's printed figures (research R-08;
/// a literal, never the library's constant, so a moved constant is caught).
const N_OUT_009: [u32; 7] = [346, 461, 922, 2539, 2764, 5078, 5527];

/// Plays recording `number` in blocks of `block`, calling `between(p, block_index)` before
/// each block; the whole playback, drain included.
fn play_in_blocks(
    p: &mut Pipeline,
    number: u32,
    block: usize,
    mut between: impl FnMut(&mut Pipeline, usize),
) -> Vec<f32> {
    p.start_playback(number).unwrap();
    let mut out = Vec::new();
    let mut buf = vec![0.0f32; block];
    let mut i = 0;
    loop {
        between(p, i);
        let info = p.play(&mut buf).unwrap();
        out.extend_from_slice(&buf[..info.produced]);
        i += 1;
        if info.finished {
            break;
        }
    }
    out
}

/// Research R-08 (spec 006 FR-008 restated): `drain_samples` is spec 006's figure plus `N_OUT`
/// with the chain on (every single stage too), spec 006's figure with it off; the playback's
/// total length is ⌈K·m/l⌉ + drain; the last 1 ms of the drain is below −80 dBFS on both routes.
pub fn chain_drain_table(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let i = rr_dr60::SUPPORTED_HOST_RATES
        .iter()
        .position(|&r| r == rate)
        .unwrap();
    let n_out = N_OUT_009[i];
    let on = make(configs::settings("default_output", rate));
    let off = make(configs::settings_008(rate));
    let mut rows = vec![
        MeasurementResult::new(
            "FR-008",
            "output::settle_samples against the design's printed N_OUT",
            T_RULE,
            rate,
            "constant",
            "settle_samples(rate)",
            f64::from(rr_dr60::output::settle_samples(rate).unwrap()),
            "samples",
            Tolerance::Exact(f64::from(n_out)),
        ),
        MeasurementResult::new(
            "FR-008",
            "drain with the chain bypassed (spec 006's table)",
            T_RULE,
            rate,
            "0.8 default",
            "drain_samples()",
            f64::from(off.drain_samples()),
            "samples",
            Tolerance::Exact(f64::from(DRAIN_006[i])),
        ),
        MeasurementResult::new(
            "FR-008",
            "drain with the chain on minus the bypassed drain (N_OUT)",
            T_RULE,
            rate,
            "default_output",
            "drain_samples()",
            f64::from(on.drain_samples()) - f64::from(off.drain_samples()),
            "samples",
            Tolerance::Exact(f64::from(n_out)),
        ),
    ];
    for only in 0..3 {
        let mut s = configs::settings_011(rate);
        s.output.volume_amp_enabled = only == 0;
        s.output.speaker_enabled = only == 1;
        s.output.earphone_enabled = only == 2;
        let p = make(s);
        rows.push(MeasurementResult::new(
            "FR-008",
            "drain with one output stage on minus the bypassed drain",
            T_RULE,
            rate,
            "one stage",
            format!("stage {}", 11 + only),
            f64::from(p.drain_samples()) - f64::from(off.drain_samples()),
            "samples",
            Tolerance::Exact(f64::from(n_out)),
        ));
    }
    let x = stimulus::voiced(f64::from(rate), 1.0, -20.0, 0x0D60);
    for route in [Route::Speaker, Route::Earphone] {
        let mut s = configs::settings("default_output", rate);
        s.vas.enabled = false;
        let mut p = make(s);
        p.set_route(route);
        p.set_volume(1.0).unwrap();
        let run = split_checks::record_then_play(
            &mut p,
            &x,
            split_checks::RECORD_BLOCK,
            split_checks::PLAY_BLOCK,
        );
        let StopResult::Kept { length, .. } = run.stop.result else {
            unreachable!()
        };
        rows.push(MeasurementResult::new(
            "FR-008",
            "playback length minus ceil(K·m/l) + drain",
            T_RULE,
            rate,
            "default_output",
            format!("{route:?}"),
            (run.pre_drain.len() + run.drain.len()) as f64
                - (split_checks::playback_len(length, rate) + u64::from(p.drain_samples())) as f64,
            "samples",
            Tolerance::Exact(0.0),
        ));
        let ms = (rate as usize / 1000).max(1);
        let last = &run.drain[run.drain.len() - ms..];
        rows.push(MeasurementResult::new(
            "FR-008",
            "last 1 ms of the drain, 0.9 default (the codec's tail, far below the floor)",
            T_RULE,
            rate,
            "default_output",
            format!("{route:?}"),
            analysis::rms_dbfs(last).max(-200.0),
            "dBFS",
            Tolerance::AtMost(-80.0),
        ));
    }
    // The chain's own ring-down (coach review T019, S1): the chain alone (no codec, no stage
    // 10), a full-scale 1 kHz tone recorded and played, so the drain is 2D + N_OUT zeros into
    // the chain and its last 1 ms is the 32 Hz corner's state N_OUT samples after the input
    // ended: about −100 dB of the tone, the bound −80 dBFS with its margin visible. A shorter
    // N_OUT raises this figure (the SC-006 mutant).
    let tone = tone(0.0, 0.5, rate);
    for (config, route, volume) in [
        ("output_only_speaker", Route::Speaker, VOLUME_REFERENCE),
        ("output_only_earphone", Route::Earphone, 1.0),
    ] {
        let mut p = make(configs::settings(config, rate));
        p.set_route(route);
        p.set_volume(volume).unwrap();
        let run = split_checks::record_then_play(
            &mut p,
            &tone,
            split_checks::RECORD_BLOCK,
            split_checks::PLAY_BLOCK,
        );
        let ms = (rate as usize / 1000).max(1);
        let last = &run.drain[run.drain.len() - ms..];
        rows.push(MeasurementResult::new(
            "FR-008",
            "last 1 ms of the drain, the chain alone (its ring-down at N_OUT)",
            T_RULE,
            rate,
            config,
            format!("tone 1 kHz 0 dBFS, {route:?}, position {volume}"),
            analysis::rms_dbfs(last).max(-200.0),
            "dBFS",
            Tolerance::AtMost(-80.0),
        ));
    }
    rows
}

/// FR-008 (SC-007): the three reported latencies with the chain on equal their chain-off
/// values and the 0.8 tables.
pub fn chain_latencies_unchanged(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let i = rr_dr60::SUPPORTED_HOST_RATES
        .iter()
        .position(|&r| r == rate)
        .unwrap();
    const ONE_PASS: [u32; 7] = [181, 321, 642, 1767, 1923, 3533, 3846]; // spec 008 (006 + the codec delay)
    const RECORD: [u32; 7] = [181, 281, 561, 1546, 1682, 3091, 3365]; // spec 006 FR-016 + the codec delay (spec 008 FR-004)
    const PLAYBACK: [u32; 7] = [0, 40, 81, 221, 241, 442, 481]; // spec 006 FR-016
    let off = make(configs::settings_008(rate));
    let mut rows = Vec::new();
    // AS5's "any configuration" (coach review T019, N9): the 0.9 default, the three chain-only
    // configurations and each single stage.
    let mut configs_on: Vec<(String, Settings)> = [
        "default_output",
        "output_only_speaker",
        "output_only_earphone",
        "output_only_amp",
    ]
    .iter()
    .map(|c| (c.to_string(), configs::settings(c, rate)))
    .collect();
    for only in 0..3 {
        let mut s = configs::settings_011(rate);
        s.output.volume_amp_enabled = only == 0;
        s.output.speaker_enabled = only == 1;
        s.output.earphone_enabled = only == 2;
        configs_on.push((format!("stage {} alone", 11 + only), s));
    }
    for (name, s) in &configs_on[1..] {
        let mut bypassed = *s;
        bypassed.output = OutputSettings::BYPASSED;
        bypassed.tap = Tap::AfterPlayback;
        let (on, off) = (make(*s), make(bypassed));
        rows.push(MeasurementResult::new(
            "FR-008",
            "latencies (one-pass, record, playback) unchanged by the chain",
            T_RULE,
            rate,
            "any configuration",
            name.clone(),
            pass(
                on.latency_samples() == off.latency_samples()
                    && on.record_latency_samples() == off.record_latency_samples()
                    && on.playback_latency_samples() == off.playback_latency_samples(),
            ),
            "",
            Tolerance::Exact(1.0),
        ));
    }
    // FR-013's first clause: the memory's accounting is unchanged by the chain (the same
    // recording on a chain-on and a chain-off engine reports the same status).
    let x = stimulus::voiced(f64::from(rate), 1.0, -20.0, 0x0D60);
    let status = |s: Settings| {
        let mut p = make(s);
        p.start_recording().unwrap();
        let _ = p.record(&x, &mut []).unwrap();
        let _ = p.stop_recording(&mut []).unwrap();
        (p.memory_status(), p.footprint_bytes())
    };
    let (a, b) = (
        status(configs::settings("default_output", rate)),
        status(configs::settings_008(rate)),
    );
    rows.push(MeasurementResult::new(
        "FR-013",
        "memory status and footprint equal with the chain on and off after the same recording",
        T_RULE,
        rate,
        "default_output",
        "voiced -20 dBFS, 1 s",
        pass(a == b),
        "",
        Tolerance::Exact(1.0),
    ));
    let on = make(configs::settings("default_output", rate));
    rows.extend([
        MeasurementResult::new(
            "FR-008",
            "one-pass latency with the chain on",
            T_RULE,
            rate,
            "default_output",
            "latency_samples()",
            f64::from(on.latency_samples()),
            "samples",
            Tolerance::Exact(f64::from(ONE_PASS[i])),
        ),
        MeasurementResult::new(
            "FR-008",
            "one-pass latency on minus off",
            T_RULE,
            rate,
            "default_output",
            "latency_samples()",
            f64::from(on.latency_samples()) - f64::from(off.latency_samples()),
            "samples",
            Tolerance::Exact(0.0),
        ),
        MeasurementResult::new(
            "FR-008",
            "record-path latency with the chain on",
            T_RULE,
            rate,
            "default_output",
            "record_latency_samples()",
            f64::from(on.record_latency_samples()),
            "samples",
            Tolerance::Exact(f64::from(RECORD[i])),
        ),
        MeasurementResult::new(
            "FR-008",
            "playback-path latency with the chain on",
            T_RULE,
            rate,
            "default_output",
            "playback_latency_samples()",
            f64::from(on.playback_latency_samples()),
            "samples",
            Tolerance::Exact(f64::from(PLAYBACK[i])),
        ),
    ]);
    rows
}

/// Every US2 check at `rate` (the matrix report and the story test).
pub fn all_us2(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let mut rows = Vec::new();
    for (route, volume) in [
        (Route::Speaker, VOLUME_REFERENCE),
        (Route::Speaker, 1.0),
        (Route::Earphone, VOLUME_REFERENCE),
        (Route::Earphone, 1.0),
    ] {
        rows.extend(chain_equivalence(rate, route, volume, make));
    }
    rows.extend(chain_after_playback_equals_0_8(rate, make));
    rows.extend(chain_route_change_timing(rate, make));
    rows.extend(chain_volume_change_timing(rate, make));
    rows.extend(chain_legacy_recording(rate, make));
    rows.extend(chain_drain_table(rate, make));
    rows.extend(chain_latencies_unchanged(rate, make));
    rows
}
