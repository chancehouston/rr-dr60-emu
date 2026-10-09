//! Deterministic stimulus generators (tasks.md T020).
//!
//! Every stimulus is bit-reproducible on every platform: the math uses only
//! `rr_dr60_detmath` and integer arithmetic, never the platform math library. That's because
//! stimuli feed the golden files (FR-014, FR-021). This module stays under the clippy R-04 ban.

use rr_dr60_detmath::{TAU, exp, ln, sin};

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
