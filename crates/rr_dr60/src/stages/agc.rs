//! AGC stage: signal-chain stage 3 (spec 002; A-017 – A-020).
//!
//! Modeled on an *assumed* record-path AGC (S-004 claims auto-gain; A-007). All defaults are
//! low-confidence assumptions until captures from a real unit exist. The design follows
//! specs/002-agc/research.md R-02 – R-07 and data-model.md › Per-sample processing:
//!
//! 1. **Detector** (A-019): the squared envelope is the larger of the analytic envelope
//!    x[n−31]² + h[n]² (h from a 63-tap Hilbert FIR) and a peak hold over the last 32 samples.
//! 2. **Gain computer** (A-017): a 10:1 slope through the target, clamped to the gain limits.
//! 3. **Smoother** (A-018): one pole on the gain in dB, with separate attack and release
//!    coefficients, so the gain approaches its target exponentially in the dB domain.
//!
//! The stage runs at the 8 kHz device rate (A-001, R-01), adds no latency (FR-010), and uses
//! only basic IEEE operations plus `rr_dr60_detmath::{ln, exp}`, so it is bit-identical on
//! every platform (R-06).

use core::f64::consts::LN_10;

use rr_dr60_detmath::{exp, ln};

use super::agc_hilbert_coeffs::AGC_HILBERT;
use crate::sanitize::flush_state;
use crate::settings::{AgcSettings, DEVICE_RATE_HZ};

/// Hilbert FIR length (engineering target, 002 R-15).
const TAPS: usize = AGC_HILBERT.len();
/// Hilbert FIR delay in samples, (TAPS − 1) / 2 (R-03).
const DELAY: usize = (TAPS - 1) / 2;
/// Detector floor for e², −200 dB, so `ln` never sees 0 (engineering target, 002 R-15).
const E2_FLOOR: f64 = 1e-20;
/// 10:1 regulation slope: the gain changes by −(1 − 1/10) dB per dB of level (A-017).
const SLOPE: f64 = 0.9;

/// One AGC instance. All state is inline, so creating one allocates nothing beyond the stage
/// itself and processing never allocates (FR-013).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AgcStage {
    /// The last `TAPS` input samples, stored twice so they can be read as one contiguous slice
    /// (`history[pos + 1 ..= pos + TAPS]`, oldest first).
    history: [f64; 2 * TAPS],
    /// Index of the newest sample in the first copy.
    pos: usize,
    /// Current gain in dB.
    gain_db: f64,
    /// Current linear gain, exp(gain_db · ln 10 / 20), applied to the next sample.
    gain: f64,
    target_dbfs: f64,
    max_gain_db: f64,
    max_attenuation_db: f64,
    /// One-pole coefficients for gain falling (attack) and rising (release) (R-05).
    alpha_attack: f64,
    alpha_release: f64,
    #[cfg(feature = "op-count")]
    pub(crate) ops: u64,
}

impl AgcStage {
    /// A stage for validated `settings`, starting at maximum gain as if after silence (A-019).
    pub(crate) fn new(settings: &AgcSettings) -> Self {
        let fs = f64::from(DEVICE_RATE_HZ);
        // R-05: a single pole in dB settles within 2/27 of a step after τ·ln(27/2).
        let ln_13_5 = ln(13.5);
        let tau_attack = f64::from(settings.attack_ms) / 1000.0 / ln_13_5;
        // On a downward step the old peak stays in the detector for DELAY samples (R-05).
        let tau_release = (f64::from(settings.release_ms) / 1000.0 - DELAY as f64 / fs) / ln_13_5;
        let alpha = |tau: f64| 1.0 - exp(-1.0 / (tau * fs));
        let max_gain_db = f64::from(settings.max_gain_db);
        Self {
            history: [0.0; 2 * TAPS],
            pos: 0,
            gain_db: max_gain_db,
            gain: db_to_gain(max_gain_db),
            target_dbfs: f64::from(settings.target_dbfs),
            max_gain_db,
            max_attenuation_db: f64::from(settings.max_attenuation_db),
            alpha_attack: alpha(tau_attack),
            alpha_release: alpha(tau_release),
            #[cfg(feature = "op-count")]
            ops: 0,
        }
    }

    /// Processes one device-rate sample. The sample gets the gain from before this sample's
    /// update, so the first sample after creation or reset gets exactly the maximum gain.
    #[inline]
    pub(crate) fn process(&mut self, x: f64) -> f64 {
        let y = x * self.gain;

        // Push x into both copies of the history.
        self.pos = if self.pos + 1 == TAPS {
            0
        } else {
            self.pos + 1
        };
        self.history[self.pos] = x;
        self.history[self.pos + TAPS] = x;
        // window[j] = x[n − (TAPS − 1) + j]; x[n − k] = window[TAPS − 1 − k].
        let window = &self.history[self.pos + 1..=self.pos + TAPS];

        // Detector (R-02): analytic envelope and peak hold over k = 0 ..= DELAY.
        let mut h = 0.0;
        for k in (0..TAPS).step_by(2) {
            h += AGC_HILBERT[k] * window[TAPS - 1 - k];
        }
        let delayed = window[TAPS - 1 - DELAY];
        let mut e2 = delayed * delayed + h * h;
        for &v in &window[TAPS - 1 - DELAY..] {
            let v2 = v * v;
            if v2 > e2 {
                e2 = v2;
            }
        }
        if e2 < E2_FLOOR {
            e2 = E2_FLOOR;
        }

        // Gain computer (R-04, A-017) and smoother (R-05, A-018).
        let level_dbfs = 10.0 / LN_10 * ln(e2);
        let target_gain = (-SLOPE * (level_dbfs - self.target_dbfs))
            .clamp(-self.max_attenuation_db, self.max_gain_db);
        let alpha = if target_gain < self.gain_db {
            self.alpha_attack
        } else {
            self.alpha_release
        };
        self.gain_db = flush_state(self.gain_db + (target_gain - self.gain_db) * alpha);
        self.gain = db_to_gain(self.gain_db);

        #[cfg(feature = "op-count")]
        {
            // 32 Hilbert multiply-adds, 33 compares, and a fixed amount of scalar work.
            self.ops += (TAPS as u64).div_ceil(2) + (DELAY as u64 + 2) + 12;
        }
        y
    }

    /// Current gain in dB (unit tests).
    #[cfg(test)]
    pub(crate) fn gain_db(&self) -> f64 {
        self.gain_db
    }

    /// Returns to the freshly created state: empty detector, maximum gain (FR-013, A-019).
    pub(crate) fn reset(&mut self) {
        self.history = [0.0; 2 * TAPS];
        self.pos = 0;
        self.gain_db = self.max_gain_db;
        self.gain = db_to_gain(self.max_gain_db);
    }
}

/// dB to linear gain, exp(dB · ln 10 / 20), with detmath (R-06).
#[inline]
fn db_to_gain(gain_db: f64) -> f64 {
    exp(gain_db * (LN_10 / 20.0))
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::settings::AgcSettings;
    use rr_dr60_detmath::{TAU, ln, sin};
    use std::vec::Vec;

    const FS: f64 = 8000.0; // A-001: the AGC runs at the device rate (002 R-01)

    fn amp(dbfs: f64) -> f64 {
        rr_dr60_detmath::exp(dbfs * ln(10.0) / 20.0)
    }

    fn to_db(ratio: f64) -> f64 {
        20.0 * ln(ratio) / ln(10.0)
    }

    /// 1 kHz tone sample `n` at `dbfs` (AES17: peak amplitude). At 8 kHz a 1 kHz tone has
    /// exactly 8 samples per cycle, so sample peaks hit the true peak.
    fn tone(dbfs: f64, n: usize) -> f64 {
        if dbfs == f64::NEG_INFINITY {
            return 0.0; // digital silence (+0.0, not 0 · sin = −0.0)
        }
        amp(dbfs) * sin(TAU * ((n % 8) as f64) / 8.0)
    }

    /// Runs `levels` (dBFS, seconds) through a stage, returning (output, gain dB per sample).
    fn run(stage: &mut AgcStage, levels: &[(f64, f64)]) -> (Vec<f64>, Vec<f64>) {
        let (mut y, mut g) = (Vec::new(), Vec::new());
        let mut n = 0;
        for &(dbfs, secs) in levels {
            for _ in 0..(secs * FS) as usize {
                y.push(stage.process(tone(dbfs, n)));
                g.push(stage.gain_db());
                n += 1;
            }
        }
        (y, g)
    }

    fn peak_dbfs(y: &[f64]) -> f64 {
        to_db(y.iter().fold(0.0f64, |m, v| m.max(v.abs())))
    }

    /// Settling index: last sample outside 2/27 of the excursion (spec 002 Overview).
    fn settle(g: &[f64], final_db: f64, excursion: f64) -> usize {
        let band = excursion.abs() * 2.0 / 27.0;
        g.iter()
            .rposition(|v| (v - final_db).abs() > band)
            .map_or(0, |i| i + 1)
    }

    /// 002 FR-004, FR-005 (A-017): the static curve at default settings. ±1 dB is an
    /// engineering target (002 FR-015).
    #[test]
    fn static_curve_at_defaults() {
        for (input, want) in [
            (-60.0, -20.0),
            (-40.0, -13.0),
            (-10.0, -10.0),
            (0.0, -9.0),
            (5.0, -8.5),
        ] {
            let mut stage = AgcStage::new(&AgcSettings::DEVICE);
            let (y, _) = run(&mut stage, &[(input, 1.0)]);
            let got = peak_dbfs(&y[y.len() - 2000..]);
            assert!(
                (got - want).abs() <= 1.0,
                "{input} dBFS in: {got:.3} out, want {want}"
            );
        }
    }

    /// 002 FR-013, A-019: a fresh stage applies exactly the maximum gain to its first sample.
    #[test]
    fn starts_at_maximum_gain() {
        let mut stage = AgcStage::new(&AgcSettings::DEVICE);
        assert_eq!(stage.gain_db(), 40.0);
        let x = 0.25;
        let y = stage.process(x);
        assert!(
            (y / x / 100.0 - 1.0).abs() < 1e-9,
            "first-sample gain {}",
            y / x
        );
    }

    /// 002 FR-007 (US1 AS4): digital silence gives exact zeros, and after a loud tone the gain
    /// returns to maximum during silence (no hold, no gate; A-019).
    #[test]
    fn silence_and_gain_recovery() {
        let mut stage = AgcStage::new(&AgcSettings::DEVICE);
        let (y, _) = run(&mut stage, &[(f64::NEG_INFINITY, 10.0)]);
        assert!(
            y.iter().all(|&v| v.to_bits() == 0),
            "silence must stay exactly 0.0"
        );
        let (_, g) = run(&mut stage, &[(0.0, 1.0), (f64::NEG_INFINITY, 10.0)]);
        assert!(
            g[8000] < 10.0,
            "loud tone should pull the gain down: {}",
            g[8000]
        );
        assert!(
            (g[g.len() - 1] - 40.0).abs() < 0.1,
            "after silence: {}",
            g[g.len() - 1]
        );
    }

    /// 002 FR-006 (A-018): attack 10 ms ± 2 ms, release 1.0 s ± 0.2 s, and the release shape
    /// check (35–65 % recovered at 25 % of the release time). The prototype measured 10.00 ms,
    /// 1.001 s and 47 % (research.md R-05).
    #[test]
    fn attack_release_and_shape_at_defaults() {
        let mut stage = AgcStage::new(&AgcSettings::DEVICE);
        let (_, g) = run(&mut stage, &[(-40.0, 0.5), (-10.0, 0.3), (-40.0, 2.0)]);
        let (s1, s2) = (4000, 4000 + 2400);
        let up = &g[s1..s2];
        let attack = settle(up, up[up.len() - 1], g[s1 - 1] - up[up.len() - 1]);
        let attack_ms = attack as f64 / FS * 1000.0;
        std::eprintln!("AGC stage at 8 kHz: attack {attack_ms:.3} ms");
        assert!((attack_ms - 10.0).abs() <= 2.0, "attack {attack_ms:.3} ms");

        let down = &g[s2..];
        let (start, fin) = (g[s2 - 1], down[down.len() - 1]);
        let release = settle(down, fin, fin - start);
        let release_s = release as f64 / FS;
        assert!((release_s - 1.0).abs() <= 0.2, "release {release_s:.4} s");
        let frac = (down[release / 4] - start) / (fin - start);
        std::eprintln!("AGC stage at 8 kHz: release {release_s:.4} s, midpoint {frac:.3}");
        assert!((0.35..=0.65).contains(&frac), "midpoint fraction {frac:.3}");
    }

    /// 002 R-07: with maximum attenuation 0 and a tone above the target, the target gain is
    /// −0.0 and the gain decays from 40 dB toward 0. It must flush to exactly 0.0 without ever
    /// being subnormal, after which the output equals the input bit for bit.
    #[test]
    fn gain_flushes_to_zero_without_subnormals() {
        let mut settings = AgcSettings::DEVICE;
        settings.max_attenuation_db = 0.0;
        let mut stage = AgcStage::new(&settings);
        let mut flushed_at = None;
        for n in 0..(600.0 * FS) as usize {
            let x = tone(0.0, n);
            let y = stage.process(x);
            let g = stage.gain_db();
            assert!(!g.is_subnormal(), "gain subnormal at sample {n}");
            if let Some(at) = flushed_at {
                assert_eq!(y.to_bits(), x.to_bits(), "sample {n} (flushed at {at})");
            } else if g.to_bits() == 0 {
                flushed_at = Some(n);
            }
        }
        assert!(flushed_at.is_some(), "gain never reached exactly 0.0");
    }

    /// 002 FR-013: reset returns the stage to its freshly created state.
    #[test]
    fn reset_equals_fresh() {
        let mut used = AgcStage::new(&AgcSettings::DEVICE);
        run(&mut used, &[(0.0, 0.5), (-40.0, 0.2)]);
        used.reset();
        let mut fresh = AgcStage::new(&AgcSettings::DEVICE);
        let a = run(&mut used, &[(-20.0, 0.3)]);
        let b = run(&mut fresh, &[(-20.0, 0.3)]);
        assert_eq!(a, b);
    }
}
