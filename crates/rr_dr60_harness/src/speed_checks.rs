//! Spec 013 playback-speed checks: one function per measurable property (research R-08; tasks.md
//! T010, T017).
//!
//! The speed control's own response is measured in [`configs`]'s `speed_only` configuration at
//! the 6 kHz host rate: every other stage is bypassed and the player honours no tap, so with
//! stage 10 and the output chain off and the identity rate, `play()`'s output **is** the speed
//! control's device output, narrowed to `f32`, one device sample per host sample. Stimuli are
//! stored through [`configs::stored_recording`] (recorded with every record stage off, so the
//! stored samples equal the stimulus, then imported).
//!
//! The **independent reference** [`reference_output`] computes the speed control's outputs from
//! the stored stream, the published bank (`__test_hooks::speed_bank`), the four-sum order and
//! the place rule (FR-005, FR-007; research R-02, R-04) with its own integer arithmetic; the
//! engine's output must equal it bit for bit (research R-08 (c)). Filter figures are pinned to
//! `tools/playback-speed/speed_design.py`'s print (research R-03), never to the engine's output.

use rr_dr60::{Pipeline, Settings, speed};

use crate::analysis;
use crate::checks::{Make, MeasurementResult, Tolerance};
use crate::{configs, stimulus};

/// Trace: the playback speed control (A-054) and its design figures.
pub const T_SPEED: &[&str] = &["A-054", "engineering target"];

/// The levels other than the identity (spec 013 FR-002).
pub const LEVELS: [u8; 4] = [1, 2, 4, 5];

/// The stored tones' amplitude: −6 dBFS (data-model.md › Stimuli; engineering target).
pub const AMP: f64 = 0.5;

/// The script's worst spurious figure per level as (level, stored tone Hz, dB re the tone),
/// printed by `uv run tools/playback-speed/speed_design.py` on 2026-10-17 (research R-03; the
/// four-times zero-padded reader).
pub const SCRIPT_WORST_SPURIOUS: [(u8, f64, f64); 4] = [
    (1, 2853.0, -73.0),
    (2, 2853.0, -73.0),
    (4, 2503.0, -72.5),
    (5, 2203.0, -73.9),
];

/// The look-ahead D (research R-03).
const D: i64 = speed::LOOKAHEAD_SAMPLES as i64;
/// Taps per phase, 2·D + 1 (research R-03; engineering target).
const PER_PHASE: usize = 2 * speed::LOOKAHEAD_SAMPLES as usize + 1;

/// The numerator k of `level` (FR-002).
pub fn numerator(level: u8) -> u32 {
    speed::RATIO_NUMERATORS[usize::from(level) - 1]
}

/// The ratio r of `level`, for frequencies only (the place is never derived from it).
pub fn ratio(level: u8) -> f64 {
    f64::from(numerator(level)) / 5.0
}

/// A playback through the public API.
#[derive(Clone, Debug, Default)]
pub struct Played {
    /// Every sample `play` produced, to `finished`.
    pub out: Vec<f32>,
    /// `PlayInfo::place` per call.
    pub places: Vec<u64>,
    /// `PlayInfo::frame_index` per call.
    pub frame_indices: Vec<u64>,
    /// `PlayInfo::frames` summed.
    pub frames: u64,
    /// `PlayInfo::frames` per call.
    pub frames_per_call: Vec<u64>,
    /// Calls that reported `finished` (must be 1).
    pub finished: usize,
}

/// Plays recording `number` in calls of `call` host samples with the level `schedule`: each
/// `(c, level)` is set before call `c` (`c = 0`: before `start_playback`).
///
/// # Panics
///
/// On an engine error, or if the playback does not finish within a bound.
pub fn play(p: &mut Pipeline, number: u32, call: usize, schedule: &[(usize, u8)]) -> Played {
    for &(c, level) in schedule {
        if c == 0 {
            p.set_playback_speed(level).expect("a valid level");
        }
    }
    p.start_playback(number).expect("start_playback");
    let mut buf = vec![0.0f32; call];
    let mut played = Played::default();
    for c in 0..10_000_000usize {
        for &(at, level) in schedule {
            if at == c && c > 0 {
                p.set_playback_speed(level).expect("a valid level");
            }
        }
        let info = p.play(&mut buf).expect("play");
        played.out.extend_from_slice(&buf[..info.produced]);
        played.places.push(info.place);
        played.frame_indices.push(info.frame_index);
        played.frames += info.frames as u64;
        played.frames_per_call.push(info.frames as u64);
        if info.finished {
            played.finished += 1;
            // One more call must be refused (the engine is idle).
            assert!(p.play(&mut buf).is_err(), "play after finished");
            return played;
        }
    }
    panic!("the playback did not finish");
}

/// `narrow_out`: the engine's narrowing of a 64-bit sample for output (round to nearest, `f32`
/// subnormals flushed to 0.0).
fn narrow(y: f64) -> f32 {
    let v = y as f32;
    if v.is_subnormal() { 0.0 } else { v }
}

/// The independent reference (research R-08 (c)): the speed control's device outputs for the
/// stored stream `x` played at 6 kHz in calls of `call` outputs with the level `schedule` (as
/// [`play`]'s), narrowed to `f32`, and the place each call reports. The rules (FR-005, FR-007,
/// FR-009): the place starts at 0 in fifths; before each output the level latches once the place
/// has reached K, then a requested non-3 level applies at once and level 3 only at a whole
/// place; level 3 outputs x[i]; other levels the four-sum dot product of the level's bank phase
/// with x[i + 28 − m]; the recording's own part is the places before K, the tail the outputs
/// whose kernel still reaches x[K − 1], then the playback ends (the speed-only drain is 0 at
/// 6 kHz). A call reports the place of its last output, or K once the place has reached K.
pub fn reference_output(x: &[f32], call: usize, schedule: &[(usize, u8)]) -> (Vec<f32>, Vec<u64>) {
    let start = schedule
        .iter()
        .find(|(c, _)| *c == 0)
        .map_or(speed::DEFAULT_LEVEL, |&(_, l)| l);
    let dev: Vec<(u64, u8)> = schedule
        .iter()
        .filter(|(c, _)| *c > 0)
        .map(|&(c, l)| ((c * call) as u64, l))
        .collect();
    reference_output_at(x, call, start, &dev)
}

/// The crossfade's weights from the pinned expression (spec 019 research R-02): entry i − 1 is
/// `u * u * (3.0 - 2.0 * u)` at u = i ÷ N, N = `speed::CROSSFADE_SAMPLES`; equal to the engine's
/// table bit for bit (`us1_crossfade` asserts it against the hook).
pub fn fade_weights() -> Vec<f64> {
    let n = speed::CROSSFADE_SAMPLES;
    (1..=n)
        .map(|i| {
            let u = f64::from(i) / f64::from(n);
            u * u * (3.0 - 2.0 * u)
        })
        .collect()
}

/// [`reference_output`] with the level changes keyed by the **device-output index** n at which
/// the request is made (spec 015 research R-08: a change scheduled at host position P is a
/// request at n = d(P), `rr_dr60::changes::speed_landing`), `start` the level before output 0.
/// Several requests at one index apply in order (the last wins). Since spec 019 a change
/// **crossfades** (research R-01, R-06): the request is applied only while no fade runs (the
/// wait; the latest request held), a change into level 3 at a whole place; from the output it
/// applies at, for N outputs the old level's value at its own place (advancing at the old
/// ratio) and the new level's at the common place are blended as y_old + w_n·(y_new − y_old)
/// for n < N and the new value itself at n = N.
pub fn reference_output_at(
    x: &[f32],
    call: usize,
    start: u8,
    schedule: &[(u64, u8)],
) -> (Vec<f32>, Vec<u64>) {
    let banks: Vec<(u32, Vec<f64>)> = LEVELS
        .iter()
        .map(|&l| {
            (
                numerator(l),
                rr_dr60::__test_hooks::speed_bank(l).expect("bank"),
            )
        })
        .collect();
    let bank = |k: u32| &banks.iter().find(|(kk, _)| *kk == k).expect("k").1;
    let kk = x.len() as u64;
    let weights = fade_weights();
    let n_fade = weights.len() as u32;
    let mut active = numerator(start);
    let mut requested = active;
    let mut p5: u64 = 0;
    // The running crossfade: (the old numerator, the old branch's place, outputs blended).
    let mut fade: Option<(u32, u64, u32)> = None;
    let (mut out, mut place_after) = (Vec::new(), Vec::new());
    loop {
        let n = out.len() as u64;
        for &(at, l) in schedule {
            if at == n {
                requested = numerator(l);
            }
        }
        let latched = p5 >= 5 * kk;
        if !latched && requested != active && fade.is_none() && (requested != 5 || p5 % 5 == 0) {
            // A change on the first output switches at once (spec 011's first-sample rule).
            if n > 0 {
                fade = Some((active, p5, 0));
            }
            active = requested;
        }
        let i = (p5 / 5) as i64;
        let ph = (p5 % 5) as usize;
        if latched {
            let lowest = i - D + i64::from(ph != 0);
            if active == 5 || lowest > kk as i64 - 1 {
                break;
            }
        }
        let y_new = value_at(
            x,
            (active != 5).then(|| bank(active).as_slice()),
            active,
            p5,
        );
        let y = match fade {
            None => y_new,
            Some((old_k, old_p5, m)) => {
                let m = m + 1;
                let y = if m >= n_fade {
                    y_new
                } else {
                    let y_old = value_at(
                        x,
                        (old_k != 5).then(|| bank(old_k).as_slice()),
                        old_k,
                        old_p5,
                    );
                    y_old + weights[(m - 1) as usize] * (y_new - y_old)
                };
                fade = (m < n_fade).then_some((old_k, old_p5 + u64::from(old_k), m));
                y
            }
        };
        out.push(narrow(y));
        let here = p5 / 5;
        p5 += u64::from(active);
        place_after.push(if p5 >= 5 * kk { kk } else { here });
    }
    let places = (0..out.len().div_ceil(call))
        .map(|c| place_after[((c + 1) * call).min(out.len()) - 1])
        .collect();
    (out, places)
}

/// The speed control's value at place `p5` (fifths) for numerator `k` on the stored stream `x`
/// (x = 0 outside it): x[i] at k = 5, else the four-sum dot product of the level's bank phase
/// with x[i + 28 − m] (research R-03, R-08).
pub fn value_at(x: &[f32], bank: Option<&[f64]>, k: u32, p5: u64) -> f64 {
    let kk = x.len() as i64;
    let xs = |j: i64| {
        if j < 0 || j >= kk {
            0.0
        } else {
            f64::from(x[j as usize])
        }
    };
    let i = (p5 / 5) as i64;
    let ph = (p5 % 5) as usize;
    if k == 5 {
        return xs(i);
    }
    let c = &bank.expect("a bank for k ≠ 5")[ph * PER_PHASE..(ph + 1) * PER_PHASE];
    let mut s = [0.0f64; 4];
    for (m, &cm) in c.iter().enumerate() {
        s[m & 3] += cm * xs(i + D - m as i64);
    }
    (s[0] + s[1]) + (s[2] + s[3])
}

/// The stored stream of recording `number` as the level-3 playback reads it at 6 kHz in
/// `speed_only` / `speed_only_codec` (the stored samples, or the decoded frames): the
/// reference's input.
pub fn stored_stream(p: &mut Pipeline, number: u32) -> Vec<f32> {
    play(p, number, 120, &[(0, speed::DEFAULT_LEVEL)]).out
}

/// The count of samples where `a` and `b` differ bit for bit, plus the length difference.
pub fn mismatches(a: &[f32], b: &[f32]) -> usize {
    a.iter()
        .zip(b)
        .filter(|(x, y)| x.to_bits() != y.to_bits())
        .count()
        + a.len().abs_diff(b.len())
}

/// FR-005, FR-009 (US1 AS1): at each level a recording's playback produces ⌈(⌈5K ÷ k⌉ + T) ·
/// m ÷ l⌉ host samples before the drain, the drain is `drain_samples()`, `finished` comes once,
/// and `playback_length_samples` returns the total. `config` names the configuration.
pub fn lengths(make: Make<'_>, rate: u32, config: &'static str) -> Vec<MeasurementResult> {
    let x = stimulus::stored_tone(1000.0, AMP, 6007); // engineering target: a K not a multiple of 5
    let mut results = Vec::new();
    for level in 1..=speed::LEVEL_COUNT {
        let (mut p, n) = configs::stored_recording(make, configs::settings(config, rate), &x);
        let k_stored = p.recording_length(n).expect("length");
        let k = numerator(level);
        let own = (5 * k_stored).div_ceil(u64::from(k));
        let tail = rr_dr60::__test_hooks::speed_tail_len(k, k_stored);
        let pre_drain = ((own + tail) * u64::from(rate)).div_ceil(6000);
        let expected = pre_drain + u64::from(p.drain_samples());
        let helper = p.playback_length_samples(n, level).expect("helper");
        let played = play(&mut p, n, (rate / 50) as usize, &[(0, level)]);
        let stim = format!("1 kHz, K = {k_stored}, level {level}");
        results.push(MeasurementResult::new(
            "FR-005",
            "playback length − expected",
            T_SPEED,
            rate,
            config,
            stim.clone(),
            played.out.len() as f64 - expected as f64,
            "samples",
            Tolerance::Exact(0.0), // FR-005: exact
        ));
        results.push(MeasurementResult::new(
            "FR-009",
            "length helper − playback length",
            T_SPEED,
            rate,
            config,
            stim.clone(),
            helper as f64 - played.out.len() as f64,
            "samples",
            Tolerance::Exact(0.0), // FR-009: exact
        ));
        results.push(MeasurementResult::new(
            "FR-005",
            "finished reports",
            T_SPEED,
            rate,
            config,
            stim,
            played.finished as f64,
            "",
            Tolerance::Exact(1.0), // FR-005: exactly once
        ));
    }
    results
}

/// The steady middle of an output: `margin` samples off each end.
fn middle(y: &[f32], margin: usize) -> &[f32] {
    &y[margin..y.len() - margin]
}

/// FR-004 (a) – (c) at 6 kHz in `speed_only` (US1 AS2, AS3): the passband fit at r × f (±0.1
/// dB, residual ≤ −60 dB), the spurious products in 20 – 3000 Hz (≤ −60 dB; for r × f above
/// 3000 Hz the whole output), and the script's worst figures within 0.5 dB.
pub fn band_6k(make: Make<'_>) -> Vec<MeasurementResult> {
    let fs = 6000.0;
    let cfg = configs::settings("speed_only", 6000);
    let mut results = Vec::new();
    let play_tone = |level: u8, f: f64| {
        let x = stimulus::stored_tone(f, AMP, 9000);
        let (mut p, n) = configs::stored_recording(make, cfg, &x);
        play(&mut p, n, 120, &[(0, level)]).out
    };
    let ref_db = analysis::db(AMP);
    for level in LEVELS {
        let r = ratio(level);
        let top = 2250.0f64.min(2250.0 / r); // FR-004 (a)
        let mut f = 103.0;
        while f <= top {
            let y = play_tone(level, f);
            let (a, res) = analysis::sine_fit(middle(&y, 900), r * f, fs);
            let stim = format!("level {level}, stored {f:.0} Hz");
            results.push(MeasurementResult::new(
                "FR-004",
                "(a) passband level at r × f",
                T_SPEED,
                6000,
                "speed_only",
                stim.clone(),
                analysis::db(a / AMP),
                "dB",
                Tolerance::Range(-0.1, 0.1), // FR-004 (a)
            ));
            results.push(MeasurementResult::new(
                "FR-004",
                "(a) fit residual",
                T_SPEED,
                6000,
                "speed_only",
                stim,
                analysis::spurious_peak_db(&res, fs, 20.0, 3000.0) - ref_db,
                "dB",
                Tolerance::AtMost(-60.0), // FR-004 (a)
            ));
            f += 50.0;
        }
        let worst = SCRIPT_WORST_SPURIOUS
            .iter()
            .find(|(l, _, _)| *l == level)
            .expect("level");
        let mut tones: Vec<f64> = (0..29).map(|i| 103.0 + 100.0 * f64::from(i)).collect();
        tones.push(worst.1);
        for f in tones {
            let y = play_tone(level, f);
            let mid = middle(&y, 900);
            let spur = if r * f < 2990.0 {
                let (_, res) = analysis::sine_fit(mid, r * f, fs);
                analysis::spurious_peak_db(&res, fs, 20.0, 3000.0)
            } else {
                analysis::spurious_peak_db(mid, fs, 20.0, 3000.0) // FR-004 (c)
            } - ref_db;
            let stim = format!("level {level}, stored {f:.0} Hz");
            results.push(MeasurementResult::new(
                "FR-004",
                "(b), (c) spurious in 20 – 3000 Hz",
                T_SPEED,
                6000,
                "speed_only",
                stim.clone(),
                spur,
                "dB",
                Tolerance::AtMost(-60.0), // FR-004 (b), (c)
            ));
            if f == worst.1 {
                results.push(MeasurementResult::new(
                    "FR-004",
                    "worst spurious − the script's",
                    T_SPEED,
                    6000,
                    "speed_only",
                    stim,
                    spur - worst.2,
                    "dB",
                    Tolerance::Range(-0.5, 0.5), // engineering target (013 R-03 pin)
                ));
            }
        }
    }
    results
}

/// FR-004's other-rate clause (US1 AS2 at the measurement rates): a stored tone at f played at a
/// level reads within ±0.1 dB of level 3's playback of a stored tone at r × f (the host
/// converter's response common to both), in `speed_only`.
pub fn band_other_rate(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let cfg = configs::settings("speed_only", rate);
    let call = (rate / 50) as usize;
    let mut results = Vec::new();
    for level in LEVELS {
        let r = ratio(level);
        for f in [500.0, 1000.0, 1500.0] {
            if r * f > 2250.0 {
                continue;
            }
            let (mut p, n) =
                configs::stored_recording(make, cfg, &stimulus::stored_tone(f, AMP, 9000));
            let y = play(&mut p, n, call, &[(0, level)]).out;
            let (mut q, m) =
                configs::stored_recording(make, cfg, &stimulus::stored_tone(r * f, AMP, 9000));
            let z = play(&mut q, m, call, &[(0, speed::DEFAULT_LEVEL)]).out;
            let margin = (0.15 * fs) as usize;
            let (a, res) = analysis::sine_fit(middle(&y, margin), r * f, fs);
            let (b, _) = analysis::sine_fit(middle(&z, margin), r * f, fs);
            results.push(MeasurementResult::new(
                "FR-004",
                "spurious in 20 – 3000 Hz (other rates, reported)",
                T_SPEED,
                rate,
                "speed_only",
                format!("level {level}, stored {f:.0} Hz"),
                analysis::spurious_peak_db(&res, fs, 20.0, 3000.0) - analysis::db(AMP),
                "dB",
                Tolerance::Report, // FR-004: reported at the other rates
            ));
            results.push(MeasurementResult::new(
                "FR-004",
                "level at r × f − level 3's at r × f",
                T_SPEED,
                rate,
                "speed_only",
                format!("level {level}, stored {f:.0} Hz"),
                analysis::db(a / b),
                "dB",
                Tolerance::Range(-0.1, 0.1), // FR-004 (other rates)
            ));
        }
    }
    results
}

/// US1 AS5 on the default path (`speed_default`): a stored tone at f plays at r × f (the fit's
/// frequency within 0.01 %), and level(r) − level(3) equals H(r × f) − H(f) within ±0.2 dB, H
/// the level-3 response of the codec-bypassed path (`speed_default_no_codec`).
pub fn default_path(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let call = (rate / 50) as usize;
    let margin = (0.3 * fs) as usize;
    let amp_of = |config: &str, f_stored: f64, level: u8, f_out: f64| {
        let (mut p, n) = configs::stored_recording(
            make,
            configs::settings(config, rate),
            &stimulus::stored_tone(f_stored, AMP, 12_000),
        );
        let y = play(&mut p, n, call, &[(0, level)]).out;
        analysis::fit_sinusoid(middle(&y, margin), fs, f_out)
    };
    let mut results = Vec::new();
    for level in LEVELS {
        let r = ratio(level);
        for f in [500.0, 1000.0] {
            let (a_r, f_fit) = amp_of("speed_default", f, level, r * f);
            let (a_3, _) = amp_of("speed_default", f, 3, f);
            let (h_rf, _) = amp_of("speed_default_no_codec", r * f, 3, r * f);
            let (h_f, _) = amp_of("speed_default_no_codec", f, 3, f);
            let stim = format!("level {level}, stored {f:.0} Hz");
            results.push(MeasurementResult::new(
                "FR-003",
                "fit frequency ÷ (r × f) − 1",
                T_SPEED,
                rate,
                "speed_default",
                stim.clone(),
                f_fit / (r * f) - 1.0,
                "",
                Tolerance::Range(-1e-4, 1e-4), // US1 AS5: 0.01 %
            ));
            results.push(MeasurementResult::new(
                "FR-003",
                "level(r) − level(3) − (H(rf) − H(f))",
                T_SPEED,
                rate,
                "speed_default",
                stim,
                analysis::db(a_r / a_3) - analysis::db(h_rf / h_f),
                "dB",
                Tolerance::Range(-0.2, 0.2), // US1 AS5
            ));
        }
    }
    results
}

/// FR-008 (US1 AS6) at 6 kHz in `speed_only`: an impulse stored at j peaks at the nearest
/// device output and its response is symmetric about the place; at a half-sample tie (levels 2
/// and 4) the two outputs are equal. Tolerances are one `f32` rounding of the narrowed output.
pub fn impulse(make: Make<'_>) -> Vec<MeasurementResult> {
    let cfg = configs::settings("speed_only", 6000);
    let mut results = Vec::new();
    for level in LEVELS {
        let k = u64::from(numerator(level));
        let j: u64 = if level == 5 { 602 } else { 600 }; // whole places (013 plan review P8)
        let (mut p, n) =
            configs::stored_recording(make, cfg, &stimulus::stored_impulse(j as usize, 1600));
        let y = play(&mut p, n, 120, &[(0, level)]).out;
        let peak = (0..y.len())
            .max_by(|&a, &b| y[a].abs().total_cmp(&y[b].abs()))
            .expect("output");
        let want = (5 * j) / k;
        results.push(MeasurementResult::new(
            "FR-008",
            "impulse peak − nearest output",
            T_SPEED,
            6000,
            "speed_only",
            format!("level {level}, j = {j}"),
            peak as f64 - want as f64,
            "outputs",
            Tolerance::Exact(0.0), // FR-008
        ));
        let asym = (1..40)
            .map(|d| (f64::from(y[peak + d]) - f64::from(y[peak - d])).abs())
            .fold(0.0f64, f64::max);
        results.push(MeasurementResult::new(
            "FR-008",
            "impulse response asymmetry",
            T_SPEED,
            6000,
            "speed_only",
            format!("level {level}, j = {j}"),
            asym,
            "",
            Tolerance::AtMost(2e-7), // FR-008 (one f32 rounding of the narrowed output)
        ));
    }
    // The half-sample ties: level 2 (k = 4) at j = 2 (places 1.6 and 2.4), level 4 (k = 6) at
    // j = 3 (places 2.4 and 3.6).
    for (level, j, a, b) in [(2u8, 2usize, 2usize, 3usize), (4, 3, 2, 3)] {
        let (mut p, n) = configs::stored_recording(make, cfg, &stimulus::stored_impulse(j, 400));
        let y = play(&mut p, n, 120, &[(0, level)]).out;
        results.push(MeasurementResult::new(
            "FR-008",
            "half-sample tie difference",
            T_SPEED,
            6000,
            "speed_only",
            format!("level {level}, j = {j}"),
            (f64::from(y[a]) - f64::from(y[b])).abs(),
            "",
            Tolerance::AtMost(2e-7), // FR-008 (one f32 rounding)
        ));
    }
    results
}

/// FR-005, FR-007 (US1 AS6; research R-08 (c)): at each level in `config` (`speed_only` or
/// `speed_only_codec`, 6 kHz) a recording ending on a full-scale sample plays bit for bit as
/// the independent reference computes it, its tail included, with the reference's places.
pub fn tail_and_reference(make: Make<'_>, config: &'static str) -> Vec<MeasurementResult> {
    let cfg = configs::settings(config, 6000);
    let mut results = Vec::new();
    let x = stimulus::stored_end_loud(1001);
    for level in LEVELS {
        let (mut p, n) = configs::stored_recording(make, cfg, &x);
        let stream = stored_stream(&mut p, n);
        let played = play(&mut p, n, 120, &[(0, level)]);
        let (want, places) = reference_output(&stream, 120, &[(0, level)]);
        results.push(MeasurementResult::new(
            "FR-005",
            "samples differing from the reference",
            T_SPEED,
            6000,
            config,
            format!("level {level}, ends at full scale"),
            mismatches(&played.out, &want) as f64,
            "",
            Tolerance::Exact(0.0), // FR-005, FR-007: bit-exact
        ));
        results.push(MeasurementResult::new(
            "FR-009",
            "places differing from the reference",
            T_SPEED,
            6000,
            config,
            format!("level {level}"),
            played
                .places
                .iter()
                .zip(&places)
                .filter(|(a, b)| a != b)
                .count() as f64
                + played.places.len().abs_diff(places.len()) as f64,
            "",
            Tolerance::Exact(0.0), // FR-009
        ));
    }
    results
}

/// FR-006 (US1 AS4) at 6 kHz in `config`: at level 3 the stored stream plays unaltered (the
/// identity), the place reports the last output's index (at most K) and the frame index is
/// min(⌊place ÷ 120⌋, F − 1) with the codec on, 0 bypassed.
pub fn level3_identity(make: Make<'_>, config: &'static str) -> Vec<MeasurementResult> {
    let cfg = configs::settings(config, 6000);
    let x = stimulus::stored_voiced(1.0);
    let (mut p, n) = configs::stored_recording(make, cfg, &x);
    let k_stored = p.recording_length(n).expect("length");
    let played = play(&mut p, n, 120, &[(0, 3)]);
    let mut results = Vec::new();
    if !cfg.speech_codec_enabled {
        // The samples memory: the stored samples are the stimulus, then the stop's silence.
        let mut want = x.clone();
        want.resize(k_stored as usize, 0.0);
        results.push(MeasurementResult::new(
            "FR-006",
            "level-3 samples differing from the stored ones",
            T_SPEED,
            6000,
            config,
            "voiced, 1 s",
            mismatches(&played.out, &want) as f64,
            "",
            Tolerance::Exact(0.0), // FR-006: bit-identical
        ));
    }
    let frames = k_stored / 120;
    let bad_index = played
        .places
        .iter()
        .zip(&played.frame_indices)
        .filter(|(place, fi)| {
            let want = if cfg.speech_codec_enabled {
                (**place / 120).min(frames.saturating_sub(1))
            } else {
                0
            };
            **fi != want
        })
        .count();
    results.push(MeasurementResult::new(
        "FR-009",
        "frame indices differing from the place rule",
        T_SPEED,
        6000,
        config,
        "voiced, 1 s",
        bad_index as f64,
        "",
        Tolerance::Exact(0.0), // FR-009
    ));
    if cfg.speech_codec_enabled {
        // FR-006: nothing read ahead at level 3, so after each call the frames decoded are
        // those holding the places reached, as in 0.12: ⌊place ÷ 120⌋ + 1 in all.
        let mut total = 0u64;
        let mut bad = 0usize;
        for (place, f) in played.places.iter().zip(&played.frames_per_call) {
            total += f;
            if total != (place / 120 + 1).min(frames) {
                bad += 1;
            }
        }
        results.push(MeasurementResult::new(
            "FR-006",
            "calls whose decoded frames differ from 0.12's",
            T_SPEED,
            6000,
            config,
            "voiced, 1 s",
            bad as f64,
            "",
            Tolerance::Exact(0.0), // FR-006: the reports identical
        ));
    }
    let decreasing = played.places.windows(2).filter(|w| w[1] < w[0]).count();
    results.push(MeasurementResult::new(
        "FR-009",
        "place decreases",
        T_SPEED,
        6000,
        config,
        "voiced, 1 s",
        decreasing as f64,
        "",
        Tolerance::Exact(0.0), // FR-009: never decreases
    ));
    results.push(MeasurementResult::new(
        "FR-009",
        "final place − K",
        T_SPEED,
        6000,
        config,
        "voiced, 1 s",
        *played.places.last().expect("a call") as f64 - k_stored as f64,
        "",
        Tolerance::Exact(0.0), // FR-009: reaches the end
    ));
    results
}

/// The settings of a named configuration (a re-export for the tests' convenience).
pub fn settings(name: &str, rate: u32) -> Settings {
    configs::settings(name, rate)
}

/// FR-005, FR-007 (US2 AS1 – AS3; research R-08 (c)) as rows, for the mutation tests: a set of
/// schedules at 6 kHz in `config` against the independent reference: a pair at two change
/// points, into level 3 at a place that is not whole (calls of 121), and changes into level 3
/// and to another level in the last five outputs before the place reaches K and just after it
/// (one-sample calls).
pub fn schedules(make: Make<'_>, config: &'static str) -> Vec<MeasurementResult> {
    let cfg = configs::settings(config, 6000);
    // K = 420 stored samples: 5K is a multiple of every non-3 numerator (3, 4, 6, 7), so a
    // pending switch into level 3 can meet the place exactly at K at every level (the latch's
    // order, post-implementation review B5). The stop adds its flush of silence to the stimulus.
    let flush = {
        let (p, n) = configs::stored_recording(make, cfg, &[0.5]);
        p.recording_length(n).expect("length") as usize - 1
    };
    let x = stimulus::stored_end_loud(420 - flush);
    let mut cases: Vec<(usize, Vec<(usize, u8)>)> =
        vec![(120, vec![(0, 1), (1, 5)]), (121, vec![(0, 5), (1, 2)])];
    for a in LEVELS {
        cases.push((121, vec![(0, a), (1, 3)]));
        let own = (5 * 420u64).div_ceil(u64::from(numerator(a))) as usize;
        for back in 0..=5usize {
            cases.push((1, vec![(0, a), (own - back, 3)]));
        }
        cases.push((1, vec![(0, a), (own + 1, 1)]));
    }
    let mut results = Vec::new();
    for (call, schedule) in cases {
        let (mut p, n) = configs::stored_recording(make, cfg, &x);
        let stream = stored_stream(&mut p, n);
        let played = play(&mut p, n, call, &schedule);
        let (want, _) = reference_output(&stream, call, &schedule);
        results.push(MeasurementResult::new(
            "FR-007",
            "schedule samples differing from the reference",
            T_SPEED,
            6000,
            config,
            format!("calls of {call}, {schedule:?}"),
            mismatches(&played.out, &want) as f64,
            "",
            Tolerance::Exact(0.0), // FR-007: bit-exact
        ));
    }
    results
}

/// The script's level-change steps (FR-007, reported; post-implementation review B2) as (stored
/// tone Hz, from level, to level, dB re the −6 dBFS tone), printed by
/// `uv run tools/playback-speed/speed_design.py` on 2026-10-17.
pub const SCRIPT_STEPS: [(f64, u8, u8, f64); 20] = [
    (1000.0, 1, 5, -77.4),
    (1000.0, 5, 1, -73.6),
    (1000.0, 5, 3, -99.3),
    (1000.0, 3, 5, -99.3),
    (1000.0, 2, 4, -92.8),
    (1000.0, 4, 2, -93.2),
    (1000.0, 3, 1, -93.6),
    (2000.0, 1, 5, -1.1),
    (2000.0, 5, 1, -0.7),
    (2000.0, 5, 3, -1.9),
    (2000.0, 3, 5, -1.9),
    (2000.0, 2, 4, -40.0),
    (2000.0, 4, 2, -40.4),
    (2000.0, 3, 1, -83.8),
    (2400.0, 1, 5, -0.0),
    (2400.0, 5, 1, -0.0),
    (2400.0, 5, 3, -0.4),
    (2400.0, 3, 5, -0.4),
    (2400.0, 2, 4, -0.0),
    (2400.0, 4, 2, -0.0),
];

/// FR-007's reported step (post-implementation review B2): the output after a change at place p
/// is exactly the new level's value at p, so the step a change makes is the largest |y_new(p) −
/// y_old(p)| over the old level's places (whole places when either level is 3), for a stored
/// tone at −6 dBFS, dB re the tone; pinned to the script's figure within 0.5 dB.
pub fn change_step() -> Vec<MeasurementResult> {
    let bank = |level: u8| rr_dr60::__test_hooks::speed_bank(level);
    let mut results = Vec::new();
    for &(f, a, b, want) in &SCRIPT_STEPS {
        let x = stimulus::stored_tone(f, AMP, 4000);
        let (ka, kb) = (numerator(a), numerator(b));
        let (ba, bb) = (bank(a), bank(b));
        let mut worst = 0.0f64;
        for n in 200u64..2000 {
            let p5 = n * u64::from(ka);
            if (ka == 5 || kb == 5) && p5 % 5 != 0 {
                continue;
            }
            if p5 / 5 >= 3900 {
                break;
            }
            let ya = value_at(&x, ba.as_deref(), ka, p5);
            let yb = value_at(&x, bb.as_deref(), kb, p5);
            worst = worst.max((yb - ya).abs());
        }
        let got = analysis::db(worst.max(1e-300) / AMP);
        results.push(MeasurementResult::new(
            "FR-007",
            "level-change step − the script's",
            T_SPEED,
            6000,
            "speed_only",
            format!("stored {f:.0} Hz, level {a} → {b}: {got:.1} dB re the tone"),
            got - want,
            "dB",
            Tolerance::Range(-0.5, 0.5), // engineering target (013 R-03 pin)
        ));
    }
    results
}

/// Spec 009 FR-008's drain bound at the other levels (research R-12; post-implementation review
/// B7): the default engine, a 1 kHz tone at 0 dBFS recorded and played at each level on both
/// routes: the RMS of the last 1 ms is at most −80 dBFS.
pub fn drain_tail(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let x = stimulus::tone(1000.0, 1.0, fs, (fs * 0.5) as usize);
    let mut results = Vec::new();
    for (route, rname) in [
        (rr_dr60::Route::Speaker, "speaker"),
        (rr_dr60::Route::Earphone, "earphone"),
    ] {
        let mut p = make(configs::settings("speed_default", rate));
        p.start_recording().expect("start");
        let _ = p.record(&x, &mut []).expect("record");
        let _ = p.stop_recording(&mut []).expect("stop");
        p.set_route(route);
        for level in LEVELS {
            let y = play(&mut p, 1, (rate / 50) as usize, &[(0, level)]).out;
            let ms = (rate as usize / 1000).max(1);
            results.push(MeasurementResult::new(
                "FR-005",
                "last 1 ms of the drain (009 FR-008's bound)",
                T_SPEED,
                rate,
                "speed_default",
                format!("1 kHz 0 dBFS, {rname}, level {level}"),
                analysis::rms_dbfs(&y[y.len() - ms..]).max(-200.0),
                "dBFS",
                Tolerance::AtMost(-80.0), // 009 FR-008
            ));
        }
    }
    results
}

/// FR-006 (US1 AS4) at a host rate in the default engine (coach review R7): at level 3 the
/// per-call reports follow 0.12's rules: `finished` once, in the call where the produced total
/// reaches ⌈K · m ÷ l⌉ + the drain; Σ `frames` = F; `frame_index` = ⌊place ÷ 120⌋ (at most F − 1).
pub fn level3_reports(make: Make<'_>, rate: u32) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let x = stimulus::voiced(fs, 1.0, -20.0, 0x013);
    let mut p = make(configs::settings("speed_default", rate));
    p.start_recording().expect("start");
    let _ = p.record(&x, &mut []).expect("record");
    let _ = p.stop_recording(&mut []).expect("stop");
    let k = p.recording_length(1).expect("length");
    let frames = k / 120;
    let total = (k * u64::from(rate)).div_ceil(6000) + u64::from(p.drain_samples());
    let call = (rate / 50) as usize;
    let played = play(&mut p, 1, call, &[(0, 3)]);
    let bad_index = played
        .places
        .iter()
        .zip(&played.frame_indices)
        .filter(|(place, fi)| **fi != (**place / 120).min(frames - 1))
        .count();
    let mut results = Vec::new();
    for (property, measured, want) in [
        (
            "produced − ⌈K·m/l⌉ − drain",
            played.out.len() as f64 - total as f64,
            0.0,
        ),
        ("Σ frames − F", played.frames as f64 - frames as f64, 0.0),
        ("frame indices off 0.12's rule", bad_index as f64, 0.0),
        ("finished reports", played.finished as f64, 1.0),
    ] {
        results.push(MeasurementResult::new(
            "FR-006",
            property,
            T_SPEED,
            rate,
            "speed_default",
            "voiced, 1 s, level 3",
            measured,
            "",
            Tolerance::Exact(want), // FR-006: 0.12's reports
        ));
    }
    results
}
