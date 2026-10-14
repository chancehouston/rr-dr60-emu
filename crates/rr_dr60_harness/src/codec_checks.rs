//! Spec 008 checks: the speech codec in the record path, the playback path and the one-pass
//! (US1; FR-002 – FR-009, FR-014 – FR-016; SC-001 – SC-005; research R-09). Every check returns
//! [`MeasurementResult`]s with `008/FR-xxx` and trace IDs; `us1_codec` runs them per rate.
//!
//! Story boundary (Principle VII): these checks use engines and the codec-only configuration
//! only: no form bytes, no registers, no C. The tolerances are the calibrated figures of
//! `specs/008-celp-codec/calibration.md` (the ITU-T G.723.1 reference coder at 5.3 kbit/s
//! through the same metrics, plus a stated margin), never looser than the spec's floors.

use rr_dr60::{Pipeline, Settings, StopResult, Tap, VasEvent, speech_codec};

use crate::checks::{MeasurementResult, Tolerance};
use crate::split_checks::{self, PLAY_BLOCK, RECORD_BLOCK};
use crate::{analysis, configs, stimulus};

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
        let frame_out = d + 120 * k..d + 120 * (k + 1);
        let window_end = 120 * (k + 1) + 60;
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
/// (`silence_fed_samples` + 120) device samples' worth of host silence; its output is skipped
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
                let tail =
                    pre_encoder + (run.stop.silence_fed_samples as usize + 120 + 1) * per_device;
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
