//! The **control manifest** (spec 014): constant data describing every adjustable setting and
//! live control of the engine, so an app builds its controls from the engine and cannot drift
//! from what the engine accepts.
//!
//! One [`ControlEntry`] per construction-time setting field and per live control, 29 today,
//! in a stable order: the live controls first ([`Pipeline::set_route`](crate::Pipeline::set_route),
//! [`Pipeline::set_volume`](crate::Pipeline::set_volume),
//! [`Pipeline::set_playback_speed`](crate::Pipeline::set_playback_speed)), then the settings in
//! the declaration order of [`Settings`](crate::Settings), each sub-structure's fields in place.
//!
//! Each entry gives its **kind** ([`ControlKind`]: a device control, a unit condition, a model
//! internal, an emulator option or a host parameter), its **timing** ([`ControlTiming`]: live,
//! idle, idle and memory-dependent, or host-fixed), the paths it applies to, its signal-chain
//! stage (0 for none, as in the [stage manifest](crate::manifest())), its value's type, range and
//! default as the engine defines them, the source and assumption IDs it rests on, and one
//! neutral sentence saying what changing it does to the sound. The descriptions describe the
//! signal chain only; they make no claim about what a recording contains, and they state no
//! assumed figure (the figures are in the numeric fields and the registers).
//!
//! Reading the manifest allocates nothing and performs no I/O; the data are identical on every
//! platform. Tests bind every range and default to the engine's own validation and defaults,
//! every entry to its stage's controls note in the stage manifest, and every ID to the
//! registers (spec 014 FR-007 – FR-010, FR-016).

use crate::manifest::{bytes_eq, chars, ends_with_dot, ids_ok};
use crate::settings::{Tap, VasMode};

/// The number of entries: three live controls and 26 settings (014 FR-001), then the appended
/// live controls (spec 018's battery voltage, spec 023's speed mode).
pub const CONTROL_COUNT: usize = 31; // FR-001; spec 018: `battery_volts` appended at 29; spec 023: `playback_speed_mode` at 30

/// What kind of adjustment an entry is (014 FR-003; the owner's decisions of 2026-10-29).
#[non_exhaustive]
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ControlKind {
    /// A control the owner's manual or the service manual gives the user.
    DeviceControl = 0,
    /// A physical state of one unit that an owner could hear between units or over a unit's
    /// life.
    UnitCondition = 1,
    /// A value the device has that its user cannot reach.
    ModelInternal = 2,
    /// Something the device cannot do.
    EmulatorOption = 3,
    /// Set by the host for reproducibility or by its audio session.
    HostParameter = 4,
}

/// When an entry may change (014 FR-004).
#[non_exhaustive]
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ControlTiming {
    /// Changes while the engine runs; real-time safe. A live control's value is kept by
    /// [`Pipeline::reset`](crate::Pipeline::reset) and [`Pipeline::reconfigure`](crate::Pipeline::reconfigure).
    Live = 0,
    /// Applied only when nothing is recording or playing ([`Pipeline::reconfigure`](crate::Pipeline::reconfigure)).
    Idle = 1,
    /// Idle, and also refused while the stored recordings do not fit the new value.
    IdleMemoryDependent = 2,
    /// Set by the host to match its audio session; the engine accepts a new value at
    /// construction or reconfiguration like an idle setting.
    HostFixed = 3,
}

/// The paths an entry applies to (014 FR-002): a bit set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ControlPaths(u8);

impl ControlPaths {
    /// The record path.
    pub const RECORD: Self = Self(1); // 014 FR-013: RR_DR60_CONTROL_PATH_RECORD
    /// The playback path.
    pub const PLAYBACK: Self = Self(2); // 014 FR-013: RR_DR60_CONTROL_PATH_PLAYBACK
    /// The one-pass engine ([`Pipeline::process`](crate::Pipeline::process)).
    pub const ONE_PASS: Self = Self(4); // 014 FR-013: RR_DR60_CONTROL_PATH_ONE_PASS

    /// The bits: `RECORD = 1`, `PLAYBACK = 2`, `ONE_PASS = 4`.
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Whether every path in `other` is in `self`.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

/// One option of a choice (014 FR-002): a stable identifier and the value the engine takes
/// (the C enumeration value, or the rate in hertz for the host rate).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ControlOption {
    /// Stable identifier, e.g. `speaker`, `after_record`, `hz_48000`.
    pub identifier: &'static str,
    /// The engine's value for this option.
    pub value: u32,
}

/// An entry's value type, range and default (014 FR-002, FR-005; research R-02). Integer
/// bounds are exact up to `u64`; continuous ones are the engine's `f32` values. The step is 1
/// for integers and 0 (continuous) otherwise.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ControlValue {
    /// Off or on.
    Switch {
        /// The default.
        default: bool,
    },
    /// An integer in `min ..= max`.
    Integer {
        /// Inclusive minimum.
        min: u64,
        /// Inclusive maximum.
        max: u64,
        /// The default.
        default: u64,
    },
    /// A continuous value in `min ..= max`.
    Continuous {
        /// Inclusive minimum.
        min: f32,
        /// Inclusive maximum.
        max: f32,
        /// The default.
        default: f32,
    },
    /// One of a fixed set of options.
    Choice {
        /// The options, in the engine's value order.
        options: &'static [ControlOption],
        /// The default option's value, or `None` when the engine has no default (the host
        /// rate).
        default: Option<u32>,
    },
}

/// One adjustable setting or live control (014 FR-002).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlEntry {
    /// Stable identifier: the setting's dotted path in [`Settings`](crate::Settings) (e.g.
    /// `vas.sensitivity`) or the live control's name (`route`, `volume`, `playback_speed`).
    pub identifier: &'static str,
    /// Short English name (at most 40 characters).
    pub name: &'static str,
    /// The kind.
    pub kind: ControlKind,
    /// When it may change.
    pub timing: ControlTiming,
    /// The paths it applies to.
    pub paths: ControlPaths,
    /// The signal-chain stage (1 – 15), or 0 for none.
    pub stage: u8,
    /// Type, range and default.
    pub value: ControlValue,
    /// Unit (`dBFS`, `ms`, `dB`, `s`, `level`, `Hz`), or empty.
    pub unit: &'static str,
    /// Source IDs, ascending.
    pub sources: &'static [&'static str],
    /// Assumption IDs, ascending.
    pub assumptions: &'static [&'static str],
    /// One English sentence: what changing it does to the sound or the recording, in the
    /// default engine configuration (every other stage on, the speech coder on).
    pub description: &'static str,
}

/// All entries, in the order of 014 FR-001.
pub fn controls() -> &'static [ControlEntry] {
    #[allow(clippy::let_unit_value)]
    let () = CONTROL_FORM_RULES_HOLD;
    &CONTROLS
}

/// The engine value of a tap choice (the C enumeration's; spec 014 FR-005, spec 020's record).
pub(crate) const fn tap_code(t: Tap) -> u32 {
    match t {
        Tap::AfterRecord => 0,
        Tap::AfterPlayback => 1,
        Tap::AfterAgc => 2,
        Tap::AfterVas => 3,
        Tap::AfterDecoder => 4,
        Tap::RouteOutput => 5,
        Tap::AfterVolumeAmp => 6,
        Tap::AfterSpeaker => 7,
        Tap::AfterEarphone => 8,
        Tap::AfterMicCapsule => 9,
        Tap::AfterMicAmp => 10,
    }
}

/// The tap of an engine value, if any.
pub(crate) const fn tap_from_code(c: u32) -> Option<Tap> {
    Some(match c {
        0 => Tap::AfterRecord,
        1 => Tap::AfterPlayback,
        2 => Tap::AfterAgc,
        3 => Tap::AfterVas,
        4 => Tap::AfterDecoder,
        5 => Tap::RouteOutput,
        6 => Tap::AfterVolumeAmp,
        7 => Tap::AfterSpeaker,
        8 => Tap::AfterEarphone,
        9 => Tap::AfterMicCapsule,
        10 => Tap::AfterMicAmp,
        _ => return None,
    })
}

/// The engine value of a voice-activation mode.
pub(crate) const fn mode_code(m: VasMode) -> u32 {
    match m {
        VasMode::Drop => 0,
        VasMode::Mute => 1,
    }
}

/// The voice-activation mode of an engine value, if any.
pub(crate) const fn mode_from_code(c: u32) -> Option<VasMode> {
    match c {
        0 => Some(VasMode::Drop),
        1 => Some(VasMode::Mute),
        _ => None,
    }
}

/// The entry with `identifier`, or `None`.
pub fn control(identifier: &str) -> Option<&'static ControlEntry> {
    CONTROLS.iter().find(|e| e.identifier == identifier)
}

macro_rules! control_value {
    (switch($d:expr)) => {
        ControlValue::Switch { default: $d }
    };
    (integer($lo:expr, $hi:expr, $d:expr)) => {
        ControlValue::Integer { min: $lo, max: $hi, default: $d }
    };
    (continuous($lo:expr, $hi:expr, $d:expr)) => {
        ControlValue::Continuous { min: $lo, max: $hi, default: $d }
    };
    (choice([$(($oid:literal, $ov:expr)),* $(,)?], $d:expr)) => {
        ControlValue::Choice {
            options: &[$(ControlOption { identifier: $oid, value: $ov }),*],
            default: $d,
        }
    };
}

macro_rules! option_twins {
    (choice([$(($oid:literal, $ov:expr)),* $(,)?], $d:expr)) => {
        &[$(concat!($oid, "\0").as_bytes()),*]
    };
    ($($other:tt)*) => {
        &[]
    };
}

/// Builds the public table and its hidden C twin from one list (014 research R-01), so the two
/// cannot drift: every string in `__c` is the same literal with a trailing NUL.
macro_rules! control_table {
    ($( {
        identifier: $id:literal, name: $name:literal, kind: $kind:ident, timing: $timing:ident,
        paths: [$($path:ident),+], stage: $stage:literal,
        value: $vk:ident ( $($vt:tt)* ), unit: $unit:literal,
        sources: [$($s:literal),* $(,)?], assumptions: [$($a:literal),* $(,)?],
        description: $desc:literal $(,)?
    } ),* $(,)?) => {
        const CONTROLS: [ControlEntry; CONTROL_COUNT] = [ $( ControlEntry {
            identifier: $id,
            name: $name,
            kind: ControlKind::$kind,
            timing: ControlTiming::$timing,
            paths: ControlPaths(0 $(| ControlPaths::$path.0)+),
            stage: $stage,
            value: control_value!($vk($($vt)*)),
            unit: $unit,
            sources: &[$($s),*],
            assumptions: &[$($a),*],
            description: $desc,
        } ),* ];

        /// NUL-terminated twins of every control-manifest string, for the C API (014 FR-013;
        /// research R-01). Not part of the public API; may change without notice.
        #[doc(hidden)]
        pub mod __c {
            /// One entry's strings with a trailing NUL, parallel to [`super::controls`].
            pub struct ControlStringsC {
                /// `identifier` + NUL.
                pub identifier: &'static [u8],
                /// `name` + NUL.
                pub name: &'static [u8],
                /// `unit` + NUL.
                pub unit: &'static [u8],
                /// `description` + NUL.
                pub description: &'static [u8],
                /// Each option identifier + NUL (empty unless a choice).
                pub options: &'static [&'static [u8]],
                /// Each source ID + NUL.
                pub sources: &'static [&'static [u8]],
                /// Each assumption ID + NUL.
                pub assumptions: &'static [&'static [u8]],
            }

            /// The twins, in the table's order.
            pub static CONTROLS: [ControlStringsC; super::CONTROL_COUNT] = [ $( ControlStringsC {
                identifier: concat!($id, "\0").as_bytes(),
                name: concat!($name, "\0").as_bytes(),
                unit: concat!($unit, "\0").as_bytes(),
                description: concat!($desc, "\0").as_bytes(),
                options: option_twins!($vk($($vt)*)),
                sources: &[$(concat!($s, "\0").as_bytes()),*],
                assumptions: &[$(concat!($a, "\0").as_bytes()),*],
            } ),* ];
        }
    };
}

// The content: spec 014 data-model.md › The 29 entries (after the plan reviews of 2026-10-29).
// Ranges and defaults are the engine's (the tests pin them to `Setting::range`, the memory
// ranges and `Settings::new`); the IDs are the tags on each default's line (014 R-06).
control_table! {
    {
        identifier: "route", name: "Speaker or earphone", kind: DeviceControl, timing: Live,
        paths: [PLAYBACK, ONE_PASS], stage: 11,
        value: choice([("speaker", 0), ("earphone", 1)], Some(0)), unit: "",
        sources: ["S-001"], assumptions: ["A-010", "A-032", "A-042"],
        description: "Sends playback to the modeled speaker or to the earphone output, each with its own response; their relative loudness is left to the host.",
    },
    {
        identifier: "volume", name: "Volume", kind: DeviceControl, timing: Live,
        paths: [PLAYBACK, ONE_PASS], stage: 11,
        value: continuous(0.0, 1.0, crate::output::VOLUME_REFERENCE), unit: "",
        sources: ["S-002"], assumptions: ["A-031", "A-043", "A-044", "A-045"],
        description: "Sets the playback level; turned up, loud passages clip in the speaker amplifier.",
    },
    {
        identifier: "playback_speed", name: "Playback speed", kind: DeviceControl, timing: Live,
        paths: [PLAYBACK], stage: 9,
        value: integer(1, 5, 3), unit: "level",
        sources: ["S-001", "S-012"], assumptions: ["A-054"],
        description: "Plays recordings slower or faster than normal; whether the pitch follows the speed is the speed mode's choice.",
    },
    {
        identifier: "host_rate_hz", name: "Host sample rate", kind: HostParameter, timing: HostFixed,
        paths: [RECORD, PLAYBACK, ONE_PASS], stage: 0,
        value: choice([
            ("hz_6000", 6000), ("hz_8000", 8000), ("hz_16000", 16000), ("hz_44100", 44100),
            ("hz_48000", 48000), ("hz_88200", 88200), ("hz_96000", 96000),
        ], None), unit: "Hz",
        sources: [], assumptions: ["A-026"],
        description: "Tells the engine the sample rate of the host's audio; the engine converts to and from the device's own rate when they differ.",
    },
    {
        identifier: "record_stage_enabled", name: "Codec input filter", kind: EmulatorOption, timing: Idle,
        paths: [RECORD, ONE_PASS], stage: 4,
        value: switch(true), unit: "",
        sources: [], assumptions: ["A-027"],
        description: "Switches the codec's input band-pass filter off, passing the lows and the top edge it removes; the device's sample rate still limits the top.",
    },
    {
        identifier: "playback_stage_enabled", name: "Codec output filter", kind: EmulatorOption, timing: Live,
        paths: [PLAYBACK, ONE_PASS], stage: 10,
        value: switch(true), unit: "",
        sources: [], assumptions: ["A-027"],
        description: "Switches the codec's playback low-pass filter off, removing its roll-off at the top of the band.",
    },
    {
        identifier: "adc_clamp_enabled", name: "Converter clipping", kind: EmulatorOption, timing: Idle,
        paths: [RECORD, ONE_PASS], stage: 4,
        value: switch(true), unit: "",
        sources: [], assumptions: ["A-030"],
        description: "Switches the converter's full-scale clipping off, so loud peaks pass above full scale; the speech coder, when on, still clips them at its input.",
    },
    {
        identifier: "tap", name: "Listen point", kind: EmulatorOption, timing: Idle,
        paths: [ONE_PASS], stage: 0,
        value: choice([
            ("after_record", 0), ("after_playback", 1), ("after_agc", 2), ("after_vas", 3),
            ("after_decoder", 4), ("route_output", 5), ("after_volume_amp", 6),
            ("after_speaker", 7), ("after_earphone", 8), ("after_mic_capsule", 9),
            ("after_mic_amp", 10),
        ], Some(5)), unit: "",
        sources: [], assumptions: [],
        description: "Chooses which stage's output the one-pass engine returns, to hear the chain up to that point; recording and playback ignore it.",
    },
    {
        identifier: "seed", name: "Hiss seed", kind: HostParameter, timing: Idle,
        paths: [RECORD, ONE_PASS], stage: 0,
        value: integer(0, u64::MAX, 0), unit: "",
        sources: [], assumptions: ["A-049"],
        description: "Picks a different but repeatable pattern of microphone hiss, with the same level and tone.",
    },
    {
        identifier: "agc.enabled", name: "Limiter (AGC)", kind: EmulatorOption, timing: Idle,
        paths: [RECORD, ONE_PASS], stage: 3,
        value: switch(true), unit: "",
        sources: [], assumptions: ["A-020", "A-028", "A-030"],
        description: "Switches the limiter off, so loud sounds reach the converter's clipping instead of being held just below it.",
    },
    {
        identifier: "agc.knee_dbfs", name: "Limiter threshold", kind: ModelInternal, timing: Live,
        paths: [RECORD, ONE_PASS], stage: 3,
        value: continuous(-40.0, 3.0, -1.0), unit: "dBFS",
        sources: [], assumptions: ["A-028"],
        description: "Sets the level above which the limiter holds sound down; lowered, ordinary speech is limited and pumps.",
    },
    {
        identifier: "agc.attack_ms", name: "Limiter attack", kind: ModelInternal, timing: Live,
        paths: [RECORD, ONE_PASS], stage: 3,
        value: continuous(1.0, 50.0, 3.0), unit: "ms",
        sources: [], assumptions: ["A-034"],
        description: "Sets how long a loud onset passes at full level before the limiter acts; smaller overloads pass for longer.",
    },
    {
        identifier: "agc.release_ms", name: "Limiter release", kind: ModelInternal, timing: Live,
        paths: [RECORD, ONE_PASS], stage: 3,
        value: continuous(200.0, 10_000.0, 1370.0), unit: "ms",
        sources: [], assumptions: ["A-035"],
        description: "Sets how long the level stays down after a loud sound.",
    },
    {
        identifier: "vas.enabled", name: "Voice-activated pause (VAS)", kind: EmulatorOption, timing: Idle,
        paths: [RECORD, ONE_PASS], stage: 5,
        value: switch(true), unit: "",
        sources: ["S-001"], assumptions: ["A-008"],
        description: "Switches voice-activated pausing off, so quiet stretches stay in the recording.",
    },
    {
        identifier: "vas.mode", name: "Pause handling", kind: EmulatorOption, timing: Idle,
        paths: [RECORD, ONE_PASS], stage: 5,
        value: choice([("drop", 0), ("mute", 1)], Some(0)), unit: "",
        sources: ["S-001"], assumptions: ["A-008"],
        description: "Chooses whether paused stretches are left out of the recording or kept as silence.",
    },
    {
        identifier: "vas.sensitivity", name: "Microphone sensitivity", kind: DeviceControl, timing: Live,
        paths: [RECORD, ONE_PASS], stage: 5,
        value: integer(1, 5, 3), unit: "level",
        sources: ["S-001"], assumptions: ["A-021", "A-022", "A-033"],
        description: "Sets how quiet the input must be before recording pauses; the model applies the level to the pause threshold only.",
    },
    {
        identifier: "vas.threshold_dbfs", name: "Pause threshold", kind: ModelInternal, timing: Live,
        paths: [RECORD, ONE_PASS], stage: 5,
        value: continuous(-60.0, 0.0, -40.0), unit: "dBFS",
        sources: [], assumptions: ["A-022"],
        description: "Sets the pause threshold at the factory sensitivity level; the other levels step up or down from it.",
    },
    {
        identifier: "vas.hang_ms", name: "Pause delay", kind: ModelInternal, timing: Live,
        paths: [RECORD, ONE_PASS], stage: 5,
        value: continuous(50.0, 10_000.0, 1000.0), unit: "ms",
        sources: [], assumptions: ["A-023"],
        description: "Sets how long the input must stay quiet before recording pauses.",
    },
    {
        identifier: "vas.onset_ms", name: "Resume delay", kind: ModelInternal, timing: Live,
        paths: [RECORD, ONE_PASS], stage: 5,
        value: continuous(0.0, 200.0, 20.0), unit: "ms",
        sources: [], assumptions: ["A-024"],
        description: "Sets how much of the first sound after a pause is lost before recording resumes.",
    },
    {
        identifier: "memory.capacity_s", name: "Memory length", kind: EmulatorOption, timing: IdleMemoryDependent,
        paths: [RECORD, PLAYBACK], stage: 8,
        value: integer(1, 86_400, 3600), unit: "s",
        sources: ["S-001"], assumptions: ["A-038"],
        description: "Sets the total recording time the memory holds; when it is used up, recording stops.",
    },
    {
        identifier: "memory.max_recordings", name: "Maximum recordings", kind: EmulatorOption, timing: IdleMemoryDependent,
        paths: [RECORD, PLAYBACK], stage: 8,
        value: integer(1, 9_999, 99), unit: "",
        sources: ["S-001"], assumptions: [],
        description: "Sets how many recordings the memory holds before it refuses a new one.",
    },
    {
        identifier: "speech_codec_enabled", name: "Speech coder (CELP)", kind: EmulatorOption, timing: IdleMemoryDependent,
        paths: [RECORD, PLAYBACK, ONE_PASS], stage: 7,
        value: switch(true), unit: "",
        sources: [], assumptions: ["A-004", "A-040", "A-041"],
        description: "Switches the speech coder off, removing its coding artifacts from recordings.",
    },
    {
        identifier: "output.volume_amp_enabled", name: "Volume and power amplifier", kind: EmulatorOption, timing: Live,
        paths: [PLAYBACK, ONE_PASS], stage: 11,
        value: switch(true), unit: "",
        sources: [], assumptions: ["A-031"],
        description: "Switches the volume control and speaker amplifier off: unity gain and no clipping, and the volume control has no effect.",
    },
    {
        identifier: "output.speaker_enabled", name: "Speaker", kind: EmulatorOption, timing: Live,
        paths: [PLAYBACK, ONE_PASS], stage: 12,
        value: switch(true), unit: "",
        sources: [], assumptions: ["A-010"],
        description: "Switches the small speaker's modeled tone off on the speaker route, removing its midrange resonance.",
    },
    {
        identifier: "output.earphone_enabled", name: "Earphone output", kind: EmulatorOption, timing: Live,
        paths: [PLAYBACK, ONE_PASS], stage: 13,
        value: switch(true), unit: "",
        sources: [], assumptions: ["A-032"],
        description: "Switches the earphone output's modeled tone off on the earphone route, removing its low-frequency cut.",
    },
    {
        identifier: "output.clamp_enabled", name: "Output clamp", kind: EmulatorOption, timing: Idle,
        paths: [PLAYBACK, ONE_PASS], stage: 0,
        value: switch(false), unit: "",
        sources: [], assumptions: ["A-042"],
        description: "Bounds the returned output to full scale, flattening the peaks the modeled speaker and earphone paths produce above it.",
    },
    {
        identifier: "mic.capsule_enabled", name: "Microphone hiss", kind: EmulatorOption, timing: Idle,
        paths: [RECORD, ONE_PASS], stage: 1,
        value: switch(true), unit: "",
        sources: [], assumptions: ["A-048", "A-052"],
        description: "Switches the microphone capsule's hiss off, removing the steady background hiss from quiet passages.",
    },
    {
        identifier: "mic.amp_enabled", name: "Microphone amplifier", kind: EmulatorOption, timing: Idle,
        paths: [RECORD, ONE_PASS], stage: 2,
        value: switch(true), unit: "",
        sources: [], assumptions: ["A-046", "A-047", "A-051"],
        description: "Switches the microphone amplifier off, removing its thin low end and its clipping on loud input.",
    },
    {
        identifier: "mic.hiss_offset_db", name: "Hiss level", kind: UnitCondition, timing: Live,
        paths: [RECORD, ONE_PASS], stage: 1,
        value: continuous(-18.0, 12.0, 0.0), unit: "dB",
        sources: [], assumptions: ["A-048"],
        description: "Raises or lowers the microphone hiss, whose level differs between capsules and is unknown for this unit; lower it when the host's microphone adds its own floor.",
    },
    // Spec 018: appended after the 0.14 entries so no earlier index moves (the order rule:
    // later entries append; the change log and the goldens store indices).
    {
        identifier: "battery_volts", name: "Battery voltage", kind: UnitCondition, timing: Live,
        paths: [PLAYBACK, ONE_PASS], stage: 15,
        value: continuous(2.2, 3.0, 3.0), unit: "V",
        sources: ["S-002", "S-006", "S-017"], assumptions: ["A-056"],
        description: "Sets the played unit's cell voltage: the lower the cells, the lower the speaker amplifier's clip ceiling; set by the user, never read from the host device.",
    },
    // Spec 023 D1: appended at 30 (the same order rule).
    {
        identifier: "playback_speed_mode", name: "Playback speed mode", kind: ModelInternal, timing: Live,
        paths: [PLAYBACK], stage: 9,
        value: choice([("pitch_with_speed", 0), ("pitch_kept", 1)], Some(0)), unit: "",
        sources: ["S-001", "S-012"], assumptions: ["A-054"],
        description: "Chooses whether slow and fast playback moves the pitch with the speed (the modeled default) or keeps it by time-scaling the decoded stream.",
    },
}

// ---- Compile-time form rules (014 FR-002, FR-006; data-model.md › Form rules) ---------------

/// `[a-z0-9_.]{1,32}` (014 FR-002).
pub(crate) const fn is_control_identifier(s: &str) -> bool {
    let b = s.as_bytes();
    if b.is_empty() || b.len() > 32 {
        return false;
    }
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'.') {
            return false;
        }
        i += 1;
    }
    true
}

/// No ASCII digit (014 FR-006: no figures in a description) and no sentence break (". ").
pub(crate) const fn one_sentence_no_digits(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() {
            return false;
        }
        if b[i] == b'.' && i + 1 < b.len() && b[i + 1] == b' ' {
            return false;
        }
        i += 1;
    }
    true
}

/// The value's own invariants: `min ≤ default ≤ max`; a choice's default among its options
/// and its option identifiers well formed and unique.
pub(crate) const fn value_ok(v: &ControlValue) -> bool {
    match *v {
        ControlValue::Switch { .. } => true,
        ControlValue::Integer { min, max, default } => min <= default && default <= max,
        ControlValue::Continuous { min, max, default } => min <= default && default <= max,
        ControlValue::Choice { options, default } => {
            if options.is_empty() {
                return false;
            }
            let mut found = default.is_none();
            let mut i = 0;
            while i < options.len() {
                if !is_control_identifier(options[i].identifier) {
                    return false;
                }
                if let Some(d) = default {
                    if options[i].value == d {
                        found = true;
                    }
                }
                let mut j = 0;
                while j < i {
                    if bytes_eq(
                        options[j].identifier.as_bytes(),
                        options[i].identifier.as_bytes(),
                    ) || options[j].value == options[i].value
                    {
                        return false;
                    }
                    j += 1;
                }
                i += 1;
            }
            found
        }
    }
}

pub(crate) const fn check_controls(entries: &[ControlEntry; CONTROL_COUNT]) {
    let mut i = 0;
    while i < CONTROL_COUNT {
        let e = &entries[i];
        assert!(
            is_control_identifier(e.identifier),
            "identifier: lowercase ASCII letters, digits, '_' and '.', 1 to 32 characters"
        ); // engineering target (014 FR-002)
        assert!(
            !e.name.is_empty() && chars(e.name) <= 40,
            "name: 1 to 40 characters"
        ); // engineering target (014 FR-002)
        assert!(
            chars(e.description) <= 160,
            "description: at most 160 characters"
        ); // engineering target (014 FR-006)
        assert!(
            ends_with_dot(e.description),
            "description: one sentence ending in '.'"
        );
        assert!(
            one_sentence_no_digits(e.description),
            "description: one sentence, no digits"
        );
        assert!(e.stage <= 15, "stage: 0 (none) or 1 to 15");
        assert!(e.paths.0 != 0 && e.paths.0 < 8, "paths: a non-empty set");
        assert!(ids_ok(e.sources, b'S'), "sources: S-### ascending");
        assert!(ids_ok(e.assumptions, b'A'), "assumptions: A-### ascending");
        assert!(value_ok(&e.value), "value: invariants");
        let mut j = 0;
        while j < i {
            assert!(
                !bytes_eq(entries[j].identifier.as_bytes(), e.identifier.as_bytes()),
                "identifiers must be unique"
            );
            j += 1;
        }
        i += 1;
    }
}

/// The form rules, evaluated at compile time: a string over a limit or a broken invariant
/// fails the build with the rule's message. [`controls`] references it, so it counts as used
/// on Rust 1.85 (spec 007 lesson: an anonymous `const _` is not a use for the dead-code lint).
const CONTROL_FORM_RULES_HOLD: () = check_controls(&CONTROLS);

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::error::{MEMORY_CAPACITY_S_RANGE, MEMORY_MAX_RECORDINGS_RANGE};
    use crate::{
        AgcSettings, MemorySettings, MicSettings, OutputSettings, Route, SUPPORTED_HOST_RATES,
        Setting, Settings, Tap, VasMode, VasSettings,
    };
    use std::format;
    use std::string::String;
    use std::vec::Vec;

    /// The identifiers of every `Settings` field, from an exhaustive destructuring (no `..`):
    /// a new field fails to compile here until it is mapped (014 FR-007; research R-03).
    fn setting_identifiers() -> [&'static str; 26] {
        let Settings {
            host_rate_hz: _,
            record_stage_enabled: _,
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
            memory:
                MemorySettings {
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
        } = Settings::new(6000);
        [
            "host_rate_hz",
            "record_stage_enabled",
            "playback_stage_enabled",
            "adc_clamp_enabled",
            "tap",
            "seed",
            "agc.enabled",
            "agc.knee_dbfs",
            "agc.attack_ms",
            "agc.release_ms",
            "vas.enabled",
            "vas.mode",
            "vas.sensitivity",
            "vas.threshold_dbfs",
            "vas.hang_ms",
            "vas.onset_ms",
            "memory.capacity_s",
            "memory.max_recordings",
            "speech_codec_enabled",
            "output.volume_amp_enabled",
            "output.speaker_enabled",
            "output.earphone_enabled",
            "output.clamp_enabled",
            "mic.capsule_enabled",
            "mic.amp_enabled",
            "mic.hiss_offset_db",
        ]
    }

    const LIVE: [&str; 3] = ["route", "volume", "playback_speed"];
    /// Spec 018: entries added after 0.14 are appended in their order (indices are stable).
    const APPENDED: [&str; 2] = ["battery_volts", "playback_speed_mode"]; // 018, 023

    /// 014 FR-007: the mapped identifiers plus the live ones equal the table exactly, in
    /// FR-001's order (live first, then the settings' declaration order).
    fn missing_or_extra(entries: &[ControlEntry]) -> Result<(), String> {
        let expected: Vec<&str> = LIVE
            .iter()
            .copied()
            .chain(setting_identifiers())
            .chain(APPENDED.iter().copied())
            .collect();
        for id in &expected {
            if !entries.iter().any(|e| e.identifier == *id) {
                return Err(format!("missing entry: {id}"));
            }
        }
        for e in entries {
            if !expected.contains(&e.identifier) {
                return Err(format!("entry for nothing: {}", e.identifier));
            }
        }
        let got: Vec<&str> = entries.iter().map(|e| e.identifier).collect();
        if got != expected {
            return Err(format!("order differs: {got:?}"));
        }
        Ok(())
    }

    #[test]
    fn complete_and_in_order() {
        assert_eq!(missing_or_extra(controls()), Ok(()));
        assert_eq!(controls().len(), CONTROL_COUNT);
    }

    /// SC-003 case "missing entry": a doctored table without one entry is caught and named.
    #[test]
    fn doctored_missing_entry_is_named() {
        let doctored: Vec<ControlEntry> = controls()
            .iter()
            .copied()
            .filter(|e| e.identifier != "vas.hang_ms")
            .collect();
        assert_eq!(
            missing_or_extra(&doctored),
            Err(String::from("missing entry: vas.hang_ms"))
        );
        let mut extra: Vec<ControlEntry> = controls().to_vec();
        let mut ghost = extra[0];
        ghost.identifier = "battery";
        extra.push(ghost);
        assert_eq!(
            missing_or_extra(&extra),
            Err(String::from("entry for nothing: battery"))
        );
    }

    fn range_of(id: &str) -> Option<(f32, f32)> {
        let setting = match id {
            "agc.knee_dbfs" => Setting::AgcKneeDbfs,
            "agc.attack_ms" => Setting::AgcAttackMs,
            "agc.release_ms" => Setting::AgcReleaseMs,
            "vas.sensitivity" => Setting::VasSensitivity,
            "vas.threshold_dbfs" => Setting::VasThresholdDbfs,
            "vas.hang_ms" => Setting::VasHangMs,
            "vas.onset_ms" => Setting::VasOnsetMs,
            "mic.hiss_offset_db" => Setting::MicHissOffsetDb,
            _ => return None,
        };
        Some(setting.range())
    }

    /// 014 FR-005: every bound equals the engine's validation range.
    #[test]
    fn ranges_equal_the_engine() {
        for e in controls() {
            match (e.identifier, e.value) {
                ("memory.capacity_s", ControlValue::Integer { min, max, .. }) => {
                    let (lo, hi) = MEMORY_CAPACITY_S_RANGE;
                    assert_eq!((min, max), (u64::from(lo), u64::from(hi)));
                }
                ("memory.max_recordings", ControlValue::Integer { min, max, .. }) => {
                    let (lo, hi) = MEMORY_MAX_RECORDINGS_RANGE;
                    assert_eq!((min, max), (u64::from(lo), u64::from(hi)));
                }
                ("seed", ControlValue::Integer { min, max, .. }) => {
                    assert_eq!((min, max), (0, u64::MAX));
                }
                ("playback_speed", ControlValue::Integer { min, max, .. }) => {
                    assert_eq!((min, max), (1, u64::from(crate::speed::LEVEL_COUNT)));
                }
                ("volume", ControlValue::Continuous { min, max, .. }) => {
                    assert_eq!((min, max), (0.0, 1.0)); // set_volume: 0 … 1 (011 FR-001)
                }
                ("battery_volts", ControlValue::Continuous { min, max, .. }) => {
                    // Spec 018 FR-001: the range `set_battery_volts` accepts.
                    assert_eq!((min, max), crate::battery::BATTERY_VOLTS_RANGE);
                }
                (id, ControlValue::Integer { min, max, .. }) => {
                    let (lo, hi) = range_of(id).unwrap_or_else(|| panic!("{id}: no range"));
                    assert_eq!((min as f32, max as f32), (lo, hi), "{id}");
                }
                (id, ControlValue::Continuous { min, max, .. }) => {
                    let (lo, hi) = range_of(id).unwrap_or_else(|| panic!("{id}: no range"));
                    assert_eq!((min, max), (lo, hi), "{id}");
                }
                ("host_rate_hz", ControlValue::Choice { options, default }) => {
                    let values: Vec<u32> = options.iter().map(|o| o.value).collect();
                    assert_eq!(values, SUPPORTED_HOST_RATES.to_vec());
                    assert_eq!(default, None);
                    for o in options {
                        assert_eq!(o.identifier, format!("hz_{}", o.value));
                    }
                }
                _ => {}
            }
        }
    }

    #[derive(Debug, PartialEq)]
    enum Def {
        B(bool),
        I(u64),
        F(f32),
        C(Option<u32>),
    }

    fn tap_value(t: Tap) -> (&'static str, u32) {
        // The values are the C enumeration's (`tap_code`); the harness checks them against the
        // header. An exhaustive match on the names: a new tap fails to compile until listed.
        let name = match t {
            Tap::AfterRecord => "after_record",
            Tap::AfterPlayback => "after_playback",
            Tap::AfterAgc => "after_agc",
            Tap::AfterVas => "after_vas",
            Tap::AfterDecoder => "after_decoder",
            Tap::RouteOutput => "route_output",
            Tap::AfterVolumeAmp => "after_volume_amp",
            Tap::AfterSpeaker => "after_speaker",
            Tap::AfterEarphone => "after_earphone",
            Tap::AfterMicCapsule => "after_mic_capsule",
            Tap::AfterMicAmp => "after_mic_amp",
        };
        assert_eq!(tap_from_code(tap_code(t)), Some(t));
        (name, tap_code(t))
    }

    fn mode_value(m: VasMode) -> (&'static str, u32) {
        let name = match m {
            VasMode::Drop => "drop",
            VasMode::Mute => "mute",
        };
        assert_eq!(mode_from_code(mode_code(m)), Some(m));
        (name, mode_code(m))
    }

    fn route_value(r: Route) -> (&'static str, u32) {
        match r {
            Route::Speaker => ("speaker", 0),
            Route::Earphone => ("earphone", 1),
        }
    }

    fn engine_default(id: &str, s: &Settings) -> Def {
        match id {
            "route" => Def::C(Some(route_value(Route::default()).1)),
            "volume" => Def::F(crate::output::VOLUME_REFERENCE),
            "playback_speed" => Def::I(u64::from(crate::speed::DEFAULT_LEVEL)),
            "battery_volts" => Def::F(crate::battery::DEFAULT_BATTERY_VOLTS), // spec 018
            "playback_speed_mode" => Def::C(Some(crate::speed::SpeedMode::default() as u32)), // spec 023
            "host_rate_hz" => Def::C(None),
            "record_stage_enabled" => Def::B(s.record_stage_enabled),
            "playback_stage_enabled" => Def::B(s.playback_stage_enabled),
            "adc_clamp_enabled" => Def::B(s.adc_clamp_enabled),
            "tap" => Def::C(Some(tap_value(s.tap).1)),
            "seed" => Def::I(s.seed),
            "agc.enabled" => Def::B(s.agc.enabled),
            "agc.knee_dbfs" => Def::F(s.agc.knee_dbfs),
            "agc.attack_ms" => Def::F(s.agc.attack_ms),
            "agc.release_ms" => Def::F(s.agc.release_ms),
            "vas.enabled" => Def::B(s.vas.enabled),
            "vas.mode" => Def::C(Some(mode_value(s.vas.mode).1)),
            "vas.sensitivity" => Def::I(u64::from(s.vas.sensitivity)),
            "vas.threshold_dbfs" => Def::F(s.vas.threshold_dbfs),
            "vas.hang_ms" => Def::F(s.vas.hang_ms),
            "vas.onset_ms" => Def::F(s.vas.onset_ms),
            "memory.capacity_s" => Def::I(u64::from(s.memory.capacity_s)),
            "memory.max_recordings" => Def::I(u64::from(s.memory.max_recordings)),
            "speech_codec_enabled" => Def::B(s.speech_codec_enabled),
            "output.volume_amp_enabled" => Def::B(s.output.volume_amp_enabled),
            "output.speaker_enabled" => Def::B(s.output.speaker_enabled),
            "output.earphone_enabled" => Def::B(s.output.earphone_enabled),
            "output.clamp_enabled" => Def::B(s.output.clamp_enabled),
            "mic.capsule_enabled" => Def::B(s.mic.capsule_enabled),
            "mic.amp_enabled" => Def::B(s.mic.amp_enabled),
            "mic.hiss_offset_db" => Def::F(s.mic.hiss_offset_db),
            other => panic!("{other}: no engine default mapped"),
        }
    }

    fn table_default(v: ControlValue) -> Def {
        match v {
            ControlValue::Switch { default } => Def::B(default),
            ControlValue::Integer { default, .. } => Def::I(default),
            ControlValue::Continuous { default, .. } => Def::F(default),
            ControlValue::Choice { default, .. } => Def::C(default),
        }
    }

    /// 014 FR-005: every default equals the engine's at every supported host rate.
    #[test]
    fn defaults_at_every_rate() {
        for rate in SUPPORTED_HOST_RATES {
            let s = Settings::new(rate);
            for e in controls() {
                assert_eq!(
                    table_default(e.value),
                    engine_default(e.identifier, &s),
                    "{} at {rate} Hz",
                    e.identifier
                );
            }
        }
    }

    fn options(id: &str) -> &'static [ControlOption] {
        match control(id).map(|e| e.value) {
            Some(ControlValue::Choice { options, .. }) => options,
            _ => panic!("{id} is not a choice"),
        }
    }

    /// 014 FR-005: each choice lists exactly the engine's variants, by identifier and value.
    #[test]
    fn enum_options_exhaustive() {
        let taps = [
            Tap::AfterRecord,
            Tap::AfterPlayback,
            Tap::AfterAgc,
            Tap::AfterVas,
            Tap::AfterDecoder,
            Tap::RouteOutput,
            Tap::AfterVolumeAmp,
            Tap::AfterSpeaker,
            Tap::AfterEarphone,
            Tap::AfterMicCapsule,
            Tap::AfterMicAmp,
        ];
        let want: Vec<(&str, u32)> = taps.iter().map(|&t| tap_value(t)).collect();
        let got: Vec<(&str, u32)> = options("tap")
            .iter()
            .map(|o| (o.identifier, o.value))
            .collect();
        assert_eq!(got, want);
        assert_eq!(
            tap_value(Tap::AFTER_STAGE_10),
            tap_value(Tap::AfterPlayback)
        ); // an alias
        let want: Vec<(&str, u32)> = [VasMode::Drop, VasMode::Mute].map(mode_value).to_vec();
        let got: Vec<(&str, u32)> = options("vas.mode")
            .iter()
            .map(|o| (o.identifier, o.value))
            .collect();
        assert_eq!(got, want);
        let want: Vec<(&str, u32)> = [Route::Speaker, Route::Earphone].map(route_value).to_vec();
        let got: Vec<(&str, u32)> = options("route")
            .iter()
            .map(|o| (o.identifier, o.value))
            .collect();
        assert_eq!(got, want);
    }

    /// 014 FR-013: every twin string is the Rust string plus a NUL.
    #[test]
    fn c_twins_equal_strings_plus_nul() {
        fn nul(s: &str) -> Vec<u8> {
            let mut v = s.as_bytes().to_vec();
            v.push(0);
            v
        }
        for (e, c) in controls().iter().zip(__c::CONTROLS.iter()) {
            assert_eq!(c.identifier, nul(e.identifier));
            assert_eq!(c.name, nul(e.name));
            assert_eq!(c.unit, nul(e.unit));
            assert_eq!(c.description, nul(e.description));
            let opts: &[ControlOption] = match e.value {
                ControlValue::Choice { options, .. } => options,
                _ => &[],
            };
            assert_eq!(c.options.len(), opts.len());
            for (o, t) in opts.iter().zip(c.options) {
                assert_eq!(*t, nul(o.identifier));
            }
            assert_eq!(c.sources.len(), e.sources.len());
            for (s, t) in e.sources.iter().zip(c.sources) {
                assert_eq!(*t, nul(s));
            }
            assert_eq!(c.assumptions.len(), e.assumptions.len());
            for (a, t) in e.assumptions.iter().zip(c.assumptions) {
                assert_eq!(*t, nul(a));
            }
        }
    }

    /// The const form rules, called at run time so coverage sees them (spec 007 lesson), and
    /// their refusals.
    #[test]
    fn form_rules_called_at_runtime() {
        check_controls(&CONTROLS);
        assert!(is_control_identifier("vas.sensitivity"));
        assert!(!is_control_identifier(""));
        assert!(!is_control_identifier("Vas"));
        assert!(!is_control_identifier("a_very_long_identifier_beyond_32_c"));
        assert!(one_sentence_no_digits("Sets the level."));
        assert!(!one_sentence_no_digits("Sets it to 3 dB."));
        assert!(!one_sentence_no_digits("One. Two."));
        assert!(value_ok(&ControlValue::Integer {
            min: 1,
            max: 5,
            default: 3
        }));
        assert!(!value_ok(&ControlValue::Integer {
            min: 1,
            max: 5,
            default: 6
        }));
        assert!(!value_ok(&ControlValue::Continuous {
            min: 0.0,
            max: 1.0,
            default: 2.0
        }));
        assert!(!value_ok(&ControlValue::Choice {
            options: &[],
            default: None
        }));
        const A: ControlOption = ControlOption {
            identifier: "a",
            value: 0,
        };
        const B: ControlOption = ControlOption {
            identifier: "b",
            value: 1,
        };
        assert!(value_ok(&ControlValue::Choice {
            options: &[A, B],
            default: Some(1)
        }));
        assert!(!value_ok(&ControlValue::Choice {
            options: &[A, B],
            default: Some(2)
        }));
        assert!(!value_ok(&ControlValue::Choice {
            options: &[A, A],
            default: None
        }));
        assert!(!value_ok(&ControlValue::Choice {
            options: &[ControlOption {
                identifier: "A",
                value: 0
            }],
            default: None
        }));
    }

    /// 014 FR-016 (hardware review B9): every entry's unit, by an exhaustive match on `Setting`
    /// (a new variant fails to compile) for the validated settings, and literally for the rest.
    #[test]
    fn units_equal_the_engine() {
        fn setting_id(s: Setting) -> &'static str {
            match s {
                Setting::AgcKneeDbfs => "agc.knee_dbfs",
                Setting::AgcAttackMs => "agc.attack_ms",
                Setting::AgcReleaseMs => "agc.release_ms",
                Setting::VasSensitivity => "vas.sensitivity",
                Setting::VasThresholdDbfs => "vas.threshold_dbfs",
                Setting::VasHangMs => "vas.hang_ms",
                Setting::VasOnsetMs => "vas.onset_ms",
                Setting::MemoryCapacityS => "memory.capacity_s",
                Setting::MemoryMaxRecordings => "memory.max_recordings",
                Setting::MicHissOffsetDb => "mic.hiss_offset_db",
            }
        }
        let all = [
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
        ];
        for s in all {
            let (name, unit) = s.name_and_unit();
            let id = setting_id(s);
            assert_eq!(name, id);
            // The engine names no unit for a level or a count; the table's word is pinned here.
            let want = match (id, unit) {
                ("vas.sensitivity", "") => "level",
                (_, u) => u,
            };
            assert_eq!(control(id).map(|e| e.unit), Some(want), "{id}");
        }
        for (id, want) in [
            ("route", ""),
            ("volume", ""),
            ("playback_speed", "level"),
            ("host_rate_hz", "Hz"),
            ("seed", ""),
            ("tap", ""),
        ] {
            assert_eq!(control(id).map(|e| e.unit), Some(want), "{id}");
        }
        for e in controls() {
            if let ControlValue::Switch { .. } = e.value {
                assert_eq!(e.unit, "", "{}", e.identifier);
            }
        }
    }

    #[test]
    fn lookup_and_paths() {
        assert_eq!(control("vas.sensitivity").map(|e| e.stage), Some(5));
        assert!(control("battery").is_none());
        let v = control("volume").unwrap();
        assert!(v.paths.contains(ControlPaths::PLAYBACK));
        assert!(!v.paths.contains(ControlPaths::RECORD));
        assert_eq!(v.paths.bits(), 6);
        assert_eq!(ControlKind::HostParameter as u32, 4);
        assert_eq!(ControlTiming::HostFixed as u32, 3);
    }
}
