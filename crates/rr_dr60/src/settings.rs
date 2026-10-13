//! Pipeline settings, defaults and supported rates (data-model.md › Settings).

/// Host sample rates supported by this version, in Hz (spec 004 FR-002).
///
/// 6 kHz is the device's own rate (A-026) and the identity rate: no rate conversion is applied.
/// The other six rates convert to and from the device rate at the pipeline boundary; 8 and
/// 16 kHz are ordinary conversions (3:4 and 3:8), not the identity. Any other rate is rejected
/// by [`Pipeline::new`](crate::Pipeline::new) with
/// [`Error::UnsupportedHostRate`](crate::Error::UnsupportedHostRate).
pub const SUPPORTED_HOST_RATES: [u32; 7] = [6000, 8000, 16000, 44100, 48000, 88200, 96000]; // 004 FR-002

/// The emulated device's internal sample rate, in Hz.
///
/// A-026 (S-002, verified): the RR-DR60 samples at 6 kHz, so nothing above 3 kHz exists inside
/// the device. Supersedes A-001's assumed 8 kHz (spec 004).
pub const DEVICE_RATE_HZ: u32 = 6000;

/// Where the pipeline output is taken (FR-007; spec 002 FR-003; spec 005 FR-001).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Tap {
    /// After signal-chain stage 3 (the limiter), **at the host rate**: the limiter's output
    /// sample for sample. The rate conversion, the band-limit stages, the ADC clamp and the VAS
    /// are not run, whatever their settings; latency is 0 and no VAS events occur (spec 005
    /// FR-001; since 0.5 this tap is before the rate conversion, where the limiter runs).
    AfterAgc,
    /// After signal-chain stage 4 (record band-limit) and the ADC's full-scale clamp (spec 005
    /// FR-008). Stages 5 (VAS) and 10 are not run.
    AfterRecord,
    /// After signal-chain stage 5 (VAS). Stage 10 is not run, whatever its setting
    /// (spec 003 FR-003). In drop mode the output can be shorter than the input.
    AfterVas,
    /// After signal-chain stage 10 (playback band-limit). The default, because the device
    /// band-limits on both record and playback (A-002).
    #[default]
    AfterPlayback,
    /// After signal-chain stage 9 (the speech decoder), before stage 10: "the codec's output"
    /// (spec 008 FR-015). The speech encoder and decoder run in line with their 180-sample
    /// delay; stage 10 does not. With the speech codec bypassed this tap equals
    /// [`Tap::AfterVas`]. Appended in 0.8.
    AfterDecoder,
}

/// Record-path automatic gain control, signal-chain stage 3: a **limiter** (spec 005; replaces
/// the spec 002 compressor).
///
/// Modeled on the limiter in the service manual's schematic (S-002; A-028 adopted): a shunt
/// transistor driven by a peak detector on the codec's input. Below the knee the stage is the
/// identity, bit for bit (quiet input is **not** raised and nothing happens in pauses); above
/// it the output is held at a ceiling at the knee level, with a dead time of a few
/// milliseconds on a loud onset (the whole onset passes at full level first, so the codec
/// clips it), a shorter dead time for a few seconds after a loud sound, and a release of
/// about 1.4 s. The component values are the schematic's; the transistor parameters are
/// estimates (A-028, A-034 – A-036, `docs/hardware/assumptions.md`), so the dead time, the
/// release and the knee carry low confidence until captures from a real unit exist. The stage
/// runs at the host rate, before the rate conversion, so content above the device's band also
/// drives it (spec 005 FR-007).
///
/// # Output level
///
/// 0 dBFS (a sine of peak amplitude 1.0, AES17) is the codec's full scale (A-030). Sustained
/// input above the knee comes out at about −1 dBFS, just below full scale, so only the dead
/// time of a loud onset reaches the ADC clamp ([`Settings::adc_clamp_enabled`]). Inputs above
/// 0 dBFS are within the limiter's normal operating range. Where speech sits against the knee
/// depends on the mic chain's gain, which is the least certain number in the register (A-029:
/// roughly 10–35 dB below full scale at a conversational distance), so the limiter acts on
/// loud sounds and, at the high end of that estimate, on the peaks of ordinary speech.
///
/// Ranges are inclusive; values outside them, NaN and ±Inf are rejected by
/// [`Pipeline::new`](crate::Pipeline::new) (spec 002 FR-011, kept by spec 005), even when the
/// stage is bypassed.
///
/// # Example
///
/// ```
/// use rr_dr60::{Error, Pipeline, Setting, Settings, Tap};
///
/// // Bypass the limiter (the spec 001 sound: band-limiting only).
/// let mut s = Settings::new(48_000);
/// s.agc.enabled = false;
/// Pipeline::new(s)?;
///
/// // Move the knee down so ordinary speech reaches it, and hear only the limiter.
/// let mut s = Settings::new(48_000);
/// s.agc.knee_dbfs = -6.0;
/// s.tap = Tap::AfterAgc;
/// let mut p = Pipeline::new(s)?;
/// assert_eq!(p.latency_samples(), 0);
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
    /// The knee in dBFS: the input level at which the limiter begins to act, and the level its
    /// output is held at above it; modeled at −1 dBFS (A-028, ±2 dB: the detector's steady-state
    /// threshold puts the ceiling just below the codec's full scale). Default −1.0. Range −40.0
    /// to 3.0 (above about +4.7 dBFS the detector could never conduct, spec 005 research R-05).
    pub knee_dbfs: f32,
    /// Attack, in ms, as the **dead time** at 10 dB over the knee: how long a loud onset passes
    /// at full level before the limiter acts (A-034). Longer at smaller overdrives (about 20–25 ms
    /// at 2 dB over the knee), shorter (≤ 0.5 ms) for a few seconds after a loud sound.
    /// Default 3.0. Range 1.0 to 50.0 (the circuit floors it near 1 ms, research R-05).
    pub attack_ms: f32,
    /// Release, in ms: the time to return within 1 dB of unity from 10 dB of attenuation
    /// (A-035), fast at first and slower near unity. Default 1370.0. Range 200.0 to 10000.0.
    /// The arming memory (how long the shorter dead time lasts) scales with it.
    pub release_ms: f32,
}

impl AgcSettings {
    /// The modeled device values (A-020, A-028, A-034, A-035).
    pub const DEVICE: Self = Self {
        enabled: true,      // A-020
        knee_dbfs: -1.0,    // A-028: the knee just below the codec's full scale
        attack_ms: 3.0,     // A-034: dead time at 10 dB over the knee
        release_ms: 1370.0, // A-035: to −1 dB from 10 dB of attenuation
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
    /// VAS threshold at sensitivity level 3, in dBFS (sine peak, AES17). Default −40.0 (A-022,
    /// revised by spec 005: about 40 dB below the limiter's knee, around the level of quiet
    /// speech at a conversational distance, A-029, low confidence). Range −60.0 to 0.0. Below the limiter's knee the
    /// threshold is input-referred one to one (spec 005 FR-012).
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
        threshold_dbfs: -40.0, // A-022 (revised by spec 005): −40 dBFS at level 3, 3 dB per step
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

/// The modeled memory's capacity (spec 006 FR-010; data-model.md › Settings): recorded device
/// time and number of recordings, both counted the way the device reports them (A-038) and
/// both configurable at construction. The store for the full capacity is reserved when the
/// pipeline is created or reconfigured (FR-017), so starting a recording never allocates.
///
/// The defaults are the device's figures: 60 minutes and 99 items (S-001). Figures above them
/// are allowed and are **not the device's** (an expanded-memory configuration). A value of 0
/// is the spec's "capacity too small" refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct MemorySettings {
    /// Capacity in seconds of recorded device time. Default 3600 (S-001: 60 minutes). Range 1 to
    /// 86 400 (24 hours; engineering target, 006 FR-010).
    pub capacity_s: u32,
    /// The most recordings the memory holds. Default 99 (S-001). Range 1 to 9 999 (engineering
    /// target, 006 FR-010).
    pub max_recordings: u32,
}

impl MemorySettings {
    /// The device's figures (S-001: 60 minutes, 99 items).
    pub const DEVICE: Self = Self {
        capacity_s: 3600,   // S-001: 60 minutes
        max_recordings: 99, // S-001: 99 items
    };
}

impl Default for MemorySettings {
    /// [`MemorySettings::DEVICE`].
    fn default() -> Self {
        Self::DEVICE
    }
}

/// The playback output route (spec 006 FR-015): the speaker, or the 2.5 mm earphone jack. The
/// routes are exclusive, as on the device (S-001: no sound from the speaker while an earphone
/// is connected). It is **not** a setting: the host selects it at any time, during playback
/// too, with `Pipeline::set_route`, and it survives `reset` and `reconfigure`.
///
/// In this version the two routes produce identical output. Signal-chain stages 11–13 (the
/// volume control and power amplifier, the speaker, the earphone follower; A-031, A-010,
/// A-032) are not yet modeled; the route exists so that those features do not change the
/// interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Route {
    /// The built-in speaker (the default: nothing plugged in).
    #[default]
    Speaker,
    /// The earphone jack (S-001; its path bypasses the power amplifier, A-032).
    Earphone,
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
    /// The ADC's full-scale saturation after stage 4: output beyond ±1.0 is clamped to ±1.0
    /// (`true`, default; A-030: the codec cannot record more than full scale) or passed
    /// unchanged. The limiter holds sustained loud input about 1 dB below full scale, so with
    /// both on the codec clips only the dead time of a loud onset (spec 005 FR-008). Not applied
    /// at [`Tap::AfterAgc`].
    pub adc_clamp_enabled: bool,
    /// Output tap point. Default [`Tap::AfterPlayback`].
    pub tap: Tap,
    /// Seed for stochastic stages (FR-009). No stage is stochastic in this version, so the
    /// seed has no effect on output.
    pub seed: u64,
    /// Signal-chain stage 3 (AGC) settings. Default [`AgcSettings::DEVICE`] (spec 002).
    pub agc: AgcSettings,
    /// Signal-chain stage 5 (VAS) settings. Default [`VasSettings::DEVICE`] (spec 003).
    pub vas: VasSettings,
    /// The modeled memory's capacity. Default [`MemorySettings::DEVICE`] (spec 006).
    pub memory: MemorySettings,
    /// Signal-chain stages 7 and 9, the **speech codec** (encoder and decoder together), on
    /// (`true`, default: the device has no bypass; A-004 adopted by spec 008) or bypassed. On, a
    /// recording is stored as 4 kbit/s frames and the one-pass processing runs encoder and
    /// decoder in line with a 180-sample delay; bypassed, the engine stores and plays the
    /// post-VAS device-rate samples as in 0.7, a combination the device cannot produce. The
    /// setting also fixes the memory's encoding (spec 008 FR-002, FR-012): changing it with
    /// recordings present is refused. Modeled on a 4 kbit/s CELP-class coder of the period, not
    /// on the device's own coder (A-040, A-041).
    pub speech_codec_enabled: bool,
}

impl Settings {
    /// Default settings for the given host rate: the limiter and the VAS on with the device
    /// values, both band-limit stages and the ADC clamp on, tap after playback, seed 0, the
    /// memory at the device's 60 minutes and 99 recordings.
    ///
    /// The rate is not validated here. [`Pipeline::new`](crate::Pipeline::new) validates it.
    pub const fn new(host_rate_hz: u32) -> Self {
        Self {
            host_rate_hz,
            record_stage_enabled: true,   // A-002
            playback_stage_enabled: true, // A-002
            adc_clamp_enabled: true,      // A-030
            tap: Tap::AfterPlayback,
            seed: 0,                        // FR-009
            agc: AgcSettings::DEVICE,       // A-020, A-028, A-034, A-035
            vas: VasSettings::DEVICE,       // S-001, A-008, A-021 – A-024
            memory: MemorySettings::DEVICE, // S-001: 60 min, 99 items (006 FR-010)
            speech_codec_enabled: true,     // A-004 adopted (008 FR-002): the device has no bypass
        }
    }

    /// Checks these settings without creating a pipeline: the host rate first (FR-002), then
    /// every AGC field (spec 002 FR-011; knee, attack, release since spec 005), then every VAS
    /// field (spec 003 FR-012), then the memory figures (spec 006 FR-010).
    /// [`Pipeline::new`](crate::Pipeline::new) runs the same check. Never allocates.
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
    extern crate std;
    use super::*;
    use std::string::ToString;

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

    /// 004 T004 (spec 004 FR-001, FR-002; contracts/rust-api.md): seven host rates with 6 kHz
    /// first, and the measured 6 kHz device rate (A-026, S-002).
    #[test]
    fn supported_rates_and_device_rate() {
        let rates: [u32; 7] = SUPPORTED_HOST_RATES; // the type is part of the contract
        assert_eq!(rates, [6000, 8000, 16000, 44100, 48000, 88200, 96000]); // 004 FR-002
        assert_eq!(DEVICE_RATE_HZ, 6000); // A-026
        assert_eq!(
            SUPPORTED_HOST_RATES[0], DEVICE_RATE_HZ,
            "the identity rate is listed"
        );
    }

    /// 004 T004 (spec 004 FR-002): 6 kHz validates; other rates are rejected with an error that
    /// names all seven supported rates.
    #[test]
    fn host_rate_6k_accepted_and_others_rejected() {
        assert_eq!(Settings::new(6000).validate(), Ok(()));
        for bad in [7000, 11_025, 32_000] {
            let err = Settings::new(bad)
                .validate()
                .expect_err("unsupported rate must be rejected");
            assert_eq!(
                err,
                crate::Error::UnsupportedHostRate { requested: bad },
                "{bad}"
            );
            let text = err.to_string();
            for rate in SUPPORTED_HOST_RATES {
                assert!(text.contains(&rate.to_string()), "{text} is missing {rate}");
            }
        }
    }

    /// 005 T004 (spec 005 FR-002, FR-010; contracts/rust-api.md; data-model › Settings): the
    /// limiter's device values. Replaces the spec 002 compressor defaults (A-017, A-018
    /// superseded).
    #[test]
    fn agc_defaults_cite_a020_a028_a034_a035() {
        let d = AgcSettings::DEVICE;
        assert!(d.enabled); // A-020
        assert_eq!(d.knee_dbfs, -1.0); // A-028: the knee just below the codec's full scale
        assert_eq!(d.attack_ms, 3.0); // A-034: dead time at 10 dB over the knee
        assert_eq!(d.release_ms, 1370.0); // A-035: to −1 dB from 10 dB of attenuation
        assert_eq!(AgcSettings::default(), AgcSettings::DEVICE);
        assert_eq!(Settings::new(48_000).agc, AgcSettings::DEVICE);
    }

    /// 005 T004 (spec 005 FR-008; A-030): the ADC's full-scale clamp is its own setting, on by
    /// default.
    #[test]
    fn adc_clamp_is_on_by_default() {
        assert!(Settings::new(48_000).adc_clamp_enabled); // A-030
        assert!(Settings::new(6000).adc_clamp_enabled);
    }

    #[test]
    fn tap_after_agc_exists_and_default_is_unchanged() {
        assert_ne!(Tap::AfterAgc, Tap::AfterPlayback);
        assert_eq!(Tap::default(), Tap::AfterPlayback);
    }

    #[test]
    fn vas_defaults_cite_s001_a008_a021_to_a024() {
        // 003 T003 / data-model.md › VasSettings; the threshold restated by 005 T004 (FR-012).
        let d = VasSettings::DEVICE;
        assert!(d.enabled); // S-001, A-008: no VAS switch on the device
        assert_eq!(d.mode, VasMode::Drop); // S-001, A-008
        assert_eq!(d.sensitivity, 3); // S-001, A-021
        assert_eq!(d.threshold_dbfs, -40.0); // A-022 (revised by spec 005): −40 dBFS at level 3
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

    /// 006 T011 (spec 006 FR-010; contracts/rust-api.md): the memory's device figures, 60 min
    /// and 99 items (S-001), are the defaults; the route defaults to the speaker.
    #[test]
    fn memory_defaults_cite_s001_and_route_defaults_to_speaker() {
        let d = MemorySettings::DEVICE;
        assert_eq!(d.capacity_s, 3600); // S-001: 60 minutes
        assert_eq!(d.max_recordings, 99); // S-001: 99 items
        assert_eq!(MemorySettings::default(), MemorySettings::DEVICE);
        assert_eq!(Settings::new(48_000).memory, MemorySettings::DEVICE);
        assert_eq!(Settings::new(6000).memory, MemorySettings::DEVICE);
        assert_eq!(Route::default(), Route::Speaker); // S-001: the earphone is plugged in
        assert_ne!(Route::Speaker, Route::Earphone);
    }

    #[test]
    fn new_does_not_validate() {
        // Validation happens in Pipeline::new / reconfigure (data-model.md).
        assert_eq!(Settings::new(22_050).host_rate_hz, 22_050);
    }
}
