# Contract: the processing record form, version 1 (spec 020 D2)

ASCII, LF line ends, `name = value` with one space each side. Lines in this order:

```
rr_dr60_processing_record_version = 1
library_version = 0.20.0
stage.1.model_version = 0.12.0        # one line per modeled stage, in stage order
...
host_rate_hz = 48000
origin = engine                        # or: imported
seed = 0                               # absent when imported
recording_counter = 3                  # absent when imported
stored_form_version = 2
encoding = 1                           # 1 frames, 2 samples
unit_count = 1500
recording_digest = fnv1a64:0123456789abcdef
setting.<identifier> = <value>         # every control-manifest setting in manifest order; absent when imported
run.remaining_samples_at_start = 21600000
run.events_total = 12
run.changes_total = 2
run.stop = host                        # or: memory_full
run.stop_input_position = 1440000
run.padding_start_sample = 179940
run.input_samples_total = 1441920
run.silence_fed_samples = 1920
event.0 = <kept_position> <input_length> <host_input_position>
change.0 = <position> <control> <before> <after> <tag> <stream>
record_checksum = fnv1a64:<16 hex>     # over every byte above this line
```

Values: integers decimal; reals shortest round-trip decimal (f32 for settings, f64 for before/after); bools `true`/`false`; choices the engine value; an unknown value is an absent line. The run lines are absent when `origin = imported`. The record contains no time, place, device identity, recording number or app data.

Reading: version > 1 → `UnknownRecordVersion`; the last line's checksum first; then shapes; then required fields. Differences (library, stage model, host rate, unknown setting names, counts below the totals, a digest that does not match a named recording) are flags, never refusals.
