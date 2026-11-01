//! Deterministic stimulus generators (tasks.md T020).
//!
//! Every stimulus is bit-reproducible on every platform: the math uses only
//! `rr_dr60_detmath` and integer arithmetic, never the platform math library. That's because
//! stimuli feed the golden files (FR-014, FR-021). This module stays under the clippy R-04 ban.

use rr_dr60_detmath::{PI, TAU, bessel_i0, cos, exp, ln, sin, sqrt};

/// Fractional part of a non-negative value, using basic operations only.
fn frac(x: f64) -> f64 {
    x - (x as u64) as f64
}

/// One sample of `amp · sin(2π·freq·n/fs)`, in f64. The phase is reduced to one cycle before
/// calling `sin`, so long tones stay accurate.
pub fn tone_sample(freq_hz: f64, amp: f64, fs: f64, n: usize) -> f64 {
    amp * sin(TAU * frac(freq_hz * n as f64 / fs))
}

/// A sine tone `amp · sin(2π·freq·n/fs)`, n = 0..len.
pub fn tone(freq_hz: f64, amp: f64, fs: f64, len: usize) -> Vec<f32> {
    (0..len)
        .map(|n| tone_sample(freq_hz, amp, fs, n) as f32)
        .collect()
}

/// A square wave of `±amp` at `freq_hz`: a half-period of `round(fs / (2·freq_hz))` samples,
/// starting at `+amp` (spec 010 data-model.md › Stimuli; the golden's `square_1k_fs`). At 1 kHz
/// the half-period is 3 samples at 6 kHz, 4 at 8 kHz, 22 at 44.1 kHz and 24 at 48 kHz; a
/// whole-sample half-period keeps the stimulus defined at every host rate.
pub fn square(freq_hz: f64, amp: f32, fs: f64, len: usize) -> Vec<f32> {
    let half = (fs / (2.0 * freq_hz)).round().max(1.0) as usize;
    (0..len)
        .map(|n| if (n / half) % 2 == 0 { amp } else { -amp })
        .collect()
}

/// Spec 011 (research R-05): a 1 kHz cosine at `level_dbfs`, `len` samples, whose positive
/// peak falls on **output** sample `peak_at` of a path with `latency` host samples, i.e. on
/// input sample `peak_at − latency` (the coupling corners' phase lead at 1 kHz, 1.7 – 10.9°, is
/// not compensated). The click-energy stimulus of FR-004.
pub fn tone_peaking_at(
    level_dbfs: f64,
    rate: u32,
    len: usize,
    peak_at: usize,
    latency: usize,
) -> Vec<f32> {
    let (fs, amp) = (f64::from(rate), amplitude(level_dbfs));
    let at = peak_at as f64 - latency as f64;
    (0..len)
        .map(|n| (amp * cos(TAU * 1000.0 * (n as f64 - at) / fs)) as f32)
        .collect()
}

/// A tone at `level_dbfs` starting at `phase_rad`: `A · sin(2π·freq·n/fs + φ)`, n = 0..len,
/// bit-reproducible (detmath). The cycle count is reduced to one cycle before `sin`.
pub fn tone_at_phase(
    freq_hz: f64,
    level_dbfs: f64,
    phase_rad: f64,
    fs: f64,
    len: usize,
) -> Vec<f32> {
    let amp = amplitude(level_dbfs);
    (0..len)
        .map(|n| (amp * sin(TAU * frac(freq_hz * n as f64 / fs) + phase_rad)) as f32)
        .collect()
}

/// The starting phase that puts a device sample on the peak of a tone at `freq_hz` (spec 004
/// FR-012, research R-08): for a tone whose frequency divides the 6 kHz device rate (A-026),
/// such as 1 kHz with only 6 samples per cycle, a sample-peak detector reads up to 1.25 dB low
/// unless some device sample lands on the peak.
///
/// Derivation: the host-to-device decimator has a delay of `D = bypass_latency_samples / 2`
/// host samples (the rate conversion boundary is two equal linear-phase converters, 001 R-10;
/// `D = 0` at the 6 kHz host rate), so device sample `k` sees the input at time
/// `k / 6000 − D / host` and has phase `2π·f·k/6000 − 2π·f·D/host + φ`. Choosing
///
/// `φ = (π/2 + 2π·f·D/host) mod (2π·f/6000)`
///
/// puts the phase of some device sample at exactly π/2. At 6 kHz with f = 1 kHz this is π/6.
/// `bypass_latency_samples` is `Pipeline::new(bypass_all settings).latency_samples()` at the
/// host rate. The rule is an engineering target (004 FR-015).
pub fn aligned_phase(freq_hz: f64, host_hz: u32, bypass_latency_samples: u32) -> f64 {
    let d = f64::from(bypass_latency_samples) / 2.0;
    let period = TAU * freq_hz / f64::from(rr_dr60::DEVICE_RATE_HZ); // A-026
    let raw = TAU / 4.0 + TAU * freq_hz * d / f64::from(host_hz);
    raw - (raw / period) as u64 as f64 * period
}

/// The starting phase that puts a **host** sample on the positive peak of a tone at `freq_hz`
/// (spec 005 research R-08, plan review P8): `φ = (π/2) mod (2π·freq/host)`. Used by every
/// stimulus that feeds the limiter at the host-rate "after AGC" tap, where there is no device
/// grid: for 1 kHz this is π/6 at 6 kHz and 0 at 8, 16, 48 and 96 kHz, whereas spec 004's
/// [`aligned_phase`] gives π/6 at every rate and misses the 8 kHz peak by 15° (the ceiling then
/// reads 0.3 dB low). Chain tests at the default output keep [`aligned_phase`]. Engineering
/// target (005 FR-017).
pub fn peak_phase(freq_hz: f64, host_hz: u32) -> f64 {
    let period = TAU * freq_hz / f64::from(host_hz);
    let raw = PI / 2.0;
    raw - (raw / period) as u64 as f64 * period
}

/// The spec 005 FR-007 asymmetric stimulus: `sign · A · (sin ωt + 0.5 cos 2ωt)` at 1 kHz,
/// whose peaks are +0.75·A and −1.5·A (6 dB apart), with `A = 10^(level/20) / 1.5` so the
/// larger peak sits at `level_dbfs_of_larger_peak` and the smaller 6 dB lower. With `sign`
/// +1 the larger peak is negative (not sensed, A-036); with −1 it is positive (sensed).
/// `phase_rad` is the fundamental's starting phase: [`peak_phase`] puts a host sample on the
/// 270° peak (and on the 30° / 150° peaks) at every rate whose grid can hold it, where phase 0
/// misses the 6 kHz grid's peak by 30° and reads it 2.6 dB low (005 R-08). Bit-reproducible
/// (detmath).
pub fn asymmetric(
    level_dbfs_of_larger_peak: f64,
    sign: f64,
    fs: f64,
    len: usize,
    phase_rad: f64,
) -> Vec<f32> {
    let a = amplitude(level_dbfs_of_larger_peak) / 1.5; // 005 FR-007: peaks +0.75 / −1.5
    (0..len)
        .map(|n| {
            let theta = TAU * frac(1000.0 * n as f64 / fs) + phase_rad;
            (sign * a * (sin(theta) + 0.5 * cos(2.0 * theta))) as f32
        })
        .collect()
}

/// Two tones summed, each at its own level (dBFS), both at phase 0 (spec 005 FR-007
/// bandwidth test: 1 kHz at −20 dBFS plus 6 kHz at +6 dBFS).
pub fn two_tone(
    f1: f64,
    level1_dbfs: f64,
    f2: f64,
    level2_dbfs: f64,
    fs: f64,
    len: usize,
) -> Vec<f32> {
    let (a1, a2) = (amplitude(level1_dbfs), amplitude(level2_dbfs));
    (0..len)
        .map(|n| (tone_sample(f1, a1, fs, n) + tone_sample(f2, a2, fs, n)) as f32)
        .collect()
}

/// A phase-continuous 1 kHz level staircase: each level in `levels_dbfs` for `seg_s` seconds,
/// starting at `phase_rad` (spec 005 golden stimuli, research R-10; the AGC checks' steps).
pub fn staircase(levels_dbfs: &[f64], seg_s: f64, fs: f64, phase_rad: f64) -> Vec<f32> {
    let durations: Vec<f64> = levels_dbfs.iter().map(|_| seg_s).collect();
    step_at_phase(1000.0, levels_dbfs, &durations, phase_rad, fs)
}

/// A unit impulse: 1.0 at n = 0, otherwise 0.
pub fn impulse(len: usize) -> Vec<f32> {
    let mut x = vec![0.0; len];
    if let Some(first) = x.first_mut() {
        *first = 1.0;
    }
    x
}

/// Digital silence.
pub fn silence(len: usize) -> Vec<f32> {
    vec![0.0; len]
}

/// A constant (DC) signal.
pub fn dc(level: f32, len: usize) -> Vec<f32> {
    vec![level; len]
}

/// Phase in radians of sample `k` of an exponential sine sweep from `f0` to `f1` over `len`
/// samples: φ(t) = 2π·f0·L·(e^(t/L) − 1), with L = T / ln(f1/f0).
/// Accurate for sweeps of up to about 80 s (the range of `detmath::sin`).
pub fn log_sweep_phase(f0: f64, f1: f64, fs: f64, len: usize, k: usize) -> f64 {
    let duration = len as f64 / fs;
    let l = duration / ln(f1 / f0);
    TAU * f0 * l * (exp((k as f64 / fs) / l) - 1.0)
}

/// An exponential (log) sine sweep from `f0` to `f1` Hz, starting at phase 0.
pub fn log_sweep(f0: f64, f1: f64, amp: f64, fs: f64, len: usize) -> Vec<f32> {
    (0..len)
        .map(|k| (amp * sin(log_sweep_phase(f0, f1, fs, len, k))) as f32)
        .collect()
}

/// Amplitude of a sine at `level_dbfs` (AES17: 0 dBFS = peak 1.0), computed with detmath.
pub fn amplitude(level_dbfs: f64) -> f64 {
    exp(level_dbfs * ln(10.0) / 20.0)
}

/// A tone at `freq_hz` whose level steps through `levels_dbfs`, holding each for the matching
/// entry of `durations_s` (spec 002 step stimulus). The phase is continuous across steps.
/// Segment `i` has `(durations_s[i] · fs) as usize` samples.
pub fn step(freq_hz: f64, levels_dbfs: &[f64], durations_s: &[f64], fs: f64) -> Vec<f32> {
    assert_eq!(levels_dbfs.len(), durations_s.len());
    let mut out = Vec::new();
    for (&level, &secs) in levels_dbfs.iter().zip(durations_s) {
        let amp = amplitude(level);
        let start = out.len();
        out.extend(
            (start..start + (secs * fs) as usize).map(|n| tone_sample(freq_hz, amp, fs, n) as f32),
        );
    }
    out
}

/// [`step`] with a starting phase: `A_i · sin(2π·freq·n/fs + φ)` (spec 004 research R-08, applied
/// to the AGC timing step: with 6 device samples per 1 kHz cycle, the AGC's peak-hold branch
/// reads a phase-0 tone 1.25 dB low for the first 31 device samples after a step unless a
/// device sample lands on a peak). Segment `i` has `(durations_s[i] · fs) as usize` samples.
pub fn step_at_phase(
    freq_hz: f64,
    levels_dbfs: &[f64],
    durations_s: &[f64],
    phase_rad: f64,
    fs: f64,
) -> Vec<f32> {
    assert_eq!(levels_dbfs.len(), durations_s.len());
    let mut out = Vec::new();
    for (&level, &secs) in levels_dbfs.iter().zip(durations_s) {
        let amp = amplitude(level);
        let start = out.len();
        out.extend(
            (start..start + (secs * fs) as usize)
                .map(|n| (amp * sin(TAU * frac(freq_hz * n as f64 / fs) + phase_rad)) as f32),
        );
    }
    out
}

/// `cycles` repetitions of a tone burst at `level_dbfs` for `on_s` seconds followed by `off_s`
/// seconds of silence (spec 002 FR-009). The tone's phase follows the global sample index.
pub fn tone_bursts(
    freq_hz: f64,
    level_dbfs: f64,
    on_s: f64,
    off_s: f64,
    cycles: usize,
    fs: f64,
) -> Vec<f32> {
    let amp = amplitude(level_dbfs);
    let (on, off) = ((on_s * fs) as usize, (off_s * fs) as usize);
    let mut out = Vec::with_capacity(cycles * (on + off));
    for _ in 0..cycles {
        let start = out.len();
        out.extend((start..start + on).map(|n| tone_sample(freq_hz, amp, fs, n) as f32));
        out.extend(std::iter::repeat_n(0.0f32, off));
    }
    out
}

/// Samples in `secs` seconds at `fs`, rounded to the nearest sample, so millisecond lengths
/// such as 18 ms at 44.1 kHz don't lose a sample to floating-point truncation.
fn samples(secs: f64, fs: f64) -> usize {
    (secs * fs + 0.5) as usize
}

/// A burst-gap stimulus (spec 003 definitions): a tone at `level_dbfs` during each `(true, s)`
/// segment ("burst") and digital silence (+0.0) during each `(false, s)` segment ("gap").
/// Every burst starts at phase 0, so its timing doesn't depend on what came before.
/// Foundational for spec 003 (plan › Story boundaries).
pub fn burst_gap(freq_hz: f64, level_dbfs: f64, segments: &[(bool, f64)], fs: f64) -> Vec<f32> {
    let amp = amplitude(level_dbfs);
    let mut out = Vec::new();
    for &(on, secs) in segments {
        let n = samples(secs, fs);
        if on {
            out.extend((0..n).map(|k| tone_sample(freq_hz, amp, fs, k) as f32));
        } else {
            out.extend(std::iter::repeat_n(0.0f32, n));
        }
    }
    out
}

/// `lead_s` of silence, then one burst per entry of `bursts_ms`, each followed by `gap_s` of
/// silence (spec 003 FR-009 onset checks; `vas_short_bursts` golden stimulus).
pub fn short_bursts(
    freq_hz: f64,
    level_dbfs: f64,
    lead_s: f64,
    bursts_ms: &[f64],
    gap_s: f64,
    fs: f64,
) -> Vec<f32> {
    let mut segments = vec![(false, lead_s)];
    for &ms in bursts_ms {
        segments.push((true, ms / 1000.0));
        segments.push((false, gap_s));
    }
    burst_gap(freq_hz, level_dbfs, &segments, fs)
}

/// Seeded white noise band-limited to the 225–2550 Hz device band (A-027) and scaled to
/// `level_dbfs` by the AES17 convention (RMS = 10^(L/20) / √2), for spec 002 FR-009 (restated
/// by spec 004 FR-011) and spec 003 FR-006 (restated by 004 FR-012).
///
/// The band-pass is a Kaiser-windowed (β = 8) sinc FIR of about 10 ms, designed here with
/// detmath. It never uses `rr_dr60`'s own filters, so the stimulus does not depend on
/// signal-chain stage 4 (Principle VII). Filter length, window and band edges are engineering
/// targets (spec 002 R-15). Bit-reproducible on every platform.
pub fn bandlimited_noise(seed: u64, len: usize, fs: f64, level_dbfs: f64) -> Vec<f32> {
    let taps = ((fs * 0.01) as usize) | 1;
    let mid = (taps - 1) as f64 / 2.0;
    let (f1, f2) = (225.0 / fs, 2550.0 / fs); // A-027 device band; 002 FR-009 restated by 004 FR-011
    let lowpass = |fc: f64, m: f64| {
        if m == 0.0 {
            2.0 * fc
        } else {
            sin(2.0 * PI * fc * m) / (PI * m)
        }
    };
    let beta = 8.0;
    let i0_beta = bessel_i0(beta);
    let h: Vec<f64> = (0..taps)
        .map(|k| {
            let m = k as f64 - mid;
            let r = m / mid;
            let w = bessel_i0(beta * sqrt(1.0 - r * r)) / i0_beta;
            w * (lowpass(f2, m) - lowpass(f1, m))
        })
        .collect();
    let src = noise(seed, len + taps - 1);
    let filtered: Vec<f64> = (0..len)
        .map(|n| {
            h.iter()
                .enumerate()
                .map(|(k, &hk)| hk * f64::from(src[n + taps - 1 - k]))
                .fold(0.0, |acc, v| acc + v)
        })
        .collect();
    let mean_square = filtered.iter().fold(0.0, |acc, &v| acc + v * v) / len as f64;
    let target_rms = amplitude(level_dbfs) / sqrt(2.0);
    let scale = target_rms / sqrt(mean_square);
    filtered.iter().map(|&v| (v * scale) as f32).collect()
}

/// PCG32 (XSH-RR 64/32) random number generator, as in pcg-random.org's `pcg32_random_r`.
/// It uses integer arithmetic only, so it is bit-identical everywhere.
#[derive(Clone, Debug)]
pub struct Pcg32 {
    state: u64,
    inc: u64,
}

impl Pcg32 {
    const MUL: u64 = 6_364_136_223_846_793_005;

    /// Seeds the generator like `pcg32_srandom_r(seed, stream)`.
    pub fn new(seed: u64, stream: u64) -> Self {
        let mut r = Self {
            state: 0,
            inc: (stream << 1) | 1,
        };
        r.next_u32();
        r.state = r.state.wrapping_add(seed);
        r.next_u32();
        r
    }

    /// Next 32-bit output.
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(Self::MUL).wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        xorshifted.rotate_right((old >> 59) as u32)
    }

    /// Uniform integer in `0..bound` (bound > 0), using a simple modulo (slight bias is fine
    /// for test block sizes).
    pub fn below(&mut self, bound: u32) -> u32 {
        self.next_u32() % bound
    }
}

/// Seeded uniform white noise in [−0.5, 0.5), from PCG32 stream 0. Each sample is
/// `(u >> 8) · 2⁻²⁴ − 0.5`, which is exact in f32.
pub fn noise(seed: u64, len: usize) -> Vec<f32> {
    let mut r = Pcg32::new(seed, 0);
    (0..len)
        .map(|_| (r.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0) - 0.5)
        .collect()
}

/// A synthetic **voiced** speech-like stimulus for the speech codec checks (spec 008 FR-005;
/// research R-09): a glottal pulse train whose period glides exponentially 90 → 220 → 90 Hz
/// over the duration, through three time-varying two-pole resonators (centres sweeping
/// 300 → 750, 1100 → 1900 and 2300 → 2600 Hz, bandwidths 80 – 150 Hz; the third sits at the
/// band's top edge, as a real third formant would), plus aspiration noise (`Pcg32(seed)`,
/// shaped by the same resonators) 20 dB below the harmonics; scaled to `level_dbfs` RMS.
///
/// Generated directly at `fs` from the same formulas (detmath only), so it is deterministic
/// at every rate; the codec checks compare the codec's output with the same stimulus through
/// the same configuration with the codec bypassed, so no cross-rate identity is needed.
/// Synthetic: it is not speech and makes no claim about speech.
pub fn voiced(fs: f64, seconds: f64, level_dbfs: f64, seed: u64) -> Vec<f32> {
    voiced_with_f0(fs, seconds, level_dbfs, seed, None)
}

/// [`voiced`] with the pitch glide replaced by a steady fundamental `f0_hz` when given (spec
/// 023 research R-09: the steady vowel at 130 Hz is the glide generator at a fixed f0; the
/// formant sweep is unchanged). `None` keeps the glide.
pub fn voiced_with_f0(
    fs: f64,
    seconds: f64,
    level_dbfs: f64,
    seed: u64,
    f0_hz: Option<f64>,
) -> Vec<f32> {
    let len = (fs * seconds) as usize;
    let mut rng = Pcg32::new(seed, 7);
    let mut res = [Resonator::default(); 3];
    let mut phase = 0.0f64; // glottal cycle phase in 0..1
    let mut out = Vec::with_capacity(len);
    let mut pulse_env = 0.0f64;
    for n in 0..len {
        let t = n as f64 / fs;
        let u = t / seconds; // 0..1
        // Pitch glide 90 → 220 → 90 Hz, exponential (one octave and a third each way).
        let f0 = f0_hz.unwrap_or_else(|| glide_f0(u));
        phase += f0 / fs;
        let mut excitation = 0.0;
        if phase >= 1.0 {
            phase -= 1.0;
            pulse_env = 1.0;
        }
        // A two-sided exponential glottal pulse: a sharp rise decaying over ~1 ms.
        excitation += pulse_env;
        pulse_env *= exp(-1.0 / (fs * 0.0008));
        // Aspiration noise 20 dB below the harmonics (the pulse train's RMS ≈ 0.25 at this
        // decay), shaped by the same resonators.
        let noise = ((rng.next_u32() >> 8) as f64 * (1.0 / 16_777_216.0) - 0.5) * 2.0;
        excitation += noise * 0.1 * 0.25;
        // Formant-like resonators with slowly moving centres.
        let centres = [300.0 + 450.0 * u, 1100.0 + 800.0 * u, 2300.0 + 300.0 * u];
        let bws = [80.0, 110.0, 150.0];
        let mut y = 0.0;
        for (r, (&fc, &bw)) in res.iter_mut().zip(centres.iter().zip(bws.iter())) {
            y += r.step(excitation, fc, bw, fs);
        }
        out.push(y);
    }
    scale_to_level(&out, level_dbfs)
}

/// The glide's fundamental at fraction `u` (0 … 1) of the voiced stimulus: 90 → 220 → 90 Hz,
/// exponential (one octave and a third each way).
pub fn glide_f0(u: f64) -> f64 {
    let u = u.clamp(0.0, 1.0);
    let tri = if u < 0.5 { 2.0 * u } else { 2.0 - 2.0 * u };
    90.0 * exp(tri * ln(220.0 / 90.0))
}

/// A synthetic **unvoiced-to-voiced** sequence (spec 008 FR-005, FR-006): 300 ms of resonator-
/// shaped noise at −10 dB relative to the voiced part, a 50 ms ramp, then the voiced stimulus,
/// repeated until `seconds`; scaled so the voiced part sits at `level_dbfs` RMS. Used for the
/// level-tracking check (no pumping or dropouts).
pub fn unvoiced_voiced(fs: f64, seconds: f64, level_dbfs: f64, seed: u64) -> Vec<f32> {
    let len = (fs * seconds) as usize;
    let cycle_s = 1.35; // 0.3 s noise + 0.05 s ramp + 1.0 s voiced
    let voiced_part = voiced(fs, 1.0, level_dbfs, seed);
    let mut rng = Pcg32::new(seed ^ 0x5151, 9);
    let mut res = [Resonator::default(); 3];
    let noise_gain = amplitude(level_dbfs - 10.0) / sqrt(2.0);
    let mut out = Vec::with_capacity(len);
    let mut n_in_cycle = 0usize;
    let cycle_len = (fs * cycle_s) as usize;
    let noise_len = (fs * 0.3) as usize;
    let ramp_len = (fs * 0.05) as usize;
    let mut noise_acc = Vec::with_capacity(noise_len + ramp_len);
    for _ in 0..len {
        if n_in_cycle == 0 {
            noise_acc.clear();
            for _ in 0..noise_len + ramp_len {
                let x = ((rng.next_u32() >> 8) as f64 * (1.0 / 16_777_216.0) - 0.5) * 2.0;
                let mut y = 0.0;
                for (r, (&fc, &bw)) in res.iter_mut().zip(
                    [1500.0, 2200.0, 2600.0]
                        .iter()
                        .zip([300.0, 400.0, 300.0].iter()),
                ) {
                    y += r.step(x, fc, bw, fs);
                }
                noise_acc.push(y);
            }
            let ms = noise_acc.iter().fold(0.0, |a, &v| a + v * v) / noise_acc.len() as f64;
            let k = noise_gain / sqrt(ms.max(1e-30));
            for v in &mut noise_acc {
                *v *= k;
            }
        }
        let v = if n_in_cycle < noise_len {
            noise_acc[n_in_cycle]
        } else if n_in_cycle < noise_len + ramp_len {
            let a = (n_in_cycle - noise_len) as f64 / ramp_len as f64;
            noise_acc[n_in_cycle] * (1.0 - a)
                + f64::from(voiced_part[(n_in_cycle - noise_len) % voiced_part.len()]) * a
        } else {
            let i = n_in_cycle - noise_len;
            f64::from(voiced_part[i % voiced_part.len()])
        };
        out.push(v as f32);
        n_in_cycle += 1;
        if n_in_cycle >= cycle_len {
            n_in_cycle = 0;
        }
    }
    out
}

/// A two-pole resonator with a per-sample centre and bandwidth (detmath only).
#[derive(Clone, Copy, Debug, Default)]
struct Resonator {
    y1: f64,
    y2: f64,
}

impl Resonator {
    fn step(&mut self, x: f64, fc_hz: f64, bw_hz: f64, fs: f64) -> f64 {
        let r = exp(-PI * bw_hz / fs);
        let theta = TAU * fc_hz / fs;
        let a1 = 2.0 * r * cos(theta);
        let a2 = -r * r;
        let gain = 1.0 - r;
        let y = gain * x + a1 * self.y1 + a2 * self.y2;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// Scales `x` to `level_dbfs` RMS (0 dBFS = a full-scale sine, RMS 1/√2).
fn scale_to_level(x: &[f64], level_dbfs: f64) -> Vec<f32> {
    let ms = x.iter().fold(0.0, |a, &v| a + v * v) / x.len().max(1) as f64;
    let k = amplitude(level_dbfs) / sqrt(2.0) / sqrt(ms.max(1e-30));
    x.iter().map(|&v| (v * k) as f32).collect()
}

// ---- Spec 013: stored stimuli (data-model.md › Stimuli) ------------------------------------

/// Spec 013: a stored tone of `amp` at `freq_hz`, `len` device samples at 6 kHz, starting at
/// phase 0 (`tone` at the device rate). `stored_tone(1000.0, 0.5, n)` is `stored_tone_1k`
/// (−6 dBFS).
pub fn stored_tone(freq_hz: f64, amp: f64, len: usize) -> Vec<f32> {
    tone(freq_hz, amp, 6000.0, len)
}

/// Spec 013: a stored impulse of 1.0 at device sample `j` in `len` samples (FR-008).
pub fn stored_impulse(j: usize, len: usize) -> Vec<f32> {
    let mut x = vec![0.0; len];
    x[j] = 1.0;
    x
}

/// Spec 013: a stored 1 kHz tone at −6 dBFS whose last sample is full scale, so the speed
/// control's tail is audible if it were cut (FR-005, research R-04).
pub fn stored_end_loud(len: usize) -> Vec<f32> {
    let mut x = stored_tone(1000.0, 0.5, len);
    if let Some(last) = x.last_mut() {
        *last = 1.0;
    }
    x
}

/// Spec 013: the voiced stimulus at the device rate (`voiced_m20` at 6 kHz), `seconds` long.
pub fn stored_voiced(seconds: f64) -> Vec<f32> {
    voiced(6000.0, seconds, -20.0, 0x013) // seed: spec 013 (contracts/golden-format.md)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec 011 T010: the input peaks at `peak_at − latency`.
    #[test]
    fn tone_peaking_at_places_the_peak() {
        for (rate, peak_at, latency) in [
            (6000u32, 3000usize, 181usize),
            (44_100, 22_050, 1771),
            (48_000, 24_000, 1923),
        ] {
            let x = tone_peaking_at(0.0, rate, 2 * peak_at, peak_at, latency);
            let i = peak_at - latency;
            assert_eq!(x[i], 1.0, "{rate} Hz");
            assert!(x[i - 1] < x[i] && x[i + 1] < x[i], "{rate} Hz");
        }
        let quiet = tone_peaking_at(-30.0, 48_000, 100, 10, 0);
        assert!((f64::from(quiet[10]) - amplitude(-30.0)).abs() < 1e-7);
    }

    #[test]
    fn tone_length_and_peak() {
        let amp = 0.1;
        let x = tone(1000.0, amp, 48_000.0, 48_000);
        assert_eq!(x.len(), 48_000);
        let peak = x.iter().fold(0.0f32, |m, &v| m.max(v.abs()));
        // The output is f32, so "within 1e-12" (T015) applies to the f64 generator; the f32
        // samples are within f32 rounding of the amplitude.
        assert!(
            (f64::from(peak) - amp).abs() <= amp * f64::from(f32::EPSILON),
            "peak {peak}"
        );
        let peak64 = (0..48_000)
            .map(|n| tone_sample(1000.0, amp, 48_000.0, n).abs())
            .fold(0.0, f64::max);
        assert!((peak64 - amp).abs() <= 1e-12, "f64 peak {peak64}");
        assert_eq!(x[0], 0.0);
    }

    #[test]
    fn impulse_silence_dc() {
        let i = impulse(8);
        assert_eq!(i[0], 1.0);
        assert!(i[1..].iter().all(|&v| v == 0.0));
        assert!(silence(16).iter().all(|&v| v == 0.0));
        assert!(dc(-0.5, 16).iter().all(|&v| v == -0.5));
        assert_eq!(impulse(0).len(), 0);
    }

    #[test]
    fn pcg32_known_answer() {
        // Published pcg32-demo output for seed 42, stream 54 (pcg-random.org reference).
        let mut r = Pcg32::new(42, 54);
        let want = [
            0xa15c_02b7,
            0x7b47_f409,
            0xba1d_3330,
            0x83d2_f293,
            0xbfa4_784b,
            0xcbed_606e,
        ];
        for w in want {
            assert_eq!(r.next_u32(), w);
        }
    }

    #[test]
    fn pcg32_seed_0d60_matches_reference_algorithm() {
        // pcg32_srandom_r / pcg32_random_r written out from the published constants.
        let mul: u64 = 6_364_136_223_846_793_005;
        let stream: u64 = 0;
        let inc: u64 = (stream << 1) | 1; // pcg32_srandom_r: inc = (initseq << 1) | 1
        let mut state: u64 = 0;
        let step = |state: &mut u64| {
            let old = *state;
            *state = old.wrapping_mul(mul).wrapping_add(inc);
            let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
            let rot = (old >> 59) as u32;
            xorshifted.rotate_right(rot)
        };
        step(&mut state);
        state = state.wrapping_add(0x0D60);
        step(&mut state);
        let mut r = Pcg32::new(0x0D60, 0);
        for _ in 0..4 {
            assert_eq!(r.next_u32(), step(&mut state));
        }
    }

    #[test]
    fn noise_range_and_determinism() {
        let a = noise(0x0D60, 10_000);
        assert!(a.iter().all(|&v| (-0.5..0.5).contains(&v)));
        assert_eq!(a, noise(0x0D60, 10_000));
        assert_ne!(a, noise(0x0D61, 10_000));
    }

    /// 002 T009: level steps have exact segment lengths and amplitudes.
    #[test]
    fn step_lengths_and_levels() {
        let fs = 8000.0;
        let x = step(1000.0, &[-40.0, -10.0], &[0.5, 0.25], fs);
        assert_eq!(x.len(), 4000 + 2000);
        let peak = |s: &[f32]| s.iter().fold(0.0f32, |m, &v| m.max(v.abs()));
        assert!((f64::from(peak(&x[..4000])) - 0.01).abs() < 1e-6);
        assert!((f64::from(peak(&x[4000..])) - 0.316_227_766).abs() < 1e-6);
        assert_eq!(x, step(1000.0, &[-40.0, -10.0], &[0.5, 0.25], fs));
    }

    /// 002 T009: bursts are on/off with exact counts.
    #[test]
    fn tone_burst_layout() {
        let x = tone_bursts(1000.0, -10.0, 0.1, 0.4, 2, 8000.0);
        assert_eq!(x.len(), 2 * (800 + 3200));
        assert!(x[800..4000].iter().all(|&v| v == 0.0));
        assert!(x[..800].iter().any(|&v| v != 0.0));
        assert!(x[4800..].iter().all(|&v| v == 0.0));
    }

    /// 002 T009: band-limited noise hits its AES17 level and is bit-reproducible.
    #[test]
    fn bandlimited_noise_level_and_determinism() {
        let fs = 48_000.0;
        let a = bandlimited_noise(0x0D60, 48_000, fs, -70.0);
        assert_eq!(a.len(), 48_000);
        let mean_square =
            a.iter().map(|&v| f64::from(v) * f64::from(v)).sum::<f64>() / a.len() as f64;
        // AES17: level = 20·log10(RMS·√2) = 10·log10(2·mean square).
        let level = 10.0 * rr_dr60_detmath::ln(2.0 * mean_square) / rr_dr60_detmath::ln(10.0);
        assert!((level + 70.0).abs() < 0.05, "level {level}");
        assert_eq!(a, bandlimited_noise(0x0D60, 48_000, fs, -70.0));
    }

    /// 003 T007: burst-gap segments have exact (rounded) lengths, bursts start at phase 0, gaps
    /// are digital silence (+0.0), and the result is bit-reproducible.
    #[test]
    fn burst_gap_layout() {
        for fs in [8000.0, 44_100.0, 48_000.0] {
            let seg = [(true, 1.0), (false, 0.018), (true, 0.5), (false, 2.0)];
            let x = burst_gap(1000.0, -8.0, &seg, fs);
            let n = |s: f64| (s * fs + 0.5) as usize;
            assert_eq!(x.len(), n(1.0) + n(0.018) + n(0.5) + n(2.0), "{fs}");
            let b2 = n(1.0) + n(0.018);
            assert_eq!(x[0], 0.0);
            assert_eq!(x[b2], 0.0, "second burst starts at phase 0");
            assert!(x[n(1.0)..b2].iter().all(|v| v.to_bits() == 0));
            assert!(x[b2 + n(0.5)..].iter().all(|v| v.to_bits() == 0));
            let peak = x[..n(1.0)].iter().fold(0.0f32, |m, &v| m.max(v.abs()));
            assert!(
                (f64::from(peak) - amplitude(-8.0)).abs() < 1e-3, // engineering target (003 R-15)
                "{fs}: {peak}"
            );
            assert_eq!(x, burst_gap(1000.0, -8.0, &seg, fs));
        }
        assert_eq!(
            burst_gap(1000.0, -8.0, &[(true, 0.018)], 44_100.0).len(),
            794
        );
    }

    /// 003 T007: short bursts are lead + (burst + gap) × n with exact counts.
    #[test]
    fn short_bursts_layout() {
        let x = short_bursts(1000.0, -8.0, 2.0, &[10.0, 30.0, 200.0], 2.0, 8000.0);
        assert_eq!(x.len(), 16_000 + 80 + 16_000 + 240 + 16_000 + 1600 + 16_000);
        assert!(x[..16_000].iter().all(|&v| v == 0.0));
        assert!(x[16_000..16_080].iter().any(|&v| v != 0.0));
        assert!(x[16_080..32_080].iter().all(|&v| v == 0.0));
        assert_eq!(
            x,
            short_bursts(1000.0, -8.0, 2.0, &[10.0, 30.0, 200.0], 2.0, 8000.0)
        );
    }

    /// 004 T032 (research R-08): the phase rule puts a device sample on the peak.
    #[test]
    fn aligned_phase_rule() {
        // At the 6 kHz identity rate, 1 kHz: π/2 mod π/3 = π/6 (the spec's example).
        assert!((aligned_phase(1000.0, 6000, 0) - PI / 6.0).abs() < 1e-12);
        // Formula check at 48 kHz with a hypothetical bypass latency of 462 samples (D = 231;
        // the realized 48 kHz figure is 476):
        // (0.25 + 1000·231/48000) cycles mod 1/6 cycle = 1.0625 mod 0.1667 = 0.0625 cycles = 22.5°.
        let phi = aligned_phase(1000.0, 48_000, 462);
        assert!((phi - TAU * 0.0625).abs() < 1e-12, "{phi}");
        // The generated tone, seen on the device grid (host index 8k − D), peaks exactly.
        let x = tone_at_phase(1000.0, 0.0, phi, 48_000.0, 48_000);
        let peak = (0..6000)
            .filter_map(|k: usize| (8 * k).checked_sub(231))
            .map(|i| f64::from(x[i]))
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((peak - 1.0).abs() < 1e-7, "device-grid peak {peak}");
        // Without alignment, 1 kHz at phase 0 on the 6 kHz grid never reaches the peak.
        let x0 = tone(1000.0, 1.0, 6000.0, 6000);
        let peak0 = x0.iter().fold(0.0f32, |m, &v| m.max(v.abs()));
        assert!((f64::from(peak0) - 0.866).abs() < 1e-3, "{peak0}");
        let xa = tone_at_phase(1000.0, 0.0, aligned_phase(1000.0, 6000, 0), 6000.0, 6000);
        let peaka = xa.iter().fold(0.0f32, |m, &v| m.max(v.abs()));
        assert!((f64::from(peaka) - 1.0).abs() < 1e-7, "{peaka}");
        // Phase 0 reproduces `tone` and `step`.
        assert_eq!(
            tone_at_phase(440.0, 0.0, 0.0, 48_000.0, 100),
            tone(440.0, 1.0, 48_000.0, 100)
        );
        assert_eq!(
            step_at_phase(1000.0, &[-40.0, -10.0], &[0.5, 0.25], 0.0, 6000.0),
            step(1000.0, &[-40.0, -10.0], &[0.5, 0.25], 6000.0)
        );
        let x = step_at_phase(1000.0, &[0.0], &[0.01], PI / 6.0, 6000.0);
        assert!((f64::from(x[1]) - 1.0).abs() < 1e-7, "{}", x[1]);
    }

    /// 005 T020 (research R-08): the host-grid peak phase, the asymmetric stimulus's peaks,
    /// the two-tone sum and the staircase.
    #[test]
    fn peak_phase_asymmetric_two_tone_and_staircase() {
        assert!((peak_phase(1000.0, 6000) - PI / 6.0).abs() < 1e-12);
        assert_eq!(peak_phase(1000.0, 8000), 0.0);
        assert_eq!(peak_phase(1000.0, 48_000), 0.0);
        assert_eq!(peak_phase(1000.0, 96_000), 0.0);
        for rate in [6000u32, 8000, 16_000, 44_100, 48_000, 88_200, 96_000] {
            let x = tone_at_phase(
                1000.0,
                0.0,
                peak_phase(1000.0, rate),
                f64::from(rate),
                rate as usize,
            );
            let peak = x.iter().fold(f32::MIN, |m, &v| m.max(v));
            assert!(
                (f64::from(peak) - 1.0).abs() < 1e-6,
                "{rate} Hz: peak {peak}"
            );
        }
        let fs = 48_000.0;
        let x = asymmetric(4.0, 1.0, fs, 48_000, 0.0);
        let max = x.iter().fold(f32::MIN, |m, &v| m.max(v));
        let min = x.iter().fold(f32::MAX, |m, &v| m.min(v));
        assert!(
            (f64::from(max) - amplitude(4.0) * 0.5).abs() < 1e-6,
            "max {max}"
        );
        assert!((f64::from(min) + amplitude(4.0)).abs() < 1e-6, "min {min}");
        let y = asymmetric(4.0, -1.0, fs, 48_000, 0.0);
        assert!(x.iter().zip(&y).all(|(a, b)| *a == -*b));
        let z = asymmetric(4.0, 1.0, 6000.0, 6000, peak_phase(1000.0, 6000));
        let min6 = z.iter().fold(f32::MAX, |m, &v| m.min(v));
        assert!(
            (f64::from(min6) + amplitude(4.0)).abs() < 1e-6,
            "6 kHz min {min6}"
        );
        let t = two_tone(1000.0, -20.0, 6000.0, 6.0, fs, 48_000);
        let peak = t.iter().fold(0.0f32, |m, &v| m.max(v.abs()));
        assert!(
            (f64::from(peak) - (amplitude(-20.0) + amplitude(6.0))).abs() < 0.01,
            "{peak}"
        );
        let s = staircase(&[-20.0, 0.0, 3.0], 0.4, 6000.0, 0.0);
        assert_eq!(s.len(), 3 * 2400);
        assert_eq!(
            s,
            step(1000.0, &[-20.0, 0.0, 3.0], &[0.4, 0.4, 0.4], 6000.0)
        );
    }

    #[test]
    fn log_sweep_phase_and_instantaneous_frequency() {
        let (f0, f1, fs, n) = (20.0, 21_600.0, 48_000.0, 48_000);
        let x = log_sweep(f0, f1, 0.25, fs, n);
        assert_eq!(x.len(), n);
        assert_eq!(log_sweep_phase(f0, f1, fs, n, 0), 0.0);
        assert_eq!(x[0], 0.0);
        let inst = |k: usize| {
            (log_sweep_phase(f0, f1, fs, n, k + 1) - log_sweep_phase(f0, f1, fs, n, k)) * fs
                / rr_dr60_detmath::TAU
        };
        assert!((inst(0) / f0 - 1.0).abs() < 0.01, "start {}", inst(0));
        assert!((inst(n - 2) / f1 - 1.0).abs() < 0.01, "end {}", inst(n - 2));
    }

    /// Spec 008 FR-005 (T004): the speech-like stimuli are deterministic, sit at their level,
    /// and populate every third-octave band from 315 to 2000 Hz.
    #[test]
    fn voiced_stimulus_level_determinism_and_bands() {
        for &fs in &[6000.0, 48_000.0] {
            let a = voiced(fs, 2.0, -20.0, 0x0D60);
            let b = voiced(fs, 2.0, -20.0, 0x0D60);
            assert_eq!(a, b, "same seed, same samples at {fs} Hz");
            let ms = a.iter().map(|&v| f64::from(v) * f64::from(v)).sum::<f64>() / a.len() as f64;
            let level = 10.0 * ln(ms * 2.0) / ln(10.0);
            assert!(
                (level + 20.0).abs() < 0.1,
                "RMS level {level:.2} dBFS at {fs} Hz"
            );
            let bands = crate::analysis::third_octave_levels(&a, fs);
            let peak = bands.iter().cloned().fold(f64::MIN, f64::max);
            for (i, &b) in bands.iter().enumerate().take(9) {
                assert!(
                    peak - b < 40.0,
                    "band {i} is {:.1} dB below the peak at {fs} Hz",
                    peak - b
                );
            }
        }
        let c = voiced(6000.0, 1.0, -20.0, 1);
        let d = voiced(6000.0, 1.0, -20.0, 2);
        assert_ne!(c, d, "different seeds differ (the aspiration noise)");
        let uv = unvoiced_voiced(6000.0, 3.0, -20.0, 0x0D60);
        assert_eq!(uv.len(), 18_000);
        assert_eq!(uv, unvoiced_voiced(6000.0, 3.0, -20.0, 0x0D60));
    }

    /// Spec 010 T002: the square wave's half-period rule and its first sample.
    #[test]
    fn square_half_period_and_first_sample() {
        for (fs, half) in [
            (6000.0, 3usize),
            (8000.0, 4),
            (44_100.0, 22),
            (48_000.0, 24),
        ] {
            let x = square(1000.0, 1.0, fs, 4 * half);
            assert_eq!(x[0], 1.0, "{fs}");
            assert!(x[..half].iter().all(|&v| v == 1.0), "{fs}");
            assert!(x[half..2 * half].iter().all(|&v| v == -1.0), "{fs}");
            assert_eq!(x[2 * half], 1.0, "{fs}");
        }
        assert_eq!(square(1000.0, 0.125, 48_000.0, 48)[24], -0.125);
    }
}
