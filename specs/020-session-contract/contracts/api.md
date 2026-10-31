# Contract: the Rust and C API of spec 020

## Rust (`rr_dr60`)

```rust
pub mod digest { pub fn fnv1a64(bytes: &[u8]) -> u64; }
pub mod input  { pub fn samples_from_i24(src: &[i32], dst: &mut [f32]); }
pub mod record {
    pub struct RunData<'a> { pub events: &'a [VasEvent], pub changes: &'a [ChangeEntry], pub stop: &'a StopOutcome }
    pub struct RecordReadout { /* data-model.md */ }
    pub struct RecordEvent { pub kept_position: u64, pub input_length: u64, pub host_input_position: u64 }
    pub fn read(bytes: &[u8], out: &mut RecordReadout) -> Result<(), Error>;
    pub fn record_event(bytes: &[u8], i: u64) -> Option<RecordEvent>;
    pub fn record_change(bytes: &[u8], i: u64) -> Option<ChangeEntry>;
    pub const FORM_VERSION: u32 = 1;
}
pub mod timeline {
    pub fn kept_to_input(k: u64, settings: &Settings, events: &[VasEvent], stop: &StopFields) -> InputAlignment;
    pub fn input_to_kept(p: u64, settings: &Settings, events: &[VasEvent]) -> KeptOrRemoved;
    pub fn onepass_output_to_input(position: u64, events: &[VasEvent]) -> u64;
    pub fn place_at(index: u64, start_level: u8, unit_count: u64, entries: &[ChangeEntry]) -> Result<PlaceState, Error>;
}
impl Pipeline {
    pub fn new_at_counter(settings: Settings, counter: u64) -> Result<Self, Error>;
    pub fn processing_record_len(&self, number: u32, run: &RunData<'_>) -> Result<usize, Error>;
    pub fn processing_record_write(&self, number: u32, run: &RunData<'_>, out: &mut [u8]) -> Result<usize, Error>;
    pub fn recording_digest(&self, number: u32) -> Result<u64, Error>;
    pub fn recording_digest_matches(&self, number: u32, readout: &mut RecordReadout) -> Result<bool, Error>;
    pub fn recording_counter_of(&self, number: u32) -> Result<Option<u64>, Error>;   // None: imported
}
```

Existing items changed: `ChangeEntry.reserved` → `stream`; `StopOutcome` + `stop_input_position`, `padding_start_sample`.

## C (`rr_dr60_ffi`, header regenerated)

```c
RrDr60Status rr_dr60_create_at_counter(const RrDr60Settings*, uint64_t counter, RrDr60Pipeline**);
uint64_t     rr_dr60_digest(const uint8_t*, size_t);
RrDr60Status rr_dr60_recording_digest(const RrDr60Pipeline*, uint32_t number, uint64_t* out);
RrDr60Status rr_dr60_processing_record_len(const RrDr60Pipeline*, uint32_t, const RrDr60RunData*, size_t* out);
RrDr60Status rr_dr60_processing_record_write(const RrDr60Pipeline*, uint32_t, const RrDr60RunData*, uint8_t*, size_t, size_t* written);
RrDr60Status rr_dr60_processing_record_read(const uint8_t*, size_t, RrDr60RecordReadout*);       // struct_size first
RrDr60Status rr_dr60_recording_digest_matches(const RrDr60Pipeline*, uint32_t, RrDr60RecordReadout*, bool* out);
bool         rr_dr60_record_event(const uint8_t*, size_t, uint64_t i, RrDr60RecordEvent*);
bool         rr_dr60_record_change(const uint8_t*, size_t, uint64_t i, RrDr60ChangeEntry*);
RrDr60Status rr_dr60_kept_to_input(const RrDr60Settings*, uint64_t k, const RrDr60VasEvent*, size_t, const RrDr60StopOutcome*, RrDr60InputAlignment*);
RrDr60Status rr_dr60_input_to_kept(const RrDr60Settings*, uint64_t p, const RrDr60VasEvent*, size_t, uint64_t* kept, uint64_t* removed_by_event);  // removed: UINT64_MAX when kept
uint64_t     rr_dr60_onepass_output_to_input(uint64_t position, const RrDr60VasEvent*, size_t);
RrDr60Status rr_dr60_place_at(uint64_t index, uint8_t start_level, uint64_t unit_count, const RrDr60ChangeEntry*, size_t, RrDr60PlaceState*);
void         rr_dr60_samples_from_i24(const int32_t*, float*, size_t);
#define RR_DR60_PROCESSING_RECORD_VERSION 1
```

Statuses 31 – 35 appended; `RrDr60StopOutcome` grows by two `uint64_t` (no size field; the pre-1.0 ABI pass); `RrDr60ChangeEntry.reserved` keeps its name in C with the comment "the stream word since 0.20" (no C rename: the field's position and size are unchanged).

The form itself: [record-form.md](record-form.md).
