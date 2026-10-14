//! Fixed-point operators for the speech codec (spec 008 research R-03; contracts/codec-design.md
//! › Arithmetic).
//!
//! Q15 samples and coefficients live in `i32` holding the 16-bit range −32768 … 32767 (one
//! Q15 unit is 2⁻¹⁵ of full scale); accumulators are `i64`; ratio comparisons use `i128` so no
//! square root or division is needed to rank search candidates. Every operation is an explicit
//! `saturating_*`, `wrapping_*` or `checked_*` method, a clamp, or a primitive whose range is
//! proven in a comment, so debug and release builds agree on every target (FR-016). The
//! operators are modeled on the ITU-T basic operators of the G.729 / G.723.1 reference code
//! (S-009, S-008) in form only; they are written here and hold no device figure.

/// The largest Q15 value.
pub(crate) const Q15_MAX: i32 = 32767;
/// The smallest Q15 value.
pub(crate) const Q15_MIN: i32 = -32768;
/// One in Q15 as an `i64` scale (not representable as a Q15 value itself).
pub(crate) const Q15_ONE: i64 = 32768;
/// One in Q14 (the adaptive-gain table's format, `tables::ADAPT_GAIN`).
pub(crate) const Q14_ONE: i64 = 16384;
/// One in Q12 (the format of the LPC coefficients, S-009 §3.2.2).
pub(crate) const Q12_ONE: i64 = 4096;
/// Rounding constant for a `>> 15`.
const HALF_Q15: i64 = 1 << 14;
/// Rounding constant for a `>> 12`.
const HALF_Q12: i64 = 1 << 11;

/// Saturates an accumulator to the 16-bit Q15 range.
#[inline]
pub(crate) fn sat16(x: i64) -> i32 {
    // range-proven: the clamp bounds the value to the i32 range before the cast.
    x.clamp(i64::from(Q15_MIN), i64::from(Q15_MAX)) as i32
}

/// Saturates an accumulator to the `i32` range.
#[inline]
pub(crate) fn sat32(x: i64) -> i32 {
    // range-proven: clamped to the i32 range before the cast.
    x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// Q15 + Q15, saturated to 16 bits.
#[inline]
pub(crate) fn add(a: i32, b: i32) -> i32 {
    sat16(i64::from(a).saturating_add(i64::from(b)))
}

/// Q15 − Q15, saturated to 16 bits.
#[inline]
pub(crate) fn sub(a: i32, b: i32) -> i32 {
    sat16(i64::from(a).saturating_sub(i64::from(b)))
}

/// Q15 × Q15 → Q15: `(a·b + 2¹⁴) >> 15`, saturated (the one case that saturates is
/// −32768 × −32768 = +32768 → 32767).
#[inline]
pub(crate) fn mult(a: i32, b: i32) -> i32 {
    sat16(
        i64::from(a)
            .saturating_mul(i64::from(b))
            .saturating_add(HALF_Q15)
            .wrapping_shr(15),
    )
}

/// `acc + a·b` in the `i64` accumulator, saturated.
#[inline]
pub(crate) fn mac(acc: i64, a: i32, b: i32) -> i64 {
    acc.saturating_add(i64::from(a).saturating_mul(i64::from(b)))
}

/// `acc − a·b` in the `i64` accumulator, saturated.
#[inline]
pub(crate) fn msu(acc: i64, a: i32, b: i32) -> i64 {
    acc.saturating_sub(i64::from(a).saturating_mul(i64::from(b)))
}

/// A Q15 value shifted left by `n` (0 … 31), saturated to 16 bits.
#[inline]
pub(crate) fn shl(a: i32, n: u32) -> i32 {
    // range-proven: |a| < 2³¹ and n ≤ 31, so the i64 shift cannot overflow.
    sat16(i64::from(a).wrapping_shl(n.min(31)))
}

/// A Q15 value shifted right by `n` (0 … 31), arithmetic (toward −∞).
#[inline]
pub(crate) fn shr(a: i32, n: u32) -> i32 {
    a.wrapping_shr(n.min(31))
}

/// A Q31 accumulator rounded to Q15 (`(x + 2¹⁵) >> 16`), saturated to 16 bits.
#[inline]
pub(crate) fn round16(x: i64) -> i32 {
    sat16(x.saturating_add(1 << 15).wrapping_shr(16))
}

/// An accumulator in Q(15 + 15) (a Q15 × Q15 product sum) rounded to Q15, saturated.
#[inline]
pub(crate) fn round_q15(acc: i64) -> i32 {
    sat16(acc.saturating_add(HALF_Q15).wrapping_shr(15))
}

/// An accumulator in Q(15 + 12) (a Q15 × Q12 product sum) rounded to Q15, saturated.
#[inline]
pub(crate) fn round_q12(acc: i64) -> i32 {
    sat16(acc.saturating_add(HALF_Q12).wrapping_shr(12))
}

/// An accumulator rounded right by `n` bits (to nearest, ties toward +∞), unsaturated.
#[inline]
pub(crate) fn round_shr(acc: i64, n: u32) -> i64 {
    if n == 0 {
        return acc;
    }
    let n = n.min(62);
    // range-proven: n ≥ 1 and ≤ 62, so the rounding constant is at most 2⁶¹.
    acc.saturating_add(1i64.wrapping_shl(n.wrapping_sub(1)))
        .wrapping_shr(n)
}

/// The number of left shifts that bring `x` (non-zero) into the range 2³⁰ ≤ |x| < 2³¹;
/// negative when `x` needs right shifts; 0 for 0.
#[inline]
pub(crate) fn norm32(x: i64) -> i32 {
    if x == 0 {
        return 0;
    }
    // An `i64` has 64 bits; a value in [2³⁰, 2³¹) has 33 leading zeros as a magnitude.
    // range-proven: leading_zeros is 0 … 64, so the subtraction stays within i32.
    let lz = x.unsigned_abs().leading_zeros() as i32;
    lz.wrapping_sub(33)
}

/// `x` shifted by a signed amount: left for positive `shift`, right (arithmetic, truncating)
/// for negative. Saturates to the `i64` range on a left shift.
#[inline]
pub(crate) fn shift_signed(x: i64, shift: i32) -> i64 {
    if shift >= 0 {
        let n = shift.unsigned_abs().min(63);
        // A left shift that would overflow i64 saturates instead of wrapping.
        let back = x.wrapping_shl(n).wrapping_shr(n);
        if back == x {
            x.wrapping_shl(n)
        } else if x < 0 {
            i64::MIN
        } else {
            i64::MAX
        }
    } else {
        x.wrapping_shr(shift.unsigned_abs().min(63))
    }
}

/// Q15 of `num / den` for `0 ≤ num ≤ den`, `den > 0`; saturates to 32767 at `num == den` and
/// above. Integer division is exact and platform-independent, so it replaces the reference
/// code's 15-step restoring division (R-03). A non-positive `den` gives 32767.
#[inline]
pub(crate) fn div_pos(num: i32, den: i32) -> i32 {
    if den <= 0 || num < 0 {
        return if num < 0 { 0 } else { Q15_MAX };
    }
    // range-proven: |num| < 2³¹, shifted by 15 stays within i64.
    sat16(
        i64::from(num)
            .wrapping_shl(15)
            .checked_div(i64::from(den))
            .unwrap_or(i64::from(Q15_MAX)),
    )
}

/// `(num << q) / den` in `i128`, saturated to `i64`; `den ≤ 0` gives 0 for `num == 0`, else
/// the saturated value of `num`'s sign. Exact integer division (R-03).
#[inline]
pub(crate) fn div_q(num: i64, den: i64, q: u32) -> i64 {
    if den <= 0 {
        return match num {
            0 => 0,
            n if n > 0 => i64::MAX,
            _ => i64::MIN,
        };
    }
    // range-proven: q ≤ 62, so an i64 shifted into i128 cannot overflow.
    let scaled = i128::from(num).wrapping_shl(q.min(62));
    let quotient = scaled.checked_div(i128::from(den)).unwrap_or(0);
    // range-proven: clamped to the i64 range before the cast.
    quotient.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

/// The square of an accumulator in `i128`.
#[inline]
pub(crate) fn sq(x: i64) -> i128 {
    i128::from(x).saturating_mul(i128::from(x))
}

/// `a > b` for the two ratios `a_num² / a_den` and `b_num² / b_den` (the search criteria
/// correlation² / energy), by `i128` cross-multiplication; a non-positive denominator ranks as
/// zero (FR-016: ties are true ties, resolved by the caller to the lowest index).
#[inline]
pub(crate) fn ratio_sq_greater(a_num: i64, a_den: i64, b_num: i64, b_den: i64) -> bool {
    let (a_den, b_den) = (a_den.max(0), b_den.max(0));
    if a_den == 0 {
        return false;
    }
    if b_den == 0 {
        return a_num != 0;
    }
    sq(a_num).saturating_mul(i128::from(b_den)) > sq(b_num).saturating_mul(i128::from(a_den))
}

/// The integer square root ⌊√x⌋ by bitwise restoring steps (deterministic, no floating point).
#[inline]
pub(crate) fn isqrt(x: u64) -> u64 {
    let mut rem = x;
    let mut root = 0u64;
    let mut bit = 1u64 << 62;
    while bit > rem.max(1).wrapping_shl(0) && bit > 1 {
        bit = bit.wrapping_shr(2);
    }
    while bit != 0 {
        let trial = root.saturating_add(bit);
        if rem >= trial {
            rem = rem.wrapping_sub(trial);
            root = root.wrapping_shr(1).saturating_add(bit);
        } else {
            root = root.wrapping_shr(1);
        }
        bit = bit.wrapping_shr(2);
    }
    root
}

/// A host `f32` sample to Q15: scaled by 32768, rounded to nearest with ties away from zero,
/// saturated to ±32767 … −32768 (contracts/codec-design.md › Arithmetic: the `core`-only
/// expression; `f32::round` is not in `core` at MSRV 1.85). Non-finite input gives 0.
#[inline]
pub(crate) fn f32_to_q15(x: f32) -> i32 {
    if !x.is_finite() {
        return 0;
    }
    let s = x * 32768.0;
    let r = if s >= 0.0 { s + 0.5 } else { s - 0.5 };
    // range-proven: `as` from f32 to i32 saturates in Rust; the clamp then applies Q15's.
    (r as i32).clamp(Q15_MIN, Q15_MAX)
}

/// A Q15 value to a host `f32`: ÷ 32768, exact in binary32.
#[inline]
pub(crate) fn q15_to_f32(v: i32) -> f32 {
    // range-proven: |v| ≤ 32768 is exact in f32, and the division by a power of two is exact.
    (v as f32) / 32768.0
}

/// The Q15 → `f32` conversion of a whole buffer (the decoder's output).
#[inline]
pub(crate) fn q15_slice_to_f32(src: &[i32], dst: &mut [f32]) {
    for (d, &s) in dst.iter_mut().zip(src) {
        *d = q15_to_f32(s);
    }
}

/// 008 T013 (spec 008 FR-016; research R-03; contracts/codec-design.md › Arithmetic): every
/// operator's saturating branch, the rounding rules and the conversions.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_sub_saturate() {
        assert_eq!(add(32000, 1000), Q15_MAX);
        assert_eq!(add(-32000, -1000), Q15_MIN);
        assert_eq!(add(100, -250), -150);
        assert_eq!(sub(-32000, 1000), Q15_MIN);
        assert_eq!(sub(32000, -1000), Q15_MAX);
        assert_eq!(sub(100, 250), -150);
    }

    #[test]
    fn mult_rounds_and_saturates() {
        assert_eq!(
            mult(Q15_MIN, Q15_MIN),
            Q15_MAX,
            "the one saturating product"
        );
        assert_eq!(mult(Q15_MAX, Q15_MAX), 32766);
        assert_eq!(mult(16384, 16384), 8192); // 0.5 × 0.5 = 0.25
        assert_eq!(mult(16384, -16384), -8192);
        assert_eq!(mult(1, 16384), 1, "rounds half up: 0.5 → 1");
        assert_eq!(mult(1, 16383), 0);
        assert_eq!(
            mult(-1, 16384),
            0,
            "ties toward +∞ in the >> 15 (as the reference)"
        );
    }

    #[test]
    fn mac_msu_accumulate_in_i64() {
        let mut acc = 0i64;
        for _ in 0..1000 {
            acc = mac(acc, Q15_MIN, Q15_MIN); // 2³⁰ each
        }
        assert_eq!(acc, 1000 << 30);
        assert_eq!(msu(acc, Q15_MIN, Q15_MIN), 999 << 30);
        assert_eq!(mac(i64::MAX, 1, 1), i64::MAX, "saturates");
        assert_eq!(msu(i64::MIN, 1, 1), i64::MIN, "saturates");
    }

    #[test]
    fn shifts() {
        assert_eq!(shl(1, 14), 16384);
        assert_eq!(shl(1, 15), Q15_MAX, "saturates");
        assert_eq!(shl(-1, 15), Q15_MIN);
        assert_eq!(shl(-2, 15), Q15_MIN, "saturates");
        assert_eq!(shl(3, 40), Q15_MAX, "an over-long shift saturates");
        assert_eq!(shr(-1, 1), -1, "arithmetic: toward −∞");
        assert_eq!(shr(Q15_MAX, 15), 0);
        assert_eq!(shr(Q15_MIN, 40), -1, "clamped to 31");
        assert_eq!(round_shr(5, 1), 3, "ties toward +∞");
        assert_eq!(round_shr(-5, 1), -2);
        assert_eq!(round_shr(7, 0), 7);
        assert_eq!(
            round_shr(i64::MAX, 70),
            1,
            "clamped to 62: (2⁶³ − 1 + 2⁶¹) >> 62"
        );
    }

    #[test]
    fn rounding_to_q15() {
        assert_eq!(round16(1 << 16), 1);
        assert_eq!(round16((1 << 15) - 1), 0);
        assert_eq!(round16(1 << 15), 1, "half rounds up");
        assert_eq!(round16(i64::MAX), Q15_MAX, "saturates");
        assert_eq!(round16(i64::MIN), Q15_MIN);
        assert_eq!(round_q15(1 << 15), 1);
        assert_eq!(round_q15(1 << 14), 1);
        assert_eq!(round_q15((1 << 14) - 1), 0);
        assert_eq!(round_q15(i64::MAX), Q15_MAX);
        assert_eq!(round_q12(1 << 12), 1);
        assert_eq!(round_q12(-(1 << 12)), -1);
        assert_eq!(round_q12(i64::MIN), Q15_MIN);
    }

    #[test]
    fn norm_and_shift_signed() {
        assert_eq!(norm32(0), 0);
        assert_eq!(norm32(1), 30);
        assert_eq!(norm32(1 << 30), 0);
        assert_eq!(norm32((1 << 31) - 1), 0);
        assert_eq!(norm32(1 << 31), -1);
        assert_eq!(norm32(-(1 << 30)), 0);
        assert_eq!(norm32(i64::MAX), -32);
        for x in [1i64, 123_456, -77, 1 << 40, -(1 << 50), 999_999_999_999] {
            let n = norm32(x);
            let y = shift_signed(x, n);
            assert!((1 << 30..1 << 31).contains(&y.abs()), "{x}: {n} → {y}");
        }
        assert_eq!(shift_signed(3, 0), 3);
        assert_eq!(shift_signed(3, 2), 12);
        assert_eq!(shift_signed(-13, -2), -4, "arithmetic right shift");
        assert_eq!(shift_signed(1 << 62, 2), i64::MAX, "saturates");
        assert_eq!(shift_signed(-(1 << 62), 2), i64::MIN, "saturates");
        assert_eq!(shift_signed(5, -70), 0, "clamped to 63");
    }

    #[test]
    fn div_pos_and_div_q() {
        assert_eq!(div_pos(1, 2), 16384);
        assert_eq!(div_pos(1, 3), 10922);
        assert_eq!(div_pos(0, 5), 0);
        assert_eq!(div_pos(5, 5), Q15_MAX, "saturates at 1.0");
        assert_eq!(div_pos(7, 5), Q15_MAX, "num > den saturates");
        assert_eq!(div_pos(3, 0), Q15_MAX, "den 0 saturates");
        assert_eq!(div_pos(3, -1), Q15_MAX);
        assert_eq!(div_pos(-3, 5), 0, "a negative numerator is clamped");
        assert_eq!(div_q(1, 4, 15), 8192);
        assert_eq!(div_q(-1, 4, 15), -8192);
        assert_eq!(div_q(i64::MAX, 1, 2), i64::MAX, "saturates");
        assert_eq!(div_q(i64::MIN, 1, 2), i64::MIN);
        assert_eq!(div_q(0, 0, 15), 0);
        assert_eq!(div_q(5, 0, 15), i64::MAX);
        assert_eq!(div_q(-5, -1, 15), i64::MIN);
        assert_eq!(div_q(1, 1, 70), 1 << 62, "q clamped to 62");
    }

    #[test]
    fn ratio_comparison() {
        // 3²/9 = 1 against 2²/4 = 1: equal, so not greater (a true tie).
        assert!(!ratio_sq_greater(3, 9, 2, 4));
        assert!(ratio_sq_greater(3, 8, 2, 4));
        assert!(!ratio_sq_greater(0, 8, 2, 4));
        assert!(ratio_sq_greater(2, 4, 0, 8));
        // A zero or negative energy ranks as zero.
        assert!(!ratio_sq_greater(5, 0, 1, 1));
        assert!(ratio_sq_greater(1, 1, 5, 0));
        assert!(!ratio_sq_greater(0, 1, 5, 0));
        assert!(!ratio_sq_greater(5, -3, 1, 1));
        // Large values: no overflow in i128 (|num| up to 2⁴⁰, den up to 2⁴⁰).
        assert!(ratio_sq_greater(1 << 40, (1 << 40) - 1, 1 << 40, 1 << 40));
        assert_eq!(sq(1 << 40), 1i128 << 80);
        assert_eq!(sq(i64::MIN), (1i128 << 126));
    }

    #[test]
    fn integer_sqrt() {
        for (x, want) in [
            (0u64, 0u64),
            (1, 1),
            (2, 1),
            (3, 1),
            (4, 2),
            (15, 3),
            (16, 4),
            (17, 4),
        ] {
            assert_eq!(isqrt(x), want, "√{x}");
        }
        for x in [1u64 << 20, (1 << 40) + 12345, u64::MAX, 999_999_999_999] {
            let r = isqrt(x);
            assert!(r.saturating_mul(r) <= x, "√{x} = {r}");
            let r1 = r.saturating_add(1);
            assert!(
                r1.saturating_mul(r1) > x || r1.checked_mul(r1).is_none(),
                "√{x} = {r}"
            );
        }
        assert_eq!(isqrt(u64::MAX), (1 << 32) - 1);
    }

    #[test]
    fn f32_to_q15_rounds_ties_away_from_zero_and_saturates() {
        assert_eq!(f32_to_q15(0.0), 0);
        assert_eq!(f32_to_q15(-0.0), 0);
        assert_eq!(f32_to_q15(1.0), Q15_MAX, "+1.0 saturates to 32767");
        assert_eq!(f32_to_q15(-1.0), Q15_MIN);
        assert_eq!(f32_to_q15(1.5), Q15_MAX, "saturates");
        assert_eq!(f32_to_q15(-1.5), Q15_MIN);
        assert_eq!(
            f32_to_q15(0.5 / 32768.0),
            1,
            "a half unit rounds away from zero"
        );
        assert_eq!(f32_to_q15(-0.5 / 32768.0), -1);
        assert_eq!(f32_to_q15(0.49 / 32768.0), 0);
        assert_eq!(f32_to_q15(0.25), 8192);
        assert_eq!(f32_to_q15(f32::NAN), 0);
        assert_eq!(f32_to_q15(f32::INFINITY), 0);
        assert_eq!(f32_to_q15(f32::NEG_INFINITY), 0);
        assert_eq!(f32_to_q15(1.0e-39), 0, "a subnormal rounds to 0");
    }

    #[test]
    fn q15_to_f32_is_exact() {
        assert_eq!(q15_to_f32(Q15_MAX), 32767.0 / 32768.0);
        assert_eq!(q15_to_f32(Q15_MIN), -1.0);
        assert_eq!(q15_to_f32(0), 0.0);
        assert_eq!(q15_to_f32(-16384), -0.5);
        let mut out = [9.0f32; 3];
        q15_slice_to_f32(&[1, -1, 16384], &mut out);
        assert_eq!(out, [1.0 / 32768.0, -1.0 / 32768.0, 0.5]);
        // Round trip of every Q15 value.
        for v in Q15_MIN..=Q15_MAX {
            assert_eq!(f32_to_q15(q15_to_f32(v)), v, "{v}");
        }
    }
}
