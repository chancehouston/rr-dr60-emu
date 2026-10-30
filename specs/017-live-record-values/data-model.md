# Data model: Live Record-Side Values (017)

## Record-side values (`RecordValues`, crate-private)

The eight live settings as one copyable record: `sensitivity: u8`, `threshold_dbfs: f32`, `hang_ms: f32`, `onset_ms: f32`, `knee_dbfs: f32`, `attack_ms: f32`, `release_ms: f32`, `hiss_offset_db: f32`; `from(&Settings)` and `write_into(&mut Settings)`. Each has its control manifest index (`controls::index_of`), its `Setting` for validation and its stage (5, 5, 5, 5, 3, 3, 3, 1).

## Record control state (`RecordCtl`, in `RecordPath`)

| State | Type | Reset by | Spec |
|---|---|---|---|
| input (the clock) | `u64`, host input samples processed; the next sample has this index | the recorder at `start_recording`; the one-pass at construction, `reset`, `reconfigure` | D2, FR-002 |
| schedule | `Schedule` (spec 015's, 64) | never by the path's reset; an explicit clear at every end of a recording (`stop_recording`, the self-stop), `reset`, `reconfigure`; kept by `start_recording` | D5, FR-002 |
| `next_apply` | `u64`: 0 while a cross-stream pending change waits, else the earliest pending position, `u64::MAX` when none | with the path | R-02 |
| last applied | `RecordValues` | set without an entry by `adopt(&settings)`, which also applies the values to the three stages: `start_recording` (the recorder), `reset` (both); `reconfigure` rebuilds both | D4 |
| pending cross-stream | up to eight `(control, value, tag)` from `begin_call`'s comparison of the settings with `last` | applied at the first input sample of the call, before the stream's own due entries; logged with the landing's tag (the playback side's mirror carries 0, spec 015) | D4, R-02 |
| `landed` | the last landed `(value, tag)` per control in the call | cleared at the call's entry; written to the settings and `record_tags` in the epilogue | D4 |
| staging log | `[ChangeEntry; 72]`, `len` (64 scheduled + 8 cross-stream) | cleared at every call's entry | FR-005 |

`RecordPath` is `repr(C)`: its hot scalars first, `ctl` (`input`, `next_apply`, `len`, the small fields, then `schedule`, then the log) last. The hiss's at-once rule reads `input == 0`.

## The landing rules (D3), by control

| Control | Applied | First affected | Logged position |
|---|---|---|---|
| `agc.knee_dbfs` | at the top of input sample P: `Limiter::retune` (vbe6, ceiling_fs, charge_gain) | sample P; vc, vc_max, v_floor, v_limit carried | P |
| `agc.attack_ms` | `retune` (charge_gain) | sample P | P |
| `agc.release_ms` | `retune` (dis_r11, dis_b5) | sample P | P |
| `mic.hiss_offset_db` | `MicCapsule::set_offset`: a transition from the gain at P over N = 0.02 × rate samples (the smoothstep): the gain at sample P + j is `g_from + (g_to − g_from) · s((j + 1) ÷ N)` for j = 0 … N − 2 and `g_to` at j = N − 1 (spec 011's indexing: the landing sample is transition sample 1); at once when `input == 0` or the capsule is bypassed | sample P (the gain's first move) | P |
| `vas.sensitivity`, `vas.threshold_dbfs` | `VasStage::set_values` at the top of input sample P (the threshold from both) | device sample k(P) = ⌈P · 6000 ÷ rate⌉ | P |
| `vas.hang_ms`, `vas.onset_ms` | `set_values` (the lengths; counts kept) | k(P): a silent count past the new hang pauses at the first non-sound device sample ≥ k(P); a sound run past the new onset resumes at the next sound sample | P |

A position at or below `input` lands on the next input sample. A cross-stream pending change lands at the call's first input sample with the landing's tag.

## Clocks and helpers

- `changes::Clock { Input = 0, Output = 1 }`; `clock_of(control) -> Result<Clock, Error>`: Input when the control's paths include the record path, else Output; `NoSuchControl` past the list.
- `record_landing(P, rate) -> Result<u64, Error>` = ⌈P · 6000 ÷ rate⌉ (0 at P = 0; `UnsupportedHostRate` otherwise). `speed_landing` unchanged.
- `input_position(stream)`: `Record` → `recorder.ctl.input` while recording, 0 idle; `OnePass` → `one_pass.rec.ctl.input`; `Playback` → `Err(NoSuchClock)`. `stream_position(Record)` → `Err(NoSuchClock)`.

## Reports

- `RecordInfo` gains `changes: usize` and `input_samples_total: u64` (the self-stop's final position); `StopOutcome` gains `changes: usize` and `input_samples_total: u64`; `BlockInfo.changes` counts both sides. The C report structures are unchanged (counts and positions as out-parameters; the pre-1.0 ABI list). `rest_meters()` reads the settings' hang, not the recorder's stage (no audio change).
- `MAX_CHANGES_PER_CALL` = **139** = 2 · 64 + 3 + 8 (Rust and C; the one-pass attains 138, the recorder 72, the player 67); `SCHEDULE_CAPACITY` 64 per schedule.

## Errors

`InvalidSetting { setting }` for an out-of-range record-side value (the existing error); `ControlNotOnStream` for an idle control or the wrong stream; `NoSuchClock { stream }` (new; C status 29 `NO_SUCH_CLOCK`) for a clock a stream does not have. `Error::ReservedStream` is kept for future reserved `Stream` values (removing a public variant is a break) but no 0.17 call returns it; its C mapping (28) is unchanged and its doc says so.
