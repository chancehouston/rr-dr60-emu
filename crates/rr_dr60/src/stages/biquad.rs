//! Transposed direct form II biquad section in f64 (research.md R-07).

use crate::sanitize::flush_state;

/// One second-order IIR section in transposed direct form II, f64 coefficients and state
/// (research.md R-07). `a0` is normalized to 1.
///
/// y = b0·x + s1;  s1 ← b1·x − a1·y + s2;  s2 ← b2·x − a2·y
///
/// Both state values are flushed to exactly 0.0 together once both are below 1e-30 (R-05). Only
/// basic IEEE operations are used, so results are bit-identical on every platform (R-04).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    s1: f64,
    s2: f64,
    /// Multiply-adds performed (test-only, FR-015 bounded work; tasks.md T059).
    #[cfg(feature = "op-count")]
    pub(crate) ops: u64,
}

impl Biquad {
    /// A section from `[b0, b1, b2, a1, a2]`, with zero state.
    pub(crate) const fn from_sos(c: [f64; 5]) -> Self {
        Self {
            b0: c[0],
            b1: c[1],
            b2: c[2],
            a1: c[3],
            a2: c[4],
            s1: 0.0,
            s2: 0.0,
            #[cfg(feature = "op-count")]
            ops: 0,
        }
    }

    /// Processes one sample.
    #[inline]
    pub(crate) fn process(&mut self, x: f64) -> f64 {
        #[cfg(feature = "op-count")]
        {
            self.ops += 5;
        }
        let y = self.b0 * x + self.s1;
        let s1 = self.b1 * x - self.a1 * y + self.s2;
        let s2 = self.b2 * x - self.a2 * y;
        // Flush both states together, and only when both are tiny (R-05). Flushing them one
        // at a time breaks the cancellation between s1 and s2 in sections with poles near
        // z = 1, which can sustain a ~1e-30 limit cycle forever.
        if flush_state(s1) == 0.0 && flush_state(s2) == 0.0 {
            self.s1 = 0.0;
            self.s2 = 0.0;
        } else {
            self.s1 = s1;
            self.s2 = s2;
        }
        y
    }

    /// Clears the state. No allocation (FR-017).
    pub(crate) fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    /// The coefficients `[b0, b1, b2, a1, a2]`.
    pub(crate) const fn coefficients(&self) -> [f64; 5] {
        [self.b0, self.b1, self.b2, self.a1, self.a2]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stages::voiceband_coeffs::VOICEBAND_SOS;

    #[test]
    fn impulse_response_matches_hand_computation() {
        // y[n] = 0.5 x[n] + 0.25 x[n-1] + 0.5 y[n-1]
        let mut b = Biquad::from_sos([0.5, 0.25, 0.0, -0.5, 0.0]);
        let want = [0.5, 0.5, 0.25, 0.125, 0.0625, 0.03125, 0.015625, 0.0078125];
        for (n, w) in want.iter().enumerate() {
            let x = if n == 0 { 1.0 } else { 0.0 };
            assert_eq!(b.process(x), *w, "n = {n}");
        }
    }

    #[test]
    fn cascade_tail_reaches_exact_zero_without_a_limit_cycle() {
        // Regression (found by the US3 tail edge-case test): flushing s1 and s2 separately let
        // the two-stage cascade sustain a ~1.9e-30 oscillation forever after a full-scale sweep.
        let mut cascade: [Biquad; 12] =
            core::array::from_fn(|i| Biquad::from_sos(VOICEBAND_SOS[i % 6]));
        let mut phase = 0.0f64;
        for n in 0..8000 {
            // Exponential chirp 20 Hz -> 3600 Hz at 8 kHz, amplitude 1 (basic ops only).
            let f = 20.0 * (1.0 + 179.0 * n as f64 / 8000.0);
            phase += f / 8000.0;
            phase -= (phase as i64) as f64;
            let x = if phase < 0.5 {
                4.0 * phase - 1.0
            } else {
                3.0 - 4.0 * phase
            }; // triangle
            cascade.iter_mut().fold(x, |v, s| s.process(v));
        }
        let mut last_nonzero = 0;
        for n in 0..160_000 {
            if cascade.iter_mut().fold(0.0, |v, s| s.process(v)) != 0.0 {
                last_nonzero = n;
            }
        }
        assert!(
            cascade.iter().all(|s| s.s1 == 0.0 && s.s2 == 0.0),
            "state never reached exact zero"
        );
        assert!(
            last_nonzero < 40_000,
            "output nonzero until sample {last_nonzero} (5 s)"
        );
    }

    #[test]
    fn reset_zeroes_state() {
        let mut b = Biquad::from_sos([0.5, 0.25, 0.1, -0.5, 0.1]);
        b.process(1.0);
        b.reset();
        assert_eq!(b.process(0.0), 0.0);
        assert_eq!((b.s1, b.s2), (0.0, 0.0));
    }

    #[test]
    fn voiceband_cascade_tail_decays_then_flushes_to_zero() {
        // Spec Edge Cases: below -120 dBFS within 0.5 s (4,000 samples at 8 kHz) after input
        // stops; exactly 0.0 within 20,000 samples (R-05).
        let mut cascade: [Biquad; 6] = VOICEBAND_SOS.map(Biquad::from_sos);
        for n in 0..800 {
            let x = if n % 16 < 8 { 1.0 } else { -1.0 }; // 0 dBFS square-ish drive
            let mut y = x;
            for s in cascade.iter_mut() {
                y = s.process(y);
            }
        }
        let mut last_nonzero = 0;
        for n in 0..20_000 {
            let mut y: f64 = 0.0;
            for s in cascade.iter_mut() {
                y = s.process(y);
            }
            if n >= 4_000 {
                assert!(
                    y.abs() < 1e-6,
                    "sample {n} after stop = {y:e} (> -120 dBFS)"
                );
            }
            if y != 0.0 {
                last_nonzero = n;
            }
        }
        assert!(last_nonzero < 19_999, "tail never reached exact 0.0");
    }
}
