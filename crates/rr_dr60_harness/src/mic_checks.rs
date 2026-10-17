//! Spec 012 microphone-path checks: one function per measurable property of signal-chain
//! stages 1 – 2 (research R-02, R-04, R-05; tasks.md T021, T031).
//!
//! US1's figures are measured in the **stages-only configurations** (`mic_only`,
//! `mic_only_quiet`, `mic_capsule_only`: every later stage bypassed, read at the taps after
//! stage 1 or 2), so no other stage is needed (Principle VII). Every expected value is a figure
//! `tools/mic-chain/design_mic_chain.py` printed on 2026-10-16, copied here; nothing is pinned
//! to the engine's own output (spec 005's lesson).
//!
//! - **The band** (US1 AS1, SC-002): the impulse response at the tap after stage 2 with the
//!   hiss off, read by DTFT at exact third-octave centres (spec 004's lesson), against the
//!   circuit model's three-pole prototype normalized at 1 kHz.
//! - **The hiss** (US1 AS2, AS3, SC-001): ten seconds of silence with the hiss on; the
//!   300 – 3000 Hz level (AES17) and the third-octave shape 315 – 2500 Hz.
//! - **The streams** (US1 AS4, SC-005): the white hiss at the tap after stage 1 for two seeds
//!   and two consecutive recording counters, by normalized cross-correlation.
//! - **Determinism** (US1 AS4): four block partitions give the same bits.

use rr_dr60::{Pipeline, Settings};

use crate::analysis::{self, Complex64};
use crate::checks::{Make, MeasurementResult, Tolerance};
use crate::{configs, stimulus};

/// Trace: the input high-pass (A-046), the output coupling and the roll-off (A-047), lumped
/// ahead of the limiter (A-053).
pub const T_BAND: &[&str] = &["A-046", "A-047", "A-053"];
/// Trace: the hiss level and shape (A-048).
pub const T_HISS: &[&str] = &["A-048"];
/// Trace: the generator and its streams (A-049).
pub const T_GEN: &[&str] = &["A-049"];

/// The circuit model's three corners (input high-pass, output coupling, roll-off) in Hz, the
/// script's three-pole fit to the transfer at GSX, printed 2026-10-16 (A-046, A-047).
pub const CORNERS_HZ: [f64; 3] = [818.4, 76.8, 12939.0];

/// The hiss in the 300 – 3000 Hz band at stage 2's output, dBFS (AES17), printed 2026-10-16
/// (A-048; FR-003).
pub const HISS_BAND_DBFS: f64 = -61.96;

/// The hiss's third-octave levels at [`analysis::THIRD_OCTAVE_CENTRES_HZ`] (315 – 2500 Hz),
/// dBFS, printed 2026-10-16 (A-048; US1 AS2).
pub const HISS_SHAPE_DBFS: [f64; 10] = [
    -85.13, -82.27, -79.77, -77.38, -75.15, -73.30, -71.66, -70.08, -68.80, -67.66,
];

/// Stage 2's group delay at 1 kHz per host rate (`SUPPORTED_HOST_RATES` order), ms, printed
/// 2026-10-16 (FR-008: reported, not added to the latency).
pub const GROUP_DELAY_1K_MS: [f64; 7] = [0.104, 0.099, 0.095, 0.097, 0.097, 0.099, 0.099];

/// The hiss source's RMS per host rate at offset 0 (white at the host rate, the tap after
/// stage 1), printed 2026-10-16 (A-048).
pub const HISS_SIGMA: [f64; 7] = [
    5.498393e-04,
    6.351181e-04,
    8.982816e-04,
    1.490779e-03,
    1.556366e-03,
    2.107825e-03,
    2.198993e-03,
];

/// The white stream's level against σ · 10^(offset/20), dB: the estimate's standard error on
/// 480 000 samples is under 0.01 dB (engineering target).
pub const SIGMA_TOL_DB: f64 = 0.1;

/// The band match, dB (SC-002; US1 AS1).
pub const BAND_TOL_DB: f64 = 0.1;
/// The prototype floor below which a centre is not read, dB (US1 AS1).
pub const BAND_FLOOR_DB: f64 = -40.0;
/// The hiss level's tolerance, dB (SC-001; US1 AS2, AS3).
pub const HISS_TOL_DB: f64 = 0.5;
/// The hiss shape's tolerance, dB (SC-001; US1 AS2).
pub const SHAPE_TOL_DB: f64 = 1.0;
/// The hiss run's length, s (US1 AS2: at least 10 s).
pub const HISS_SECONDS: f64 = 10.0;
/// The streams' length in samples (US1 AS4).
pub const STREAM_LEN: usize = 480_000;
/// The streams' lag range (US1 AS4: every lag within ±64).
pub const MAX_LAG: usize = 64;
/// The bounds on the normalized cross-correlation, × 1/√N, at lag 0 and over the lags (US1 AS4).
pub const XCORR_LAG0: f64 = 5.0;
/// See [`XCORR_LAG0`].
pub const XCORR_LAGS: f64 = 6.0;

fn rate_index(rate: u32) -> usize {
    rr_dr60::SUPPORTED_HOST_RATES
        .iter()
        .position(|&r| r == rate)
        .expect("a supported rate")
}

/// Processes `x` in one block through a pipeline built for `settings`.
fn run(make: Make<'_>, settings: Settings, x: &[f32]) -> Vec<f32> {
    run_on(&mut make(settings), x)
}

/// Processes `x` in one block through `p`.
fn run_on(p: &mut Pipeline, x: &[f32]) -> Vec<f32> {
    let mut y = vec![0.0f32; x.len()];
    let n = p.process(x, &mut y).expect("an idle pipeline").produced;
    y.truncate(n);
    y
}

/// The circuit model's three-pole prototype in dB re its 1 kHz gain (A-046, A-047).
pub fn prototype_db(f: f64) -> f64 {
    let h = |f: f64| {
        let s = Complex64::new(0.0, f);
        let [fa, fb, fc] = CORNERS_HZ;
        (s / fa) / (1.0 + s / fa) * (s / fb) / (1.0 + s / fb) / (1.0 + s / fc)
    };
    analysis::db((h(f) / h(1000.0)).norm())
}

/// The third-octave centres US1 AS1 reads at `rate`: up to the lower of 20 kHz and 0.45 × the
/// rate, wherever the prototype is above −40 dB.
pub fn band_centres(rate: u32) -> Vec<f64> {
    let top = (0.45 * f64::from(rate)).min(20_000.0);
    analysis::THIRD_OCTAVE_CENTRES_20_20K
        .iter()
        .copied()
        .filter(|&f| f <= top && prototype_db(f) > BAND_FLOOR_DB)
        .collect()
}

/// Stage 2's impulse response at `rate` with the hiss off (`mic_only_quiet`), one second.
pub fn impulse_response(make: Make<'_>, rate: u32) -> Vec<f32> {
    let s = configs::settings("mic_only_quiet", rate);
    run(make, s, &stimulus::impulse(rate as usize))
}

/// The frequency where |H| crosses `level` (linear) between `lo` and `hi` Hz, by bisection on
/// the DTFT; `None` when it does not cross.
fn crossing(ir: &[f32], fs: f64, level: f64, lo: f64, hi: f64) -> Option<f64> {
    let g = |f: f64| analysis::dtft(ir, f, fs).norm() - level;
    let (mut a, mut b) = (lo, hi);
    let (ga, gb) = (g(a), g(b));
    if ga.signum() == gb.signum() {
        return None;
    }
    for _ in 0..60 {
        let m = 0.5 * (a + b);
        if g(m).signum() == ga.signum() {
            a = m;
        } else {
            b = m;
        }
    }
    Some(0.5 * (a + b))
}

/// US1 AS1, SC-002: the response after stage 2 against the prototype within ±0.1 dB at every
/// third-octave centre above −40 dB up to min(20 kHz, 0.45·fs); the 1 kHz group delay and the
/// realized −3 dB points (re the passband maximum) are reported (FR-008).
pub fn band(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let ir = impulse_response(make, rate);
    let proto: Vec<(f64, f64)> = band_centres(rate)
        .into_iter()
        .map(|f| (f, prototype_db(f)))
        .collect();
    let mut rows: Vec<MeasurementResult> = analysis::prototype_deviation(&ir, fs, &proto)
        .into_iter()
        .map(|(f, dev)| {
            MeasurementResult::new(
                "SC-002",
                "deviation from the prototype",
                T_BAND,
                rate,
                "mic_only_quiet",
                format!("impulse, {f} Hz"),
                dev,
                "dB",
                Tolerance::Range(-BAND_TOL_DB, BAND_TOL_DB), // SC-002
            )
        })
        .collect();
    let gd = analysis::group_delay_samples(|f| analysis::dtft(&ir, f, fs), 1000.0, fs) / fs;
    rows.push(MeasurementResult::new(
        "FR-008",
        "group delay at 1 kHz",
        T_BAND,
        rate,
        "mic_only_quiet",
        format!(
            "impulse (the script: {} ms)",
            GROUP_DELAY_1K_MS[rate_index(rate)]
        ),
        gd * 1e3,
        "ms",
        Tolerance::Report,
    ));
    // The realized −3 dB points re the passband maximum (reported, US1 AS1).
    let top = 0.499 * fs;
    let peak = (0..400)
        .map(|k| 20.0 * analysis::from_db(analysis::db(top / 20.0) * f64::from(k) / 399.0))
        .map(|f| analysis::dtft(&ir, f, fs).norm())
        .fold(0.0, f64::max);
    let half = peak * std::f64::consts::FRAC_1_SQRT_2;
    if let Some(f) = crossing(&ir, fs, half, 20.0, 1000.0) {
        rows.push(MeasurementResult::new(
            "US1 AS1",
            "lower -3 dB point re the passband maximum",
            T_BAND,
            rate,
            "mic_only_quiet",
            "impulse",
            f,
            "Hz",
            Tolerance::Report,
        ));
    }
    if let Some(f) = crossing(&ir, fs, half, 3000.0_f64.min(0.4 * fs), top) {
        rows.push(MeasurementResult::new(
            "US1 AS1",
            "upper -3 dB point re the passband maximum",
            T_BAND,
            rate,
            "mic_only_quiet",
            "impulse",
            f,
            "Hz",
            Tolerance::Report,
        ));
    }
    rows
}

/// The `mic_only` configuration at `rate` with the hiss offset `offset_db`.
pub fn mic_only(rate: u32, offset_db: f32) -> Settings {
    let mut s = configs::settings("mic_only", rate);
    s.mic.hiss_offset_db = offset_db;
    s
}

/// US1 AS2, AS3, SC-001: ten seconds of silence through stages 1 – 2 at `offset_db`; the
/// 300 – 3000 Hz level within ±0.5 dB of the script's figure + the offset, and the third-octave
/// levels 315 – 2500 Hz each within ±1 dB of the script's shape + the offset.
pub fn hiss_level(make: Make<'_>, rate: u32, offset_db: f32) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let y = run(
        make,
        mic_only(rate, offset_db),
        &stimulus::silence((HISS_SECONDS * fs) as usize),
    );
    let off = f64::from(offset_db);
    let mut rows = vec![MeasurementResult::new(
        "SC-001",
        "hiss level, 300-3000 Hz (AES17) re the figure + offset",
        T_HISS,
        rate,
        "mic_only",
        format!("silence {HISS_SECONDS} s, offset {offset_db} dB"),
        analysis::band_level_aes17(&y, fs, 300.0, 3000.0) - (HISS_BAND_DBFS + off),
        "dB",
        Tolerance::Range(-HISS_TOL_DB, HISS_TOL_DB), // SC-001
    )];
    let levels = analysis::third_octave_dbfs(&y, fs, &analysis::THIRD_OCTAVE_CENTRES_HZ);
    for ((&fc, &got), &want) in analysis::THIRD_OCTAVE_CENTRES_HZ
        .iter()
        .zip(&levels)
        .zip(&HISS_SHAPE_DBFS)
    {
        rows.push(MeasurementResult::new(
            "SC-001",
            "hiss third-octave level re the shape + offset",
            T_HISS,
            rate,
            "mic_only",
            format!("silence {HISS_SECONDS} s, offset {offset_db} dB, {fc} Hz"),
            got - (want + off),
            "dB",
            Tolerance::Range(-SHAPE_TOL_DB, SHAPE_TOL_DB), // SC-001
        ));
    }
    rows
}

/// The white hiss after stage 1 (`mic_capsule_only`) for `seed`, keyed to recording counter
/// `counter`: `counter` short recordings are made first, then `reset` keys the one-pass to the
/// counter's current value (FR-009), and [`STREAM_LEN`] samples of silence are processed.
pub fn capsule_stream(make: Make<'_>, rate: u32, seed: u64, counter: u64) -> Vec<f32> {
    let mut s = configs::memory_capacity(configs::settings("mic_capsule_only", rate), 2, 9);
    s.seed = seed;
    let mut p = make(s);
    let block = stimulus::silence(64);
    for _ in 0..counter {
        p.start_recording().expect("room for a recording");
        let _ = p.record(&block, &mut []).expect("recording");
        let _ = p.stop_recording(&mut []).expect("recording");
    }
    p.reset();
    run_on(&mut p, &stimulus::silence(STREAM_LEN))
}

/// US1 AS4, SC-005: two streams' normalized cross-correlation below 5/√N at lag 0 and 6/√N
/// over the lags ±64, for seeds 0 and 1 (counter 0) and for counters 0 and 1 (seed 0).
pub fn streams(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let a = capsule_stream(make, rate, 0, 0);
    let pairs = [
        ("seeds 0 / 1", capsule_stream(make, rate, 1, 0)),
        ("counters 0 / 1", capsule_stream(make, rate, 0, 1)),
    ];
    let root_n = rr_dr60_detmath::sqrt(STREAM_LEN as f64);
    // The stream is the white hiss at σ · 10^(offset/20) (`mic_capsule_only` is at −12 dB).
    let offset = f64::from(
        configs::settings("mic_capsule_only", rate)
            .mic
            .hiss_offset_db,
    );
    // AES17 RMS dBFS less 20·log10(√2): the plain RMS in dB re 1.
    let rms_db = analysis::rms_dbfs(&a) - analysis::db(std::f64::consts::SQRT_2);
    let want = HISS_SIGMA[rate_index(rate)] * analysis::from_db(offset);
    let mut rows = vec![MeasurementResult::new(
        "FR-003",
        "white hiss RMS after stage 1 re sigma x offset",
        T_HISS,
        rate,
        "mic_capsule_only",
        format!("silence, seed 0, counter 0, offset {offset} dB"),
        rms_db - analysis::db(want),
        "dB",
        Tolerance::Range(-SIGMA_TOL_DB, SIGMA_TOL_DB), // engineering target (A-048)
    )];
    for (name, b) in pairs {
        let (lag0, worst) = analysis::norm_xcorr_max(&a, &b, MAX_LAG);
        rows.push(MeasurementResult::new(
            "SC-005",
            "normalized cross-correlation at lag 0, x sqrt(N)",
            T_GEN,
            rate,
            "mic_capsule_only",
            format!("{name}, {STREAM_LEN} samples"),
            lag0 * root_n,
            "",
            Tolerance::AtMost(XCORR_LAG0), // US1 AS4
        ));
        rows.push(MeasurementResult::new(
            "SC-005",
            "normalized cross-correlation over lags +-64, x sqrt(N)",
            T_GEN,
            rate,
            "mic_capsule_only",
            format!("{name}, {STREAM_LEN} samples"),
            worst * root_n,
            "",
            Tolerance::AtMost(XCORR_LAGS), // US1 AS4
        ));
    }
    // The same key twice gives the same stream (the determinism the rows above rely on).
    let again = capsule_stream(make, rate, 0, 0);
    rows.push(MeasurementResult::new(
        "SC-005",
        "samples differing between two runs of one key",
        T_GEN,
        rate,
        "mic_capsule_only",
        "seed 0, counter 0",
        differing(&a, &again) as f64,
        "samples",
        Tolerance::Exact(0.0), // SC-005: bit-identical
    ));
    rows
}

/// The number of samples whose bits differ (a length difference counts every missing sample).
pub fn differing(a: &[f32], b: &[f32]) -> usize {
    let common = a
        .iter()
        .zip(b)
        .filter(|(x, y)| x.to_bits() != y.to_bits())
        .count();
    common + a.len().abs_diff(b.len())
}

/// Processes `x` through a pipeline built for `settings` in blocks of `sizes` (cycled).
pub fn run_partitioned(make: Make<'_>, settings: Settings, x: &[f32], sizes: &[usize]) -> Vec<f32> {
    let mut p = make(settings);
    let mut y = Vec::with_capacity(x.len());
    let (mut pos, mut k) = (0usize, 0usize);
    while pos < x.len() {
        let n = sizes[k % sizes.len()].min(x.len() - pos);
        let mut out = vec![0.0f32; n];
        let produced = p.process(&x[pos..pos + n], &mut out).expect("ok").produced;
        y.extend_from_slice(&out[..produced]);
        pos += n;
        k += 1;
    }
    y
}

/// US1 AS4 (SC-005): the same seed, counter and input in four block partitions (1, 64, 4096
/// and an irregular one) are bit-identical to one block, with the hiss on (`mic_only`).
pub fn determinism(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let mut x = stimulus::voiced(fs, 0.5, -20.0, 7);
    x.extend(stimulus::silence(rate as usize / 2));
    let s = mic_only(rate, 0.0);
    let whole = run(make, s, &x);
    let partitions: [(&str, &[usize]); 4] = [
        ("blocks of 1", &[1]),
        ("blocks of 64", &[64]),
        ("blocks of 4096", &[4096]),
        ("irregular blocks", &[1, 7, 300, 2, 64, 1023, 5, 4096, 13]),
    ];
    partitions
        .iter()
        .map(|&(name, sizes)| {
            let y = run_partitioned(make, s, &x, sizes);
            MeasurementResult::new(
                "SC-005",
                "samples differing from one block",
                T_GEN,
                rate,
                "mic_only",
                format!("voiced 0.5 s + silence 0.5 s, {name}"),
                differing(&whole, &y) as f64,
                "samples",
                Tolerance::Exact(0.0), // SC-005: bit-identical
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// US3 AS1 – AS3 at the P1 checkpoint (tasks.md T031): the hiss is on by default from here.
// ---------------------------------------------------------------------------------------------

/// Trace: the streams and the record path's rules (spec 012 FR-009; spec 006 R-01; spec 008
/// FR-009).
pub const T_RULES: &[&str] = &["A-049", "engineering target"];

/// 1.0 for true, 0.0 for false (the rows' pass flags).
fn pass(ok: bool) -> f64 {
    if ok { 1.0 } else { 0.0 }
}

/// Processes `x` through `p` in blocks of `block`.
fn run_blocks(p: &mut Pipeline, x: &[f32], block: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(x.len());
    for chunk in x.chunks(block.max(1)) {
        let mut y = vec![0.0f32; chunk.len()];
        let n = p.process(chunk, &mut y).expect("an idle pipeline").produced;
        out.extend_from_slice(&y[..n]);
    }
    out
}

/// Some history for the reset and reconfigure cases: two recordings and some one-pass
/// processing (the counter ends at 2).
fn history(p: &mut Pipeline, rate: u32) {
    let fs = f64::from(rate);
    let noise = stimulus::voiced(fs, 0.1, -30.0, 3);
    for _ in 0..2 {
        p.start_recording().expect("room");
        let _ = p.record(&noise, &mut []).expect("recording");
        let _ = p.stop_recording(&mut []).expect("recording");
        let _ = run_blocks(p, &noise, 512);
    }
}

/// US3 AS1, SC-008 (FR-009): the first recording after construction, after `reset` and after
/// `reconfigure` (a new seed) equals the one-pass bit for bit under spec 008's coordinates: the
/// one-pass is fed the input, the record-latency silence before the encoder, then
/// (`silence_fed_samples` + one frame + 1) device samples' worth of zeros, and its output is
/// skipped by the codec's delay (`default_mic`: the 0.12 default with the voice activation
/// bypassed). The stop feeds zeros through stages 1 – 2, so the recording's tail carries the
/// hiss, and so does the one-pass's. The one-pass is run after the recording on the same engine:
/// a recording neither advances nor rekeys the one-pass stream (research R-05).
pub fn equivalence(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    use rr_dr60::speech_codec;
    let fs = f64::from(rate);
    let x = stimulus::voiced(fs, 0.5, -20.0, 7);
    let settings = configs::memory_capacity(configs::settings("default_mic", rate), 60, 9);
    let d = crate::codec_checks::delay_host(rate);
    let per_device = crate::split_checks::host_per_device(rate);
    let drain = (speech_codec::DELAY_SAMPLES - speech_codec::LOOKAHEAD_SAMPLES) as usize;
    let mut rows = Vec::new();
    for case in ["after construction", "after reset", "after reconfigure"] {
        let mut p = make(settings);
        match case {
            "after reset" => {
                history(&mut p, rate);
                p.reset();
            }
            "after reconfigure" => {
                history(&mut p, rate);
                let mut t = settings;
                t.seed = 9;
                p.reconfigure(t).expect("an idle pipeline");
            }
            _ => {}
        }
        let run = crate::split_checks::record_then_play(
            &mut p,
            &x,
            crate::split_checks::RECORD_BLOCK,
            crate::split_checks::PLAY_BLOCK,
        );
        let pre_encoder = p.record_latency_samples() as usize - d;
        let tail = pre_encoder + (run.stop.silence_fed_samples as usize + drain + 1) * per_device;
        let mut y = x.clone();
        y.extend(std::iter::repeat_n(0.0f32, tail));
        let want = run_blocks(&mut p, &y, 4096);
        let got = &run.pre_drain;
        let n = got.len();
        let same = want.len() >= d + n
            && got
                .iter()
                .zip(&want[d..d + n])
                .all(|(a, b)| a.to_bits() == b.to_bits());
        rows.push(MeasurementResult::new(
            "SC-008",
            "first recording equals the one-pass skipped by the codec delay",
            T_RULES,
            rate,
            "default_mic",
            format!("voiced 0.5 s -20 dBFS, {case}"),
            pass(same),
            "",
            Tolerance::Exact(1.0), // SC-008
        ));
        // The stop's flush carries the hiss: the last frame, completed from the stop's zeros,
        // differs from the same recording with the hiss off (`default_mic_quiet`).
        let mut quiet = settings;
        quiet.mic.capsule_enabled = false;
        let q = crate::split_checks::record_then_play(
            &mut make(quiet),
            &x,
            crate::split_checks::RECORD_BLOCK,
            crate::split_checks::PLAY_BLOCK,
        )
        .pre_drain;
        let last = n.saturating_sub(120 * per_device);
        rows.push(MeasurementResult::new(
            "FR-009",
            "last frame: samples differing from the hiss-off recording",
            T_RULES,
            rate,
            "default_mic",
            format!("voiced 0.5 s -20 dBFS, {case}"),
            differing(&got[last..], &q[last.min(q.len())..]) as f64,
            "samples",
            Tolerance::AtLeast(1.0), // FR-009: the stop feeds zeros, the hiss continues
        ));
    }
    rows
}

/// US3 AS2 (FR-009): after `reset` the sections are at rest and the hiss restarts at the
/// current counter's stream; two engines with the same seed and call history carry identical
/// hiss in their k-th recordings, and two recordings of one engine differ.
pub fn rest_and_rekey(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let x = stimulus::voiced(fs, 0.2, -20.0, 7);
    let loud = stimulus::voiced(fs, 0.1, 0.0, 5);
    let small = |name: &str| configs::memory_capacity(configs::settings(name, rate), 10, 9);
    let fresh = |name: &str| run_blocks(&mut make(small(name)), &x, 512);
    let mut rows = Vec::new();
    let mut row = |property: &'static str, config: &'static str, value: f64, tol: Tolerance| {
        rows.push(MeasurementResult::new(
            "FR-009",
            property,
            T_RULES,
            rate,
            config,
            "voiced 0.2 s",
            value,
            "",
            tol,
        ));
    };
    for config in ["mic_only", "mic_only_quiet"] {
        let mut p = make(small(config));
        let _ = run_blocks(&mut p, &loud, 333);
        p.reset();
        let y = run_blocks(&mut p, &x, 512);
        row(
            "after reset (counter 0) equals a fresh engine",
            config,
            pass(y == fresh(config)),
            Tolerance::Exact(1.0), // FR-009: rest and the stream's beginning
        );
    }
    // After a recording the one-pass restarts at counter 1's stream: not counter 0's, and the
    // same as another engine's with the same history.
    let after_one = || {
        let mut p = make(small("mic_only"));
        let _ = run_blocks(&mut p, &loud, 333);
        p.start_recording().expect("room");
        let _ = p.record(&loud, &mut []).expect("recording");
        let _ = p.stop_recording(&mut []).expect("recording");
        p.reset();
        run_blocks(&mut p, &x, 512)
    };
    let y1 = after_one();
    row(
        "after a recording and reset, differing samples from counter 0's",
        "mic_only",
        differing(&y1, &fresh("mic_only")) as f64,
        Tolerance::AtLeast(1.0), // FR-009: stream 1 is not stream 0
    );
    row(
        "two engines with one history, differing samples",
        "mic_only",
        differing(&y1, &after_one()) as f64,
        Tolerance::Exact(0.0), // SC-008
    );
    // SC-008: the k-th recordings of two engines with one history are identical; the two
    // recordings of one engine differ (silence in, the hiss out).
    let silence = stimulus::silence(rate as usize / 5);
    let recordings = || {
        let mut p = make(small("default_mic"));
        let a = crate::split_checks::record_then_play(&mut p, &silence, 4801, 997).pre_drain;
        let b = crate::split_checks::record_then_play(&mut p, &silence, 4801, 997).pre_drain;
        (a, b)
    };
    let (a, b) = recordings();
    let (c, e) = recordings();
    row(
        "recordings 1 and 2 of one engine, differing samples",
        "default_mic",
        differing(&a, &b) as f64,
        Tolerance::AtLeast(1.0), // SC-008: no two recordings share a stream
    );
    row(
        "the k-th recordings of two engines, differing samples",
        "default_mic",
        (differing(&a, &c) + differing(&b, &e)) as f64,
        Tolerance::Exact(0.0), // SC-008
    );
    rows
}

/// US3 AS3 (FR-008): the tap after stage 1 is the input plus the white hiss and the tap after
/// stage 2 the shaped sum, both with latency 0, each read against the superposition of its
/// input-only and hiss-only runs within one `f32` rounding; every other tap's latency is
/// unchanged by stages 1 – 2.
pub fn taps(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let x = stimulus::voiced(fs, 0.2, -20.0, 7);
    let zeros = stimulus::silence(x.len());
    let mut rows = Vec::new();
    let superposition = |sum: &[f32], a: &[f32], b: &[f32]| {
        let peak = sum.iter().fold(0.0f64, |m, &v| m.max(f64::from(v).abs()));
        let worst = sum
            .iter()
            .zip(a.iter().zip(b))
            .map(|(&s, (&a, &b))| (f64::from(s) - f64::from(a) - f64::from(b)).abs())
            .fold(0.0, f64::max);
        worst / peak
    };
    // After stage 1: y − x is the stream itself (`mic_capsule_only`).
    let cap = configs::settings("mic_capsule_only", rate);
    let y = run(make, cap, &x);
    let h = run(make, cap, &zeros);
    rows.push(MeasurementResult::new(
        "FR-008",
        "after stage 1: |y - x - hiss| re the peak",
        T_HISS,
        rate,
        "mic_capsule_only",
        "voiced 0.2 s -20 dBFS",
        superposition(&y, &x, &h),
        "",
        Tolerance::AtMost(f64::from(f32::EPSILON)), // engineering target: one f32 rounding
    ));
    // After stage 2: the amplifier is linear below its clip onsets, so the shaped sum is the
    // input's run with the hiss off plus the hiss's run.
    let on = run(make, mic_only(rate, 0.0), &x);
    let quiet = run(make, configs::settings("mic_only_quiet", rate), &x);
    let hiss = run(make, mic_only(rate, 0.0), &zeros);
    rows.push(MeasurementResult::new(
        "FR-008",
        "after stage 2: |y - input's run - hiss's run| re the peak",
        T_BAND,
        rate,
        "mic_only",
        "voiced 0.2 s -20 dBFS",
        superposition(&on, &quiet, &hiss),
        "",
        Tolerance::AtMost(2.0 * f64::from(f32::EPSILON)), // engineering target: two roundings
    ));
    for config in ["mic_capsule_only", "mic_only"] {
        rows.push(MeasurementResult::new(
            "FR-008",
            "latency",
            T_RULES,
            rate,
            config,
            "",
            f64::from(make(configs::settings(config, rate)).latency_samples()),
            "samples",
            Tolerance::Exact(0.0), // FR-008
        ));
    }
    // Every earlier tap: the latencies with stages 1 – 2 on equal 0.11's.
    use rr_dr60::Tap;
    for tap in [
        Tap::AfterAgc,
        Tap::AfterRecord,
        Tap::AfterVas,
        Tap::AfterPlayback,
        Tap::AfterDecoder,
        Tap::RouteOutput,
        Tap::AfterVolumeAmp,
        Tap::AfterSpeaker,
        Tap::AfterEarphone,
    ] {
        let mut on = Settings::new(rate);
        on.tap = tap;
        let mut off = configs::settings_011(rate);
        off.tap = tap;
        let lat = |s: Settings| {
            let p = make(configs::memory_capacity(s, 10, 9));
            [
                p.latency_samples(),
                p.record_latency_samples(),
                p.playback_latency_samples(),
            ]
        };
        rows.push(MeasurementResult::new(
            "FR-008",
            "latencies (one-pass, record, playback) equal 0.11's",
            T_RULES,
            rate,
            "default",
            format!("{tap:?}"),
            pass(lat(on) == lat(off)),
            "",
            Tolerance::Exact(1.0), // FR-008: no earlier tap's latency changes
        ));
    }
    rows
}

// ---------------------------------------------------------------------------------------------
// US2: the clipper (tasks.md T034; spec 012 FR-006, SC-003; research R-02, R-03).
// ---------------------------------------------------------------------------------------------

/// Trace: the clip limits, the knee and the first-clipping polarity (A-051, A-036).
pub const T_CLIP: &[&str] = &["A-051", "A-036"];

/// L₋ and L₊ at the engine input at 1 kHz, dBFS: negative samples are limited first (A-051),
/// printed 2026-10-16.
pub const LIMIT_DBFS: [f64; 2] = [7.15, 16.37];
/// Each polarity's onset (0.1 dB compression) re its limit, dB (A-051; k = 0.85), printed
/// 2026-10-16.
pub const ONSET_RE_LIMIT_DB: f64 = -0.631;
/// The knee start, a fraction of the limit (A-051; spec 009's knee), printed 2026-10-16.
pub const KNEE_K: f64 = 0.85;

/// The limits in clipper units per host rate (`SUPPORTED_HOST_RATES` order) as `[L₋, L₊]`,
/// written by the script into `stages/mic_coeffs.rs` on 2026-10-16 (A-051).
pub const CLIP_LIMITS: [[f64; 2]; 7] = [
    [2.28696607, 6.60679088],
    [2.29306065, 6.62439744],
    [2.29404494, 6.62724093],
    [2.2916198, 6.62023498],
    [2.29132174, 6.61937393],
    [2.29199517, 6.62131938],
    [2.29201107, 6.62136531],
];

/// The 1 kHz cosine start phase that puts a host sample on the clipper-input peak, radians
/// (it compensates the input high-pass's phase), written 2026-10-16 (research R-03).
pub const START_PHASE: [f64; 7] = [
    -0.610640758,
    -0.645518044,
    -0.680579038,
    -0.682302425,
    -0.679252872,
    -0.685236908,
    -0.685431091,
];

/// H2 and H3 re the fundamental for a 1 kHz tone 6 dB above the first onset after stage 2, the
/// script's discrete model per rate, dB, printed 2026-10-16 (US2 AS3).
pub const HARMONICS_DB: [[f64; 2]; 7] = [
    [-14.91, -15.02],
    [-17.16, -23.54],
    [-16.41, -22.33],
    [-16.34, -21.91],
    [-16.34, -21.89],
    [-16.35, -21.92],
    [-16.35, -21.92],
];
/// The near-continuous model's H2 and H3, dB (the aliases are reported against it).
pub const HARMONICS_CONTINUOUS_DB: [f64; 2] = [-16.35, -21.92];

/// The time to return within ±0.1 dB of the linear response after +13 dBFS for 50 ms then
/// −20 dBFS, ms, per rate, printed 2026-10-16 (US2 AS4).
pub const RECOVERY_MS: [f64; 7] = [12.17, 12.62, 12.62, 12.43, 12.33, 12.46, 12.47];

/// The onsets' and limits' tolerance, dB (SC-003).
pub const CLIP_TOL_DB: f64 = 0.5;
/// The harmonics' tolerance against the discrete model, dB (US2 AS3).
pub const HARMONIC_TOL_DB: f64 = 0.5;
/// The printed limits carry 8 – 9 significant digits (engineering target).
pub const LIMIT_PRINT_TOL: f64 = 1e-7;

/// The clipper's output bound after stage 2 at `rate`: L₊ + L₋ in clipper units (the 1 − LP′
/// identity of research R-02), plus the print's rounding, rounded up to `f32`.
pub fn stage2_bound(rate: u32) -> f64 {
    let [ln, lp] = CLIP_LIMITS[rate_index(rate)];
    let b = ln + lp + 2.0 * LIMIT_PRINT_TOL;
    let f = b as f32;
    let up = if f64::from(f) < b {
        f32::from_bits(f.to_bits() + 1)
    } else {
        f
    };
    f64::from(up)
}

/// A 1 kHz tone at `level_dbfs` whose host sample 0 lands on the clipper-input peak (the
/// script's cosine at [`START_PHASE`]).
pub fn peak_tone(rate: u32, level_dbfs: f64, len: usize) -> Vec<f32> {
    let phase = START_PHASE[rate_index(rate)] + std::f64::consts::FRAC_PI_2;
    stimulus::tone_at_phase(1000.0, level_dbfs, phase, f64::from(rate), len)
}

/// The engine's clipper trace for `x` through `settings` (`__mic_clip_trace`).
pub fn clip_trace(make: Make<'_>, settings: Settings, x: &[f32]) -> Vec<(f64, f64)> {
    let mut p = make(settings);
    let mut t = vec![(0.0, 0.0); x.len()];
    let n = p.__mic_clip_trace(x, &mut t);
    assert_eq!(n, x.len(), "stage 2 is on");
    t
}

/// Each polarity's compression (output peak re input peak, dB) of a 1 kHz tone at
/// `level_dbfs` at the engine input, read through the clipper trace over the last 20 ms of
/// 40 ms, and the engine-input limit estimate `out peak × A / in peak` in dBFS: `[pos, neg]`
/// pairs `(compression, limit)`.
fn compression(make: Make<'_>, settings: Settings, rate: u32, level_dbfs: f64) -> [(f64, f64); 2] {
    let n = rate as usize / 25;
    let t = clip_trace(make, settings, &peak_tone(rate, level_dbfs, n));
    let tail = &t[n / 2..];
    let a = analysis::from_db(level_dbfs);
    let pk = |sel: fn(f64) -> f64| {
        let ci = tail.iter().map(|&(i, _)| sel(i)).fold(f64::MIN, f64::max);
        let co = tail.iter().map(|&(_, o)| sel(o)).fold(f64::MIN, f64::max);
        (analysis::db(co / ci), analysis::db(co * a / ci))
    };
    [pk(|v| v), pk(|v| -v)]
}

/// The engine-input level (dBFS) at which polarity `pol` (0 positive, 1 negative) compresses by
/// 0.1 dB, by bisection between `lo` and `hi`.
fn onset_level(make: Make<'_>, settings: Settings, rate: u32, pol: usize, lo: f64, hi: f64) -> f64 {
    let (mut a, mut b) = (lo, hi);
    for _ in 0..30 {
        let m = 0.5 * (a + b);
        if compression(make, settings, rate, m)[pol].0 > -0.1 {
            a = m;
        } else {
            b = m;
        }
    }
    0.5 * (a + b)
}

/// US2 AS1 (SC-003), the static curve through the engine's own clipper probe, in clipper units:
/// a 1 kHz tone rising from −20 to +40 dBFS over one second (`mic_only_quiet`). The output
/// equals the input exactly inside the linear range, never exceeds either limit, reaches each
/// limit as printed, and compresses by 0.1 dB at the printed onset; the trace is the same
/// whatever the limiter's knee setting.
pub fn static_curve(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let n = rate as usize;
    let unit = stimulus::tone_at_phase(1000.0, 0.0, 0.0, fs, n);
    let x: Vec<f32> = unit
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            (f64::from(v) * analysis::from_db(-20.0 + 60.0 * i as f64 / n as f64)) as f32
        })
        .collect();
    let s = configs::settings("mic_only_quiet", rate);
    let t = clip_trace(make, s, &x);
    let [ln, lp] = CLIP_LIMITS[rate_index(rate)];
    let row = |property: &'static str, value: f64, unit: &'static str, tol: Tolerance| {
        MeasurementResult::new(
            "SC-003",
            property,
            T_CLIP,
            rate,
            "mic_only_quiet",
            "1 kHz rising -20 to +40 dBFS over 1 s",
            value,
            unit,
            tol,
        )
    };
    // The linear range, kept clear of the printed thresholds' last digits.
    let margin = 1.0 - 1e-6;
    let nonlinear_inside = t
        .iter()
        .filter(|&&(i, o)| i <= KNEE_K * lp * margin && i >= -KNEE_K * ln * margin && o != i)
        .count();
    let max_out = t.iter().map(|&(_, o)| o).fold(f64::MIN, f64::max);
    let min_out = t.iter().map(|&(_, o)| o).fold(f64::MAX, f64::min);
    // The onset in clipper units: the smallest |input| per polarity compressed by 0.1 dB.
    let onset = |sign: f64| {
        t.iter()
            .filter(|&&(i, o)| i * sign > 0.0 && o / i <= analysis::from_db(-0.1))
            .map(|&(i, _)| i.abs())
            .fold(f64::MAX, f64::min)
    };
    let mut rows = vec![
        row(
            "samples not equal to the input inside the linear range",
            nonlinear_inside as f64,
            "samples",
            Tolerance::Exact(0.0), // FR-006 (US2 AS1): exactly linear below the onset
        ),
        row(
            "largest output re L+ (clipper units)",
            max_out - lp,
            "",
            Tolerance::Range(-LIMIT_PRINT_TOL, LIMIT_PRINT_TOL), // SC-003: reaches, never exceeds
        ),
        row(
            "smallest output re -L- (clipper units)",
            min_out + ln,
            "",
            Tolerance::Range(-LIMIT_PRINT_TOL, LIMIT_PRINT_TOL), // SC-003: reaches, never exceeds
        ),
        row(
            "positive onset re L+",
            analysis::db(onset(1.0) / lp) - ONSET_RE_LIMIT_DB,
            "dB",
            Tolerance::Range(-CLIP_TOL_DB, CLIP_TOL_DB), // SC-003
        ),
        row(
            "negative onset re L-",
            analysis::db(onset(-1.0) / ln) - ONSET_RE_LIMIT_DB,
            "dB",
            Tolerance::Range(-CLIP_TOL_DB, CLIP_TOL_DB), // SC-003
        ),
    ];
    // Whatever the limiter's knee: the clipper sits before it.
    for knee in [-40.0f32, 3.0] {
        let mut k = s;
        k.agc.enabled = true;
        k.agc.knee_dbfs = knee;
        rows.push(row(
            "trace equals the default's with the limiter's knee moved",
            if clip_trace(make, k, &x) == t {
                1.0
            } else {
                0.0
            },
            "",
            Tolerance::Exact(1.0), // FR-006 (US2 AS1): independent of the knee setting
        ));
    }
    rows
}

/// US2 AS1 (SC-003) at the engine input: 1 kHz tones at the printed start phase; each
/// polarity's onset (0.1 dB compression, by bisection on the level) and limit (`out peak × A /
/// in peak` at +40 dBFS) within ±0.5 dB of the script's figures; the negative polarity clips
/// first.
pub fn engine_onsets(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let s = configs::settings("mic_only_quiet", rate);
    let [neg_l, pos_l] = LIMIT_DBFS;
    let pos_on = onset_level(make, s, rate, 0, pos_l - 6.0, pos_l + 3.0);
    let neg_on = onset_level(make, s, rate, 1, neg_l - 6.0, neg_l + 3.0);
    let [(_, pos_lim), (_, neg_lim)] = compression(make, s, rate, 40.0);
    let row = |property: &'static str, stim: &str, value: f64, tol: Tolerance| {
        MeasurementResult::new(
            "SC-003",
            property,
            T_CLIP,
            rate,
            "mic_only_quiet",
            stim,
            value,
            "dB",
            tol,
        )
    };
    vec![
        row(
            "positive onset at the engine input re the figure",
            "1 kHz at the start phase, bisected",
            pos_on - (pos_l + ONSET_RE_LIMIT_DB),
            Tolerance::Range(-CLIP_TOL_DB, CLIP_TOL_DB), // SC-003
        ),
        row(
            "negative onset at the engine input re the figure",
            "1 kHz at the start phase, bisected",
            neg_on - (neg_l + ONSET_RE_LIMIT_DB),
            Tolerance::Range(-CLIP_TOL_DB, CLIP_TOL_DB), // SC-003
        ),
        row(
            "positive limit at the engine input re L+",
            "1 kHz +40 dBFS",
            pos_lim - pos_l,
            Tolerance::Range(-CLIP_TOL_DB, CLIP_TOL_DB), // SC-003
        ),
        row(
            "negative limit at the engine input re L-",
            "1 kHz +40 dBFS",
            neg_lim - neg_l,
            Tolerance::Range(-CLIP_TOL_DB, CLIP_TOL_DB), // SC-003
        ),
        row(
            "positive onset minus negative onset (the negative clips first)",
            "1 kHz at the start phase, bisected",
            pos_on - neg_on,
            Tolerance::AtLeast(0.0), // A-051, A-036: negative samples are limited first
        ),
    ]
}

/// US2 AS2 (SC-003): every sample after stage 2 within L₊ + L₋ (rounded up to `f32`) for
/// +40 dBFS tones, a +40 dBFS step, squares toggling between ±100, random input at ±1 and ±10⁴,
/// and **triangles of ±10⁵** (`mic_only_quiet`). The input high-pass differentiates its input,
/// so an input square only spikes the clipper; a triangle becomes a square at the clipper,
/// holding it at each limit for half a period: the bound's worst case, reaching about 97 % of
/// L₊ + L₋ at 20 ms periods (the 1 − LP′ identity of research R-02 makes the bound exact only in
/// the limit of an infinitely long hold). The closest stimulus's margin is reported.
pub fn bound(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let n = rate as usize / 5;
    let square = |half: usize| -> Vec<f32> {
        (0..n)
            .map(|i| if (i / half) % 2 == 0 { 100.0 } else { -100.0 })
            .collect()
    };
    let scaled = |k: f32| -> Vec<f32> {
        stimulus::noise(0x0D60, n)
            .iter()
            .map(|&v| 2.0 * v * k)
            .collect()
    };
    let mut stimuli: Vec<(String, Vec<f32>)> = vec![
        (
            "1 kHz +40 dBFS".into(),
            stimulus::tone_at_phase(1000.0, 40.0, 0.0, fs, n),
        ),
        (
            "100 Hz +40 dBFS".into(),
            stimulus::tone_at_phase(100.0, 40.0, 0.0, fs, n),
        ),
        ("step to +40 dBFS".into(), stimulus::dc(100.0, n)),
        ("noise +-1".into(), scaled(1.0)),
        ("noise +-1e4".into(), scaled(1e4)),
    ];
    for half in [1, 3, 10, 50, 200] {
        stimuli.push((format!("square +-100, half period {half}"), square(half)));
    }
    let triangle = |period_ms: usize| -> Vec<f32> {
        let p = rate as usize * period_ms / 1000;
        (0..n)
            .map(|i| {
                let ph = (i % p) as f64 / p as f64;
                let tri = if ph < 0.5 {
                    4.0 * ph - 1.0
                } else {
                    3.0 - 4.0 * ph
                };
                (1e5 * tri) as f32
            })
            .collect()
    };
    for period_ms in [5, 10, 20, 50] {
        stimuli.push((
            format!("triangle +-1e5, period {period_ms} ms"),
            triangle(period_ms),
        ));
    }
    let limit = stage2_bound(rate);
    let mut closest = f64::MIN;
    let mut rows: Vec<MeasurementResult> = stimuli
        .into_iter()
        .map(|(name, x)| {
            let y = run(make, configs::settings("mic_only_quiet", rate), &x);
            let peak = y.iter().fold(0.0f64, |m, &v| m.max(f64::from(v).abs()));
            closest = closest.max(peak / limit);
            MeasurementResult::new(
                "SC-003",
                "largest |sample| after stage 2 re L+ + L-",
                T_CLIP,
                rate,
                "mic_only_quiet",
                name,
                peak - limit,
                "",
                Tolerance::AtMost(0.0), // SC-003: never above L+ + L- (f32, rounded up)
            )
        })
        .collect();
    rows.push(MeasurementResult::new(
        "SC-003",
        "the closest stimulus's peak re L+ + L-",
        T_CLIP,
        rate,
        "mic_only_quiet",
        "every stimulus above",
        closest,
        "",
        Tolerance::Range(0.9, 1.0), // engineering target: the triangles reach the worst case
    ));
    rows
}

/// US2 AS3: a 1 kHz tone 6 dB above the first (negative) onset at the start phase, after a
/// 0.5 s warm-up, over 100 periods: H2 and H3 re the fundamental after stage 2 within ±0.5 dB
/// of the script's discrete model; the alias level (discrete − continuous) is reported.
pub fn harmonics(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let warm = rate as usize / 2;
    let n = rate as usize / 10;
    let level = LIMIT_DBFS[0] + ONSET_RE_LIMIT_DB + 6.0;
    let x = peak_tone(rate, level, warm + n);
    let y = run(make, configs::settings("mic_only_quiet", rate), &x);
    let w = &y[warm..warm + n];
    let mag = |f: f64| analysis::dtft(w, f, fs).norm();
    let f1 = mag(1000.0);
    let want = HARMONICS_DB[rate_index(rate)];
    let mut rows = Vec::new();
    for (j, h) in [2.0, 3.0].into_iter().enumerate() {
        rows.push(MeasurementResult::new(
            "US2 AS3",
            if j == 0 {
                "H2 re the fundamental, re the discrete model"
            } else {
                "H3 re the fundamental, re the discrete model"
            },
            T_CLIP,
            rate,
            "mic_only_quiet",
            format!("1 kHz {level:+.2} dBFS (the first onset + 6 dB)"),
            analysis::db(mag(1000.0 * h) / f1) - want[j],
            "dB",
            Tolerance::Range(-HARMONIC_TOL_DB, HARMONIC_TOL_DB), // FR-006 (US2 AS3)
        ));
        rows.push(MeasurementResult::new(
            "US2 AS3",
            if j == 0 {
                "H2 alias level (discrete - continuous)"
            } else {
                "H3 alias level (discrete - continuous)"
            },
            T_CLIP,
            rate,
            "mic_only_quiet",
            "the script's models",
            want[j] - HARMONICS_CONTINUOUS_DB[j],
            "dB",
            Tolerance::Report,
        ));
    }
    rows
}

/// US2 AS4: a 1 kHz tone at +13 dBFS for 50 ms then −20 dBFS, at the start phase, against the
/// clipper-off twin (`MicMutation::clipper_off`). The clipper is memoryless: after the last
/// clipped sample its output equals the twin's bit for bit (so the difference after stage 2 is
/// the output coupling's and the roll-off's decaying tail); the difference falls within ±0.1 dB
/// of the −20 dBFS response by the script's printed time (within one sample).
pub fn recovery(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let (n1, n2) = (rate as usize / 20, rate as usize / 10);
    let unit = peak_tone(rate, 0.0, n1 + n2);
    let x: Vec<f32> = unit
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let a = if i < n1 { 13.0 } else { -20.0 };
            (f64::from(v) * analysis::from_db(a)) as f32
        })
        .collect();
    let s = configs::settings("mic_only_quiet", rate);
    let off = rr_dr60::__test_hooks::MicMutation {
        clipper_off: true,
        ..Default::default()
    };
    let twin = |s: Settings| Pipeline::__with_mic_mutation(s, &off).expect("supported settings");
    // Memorylessness, through the traces.
    let tc = clip_trace(make, s, &x);
    let tl = clip_trace(&twin, s, &x);
    let last = tc.iter().rposition(|&(i, o)| i != o).unwrap_or(0);
    let after = tc[last + 1..]
        .iter()
        .zip(&tl[last + 1..])
        .filter(|(a, b)| a.1.to_bits() != b.1.to_bits())
        .count();
    // The settle time after the step down.
    let yc = run(make, s, &x);
    let yl = run(&twin, s, &x);
    let tol = analysis::from_db(-20.0) * (analysis::from_db(0.1) - 1.0);
    let late = yc[n1..]
        .iter()
        .zip(&yl[n1..])
        .rposition(|(a, b)| (f64::from(*a) - f64::from(*b)).abs() > tol);
    let settle_ms = late.map_or(0.0, |k| (k + 1) as f64 / fs * 1e3);
    let one = 1e3 / fs;
    let want = RECOVERY_MS[rate_index(rate)];
    let row = |property: &'static str, value: f64, unit: &'static str, tol: Tolerance| {
        MeasurementResult::new(
            "US2 AS4",
            property,
            T_CLIP,
            rate,
            "mic_only_quiet",
            "1 kHz +13 dBFS 50 ms, then -20 dBFS",
            value,
            unit,
            tol,
        )
    };
    vec![
        row(
            "clipped samples (the burst clips)",
            tc.iter().filter(|&&(i, o)| i != o).count() as f64,
            "samples",
            Tolerance::AtLeast(1.0), // FR-006 (US2 AS4): clipping in progress
        ),
        row(
            "clipper outputs differing from the twin after the last clipped sample",
            after as f64,
            "samples",
            Tolerance::Exact(0.0), // FR-006 (US2 AS4): memoryless
        ),
        row(
            "time within +-0.1 dB of the linear response re the script's",
            settle_ms - want,
            "ms",
            Tolerance::Range(-one, one), // FR-006 (US2 AS4): the printed time, one sample
        ),
    ]
}
