# Contract: Rust and C API (017)

Additive but for one constant: `MAX_CHANGES_PER_CALL` 67 → 139 (= 2 · 64 schedules + 3 plain setters + 8 cross-stream landings; buffers sized by the constant recompile; a host that hard-coded 67 still receives the first 67 entries and the true count; recorded in the CHANGELOG). `Error::ReservedStream` stays declared but is no longer returned. `RrDr60Settings` stays 120 bytes; the C report structures are unchanged. Version 0.17.0.

## Rust (`rr_dr60`)

```rust
pub mod changes {
    pub const MAX_CHANGES_PER_CALL: usize = 139;   // 2·64 + 3 + 8 (spec 017 D2, restated at the plan)
    #[non_exhaustive] #[repr(u32)] pub enum Clock { Input = 0, Output = 1 }
    /// Input for a control whose manifest paths include the record path (record wins for the idle controls on both paths), else Output; total over the manifest.
    pub fn clock_of(control: u32) -> Result<Clock, Error>;          // Err(NoSuchControl)
    /// k(P) = ⌈P · 6000 ÷ rate⌉: the device sample at which a gate value scheduled at input P takes effect.
    pub fn record_landing(position: u64, host_rate_hz: u32) -> Result<u64, Error>;
}

impl Pipeline {
    // schedule_change accepts the eight record-side controls on Stream::Record and Stream::OnePass
    // (positions in the input clock); the value validated as the setting (InvalidSetting).
    /// The stream's input position (D2). Err(NoSuchClock) for Playback.
    pub fn input_position(&self, stream: Stream) -> Result<u64, Error>;
    /// As record, and writes the call's change entries (FR-005).
    pub fn record_with_changes(&mut self, input: &[f32], events: &mut [VasEvent], changes: &mut [ChangeEntry]) -> Result<RecordInfo, Error>;
    /// As stop_recording, and writes the entries landed during the stop's feed (FR-005).
    pub fn stop_recording_with_changes(&mut self, events: &mut [VasEvent], changes: &mut [ChangeEntry]) -> Result<StopOutcome, Error>;
}
// RecordInfo gains `changes: usize` and `input_samples_total: u64`; StopOutcome gains `changes: usize` and `input_samples_total: u64`
// (both #[non_exhaustive]). pending_changes / clear_schedule accept Stream::Record; stream_position(Record) → Err(NoSuchClock).
// Error gains NoSuchClock { stream }. settings() reflects landed values.
```

## C (`include/rr_dr60.h`)

```c
#define RR_DR60_MAX_CHANGES_PER_CALL 139
typedef enum RrDr60Clock { RR_DR60_CLOCK_INPUT = 0, RR_DR60_CLOCK_OUTPUT = 1 } RrDr60Clock;

RrDr60Status rr_dr60_clock_of(uint32_t control, uint32_t *out_clock);                 /* NO_SUCH_CONTROL */
RrDr60Status rr_dr60_record_landing(uint64_t position, uint32_t host_rate_hz, uint64_t *out_index);
RrDr60Status rr_dr60_input_position(const RrDr60Pipeline *p, uint32_t stream, uint64_t *out_position); /* NO_SUCH_CLOCK for playback */
RrDr60Status rr_dr60_record_with_changes(RrDr60Pipeline *p, const float *input, size_t frames, RrDr60RecordInfo *out_info,
    RrDr60VasEvent *events, size_t events_capacity, RrDr60ChangeEntry *changes, size_t changes_capacity, size_t *out_change_count,
    uint64_t *out_input_samples_total);   /* the final position after a self-stop */
RrDr60Status rr_dr60_stop_recording_with_changes(RrDr60Pipeline *p, RrDr60StopOutcome *out, RrDr60VasEvent *events, size_t events_capacity,
    RrDr60ChangeEntry *changes, size_t changes_capacity, size_t *out_change_count, uint64_t *out_input_samples_total);
```

Statuses: `RR_DR60_STATUS_NO_SUCH_CLOCK = 29`; `rr_dr60_clock_of` writes a `uint32_t` whose Rust enum is `#[non_exhaustive]` (a Swift switch needs a default); a record-side value out of range → `INVALID_SETTING` with the field in `rr_dr60_settings_validate`'s vocabulary (the schedule call reports the status only); an idle control or the wrong stream → `CONTROL_NOT_ON_STREAM`. The control manifest's eight entries read `RR_DR60_CONTROL_TIMING_LIVE`. Header regenerated and drift-checked; `RrDr60Clock` in `cbindgen.toml`'s `include`.

Real-time list additions: `rr_dr60_input_position`, `rr_dr60_record_with_changes`, `rr_dr60_stop_recording_with_changes`, `rr_dr60_clock_of`, `rr_dr60_record_landing`.
