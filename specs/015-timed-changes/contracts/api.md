# Contract: Rust and C API (015)

Additive only: no existing item changes its signature, layout or behavior; `RrDr60Settings` stays 120 bytes. Version 0.16.0.

## Rust (`rr_dr60`)

```rust
pub mod changes {
    pub const SCHEDULE_CAPACITY: usize = 64;      // pending changes per stream (D4)
    pub const MAX_CHANGES_PER_CALL: usize = 67;   // an always-enough log length (D5)
    /// d(P): the device-output index at which a speed change scheduled at host output P lands:
    /// ⌊(P−1)·6000÷rate⌋+1, d(0)=0. Err(UnsupportedHostRate) for a rate not in SUPPORTED_HOST_RATES.
    pub fn speed_landing(position: u64, host_rate_hz: u32) -> Result<u64, Error>;
}

#[non_exhaustive] #[repr(u32)] #[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream { Playback = 0, OnePass = 1, Record = 2 /* reserved */ }

#[non_exhaustive] #[repr(C)] #[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ChangeEntry {
    pub position: u64, pub before: f64, pub after: f64, pub tag: u64,
    pub control: u32, pub reserved: u32,
}

impl Pipeline {
    /// Schedules a change (spec 015 FR-001). Real-time safe; accepted in every engine state.
    /// The volume is rounded to single precision. Errors, in this order: ReservedStream,
    /// ControlNotOnStream, InvalidVolume / InvalidSpeedLevel / InvalidControlValue,
    /// TapNotOnRoute (the one-pass stream, a named-route tap), ScheduleFull. Nothing stored on an error.
    pub fn schedule_change(&mut self, stream: Stream, position: u64, control: u32, value: f64, tag: u64) -> Result<(), Error>;
    /// Pending changes on the stream (FR-002). Err(ReservedStream) for Record.
    pub fn pending_changes(&self, stream: Stream) -> Result<u32, Error>;
    /// The stream's position: host output samples produced (D2).
    pub fn stream_position(&self, stream: Stream) -> Result<u64, Error>;
    /// Drops every pending change on the stream (FR-002).
    pub fn clear_schedule(&mut self, stream: Stream) -> Result<(), Error>;
    /// The recording counter (D9; spec 012 FR-009).
    pub fn recording_counter(&self) -> u64;

    /// As process_with_events, and writes the call's change entries to `changes` (FR-005):
    /// `info.changes` counts them all; `MAX_CHANGES_PER_CALL` is always enough.
    pub fn process_with_events_and_changes(&mut self, input: &[f32], output: &mut [f32],
        events: &mut [VasEvent], changes: &mut [ChangeEntry]) -> Result<BlockInfo, Error>;
    /// As process_in_place_with_events, and writes the call's change entries (FR-005; the C in-place path).
    pub fn process_in_place_with_events_and_changes(&mut self, buffer: &mut [f32],
        events: &mut [VasEvent], changes: &mut [ChangeEntry]) -> BlockInfo;
    /// As play, and writes the call's change entries (FR-005).
    pub fn play_with_changes(&mut self, output: &mut [f32], changes: &mut [ChangeEntry]) -> Result<PlayInfo, Error>;
}

// BlockInfo and PlayInfo (both #[non_exhaustive]) gain `pub changes: usize`: the changes the
// call applied (filled by every variant; only the *_with_changes variants write entries).
// Error gains ScheduleFull { stream }, ControlNotOnStream { control, stream }, ReservedStream { stream },
// InvalidControlValue { control, value }.
```

Behavior contract: the landing rules of data-model.md; the plain setters unchanged (FR-004); the readings move at landing (R-07); the schedules' clearing rules (FR-002; `start_playback` keeps); the control index is the control manifest's, resolved by identifier, never assumed; tag 0 is reserved for plain-setter changes; no allocation anywhere (the schedule and the staging log are inline in the path).

## C (`include/rr_dr60.h`)

```c
#define RR_DR60_SCHEDULE_CAPACITY 64        /* literal consts in the FFI crate, pinned to the core's */
#define RR_DR60_MAX_CHANGES_PER_CALL 67

typedef enum RrDr60Stream {                /* emitted by cbindgen (include list), the RrDr60Route pattern */
  RR_DR60_STREAM_PLAYBACK = 0, RR_DR60_STREAM_ONE_PASS = 1, RR_DR60_STREAM_RECORD = 2 /* reserved */
} RrDr60Stream;

typedef struct RrDr60ChangeEntry {   /* 40 bytes; no size field (pre-1.0 ABI-pass list) */
  uint64_t position; double before; double after; uint64_t tag; uint32_t control; uint32_t reserved;
} RrDr60ChangeEntry;

RrDr60Status rr_dr60_schedule_change(RrDr60Pipeline *p, uint32_t stream, uint64_t position,
                                     uint32_t control, double value, uint64_t tag);
RrDr60Status rr_dr60_pending_changes(const RrDr60Pipeline *p, uint32_t stream, uint32_t *out_count);
RrDr60Status rr_dr60_stream_position(const RrDr60Pipeline *p, uint32_t stream, uint64_t *out_position);
RrDr60Status rr_dr60_clear_schedule(RrDr60Pipeline *p, uint32_t stream);
RrDr60Status rr_dr60_recording_counter(const RrDr60Pipeline *p, uint64_t *out_counter);
RrDr60Status rr_dr60_speed_landing(uint64_t position, uint32_t host_rate_hz, uint64_t *out_index);

RrDr60Status rr_dr60_process_with_events_and_changes(RrDr60Pipeline *p, const float *input, float *output,
    size_t frames, RrDr60BlockInfo *out_info, RrDr60VasEvent *events, size_t events_capacity,
    RrDr60ChangeEntry *changes, size_t changes_capacity, size_t *out_change_count);
RrDr60Status rr_dr60_play_with_changes(RrDr60Pipeline *p, float *output, size_t frames,
    RrDr60PlayInfo *out_info, RrDr60ChangeEntry *changes, size_t changes_capacity, size_t *out_change_count);
```

Statuses: `RR_DR60_STATUS_SCHEDULE_FULL = 27`, `RR_DR60_STATUS_CONTROL_NOT_ON_STREAM = 28` (also for a reserved or unknown stream value and a control index that is not a live control on the stream); a route value not 0 or 1 → `INVALID_ARGUMENT`; the volume and the level their existing statuses; `rr_dr60_speed_landing` with a rate not in the supported list → `INVALID_ARGUMENT` (a 0 rate never divides); a NULL handle or out-pointer → `NULL_POINTER`; `changes == NULL` with `changes_capacity > 0` → `NULL_POINTER`; `out_change_count` may be NULL (not written). `RrDr60BlockInfo` and `RrDr60PlayInfo` are unchanged. The header is regenerated with cbindgen and drift-checked; `RrDr60Stream` is added to `cbindgen.toml`'s `include` (the struct is emitted by reference).

Real-time list (the guide and the crate doc): `rr_dr60_schedule_change`, `rr_dr60_pending_changes`, `rr_dr60_stream_position`, `rr_dr60_clear_schedule`, `rr_dr60_recording_counter`, `rr_dr60_speed_landing`, the two log-taking calls.
