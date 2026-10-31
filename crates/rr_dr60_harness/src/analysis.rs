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

/// Spec 013 (research R-08): the least-squares fit of a sinusoid at exactly `freq_hz` in `x`
/// (no window: the cosine and sine at the given frequency over every sample), as its amplitude
/// and the residual `x − fit`. A pure sinusoid fits exactly at any length, so the amplitude
/// resolves level differences far below an FFT bin's, and the residual carries every other
/// component (FR-004 (a), (b)).
pub fn sine_fit<T: Copy + Into<f64>>(x: &[T], freq_hz: f64, fs: f64) -> (f64, Vec<f64>) {
    let w = TAU * freq_hz / fs;
    let (mut scc, mut sss, mut scs, mut sxc, mut sxs) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (n, &v) in x.iter().enumerate() {
        let (c, s) = ((w * n as f64).cos(), (w * n as f64).sin());
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
    let residual = x
        .iter()
        .enumerate()
        .map(|(n, &v)| v.into() - a * (w * n as f64).cos() - b * (w * n as f64).sin())
        .collect();
    ((a * a + b * b).sqrt(), residual)
}

/// Spec 013 (research R-08): the largest spectral component of `x` between `lo_hz` and `hi_hz`,
/// in dB relative to a unit-amplitude sine: a Kaiser window (β 20, sidelobes far below the
/// −60 dB floor) and an FFT zero-padded four times (scalloping under 0.03 dB), each bin's
/// magnitude scaled by 2 ÷ Σw so a sine of amplitude A reads 20·log10(A) at its peak bin (FR-004 (b), (c); `tools/playback-speed/speed_design.py`'s
/// `peak_db`).
pub fn spurious_peak_db<T: Copy + Into<f64>>(x: &[T], fs: f64, lo_hz: f64, hi_hz: f64) -> f64 {
    let n = x.len();
    let i0 = |v: f64| {
        // The modified Bessel function of the first kind, order 0, by its power series.
        let (mut sum, mut term, mut k) = (1.0, 1.0, 1.0);
        while term > 1e-17 * sum {
            term *= (v / (2.0 * k)) * (v / (2.0 * k));
            sum += term;
            k += 1.0;
        }
        sum
    };
    let beta = 20.0; // engineering target (013 R-08: the script's window)
    let denom = i0(beta);
    let w: Vec<f64> = (0..n)
        .map(|k| {
            let r = 2.0 * k as f64 / (n - 1) as f64 - 1.0;
            i0(beta * (1.0 - r * r).max(0.0).sqrt()) / denom
        })
        .collect();
    let wsum: f64 = w.iter().sum();
    let padded = 4 * n; // engineering target (013 R-08: the script's zero padding)
    let mut buf: Vec<Complex64> = x
        .iter()
        .zip(&w)
        .map(|(&v, &wk)| Complex64::new(v.into() * wk, 0.0))
        .chain(std::iter::repeat(Complex64::new(0.0, 0.0)))
        .take(padded)
        .collect();
    rustfft::FftPlanner::<f64>::new()
        .plan_fft_forward(padded)
        .process(&mut buf);
    let mut peak = 0.0f64;
    for (k, c) in buf.iter().enumerate().take(padded / 2 + 1) {
        let f = k as f64 * fs / padded as f64;
        if f > lo_hz && f < hi_hz {
            peak = peak.max(c.norm() * 2.0 / wsum);
        }
    }
    db(peak.max(1e-300))
}

/// Amplitude ratio to decibels.
pub fn db(ratio: f64) -> f64 {
    20.0 * ratio.log10()
}

/// Decibels to amplitude ratio.
pub fn from_db(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

/// `x` at `fs_in` resampled to `fs_out` by a windowed-sinc interpolator (a Hamming window over
/// 64 zero crossings at the lower of the two rates, cut-off at 0.45 of the lower rate): the
/// codec checks measure the spectral figures at the calibration's 8 kHz whatever the host rate
/// (spec 008 research R-09). Measurement only (not bit-critical; `f64` with `std` math).
pub fn resample_to<T: Copy + Into<f64>>(x: &[T], fs_in: f64, fs_out: f64) -> Vec<f64> {
    if (fs_in - fs_out).abs() < 1e-9 {
        return x.iter().map(|&v| v.into()).collect();
    }
    let lower = fs_in.min(fs_out);
    let cutoff = 0.45 * lower / fs_in; // cycles per input sample
    let half_span = (32.0 * fs_in / lower).ceil() as i64; // 32 zero crossings each side
    let out_len = (x.len() as f64 * fs_out / fs_in).floor() as usize;
    let mut out = Vec::with_capacity(out_len);
    let xs: Vec<f64> = x.iter().map(|&v| v.into()).collect();
    for m in 0..out_len {
        let centre = m as f64 * fs_in / fs_out;
        let c0 = centre.floor() as i64;
        let mut acc = 0.0;
        for n in (c0 - half_span)..=(c0 + half_span + 1) {
            if n < 0 || n >= xs.len() as i64 {
                continue;
            }
            let t = n as f64 - centre;
            let sinc = if t.abs() < 1e-12 {
                1.0
            } else {
                (TAU * cutoff * t).sin() / (TAU * cutoff * t)
            };
            let w = 0.54 + 0.46 * (std::f64::consts::PI * t / (half_span as f64 + 1.0)).cos();
            acc += xs[n as usize] * sinc * w;
        }
        out.push(acc * 2.0 * cutoff);
    }
    out
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

/// Spec 011 FR-004's **click energy** of `y` around `centre` (a host-sample index, fractional
/// allowed): the summed power of the bins of a Hann window of `round(0.5 · rate)` samples
/// centred on `centre`, at frequencies 20 Hz ≤ f ≤ 2750 Hz with |f − 1000 Hz| > 250 Hz (the same
/// band at every rate, below 6 kHz's Nyquist). The reduction of a transition against the
/// instantaneous step is `db(step / transition)` with the step's window centred on the step
/// and the transition's on its midpoint (research R-02, R-05). Samples outside `y` read as 0.
pub fn click_energy<T: Copy + Into<f64>>(y: &[T], rate: u32, centre: f64) -> f64 {
    click_energy_in(y, rate, centre, 0.5, &[1000.0])
}

/// Spec 019 research R-06: spec 011's click reader with its window length and exclusions as
/// parameters: a Hann window of `round(window_s · rate)` samples centred on `centre` (a host-
/// or device-sample index, fractional allowed), the summed power of the bins at 20 Hz ≤ f ≤
/// 2750 Hz outside every `exclusions_hz[i]` ± 250 Hz (an exclusion above the Nyquist frequency
/// folds to `rate − f`; overlapping exclusions are a union). `click_energy` is this reader at
/// 0.5 s and `[1000.0]`.
pub fn click_energy_in<T: Copy + Into<f64>>(
    y: &[T],
    rate: u32,
    centre: f64,
    window_s: f64,
    exclusions_hz: &[f64],
) -> f64 {
    click_energy_band(y, rate, centre, window_s, exclusions_hz, 2750.0)
}

/// Spec 021 research R-06: the spec 019 reader with the band's top as a parameter (2750 Hz at
/// 6 kHz; 0.45 × rate for the 48 kHz rows, where a step's splatter extends past 2750 Hz).
pub fn click_energy_band<T: Copy + Into<f64>>(
    y: &[T],
    rate: u32,
    centre: f64,
    window_s: f64,
    exclusions_hz: &[f64],
    top_hz: f64,
) -> f64 {
    let fs = f64::from(rate);
    let len = (window_s * fs).round() as usize;
    let excl: Vec<f64> = exclusions_hz
        .iter()
        .map(|&e| if e > fs / 2.0 { fs - e } else { e })
        .collect();
    let start = (centre - len as f64 / 2.0).round() as i64;
    let mut buf: Vec<Complex64> = (0..len)
        .map(|n| {
            let w = 0.5 - 0.5 * (TAU * n as f64 / (len - 1) as f64).cos();
            let i = start + n as i64;
            let v = if i >= 0 && (i as usize) < y.len() {
                y[i as usize].into()
            } else {
                0.0
            };
            Complex64::new(v * w, 0.0)
        })
        .collect();
    rustfft::FftPlanner::<f64>::new()
        .plan_fft_forward(len)
        .process(&mut buf);
    (0..=len / 2)
        .filter(|&k| {
            let f = k as f64 * fs / len as f64;
            (20.0..=top_hz).contains(&f) && excl.iter().all(|&e| (f - e).abs() > 250.0)
        })
        .map(|k| buf[k].norm_sqr())
        .sum()
}

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

/// The third-octave centres from 20 Hz to 20 kHz (spec 012 US1 AS1: the band match is read at
/// each centre below the lower of 20 kHz and 0.45 × the host rate).
pub const THIRD_OCTAVE_CENTRES_20_20K: [f64; 31] = [
    20.0, 25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0,
    500.0, 630.0, 800.0, 1000.0, 1250.0, 1600.0, 2000.0, 2500.0, 3150.0, 4000.0, 5000.0, 6300.0,
    8000.0, 10000.0, 12500.0, 16000.0, 20000.0,
];

/// Mean power of `x` in the band `lo` … `hi` Hz (one FFT of the next power of two, Parseval:
/// the one-sided bins doubled), relative to a full-scale sine's mean power: so
/// `db(band_power_aes17(..)) / 2` is the band level in dBFS by the AES17 convention (spec 012
/// FR-003).
pub fn band_power_aes17<T: Copy + Into<f64>>(x: &[T], fs: f64, lo: f64, hi: f64) -> f64 {
    let n = x.len().next_power_of_two().max(16);
    let spectrum = Spectrum::from_impulse_response(x, fs, n);
    let p: f64 = spectrum
        .range(lo, hi.min(fs / 2.0 - spectrum.resolution()))
        .map(|k| spectrum.bins[k].norm_sqr())
        .sum();
    // Σ x² = (1/n) Σ |X_k|² over both sides; the band's mean square over the signal's length.
    let ms = 2.0 * p / n as f64 / x.len() as f64;
    2.0 * ms // re a full-scale sine's mean square (0.5)
}

/// The band level of `x` between `lo` and `hi` Hz in dBFS (AES17: a full-scale sine reads
/// 0 dBFS; spec 012 FR-003).
pub fn band_level_aes17<T: Copy + Into<f64>>(x: &[T], fs: f64, lo: f64, hi: f64) -> f64 {
    db(band_power_aes17(x, fs, lo, hi).max(1e-60)) / 2.0
}

/// Absolute third-octave band levels of `x` in dBFS (AES17) at each centre (edges at
/// centre × 2^(±1/6); spec 012 US1 AS2).
pub fn third_octave_dbfs<T: Copy + Into<f64>>(x: &[T], fs: f64, centres: &[f64]) -> Vec<f64> {
    let k = 2f64.powf(1.0 / 6.0);
    centres
        .iter()
        .map(|&fc| band_level_aes17(x, fs, fc / k, fc * k))
        .collect()
}

/// The deviation (dB) of the response whose impulse response is `ir` from a prototype, at each
/// `(frequency, prototype dB)` pair, by DTFT at the exact frequency (spec 004's lesson: never
/// adjacent FFT bins; spec 012 US1 AS1).
pub fn prototype_deviation<T: Copy + Into<f64>>(
    ir: &[T],
    fs: f64,
    prototype_db: &[(f64, f64)],
) -> Vec<(f64, f64)> {
    prototype_db
        .iter()
        .map(|&(f, want)| (f, db(dtft(ir, f, fs).norm()) - want))
        .collect()
}

/// The number of samples of `x` whose magnitude exceeds `threshold` (spec 012 R-04: the tail
/// counts).
pub fn exceedance_count<T: Copy + Into<f64>>(x: &[T], threshold: f64) -> usize {
    x.iter().filter(|&&v| v.into().abs() > threshold).count()
}

/// The normalized cross-correlation of `a` and `b` (their common length): the magnitude at lag 0
/// and the largest magnitude over lags −`max_lag` … +`max_lag`, each normalized by
/// `sqrt(Σa²·Σb²)` over the overlap (spec 012 US1 AS4: independent streams read below 5/√N and
/// 6/√N; a shifted copy reads 1 at its lag).
pub fn norm_xcorr_max<T: Copy + Into<f64>, U: Copy + Into<f64>>(
    a: &[T],
    b: &[U],
    max_lag: usize,
) -> (f64, f64) {
    let n = a.len().min(b.len());
    let a: Vec<f64> = a[..n].iter().map(|&v| v.into()).collect();
    let b: Vec<f64> = b[..n].iter().map(|&v| v.into()).collect();
    let corr = |lag: i64| {
        let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
        for i in 0..n as i64 {
            let j = i + lag;
            if j < 0 || j >= n as i64 {
                continue;
            }
            let (x, y) = (a[i as usize], b[j as usize]);
            sab += x * y;
            saa += x * x;
            sbb += y * y;
        }
        if saa == 0.0 || sbb == 0.0 {
            0.0
        } else {
            (sab / (saa * sbb).sqrt()).abs()
        }
    };
    let zero = corr(0);
    let mut worst = zero;
    for lag in 1..=max_lag as i64 {
        worst = worst.max(corr(lag)).max(corr(-lag));
    }
    (zero, worst)
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
    /// 013 T007: the fit recovers a tone's amplitude exactly and leaves a second tone in the
    /// residual, which the spurious reader finds at its level.
    #[test]
    fn sine_fit_and_spurious_peak() {
        let fs = 6000.0;
        let x: Vec<f64> = (0..9000)
            .map(|n| {
                let t = n as f64 / fs;
                0.5 * (TAU * 1003.0 * t + 0.3).sin() + 1e-3 * (TAU * 2203.0 * t).sin()
            })
            .collect();
        let (a, res) = super::sine_fit(&x, 1003.0, fs);
        assert!((a - 0.5).abs() < 1e-6, "{a}");
        let peak = super::spurious_peak_db(&res, fs, 20.0, 3000.0);
        assert!((peak - (-60.0)).abs() < 0.05, "{peak}"); // 1e-3 is −60 dB re a unit sine
    }

    use super::*;

    /// Spec 012 T019: the band level of a full-scale sine inside the band is 0 dBFS; outside it
    /// reads nothing; white noise's band level follows its share of the band.
    #[test]
    fn band_level_aes17_closed_forms() {
        let fs = 48_000.0;
        let n = 1 << 16;
        let k = 1365.0; // an exact bin: no leakage
        let f = k * fs / n as f64;
        let x: Vec<f64> = (0..n).map(|i| (TAU * f * i as f64 / fs).cos()).collect();
        assert!(band_level_aes17(&x, fs, 300.0, 3000.0).abs() < 1e-9);
        assert!(band_level_aes17(&x, fs, 3000.0, 6000.0) < -200.0);
        // White uniform noise of variance v: the band 300 – 3000 Hz holds 2700 / 24 000 of it.
        let mut r = 12345u64;
        let w: Vec<f64> = (0..n)
            .map(|_| {
                r = r
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                (r >> 11) as f64 / (1u64 << 53) as f64 - 0.5
            })
            .collect();
        let want = db(2.0 * (1.0 / 12.0) * 2700.0 / 24_000.0) / 2.0;
        let got = band_level_aes17(&w, fs, 300.0, 3000.0);
        assert!((got - want).abs() < 0.2, "{got} vs {want}");
        let thirds = third_octave_dbfs(&x, fs, &[1000.0, 4000.0]);
        assert!(thirds[0].abs() < 1e-9 && thirds[1] < -200.0, "{thirds:?}");
    }

    /// Spec 012 T019: an impulse has a flat 0 dB response, so its deviation from a flat
    /// prototype is 0; a shifted copy correlates at its lag; the exceedance count is exact.
    #[test]
    fn deviation_xcorr_and_exceedance_closed_forms() {
        let mut ir = vec![0.0f64; 64];
        ir[0] = 1.0;
        for (_, d) in prototype_deviation(&ir, 48_000.0, &[(100.0, 0.0), (10_000.0, 0.0)]) {
            assert!(d.abs() < 1e-12, "{d}");
        }
        let mut r = 7u64;
        let a: Vec<f64> = (0..4000)
            .map(|_| {
                r = r
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                (r >> 11) as f64 / (1u64 << 53) as f64 - 0.5
            })
            .collect();
        let mut b = vec![0.0; 4000];
        b[5..].copy_from_slice(&a[..3995]);
        let (zero, worst) = norm_xcorr_max(&a, &b, 8);
        assert!(zero < 0.1 && (worst - 1.0).abs() < 1e-12, "{zero} {worst}");
        assert_eq!(exceedance_count(&[0.5, -0.6, 0.7, 0.1], 0.55), 2);
    }

    /// Spec 011 T009: the click energy against `tools/volume-ramp/ramp_model.py`'s earphone
    /// figure at 48 kHz (a 1 kHz cosine peaking at the change, 0.1 → 1, the smoothstep over
    /// 960 samples against the instantaneous step: 40.64 dB, research R-02).
    #[test]
    fn click_energy_matches_the_reference_script() {
        let rate = 48_000u32;
        let law = |p: f64| from_db(f64::from(rr_dr60::output::earphone_gain_db(p as f32)));
        let n = 960usize;
        let (pad, n0) = (24_000usize, 24_000usize);
        let total = 2 * pad + n;
        let x: Vec<f64> = (0..total)
            .map(|i| (TAU * 1000.0 * (i as f64 - n0 as f64) / f64::from(rate)).cos())
            .collect();
        let (g0, g1) = (law(0.1), law(1.0));
        let step: Vec<f64> = (0..total)
            .map(|i| x[i] * if i < n0 { g0 } else { g1 })
            .collect();
        let ramp: Vec<f64> = (0..total)
            .map(|i| {
                let g = if i < n0 {
                    g0
                } else if i >= n0 + n - 1 {
                    g1
                } else {
                    let u = (i - n0 + 1) as f64 / n as f64;
                    law(0.1 + 0.9 * u * u * (3.0 - 2.0 * u))
                };
                x[i] * g
            })
            .collect();
        let r = db(click_energy(&step, rate, n0 as f64)
            / click_energy(&ramp, rate, n0 as f64 + (n as f64 - 1.0) / 2.0))
            / 2.0;
        assert!((r - 40.64).abs() <= 0.05, "{r} dB");
    }

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

/// The output chain's section sets (spec 009 research R-10): the design's discrete realization,
/// computed here from the registered figures with the design script's formulas, so that the
/// harness compares the Rust against an independent realization of the same design.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputTemplate {
    /// Stage 11 on the earphone route plus stage 13: the 32 Hz volume-control corner and the
    /// earphone follower's 120 and 280 Hz corners (A-031, A-032).
    Earphone,
    /// Stage 11 on the speaker route with the speaker template off: the 32 Hz and 159 Hz
    /// corners (A-031).
    Amplifier,
    /// Stage 11 and stage 12 on the speaker route: the two corners, the 750 Hz / Q 1.8
    /// resonance and the +4.5 dB shelf (780 → 1310 Hz), the template peak-normalized to 1.0 on
    /// a 1 Hz grid at the rate (A-010, revised).
    Speaker,
}

/// The registered figures (spec 009 contracts/output-chain-design.md; A-010, A-031, A-032).
pub mod output_template {
    /// The volume control's input coupling corner, Hz (C14 0.1 µF into VR1 50 kΩ; A-031).
    pub const F_VOL_HZ: f64 = 1.0 / (std::f64::consts::TAU * 0.1e-6 * 50e3);
    /// The amplifier's input coupling corner, Hz (C8 0.1 µF into R16 10 kΩ; A-031).
    pub const F_AMP_HZ: f64 = 1.0 / (std::f64::consts::TAU * 0.1e-6 * 10e3);
    /// The earphone follower's input coupling corner, Hz, nominal (A-032).
    pub const F_EAR_IN_HZ: f64 = 120.0;
    /// The earphone follower's output coupling corner into 16 Ω, Hz, nominal (A-032).
    pub const F_EAR_OUT_HZ: f64 = 280.0;
    /// The speaker template's resonance, Hz, and its Q (A-010).
    pub const SPK_F0_HZ: f64 = 750.0;
    /// See [`SPK_F0_HZ`].
    pub const SPK_Q: f64 = 1.8;
    /// The speaker template's shelf zero and pole, Hz (+4.5 dB; A-010).
    pub const SPK_SHELF_ZERO_HZ: f64 = 780.0;
    /// See [`SPK_SHELF_ZERO_HZ`].
    pub const SPK_SHELF_POLE_HZ: f64 = 1310.0;
}

/// A first-order bilinear high-pass `[b0, b1, a1]` at `fc`, prewarped, unity at Nyquist (the
/// design script's formula).
fn template_hp(fc: f64, fs: f64) -> [f64; 3] {
    let t = (std::f64::consts::PI * fc / fs).tan();
    let a = (1.0 - t) / (1.0 + t);
    let g = (1.0 + a) / 2.0;
    [g, -g, -a]
}

/// The shelf `(1 + s/ωz) / (1 + s/ωp)` by the bilinear transform, not prewarped: `[b0, b1, a1]`.
fn template_shelf(fs: f64) -> [f64; 3] {
    let c = 2.0 * fs;
    let (wz, wp) = (
        TAU * output_template::SPK_SHELF_ZERO_HZ,
        TAU * output_template::SPK_SHELF_POLE_HZ,
    );
    let (b0, b1) = (1.0 + c / wz, 1.0 - c / wz);
    let (a0, a1) = (1.0 + c / wp, 1.0 - c / wp);
    [b0 / a0, b1 / a0, a1 / a0]
}

/// The resonance `s² / (s² + (ω0/Q) s + ω0²)` by the bilinear transform prewarped at f0, before
/// normalization: `[b0, b1, b2, a1, a2]`.
fn template_resonance(fs: f64) -> [f64; 5] {
    let w0 = 2.0 * fs * (std::f64::consts::PI * output_template::SPK_F0_HZ / fs).tan();
    let c = 2.0 * fs;
    // s = c (1 − z⁻¹) / (1 + z⁻¹); multiply numerator and denominator by (1 + z⁻¹)².
    let (k1, k2) = (w0 / output_template::SPK_Q, w0 * w0);
    let b = [c * c, -2.0 * c * c, c * c];
    let a = [
        c * c + k1 * c + k2,
        -2.0 * c * c + 2.0 * k2,
        c * c - k1 * c + k2,
    ];
    [
        b[0] / a[0],
        b[1] / a[0],
        b[2] / a[0],
        a[1] / a[0],
        a[2] / a[0],
    ]
}

fn section_response(sec: &[f64], f_hz: f64, fs: f64) -> Complex64 {
    let z1 = Complex64::from_polar(1.0, -TAU * f_hz / fs);
    if sec.len() == 3 {
        (sec[0] + sec[1] * z1) / (1.0 + sec[2] * z1)
    } else {
        (sec[0] + sec[1] * z1 + sec[2] * z1 * z1) / (1.0 + sec[3] * z1 + sec[4] * z1 * z1)
    }
}

/// The sections of a template at `fs`, the speaker's resonance scaled so the template's peak
/// gain on a 1 Hz grid from 20 Hz to Nyquist is exactly 1.0 (spec 009 FR-005).
fn template_sections(template: OutputTemplate, fs: f64) -> Vec<Vec<f64>> {
    let vol = template_hp(output_template::F_VOL_HZ, fs).to_vec();
    match template {
        OutputTemplate::Earphone => vec![
            vol,
            template_hp(output_template::F_EAR_IN_HZ, fs).to_vec(),
            template_hp(output_template::F_EAR_OUT_HZ, fs).to_vec(),
        ],
        OutputTemplate::Amplifier => vec![vol, template_hp(output_template::F_AMP_HZ, fs).to_vec()],
        OutputTemplate::Speaker => {
            let mut res = template_resonance(fs);
            let shelf = template_shelf(fs);
            let mut peak = 0.0f64;
            let mut f = 20.0;
            while f <= fs / 2.0 + 0.5 {
                let m = (section_response(&res, f, fs) * section_response(&shelf, f, fs)).norm();
                peak = peak.max(m);
                f += 1.0;
            }
            for b in &mut res[..3] {
                *b /= peak;
            }
            vec![
                vol,
                template_hp(output_template::F_AMP_HZ, fs).to_vec(),
                res.to_vec(),
                shelf.to_vec(),
            ]
        }
    }
}

/// The design's discrete realization of an output-chain template at `f_hz` and `fs`, in dB
/// (spec 009 research R-10; the only new analysis helper of spec 009). The Rust's measured
/// response is compared against this within ±0.5 dB; the design script separately checks this
/// realization against the analog prototype.
pub fn discrete_template_response(template: OutputTemplate, f_hz: f64, fs: f64) -> f64 {
    let h: Complex64 = template_sections(template, fs)
        .iter()
        .map(|s| section_response(s, f_hz, fs))
        .product();
    db(h.norm())
}

/// The L1 norm (the sum of absolute values) of the discrete impulse response of a template's
/// transducer sections alone, over 2 s at `fs` (spec 010 research R-04; FR-011): the speaker
/// template's resonance and shelf (stages 12), or the earphone's two corners (stage 13). For the
/// speaker route this is a **provable bound** on the route output, because stage 11 is held at
/// 1.0 by the clipper: 1.75 at 6 kHz up to 2.29 at 96 kHz (data-model.md › Expected report).
/// The sections are the design's discrete realization (`template_sections`), which the Rust's
/// committed coefficients match within 0.000 dB (spec 009).
pub fn impulse_l1(template: OutputTemplate, fs: f64) -> f64 {
    let sections = template_sections(template, fs);
    let skip = if template == OutputTemplate::Speaker {
        2
    } else {
        1
    };
    let n = (2.0 * fs) as usize;
    // Direct-form recursions, one state pair per section.
    let mut states: Vec<[f64; 2]> = vec![[0.0; 2]; sections.len() - skip];
    let mut l1 = 0.0;
    for i in 0..n {
        let mut y = if i == 0 { 1.0 } else { 0.0 };
        for (sec, st) in sections[skip..].iter().zip(states.iter_mut()) {
            // Transposed direct form II: y = b0·x + s1; s1 = b1·x − a1·y + s2; s2 = b2·x − a2·y.
            let x = y;
            if sec.len() == 3 {
                y = sec[0] * x + st[0];
                st[0] = sec[1] * x - sec[2] * y;
            } else {
                y = sec[0] * x + st[0];
                st[0] = sec[1] * x - sec[3] * y + st[1];
                st[1] = sec[2] * x - sec[4] * y;
            }
        }
        l1 += y.abs();
    }
    l1
}

/// The template's response without the volume-control corner: the speaker template alone
/// (stage 12) or the earphone's two corners alone (stage 13), in dB.
pub fn discrete_stage_response(template: OutputTemplate, f_hz: f64, fs: f64) -> f64 {
    let sections = template_sections(template, fs);
    let skip = if template == OutputTemplate::Speaker {
        2
    } else {
        1
    };
    let h: Complex64 = sections[skip..]
        .iter()
        .map(|s| section_response(s, f_hz, fs))
        .product();
    db(h.norm())
}

#[cfg(test)]
mod output_template_tests {
    use super::*;

    /// Spec 010 T003 (research R-04): the speaker template's impulse-response L1 norm, the
    /// hardware reviewer's figures recomputed.
    #[test]
    fn speaker_template_impulse_l1_matches_the_reviewed_figures() {
        for (fs, want) in [
            (6000.0, 1.75),
            (8000.0, 1.88),
            (16_000.0, 2.09),
            (44_100.0, 2.24),
            (48_000.0, 2.25),
            (88_200.0, 2.28),
            (96_000.0, 2.29),
        ] {
            let l1 = impulse_l1(OutputTemplate::Speaker, fs);
            assert!(
                (l1 - want).abs() <= 0.02,
                "{fs} Hz: L1 {l1:.4}, want {want}"
            );
        }
        // The earphone corners' L1 is above 1 too (a high-pass overshoots a square edge).
        assert!(impulse_l1(OutputTemplate::Earphone, 48_000.0) > 1.0);
    }

    /// 009 T007: the helper's earphone 1 kHz reading at 48 kHz is within FR-007's −0.5 … 0 dB,
    /// the speaker template's peak is 1.0 within 1e-9 at every rate, and each corner's −3 dB
    /// point from the helper's own coefficients is the registered figure.
    #[test]
    fn helper_reproduces_the_design_figures() {
        let e = discrete_template_response(OutputTemplate::Earphone, 1000.0, 48_000.0);
        assert!((-0.5..=0.0).contains(&e), "{e}");
        for fs in rr_dr60::SUPPORTED_HOST_RATES {
            let fs = f64::from(fs);
            let mut peak = f64::NEG_INFINITY;
            let mut f = 20.0;
            while f <= fs / 2.0 {
                peak = peak.max(
                    discrete_template_response(OutputTemplate::Speaker, f, fs)
                        - discrete_template_response(OutputTemplate::Amplifier, f, fs),
                );
                f += 1.0;
            }
            assert!(peak.abs() < 1e-8, "{fs}: template peak {peak} dB");
            for (fc, name) in [
                (output_template::F_VOL_HZ, "vol"),
                (output_template::F_AMP_HZ, "amp"),
                (output_template::F_EAR_IN_HZ, "ear_in"),
                (output_template::F_EAR_OUT_HZ, "ear_out"),
            ] {
                let c = template_hp(fc, fs);
                let a = -c[2];
                let got = fs * ((1.0 - a) / (1.0 + a)).atan() / std::f64::consts::PI;
                assert!((got - fc).abs() < 1e-6 * fc, "{fs} {name}: {got} vs {fc}");
            }
        }
        // The amplifier path above 800 Hz at 48 kHz is flat within 0.2 dB (FR-006).
        let a800 = discrete_template_response(OutputTemplate::Amplifier, 800.0, 48_000.0);
        let a2500 = discrete_template_response(OutputTemplate::Amplifier, 2500.0, 48_000.0);
        assert!((a2500 - a800).abs() <= 0.2, "{a800} {a2500}");
        // The stage-only reading drops the volume corner.
        let s = discrete_stage_response(OutputTemplate::Speaker, 1000.0, 48_000.0);
        assert!(s < 0.0 && s > -1.0, "{s}");
    }
}
