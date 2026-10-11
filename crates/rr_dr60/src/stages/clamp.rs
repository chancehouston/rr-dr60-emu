//! ADC full-scale clamp: the codec's saturation after the record band-limit stage (spec 005
//! FR-008; research R-06).
//!
//! The MSM7702's A/D converter has a full-scale code (S-005); nothing larger than full scale
//! can be recorded. The emulator's dBFS scale puts that full scale at ±1.0 (A-030: 0 dBFS =
//! +3 dBm0 = 0.7 V peak at the codec input). This stage is the **codec's** saturation, not the
//! limiter's: the limiter (stage 3) holds sustained loud input at about −1 dBFS (A-028), just
//! below full scale, so in steady state the codec does not clip; during the limiter's dead time
//! the whole onset passes at full level and the codec clips it. Modeling the two separately keeps each bypassable and lets a user hear the
//! limiter alone (tap after the AGC) or the codec's clipping alone (AGC off).
//!
//! The clamp is a basic IEEE operation, so it is bit-identical on every platform; a bypassed
//! clamp returns the sample unchanged. NaN never reaches it (the pipeline sanitizes its input,
//! spec 001 Edge Cases). It has no state.

/// The codec's full-scale saturation, applied after stage 4 (data-model.md › ADC clamp).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ClampStage {
    enabled: bool,
}

impl ClampStage {
    /// A clamp that saturates when `enabled`, and passes samples unchanged otherwise.
    pub(crate) fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    /// One device-rate sample. `d.clamp(-1.0, 1.0)` when enabled; `d` unchanged when bypassed.
    #[inline]
    pub(crate) fn process(&self, d: f64) -> f64 {
        if self.enabled {
            // A-030: the codec's full scale (0 dBFS = +3 dBm0); S-005 full-scale code.
            d.clamp(-1.0, 1.0)
        } else {
            d
        }
    }

    /// No state to reset; kept for symmetry with the other stages.
    pub(crate) fn reset(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 005 T005 (FR-008; research R-06): saturation at exactly ±1.0, and anything inside full
    /// scale passes bit-exact.
    #[test]
    fn saturates_at_full_scale() {
        let clamp = ClampStage::new(true); // A-030: full scale; S-005 full-scale code
        assert_eq!(clamp.process(1.5), 1.0);
        assert_eq!(clamp.process(-1.5), -1.0);
        assert_eq!(clamp.process(0.999_999).to_bits(), 0.999_999f64.to_bits());
        assert_eq!(clamp.process(1.0).to_bits(), 1.0f64.to_bits());
        assert_eq!(clamp.process(-1.0).to_bits(), (-1.0f64).to_bits());
        assert_eq!(clamp.process(0.0).to_bits(), 0.0f64.to_bits());
        assert_eq!(clamp.process(2.82), 1.0); // +9 dBFS, 10 dB over the knee: the onset stimulus
    }

    /// 005 T005 (FR-008): bypassed, every input is returned bit for bit, including values
    /// beyond full scale.
    #[test]
    fn bypassed_returns_input_unchanged() {
        let clamp = ClampStage::new(false);
        for x in [
            1.5,
            -2.0,
            0.25,
            -0.0,
            0.0,
            1.0,
            -1.0,
            1e300,
            f64::MIN_POSITIVE,
        ] {
            assert_eq!(clamp.process(x).to_bits(), x.to_bits(), "{x:e}");
        }
    }

    /// 005 T005: no state. `reset` is a no-op and two fresh stages agree sample for sample.
    #[test]
    fn stateless() {
        let mut a = ClampStage::new(true);
        let b = ClampStage::new(true);
        for x in [0.5, 2.0, -3.0, 0.9] {
            assert_eq!(a.process(x).to_bits(), b.process(x).to_bits());
        }
        a.reset();
        assert_eq!(a, b);
        assert_eq!(a.process(2.0), 1.0);
    }
}
