# Contract: Rust and C API (016)

Additive only: no existing item changes its signature, layout or behavior. Version 0.15.0.

## Rust (`rr_dr60`)

```rust
/// The meter read-out of the last non-empty processing call (spec 016 FR-001 – FR-003).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
#[non_exhaustive]
pub struct Meters {
    pub input_peak: f32,                 // (a) linear full-scale, ≥ 0
    pub limiter_input_peak: f32,         // (b) may exceed 1.0
    pub gain_reduction_db: f32,          // (c) dB, ≥ 0
    pub hold_charge: f32,                // (d) 0 – 1, the limiter's hold capacitor
    pub record_filter_peak: f32,         // (e) stage 4, before the ADC clamp; may exceed 1.0
    pub adc_clamp_count: u32,            // (f) device samples
    pub vas_paused: bool,                // (g)
    pub hang_remaining_samples: u64,     // (h) device samples; 0 while paused
    pub reconstruction_peak: f32,        // (i) host rate, before the output chain
    pub route_peak: f32,                 // (j) before the output clamp; speaker route up to ~2.3
    pub output_clamp_count: u32,         // (k) host samples
}

impl Pipeline {
    /// A copy of the read-out. Real-time safe; never changes the engine.
    pub fn meters(&self) -> Meters;
    /// The rest state of this engine: `Meters::default()` with the configured hang.
    pub fn rest_meters(&self) -> Meters;
}
```

`Meters::default()` is the all-zero value; `Pipeline::new`, `reset` and `reconfigure` set the read-out to `rest_meters()` (data-model.md). Every peak's doc comment states its linear scale, that it may exceed 1.0, and (for the route peak) what 1.0 means per route.

Test hooks (`__test-hooks` feature, `rr_dr60::__test_hooks`):

```rust
/// Stage 4's output before the ADC clamp for host sample `x` through the one-pass's record
/// path (the front, the conversion, the band-limit), when a device sample is due; nothing
/// after it runs (research R-06).
pub fn stage4_trace(pipeline: &mut Pipeline, x: f32) -> Option<f64>;
/// The limiter's capacitor-voltage limit, V (the charge's denominator; research R-07).
pub fn limiter_v_limit(pipeline: &Pipeline) -> f64;
/// Deliberate defects for the mutation tests (research R-12); `Default` is the committed
/// behavior.
pub struct MeterMutation {
    pub filter_peak_after_clamp: bool,
    pub gain_from_state_after_update: bool,
    pub hang_off_by_one: bool,
    pub skip_clear: bool,
}
impl Pipeline {
    pub fn __with_meter_mutation(settings: Settings, m: &MeterMutation) -> Result<Self, Error>;
}
```

## C

```c
/* 56 bytes, 8-aligned. Offsets are asserted in the FFI crate. */
typedef struct RrDr60Meters {
    uint32_t struct_size;             /* 0: set by the caller to sizeof(RrDr60Meters) */
    float    input_peak;              /* 4 */
    float    limiter_input_peak;      /* 8 */
    float    gain_reduction_db;       /* 12 */
    float    hold_charge;             /* 16 */
    float    record_filter_peak;      /* 20 */
    uint32_t adc_clamp_count;         /* 24 */
    uint32_t vas_paused;              /* 28: 0 or 1 */
    uint64_t hang_remaining_samples;  /* 32 */
    float    reconstruction_peak;     /* 40 */
    float    route_peak;              /* 44 */
    uint32_t output_clamp_count;      /* 48 */
    uint32_t reserved_016;            /* 52: explicit tail padding, written 0 */
} RrDr60Meters;

/* Copies the read-out of the last non-empty processing call into *out. Real-time safe.
 * The caller sets out->struct_size; `out` must be 8-aligned and struct_size bytes writable.
 * struct_size < 56 -> RR_DR60_STATUS_INVALID_ARGUMENT, nothing written;
 * larger -> the 56 bytes filled and the bytes [56, struct_size) zeroed (no upper cap).
 * NULL handle or out -> RR_DR60_STATUS_NULL_POINTER, nothing written; a poisoned handle ->
 * RR_DR60_STATUS_INTERNAL_ERROR as for every call. */
int32_t rr_dr60_meters(RrDr60Pipeline *pipeline, RrDr60Meters *out);
```

No new status code. `rr_dr60_meters` joins the header's real-time list. `RrDr60BlockInfo`, `RrDr60RecordInfo`, `RrDr60PlayInfo` and `RrDr60Settings` are unchanged (`struct_size` 120 still). The header is regenerated with cbindgen 0.29.4; `RrDr60Meters` is referenced by the function, so no `include` entry is needed.

## Behavior shared by both

- The figures are of the last non-empty one-pass, record, play or stop-flush call; zero-length calls, a skipped flush, start/stop/erase calls leave them unchanged (data-model.md › State transitions).
- Bit-identical on every golden target and between Rust and C: the harness `ffi_parity meters` (in-process, the voiced stimulus) and the C smoke `--meters` case (the `clamp_square` stimulus, 1024-sample blocks, floats printed as `%08x` bit patterns, diffed against `examples/meters_rows.rs`).
- Reading allocates nothing and takes no lock (`alloc_free meters`).

## Golden format

`golden-meters-v1.json` follows spec 001's `contracts/golden-format.md` with the optional `meters` object per entry (data-model.md › MetersGolden: `block_len`, `blocks`, `record_blocks`, `flush`, `play_blocks`, `drain_blocks`, `digest`). The nine earlier files do not carry it and read unchanged.

## The A/B script

`scripts/ab-timing.sh <baseline> meters`: copies `crates/rr_dr60_harness/tests/meters_timing.rs` into the baseline worktree, builds both release binaries, runs them 45 times strictly alternating, and asserts `head_default / base_default − 1 ≤ ceiling` and `head_off − base_off ≤ ceiling × base_default` with `ceiling = RR_DR60_METERS_COST_CEILING` (default `0.05`; `.github/workflows/ab-timing.yml` sets `0.12`), exiting non-zero when over.
