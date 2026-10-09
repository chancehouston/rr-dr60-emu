//! Kaiser-windowed sinc prototype design using deterministic math (research.md R-08).

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

use rr_dr60_detmath::{PI, bessel_i0, sin, sqrt};

/// Kaiser window β for a stopband attenuation of `atten_db` (> 50 dB form).
pub(crate) fn kaiser_beta(atten_db: f64) -> f64 {
    0.1102 * (atten_db - 8.7)
}

/// Linear-phase Kaiser-windowed sinc low-pass with `num_taps` (odd) taps and cutoff
/// `cutoff` in cycles per sample. The DC gain is about 1.
pub(crate) fn kaiser_lowpass(num_taps: usize, cutoff: f64, beta: f64) -> Vec<f64> {
    let center = (num_taps - 1) as f64 / 2.0;
    let i0_beta = bessel_i0(beta);
    (0..num_taps)
        .map(|k| {
            let t = k as f64 - center;
            let x = 2.0 * cutoff * t;
            let sinc = if t == 0.0 {
                1.0
            } else {
                sin(PI * x) / (PI * x)
            };
            let r = t / center;
            let window = bessel_i0(beta * sqrt(1.0 - r * r)) / i0_beta;
            2.0 * cutoff * sinc * window
        })
        .collect()
}

/// Splits `taps` into `phases` polyphase branches: branch p is `taps[p], taps[p + phases], …`,
/// zero-padded to a common length. Each branch is normalized to sum to exactly 1, so every
/// branch has unity DC gain and the converter's passband is flat (FR-005).
/// Returns the branches, stored contiguously, and the taps per branch.
pub(crate) fn polyphase_split(taps: &[f64], phases: usize) -> (Box<[f64]>, usize) {
    let per_branch = taps.len().div_ceil(phases);
    let mut out = vec![0.0; per_branch * phases];
    for p in 0..phases {
        let branch = &mut out[p * per_branch..(p + 1) * per_branch];
        for (i, slot) in branch.iter_mut().enumerate() {
            *slot = taps.get(p + i * phases).copied().unwrap_or(0.0);
        }
        let sum: f64 = branch.iter().sum();
        for slot in branch.iter_mut() {
            *slot /= sum;
        }
    }
    (out.into_boxed_slice(), per_branch)
}
