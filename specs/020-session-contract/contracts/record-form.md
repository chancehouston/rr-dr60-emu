# Contract: the processing record form, version 1 (spec 020 D2)

ASCII, LF line ends, every line `name = value` with one space each side; names `[a-z0-9_.]+`; the record ends with exactly one LF. **The form has no comment syntax**: the notes in the Required column below are this contract's, never part of a record; a line that is not `name = value`, or whose name version 1 does not define, is refused as malformed.

| Line (in this order) | Required | Example |
|---|---|---|
| `rr_dr60_processing_record_version = 1` | always (first line) | |
| `library_version = <major.minor.patch>` | always: the engine that wrote the record (under `origin = engine` also the engine that made the recording) | `0.20.0` |
| `origin = engine` or `imported` | always | |
| `host_rate_hz = <u32>` | engine origin | `48000` |
| `stage.<n>.model_version = <major.minor.patch>` | engine origin, one line per modeled stage in stage order (absent = not modeled) | `stage.3.model_version = 0.5.0` |
| `seed = <u64>` | engine origin | `9123372036854775807` |
| `recording_counter = <u64>` | engine origin | `3` |
| `stored_form_version = 2` | always | |
| `stored_codec_model_version = <major.minor.patch>` | always (`0.0.0` with samples) | `0.8.0` |
| `encoding = frames` or `samples` | always | |
| `stored_unit_count = <u64>` | always (frames or samples by `encoding`) | `1500` |
| `recording_digest = fnv1a64:<16 lowercase hex>` | always | |
| `setting.<identifier> = <value>` | engine origin: the control manifest's 24 settings in manifest order, except `host_rate_hz` and `seed`, which identity carries | `setting.vas.sensitivity = 3` |
| `run.remaining_samples_at_start = <u64>` (device samples) | engine origin | `21600000` |
| `run.events_total = <u64>` | engine origin | `12` |
| `run.changes_total = <u64>` | engine origin | `2` |
| `run.stop = stop_call` or `memory_full` | engine origin (`stop_call` whenever the host called the stop, even if its flush filled the memory) | |
| `run.stop_input_position = <u64>` (host input samples fed before the stop) | engine origin | `1440000` |
| `run.padding_start_sample = <u64>` (kept device samples) | engine origin | `179940` |
| `run.input_samples_total = <u64>` (host input samples, the stop's silence included) | engine origin | `1440242` + the padding's zeros |
| `run.silence_fed_samples = <u64>` (post-gate device samples of the padding, ≤ 179) | engine origin | `119` |
| `event.fields = kept_position input_length host_input_position` | engine origin, before the first event row (also present with zero events) | |
| `event.<i> = <kept_position> <input_length> <host_input_position>` (device, device, host) | per event, i from 0 | `event.0 = 6000 3000 48242` |
| `change.fields = position control before after tag stream` | engine origin, before the first change row | |
| `change.<i> = <position> <control identifier> <before> <after> <tag> <stream word>` | per entry, i from 0 | `change.0 = 48000 agc.knee_dbfs -1 -4 11 record` |
| `record_checksum = fnv1a64:<16 lowercase hex>` | always (last line): over every byte above it, the LF before it included | |

Values: integers decimal; reals the shortest decimal that reads back to the same bits (f32 for settings, f64 for before/after); bools `true`/`false`; choices the control manifest's engine value; an unknown value is an absent line, never a sentinel. Under `origin = imported` only the always-required lines are present: the record names the stored form and its digest; there is nothing to re-render. The record contains no time, place, device identity, recording number or app data.

Reading: version 0 or above 1 → `UnknownRecordVersion`; the last line's checksum (`RecordChecksumMismatch`; a missing final LF is malformed); then shapes and names (`MalformedRecord`); then the required fields per origin (`MissingRecordField`). Differences (library, stage model, host rate, a control identifier or setting name this library does not know, rows fewer than the totals, a digest that does not match a named recording) are flags, never refusals; every later library reads every earlier form version.
