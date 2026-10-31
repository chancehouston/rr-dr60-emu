# Data Model: Processing Record (spec 020)

## SlotBlock (inside `Memory`, parallel to the spans)

| Field | Type | Written | Meaning |
|---|---|---|---|
| counter | u64 | `open` | the recording counter the hiss stream was keyed with (read before the increment) |
| settings | Settings (Copy) | `open` | the configuration at the start (the seed and the host rate inside; after the adopt) |
| remaining_samples | u64 | `open` | the memory's remaining capacity before the recording opened (device samples) |
| events_total | u64 | per event (cold) | voice-activation events the recording generated |
| changes_total | u64 | per landing (cold) | record-stream entries that landed during the recording |

Lifecycle: `Vec<Option<SlotBlock>>` with capacity `max_recordings` at construction; pushed `Some` at `open`; popped on `close(false)` (a discarded empty recording) and `discard_in_progress`; `remove` on `erase`; `push(None)` on `import_samples` / `import_frames`; cleared on `erase_all`; copied by `copied_to` (a memory-keeping `reconfigure` re-reserves to the new count); kept by `reset`.

## StopFields (plain, Copy, buildable by anyone)

| Field | Type | From `StopOutcome` | From `RecordInfo` (self-stop) |
|---|---|---|---|
| how | StopHow::{StopCall, MemoryFull} | StopCall (even if the flush filled) | MemoryFull |
| stop_input_position | u64 | the position at the stop's entry | `input_samples_total` |
| padding_start_sample | u64 | `represented` (codec on) or the length | the length |
| input_samples_total | u64 | as reported | as reported |
| silence_fed_samples | u64 | as reported | 0 |

Also `From<&RecordReadout>`.

## RunData (caller → writer)

| Field | Type | Validation (`InconsistentRun { reason }`) |
|---|---|---|
| events | &[VasEvent] | positions non-decreasing ("out of order"); each `output_position` ≤ the stored device length + 60 with frames, ≤ the length with samples ("past the recording"); `len() ≤ events_total` ("more than counted") |
| changes | &[ChangeEntry] | positions non-decreasing; each `position < stop.input_samples_total` ("at or past the final input position"); `stream == 3` ("no stream" for 0, "not the record stream" otherwise); `len() ≤ changes_total` |
| stop | StopFields | `stop_input_position ≤ input_samples_total`; `padding_start_sample ≤ the length` |

## RecordReadout (reader → caller; fixed size)

| Field | Type |
|---|---|
| form_version | u32 |
| library_version | [u16; 3] (the writing engine's) |
| stage_model_versions | [[u16; 3]; 15] (0.0.0 absent or not modeled; absent under imported) |
| host_rate_hz | u32 (0 under imported) |
| origin | Engine / Imported |
| seed, recording_counter | u64 (valid under Engine) |
| stored_form_version, stored_codec_model_version, encoding (Frames/Samples), stored_unit_count | u16, [u16; 3], enum, u64 |
| recording_digest | u64 |
| settings | Settings (the record's 24 setting lines plus identity's host rate and seed; unknown names skipped) |
| remaining_samples_at_start, events_total, changes_total | u64 |
| stop | StopFields (`how` from `run.stop`) |
| event_count, change_count | u64 (rows present) |
| unknown_setting_count | u32 (the names through `record_unknown_setting`) |
| flags | bitset: LIBRARY_DIFFERS, STAGE_MODEL_DIFFERS (+ a 15-bit stage mask), HOST_RATE_DIFFERS, UNKNOWN_SETTINGS, INCOMPLETE_RUN (fewer rows than the totals), IMPORTED_ORIGIN, DIGEST_MISMATCH (set by `recording_digest_matches`) |

Cursors and batch reads (no allocation): `record_event(bytes, i) -> Option<RecordEvent { event: VasEvent, host_input_position: u64 }>`, `record_change(bytes, i) -> Option<ChangeEntry>`, `record_unknown_setting(bytes, i) -> Option<&str>`, `record_events(bytes, first, &mut [VasEvent]) -> usize`, `record_changes(bytes, first, &mut [ChangeEntry]) -> usize`.

## StopOutcome (extended, non-exhaustive)

`+ stop_input_position: u64`, `+ padding_start_sample: u64`.

## ChangeEntry

`reserved: u32` → `stream: u32` (0 none/pre-0.20, 1 playback, 2 one-pass, 3 record), set at the landing; the C field keeps the name `reserved`.

## VasEvent

`pub const fn new(output_position, input_length)` (so a recipient can assemble the maps' inputs).

## Timeline types

`InputAlignment { centre: u64, first: u64, last: u64, clamped: bool }` (host input samples); `KeptOrRemoved::Kept(k) | Removed { event_index: u64 }`; `PlaceState { place_fifths: u64, level: u8, fading: bool, old_place_fifths: u64, latched: bool }`.

## Statuses

`UnknownRecordVersion { version }`, `MalformedRecord { line }`, `MissingRecordField { field: &'static str }`, `RecordChecksumMismatch`, `InconsistentRun { reason: &'static str }`; C 31 – 35.
