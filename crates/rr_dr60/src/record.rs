//! The processing record (spec 020): one versioned text document per recording that says
//! how the recording was processed, written beside the stored form and never inside it
//! (contracts/record-form.md); its reader; the stop fields a run carries; the cursors and
//! batch reads over a record's rows. Everything here is a pure function over caller
//! buffers: no allocation in either direction (research R-02).

use core::fmt::{self, Write as _};
use core::str::FromStr;

use crate::changes::{ChangeEntry, STREAM_ONE_PASS, STREAM_PLAYBACK, STREAM_RECORD};
use crate::controls::{controls, mode_code, mode_from_code, tap_code, tap_from_code};
use crate::digest::Fnv1a64;
use crate::error::Error;
use crate::memory::Encoding;
use crate::pipeline::VasEvent;
use crate::settings::Settings;
use crate::transport::{RecordInfo, StopOutcome, StopResult};

/// The record form's version this library writes (spec 020 D2); it reads every earlier one
/// (none yet) and refuses 0 and any later one.
pub const FORM_VERSION: u32 = 1; // engineering target (020 D2): the form's first version

/// The first line of every record.
pub const VERSION_LINE_NAME: &str = "rr_dr60_processing_record_version"; // engineering target (020 D2)

/// The number of modeled stages the record names (the stage manifest's count).
pub const STAGE_COUNT: usize = crate::manifest::STAGE_COUNT;

/// The read-out's flags (spec 020 D4; data-model.md): differences reported, never refusals.
pub mod flags {
    /// The record's library version differs from this library's.
    pub const LIBRARY_DIFFERS: u32 = 1; // FR-002: a reported difference (020 D4)
    /// A stage's model version differs (the stage mask names which).
    pub const STAGE_MODEL_DIFFERS: u32 = 2; // FR-002: a reported difference (020 D4)
    /// The record's host rate differs from the reading engine's (set by the engine's reader).
    pub const HOST_RATE_DIFFERS: u32 = 4; // FR-002: a reported difference (020 D4)
    /// Setting names or control identifiers this library does not know (counted and listed).
    pub const UNKNOWN_SETTINGS: u32 = 8; // FR-002: a reported difference (020 D4)
    /// Fewer event or change rows than the engine counted.
    pub const INCOMPLETE_RUN: u32 = 16; // FR-002: a reported difference (020 D4)
    /// The recording was imported: the record names the stored form and its digest only.
    pub const IMPORTED_ORIGIN: u32 = 32; // FR-002: a reported difference (020 D4)
    /// Checked against a recording in memory: the digests differ.
    pub const DIGEST_MISMATCH: u32 = 64; // FR-002: a reported difference (020 D4)
}

/// How a recording ended (spec 020 D9; research R-07).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StopHow {
    /// The host called `stop_recording` (also when its flush filled the memory).
    #[default]
    StopCall,
    /// The memory filled during a `record` call (the only self-stop, spec 006 FR-012).
    MemoryFull,
}

/// Where a recording came from (spec 020 D1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Origin {
    /// Recorded by the engine that wrote the record (a slot block exists).
    #[default]
    Engine,
    /// Imported from a stored form: identity only, nothing to re-render.
    Imported,
}

/// The stop's figures a run carries (spec 020 D9, D10): plain and buildable by anyone, from a
/// [`StopOutcome`], from the [`RecordInfo`] of a self-stop, or from a read record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct StopFields {
    /// How the recording ended.
    pub how: StopHow,
    /// The host samples fed before the stop was called; for a self-stop the input samples
    /// total (nothing after the fill is fed).
    pub stop_input_position: u64,
    /// The kept device sample at which the codec's padding begins; the length without
    /// padding. 0 from a self-stop's `RecordInfo`: the writer fills the length in.
    pub padding_start_sample: u64,
    /// Every host sample fed since the start, the stop's silence included (spec 017).
    pub input_samples_total: u64,
    /// The post-gate device samples of the padding (≤ 179; 0 without the codec or after a
    /// self-stop).
    pub silence_fed_samples: u64,
}

impl From<&StopOutcome> for StopFields {
    fn from(o: &StopOutcome) -> Self {
        Self {
            how: StopHow::StopCall,
            stop_input_position: o.stop_input_position,
            padding_start_sample: o.padding_start_sample,
            input_samples_total: o.input_samples_total,
            silence_fed_samples: o.silence_fed_samples,
        }
    }
}

impl From<&RecordInfo> for StopFields {
    /// The self-stop's fields (spec 020 research R-07) from the `record` call's report with
    /// `stopped` set: nothing after the fill was fed, no padding.
    fn from(i: &RecordInfo) -> Self {
        Self {
            how: StopHow::MemoryFull,
            stop_input_position: i.input_samples_total,
            padding_start_sample: 0,
            input_samples_total: i.input_samples_total,
            silence_fed_samples: 0,
        }
    }
}

impl From<&RecordReadout> for StopFields {
    fn from(r: &RecordReadout) -> Self {
        r.stop
    }
}

impl StopFields {
    /// `true` when the outcome kept the recording (the fields of a discarded one are
    /// meaningless).
    #[must_use]
    pub fn kept(o: &StopOutcome) -> bool {
        matches!(o.result, StopResult::Kept { .. })
    }
}

/// The run data the host collected from a recording's calls (spec 020 D10): input to the
/// engine's writer, which validates it (`InconsistentRun`).
#[derive(Clone, Copy, Debug)]
pub struct RunData<'a> {
    /// The voice-activation events of every `record` call and the stop, in order.
    pub events: &'a [VasEvent],
    /// The record-stream change entries of every call and the stop, in order.
    pub changes: &'a [ChangeEntry],
    /// The stop's figures.
    pub stop: StopFields,
}

/// One event row of a record: the event and the host-input position it aligns with (map (b)
/// of spec 020 D8: the first removed input sample).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct RecordEvent {
    /// The event (kept position; removed length in device samples).
    pub event: VasEvent,
    /// The host-input sample the event's position aligns with.
    pub host_input_position: u64,
}

/// What a record says (spec 020 D1; data-model.md): the reader fills one; the engine's
/// writer builds one from the slot block; the pure writer takes one.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct RecordReadout {
    /// The record's form version.
    pub form_version: u32,
    /// The library that wrote the record (under `Origin::Engine` also the one that made the
    /// recording).
    pub library_version: [u16; 3],
    /// Each stage's model version as the record names it; `[0, 0, 0]` absent or not modeled.
    pub stage_model_versions: [[u16; 3]; STAGE_COUNT],
    /// The host rate (0 under `Origin::Imported`).
    pub host_rate_hz: u32,
    /// Where the recording came from.
    pub origin: Origin,
    /// The seed (valid under `Origin::Engine`).
    pub seed: u64,
    /// The recording counter the hiss stream was keyed with (valid under `Origin::Engine`).
    pub recording_counter: u64,
    /// The stored form's version (2).
    pub stored_form_version: u16,
    /// The codec model version the stored form declares (`[0, 0, 0]` with samples).
    pub stored_codec_model_version: [u16; 3],
    /// The stored form's encoding.
    pub encoding: Encoding,
    /// The stored units (frames or samples by `encoding`).
    pub stored_unit_count: u64,
    /// The FNV-1a 64 digest of the stored form's bytes.
    pub recording_digest: u64,
    /// The configuration at the recording's start (the host rate and the seed included;
    /// unknown names skipped). `Settings::new(6000)` under `Origin::Imported`.
    pub settings: Settings,
    /// The memory's remaining capacity before the recording opened, in device samples.
    pub remaining_samples_at_start: u64,
    /// The voice-activation events the engine counted.
    pub events_total: u64,
    /// The record-stream entries the engine counted.
    pub changes_total: u64,
    /// The stop's figures.
    pub stop: StopFields,
    /// Event rows present.
    pub event_count: u64,
    /// Change rows present.
    pub change_count: u64,
    /// Setting names and control identifiers this library does not know.
    pub unknown_setting_count: u32,
    /// The [`flags`].
    pub flags: u32,
    /// Under `STAGE_MODEL_DIFFERS`: bit `n − 1` for stage `n`.
    pub stage_mask: u16,
}

impl Default for RecordReadout {
    fn default() -> Self {
        Self {
            form_version: FORM_VERSION,
            library_version: [0; 3],
            stage_model_versions: [[0; 3]; STAGE_COUNT],
            host_rate_hz: 0,
            origin: Origin::Engine,
            seed: 0,
            recording_counter: 0,
            stored_form_version: 2,
            stored_codec_model_version: [0; 3],
            encoding: Encoding::Samples,
            stored_unit_count: 0,
            recording_digest: 0,
            settings: Settings::new(crate::settings::DEVICE_RATE_HZ),
            remaining_samples_at_start: 0,
            events_total: 0,
            changes_total: 0,
            stop: StopFields::default(),
            event_count: 0,
            change_count: 0,
            unknown_setting_count: 0,
            flags: 0,
            stage_mask: 0,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The settings table (the 24 lines; contracts/record-form.md).

/// A setting's value in the record's four kinds.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Val {
    B(bool),
    I(u64),
    F(f32),
    C(u32),
}

/// The 24 settings the record carries, in the control manifest's order (the manifest's
/// settings except `host_rate_hz` and `seed`, which identity carries, and the four live
/// controls, which are not settings).
const SETTING_IDS: [&str; 24] = [
    "record_stage_enabled",
    "playback_stage_enabled",
    "adc_clamp_enabled",
    "tap",
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
];

/// The value of setting `id` in `s`, or `None` for a name that is not one of the 24.
fn get(id: &str, s: &Settings) -> Option<Val> {
    Some(match id {
        "record_stage_enabled" => Val::B(s.record_stage_enabled),
        "playback_stage_enabled" => Val::B(s.playback_stage_enabled),
        "adc_clamp_enabled" => Val::B(s.adc_clamp_enabled),
        "tap" => Val::C(tap_code(s.tap)),
        "agc.enabled" => Val::B(s.agc.enabled),
        "agc.knee_dbfs" => Val::F(s.agc.knee_dbfs),
        "agc.attack_ms" => Val::F(s.agc.attack_ms),
        "agc.release_ms" => Val::F(s.agc.release_ms),
        "vas.enabled" => Val::B(s.vas.enabled),
        "vas.mode" => Val::C(mode_code(s.vas.mode)),
        "vas.sensitivity" => Val::I(u64::from(s.vas.sensitivity)),
        "vas.threshold_dbfs" => Val::F(s.vas.threshold_dbfs),
        "vas.hang_ms" => Val::F(s.vas.hang_ms),
        "vas.onset_ms" => Val::F(s.vas.onset_ms),
        "memory.capacity_s" => Val::I(u64::from(s.memory.capacity_s)),
        "memory.max_recordings" => Val::I(u64::from(s.memory.max_recordings)),
        "speech_codec_enabled" => Val::B(s.speech_codec_enabled),
        "output.volume_amp_enabled" => Val::B(s.output.volume_amp_enabled),
        "output.speaker_enabled" => Val::B(s.output.speaker_enabled),
        "output.earphone_enabled" => Val::B(s.output.earphone_enabled),
        "output.clamp_enabled" => Val::B(s.output.clamp_enabled),
        "mic.capsule_enabled" => Val::B(s.mic.capsule_enabled),
        "mic.amp_enabled" => Val::B(s.mic.amp_enabled),
        "mic.hiss_offset_db" => Val::F(s.mic.hiss_offset_db),
        _ => return None,
    })
}

/// Sets setting `id` in `s` from `text`: `Ok(true)` set, `Ok(false)` an unknown name,
/// `Err(())` a value that does not parse for the name's kind.
fn set(id: &str, s: &mut Settings, text: &str) -> Result<bool, ()> {
    let kind = match get(id, s) {
        Some(v) => v,
        None => return Ok(false),
    };
    let v = match kind {
        Val::B(_) => Val::B(match text {
            "true" => true,
            "false" => false,
            _ => return Err(()),
        }),
        Val::I(_) => Val::I(parse::<u64>(text)?),
        Val::F(_) => Val::F(parse_f32(text)?),
        Val::C(_) => Val::C(parse::<u32>(text)?),
    };
    match (id, v) {
        ("record_stage_enabled", Val::B(b)) => s.record_stage_enabled = b,
        ("playback_stage_enabled", Val::B(b)) => s.playback_stage_enabled = b,
        ("adc_clamp_enabled", Val::B(b)) => s.adc_clamp_enabled = b,
        ("tap", Val::C(c)) => s.tap = tap_from_code(c).ok_or(())?,
        ("agc.enabled", Val::B(b)) => s.agc.enabled = b,
        ("agc.knee_dbfs", Val::F(f)) => s.agc.knee_dbfs = f,
        ("agc.attack_ms", Val::F(f)) => s.agc.attack_ms = f,
        ("agc.release_ms", Val::F(f)) => s.agc.release_ms = f,
        ("vas.enabled", Val::B(b)) => s.vas.enabled = b,
        ("vas.mode", Val::C(c)) => s.vas.mode = mode_from_code(c).ok_or(())?,
        ("vas.sensitivity", Val::I(i)) => s.vas.sensitivity = u8::try_from(i).map_err(|_| ())?,
        ("vas.threshold_dbfs", Val::F(f)) => s.vas.threshold_dbfs = f,
        ("vas.hang_ms", Val::F(f)) => s.vas.hang_ms = f,
        ("vas.onset_ms", Val::F(f)) => s.vas.onset_ms = f,
        ("memory.capacity_s", Val::I(i)) => {
            s.memory.capacity_s = u32::try_from(i).map_err(|_| ())?;
        }
        ("memory.max_recordings", Val::I(i)) => {
            s.memory.max_recordings = u32::try_from(i).map_err(|_| ())?;
        }
        ("speech_codec_enabled", Val::B(b)) => s.speech_codec_enabled = b,
        ("output.volume_amp_enabled", Val::B(b)) => s.output.volume_amp_enabled = b,
        ("output.speaker_enabled", Val::B(b)) => s.output.speaker_enabled = b,
        ("output.earphone_enabled", Val::B(b)) => s.output.earphone_enabled = b,
        ("output.clamp_enabled", Val::B(b)) => s.output.clamp_enabled = b,
        ("mic.capsule_enabled", Val::B(b)) => s.mic.capsule_enabled = b,
        ("mic.amp_enabled", Val::B(b)) => s.mic.amp_enabled = b,
        ("mic.hiss_offset_db", Val::F(f)) => s.mic.hiss_offset_db = f,
        _ => return Err(()),
    }
    Ok(true)
}

impl fmt::Display for Val {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Val::B(b) => f.write_str(if b { "true" } else { "false" }),
            Val::I(i) => write!(f, "{i}"),
            Val::F(x) => write!(f, "{x}"),
            Val::C(c) => write!(f, "{c}"),
        }
    }
}

fn parse<T: FromStr>(text: &str) -> Result<T, ()> {
    // Decimal digits only (no sign, no `+`, no leading space): the writer's form.
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(());
    }
    text.parse::<T>().map_err(|_| ())
}

fn parse_f32(text: &str) -> Result<f32, ()> {
    let v: f32 = text.parse().map_err(|_| ())?;
    if v.is_finite() { Ok(v) } else { Err(()) }
}

fn parse_f64(text: &str) -> Result<f64, ()> {
    let v: f64 = text.parse().map_err(|_| ())?;
    if v.is_finite() { Ok(v) } else { Err(()) }
}

/// `M.m.p` → `[M, m, p]`.
fn parse_version(text: &str) -> Result<[u16; 3], ()> {
    let mut parts = text.split('.');
    let mut v = [0u16; 3];
    for slot in &mut v {
        *slot = parse::<u16>(parts.next().ok_or(())?)?;
    }
    if parts.next().is_some() {
        return Err(());
    }
    Ok(v)
}

/// `fnv1a64:<16 lowercase hex>` → the value.
fn parse_hash(text: &str) -> Result<u64, ()> {
    let hex = text.strip_prefix("fnv1a64:").ok_or(())?;
    if hex.len() != 16
        || !hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(());
    }
    u64::from_str_radix(hex, 16).map_err(|_| ())
}

fn stream_word(stream: u32) -> &'static str {
    match stream {
        STREAM_PLAYBACK => "playback",
        STREAM_ONE_PASS => "one_pass",
        STREAM_RECORD => "record",
        _ => "none",
    }
}

fn stream_of(word: &str) -> Result<u32, ()> {
    Ok(match word {
        "none" => 0,
        "playback" => STREAM_PLAYBACK,
        "one_pass" => STREAM_ONE_PASS,
        "record" => STREAM_RECORD,
        _ => return Err(()),
    })
}

/// The manifest index of a control identifier.
fn control_index(identifier: &str) -> Option<u32> {
    controls()
        .iter()
        .position(|e| e.identifier == identifier)
        .map(|i| i as u32)
}

// ---------------------------------------------------------------------------------------------
// The writer.

/// A `fmt::Write` over a caller buffer that counts (and hashes) every byte; with no buffer it
/// only counts (research R-02, R-09: the length is exact).
struct Sink<'a> {
    out: Option<&'a mut [u8]>,
    pos: usize,
    hash: Fnv1a64,
}

impl fmt::Write for Sink<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let b = s.as_bytes();
        self.hash.update(b);
        if let Some(out) = &mut self.out {
            match out.get_mut(self.pos..self.pos + b.len()) {
                Some(slot) => slot.copy_from_slice(b),
                None => return Err(fmt::Error),
            }
        }
        self.pos += b.len();
        Ok(())
    }
}

fn version_str(v: [u16; 3], f: &mut Sink<'_>) -> fmt::Result {
    write!(f, "{}.{}.{}", v[0], v[1], v[2])
}

/// Writes the record of `r` with the rows of `events` and `changes` into the sink
/// (contracts/record-form.md's order).
fn emit<I: Iterator<Item = RecordEvent>>(
    r: &RecordReadout,
    events: I,
    changes: &[ChangeEntry],
    s: &mut Sink<'_>,
) -> fmt::Result {
    writeln!(s, "{VERSION_LINE_NAME} = {FORM_VERSION}")?;
    s.write_str("library_version = ")?;
    version_str(r.library_version, s)?;
    s.write_str("\n")?;
    let engine = r.origin == Origin::Engine;
    writeln!(s, "origin = {}", if engine { "engine" } else { "imported" })?;
    if engine {
        writeln!(s, "host_rate_hz = {}", r.host_rate_hz)?;
        for (i, v) in r.stage_model_versions.iter().enumerate() {
            if *v != [0; 3] {
                write!(s, "stage.{}.model_version = ", i + 1)?;
                version_str(*v, s)?;
                s.write_str("\n")?;
            }
        }
        writeln!(s, "seed = {}", r.seed)?;
        writeln!(s, "recording_counter = {}", r.recording_counter)?;
    }
    writeln!(s, "stored_form_version = {}", r.stored_form_version)?;
    s.write_str("stored_codec_model_version = ")?;
    version_str(r.stored_codec_model_version, s)?;
    s.write_str("\n")?;
    writeln!(
        s,
        "encoding = {}",
        match r.encoding {
            Encoding::Frames => "frames",
            Encoding::Samples => "samples",
        }
    )?;
    writeln!(s, "stored_unit_count = {}", r.stored_unit_count)?;
    writeln!(s, "recording_digest = fnv1a64:{:016x}", r.recording_digest)?;
    if engine {
        for id in SETTING_IDS {
            let v = get(id, &r.settings).expect("a table identifier");
            writeln!(s, "setting.{id} = {v}")?;
        }
        writeln!(
            s,
            "run.remaining_samples_at_start = {}",
            r.remaining_samples_at_start
        )?;
        writeln!(s, "run.events_total = {}", r.events_total)?;
        writeln!(s, "run.changes_total = {}", r.changes_total)?;
        writeln!(
            s,
            "run.stop = {}",
            match r.stop.how {
                StopHow::StopCall => "stop_call",
                StopHow::MemoryFull => "memory_full",
            }
        )?;
        writeln!(
            s,
            "run.stop_input_position = {}",
            r.stop.stop_input_position
        )?;
        writeln!(
            s,
            "run.padding_start_sample = {}",
            r.stop.padding_start_sample
        )?;
        writeln!(
            s,
            "run.input_samples_total = {}",
            r.stop.input_samples_total
        )?;
        writeln!(
            s,
            "run.silence_fed_samples = {}",
            r.stop.silence_fed_samples
        )?;
        writeln!(
            s,
            "event.fields = kept_position input_length host_input_position"
        )?;
        for (i, e) in events.enumerate() {
            writeln!(
                s,
                "event.{i} = {} {} {}",
                e.event.output_position, e.event.input_length, e.host_input_position
            )?;
        }
        writeln!(
            s,
            "change.fields = position control before after tag stream"
        )?;
        for (i, c) in changes.iter().enumerate() {
            let id = controls()
                .get(c.control as usize)
                .map_or("unknown", |e| e.identifier);
            writeln!(
                s,
                "change.{i} = {} {id} {} {} {} {}",
                c.position,
                c.before,
                c.after,
                c.tag,
                stream_word(c.stream)
            )?;
        }
    }
    let h = s.hash.finish();
    writeln!(s, "record_checksum = fnv1a64:{h:016x}")
}

/// The exact length of the record `write` produces for the same inputs (spec 020 FR-001;
/// research R-09): one counting pass, no allocation.
#[must_use]
pub fn len(r: &RecordReadout, events: &[RecordEvent], changes: &[ChangeEntry]) -> usize {
    len_iter(r, events.iter().copied(), changes)
}

pub(crate) fn len_iter<I: Iterator<Item = RecordEvent>>(
    r: &RecordReadout,
    events: I,
    changes: &[ChangeEntry],
) -> usize {
    let mut s = Sink {
        out: None,
        pos: 0,
        hash: Fnv1a64::new(),
    };
    emit(r, events, changes, &mut s).expect("a counting sink never fails");
    s.pos
}

/// Writes the record of `r` with its rows into `out` and returns the bytes written
/// (spec 020 D2, FR-001; contracts/record-form.md). The same inputs give the same bytes on
/// every platform. Allocates nothing.
///
/// # Errors
///
/// [`Error::BufferTooSmall`] with the needed length; `out` is then partly written.
pub fn write(
    r: &RecordReadout,
    events: &[RecordEvent],
    changes: &[ChangeEntry],
    out: &mut [u8],
) -> Result<usize, Error> {
    write_iter(r, events.iter().copied(), changes, out)
}

pub(crate) fn write_iter<I: Iterator<Item = RecordEvent> + Clone>(
    r: &RecordReadout,
    events: I,
    changes: &[ChangeEntry],
    out: &mut [u8],
) -> Result<usize, Error> {
    let needed = len_iter(r, events.clone(), changes);
    if out.len() < needed {
        return Err(Error::BufferTooSmall { needed });
    }
    let mut s = Sink {
        out: Some(out),
        pos: 0,
        hash: Fnv1a64::new(),
    };
    emit(r, events, changes, &mut s).map_err(|_| Error::BufferTooSmall { needed })?;
    Ok(s.pos)
}

// ---------------------------------------------------------------------------------------------
// The reader.

/// One `name = value` line; `None` when the line has not that shape or the name a character
/// outside `[a-z0-9_.]`.
fn split_line(line: &[u8]) -> Option<(&str, &str)> {
    let text = core::str::from_utf8(line).ok()?;
    let at = text.find(" = ")?;
    let (name, value) = (&text[..at], &text[at + 3..]);
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'.')
        || value.contains('\r')
    {
        return None;
    }
    Some((name, value))
}

/// The record's lines without their line ends (the record ends with one LF, checked by the
/// reader before iterating).
fn lines(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    let body = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    body.split(|&b| b == b'\n')
}

/// The required fields' bits (data-model.md; contracts/record-form.md's Required column).
mod req {
    pub const LIBRARY: u32 = 1; // FR-002 (020 D2): a required line
    pub const ORIGIN: u32 = 2; // FR-002 (020 D2): a required line
    pub const HOST_RATE: u32 = 4; // FR-002 (020 D2): a required line
    pub const SEED: u32 = 8; // FR-002 (020 D2): a required line
    pub const COUNTER: u32 = 16; // FR-002 (020 D2): a required line
    pub const STORED_FORM: u32 = 32; // FR-002 (020 D2): a required line
    pub const STORED_CODEC: u32 = 1 << 6; // engineering target (020 D2): one bit per required line
    pub const ENCODING: u32 = 1 << 7; // engineering target (020 D2)
    pub const UNITS: u32 = 1 << 8; // engineering target (020 D2)
    pub const DIGEST: u32 = 1 << 9; // engineering target (020 D2)
    pub const REMAINING: u32 = 1 << 10; // FR-002 (020 D2): a required line
    pub const EVENTS_TOTAL: u32 = 1 << 11; // FR-002 (020 D2): a required line
    pub const CHANGES_TOTAL: u32 = 1 << 12; // FR-002 (020 D2): a required line
    pub const STOP: u32 = 1 << 13; // FR-002 (020 D2): a required line
    pub const STOP_POS: u32 = 1 << 14; // FR-002 (020 D2): a required line
    pub const PADDING: u32 = 1 << 15; // FR-002 (020 D2): a required line
    pub const INPUT_TOTAL: u32 = 1 << 16; // FR-002 (020 D2): a required line
    pub const SILENCE: u32 = 1 << 17; // FR-002 (020 D2): a required line
    pub const EVENT_FIELDS: u32 = 1 << 18; // FR-002 (020 D2): a required line
    pub const CHANGE_FIELDS: u32 = 1 << 19; // FR-002 (020 D2): a required line
    pub const ALWAYS: u32 =
        LIBRARY | ORIGIN | STORED_FORM | STORED_CODEC | ENCODING | UNITS | DIGEST;
    pub const ENGINE: u32 = HOST_RATE
        | SEED
        | COUNTER
        | REMAINING
        | EVENTS_TOTAL
        | CHANGES_TOTAL
        | STOP
        | STOP_POS
        | PADDING
        | INPUT_TOTAL
        | SILENCE
        | EVENT_FIELDS
        | CHANGE_FIELDS;

    pub const fn name(bit: u32) -> &'static str {
        match bit {
            LIBRARY => "library_version",
            ORIGIN => "origin",
            HOST_RATE => "host_rate_hz",
            SEED => "seed",
            COUNTER => "recording_counter",
            STORED_FORM => "stored_form_version",
            STORED_CODEC => "stored_codec_model_version",
            ENCODING => "encoding",
            UNITS => "stored_unit_count",
            DIGEST => "recording_digest",
            REMAINING => "run.remaining_samples_at_start",
            EVENTS_TOTAL => "run.events_total",
            CHANGES_TOTAL => "run.changes_total",
            STOP => "run.stop",
            STOP_POS => "run.stop_input_position",
            PADDING => "run.padding_start_sample",
            INPUT_TOTAL => "run.input_samples_total",
            SILENCE => "run.silence_fed_samples",
            EVENT_FIELDS => "event.fields",
            CHANGE_FIELDS => "change.fields",
            _ => "setting",
        }
    }
}

/// Sets `bit` in `seen` once: a repeated line is malformed (hardware review B5).
fn once(seen: u32, bit: u32) -> Option<u32> {
    (seen & bit == 0).then_some(seen | bit)
}

/// The first line's version, or the refusal (research R-02: version first).
fn read_version(bytes: &[u8]) -> Result<u32, Error> {
    let first = lines(bytes).next().unwrap_or(&[]);
    let bad = Error::MalformedRecord { line: 1 };
    let (name, value) = split_line(first).ok_or(bad)?;
    if name != VERSION_LINE_NAME {
        return Err(bad);
    }
    let version = parse::<u32>(value).map_err(|()| bad)?;
    if version == 0 || version > FORM_VERSION {
        return Err(Error::UnknownRecordVersion { version });
    }
    Ok(version)
}

/// The checksum check (research R-02): the last line is `record_checksum = fnv1a64:…` over
/// every byte above it; the record ends with exactly one LF.
fn check_checksum(bytes: &[u8]) -> Result<(), Error> {
    let Some(body) = bytes.strip_suffix(b"\n") else {
        return Err(Error::MalformedRecord { line: 0 });
    };
    let start = body.iter().rposition(|&b| b == b'\n').map_or(0, |p| p + 1);
    let last = &body[start..];
    let n = lines(bytes).count() as u32;
    let (name, value) = split_line(last).ok_or(Error::MalformedRecord { line: n })?;
    if name != "record_checksum" {
        return Err(Error::MissingRecordField {
            field: "record_checksum",
        });
    }
    let want = parse_hash(value).map_err(|()| Error::MalformedRecord { line: n })?;
    if crate::digest::fnv1a64(&bytes[..start]) != want {
        return Err(Error::RecordChecksumMismatch);
    }
    Ok(())
}

/// Reads a record into `out` (spec 020 D4, FR-002; research R-02): refuses an unknown form
/// version, a checksum mismatch, a malformed line or an undefined name, and a missing
/// required field, each with its status; reports every difference as a flag
/// ([`flags`]; the host-rate flag is the engine reader's, [`Pipeline::processing_record_read`]).
/// Allocates nothing; the rows are counted and validated here and read by the cursors.
///
/// # Errors
///
/// [`Error::UnknownRecordVersion`], [`Error::MalformedRecord`],
/// [`Error::RecordChecksumMismatch`], [`Error::MissingRecordField`].
///
/// [`Pipeline::processing_record_read`]: crate::Pipeline::processing_record_read
pub fn read(bytes: &[u8], out: &mut RecordReadout) -> Result<(), Error> {
    let version = read_version(bytes)?;
    check_checksum(bytes)?;
    let mut r = RecordReadout {
        form_version: version,
        ..RecordReadout::default()
    };
    let mut seen = 0u32;
    let mut origin_line: Option<Origin> = None;
    let mut host_rate: Option<u32> = None;
    let mut seed: Option<u64> = None;
    let mut unknown = 0u32;
    let total = lines(bytes).count();
    for (idx, line) in lines(bytes).enumerate() {
        let n = (idx + 1) as u32;
        let bad = Error::MalformedRecord { line: n };
        if idx == 0 || idx + 1 == total {
            continue; // the version line and the checksum line, checked above
        }
        let (name, value) = split_line(line).ok_or(bad)?;
        let e = |()| bad;
        match name {
            "library_version" => {
                r.library_version = parse_version(value).map_err(e)?;
                seen = once(seen, req::LIBRARY).ok_or(bad)?;
            }
            "origin" => {
                let o = match value {
                    "engine" => Origin::Engine,
                    "imported" => Origin::Imported,
                    _ => return Err(bad),
                };
                origin_line = Some(o);
                r.origin = o;
                seen |= req::ORIGIN;
            }
            "host_rate_hz" => {
                host_rate = Some(parse::<u32>(value).map_err(e)?);
                seen |= req::HOST_RATE;
            }
            "seed" => {
                seed = Some(parse::<u64>(value).map_err(e)?);
                seen |= req::SEED;
            }
            "recording_counter" => {
                r.recording_counter = parse::<u64>(value).map_err(e)?;
                seen |= req::COUNTER;
            }
            "stored_form_version" => {
                r.stored_form_version = parse::<u16>(value).map_err(e)?;
                seen |= req::STORED_FORM;
            }
            "stored_codec_model_version" => {
                r.stored_codec_model_version = parse_version(value).map_err(e)?;
                seen |= req::STORED_CODEC;
            }
            "encoding" => {
                r.encoding = match value {
                    "frames" => Encoding::Frames,
                    "samples" => Encoding::Samples,
                    _ => return Err(bad),
                };
                seen |= req::ENCODING;
            }
            "stored_unit_count" => {
                r.stored_unit_count = parse::<u64>(value).map_err(e)?;
                seen |= req::UNITS;
            }
            "recording_digest" => {
                r.recording_digest = parse_hash(value).map_err(e)?;
                seen |= req::DIGEST;
            }
            "run.remaining_samples_at_start" => {
                r.remaining_samples_at_start = parse::<u64>(value).map_err(e)?;
                seen |= req::REMAINING;
            }
            "run.events_total" => {
                r.events_total = parse::<u64>(value).map_err(e)?;
                seen |= req::EVENTS_TOTAL;
            }
            "run.changes_total" => {
                r.changes_total = parse::<u64>(value).map_err(e)?;
                seen |= req::CHANGES_TOTAL;
            }
            "run.stop" => {
                r.stop.how = match value {
                    "stop_call" => StopHow::StopCall,
                    "memory_full" => StopHow::MemoryFull,
                    _ => return Err(bad),
                };
                seen |= req::STOP;
            }
            "run.stop_input_position" => {
                r.stop.stop_input_position = parse::<u64>(value).map_err(e)?;
                seen |= req::STOP_POS;
            }
            "run.padding_start_sample" => {
                r.stop.padding_start_sample = parse::<u64>(value).map_err(e)?;
                seen |= req::PADDING;
            }
            "run.input_samples_total" => {
                r.stop.input_samples_total = parse::<u64>(value).map_err(e)?;
                seen |= req::INPUT_TOTAL;
            }
            "run.silence_fed_samples" => {
                r.stop.silence_fed_samples = parse::<u64>(value).map_err(e)?;
                seen |= req::SILENCE;
            }
            "event.fields" => {
                if value != "kept_position input_length host_input_position" {
                    return Err(bad);
                }
                seen |= req::EVENT_FIELDS;
            }
            "change.fields" => {
                if value != "position control before after tag stream" {
                    return Err(bad);
                }
                seen |= req::CHANGE_FIELDS;
            }
            _ => {
                if let Some(rest) = name.strip_prefix("stage.") {
                    let (num, field) = rest.split_once('.').ok_or(bad)?;
                    if field != "model_version" {
                        return Err(bad);
                    }
                    let stage = parse::<usize>(num).map_err(e)?;
                    if stage == 0 || stage > STAGE_COUNT {
                        return Err(bad);
                    }
                    r.stage_model_versions[stage - 1] = parse_version(value).map_err(e)?;
                } else if let Some(id) = name.strip_prefix("setting.") {
                    match set(id, &mut r.settings, value) {
                        Ok(true) => {}
                        Ok(false) => unknown += 1,
                        Err(()) => return Err(bad),
                    }
                } else if let Some(i) = name.strip_prefix("event.") {
                    if parse::<u64>(i).map_err(e)? != r.event_count {
                        return Err(bad);
                    }
                    parse_event_row(value).ok_or(bad)?;
                    r.event_count += 1;
                } else if let Some(i) = name.strip_prefix("change.") {
                    if parse::<u64>(i).map_err(e)? != r.change_count {
                        return Err(bad);
                    }
                    let (known, _) = parse_change_row(value).ok_or(bad)?;
                    if !known {
                        unknown += 1;
                    }
                    r.change_count += 1;
                } else {
                    return Err(bad); // a name version 1 does not define
                }
            }
        }
    }
    // The required fields per origin (contracts/record-form.md).
    let origin = origin_line.ok_or(Error::MissingRecordField { field: "origin" })?;
    let required = match origin {
        Origin::Engine => req::ALWAYS | req::ENGINE,
        Origin::Imported => req::ALWAYS,
    };
    let missing = required & !seen;
    if missing != 0 {
        let bit = 1u32 << missing.trailing_zeros();
        return Err(Error::MissingRecordField {
            field: req::name(bit),
        });
    }
    // The settings carry the host rate and the seed from identity.
    if let Some(rate) = host_rate {
        r.host_rate_hz = rate;
        r.settings.host_rate_hz = rate;
    }
    if let Some(s) = seed {
        r.seed = s;
        r.settings.seed = s;
    }
    // The flags.
    if r.library_version != crate::form::LIB_VERSION {
        r.flags |= flags::LIBRARY_DIFFERS;
    }
    if origin == Origin::Engine {
        for (i, entry) in crate::manifest::manifest().stages.iter().enumerate() {
            let ours = parse_version(entry.model_version).unwrap_or([0; 3]);
            if ours != r.stage_model_versions[i] {
                r.stage_mask |= 1 << i;
            }
        }
        if r.stage_mask != 0 {
            r.flags |= flags::STAGE_MODEL_DIFFERS;
        }
        if r.event_count < r.events_total || r.change_count < r.changes_total {
            r.flags |= flags::INCOMPLETE_RUN;
        }
    } else {
        r.flags |= flags::IMPORTED_ORIGIN;
    }
    if unknown > 0 {
        r.flags |= flags::UNKNOWN_SETTINGS;
    }
    r.unknown_setting_count = unknown;
    *out = r;
    Ok(())
}

fn parse_event_row(value: &str) -> Option<RecordEvent> {
    let mut f = value.split(' ');
    let a = parse::<u64>(f.next()?).ok()?;
    let b = parse::<u64>(f.next()?).ok()?;
    let c = parse::<u64>(f.next()?).ok()?;
    if f.next().is_some() {
        return None;
    }
    Some(RecordEvent {
        event: VasEvent::new(a, b),
        host_input_position: c,
    })
}

/// `(known control, entry)`; an unknown identifier yields `control = u32::MAX`.
fn parse_change_row(value: &str) -> Option<(bool, ChangeEntry)> {
    let mut f = value.split(' ');
    let position = parse::<u64>(f.next()?).ok()?;
    let id = f.next()?;
    let before = parse_f64(f.next()?).ok()?;
    let after = parse_f64(f.next()?).ok()?;
    let tag = parse::<u64>(f.next()?).ok()?;
    let stream = stream_of(f.next()?).ok()?;
    if f.next().is_some() {
        return None;
    }
    let control = control_index(id);
    Some((
        control.is_some(),
        ChangeEntry::new(
            position,
            control.unwrap_or(u32::MAX),
            before,
            after,
            tag,
            stream,
        ),
    ))
}

/// The value of the line named `name` in a record the reader accepted, if present.
fn find_line<'a>(bytes: &'a [u8], name: &str) -> Option<&'a str> {
    lines(bytes).find_map(|l| {
        let (n, v) = split_line(l)?;
        (n == name).then_some(v)
    })
}

/// A small fixed buffer for a row's name (`event.<i>` / `change.<i>` with a 20-digit index).
fn row_name(prefix: &str, i: u64) -> ([u8; 32], usize) {
    struct Buf([u8; 32], usize);
    impl fmt::Write for Buf {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            let b = s.as_bytes();
            self.0[self.1..self.1 + b.len()].copy_from_slice(b);
            self.1 += b.len();
            Ok(())
        }
    }
    let mut b = Buf([0; 32], 0);
    write!(b, "{prefix}{i}").expect("fits");
    (b.0, b.1)
}

fn find_row<'a>(bytes: &'a [u8], prefix: &str, i: u64) -> Option<&'a str> {
    let (buf, n) = row_name(prefix, i);
    let name = core::str::from_utf8(&buf[..n]).ok()?;
    find_line(bytes, name)
}

/// Event row `i` of a record the reader accepted (spec 020 FR-002); re-scans the lines, so
/// extract once with [`record_events`] rather than calling it per sample.
#[must_use]
pub fn record_event(bytes: &[u8], i: u64) -> Option<RecordEvent> {
    parse_event_row(find_row(bytes, "event.", i)?)
}

/// Change row `i` of a record the reader accepted, with the control identifier resolved to
/// this library's manifest index; `None` past the rows or for an identifier this library does
/// not know (counted by the reader's flag).
#[must_use]
pub fn record_change(bytes: &[u8], i: u64) -> Option<ChangeEntry> {
    let (known, e) = parse_change_row(find_row(bytes, "change.", i)?)?;
    known.then_some(e)
}

/// The `i`-th setting name or control identifier this library does not know, in the record's
/// order (spec 020 US2 AS2: the unknown names listed).
#[must_use]
pub fn record_unknown_setting(bytes: &[u8], i: u32) -> Option<&str> {
    let probe = Settings::new(crate::settings::DEVICE_RATE_HZ);
    let mut seen = 0u32;
    for l in lines(bytes) {
        let Some((name, value)) = split_line(l) else {
            continue;
        };
        let unknown_name = if let Some(id) = name.strip_prefix("setting.") {
            get(id, &probe).is_none().then_some(id)
        } else if name.starts_with("change.") && name != "change.fields" {
            let id = value.split(' ').nth(1)?;
            (control_index(id).is_none()).then_some(id)
        } else {
            None
        };
        if let Some(id) = unknown_name {
            if seen == i {
                return Some(id);
            }
            seen += 1;
        }
    }
    None
}

/// Fills `out` with the event rows from `first` on, in one pass; returns how many were
/// written (the rows present from `first`, at most `out.len()`).
pub fn record_events(bytes: &[u8], first: u64, out: &mut [VasEvent]) -> usize {
    let mut n = 0usize;
    let mut i = 0u64;
    for l in lines(bytes) {
        if n == out.len() {
            break;
        }
        let Some((name, value)) = split_line(l) else {
            continue;
        };
        if name.starts_with("event.") && name != "event.fields" {
            if i >= first {
                if let Some(e) = parse_event_row(value) {
                    out[n] = e.event;
                    n += 1;
                }
            }
            i += 1;
        }
    }
    n
}

/// As [`record_events`] for the change rows (an unknown identifier's row is skipped).
pub fn record_changes(bytes: &[u8], first: u64, out: &mut [ChangeEntry]) -> usize {
    let mut n = 0usize;
    let mut i = 0u64;
    for l in lines(bytes) {
        if n == out.len() {
            break;
        }
        let Some((name, value)) = split_line(l) else {
            continue;
        };
        if name.starts_with("change.") && name != "change.fields" {
            if i >= first {
                if let Some((true, e)) = parse_change_row(value) {
                    out[n] = e;
                    n += 1;
                }
            }
            i += 1;
        }
    }
    n
}

/// The host-input centre a pre-gate device index aligns with (spec 020 D8 (a), research
/// R-01): ⌊k · m ÷ l⌋ − L_pre, clamped at 0 (`clamped` then true).
pub(crate) const fn host_centre(
    pre_gate_device: u64,
    l: u64,
    m: u64,
    latency_pre: u64,
) -> (u64, bool) {
    let host = pre_gate_device.saturating_mul(m) / l;
    if host < latency_pre {
        (0, true)
    } else {
        (host - latency_pre, false)
    }
}

/// The event rows of `events` for a recorder at `(l, m)` with pre-encoder latency
/// `latency_pre`: each event's host position is the centre of the first removed input sample
/// (the kept position plus the lengths of the events before it).
#[derive(Clone)]
pub(crate) struct EventRows<'a> {
    events: core::slice::Iter<'a, VasEvent>,
    removed: u64,
    l: u64,
    m: u64,
    latency_pre: u64,
    /// The gate drops (else a region is stored and nothing is removed; hardware review B1).
    drop: bool,
}

impl<'a> EventRows<'a> {
    pub(crate) fn new(
        events: &'a [VasEvent],
        l: u64,
        m: u64,
        latency_pre: u64,
        drop: bool,
    ) -> Self {
        Self {
            events: events.iter(),
            removed: 0,
            l,
            m,
            latency_pre,
            drop,
        }
    }
}

impl Iterator for EventRows<'_> {
    type Item = RecordEvent;
    fn next(&mut self) -> Option<RecordEvent> {
        let e = *self.events.next()?;
        let (host_input_position, _) = host_centre(
            e.output_position + self.removed,
            self.l,
            self.m,
            self.latency_pre,
        );
        if self.drop {
            self.removed += e.input_length;
        }
        Some(RecordEvent {
            event: e,
            host_input_position,
        })
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::controls::ControlKind;
    use alloc::vec::Vec;
    use std::string::String;

    /// The table is the manifest's settings except the two identity lines and the five live
    /// controls (spec 023's mode included), in manifest order.
    #[test]
    fn setting_table_matches_the_manifest() {
        let want: Vec<&str> = controls()
            .iter()
            .filter(|e| {
                !matches!(e.identifier, "host_rate_hz" | "seed")
                    && !matches!(
                        e.identifier,
                        "route"
                            | "volume"
                            | "playback_speed"
                            | "battery_volts"
                            | "playback_speed_mode"
                    )
            })
            .map(|e| e.identifier)
            .collect();
        assert_eq!(want, SETTING_IDS.to_vec());
        let s = Settings::new(48_000);
        for id in SETTING_IDS {
            assert!(get(id, &s).is_some(), "{id}");
        }
        assert!(get("route", &s).is_none());
        assert_eq!(
            controls()
                .iter()
                .filter(|e| e.kind == ControlKind::HostParameter)
                .count(),
            2
        );
    }

    fn readout() -> RecordReadout {
        let mut r = RecordReadout {
            library_version: crate::form::LIB_VERSION,
            host_rate_hz: 48_000,
            seed: 9,
            recording_counter: 3,
            stored_form_version: 2,
            stored_codec_model_version: [0, 8, 0],
            encoding: Encoding::Frames,
            stored_unit_count: 25,
            recording_digest: 0x0123_4567_89ab_cdef,
            settings: Settings::new(48_000),
            remaining_samples_at_start: 21_600_000,
            events_total: 2,
            changes_total: 1,
            stop: StopFields {
                how: StopHow::StopCall,
                stop_input_position: 24_000,
                padding_start_sample: 3001,
                input_samples_total: 24_242 + 8 * 179,
                silence_fed_samples: 179,
            },
            ..RecordReadout::default()
        };
        r.settings.seed = 9;
        r.settings.agc.knee_dbfs = -1.5;
        for (i, e) in crate::manifest::manifest().stages.iter().enumerate() {
            r.stage_model_versions[i] = parse_version(e.model_version).unwrap_or([0; 3]);
        }
        r
    }

    fn rows() -> ([RecordEvent; 2], [ChangeEntry; 1]) {
        (
            [
                RecordEvent {
                    event: VasEvent::new(600, 300),
                    host_input_position: 4800 - 242,
                },
                RecordEvent {
                    event: VasEvent::new(1800, 120),
                    host_input_position: (1800 + 300) * 8 - 242,
                },
            ],
            [ChangeEntry::new(12_000, 10, -1.5, -4.0, 11, STREAM_RECORD)],
        )
    }

    fn written(r: &RecordReadout, ev: &[RecordEvent], ch: &[ChangeEntry]) -> Vec<u8> {
        let n = len(r, ev, ch);
        let mut out = std::vec![0u8; n];
        assert_eq!(write(r, ev, ch, &mut out).unwrap(), n);
        out
    }

    #[test]
    fn writes_the_contract_form() {
        let r = readout();
        let (ev, ch) = rows();
        let bytes = written(&r, &ev, &ch);
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(text.starts_with("rr_dr60_processing_record_version = 1\nlibrary_version = "));
        assert!(text.contains("\norigin = engine\nhost_rate_hz = 48000\nstage.1.model_version = "));
        assert!(text.contains("\nseed = 9\nrecording_counter = 3\nstored_form_version = 2\nstored_codec_model_version = 0.8.0\nencoding = frames\nstored_unit_count = 25\nrecording_digest = fnv1a64:0123456789abcdef\nsetting.record_stage_enabled = true\n"));
        assert!(text.contains("\nsetting.agc.knee_dbfs = -1.5\n"));
        assert!(text.contains("\nsetting.tap = 5\n"), "{text}");
        assert!(text.contains("\nrun.stop = stop_call\nrun.stop_input_position = 24000\n"));
        assert!(text.contains("\nevent.fields = kept_position input_length host_input_position\nevent.0 = 600 300 4558\nevent.1 = 1800 120 16558\nchange.fields = position control before after tag stream\nchange.0 = 12000 agc.knee_dbfs -1.5 -4 11 record\nrecord_checksum = fnv1a64:"));
        assert!(text.ends_with('\n') && !text.ends_with("\n\n"));
        assert_eq!(text.matches("\nsetting.").count(), 24);
        // A short buffer is refused with the needed length; the counting pass is exact.
        let mut short = std::vec![0u8; bytes.len() - 1];
        assert_eq!(
            write(&r, &ev, &ch, &mut short),
            Err(Error::BufferTooSmall {
                needed: bytes.len()
            })
        );
        // Deterministic.
        assert_eq!(written(&r, &ev, &ch), bytes);
    }

    #[test]
    fn round_trip_write_read_rewrite() {
        let r = readout();
        let (ev, ch) = rows();
        let bytes = written(&r, &ev, &ch);
        let mut got = RecordReadout::default();
        read(&bytes, &mut got).unwrap();
        assert_eq!(got.flags, 0, "no difference: same library, same stages");
        assert_eq!(got.seed, 9);
        assert_eq!(got.settings, r.settings);
        assert_eq!(got.stop, r.stop);
        assert_eq!((got.event_count, got.change_count), (2, 1));
        assert_eq!(got.recording_digest, r.recording_digest);
        assert_eq!(record_event(&bytes, 0), Some(ev[0]));
        assert_eq!(record_event(&bytes, 1), Some(ev[1]));
        assert_eq!(record_event(&bytes, 2), None);
        assert_eq!(record_change(&bytes, 0), Some(ch[0]));
        let mut e2 = [VasEvent::default(); 4];
        assert_eq!(record_events(&bytes, 0, &mut e2), 2);
        assert_eq!(e2[1], ev[1].event);
        assert_eq!(record_events(&bytes, 1, &mut e2), 1);
        let mut c2 = [ChangeEntry::default(); 2];
        assert_eq!(record_changes(&bytes, 0, &mut c2), 1);
        assert_eq!(c2[0], ch[0]);
        let again = written(
            &got,
            &[
                record_event(&bytes, 0).unwrap(),
                record_event(&bytes, 1).unwrap(),
            ],
            &c2[..1],
        );
        assert_eq!(again, bytes, "the round trip");
    }

    fn doctor(bytes: &[u8], from: &str, to: &str) -> Vec<u8> {
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(text.contains(from), "{from}");
        text.replacen(from, to, 1).into_bytes()
    }

    /// A record re-signed after an edit (the checksum line recomputed).
    fn resign(bytes: Vec<u8>) -> Vec<u8> {
        let body_end = bytes[..bytes.len() - 1]
            .iter()
            .rposition(|&b| b == b'\n')
            .unwrap()
            + 1;
        let h = crate::digest::fnv1a64(&bytes[..body_end]);
        let mut v = bytes[..body_end].to_vec();
        v.extend_from_slice(std::format!("record_checksum = fnv1a64:{h:016x}\n").as_bytes());
        v
    }

    fn read_err(bytes: &[u8]) -> Error {
        read(bytes, &mut RecordReadout::default()).unwrap_err()
    }

    #[test]
    fn refusals() {
        let r = readout();
        let (ev, ch) = rows();
        let bytes = written(&r, &ev, &ch);
        assert_eq!(
            read_err(&doctor(&bytes, "version = 1", "version = 0")),
            Error::UnknownRecordVersion { version: 0 }
        );
        assert_eq!(
            read_err(&doctor(&bytes, "version = 1", "version = 2")),
            Error::UnknownRecordVersion { version: 2 }
        );
        assert_eq!(
            read_err(&doctor(
                &bytes,
                "rr_dr60_processing_record_version",
                "rr_dr60_record_version"
            )),
            Error::MalformedRecord { line: 1 }
        );
        // A flipped byte anywhere above the last line.
        let mut flipped = bytes.clone();
        flipped[60] ^= 1;
        assert_eq!(read_err(&flipped), Error::RecordChecksumMismatch);
        // Truncated: the checksum line is gone, or cut.
        assert_eq!(
            read_err(&bytes[..bytes.len() - 1]),
            Error::MalformedRecord { line: 0 }
        );
        let cut = bytes[..bytes.len() - 10].to_vec();
        let mut cut_lf = cut.clone();
        cut_lf.push(b'\n');
        assert!(matches!(
            read_err(&cut_lf),
            Error::MalformedRecord { .. } | Error::MissingRecordField { .. }
        ));
        // A comment line, an undefined name, a bad value, a NaN: malformed at their line.
        assert!(matches!(
            read_err(&resign(doctor(
                &bytes,
                "seed = 9\n",
                "seed = 9\n# a note\n"
            ))),
            Error::MalformedRecord { .. }
        ));
        assert!(matches!(
            read_err(&resign(doctor(
                &bytes,
                "seed = 9\n",
                "seed = 9\nmood = calm\n"
            ))),
            Error::MalformedRecord { .. }
        ));
        assert!(matches!(
            read_err(&resign(doctor(&bytes, "seed = 9", "seed = nine"))),
            Error::MalformedRecord { .. }
        ));
        assert!(matches!(
            read_err(&resign(doctor(
                &bytes,
                "knee_dbfs = -1.5",
                "knee_dbfs = NaN"
            ))),
            Error::MalformedRecord { .. }
        ));
        assert!(matches!(
            read_err(&resign(doctor(
                &bytes,
                "knee_dbfs = -1.5",
                "knee_dbfs = inf"
            ))),
            Error::MalformedRecord { .. }
        ));
        // A missing required field per origin.
        assert_eq!(
            read_err(&resign(doctor(&bytes, "seed = 9\n", ""))),
            Error::MissingRecordField { field: "seed" }
        );
        assert_eq!(
            read_err(&resign(doctor(&bytes, "run.stop = stop_call\n", ""))),
            Error::MissingRecordField { field: "run.stop" }
        );
        // An imported record needs only the always lines.
        let mut imp = readout();
        imp.origin = Origin::Imported;
        let ib = written(&imp, &[], &[]);
        let mut got = RecordReadout::default();
        read(&ib, &mut got).unwrap();
        assert_eq!(got.flags, flags::IMPORTED_ORIGIN);
        assert_eq!(got.host_rate_hz, 0);
        assert!(!String::from_utf8(ib.clone()).unwrap().contains("run."));
        assert_eq!(
            read_err(&resign(doctor(&ib, "stored_unit_count = 25\n", ""))),
            Error::MissingRecordField {
                field: "stored_unit_count"
            }
        );
    }

    #[test]
    fn flags_and_unknown_names() {
        let r = readout();
        let (ev, ch) = rows();
        let bytes = written(&r, &ev, &ch);
        let mut got = RecordReadout::default();
        read(
            &resign(doctor(
                &bytes,
                "library_version = 0.",
                "library_version = 9.",
            )),
            &mut got,
        )
        .unwrap();
        assert_eq!(got.flags, flags::LIBRARY_DIFFERS);
        read(
            &resign(doctor(
                &bytes,
                "stage.5.model_version = 0.",
                "stage.5.model_version = 9.",
            )),
            &mut got,
        )
        .unwrap();
        assert_eq!(got.flags, flags::STAGE_MODEL_DIFFERS);
        assert_eq!(got.stage_mask, 1 << 4);
        // An absent stage line for a modeled stage differs too.
        read(
            &resign(doctor(&bytes, "stage.3.model_version = 0.5.0\n", "")),
            &mut got,
        )
        .unwrap();
        assert_eq!(got.stage_mask, 1 << 2);
        // Unknown names: a setting and a control identifier, counted and listed.
        let b2 = resign(doctor(
            &bytes,
            "setting.tap = 5\n",
            "setting.tap = 5\nsetting.future.knob = 1\n",
        ));
        let b2 = resign(doctor(
            &b2,
            "change.0 = 12000 agc.knee_dbfs",
            "change.0 = 12000 agc.future",
        ));
        read(&b2, &mut got).unwrap();
        assert_eq!(got.flags, flags::UNKNOWN_SETTINGS);
        assert_eq!(got.unknown_setting_count, 2);
        assert_eq!(record_unknown_setting(&b2, 0), Some("future.knob"));
        assert_eq!(record_unknown_setting(&b2, 1), Some("agc.future"));
        assert_eq!(record_unknown_setting(&b2, 2), None);
        assert_eq!(record_change(&b2, 0), None);
        assert_eq!(got.change_count, 1);
        // Fewer rows than the totals.
        read(
            &resign(doctor(&bytes, "event.1 = 1800 120 16558\n", "")),
            &mut got,
        )
        .unwrap();
        assert_eq!(got.flags, flags::INCOMPLETE_RUN);
        assert_eq!(got.event_count, 1);
        // A row out of sequence is malformed.
        assert!(matches!(
            read_err(&resign(doctor(&bytes, "event.1 = ", "event.2 = "))),
            Error::MalformedRecord { .. }
        ));
    }

    #[test]
    fn checksum_covers_the_lf_before_it() {
        let r = readout();
        let bytes = written(&r, &[], &[]);
        let body_end = bytes[..bytes.len() - 1]
            .iter()
            .rposition(|&b| b == b'\n')
            .unwrap()
            + 1;
        assert_eq!(bytes[body_end - 1], b'\n');
        let h = crate::digest::fnv1a64(&bytes[..body_end]);
        let want = std::format!("record_checksum = fnv1a64:{h:016x}\n");
        assert_eq!(&bytes[body_end..], want.as_bytes());
    }

    #[test]
    fn host_centre_rule() {
        assert_eq!(host_centre(0, 1, 8, 242), (0, true));
        assert_eq!(host_centre(100, 1, 8, 242), (558, false));
        assert_eq!(host_centre(7, 20, 147, 0), (51, false)); // ⌊7 · 147 ÷ 20⌋
        let ev = [VasEvent::new(600, 300), VasEvent::new(1800, 120)];
        let rows: Vec<RecordEvent> = EventRows::new(&ev, 1, 8, 242, true).collect();
        assert_eq!(rows[0].host_input_position, 600 * 8 - 242);
        assert_eq!(rows[1].host_input_position, (1800 + 300) * 8 - 242);
        let rows: Vec<RecordEvent> = EventRows::new(&ev, 1, 8, 242, false).collect();
        assert_eq!(
            rows[1].host_input_position,
            1800 * 8 - 242,
            "mute mode: nothing removed"
        );
    }

    #[test]
    fn stop_fields_from_readout() {
        let r = readout();
        assert_eq!(StopFields::from(&r), r.stop);
    }
}
