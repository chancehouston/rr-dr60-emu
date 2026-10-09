//! Pipeline settings, defaults and supported rates (data-model.md › Settings).

/// Host sample rates supported by this version, in Hz (FR-002).
///
/// Any other rate is rejected by [`Pipeline::new`](crate::Pipeline::new) with
/// [`Error::UnsupportedHostRate`](crate::Error::UnsupportedHostRate).
pub const SUPPORTED_HOST_RATES: [u32; 6] = [8000, 16000, 44100, 48000, 88200, 96000]; // FR-002

/// The emulated device's internal sample rate, in Hz.
///
/// A-001: assumed 8 kHz, the standard rate for the MSM7702 class of voice-band codec.
pub const DEVICE_RATE_HZ: u32 = 8000;

/// Where the pipeline output is taken (FR-007).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Tap {
    /// After signal-chain stage 4 (record band-limit). Stage 10 is not run.
    AfterRecord,
    /// After signal-chain stage 10 (playback band-limit). The default, because the device
    /// band-limits on both record and playback (A-002).
    #[default]
    AfterPlayback,
}

/// Pipeline configuration (data-model.md › Settings).
///
/// Settings are fixed for the life of a configuration. To change them, call
/// [`Pipeline::reconfigure`](crate::Pipeline::reconfigure), which is not real-time safe (FR-008).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Settings {
    /// Host sample rate in Hz. Must be one of [`SUPPORTED_HOST_RATES`].
    pub host_rate_hz: u32,
    /// Stage 4 (record anti-alias filter + ADC) on (`true`, default, A-002) or bypassed.
    pub record_stage_enabled: bool,
    /// Stage 10 (playback DAC + reconstruction filter) on (`true`, default, A-002) or bypassed.
    /// Ignored when `tap` is [`Tap::AfterRecord`].
    pub playback_stage_enabled: bool,
    /// Output tap point. Default [`Tap::AfterPlayback`].
    pub tap: Tap,
    /// Seed for stochastic stages (FR-009). No stage is stochastic in this version, so the
    /// seed has no effect on output.
    pub seed: u64,
}

impl Settings {
    /// Default settings for the given host rate: both stages on, tap after playback, seed 0.
    ///
    /// The rate is not validated here. [`Pipeline::new`](crate::Pipeline::new) validates it.
    pub const fn new(host_rate_hz: u32) -> Self {
        Self {
            host_rate_hz,
            record_stage_enabled: true,   // A-002
            playback_stage_enabled: true, // A-002
            tap: Tap::AfterPlayback,
            seed: 0, // FR-009
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_cite_a002_and_fr009() {
        let s = Settings::new(48_000);
        assert_eq!(s.host_rate_hz, 48_000);
        assert!(s.record_stage_enabled); // A-002
        assert!(s.playback_stage_enabled); // A-002
        assert_eq!(s.tap, Tap::AfterPlayback);
        assert_eq!(s.seed, 0); // FR-009
        assert_eq!(Tap::default(), Tap::AfterPlayback);
    }

    #[test]
    fn supported_rates_and_device_rate() {
        assert_eq!(
            SUPPORTED_HOST_RATES,
            [8000, 16000, 44100, 48000, 88200, 96000]
        ); // FR-002
        assert_eq!(DEVICE_RATE_HZ, 8000); // A-001
    }

    #[test]
    fn new_does_not_validate() {
        // Validation happens in Pipeline::new / reconfigure (data-model.md).
        assert_eq!(Settings::new(22_050).host_rate_hz, 22_050);
    }
}
