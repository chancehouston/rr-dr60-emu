//! The decoder's short-term postfilter (spec 008 research R-05; contracts/codec-design.md ›
//! Decoder): `H(z) = Â(z/0.65) / Â(z/0.75)` (S-008 §3.6, factors as published), a first-order
//! tilt compensation `1 − μ z⁻¹` in the form of S-008's reference code (`Spf`, "Section 3.8"):
//! k₁ = R(1)/R(0) of the **synthesized speech entering the postfilter** in the subframe,
//! smoothed as `p = 0.75 p + 0.25 k₁`, and `μ = 0.5 p` (the reference's `PreCoef` is −0.25·2;
//! the design tool's `TILT_FACTOR` 0.25 is the smoother's weight and half the coefficient), and
//! gain scaling to the subframe energy before the postfilter, smoothed per sample with a
//! one-pole smoother of 0.9 (S-008 §3.7; the reference code's 15/16, rounded). No pitch postfilter in this model version (A-041).
//! Design constants of the model, not device figures. (The first implementation took k₁ from
//! the filter's own response with the 0.25 factor; the US1 transparency check read 2 – 3 dB low
//! above 1.2 kHz, which this form recovers.)

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
    /// The smoothed first reflection coefficient of the input speech, Q15 (S-008 `Park`).
    park: i32,
    /// The smoothed gain, Q12.
    gain_q12: i32,
}

impl Default for Postfilter {
    fn default() -> Self {
        Self {
            num_mem: [0; LPC_ORDER],
            den_mem: [0; LPC_ORDER],
            tilt_mem: 0,
            park: 0,
            gain_q12: GAIN_ONE_Q12,
        }
    }
}

/// The tilt parameter's smoother: `p = 0.75 p + 0.25 k₁` (S-008 `Spf`; `TILT_FACTOR` is the 0.25).
const TILT_SMOOTH_Q15: i32 = 32768 - TILT_FACTOR;
/// The tilt coefficient on the smoothed k₁: 0.5 (S-008 `PreCoef = −0.25·2`).
const TILT_COEF_Q15: i32 = 2 * TILT_FACTOR;

/// k₁ = R(1)/R(0) of a subframe (Q15, −32767 … 32767): the first reflection coefficient of the
/// speech entering the postfilter (S-008 `Spf`); 0 for silence.
pub(crate) fn first_reflection(x: &[i32; SUBFRAME_SAMPLES]) -> i32 {
    let (mut r0, mut r1) = (0i64, 0i64);
    for (i, &v) in x.iter().enumerate() {
        r0 = mac(r0, v, v);
        if let Some(&next) = x.get(i.wrapping_add(1)) {
            r1 = mac(r1, v, next);
        }
    }
    if r0 == 0 {
        return 0;
    }
    sat16(div_q(r1, r0, 15)).clamp(-32767, 32767)
}

impl Postfilter {
    #[cfg(test)]
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
        // Tilt compensation (S-008 `Spf`): p = 0.75 p + 0.25 k₁(input); t[n] = y[n] − 0.5 p y[n−1].
        self.park = sat16(
            i64::from(mult(self.park, TILT_SMOOTH_Q15))
                .saturating_add(i64::from(mult(first_reflection(input), TILT_FACTOR))),
        );
        let mu = mult(self.park, TILT_COEF_Q15);
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
        // A period-4 pattern (+ + − −) with its last sample zeroed has R(1) = 0 exactly (the 58
        // lag-1 products alternate in sign): no tilt either, so the identity is exact.
        let x: [i32; 60] = core::array::from_fn(|n| match n % 4 {
            _ if n == 59 => 0,
            0 | 1 => 8000,
            _ => -8000,
        });
        assert_eq!(first_reflection(&x), 0);
        let mut y = [0i32; 60];
        pf.process(&FLAT, &x, &mut y);
        assert_eq!(y, x, "the flat predictor and a zero k₁: the identity");
        assert_eq!(pf.park, 0);
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

    /// The tilt term follows the input speech's first reflection coefficient (S-008 `Spf`): a
    /// low-pass (falling-spectrum) input gives k₁ > 0, a smoothed `park` that converges to it
    /// over a few subframes, and μ = 0.5·park > 0 so that `1 − μ z⁻¹` lifts the highs; an
    /// alternating input gives k₁ < 0 and the opposite tilt; silence gives 0.
    #[test]
    fn tilt_follows_the_inputs_first_reflection_coefficient() {
        let mut low = [0i32; 60];
        let mut v = 0i64;
        for (n, s) in low.iter_mut().enumerate() {
            v = (v * 9) / 10 + if n % 7 == 0 { 6000 } else { 0 };
            *s = v as i32;
        }
        let k1 = first_reflection(&low);
        assert!(k1 > 20_000, "k₁ {k1}");
        let high: [i32; 60] = core::array::from_fn(|n| if n % 2 == 0 { 5000 } else { -5000 });
        assert!(first_reflection(&high) < -30_000);
        assert_eq!(first_reflection(&[0; 60]), 0);
        let mut pf = Postfilter::default();
        let mut y = [0i32; 60];
        for _ in 0..40 {
            pf.process(&FLAT, &low, &mut y);
        }
        assert!((pf.park - k1).abs() < 400, "park {} vs k₁ {k1}", pf.park);
        let mu = mult(pf.park, TILT_COEF_Q15);
        assert!(mu > 10_000 && mu <= 16384, "μ {mu}");
        // The flat predictor leaves only the tilt: the output has more high-frequency energy
        // relative to its low-frequency energy than the input (a lifted spectrum).
        let ratio = |x: &[i32; 60]| {
            let diff: f64 = x.windows(2).map(|w| f64::from(w[1] - w[0]).powi(2)).sum();
            diff / energy(x)
        };
        assert!(ratio(&y) > ratio(&low), "{} vs {}", ratio(&y), ratio(&low));
        assert_eq!(TILT_SMOOTH_Q15, 24576);
        assert_eq!(TILT_COEF_Q15, 16384);
        assert_eq!(TILT_FACTOR, 8192);
        assert_eq!(POST_GAIN_SMOOTH, 29491);
    }
}
