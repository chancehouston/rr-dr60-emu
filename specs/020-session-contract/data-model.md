# Data Model: Processing Record (spec 020)

## SlotBlock (core, per recording slot)

| Field | Type | Written | Meaning |
|---|---|---|---|
| counter | u64 | `start_recording` | the recording counter the hiss stream was keyed with |
| settings | Settings (Copy) | `start_recording` | the configuration at the start (seed and host rate inside) |
| remaining_samples | u64 | `start_recording` | the memory's remaining capacity before the recording opened |
| events_total | u64 | per event (cold) | voice-activation events the recording generated |
| changes_total | u64 | per landing (cold) | record-stream entries that landed during the recording |

Lifecycle: `Vec<Option<SlotBlock>>` with capacity `max_recordings` at construction; `Some` at `start_recording`; moved with the recording by `erase` (`remove`); `None` pushed by `import`; cleared with `erase_all` and a memory-emptying `reconfigure`; kept by a memory-keeping `reconfigure` and by `reset`.

## RunData (caller → writer)

| Field | Type | Validation (`InconsistentRun`) |
|---|---|---|
| events | &[VasEvent] | positions non-decreasing; each `output_position` ≤ the kept count |
| changes | &[ChangeEntry] | positions non-decreasing; each ≤ `stop.stop_input_position`; `stream == Stream::Record` |
| stop | &StopOutcome | `kept == true`; `number` the recording's |

## RecordReadout (reader → caller; fixed size)

| Field | Type |
|---|---|
| form_version | u32 |
| library_version | [u16; 3] |
| stage_model_versions | [[u16; 3]; 15] (0.0.0 for a stage not modeled) |
| host_rate_hz | u32 |
| seed | u64 |
| recording_counter | u64 (valid when `origin == Engine`) |
| origin | Engine / Imported |
| stored_form_version, encoding, unit_count | u16, u16, u64 |
| recording_digest | u64 |
| settings | Settings (the record's; unknown names skipped) |
| remaining_samples_at_start, events_total, changes_total | u64 |
| stop | Host / MemoryFull |
| stop_input_position, padding_start_sample, input_samples_total, silence_fed_samples | u64 |
| event_count, change_count | u64 (lines present) |
| unknown_setting_count | u32 |
| flags | bitset: LIBRARY_DIFFERS, STAGE_MODEL_DIFFERS (+ a 15-bit stage mask), HOST_RATE_DIFFERS, UNKNOWN_SETTINGS, INCOMPLETE_RUN, IMPORTED_ORIGIN, DIGEST_MISMATCH (set by `recording_digest_matches`) |

Cursors: `record_event(bytes, i) -> Option<RecordEvent { kept_position, input_length, host_input_position }>`, `record_change(bytes, i) -> Option<ChangeEntry>` (re-scan; no allocation).

## StopOutcome (extended, non-exhaustive)

`+ stop_input_position: u64`, `+ padding_start_sample: u64`.

## ChangeEntry

`reserved: u32` → `stream: u32` (0 playback, 1 one-pass, 2 record), set at the landing.

## InputAlignment / KeptOrRemoved / PlaceState (timeline)

`InputAlignment { centre: u64, first: u64, last: u64, clamped: bool }` (host input samples); `KeptOrRemoved::Kept(k) | Removed { event_index }`; `PlaceState { place_fifths: u64, level: u8, fading: bool, old_place_fifths: u64, latched: bool }`.

## Statuses

`UnknownRecordVersion { version }`, `MalformedRecord { line }`, `MissingRecordField { field: &'static str }`, `RecordChecksumMismatch`, `InconsistentRun { reason: &'static str }`; C 31 – 35.
