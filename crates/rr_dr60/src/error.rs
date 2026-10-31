//! Error type for configuration and processing (contracts/rust-api.md).

use core::fmt;

use crate::settings::SUPPORTED_HOST_RATES;

/// Errors from configuration and processing.
///
/// Since 0.9.0, `Error` is no longer `Eq`, because [`Error::InvalidVolume`] carries the rejected
/// `f32` position.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// The requested host rate is not in [`SUPPORTED_HOST_RATES`] (FR-002).
    UnsupportedHostRate {
        /// The rejected rate, in Hz.
        requested: u32,
    },
    /// `Pipeline::process` was given input and output slices of different lengths (FR-003).
    LengthMismatch {
        /// Input length in samples.
        input: usize,
        /// Output length in samples.
        output: usize,
    },
    /// A setting is outside its valid range, or is NaN or ±Inf (spec 002 FR-011, spec 003
    /// FR-012, spec 006 FR-010: a memory capacity of 0 is the "capacity too small" refusal;
    /// spec 012 FR-003: the hiss offset, checked whether or not the capsule is on).
    /// Returned by `Pipeline::new` and `Pipeline::reconfigure`; a failed reconfigure changes
    /// nothing.
    InvalidSetting {
        /// The offending setting.
        setting: Setting,
    },
    /// A recording is in progress, so the call is refused (spec 006 FR-003, FR-013, FR-019):
    /// starting a recording or a playback, the one-pass processing, erase, import, reconfigure.
    RecordingInProgress,
    /// A playback is in progress, so the call is refused (spec 006 FR-003, FR-019): starting a
    /// recording or a playback, the one-pass processing, reconfigure.
    PlaybackInProgress,
    /// `record` or `stop_recording` while no recording is in progress (spec 006 Edge Cases;
    /// also after a recording stopped by itself at the memory's limit).
    NotRecording,
    /// `play` or `stop_playback` while no playback is in progress (spec 006 Edge Cases).
    NotPlaying,
    /// No recorded time remains, so a recording cannot start (spec 006 FR-012; S-001: "FULL").
    MemoryFull,
    /// The recording count is at its capacity, so a recording cannot start (spec 006 FR-012;
    /// S-001: 99 items).
    NoRecordingSlots,
    /// No recording has this number: 0, or above the count (spec 006 FR-013, Edge Cases).
    NoSuchRecording {
        /// The requested number.
        number: u32,
    },
    /// An import exceeds the remaining time or the remaining recording slots; the memory is
    /// unchanged (spec 006 FR-014).
    DoesNotFit,
    /// The bytes are not an exported recording of this engine (no magic, or too short to carry
    /// one; spec 006 FR-014).
    UnknownForm,
    /// The bytes carry a form version (or flags) this engine does not read: version 1, spec
    /// 006's never-shipped samples form, and any later version are refused (spec 008 FR-011;
    /// spec 006 FR-014, US3 scenario 5).
    UnsupportedFormVersion {
        /// The form version found.
        version: u16,
    },
    /// The bytes are an exported recording but are truncated, altered or inconsistent (length,
    /// count, device rate, CRC-32, or a sample that is not finite or is subnormal; spec 006
    /// FR-014).
    Damaged,
    /// `reconfigure` to a configuration the stored recordings do not fit: a capacity smaller
    /// than what the memory holds (time or count; spec 006 FR-019), or the other speech-codec
    /// setting while recordings are present (one memory holds one encoding; spec 008 FR-012).
    /// The engine is unchanged.
    StoredExceedsCapacity,
    /// The recording store for the configured capacity could not be reserved (spec 006 FR-017;
    /// an error, not an abort).
    AllocationFailed,
    /// `export` into a buffer shorter than the recording's byte form; nothing was written
    /// (spec 006 FR-014).
    BufferTooSmall {
        /// The length the buffer needs (`export_len`).
        needed: usize,
    },
    /// An import whose encoding (speech-codec frames or device-rate samples) differs from the
    /// engine's configured one; one memory holds one encoding (spec 008 FR-012). The memory is
    /// unchanged.
    EncodingDiffers,
    /// The bytes carry speech-codec frames of a model version this release cannot decode
    /// (spec 008 FR-010, FR-011: before 1.0 a dropped model version is refused; from 1.0 every
    /// released one keeps its decoder).
    UnsupportedCodecModelVersion {
        /// The model version found: major.
        major: u16,
        /// Minor.
        minor: u16,
        /// Patch.
        patch: u16,
    },
    /// `set_volume` with a position outside 0 … 1 or not finite (spec 009 FR-003); the position
    /// is unchanged.
    InvalidVolume {
        /// The rejected position.
        position: f32,
    },
    /// `set_playback_speed` or `playback_length_samples` with a level other than 1 … 5 (spec 013
    /// FR-001); the level is unchanged.
    InvalidSpeedLevel {
        /// The rejected level.
        level: u8,
    },
    /// A processing call with a named route tap ([`Tap::AfterSpeaker`](crate::Tap::AfterSpeaker)
    /// or [`Tap::AfterEarphone`](crate::Tap::AfterEarphone)) while the other route is selected
    /// (spec 009 FR-011); nothing was processed. The route output tap
    /// ([`Tap::RouteOutput`](crate::Tap::RouteOutput)) follows the route and is never refused.
    TapNotOnRoute {
        /// The configured tap.
        tap: crate::Tap,
        /// The selected route.
        route: crate::Route,
    },
    /// `schedule_change` on a stream whose schedule already holds
    /// [`SCHEDULE_CAPACITY`](crate::changes::SCHEDULE_CAPACITY) pending changes (spec 015
    /// FR-002); nothing stored.
    ScheduleFull {
        /// The stream.
        stream: crate::Stream,
    },
    /// `schedule_change` with a control that is not a live control of the stream (spec 015
    /// FR-001): a control manifest index that is not the route, the volume or the playback
    /// speed, or the playback speed on the one-pass stream; nothing stored.
    ControlNotOnStream {
        /// The control manifest index given.
        control: u32,
        /// The stream.
        stream: crate::Stream,
    },
    /// A call on a reserved stream value (spec 015 FR-001). Declared for future reserved
    /// [`Stream`](crate::Stream) values; **no call returns it since 0.17**, where the record
    /// stream carries the record-side values (spec 017).
    ReservedStream {
        /// The stream.
        stream: crate::Stream,
    },
    /// `schedule_change` with a value the control cannot take (spec 015 FR-001): a route other
    /// than 0 or 1, or a non-integral level; nothing stored. (A volume outside 0 … 1 is
    /// [`Error::InvalidVolume`], a level outside 1 … 5 [`Error::InvalidSpeedLevel`].)
    InvalidControlValue {
        /// The control manifest index.
        control: u32,
        /// The rejected value.
        value: f64,
    },
    /// A position read in a clock the stream does not have (spec 017 D2):
    /// `stream_position(Stream::Record)` (the recorder has no output clock) or
    /// `input_position(Stream::Playback)` (the player has no input clock); nothing changes.
    NoSuchClock {
        /// The stream.
        stream: crate::Stream,
    },
    /// A control manifest index at or past [`CONTROL_COUNT`](crate::controls::CONTROL_COUNT) (spec 017:
    /// `changes::clock_of`).
    NoSuchControl {
        /// The index given.
        control: u32,
    },
    /// `set_battery_volts` or a scheduled battery change with a value outside
    /// [`BATTERY_VOLTS_RANGE`](crate::battery::BATTERY_VOLTS_RANGE) or not finite (spec 018
    /// FR-001); the value is unchanged, nothing stored.
    InvalidBatteryVolts {
        /// The rejected value.
        volts: f32,
    },
}

/// Inclusive range of `memory.capacity_s`, in seconds: one second to 24 hours (engineering
/// target, spec 006 FR-010; the device's 3600 s, S-001, sits inside it).
pub(crate) const MEMORY_CAPACITY_S_RANGE: (u32, u32) = (1, 86_400); // engineering target (006 FR-010)
/// Inclusive range of `memory.max_recordings` (engineering target, spec 006 FR-010; the
/// device's 99, S-001, sits inside it).
pub(crate) const MEMORY_MAX_RECORDINGS_RANGE: (u32, u32) = (1, 9_999); // engineering target (006 FR-010)

/// Names a setting in [`Error::InvalidSetting`]. `Display` gives the field name and its valid
/// range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Setting {
    /// `agc.knee_dbfs`, valid −40 to 3 dBFS (spec 005; replaces the spec 002 target level).
    AgcKneeDbfs,
    /// `agc.attack_ms` (the dead time at 10 dB over the knee), valid 1 to 50 ms (spec 005).
    AgcAttackMs,
    /// `agc.release_ms` (to −1 dB from 10 dB of attenuation), valid 200 to 10000 ms (spec 005).
    AgcReleaseMs,
    /// `vas.sensitivity`, valid 1 to 5 (spec 003).
    VasSensitivity,
    /// `vas.threshold_dbfs`, valid −60 to 0 dBFS (spec 003).
    VasThresholdDbfs,
    /// `vas.hang_ms`, valid 50 to 10000 ms (spec 003).
    VasHangMs,
    /// `vas.onset_ms`, valid 0 to 200 ms (spec 003).
    VasOnsetMs,
    /// `memory.capacity_s`, valid 1 to 86 400 s (spec 006).
    MemoryCapacityS,
    /// `memory.max_recordings`, valid 1 to 9 999 (spec 006).
    MemoryMaxRecordings,
    /// `mic.hiss_offset_db`, the hiss level offset, valid −18 to +12 dB (spec 012 FR-003).
    /// Checked whether or not the capsule is on. Appended in 0.12.
    MicHissOffsetDb,
}

impl Setting {
    /// Inclusive valid range (005 and 003 data-model.md; engineering targets, 005 FR-010 and
    /// 003 FR-017). The AGC bounds come from the circuit model (005 research R-05): above
    /// about +4.7 dBFS the detector could never conduct because the codec amp's rail bounds its
    /// overdrive, so the knee is capped at +3 with its ±2 dB tolerance inside the usable range;
    /// the R12 charge-current limit floors the realizable dead time near 1.1 ms, so the attack
    /// starts at 1 ms.
    pub(crate) const fn range(self) -> (f32, f32) {
        match self {
            Setting::AgcKneeDbfs => (-40.0, 3.0), // engineering target (005 FR-010; R-05); A-028
            Setting::AgcAttackMs => (1.0, 50.0),  // engineering target (005 FR-010; R-05); A-034
            Setting::AgcReleaseMs => (200.0, 10_000.0), // engineering target (005 FR-010); A-035
            Setting::VasSensitivity => (1.0, 5.0), // S-001, A-021: five levels
            Setting::VasThresholdDbfs => (-60.0, 0.0), // engineering target (003 FR-012)
            Setting::VasHangMs => (50.0, 10_000.0), // engineering target (003 FR-012)
            Setting::VasOnsetMs => (0.0, 200.0),  // engineering target (003 FR-012)
            Setting::MemoryCapacityS => (
                MEMORY_CAPACITY_S_RANGE.0 as f32,
                MEMORY_CAPACITY_S_RANGE.1 as f32,
            ), // engineering target (006 FR-010)
            Setting::MemoryMaxRecordings => (
                MEMORY_MAX_RECORDINGS_RANGE.0 as f32,
                MEMORY_MAX_RECORDINGS_RANGE.1 as f32,
            ), // engineering target (006 FR-010)
            Setting::MicHissOffsetDb => crate::mic::HISS_OFFSET_RANGE_DB, // A-048 (012 FR-003)
        }
    }

    pub(crate) const fn name_and_unit(self) -> (&'static str, &'static str) {
        match self {
            Setting::AgcKneeDbfs => ("agc.knee_dbfs", "dBFS"),
            Setting::AgcAttackMs => ("agc.attack_ms", "ms"),
            Setting::AgcReleaseMs => ("agc.release_ms", "ms"),
            Setting::VasSensitivity => ("vas.sensitivity", ""),
            Setting::VasThresholdDbfs => ("vas.threshold_dbfs", "dBFS"),
            Setting::VasHangMs => ("vas.hang_ms", "ms"),
            Setting::VasOnsetMs => ("vas.onset_ms", "ms"),
            Setting::MemoryCapacityS => ("memory.capacity_s", "s"),
            Setting::MemoryMaxRecordings => ("memory.max_recordings", ""),
            Setting::MicHissOffsetDb => ("mic.hiss_offset_db", "dB"),
        }
    }
}

impl fmt::Display for Setting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (name, unit) = self.name_and_unit();
        let (lo, hi) = self.range();
        if unit.is_empty() {
            write!(f, "{name} (valid range {lo} to {hi})")
        } else {
            write!(f, "{name} (valid range {lo} to {hi} {unit})")
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Error::UnsupportedHostRate { requested } => {
                write!(
                    f,
                    "unsupported host sample rate {requested} Hz; supported rates are "
                )?;
                for (i, rate) in SUPPORTED_HOST_RATES.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{rate}")?;
                }
                f.write_str(" Hz")
            }
            Error::LengthMismatch { input, output } => write!(
                f,
                "input and output lengths differ: input has {input} samples, output has {output}"
            ),
            Error::InvalidSetting { setting } => {
                write!(f, "invalid setting: {setting} must be finite and in range")
            }
            Error::RecordingInProgress => f.write_str("recording in progress"),
            Error::PlaybackInProgress => f.write_str("playback in progress"),
            Error::NotRecording => f.write_str("not recording"),
            Error::NotPlaying => f.write_str("not playing"),
            Error::MemoryFull => f.write_str("memory full: no recorded time remains"),
            Error::NoRecordingSlots => f.write_str("no recording slots left"),
            Error::NoSuchRecording { number } => write!(f, "no such recording: {number}"),
            Error::DoesNotFit => {
                f.write_str("does not fit: the recording exceeds the remaining time or slots")
            }
            Error::UnknownForm => f.write_str("unknown form: not an exported recording"),
            Error::UnsupportedFormVersion { version } => {
                write!(f, "unsupported form version {version}")
            }
            Error::Damaged => {
                f.write_str("damaged: the exported recording is truncated, altered or inconsistent")
            }
            Error::StoredExceedsCapacity => {
                f.write_str("stored recordings do not fit the new configuration")
            }
            Error::AllocationFailed => {
                f.write_str("the recording store for the configured capacity could not be reserved")
            }
            Error::BufferTooSmall { needed } => {
                write!(f, "buffer too small: the export needs {needed} bytes")
            }
            Error::EncodingDiffers => {
                f.write_str("encoding differs from the engine's configuration")
            }
            Error::UnsupportedCodecModelVersion {
                major,
                minor,
                patch,
            } => write!(f, "unsupported codec model version {major}.{minor}.{patch}"),
            Error::InvalidVolume { position } => write!(
                f,
                "invalid volume position {position}: must be finite and within 0 … 1"
            ),
            Error::TapNotOnRoute { tap, route } => {
                write!(f, "tap {tap:?} is not on the selected route {route:?}")
            }
            Error::InvalidSpeedLevel { level } => {
                write!(f, "invalid playback speed level {level}: expected 1 to 5")
            }
            Error::ScheduleFull { stream } => {
                write!(
                    f,
                    "the {stream:?} stream's schedule is full (64 pending changes)"
                )
            }
            Error::ControlNotOnStream { control, stream } => {
                write!(
                    f,
                    "control {control} is not a live control of the {stream:?} stream"
                )
            }
            Error::ReservedStream { stream } => {
                write!(f, "the {stream:?} stream is reserved")
            }
            Error::InvalidControlValue { control, value } => {
                write!(f, "invalid value {value} for control {control}")
            }
            Error::NoSuchClock { stream } => {
                write!(f, "the {stream:?} stream has no such clock")
            }
            Error::NoSuchControl { control } => {
                write!(f, "no such control: {control}")
            }
            Error::InvalidBatteryVolts { volts } => write!(
                f,
                "invalid battery value {volts} V: must be finite and within 2.2 … 3.0"
            ),
        }
    }
}

impl core::error::Error for Error {}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::string::ToString;

    #[test]
    fn unsupported_rate_lists_supported_rates() {
        let text = Error::UnsupportedHostRate { requested: 22_050 }.to_string();
        assert!(text.contains("22050"), "{text}");
        for rate in crate::settings::SUPPORTED_HOST_RATES {
            assert!(text.contains(&rate.to_string()), "{text} is missing {rate}");
        }
    }

    #[test]
    fn length_mismatch_shows_both_lengths() {
        let text = Error::LengthMismatch {
            input: 64,
            output: 32,
        }
        .to_string();
        assert!(text.contains("64") && text.contains("32"), "{text}");
    }

    /// 006 T012 (spec 006 research R-05; data-model › Errors): every transport, memory and form
    /// refusal has a `Display` that names the reason in the spec's words and includes its field.
    #[test]
    fn transport_errors_display_the_reason_and_their_field() {
        let cases: [(Error, &[&str]); 19] = [
            (Error::RecordingInProgress, &["recording in progress"]),
            (Error::PlaybackInProgress, &["playback in progress"]),
            (Error::NotRecording, &["not recording"]),
            (Error::NotPlaying, &["not playing"]),
            (Error::MemoryFull, &["memory full"]),
            (Error::NoRecordingSlots, &["no recording slots left"]),
            (
                Error::NoSuchRecording { number: 7 },
                &["no such recording", "7"],
            ),
            (Error::DoesNotFit, &["does not fit"]),
            (Error::UnknownForm, &["unknown form"]),
            (
                Error::UnsupportedFormVersion { version: 2 },
                &["unsupported form version", "2"],
            ),
            (Error::Damaged, &["damaged"]),
            (
                Error::StoredExceedsCapacity,
                &["stored recordings do not fit the new configuration"],
            ),
            (Error::AllocationFailed, &["could not be reserved"]),
            (
                Error::BufferTooSmall { needed: 1234 },
                &["buffer too small", "1234"],
            ),
            // Spec 008 FR-010, FR-012.
            (Error::EncodingDiffers, &["encoding differs"]),
            (
                Error::UnsupportedCodecModelVersion {
                    major: 0,
                    minor: 7,
                    patch: 0,
                },
                &["unsupported codec model version", "0.7.0"],
            ),
            // Spec 009 FR-003, FR-011.
            (
                Error::InvalidVolume { position: 1.5 },
                &["invalid volume position", "1.5", "within 0 … 1"],
            ),
            (
                Error::TapNotOnRoute {
                    tap: crate::Tap::AfterSpeaker,
                    route: crate::Route::Earphone,
                },
                &["is not on the selected route", "AfterSpeaker", "Earphone"],
            ),
            // Spec 013 FR-001.
            (
                Error::InvalidSpeedLevel { level: 6 },
                &["invalid playback speed level", "6", "1 to 5"],
            ),
        ];
        let mut texts = std::vec::Vec::new();
        for (error, words) in cases {
            let text = error.to_string();
            for w in words {
                assert!(text.contains(w), "{error:?}: {text:?} lacks {w:?}");
            }
            assert!(!texts.contains(&text), "duplicate message {text:?}");
            texts.push(text);
        }
        // The two memory settings display their names and ranges (006 FR-010).
        let text = Error::InvalidSetting {
            setting: Setting::MemoryCapacityS,
        }
        .to_string();
        assert!(
            text.contains("memory.capacity_s") && text.contains("86400"),
            "{text}"
        );
        let text = Error::InvalidSetting {
            setting: Setting::MemoryMaxRecordings,
        }
        .to_string();
        assert!(
            text.contains("memory.max_recordings") && text.contains("9999"),
            "{text}"
        );
    }

    #[test]
    fn is_core_error() {
        fn takes_error(_: &dyn core::error::Error) {}
        takes_error(&Error::LengthMismatch {
            input: 1,
            output: 2,
        });
        takes_error(&Error::ScheduleFull {
            stream: crate::Stream::Playback,
        });
    }

    #[test]
    fn schedule_errors_display_the_stream_and_the_control() {
        let full = Error::ScheduleFull {
            stream: crate::Stream::OnePass,
        }
        .to_string();
        assert!(full.contains("OnePass") && full.contains("64"), "{full}");
        let not = Error::ControlNotOnStream {
            control: 2,
            stream: crate::Stream::OnePass,
        }
        .to_string();
        assert!(
            not.contains("control 2") && not.contains("OnePass"),
            "{not}"
        );
        let reserved = Error::ReservedStream {
            stream: crate::Stream::Record,
        }
        .to_string();
        assert!(reserved.contains("Record"), "{reserved}");
        let value = Error::InvalidControlValue {
            control: 0,
            value: 2.0,
        }
        .to_string();
        assert!(
            value.contains("2") && value.contains("control 0"),
            "{value}"
        );
        let clock = Error::NoSuchClock {
            stream: crate::Stream::Record,
        }
        .to_string();
        assert!(
            clock.contains("Record") && clock.contains("clock"),
            "{clock}"
        );
        let control = Error::NoSuchControl { control: 29 }.to_string();
        assert!(control.contains("29"), "{control}");
        let volts = Error::InvalidBatteryVolts { volts: 3.1 }.to_string();
        assert!(volts.contains("3.1") && volts.contains("2.2"), "{volts}");
    }
}
