//! The decoder's short-term postfilter (spec 008 research R-05; contracts/codec-design.md ›
//! Decoder): `H(z) = Â(z/0.65) / Â(z/0.75)` (S-008 §3.6, factors as published), a first-order
//! tilt compensation `1 − μ z⁻¹` with `μ = 0.25 · k₁` (k₁ the first reflection coefficient of
//! the numerator's impulse response) and gain scaling to the subframe energy before the
//! postfilter, smoothed per sample with a one-pole smoother of 0.9 (S-008 §3.7). No pitch
//! postfilter in this model version (A-041). Design constants of the model, not device figures.

use super::fixed::{Q12_ONE, Q15_ONE, div_q, isqrt, mac, mult, round_q12, sat16};
use super::lpc::{self, Lpc};
use super::tables::{
    GAMMA_PD_POW, GAMMA_PN_POW, LPC_ORDER, POST_GAIN_SMOOTH, SUBFRAME_SAMPLES, TILT_FACTOR,
};

/// The gain scaler's format: Q12, so a factor up to 8 fits.
const GAIN_ONE_Q12: i32 = Q12_ONE as i32;
/// The largest scaling factor (8.0 in Q12; engineering target, the reference code's range).
const GAIN_MAX_Q12: i32 = 32767; // engineering target (008 R-05)

/// The postfilter's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Postfilter {
    /// The numerator's memory: the last eight inputs.
    num_mem: [i32; LPC_ORDER],
    /// The denominator's memory: the last eight outputs of the all-pole part.
    den_mem: [i32; LPC_ORDER],
    /// The tilt filter's memory: the previous all-pole output.
    tilt_mem: i32,
    /// The smoothed gain, Q12.
    gain_q12: i32,
}

impl Default for Postfilter {
    fn default() -> Self {
        Self {
            num_mem: [0; LPC_ORDER],
            den_mem: [0; LPC_ORDER],
            tilt_mem: 0,
            gain_q12: GAIN_ONE_Q12,
        }
    }
}

/// `μ = 0.25 · k₁` (Q15) of the numerator polynomial `an`: k₁ = r(1) / r(0) of its impulse
/// response (the nine coefficients themselves).
pub(crate) fn tilt_mu(an: &Lpc) -> i32 {
    let (mut r0, mut r1) = (0i64, 0i64);
    for (i, &a) in an.iter().enumerate() {
        r0 = mac(r0, a, a);
        if let Some(&next) = an.get(i.wrapping_add(1)) {
            r1 = mac(r1, a, next);
        }
    }
    let k1 = sat16(div_q(r1, r0.max(1), 15));
    mult(TILT_FACTOR, k1)
}

impl Postfilter {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    /// One subframe of decoded speech `input` (Q15) through the postfilter with the subframe's
    /// predictor `a` (Q12) into `out`.
    pub(crate) fn process(
        &mut self,
        a: &Lpc,
        input: &[i32; SUBFRAME_SAMPLES],
        out: &mut [i32; SUBFRAME_SAMPLES],
    ) {
        let an = lpc::weighted(a, &GAMMA_PN_POW);
        let ad = lpc::weighted(a, &GAMMA_PD_POW);
        let mut r = [0i32; SUBFRAME_SAMPLES];
        lpc::fir(&an, input, &mut self.num_mem, &mut r);
        let mut y = [0i32; SUBFRAME_SAMPLES];
        lpc::iir(&ad, &r, &mut self.den_mem, &mut y);
        // Tilt compensation: t[n] = y[n] − μ y[n−1].
        let mu = tilt_mu(&an);
        let mut t = [0i32; SUBFRAME_SAMPLES];
        let mut prev = self.tilt_mem;
        for (tn, &yn) in t.iter_mut().zip(&y) {
            *tn = sat16(i64::from(yn).saturating_sub(i64::from(mult(mu, prev))));
            prev = yn;
        }
        self.tilt_mem = prev;
        // Gain scaling: √(E_in / E_out) in Q12, smoothed per sample.
        let (mut e_in, mut e_out) = (0i64, 0i64);
        for (&x, &tn) in input.iter().zip(&t) {
            e_in = mac(e_in, x, x);
            e_out = mac(e_out, tn, tn);
        }
        let target = if e_out == 0 {
            GAIN_ONE_Q12
        } else {
            // √(E_in · 2²⁴ / E_out): the ratio in Q24, its root in Q12.
            let ratio = div_q(e_in, e_out, 24).max(0);
            // range-proven: the clamp keeps the root within i32.
            isqrt(ratio as u64).min(i64::from(GAIN_MAX_Q12) as u64) as i32
        };
        let keep = POST_GAIN_SMOOTH;
        let feed = sat16(Q15_ONE.saturating_sub(i64::from(keep)));
        for (o, &tn) in out.iter_mut().zip(&t) {
            // gs = 0.9 gs + 0.1 target (Q12 values with Q15 factors).
            self.gain_q12 = sat16(
                i64::from(mult(self.gain_q12, keep)).saturating_add(i64::from(mult(target, feed))),
            );
            *o = round_q12(i64::from(tn).saturating_mul(i64::from(self.gain_q12)));
        }
    }
}

/// 008 T013 (spec 008 research R-05; S-008 §3.6 – 3.7): unity gain on a flat spectrum, the
/// energy scaling, and the tilt term's sign.
#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, clippy::disallowed_methods)] // test arithmetic
mod tests {
    extern crate std;
    use super::*;
    use crate::codec::lpc::FLAT;

    fn energy(x: &[i32]) -> f64 {
        x.iter().map(|&v| f64::from(v) * f64::from(v)).sum()
    }

    /// With the flat predictor the postfilter is the identity: unity gain within 0.1 dB (it is
    /// exact) and no tilt.
    #[test]
    fn unity_gain_on_a_flat_spectrum() {
        let mut pf = Postfilter::default();
        let mut x = [0i32; 60];
        for (n, v) in x.iter_mut().enumerate() {
            *v = ((n as i32 * 1931) % 16001) - 8000;
        }
        let mut y = [0i32; 60];
        pf.process(&FLAT, &x, &mut y);
        assert_eq!(y, x);
        assert_eq!(tilt_mu(&FLAT), 0);
        let db = 10.0 * (energy(&y) / energy(&x)).log10();
        assert!(db.abs() <= 0.1, "{db} dB");
        assert_eq!(pf.gain_q12, 4096);
    }

    /// With a resonant predictor the output's energy tracks the input's within 1 dB once the
    /// smoother has settled (the second subframe), and the state resets.
    #[test]
    fn gain_scaling_tracks_the_input_energy() {
        let a: Lpc = [4096, -6000, 5000, -2500, 1200, -500, 200, -80, 30];
        let mut pf = Postfilter::default();
        let mut x = [0i32; 60];
        for (n, v) in x.iter_mut().enumerate() {
            *v = (8000.0 * (n as f64 * 0.7).sin() + 3000.0 * (n as f64 * 2.1).cos()) as i32;
        }
        let mut y = [0i32; 60];
        pf.process(&a, &x, &mut y);
        pf.process(&a, &x, &mut y);
        let db = 10.0 * (energy(&y) / energy(&x)).log10();
        assert!(db.abs() <= 1.0, "{db} dB");
        assert_ne!(y, x, "the postfilter shapes the spectrum");
        pf.reset();
        assert_eq!(pf, Postfilter::default());
        // Silence in, silence out, the gain held at unity.
        pf.process(&a, &[0; 60], &mut y);
        assert_eq!(y, [0; 60]);
        assert_eq!(pf.gain_q12, 4096);
    }

    /// The tilt term's sign follows k₁ of the numerator: a numerator with a positive first
    /// coefficient (its impulse response correlates positively at lag 1) gives μ > 0, a negative
    /// one μ < 0, and |μ| ≤ 0.25.
    #[test]
    fn tilt_sign_follows_the_numerator() {
        let mut up = FLAT;
        up[1] = 2048; // h = [1, 0.5]: r1 > 0
        let mu_up = tilt_mu(&up);
        assert!(mu_up > 0, "{mu_up}");
        let mut down = FLAT;
        down[1] = -2048;
        let mu_down = tilt_mu(&down);
        assert!(mu_down < 0, "{mu_down}");
        assert_eq!(mu_up, -mu_down);
        // k₁ = 0.5 / 1.25 = 0.4 → μ = 0.1.
        assert!((mu_up - 3277).abs() <= 2, "{mu_up}");
        assert!(mu_up.abs() <= 8192 && mu_down.abs() <= 8192);
        assert_eq!(TILT_FACTOR, 8192);
        assert_eq!(POST_GAIN_SMOOTH, 29491);
    }
}
