//! Spec 008 checks: the speech codec in the record path, the playback path and the one-pass
//! (US1; FR-002 – FR-009, FR-014 – FR-016; SC-001 – SC-005; research R-09). Every check returns
//! [`MeasurementResult`]s with `008/FR-xxx` and trace IDs; `us1_codec` runs them per rate.
//!
//! Story boundary (Principle VII): the US1 checks use engines and the codec-only configuration
//! only: no form bytes, no registers, no C. The tolerances are the calibrated figures of
//! `specs/008-celp-codec/calibration.md` (the ITU-T G.723.1 reference coder at 5.3 kbit/s
//! through the same metrics, plus a stated margin), never looser than the spec's floors. The
//! US2 checks (the second half of this file; FR-010 – FR-014; SC-007, SC-009) add the memory's
//! footprint, the capacity in frames, the stop padding, the version 2 byte form and its
//! corruption set, with small configured capacities; still no registers and no C.

use rr_dr60::{
    __test_hooks, Error, Pipeline, Settings, State, StopReason, StopResult, Tap, VasEvent,
    speech_codec,
};

use crate::checks::{MeasurementResult, Tolerance};
use crate::split_checks::{self, PLAY_BLOCK, RECORD_BLOCK};
use crate::{analysis, chain_checks, configs, stimulus};

/// RMS level error tolerance, dB (spec FR-005's floor; calibration.md: the reference reads
/// 0.04 dB).
pub const TOL_RMS_DB: f64 = 1.5; // 008 FR-005 (calibration.md)
/// Third-octave band tolerance 315 – 2000 Hz, dB (spec FR-005's floor; the reference 1.2 dB).
pub const TOL_THIRD_OCTAVE_DB: f64 = 3.0; // 008 FR-005 (calibration.md)
/// Log-spectral distance, dB (calibration.md: the reference 1.95 dB plus 1 dB).
pub const TOL_LSD_DB: f64 = 3.0; // 008 FR-005 (calibration.md)
/// Segmental SNR floor, dB (calibration.md: the reference 9.05 dB minus 2 dB; the spec's floor
/// is 4 dB).
pub const MIN_SEGSNR_DB: f64 = 7.0; // 008 FR-006 (calibration.md)
/// Level tracking on the unvoiced-to-voiced sequence, dB (calibration.md: the reference
/// 5.77 dB plus 1 dB; the spec's 3 dB floor is amended, FR-006).
pub const TOL_LEVEL_TRACK_DB: f64 = 6.8; // 008 FR-006 (calibration.md)
/// The silence floor after the first frame, dBFS (spec FR-006).
pub const SILENCE_FLOOR_DBFS: f64 = -60.0; // 008 FR-006
/// The delay measurement's slack, device samples (spec FR-004).
pub const DELAY_SLACK: f64 = 2.0; // 008 FR-004
/// The latency bound: 50 ms plus the bypassed figure (spec SC-002).
pub const LATENCY_BOUND_MS: f64 = 50.0; // 008 SC-002
/// The stimulus seed and length (engineering targets, 008 R-09).
const SEED: u64 = 0x0D60; // engineering target (008 R-09)
const STIM_S: f64 = 6.0; // engineering target (008 R-09)
/// The levels the transparency checks run at (spec FR-005; A-029's anchor).
pub const LEVELS_DBFS: [f64; 3] = [-10.0, -20.0, -40.0]; // 008 FR-005

/// Trace: the design constants and the class match.
const DESIGN: &[&str] = &["A-004", "A-040", "A-041", "S-008", "S-009"];
/// Trace: the integration rules.
const RULES: &[&str] = &["A-026", "A-040", "engineering target"];

#[allow(clippy::too_many_arguments)]
fn row(
    requirement: &'static str,
    property: &'static str,
    trace: &'static [&'static str],
    rate: u32,
    config: &'static str,
    stimulus: impl Into<String>,
    measured: f64,
    unit: &'static str,
    tolerance: Tolerance,
) -> MeasurementResult {
    MeasurementResult::new(
        requirement,
        property,
        trace,
        rate,
        config,
        stimulus,
        measured,
        unit,
        tolerance,
    )
}

fn pass(ok: bool) -> f64 {
    f64::from(u8::from(ok))
}

/// The one-pass output of `x` in blocks of `block` (the frames counted).
fn one_pass(settings: Settings, x: &[f32], block: usize) -> (Vec<f32>, usize) {
    let mut p = Pipeline::new(settings).expect("supported settings");
    let mut out = Vec::with_capacity(x.len());
    let mut frames = 0;
    for chunk in x.chunks(block.max(1)) {
        let mut y = vec![0.0f32; chunk.len()];
        let info = p.process(chunk, &mut y).expect("equal lengths");
        out.extend_from_slice(&y[..info.produced]);
        frames += info.frames;
    }
    (out, frames)
}

/// The codec's delay in host samples at `rate` (an integer at every rate, FR-004).
pub fn delay_host(rate: u32) -> usize {
    (speech_codec::DELAY_SAMPLES * rate / 6000) as usize
}

/// `codec_only` with the codec bypassed: the reference every codec-only comparison uses (the
/// resampling boundary identical on both sides, research R-09).
fn codec_only_bypassed(rate: u32) -> Settings {
    let mut s = configs::settings("codec_only", rate);
    s.speech_codec_enabled = false;
    s
}

fn voiced(rate: u32, level: f64) -> Vec<f32> {
    stimulus::voiced(f64::from(rate), STIM_S, level, SEED)
}

fn unvoiced_voiced(rate: u32, level: f64) -> Vec<f32> {
    stimulus::unvoiced_voiced(f64::from(rate), STIM_S, level, SEED)
}

/// The codec output aligned with its bypassed reference: the reference and the output with
/// the delay skipped, cut to the same length, at the host rate.
fn aligned(rate: u32, x: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let (reference, _) = one_pass(codec_only_bypassed(rate), x, 4096);
    let (coded, _) = one_pass(configs::settings("codec_only", rate), x, 4096);
    let d = delay_host(rate);
    let n = reference.len().min(coded.len().saturating_sub(d));
    (reference[..n].to_vec(), coded[d..d + n].to_vec())
}

/// The rate the spectral figures are measured at: the calibration's 8 kHz (research R-09; the
/// reference coder ran there), whatever the host rate; an order-8 envelope over a 44.1 kHz
/// bandwidth would not measure a 3 kHz signal.
pub const METRIC_RATE_HZ: f64 = 8000.0; // 008 R-09 (calibration.md)

/// [`aligned`] resampled to the metric rate (both signals through the same resampler).
fn aligned_metric(rate: u32, x: &[f32]) -> (Vec<f64>, Vec<f64>) {
    let (reference, coded) = aligned(rate, x);
    let fs = f64::from(rate);
    (
        analysis::resample_to(&reference, fs, METRIC_RATE_HZ),
        analysis::resample_to(&coded, fs, METRIC_RATE_HZ),
    )
}

/// SC-001 (FR-003): every recording of the harness's stimuli has a length that is a whole
/// number of frames and a bit rate of exactly 4000 bit/s (frames × 80 ÷ (frames × 120 ⁄ 6000)).
pub fn bit_rate(rate: u32) -> Vec<MeasurementResult> {
    let mut rows = Vec::new();
    for (name, x) in [
        ("voiced_m20", voiced(rate, -20.0)),
        ("unvoiced_voiced_m20", unvoiced_voiced(rate, -20.0)),
        ("tone_1k_m20", crate::golden::stimulus("tone_1k_m20", rate)),
    ] {
        let mut p = Pipeline::new(configs::settings("default_codec", rate)).expect("rate");
        let run = split_checks::record_then_play(&mut p, &x, RECORD_BLOCK, PLAY_BLOCK);
        let StopResult::Kept { length, .. } = run.stop.result else {
            unreachable!()
        };
        let frames = run.stop.flush.frames_total;
        rows.push(row(
            "008/FR-003",
            "recording length modulo 120",
            DESIGN,
            rate,
            "default_codec",
            name,
            (length % 120) as f64,
            "samples",
            Tolerance::Exact(0.0), // FR-003
        ));
        // In integers, from two independent counters: the stop's frame count × 80 bits over
        // the recording's device-sample length ⁄ 6000 (an exact 4000 only when every stored
        // frame covers 120 samples and nothing else is stored).
        let bit_rate = (frames * 80 * 6000).checked_div(length).unwrap_or(0);
        rows.push(row(
            "008/FR-003",
            "bit rate",
            DESIGN,
            rate,
            "default_codec",
            name,
            bit_rate as f64,
            "bit/s",
            Tolerance::Exact(4000.0), // FR-003, S-002
        ));
        rows.push(row(
            "008/FR-003",
            "frames minus length / 120",
            DESIGN,
            rate,
            "default_codec",
            name,
            frames as f64 - (length / 120) as f64,
            "frames",
            Tolerance::Exact(0.0), // FR-003
        ));
    }
    rows
}

/// FR-004 (SC-002): in the codec-only configuration the cross-correlation lag of the codec's
/// output against the same configuration with the codec bypassed is the codec's delay,
/// 180 ± 2 device samples (the resampling boundary is identical on both sides).
pub fn delay_lag(rate: u32) -> Vec<MeasurementResult> {
    let x = voiced(rate, -20.0);
    let (reference, _) = one_pass(codec_only_bypassed(rate), &x, 4096);
    let (coded, _) = one_pass(configs::settings("codec_only", rate), &x, 4096);
    let per_device = f64::from(rate) / 6000.0;
    let max_lag = (300.0 * per_device) as usize;
    let lag = analysis::xcorr_lag(&reference, &coded, max_lag);
    let device = lag as f64 / per_device;
    vec![row(
        "008/FR-004",
        "cross-correlation lag of the codec output against the bypassed reference",
        DESIGN,
        rate,
        "codec_only",
        "voiced −20 dBFS",
        device,
        "device samples",
        Tolerance::Range(180.0 - DELAY_SLACK, 180.0 + DELAY_SLACK), // FR-004
    )]
}

/// FR-007 (US1 scenario 5): frame k's output depends on nothing after sample 120(k+1) + 59.
/// Two codec-only one-pass streams at 6 kHz identical through sample 120(k+1) + j − 1 and
/// differing from there (a full-scale burst to the window's end) give identical output over
/// frame k for every j ≥ 60 (asserted); the largest j < 60 at which the burst still changes
/// frame k is reported (plan review P5: a one-sample change at the window's tail often flips
/// no index).
pub fn frame_causality() -> Vec<MeasurementResult> {
    let rate = 6000;
    let x = voiced(rate, -20.0);
    let settings = configs::settings("codec_only", rate);
    let (base, _) = one_pass(settings, &x, 4096);
    let d = delay_host(rate);
    let mut rows = Vec::new();
    let mut largest_changing: Option<usize> = None;
    for k in [2usize, 9] {
        let frame = speech_codec::FRAME_SAMPLES as usize;
        let frame_out = d + frame * k..d + frame * (k + 1);
        let window_end = frame * (k + 1) + speech_codec::LOOKAHEAD_SAMPLES as usize;
        let mut violations = 0;
        for j in 0..180 {
            let start = 120 * (k + 1) + j;
            let mut y = x.clone();
            for (i, v) in y.iter_mut().enumerate().skip(start) {
                *v = if i % 2 == 0 { 0.95 } else { -0.95 };
                if i >= window_end + 300 {
                    break;
                }
            }
            let (out, _) = one_pass(settings, &y, 4096);
            let same = out[frame_out.clone()]
                .iter()
                .zip(&base[frame_out.clone()])
                .all(|(a, b)| a.to_bits() == b.to_bits());
            if j >= 60 && !same {
                violations += 1;
            }
            if j < 60 && !same {
                largest_changing = Some(largest_changing.map_or(j, |m| m.max(j)));
            }
        }
        rows.push(row(
            "008/FR-007",
            "frame outputs changed by a burst starting 60 or more samples past the frame",
            DESIGN,
            rate,
            "codec_only",
            format!("voiced −20 dBFS, frame {k}, offsets 60 … 179"),
            f64::from(violations),
            "offsets",
            Tolerance::Exact(0.0), // FR-007
        ));
    }
    rows.push(row(
        "008/FR-007",
        "largest offset below 60 at which a burst still changes the frame (report only)",
        DESIGN,
        rate,
        "codec_only",
        "voiced −20 dBFS, frames 2 and 9",
        largest_changing.map_or(-1.0, |j| j as f64),
        "samples",
        Tolerance::Report,
    ));
    rows
}

/// FR-005 (SC-003): the codec's output at the decoder tap against the same configuration with
/// the codec bypassed, at −10, −20 and −40 dBFS: RMS within ±1.5 dB, third-octave levels
/// 315 – 2000 Hz within ±3 dB (2500 Hz reported), the LPC log-spectral distance within the
/// calibrated figure.
pub fn transparency(rate: u32, level: f64) -> Vec<MeasurementResult> {
    let x = voiced(rate, level);
    let (reference_host, coded_host) = aligned(rate, &x);
    let label = format!("voiced {level} dBFS");
    let mut rows = Vec::new();
    rows.push(row(
        "008/FR-005",
        "RMS level of the codec output minus the reference",
        DESIGN,
        rate,
        "codec_only",
        label.clone(),
        analysis::rms_dbfs(&coded_host) - analysis::rms_dbfs(&reference_host),
        "dB",
        Tolerance::Range(-TOL_RMS_DB, TOL_RMS_DB), // FR-005 (calibration.md)
    ));
    // The spectral figures at the metric rate (the calibration's 8 kHz).
    let (reference, coded) = aligned_metric(rate, &x);
    let fs = METRIC_RATE_HZ;
    let bx = analysis::third_octave_levels(&reference, fs);
    let by = analysis::third_octave_levels(&coded, fs);
    for (i, &fc) in analysis::THIRD_OCTAVE_CENTRES_HZ.iter().enumerate() {
        let tolerance = if fc <= 2000.0 {
            Tolerance::Range(-TOL_THIRD_OCTAVE_DB, TOL_THIRD_OCTAVE_DB) // FR-005
        } else {
            Tolerance::Report // the band at the edge of the 225 – 2550 Hz pass-band
        };
        rows.push(row(
            "008/FR-005",
            "third-octave level deviation",
            DESIGN,
            rate,
            "codec_only",
            format!("{label}, {fc} Hz"),
            by[i] - bx[i],
            "dB",
            tolerance,
        ));
    }
    rows.push(row(
        "008/FR-005",
        "LPC log-spectral distance",
        DESIGN,
        rate,
        "codec_only",
        label,
        analysis::lpc_lsd(&reference, &coded, fs),
        "dB",
        Tolerance::AtMost(TOL_LSD_DB), // FR-005 (calibration.md)
    ));
    rows
}

/// FR-006 (SC-004): the segmental SNR of the voiced stimulus at `level` is at least the
/// calibrated figure (the spec's floor is 4 dB); two seconds of digital silence decode below
/// −60 dBFS after the first frame; the level tracking of the unvoiced-to-voiced sequence stays
/// within the calibrated figure (no pumping or dropouts).
pub fn noise_floor(rate: u32, level: f64) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let mut rows = Vec::new();
    let x = voiced(rate, level);
    let (reference, coded) = aligned_metric(rate, &x);
    rows.push(row(
        "008/FR-006",
        "segmental SNR",
        DESIGN,
        rate,
        "codec_only",
        format!("voiced {level} dBFS"),
        analysis::segmental_snr(&reference, &coded, METRIC_RATE_HZ, 0),
        "dB",
        Tolerance::AtLeast(MIN_SEGSNR_DB), // FR-006 (calibration.md; the floor 4 dB)
    ));
    let silence = vec![0.0f32; (2.0 * fs) as usize];
    let (out, _) = one_pass(configs::settings("codec_only", rate), &silence, 4096);
    let skip = delay_host(rate) + (120 * rate / 6000) as usize;
    rows.push(row(
        "008/FR-006",
        "digital silence output level after the first frame",
        DESIGN,
        rate,
        "codec_only",
        format!("2 s of zeros (level {level} dBFS run)"),
        analysis::rms_dbfs(&out[skip..]),
        "dBFS",
        Tolerance::AtMost(SILENCE_FLOOR_DBFS), // FR-006
    ));
    let x = unvoiced_voiced(rate, level);
    let (reference, coded) = aligned(rate, &x);
    rows.push(row(
        "008/FR-006",
        "level tracking: largest 100 ms RMS difference",
        DESIGN,
        rate,
        "codec_only",
        format!("unvoiced-voiced {level} dBFS"),
        analysis::level_track(&reference, &coded, fs, 0),
        "dB",
        Tolerance::AtMost(TOL_LEVEL_TRACK_DB), // FR-006 as amended (calibration.md)
    ));
    rows
}

/// FR-009 (SC-002): record-then-play equals the one-pass. The one-pass is fed the input, then
/// 006's record-latency host silence (the part before the encoder), then
/// (`silence_fed_samples` + the delay less the look-ahead, one frame) device samples' worth of host silence; its output is skipped
/// by the codec's delay in host samples and compared with the playback before the drain,
/// bit for bit, under 006's slack of one device sample's worth (research R-06). In
/// `default_codec` with the VAS bypassed and in `codec_only_vas` with the VAS open at the end
/// (the stimuli end in sound); the record block partitions (64, 1) agree with (4801, 997).
pub fn one_pass_equivalence(rate: u32) -> Vec<MeasurementResult> {
    let mut rows = Vec::new();
    let mut no_vas = configs::settings("default_codec", rate);
    no_vas.vas.enabled = false;
    let configs: [(&'static str, Settings); 2] = [
        ("default_codec (VAS off)", no_vas),
        ("codec_only_vas", configs::settings("codec_only_vas", rate)),
    ];
    let d = delay_host(rate);
    let per_device = split_checks::host_per_device(rate);
    for (name, x) in [
        ("voiced_m20", voiced(rate, -20.0)),
        ("unvoiced_voiced_m20", unvoiced_voiced(rate, -20.0)),
    ] {
        for (config, settings) in configs {
            let mut first: Option<Vec<f32>> = None;
            for (rb, pb) in [(RECORD_BLOCK, PLAY_BLOCK), (64, 1)] {
                let mut p = Pipeline::new(settings).expect("supported settings");
                let run = split_checks::record_then_play(&mut p, &x, rb, pb);
                let StopResult::Kept { length, .. } = run.stop.result else {
                    unreachable!()
                };
                // 006's record-latency silence (the part before the encoder), then the stop's
                // silence plus the 120 device samples that drain the delay line, plus one
                // device sample's worth: 006's slack (at an exact ratio such as 1:8 the
                // conversion yields no rounding surplus of its own).
                let pre_encoder = p.record_latency_samples() as usize - d;
                // FR-009: the delay less the look-ahead (one frame) drains the last frame.
                let drain =
                    (speech_codec::DELAY_SAMPLES - speech_codec::LOOKAHEAD_SAMPLES) as usize;
                let tail =
                    pre_encoder + (run.stop.silence_fed_samples as usize + drain + 1) * per_device;
                let mut y = x.clone();
                y.extend(std::iter::repeat_n(0.0f32, tail));
                let (want, _) = one_pass(settings, &y, 4096);
                let got = &run.pre_drain;
                let n = got.len();
                let long_enough = want.len() >= d + n;
                let same = long_enough
                    && got
                        .iter()
                        .zip(&want[d..d + n])
                        .all(|(a, b)| a.to_bits() == b.to_bits());
                let label = format!("{name}, blocks {rb}/{pb}");
                rows.push(row(
                    "008/FR-009",
                    "record-then-play equals the one-pass skipped by the codec delay",
                    RULES,
                    rate,
                    config,
                    label.clone(),
                    pass(same),
                    "",
                    Tolerance::Exact(1.0), // FR-009
                ));
                rows.push(row(
                    "008/FR-009",
                    "playback length minus ceil(120·F·m/l)",
                    RULES,
                    rate,
                    config,
                    label.clone(),
                    n as f64 - split_checks::playback_len(length, rate) as f64,
                    "samples",
                    Tolerance::Exact(0.0), // FR-009
                ));
                match &first {
                    None => first = Some(got.clone()),
                    Some(f) => rows.push(row(
                        "008/FR-009",
                        "partitions give identical playback",
                        RULES,
                        rate,
                        config,
                        label,
                        pass(
                            f.len() == n
                                && f.iter().zip(got).all(|(a, b)| a.to_bits() == b.to_bits()),
                        ),
                        "",
                        Tolerance::Exact(1.0), // FR-009, FR-016
                    )),
                }
            }
        }
    }
    rows
}

/// FR-014: the one-pass at the after-VAS tap is unchanged by the codec (it sits after the
/// gate), and the VAS events of the same stimulus are identical with the codec on and off.
pub fn after_vas_tap_unchanged(rate: u32) -> Vec<MeasurementResult> {
    let mut on = configs::settings("codec_only_vas", rate);
    on.tap = Tap::AfterVas;
    let mut off = on;
    off.speech_codec_enabled = false;
    let x = crate::golden::stimulus("vas_burst_gap", rate);
    let run = |s: Settings| {
        let mut p = Pipeline::new(s).expect("rate");
        let mut y = vec![0.0f32; x.len()];
        let mut ev = vec![VasEvent::default(); p.max_events(x.len())];
        let info = p.process_with_events(&x, &mut y, &mut ev).expect("lengths");
        y.truncate(info.produced);
        ev.truncate(info.events);
        (y, ev, info.frames)
    };
    let (ya, ea, fa) = run(on);
    let (yb, eb, fb) = run(off);
    vec![
        row(
            "008/FR-014",
            "after-VAS tap output identical with the codec on and off",
            RULES,
            rate,
            "codec_only_vas",
            "vas_burst_gap",
            pass(
                ya.len() == yb.len() && ya.iter().zip(&yb).all(|(a, b)| a.to_bits() == b.to_bits()),
            ),
            "",
            Tolerance::Exact(1.0), // FR-014
        ),
        row(
            "008/FR-014",
            "VAS events identical with the codec on and off",
            RULES,
            rate,
            "codec_only_vas",
            "vas_burst_gap",
            pass(ea == eb && !ea.is_empty()),
            "",
            Tolerance::Exact(1.0), // FR-014
        ),
        row(
            "008/FR-008",
            "frames reported at a tap before the codec",
            RULES,
            rate,
            "codec_only_vas",
            "vas_burst_gap",
            (fa + fb) as f64,
            "frames",
            Tolerance::Exact(0.0), // FR-008
        ),
    ]
}

/// FR-014: the VAS events of a recording are identical with the codec on and off (positions
/// are post-VAS device samples).
pub fn vas_events_unchanged(rate: u32) -> Vec<MeasurementResult> {
    let on = configs::settings("codec_only_vas", rate);
    let mut off = on;
    off.speech_codec_enabled = false;
    let x = crate::golden::stimulus("vas_burst_gap", rate);
    let events = |s: Settings| {
        let mut p = Pipeline::new(s).expect("rate");
        let run = split_checks::record_then_play(&mut p, &x, RECORD_BLOCK, PLAY_BLOCK);
        (run.record_events, run.last_paused)
    };
    let a = events(on);
    let b = events(off);
    vec![row(
        "008/FR-014",
        "recorded VAS events identical with the codec on and off",
        RULES,
        rate,
        "codec_only_vas",
        "vas_burst_gap",
        pass(a == b && !a.0.is_empty()),
        "",
        Tolerance::Exact(1.0), // FR-014
    )]
}

/// SC-002 (FR-004): the one-pass latency with the codec is at most 50 ms plus the bypassed
/// figure at every rate, the reported latencies are the data-model figures, and the sum rule
/// holds.
pub fn latency_bound(rate: u32) -> Vec<MeasurementResult> {
    let on = Pipeline::new(configs::settings("default_codec", rate)).expect("rate");
    let off = Pipeline::new(configs::settings_007(rate)).expect("rate");
    let ms = |samples: u32| f64::from(samples) / f64::from(rate) * 1000.0;
    let expected = [
        (6000u32, 181u32),
        (8000, 321),
        (16_000, 642),
        (44_100, 1767),
        (48_000, 1923),
        (88_200, 3533),
        (96_000, 3846),
    ];
    let table = expected
        .iter()
        .find(|(r, _)| *r == rate)
        .map_or(0, |(_, l)| *l);
    vec![
        row(
            "008/SC-002",
            "one-pass latency with the codec",
            RULES,
            rate,
            "default_codec",
            "reported figures",
            ms(on.latency_samples()),
            "ms",
            Tolerance::AtMost(LATENCY_BOUND_MS + ms(off.latency_samples())), // SC-002
        ),
        row(
            "008/FR-004",
            "one-pass latency minus the data-model figure",
            RULES,
            rate,
            "default_codec",
            "reported figures",
            f64::from(on.latency_samples()) - f64::from(table),
            "samples",
            Tolerance::Exact(0.0), // FR-004 (data-model.md › Per-rate figures)
        ),
        row(
            "008/FR-004",
            "record latency minus (006 figure + codec delay)",
            RULES,
            rate,
            "default_codec",
            "reported figures",
            f64::from(on.record_latency_samples())
                - f64::from(off.record_latency_samples())
                - delay_host(rate) as f64,
            "samples",
            Tolerance::Exact(0.0), // FR-004
        ),
        row(
            "008/FR-004",
            "playback latency with the codec minus without",
            RULES,
            rate,
            "default_codec",
            "reported figures",
            f64::from(on.playback_latency_samples()) - f64::from(off.playback_latency_samples()),
            "samples",
            Tolerance::Exact(0.0), // FR-004: the decoder adds none
        ),
        row(
            "008/FR-004",
            "record + playback latency minus one-pass latency",
            RULES,
            rate,
            "default_codec",
            "reported figures",
            f64::from(on.record_latency_samples()) + f64::from(on.playback_latency_samples())
                - f64::from(on.latency_samples()),
            "samples",
            Tolerance::Range(-1.0, 1.0), // 006 FR-016's sum rule, kept
        ),
    ]
}

/// FR-008: the frames encoded per block sum to the recording's frames, the stop's
/// `frames_total` equals length / 120, and the playback's frames and `frame_index` end at
/// frames − 1.
pub fn frame_reports(rate: u32) -> Vec<MeasurementResult> {
    let x = voiced(rate, -20.0);
    let mut p = Pipeline::new(configs::settings("default_codec", rate)).expect("rate");
    p.start_recording().expect("idle");
    let mut sum = 0usize;
    let mut totals_consistent = true;
    for chunk in x.chunks(RECORD_BLOCK) {
        let info = p.record(chunk, &mut []).expect("recording");
        sum += info.frames;
        totals_consistent &= info.frames_total as usize == sum;
    }
    let stop = p.stop_recording(&mut []).expect("recording");
    sum += stop.flush.frames;
    let StopResult::Kept { number, length } = stop.result else {
        unreachable!()
    };
    p.start_playback(number).expect("idle");
    let mut buf = vec![0.0f32; PLAY_BLOCK];
    let mut played = 0usize;
    let last_index = loop {
        let info = p.play(&mut buf).expect("playing");
        played += info.frames;
        if info.finished {
            break info.frame_index;
        }
    };
    vec![
        row(
            "008/FR-008",
            "frames per block summed minus length / 120",
            RULES,
            rate,
            "default_codec",
            "voiced −20 dBFS",
            sum as f64 - (length / 120) as f64,
            "frames",
            Tolerance::Exact(0.0), // FR-008
        ),
        row(
            "008/FR-008",
            "frames_total consistent in every block and at the stop",
            RULES,
            rate,
            "default_codec",
            "voiced −20 dBFS",
            pass(totals_consistent && stop.flush.frames_total as usize == sum),
            "",
            Tolerance::Exact(1.0), // FR-008
        ),
        row(
            "008/FR-008",
            "frames decoded in playback minus frames recorded",
            RULES,
            rate,
            "default_codec",
            "voiced −20 dBFS",
            played as f64 - sum as f64,
            "frames",
            Tolerance::Exact(0.0), // FR-008
        ),
        row(
            "008/FR-008",
            "frame_index at the end minus (frames − 1)",
            RULES,
            rate,
            "default_codec",
            "voiced −20 dBFS",
            last_index as f64 - (sum as f64 - 1.0),
            "frames",
            Tolerance::Exact(0.0), // FR-008
        ),
    ]
}

/// FR-021 (US1 scenario 4): with the codec bypassed the engine is spec 006's: record-then-play
/// equals the one-pass prefix (006's `live_equivalence`, which runs with the codec off), and
/// every frame read-out reads 0.
pub fn bypassed_equals_006(rate: u32) -> Vec<MeasurementResult> {
    let mut rows = split_checks::live_equivalence(rate);
    let x = voiced(rate, -20.0);
    let mut p = Pipeline::new(configs::settings_007(rate)).expect("rate");
    p.start_recording().expect("idle");
    let info = p.record(&x, &mut []).expect("recording");
    let stop = p.stop_recording(&mut []).expect("recording");
    let StopResult::Kept { number, .. } = stop.result else {
        unreachable!()
    };
    p.start_playback(number).expect("idle");
    let mut buf = vec![0.0f32; 1 << 16];
    let mut play_frames = 0usize;
    let index = loop {
        let pi = p.play(&mut buf).expect("playing");
        play_frames += pi.frames;
        if pi.finished {
            break pi.frame_index;
        }
    };
    let mut y = x.clone();
    let one = Pipeline::new(configs::settings_007(rate))
        .expect("rate")
        .process_in_place(&mut y);
    rows.push(row(
        "008/FR-021",
        "frame read-outs with the codec bypassed",
        RULES,
        rate,
        "default_vas",
        "voiced −20 dBFS",
        (info.frames + stop.flush.frames + play_frames + one.frames) as f64
            + info.frames_total as f64
            + stop.silence_fed_samples as f64
            + index as f64,
        "",
        Tolerance::Exact(0.0), // FR-021
    ));
    rows
}

// ---- User Story 2: a memory of frames and the stored form (008 T026) ----

/// Trace: the frame store, its accounting and the form.
const FORM: &[&str] = &["A-026", "A-004", "A-038", "A-040", "engineering target"];
/// The small capacities of the US2 scenarios (engineering targets, 008 US2; research R-11 of
/// spec 006: never the default 60 minutes in a debug test).
const SMALL_CAPACITY_S: u32 = 10; // engineering target (008 US2)
const SMALL_MAX: u32 = 3; // engineering target (008 US2)
/// US2 scenario 2's capacity: 2 s = 100 frames (engineering target).
const LIMIT_CAPACITY_S: u32 = 2; // engineering target (008 US2 scenario 2)
/// The stimulus of the round trip and the corruption set: 2 s of the voiced stimulus
/// (engineering target, 008 US2 scenario 4).
const FORM_STIMULUS_S: f64 = 2.0; // engineering target (008 US2 scenario 4)
/// US2 scenario 3's input: 1234 device samples' worth (the spec's figure).
const MID_FRAME_DEVICE_SAMPLES: u64 = 1234; // 008 US2 scenario 3
/// Truncation boundary of the corruption set: a 2 s frames form is about 1 kB, so every 128th
/// byte (engineering target, 008 US2 scenario 6).
const TRUNCATION_STEP: usize = 128; // engineering target (008 US2 scenario 6)
/// Single-byte flips in the corruption set (engineering target, 008 US2 scenario 6).
const FLIP_POSITIONS: usize = 64; // engineering target (008 US2 scenario 6)
/// SC-007's stated figure: 180 000 frames × 10 bytes + 100 spans × 16 bytes.
const STATED_FOOTPRINT_BYTES: f64 = 1_801_600.0; // 008 SC-007 (research R-07)
/// SC-007's bound.
const FOOTPRINT_BOUND_BYTES: f64 = 2_500_000.0; // 008 SC-007
/// Spec 006's figure with the codec bypassed: 21 600 000 samples × 4 + 100 × 16.
const BYPASSED_FOOTPRINT_BYTES: f64 = 86_401_600.0; // 006 SC-007 (008 FR-012)

/// The version 2 header's offsets (contracts/recording-form.md › Layout), read here
/// independently of the core.
mod v2 {
    pub const VERSION: usize = 8;
    pub const FLAGS: usize = 10;
    pub const RATE: usize = 12;
    pub const LIB_VERSION: usize = 16;
    pub const ENCODING: usize = 22;
    pub const MODEL_VERSION: usize = 24;
    pub const RESERVED_1: usize = 30;
    pub const FRAME_SAMPLES: usize = 32;
    pub const BITS: usize = 34;
    pub const RESERVED_2: usize = 36;
    pub const COUNT: usize = 40;
    pub const HEADER: usize = 48;
    pub const OVERHEAD: usize = 52;
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().expect("4 bytes"))
}

fn u64_at(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at + 8].try_into().expect("8 bytes"))
}

fn same_bits(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

/// `configs::settings(config, rate)` with the memory at `secs` seconds and `max` recordings.
fn sized(config: &str, rate: u32, secs: u32, max: u32) -> Settings {
    configs::memory_capacity(configs::settings(config, rate), secs, max)
}

/// The 0.8 default with the VAS off (a tone must not be gated), at the small capacity.
fn codec_no_vas(rate: u32, secs: u32, max: u32) -> Settings {
    let mut s = sized("default_codec", rate, secs, max);
    s.vas.enabled = false;
    s
}

/// A −20 dBFS 1 kHz tone of `secs` seconds at `rate`.
fn limit_tone(rate: u32, secs: f64) -> Vec<f32> {
    let fs = f64::from(rate);
    stimulus::tone(1000.0, stimulus::amplitude(-20.0), fs, (secs * fs) as usize)
}

/// Records `x` in one block and stops; the recording's number. Panics on a self-stop or an
/// empty recording.
fn record_one(p: &mut Pipeline, x: &[f32]) -> u32 {
    p.start_recording().expect("idle, room and a slot");
    let info = p.record(x, &mut []).expect("recording");
    assert!(
        info.stopped.is_none(),
        "the memory filled: use a larger capacity"
    );
    match p.stop_recording(&mut []).expect("recording").result {
        StopResult::Kept { number, .. } => number,
        other => panic!("{other:?}"),
    }
}

/// `bytes` with the CRC recomputed (the harness's own CRC, `split_checks::crc32`).
fn reformed(mut bytes: Vec<u8>) -> Vec<u8> {
    let n = bytes.len();
    let c = split_checks::crc32(&bytes[..n - 4]);
    bytes[n - 4..].copy_from_slice(&c.to_le_bytes());
    bytes
}

/// A form **version 1** sequence (spec 006's 32-byte header: magic, version 1, flags, rate,
/// library version, reserved, K; then the samples; then the CRC) carrying the samples of the
/// version 2 samples form `v2_samples`, as 0.6 and 0.7 would have written them.
pub fn version_1_of(v2_samples: &[u8]) -> Vec<u8> {
    assert_eq!(u16_at(v2_samples, v2::ENCODING), 2, "a samples form");
    let k = u64_at(v2_samples, v2::COUNT);
    let mut b = v2_samples[..8].to_vec();
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&0u16.to_le_bytes());
    b.extend_from_slice(&v2_samples[v2::RATE..v2::LIB_VERSION]);
    b.extend_from_slice(&v2_samples[v2::LIB_VERSION..v2::ENCODING]);
    b.extend_from_slice(&0u16.to_le_bytes());
    b.extend_from_slice(&k.to_le_bytes());
    assert_eq!(b.len(), 32);
    b.extend_from_slice(&v2_samples[v2::HEADER..v2_samples.len() - 4]);
    let crc = split_checks::crc32(&b);
    b.extend_from_slice(&crc.to_le_bytes());
    assert_eq!(b.len(), 36 + 4 * k as usize);
    b
}

/// A frame whose subframe-1 lag code is 127 (bits 22 … 28 set; contracts/codec-design.md › Bit
/// order), the one code point the encoder never produces.
pub const LAG_CODE_127_FRAME: [u8; 10] = [0, 0, 0x03, 0xF8, 0, 0, 0, 0, 0, 0];

/// US2 scenario 1 (FR-012, SC-007): the default engine with the codec on reserves at most
/// 2.5 MB, the stated 1 801 600 bytes within ±10 %, equal to the reserved capacities; the
/// capacity still reads 3600 s and 99 recordings; bypassed, the reservation is spec 006's;
/// and at a small capacity the footprint does not change through recording to the limit, an
/// erase, erase all, a playback or a clone.
pub fn footprint(rate: u32) -> Vec<MeasurementResult> {
    let p = Pipeline::new(configs::settings("default_codec", rate)).expect("rate");
    let (store, spans) = __test_hooks::memory_capacities(&p);
    let st = p.memory_status();
    let off = Pipeline::new(configs::settings_007(rate)).expect("rate");
    let mut rows = vec![
        row(
            "008/SC-007",
            "default footprint with the codec on",
            FORM,
            rate,
            "default_codec",
            "default capacity",
            p.footprint_bytes() as f64,
            "bytes",
            Tolerance::AtMost(FOOTPRINT_BOUND_BYTES), // SC-007
        ),
        row(
            "008/SC-007",
            "footprint over the stated 1 801 600 bytes",
            FORM,
            rate,
            "default_codec",
            "default capacity",
            p.footprint_bytes() as f64 / STATED_FOOTPRINT_BYTES,
            "ratio",
            Tolerance::Range(0.9, 1.1), // SC-007: ±10 %
        ),
        row(
            "008/SC-007",
            "footprint equals the reserved capacities: 180 000 frames × 10 + spans × 16",
            FORM,
            rate,
            "default_codec",
            "default capacity",
            pass(p.footprint_bytes() == store * 10 + spans * 16 && store == 180_000),
            "",
            Tolerance::Exact(1.0), // SC-007 (006 plan review P6)
        ),
        row(
            "008/FR-013",
            "capacity still reads 3600 s, 21 600 000 samples and 99 recordings",
            FORM,
            rate,
            "default_codec",
            "default capacity",
            pass(
                st.capacity_seconds() == 3600.0
                    && st.capacity_samples == 21_600_000
                    && st.remaining_samples == 21_600_000
                    && st.max_recordings == 99
                    && st.slots_remaining == 99,
            ),
            "",
            Tolerance::Exact(1.0), // FR-013, US2 scenario 1
        ),
        row(
            "008/FR-012",
            "footprint with the codec bypassed",
            FORM,
            rate,
            "default_vas",
            "default capacity",
            off.footprint_bytes() as f64,
            "bytes",
            Tolerance::Exact(BYPASSED_FOOTPRINT_BYTES), // FR-012: spec 006's reservation
        ),
    ];
    // Invariance at a small capacity (2 s, 3 recordings).
    let mut p = Pipeline::new(codec_no_vas(rate, LIMIT_CAPACITY_S, SMALL_MAX)).expect("rate");
    let before = (p.footprint_bytes(), __test_hooks::memory_capacities(&p));
    let x = limit_tone(rate, f64::from(LIMIT_CAPACITY_S) + 1.0);
    p.start_recording().expect("idle");
    let mut stopped = None;
    for chunk in x.chunks(RECORD_BLOCK) {
        let info = p.record(chunk, &mut []).expect("recording");
        if info.stopped.is_some() {
            stopped = info.stopped;
            break;
        }
    }
    assert_eq!(stopped, Some(StopReason::MemoryFull));
    let mut same = |p: &Pipeline, what: &'static str| {
        rows.push(row(
            "008/SC-007",
            what,
            FORM,
            rate,
            "default_codec",
            "2 s capacity",
            pass((p.footprint_bytes(), __test_hooks::memory_capacities(p)) == before),
            "",
            Tolerance::Exact(1.0), // SC-007
        ));
    };
    same(&p, "footprint unchanged after recording to the limit");
    p.erase(1).expect("idle, exists");
    same(&p, "footprint unchanged after an erase");
    let short = limit_tone(rate, 0.5);
    let _ = record_one(&mut p, &short);
    let _ = record_one(&mut p, &short);
    p.erase_all().expect("idle");
    same(&p, "footprint unchanged after erase all");
    let n = record_one(&mut p, &short);
    let _ = split_checks::play_to_end(&mut p, n, PLAY_BLOCK);
    same(&p, "footprint unchanged after a playback");
    let c = p.clone();
    same(&c, "a clone reserves the same footprint");
    rows
}

/// US2 scenario 2 (FR-013, SC-001): a 3 s tone into a 2 s memory (100 frames) with the codec
/// on stops by itself on the 100th frame with the reason "memory full"; the recording is kept
/// with a whole number of frames, the remaining time reads exactly zero, a start is refused,
/// and the frame read-outs agree.
pub fn capacity_frames(rate: u32) -> Vec<MeasurementResult> {
    let capacity_frames = u64::from(LIMIT_CAPACITY_S) * 50;
    let mut p = Pipeline::new(codec_no_vas(rate, LIMIT_CAPACITY_S, 2)).expect("rate");
    let x = limit_tone(rate, f64::from(LIMIT_CAPACITY_S) + 1.0);
    p.start_recording().expect("idle");
    let mut stopped = None;
    let mut stored = 0u64;
    let mut frames = 0u64;
    let mut frames_total = 0u64;
    for chunk in x.chunks(RECORD_BLOCK) {
        let info = p.record(chunk, &mut []).expect("recording");
        stored += info.stored as u64;
        frames += info.frames as u64;
        frames_total = info.frames_total;
        if info.stopped.is_some() {
            stopped = info.stopped;
            break;
        }
    }
    let st = p.memory_status();
    let length = p.recording_length(1).unwrap_or(0);
    let label = format!("{} s tone into {LIMIT_CAPACITY_S} s", LIMIT_CAPACITY_S + 1);
    let mut rows = vec![
        row(
            "008/FR-013",
            "stop reason is memory full, engine idle, recording kept as number 1",
            FORM,
            rate,
            "default_codec",
            label.clone(),
            pass(
                stopped == Some(StopReason::MemoryFull)
                    && p.state().state == State::Idle
                    && st.count == 1,
            ),
            "",
            Tolerance::Exact(1.0), // FR-013
        ),
        row(
            "008/FR-013",
            "kept length minus the capacity in frames × 120",
            FORM,
            rate,
            "default_codec",
            label.clone(),
            length as f64 - (capacity_frames * 120) as f64,
            "samples",
            Tolerance::Exact(0.0), // FR-013: the last frame boundary that fits
        ),
        row(
            "008/FR-013",
            "kept length modulo the frame length",
            FORM,
            rate,
            "default_codec",
            label.clone(),
            (length % 120) as f64,
            "samples",
            Tolerance::Exact(0.0), // FR-013: whole frames
        ),
        row(
            "008/FR-013",
            "remaining samples after the self-stop",
            FORM,
            rate,
            "default_codec",
            label.clone(),
            st.remaining_samples as f64,
            "samples",
            Tolerance::Exact(0.0), // FR-013: exactly zero
        ),
        row(
            "008/FR-013",
            "remaining seconds after the self-stop",
            FORM,
            rate,
            "default_codec",
            label.clone(),
            st.remaining_seconds(),
            "s",
            Tolerance::Exact(0.0), // FR-013, US2 scenario 2
        ),
        row(
            "008/FR-008",
            "frames per block summed, and frames_total, minus the capacity in frames",
            FORM,
            rate,
            "default_codec",
            label.clone(),
            (frames as f64 - capacity_frames as f64).abs()
                + (frames_total as f64 - capacity_frames as f64).abs(),
            "frames",
            Tolerance::Exact(0.0), // FR-008
        ),
        row(
            "008/FR-013",
            "kept samples at the self-stop minus (capacity + the look-ahead)",
            FORM,
            rate,
            "default_codec",
            label.clone(),
            stored as f64 - (capacity_frames * 120 + 60) as f64,
            "samples",
            Tolerance::Exact(0.0), // FR-013: the frame completes 60 kept samples after its end
        ),
        row(
            "008/FR-013",
            "start refused with memory full; after an erase it proceeds",
            FORM,
            rate,
            "default_codec",
            label.clone(),
            pass({
                let refused = p.start_recording() == Err(Error::MemoryFull);
                p.erase(1).expect("idle, exists");
                let proceeds = p.start_recording().is_ok();
                p.reset();
                refused && proceeds
            }),
            "",
            Tolerance::Exact(1.0), // FR-013 (006 FR-012 with the frame as the unit)
        ),
    ];
    rows.push(row(
        "008/SC-001",
        "stored bits over stored seconds",
        FORM,
        rate,
        "default_codec",
        label,
        (capacity_frames * 80) as f64 / (length as f64 / 6000.0),
        "bit/s",
        Tolerance::Exact(4000.0), // SC-001
    ));
    rows
}

/// US2 scenario 3 (FR-013): a recording stopped by the host after 1234 device samples' worth
/// of input in the codec-only configuration has the smallest whole number of frames that holds
/// every sample the pre-encoder flush delivered (n′), and the stop reports the silence fed:
/// `120·F − n′ + 60`.
pub fn mid_frame_stop(rate: u32) -> Vec<MeasurementResult> {
    let host = (MID_FRAME_DEVICE_SAMPLES * u64::from(rate)).div_ceil(6000) as usize;
    let mut p =
        Pipeline::new(sized("codec_only", rate, SMALL_CAPACITY_S, SMALL_MAX)).expect("rate");
    let x = limit_tone(rate, host as f64 / f64::from(rate));
    let x = &x[..host.min(x.len())];
    p.start_recording().expect("idle");
    let info = p.record(x, &mut []).expect("recording");
    let out = p.stop_recording(&mut []).expect("recording");
    let StopResult::Kept { length, .. } = out.result else {
        panic!("{:?}", out.result)
    };
    let kept_end = info.stored as u64 + out.flush.stored as u64;
    let represented = kept_end - out.silence_fed_samples;
    let frames = length / 120;
    let label = format!("{host} host samples ({MID_FRAME_DEVICE_SAMPLES} device samples' worth)");
    vec![
        row(
            "008/FR-013",
            "kept length modulo the frame length",
            FORM,
            rate,
            "codec_only",
            label.clone(),
            (length % 120) as f64,
            "samples",
            Tolerance::Exact(0.0), // FR-013
        ),
        row(
            "008/FR-013",
            "frames minus ⌈n′ / 120⌉ (n′ = kept samples less the silence fed)",
            FORM,
            rate,
            "codec_only",
            label.clone(),
            frames as f64 - represented.div_ceil(120) as f64,
            "frames",
            Tolerance::Exact(0.0), // FR-013, US2 scenario 3
        ),
        row(
            "008/FR-013",
            "silence fed minus (120·F − n′ + 60)",
            FORM,
            rate,
            "codec_only",
            label.clone(),
            out.silence_fed_samples as f64 - (length as f64 - represented as f64 + 60.0),
            "samples",
            Tolerance::Exact(0.0), // FR-013: padding plus the look-ahead
        ),
        row(
            "008/FR-013",
            "n′ minus the input's device samples (the excess is the conversion's flush)",
            FORM,
            rate,
            "codec_only",
            label.clone(),
            represented as f64 - MID_FRAME_DEVICE_SAMPLES as f64,
            "samples",
            Tolerance::Range(0.0, 119.0), // FR-013: every input sample represented
        ),
        row(
            "008/FR-013",
            "silence fed",
            FORM,
            rate,
            "codec_only",
            label,
            out.silence_fed_samples as f64,
            "samples",
            Tolerance::Range(60.0, 179.0), // FR-013: the look-ahead plus under one frame
        ),
    ]
}

/// The 2 s voiced stimulus at `rate` recorded into a fresh small engine of `config`; the engine
/// and the exported bytes.
fn recorded_form(config: &str, rate: u32) -> (Pipeline, Vec<u8>) {
    let mut p = Pipeline::new(sized(config, rate, SMALL_CAPACITY_S, SMALL_MAX)).expect("rate");
    let x = voiced(rate, -20.0);
    let n = ((FORM_STIMULUS_S * f64::from(rate)) as usize).min(x.len());
    let number = record_one(&mut p, &x[..n]);
    let bytes = split_checks::export_bytes(&p, number);
    (p, bytes)
}

/// US2 scenario 4 (FR-010, SC-009): the recording made at `record_rate` with the codec on
/// exports to the same bytes twice; the bytes declare form version 2, the frames encoding, the
/// codec's model version, 120 samples and 80 bits per frame and K = length / 120 in 52 + 10K
/// bytes; imported into a fresh engine at each of the seven rates it has the same length and
/// its playback is bit-identical to the original engine's reconfigured to that rate; its
/// re-export equals the bytes. The samples form of a bypassed engine declares encoding 2, model
/// version 0 and K = length in 52 + 4K bytes (its playback round trip is spec 006's).
pub fn round_trip(record_rate: u32) -> Vec<MeasurementResult> {
    let (mut original, bytes) = recorded_form("default_codec", record_rate);
    let again = split_checks::export_bytes(&original, 1);
    let length = original.recording_length(1).expect("exists");
    let k = length / 120;
    let label = format!("recorded at {record_rate} Hz, {k} frames");
    let model = speech_codec::MODEL_VERSION;
    let mut rows = vec![
        row(
            "008/FR-010",
            "export is identical every time",
            FORM,
            record_rate,
            "default_codec",
            label.clone(),
            pass(again == bytes),
            "",
            Tolerance::Exact(1.0), // FR-010
        ),
        row(
            "008/FR-010",
            "form version 2, flags 0, device rate 6000",
            FORM,
            record_rate,
            "default_codec",
            label.clone(),
            pass(
                &bytes[..8] == b"RRDR60RC"
                    && u16_at(&bytes, v2::VERSION) == 2
                    && u16_at(&bytes, v2::FLAGS) == 0
                    && u32_at(&bytes, v2::RATE) == 6000,
            ),
            "",
            Tolerance::Exact(1.0), // FR-010, US2 scenario 4
        ),
        row(
            "008/FR-010",
            "encoding 1 (frames), the codec's model version, 120 samples and 80 bits per frame",
            FORM,
            record_rate,
            "default_codec",
            label.clone(),
            pass(
                u16_at(&bytes, v2::ENCODING) == 1
                    && [
                        u16_at(&bytes, v2::MODEL_VERSION),
                        u16_at(&bytes, v2::MODEL_VERSION + 2),
                        u16_at(&bytes, v2::MODEL_VERSION + 4),
                    ] == model
                    && u16_at(&bytes, v2::RESERVED_1) == 0
                    && u16_at(&bytes, v2::FRAME_SAMPLES) == 120
                    && u16_at(&bytes, v2::BITS) == 80
                    && u32_at(&bytes, v2::RESERVED_2) == 0,
            ),
            "",
            Tolerance::Exact(1.0), // FR-010, FR-011
        ),
        row(
            "008/FR-010",
            "K equals length / 120 and the length is 52 + 10K",
            FORM,
            record_rate,
            "default_codec",
            label.clone(),
            pass(
                u64_at(&bytes, v2::COUNT) == k
                    && bytes.len() == v2::OVERHEAD + 10 * k as usize
                    && original.export_len(1) == Ok(bytes.len()),
            ),
            "",
            Tolerance::Exact(1.0), // FR-010
        ),
        row(
            "008/FR-010",
            "the frames are the stored bytes verbatim",
            FORM,
            record_rate,
            "default_codec",
            label.clone(),
            pass(&bytes[v2::HEADER..bytes.len() - 4] == __test_hooks::stored_frames(&original, 1)),
            "",
            Tolerance::Exact(1.0), // FR-010
        ),
    ];
    for play_rate in configs::all_rates() {
        let mut imported = Pipeline::new(sized(
            "default_codec",
            play_rate,
            SMALL_CAPACITY_S,
            SMALL_MAX,
        ))
        .expect("rate");
        let number = imported.import(&bytes);
        rows.push(row(
            "008/FR-010",
            "import accepted into a fresh engine as number 1",
            FORM,
            play_rate,
            "default_codec",
            label.clone(),
            pass(number == Ok(1)),
            "",
            Tolerance::Exact(1.0), // FR-010, US2 scenario 4
        ));
        if number.is_err() {
            continue;
        }
        original
            .reconfigure(sized(
                "default_codec",
                play_rate,
                SMALL_CAPACITY_S,
                SMALL_MAX,
            ))
            .expect("keeps the memory");
        rows.push(row(
            "008/FR-010",
            "imported length minus the original's",
            FORM,
            play_rate,
            "default_codec",
            label.clone(),
            imported.recording_length(1).expect("imported") as f64 - length as f64,
            "samples",
            Tolerance::Exact(0.0), // FR-010
        ));
        let want = split_checks::play_to_end(&mut original, 1, PLAY_BLOCK);
        let got = split_checks::play_to_end(&mut imported, 1, PLAY_BLOCK);
        rows.push(row(
            "008/FR-010",
            "imported playback bit-identical to the original's",
            FORM,
            play_rate,
            "default_codec",
            label.clone(),
            pass(same_bits(&got, &want)),
            "",
            Tolerance::Exact(1.0), // FR-010, SC-009
        ));
        rows.push(row(
            "008/FR-010",
            "re-export of the import equals the bytes",
            FORM,
            play_rate,
            "default_codec",
            label.clone(),
            pass(split_checks::export_bytes(&imported, 1) == bytes),
            "",
            Tolerance::Exact(1.0), // FR-010
        ));
    }
    // The samples encoding's header (FR-012).
    let (bypassed, sb) = recorded_form("default_vas", record_rate);
    let ks = bypassed.recording_length(1).expect("exists");
    rows.push(row(
        "008/FR-012",
        "bypassed: version 2, encoding 2 (samples), model version 0, K = length, 52 + 4K bytes",
        FORM,
        record_rate,
        "default_vas",
        format!("recorded at {record_rate} Hz, {ks} samples"),
        pass(
            u16_at(&sb, v2::VERSION) == 2
                && u16_at(&sb, v2::ENCODING) == 2
                && sb[v2::MODEL_VERSION..v2::RESERVED_1] == [0; 6]
                && u16_at(&sb, v2::FRAME_SAMPLES) == 0
                && u16_at(&sb, v2::BITS) == 0
                && u64_at(&sb, v2::COUNT) == ks
                && sb.len() == v2::OVERHEAD + 4 * ks as usize,
        ),
        "",
        Tolerance::Exact(1.0), // FR-012
    ));
    rows
}

/// US2 scenarios 5 and 6 (FR-010 – FR-012, SC-009): every sequence of the corruption set is
/// refused with the stated reason and leaves the memory unchanged: truncations, single-byte
/// flips, foreign bytes, a version 1 sequence, version 3, a codec model version this release
/// does not decode, an unknown encoding, non-zero reserved fields, other frame constants, a
/// samples form with a model version, a CRC mismatch, a subnormal sample, a frame with lag code
/// 127, K = 0 and the empty sequence; then the other encoding in both directions (checked before
/// the fit), a frame count that does not fit, a missing slot; afterwards the untouched bytes
/// still import.
pub fn corruption_set(rate: u32) -> Vec<MeasurementResult> {
    let (_, good) = recorded_form("default_codec", rate);
    let (_, good_s) = recorded_form("default_vas", rate);
    let n = good.len();
    let model = |b: &[u8]| Error::UnsupportedCodecModelVersion {
        major: u16_at(b, v2::MODEL_VERSION),
        minor: u16_at(b, v2::MODEL_VERSION + 2),
        patch: u16_at(b, v2::MODEL_VERSION + 4),
    };
    let mut cases: Vec<(String, Vec<u8>, Error)> = Vec::new();
    // Truncation at every 128-byte boundary and at the last byte.
    let mut at = TRUNCATION_STEP;
    while at < n {
        let want = if at < 8 {
            Error::UnknownForm
        } else {
            Error::Damaged
        };
        cases.push((
            format!("truncated to {at} bytes"),
            good[..at].to_vec(),
            want,
        ));
        at += TRUNCATION_STEP;
    }
    cases.push((
        "truncated by the last byte".into(),
        good[..n - 1].to_vec(),
        Error::Damaged,
    ));
    cases.push((
        "truncated by one whole frame".into(),
        good[..n - 10].to_vec(),
        Error::Damaged,
    ));
    // A single-byte flip at 64 evenly spaced positions (the first and last bytes included),
    // plus one in every header field and one inside the CRC.
    let flip = |pos: usize| {
        let mut b = good.clone();
        b[pos] ^= 0x40;
        let want = match pos {
            0..8 => Error::UnknownForm,
            8..10 => Error::UnsupportedFormVersion {
                version: u16_at(&b, v2::VERSION),
            },
            10..12 => Error::UnsupportedFormVersion { version: 2 },
            24..30 => model(&b),
            _ => Error::Damaged,
        };
        (format!("byte {pos} flipped"), b, want)
    };
    for i in 0..FLIP_POSITIONS {
        cases.push(flip(i * (n - 1) / (FLIP_POSITIONS - 1)));
    }
    for pos in [
        8,
        10,
        12,
        16,
        22,
        23,
        24,
        26,
        28,
        30,
        32,
        34,
        36,
        40,
        47,
        48,
        57,
        n - 3,
    ] {
        cases.push(flip(pos));
    }
    // Foreign bytes behind a valid magic (seeded, so the expected reason is fixed).
    let mut rng = stimulus::Pcg32::new(0x0D63, 7);
    let mut foreign = good[..8].to_vec();
    foreign.extend((0..1000).map(|_| rng.below(256) as u8));
    let version = u16_at(&foreign, v2::VERSION);
    let flags = u16_at(&foreign, v2::FLAGS);
    let want = if version != 2 || flags != 0 {
        Error::UnsupportedFormVersion { version }
    } else {
        Error::Damaged
    };
    cases.push(("foreign bytes with a valid magic".into(), foreign, want));
    // Version 1 (spec 006's never-shipped samples form), version 3, flags 1.
    cases.push((
        "version 1 (spec 006's samples form)".into(),
        version_1_of(&good_s),
        Error::UnsupportedFormVersion { version: 1 },
    ));
    let mut v3 = good.clone();
    v3[v2::VERSION] = 3;
    cases.push((
        "version 3".into(),
        reformed(v3),
        Error::UnsupportedFormVersion { version: 3 },
    ));
    let mut flags = good.clone();
    flags[v2::FLAGS] = 1;
    cases.push((
        "flags 1".into(),
        reformed(flags),
        Error::UnsupportedFormVersion { version: 2 },
    ));
    // A codec model version this release does not decode, with a valid CRC.
    let mut old_model = good.clone();
    old_model[v2::MODEL_VERSION..v2::RESERVED_1].copy_from_slice(&[0, 0, 7, 0, 0, 0]);
    cases.push((
        "codec model version 0.7.0".into(),
        reformed(old_model),
        Error::UnsupportedCodecModelVersion {
            major: 0,
            minor: 7,
            patch: 0,
        },
    ));
    let mut next_model = good.clone();
    next_model[v2::MODEL_VERSION..v2::RESERVED_1].copy_from_slice(&[1, 0, 0, 0, 0, 0]);
    cases.push((
        "codec model version 1.0.0".into(),
        reformed(next_model),
        Error::UnsupportedCodecModelVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
    ));
    // Damaged headers with a valid CRC.
    let mut enc = good.clone();
    enc[v2::ENCODING] = 3;
    cases.push(("encoding 3".into(), reformed(enc), Error::Damaged));
    let mut r1 = good.clone();
    r1[v2::RESERVED_1] = 1;
    cases.push((
        "reserved field at 30 set".into(),
        reformed(r1),
        Error::Damaged,
    ));
    let mut r2 = good.clone();
    r2[v2::RESERVED_2 + 3] = 1;
    cases.push((
        "reserved field at 36 set".into(),
        reformed(r2),
        Error::Damaged,
    ));
    let mut fs = good.clone();
    fs[v2::FRAME_SAMPLES..v2::BITS].copy_from_slice(&119u16.to_le_bytes());
    cases.push(("frame samples 119".into(), reformed(fs), Error::Damaged));
    let mut bits = good.clone();
    bits[v2::BITS..v2::RESERVED_2].copy_from_slice(&79u16.to_le_bytes());
    cases.push(("bits per frame 79".into(), reformed(bits), Error::Damaged));
    let mut rate8 = good.clone();
    rate8[v2::RATE..v2::LIB_VERSION].copy_from_slice(&8000u32.to_le_bytes());
    cases.push(("device rate 8000".into(), reformed(rate8), Error::Damaged));
    let mut sm = good_s.clone();
    sm[v2::MODEL_VERSION + 2] = 8;
    cases.push((
        "samples form with a model version".into(),
        reformed(sm),
        Error::Damaged,
    ));
    let mut sfs = good_s.clone();
    sfs[v2::FRAME_SAMPLES..v2::BITS].copy_from_slice(&120u16.to_le_bytes());
    cases.push((
        "samples form with frame samples 120".into(),
        reformed(sfs),
        Error::Damaged,
    ));
    cases.push((
        "the 20-byte header".into(),
        good[..20].to_vec(),
        Error::Damaged,
    ));
    cases.push((
        "the 51-byte header".into(),
        good[..51].to_vec(),
        Error::Damaged,
    ));
    let mut huge = good.clone();
    huge[v2::COUNT..v2::HEADER].copy_from_slice(&(1u64 << 62).to_le_bytes());
    cases.push(("K = 2^62".into(), reformed(huge), Error::Damaged));
    let mut k_off = good.clone();
    let k = u64_at(&good, v2::COUNT);
    k_off[v2::COUNT..v2::HEADER].copy_from_slice(&(k - 1).to_le_bytes());
    cases.push(("K one short".into(), reformed(k_off), Error::Damaged));
    let mut zero = good[..v2::HEADER].to_vec();
    zero[v2::COUNT..v2::HEADER].copy_from_slice(&0u64.to_le_bytes());
    zero.extend_from_slice(&[0; 4]);
    cases.push(("K = 0".into(), reformed(zero), Error::Damaged));
    cases.push(("empty sequence".into(), Vec::new(), Error::UnknownForm));
    // The payload: a CRC mismatch, a frame with lag code 127 (first, middle, last), and in the
    // samples form a subnormal and a NaN sample with a valid CRC.
    let mut crc = good.clone();
    crc[n - 2] ^= 0x10;
    cases.push(("CRC bit flipped".into(), crc, Error::Damaged));
    let frames = (n - v2::OVERHEAD) / 10;
    for (name, index) in [("first", 0), ("middle", frames / 2), ("last", frames - 1)] {
        let mut b = good.clone();
        let at = v2::HEADER + 10 * index;
        b[at..at + 10].copy_from_slice(&LAG_CODE_127_FRAME);
        cases.push((
            format!("{name} frame with lag code 127, valid CRC"),
            reformed(b),
            Error::Damaged,
        ));
    }
    let mut sub = good_s.clone();
    sub[v2::HEADER..v2::HEADER + 4].copy_from_slice(&1.0e-40f32.to_le_bytes());
    cases.push((
        "subnormal sample, valid CRC (samples form)".into(),
        reformed(sub),
        Error::Damaged,
    ));
    let mut nan = good_s.clone();
    nan[v2::HEADER..v2::HEADER + 4].copy_from_slice(&f32::NAN.to_le_bytes());
    cases.push((
        "NaN sample, valid CRC (samples form)".into(),
        reformed(nan),
        Error::Damaged,
    ));
    // The other encoding (FR-012): the samples form into the frames engine.
    cases.push((
        "the samples form into a codec engine".into(),
        good_s.clone(),
        Error::EncodingDiffers,
    ));

    let mut rows = Vec::new();
    let mut p =
        Pipeline::new(sized("default_codec", rate, SMALL_CAPACITY_S, SMALL_MAX)).expect("rate");
    let before = p.memory_status();
    for (name, bytes, want) in &cases {
        let got = p.import(bytes);
        let unchanged = p.memory_status() == before && p.state().state == State::Idle;
        rows.push(row(
            "008/FR-010",
            "corrupt sequence refused with the stated reason, memory unchanged",
            FORM,
            rate,
            "default_codec",
            format!("{name} -> {want:?}"),
            pass(got.as_ref() == Err(want) && unchanged),
            "",
            Tolerance::Exact(1.0), // FR-010, SC-009
        ));
    }
    // The frames form into a bypassed engine (FR-012).
    let mut off =
        Pipeline::new(sized("default_vas", rate, SMALL_CAPACITY_S, SMALL_MAX)).expect("rate");
    let before_off = off.memory_status();
    rows.push(row(
        "008/FR-012",
        "the frames form into a bypassed engine: encoding differs, memory unchanged",
        FORM,
        rate,
        "default_vas",
        format!("{n} bytes of frames"),
        pass(off.import(&good) == Err(Error::EncodingDiffers) && off.memory_status() == before_off),
        "",
        Tolerance::Exact(1.0), // FR-012, SC-009
    ));
    // Encoding before fit: the 2 s samples form into a 1 s codec engine.
    let mut tiny = Pipeline::new(sized("default_codec", rate, 1, SMALL_MAX)).expect("rate");
    let before_tiny = tiny.memory_status();
    rows.push(row(
        "008/FR-012",
        "encoding differs is reported before does not fit",
        FORM,
        rate,
        "default_codec",
        format!("{} bytes of samples into 1 s", good_s.len()),
        pass(
            tiny.import(&good_s) == Err(Error::EncodingDiffers)
                && tiny.memory_status() == before_tiny,
        ),
        "",
        Tolerance::Exact(1.0), // FR-012: the contract's order
    ));
    // Oversized for the remaining time (1 s memory = 50 frames, a 2 s recording) and for the
    // slots.
    rows.push(row(
        "008/FR-013",
        "frames that do not fit the remaining time: does not fit, memory unchanged",
        FORM,
        rate,
        "default_codec",
        format!("{frames} frames into 50"),
        pass(tiny.import(&good) == Err(Error::DoesNotFit) && tiny.memory_status() == before_tiny),
        "",
        Tolerance::Exact(1.0), // FR-013, SC-009
    ));
    let mut one_slot =
        Pipeline::new(sized("default_codec", rate, SMALL_CAPACITY_S, 1)).expect("rate");
    assert_eq!(one_slot.import(&good), Ok(1));
    let before_slot = one_slot.memory_status();
    rows.push(row(
        "008/FR-013",
        "no slot left: does not fit, memory unchanged",
        FORM,
        rate,
        "default_codec",
        "second import into one slot",
        pass(
            one_slot.import(&good) == Err(Error::DoesNotFit)
                && one_slot.memory_status() == before_slot,
        ),
        "",
        Tolerance::Exact(1.0), // FR-013, SC-009
    ));
    rows.push(row(
        "008/FR-010",
        "the untouched bytes still import after the refusals",
        FORM,
        rate,
        "default_codec",
        format!("{} refusals", cases.len()),
        pass(p.import(&good) == Ok(1)),
        "",
        Tolerance::Exact(1.0), // FR-010
    ));
    rows
}

/// US2 scenario 7 (FR-014): the VAS events of the speech-like pattern recorded in drop mode
/// are identical with the codec on and off, positions in post-VAS device samples; some fall
/// inside frames (not on a 120-sample boundary) and none past the recording's length.
pub fn vas_splices_inside_frames(rate: u32) -> Vec<MeasurementResult> {
    let on = configs::settings("codec_only_vas", rate);
    let mut off = on;
    off.speech_codec_enabled = false;
    let (speech, _) =
        chain_checks::speech_like_pattern(rate, chain_checks::BURST_DBFS, -60.0, 0x0D64);
    let run = |s: Settings| {
        let mut p = Pipeline::new(s).expect("rate");
        let r = split_checks::record_then_play(&mut p, &speech, RECORD_BLOCK, PLAY_BLOCK);
        let StopResult::Kept { length, .. } = r.stop.result else {
            unreachable!()
        };
        (r.record_events, r.flush_events, length)
    };
    let (a, fa, length) = run(on);
    let (b, fb, _) = run(off);
    let inside = a.iter().filter(|e| e.output_position % 120 != 0).count();
    let past = a.iter().filter(|e| e.output_position > length).count();
    vec![
        row(
            "008/FR-014",
            "recorded VAS events identical with the codec on and off (record and flush)",
            FORM,
            rate,
            "codec_only_vas",
            "speech-like pattern",
            pass(a == b && fa == fb && !a.is_empty()),
            "",
            Tolerance::Exact(1.0), // FR-014, US2 scenario 7
        ),
        row(
            "008/FR-014",
            "events whose position is not on a frame boundary",
            FORM,
            rate,
            "codec_only_vas",
            format!("speech-like pattern, {} events", a.len()),
            inside as f64,
            "events",
            Tolerance::AtLeast(1.0), // FR-014: splices fall inside frames
        ),
        row(
            "008/FR-014",
            "events past the recording's length (the host stopped: none)",
            FORM,
            rate,
            "codec_only_vas",
            "speech-like pattern",
            past as f64,
            "events",
            Tolerance::Exact(0.0), // FR-014
        ),
    ]
}

/// The release-only cross-rate grid (FR-010; spec 006 US1 scenario 5 carried over): a 5 s
/// voiced recording at `record_rate` exported and imported at `play_rate` has identical stored
/// frames and length, and its playback at `play_rate` is bit-identical to the original engine's
/// reconfigured to that rate.
pub fn cross_rate(record_rate: u32, play_rate: u32) -> Vec<MeasurementResult> {
    let mut original = Pipeline::new(sized(
        "default_codec",
        record_rate,
        SMALL_CAPACITY_S,
        SMALL_MAX,
    ))
    .expect("rate");
    let x = stimulus::voiced(f64::from(record_rate), 5.0, -20.0, SEED);
    let n = record_one(&mut original, &x);
    let bytes = split_checks::export_bytes(&original, n);
    let length = original.recording_length(n).expect("exists");
    let mut imported = Pipeline::new(sized(
        "default_codec",
        play_rate,
        SMALL_CAPACITY_S,
        SMALL_MAX,
    ))
    .expect("rate");
    let number = imported.import(&bytes);
    let label = format!(
        "recorded at {record_rate} Hz, played at {play_rate} Hz, {} frames",
        length / 120
    );
    let mut rows = vec![row(
        "008/FR-010",
        "cross-rate import accepted",
        FORM,
        play_rate,
        "default_codec",
        label.clone(),
        pass(number == Ok(1)),
        "",
        Tolerance::Exact(1.0), // FR-010
    )];
    if number.is_err() {
        return rows;
    }
    rows.push(row(
        "008/FR-010",
        "stored frames identical after the cross-rate import",
        FORM,
        play_rate,
        "default_codec",
        label.clone(),
        pass(
            __test_hooks::stored_frames(&imported, 1) == __test_hooks::stored_frames(&original, n)
                && imported.recording_length(1) == Ok(length),
        ),
        "",
        Tolerance::Exact(1.0), // FR-010
    ));
    original
        .reconfigure(sized(
            "default_codec",
            play_rate,
            SMALL_CAPACITY_S,
            SMALL_MAX,
        ))
        .expect("keeps the memory");
    let want = split_checks::play_to_end(&mut original, n, PLAY_BLOCK);
    let got = split_checks::play_to_end(&mut imported, 1, PLAY_BLOCK);
    rows.push(row(
        "008/FR-010",
        "cross-rate playback bit-identical to the original engine's at the play rate",
        FORM,
        play_rate,
        "default_codec",
        label,
        pass(same_bits(&got, &want)),
        "",
        Tolerance::Exact(1.0), // FR-010, SC-009
    ));
    rows
}

/// The release-only 60-minute recording through the codec at 6 kHz (FR-013, SC-007; S-001):
/// a 3601 s tone into the default capacity with the VAS off stops by itself at 180 000 frames
/// (21 600 000 samples), the remaining time is exactly zero and the footprint is unchanged.
pub fn sixty_minutes() -> Vec<MeasurementResult> {
    let rate = 6000;
    let mut s = configs::settings("default_codec", rate);
    s.vas.enabled = false;
    let mut p = Pipeline::new(s).expect("rate");
    let before = (p.footprint_bytes(), __test_hooks::memory_capacities(&p));
    let x = limit_tone(rate, 3601.0);
    p.start_recording().expect("idle");
    let mut stopped = None;
    let mut frames_total = 0;
    for chunk in x.chunks(RECORD_BLOCK) {
        let info = p.record(chunk, &mut []).expect("recording");
        frames_total = info.frames_total;
        if info.stopped.is_some() {
            stopped = info.stopped;
            break;
        }
    }
    let st = p.memory_status();
    vec![
        row(
            "008/FR-013",
            "60-minute recording stops by itself at 180 000 frames (memory full)",
            FORM,
            rate,
            "default_codec",
            "3601 s tone into 3600 s",
            pass(
                stopped == Some(StopReason::MemoryFull)
                    && frames_total == 180_000
                    && p.recording_length(1) == Ok(21_600_000)
                    && st.remaining_samples == 0,
            ),
            "",
            Tolerance::Exact(1.0), // FR-013, SC-001
        ),
        row(
            "008/SC-007",
            "footprint unchanged after the 60-minute recording",
            FORM,
            rate,
            "default_codec",
            "3601 s tone into 3600 s",
            pass((p.footprint_bytes(), __test_hooks::memory_capacities(&p)) == before),
            "",
            Tolerance::Exact(1.0), // SC-007
        ),
    ]
}

/// 008 T016: the check module's own sanity (the delay in host samples is an integer at every
/// rate).
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delay_is_a_whole_number_of_host_samples() {
        for rate in configs::all_rates() {
            assert_eq!(speech_codec::DELAY_SAMPLES * rate % 6000, 0, "{rate} Hz");
        }
        assert_eq!(delay_host(48_000), 1440);
        assert_eq!(delay_host(44_100), 1323);
        assert_eq!(delay_host(6000), 180);
    }
}
