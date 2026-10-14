//! The two gains of the speech codec (spec 008 research R-02, R-05; contracts/codec-design.md
//! › Quantizers and › Searches): the 3-bit adaptive gain table, the 3-bit fixed-gain correction
//! about a predicted level, the fourth-order moving-average predictor (G.729's coefficients,
//! S-009 §3.9, borrowed; its mean and clamp derived from the level anchor A-029 by
//! `tools/celp-design`), and the sequential quantization of both against the weighted error.
//!
//! Levels are in **dB of the pulse amplitude in Q15 units**, held as Q8 (1/256 dB); the
//! dB → linear conversion runs through `tables::DB_TO_LOG2` and `POW2_FRAC` only. Design
//! constants of the model (A-041), not device figures.

use super::fixed::{mac, round_shr, sat16};
use super::tables::{
    ADAPT_GAIN, DB_TO_LOG2, FIXED_GAIN_CLAMP_DB, FIXED_GAIN_CORR_DB, FIXED_GAIN_MEAN_DB, POW2_FRAC,
    PRED_COEF, SUBFRAME_SAMPLES,
};

/// The predictor's order (S-009 §3.9).
const ORDER: usize = 4;
/// Q8 dB: 1 dB.
const DB_Q8: i32 = 256; // the predictor's 1/256 dB format (A-041)
/// The correction index whose correction is 0 dB (the predicted level itself).
pub(crate) const NEUTRAL_CORRECTION: u8 = 3;

/// The adaptive-codebook gain of `index` (0 … 7), Q14.
pub(crate) fn adaptive_gain_q14(index: u8) -> i32 {
    i32::from(ADAPT_GAIN[usize::from(index & 7)])
}

/// The fixed-gain correction of `index` (0 … 7) in Q8 dB.
pub(crate) fn correction_q8(index: u8) -> i32 {
    i32::from(FIXED_GAIN_CORR_DB[usize::from(index & 7)]).wrapping_mul(DB_Q8)
}

/// A level in Q8 dB → the linear amplitude in Q15 units (0 … 32767), as
/// `2^(dB · log2(10) / 20)` with the 33-entry mantissa table and linear interpolation.
pub(crate) fn amplitude_from_db_q8(db_q8: i32) -> i32 {
    // log2 of the amplitude in Q23: Q8 dB × Q15 (log2(10)/20).
    let log2_q23 = i64::from(db_q8).saturating_mul(i64::from(DB_TO_LOG2));
    let int = log2_q23.wrapping_shr(23); // floor (arithmetic shift)
    let frac = log2_q23 & ((1i64 << 23) - 1); // 0 … 2²³ − 1
    let k = frac.wrapping_shr(18) as usize; // 0 … 31
    let r = frac & ((1i64 << 18) - 1);
    let (m0, m1) = (
        i64::from(POW2_FRAC[k]),
        i64::from(POW2_FRAC[k.wrapping_add(1)]),
    );
    let mantissa = m0.saturating_add(round_shr(m1.saturating_sub(m0).saturating_mul(r), 18)); // Q14
    // amplitude = mantissa · 2^int / 2^14.
    let shift = int.saturating_sub(14);
    let value = if shift >= 0 {
        if shift >= 20 {
            i64::from(i32::MAX)
        } else {
            // range-proven: mantissa < 2¹⁵ and shift < 20.
            mantissa.wrapping_shl(shift as u32)
        }
    } else {
        round_shr(mantissa, shift.unsigned_abs().min(63) as u32)
    };
    sat16(value).max(0)
}

/// The fixed-gain predictor: the mean level plus a clamped moving average of the past four
/// quantized corrections (S-009 §3.9's coefficients, borrowed; mean and clamp from the tool).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GainPredictor {
    /// The last four corrections in Q8 dB, most recent first.
    past: [i32; ORDER],
}

impl GainPredictor {
    #[cfg(test)]
    pub(crate) fn reset(&mut self) {
        self.past = [0; ORDER];
    }

    /// The predicted level in Q8 dB: `MEAN + clamp(Σ coef_i · past_i, ±CLAMP)`.
    pub(crate) fn predicted_q8(&self) -> i32 {
        let mut acc = 0i64;
        for (&c, &u) in PRED_COEF.iter().zip(&self.past) {
            acc = mac(acc, i32::from(c), u);
        }
        let ma = round_shr(acc, 15);
        let clamp = i64::from(FIXED_GAIN_CLAMP_DB).saturating_mul(i64::from(DB_Q8));
        // range-proven: the mean is 60 dB (15 360 in Q8) and the clamp ±22 dB.
        (i64::from(FIXED_GAIN_MEAN_DB)
            .saturating_mul(i64::from(DB_Q8))
            .saturating_add(ma.clamp(clamp.saturating_neg(), clamp))) as i32
    }

    /// Shifts the chosen correction (Q8 dB) in.
    pub(crate) fn update(&mut self, correction_q8: i32) {
        self.past.copy_within(0..ORDER.wrapping_sub(1), 1);
        self.past[0] = correction_q8;
    }
}

/// The correlations the gain quantizer minimizes over: the target `x` (Q15), the filtered
/// adaptive vector `y` (Q15) and the filtered codebook vector `z` (in the impulse response's
/// Q12 units).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Terms {
    xx: i64,
    xy: i64,
    xz: i64,
    yy: i64,
    zz: i64,
    yz: i64,
}

impl Terms {
    pub(crate) fn new(
        x: &[i32; SUBFRAME_SAMPLES],
        y: &[i32; SUBFRAME_SAMPLES],
        z: &[i32; SUBFRAME_SAMPLES],
    ) -> Self {
        let mut t = Self::default();
        for ((&xi, &yi), &zi) in x.iter().zip(y).zip(z) {
            t.xx = mac(t.xx, xi, xi);
            t.xy = mac(t.xy, xi, yi);
            t.xz = mac(t.xz, xi, zi);
            t.yy = mac(t.yy, yi, yi);
            t.zz = mac(t.zz, zi, zi);
            t.yz = mac(t.yz, yi, zi);
        }
        t
    }

    /// `2²⁸ · Σ (x − g_a y / 2¹⁴ − g_c z / 2¹²)²` in `i128`: exact, so a tie is a true tie.
    pub(crate) fn error(&self, g_a_q14: i64, g_c: i64) -> i128 {
        let (ga, gc) = (i128::from(g_a_q14), i128::from(g_c));
        let xx = i128::from(self.xx).wrapping_shl(28);
        let xy = i128::from(self.xy).saturating_mul(ga).wrapping_shl(15);
        let xz = i128::from(self.xz).saturating_mul(gc).wrapping_shl(17);
        let yy = i128::from(self.yy).saturating_mul(ga).saturating_mul(ga);
        let zz = i128::from(self.zz)
            .saturating_mul(gc)
            .saturating_mul(gc)
            .wrapping_shl(4);
        let yz = i128::from(self.yz)
            .saturating_mul(ga)
            .saturating_mul(gc)
            .wrapping_shl(3);
        xx.saturating_sub(xy)
            .saturating_sub(xz)
            .saturating_add(yy)
            .saturating_add(zz)
            .saturating_add(yz)
    }
}

/// The quantized gains of a subframe.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Gains {
    /// The adaptive-gain index (3 bits).
    pub(crate) adaptive_index: u8,
    /// The fixed-gain correction index (3 bits).
    pub(crate) fixed_index: u8,
    /// The adaptive gain, Q14.
    pub(crate) g_a_q14: i32,
    /// The fixed gain: the pulse amplitude in Q15 units.
    pub(crate) g_c: i32,
}

/// The gains for a given predicted level: the fixed gain of `fixed_index` about
/// `predicted_q8`, the adaptive gain of `adaptive_index`.
pub(crate) fn gains_for(predicted_q8: i32, adaptive_index: u8, fixed_index: u8) -> Gains {
    Gains {
        adaptive_index: adaptive_index & 7,
        fixed_index: fixed_index & 7,
        g_a_q14: adaptive_gain_q14(adaptive_index),
        g_c: amplitude_from_db_q8(predicted_q8.saturating_add(correction_q8(fixed_index))),
    }
}

/// Sequential quantization (contracts/codec-design.md › Searches): the adaptive index
/// minimizing the weighted error with the fixed gain at its predicted level (correction 0 dB),
/// then the fixed index minimizing it given that adaptive gain; ties to the lowest index.
pub(crate) fn quantize(terms: &Terms, predicted_q8: i32) -> Gains {
    let g_c_pred = i64::from(amplitude_from_db_q8(
        predicted_q8.saturating_add(correction_q8(NEUTRAL_CORRECTION)),
    ));
    let mut best_a = (i128::MAX, 0u8);
    for idx in 0..8u8 {
        let e = terms.error(i64::from(adaptive_gain_q14(idx)), g_c_pred);
        if e < best_a.0 {
            best_a = (e, idx);
        }
    }
    let g_a = i64::from(adaptive_gain_q14(best_a.1));
    let mut best_c = (i128::MAX, 0u8);
    for idx in 0..8u8 {
        let g_c = amplitude_from_db_q8(predicted_q8.saturating_add(correction_q8(idx)));
        let e = terms.error(g_a, i64::from(g_c));
        if e < best_c.0 {
            best_c = (e, idx);
        }
    }
    gains_for(predicted_q8, best_a.1, best_c.1)
}

/// 008 T013 (spec 008 research R-02; contracts/codec-design.md › Quantizers; A-029): the
/// tables, the dB conversion, the predictor's clamp, and the level anchor's two ends inside
/// the range.
#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, clippy::disallowed_methods)] // test arithmetic
mod tests {
    extern crate std;
    use super::*;

    #[test]
    fn tables_and_steps() {
        let want = [0.0, 0.2, 0.4, 0.6, 0.8, 1.0, 1.2, 1.4];
        for (i, w) in want.iter().enumerate() {
            assert_eq!(adaptive_gain_q14(i as u8), (w * 16384.0f64).round() as i32);
        }
        assert_eq!(adaptive_gain_q14(9), adaptive_gain_q14(1), "masked");
        for i in 0..8u8 {
            assert_eq!(correction_q8(i), (-12 + 4 * i32::from(i)) * 256);
        }
        assert_eq!(correction_q8(NEUTRAL_CORRECTION), 0);
        assert_eq!(FIXED_GAIN_MEAN_DB, 60);
        assert_eq!(FIXED_GAIN_CLAMP_DB, 22);
        assert_eq!(PRED_COEF, [22282, 19005, 11141, 6226]); // 0.68, 0.58, 0.34, 0.19 (S-009)
    }

    /// dB → amplitude within 0.5 % over the useful range; saturates at 32767; 0 dB is 1 unit.
    #[test]
    fn amplitude_from_db() {
        for db in (-20..=90).step_by(3) {
            let got = f64::from(amplitude_from_db_q8(db * 256));
            let want = 10f64.powf(f64::from(db) / 20.0);
            if want < 50.0 {
                assert!((got - want).abs() <= 1.0, "{db} dB: {got} vs {want}");
            } else {
                assert!((got / want - 1.0).abs() < 0.005, "{db} dB: {got} vs {want}");
            }
        }
        assert_eq!(amplitude_from_db_q8(0), 1);
        assert_eq!(amplitude_from_db_q8(60 * 256), 1000);
        assert_eq!(amplitude_from_db_q8(100 * 256), 32767, "saturates");
        assert_eq!(amplitude_from_db_q8(200 * 256), 32767);
        assert_eq!(amplitude_from_db_q8(-100 * 256), 0);
        // Half-dB steps are monotone.
        let mut last = -1;
        for q8 in (-30 * 256..=95 * 256).step_by(128) {
            let a = amplitude_from_db_q8(q8);
            assert!(a >= last, "{q8}: {a} < {last}");
            last = a;
        }
    }

    /// The predictor starts at the mean, follows the corrections with G.729's weights and
    /// clamps at ±22 dB.
    #[test]
    fn predictor_follows_and_clamps() {
        let mut p = GainPredictor::default();
        assert_eq!(p.predicted_q8(), 60 * 256);
        p.update(16 * 256);
        assert_eq!(
            p.predicted_q8(),
            60 * 256 + ((16 * 256 * 22282 + 16384) >> 15)
        );
        for _ in 0..8 {
            p.update(16 * 256);
        }
        assert_eq!(p.predicted_q8(), (60 + 22) * 256, "clamped at +22 dB");
        for _ in 0..8 {
            p.update(-12 * 256);
        }
        // −12 dB × (0.68 + 0.58 + 0.34 + 0.19) = −21.5 dB: inside the clamp.
        let ma = (-12i64 * 256 * (22282 + 19005 + 11141 + 6226) + 16384) >> 15;
        assert_eq!(p.predicted_q8(), 60 * 256 + ma as i32);
        assert!(ma > -22 * 256 && ma < -21 * 256, "{ma}");
        p.reset();
        assert_eq!(p, GainPredictor::default());
        // A long run of −12 dB corrections cannot pass the lower clamp either.
        let mut q = GainPredictor::default();
        for _ in 0..8 {
            q.update(-40 * 256);
        }
        assert_eq!(q.predicted_q8(), (60 - 22) * 256);
    }

    /// A-029 (the tool's anchor, `fixed_gain_design` in `tools/celp-design/design_celp.py`):
    /// a −10 dBFS noise-like frame (prediction gain ≈ 3 dB) is not gain-limited: with the
    /// predictor converged the chosen correction index is strictly inside 0 … 7 (the unit test
    /// contracts/codec-design.md asks for). The −40 dBFS voiced end (prediction gain ≈ 30 dB)
    /// is **reported**: it is reachable, but only with the lowest correction held, because the
    /// moving average sustains an offset of 1.79 × the correction, not the clamp the tool's
    /// margin arithmetic assumed; and the tool reads "−N dBFS" as an amplitude of
    /// 32768 · 10^(−N/20), 3 dB above the project's AES17 convention. Both are noted for the
    /// post-implementation hardware review (T053); the harness measures the −40 dBFS level.
    #[test]
    fn level_anchor_ends_are_not_gain_limited() {
        let pulse_factor = 20.0 * (60.0f64 / 4.0).sqrt().log10(); // ≈ 11.76 dB
        let required = |level_dbfs: f64, prediction_gain_db: f64| -> f64 {
            // AES17: a full-scale sine is 0 dBFS, RMS 1/√2.
            let rms = 32768.0 * 10f64.powf(level_dbfs / 20.0) / 2f64.sqrt();
            20.0 * rms.log10() - prediction_gain_db + pulse_factor
        };
        let converge = |need: f64| -> (u8, f64) {
            let mut p = GainPredictor::default();
            let mut idx = 0u8;
            for _ in 0..12 {
                let pred = f64::from(p.predicted_q8()) / 256.0;
                idx = ((need - pred + 12.0) / 4.0).round().clamp(0.0, 7.0) as u8;
                p.update(correction_q8(idx));
            }
            (idx, f64::from(p.predicted_q8()) / 256.0)
        };
        let (lo, hi) = (60.0 - 22.0 - 12.0, 60.0 + 22.0 + 16.0);
        // The top end: −10 dBFS noise, strictly inside with margin.
        let need = required(-10.0, 3.0);
        assert!(hi - need >= 6.0, "−10 dBFS needs {need} dB of {hi}");
        let (idx, _) = converge(need);
        assert!((1..=6).contains(&idx), "−10 dBFS: index {idx}");
        // The bottom end: −40 dBFS strongly voiced, reachable, index 0 held (reported).
        let need = required(-40.0, 30.0);
        assert!(need >= lo, "−40 dBFS needs {need} dB, below {lo}");
        let (idx, pred) = converge(need);
        assert!(idx <= 1, "−40 dBFS: index {idx}");
        assert!(
            pred - 12.0 <= need + 2.0,
            "the lowest reachable level {} vs {need}",
            pred - 12.0
        );
        std::println!(
            "gain range: −10 dBFS noise needs {:.1} dB (top {hi}); −40 dBFS voiced needs {need:.1} dB (bottom {lo}; converged index {idx})",
            required(-10.0, 3.0)
        );
    }

    /// The error expansion equals the direct sum of squares (scaled by 2²⁸), and the sequential
    /// quantizer picks the gains that reproduce a known excitation.
    #[test]
    fn error_terms_and_sequential_quantization() {
        let mut y = [0i32; 60];
        let mut z = [0i32; 60];
        for n in 0..60 {
            y[n] = ((n as i32 * 2731) % 8001) - 4000;
            z[n] = if n % 7 == 0 { 4096 } else { 0 } - if n % 11 == 0 { 2048 } else { 0 };
        }
        // x = 0.8 y + g_c z / 4096 with g_c = 1000 (60 dB: the predictor's mean, correction 0).
        let (ga, gc) = (13107i64, 1000i64);
        let mut x = [0i32; 60];
        for n in 0..60 {
            x[n] = ((ga * i64::from(y[n]) + 8192) >> 14) as i32
                + ((gc * i64::from(z[n])) >> 12) as i32;
        }
        let t = Terms::new(&x, &y, &z);
        // Direct evaluation at a few gain pairs.
        for (a, c) in [(ga, gc), (0, 0), (16384, 500), (22938, 32767)] {
            let direct: i128 = (0..60)
                .map(|n| {
                    let e = (i128::from(x[n]) << 28)
                        - ((i128::from(y[n]) * i128::from(a)) << 14)
                        - ((i128::from(z[n]) * i128::from(c)) << 16);
                    // e is 2²⁸ x − 2¹⁴ g_a y − 2¹⁶ g_c z = 2¹⁴ (2¹⁴ x − g_a y − 4 g_c z); squared → 2²⁸ (…)²
                    (e >> 14) * (e >> 14)
                })
                .sum();
            assert_eq!(t.error(a, c), direct, "gains {a} {c}");
        }
        let g = quantize(&t, 60 * 256);
        assert_eq!(
            (g.adaptive_index, g.fixed_index),
            (4, NEUTRAL_CORRECTION),
            "{g:?}"
        );
        assert_eq!(g.g_a_q14, 13107);
        assert_eq!(g.g_c, 1000);
        // A target of zeros: both indices 0 (the error is minimized by zero gains; ties → 0).
        let t0 = Terms::new(&[0; 60], &y, &z);
        let g0 = quantize(&t0, 60 * 256);
        assert_eq!((g0.adaptive_index, g0.fixed_index), (0, 0));
        assert_eq!(
            gains_for(60 * 256, 9, 11),
            gains_for(60 * 256, 1, 3),
            "indices masked"
        );
    }
}
