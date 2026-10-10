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

/// Where the pipeline output is taken (FR-007; spec 002 FR-003).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Tap {
    /// After signal-chain stage 3 (AGC). Stages 4 and 10 are not run, whatever their settings
    /// (spec 002 FR-003).
    AfterAgc,
    /// After signal-chain stage 4 (record band-limit). Stages 5 (VAS) and 10 are not run.
    AfterRecord,
    /// After signal-chain stage 5 (VAS). Stage 10 is not run, whatever its setting
    /// (spec 003 FR-003). In drop mode the output can be shorter than the input.
    AfterVas,
    /// After signal-chain stage 10 (playback band-limit). The default, because the device
    /// band-limits on both record and playback (A-002).
    #[default]
    AfterPlayback,
}

/// Record-path automatic gain control, signal-chain stage 3 (spec 002).
///
/// Modeled on an *assumed* AGC: every default below is a low-confidence assumption
/// (A-017 – A-020, `docs/hardware/assumptions.md`) until captures from a real unit exist.
/// The AGC raises quiet input and lowers loud input toward [`target_dbfs`](Self::target_dbfs)
/// with a fixed 10:1 slope (A-017). Its side effects are level "pumping" after loud sounds and
/// background noise that rises during pauses.
///
/// # Output level
///
/// Right after the pipeline is created or reset, or after a long silence, the gain is at
/// [`max_gain_db`](Self::max_gain_db). A loud first sound then comes out up to that many dB
/// above its final level (+40 dB, ×100 amplitude, with the defaults; +60 dB at most) for about
/// one attack time. The output is not clipped. Limit or clip it yourself before converting to
/// a fixed-point format such as 16-bit PCM. Inputs above 0 dBFS are valid AGC input.
///
/// Levels use the AES17 convention: a sine with peak amplitude 1.0 is 0 dBFS. Ranges are
/// inclusive; values outside them, NaN and ±Inf are rejected by
/// [`Pipeline::new`](crate::Pipeline::new) (spec 002 FR-011), even when the AGC is bypassed.
///
/// # Example
///
/// ```
/// use rr_dr60::{Error, Pipeline, Setting, Settings, Tap};
///
/// // Bypass the AGC (the spec 001 sound: band-limiting only).
/// let mut s = Settings::new(48_000);
/// s.agc.enabled = false;
/// Pipeline::new(s)?;
///
/// // Keep the AGC, but recover more slowly after loud sounds, and hear only the AGC.
/// let mut s = Settings::new(48_000);
/// s.agc.release_ms = 3000.0;
/// s.tap = Tap::AfterAgc;
/// let mut p = Pipeline::new(s)?;
/// let mut buffer = vec![0.0f32; 512];
/// p.process_in_place(&mut buffer);
///
/// // Out-of-range values are rejected, naming the setting.
/// s.agc.attack_ms = 0.0;
/// assert_eq!(
///     Pipeline::new(s).err(),
///     Some(Error::InvalidSetting { setting: Setting::AgcAttackMs })
/// );
/// # Ok::<(), rr_dr60::Error>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct AgcSettings {
    /// On (`true`, default) or bypassed. A-020: the device's AGC is assumed always active.
    pub enabled: bool,
    /// Target level in dBFS: a steady input at this level gets 0 dB of gain.
    /// Default −10.0 (A-017). Range −30.0 to 0.0.
    pub target_dbfs: f32,
    /// Maximum gain in dB. Default 40.0 (A-017). Range 0.0 to 60.0.
    pub max_gain_db: f32,
    /// Maximum attenuation in dB, as a positive number. Default 20.0 (A-017). Range 0.0 to 40.0.
    pub max_attenuation_db: f32,
    /// Attack time in ms: after an upward level step, the time for the output to settle within
    /// 2/27 of its dB excursion. Default 10.0 (A-018). Range 1.0 to 100.0.
    pub attack_ms: f32,
    /// Release time in ms, defined like the attack time for a downward step.
    /// Default 1000.0 (A-018). Range 50.0 to 10000.0.
    pub release_ms: f32,
}

impl AgcSettings {
    /// The assumed device values (A-017, A-018, A-020).
    pub const DEVICE: Self = Self {
        enabled: true,            // A-020
        target_dbfs: -10.0,       // A-017
        max_gain_db: 40.0,        // A-017
        max_attenuation_db: 20.0, // A-017
        attack_ms: 10.0,          // A-018
        release_ms: 1000.0,       // A-018
    };
}

impl Default for AgcSettings {
    /// [`AgcSettings::DEVICE`].
    fn default() -> Self {
        Self::DEVICE
    }
}

/// What happens to audio while the VAS is paused (spec 003 FR-004).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum VasMode {
    /// Removed from the output, so the output gets shorter. This is what the device does
    /// (S-001, A-008).
    #[default]
    Drop,
    /// Replaced by digital silence, so the output keeps the input's length. An emulator option
    /// for real-time hosts that must output one sample per input sample; not a device behavior.
    Mute,
}

/// Voice Activated System, signal-chain stage 5 (spec 003).
///
/// The owner's manual (S-001) says: "Recording automatically pauses when no sound is detected."
/// The device has no VAS switch and no VAS sensitivity control of its own; its threshold follows
/// the five-level microphone sensitivity setting (A-008, A-021). The emulator models this with
/// assumed values (A-022 – A-025) until captures from a real unit exist: a peak-level threshold,
/// a hang time before pausing, and an onset time that is lost when recording resumes.
///
/// # Output length
///
/// In [`VasMode::Drop`] (the default), paused audio is removed, so **a block can produce fewer
/// output samples than it consumed, or none**. Always use
/// [`BlockInfo::produced`](crate::BlockInfo::produced). [`VasMode::Mute`] keeps the length
/// and replaces paused audio with silence.
///
/// Ranges are inclusive; values outside them, NaN and ±Inf are rejected by
/// [`Pipeline::new`](crate::Pipeline::new) (spec 003 FR-012), even when the VAS is bypassed.
///
/// # Example
///
/// ```
/// use rr_dr60::{Pipeline, Settings, VasEvent, VasMode};
///
/// // Bypass the VAS (the spec 002 sound: every block produces as many samples as it consumed).
/// let mut s = Settings::new(48_000);
/// s.vas.enabled = false;
/// Pipeline::new(s)?;
///
/// // Keep the VAS, but replace pauses with silence instead of dropping them (fixed length),
/// // and record quieter sounds (sensitivity 5).
/// let mut s = Settings::new(48_000);
/// s.vas.mode = VasMode::Mute;
/// s.vas.sensitivity = 5;
/// let mut p = Pipeline::new(s)?;
///
/// // Process one block and read what it produced and where the VAS acted.
/// let input = vec![0.0f32; 4800];
/// let mut output = vec![0.0f32; 4800];
/// let mut events = vec![VasEvent::default(); p.max_events(input.len())];
/// let info = p.process_with_events(&input, &mut output, &mut events)?;
/// assert_eq!(info.produced, 4800);           // mute mode keeps the length
/// for event in &events[..info.events] {       // muted regions (splices in drop mode)
///     println!("muted {} samples at {}", event.input_length, event.output_position);
/// }
/// # Ok::<(), rr_dr60::Error>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct VasSettings {
    /// On (`true`, default) or bypassed. S-001, A-008: the device has no VAS switch.
    pub enabled: bool,
    /// [`VasMode::Drop`] (default) or [`VasMode::Mute`].
    pub mode: VasMode,
    /// Microphone sensitivity level, 1 to 5. Default 3 (S-001, A-021). In this version it moves
    /// only the VAS threshold: 3 dB per level, and higher levels record quieter sounds (A-022).
    pub sensitivity: u8,
    /// VAS threshold at sensitivity level 3, in dBFS (sine peak, AES17). Default −18.0 (A-022).
    /// Range −60.0 to 0.0.
    pub threshold_dbfs: f32,
    /// Hang time in ms: how long the input must stay below the threshold before recording
    /// pauses. Default 1000.0 (A-023). Range 50.0 to 10000.0.
    pub hang_ms: f32,
    /// Onset time in ms: how long a sound must last before a paused recording resumes; this
    /// much of the sound is lost. Default 20.0 (A-024). Range 0.0 to 200.0.
    pub onset_ms: f32,
}

impl VasSettings {
    /// The assumed device values (S-001, A-008, A-021 – A-024).
    pub const DEVICE: Self = Self {
        enabled: true,         // S-001, A-008
        mode: VasMode::Drop,   // S-001, A-008
        sensitivity: 3,        // S-001, A-021
        threshold_dbfs: -18.0, // A-022
        hang_ms: 1000.0,       // A-023
        onset_ms: 20.0,        // A-024
    };
}

impl Default for VasSettings {
    /// [`VasSettings::DEVICE`].
    fn default() -> Self {
        Self::DEVICE
    }
}

/// Pipeline configuration (data-model.md › Settings).
///
/// Settings are fixed for the life of a configuration. To change them, call
/// [`Pipeline::reconfigure`](crate::Pipeline::reconfigure), which is not real-time safe (FR-008).
///
/// Since 0.2.0, `Settings` is no longer `Eq` or `Hash`, because [`AgcSettings`] holds floats.
#[derive(Clone, Copy, Debug, PartialEq)]
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
    /// Signal-chain stage 3 (AGC) settings. Default [`AgcSettings::DEVICE`] (spec 002).
    pub agc: AgcSettings,
    /// Signal-chain stage 5 (VAS) settings. Default [`VasSettings::DEVICE`] (spec 003).
    pub vas: VasSettings,
}

impl Settings {
    /// Default settings for the given host rate: AGC and VAS on with the device values, both
    /// band-limit stages on, tap after playback, seed 0.
    ///
    /// The rate is not validated here. [`Pipeline::new`](crate::Pipeline::new) validates it.
    pub const fn new(host_rate_hz: u32) -> Self {
        Self {
            host_rate_hz,
            record_stage_enabled: true,   // A-002
            playback_stage_enabled: true, // A-002
            tap: Tap::AfterPlayback,
            seed: 0,                  // FR-009
            agc: AgcSettings::DEVICE, // A-017, A-018, A-020
            vas: VasSettings::DEVICE, // S-001, A-008, A-021 – A-024
        }
    }

    /// Checks these settings without creating a pipeline: the host rate first (FR-002), then
    /// every AGC field (spec 002 FR-011), then every VAS field (spec 003 FR-012). [`Pipeline::new`](crate::Pipeline::new) runs the
    /// same check. Never allocates.
    ///
    /// # Errors
    ///
    /// [`Error::UnsupportedHostRate`](crate::Error::UnsupportedHostRate) or
    /// [`Error::InvalidSetting`](crate::Error::InvalidSetting).
    pub fn validate(&self) -> Result<(), crate::Error> {
        crate::validate::validate(self)
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
    fn agc_defaults_cite_a017_a018_a020() {
        // 002 T004 / data-model.md › AgcSettings.
        let d = AgcSettings::DEVICE;
        assert!(d.enabled); // A-020
        assert_eq!(d.target_dbfs, -10.0); // A-017
        assert_eq!(d.max_gain_db, 40.0); // A-017
        assert_eq!(d.max_attenuation_db, 20.0); // A-017
        assert_eq!(d.attack_ms, 10.0); // A-018
        assert_eq!(d.release_ms, 1000.0); // A-018
        assert_eq!(AgcSettings::default(), AgcSettings::DEVICE);
        assert_eq!(Settings::new(48_000).agc, AgcSettings::DEVICE);
    }

    #[test]
    fn tap_after_agc_exists_and_default_is_unchanged() {
        assert_ne!(Tap::AfterAgc, Tap::AfterPlayback);
        assert_eq!(Tap::default(), Tap::AfterPlayback);
    }

    #[test]
    fn vas_defaults_cite_s001_a008_a021_to_a024() {
        // 003 T003 / data-model.md › VasSettings.
        let d = VasSettings::DEVICE;
        assert!(d.enabled); // S-001, A-008: no VAS switch on the device
        assert_eq!(d.mode, VasMode::Drop); // S-001, A-008
        assert_eq!(d.sensitivity, 3); // S-001, A-021
        assert_eq!(d.threshold_dbfs, -18.0); // A-022
        assert_eq!(d.hang_ms, 1000.0); // A-023
        assert_eq!(d.onset_ms, 20.0); // A-024
        assert_eq!(VasSettings::default(), VasSettings::DEVICE);
        assert_eq!(VasMode::default(), VasMode::Drop);
        assert_eq!(Settings::new(48_000).vas, VasSettings::DEVICE);
    }

    #[test]
    fn tap_after_vas_exists_and_default_is_unchanged() {
        assert_ne!(Tap::AfterVas, Tap::AfterPlayback);
        assert_eq!(Tap::default(), Tap::AfterPlayback);
    }

    #[test]
    fn new_does_not_validate() {
        // Validation happens in Pipeline::new / reconfigure (data-model.md).
        assert_eq!(Settings::new(22_050).host_rate_hz, 22_050);
    }
}
