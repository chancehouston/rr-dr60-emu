//! Deterministic math for the RR-DR60 emulator.
//!
//! Every function in this crate is built only from IEEE-754 basic operations
//! (`+ − × ÷` and comparisons), which are correctly rounded on every supported
//! platform. The results are therefore bit-identical on Linux, macOS, Windows
//! and iOS, on both x86-64 and ARM64 (research.md R-04, R-06; spec FR-014).
//!
//! The platform math library (`f64::sin`, `f64::exp`, …) is deliberately not
//! used: its results differ in the last bits between operating systems, which
//! would break the single set of golden files.
//!
//! Accuracy targets (research.md R-06): `sin`, `cos`, `exp` and `ln` are
//! within 2 ulp (for `sin`/`cos`, for |x| < 2²⁰·π/2 ≈ 1.6e6), `sqrt` is
//! correctly rounded, and `bessel_i0` has a relative error of at most 1e-14.
//!
//! ## Third-party notice
//!
//! The polynomial coefficients and argument reductions for `sin`, `cos`, `exp`
//! and `ln` are taken from fdlibm (via musl), which carries this notice:
//!
//! > Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//! >
//! > Developed at SunPro, a Sun Microsystems, Inc. business.
//! > Permission to use, copy, modify, and distribute this
//! > software is freely granted, provided that this notice
//! > is preserved.
//!
//! See also `THIRD_PARTY.md` at the repository root.
#![no_std]
#![forbid(unsafe_code)]

/// π, correctly rounded.
pub const PI: f64 = core::f64::consts::PI;

/// 2π, correctly rounded.
pub const TAU: f64 = core::f64::consts::TAU;

/// High 32 bits of the IEEE-754 representation.
#[inline]
fn high_word(x: f64) -> u32 {
    (x.to_bits() >> 32) as u32
}

// ---------------------------------------------------------------------------
// sin / cos (fdlibm __kernel_sin, __kernel_cos, __rem_pio2 medium case)
// ---------------------------------------------------------------------------

const S1: f64 = f64::from_bits(0xBFC5_5555_5555_5549); // -1.66666666666666324348e-01
const S2: f64 = f64::from_bits(0x3F81_1111_1110_F8A6); //  8.33333333332248946124e-03
const S3: f64 = f64::from_bits(0xBF2A_01A0_19C1_61D5); // -1.98412698298579493134e-04
const S4: f64 = f64::from_bits(0x3EC7_1DE3_57B1_FE7D); //  2.75573137070700676789e-06
const S5: f64 = f64::from_bits(0xBE5A_E5E6_8A2B_9CEB); // -2.50507602534068634195e-08
const S6: f64 = f64::from_bits(0x3DE5_D93A_5ACF_D57C); //  1.58969099521155010221e-10

const C1: f64 = f64::from_bits(0x3FA5_5555_5555_554C); //  4.16666666666666019037e-02
const C2: f64 = f64::from_bits(0xBF56_C16C_16C1_5177); // -1.38888888888741095749e-03
const C3: f64 = f64::from_bits(0x3EFA_01A0_19CB_1590); //  2.48015872894767294178e-05
const C4: f64 = f64::from_bits(0xBE92_7E4F_809C_52AD); // -2.75573143513906633035e-07
const C5: f64 = f64::from_bits(0x3E21_EE9E_BDB4_B1C4); //  2.08757232129817482790e-09
const C6: f64 = f64::from_bits(0xBDA8_FAE9_BE88_38D4); // -1.13596475577881948265e-11

const INVPIO2: f64 = f64::from_bits(0x3FE4_5F30_6DC9_C883); // 6.36619772367581382433e-01
const PIO2_1: f64 = f64::from_bits(0x3FF9_21FB_5440_0000); // first 33 bits of pi/2
const PIO2_1T: f64 = f64::from_bits(0x3DD0_B461_1A62_6331); // pi/2 - PIO2_1
const PIO2_2: f64 = f64::from_bits(0x3DD0_B461_1A60_0000); // second 33 bits of pi/2
const PIO2_2T: f64 = f64::from_bits(0x3BA3_198A_2E03_7073); // pi/2 - (PIO2_1 + PIO2_2)
const PIO2_3: f64 = f64::from_bits(0x3BA3_198A_2E00_0000); // third 33 bits of pi/2
const PIO2_3T: f64 = f64::from_bits(0x397B_839A_2520_49C1); // pi/2 - (PIO2_1 + PIO2_2 + PIO2_3)

/// 1.5 / f64::EPSILON: adding and subtracting it rounds to the nearest integer.
const TOINT: f64 = 6_755_399_441_055_744.0;

/// sin(x + y) for |x| ≤ π/4, where y is the tail of x. `has_tail` is false when y = 0.
fn kernel_sin(x: f64, y: f64, has_tail: bool) -> f64 {
    let z = x * x;
    let w = z * z;
    let r = S2 + z * (S3 + z * S4) + z * w * (S5 + z * S6);
    let v = z * x;
    if has_tail {
        x - ((z * (0.5 * y - v * r) - y) - v * S1)
    } else {
        x + v * (S1 + z * r)
    }
}

/// cos(x + y) for |x| ≤ π/4, where y is the tail of x.
fn kernel_cos(x: f64, y: f64) -> f64 {
    let z = x * x;
    let w = z * z;
    let r = z * (C1 + z * (C2 + z * C3)) + w * w * (C4 + z * (C5 + z * C6));
    let hz = 0.5 * z;
    let w = 1.0 - hz;
    w + (((1.0 - w) - hz) + (z * r - x * y))
}

/// Reduces x to y0 + y1 = x − n·π/2 with |y0| ≤ π/4 (Cody-Waite, 3-part split).
/// Accurate for |x| < 2²⁰·π/2, the range this crate supports.
fn rem_pio2(x: f64) -> (i32, f64, f64) {
    let ix = high_word(x) & 0x7fff_ffff;
    let fnum = x * INVPIO2 + TOINT - TOINT;
    let n = fnum as i32;
    let mut r = x - fnum * PIO2_1;
    let mut w = fnum * PIO2_1T;
    let mut y0 = r - w;
    let ex = (ix >> 20) as i32;
    let ey = ((y0.to_bits() >> 52) & 0x7ff) as i32;
    if ex - ey > 16 {
        // Second round, good to 118 bits.
        let t = r;
        w = fnum * PIO2_2;
        r = t - w;
        w = fnum * PIO2_2T - ((t - r) - w);
        y0 = r - w;
        let ey = ((y0.to_bits() >> 52) & 0x7ff) as i32;
        if ex - ey > 49 {
            // Third round, good to 151 bits.
            let t = r;
            w = fnum * PIO2_3;
            r = t - w;
            w = fnum * PIO2_3T - ((t - r) - w);
            y0 = r - w;
        }
    }
    let y1 = (r - y0) - w;
    (n, y0, y1)
}

/// Sine of `x` (radians), bit-identical on every platform.
pub fn sin(x: f64) -> f64 {
    let ix = high_word(x) & 0x7fff_ffff;
    if ix <= 0x3fe9_21fb {
        // |x| <= pi/4
        if ix < 0x3e50_0000 {
            return x; // |x| < 2^-26: sin(x) rounds to x
        }
        return kernel_sin(x, 0.0, false);
    }
    if ix >= 0x7ff0_0000 {
        return f64::NAN; // sin(NaN) and sin(±inf) are NaN
    }
    let (n, y0, y1) = rem_pio2(x);
    match n & 3 {
        0 => kernel_sin(y0, y1, true),
        1 => kernel_cos(y0, y1),
        2 => -kernel_sin(y0, y1, true),
        _ => -kernel_cos(y0, y1),
    }
}

/// Cosine of `x` (radians), bit-identical on every platform.
pub fn cos(x: f64) -> f64 {
    let ix = high_word(x) & 0x7fff_ffff;
    if ix <= 0x3fe9_21fb {
        if ix < 0x3e46_a09e {
            return 1.0; // |x| < 2^-27 * sqrt(2): cos(x) rounds to 1
        }
        return kernel_cos(x, 0.0);
    }
    if ix >= 0x7ff0_0000 {
        return f64::NAN; // cos(NaN) and cos(±inf) are NaN
    }
    let (n, y0, y1) = rem_pio2(x);
    match n & 3 {
        0 => kernel_cos(y0, y1),
        1 => -kernel_sin(y0, y1, true),
        2 => -kernel_cos(y0, y1),
        _ => kernel_sin(y0, y1, true),
    }
}

// ---------------------------------------------------------------------------
// exp / ln (fdlibm e_exp, e_log)
// ---------------------------------------------------------------------------

const LN2_HI: f64 = f64::from_bits(0x3FE6_2E42_FEE0_0000); // 6.93147180369123816490e-01
const LN2_LO: f64 = f64::from_bits(0x3DEA_39EF_3579_3C76); // 1.90821492927058770002e-10
const INVLN2: f64 = f64::from_bits(0x3FF7_1547_652B_82FE); // 1.44269504088896338700e+00
const P1: f64 = f64::from_bits(0x3FC5_5555_5555_553E); //  1.66666666666666019037e-01
const P2: f64 = f64::from_bits(0xBF66_C16C_16BE_BD93); // -2.77777777770155933842e-03
const P3: f64 = f64::from_bits(0x3F11_566A_AF25_DE2C); //  6.61375632143793436117e-05
const P4: f64 = f64::from_bits(0xBEBB_BD41_C5D2_6BF1); // -1.65339022054652515390e-06
const P5: f64 = f64::from_bits(0x3E66_3769_72BE_A4D0); //  4.13813679705723846039e-08

const LG1: f64 = f64::from_bits(0x3FE5_5555_5555_5593); // 6.666666666666735130e-01
const LG2: f64 = f64::from_bits(0x3FD9_9999_9997_FA04); // 3.999999999940941908e-01
const LG3: f64 = f64::from_bits(0x3FD2_4924_9422_9359); // 2.857142874366239149e-01
const LG4: f64 = f64::from_bits(0x3FCC_71C5_1D8E_78AF); // 2.222219843214978396e-01
const LG5: f64 = f64::from_bits(0x3FC7_4664_96CB_03DE); // 1.818357216161805012e-01
const LG6: f64 = f64::from_bits(0x3FC3_9A09_D078_C69F); // 1.531383769920937332e-01
const LG7: f64 = f64::from_bits(0x3FC2_F112_DF3E_5244); // 1.479819860511658591e-01

/// x · 2ⁿ, exact whenever the result is representable (musl scalbn).
fn scalbn(mut x: f64, mut n: i32) -> f64 {
    const P1023: f64 = f64::from_bits(0x7FE0_0000_0000_0000); // 2^1023
    const PM969: f64 = f64::from_bits(0x0360_0000_0000_0000); // 2^-1022 * 2^53
    if n > 1023 {
        x *= P1023;
        n -= 1023;
        if n > 1023 {
            x *= P1023;
            n -= 1023;
            if n > 1023 {
                n = 1023;
            }
        }
    } else if n < -1022 {
        x *= PM969;
        n += 1022 - 53;
        if n < -1022 {
            x *= PM969;
            n += 1022 - 53;
            if n < -1022 {
                n = -1022;
            }
        }
    }
    x * f64::from_bits(((0x3ff + n) as u64) << 52)
}

/// eˣ, bit-identical on every platform.
pub fn exp(x: f64) -> f64 {
    let sign = (x.to_bits() >> 63) as i32;
    let hx = high_word(x) & 0x7fff_ffff;
    if hx >= 0x4086_232b {
        // |x| >= 708.39 or NaN
        if x.is_nan() {
            return x;
        }
        if x > 709.782_712_893_384 {
            return f64::INFINITY;
        }
        if x < -745.133_219_101_941_1 {
            return 0.0;
        }
    }
    let (hi, lo, k, xr);
    if hx > 0x3fd6_2e42 {
        // |x| > 0.5 ln2
        k = if hx >= 0x3ff0_a2b2 {
            // |x| >= 1.5 ln2
            (INVLN2 * x + if sign == 0 { 0.5 } else { -0.5 }) as i32
        } else {
            1 - sign - sign
        };
        hi = x - k as f64 * LN2_HI;
        lo = k as f64 * LN2_LO;
        xr = hi - lo;
    } else if hx > 0x3e30_0000 {
        // |x| > 2^-28
        k = 0;
        hi = x;
        lo = 0.0;
        xr = x;
    } else {
        return 1.0 + x;
    }
    let xx = xr * xr;
    let c = xr - xx * (P1 + xx * (P2 + xx * (P3 + xx * (P4 + xx * P5))));
    let y = 1.0 + (xr * c / (2.0 - c) - lo + hi);
    if k == 0 { y } else { scalbn(y, k) }
}

/// Natural logarithm, bit-identical on every platform.
pub fn ln(x: f64) -> f64 {
    let mut bits = x.to_bits();
    let mut hx = (bits >> 32) as u32;
    let mut k: i32 = 0;
    let mut x = x;
    if hx < 0x0010_0000 || (hx >> 31) != 0 {
        if bits << 1 == 0 {
            return f64::NEG_INFINITY; // ln(±0)
        }
        if (hx >> 31) != 0 {
            return f64::NAN; // ln(negative)
        }
        // Subnormal: scale up by 2^54.
        k -= 54;
        x *= f64::from_bits(0x4350_0000_0000_0000);
        bits = x.to_bits();
        hx = (bits >> 32) as u32;
    } else if hx >= 0x7ff0_0000 {
        return x; // +inf or NaN
    } else if hx == 0x3ff0_0000 && bits << 32 == 0 {
        return 0.0;
    }
    // Reduce x into [sqrt(2)/2, sqrt(2)].
    hx = hx.wrapping_add(0x3ff0_0000 - 0x3fe6_a09e);
    k += (hx >> 20) as i32 - 0x3ff;
    hx = (hx & 0x000f_ffff) + 0x3fe6_a09e;
    let x = f64::from_bits((u64::from(hx) << 32) | (bits & 0xffff_ffff));
    let f = x - 1.0;
    let hfsq = 0.5 * f * f;
    let s = f / (2.0 + f);
    let z = s * s;
    let w = z * z;
    let t1 = w * (LG2 + w * (LG4 + w * LG6));
    let t2 = z * (LG1 + w * (LG3 + w * (LG5 + w * LG7)));
    let r = t2 + t1;
    let dk = k as f64;
    s * (hfsq + r) + dk * LN2_LO - hfsq + f + dk * LN2_HI
}

// ---------------------------------------------------------------------------
// sqrt (exact integer method) and Bessel I0
// ---------------------------------------------------------------------------

/// Floor square root of a u128 by the bit-by-bit method. Returns (root, remainder).
fn isqrt_u128(n: u128) -> (u128, u128) {
    let mut rem = n;
    let mut root: u128 = 0;
    let mut bit: u128 = 1 << 126;
    while bit > n {
        bit >>= 2;
    }
    while bit != 0 {
        if rem >= root + bit {
            rem -= root + bit;
            root = (root >> 1) + bit;
        } else {
            root >>= 1;
        }
        bit >>= 2;
    }
    (root, rem)
}

/// Correctly rounded square root (round-to-nearest-even), using integer arithmetic only.
pub fn sqrt(x: f64) -> f64 {
    if x.is_nan() || x < 0.0 {
        return f64::NAN;
    }
    if x == 0.0 || x == f64::INFINITY {
        return x; // keeps the sign of -0.0
    }
    // Decompose x = m · 2^e with m an integer in [2^52, 2^53).
    let bits = x.to_bits();
    let raw_exp = ((bits >> 52) & 0x7ff) as i32;
    let frac = bits & 0x000f_ffff_ffff_ffff;
    let (mut m, mut e) = if raw_exp == 0 {
        // Subnormal: normalize.
        let shift = frac.leading_zeros() as i32 - 11;
        (frac << shift, -1074 - shift)
    } else {
        (frac | (1 << 52), raw_exp - 1075)
    };
    if e & 1 != 0 {
        m <<= 1;
        e -= 1;
    }
    // sqrt(m · 2^54) has 54 significant bits: 53 plus one guard bit.
    let (s, rem) = isqrt_u128(u128::from(m) << 54);
    let mut q = (s >> 1) as u64;
    let guard = s & 1 == 1;
    let sticky = rem != 0;
    if guard && (sticky || q & 1 == 1) {
        q += 1;
    }
    // Result = q · 2^((e - 54) / 2 + 1). q ≤ 2^53, so the conversion is exact.
    let scale = (e - 54) / 2 + 1;
    scalbn(q as f64, scale)
}

/// Modified Bessel function of the first kind, order zero (power series).
///
/// Used for the Kaiser window (research.md R-08). The series is summed until a term falls
/// below 1e-17 of the sum, or 64 terms.
pub fn bessel_i0(x: f64) -> f64 {
    let q = 0.25 * x * x;
    let mut term = 1.0;
    let mut sum = 1.0;
    let mut k = 1.0;
    while k <= 64.0 {
        term *= q / (k * k);
        sum += term;
        if term < 1e-17 * sum {
            break;
        }
        k += 1.0;
    }
    sum
}

#[cfg(test)]
mod test_refs;

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // T012: tests may compare against std's correctly rounded sqrt.
mod tests {
    extern crate std;
    use super::*;
    use test_refs::*;

    /// Distance in units in the last place between two finite f64 values.
    fn ulps(a: f64, b: f64) -> u64 {
        if a == b {
            return 0;
        }
        let key = |x: f64| {
            let i = x.to_bits() as i64;
            if i < 0 { i64::MIN - i } else { i }
        };
        key(a).abs_diff(key(b))
    }

    fn check_ulps(name: &str, f: fn(f64) -> f64, refs: &[(f64, f64)], max_ulps: u64) {
        for &(x, want) in refs {
            let got = f(x);
            let d = ulps(got, want);
            assert!(
                d <= max_ulps,
                "{name}({x:e}) = {got:e}, want {want:e} ({d} ulp > {max_ulps})"
            );
        }
    }

    #[test]
    fn sin_within_2_ulp() {
        assert!(SIN_REFS.len() >= 64);
        check_ulps("sin", sin, SIN_REFS, 2);
    }

    #[test]
    fn cos_within_2_ulp() {
        assert!(COS_REFS.len() >= 64);
        check_ulps("cos", cos, COS_REFS, 2);
    }

    #[test]
    fn exp_within_2_ulp() {
        assert!(EXP_REFS.len() >= 64);
        check_ulps("exp", exp, EXP_REFS, 2);
    }

    #[test]
    fn ln_within_2_ulp() {
        assert!(LN_REFS.len() >= 64);
        check_ulps("ln", ln, LN_REFS, 2);
    }

    #[test]
    fn bessel_i0_relative_error() {
        for &(x, want) in I0_REFS {
            let got = bessel_i0(x);
            let rel = ((got - want) / want).abs();
            assert!(
                rel <= 1e-14,
                "bessel_i0({x}) = {got:e}, want {want:e} (rel {rel:e})"
            );
        }
    }

    #[test]
    fn sin_cos_identity() {
        let mut x = -1e4;
        let step = 2e4 / 10_000.0;
        for _ in 0..10_000 {
            let (s, c) = (sin(x), cos(x));
            let e = (s * s + c * c - 1.0).abs();
            assert!(e <= 4e-16, "sin²+cos²-1 = {e:e} at x = {x}");
            x += step;
        }
    }

    #[test]
    fn sqrt_is_correctly_rounded() {
        // Seeded xorshift over the full exponent range, plus edge cases.
        let mut s: u64 = 0x0D60_0D60_0D60_0D60;
        let mut checked = 0;
        while checked < 10_000 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let x = f64::from_bits(s >> 1); // positive, any exponent
            if !x.is_finite() {
                continue;
            }
            assert_eq!(sqrt(x).to_bits(), x.sqrt().to_bits(), "sqrt({x:e})");
            checked += 1;
        }
        for x in [
            0.0,
            -0.0,
            1.0,
            2.0,
            4.0,
            f64::MIN_POSITIVE,
            5e-324,
            f64::MAX,
            f64::INFINITY,
        ] {
            assert_eq!(sqrt(x).to_bits(), x.sqrt().to_bits(), "sqrt({x:e})");
        }
        assert!(sqrt(-1.0).is_nan());
        assert!(sqrt(f64::NAN).is_nan());
    }

    #[test]
    fn special_values() {
        assert!(sin(f64::NAN).is_nan() && sin(f64::INFINITY).is_nan());
        assert!(cos(f64::NAN).is_nan() && cos(f64::NEG_INFINITY).is_nan());
        assert_eq!(exp(f64::NEG_INFINITY), 0.0);
        assert_eq!(exp(f64::INFINITY), f64::INFINITY);
        assert_eq!(exp(-800.0), 0.0);
        assert_eq!(exp(800.0), f64::INFINITY);
        assert_eq!(ln(0.0), f64::NEG_INFINITY);
        assert!(ln(-1.0).is_nan());
        assert_eq!(ln(f64::INFINITY), f64::INFINITY);
        assert_eq!(ln(1.0), 0.0);
        assert!((ln(5e-324) - -744.4400719213812).abs() < 1e-12); // subnormal input
        assert_eq!(bessel_i0(-2.0), bessel_i0(2.0));
        assert!((TAU - 2.0 * PI).abs() == 0.0);
    }
}
