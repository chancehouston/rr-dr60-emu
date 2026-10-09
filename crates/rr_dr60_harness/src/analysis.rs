//! Response analysis: single-bin DFT, group delay, power ratios (tasks.md T021).
//!
//! Analysis has tolerances and doesn't need to be bit-identical, so this module (and only this
//! module) may use the platform math library (tasks.md T021).
#![allow(clippy::disallowed_methods)]

use std::f64::consts::TAU;

pub use rustfft::num_complex::Complex64;

/// Complex amplitude `A·e^(jφ)` of the component `A·cos(ω·m + φ)` at `freq_hz` in
/// `x[start..start + len]`, where m counts from `start`.
///
/// It uses a Hann-weighted least-squares fit of a cosine and a sine at the exact frequency.
/// That is exact for a pure sinusoid of any length (no whole-number-of-cycles requirement), and
/// the window strongly suppresses leakage from other components.
pub fn single_bin<T: Copy + Into<f64>>(
    x: &[T],
    freq_hz: f64,
    fs: f64,
    start: usize,
    len: usize,
) -> Complex64 {
    let w = TAU * freq_hz / fs;
    let (mut scc, mut sss, mut scs, mut sxc, mut sxs) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (m, &v) in x[start..start + len].iter().enumerate() {
        let win = 0.5 - 0.5 * (TAU * (m as f64 + 0.5) / len as f64).cos();
        let (s, c) = (w * m as f64).sin_cos();
        let v: f64 = v.into();
        scc += win * c * c;
        sss += win * s * s;
        scs += win * c * s;
        sxc += win * v * c;
        sxs += win * v * s;
    }
    let det = scc * sss - scs * scs;
    let a = (sxc * sss - sxs * scs) / det;
    let b = (sxs * scc - sxc * scs) / det;
    // x ≈ a·cos + b·sin = A·cos(ωm + φ) with a = A·cos φ and b = −A·sin φ.
    Complex64::new(a, -b)
}

/// Complex gain `output / input` at `freq_hz`, measured over `settle..len` (both signals the
/// same length).
pub fn gain_and_phase<T: Copy + Into<f64>, U: Copy + Into<f64>>(
    input: &[T],
    output: &[U],
    freq_hz: f64,
    fs: f64,
    settle: usize,
) -> Complex64 {
    let len = input.len().min(output.len()) - settle;
    single_bin(output, freq_hz, fs, settle, len) / single_bin(input, freq_hz, fs, settle, len)
}

/// Gain in dB and phase in radians of `output / input` at `freq_hz` (see [`gain_and_phase`]).
pub fn gain_db_and_phase<T: Copy + Into<f64>, U: Copy + Into<f64>>(
    input: &[T],
    output: &[U],
    freq_hz: f64,
    fs: f64,
    settle: usize,
) -> (f64, f64) {
    let h = gain_and_phase(input, output, freq_hz, fs, settle);
    (db(h.norm()), h.arg())
}

/// Group delay in samples at `freq_hz`, by central difference of the phase of `measure` (a
/// complex gain as a function of frequency) at `freq_hz ± 1 Hz`.
pub fn group_delay_samples(measure: impl Fn(f64) -> Complex64, freq_hz: f64, fs: f64) -> f64 {
    let lo = measure(freq_hz - 1.0);
    let hi = measure(freq_hz + 1.0);
    let dphi = (hi / lo).arg(); // wrapped to (−π, π]
    -dphi / (TAU * 2.0 / fs)
}

/// Output power relative to input power in dB, over `settle..` (includes every frequency, so
/// alias and image products count).
pub fn power_ratio_db<T: Copy + Into<f64>, U: Copy + Into<f64>>(
    input: &[T],
    output: &[U],
    settle: usize,
) -> f64 {
    let energy = |s: &mut dyn Iterator<Item = f64>| s.map(|v| v * v).sum::<f64>();
    let pin = energy(&mut input[settle..].iter().map(|&v| v.into()));
    let pout = energy(&mut output[settle..].iter().map(|&v| v.into()));
    10.0 * (pout / pin).log10()
}

/// Amplitude ratio to decibels.
pub fn db(ratio: f64) -> f64 {
    20.0 * ratio.log10()
}

/// Decibels to amplitude ratio.
pub fn from_db(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sinusoid(amp: f64, phase: f64, freq: f64, fs: f64, n: usize) -> Vec<f64> {
        (0..n)
            .map(|i| amp * (TAU * freq * i as f64 / fs + phase).cos())
            .collect()
    }

    #[test]
    fn single_bin_recovers_amplitude_and_phase() {
        let (fs, f) = (48_000.0, 1000.0);
        let x = sinusoid(0.3, 0.7, f, fs, 24_000);
        let c = single_bin(&x, f, fs, 0, x.len());
        assert!((c.norm() / 0.3 - 1.0).abs() < 1e-9, "amp {}", c.norm());
        assert!((c.arg() - 0.7).abs() < 1e-9, "phase {}", c.arg());
        // Non-integer number of cycles: the Hann window keeps leakage negligible.
        let y = sinusoid(0.3, -1.1, 997.3, 44_100.0, 22_050);
        let c = single_bin(&y, 997.3, 44_100.0, 0, y.len());
        assert!((c.norm() / 0.3 - 1.0).abs() < 1e-9 && (c.arg() + 1.1).abs() < 1e-9);
    }

    #[test]
    fn group_delay_of_pure_delay() {
        let fs = 48_000.0;
        let delay = 37;
        let measure = |f: f64| {
            let x: Vec<f64> = sinusoid(0.5, 0.0, f, fs, 48_000);
            let mut y = vec![0.0; x.len()];
            y[delay..].copy_from_slice(&x[..x.len() - delay]);
            gain_and_phase(&x, &y, f, fs, 1000)
        };
        let d = group_delay_samples(measure, 1000.0, fs);
        assert!((d - 37.0).abs() < 0.01, "delay {d}");
    }

    #[test]
    fn power_ratio_and_db() {
        let x = sinusoid(1.0, 0.0, 440.0, 48_000.0, 4800);
        let y: Vec<f64> = x.iter().map(|v| 0.5 * v).collect();
        assert!((power_ratio_db(&x, &y, 0) - (-6.020_599_913_279_624)).abs() < 1e-6);
        for v in [-120.0, -6.0, 0.0, 3.5] {
            assert!((db(from_db(v)) - v).abs() < 1e-12);
        }
    }

    #[test]
    fn gain_db_matches_ratio() {
        let x = sinusoid(1.0, 0.0, 1000.0, 48_000.0, 48_000);
        let y: Vec<f64> = x.iter().map(|v| 0.1 * v).collect();
        let (g, ph) = gain_db_and_phase(&x, &y, 1000.0, 48_000.0, 0);
        assert!((g + 20.0).abs() < 1e-9 && ph.abs() < 1e-9);
    }
}
