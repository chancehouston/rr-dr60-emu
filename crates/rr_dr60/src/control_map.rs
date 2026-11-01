//! Control access by the control manifest's index (spec 025): one mapping from every control
//! (31 at 0.91) to the settings field it stands for (26) or the live control it reads and
//! sets (5), a typed [`ControlReading`], and the reads and writes of a settings structure by
//! index.
//!
//! The mapping is closed three ways (research R-01): the core test
//! `targets_match_manifest_by_index` compares every index's identifier with the manifest's;
//! `exhaustive` destructures [`Settings`] with no `..`, so a new settings field fails to
//! compile until it is named here; and `setting_variants_are_26` counts the settings targets.
//! The writes check **representability only** (the value type, an option, a finite value, the
//! Rust field's width); ranges stay with the settings' validation, which names the field.

use crate::controls::{CONTROL_COUNT, mode_code, mode_from_code, tap_code, tap_from_code};
#[cfg(test)]
use crate::controls::{ControlValue, controls};
use crate::error::{Error, Setting};
use crate::settings::{
    AgcSettings, MemorySettings, MicSettings, OutputSettings, Settings, VasSettings,
};

/// A control's value in its manifest value type (spec 025 D2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ControlReading {
    /// A switch: off or on.
    Switch(bool),
    /// An integer, exact over the whole `u64` range (the seed spans it).
    Integer(u64),
    /// A continuous value, single precision as the engine stores it.
    Continuous(f32),
    /// A choice: the engine value of one of the control's options.
    Choice(u32),
}

/// One settings field (research R-01): 26 at 0.91.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SettingAccess {
    HostRate,
    RecordStage,
    PlaybackStage,
    AdcClamp,
    Tap,
    Seed,
    AgcEnabled,
    AgcKnee,
    AgcAttack,
    AgcRelease,
    VasEnabled,
    VasMode,
    VasSensitivity,
    VasThreshold,
    VasHang,
    VasOnset,
    MemoryCapacity,
    MemoryMaxRecordings,
    SpeechCodec,
    VolumeAmp,
    Speaker,
    Earphone,
    OutputClamp,
    MicCapsule,
    MicAmp,
    MicHissOffset,
}

/// The live controls with a plain setter (spec 025 D4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LiveControl {
    Route,
    Volume,
    SpeedLevel,
    SpeedMode,
    Battery,
}

/// What a control index stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Setting(SettingAccess),
    Live(LiveControl),
}

use LiveControl as L;
use SettingAccess as S;
use Target::{Live, Setting as Set};

/// Every control in the manifest's order (spec 014's indices; appended entries last).
const TARGETS: [Target; CONTROL_COUNT] = [
    Live(L::Route),
    Live(L::Volume),
    Live(L::SpeedLevel),
    Set(S::HostRate),
    Set(S::RecordStage),
    Set(S::PlaybackStage),
    Set(S::AdcClamp),
    Set(S::Tap),
    Set(S::Seed),
    Set(S::AgcEnabled),
    Set(S::AgcKnee),
    Set(S::AgcAttack),
    Set(S::AgcRelease),
    Set(S::VasEnabled),
    Set(S::VasMode),
    Set(S::VasSensitivity),
    Set(S::VasThreshold),
    Set(S::VasHang),
    Set(S::VasOnset),
    Set(S::MemoryCapacity),
    Set(S::MemoryMaxRecordings),
    Set(S::SpeechCodec),
    Set(S::VolumeAmp),
    Set(S::Speaker),
    Set(S::Earphone),
    Set(S::OutputClamp),
    Set(S::MicCapsule),
    Set(S::MicAmp),
    Set(S::MicHissOffset),
    Live(L::Battery),
    Live(L::SpeedMode),
];

impl SettingAccess {
    /// The manifest identifier of this field.
    #[cfg(test)]
    pub(crate) const fn identifier(self) -> &'static str {
        match self {
            S::HostRate => "host_rate_hz",
            S::RecordStage => "record_stage_enabled",
            S::PlaybackStage => "playback_stage_enabled",
            S::AdcClamp => "adc_clamp_enabled",
            S::Tap => "tap",
            S::Seed => "seed",
            S::AgcEnabled => "agc.enabled",
            S::AgcKnee => "agc.knee_dbfs",
            S::AgcAttack => "agc.attack_ms",
            S::AgcRelease => "agc.release_ms",
            S::VasEnabled => "vas.enabled",
            S::VasMode => "vas.mode",
            S::VasSensitivity => "vas.sensitivity",
            S::VasThreshold => "vas.threshold_dbfs",
            S::VasHang => "vas.hang_ms",
            S::VasOnset => "vas.onset_ms",
            S::MemoryCapacity => "memory.capacity_s",
            S::MemoryMaxRecordings => "memory.max_recordings",
            S::SpeechCodec => "speech_codec_enabled",
            S::VolumeAmp => "output.volume_amp_enabled",
            S::Speaker => "output.speaker_enabled",
            S::Earphone => "output.earphone_enabled",
            S::OutputClamp => "output.clamp_enabled",
            S::MicCapsule => "mic.capsule_enabled",
            S::MicAmp => "mic.amp_enabled",
            S::MicHissOffset => "mic.hiss_offset_db",
        }
    }
}

impl LiveControl {
    /// The manifest identifier of this live control.
    #[cfg(test)]
    pub(crate) const fn identifier(self) -> &'static str {
        match self {
            L::Route => "route",
            L::Volume => "volume",
            L::SpeedLevel => "playback_speed",
            L::SpeedMode => "playback_speed_mode",
            L::Battery => "battery_volts",
        }
    }
}

impl Target {
    /// The manifest identifier of this target.
    #[cfg(test)]
    pub(crate) const fn identifier(self) -> &'static str {
        match self {
            Target::Setting(s) => s.identifier(),
            Target::Live(l) => l.identifier(),
        }
    }
}

/// The target of control `index`, or `NoSuchControl`.
pub(crate) fn target(index: u32) -> Result<Target, Error> {
    TARGETS
        .get(index as usize)
        .copied()
        .ok_or(Error::NoSuchControl { control: index })
}

/// The compile-time completeness check (research R-01 (b)): every field of [`Settings`] and its
/// sub-structures is named here with no `..`, so a new field fails to compile until it is
/// mapped. Never called.
#[allow(dead_code)]
fn exhaustive(s: &Settings) {
    let Settings {
        host_rate_hz: _,         // S::HostRate
        record_stage_enabled: _, // S::RecordStage
        playback_stage_enabled: _,
        adc_clamp_enabled: _,
        tap: _,
        seed: _,
        agc:
            AgcSettings {
                enabled: _,
                knee_dbfs: _,
                attack_ms: _,
                release_ms: _,
            },
        vas:
            VasSettings {
                enabled: _,
                mode: _,
                sensitivity: _,
                threshold_dbfs: _,
                hang_ms: _,
                onset_ms: _,
            },
        memory: MemorySettings {
            capacity_s: _,
            max_recordings: _,
        },
        speech_codec_enabled: _,
        output:
            OutputSettings {
                volume_amp_enabled: _,
                speaker_enabled: _,
                earphone_enabled: _,
                clamp_enabled: _,
            },
        mic:
            MicSettings {
                capsule_enabled: _,
                amp_enabled: _,
                hiss_offset_db: _,
            },
    } = *s;
}

/// The value of field `a` in `s`.
pub(crate) fn get(s: &Settings, a: SettingAccess) -> ControlReading {
    use ControlReading::{Choice, Continuous, Integer, Switch};
    match a {
        S::HostRate => Choice(s.host_rate_hz),
        S::RecordStage => Switch(s.record_stage_enabled),
        S::PlaybackStage => Switch(s.playback_stage_enabled),
        S::AdcClamp => Switch(s.adc_clamp_enabled),
        S::Tap => Choice(tap_code(s.tap)),
        S::Seed => Integer(s.seed),
        S::AgcEnabled => Switch(s.agc.enabled),
        S::AgcKnee => Continuous(s.agc.knee_dbfs),
        S::AgcAttack => Continuous(s.agc.attack_ms),
        S::AgcRelease => Continuous(s.agc.release_ms),
        S::VasEnabled => Switch(s.vas.enabled),
        S::VasMode => Choice(mode_code(s.vas.mode)),
        S::VasSensitivity => Integer(u64::from(s.vas.sensitivity)),
        S::VasThreshold => Continuous(s.vas.threshold_dbfs),
        S::VasHang => Continuous(s.vas.hang_ms),
        S::VasOnset => Continuous(s.vas.onset_ms),
        S::MemoryCapacity => Integer(u64::from(s.memory.capacity_s)),
        S::MemoryMaxRecordings => Integer(u64::from(s.memory.max_recordings)),
        S::SpeechCodec => Switch(s.speech_codec_enabled),
        S::VolumeAmp => Switch(s.output.volume_amp_enabled),
        S::Speaker => Switch(s.output.speaker_enabled),
        S::Earphone => Switch(s.output.earphone_enabled),
        S::OutputClamp => Switch(s.output.clamp_enabled),
        S::MicCapsule => Switch(s.mic.capsule_enabled),
        S::MicAmp => Switch(s.mic.amp_enabled),
        S::MicHissOffset => Continuous(s.mic.hiss_offset_db),
    }
}

/// The representability refusal for control `index` (research R-06): the value as f64, NaN
/// for a reading of the wrong type.
fn refuse(index: u32, r: ControlReading) -> Error {
    let value = match r {
        ControlReading::Switch(b) => f64::from(u8::from(b)),
        ControlReading::Integer(n) => n as f64,
        ControlReading::Continuous(v) => f64::from(v),
        ControlReading::Choice(c) => f64::from(c),
    };
    Error::InvalidControlValue {
        control: index,
        value,
    }
}

/// A continuous value as stored: finite, −0.0 as 0.0 (spec 015 R-04).
pub(crate) fn finite(index: u32, r: ControlReading) -> Result<f32, Error> {
    match r {
        ControlReading::Continuous(v) if v.is_finite() => Ok(v + 0.0),
        _ => Err(refuse(index, r)),
    }
}

/// Writes field `a` of `s` from `r`, checking representability only (spec 025 D3).
pub(crate) fn set(
    s: &mut Settings,
    a: SettingAccess,
    index: u32,
    r: ControlReading,
) -> Result<(), Error> {
    use ControlReading::{Choice, Integer, Switch};
    let bad = || refuse(index, r);
    let switch = |r: ControlReading| match r {
        Switch(b) => Ok(b),
        _ => Err(refuse(index, r)),
    };
    match a {
        S::HostRate => match r {
            Choice(c) if crate::settings::SUPPORTED_HOST_RATES.contains(&c) => s.host_rate_hz = c,
            _ => return Err(bad()),
        },
        S::RecordStage => s.record_stage_enabled = switch(r)?,
        S::PlaybackStage => s.playback_stage_enabled = switch(r)?,
        S::AdcClamp => s.adc_clamp_enabled = switch(r)?,
        S::Tap => match r {
            Choice(c) => s.tap = tap_from_code(c).ok_or_else(bad)?,
            _ => return Err(bad()),
        },
        S::Seed => match r {
            Integer(n) => s.seed = n,
            _ => return Err(bad()),
        },
        S::AgcEnabled => s.agc.enabled = switch(r)?,
        S::AgcKnee => s.agc.knee_dbfs = finite(index, r)?,
        S::AgcAttack => s.agc.attack_ms = finite(index, r)?,
        S::AgcRelease => s.agc.release_ms = finite(index, r)?,
        S::VasEnabled => s.vas.enabled = switch(r)?,
        S::VasMode => match r {
            Choice(c) => s.vas.mode = mode_from_code(c).ok_or_else(bad)?,
            _ => return Err(bad()),
        },
        S::VasSensitivity => match r {
            Integer(n) => s.vas.sensitivity = u8::try_from(n).map_err(|_| bad())?,
            _ => return Err(bad()),
        },
        S::VasThreshold => s.vas.threshold_dbfs = finite(index, r)?,
        S::VasHang => s.vas.hang_ms = finite(index, r)?,
        S::VasOnset => s.vas.onset_ms = finite(index, r)?,
        S::MemoryCapacity => match r {
            Integer(n) => s.memory.capacity_s = u32::try_from(n).map_err(|_| bad())?,
            _ => return Err(bad()),
        },
        S::MemoryMaxRecordings => match r {
            Integer(n) => s.memory.max_recordings = u32::try_from(n).map_err(|_| bad())?,
            _ => return Err(bad()),
        },
        S::SpeechCodec => s.speech_codec_enabled = switch(r)?,
        S::VolumeAmp => s.output.volume_amp_enabled = switch(r)?,
        S::Speaker => s.output.speaker_enabled = switch(r)?,
        S::Earphone => s.output.earphone_enabled = switch(r)?,
        S::OutputClamp => s.output.clamp_enabled = switch(r)?,
        S::MicCapsule => s.mic.capsule_enabled = switch(r)?,
        S::MicAmp => s.mic.amp_enabled = switch(r)?,
        S::MicHissOffset => s.mic.hiss_offset_db = finite(index, r)?,
    }
    Ok(())
}

/// The value of control `index` in settings `s` (spec 025 D3).
///
/// # Errors
///
/// [`Error::NoSuchControl`] past the manifest; [`Error::NotASetting`] for a live control.
pub fn settings_value(s: &Settings, index: u32) -> Result<ControlReading, Error> {
    #[cfg(feature = "__test-hooks")]
    return __hooks::settings_value_with(MapMutation::default(), s, index);
    #[cfg(not(feature = "__test-hooks"))]
    match target(index)? {
        Target::Setting(a) => Ok(get(s, a)),
        Target::Live(_) => Err(Error::NotASetting { control: index }),
    }
}

/// Writes control `index` of settings `s` from `reading`, checking representability only: the
/// value type, an option, a finite value (−0.0 stored as 0.0), the field's width. Ranges are
/// checked by the settings' validation, which names the field (spec 025 D3).
///
/// # Errors
///
/// [`Error::NoSuchControl`], [`Error::NotASetting`], [`Error::InvalidControlValue`]; `s` is
/// unchanged on any error.
pub fn set_settings_value(
    s: &mut Settings,
    index: u32,
    reading: ControlReading,
) -> Result<(), Error> {
    #[cfg(feature = "__test-hooks")]
    return __hooks::set_settings_value_with(MapMutation::default(), s, index, reading);
    #[cfg(not(feature = "__test-hooks"))]
    match target(index)? {
        Target::Setting(a) => set(s, a, index, reading),
        Target::Live(_) => Err(Error::NotASetting { control: index }),
    }
}

/// The control index of a validated settings field (spec 025 D6).
pub fn setting_control(field: Setting) -> u32 {
    let a = match field {
        Setting::AgcKneeDbfs => S::AgcKnee,
        Setting::AgcAttackMs => S::AgcAttack,
        Setting::AgcReleaseMs => S::AgcRelease,
        Setting::VasSensitivity => S::VasSensitivity,
        Setting::VasThresholdDbfs => S::VasThreshold,
        Setting::VasHangMs => S::VasHang,
        Setting::VasOnsetMs => S::VasOnset,
        Setting::MemoryCapacityS => S::MemoryCapacity,
        Setting::MemoryMaxRecordings => S::MemoryMaxRecordings,
        Setting::MicHissOffsetDb => S::MicHissOffset,
    };
    index_of(Target::Setting(a))
}

/// The index of a target in the manifest's order.
pub(crate) fn index_of(t: Target) -> u32 {
    TARGETS
        .iter()
        .position(|x| *x == t)
        .expect("every target is in the table") as u32
}

/// The mapping's mutants (spec 025 research R-07): each a named fault the tests must catch.
#[cfg(feature = "__test-hooks")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MapMutation {
    /// Index i reads and writes the field of index i + 1.
    pub index_plus_one: bool,
    /// The attack and release fields swapped.
    pub attack_release_swapped: bool,
    /// The speaker and earphone switches swapped.
    pub speaker_earphone_swapped: bool,
    /// A too-wide integer wrapped (`as u8`) instead of refused.
    pub wide_integer_wraps: bool,
}

/// The mutation hooks (research R-07).
#[cfg(feature = "__test-hooks")]
pub mod __hooks {
    use super::*;

    fn mutate(m: MapMutation, a: SettingAccess) -> SettingAccess {
        match a {
            S::AgcAttack if m.attack_release_swapped => S::AgcRelease,
            S::AgcRelease if m.attack_release_swapped => S::AgcAttack,
            S::Speaker if m.speaker_earphone_swapped => S::Earphone,
            S::Earphone if m.speaker_earphone_swapped => S::Speaker,
            other => other,
        }
    }

    fn mutated_target(m: MapMutation, index: u32) -> Result<Target, Error> {
        let t = target(index)?;
        if m.index_plus_one {
            if let Ok(next) = target(index + 1) {
                if let (Target::Setting(_), Target::Setting(_)) = (t, next) {
                    return Ok(next);
                }
            }
        }
        Ok(t)
    }

    /// `settings_value` with mutant `m`.
    pub fn settings_value_with(
        m: MapMutation,
        s: &Settings,
        index: u32,
    ) -> Result<ControlReading, Error> {
        match mutated_target(m, index)? {
            Target::Setting(a) => Ok(get(s, mutate(m, a))),
            Target::Live(_) => Err(Error::NotASetting { control: index }),
        }
    }

    /// `set_settings_value` with mutant `m`.
    pub fn set_settings_value_with(
        m: MapMutation,
        s: &mut Settings,
        index: u32,
        reading: ControlReading,
    ) -> Result<(), Error> {
        match mutated_target(m, index)? {
            Target::Setting(a) => {
                let a = mutate(m, a);
                if m.wide_integer_wraps && a == S::VasSensitivity {
                    if let ControlReading::Integer(n) = reading {
                        s.vas.sensitivity = n as u8;
                        return Ok(());
                    }
                }
                set(s, a, index, reading)
            }
            Target::Live(_) => Err(Error::NotASetting { control: index }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::ControlEntry;
    use crate::settings::{Tap, VasMode};

    #[test]
    fn targets_match_manifest_by_index() {
        let c: &[ControlEntry] = controls();
        assert_eq!(c.len(), TARGETS.len());
        for (i, (e, t)) in c.iter().zip(TARGETS.iter()).enumerate() {
            assert_eq!(e.identifier, t.identifier(), "index {i}");
        }
    }

    #[test]
    fn setting_variants_are_26() {
        let n = TARGETS
            .iter()
            .filter(|t| matches!(t, Target::Setting(_)))
            .count();
        assert_eq!(n, 26);
        let mut seen: alloc::vec::Vec<SettingAccess> = alloc::vec::Vec::new();
        for t in TARGETS {
            if let Target::Setting(a) = t {
                assert!(!seen.contains(&a), "{a:?} twice");
                seen.push(a);
            }
        }
    }

    #[test]
    fn option_maps_match_manifest() {
        for (i, e) in controls().iter().enumerate() {
            if let ControlValue::Choice { options, .. } = e.value {
                for o in options {
                    let r = ControlReading::Choice(o.value);
                    match target(i as u32).unwrap() {
                        Target::Setting(a) => {
                            let mut s = Settings::new(48_000);
                            set(&mut s, a, i as u32, r)
                                .unwrap_or_else(|_| panic!("{} {}", e.identifier, o.identifier));
                            assert_eq!(get(&s, a), r, "{} {}", e.identifier, o.identifier);
                        }
                        Target::Live(_) => {} // route and speed mode: the pipeline's tests
                    }
                }
            }
        }
        // The tap and the mode by name.
        let mut s = Settings::new(48_000);
        let tap = index_of(Target::Setting(S::Tap));
        set_settings_value(&mut s, tap, ControlReading::Choice(10)).unwrap();
        assert_eq!(s.tap, Tap::AfterMicAmp);
        let mode = index_of(Target::Setting(S::VasMode));
        set_settings_value(&mut s, mode, ControlReading::Choice(1)).unwrap();
        assert_eq!(s.vas.mode, VasMode::Mute);
    }

    /// The one-hot oracle (research R-07): for every settings index a unique non-default value
    /// changes exactly the field named by its literal identifier below.
    #[test]
    fn one_hot() {
        let base = Settings::new(48_000);
        for (i, e) in controls().iter().enumerate() {
            let i = i as u32;
            let Ok(Target::Setting(_)) = target(i) else {
                continue;
            };
            let reading = match e.value {
                ControlValue::Switch { default } => ControlReading::Switch(!default),
                ControlValue::Integer { default, .. } => {
                    ControlReading::Integer(if default == u64::from(i) + 2 {
                        u64::from(i) + 3
                    } else {
                        u64::from(i) + 2
                    })
                }
                ControlValue::Continuous { default, .. } => {
                    ControlReading::Continuous(if default == i as f32 + 0.5 {
                        i as f32 + 1.5
                    } else {
                        i as f32 + 0.5
                    })
                }
                ControlValue::Choice { options, .. } => {
                    let current = match settings_value(&base, i).unwrap() {
                        ControlReading::Choice(c) => c,
                        _ => unreachable!(),
                    };
                    ControlReading::Choice(
                        options
                            .iter()
                            .map(|o| o.value)
                            .find(|v| *v != current)
                            .unwrap(),
                    )
                }
            };
            let mut s = base;
            set_settings_value(&mut s, i, reading)
                .unwrap_or_else(|err| panic!("{}: {err}", e.identifier));
            assert_eq!(settings_value(&s, i).unwrap(), reading, "{}", e.identifier);
            let changed = changed_fields(&base, &s);
            assert_eq!(changed, [e.identifier], "index {i}");
        }
    }

    /// The fields that differ between `a` and `b`, each named by a literal identifier.
    fn changed_fields(a: &Settings, b: &Settings) -> alloc::vec::Vec<&'static str> {
        let Settings {
            host_rate_hz,
            record_stage_enabled,
            playback_stage_enabled,
            adc_clamp_enabled,
            tap,
            seed,
            agc:
                AgcSettings {
                    enabled: agc_enabled,
                    knee_dbfs,
                    attack_ms,
                    release_ms,
                },
            vas:
                VasSettings {
                    enabled: vas_enabled,
                    mode,
                    sensitivity,
                    threshold_dbfs,
                    hang_ms,
                    onset_ms,
                },
            memory:
                MemorySettings {
                    capacity_s,
                    max_recordings,
                },
            speech_codec_enabled,
            output:
                OutputSettings {
                    volume_amp_enabled,
                    speaker_enabled,
                    earphone_enabled,
                    clamp_enabled,
                },
            mic:
                MicSettings {
                    capsule_enabled,
                    amp_enabled,
                    hiss_offset_db,
                },
        } = *a;
        let mut out = alloc::vec::Vec::new();
        let mut d = |same: bool, id: &'static str| {
            if !same {
                out.push(id);
            }
        };
        d(host_rate_hz == b.host_rate_hz, "host_rate_hz");
        d(
            record_stage_enabled == b.record_stage_enabled,
            "record_stage_enabled",
        );
        d(
            playback_stage_enabled == b.playback_stage_enabled,
            "playback_stage_enabled",
        );
        d(
            adc_clamp_enabled == b.adc_clamp_enabled,
            "adc_clamp_enabled",
        );
        d(tap == b.tap, "tap");
        d(seed == b.seed, "seed");
        d(agc_enabled == b.agc.enabled, "agc.enabled");
        d(
            knee_dbfs.to_bits() == b.agc.knee_dbfs.to_bits(),
            "agc.knee_dbfs",
        );
        d(
            attack_ms.to_bits() == b.agc.attack_ms.to_bits(),
            "agc.attack_ms",
        );
        d(
            release_ms.to_bits() == b.agc.release_ms.to_bits(),
            "agc.release_ms",
        );
        d(vas_enabled == b.vas.enabled, "vas.enabled");
        d(mode == b.vas.mode, "vas.mode");
        d(sensitivity == b.vas.sensitivity, "vas.sensitivity");
        d(
            threshold_dbfs.to_bits() == b.vas.threshold_dbfs.to_bits(),
            "vas.threshold_dbfs",
        );
        d(hang_ms.to_bits() == b.vas.hang_ms.to_bits(), "vas.hang_ms");
        d(
            onset_ms.to_bits() == b.vas.onset_ms.to_bits(),
            "vas.onset_ms",
        );
        d(capacity_s == b.memory.capacity_s, "memory.capacity_s");
        d(
            max_recordings == b.memory.max_recordings,
            "memory.max_recordings",
        );
        d(
            speech_codec_enabled == b.speech_codec_enabled,
            "speech_codec_enabled",
        );
        d(
            volume_amp_enabled == b.output.volume_amp_enabled,
            "output.volume_amp_enabled",
        );
        d(
            speaker_enabled == b.output.speaker_enabled,
            "output.speaker_enabled",
        );
        d(
            earphone_enabled == b.output.earphone_enabled,
            "output.earphone_enabled",
        );
        d(
            clamp_enabled == b.output.clamp_enabled,
            "output.clamp_enabled",
        );
        d(
            capsule_enabled == b.mic.capsule_enabled,
            "mic.capsule_enabled",
        );
        d(amp_enabled == b.mic.amp_enabled, "mic.amp_enabled");
        d(
            hiss_offset_db.to_bits() == b.mic.hiss_offset_db.to_bits(),
            "mic.hiss_offset_db",
        );
        out
    }

    #[test]
    fn representability() {
        let sens = index_of(Target::Setting(S::VasSensitivity));
        let mut s = Settings::new(48_000);
        assert!(set_settings_value(&mut s, sens, ControlReading::Integer(256)).is_err());
        assert_eq!(s.vas.sensitivity, Settings::new(48_000).vas.sensitivity);
        set_settings_value(&mut s, sens, ControlReading::Integer(6)).unwrap();
        assert_eq!(s.vas.sensitivity, 6);
        assert!(
            s.validate().is_err(),
            "6 is out of range: the validator refuses"
        );
        // Wrong types, non-options, non-finite, −0.0.
        let knee = index_of(Target::Setting(S::AgcKnee));
        assert!(set_settings_value(&mut s, knee, ControlReading::Integer(1)).is_err());
        assert!(set_settings_value(&mut s, knee, ControlReading::Continuous(f32::NAN)).is_err());
        assert!(
            set_settings_value(&mut s, knee, ControlReading::Continuous(f32::INFINITY)).is_err()
        );
        set_settings_value(&mut s, knee, ControlReading::Continuous(-0.0)).unwrap();
        assert_eq!(s.agc.knee_dbfs.to_bits(), 0.0f32.to_bits());
        let tap = index_of(Target::Setting(S::Tap));
        assert!(set_settings_value(&mut s, tap, ControlReading::Choice(99)).is_err());
        let rate = index_of(Target::Setting(S::HostRate));
        assert!(set_settings_value(&mut s, rate, ControlReading::Choice(12_345)).is_err());
        set_settings_value(&mut s, rate, ControlReading::Choice(6000)).unwrap();
        let cap = index_of(Target::Setting(S::MemoryCapacity));
        assert!(set_settings_value(&mut s, cap, ControlReading::Integer(1 << 32)).is_err());
        let seed = index_of(Target::Setting(S::Seed));
        set_settings_value(&mut s, seed, ControlReading::Integer((1 << 53) + 1)).unwrap();
        assert_eq!(s.seed, (1 << 53) + 1);
        // Live controls are not settings; an index past the count is no control.
        assert!(matches!(
            settings_value(&s, 0),
            Err(Error::NotASetting { control: 0 })
        ));
        assert!(matches!(
            settings_value(&s, 31),
            Err(Error::NoSuchControl { control: 31 })
        ));
    }

    #[test]
    fn setting_control_exhaustive() {
        for f in [
            Setting::AgcKneeDbfs,
            Setting::AgcAttackMs,
            Setting::AgcReleaseMs,
            Setting::VasSensitivity,
            Setting::VasThresholdDbfs,
            Setting::VasHangMs,
            Setting::VasOnsetMs,
            Setting::MemoryCapacityS,
            Setting::MemoryMaxRecordings,
            Setting::MicHissOffsetDb,
        ] {
            let i = setting_control(f);
            let id = controls()[i as usize].identifier;
            assert!(matches!(target(i), Ok(Target::Setting(_))), "{f:?} → {id}");
        }
        assert_eq!(
            controls()[setting_control(Setting::VasHangMs) as usize].identifier,
            "vas.hang_ms"
        );
    }

    #[cfg(feature = "__test-hooks")]
    #[test]
    fn mutants_change_what_they_name() {
        use super::__hooks::*;
        let base = Settings::new(48_000);
        let attack = index_of(Target::Setting(S::AgcAttack));
        let m = MapMutation {
            attack_release_swapped: true,
            ..MapMutation::default()
        };
        let mut s = base;
        set_settings_value_with(m, &mut s, attack, ControlReading::Continuous(7.0)).unwrap();
        assert_eq!(s.agc.release_ms, 7.0);
        let sens = index_of(Target::Setting(S::VasSensitivity));
        let m = MapMutation {
            wide_integer_wraps: true,
            ..MapMutation::default()
        };
        let mut s = base;
        set_settings_value_with(m, &mut s, sens, ControlReading::Integer(259)).unwrap();
        assert_eq!(s.vas.sensitivity, 3);
        let m = MapMutation {
            index_plus_one: true,
            ..MapMutation::default()
        };
        assert_ne!(
            settings_value_with(m, &base, attack).unwrap(),
            settings_value(&base, attack).unwrap()
        );
        let spk = index_of(Target::Setting(S::Speaker));
        let m = MapMutation {
            speaker_earphone_swapped: true,
            ..MapMutation::default()
        };
        let mut s = base;
        set_settings_value_with(m, &mut s, spk, ControlReading::Switch(false)).unwrap();
        assert!(!s.output.earphone_enabled && s.output.speaker_enabled);
    }
}
