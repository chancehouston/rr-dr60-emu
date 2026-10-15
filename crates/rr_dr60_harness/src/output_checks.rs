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
    let rows = vec![MeasurementResult::new(
        requirement,
        "composite response vs the design's discrete realization, max |dev|",
        trace,
        rate,
        config,
        format!(
            "impulse -40 dBFS, {:.0}-{:.0} Hz, position {volume}",
            band.0, band.1
        ),
        worst,
        "dB",
        Tolerance::AtMost(0.5),
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

/// FR-009 (SC-004): at position 0 from the next block, every sample at the after-stage-11 tap
/// on the earphone route is exactly 0.0, and the route output decays below −120 dBFS within
/// 100 ms on both routes (the time to cross reported).
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
    let mut rows = Vec::new();
    let z = run_then_zero(configs::settings(config, rate));
    let last_loud = z
        .iter()
        .rposition(|v| f64::from(v.abs()) >= 1e-6)
        .map_or(0, |i| i + 1);
    rows.push(MeasurementResult::new(
        "FR-009",
        "route output below -120 dBFS after position 0, time to cross",
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
    let nonzero = z.iter().filter(|v| v.to_bits() != 0).count();
    rows.push(MeasurementResult::new(
        "FR-009",
        "samples not exactly +0.0 at the after-stage-11 tap after position 0",
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
/// reads the loaded law's earphone figure (about −18.6 dB) within ±1.5 dB on the earphone
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

/// FR-007 (SC-001): a full-scale sine at position 1 on the earphone route peaks within
/// −0.5 … 0 dB of full scale, no sample above 1.0, and at 44.1 kHz and above THD+N below
/// 0.1 % (no clipper on the earphone path).
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
