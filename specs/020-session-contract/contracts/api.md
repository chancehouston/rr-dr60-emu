# Contract: the Rust and C API of spec 020

## Rust (`rr_dr60`)

```rust
pub mod digest { pub fn fnv1a64(bytes: &[u8]) -> u64; }
pub mod input  { pub fn samples_from_i24(src: &[i32], dst: &mut [f32]); }
pub mod record {
    pub const FORM_VERSION: u32 = 1;
    pub struct StopFields { pub how: StopHow, pub stop_input_position: u64, pub padding_start_sample: u64, pub input_samples_total: u64, pub silence_fed_samples: u64 }
    pub struct RunData<'a> { pub events: &'a [VasEvent], pub changes: &'a [ChangeEntry], pub stop: StopFields }
    pub struct RecordReadout { /* data-model.md */ }
    pub struct RecordEvent { pub event: VasEvent, pub host_input_position: u64 }
    pub fn len(readout: &RecordReadout, events: &[RecordEvent], changes: &[ChangeEntry]) -> usize;
    pub fn write(readout: &RecordReadout, events: &[RecordEvent], changes: &[ChangeEntry], out: &mut [u8]) -> Result<usize, Error>;
    pub fn read(bytes: &[u8], out: &mut RecordReadout) -> Result<(), Error>;
    pub fn record_event(bytes: &[u8], i: u64) -> Option<RecordEvent>;
    pub fn record_change(bytes: &[u8], i: u64) -> Option<ChangeEntry>;
    pub fn record_unknown_setting(bytes: &[u8], i: u32) -> Option<&str>;
    pub fn record_events(bytes: &[u8], first: u64, out: &mut [VasEvent]) -> usize;
    pub fn record_changes(bytes: &[u8], first: u64, out: &mut [ChangeEntry]) -> usize;
}
pub mod timeline {
    pub fn kept_to_input(k: u64, settings: &Settings, events: &[VasEvent], stop: &StopFields) -> InputAlignment;
    pub fn input_to_kept(p: u64, settings: &Settings, events: &[VasEvent]) -> KeptOrRemoved;
    pub fn onepass_output_to_input(position: u64, latency: u32, events: &[VasEvent], drop: bool) -> (u64, bool /* clamped */);   // latency: Pipeline::latency_samples(); drop: the gate in drop mode (restated at implementation)
    pub fn place_at(index: u64, host_rate_hz: u32, start_level: u8, unit_count: u64, entries: &[ChangeEntry]) -> Result<PlaceState, Error>;
}
impl VasEvent { pub const fn new(output_position: u64, input_length: u64) -> Self; }
impl Pipeline {
    pub fn new_at_counter(settings: Settings, counter: u64) -> Result<Self, Error>;
    pub fn record_latency_pre_encoder_samples(&self) -> u32;
    pub fn processing_record_len(&self, number: u32, run: &RunData<'_>) -> Result<usize, Error>;
    pub fn processing_record_write(&self, number: u32, run: &RunData<'_>, out: &mut [u8]) -> Result<usize, Error>;
    pub fn recording_digest(&self, number: u32) -> Result<u64, Error>;
    pub fn recording_digest_matches(&self, number: u32, readout: &mut RecordReadout) -> Result<bool, Error>;
    pub fn recording_counter_of(&self, number: u32) -> Result<Option<u64>, Error>;   // None: imported
    pub fn processing_record_read(&self, bytes: &[u8], out: &mut RecordReadout) -> Result<(), Error>;   // record::read plus the host-rate flag
}
```

Existing items changed: `ChangeEntry.reserved` → `stream`; `StopOutcome` + `stop_input_position`, `padding_start_sample`; `StopFields: From<&StopOutcome> + From<&RecordInfo> + From<&RecordReadout>`.

## C (`rr_dr60_ffi`, header regenerated)

```c
RrDr60Status rr_dr60_create_at_counter(const RrDr60Settings*, uint64_t counter, RrDr60Pipeline**);
uint32_t     rr_dr60_record_latency_pre_encoder_samples(const RrDr60Pipeline*);
uint64_t     rr_dr60_digest(const uint8_t*, size_t);
RrDr60Status rr_dr60_recording_digest(const RrDr60Pipeline*, uint32_t number, uint64_t* out);
RrDr60Status rr_dr60_processing_record_len(const RrDr60Pipeline*, uint32_t, const RrDr60RunData*, size_t* out);
RrDr60Status rr_dr60_processing_record_write(const RrDr60Pipeline*, uint32_t, const RrDr60RunData*, uint8_t*, size_t, size_t* written);
RrDr60Status rr_dr60_processing_record_read(const uint8_t*, size_t, RrDr60RecordReadout*);          // struct_size first
RrDr60Status rr_dr60_processing_record_rewrite(const uint8_t*, size_t, uint8_t*, size_t, size_t* written);   // read + write
RrDr60Status rr_dr60_recording_digest_matches(const RrDr60Pipeline*, uint32_t, RrDr60RecordReadout*, bool* out);
bool         rr_dr60_record_event(const uint8_t*, size_t, uint64_t i, RrDr60VasEvent*, uint64_t* host_input_position);
bool         rr_dr60_record_change(const uint8_t*, size_t, uint64_t i, RrDr60ChangeEntry*);
size_t       rr_dr60_record_events(const uint8_t*, size_t, uint64_t first, RrDr60VasEvent*, size_t);
size_t       rr_dr60_record_changes(const uint8_t*, size_t, uint64_t first, RrDr60ChangeEntry*, size_t);
bool         rr_dr60_record_unknown_setting(const uint8_t*, size_t, uint32_t i, char*, size_t);
RrDr60Status rr_dr60_kept_to_input(const RrDr60Settings*, uint64_t k, const RrDr60VasEvent*, size_t, const RrDr60StopFields*, RrDr60InputAlignment*);
RrDr60Status rr_dr60_input_to_kept(const RrDr60Settings*, uint64_t p, const RrDr60VasEvent*, size_t, uint64_t* kept, uint64_t* removed_by_event);  // removed: UINT64_MAX when kept
RrDr60Status rr_dr60_onepass_output_to_input(uint64_t position, uint32_t latency, const RrDr60VasEvent*, size_t, bool drop, uint64_t* input, bool* clamped);
RrDr60Status rr_dr60_record_write(const RrDr60RecordReadout*, const RrDr60VasEvent*, const uint64_t* host_input_positions, size_t, const RrDr60ChangeEntry*, size_t, uint8_t*, size_t, size_t* written);   // the pure writer
RrDr60Status rr_dr60_recording_counter_of(RrDr60Pipeline*, uint32_t number, bool* known, uint64_t* counter);
RrDr60Status rr_dr60_place_at(uint64_t index, uint32_t host_rate_hz, uint8_t start_level, uint64_t unit_count, const RrDr60ChangeEntry*, size_t, RrDr60PlaceState*);
void         rr_dr60_samples_from_i24(const int32_t*, float*, size_t);
#define RR_DR60_PROCESSING_RECORD_VERSION 1
```

Statuses 31 – 35 appended; `RrDr60StopOutcome` grows 64 → 80 bytes (no size field; the pre-1.0 ABI pass); `RrDr60ChangeEntry.reserved` keeps its C name with the comment "the stream word since 0.20 (0 none, 1 playback, 2 one-pass, 3 record)". The new structs `RrDr60StopFields`, `RrDr60RunData`, `RrDr60RecordReadout`, `RrDr60InputAlignment`, `RrDr60PlaceState` start with `struct_size` (prefix-fill as spec 018's meters). *Amended by spec 022 (2026-10-31): `RrDr60StopOutcome` starts with `struct_size` (88 bytes) with the flush's fields inline (`flush_*`); `RrDr60ChangeEntry.reserved` is named `stream` and the struct is frozen; `RrDr60StopFields` is frozen in layout (its size written as 40, ignored on read); `RrDr60RunData` is refused above `RR_DR60_MAX_STRUCT_SIZE` on read; `RrDr60RecordReadout`, `RrDr60InputAlignment` and `RrDr60PlaceState` take the cap and read back the filled size, with every padding byte zero (the read-out's fifteen padding bytes, its own 212 – 215 and the embedded settings' four runs, were copied from uninitialized stack values before 0.22); in a prefix fill of the read-out the embedded `settings.struct_size` is the settings bytes the prefix holds.*

The form itself: [record-form.md](record-form.md).
