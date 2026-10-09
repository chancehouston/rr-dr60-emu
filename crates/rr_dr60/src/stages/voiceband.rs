//! Voice-band stage: signal-chain stages 4 and 10 (A-002, A-014, A-015, A-016).
//!
//! Models signal-chain stage 4 (record anti-alias filter + ADC) and stage 10 (DAC +
//! reconstruction filter) of the MSM7702 voice-band codec. Both are assumed to share one
//! G.712-like band-pass template (A-002, A-014), with unity passband gain (A-015), and to be
//! minimum-phase (A-016). These are modeled on the codec class, not measured from a real unit.

use rr_dr60_detmath::{TAU, cos, sin};

use super::biquad::Biquad;
use super::voiceband_coeffs::VOICEBAND_SOS;
use crate::settings::DEVICE_RATE_HZ;

/// The voice-band stage: six biquad sections at the 8 kHz device rate (A-001).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct VoiceBandStage {
    sections: [Biquad; 6],
}

impl VoiceBandStage {
    /// A stage using the committed coefficients (`voiceband_coeffs.rs`), with zero state.
    pub(crate) fn new() -> Self {
        Self::with_sos(VOICEBAND_SOS)
    }

    /// A stage with explicit coefficients. Used by `new` and, through the hidden
    /// `__test-hooks` feature, by the SC-008 mutation test (tasks.md T057).
    pub(crate) fn with_sos(sos: [[f64; 5]; 6]) -> Self {
        Self {
            sections: sos.map(Biquad::from_sos),
        }
    }

    /// Processes one device-rate sample.
    #[inline]
    pub(crate) fn process(&mut self, x: f64) -> f64 {
        let mut y = x;
        for s in &mut self.sections {
            y = s.process(y);
        }
        y
    }

    /// Multiply-adds performed by all sections (test-only; tasks.md T059).
    #[cfg(all(test, feature = "op-count"))]
    pub(crate) fn ops(&self) -> u64 {
        self.sections.iter().map(|s| s.ops).sum()
    }

    /// Clears all state. No allocation (FR-017).
    pub(crate) fn reset(&mut self) {
        for s in &mut self.sections {
            s.reset();
        }
    }

    /// Group delay at 1 kHz, in device-rate samples, computed analytically from the
    /// coefficients (research.md R-10). Uses `rr_dr60_detmath`, so it is bit-identical
    /// everywhere and the reported latency is too.
    pub(crate) fn group_delay_1k_samples(&self) -> f64 {
        let w = TAU * 1000.0 / f64::from(DEVICE_RATE_HZ);
        self.sections
            .iter()
            .map(|s| {
                let [b0, b1, b2, a1, a2] = s.coefficients();
                poly_group_delay([b0, b1, b2], w) - poly_group_delay([1.0, a1, a2], w)
            })
            .sum()
    }
}

/// Group delay in samples of P(z) = p0 + p1·z⁻¹ + p2·z⁻², at angular frequency `w`:
/// Re(Σ k·p_k·e^(−jωk) / Σ p_k·e^(−jωk)).
fn poly_group_delay(p: [f64; 3], w: f64) -> f64 {
    let (c1, s1, c2, s2) = (cos(w), sin(w), cos(2.0 * w), sin(2.0 * w));
    // Denominator D = p0 + p1·e^(−jω) + p2·e^(−j2ω); numerator N = p1·e^(−jω) + 2·p2·e^(−j2ω).
    let (dr, di) = (p[0] + p[1] * c1 + p[2] * c2, -(p[1] * s1 + p[2] * s2));
    let (nr, ni) = (p[1] * c1 + 2.0 * p[2] * c2, -(p[1] * s1 + 2.0 * p[2] * s2));
    (nr * dr + ni * di) / (dr * dr + di * di)
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test-only analysis; not bit-critical.
mod tests {
    extern crate std;
    use super::*;
    use crate::stages::voiceband_coeffs::VOICEBAND_SOS;
    use std::vec::Vec;

    const FS: f64 = 8000.0; // A-001

    /// Complex response of the committed cascade at `f` Hz: (re, im).
    fn response(f: f64) -> (f64, f64) {
        let w = std::f64::consts::TAU * f / FS;
        let (z1r, z1i) = (w.cos(), -w.sin());
        let (z2r, z2i) = ((2.0 * w).cos(), -(2.0 * w).sin());
        let mut h = (1.0, 0.0);
        for [b0, b1, b2, a1, a2] in VOICEBAND_SOS {
            let br = b0 + b1 * z1r + b2 * z2r;
            let bi = b1 * z1i + b2 * z2i;
            let ar = 1.0 + a1 * z1r + a2 * z2r;
            let ai = a1 * z1i + a2 * z2i;
            let d = ar * ar + ai * ai;
            let (qr, qi) = ((br * ar + bi * ai) / d, (bi * ar - br * ai) / d);
            h = (h.0 * qr - h.1 * qi, h.0 * qi + h.1 * qr);
        }
        h
    }

    fn mag_db(f: f64) -> f64 {
        let (r, i) = response(f);
        10.0 * (r * r + i * i).log10()
    }

    /// Numerical group delay in samples (central difference of the unwrapped phase).
    fn group_delay_numeric(f: f64, h: f64) -> f64 {
        let (r1, i1) = response(f - h);
        let (r2, i2) = response(f + h);
        // arg(H2 / H1)
        let dphi = (i2 * r1 - r2 * i1).atan2(r2 * r1 + i2 * i1);
        -dphi / (std::f64::consts::TAU * 2.0 * h / FS)
    }

    #[test]
    fn fr010_gain_at_1k_is_unity() {
        assert!(mag_db(1000.0).abs() <= 0.1, "{}", mag_db(1000.0)); // A-015
    }

    #[test]
    fn fr010_band_edges_ripple_and_stopbands() {
        let g1k = mag_db(1000.0);
        let grid: Vec<(f64, f64)> = (0..=4000)
            .map(|f| (f as f64, mag_db(f as f64) - g1k))
            .collect();
        let lo3 = grid
            .iter()
            .filter(|(f, r)| *f < 1000.0 && *r < -3.0)
            .map(|p| p.0)
            .fold(f64::MIN, f64::max);
        let hi3 = grid
            .iter()
            .filter(|(f, r)| *f > 1000.0 && *r < -3.0)
            .map(|p| p.0)
            .fold(f64::MAX, f64::min);
        assert!((lo3 - 300.0).abs() <= 50.0, "lower -3 dB at {lo3} Hz"); // A-002, A-014
        assert!((hi3 - 3400.0).abs() <= 50.0, "upper -3 dB at {hi3} Hz"); // A-002, A-014
        for &(f, r) in &grid {
            if (400.0..=3200.0).contains(&f) {
                assert!(r.abs() <= 0.5, "ripple {r} dB at {f} Hz"); // A-014
            }
            if (1.0..=60.0).contains(&f) {
                assert!(r <= -20.0, "{r} dB at {f} Hz"); // A-014
            }
        }
        assert!(mag_db(0.0) - g1k <= -40.0, "DC"); // A-014
        assert!(
            mag_db(4000.0) - g1k <= -14.0,
            "4000 Hz: {}",
            mag_db(4000.0) - g1k
        ); // A-014, z = -1
    }

    #[test]
    fn fr010_group_delay_and_minimum_phase() {
        let ms = |samples: f64| samples / FS * 1000.0;
        let gd1k = group_delay_numeric(1000.0, 0.01);
        assert!(ms(gd1k) <= 2.0, "group delay at 1 kHz = {} ms", ms(gd1k)); // A-016
        assert!(group_delay_numeric(400.0, 0.01) > gd1k); // A-016
        assert!(group_delay_numeric(3200.0, 0.01) > gd1k); // A-016
        for [b0, b1, b2, a1, a2] in VOICEBAND_SOS {
            // Jury test: poles strictly inside the unit circle...
            assert!(
                a2.abs() < 1.0 && a1.abs() < 1.0 + a2,
                "unstable section {a1} {a2}"
            );
            // ...and zeros on or inside it (minimum phase), with root-finding slack.
            let (c0, c1, c2) = if b0 < 0.0 {
                (-b0, -b1, -b2)
            } else {
                (b0, b1, b2)
            };
            assert!(
                c2.abs() <= c0 * (1.0 + 1e-9) && c1.abs() <= (c0 + c2) * (1.0 + 1e-9),
                "zeros outside"
            );
        }
    }

    #[test]
    fn analytic_group_delay_matches_numeric() {
        let g = VoiceBandStage::new().group_delay_1k_samples();
        let n = group_delay_numeric(1000.0, 0.01);
        assert!((g - n).abs() <= 1e-8, "analytic {g} vs numeric {n}");
    }

    #[test]
    fn stage_process_matches_cascade_and_resets() {
        let mut s = VoiceBandStage::new();
        let first: Vec<f64> = (0..64)
            .map(|n| s.process(if n == 0 { 1.0 } else { 0.0 }))
            .collect();
        s.reset();
        let again: Vec<f64> = (0..64)
            .map(|n| s.process(if n == 0 { 1.0 } else { 0.0 }))
            .collect();
        assert_eq!(first, again);
        assert!(first.iter().any(|&v| v != 0.0));
    }
}
