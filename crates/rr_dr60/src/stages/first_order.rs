//! First-order IIR section in f64 (spec 009 research R-03).
//!
//! The output chain runs up to three of these per host sample (the volume control's 32 Hz
//! coupling corner, the amplifier's 159 Hz corner or the earphone follower's two corners, and
//! the speaker template's shelf), so a dedicated section with three multiplies and one state
//! replaces a [`Biquad`](super::biquad::Biquad) with zeroed second-order terms (five multiplies).

use crate::sanitize::flush_state;

/// One first-order IIR section in transposed direct form II, f64 coefficients and state:
///
/// y = b0·x + s;  s ← b1·x − a1·y
///
/// i.e. `H(z) = (b0 + b1 z⁻¹) / (1 + a1 z⁻¹)`. The state is flushed to exactly 0.0 below 1e-30
/// (the biquad's rule, spec 001 research R-05), so a tail reaches digital silence and no
/// subnormal slows the per-sample path. Only basic IEEE operations are used, so results are
/// bit-identical on every platform (001 R-04).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FirstOrder {
    b0: f64,
    b1: f64,
    a1: f64,
    s: f64,
}

impl FirstOrder {
    /// A section from `[b0, b1, a1]` (the generated tables' form), with zero state.
    pub(crate) const fn new(c: [f64; 3]) -> Self {
        Self {
            b0: c[0],
            b1: c[1],
            a1: c[2],
            s: 0.0,
        }
    }

    /// Processes one sample.
    #[inline]
    pub(crate) fn process(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.s;
        self.s = flush_state(self.b1 * x - self.a1 * y);
        y
    }

    /// Clears the state. No allocation.
    pub(crate) fn reset(&mut self) {
        self.s = 0.0;
    }

    /// The state (test-only: the flush and reset assertions).
    #[cfg(test)]
    pub(crate) const fn state(&self) -> f64 {
        self.s
    }

    /// The coefficients `[b0, b1, a1]`.
    pub(crate) const fn coefficients(&self) -> [f64; 3] {
        [self.b0, self.b1, self.a1]
    }
}

/// The −3 dB point in Hz of a first-order high-pass section from its pole alone (spec 009
/// FR-006, FR-007: the corners are too close to fit from a measured response, hardware review
/// P8, so each one is read from the realized coefficients): the bilinear pole `a = −a1` maps to
/// `fc = fs · atan((1 − a) / (1 + a)) / π`. Test-only.
#[cfg(test)]
#[allow(clippy::disallowed_methods)] // analysis in a test, not the DSP path (001 R-04)
pub(crate) fn corner_hz(c: [f64; 3], fs: f64) -> f64 {
    let a = -c[2];
    fs * ((1.0 - a) / (1.0 + a)).atan() / core::f64::consts::PI
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // closed forms and prototypes in tests, not the DSP path (001 R-04)
mod tests {
    use super::*;
    use crate::settings::SUPPORTED_HOST_RATES;
    use crate::stages::output_coeffs::{
        AMP_HPF, CORNER_HZ, EAR_IN, EAR_OUT, F_AMP_HZ, F_EAR_IN_HZ, F_EAR_OUT_HZ, F_VOL_HZ, RATES,
        VOL_HPF,
    };

    /// A prewarped bilinear high-pass at `fc` and `fs` (the script's formula, repeated here so
    /// the impulse-response test does not depend on the generated tables).
    fn hp(fc: f64, fs: f64) -> [f64; 3] {
        let t = (core::f64::consts::PI * fc / fs).tan();
        let a = (1.0 - t) / (1.0 + t);
        let g = (1.0 + a) / 2.0;
        [g, -g, -a]
    }

    /// 009 T004: the impulse response equals the closed form `h[0] = b0`,
    /// `h[n] = (b1 − a1 b0) (−a1)^(n−1)` for `n ≥ 1`.
    #[test]
    fn impulse_response_matches_closed_form() {
        let c = hp(100.0, 48_000.0);
        let mut f = FirstOrder::new(c);
        assert_eq!(f.coefficients(), c);
        let (b0, b1, a1) = (c[0], c[1], c[2]);
        for n in 0..64 {
            let x = if n == 0 { 1.0 } else { 0.0 };
            let got = f.process(x);
            let want = if n == 0 {
                b0
            } else {
                (b1 - a1 * b0) * (-a1).powi(n - 1)
            };
            assert!((got - want).abs() < 1e-15, "n = {n}: {got} vs {want}");
        }
    }

    /// 009 T004: a high-pass with `b1 = −b0` has exactly zero DC gain, `(b0 + b1) / (1 + a1)`.
    #[test]
    fn high_pass_blocks_dc() {
        let c = hp(32.0, 6000.0);
        let dc = (c[0] + c[1]) / (1.0 + c[2]);
        assert!(dc.abs() < 1e-12, "{dc}");
        let mut f = FirstOrder::new(c);
        let mut y = 0.0;
        for _ in 0..200_000 {
            y = f.process(1.0);
        }
        assert!(y.abs() < 1e-12, "a unit step settles to {y}");
    }

    /// 009 T004: after a 1 ms impulse's tail the state flushes to exactly 0.0, and `reset`
    /// zeroes it.
    #[test]
    fn tail_flushes_to_exact_zero_and_reset_clears() {
        let mut f = FirstOrder::new(hp(100.0, 48_000.0));
        for n in 0..48 {
            f.process(if n < 48 { 1.0 } else { 0.0 });
        }
        let mut last_nonzero = 0;
        for n in 0..480_000 {
            if f.process(0.0) != 0.0 {
                last_nonzero = n;
            }
        }
        assert_eq!(f.state(), 0.0, "the state flushed");
        assert!(last_nonzero < 480_000 - 1, "the output reached exact zero");
        let mut g = FirstOrder::new(hp(100.0, 48_000.0));
        g.process(1.0);
        assert_ne!(g.state(), 0.0);
        g.reset();
        assert_eq!(g.state(), 0.0);
    }

    /// 009 T004: the state stays finite on ±1e6 input.
    #[test]
    fn state_stays_finite_on_large_input() {
        let mut f = FirstOrder::new(hp(32.0, 6000.0));
        for n in 0..6000 {
            let x = if n % 2 == 0 { 1e6 } else { -1e6 };
            let y = f.process(x);
            assert!(y.is_finite() && f.state().is_finite());
        }
    }

    /// 009 T005 (FR-006, FR-007; A-031, A-032): each first-order corner, computed from its
    /// realized coefficients at each of the seven rates, is within ±10 % of its registered
    /// figure (32, 159, 120 and 280 Hz) and within 0.1 % of the script's own recomputation.
    #[test]
    fn corners_from_coefficients_are_within_10_percent_at_seven_rates() {
        assert_eq!(RATES, SUPPORTED_HOST_RATES, "the tables' index order");
        let sets: [(&str, &[[f64; 3]; 7], f64); 4] = [
            ("vol", &VOL_HPF, F_VOL_HZ),         // A-031
            ("amp", &AMP_HPF, F_AMP_HZ),         // A-031
            ("ear_in", &EAR_IN, F_EAR_IN_HZ),    // A-032
            ("ear_out", &EAR_OUT, F_EAR_OUT_HZ), // A-032
        ];
        for (i, &fs) in RATES.iter().enumerate() {
            for (j, (name, table, want)) in sets.iter().enumerate() {
                let got = corner_hz(table[i], f64::from(fs));
                assert!(
                    (got - want).abs() <= 0.1 * want,
                    "{fs} Hz {name}: {got} Hz vs {want} ± 10 %"
                );
                let script = CORNER_HZ[i][j];
                assert!(
                    (got - script).abs() <= 1e-3 * script,
                    "{fs} Hz {name}: {got} vs the script's {script}"
                );
                // Each section is a high-pass with unity gain at Nyquist: b1 = −b0, b0 = (1 + a)/2.
                assert_eq!(table[i][1], -table[i][0], "{fs} Hz {name}");
                let a = -table[i][2];
                assert!(
                    (table[i][0] - (1.0 + a) / 2.0).abs() < 1e-15,
                    "{fs} Hz {name}"
                );
            }
        }
    }
}
