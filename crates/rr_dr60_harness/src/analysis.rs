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

/// Analytic-signal envelope |x + j·H{x}| of `x`, via an FFT Hilbert transform. Edge samples
/// are affected by the transform's circularity; callers measure away from the ends.
///
/// Kept for diagnostics only: spec 002 found it reads AGC attack times about 1.6 ms long
/// (the 1 kHz test tone is too slow a carrier for a 3.7 ms time constant), so the AGC checks
/// use the reference-ratio method instead (research.md R-11, T026).
pub fn analytic_envelope<T: Copy + Into<f64>>(x: &[T]) -> Vec<f64> {
    let n = x.len();
    if n == 0 {
        return Vec::new();
    }
    let mut planner = rustfft::FftPlanner::<f64>::new();
    let mut buf: Vec<Complex64> = x.iter().map(|&v| Complex64::new(v.into(), 0.0)).collect();
    planner.plan_fft_forward(n).process(&mut buf);
    // Keep DC (and Nyquist for even n), double the positive frequencies, drop the negative ones.
    let half = n.div_ceil(2);
    for (k, c) in buf.iter_mut().enumerate() {
        if k == 0 || (n % 2 == 0 && k == n / 2) {
            continue;
        } else if k < half {
            *c *= 2.0;
        } else {
            *c = Complex64::new(0.0, 0.0);
        }
    }
    planner.plan_fft_inverse(n).process(&mut buf);
    buf.iter().map(|c| c.norm() / n as f64).collect()
}

/// Exact per-sample gain in dB, `|y[n] / x[n]|`, at every sample where `|x[n]| ≥ 0.1·amp`
/// (gating threshold: engineering target, spec 002 R-15). Valid where the device under test is
/// a pure multiplier with no delay, e.g. the AGC at the 6 kHz identity host rate (002 R-11;
/// spec 004 FR-002).
pub fn gain_trajectory_exact<T: Copy + Into<f64>, U: Copy + Into<f64>>(
    input: &[T],
    output: &[U],
    amp: f64,
) -> Vec<(usize, f64)> {
    input
        .iter()
        .zip(output)
        .enumerate()
        .filter_map(|(n, (&x, &y))| {
            let (x, y): (f64, f64) = (x.into(), y.into());
            (x.abs() >= 0.1 * amp).then(|| (n, db((y / x).abs())))
        })
        .collect()
}

/// Index of the last trajectory point outside the settling band: `|v − final| > 2/27 ·
/// |excursion|` (spec 002, Overview › Settling band). Returns 0 if every point is inside.
pub fn settle_index(trajectory: &[(usize, f64)], final_db: f64, excursion_db: f64) -> usize {
    let band = excursion_db.abs() * 2.0 / 27.0;
    trajectory
        .iter()
        .rev()
        .find(|&&(_, v)| (v - final_db).abs() > band)
        .map_or(0, |&(n, _)| n)
}

/// Fraction of the dB change from `start_db` to `final_db` reached at `step_index +
/// release_samples / 4` (spec 002 FR-006 shape check). Uses the first trajectory point at or
/// after that time.
pub fn midpoint_fraction(
    trajectory: &[(usize, f64)],
    step_index: usize,
    start_db: f64,
    final_db: f64,
    release_samples: usize,
) -> f64 {
    let at = step_index + release_samples / 4;
    let v = trajectory
        .iter()
        .find(|&&(n, _)| n >= at)
        .map_or(final_db, |&(_, v)| v);
    (v - start_db) / (final_db - start_db)
}

/// THD+N in dB of `x[start..start + len]` (spec 002 FR-008, research.md R-11): a least-squares
/// fit of the fundamental's cosine and sine at `freq_hz`, then the power of everything left
/// over, DC included, relative to the fitted fundamental's power. A harmonic that folds onto
/// the fundamental's own frequency cannot be separated from it; it shows up as a level change,
/// which the FR-008 level check covers.
pub fn thd_n_db<T: Copy + Into<f64>>(
    x: &[T],
    freq_hz: f64,
    fs: f64,
    start: usize,
    len: usize,
) -> f64 {
    let w = TAU * freq_hz / fs;
    let seg: Vec<f64> = x[start..start + len].iter().map(|&v| v.into()).collect();
    let (mut scc, mut sss, mut scs, mut sxc, mut sxs) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (m, &v) in seg.iter().enumerate() {
        let (s, c) = (w * m as f64).sin_cos();
        scc += c * c;
        sss += s * s;
        scs += c * s;
        sxc += v * c;
        sxs += v * s;
    }
    let det = scc * sss - scs * scs;
    let a = (sxc * sss - sxs * scs) / det;
    let b = (sxs * scc - sxc * scs) / det;
    let (mut fund, mut resid) = (0.0, 0.0);
    for (m, &v) in seg.iter().enumerate() {
        let (s, c) = (w * m as f64).sin_cos();
        let f = a * c + b * s;
        fund += f * f;
        resid += (v - f) * (v - f);
    }
    10.0 * (resid / fund).log10()
}

/// Level in dBFS by the AES17 convention (a full-scale sine is 0 dBFS):
/// `20·log10(RMS·√2) = 10·log10(2·mean square)` (spec 002, Overview › Level).
pub fn level_dbfs_aes17<T: Copy + Into<f64>>(x: &[T]) -> f64 {
    let ms = x.iter().map(|&v| v.into() * v.into()).sum::<f64>() / x.len() as f64;
    10.0 * (2.0 * ms).log10()
}

/// Least-squares amplitude of a sinusoid at `freq_hz` in `x` (the normal equations of
/// `a·cos + b·sin`), with no window.
fn ls_amplitude<T: Copy + Into<f64>>(x: &[T], freq_hz: f64, fs: f64) -> f64 {
    let w = TAU * freq_hz / fs;
    let (mut scc, mut sss, mut scs, mut sxc, mut sxs) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (n, &v) in x.iter().enumerate() {
        let (s, c) = (w * n as f64).sin_cos();
        let v: f64 = v.into();
        scc += c * c;
        sss += s * s;
        scs += c * s;
        sxc += v * c;
        sxs += v * s;
    }
    let det = scc * sss - scs * scs;
    let a = (sxc * sss - sxs * scs) / det;
    let b = (sxs * scc - sxc * scs) / det;
    (a * a + b * b).sqrt()
}

/// Least-squares sinusoid fit (spec 006 US1 scenario 5; hardware review B9): the amplitude
/// and frequency of the sinusoid in `x` nearest `guess_hz`, found by maximizing the
/// least-squares amplitude over a grid of ±5 Hz in 0.1 Hz steps and then ±0.1 Hz in 0.002 Hz
/// steps (engineering target). Phase-independent, so no aligned start is needed.
pub fn fit_sinusoid<T: Copy + Into<f64>>(x: &[T], fs: f64, guess_hz: f64) -> (f64, f64) {
    let search = |center: f64, half: f64, step: f64| {
        let n = (half / step).round() as i64;
        (-n..=n)
            .map(|k| {
                let f = center + k as f64 * step;
                (ls_amplitude(x, f, fs), f)
            })
            .fold((f64::MIN, center), |m, c| if c.0 > m.0 { c } else { m })
    };
    let (_, coarse) = search(guess_hz, 5.0, 0.1);
    let (amp, fine) = search(coarse, 0.1, 0.002);
    (amp, fine)
}

/// Amplitude ratio to decibels.
pub fn db(ratio: f64) -> f64 {
    20.0 * ratio.log10()
}

/// Decibels to amplitude ratio.
pub fn from_db(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

/// RMS level in dBFS of `x` (0 dBFS = a full-scale sine, RMS 1/√2).
pub fn rms_dbfs<T: Copy + Into<f64>>(x: &[T]) -> f64 {
    let ms = x.iter().map(|&v| v.into() * v.into()).sum::<f64>() / x.len().max(1) as f64;
    10.0 * (ms * 2.0).max(1e-30).log10()
}

/// The ten third-octave band centres used by the codec checks (spec 008 FR-005): 315 … 2500 Hz.
pub const THIRD_OCTAVE_CENTRES_HZ: [f64; 10] = [
    315.0, 400.0, 500.0, 630.0, 800.0, 1000.0, 1250.0, 1600.0, 2000.0, 2500.0,
];

/// Long-term third-octave band levels (dB, arbitrary reference) of `x` at `fs`: the power of
/// the FFT bins between each band's edges (centre × 2^(±1/6)), over the whole signal in one
/// FFT of the next power of two (spec 008 FR-005; research R-09). Differences between two
/// signals' levels are what the checks use, so the reference cancels.
pub fn third_octave_levels<T: Copy + Into<f64>>(x: &[T], fs: f64) -> [f64; 10] {
    let n = x.len().next_power_of_two().max(16);
    let spectrum = Spectrum::from_impulse_response(x, fs, n);
    let mut out = [0.0; 10];
    for (i, &fc) in THIRD_OCTAVE_CENTRES_HZ.iter().enumerate() {
        let (lo, hi) = (fc / 2f64.powf(1.0 / 6.0), fc * 2f64.powf(1.0 / 6.0));
        let p: f64 = spectrum
            .range(lo, hi.min(fs / 2.0 - spectrum.resolution()))
            .map(|k| spectrum.bins[k].norm_sqr())
            .sum();
        out[i] = 10.0 * (p / n as f64).max(1e-30).log10();
    }
    out
}

/// Autocorrelation-method LPC envelope of one frame: returns the predictor coefficients
/// a[1..=order] of A(z) = 1 − Σ a_k z^−k (Levinson–Durbin in f64, with a tiny white-noise
/// floor), or `None` when the frame is silent.
fn lpc_frame(frame: &[f64], order: usize) -> Option<Vec<f64>> {
    let mut r = vec![0.0; order + 1];
    for k in 0..=order {
        r[k] = frame[k..]
            .iter()
            .zip(frame.iter())
            .map(|(a, b)| a * b)
            .sum();
    }
    if r[0] <= 1e-12 {
        return None;
    }
    r[0] *= 1.0001;
    let mut a = vec![0.0; order + 1];
    let mut e = r[0];
    for i in 1..=order {
        let mut acc = r[i];
        for j in 1..i {
            acc -= a[j] * r[i - j];
        }
        let k = acc / e;
        let prev = a.clone();
        a[i] = k;
        for j in 1..i {
            a[j] = prev[j] - k * prev[i - j];
        }
        e *= 1.0 - k * k;
        if e <= 0.0 {
            return None;
        }
    }
    Some(a)
}

/// Mean **log-spectral distance** (dB) between the order-8 LPC envelopes of `x` and `y`, per
/// 20 ms frame (Hamming-windowed), evaluated at 64 frequencies spread evenly over 225 – 2550 Hz,
/// averaged over the frames where both are above −50 dBFS (spec 008 FR-005; research R-09).
pub fn lpc_lsd<T: Copy + Into<f64>, U: Copy + Into<f64>>(x: &[T], y: &[U], fs: f64) -> f64 {
    let frame = (fs * 0.02) as usize;
    let order = 8;
    let window: Vec<f64> = (0..frame)
        .map(|n| 0.54 - 0.46 * (TAU * n as f64 / (frame - 1) as f64).cos())
        .collect();
    let freqs: Vec<f64> = (0..64)
        .map(|i| 225.0 + (2550.0 - 225.0) * i as f64 / 63.0)
        .collect();
    let envelope = |a: &[f64], f: f64| -> f64 {
        let w = TAU * f / fs;
        let mut re = 1.0;
        let mut im = 0.0;
        for (k, &ak) in a.iter().enumerate().skip(1) {
            re -= ak * (w * k as f64).cos();
            im += ak * (w * k as f64).sin();
        }
        -10.0 * (re * re + im * im).max(1e-30).log10()
    };
    let mut total = 0.0;
    let mut count = 0usize;
    let frames = x.len().min(y.len()) / frame;
    for f in 0..frames {
        let xs: Vec<f64> = x[f * frame..(f + 1) * frame]
            .iter()
            .zip(&window)
            .map(|(&v, w)| v.into() * w)
            .collect();
        let ys: Vec<f64> = y[f * frame..(f + 1) * frame]
            .iter()
            .zip(&window)
            .map(|(&v, w)| v.into() * w)
            .collect();
        if rms_dbfs(&x[f * frame..(f + 1) * frame]) < -50.0 {
            continue;
        }
        let (Some(ax), Some(ay)) = (lpc_frame(&xs, order), lpc_frame(&ys, order)) else {
            continue;
        };
        let d: f64 = freqs
            .iter()
            .map(|&fr| {
                let diff = envelope(&ax, fr) - envelope(&ay, fr);
                diff * diff
            })
            .sum::<f64>()
            / freqs.len() as f64;
        total += d.sqrt();
        count += 1;
    }
    if count == 0 {
        0.0
    } else {
        total / count as f64
    }
}

/// Gated **segmental SNR** (dB) of `y` against the reference `x`, `y` delayed by `delay`
/// samples relative to `x` (y[n + delay] ≈ x[n]): 20 ms segments, each clamped to −10 … +35 dB,
/// segments whose reference level is below −50 dBFS excluded, mean over the rest (spec 008
/// FR-006; research R-09; plan review D3).
pub fn segmental_snr<T: Copy + Into<f64>, U: Copy + Into<f64>>(
    x: &[T],
    y: &[U],
    fs: f64,
    delay: usize,
) -> f64 {
    let seg = (fs * 0.02) as usize;
    let usable = x.len().min(y.len().saturating_sub(delay)) / seg;
    let mut total = 0.0;
    let mut count = 0usize;
    for s in 0..usable {
        let xs = &x[s * seg..(s + 1) * seg];
        if rms_dbfs(xs) < -50.0 {
            continue;
        }
        let (mut sig, mut err) = (0.0, 0.0);
        for (i, &xv) in xs.iter().enumerate() {
            let xv: f64 = xv.into();
            let yv: f64 = y[s * seg + i + delay].into();
            sig += xv * xv;
            err += (xv - yv) * (xv - yv);
        }
        let snr = 10.0 * (sig / err.max(1e-30)).log10();
        total += snr.clamp(-10.0, 35.0);
        count += 1;
    }
    if count == 0 {
        0.0
    } else {
        total / count as f64
    }
}

/// The lag (in samples, `−max_lag ..= max_lag`) that maximizes the normalized cross-correlation
/// of `y` against `x` (positive when `y` lags `x`), over the overlapping steady part (spec 008
/// FR-004; research R-09). Ties resolve to the smaller lag.
pub fn xcorr_lag<T: Copy + Into<f64>, U: Copy + Into<f64>>(
    x: &[T],
    y: &[U],
    max_lag: usize,
) -> i64 {
    let n = x.len().min(y.len());
    let mut best = (f64::MIN, 0i64);
    for lag in -(max_lag as i64)..=(max_lag as i64) {
        let (mut xy, mut xx, mut yy) = (0.0, 0.0, 0.0);
        for (i, &xi) in x.iter().enumerate().take(n) {
            let j = i as i64 + lag;
            if j < 0 || j >= n as i64 {
                continue;
            }
            let xv: f64 = xi.into();
            let yv: f64 = y[j as usize].into();
            xy += xv * yv;
            xx += xv * xv;
            yy += yv * yv;
        }
        let c = xy / (xx * yy).sqrt().max(1e-30);
        if c > best.0 {
            best = (c, lag);
        }
    }
    best.1
}

/// The largest absolute difference (dB) between the 100 ms RMS levels of `x` and of `y`
/// (`y` delayed by `delay`), over windows where `x` is above −50 dBFS (spec 008 FR-006: no
/// pumping or dropouts).
pub fn level_track<T: Copy + Into<f64>, U: Copy + Into<f64>>(
    x: &[T],
    y: &[U],
    fs: f64,
    delay: usize,
) -> f64 {
    let win = (fs * 0.1) as usize;
    let usable = x.len().min(y.len().saturating_sub(delay)) / win;
    let mut worst = 0.0f64;
    for w in 0..usable {
        let xs = &x[w * win..(w + 1) * win];
        let lx = rms_dbfs(xs);
        if lx < -50.0 {
            continue;
        }
        let ly = rms_dbfs(&y[w * win + delay..(w + 1) * win + delay]);
        worst = worst.max((lx - ly).abs());
    }
    worst
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

    /// 002 T009: the analytic envelope of a steady sine is its amplitude.
    #[test]
    fn analytic_envelope_of_sine_is_constant() {
        let x = sinusoid(0.5, 0.3, 1000.0, 48_000.0, 48_000);
        let env = analytic_envelope(&x);
        assert_eq!(env.len(), x.len());
        for &e in &env[1000..47_000] {
            assert!(db(e / 0.5).abs() < 0.01, "envelope {e}");
        }
    }

    /// 002 T009: exact gain trajectory y/x, gated where |x| < 0.1·A (002 R-15).
    #[test]
    fn exact_gain_trajectory_is_gated_ratio() {
        let x = sinusoid(0.5, 0.0, 1000.0, 8000.0, 80);
        let y: Vec<f64> = x.iter().map(|v| 0.25 * v).collect();
        let traj = gain_trajectory_exact(&x, &y, 0.5);
        assert!(!traj.is_empty() && traj.len() < x.len());
        for &(n, g) in &traj {
            assert!(x[n].abs() >= 0.05);
            assert!((g - db(0.25)).abs() < 1e-9, "n {n}: {g}");
        }
    }

    /// 002 T009: settle index and release midpoint on an exponential with a known τ.
    #[test]
    fn settle_index_and_midpoint_on_exponential() {
        let tau = 400.0;
        let exc = 27.0;
        let traj: Vec<(usize, f64)> = (0..10_000)
            .map(|n| (n, -exc * (-(n as f64) / tau).exp()))
            .collect();
        let idx = settle_index(&traj, 0.0, exc);
        let expected = tau * 13.5f64.ln();
        assert!((idx as f64 - expected).abs() <= 1.0, "{idx} vs {expected}");
        let release = idx + 1;
        let frac = midpoint_fraction(&traj, 0, -exc, 0.0, release);
        let want = 1.0 - (-(13.5f64.ln()) / 4.0).exp();
        assert!((frac - want).abs() < 0.01, "{frac} vs {want}");
    }

    /// 002 T009: THD+N fits the fundamental and counts everything else, DC included.
    #[test]
    fn thd_n_counts_harmonics_and_folded_dc() {
        let fs = 48_000.0;
        let n = 48_000;
        let pure = sinusoid(1.0, 0.2, 1000.0, fs, n);
        assert!(thd_n_db(&pure, 1000.0, fs, 0, n) < -100.0);
        let h3: Vec<f64> = pure
            .iter()
            .zip(sinusoid(0.01, 0.0, 3000.0, fs, n))
            .map(|(a, b)| a + b)
            .collect();
        let t = thd_n_db(&h3, 1000.0, fs, 0, n);
        assert!((t + 40.0).abs() < 0.1, "1 % third harmonic: {t} dB");
        // At 8 kHz, the 3rd harmonic of 8000/3 Hz falls at 8000 Hz and samples as DC (here a
        // cosine, so DC = 0.01). THD+N must count it: 10·log10(0.01² / 0.5) = −36.99 dB.
        let f = 8000.0 / 3.0;
        let x: Vec<f64> = sinusoid(1.0, 0.0, f, 8000.0, 24_000)
            .iter()
            .zip(sinusoid(0.01, 0.0, 3.0 * f, 8000.0, 24_000))
            .map(|(a, b)| a + b)
            .collect();
        let t = thd_n_db(&x, f, 8000.0, 0, 24_000);
        let want = 10.0 * (1e-4f64 / 0.5).log10();
        assert!((t - want).abs() < 0.1, "folded to DC: {t} vs {want}");
    }

    /// 002 T009: AES17 level: a full-scale sine is 0 dBFS (RMS · √2).
    #[test]
    fn aes17_level_of_full_scale_sine() {
        let x = sinusoid(1.0, 0.0, 1000.0, 48_000.0, 48_000);
        assert!(level_dbfs_aes17(&x).abs() < 1e-6);
        let y = sinusoid(0.1, 0.0, 1000.0, 48_000.0, 48_000);
        assert!((level_dbfs_aes17(&y) + 20.0).abs() < 1e-6);
    }

    #[test]
    fn gain_db_matches_ratio() {
        let x = sinusoid(1.0, 0.0, 1000.0, 48_000.0, 48_000);
        let y: Vec<f64> = x.iter().map(|v| 0.1 * v).collect();
        let (g, ph) = gain_db_and_phase(&x, &y, 1000.0, 48_000.0, 0);
        assert!((g + 20.0).abs() < 1e-9 && ph.abs() < 1e-9);
    }
}

/// Complex frequency response of the impulse response `ir` at exactly `freq_hz` (direct DTFT).
pub fn dtft<T: Copy + Into<f64>>(ir: &[T], freq_hz: f64, fs: f64) -> Complex64 {
    let w = TAU * freq_hz / fs;
    ir.iter()
        .enumerate()
        .fold(Complex64::new(0.0, 0.0), |acc, (n, &v)| {
            let v: f64 = v.into();
            acc + Complex64::from_polar(v, -w * n as f64)
        })
}

/// Frequency response from an impulse response, on an FFT grid (tasks.md T047, T048).
#[derive(Clone, Debug)]
pub struct Spectrum {
    /// Sample rate of the impulse response, in Hz.
    pub fs: f64,
    /// FFT bins 0..n (bin k is at k·fs/n Hz).
    pub bins: Vec<Complex64>,
}

impl Spectrum {
    /// FFT of `ir`, zero-padded or truncated to `n` (a power of two).
    pub fn from_impulse_response<T: Copy + Into<f64>>(ir: &[T], fs: f64, n: usize) -> Self {
        let mut bins: Vec<Complex64> = (0..n)
            .map(|i| Complex64::new(ir.get(i).map_or(0.0, |&v| v.into()), 0.0))
            .collect();
        rustfft::FftPlanner::new()
            .plan_fft_forward(n)
            .process(&mut bins);
        Self { fs, bins }
    }

    /// Bin spacing in Hz.
    pub fn resolution(&self) -> f64 {
        self.fs / self.bins.len() as f64
    }

    /// Frequency of bin `k`, in Hz.
    pub fn freq(&self, k: usize) -> f64 {
        k as f64 * self.resolution()
    }

    /// Bins from `f_lo` to `f_hi` Hz inclusive, up to (and excluding) Nyquist.
    pub fn range(&self, f_lo: f64, f_hi: f64) -> std::ops::RangeInclusive<usize> {
        let r = self.resolution();
        let hi = ((f_hi / r).floor() as usize).min(self.bins.len() / 2 - 1);
        ((f_lo / r).ceil() as usize)..=hi
    }

    /// This spectrum divided bin by bin by `other` (e.g. a configuration over the bypassed
    /// baseline, FR-010).
    pub fn divide(&self, other: &Spectrum) -> Spectrum {
        Spectrum {
            fs: self.fs,
            bins: self
                .bins
                .iter()
                .zip(&other.bins)
                .map(|(a, b)| a / b)
                .collect(),
        }
    }

    /// Group delay in samples near `freq_hz`, from the phase difference of adjacent bins.
    pub fn group_delay_samples(&self, freq_hz: f64) -> f64 {
        let k = (freq_hz / self.resolution()).round() as usize;
        let dphi = (self.bins[k + 1] / self.bins[k]).arg();
        -dphi / (TAU / self.bins.len() as f64)
    }
}

/// Minimum-phase phase (radians) for every FFT bin, reconstructed from the magnitude of
/// `spectrum` by the real-cepstrum method. Magnitudes are floored at 1e-30 (−600 dB) so exact
/// zeros on the unit circle stay finite. A higher floor (e.g. 1e-12) visibly distorts the
/// reconstruction near such zeros (0.54° instead of 0.11° on the voice-band cascade).
pub fn minimum_phase(spectrum: &Spectrum) -> Vec<f64> {
    let n = spectrum.bins.len();
    let mut planner = rustfft::FftPlanner::new();
    let mut c: Vec<Complex64> = spectrum
        .bins
        .iter()
        .map(|h| Complex64::new(h.norm().max(1e-30).ln(), 0.0))
        .collect();
    planner.plan_fft_inverse(n).process(&mut c);
    for (i, v) in c.iter_mut().enumerate() {
        let fold = if i == 0 || i == n / 2 {
            1.0
        } else if i < n / 2 {
            2.0
        } else {
            0.0
        };
        *v = Complex64::new(v.re / n as f64 * fold, 0.0);
    }
    planner.plan_fft_forward(n).process(&mut c);
    c.iter().map(|v| v.im).collect()
}

/// Largest phase difference, in degrees, between `spectrum` and its minimum-phase
/// reconstruction over `f_lo..=f_hi` Hz (FR-010 phase check, A-016).
pub fn phase_deviation_deg(spectrum: &Spectrum, f_lo: f64, f_hi: f64) -> f64 {
    let minphase = minimum_phase(spectrum);
    spectrum
        .range(f_lo, f_hi)
        .map(|k| {
            let d = spectrum.bins[k].arg() - minphase[k];
            let wrapped = (d + std::f64::consts::PI).rem_euclid(TAU) - std::f64::consts::PI;
            wrapped.abs().to_degrees()
        })
        .fold(0.0, f64::max)
}

/// Power of everything in `output` except the sinusoid at `freq_hz`, relative to the power of
/// `input`, in dB, over `settle..` (alias and image products, FR-005). Uses an unwindowed
/// least-squares fit, so the fitted tone is removed exactly.
pub fn residual_power_db<T: Copy + Into<f64>, U: Copy + Into<f64>>(
    input: &[T],
    output: &[U],
    freq_hz: f64,
    fs: f64,
    settle: usize,
) -> f64 {
    let y: Vec<f64> = output[settle..].iter().map(|&v| v.into()).collect();
    let w = TAU * freq_hz / fs;
    let (mut scc, mut sss, mut scs, mut syc, mut sys) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (n, &v) in y.iter().enumerate() {
        let (s, c) = (w * n as f64).sin_cos();
        scc += c * c;
        sss += s * s;
        scs += c * s;
        syc += v * c;
        sys += v * s;
    }
    let det = scc * sss - scs * scs;
    let (a, b) = ((syc * sss - sys * scs) / det, (sys * scc - syc * scs) / det);
    let residual: f64 = y
        .iter()
        .enumerate()
        .map(|(n, &v)| {
            let (s, c) = (w * n as f64).sin_cos();
            let r = v - (a * c + b * s);
            r * r
        })
        .sum();
    let pin: f64 = input[settle..].iter().map(|&v| v.into() * v.into()).sum();
    10.0 * (residual / pin).log10()
}

#[cfg(test)]
mod spectrum_tests {
    use super::*;

    fn filter(sos: &[[f64; 5]], x: &[f64]) -> Vec<f64> {
        let mut y = x.to_vec();
        for &[b0, b1, b2, a1, a2] in sos {
            let (mut s1, mut s2) = (0.0, 0.0);
            for v in &mut y {
                let out = b0 * *v + s1;
                s1 = b1 * *v - a1 * out + s2;
                s2 = b2 * *v - a2 * out;
                *v = out;
            }
        }
        y
    }

    fn impulse(n: usize) -> Vec<f64> {
        let mut x = vec![0.0; n];
        x[0] = 1.0;
        x
    }

    #[test]
    fn minimum_phase_cascade_has_small_deviation() {
        let ir = filter(&rr_dr60::__test_hooks::record_sos(), &impulse(65_536));
        let s = Spectrum::from_impulse_response(&ir, 8000.0, 65_536);
        let d = phase_deviation_deg(&s, 400.0, 3200.0);
        assert!(d <= 0.5, "min-phase cascade deviates {d:.3} deg");
    }

    #[test]
    fn linear_phase_fir_has_large_deviation() {
        // 31-tap symmetric (linear-phase) Hann-windowed low-pass at 2 kHz.
        let taps: Vec<f64> = (0..31)
            .map(|k| {
                let t = k as f64 - 15.0;
                let sinc = if t == 0.0 {
                    0.5
                } else {
                    (std::f64::consts::PI * 0.5 * t).sin() / (std::f64::consts::PI * t)
                };
                sinc * (0.5 - 0.5 * (TAU * k as f64 / 30.0).cos())
            })
            .collect();
        let s = Spectrum::from_impulse_response(&taps, 8000.0, 65_536);
        let d = phase_deviation_deg(&s, 400.0, 1600.0);
        assert!(d > 5.0, "linear-phase FIR deviates only {d:.3} deg");
    }

    #[test]
    fn spectrum_helpers() {
        // A pure 10-sample delay: unit magnitude, group delay 10, dtft agrees with the FFT.
        let mut ir = vec![0.0; 64];
        ir[10] = 1.0;
        let s = Spectrum::from_impulse_response(&ir, 8000.0, 1024);
        assert!((s.group_delay_samples(1000.0) - 10.0).abs() < 1e-9);
        assert!((s.bins[128] - dtft(&ir, s.freq(128), 8000.0)).norm() < 1e-9);
        assert_eq!(s.range(0.0, 8000.0), 0..=511);
        let flat = s.divide(&s);
        assert!(
            flat.bins
                .iter()
                .all(|b| (b - Complex64::new(1.0, 0.0)).norm() < 1e-12)
        );
    }

    #[test]
    fn residual_of_pure_tone_is_tiny() {
        let x: Vec<f64> = (0..48_000)
            .map(|n| (TAU * 1000.0 * n as f64 / 48_000.0).sin())
            .collect();
        let mut y = x.clone();
        for (n, v) in y.iter_mut().enumerate() {
            *v += 1e-4 * (TAU * 3000.0 * n as f64 / 48_000.0).sin(); // a -80 dB "alias"
        }
        let r = residual_power_db(&x, &y, 1000.0, 48_000.0, 0);
        assert!((r + 80.0).abs() < 0.1, "residual {r:.2} dB");
    }

    /// Spec 008 (T005): the codec metrics on synthetic pairs.
    #[test]
    fn codec_metrics_on_synthetic_pairs() {
        let fs = 6000.0;
        let x = crate::stimulus::voiced(fs, 1.0, -20.0, 3);
        // A delayed copy: lag found exactly, segmental SNR at the clamp, LSD ≈ 0.
        let delay = 17usize;
        let mut y = vec![0.0f32; delay];
        y.extend_from_slice(&x);
        assert_eq!(xcorr_lag(&x, &y, 100), delay as i64);
        assert!((segmental_snr(&x, &y, fs, delay) - 35.0).abs() < 1e-9);
        assert!(lpc_lsd(&x, &y[delay..], fs) < 1e-6);
        assert!(level_track(&x, &y, fs, delay) < 1e-9);
        // A 6 dB gain: every band and the level track read 6 dB; the lag is unchanged.
        let g: Vec<f32> = y.iter().map(|&v| v * 2.0).collect();
        let bx = third_octave_levels(&x, fs);
        let bg = third_octave_levels(&g[delay..], fs);
        for (a, b) in bx.iter().zip(&bg) {
            assert!((b - a - 6.0206).abs() < 0.05, "{a} {b}");
        }
        assert!((level_track(&x, &g, fs, delay) - 6.0206).abs() < 0.01);
        assert!((rms_dbfs(&x) + 20.0).abs() < 0.1);
        assert_eq!(xcorr_lag(&x, &g, 100), delay as i64);
        // Noise against the signal: a low segmental SNR, clamped at −10 from below.
        let noise = crate::stimulus::noise(9, x.len());
        assert!(segmental_snr(&x, &noise, fs, 0) < 0.0);
        assert!(segmental_snr(&x, &noise, fs, 0) >= -10.0);
    }
}
