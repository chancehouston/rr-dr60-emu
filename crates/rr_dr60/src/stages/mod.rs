//! Modeled signal-chain stages (docs/hardware/signal-chain.md).

// Used by the AGC stage (spec 002, tasks.md T022); until then only the coefficients exist.
#[allow(dead_code)]
pub(crate) mod agc_hilbert_coeffs;
mod biquad;
pub(crate) mod voiceband;
pub(crate) mod voiceband_coeffs;

#[cfg(test)]
mod agc_hilbert_tests {
    use super::agc_hilbert_coeffs::AGC_HILBERT;
    use rr_dr60_detmath::{PI, cos, ln, sin};

    /// |H(f)| in dB at the 8 kHz device rate (A-001), evaluated analytically with detmath.
    fn magnitude_db(freq_hz: f64) -> f64 {
        let w = 2.0 * PI * freq_hz / 8000.0;
        let (mut re, mut im) = (0.0, 0.0);
        for (k, &h) in AGC_HILBERT.iter().enumerate() {
            re += h * cos(w * k as f64);
            im -= h * sin(w * k as f64);
        }
        10.0 * ln(re * re + im * im) / ln(10.0)
    }

    /// 002 T008 / research.md R-03: within ±0.01 dB (engineering target, 002 R-15) at the
    /// FR-008 test frequencies.
    #[test]
    fn magnitude_is_flat_over_the_test_band() {
        for f in [300.0, 500.0, 1000.0, 2000.0, 8000.0 / 3.0, 3400.0] {
            let m = magnitude_db(f);
            assert!(m.abs() <= 0.01, "{f} Hz: {m:+.5} dB");
        }
    }

    /// Type III: antisymmetric, and every odd-index tap (including the centre) is exactly 0.
    #[test]
    fn antisymmetric_with_zero_odd_taps() {
        for k in 0..AGC_HILBERT.len() {
            // `==` treats 0.0 and -0.0 as equal, which is what antisymmetry means here.
            assert_eq!(AGC_HILBERT[k], -AGC_HILBERT[62 - k], "h[{k}]");
            if k % 2 == 1 {
                assert_eq!(AGC_HILBERT[k], 0.0, "h[{k}]");
            }
        }
    }
}
