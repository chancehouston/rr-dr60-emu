//! Deterministic stimulus generators (tasks.md T020).
//!
//! Every stimulus is bit-reproducible on every platform: the math uses only
//! `rr_dr60_detmath` and integer arithmetic, never the platform math library. That's because
//! stimuli feed the golden files (FR-014, FR-021). This module stays under the clippy R-04 ban.

use rr_dr60_detmath::{PI, TAU, bessel_i0, exp, ln, sin, sqrt};

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

/// Seeded white noise band-limited to 300–3400 Hz and scaled to `level_dbfs` by the AES17
/// convention (RMS = 10^(L/20) / √2), for spec 002 FR-009.
///
/// The band-pass is a Kaiser-windowed (β = 8) sinc FIR of about 10 ms, designed here with
/// detmath. It never uses `rr_dr60`'s own filters, so the stimulus does not depend on
/// signal-chain stage 4 (Principle VII). Filter length, window and band edges are engineering
/// targets (spec 002 R-15). Bit-reproducible on every platform.
pub fn bandlimited_noise(seed: u64, len: usize, fs: f64, level_dbfs: f64) -> Vec<f32> {
    let taps = ((fs * 0.01) as usize) | 1;
    let mid = (taps - 1) as f64 / 2.0;
    let (f1, f2) = (300.0 / fs, 3400.0 / fs);
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

#[cfg(test)]
mod tests {
    use super::*;

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
                (f64::from(peak) - amplitude(-8.0)).abs() < 1e-3,
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
}
