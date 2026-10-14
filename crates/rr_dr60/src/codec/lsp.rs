//! Line-spectral pairs of the speech codec (spec 008 research R-02; contracts/codec-design.md
//! › Analysis and › Quantizers): LPC → LSP by the Chebyshev-polynomial root search (S-009
//! §3.2.3), LSP → LPC (S-009 §3.2.6), the companded predictive scalar quantizer with its
//! ordering and minimum spacing, and the subframe interpolation.
//!
//! An LSP is an **angle** ω in 0 … π, stored as Q15 of ω/π (0 … 32768), the unit of the
//! quantizer tables `tables::LSP_MEAN`, `LSP_STEP` and `LSP_MIN_GAP`; the root search works on
//! cos ω in Q15 and the two forms convert through `tables::COS_TABLE`. Every table is a design
//! constant (A-041; `tools/celp-design`), none a device figure.

use super::fixed::{Q12_ONE, div_q, round_shr, sat16};
use super::lpc::{FLAT, Lpc};
use super::tables::{
    CHEB_GRID, COS_TABLE, LPC_ORDER, LSP_BITS, LSP_MEAN, LSP_MIN_GAP, LSP_PRED, LSP_STEP,
};

/// Eight LSP angles, Q15 of ω/π, ascending.
pub(crate) type Lsp = [i32; LPC_ORDER];

/// Half the order: the number of roots of each of the two polynomials.
const NC: usize = LPC_ORDER / 2;
/// Bisection steps inside the grid cell that holds a sign change (S-009 §3.2.3).
const BISECTIONS: usize = 4;
/// π as a Q15 angle.
const PI_Q15: i32 = 32768; // the angle format of the LSP tables (A-041)
/// The angle table's step: 256 entries over 0 … π, 128 Q15 units each.
const COS_STEP_BITS: u32 = 7;

/// The mean vector as an `Lsp` (the reset state of the predictors, FR-018).
pub(crate) fn mean() -> Lsp {
    let mut m = [0i32; LPC_ORDER];
    for (o, &v) in m.iter_mut().zip(LSP_MEAN.iter()) {
        *o = i32::from(v);
    }
    m
}

/// cos of a Q15 angle (0 … 32768 ↦ 1 … −1), by the 257-entry table with linear
/// interpolation.
pub(crate) fn cos_q15(angle: i32) -> i32 {
    let angle = angle.clamp(0, PI_Q15);
    let k = (angle.wrapping_shr(COS_STEP_BITS)) as usize;
    if k >= COS_TABLE.len().wrapping_sub(1) {
        return i32::from(COS_TABLE[COS_TABLE.len().wrapping_sub(1)]);
    }
    let frac = i64::from(angle) & 0x7f;
    let (c0, c1) = (
        i64::from(COS_TABLE[k]),
        i64::from(COS_TABLE[k.wrapping_add(1)]),
    );
    // range-proven: |c1 − c0| < 2¹⁶ and frac < 2⁷.
    sat16(c0.saturating_add(round_shr(
        c1.saturating_sub(c0).saturating_mul(frac),
        COS_STEP_BITS,
    )))
}

/// The Q15 angle whose cosine is `c` (Q15), by binary search in the table and linear
/// interpolation: the inverse of [`cos_q15`] within one unit.
pub(crate) fn acos_q15(c: i32) -> i32 {
    let c = c.clamp(-32768, 32767);
    // The table decreases: find the last k (0 … 255) with COS_TABLE[k] ≥ c.
    let (mut lo, mut hi) = (0usize, COS_TABLE.len().wrapping_sub(2));
    while lo < hi {
        let mid = lo.wrapping_add(hi).wrapping_add(1).wrapping_shr(1);
        if i32::from(COS_TABLE[mid]) >= c {
            lo = mid;
        } else {
            hi = mid.wrapping_sub(1);
        }
    }
    let k = lo;
    let (c0, c1) = (
        i64::from(COS_TABLE[k]),
        i64::from(COS_TABLE[k.wrapping_add(1)]),
    );
    let span = c0.saturating_sub(c1).max(1);
    let frac = div_q(c0.saturating_sub(i64::from(c)), span, COS_STEP_BITS).clamp(0, 128);
    // range-proven: k ≤ 255, so k << 7 ≤ 32640, plus frac ≤ 128.
    sat32((k as i64).wrapping_shl(COS_STEP_BITS).saturating_add(frac))
}

/// Saturates to the `i32` range.
fn sat32(x: i64) -> i32 {
    super::fixed::sat32(x)
}

/// The Chebyshev form of the symmetric and antisymmetric polynomials of `a` (S-009 §3.2.3,
/// eq. 20–23): `f1[i] = a[i] + a[9−i] − f1[i−1]`, `f2[i] = a[i] − a[9−i] + f2[i−1]`, i = 1 … 4,
/// in Q12 held in `i64`.
fn sum_difference_polynomials(a: &Lpc) -> ([i64; NC + 1], [i64; NC + 1]) {
    let mut f1 = [0i64; NC + 1];
    let mut f2 = [0i64; NC + 1];
    f1[0] = Q12_ONE;
    f2[0] = Q12_ONE;
    for i in 1..=NC {
        let lo = i64::from(a[i]);
        let hi = i64::from(a[LPC_ORDER.wrapping_add(1).wrapping_sub(i)]);
        f1[i] = lo.saturating_add(hi).saturating_sub(f1[i.wrapping_sub(1)]);
        f2[i] = lo.saturating_sub(hi).saturating_add(f2[i.wrapping_sub(1)]);
    }
    (f1, f2)
}

/// `C(x) = T₄(x) + f[1] T₃(x) + f[2] T₂(x) + f[3] T₁(x) + f[4]/2` by the Clenshaw recurrence
/// (S-009 §3.2.3, eq. 25): x in Q15, f in Q12, the result in Q12 (sign and ratios only
/// matter).
fn chebyshev(f: &[i64; NC + 1], x: i32) -> i64 {
    // 2x·b in Q12: (x · b) >> 14.
    let two_x = |b: i64| round_shr(i64::from(x).saturating_mul(b), 14);
    let mut b2 = 0i64;
    let mut b1 = f[0]; // c₄ = 1 (T₄'s coefficient; f[0] is 1.0 in Q12)
    for &fk in &f[1..NC] {
        // b_k = c_k + 2x b_{k+1} − b_{k+2}
        let b0 = fk.saturating_add(two_x(b1)).saturating_sub(b2);
        b2 = b1;
        b1 = b0;
    }
    // S = c₀ + x b₁ − b₂ with c₀ = f[4] / 2.
    round_shr(f[NC], 1)
        .saturating_add(round_shr(i64::from(x).saturating_mul(b1), 15))
        .saturating_sub(b2)
}

/// LPC → LSP angles by the root search on the 61-point cosine grid with four bisections and a
/// linear interpolation per root, alternating between the two polynomials (S-009 §3.2.3).
/// Fewer than eight roots (a non-minimum-phase or degenerate predictor) keeps `prev` and
/// returns `false` (contracts/codec-design.md › Analysis).
pub(crate) fn lpc_to_lsp(a: &Lpc, prev: &Lsp) -> (Lsp, bool) {
    let (f1, f2) = sum_difference_polynomials(a);
    let polys = [f1, f2];
    let mut roots = [0i32; LPC_ORDER];
    let mut found = 0usize;
    let mut poly = 0usize;
    let mut xlow = i32::from(CHEB_GRID[0]);
    let mut ylow = chebyshev(&polys[poly], xlow);
    let mut j = 0usize;
    let mut hold = false;
    while found < LPC_ORDER && j < CHEB_GRID.len().wrapping_sub(1) {
        if hold {
            hold = false;
        } else {
            j = j.wrapping_add(1);
        }
        let mut xhigh = i32::from(CHEB_GRID[j]);
        let mut yhigh = chebyshev(&polys[poly], xhigh);
        if ylow.signum().saturating_mul(yhigh.signum()) > 0 {
            xlow = xhigh;
            ylow = yhigh;
            continue;
        }
        // A sign change (or a zero) in [xhigh, xlow]: bisect, then interpolate.
        let (mut xl, mut yl) = (xlow, ylow);
        for _ in 0..BISECTIONS {
            // range-proven: both within ±2¹⁵.
            let xmid = sat16(round_shr(i64::from(xl).saturating_add(i64::from(xhigh)), 1));
            let ymid = chebyshev(&polys[poly], xmid);
            if yl.signum().saturating_mul(ymid.signum()) > 0 {
                xl = xmid;
                yl = ymid;
            } else {
                xhigh = xmid;
                yhigh = ymid;
            }
        }
        let denom = yl.saturating_sub(yhigh);
        let root = if denom == 0 {
            xl
        } else {
            // xl + (xhigh − xl) · yl / (yl − yhigh)
            let step = div_q(
                i64::from(xhigh)
                    .saturating_sub(i64::from(xl))
                    .saturating_mul(yl),
                denom.abs(),
                0,
            );
            let step = if denom < 0 {
                step.saturating_neg()
            } else {
                step
            };
            sat16(i64::from(xl).saturating_add(step))
        };
        roots[found] = root;
        found = found.wrapping_add(1);
        // Continue from the root's lower bracket with the other polynomial, on this grid cell.
        poly ^= 1;
        xlow = xhigh;
        ylow = chebyshev(&polys[poly], xlow);
        hold = true;
    }
    if found < LPC_ORDER {
        return (*prev, false);
    }
    let mut lsp = [0i32; LPC_ORDER];
    for (o, &r) in lsp.iter_mut().zip(roots.iter()) {
        *o = acos_q15(r);
    }
    (order_and_space(lsp), true)
}

/// LSP angles → the Q12 predictor (S-009 §3.2.6): the odd LSPs build `f1(z)`, the even ones
/// `f2(z)`, `a_i = ½(f1'(i) + f2'(i))` with `f1' = f1 (1 + z⁻¹)`, `f2' = f2 (1 − z⁻¹)`, in Q24
/// `i64` with one rounding to Q12 (saturated at ±8).
pub(crate) fn lsp_to_lpc(lsp: &Lsp) -> Lpc {
    let mut f1 = [0i64; NC + 1];
    let mut f2 = [0i64; NC + 1];
    f1[0] = 1 << 24;
    f2[0] = 1 << 24;
    for i in 0..NC {
        let q_odd = i64::from(cos_q15(lsp[i.wrapping_mul(2)]));
        let q_even = i64::from(cos_q15(lsp[i.wrapping_mul(2).wrapping_add(1)]));
        multiply_by_section(&mut f1, q_odd);
        multiply_by_section(&mut f2, q_even);
    }
    // f1'(k) = f1(k) + f1(k−1), f2'(k) = f2(k) − f2(k−1), k = 1 … 4.
    let mut a = FLAT;
    for k in 1..=NC {
        let f1p = f1[k].saturating_add(f1[k.wrapping_sub(1)]);
        let f2p = f2[k].saturating_sub(f2[k.wrapping_sub(1)]);
        a[k] = sat16(round_shr(f1p.saturating_add(f2p), 13)); // ½ and Q24 → Q12
        let mirror = LPC_ORDER.wrapping_add(1).wrapping_sub(k);
        a[mirror] = sat16(round_shr(f1p.saturating_sub(f2p), 13));
    }
    a
}

/// `f ← f · (1 − 2q z⁻¹ + z⁻²)` on the kept coefficients `f[0..=4]` (Q24 held in `i64`, q in
/// Q15); the full product has degree 8 and is symmetric, so its first half determines it, and a
/// coefficient depends only on lower-indexed ones, which is why the update runs downward.
fn multiply_by_section(f: &mut [i64; NC + 1], q: i64) {
    for k in (1..=NC).rev() {
        let prev1 = f[k.wrapping_sub(1)];
        let prev2 = if k >= 2 { f[k.wrapping_sub(2)] } else { 0 };
        // 2q · f[k−1]: Q15 × Q24 → >> 14.
        f[k] = f[k]
            .saturating_sub(round_shr(q.saturating_mul(prev1), 14))
            .saturating_add(prev2);
    }
}

/// The ordered, spaced form every decoded LSP vector takes (contracts/codec-design.md ›
/// Quantizers): sorted ascending, then at least `LSP_MIN_GAP` apart and inside
/// (gap, π − gap). Nine gaps fit in π with room, so the result is always valid.
pub(crate) fn order_and_space(mut lsp: Lsp) -> Lsp {
    // Insertion sort (eight elements, deterministic).
    for i in 1..LPC_ORDER {
        let v = lsp[i];
        let mut j = i;
        while j > 0 && lsp[j.wrapping_sub(1)] > v {
            lsp[j] = lsp[j.wrapping_sub(1)];
            j = j.wrapping_sub(1);
        }
        lsp[j] = v;
    }
    lsp[0] = lsp[0].max(LSP_MIN_GAP);
    for i in 1..LPC_ORDER {
        lsp[i] = lsp[i].max(add32(lsp[i.wrapping_sub(1)], LSP_MIN_GAP));
    }
    let top = PI_Q15.saturating_sub(LSP_MIN_GAP);
    lsp[LPC_ORDER.wrapping_sub(1)] = lsp[LPC_ORDER.wrapping_sub(1)].min(top);
    for i in (0..LPC_ORDER.wrapping_sub(1)).rev() {
        lsp[i] = lsp[i].min(lsp[i.wrapping_add(1)].saturating_sub(LSP_MIN_GAP));
    }
    lsp
}

fn add32(a: i32, b: i32) -> i32 {
    a.saturating_add(b)
}

/// The predicted angle of LSP `i` from the previous frame's decoded vector:
/// `m_i + 0.6 (q̂_i − m_i)` (contracts/codec-design.md › Quantizers).
fn predict(i: usize, prev_q: &Lsp) -> i32 {
    let m = i32::from(LSP_MEAN[i]);
    add32(m, mult_q15_wide(LSP_PRED, prev_q[i].saturating_sub(m)))
}

/// Q15 × angle → angle (an angle can exceed the 16-bit range in intermediate sums).
fn mult_q15_wide(k: i32, v: i32) -> i32 {
    sat32(round_shr(i64::from(k).saturating_mul(i64::from(v)), 15))
}

/// The companded level of `code` (an unsigned `bits`-bit two's-complement index j) at `step`:
/// `sign(j) · step · (2^|j| − 1)` (contracts/codec-design.md › Quantizers; the table of
/// `tools/celp-design`).
pub(crate) fn level(code: u8, bits: u8, step: i32) -> i32 {
    let half = 1u8.wrapping_shl(u32::from(bits).wrapping_sub(1));
    // Sign-extend: codes ≥ 2^(bits−1) are negative.
    let j: i32 = if code >= half {
        i32::from(code).saturating_sub(i32::from(half).wrapping_shl(1))
    } else {
        i32::from(code)
    };
    let magnitude = 1i64
        .wrapping_shl(j.unsigned_abs().min(30))
        .saturating_sub(1)
        .saturating_mul(i64::from(step));
    let signed = if j < 0 {
        magnitude.saturating_neg()
    } else {
        magnitude
    };
    sat32(signed)
}

/// Quantizes the eight LSP angles against the previous frame's decoded vector: the indices
/// (each the nearest companded level to its predicted residual, ties to the lowest code) and the
/// decoded, ordered, spaced vector the decoder will hold.
pub(crate) fn quantize(lsp: &Lsp, prev_q: &Lsp) -> ([u8; LPC_ORDER], Lsp) {
    let mut idx = [0u8; LPC_ORDER];
    for i in 0..LPC_ORDER {
        let d = lsp[i].saturating_sub(predict(i, prev_q));
        let bits = LSP_BITS[i];
        let step = i32::from(LSP_STEP[i]);
        let mut best = (i32::MAX, 0u8);
        for code in 0..1u8.wrapping_shl(u32::from(bits)) {
            let err = d.saturating_sub(level(code, bits, step)).abs();
            if err < best.0 {
                best = (err, code);
            }
        }
        idx[i] = best.1;
    }
    (idx, decode(&idx, prev_q))
}

/// Decodes eight indices against the previous decoded vector: prediction plus level, then
/// ordering and spacing (every index combination gives a valid vector, FR-010).
pub(crate) fn decode(idx: &[u8; LPC_ORDER], prev_q: &Lsp) -> Lsp {
    let mut q = [0i32; LPC_ORDER];
    for i in 0..LPC_ORDER {
        let code = idx[i] & (1u8.wrapping_shl(u32::from(LSP_BITS[i])).wrapping_sub(1));
        q[i] = add32(
            predict(i, prev_q),
            level(code, LSP_BITS[i], i32::from(LSP_STEP[i])),
        );
    }
    order_and_space(q)
}

/// The subframe-1 vector: the mean of the previous and current decoded vectors (S-009
/// §3.2.5), which stays ordered and spaced.
pub(crate) fn interpolate(prev_q: &Lsp, cur_q: &Lsp) -> Lsp {
    let mut out = [0i32; LPC_ORDER];
    for ((o, &p), &c) in out.iter_mut().zip(prev_q).zip(cur_q) {
        *o = sat32(round_shr(i64::from(p).saturating_add(i64::from(c)), 1));
    }
    out
}

/// 008 T013 (spec 008 FR-010; research R-02; contracts/codec-design.md › Quantizers): the
/// round trip, the two fallbacks, every index decoding to a valid vector, the companded tables.
#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, clippy::disallowed_methods)] // test arithmetic
mod tests {
    extern crate std;
    use super::*;
    use crate::codec::tables::LSP_MIN_GAP;
    use std::f64::consts::PI;
    use std::vec::Vec;

    /// The predictor of a cascade of two-pole resonators (centre Hz, radius) at 6 kHz, Q12.
    fn resonators(poles: &[(f64, f64)]) -> Lpc {
        let mut p = std::vec![1.0f64];
        for &(fc, r) in poles {
            let c1 = -2.0 * r * (2.0 * PI * fc / 6000.0).cos();
            let c2 = r * r;
            let mut next = std::vec![0.0; p.len() + 2];
            for (i, &v) in p.iter().enumerate() {
                next[i] += v;
                next[i + 1] += c1 * v;
                next[i + 2] += c2 * v;
            }
            p = next;
        }
        let mut a = FLAT;
        for (i, v) in p.iter().enumerate().skip(1) {
            a[i] = (v * 4096.0).round() as i32;
        }
        a
    }

    fn is_ordered_and_spaced(l: &Lsp) -> bool {
        l[0] >= LSP_MIN_GAP
            && l[7] <= 32768 - LSP_MIN_GAP
            && l.windows(2).all(|w| w[1] - w[0] >= LSP_MIN_GAP)
    }

    #[test]
    fn cos_and_acos_are_inverses_within_one_unit() {
        assert_eq!(cos_q15(0), 32767);
        assert_eq!(cos_q15(32768), -32768);
        assert_eq!(cos_q15(16384), 0);
        assert_eq!(acos_q15(32767), 0);
        assert_eq!(acos_q15(-32768), 32768);
        assert!((acos_q15(0) - 16384).abs() <= 1);
        for angle in (0..=32768).step_by(7) {
            let c = cos_q15(angle);
            let want = (PI * angle as f64 / 32768.0).cos() * 32768.0;
            assert!(
                (c as f64 - want).abs() <= 2.0,
                "cos({angle}) = {c}, want {want}"
            );
            let back = acos_q15(c);
            // Near the ends the cosine is flat, so the angle resolution drops.
            let tol = if !(1500..=31000).contains(&angle) {
                400
            } else {
                8
            };
            assert!((back - angle).abs() <= tol, "acos(cos({angle})) = {back}");
        }
        for c in (-32768..=32767).step_by(13) {
            let a = acos_q15(c);
            assert!((0..=32768).contains(&a));
            assert!(
                (cos_q15(a) - c).abs() <= 4,
                "cos(acos({c})) = {}",
                cos_q15(a)
            );
        }
    }

    /// LPC → LSP → LPC on three resonator cascades: every coefficient within four Q12 steps
    /// (the cosine table's interpolation is good to about two Q15 units each way), the LSPs
    /// ordered and spaced, and the roots found without a fallback.
    #[test]
    fn lpc_lsp_round_trip_on_three_known_filters() {
        let filters = [
            resonators(&[(400.0, 0.9), (1200.0, 0.92), (2100.0, 0.9), (2700.0, 0.8)]),
            resonators(&[(250.0, 0.95), (900.0, 0.9), (1800.0, 0.85), (2500.0, 0.7)]),
            resonators(&[(600.0, 0.8), (1500.0, 0.95), (2400.0, 0.9), (2900.0, 0.6)]),
        ];
        for (n, a) in filters.iter().enumerate() {
            let (lsp, ok) = lpc_to_lsp(a, &mean());
            assert!(ok, "filter {n}: fewer than 8 roots");
            assert!(is_ordered_and_spaced(&lsp), "filter {n}: {lsp:?}");
            let back = lsp_to_lpc(&lsp);
            assert_eq!(back[0], 4096);
            for i in 1..=8 {
                assert!(
                    (back[i] - a[i]).abs() <= 4,
                    "filter {n}, a{i}: {} vs {}",
                    back[i],
                    a[i]
                );
            }
        }
        // The flat predictor's LSPs are equally spaced: i·π/9.
        let (flat, ok) = lpc_to_lsp(&FLAT, &mean());
        assert!(ok);
        for (i, &v) in flat.iter().enumerate() {
            assert!((v - i32::from(LSP_MEAN[i])).abs() <= 24, "{i}: {v}");
        }
        assert_eq!(
            lsp_to_lpc(&mean())[1..].iter().map(|v| v.abs()).max(),
            Some(1)
        );
    }

    /// A non-minimum-phase predictor (a real root at z = 2.5) has fewer than eight interlaced
    /// roots on the unit circle: the previous LSPs are kept.
    #[test]
    fn fewer_than_eight_roots_keeps_the_previous_lsps() {
        let prev: Lsp = [1000, 3000, 6000, 9000, 14000, 19000, 25000, 30000];
        let mut a = FLAT;
        a[1] = -10240; // −2.5 in Q12
        let (lsp, ok) = lpc_to_lsp(&a, &prev);
        assert!(!ok, "{lsp:?}");
        assert_eq!(lsp, prev);
        // Saturated coefficients (a resonance pushed past ±8) also fall back.
        let a: Lpc = [4096, 32767, 32767, -32768, 32767, 0, 0, 0, 0];
        assert_eq!(lpc_to_lsp(&a, &prev), (prev, false));
    }

    /// FR-010: for every index of every field, with the other seven indices at their extremes
    /// (code 0, the most positive and the most negative level) and the predictor at the mean
    /// and at its own extremes, the decoded vector is ordered, spaced and inside the range.
    #[test]
    fn every_index_decodes_to_an_ordered_spaced_vector() {
        let extremes: [fn(u8) -> u8; 3] = [|_| 0, |b| (1 << (b - 1)) - 1, |b| 1 << (b - 1)];
        let prevs = [
            mean(),
            [LSP_MIN_GAP; 8],
            [32768 - LSP_MIN_GAP; 8],
            order_and_space([32000, 100, 30000, 200, 500, 20000, 10000, 1]),
        ];
        let mut count = 0;
        for prev in &prevs {
            for i in 0..8 {
                for code in 0..(1u8 << LSP_BITS[i]) {
                    for ex in extremes {
                        let mut idx = [0u8; 8];
                        for (j, v) in idx.iter_mut().enumerate() {
                            *v = ex(LSP_BITS[j]);
                        }
                        idx[i] = code;
                        let q = decode(&idx, prev);
                        assert!(is_ordered_and_spaced(&q), "{idx:?} {prev:?} → {q:?}");
                        let a = lsp_to_lpc(&q);
                        assert_eq!(a[0], 4096);
                        count += 1;
                    }
                }
            }
        }
        assert_eq!(count, 4 * (6 * 8 + 2 * 4) * 3);
        // An index above the field's width is masked, never a panic.
        let q = decode(&[255u8; 8], &mean());
        assert!(is_ordered_and_spaced(&q));
        assert_eq!(q, decode(&[7, 7, 7, 7, 7, 7, 3, 3], &mean()));
    }

    /// The companded levels equal the tool's: 3 bits → 0, s, 3s, 7s, −15s, −7s, −3s, −s for
    /// codes 0 … 7; 2 bits → 0, s, −3s, −s.
    #[test]
    fn companded_levels_equal_the_tools_table() {
        let s3 = i32::from(LSP_STEP[0]);
        let want3 = [0, s3, 3 * s3, 7 * s3, -15 * s3, -7 * s3, -3 * s3, -s3];
        for (code, &w) in want3.iter().enumerate() {
            assert_eq!(level(code as u8, 3, s3), w, "code {code}");
        }
        let s2 = i32::from(LSP_STEP[7]);
        let want2 = [0, s2, -3 * s2, -s2];
        for (code, &w) in want2.iter().enumerate() {
            assert_eq!(level(code as u8, 2, s2), w, "code {code}");
        }
        assert_eq!(LSP_BITS, [3, 3, 3, 3, 3, 3, 2, 2]);
        assert_eq!(LSP_STEP, [416, 416, 416, 416, 416, 416, 971, 971]);
        assert_eq!(LSP_PRED, 19661);
        assert_eq!(LSP_MIN_GAP, 417);
        // Quantizing the mean vector against itself picks code 0 everywhere (ties to the
        // lowest code: residual 0 sits exactly on level 0).
        let (idx, q) = quantize(&mean(), &mean());
        assert_eq!(idx, [0; 8]);
        assert_eq!(q, mean());
        // A jump of one slot (π/9, a full-range formant move) on every LSP takes the negative
        // levels in the first frame and converges within one small step in two.
        let mut target = mean();
        for v in &mut target {
            *v -= 3000;
        }
        let (idx, q1) = quantize(&target, &mean());
        assert!(idx.iter().take(6).all(|&i| i >= 4), "{idx:?}"); // the negative levels
        let (_, q2) = quantize(&target, &q1);
        for (i, (a, b)) in q2.iter().zip(&target).enumerate() {
            let step = i32::from(LSP_STEP[i]);
            assert!(
                (a - b).abs() <= step,
                "after two frames, LSP {i}: {a} vs {b}"
            );
        }
        // Interpolation keeps order and spacing.
        let mid = interpolate(&mean(), &q1);
        assert!(is_ordered_and_spaced(&mid));
    }

    /// order_and_space sorts, then pushes from below and clamps from above.
    #[test]
    fn order_and_space_cases() {
        assert_eq!(order_and_space(mean()), mean());
        let q = order_and_space([0; 8]);
        for (i, &v) in q.iter().enumerate() {
            assert_eq!(v, LSP_MIN_GAP * (i as i32 + 1));
        }
        let q = order_and_space([32768; 8]);
        for (i, &v) in q.iter().enumerate() {
            assert_eq!(v, 32768 - LSP_MIN_GAP * (8 - i as i32));
        }
        let q = order_and_space([8000, 7000, 6000, 5000, 4000, 3000, 2000, 1000]);
        assert_eq!(q, [1000, 2000, 3000, 4000, 5000, 6000, 7000, 8000]);
        let v: Vec<i32> = q.to_vec();
        assert!(v.windows(2).all(|w| w[0] < w[1]));
    }
}
