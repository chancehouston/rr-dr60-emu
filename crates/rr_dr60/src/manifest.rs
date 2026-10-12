//! The **stage manifest**: what the library says about itself (spec 007).
//!
//! Constant data describing the fifteen stages of the RR-DR60 signal chain
//! (`docs/hardware/signal-chain.md`), one [`StageEntry`] per stage under a [`Header`]. An app
//! reads it once at start-up and generates its "about this model" view from it, so the view
//! cannot drift from the project's hardware registers: a harness test cross-checks every entry
//! against `docs/hardware/sources.md`, `docs/hardware/assumptions.md`, the signal-chain table
//! and the CHANGELOG (spec 007 FR-013), and a unit test here maps every bypass control and
//! tap in the public API to a stage and asserts that stage is modeled, so a stage modeled in
//! code cannot stay out of the manifest unnoticed. That guard binds the manifest to the
//! registers; the code is bound to the manifest by that unit test, and to the registers by
//! the traceability script and review.
//!
//! The strings describe only what each stage does to the signal (the claims policy in
//! `CLAUDE.md`): they say "modeled on" rather than claiming accuracy, they state sourced
//! properties only (007 FR-010), and they make no claim, for or against, about what
//! recordings contain. They are English; an app that localizes keys its translations on the
//! stable [`StageEntry::identifier`], never on the English text.
//!
//! **Status** (007 FR-004), derived from the assumption register's Status column:
//!
//! - [`StageStatus::NotModeled`]: the engine passes the signal through or represents the stage
//!   as a label.
//! - [`StageStatus::Assumed`]: the engine models the stage and at least one cited assumption is
//!   open (neither verified against a document nor measured on a real unit).
//! - [`StageStatus::Sourced`]: modeled, and every cited assumption is verified against a
//!   document; none is measured. Applies to no stage today.
//! - [`StageStatus::Checked`]: modeled, every cited assumption is verified or measured, and at
//!   least one is measured against a real unit. Applies to no stage today.
//!
//! The register links in the header are page links; the pages have no row anchors, so a host
//! that wants to land near an ID appends a text fragment, `#:~:text=` followed by the ID (for
//! example `…/assumptions.md#:~:text=A-028`), which scrolls to the ID's first mention on the
//! page.
//!
//! Reading the manifest allocates nothing, performs no I/O and takes no lock: it is a
//! `static` (007 FR-016). The data is identical on every supported platform.

use crate::settings::DEVICE_RATE_HZ;

/// The number of stages in the signal chain (`docs/hardware/signal-chain.md`, fifteen rows).
pub const STAGE_COUNT: usize = 15;

/// Whether and on what basis the engine models a stage (spec 007 FR-004).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StageStatus {
    /// The engine does not change the signal at this stage, or represents it only as a label.
    NotModeled = 0,
    /// Modeled, and at least one cited assumption is open.
    Assumed = 1,
    /// Modeled, every cited assumption verified against a document, none measured (none today).
    Sourced = 2,
    /// Modeled, every cited assumption verified or measured, at least one measured against a
    /// real unit (none today).
    Checked = 3,
}

/// Which signal path a stage belongs to (spec 007 FR-002).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StagePath {
    /// The record path, microphone to memory.
    Record = 0,
    /// The playback path, memory to speaker or earphone.
    Playback = 1,
    /// Both paths (the memory, the clock, the power supply).
    Both = 2,
}

/// The project's confidence in a modeled stage (spec 007 FR-006): at most the lowest headline
/// confidence among the current assumptions the stage cites; `NotApplicable` exactly for stages
/// that are not modeled.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Confidence {
    /// The stage is not modeled.
    NotApplicable = 0,
    /// Low.
    Low = 1,
    /// Medium.
    Medium = 2,
    /// High.
    High = 3,
}

/// How many stages carry each [`StageStatus`] (spec 007 FR-003); the four sum to
/// [`Header::stage_count`].
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatusCounts {
    /// Stages with [`StageStatus::NotModeled`].
    pub not_modeled: u32,
    /// Stages with [`StageStatus::Assumed`].
    pub assumed: u32,
    /// Stages with [`StageStatus::Sourced`].
    pub sourced: u32,
    /// Stages with [`StageStatus::Checked`].
    pub checked: u32,
}

/// The manifest's header (spec 007 FR-003).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// The library's own version, equal to [`crate::VERSION`].
    pub library_version: &'static str,
    /// The device's internal sample rate, 6000 Hz (A-026).
    pub device_rate_hz: u32,
    /// The number of stage entries, 15.
    pub stage_count: u32,
    /// How many stages carry each status.
    pub counts: StatusCounts,
    /// One neutral sentence on what the model rests on (007 FR-012).
    pub basis: &'static str,
    /// The hardware-evidence capture form, so an app can offer "Own one? Help check it".
    pub evidence_link: &'static str,
    /// The assumption register page (append `#:~:text=A-028` to scroll to the ID's first mention).
    pub assumptions_link: &'static str,
    /// The source register page (append `#:~:text=S-002` to scroll to the ID's first mention).
    pub sources_link: &'static str,
}

/// One stage of the signal chain (spec 007 FR-002).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StageEntry {
    /// 1 to 15, the signal-chain table's row number; stable (renumbering is a breaking change).
    pub number: u8,
    /// A stable ASCII key (`[a-z0-9_]`, at most 24 characters), equal to the signal-chain
    /// table's Identifier cell; the key for localization. Renaming is a breaking change.
    pub identifier: &'static str,
    /// The display name (at most 48 characters; may change in a minor version).
    pub name: &'static str,
    /// One English sentence (at most 160 characters) on what the stage does to sound, or
    /// supplies to the chain, on the device; sourced properties only (007 FR-010).
    pub description: &'static str,
    /// Whether and on what basis the engine models the stage.
    pub status: StageStatus,
    /// Which path the stage belongs to.
    pub path: StagePath,
    /// The project's confidence in the model; `NotApplicable` iff not modeled.
    pub confidence: Confidence,
    /// The source IDs (`S-###`) the stage's model rests on, ascending.
    pub sources: &'static [&'static str],
    /// The current assumption IDs (`A-###`) the stage's model rests on, ascending.
    pub assumptions: &'static [&'static str],
    /// What the engine leaves out at this stage (at most 120 characters); empty when nothing is
    /// left out or the stage is not modeled.
    pub coverage: &'static str,
    /// The settings, taps, state reads and reports that touch the stage, as verbatim Rust
    /// paths (VAS events are called splices; at most 120 characters); empty iff the engine
    /// offers no control or read-out, so non-empty for every modeled stage (007 FR-007).
    pub controls: &'static str,
    /// The library version in which the stage's modeled behavior last changed (007 FR-008);
    /// empty iff not modeled.
    pub model_version: &'static str,
}

/// The manifest: the header and the fifteen stage entries in signal-chain order.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Manifest {
    /// The header.
    pub header: Header,
    /// The fifteen entries, `stages[i].number == i + 1`.
    pub stages: [StageEntry; STAGE_COUNT],
}

impl Manifest {
    /// The number of stage entries.
    pub const STAGE_COUNT: usize = STAGE_COUNT;

    /// The header.
    #[must_use]
    pub const fn header(&self) -> &Header {
        &self.header
    }

    /// The fifteen entries in signal-chain order.
    #[must_use]
    pub const fn stages(&self) -> &[StageEntry] {
        &self.stages
    }

    /// The entry with the given stage number, or `None` for 0 or above 15 (007 FR-016).
    #[must_use]
    pub fn stage(&self, number: u32) -> Option<&StageEntry> {
        // 0 wraps to usize::MAX and misses; numbers above 15 miss.
        self.stages.get((number as usize).wrapping_sub(1))
    }
}

/// The stage manifest (spec 007). Constant; reading it allocates nothing and does no I/O.
#[must_use]
pub fn manifest() -> &'static Manifest {
    // Referencing the named constant makes every rustc back to the MSRV count the compile-time
    // checks as used (1.85's dead-code lint ignores uses from anonymous `const _` items).
    let () = FORM_RULES_HOLD;
    &MANIFEST
}

// The header's strings, as macros so that `concat!` can build their NUL-terminated C twins
// from the same literals (research R-01).
macro_rules! basis_text {
    () => {
        // 007 FR-012: the sentence while no stage is checked against a real unit.
        "Modeled on the RR-DR60 owner's and service manuals and the codec family datasheets; no stage has been checked against a real unit."
    };
}
macro_rules! evidence_link {
    () => {
        "https://github.com/chancehouston/rr-dr60-emu/issues/new?template=hardware_evidence.yml"
    };
}
macro_rules! assumptions_link {
    () => {
        "https://github.com/chancehouston/rr-dr60-emu/blob/main/docs/hardware/assumptions.md"
    };
}
macro_rules! sources_link {
    () => {
        "https://github.com/chancehouston/rr-dr60-emu/blob/main/docs/hardware/sources.md"
    };
}

/// Builds the public table and its hidden C twin from one list (research R-01), so the two
/// cannot drift: every string in `__c` is the same literal with a trailing NUL.
macro_rules! stage_table {
    ($( {
        number: $n:literal, identifier: $id:literal, name: $name:literal,
        description: $desc:literal, status: $status:ident, path: $path:ident,
        confidence: $conf:ident, sources: [$($s:literal),* $(,)?],
        assumptions: [$($a:literal),* $(,)?], coverage: $cov:literal,
        controls: $ctl:literal, model_version: $mv:literal $(,)?
    } ),* $(,)?) => {
        const STAGES: [StageEntry; STAGE_COUNT] = [ $( StageEntry {
            number: $n,
            identifier: $id,
            name: $name,
            description: $desc,
            status: StageStatus::$status,
            path: StagePath::$path,
            confidence: Confidence::$conf,
            sources: &[$($s),*],
            assumptions: &[$($a),*],
            coverage: $cov,
            controls: $ctl,
            model_version: $mv,
        } ),* ];

        /// NUL-terminated twins of every manifest string, for the C API (spec 007 FR-017;
        /// research R-01). Not part of the public API; may change without notice.
        #[doc(hidden)]
        pub mod __c {
            /// The header with NUL-terminated strings.
            pub struct HeaderC {
                /// `library_version` + NUL.
                pub library_version: &'static [u8],
                /// As [`super::Header::device_rate_hz`].
                pub device_rate_hz: u32,
                /// As [`super::Header::stage_count`].
                pub stage_count: u32,
                /// As [`super::StatusCounts::not_modeled`].
                pub count_not_modeled: u32,
                /// As [`super::StatusCounts::assumed`].
                pub count_assumed: u32,
                /// As [`super::StatusCounts::sourced`].
                pub count_sourced: u32,
                /// As [`super::StatusCounts::checked`].
                pub count_checked: u32,
                /// `basis` + NUL.
                pub basis: &'static [u8],
                /// `evidence_link` + NUL.
                pub evidence_link: &'static [u8],
                /// `assumptions_link` + NUL.
                pub assumptions_link: &'static [u8],
                /// `sources_link` + NUL.
                pub sources_link: &'static [u8],
            }

            /// One entry with NUL-terminated strings and the enums as their `u32` values.
            pub struct StageEntryC {
                /// As [`super::StageEntry::number`].
                pub number: u32,
                /// `identifier` + NUL.
                pub identifier: &'static [u8],
                /// `name` + NUL.
                pub name: &'static [u8],
                /// `description` + NUL.
                pub description: &'static [u8],
                /// [`super::StageStatus`] as `u32`.
                pub status: u32,
                /// [`super::StagePath`] as `u32`.
                pub path: u32,
                /// [`super::Confidence`] as `u32`.
                pub confidence: u32,
                /// Each source ID + NUL.
                pub sources: &'static [&'static [u8]],
                /// Each assumption ID + NUL.
                pub assumptions: &'static [&'static [u8]],
                /// `coverage` + NUL.
                pub coverage: &'static [u8],
                /// `controls` + NUL.
                pub controls: &'static [u8],
                /// `model_version` + NUL.
                pub model_version: &'static [u8],
            }

            /// The header's twin.
            pub static HEADER: HeaderC = HeaderC {
                library_version: concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes(),
                device_rate_hz: super::HEADER.device_rate_hz,
                stage_count: super::HEADER.stage_count,
                count_not_modeled: super::HEADER.counts.not_modeled,
                count_assumed: super::HEADER.counts.assumed,
                count_sourced: super::HEADER.counts.sourced,
                count_checked: super::HEADER.counts.checked,
                basis: concat!(basis_text!(), "\0").as_bytes(),
                evidence_link: concat!(evidence_link!(), "\0").as_bytes(),
                assumptions_link: concat!(assumptions_link!(), "\0").as_bytes(),
                sources_link: concat!(sources_link!(), "\0").as_bytes(),
            };

            /// The fifteen entries' twins, in signal-chain order.
            pub static STAGES: [StageEntryC; super::STAGE_COUNT] = [ $( StageEntryC {
                number: $n,
                identifier: concat!($id, "\0").as_bytes(),
                name: concat!($name, "\0").as_bytes(),
                description: concat!($desc, "\0").as_bytes(),
                status: super::StageStatus::$status as u32,
                path: super::StagePath::$path as u32,
                confidence: super::Confidence::$conf as u32,
                sources: &[$(concat!($s, "\0").as_bytes()),*],
                assumptions: &[$(concat!($a, "\0").as_bytes()),*],
                coverage: concat!($cov, "\0").as_bytes(),
                controls: concat!($ctl, "\0").as_bytes(),
                model_version: concat!($mv, "\0").as_bytes(),
            } ),* ];
        }
    };
}

// The content: spec 007 › Key Entities (after the plan reviews of 2026-10-12); data-model.md ›
// The fifteen entries. Descriptions state sourced properties only (S-001, S-002, S-004, S-005;
// 007 FR-010). Controls notes are verbatim Rust paths (007 FR-007). Model versions per 007
// FR-008 and the CHANGELOG.
stage_table! {
    {
        number: 1, identifier: "mic_capsule", name: "Microphone capsule",
        description: "Turns sound pressure into an electrical signal with a two-terminal electret capsule.",
        status: NotModeled, path: Record, confidence: NotApplicable,
        sources: ["S-002"], assumptions: ["A-005"],
        coverage: "", controls: "", model_version: "",
    },
    {
        number: 2, identifier: "mic_amp", name: "Microphone amplifier",
        description: "Amplifies the microphone signal ahead of the codec and carries the AGC's attenuator.",
        status: NotModeled, path: Record, confidence: NotApplicable,
        sources: ["S-002"], assumptions: ["A-006", "A-029"],
        coverage: "", controls: "", model_version: "",
    },
    {
        number: 3, identifier: "limiter", name: "Limiter (AGC)",
        description: "Limits loud sounds at the codec's input: a peak-following circuit charges a hold capacitor that attenuates the microphone signal, then releases.",
        status: Assumed, path: Record, confidence: Low,
        sources: ["S-001", "S-002"],
        assumptions: ["A-007", "A-019", "A-020", "A-028", "A-030", "A-034", "A-035", "A-036", "A-037"],
        coverage: "a soft knee, if the circuit has one; the DC block and 13 kHz roll-off ahead of the peak circuit",
        controls: "agc.enabled, agc.knee_dbfs, agc.attack_ms, agc.release_ms; Tap::AfterAgc",
        model_version: "0.5.0",
    },
    {
        number: 4, identifier: "codec_input", name: "Codec input: filter, ADC and µ-law",
        description: "Narrows the signal to the codec's voice band, samples it at 6 kHz as 8-bit µ-law and clips anything beyond full scale.",
        status: Assumed, path: Record, confidence: Medium,
        sources: ["S-002", "S-003", "S-005", "S-007"],
        assumptions: ["A-003", "A-015", "A-016", "A-026", "A-027", "A-030"],
        coverage: "µ-law quantization, idle noise, the input amp's clipping ahead of the filter, and aliasing of input above 3 kHz",
        controls: "record_stage_enabled; adc_clamp_enabled; Tap::AfterRecord",
        model_version: "0.5.0",
    },
    {
        number: 5, identifier: "vas", name: "Voice-activated pause (VAS)",
        description: "Pauses the recording while the input stays quiet and resumes when sound returns, leaving the quiet gaps out.",
        status: Assumed, path: Record, confidence: Low,
        sources: ["S-001", "S-002"],
        assumptions: ["A-008", "A-021", "A-022", "A-023", "A-024", "A-025", "A-033", "A-037"],
        coverage: "a digital gain from the sensitivity setting, if the device applies one",
        controls: "vas.enabled, mode, sensitivity, threshold_dbfs, hang_ms, onset_ms; Tap::AfterVas; splices (VasEvent); BlockInfo.paused",
        model_version: "0.5.0",
    },
    {
        number: 6, identifier: "noise_reduction", name: "Noise reduction",
        description: "Claimed by one secondary account to reduce background noise; the service manual lists no such function.",
        status: NotModeled, path: Record, confidence: NotApplicable,
        sources: ["S-002", "S-004"], assumptions: ["A-009"],
        coverage: "", controls: "", model_version: "",
    },
    {
        number: 7, identifier: "encoder", name: "Speech encoder (CELP, 4 kbit/s)",
        description: "Compresses the voice-band stream into 4 kbit/s CELP frames for storage.",
        status: NotModeled, path: Record, confidence: NotApplicable,
        sources: ["S-002", "S-004"], assumptions: ["A-004"],
        coverage: "", controls: "", model_version: "",
    },
    {
        number: 8, identifier: "memory", name: "Flash memory",
        description: "Stores up to 99 recordings and 60 minutes in total; a new recording cannot start once either limit is reached.",
        status: Assumed, path: Both, confidence: Low,
        sources: ["S-001", "S-002"], assumptions: ["A-037", "A-038", "A-039"],
        coverage: "encoded CELP frames as the stored form (the engine keeps its own samples); item locking (F.LOCK)",
        controls: "memory.capacity_s, memory.max_recordings; memory_status, recording_length, erase, erase_all, export, import",
        model_version: "0.6.0",
    },
    {
        number: 9, identifier: "decoder", name: "Speech decoder (CELP)",
        description: "Rebuilds the voice-band stream from the stored CELP frames.",
        status: NotModeled, path: Playback, confidence: NotApplicable,
        sources: ["S-002"], assumptions: ["A-004"],
        coverage: "", controls: "", model_version: "",
    },
    {
        number: 10, identifier: "codec_output", name: "Codec output: DAC and reconstruction filter",
        description: "Converts the stream back to an analog signal and smooths it with the codec's reconstruction low-pass filter.",
        status: Assumed, path: Playback, confidence: Medium,
        sources: ["S-002", "S-003", "S-005", "S-007"],
        assumptions: ["A-015", "A-016", "A-026", "A-027", "A-030"],
        coverage: "µ-law expansion, idle noise on the playback side, DAC imaging residue above 3 kHz; the SLOW/FAST speed control",
        controls: "playback_stage_enabled; Tap::AfterPlayback",
        model_version: "0.4.0",
    },
    {
        number: 11, identifier: "volume_amp", name: "Volume control and power amplifier",
        description: "Sets the playback level and drives the speaker through a power amplifier that is muted while recording.",
        status: NotModeled, path: Playback, confidence: NotApplicable,
        sources: ["S-001", "S-002", "S-006"], assumptions: ["A-031"],
        coverage: "", controls: "set_route, route; EngineState.speaker_muted", model_version: "",
    },
    {
        number: 12, identifier: "speaker", name: "Speaker",
        description: "Turns the amplified signal into sound through a 2.8 cm, 8 Ω speaker.",
        status: NotModeled, path: Playback, confidence: NotApplicable,
        sources: ["S-001", "S-002"], assumptions: ["A-010"],
        coverage: "", controls: "Route::Speaker", model_version: "",
    },
    {
        number: 13, identifier: "earphone", name: "Earphone output",
        description: "Feeds the volume-controlled signal to the 2.5 mm earphone jack, bypassing the power amplifier.",
        status: NotModeled, path: Playback, confidence: NotApplicable,
        sources: ["S-001", "S-002"], assumptions: ["A-012", "A-032"],
        coverage: "", controls: "Route::Earphone", model_version: "",
    },
    {
        number: 14, identifier: "clock", name: "Clock",
        description: "An 8 MHz resonator clocks the DSP, which derives the codec's 6 kHz sampling clock from it.",
        status: NotModeled, path: Both, confidence: NotApplicable,
        sources: ["S-002"], assumptions: ["A-011"],
        coverage: "", controls: "", model_version: "",
    },
    {
        number: 15, identifier: "power", name: "Power supply",
        description: "Two AAA cells feed a 3.3 V converter for the digital section and a switched rail for the analog section.",
        status: NotModeled, path: Both, confidence: NotApplicable,
        sources: ["S-001", "S-002"], assumptions: ["A-013"],
        coverage: "", controls: "", model_version: "",
    },
}

const HEADER: Header = Header {
    library_version: crate::VERSION,
    device_rate_hz: DEVICE_RATE_HZ, // A-026
    stage_count: STAGE_COUNT as u32,
    counts: count_statuses(&STAGES),
    basis: basis_text!(),
    evidence_link: evidence_link!(),
    assumptions_link: assumptions_link!(),
    sources_link: sources_link!(),
};

static MANIFEST: Manifest = Manifest {
    header: HEADER,
    stages: STAGES,
};

/// Tallies the entries' statuses at compile time (007 FR-003), so the header cannot disagree
/// with the entries.
const fn count_statuses(stages: &[StageEntry; STAGE_COUNT]) -> StatusCounts {
    let mut c = StatusCounts {
        not_modeled: 0,
        assumed: 0,
        sourced: 0,
        checked: 0,
    };
    let mut i = 0;
    while i < STAGE_COUNT {
        match stages[i].status {
            StageStatus::NotModeled => c.not_modeled += 1,
            StageStatus::Assumed => c.assumed += 1,
            StageStatus::Sourced => c.sourced += 1,
            StageStatus::Checked => c.checked += 1,
        }
        i += 1;
    }
    c
}

// ---- Compile-time form rules (007 FR-002, FR-003; research R-01) ----------------------------
// Lengths are in characters (UTF-8 scalar values), the spec's unit. The limits are engineering
// targets (007 FR-002, FR-003).

/// The number of UTF-8 scalar values in `s` (bytes that are not continuation bytes).
const fn chars(s: &str) -> usize {
    let b = s.as_bytes();
    let mut i = 0;
    let mut n = 0;
    while i < b.len() {
        if (b[i] & 0xC0) != 0x80 {
            n += 1;
        }
        i += 1;
    }
    n
}

const fn bytes_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Lexicographic `a < b` on byte strings.
const fn bytes_lt(a: &[u8], b: &[u8]) -> bool {
    let mut i = 0;
    while i < a.len() && i < b.len() {
        if a[i] != b[i] {
            return a[i] < b[i];
        }
        i += 1;
    }
    a.len() < b.len()
}

/// `[a-z0-9_]{1,24}` (007 FR-002).
const fn is_identifier(s: &str) -> bool {
    let b = s.as_bytes();
    if b.is_empty() || b.len() > 24 {
        return false;
    }
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_') {
            return false;
        }
        i += 1;
    }
    true
}

/// `P-###` with `P` the given prefix (007 FR-002: the registers' ID strings verbatim).
const fn is_id(s: &str, prefix: u8) -> bool {
    let b = s.as_bytes();
    b.len() == 5
        && b[0] == prefix
        && b[1] == b'-'
        && b[2].is_ascii_digit()
        && b[3].is_ascii_digit()
        && b[4].is_ascii_digit()
}

/// Every ID well-formed and the list strictly ascending (007 FR-002).
const fn ids_ok(list: &[&str], prefix: u8) -> bool {
    let mut i = 0;
    while i < list.len() {
        if !is_id(list[i], prefix) {
            return false;
        }
        if i > 0 && !bytes_lt(list[i - 1].as_bytes(), list[i].as_bytes()) {
            return false;
        }
        i += 1;
    }
    true
}

const fn ends_with_dot(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty() && b[b.len() - 1] == b'.'
}

const fn check_entries(stages: &[StageEntry; STAGE_COUNT]) {
    let mut i = 0;
    while i < STAGE_COUNT {
        let e = &stages[i];
        assert!(
            e.number as usize == i + 1,
            "stage numbers must be 1..=15 in order"
        );
        assert!(
            is_identifier(e.identifier),
            "identifier: lowercase ASCII letters, digits and underscores, 1 to 24 characters"
        );
        assert!(chars(e.name) <= 48, "name: at most 48 characters"); // engineering target (007 FR-002)
        assert!(
            chars(e.description) <= 160,
            "description: at most 160 characters"
        ); // engineering target (007 FR-002)
        assert!(
            ends_with_dot(e.description),
            "description: one sentence ending in '.'"
        );
        assert!(chars(e.coverage) <= 120, "coverage: at most 120 characters"); // engineering target (007 FR-002)
        assert!(chars(e.controls) <= 120, "controls: at most 120 characters"); // engineering target (007 FR-007)
        assert!(ids_ok(e.sources, b'S'), "sources: S-### ascending");
        assert!(ids_ok(e.assumptions, b'A'), "assumptions: A-### ascending");
        let not_modeled = matches!(e.status, StageStatus::NotModeled);
        let not_applicable = matches!(e.confidence, Confidence::NotApplicable);
        if not_modeled {
            assert!(
                not_applicable,
                "a stage that is not modeled has confidence NotApplicable"
            );
            assert!(
                e.model_version.is_empty(),
                "a stage that is not modeled has no model version"
            );
            assert!(
                e.coverage.is_empty(),
                "a stage that is not modeled has no coverage note"
            );
        } else {
            assert!(!not_applicable, "a modeled stage has a confidence level");
            assert!(
                !e.model_version.is_empty(),
                "a modeled stage has a model version"
            );
            assert!(
                !e.controls.is_empty(),
                "a modeled stage has a controls note (007 FR-013 g)"
            );
        }
        // Identifiers pairwise unique.
        let mut j = 0;
        while j < i {
            assert!(
                !bytes_eq(stages[j].identifier.as_bytes(), e.identifier.as_bytes()),
                "identifiers must be unique"
            );
            j += 1;
        }
        i += 1;
    }
}

const fn check_header(h: &Header) {
    assert!(chars(h.basis) <= 160, "basis: at most 160 characters"); // engineering target (007 FR-003)
    assert!(
        chars(h.evidence_link) <= 200,
        "link: at most 200 characters"
    ); // engineering target (007 FR-003)
    assert!(
        chars(h.assumptions_link) <= 200,
        "link: at most 200 characters"
    );
    assert!(chars(h.sources_link) <= 200, "link: at most 200 characters");
    assert!(h.stage_count as usize == STAGE_COUNT, "stage count");
    let c = h.counts;
    assert!(
        c.not_modeled + c.assumed + c.sourced + c.checked == h.stage_count,
        "counts sum to the stage count"
    );
}

/// The form rules of 007 FR-002 and FR-003, evaluated at compile time: a string over a limit or
/// a broken invariant fails the build with the rule's message.
const FORM_RULES_HOLD: () = {
    check_entries(&STAGES);
    check_header(&HEADER);
};
const _: () = FORM_RULES_HOLD;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Settings;

    #[test]
    fn counts_equal_runtime_tally() {
        // 007 FR-003, FR-013 (f).
        let m = manifest();
        let mut tally = StatusCounts::default();
        for e in m.stages() {
            match e.status {
                StageStatus::NotModeled => tally.not_modeled += 1,
                StageStatus::Assumed => tally.assumed += 1,
                StageStatus::Sourced => tally.sourced += 1,
                StageStatus::Checked => tally.checked += 1,
            }
        }
        assert_eq!(m.header.counts, tally);
        assert_eq!(
            tally.not_modeled + tally.assumed + tally.sourced + tally.checked,
            15
        );
    }

    fn twin_ok(twin: &[u8], s: &str) -> bool {
        twin.len() == s.len() + 1 && &twin[..s.len()] == s.as_bytes() && twin[s.len()] == 0
    }

    #[test]
    fn c_twins_equal_strings_plus_nul() {
        // 007 FR-017; research R-01: the hidden table cannot drift from the public one.
        let m = manifest();
        let h = &__c::HEADER;
        assert!(twin_ok(h.library_version, m.header.library_version));
        assert!(twin_ok(h.basis, m.header.basis));
        assert!(twin_ok(h.evidence_link, m.header.evidence_link));
        assert!(twin_ok(h.assumptions_link, m.header.assumptions_link));
        assert!(twin_ok(h.sources_link, m.header.sources_link));
        assert_eq!(h.device_rate_hz, m.header.device_rate_hz);
        assert_eq!(h.stage_count, m.header.stage_count);
        assert_eq!(h.count_not_modeled, m.header.counts.not_modeled);
        assert_eq!(h.count_assumed, m.header.counts.assumed);
        assert_eq!(h.count_sourced, m.header.counts.sourced);
        assert_eq!(h.count_checked, m.header.counts.checked);
        for (e, c) in m.stages().iter().zip(__c::STAGES.iter()) {
            assert_eq!(c.number, u32::from(e.number));
            assert!(twin_ok(c.identifier, e.identifier));
            assert!(twin_ok(c.name, e.name));
            assert!(twin_ok(c.description, e.description));
            assert!(twin_ok(c.coverage, e.coverage));
            assert!(twin_ok(c.controls, e.controls));
            assert!(twin_ok(c.model_version, e.model_version));
            assert_eq!(c.status, e.status as u32);
            assert_eq!(c.path, e.path as u32);
            assert_eq!(c.confidence, e.confidence as u32);
            assert_eq!(c.sources.len(), e.sources.len());
            assert_eq!(c.assumptions.len(), e.assumptions.len());
            for (t, s) in c.sources.iter().zip(e.sources.iter()) {
                assert!(twin_ok(t, s));
            }
            for (t, s) in c.assumptions.iter().zip(e.assumptions.iter()) {
                assert!(twin_ok(t, s));
            }
        }
    }

    #[test]
    fn library_version_is_crate_version() {
        assert_eq!(manifest().header.library_version, crate::VERSION);
        assert_eq!(manifest().header.device_rate_hz, 6000); // A-026
    }

    #[test]
    fn bypass_control_anchor() {
        // Research R-01 (hardware reviews P5 and G1): every bypass control and tap in the public
        // API maps to a stage, and that stage must be modeled in the manifest. The destructuring
        // is exhaustive, so a new control or tap fails to compile until it is mapped here, and
        // mapping it to a stage the manifest calls "not modeled" fails the assertion. This is
        // the code → manifest half of the guard (constitution Principle V).
        use crate::{AgcSettings, MemorySettings, Tap, VasSettings};
        let Settings {
            host_rate_hz: _,
            record_stage_enabled,
            playback_stage_enabled,
            adc_clamp_enabled,
            tap,
            seed: _,
            agc,
            vas,
            memory,
        } = Settings::new(6000);
        let AgcSettings {
            enabled: agc_enabled,
            knee_dbfs: _,
            attack_ms: _,
            release_ms: _,
        } = agc;
        let VasSettings {
            enabled: vas_enabled,
            mode: _,
            sensitivity: _,
            threshold_dbfs: _,
            hang_ms: _,
            onset_ms: _,
        } = vas;
        let MemorySettings {
            capacity_s,
            max_recordings: _,
        } = memory;
        let controls: &[(u32, bool)] = &[
            (3, agc_enabled),
            (4, record_stage_enabled),
            (4, adc_clamp_enabled),
            (5, vas_enabled),
            (8, capacity_s > 0),
            (10, playback_stage_enabled),
        ];
        let m = manifest();
        for (n, on) in controls {
            assert!(on, "stage {n}'s control is off in the device defaults");
            let e = m.stage(*n).unwrap();
            assert_ne!(
                e.status,
                StageStatus::NotModeled,
                "stage {n} has a control in the API but the manifest says not modeled"
            );
        }
        for t in [
            Tap::AfterAgc,
            Tap::AfterRecord,
            Tap::AfterVas,
            Tap::AfterPlayback,
            tap,
        ] {
            let n = match t {
                Tap::AfterAgc => 3,
                Tap::AfterRecord => 4,
                Tap::AfterVas => 5,
                Tap::AfterPlayback => 10,
            };
            assert_ne!(
                m.stage(n).unwrap().status,
                StageStatus::NotModeled,
                "tap {t:?} exposes stage {n} but the manifest says not modeled"
            );
        }
    }

    #[test]
    fn char_count_matches_std() {
        fn check(s: &str) {
            assert_eq!(chars(s), s.chars().count(), "{s}");
        }
        let m = manifest();
        for s in [
            m.header.basis,
            m.header.evidence_link,
            m.header.assumptions_link,
            m.header.sources_link,
        ] {
            check(s);
        }
        for e in m.stages() {
            for s in [
                e.identifier,
                e.name,
                e.description,
                e.coverage,
                e.controls,
                e.model_version,
            ] {
                check(s);
            }
        }
        assert_eq!(chars("µ-law Ω ≈"), 9);
    }

    #[test]
    fn lookup_edges() {
        let m = manifest();
        assert!(m.stage(0).is_none());
        assert!(m.stage(16).is_none());
        assert_eq!(m.stage(1).unwrap().identifier, "mic_capsule");
        assert_eq!(m.stage(15).unwrap().identifier, "power");
    }

    // ---- The compile-time rules, run at runtime so the profiler sees them (007 SC-008) ----

    /// Over 160 characters: over every limit but the links'.
    const LONG: &str = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx.";
    /// 210 characters: over the link limit.
    const LONG_LINK: &str = "https://example.invalid/xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";

    #[test]
    fn const_helpers_at_runtime() {
        assert_eq!(chars("abc"), 3);
        assert_eq!(chars("µΩ"), 2);
        assert!(bytes_eq(b"ab", b"ab"));
        assert!(!bytes_eq(b"ab", b"ac"));
        assert!(!bytes_eq(b"a", b"ab"));
        assert!(bytes_lt(b"A-001", b"A-002"));
        assert!(!bytes_lt(b"A-002", b"A-001"));
        assert!(bytes_lt(b"a", b"ab"));
        assert!(!bytes_lt(b"ab", b"ab"));
        assert!(is_identifier("mic_capsule"));
        assert!(!is_identifier(""));
        assert!(!is_identifier("Mic"));
        assert!(!is_identifier("a-b"));
        assert!(!is_identifier("abcdefghijklmnopqrstuvwxy"));
        assert!(is_id("A-028", b'A'));
        assert!(!is_id("A-28", b'A'));
        assert!(!is_id("S-028", b'A'));
        assert!(!is_id("A_028", b'A'));
        assert!(!is_id("A-02x", b'A'));
        assert!(ids_ok(&["A-001", "A-002"], b'A'));
        assert!(!ids_ok(&["A-002", "A-001"], b'A'));
        assert!(!ids_ok(&["A-001", "A-001"], b'A'));
        assert!(!ids_ok(&["A-001", "S-002"], b'A'));
        assert!(ends_with_dot("x."));
        assert!(!ends_with_dot("x"));
        assert!(!ends_with_dot(""));
        check_entries(&STAGES);
        check_header(&HEADER);
        assert!(LONG.chars().count() > 160);
        assert!(LONG_LINK.chars().count() > 200);
        let mut s = STAGES;
        s[0].status = StageStatus::Sourced;
        s[1].status = StageStatus::Checked;
        let c = count_statuses(&s);
        assert_eq!(
            (c.not_modeled, c.assumed, c.sourced, c.checked),
            (8, 5, 1, 1)
        );
    }

    macro_rules! entries_panic {
        ($name:ident, $msg:literal, |$s:ident| $body:expr) => {
            #[test]
            #[should_panic(expected = $msg)]
            fn $name() {
                let mut $s = STAGES;
                $body;
                check_entries(&$s);
            }
        };
    }
    entries_panic!(
        rule_number_order,
        "stage numbers must be 1..=15 in order",
        |s| s[0].number = 2
    );
    entries_panic!(
        rule_identifier_charset,
        "identifier: lowercase ASCII",
        |s| s[0].identifier = "Mic"
    );
    entries_panic!(rule_name_length, "name: at most 48 characters", |s| s[0]
        .name =
        LONG);
    entries_panic!(
        rule_description_length,
        "description: at most 160 characters",
        |s| s[0].description = LONG
    );
    entries_panic!(rule_description_dot, "one sentence ending in '.'", |s| s
        [0]
    .description =
        "No dot");
    entries_panic!(
        rule_coverage_length,
        "coverage: at most 120 characters",
        |s| s[2].coverage = LONG
    );
    entries_panic!(
        rule_controls_length,
        "controls: at most 120 characters",
        |s| s[2].controls = LONG
    );
    entries_panic!(rule_sources_order, "sources: S-### ascending", |s| s[0]
        .sources =
        &["S-002", "S-001"]);
    entries_panic!(rule_assumptions_form, "assumptions: A-### ascending", |s| {
        s[0].assumptions = &["A-1"]
    });
    entries_panic!(
        rule_not_modeled_confidence,
        "not modeled has confidence NotApplicable",
        |s| s[0].confidence = Confidence::Low
    );
    entries_panic!(
        rule_not_modeled_version,
        "not modeled has no model version",
        |s| s[0].model_version = "0.1.0"
    );
    entries_panic!(
        rule_not_modeled_coverage,
        "not modeled has no coverage note",
        |s| s[0].coverage = "x"
    );
    entries_panic!(
        rule_modeled_confidence,
        "a modeled stage has a confidence level",
        |s| s[2].confidence = Confidence::NotApplicable
    );
    entries_panic!(
        rule_modeled_version,
        "a modeled stage has a model version",
        |s| s[2].model_version = ""
    );
    entries_panic!(
        rule_modeled_controls,
        "a modeled stage has a controls note",
        |s| s[2].controls = ""
    );
    entries_panic!(rule_identifier_unique, "identifiers must be unique", |s| {
        s[1].identifier = "mic_capsule"
    });

    macro_rules! header_panic {
        ($name:ident, $msg:literal, |$h:ident| $body:expr) => {
            #[test]
            #[should_panic(expected = $msg)]
            fn $name() {
                let mut $h = HEADER;
                $body;
                check_header(&$h);
            }
        };
    }
    header_panic!(rule_basis_length, "basis: at most 160 characters", |h| h
        .basis =
        LONG);
    header_panic!(
        rule_evidence_link_length,
        "link: at most 200 characters",
        |h| h.evidence_link = LONG_LINK
    );
    header_panic!(
        rule_assumptions_link_length,
        "link: at most 200 characters",
        |h| h.assumptions_link = LONG_LINK
    );
    header_panic!(
        rule_sources_link_length,
        "link: at most 200 characters",
        |h| h.sources_link = LONG_LINK
    );
    header_panic!(rule_stage_count, "stage count", |h| h.stage_count = 14);
    header_panic!(rule_counts_sum, "counts sum to the stage count", |h| h
        .counts
        .assumed +=
        1);
}
