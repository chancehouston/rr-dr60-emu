//! Test-only measurement helpers for the core crate's unit tests. Analysis is tolerance-based,
//! not bit-critical, so std math is fine here. The harness crate has the full versions.
#![allow(clippy::disallowed_methods)]

extern crate std;
use std::f64::consts::TAU;
use std::vec::Vec;

/// Least-squares fit of `a·cos(ωn) + b·sin(ωn)` over `x`. Returns (amplitude, residual
/// signal after removing the fitted sinusoid).
pub(crate) fn fit(x: &[f64], freq: f64, fs: f64) -> (f64, Vec<f64>) {
    let w = TAU * freq / fs;
    let (mut scc, mut sss, mut scs, mut sxc, mut sxs) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (n, &v) in x.iter().enumerate() {
        let (s, c) = (w * n as f64).sin_cos();
        scc += c * c;
        sss += s * s;
        scs += c * s;
        sxc += v * c;
        sxs += v * s;
    }
    let det = scc * sss - scs * scs;
    let a = (sxc * sss - sxs * scs) / det;
    let b = (sxs * scc - sxc * scs) / det;
    let residual = x
        .iter()
        .enumerate()
        .map(|(n, &v)| {
            let (s, c) = (w * n as f64).sin_cos();
            v - (a * c + b * s)
        })
        .collect();
    ((a * a + b * b).sqrt(), residual)
}

/// Mean power of a signal.
pub(crate) fn power(x: &[f64]) -> f64 {
    x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64
}

/// Power ratio in dB.
pub(crate) fn db_power(num: f64, den: f64) -> f64 {
    10.0 * (num / den).log10()
}

/// A cosine tone, f64.
pub(crate) fn tone(freq: f64, amp: f64, fs: f64, len: usize) -> Vec<f64> {
    (0..len)
        .map(|n| amp * (TAU * freq * n as f64 / fs).cos())
        .collect()
}
