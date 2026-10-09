//! Input sanitizing and denormal flushing (research.md R-05).
//!
//! Used by the per-sample engine (data-model.md steps 1 and 3) and by every filter state
//! update. All three functions are deterministic compare-and-store operations (R-04).

/// Filter state values with magnitude below this are set to exactly 0.0 (R-05).
///
/// This makes IIR tails reach digital silence, keeps processing time constant (no subnormal
/// slowdowns), and keeps results independent of the host's FTZ/DAZ settings.
pub(crate) const STATE_FLUSH: f64 = 1e-30; // engineering target (R-05)

/// Input sample to the internal f64 domain. Non-finite and f32-subnormal samples become 0.0
/// (spec Edge Cases, FR-002). Normal values, including those beyond ±1.0, are kept exactly.
#[inline]
pub(crate) fn sanitize_in(x: f32) -> f64 {
    if x.is_finite() && !x.is_subnormal() {
        f64::from(x)
    } else {
        0.0
    }
}

/// Internal f64 sample to f32 output, rounding to nearest. Results that would be f32
/// subnormals become 0.0, so the output never contains subnormals (spec Edge Cases).
#[inline]
pub(crate) fn narrow_out(y: f64) -> f32 {
    let v = y as f32;
    if v.is_subnormal() { 0.0 } else { v }
}

/// Flushes tiny filter state to exactly 0.0 (R-05).
#[inline]
pub(crate) fn flush_state(x: f64) -> f64 {
    if x < STATE_FLUSH && x > -STATE_FLUSH {
        0.0
    } else {
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 002 T007 / research.md R-07: with up to +60 dB of AGC gain, huge finite inputs must not
    /// become ±Inf on narrowing; they saturate to ±f32::MAX.
    #[test]
    fn narrow_out_saturates_instead_of_overflowing() {
        assert_eq!(narrow_out(1e300).to_bits(), f32::MAX.to_bits());
        assert_eq!(narrow_out(-1e300).to_bits(), f32::MIN.to_bits());
        assert_eq!(narrow_out(f64::from(f32::MAX) * 2.0).to_bits(), f32::MAX.to_bits());
        assert_eq!(narrow_out(0.25).to_bits(), 0.25f32.to_bits());
        assert_eq!(narrow_out(1e-40).to_bits(), 0.0f32.to_bits());
    }

    #[test]
    fn sanitize_in_zeroes_non_finite_and_subnormal() {
        for x in [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::from_bits(1),
            -f32::from_bits(0x007f_ffff),
        ] {
            assert_eq!(sanitize_in(x).to_bits(), 0.0f64.to_bits(), "{x:e}");
        }
    }

    #[test]
    fn sanitize_in_keeps_normal_values_exactly() {
        for x in [
            0.0f32,
            1.0,
            -1.0,
            0.25,
            4.0,
            -123.456,
            f32::MIN_POSITIVE,
            f32::MAX,
        ] {
            assert_eq!(sanitize_in(x), f64::from(x), "{x:e}");
        }
    }

    #[test]
    fn narrow_out_flushes_f32_subnormals_and_rounds_to_nearest() {
        assert_eq!(narrow_out(1e-40), 0.0); // would be an f32 subnormal
        assert_eq!(narrow_out(-1e-40), 0.0);
        assert_eq!(narrow_out(0.1), 0.1f32);
        assert_eq!(narrow_out(1.0 + 1e-9), 1.0f32);
        assert_eq!(narrow_out(f64::from(f32::MIN_POSITIVE)), f32::MIN_POSITIVE);
    }

    #[test]
    fn flush_state_threshold() {
        assert_eq!(flush_state(9.9e-31), 0.0); // R-05
        assert_eq!(flush_state(-9.9e-31), 0.0);
        assert_eq!(flush_state(1e-30), 1e-30);
        assert_eq!(flush_state(-0.5), -0.5);
    }
}
