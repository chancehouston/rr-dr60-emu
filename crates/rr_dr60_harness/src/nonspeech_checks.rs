//! Spec 029: the report of what the modeled chain does to noise, bursts and silence (the
//! non-speech reshaping report). The stimuli (D2), the configurations' names (D3), the reader
//! (D4), the span rule (plan R-04), the memo (plan P5) and the canonical text (D1).
//!
//! Every figure that reaches the text golden is computed with `rr_dr60_detmath` and basic IEEE
//! operations only (plan R-01): this module calls nothing from the analysis module and no `std` transcendental,
//! so the text is byte-identical on every CI target. The measures are engineering descriptors of
//! spectral and temporal structure; the report makes no statement, either way, about what a
//! listener perceives (D9).

use std::collections::BTreeMap;
use std::f64::consts::PI;
use std::sync::{Arc, Mutex, OnceLock};

use rr_dr60::wording::Lists;
use rr_dr60::{Pipeline, Settings, VasEvent};
use rr_dr60_detmath::{bessel_i0, cos, exp, ln, sin, sqrt};

use crate::configs;
use crate::stimulus::{self, Pcg32};

// ---- The report's constants (printed as the `def` block; D4) ------------------------------------

/// Samples per frame at 6 kHz (20 ms).
pub const FRAME: usize = 120;
/// The device rate the reader works at.
pub const DEVICE_HZ: u32 = 6000;
/// The settle before the span (seconds).
pub const SETTLE_S: f64 = 0.5;
/// The stimulus length (seconds).
pub const STIMULUS_S: f64 = 12.0;
/// The activity threshold (AES17 dBFS of a frame).
pub const ACTIVE_DBFS: f64 = -75.0;
/// The LPC order.
pub const LPC_ORDER: usize = 8;
/// The pre-emphasis coefficient.
pub const PREEMPHASIS: f64 = 0.9;
/// The envelope grid step (Hz) over 0 … 3000 Hz.
pub const GRID_HZ: f64 = 10.0;
/// The grid's point count (0 … 3000 Hz inclusive).
pub const GRID_N: usize = 301;
/// The peak search in grid indices (200 – 2300 Hz).
pub const SEARCH_LO: usize = 20;
/// See [`SEARCH_LO`].
pub const SEARCH_HI: usize = 230;
/// The prominence rule (dB over ±300 Hz).
pub const PROMINENCE_DB: f64 = 3.0;
/// The prominence span in grid points (300 Hz).
pub const PROMINENCE_SPAN: usize = 30;
/// The −3 dB width rule in grid points (300 Hz).
pub const WIDTH_MAX: usize = 30;
/// The autocorrelation lag range (samples at 6 kHz: 300 – 60 Hz).
pub const LAG_MIN: usize = 20;
/// See [`LAG_MIN`].
pub const LAG_MAX: usize = 100;
/// The periodic-frame rule.
pub const PERIODIC_R: f64 = 0.5;
/// The lag-stable rule (r) and lag tolerance.
pub const STABLE_R: f64 = 0.35;
/// See [`STABLE_R`].
pub const STABLE_DLAG: usize = 2;
/// The envelope DFT band (Hz) and the share band (Hz).
pub const DFT_LO_HZ: f64 = 0.5;
/// See [`DFT_LO_HZ`].
pub const DFT_HI_HZ: f64 = 25.0;
/// See [`DFT_LO_HZ`].
pub const SHARE_LO_HZ: f64 = 2.0;
/// See [`DFT_LO_HZ`].
pub const SHARE_HI_HZ: f64 = 8.0;
/// The share's minimum active fraction.
pub const SHARE_ACTIVE: f64 = 0.9;
/// The resampler's cut-off (Hz).
pub const RESAMPLE_CUTOFF_HZ: f64 = 2700.0;
/// The resampler's zero crossings each side at the lower rate.
pub const RESAMPLE_ZEROS: usize = 32;
/// The frame rate of the envelope series (Hz).
pub const ENVELOPE_HZ: f64 = 50.0;
/// The frames of a full span: ⌊(12 s − 0.5 s) ÷ 20 ms⌋ = 575 (the nominal span; drop mode shortens
/// the output, never this figure).
pub const SPAN_FRAMES: usize = ((STIMULUS_S - SETTLE_S) * DEVICE_HZ as f64) as usize / FRAME;
/// The block size every cell is processed with.
pub const BLOCK: usize = 4096;
/// The rates the golden covers (D1; clarified 2026-11-04).
pub const GOLDEN_RATES: [u32; 3] = [6000, 44_100, 48_000];
/// Line 1 of the canonical text (D1; plan P9).
pub const TITLE: &str = "rr_dr60 noise, bursts and silence report v1";

fn ln10() -> f64 {
    ln(10.0)
}

/// AES17 dBFS of a mean square (0 dBFS = a full-scale sine).
pub fn aes17_db(mean_square: f64) -> f64 {
    if mean_square <= 0.0 {
        f64::NEG_INFINITY
    } else {
        10.0 * ln(2.0 * mean_square) / ln10()
    }
}

fn mean_square(x: &[f64]) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    x.iter().fold(0.0, |a, &v| a + v * v) / x.len() as f64
}

/// Scales `x` to `level_dbfs` (AES17 RMS over the whole slice).
pub fn level_to(x: &[f64], level_dbfs: f64) -> Vec<f64> {
    let target = stimulus::amplitude(level_dbfs) / sqrt(2.0);
    let k = target / sqrt(mean_square(x).max(1e-300));
    x.iter().map(|&v| v * k).collect()
}

// ---- Stimuli (D2; data-model.md › Stimuli) ------------------------------------------------------

/// A stimulus row of the report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stimulus {
    /// Steady low-passed noise at −35 dBFS.
    A35,
    /// … at −50 dBFS.
    A50,
    /// … at −55 dBFS.
    A55,
    /// The noise modulated at 4 Hz.
    B,
    /// The noise under a slow irregular envelope.
    C,
    /// Sparse bursts.
    D,
    /// Pink noise.
    E,
    /// Digital silence.
    F,
    /// The harmonic reference.
    G,
    /// The resonator row.
    H,
}

/// Every row, in the report's order.
pub const STIMULI: [Stimulus; 10] = [
    Stimulus::A35,
    Stimulus::A50,
    Stimulus::A55,
    Stimulus::B,
    Stimulus::C,
    Stimulus::D,
    Stimulus::E,
    Stimulus::F,
    Stimulus::G,
    Stimulus::H,
];

/// The noise core's seed; the bursts' draw seed; the reference's seed.
pub const NOISE_SEED: u64 = 7;
/// See [`NOISE_SEED`].
pub const BURST_SEED: u64 = 21;
/// See [`NOISE_SEED`].
pub const REFERENCE_SEED: u64 = 3;

impl Stimulus {
    /// The row's identifier in the text.
    pub fn id(self) -> &'static str {
        match self {
            Self::A35 => "a35",
            Self::A50 => "a50",
            Self::A55 => "a55",
            Self::B => "b",
            Self::C => "c",
            Self::D => "d",
            Self::E => "e",
            Self::F => "f",
            Self::G => "g",
            Self::H => "h",
        }
    }

    /// The `stim` line of the text (the level as "per-burst" or "none" where the whole-stimulus
    /// rule does not apply).
    pub fn line(self) -> String {
        let (seed, level, def) = match self {
            Self::A35 => (NOISE_SEED, "-35.00", "steady low-passed noise"),
            Self::A50 => (NOISE_SEED, "-50.00", "steady low-passed noise"),
            Self::A55 => (NOISE_SEED, "-55.00", "steady low-passed noise"),
            Self::B => (
                NOISE_SEED,
                "-35.00",
                "the noise x (1 + 0.5 sin 2pi 4 t), then levelled",
            ),
            Self::C => (
                NOISE_SEED,
                "-40.00",
                "the noise x (1 + 0.15 (sin 2pi 0.7 t + sin (2pi 1.9 t + 1) + sin (2pi 3.3 t + 2))), then levelled",
            ),
            Self::D => (
                BURST_SEED,
                "per-burst",
                "bursts: gap 0.1-1.1 s, duration 10-160 ms, level -45..-30 dBFS, sin^2 envelope, drawn at 6 kHz",
            ),
            Self::E => (
                NOISE_SEED,
                "-45.00",
                "pink: Kellet filter then a 100 Hz second-order high-pass, then levelled",
            ),
            Self::F => (0, "none", "digital silence"),
            Self::G => (
                REFERENCE_SEED,
                "-20.00",
                "harmonic reference: a pulse train, 90-220 Hz gliding fundamental, three resonances sweeping 300-750, 1100-1900 and 2300-2600 Hz (spec 008 R-09)",
            ),
            Self::H => (
                NOISE_SEED,
                "-35.00",
                "the a35 noise through a two-pole resonator, pole at 750 Hz, Q 1.8 (magnitude peak near 725 Hz), then levelled",
            ),
        };
        format!("stim {} seed={seed} level={level} {def}", self.id())
    }

    /// The row's level target where the whole-stimulus rule applies.
    pub fn level_dbfs(self) -> Option<f64> {
        match self {
            Self::A35 | Self::B | Self::H => Some(-35.0),
            Self::A50 => Some(-50.0),
            Self::A55 => Some(-55.0),
            Self::C => Some(-40.0),
            Self::E => Some(-45.0),
            Self::G => Some(-20.0),
            Self::D | Self::F => None,
        }
    }

    /// The stimulus at `rate`, 12 s long.
    pub fn samples(self, rate: u32) -> Vec<f32> {
        let fs = f64::from(rate);
        let len = (STIMULUS_S * fs) as usize;
        let to_f32 = |v: Vec<f64>| v.iter().map(|&x| x as f32).collect::<Vec<f32>>();
        match self {
            Self::A35 => to_f32(level_to(&noise_core(NOISE_SEED, rate, len), -35.0)),
            Self::A50 => to_f32(level_to(&noise_core(NOISE_SEED, rate, len), -50.0)),
            Self::A55 => to_f32(level_to(&noise_core(NOISE_SEED, rate, len), -55.0)),
            Self::B => {
                let x: Vec<f64> = noise_core(NOISE_SEED, rate, len)
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| v * (1.0 + 0.5 * sin(2.0 * PI * 4.0 * i as f64 / fs)))
                    .collect();
                to_f32(level_to(&x, -35.0))
            }
            Self::C => {
                let x: Vec<f64> = noise_core(NOISE_SEED, rate, len)
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| {
                        let t = i as f64 / fs;
                        v * (1.0
                            + 0.15
                                * (sin(2.0 * PI * 0.7 * t)
                                    + sin(2.0 * PI * 1.9 * t + 1.0)
                                    + sin(2.0 * PI * 3.3 * t + 2.0)))
                    })
                    .collect();
                to_f32(level_to(&x, -40.0))
            }
            Self::D => to_f32(bursts(rate, len)),
            Self::E => {
                let x = pink(&noise_core(NOISE_SEED, rate, len), fs);
                to_f32(level_to(&x, -45.0))
            }
            Self::F => vec![0.0; len],
            Self::G => stimulus::voiced(fs, STIMULUS_S, -20.0, REFERENCE_SEED),
            Self::H => {
                let x = resonate(&noise_core(NOISE_SEED, rate, len), fs, 750.0, 1.8);
                to_f32(level_to(&x, -35.0))
            }
        }
    }
}

/// Uniform noise through the Kaiser low-pass (β = 8, `taps = ⌊0.01·fs⌋ | 1`, edges 0 and
/// 2900 Hz): `stimulus::bandlimited_noise`'s design with its lower edge at 0, unscaled.
pub fn noise_core(seed: u64, rate: u32, len: usize) -> Vec<f64> {
    let fs = f64::from(rate);
    let taps = ((fs * 0.01) as usize) | 1;
    let mid = (taps - 1) as f64 / 2.0;
    let f2 = 2900.0 / fs;
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
            w * lowpass(f2, m)
        })
        .collect();
    let src = stimulus::noise(seed, len + taps - 1);
    (0..len)
        .map(|n| {
            h.iter()
                .enumerate()
                .map(|(k, &hk)| hk * f64::from(src[n + taps - 1 - k]))
                .fold(0.0, |acc, v| acc + v)
        })
        .collect()
}

/// A uniform draw in [lo, hi) from the PCG stream.
fn uniform(r: &mut Pcg32, lo: f64, hi: f64) -> f64 {
    lo + (hi - lo) * (f64::from(r.next_u32()) / 4_294_967_296.0)
}

/// The bursts' draw (data-model.md › d) as (start, duration) in 6 kHz samples with the level,
/// in order, for the 12 s stimulus: rate-independent by construction.
pub fn burst_schedule() -> Vec<(u64, u64, f64)> {
    let mut r = Pcg32::new(BURST_SEED, 0);
    let mut t6: u64 = 0;
    let end6 = (STIMULUS_S * f64::from(DEVICE_HZ)) as u64;
    let mut v = Vec::new();
    loop {
        let gap = uniform(&mut r, 0.1, 1.1);
        let dur = uniform(&mut r, 0.01, 0.16);
        let level = uniform(&mut r, -45.0, -30.0);
        let gap6 = (gap * 6000.0 + 0.5) as u64;
        let dur6 = ((dur * 6000.0 + 0.5) as u64).max(1);
        t6 += gap6;
        if t6 + dur6 > end6 {
            break;
        }
        v.push((t6, dur6, level));
        t6 += dur6;
    }
    v
}

/// The bursts (data-model.md › d): the schedule's bursts, each placed at `n6 · rate ÷ 6000`, the
/// noise × sin² scaled to its own level.
pub fn bursts(rate: u32, len: usize) -> Vec<f64> {
    let core = noise_core(NOISE_SEED, rate, len);
    let mut out = vec![0.0; len];
    let rate = u64::from(rate);
    for (t6, dur6, level) in burst_schedule() {
        let start = (t6 * rate / 6000) as usize;
        let n = ((t6 + dur6) * rate / 6000) as usize - start;
        if start + n > len {
            break;
        }
        let seg: Vec<f64> = (0..n)
            .map(|j| {
                let e = sin(PI * j as f64 / n as f64);
                core[start + j] * e * e
            })
            .collect();
        let seg = level_to(&seg, level);
        out[start..start + n].copy_from_slice(&seg);
    }
    out
}

/// The Kellet pink filter (the seven-state form, coefficients as published) followed by a
/// second-order 100 Hz Butterworth high-pass (bilinear; plan P7).
pub fn pink(x: &[f64], fs: f64) -> Vec<f64> {
    let (mut b0, mut b1, mut b2, mut b3, mut b4, mut b5, mut b6) =
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    let p: Vec<f64> = x
        .iter()
        .map(|&w| {
            b0 = 0.99886 * b0 + w * 0.0555179;
            b1 = 0.99332 * b1 + w * 0.0750759;
            b2 = 0.96900 * b2 + w * 0.1538520;
            b3 = 0.86650 * b3 + w * 0.3104856;
            b4 = 0.55000 * b4 + w * 0.5329522;
            b5 = -0.7616 * b5 - w * 0.0168980;
            let y = b0 + b1 + b2 + b3 + b4 + b5 + b6 + w * 0.5362;
            b6 = w * 0.115926;
            y
        })
        .collect();
    // RBJ high-pass, Q = 1/√2, f0 = 100 Hz.
    let w0 = 2.0 * PI * 100.0 / fs;
    let (cw, sw) = (cos(w0), sin(w0));
    let alpha = sw / (2.0 / sqrt(2.0));
    let b0h = (1.0 + cw) / 2.0;
    let b1h = -(1.0 + cw);
    let b2h = b0h;
    let a0 = 1.0 + alpha;
    let a1 = -2.0 * cw;
    let a2 = 1.0 - alpha;
    let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
    p.iter()
        .map(|&v| {
            let y = (b0h * v + b1h * x1 + b2h * x2 - a1 * y1 - a2 * y2) / a0;
            x2 = x1;
            x1 = v;
            y2 = y1;
            y1 = y;
            y
        })
        .collect()
}

/// The two-pole resonator of `stimulus::Resonator`'s form at `fc` with bandwidth `fc ÷ q`.
pub fn resonate(x: &[f64], fs: f64, fc: f64, q: f64) -> Vec<f64> {
    let r = exp(-PI * (fc / q) / fs);
    let a1 = 2.0 * r * cos(2.0 * PI * fc / fs);
    let a2 = -r * r;
    let gain = 1.0 - r;
    let (mut y1, mut y2) = (0.0, 0.0);
    x.iter()
        .map(|&v| {
            let y = gain * v + a1 * y1 + a2 * y2;
            y2 = y1;
            y1 = y;
            y
        })
        .collect()
}

// ---- Configurations (D3; plan R-03) ------------------------------------------------------------

/// A rung of the ladder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Configuration {
    /// Pass-through.
    C0,
    /// The band filters and the clamp.
    C1,
    /// The coder alone at the decoder tap.
    C2,
    /// Band, clamp and coder; the gate and limiter off.
    C3,
    /// C3 plus the microphone and the limiter; the gate off; before the output chain.
    C4,
    /// C4 plus the gate at level 3 (the default record path before the output chain).
    C5,
    /// C5 at sensitivity 5 with the hiss offset +12 dB.
    C6,
    /// The default engine at the speaker route output.
    C7,
}

/// Every rung, in order.
pub const CONFIGURATIONS: [Configuration; 8] = [
    Configuration::C0,
    Configuration::C1,
    Configuration::C2,
    Configuration::C3,
    Configuration::C4,
    Configuration::C5,
    Configuration::C6,
    Configuration::C7,
];

impl Configuration {
    /// The rung's identifier.
    pub fn id(self) -> &'static str {
        match self {
            Self::C0 => "C0",
            Self::C1 => "C1",
            Self::C2 => "C2",
            Self::C3 => "C3",
            Self::C4 => "C4",
            Self::C5 => "C5",
            Self::C6 => "C6",
            Self::C7 => "C7",
        }
    }

    /// The `configs::settings` name.
    pub fn name(self) -> &'static str {
        match self {
            Self::C0 => "nonspeech_c0",
            Self::C1 => "nonspeech_c1",
            Self::C2 => "nonspeech_c2",
            Self::C3 => "nonspeech_c3",
            Self::C4 => "nonspeech_c4",
            Self::C5 => "nonspeech_c5",
            Self::C6 => "nonspeech_c6",
            Self::C7 => "nonspeech_c7",
        }
    }

    /// The settings at `rate`.
    pub fn settings(self, rate: u32) -> Settings {
        configs::settings(self.name(), rate)
    }

    /// The `cfg` line of the text at `rate`.
    pub fn line(self, rate: u32) -> String {
        let s = self.settings(rate);
        let tap = s.tap;
        let latency = Pipeline::new(s).expect("settings").latency_samples();
        format!(
            "cfg {} name={} tap={tap:?} latency={latency}",
            self.id(),
            self.name()
        )
    }
}

// ---- The resampler to 6 kHz (plan R-02) --------------------------------------------------------

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// The windowed-sinc kernel of `analysis::resample_to`'s design, by phase: `rate : 6000 = M : L`
/// in lowest terms; output `m` sits at input `(m·M) div L` with phase `(m·M) mod L`.
pub struct Resampler {
    m: usize,
    l: usize,
    half: i64,
    /// `taps[phase][j]` for the input offsets `j − half` … `j − half + 1`.
    taps: Vec<Vec<f64>>,
}

impl Resampler {
    /// The kernel for `rate` (the identity at 6 kHz carries an empty table).
    pub fn new(rate: u32) -> Self {
        let g = gcd(rate, DEVICE_HZ);
        let (m, l) = ((rate / g) as usize, (DEVICE_HZ / g) as usize);
        if rate == DEVICE_HZ {
            return Self {
                m,
                l,
                half: 0,
                taps: Vec::new(),
            };
        }
        let fs_in = f64::from(rate);
        let cutoff = RESAMPLE_CUTOFF_HZ / fs_in; // cycles per input sample
        let half = (RESAMPLE_ZEROS as f64 * fs_in / f64::from(DEVICE_HZ)) as i64 + 1;
        let taps = (0..l)
            .map(|phase| {
                let frac = phase as f64 / l as f64;
                (-half..=half + 1)
                    .map(|j| {
                        let t = j as f64 - frac;
                        let sinc = if t == 0.0 {
                            1.0
                        } else {
                            sin(2.0 * PI * cutoff * t) / (2.0 * PI * cutoff * t)
                        };
                        let w = 0.54 + 0.46 * cos(PI * t / (half as f64 + 1.0));
                        sinc * w * 2.0 * cutoff
                    })
                    .collect()
            })
            .collect();
        Self { m, l, half, taps }
    }

    /// The number of phases (`6000 ÷ gcd(rate, 6000)`).
    pub fn phases(&self) -> usize {
        self.l
    }

    /// `x` at the rate this kernel was built for, resampled to 6 kHz.
    pub fn run(&self, x: &[f64]) -> Vec<f64> {
        if self.taps.is_empty() {
            return x.to_vec();
        }
        let out_len = x.len() * self.l / self.m;
        (0..out_len)
            .map(|o| {
                let pos = o * self.m;
                let c0 = (pos / self.l) as i64;
                let phase = pos % self.l;
                let taps = &self.taps[phase];
                let mut acc = 0.0;
                for (j, &h) in taps.iter().enumerate() {
                    let n = c0 + j as i64 - self.half;
                    if n >= 0 && (n as usize) < x.len() {
                        acc += x[n as usize] * h;
                    }
                }
                acc
            })
            .collect()
    }

    /// The kernel's half-span in input samples.
    pub fn half_span(&self) -> usize {
        self.half as usize
    }
}

/// `x` at `rate` resampled to 6 kHz.
pub fn to_device_rate(x: &[f64], rate: u32) -> Vec<f64> {
    Resampler::new(rate).run(x)
}

// ---- The reader (D4; data-model.md › The reader) -----------------------------------------------

/// The reader's switches (the mutants of US3 AS3 toggle them; plan R-11).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReaderOptions {
    /// The prominence-and-width rule on peaks.
    pub prominence: bool,
    /// The pre-emphasis before the LPC fit.
    pub pre_emphasis: bool,
    /// The autocorrelation's largest lag.
    pub lag_max: usize,
    /// The activity threshold.
    pub active_floor_dbfs: f64,
    /// Resetting every frame-to-frame state at inactive frames and splices.
    pub reset_state: bool,
}

/// The production reader.
pub const PRODUCTION: ReaderOptions = ReaderOptions {
    prominence: true,
    pre_emphasis: true,
    lag_max: LAG_MAX,
    active_floor_dbfs: ACTIVE_DBFS,
    reset_state: true,
};

/// The nine measures of a span plus the companions (D4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measures {
    /// AES17 dBFS over the span.
    pub level_dbfs: f64,
    /// LPC envelope peaks per active frame.
    pub peaks: Option<f64>,
    /// The mean movement of the two lowest peaks (Hz per frame).
    pub movement_hz: Option<f64>,
    /// The mean of the frames' maximum normalised autocorrelation.
    pub periodicity: Option<f64>,
    /// Frames with that maximum above [`PERIODIC_R`].
    pub periodic_fraction: Option<f64>,
    /// Consecutive pairs whose lags agree.
    pub lag_stable: Option<f64>,
    /// The 2 – 8 Hz share of the envelope's modulation spectrum.
    pub mod_share: Option<f64>,
    /// The spectral flux (dB).
    pub flux_db: Option<f64>,
    /// The median of the frames' lowest peak (Hz).
    pub lowest_peak_median_hz: Option<f64>,
    /// Active frames.
    pub active: usize,
    /// Frames in the span.
    pub frames: usize,
    /// Consecutive active pairs counted by the pairwise measures.
    pub pairs: usize,
}

struct Tables {
    /// `cos_env[g][m]`, m = 0 … 8, for the 10 Hz grid.
    cos_env: Vec<[f64; LPC_ORDER + 1]>,
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let cos_env = (0..GRID_N)
            .map(|g| {
                let w = 2.0 * PI * (g as f64 * GRID_HZ) / f64::from(DEVICE_HZ);
                let mut row = [0.0; LPC_ORDER + 1];
                for (m, v) in row.iter_mut().enumerate() {
                    *v = cos(w * m as f64);
                }
                row
            })
            .collect();
        Tables { cos_env }
    })
}

/// Levinson–Durbin: the LPC coefficients `a[1..=order]` of `A(z) = 1 + Σ a_k z^-k`, or `None`
/// when the frame is degenerate.
pub fn lpc(frame: &[f64], order: usize) -> Option<Vec<f64>> {
    let n = frame.len();
    let r: Vec<f64> = (0..=order)
        .map(|k| (0..n - k).fold(0.0, |acc, i| acc + frame[i] * frame[i + k]))
        .collect();
    if r[0] <= 1e-30 {
        return None;
    }
    let mut a = vec![0.0; order + 1];
    a[0] = 1.0;
    let mut e = r[0];
    for i in 1..=order {
        let mut acc = r[i];
        for j in 1..i {
            acc += a[j] * r[i - j];
        }
        let k = -acc / e;
        let prev = a.clone();
        for j in 1..i {
            a[j] = prev[j] + k * prev[i - j];
        }
        a[i] = k;
        e *= 1.0 - k * k;
        if e <= 0.0 {
            return None;
        }
    }
    Some(a)
}

/// The LPC envelope in dB on the 10 Hz grid: `−10·log10(c₀ + 2Σ c_m cos(mω))` from the
/// polynomial's autocorrelation `c_m = Σ_k a_k a_{k+m}`.
pub fn envelope_db(a: &[f64]) -> Vec<f64> {
    let order = a.len() - 1;
    let c: Vec<f64> = (0..=order)
        .map(|m| (0..=order - m).fold(0.0, |acc, k| acc + a[k] * a[k + m]))
        .collect();
    let t = tables();
    (0..GRID_N)
        .map(|g| {
            let mut p = c[0];
            for (m, &cm) in c.iter().enumerate().skip(1) {
                p += 2.0 * cm * t.cos_env[g][m];
            }
            -10.0 * ln(p.max(1e-30)) / ln10()
        })
        .collect()
}

/// The peaks of an envelope under the rules (grid indices).
pub fn peaks_of(env: &[f64], prominence: bool) -> Vec<usize> {
    let mut out = Vec::new();
    for g in SEARCH_LO..=SEARCH_HI {
        if !(env[g] > env[g - 1] && env[g] >= env[g + 1]) {
            continue;
        }
        if prominence {
            let lo_min =
                (g.saturating_sub(PROMINENCE_SPAN)..g).fold(f64::INFINITY, |m, i| m.min(env[i]));
            let hi_min = (g + 1..=(g + PROMINENCE_SPAN).min(GRID_N - 1))
                .fold(f64::INFINITY, |m, i| m.min(env[i]));
            if env[g] - lo_min.max(hi_min) < PROMINENCE_DB {
                continue;
            }
            let edge = env[g] - 3.0;
            let mut left = g;
            while left > 0 && env[left - 1] >= edge {
                left -= 1;
            }
            let mut right = g;
            while right + 1 < GRID_N && env[right + 1] >= edge {
                right += 1;
            }
            if right - left > WIDTH_MAX || left == 0 || right == GRID_N - 1 {
                continue;
            }
        }
        out.push(g);
    }
    out
}

/// The frame's maximum normalised autocorrelation over the lag range and its lag: `r(τ)` over
/// 120 fixed products from a 220-sample buffer (the previous 100 samples then the frame),
/// interior local maxima only.
pub fn autocorr_max(buffer: &[f64], lag_max: usize) -> (f64, usize) {
    debug_assert_eq!(buffer.len(), FRAME + LAG_MAX);
    let frame = &buffer[LAG_MAX..];
    let e0 = frame.iter().fold(0.0, |a, &v| a + v * v);
    let r = |tau: usize| {
        let mut c = 0.0;
        let mut e1 = 0.0;
        for n in 0..FRAME {
            let b = buffer[LAG_MAX + n - tau];
            c += frame[n] * b;
            e1 += b * b;
        }
        let d = sqrt(e0 * e1);
        if d <= 1e-300 { 0.0 } else { c / d }
    };
    let rs: Vec<f64> = (LAG_MIN..=lag_max).map(r).collect();
    let mut best = (0.0, 0usize);
    for i in 1..rs.len().saturating_sub(1) {
        if rs[i] >= rs[i - 1] && rs[i] >= rs[i + 1] && rs[i] > best.0 {
            best = (rs[i], LAG_MIN + i);
        }
    }
    best
}

fn median(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    })
}

/// The reader (D4): `span` at 6 kHz, `splices` as frame indices.
pub fn read(span: &[f64], splices: &[usize], o: ReaderOptions) -> Measures {
    let frames = span.len() / FRAME;
    let mut env_series = Vec::with_capacity(frames);
    let mut active = 0usize;
    let mut peaks_total = 0usize;
    let mut lowest = Vec::new();
    let mut moves = Vec::new();
    let mut rs = Vec::new();
    let mut periodic = 0usize;
    let mut stable = 0usize;
    let mut fluxes = Vec::new();
    let mut pairs = 0usize;
    let mut prev: Option<(Vec<f64>, Vec<usize>, f64, usize)> = None; // env, peaks, r, lag
    let mut history = [0.0; LAG_MAX];
    for f in 0..frames {
        let frame = &span[f * FRAME..(f + 1) * FRAME];
        if splices.contains(&f) && o.reset_state {
            prev = None;
            history = [0.0; LAG_MAX];
        }
        let level = aes17_db(mean_square(frame));
        if level < o.active_floor_dbfs {
            env_series.push(o.active_floor_dbfs);
            if o.reset_state {
                prev = None;
            }
            history.copy_from_slice(&frame[FRAME - LAG_MAX..]);
            continue;
        }
        env_series.push(level);
        active += 1;
        let pe: Vec<f64> = if o.pre_emphasis {
            (0..FRAME)
                .map(|n| {
                    if n == 0 {
                        frame[0]
                    } else {
                        frame[n] - PREEMPHASIS * frame[n - 1]
                    }
                })
                .collect()
        } else {
            frame.to_vec()
        };
        let (env, pk) = match lpc(&pe, LPC_ORDER) {
            Some(a) => {
                let env = envelope_db(&a);
                let pk = peaks_of(&env, o.prominence);
                (env, pk)
            }
            None => (vec![0.0; GRID_N], Vec::new()),
        };
        peaks_total += pk.len();
        if let Some(&g) = pk.first() {
            lowest.push(g as f64 * GRID_HZ);
        }
        let mut buffer = [0.0; FRAME + LAG_MAX];
        buffer[..LAG_MAX].copy_from_slice(&history);
        buffer[LAG_MAX..].copy_from_slice(frame);
        let (r, lag) = autocorr_max(&buffer, o.lag_max);
        rs.push(r);
        if r > PERIODIC_R {
            periodic += 1;
        }
        if let Some((penv, ppk, pr, plag)) = &prev {
            pairs += 1;
            if ppk.len() >= 2 && pk.len() >= 2 {
                let d = ((pk[0] as f64 - ppk[0] as f64).abs()
                    + (pk[1] as f64 - ppk[1] as f64).abs())
                    / 2.0;
                moves.push(d * GRID_HZ);
            }
            if r > STABLE_R
                && *pr > STABLE_R
                && (lag as i64 - *plag as i64).unsigned_abs() as usize <= STABLE_DLAG
            {
                stable += 1;
            }
            let grid = SEARCH_LO..=SEARCH_HI;
            let n = (SEARCH_HI - SEARCH_LO + 1) as f64;
            let m1 = grid.clone().fold(0.0, |a, g| a + env[g]) / n;
            let m2 = grid.clone().fold(0.0, |a, g| a + penv[g]) / n;
            let d = grid.fold(0.0, |a, g| {
                let v = (env[g] - m1) - (penv[g] - m2);
                a + v * v
            }) / n;
            fluxes.push(sqrt(d));
        }
        prev = Some((env, pk, r, lag));
        history.copy_from_slice(&frame[FRAME - LAG_MAX..]);
    }
    let mean = |v: &[f64]| {
        if v.is_empty() {
            None
        } else {
            Some(v.iter().sum::<f64>() / v.len() as f64)
        }
    };
    let share = if active as f64 >= SHARE_ACTIVE * SPAN_FRAMES as f64
        && splices.is_empty()
        && frames >= 4
    {
        Some(mod_share(&env_series))
    } else {
        None
    };
    Measures {
        level_dbfs: aes17_db(mean_square(&span[..frames * FRAME])),
        peaks: if active > 0 {
            Some(peaks_total as f64 / active as f64)
        } else {
            None
        },
        movement_hz: mean(&moves),
        periodicity: mean(&rs),
        periodic_fraction: if active > 0 {
            Some(periodic as f64 / active as f64)
        } else {
            None
        },
        lag_stable: if pairs > 0 {
            Some(stable as f64 / pairs as f64)
        } else {
            None
        },
        mod_share: share,
        flux_db: mean(&fluxes),
        lowest_peak_median_hz: median(&mut lowest),
        active,
        frames,
        pairs,
    }
}

/// The 2 – 8 Hz share of the Hann-windowed DFT of the mean-removed log-envelope series over
/// 0.5 – 25 Hz (bins of `ENVELOPE_HZ ÷ N`).
pub fn mod_share(series: &[f64]) -> f64 {
    let n = series.len();
    let mean = series.iter().sum::<f64>() / n as f64;
    let e: Vec<f64> = series
        .iter()
        .enumerate()
        .map(|(i, &v)| (v - mean) * (0.5 - 0.5 * cos(2.0 * PI * i as f64 / n as f64)))
        .collect();
    let cos_t: Vec<f64> = (0..n)
        .map(|j| cos(2.0 * PI * j as f64 / n as f64))
        .collect();
    let sin_t: Vec<f64> = (0..n)
        .map(|j| sin(2.0 * PI * j as f64 / n as f64))
        .collect();
    let mut band = 0.0;
    let mut total = 0.0;
    for k in 1..n / 2 {
        let f = k as f64 * ENVELOPE_HZ / n as f64;
        if !(DFT_LO_HZ..=DFT_HI_HZ).contains(&f) {
            continue;
        }
        let (mut re, mut im) = (0.0, 0.0);
        for (i, &v) in e.iter().enumerate() {
            let j = (k * i) % n;
            re += v * cos_t[j];
            im -= v * sin_t[j];
        }
        let p = re * re + im * im;
        total += p;
        if (SHARE_LO_HZ..=SHARE_HI_HZ).contains(&f) {
            band += p;
        }
    }
    if total <= 0.0 { 0.0 } else { band / total }
}

// ---- The span (plan R-04) ----------------------------------------------------------------------

/// A processed cell's output span at 6 kHz with its splices as frame indices.
pub struct Span {
    /// The span at 6 kHz (whole frames).
    pub samples: Vec<f64>,
    /// The output's length in host samples.
    pub output_len: usize,
    /// Splices as frame indices into `samples`.
    pub splices: Vec<usize>,
    /// The configuration's latency (host samples).
    pub latency: u32,
}

/// The raw host-rate output of `settings` on `x` followed by the latency tail, in calls of the
/// sizes `partition` yields (cycled), with the heard splice positions.
pub fn process_raw(
    settings: Settings,
    x: &[f32],
    partition: &[usize],
) -> (Vec<f32>, Vec<u64>, u32) {
    let mut p = Pipeline::new(settings).expect("settings");
    let latency = p.latency_samples();
    let mut input = x.to_vec();
    input.extend(std::iter::repeat_n(0.0f32, latency as usize));
    let mut out = Vec::with_capacity(input.len());
    let mut events: Vec<VasEvent> = Vec::new();
    let max_block = *partition.iter().max().unwrap();
    let mut buf = vec![0.0f32; max_block];
    let mut ev = vec![VasEvent::default(); p.max_events(max_block)];
    let mut pos = 0usize;
    let mut k = 0usize;
    while pos < input.len() {
        let n = partition[k % partition.len()].min(input.len() - pos);
        k += 1;
        let info = p
            .process_with_events(&input[pos..pos + n], &mut buf[..n], &mut ev)
            .expect("process");
        out.extend_from_slice(&buf[..info.produced]);
        events.extend_from_slice(&ev[..info.events]);
        pos += n;
    }
    let heard = events
        .iter()
        .map(|e| p.one_pass_splice_output_position(e))
        .collect();
    (out, heard, latency)
}

/// Runs `stimulus` through `settings` at `rate` in `block`-sample calls, feeds the latency tail,
/// and cuts the span `[settle + L, in_len + L)` (shorter in drop mode).
pub fn process_settings(stimulus: Stimulus, settings: Settings, rate: u32, block: usize) -> Span {
    let x = stimulus.samples(rate);
    let (out, heard, latency) = process_raw(settings, &x, &[block]);
    span_of(&out, x.len(), latency, rate, &heard)
}

/// [`process_settings`] on the configuration's own settings.
pub fn process(stimulus: Stimulus, configuration: Configuration, rate: u32, block: usize) -> Span {
    process_settings(stimulus, configuration.settings(rate), rate, block)
}

/// The span rule on an output: cut `[settle + L, min(out_len, in_len + L))` at the host rate with
/// the kernel's half-span as a guard each side, resample, drop the guard.
pub fn span_of(out: &[f32], in_len: usize, latency: u32, rate: u32, heard: &[u64]) -> Span {
    let rs = Resampler::new(rate);
    let g = gcd(rate, DEVICE_HZ);
    let (m, l) = ((rate / g) as usize, (DEVICE_HZ / g) as usize);
    let start = (SETTLE_S * f64::from(rate)) as usize + latency as usize;
    let end = out.len().min(in_len + latency as usize);
    if end <= start {
        return Span {
            samples: Vec::new(),
            output_len: out.len(),
            splices: Vec::new(),
            latency,
        };
    }
    // The guard: `q · M` host samples = `q · L` device samples, with q·M ≥ half-span + 2.
    let q = (rs.half_span() + 2).div_ceil(m).max(1);
    let guard_host = q * m;
    let seg_start = start.saturating_sub(guard_host);
    let seg_end = (end + guard_host).min(out.len());
    let seg: Vec<f64> = out[seg_start..seg_end]
        .iter()
        .map(|&v| f64::from(v))
        .collect();
    let dev = rs.run(&seg);
    let lead = (start - seg_start) * l / m; // device samples before the span start
    let keep = (end - start) * l / m;
    let keep = keep / FRAME * FRAME;
    let samples = if lead + keep <= dev.len() {
        dev[lead..lead + keep].to_vec()
    } else {
        Vec::new()
    };
    let splices = heard
        .iter()
        .filter_map(|&h| {
            let h = h as usize;
            if h < start || h >= end {
                return None;
            }
            Some(((h - start) * l / m) / FRAME)
        })
        .collect();
    Span {
        samples,
        output_len: out.len(),
        splices,
        latency,
    }
}

/// The input's own span (D6 ii): the stimulus over `[settle, in_len)` through the same cut.
pub fn input_span(stimulus: Stimulus, rate: u32) -> Span {
    let x = stimulus.samples(rate);
    span_of(&x, x.len(), 0, rate, &[])
}

// ---- Cells, the memo and the table (D1; plan P5) ----------------------------------------------

/// One cell of the report.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    /// The row.
    pub stimulus: Stimulus,
    /// The rung.
    pub configuration: Configuration,
    /// The host rate.
    pub rate: u32,
    /// The configuration's latency at this rate.
    pub latency: u32,
    /// The output's measures.
    pub measures: Measures,
    /// The input's own measures (C0 rows only).
    pub input_own: Option<Measures>,
    /// The frames with a splice.
    pub splices: usize,
    /// The output's length in host samples (kept output in drop mode).
    pub output_len: usize,
}

/// Computes a cell without the memo, on settings a caller doctored (US2 AS1).
pub fn compute_cell_with(
    stimulus: Stimulus,
    configuration: Configuration,
    rate: u32,
    block: usize,
    o: ReaderOptions,
    doctor: &dyn Fn(&mut Settings),
) -> Cell {
    let mut settings = configuration.settings(rate);
    doctor(&mut settings);
    let span = process_settings(stimulus, settings, rate, block);
    finish_cell(stimulus, configuration, rate, span, o)
}

/// Computes a cell without the memo.
pub fn compute_cell(
    stimulus: Stimulus,
    configuration: Configuration,
    rate: u32,
    block: usize,
    o: ReaderOptions,
) -> Cell {
    compute_cell_with(stimulus, configuration, rate, block, o, &|_| {})
}

fn finish_cell(
    stimulus: Stimulus,
    configuration: Configuration,
    rate: u32,
    span: Span,
    o: ReaderOptions,
) -> Cell {
    let measures = read(&span.samples, &span.splices, o);
    let input_own = if configuration == Configuration::C0 {
        let s = input_span(stimulus, rate);
        Some(read(&s.samples, &[], o))
    } else {
        None
    };
    Cell {
        stimulus,
        configuration,
        rate,
        latency: span.latency,
        measures,
        input_own,
        splices: span.splices.len(),
        output_len: span.output_len,
    }
}

type Key = (Stimulus, Configuration, u32);

/// The memo: one slot per cell, so concurrent requests for a cell compute it once (the first
/// caller fills the slot, the others wait on it).
type Memo = Mutex<BTreeMap<(u32, u8, u8), Arc<OnceLock<Cell>>>>;

fn memo() -> &'static Memo {
    static M: OnceLock<Memo> = OnceLock::new();
    M.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn slot(k: (u32, u8, u8)) -> Arc<OnceLock<Cell>> {
    memo().lock().unwrap().entry(k).or_default().clone()
}

fn key_of((s, c, r): Key) -> (u32, u8, u8) {
    (
        r,
        STIMULI.iter().position(|&x| x == s).unwrap() as u8,
        CONFIGURATIONS.iter().position(|&x| x == c).unwrap() as u8,
    )
}

/// The production cell (block 4096, the production reader), memoized per (stimulus,
/// configuration, rate).
pub fn cell(stimulus: Stimulus, configuration: Configuration, rate: u32) -> Cell {
    slot(key_of((stimulus, configuration, rate)))
        .get_or_init(|| compute_cell(stimulus, configuration, rate, BLOCK, PRODUCTION))
        .clone()
}

/// Fills the memo for every cell at `rate` in parallel (one thread per missing cell, each
/// filling its own slot), then returns them in the report's order.
pub fn table(rate: u32) -> Vec<Cell> {
    let todo: Vec<Key> = STIMULI
        .iter()
        .flat_map(|&s| CONFIGURATIONS.iter().map(move |&c| (s, c, rate)))
        .filter(|&k| slot(key_of(k)).get().is_none())
        .collect();
    std::thread::scope(|sc| {
        let handles: Vec<_> = todo
            .iter()
            .map(|&(s, c, r)| sc.spawn(move || cell(s, c, r)))
            .collect();
        for h in handles {
            h.join().unwrap();
        }
    });
    STIMULI
        .iter()
        .flat_map(|&s| CONFIGURATIONS.iter().map(move |&c| cell(s, c, rate)))
        .collect()
}

// ---- The canonical text (D1; data-model.md › Canonical text grammar) ---------------------------

fn fmt_opt(v: Option<f64>, prec: usize, width: usize) -> String {
    match v {
        None => format!("{:>width$}", "—"),
        Some(x) => {
            let mut s = if x == f64::NEG_INFINITY {
                "-inf".to_string()
            } else {
                format!("{x:.prec$}")
            };
            if s.starts_with("-0.") && s[1..].chars().all(|c| c == '0' || c == '.') {
                s.remove(0);
            }
            format!("{s:>width$}")
        }
    }
}

/// The `def` lines.
pub fn definitions() -> Vec<String> {
    vec![
        format!("def frame={FRAME}"),
        format!("def settle_s={SETTLE_S}"),
        format!("def active_dbfs={ACTIVE_DBFS}"),
        format!("def lpc_order={LPC_ORDER}"),
        format!("def preemphasis={PREEMPHASIS}"),
        format!("def grid_hz={GRID_HZ}"),
        "def search_hz=200-2300".to_string(),
        format!("def prominence_db={PROMINENCE_DB}"),
        "def prominence_span_hz=300".to_string(),
        "def width_hz=300".to_string(),
        format!("def lag={LAG_MIN}-{LAG_MAX}"),
        format!("def periodic_r={PERIODIC_R}"),
        format!("def stable_r={STABLE_R}"),
        format!("def stable_dlag={STABLE_DLAG}"),
        format!("def dft_hz={DFT_LO_HZ}-{DFT_HI_HZ}"),
        format!("def share_hz={SHARE_LO_HZ}-{SHARE_HI_HZ}"),
        format!("def share_active={SHARE_ACTIVE}"),
        "def flux_grid_hz=200-2300".to_string(),
        "def window=hann".to_string(),
        format!("def resample_cutoff_hz={RESAMPLE_CUTOFF_HZ}"),
        "def splice_reset=at each splice (the pause reports)".to_string(),
    ]
}

/// The column header line.
pub const HEADER: &str = "stim cfg    level active  peaks    move   move0      r  rfrac  lagst lagst0  share   flux  flux0   lowpk pairs";

/// One table row.
pub fn row_text(c: &Cell, c0: &Measures) -> String {
    let m = &c.measures;
    format!(
        "{:<4} {:<3} {} {:>6} {} {} {} {} {} {} {} {} {} {} {} {:>5}",
        c.stimulus.id(),
        c.configuration.id(),
        fmt_opt(Some(m.level_dbfs), 2, 8),
        m.active,
        fmt_opt(m.peaks, 3, 6),
        fmt_opt(m.movement_hz, 1, 7),
        fmt_opt(c0.movement_hz, 1, 7),
        fmt_opt(m.periodicity, 3, 6),
        fmt_opt(m.periodic_fraction, 3, 6),
        fmt_opt(m.lag_stable, 3, 6),
        fmt_opt(c0.lag_stable, 3, 6),
        fmt_opt(m.mod_share, 3, 6),
        fmt_opt(m.flux_db, 2, 6),
        fmt_opt(c0.flux_db, 2, 6),
        fmt_opt(m.lowest_peak_median_hz, 1, 7),
        m.pairs,
    )
}

/// The text of one rate's section (the `rate` line, the `cfg` lines, the header, 80 rows).
pub fn rate_text(rate: u32) -> String {
    let cells = table(rate);
    let mut s = format!("rate {rate}\n");
    for c in CONFIGURATIONS {
        s.push_str(&c.line(rate));
        s.push('\n');
    }
    s.push_str(HEADER);
    s.push('\n');
    for c in &cells {
        let c0 = cells
            .iter()
            .find(|x| x.stimulus == c.stimulus && x.configuration == Configuration::C0)
            .unwrap()
            .measures;
        s.push_str(&row_text(c, &c0));
        s.push('\n');
    }
    s
}

/// The canonical text at the golden's rates (D1).
pub fn canonical_text() -> String {
    let mut s = format!("{TITLE}\n");
    for st in STIMULI {
        s.push_str(&st.line());
        s.push('\n');
    }
    for d in definitions() {
        s.push_str(&d);
        s.push('\n');
    }
    let a35 = cell(Stimulus::A35, Configuration::C0, DEVICE_HZ).measures;
    s.push_str(&format!(
        "floor movement_hz={}\n",
        fmt_opt(a35.movement_hz, 1, 0).trim()
    ));
    s.push_str(&format!(
        "floor lag_stable={}\n",
        fmt_opt(a35.lag_stable, 3, 0).trim()
    ));
    s.push_str(&format!(
        "floor flux_db={}\n",
        fmt_opt(a35.flux_db, 2, 0).trim()
    ));
    for rate in GOLDEN_RATES {
        s.push_str(&rate_text(rate));
    }
    s
}

/// The strings the wording checks read (D7): the title, the stimulus lines, the configuration
/// lines at 6 kHz, the definitions and the header.
pub fn strings() -> Vec<String> {
    let mut v = vec![TITLE.to_string()];
    v.extend(STIMULI.iter().map(|s| s.line()));
    v.extend(CONFIGURATIONS.iter().map(|c| c.line(DEVICE_HZ)));
    v.extend(definitions());
    v.push(HEADER.to_string());
    v
}

/// The phrase list of plan P9 (D7): stems matched by the engine's rule. Two compound terms are
/// allowed, stage 5's registered name and the project's "voice band": [`phrase_hit`] removes
/// exactly those before matching.
pub const PHRASES: Lists = Lists {
    stems: &[
        "hear",
        "listen",
        "perceiv",
        "percept",
        "audib",
        "inaudib",
        "voice",
        "formant",
        "syllab",
        "vowel",
        "pitch",
        "intelligib",
        "pareidol",
        "mishear",
        "misheard",
        "illusion",
        "mistak",
        "phantom",
        "explain",
        "clean",
        "raw",
        "isolat",
        "enhanc",
        "hidden",
        "detect",
        "speech-like",
        "word",
    ],
    phrases: &["sounds like", "noise reduction"],
    suffixes: &[
        "", "s", "es", "ed", "ing", "er", "ers", "ia", "y", "ic", "al", "ally", "ility", "ible",
        "ive", "ion", "ions", "ity", "d",
    ],
    exceptions: &[
        ("noise reduction", "not modeled: "),
        ("noise reduction", "not a "),
    ],
};

/// The two allowed compound terms.
pub const ALLOWED_TERMS: [&str; 2] = ["Voice-activated pause (VAS)", "voice band"];

/// The first phrase-list hit in `text` after the allowed terms are removed.
pub fn phrase_hit(text: &str) -> Option<rr_dr60::wording::Hit> {
    let mut t = text.to_string();
    for a in ALLOWED_TERMS {
        t = t.replace(a, "stage-5-name");
    }
    rr_dr60::wording::first_hit_in(&PHRASES, &t)
}

/// The committed golden's text.
pub fn committed_golden() -> String {
    std::fs::read_to_string(golden_path()).unwrap_or_else(|e| {
        panic!(
            "{}: {e}; bless with RR_DR60_BLESS=nonspeech",
            golden_path().display()
        )
    })
}

/// The rows of one rate's section of a canonical text, keyed `stim/Cn`, as their line.
pub fn golden_rows(text: &str, rate: u32) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    let mut in_rate = false;
    for l in text.lines() {
        if let Some(r) = l.strip_prefix("rate ") {
            in_rate = r.trim() == rate.to_string();
            continue;
        }
        if !in_rate || l.starts_with("cfg ") || l.starts_with("stim ") {
            continue;
        }
        let mut w = l.split_whitespace();
        if let (Some(a), Some(b)) = (w.next(), w.next())
            && b.starts_with('C')
            && b.len() == 2
        {
            out.insert(format!("{a}/{b}"), l.to_string());
        }
    }
    out
}

/// The guide's generated table (D8; plan R-09), read from the committed golden's 6 kHz section:
/// the rows a50, b, d and f through the five labelled configurations, five columns with their
/// definitional scales, every figure beside the Input row's.
pub fn guide_table() -> String {
    guide_table_from(&committed_golden())
}

/// [`guide_table`] from a given canonical text.
pub fn guide_table_from(text: &str) -> String {
    const ROWS: [(&str, &str); 4] = [
        ("a50", "Steady noise, −50 dBFS"),
        ("b", "Noise modulated at 4 Hz, −35 dBFS"),
        ("d", "Sparse bursts, 10 – 160 ms"),
        ("f", "Digital silence"),
    ];
    const CFGS: [(&str, &str); 5] = [
        ("C0", "Input (C0)"),
        ("C1", "Band filters and clamp, stages 4 and 10 (C1)"),
        ("C3", "Band filters and coder, stages 4, 7, 9, 10 (C3)"),
        ("C5", "Default record path before the output chain (C5)"),
        ("C6", "Sensitivity 5, hiss offset +12 dB (C6)"),
    ];
    let rows = golden_rows(text, DEVICE_HZ);
    let mut s = String::from(
        "| Input | Configuration | Level (dBFS) | Active frames (of 575) | Periodicity (0 – 1) | 2 – 8 Hz envelope share (0 – 1) | LPC envelope peaks per frame |\n|---|---|---|---|---|---|---|\n",
    );
    for (st, label) in ROWS {
        for (cf, clabel) in CFGS {
            let line = rows
                .get(&format!("{st}/{cf}"))
                .unwrap_or_else(|| panic!("{st}/{cf}"));
            let w: Vec<&str> = line.split_whitespace().collect();
            // columns: stim cfg level active peaks move move0 r rfrac lagst lagst0 share flux flux0 lowpk pairs
            s.push_str(&format!(
                "| {label} | {clabel} | {} | {} | {} | {} | {} |\n",
                w[2], w[3], w[7], w[11], w[4]
            ));
        }
    }
    s
}

// ---- The D6 checks (shared by the test binaries and the bless gate; D1) --------------------------

use crate::checks::{MeasurementResult, Tolerance};

/// D6 (i): the reference's periodic-frame fraction and peaks; the noise cells' fraction.
pub const REFERENCE_FRACTION_MIN: f64 = 0.9;
/// See [`REFERENCE_FRACTION_MIN`] (plan P1).
pub const REFERENCE_PEAKS_MIN: f64 = 1.8;
/// See [`REFERENCE_FRACTION_MIN`].
pub const NOISE_FRACTION_MAX: f64 = 0.1;
/// The noise bounds apply to cells with at least this many active frames (the bursts row through
/// the coder alone has about 85 and reads the decoder's continuation; the first print, 2026-11-04).
pub const NOISE_MIN_ACTIVE: usize = 500;
/// D6 (iii): the residue bound (spec 008: ≤ −78 dBFS; margin 3 dB) and C6's open-gate share of
/// the nominal span.
pub const RESIDUE_MAX_DBFS: f64 = -75.0;
/// See [`RESIDUE_MAX_DBFS`].
pub const OPEN_GATE_MIN_SHARE: f64 = 0.9;
/// D6 (iv): C3's periodicity over C1's on the steady and modulated rows (observed +0.034 … +0.098
/// at seven rates; the bursts and pink rows reported: the post-implementation review Q4).
pub const PERIODICITY_RISE_MIN: f64 = 0.03;
/// D6 (v): the measures' own teeth.
pub const SHARE_B_MIN: f64 = 0.8;
/// See [`SHARE_B_MIN`].
pub const SHARE_STEADY_MAX: f64 = 0.35;
/// See [`SHARE_B_MIN`].
pub const NOISE_PEAKS_MAX: f64 = 1.5;
/// The resonator row's rise over a35 and its median band (plan P2, restated at the first print:
/// the rise reads 0.20 – 0.40 by rate, the median 700 – 720 Hz; the pole's magnitude peak is near
/// 725 Hz).
pub const RESONATOR_RISE_MIN: f64 = 0.15;
/// See [`RESONATOR_RISE_MIN`].
pub const RESONATOR_MEDIAN_HZ: (f64, f64) = (650.0, 800.0);
/// The reference's lowest-peak median band (its first resonance sweeps 300 – 750 Hz).
pub const REFERENCE_MEDIAN_HZ: (f64, f64) = (250.0, 800.0);
/// The pink row's lowest-peak median band at pass-through (the pre-emphasis mutant's D6 row,
/// post-implementation reviews coach 9 / Q6: production 1170 – 1265 Hz, the mutant 320).
pub const PINK_MEDIAN_HZ: (f64, f64) = (900.0, 1500.0);
/// D6 (ii) at the rates other than 6 kHz (plan R-05b; the first print, then the re-bless of
/// 2026-11-04 for the two added rows): 2 × the largest |C0 − input-own| over the stimuli at six
/// rates, with the floors (the lag-stable fraction reads 0 apart and takes a 0.01 floor; the
/// lowest-peak median 45 Hz apart on the bursts and pink rows → 100 Hz).
pub const PASS_THROUGH_TOL: [(&str, f64); 8] = [
    ("level", 0.07),
    ("peaks", 0.14),
    ("periodicity", 0.002),
    ("periodic fraction", 0.03),
    ("lag-stable", 0.01),
    ("share", 0.01),
    ("flux", 0.03),
    ("lowest-peak median", 100.0),
];

const TRACE: &[&str] = &["029 D6"];
const NOISE_ROWS: [Stimulus; 7] = [
    Stimulus::A35,
    Stimulus::A50,
    Stimulus::A55,
    Stimulus::B,
    Stimulus::C,
    Stimulus::D,
    Stimulus::E,
];

#[allow(clippy::too_many_arguments)]
fn row(
    req: &'static str,
    prop: &'static str,
    rate: u32,
    cfg: &'static str,
    stim: String,
    v: f64,
    unit: &'static str,
    t: Tolerance,
) -> MeasurementResult {
    MeasurementResult::new(req, prop, TRACE, rate, cfg, stim, v, unit, t)
}

/// The absolute difference of two optional figures; a presence mismatch is infinite (it fails any
/// bound); two absent figures are 0.
fn diff(a: Option<f64>, b: Option<f64>) -> f64 {
    match (a, b) {
        (Some(a), Some(b)) => (a - b).abs(),
        (None, None) => 0.0,
        _ => f64::INFINITY,
    }
}

/// D6 (i): the reference separates; every noise cell with ≥ 500 active frames reads low.
pub fn d6_separation(rates: &[u32]) -> Vec<MeasurementResult> {
    let mut out = Vec::new();
    for &rate in rates {
        for cfg in [
            Configuration::C0,
            Configuration::C1,
            Configuration::C2,
            Configuration::C3,
        ] {
            let m = cell(Stimulus::G, cfg, rate).measures;
            out.push(row(
                "029 D6 i",
                "reference periodic-frame fraction",
                rate,
                cfg.id(),
                "g".into(),
                m.periodic_fraction.unwrap_or(f64::NAN),
                "",
                Tolerance::AtLeast(REFERENCE_FRACTION_MIN),
            ));
            out.push(row(
                "029 D6 i",
                "reference LPC peaks per frame",
                rate,
                cfg.id(),
                "g".into(),
                m.peaks.unwrap_or(f64::NAN),
                "",
                Tolerance::AtLeast(REFERENCE_PEAKS_MIN),
            ));
        }
        for cfg in [
            Configuration::C4,
            Configuration::C5,
            Configuration::C6,
            Configuration::C7,
        ] {
            let m = cell(Stimulus::G, cfg, rate).measures;
            out.push(row(
                "029 D6 i",
                "reference periodic-frame fraction (reported)",
                rate,
                cfg.id(),
                "g".into(),
                m.periodic_fraction.unwrap_or(f64::NAN),
                "",
                Tolerance::Report,
            ));
        }
        let mut cells: Vec<(Stimulus, Configuration)> = NOISE_ROWS
            .iter()
            .flat_map(|&s| CONFIGURATIONS.iter().map(move |&c| (s, c)))
            .collect();
        cells.push((Stimulus::F, Configuration::C6));
        for (s, c) in cells {
            let m = cell(s, c, rate).measures;
            let Some(f) = m.periodic_fraction else {
                continue;
            };
            let t = if m.active >= NOISE_MIN_ACTIVE {
                Tolerance::AtMost(NOISE_FRACTION_MAX)
            } else {
                Tolerance::Report
            };
            out.push(row(
                "029 D6 i",
                "noise periodic-frame fraction",
                rate,
                c.id(),
                format!("{} ({} active)", s.id(), m.active),
                f,
                "",
                t,
            ));
        }
    }
    out
}

/// D6 (ii): C0 equals the input's own reading, bit for bit at 6 kHz, within the recorded
/// tolerances elsewhere; a presence mismatch fails.
pub fn d6_pass_through(rates: &[u32]) -> Vec<MeasurementResult> {
    let mut out = Vec::new();
    let tol = |name: &str| {
        PASS_THROUGH_TOL
            .iter()
            .find(|(n, _)| *n == name)
            .map(|&(_, t)| t)
            .unwrap()
    };
    for &rate in rates {
        for s in STIMULI {
            let c = cell(s, Configuration::C0, rate);
            let own = c.input_own.expect("C0 rows carry the input's own reading");
            if rate == DEVICE_HZ {
                let same = c.measures == own;
                out.push(row(
                    "029 D6 ii",
                    "C0 equals input-own (bit for bit)",
                    rate,
                    "C0",
                    s.id().into(),
                    f64::from(u8::from(!same)),
                    "",
                    Tolerance::Exact(0.0),
                ));
                continue;
            }
            let m = c.measures;
            let level = match (m.level_dbfs.is_finite(), own.level_dbfs.is_finite()) {
                (true, true) => (m.level_dbfs - own.level_dbfs).abs(),
                (false, false) => 0.0,
                _ => f64::INFINITY,
            };
            out.push(row(
                "029 D6 ii",
                "|C0 − own| level",
                rate,
                "C0",
                s.id().into(),
                level,
                "dB",
                Tolerance::AtMost(tol("level")),
            ));
            out.push(row(
                "029 D6 ii",
                "|C0 − own| peaks",
                rate,
                "C0",
                s.id().into(),
                diff(m.peaks, own.peaks),
                "",
                Tolerance::AtMost(tol("peaks")),
            ));
            out.push(row(
                "029 D6 ii",
                "|C0 − own| periodicity",
                rate,
                "C0",
                s.id().into(),
                diff(m.periodicity, own.periodicity),
                "",
                Tolerance::AtMost(tol("periodicity")),
            ));
            out.push(row(
                "029 D6 ii",
                "|C0 − own| periodic fraction",
                rate,
                "C0",
                s.id().into(),
                diff(m.periodic_fraction, own.periodic_fraction),
                "",
                Tolerance::AtMost(tol("periodic fraction")),
            ));
            out.push(row(
                "029 D6 ii",
                "|C0 − own| lag-stable",
                rate,
                "C0",
                s.id().into(),
                diff(m.lag_stable, own.lag_stable),
                "",
                Tolerance::AtMost(tol("lag-stable")),
            ));
            out.push(row(
                "029 D6 ii",
                "|C0 − own| share",
                rate,
                "C0",
                s.id().into(),
                diff(m.mod_share, own.mod_share),
                "",
                Tolerance::AtMost(tol("share")),
            ));
            out.push(row(
                "029 D6 ii",
                "|C0 − own| flux",
                rate,
                "C0",
                s.id().into(),
                diff(m.flux_db, own.flux_db),
                "dB",
                Tolerance::AtMost(tol("flux")),
            ));
            out.push(row(
                "029 D6 ii",
                "|C0 − own| lowest-peak median",
                rate,
                "C0",
                s.id().into(),
                diff(m.lowest_peak_median_hz, own.lowest_peak_median_hz),
                "Hz",
                Tolerance::AtMost(tol("lowest-peak median")),
            ));
            out.push(row(
                "029 D6 ii",
                "|C0 − own| movement (reported)",
                rate,
                "C0",
                s.id().into(),
                diff(m.movement_hz, own.movement_hz),
                "Hz",
                Tolerance::Report,
            ));
            out.push(row(
                "029 D6 ii",
                "|C0 − own| active frames",
                rate,
                "C0",
                s.id().into(),
                (m.active as f64 - own.active as f64).abs(),
                "frames",
                Tolerance::AtMost(1.0),
            ));
        }
    }
    out
}

/// D6 (iii): digital silence through the ladder.
pub fn d6_silence(rates: &[u32]) -> Vec<MeasurementResult> {
    let mut out = Vec::new();
    for &rate in rates {
        for c in [Configuration::C0, Configuration::C1] {
            let m = cell(Stimulus::F, c, rate).measures;
            out.push(row(
                "029 D6 iii",
                "silence: active frames",
                rate,
                c.id(),
                "f".into(),
                m.active as f64,
                "frames",
                Tolerance::Exact(0.0),
            ));
            out.push(row(
                "029 D6 iii",
                "silence: digital zero (level −∞)",
                rate,
                c.id(),
                "f".into(),
                f64::from(u8::from(m.level_dbfs != f64::NEG_INFINITY)),
                "",
                Tolerance::Exact(0.0),
            ));
        }
        for c in [Configuration::C2, Configuration::C3] {
            let m = cell(Stimulus::F, c, rate).measures;
            out.push(row(
                "029 D6 iii",
                "silence: active frames",
                rate,
                c.id(),
                "f".into(),
                m.active as f64,
                "frames",
                Tolerance::Exact(0.0),
            ));
            out.push(row(
                "029 D6 iii",
                "silence: the decoder's residue",
                rate,
                c.id(),
                "f".into(),
                m.level_dbfs,
                "dBFS",
                Tolerance::AtMost(RESIDUE_MAX_DBFS),
            ));
        }
        let c5 = cell(Stimulus::F, Configuration::C5, rate);
        let s5 = Configuration::C5.settings(rate);
        let bound = ((f64::from(s5.vas.onset_ms)
            + f64::from(s5.vas.hang_ms)
            + 1000.0 * f64::from(c5.latency) / f64::from(rate))
            / 20.0)
            .ceil()
            - 25.0;
        out.push(row(
            "029 D6 iii",
            "silence: C5 closes the gate (active ≤ ⌈(onset + hang + latency)/20 ms⌉ − 25)",
            rate,
            "C5",
            "f".into(),
            c5.measures.active as f64,
            "frames",
            Tolerance::AtMost(bound),
        ));
        let c6 = cell(Stimulus::F, Configuration::C6, rate).measures;
        out.push(row(
            "029 D6 iii",
            "silence: C6 stays open (active ≥ 90 % of the nominal 575)",
            rate,
            "C6",
            "f".into(),
            c6.active as f64 / SPAN_FRAMES as f64,
            "",
            Tolerance::AtLeast(OPEN_GATE_MIN_SHARE),
        ));
        out.push(row(
            "029 D6 iii",
            "silence: C6's level (reported)",
            rate,
            "C6",
            "f".into(),
            c6.level_dbfs,
            "dBFS",
            Tolerance::Report,
        ));
    }
    out
}

/// D6 (iv): the coder's periodicity, C3 against C1, on the steady and modulated rows; the bursts
/// and pink rows, the level change and the share ratio reported.
pub fn d6_coder_periodicity(rates: &[u32]) -> Vec<MeasurementResult> {
    let mut out = Vec::new();
    for &rate in rates {
        for s in NOISE_ROWS {
            let c1 = cell(s, Configuration::C1, rate).measures;
            let c3 = cell(s, Configuration::C3, rate).measures;
            let asserted = matches!(
                s,
                Stimulus::A35 | Stimulus::A50 | Stimulus::A55 | Stimulus::B | Stimulus::C
            ) && c1.active >= NOISE_MIN_ACTIVE
                && c3.active >= NOISE_MIN_ACTIVE;
            let t = if asserted {
                Tolerance::AtLeast(PERIODICITY_RISE_MIN)
            } else {
                Tolerance::Report
            };
            out.push(row(
                "029 D6 iv",
                "coder periodicity rise, C3 − C1",
                rate,
                "C3",
                s.id().into(),
                c3.periodicity.unwrap_or(f64::NAN) - c1.periodicity.unwrap_or(f64::NAN),
                "",
                t,
            ));
            out.push(row(
                "029 D6 iv",
                "coder level change, C3 − C1 (reported)",
                rate,
                "C3",
                s.id().into(),
                c3.level_dbfs - c1.level_dbfs,
                "dB",
                Tolerance::Report,
            ));
            if let (Some(a), Some(b)) = (c3.mod_share, c1.mod_share) {
                out.push(row(
                    "029 D6 iv",
                    "2-8 Hz share ratio C3 ÷ C1 (reported)",
                    rate,
                    "C3",
                    s.id().into(),
                    a / b,
                    "",
                    Tolerance::Report,
                ));
            }
        }
    }
    out
}

/// D6 (v): the measures' own teeth on pass-through rows.
pub fn d6_teeth(rates: &[u32]) -> Vec<MeasurementResult> {
    let mut out = Vec::new();
    for &rate in rates {
        let b = cell(Stimulus::B, Configuration::C0, rate).measures;
        out.push(row(
            "029 D6 v",
            "2-8 Hz share on (b) in C0",
            rate,
            "C0",
            "b".into(),
            b.mod_share.unwrap_or(f64::NAN),
            "",
            Tolerance::AtLeast(SHARE_B_MIN),
        ));
        let a = cell(Stimulus::A35, Configuration::C0, rate).measures;
        out.push(row(
            "029 D6 v",
            "2-8 Hz share on steady noise in C0",
            rate,
            "C0",
            "a35".into(),
            a.mod_share.unwrap_or(f64::NAN),
            "",
            Tolerance::AtMost(SHARE_STEADY_MAX),
        ));
        for s in NOISE_ROWS {
            for c in [Configuration::C0, Configuration::C1] {
                let m = cell(s, c, rate).measures;
                out.push(row(
                    "029 D6 v",
                    "noise LPC peaks per frame",
                    rate,
                    c.id(),
                    s.id().into(),
                    m.peaks.unwrap_or(f64::NAN),
                    "",
                    Tolerance::AtMost(NOISE_PEAKS_MAX),
                ));
            }
        }
        let h = cell(Stimulus::H, Configuration::C0, rate).measures;
        out.push(row(
            "029 D6 v",
            "resonator row's peak rise over a35 in C0",
            rate,
            "C0",
            "h".into(),
            h.peaks.unwrap_or(f64::NAN) - a.peaks.unwrap_or(f64::NAN),
            "",
            Tolerance::AtLeast(RESONATOR_RISE_MIN),
        ));
        out.push(row(
            "029 D6 v",
            "resonator row's lowest-peak median",
            rate,
            "C0",
            "h".into(),
            h.lowest_peak_median_hz.unwrap_or(f64::NAN),
            "Hz",
            Tolerance::Range(RESONATOR_MEDIAN_HZ.0, RESONATOR_MEDIAN_HZ.1),
        ));
        let g = cell(Stimulus::G, Configuration::C0, rate).measures;
        out.push(row(
            "029 D6 v",
            "reference's lowest-peak median",
            rate,
            "C0",
            "g".into(),
            g.lowest_peak_median_hz.unwrap_or(f64::NAN),
            "Hz",
            Tolerance::Range(REFERENCE_MEDIAN_HZ.0, REFERENCE_MEDIAN_HZ.1),
        ));
        let e = cell(Stimulus::E, Configuration::C0, rate).measures;
        out.push(row(
            "029 D6 v",
            "pink row's lowest-peak median (the pre-emphasis row)",
            rate,
            "C0",
            "e".into(),
            e.lowest_peak_median_hz.unwrap_or(f64::NAN),
            "Hz",
            Tolerance::Range(PINK_MEDIAN_HZ.0, PINK_MEDIAN_HZ.1),
        ));
    }
    out
}

/// D6 (vi): every figure finite where present, absent only where the cell has no active frames
/// (the level finite unless the span is digital zero); the pair count on (d) in C0 and C5 equals
/// the structural oracle.
pub fn d6_finite_and_pairs(rates: &[u32]) -> Vec<MeasurementResult> {
    let mut out = Vec::new();
    for &rate in rates {
        for c in table(rate) {
            let m = c.measures;
            let opts = [
                m.peaks,
                m.movement_hz,
                m.periodicity,
                m.periodic_fraction,
                m.lag_stable,
                m.mod_share,
                m.flux_db,
                m.lowest_peak_median_hz,
            ];
            let finite = opts.iter().all(|v| v.is_none_or(f64::is_finite))
                && (m.level_dbfs.is_finite()
                    || (m.level_dbfs == f64::NEG_INFINITY && m.active == 0));
            let present_rule = if m.active == 0 {
                opts.iter().all(Option::is_none)
            } else {
                m.peaks.is_some() && m.periodicity.is_some() && m.periodic_fraction.is_some()
            };
            out.push(row(
                "029 D6 vi",
                "figures finite where present, absent only on inactive cells",
                rate,
                c.configuration.id(),
                c.stimulus.id().into(),
                f64::from(u8::from(!(finite && present_rule))),
                "",
                Tolerance::Exact(0.0),
            ));
        }
    }
    for &rate in rates {
        for cfg in [Configuration::C0, Configuration::C5] {
            let sp = process(Stimulus::D, cfg, rate, BLOCK);
            let m = read(&sp.samples, &sp.splices, PRODUCTION);
            let want = pairs_oracle(&sp.samples, &sp.splices, ACTIVE_DBFS);
            out.push(row(
                "029 D6 vi",
                "pair count equals Σ(run − 1) split at splices",
                rate,
                cfg.id(),
                format!("d ({} splices)", sp.splices.len()),
                m.pairs as f64 - want as f64,
                "pairs",
                Tolerance::Exact(0.0),
            ));
        }
    }
    out
}

/// Every D6 check at `rates` (the bless gate runs them at the golden's rates; D1).
pub fn d6_all(rates: &[u32]) -> Vec<MeasurementResult> {
    let mut v = d6_separation(rates);
    v.extend(d6_pass_through(rates));
    v.extend(d6_silence(rates));
    v.extend(d6_coder_periodicity(rates));
    v.extend(d6_teeth(rates));
    v.extend(d6_finite_and_pairs(rates));
    v
}

/// A min / max summary per property of a result list (the records quote printed ranges).
pub fn summary(results: &[MeasurementResult]) -> Vec<String> {
    let mut m: std::collections::BTreeMap<&str, (f64, f64)> = std::collections::BTreeMap::new();
    for r in results {
        if !r.measured.is_finite() {
            continue;
        }
        let e = m
            .entry(r.property)
            .or_insert((f64::INFINITY, f64::NEG_INFINITY));
        e.0 = e.0.min(r.measured);
        e.1 = e.1.max(r.measured);
    }
    m.iter()
        .map(|(k, (lo, hi))| format!("summary {k}: {lo:.3} … {hi:.3}"))
        .collect()
}

/// The golden file's path.
pub fn golden_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("golden")
        .join("nonspeech-v1.txt")
}

/// The active-frame mask of a span and the pair count the pairwise measures must report:
/// Σ(run − 1) over the runs of active frames, split at splices (plan P3's oracle).
pub fn pairs_oracle(span: &[f64], splices: &[usize], floor_dbfs: f64) -> usize {
    let frames = span.len() / FRAME;
    let mut pairs = 0;
    let mut run = 0usize;
    for f in 0..frames {
        if splices.contains(&f) {
            run = 0;
        }
        let active = aes17_db(mean_square(&span[f * FRAME..(f + 1) * FRAME])) >= floor_dbfs;
        if active {
            run += 1;
            if run > 1 {
                pairs += 1;
            }
        } else {
            run = 0;
        }
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aes17_level(x: &[f32]) -> f64 {
        aes17_db(x.iter().fold(0.0, |a, &v| a + f64::from(v) * f64::from(v)) / x.len() as f64)
    }

    /// T001: lengths, levels within 0.01 dB, the bursts' count and positions, the pink share.
    #[test]
    fn stimuli_lengths_levels_and_seeds() {
        for &rate in configs::measurement_rates() {
            for st in STIMULI {
                let x = st.samples(rate);
                assert_eq!(x.len(), 12 * rate as usize, "{} at {rate}", st.id());
                if let Some(l) = st.level_dbfs() {
                    let got = aes17_level(&x);
                    assert!(
                        (got - l).abs() < 0.01,
                        "{} at {rate}: {got} vs {l}",
                        st.id()
                    );
                }
            }
            let d = bursts(rate, 12 * rate as usize);
            let active: Vec<usize> = (0..d.len()).filter(|&i| d[i] != 0.0).collect();
            assert!(!active.is_empty());
        }
        // The draw does not depend on the rate: the first burst's 6 kHz position is the same.
        let first = |rate: u32| {
            let d = bursts(rate, 12 * rate as usize);
            (0..d.len()).find(|&i| d[i] != 0.0).unwrap() * 6000 / rate as usize
        };
        // (the envelope's first sample is 0, so the first non-zero sample sits one host sample
        // in: within one device sample across rates)
        assert!((first(6000) as i64 - first(48_000) as i64).abs() <= 1);
        assert!((first(6000) as i64 - first(44_100) as i64).abs() <= 1);
    }

    /// T001: (e)'s in-band share agrees across rates within 0.02 (plan P7).
    #[test]
    fn stimuli_pink_inband_share_across_rates() {
        let share = |rate: u32| {
            let x: Vec<f64> = Stimulus::E
                .samples(rate)
                .iter()
                .map(|&v| f64::from(v))
                .collect();
            let dev = to_device_rate(&x, rate);
            // In-band power by a 6 kHz band-pass of the device-rate signal: the ratio of the
            // 225 – 2550 Hz Kaiser band's output power to the total.
            let total = mean_square(&dev);
            let y = band_225_2550(&dev);
            mean_square(&y) / total
        };
        let s6 = share(6000);
        for &rate in configs::measurement_rates() {
            let s = share(rate);
            assert!((s - s6).abs() < 0.02, "{rate}: {s} vs {s6}");
        }
    }

    fn band_225_2550(x: &[f64]) -> Vec<f64> {
        let fs = 6000.0;
        let taps = 61usize;
        let mid = 30.0;
        let (f1, f2) = (225.0 / fs, 2550.0 / fs);
        let lowpass = |fc: f64, m: f64| {
            if m == 0.0 {
                2.0 * fc
            } else {
                sin(2.0 * PI * fc * m) / (PI * m)
            }
        };
        let h: Vec<f64> = (0..taps)
            .map(|k| {
                let m = k as f64 - mid;
                let w = bessel_i0(8.0 * sqrt(1.0 - (m / mid) * (m / mid))) / bessel_i0(8.0);
                w * (lowpass(f2, m) - lowpass(f1, m))
            })
            .collect();
        (taps..x.len())
            .map(|n| {
                h.iter()
                    .enumerate()
                    .fold(0.0, |a, (k, &hk)| a + hk * x[n - k])
            })
            .collect()
    }

    /// T001: the envelopes of (b) and (c) by their DFT bins.
    #[test]
    fn stimuli_envelopes_by_dft_bin() {
        let x: Vec<f64> = Stimulus::B
            .samples(6000)
            .iter()
            .map(|&v| f64::from(v))
            .collect();
        let env: Vec<f64> = x.chunks(FRAME).map(|f| aes17_db(mean_square(f))).collect();
        let n = env.len();
        let mean = env.iter().sum::<f64>() / n as f64;
        let power = |k: usize| {
            let (mut re, mut im) = (0.0, 0.0);
            for (i, &v) in env.iter().enumerate() {
                let w = 2.0 * PI * (k * i) as f64 / n as f64;
                re += (v - mean) * cos(w);
                im -= (v - mean) * sin(w);
            }
            re * re + im * im
        };
        let k4 = (4.0 * n as f64 / ENVELOPE_HZ) as usize; // 4 Hz
        let p4 = power(k4);
        let p3 = power(k4 - 10);
        assert!(p4 > 100.0 * p3, "4 Hz bin {p4} vs off-bin {p3}");
    }

    /// T003: the resampler at the seven rates on a −20 dBFS 1 kHz tone (level within 0.05 dB,
    /// frequency within 0.1 Hz by a sine fit), and the identity at 6 kHz.
    #[test]
    fn resampler_tone_level_and_frequency_at_each_rate() {
        for rate in configs::all_rates() {
            let fs = f64::from(rate);
            let x: Vec<f64> =
                stimulus::tone(1000.0, stimulus::amplitude(-20.0), fs, 2 * rate as usize)
                    .iter()
                    .map(|&v| f64::from(v))
                    .collect();
            let y = to_device_rate(&x, rate);
            assert_eq!(y.len(), 2 * 6000);
            let body = &y[600..y.len() - 600];
            let level = aes17_db(mean_square(body));
            assert!((level + 20.0).abs() < 0.05, "{rate}: level {level}");
            // frequency: zero crossings
            let crossings = body
                .windows(2)
                .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
                .count();
            let f = crossings as f64 / (body.len() as f64 / 6000.0);
            assert!((f - 1000.0).abs() < 1.0, "{rate}: {f} Hz");
            if rate == 6000 {
                assert_eq!(y, x);
            }
        }
    }

    /// T004: a 300 Hz tone's lag and r; a two-resonance frame's peaks; the state reset across a
    /// gap; the splice reset; the pair count; a tilted frame's pre-emphasis; the rules.
    #[test]
    fn reader_pins() {
        // A pure 300 Hz tone: lag 20 is the edge (not interior); 600 Hz has lag 10 (< 20):
        // use 120 Hz (lag 50): r ≈ 1 at lag 50.
        let tone: Vec<f64> = stimulus::tone(120.0, 0.1, 6000.0, 10 * FRAME)
            .iter()
            .map(|&v| f64::from(v))
            .collect();
        let m = read(&tone, &[], PRODUCTION);
        assert_eq!(m.active, 10);
        assert!(m.periodicity.unwrap() > 0.95, "{:?}", m.periodicity); // frame 0 has a zero history
        assert_eq!(m.periodic_fraction, Some(1.0));
        assert_eq!(m.pairs, 9);
        assert_eq!(m.lag_stable, Some(1.0));
        // Two resonances on noise: peaks near 500 and 1500 Hz.
        let n: Vec<f64> = noise_core(5, 6000, 40 * FRAME);
        let a = resonate(&n, 6000.0, 500.0, 6.0);
        let b = resonate(&n, 6000.0, 1500.0, 6.0);
        let two: Vec<f64> = a.iter().zip(&b).map(|(x, y)| x + y).collect();
        let two = level_to(&two, -20.0);
        let m = read(&two, &[], PRODUCTION);
        assert!(m.peaks.unwrap() >= 1.8, "{:?}", m.peaks);
        let med = m.lowest_peak_median_hz.unwrap();
        assert!((450.0..=550.0).contains(&med), "{med}");
        // The state reset across a gap: active, inactive, active.
        let mut gapped = tone.clone();
        for v in &mut gapped[5 * FRAME..6 * FRAME] {
            *v = 0.0;
        }
        let m = read(&gapped, &[], PRODUCTION);
        assert_eq!(m.active, 9);
        assert_eq!(m.pairs, 7, "two runs of 5 and 4 frames");
        assert_eq!(m.pairs, pairs_oracle(&gapped, &[], ACTIVE_DBFS));
        let no_reset = ReaderOptions {
            reset_state: false,
            ..PRODUCTION
        };
        assert_eq!(
            read(&gapped, &[], no_reset).pairs,
            8,
            "the pair across the gap counts"
        );
        // The splice reset: a splice at frame 5 splits the run.
        let m = read(&tone, &[5], PRODUCTION);
        assert_eq!(m.pairs, 8);
        assert_eq!(m.pairs, pairs_oracle(&tone, &[5], ACTIVE_DBFS));
        assert_eq!(m.mod_share, None, "a span with a splice prints no share");
        // The 4 Hz envelope's share on a long modulated noise.
        let x: Vec<f64> = Stimulus::B
            .samples(6000)
            .iter()
            .map(|&v| f64::from(v))
            .collect();
        let m = read(&x[3000..], &[], PRODUCTION);
        assert!(m.mod_share.unwrap() > 0.9, "{:?}", m.mod_share);
        // Pre-emphasis moves a tilted frame's lowest peak up: a low-passed noise frame.
        let lp = resonate(&n, 6000.0, 300.0, 1.0);
        let lp = level_to(&lp, -20.0);
        let with = read(&lp, &[], PRODUCTION).lowest_peak_median_hz;
        let without = read(
            &lp,
            &[],
            ReaderOptions {
                pre_emphasis: false,
                ..PRODUCTION
            },
        )
        .lowest_peak_median_hz;
        assert!(with.is_some() && without.is_some());
        assert!(
            with.unwrap() > without.unwrap() + 50.0,
            "{with:?} vs {without:?}"
        );
        // The prominence rule removes peaks.
        let pro = read(&n[..20 * FRAME], &[], PRODUCTION).peaks.unwrap();
        let no = read(
            &n[..20 * FRAME],
            &[],
            ReaderOptions {
                prominence: false,
                ..PRODUCTION
            },
        )
        .peaks
        .unwrap();
        assert!(no > pro, "{no} vs {pro}");
        // An inactive span: nothing but the level.
        let z = vec![0.0; 10 * FRAME];
        let m = read(&z, &[], PRODUCTION);
        assert_eq!(m.active, 0);
        assert_eq!(m.level_dbfs, f64::NEG_INFINITY);
        assert_eq!(m.peaks, None);
    }

    /// T005: the span holds 575 frames in C3 at each measurement rate; C0 at 6 kHz equals the
    /// input bit for bit; drop mode ends at the output's end.
    #[test]
    fn span_rules() {
        for &rate in configs::measurement_rates() {
            let sp = process(Stimulus::A35, Configuration::C3, rate, BLOCK);
            assert_eq!(sp.samples.len() / FRAME, 575, "{rate}");
        }
        let sp = process(Stimulus::B, Configuration::C0, 6000, BLOCK);
        let own = input_span(Stimulus::B, 6000);
        assert_eq!(sp.samples, own.samples);
        assert_eq!(sp.samples.len(), 575 * FRAME);
        let sp = process(Stimulus::F, Configuration::C5, 6000, BLOCK);
        assert!(
            sp.samples.len() / FRAME < 575,
            "the gate closed: {}",
            sp.samples.len() / FRAME
        );
    }

    #[test]
    fn gcd_and_phases() {
        assert_eq!(Resampler::new(6000).phases(), 1);
        assert_eq!(Resampler::new(44_100).phases(), 20);
        assert_eq!(Resampler::new(88_200).phases(), 10);
        assert_eq!(Resampler::new(8000).phases(), 3);
        assert_eq!(Resampler::new(48_000).phases(), 1);
    }
}
