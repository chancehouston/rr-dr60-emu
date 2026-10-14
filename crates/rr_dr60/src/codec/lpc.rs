//! Linear-prediction analysis of the speech encoder (spec 008 research R-02, R-04;
//! contracts/codec-design.md › Analysis): the 240-sample asymmetric window, the autocorrelation
//! with its white-noise correction, lag window and `r(0)` floor, and Levinson–Durbin with the
//! non-minimum-phase fallback. Also the two filter primitives every stage of the codec runs on.
//!
//! The predictor convention is G.729's (S-009 §3.2.2): `A(z) = 1 + Σ a_i z⁻ⁱ`, coefficients in
//! Q12 with `a[0] = 4096`; the residual is `x[n] + Σ a_i x[n−i]` and the synthesis
//! `u[n] − Σ a_i ŝ[n−i]`. Every constant is a design constant of the model (A-040, A-041),
//! none a device figure.

use super::fixed::{Q12_ONE, mac, mult, norm32, round_q12, round_shr, sat16, shift_signed};
use super::tables::{LAG_WINDOW, LPC_ORDER, LPC_WINDOW, WINDOW_SAMPLES, WN_CORR_EXCESS};

/// Predictor coefficients `a[0..=8]` in Q12, `a[0] = 4096` (S-009 §3.2.2).
pub(crate) type Lpc = [i32; LPC_ORDER + 1];

/// The flat predictor: `A(z) = 1`.
pub(crate) const FLAT: Lpc = [Q12_ONE as i32, 0, 0, 0, 0, 0, 0, 0, 0]; // A(z) = 1 in S-009 §3.2.2's format

/// A reflection coefficient of this magnitude or more (Q31 of 1 − 2⁻¹¹) marks a
/// non-minimum-phase solution: the previous coefficients are kept (contracts/codec-design.md
/// › Analysis; engineering target, 008 R-02: the reference code's 32750/32768 threshold in Q15).
const K_LIMIT_Q31: i64 = (1 << 31) - (1 << 20); // engineering target (008 R-02)

/// The windowed autocorrelation `r[0..=8]` of 240 Q15 samples (60 past, the frame, 60
/// look-ahead), normalized so that `r[0]` lies in 2³⁰ … 2³¹ (Q31 of the normalized energy),
/// with the white-noise correction (`r[0] × 1.0001`), the 60 Hz lag window on `r[1..=8]` and
/// `r[0]` floored at one Q15 unit before normalization, so all-zero input is defined (S-009
/// §3.2.1; 008 FR-001, FR-006; A-041).
pub(crate) fn autocorrelation(x: &[i32; WINDOW_SAMPLES]) -> [i64; LPC_ORDER + 1] {
    let mut w = [0i32; WINDOW_SAMPLES];
    for ((w, &x), &win) in w.iter_mut().zip(x).zip(LPC_WINDOW.iter()) {
        *w = mult(x, i32::from(win)); // the window's table (tools/celp-design)
    }
    let mut r = [0i64; LPC_ORDER + 1];
    for (k, rk) in r.iter_mut().enumerate() {
        let mut acc = 0i64;
        for (&a, &b) in w[k..].iter().zip(w.iter()) {
            acc = mac(acc, a, b);
        }
        *rk = acc;
    }
    // r(0) floor (FR-006: silence is defined), then normalization to Q31.
    if r[0] < 1 {
        r[0] = 1;
    }
    let shift = norm32(r[0]);
    for rk in &mut r {
        *rk = shift_signed(*rk, shift);
    }
    // White-noise correction: r(0) × (1 + WN_CORR_EXCESS / 2¹⁵) (S-009 §3.2.1).
    r[0] = r[0].saturating_add(round_shr(
        r[0].saturating_mul(i64::from(WN_CORR_EXCESS)),
        15,
    ));
    // Lag window on r(1..=8): a 60 Hz bandwidth expansion, conserved in hertz (FR-001).
    for (rk, &lag) in r[1..].iter_mut().zip(LAG_WINDOW.iter()) {
        *rk = round_shr(rk.saturating_mul(i64::from(lag)), 15);
    }
    r
}

/// Levinson–Durbin on a Q31 autocorrelation (S-009 §3.2.2): the Q12 predictor and `true`, or
/// `prev` and `false` when a reflection coefficient reaches [`K_LIMIT_Q31`] or the prediction
/// error stops decreasing (a non-minimum-phase result; contracts/codec-design.md › Analysis).
/// The recursion runs in Q24 coefficients and Q31 error with `i64` and `i128` intermediates,
/// so the only rounding is the final one to Q12.
pub(crate) fn levinson(r: &[i64; LPC_ORDER + 1], prev: &Lpc) -> (Lpc, bool) {
    // a in Q24 (|a| < 8 → < 2²⁷), err in Q31.
    let mut a = [0i64; LPC_ORDER + 1];
    let mut err = r[0];
    if err <= 0 {
        return (*prev, false);
    }
    for i in 1..=LPC_ORDER {
        // acc = r[i] + Σ_{j=1}^{i−1} a[j]·r[i−j] in Q55 (Q24 × Q31).
        let mut acc = r[i].wrapping_shl(24);
        for j in 1..i {
            acc = acc.saturating_add(a[j].saturating_mul(r[i.wrapping_sub(j)]));
        }
        // k = −acc / err in Q31: (acc << 7) / err, the shift in i128.
        let k = {
            let num = i128::from(acc).wrapping_shl(7).saturating_neg();
            let q = num.checked_div(i128::from(err)).unwrap_or(0);
            // range-proven: clamped to the i64 range before the cast.
            q.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
        };
        if k.abs() >= K_LIMIT_Q31 {
            return (*prev, false);
        }
        let mut next = a;
        next[i] = round_shr(k, 7); // Q31 → Q24
        for j in 1..i {
            // a[j] + k·a[i−j]: Q31 × Q24 → Q55 → Q24.
            next[j] = a[j].saturating_add(round_shr(k.saturating_mul(a[i.wrapping_sub(j)]), 31));
        }
        a = next;
        // err × (1 − k²).
        let k2 = round_shr(k.saturating_mul(k), 31);
        err = err.saturating_sub(round_shr(k2.saturating_mul(err), 31));
        if err <= 0 {
            return (*prev, false);
        }
    }
    let mut out = FLAT;
    for (o, &v) in out.iter_mut().zip(a.iter()).skip(1) {
        *o = sat16(round_shr(v, 12)); // Q24 → Q12, saturated at ±8 (as the reference code)
    }
    (out, true)
}

/// The predictor `a` with its coefficients weighted by `γⁱ` (the table `gamma_pow[i]`, Q15):
/// `A(z/γ)` for the weighting and postfilter forms (S-008 §2.8, §3.6).
pub(crate) fn weighted(a: &Lpc, gamma_pow: &[i16; LPC_ORDER + 1]) -> Lpc {
    let mut out = FLAT;
    for ((o, &ai), &g) in out.iter_mut().zip(a).zip(gamma_pow.iter()).skip(1) {
        *o = mult(ai, i32::from(g));
    }
    out
}

/// The moving-average form `y[n] = x[n] + Σ a_i x[n−i]` (the residual of `A(z)`), with `mem`
/// holding the last eight inputs, most recent first; the output is rounded from Q12 and
/// saturated to Q15. Allocation-free; `out.len() == x.len()`.
pub(crate) fn fir(a: &Lpc, x: &[i32], mem: &mut [i32; LPC_ORDER], out: &mut [i32]) {
    debug_assert_eq!(x.len(), out.len());
    for (n, (&xn, o)) in x.iter().zip(out.iter_mut()).enumerate() {
        let mut acc = i64::from(xn).wrapping_shl(12);
        for i in 1..=LPC_ORDER {
            let past = if n >= i {
                x[n.wrapping_sub(i)]
            } else {
                mem[i.wrapping_sub(n).wrapping_sub(1)]
            };
            acc = mac(acc, a[i], past);
        }
        *o = round_q12(acc);
    }
    push_memory(mem, x);
}

/// The all-pole form `y[n] = x[n] − Σ a_i y[n−i]` (the synthesis filter `1/A(z)`), with `mem`
/// holding the last eight outputs, most recent first; saturated to Q15. Allocation-free.
pub(crate) fn iir(a: &Lpc, x: &[i32], mem: &mut [i32; LPC_ORDER], out: &mut [i32]) {
    debug_assert_eq!(x.len(), out.len());
    for n in 0..x.len() {
        let mut acc = i64::from(x[n]).wrapping_shl(12);
        for i in 1..=LPC_ORDER {
            let past = if n >= i {
                out[n.wrapping_sub(i)]
            } else {
                mem[i.wrapping_sub(n).wrapping_sub(1)]
            };
            acc = acc.saturating_sub(i64::from(a[i]).saturating_mul(i64::from(past)));
        }
        out[n] = round_q12(acc);
    }
    push_memory(mem, out);
}

/// Shifts the last eight samples of `x` into `mem`, most recent first (fewer than eight
/// samples shift the older memory down).
pub(crate) fn push_memory(mem: &mut [i32; LPC_ORDER], x: &[i32]) {
    let n = x.len().min(LPC_ORDER);
    if n < LPC_ORDER {
        mem.copy_within(0..LPC_ORDER.wrapping_sub(n), n);
    }
    for (m, &v) in mem.iter_mut().zip(x.iter().rev()) {
        *m = v;
    }
}

/// `x` divided by a positive `den` in Q15 for the tests' ratios.
#[cfg(test)]
pub(crate) fn ratio_q15(num: i64, den: i64) -> i64 {
    super::fixed::div_q(num, den, 15)
}

/// 008 T013 (spec 008 FR-001, FR-006; research R-02, R-04; contracts/codec-design.md ›
/// Analysis): the window, the autocorrelation of a sinusoid, Levinson on a known AR(2), the
/// `r(0)` floor and the non-minimum-phase fallback.
#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, clippy::disallowed_methods)] // test arithmetic
mod tests {
    extern crate std;
    use super::*;
    use crate::codec::fixed::Q15_MAX;

    /// The autocorrelation ratios r[k]/r[0] in Q15.
    fn ratios(r: &[i64; 9]) -> [i64; 9] {
        let mut out = [0i64; 9];
        for (o, &v) in out.iter_mut().zip(r) {
            *o = ratio_q15(v, r[0]);
        }
        out
    }

    /// The window is the tool's table: the rising half-Hamming over 200 samples peaks at
    /// sample 199, the falling quarter-cosine ends near zero; windowing applies it sample by
    /// sample (a constant input reproduces the table at half scale).
    #[test]
    fn window_equals_the_tools_table() {
        assert_eq!(LPC_WINDOW.len(), WINDOW_SAMPLES);
        assert_eq!(LPC_WINDOW[199], Q15_MAX as i16);
        assert_eq!(LPC_WINDOW[200], Q15_MAX as i16);
        assert_eq!(LPC_WINDOW[0], 2621); // 0.08 in Q15
        assert_eq!(LPC_WINDOW[239], 971);
        assert!(LPC_WINDOW.windows(2).take(199).all(|w| w[1] >= w[0]));
        assert!(LPC_WINDOW.windows(2).skip(200).all(|w| w[1] <= w[0]));
        let x = [16384i32; WINDOW_SAMPLES];
        let mut w = [0i32; WINDOW_SAMPLES];
        for (i, v) in w.iter_mut().enumerate() {
            *v = mult(x[i], i32::from(LPC_WINDOW[i]));
        }
        assert_eq!(w[199], 16384);
        assert_eq!(w[0], mult(16384, 2621));
        // The autocorrelation of the constant is the window's own: r[0] > r[1] > … > r[8] > 0.
        let r = autocorrelation(&x);
        assert!((1i64 << 30..1i64 << 31).contains(&r[0]) || r[0] >= 1 << 30);
        assert!(r.windows(2).all(|p| p[0] > p[1] && p[1] > 0), "{r:?}");
    }

    /// A 500 Hz sinusoid at the 6 kHz device rate (12 samples per period): the normalized
    /// autocorrelation follows cos(πk/6) × the lag window within 0.03 (the analysis window
    /// distorts it slightly), so r[3] ≈ 0 and r[6] ≈ −0.88.
    #[test]
    fn autocorrelation_of_a_known_sinusoid() {
        let tone = crate::test_util::tone(500.0, 16384.0, 6000.0, WINDOW_SAMPLES);
        let mut x = [0i32; WINDOW_SAMPLES];
        for (xi, &t) in x.iter_mut().zip(&tone) {
            *xi = t.round() as i32;
        }
        let r = autocorrelation(&x);
        assert!(
            r[0] >= 1 << 30 && r[0] < (1 << 31) + (1 << 20),
            "r0 {}",
            r[0]
        );
        let q = ratios(&r);
        for (k, &v) in q.iter().enumerate().skip(1) {
            let want = (std::f64::consts::PI * k as f64 / 6.0).cos() * f64::from(LAG_WINDOW[k - 1])
                / 32768.0
                / 1.0001;
            let got = v as f64 / 32768.0;
            assert!(
                (got - want).abs() < 0.03,
                "r[{k}]/r[0] = {got}, want {want}"
            );
        }
    }

    /// Yule–Walker: the autocorrelation of an AR(2) process with a₁ = −1.2, a₂ = 0.6 (Q12:
    /// −4915, 2458) is r[k] = −a₁ r[k−1] − a₂ r[k−2]; Levinson recovers the two coefficients
    /// within one Q12 step and the rest within one step of zero.
    #[test]
    fn levinson_recovers_a_known_ar2() {
        let (a1, a2) = (-1.2f64, 0.6f64);
        let mut rf = [0f64; 9];
        rf[0] = 1.0;
        rf[1] = -a1 / (1.0 + a2);
        for k in 2..9 {
            rf[k] = -a1 * rf[k - 1] - a2 * rf[k - 2];
        }
        let mut r = [0i64; 9];
        for (ri, &v) in r.iter_mut().zip(&rf) {
            *ri = (v * (1u64 << 30) as f64).round() as i64;
        }
        let (a, ok) = levinson(&r, &FLAT);
        assert!(ok);
        assert_eq!(a[0], 4096);
        assert!((a[1] - (-4915)).abs() <= 1, "a1 = {}", a[1]);
        assert!((a[2] - 2458).abs() <= 1, "a2 = {}", a[2]);
        for (i, &ai) in a.iter().enumerate().skip(3) {
            assert!(ai.abs() <= 1, "a{i} = {ai}");
        }
    }

    /// FR-006: all-zero input floors r(0) at one unit, which normalizes to a flat spectrum, and
    /// Levinson gives the flat predictor without a fallback.
    #[test]
    fn r0_is_floored_on_silence() {
        let r = autocorrelation(&[0i32; WINDOW_SAMPLES]);
        assert!(r[0] >= 1 << 30, "{}", r[0]);
        assert!(r[1..].iter().all(|&v| v == 0), "{r:?}");
        let (a, ok) = levinson(&r, &[1, 2, 3, 4, 5, 6, 7, 8, 9]);
        assert!(ok);
        assert_eq!(a, FLAT);
    }

    /// A crafted autocorrelation with |r[1]| > r[0] has a first reflection coefficient beyond
    /// 1: the previous coefficients are kept; so does one whose error turns non-positive.
    #[test]
    fn non_minimum_phase_keeps_the_previous_coefficients() {
        let prev: Lpc = [4096, -1000, 500, -250, 125, -60, 30, -15, 7];
        let mut r = [0i64; 9];
        r[0] = 1 << 30;
        r[1] = (1 << 30) + 1000;
        assert_eq!(levinson(&r, &prev), (prev, false));
        // Exactly 1: still non-minimum phase (the limit is inclusive of 1 − 2⁻¹¹).
        r[1] = 1 << 30;
        assert_eq!(levinson(&r, &prev), (prev, false));
        // A zero energy.
        r = [0; 9];
        assert_eq!(levinson(&r, &prev), (prev, false));
        // And a healthy one changes them.
        r = [1 << 30, 1 << 29, 1 << 28, 0, 0, 0, 0, 0, 0];
        let (a, ok) = levinson(&r, &prev);
        assert!(ok);
        assert_ne!(a, prev);
    }

    /// The two filter forms are inverses: `fir(a)` then `iir(a)` returns the input, memories
    /// carried across block boundaries of any size, and `weighted` scales by γⁱ.
    #[test]
    fn fir_and_iir_invert_each_other_across_blocks() {
        let a: Lpc = [4096, -3000, 1800, -900, 400, -150, 60, -20, 5];
        let x: std::vec::Vec<i32> = (0..200).map(|n| ((n * 37) % 2001) - 1000).collect();
        let mut residual = std::vec![0i32; 200];
        let mut back = std::vec![0i32; 200];
        let (mut m1, mut m2) = ([0i32; 8], [0i32; 8]);
        let mut pos = 0;
        for block in [1usize, 3, 8, 13, 60, 115] {
            let end = (pos + block).min(200);
            fir(&a, &x[pos..end], &mut m1, &mut residual[pos..end]);
            iir(&a, &residual[pos..end], &mut m2, &mut back[pos..end]);
            pos = end;
        }
        assert_eq!(pos, 200);
        assert_eq!(back, x);
        assert_eq!(
            m1,
            [
                x[199], x[198], x[197], x[196], x[195], x[194], x[193], x[192]
            ]
        );
        let w = weighted(&a, &crate::codec::tables::GAMMA_W2_POW);
        assert_eq!(w[0], 4096);
        assert_eq!(w[1], mult(-3000, 16384));
        assert_eq!(w[8], mult(5, 128));
        // push_memory with fewer than eight samples shifts the older ones down.
        let mut m = [1, 2, 3, 4, 5, 6, 7, 8];
        push_memory(&mut m, &[10, 20, 30]);
        assert_eq!(m, [30, 20, 10, 1, 2, 3, 4, 5]);
        push_memory(&mut m, &[]);
        assert_eq!(m, [30, 20, 10, 1, 2, 3, 4, 5]);
    }
}
