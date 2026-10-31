# Contract: Rust and C API (018)

Additive: the meters struct grows 56 → 64 bytes under a compatible prefix-fill rule (a 0.17 caller's `struct_size = 56` is filled to 56 and keeps working; below 56 refused; spec 016 amended 2026-10-30), `CONTROL_COUNT` 29 → 30 with the new entry **appended at index 29** (no existing index moves; spec 014's order rule restated: later entries append). Version 0.18.0.

## Rust (`rr_dr60`)

```rust
pub mod battery {
    pub const BATTERY_VOLTS_RANGE: (f32, f32) = (2.2, 3.0);
    pub const DEFAULT_BATTERY_VOLTS: f32 = 3.0;
    /// The clip-ceiling factor of the modeled law (A-056), (volts − 1.8) ÷ 1.2: exactly 1.0 at
    /// the default; total over finite inputs (the setter refuses out-of-range values).
    pub fn ceiling_factor(volts: f32) -> f32;
    /// 20·log10(ceiling_factor(volts)).
    pub fn ceiling_db(volts: f32) -> f32;
    /// A-056's alkaline reference trajectory: (hours of recording under S-001's load, volts).
    /// Row 0 (2.96 V) is a loaded-cell reading, not the default: "fresh" is DEFAULT_BATTERY_VOLTS.
    pub const ALKALINE_TRAJECTORY: [(f32, f32); 7];
    /// A NiMH pair's steady voltage as A-056 describes it (2.4 – 2.5 V): 2.45.
    pub const NIMH_PAIR_VOLTS: f32;
}

impl Pipeline {
    /// The battery value (volts of the cell pair); Err(InvalidBatteryVolts) outside 2.2 … 3.0 or not finite.
    pub fn set_battery_volts(&mut self, volts: f32) -> Result<(), Error>;
    pub fn battery_volts(&self) -> f32;
    // schedule_change accepts the control manifest's `battery_volts` index on Playback and OnePass (output clock).
}
// Meters gains `amp_clip_count: u32` and `battery_ceiling: f32`. Error gains InvalidBatteryVolts { volts: f32 }.
```

## C (`include/rr_dr60.h`)

```c
#define RR_DR60_BATTERY_VOLTS_MIN 2.2
#define RR_DR60_BATTERY_VOLTS_MAX 3.0
#define RR_DR60_BATTERY_VOLTS_DEFAULT 3.0
#define RR_DR60_BATTERY_NIMH_PAIR_VOLTS 2.45
#define RR_DR60_BATTERY_TRAJECTORY_ROWS 7
RrDr60Status rr_dr60_set_battery_volts(RrDr60Pipeline *p, float volts);      /* INVALID_BATTERY_VOLTS = 30 */
RrDr60Status rr_dr60_get_battery_volts(const RrDr60Pipeline *p, float *out_volts);
RrDr60Status rr_dr60_battery_ceiling_factor(float volts, float *out_factor); /* INVALID_ARGUMENT for a non-finite input only (total, as Rust) */
RrDr60Status rr_dr60_battery_ceiling_db(float volts, float *out_db);
RrDr60Status rr_dr60_battery_trajectory_row(uint32_t i, float *out_hours, float *out_volts); /* INVALID_ARGUMENT at or past 7 */
/* RrDr60Meters: uint32_t amp_clip_count (offset 56), float battery_ceiling (offset 60); sizeof 64; struct_size below 56 refused, 56 … 63 filled to the size, 64 and above filled with the tail zeroed. */
/* Amended by spec 022 (2026-10-31): above RR_DR60_MAX_STRUCT_SIZE (4096) refused; struct_size reads back as the filled size (56 or 64, no longer the caller's value); 56 … 63 zeroes the bytes from 56 up to the caller's size; every padding byte is zero. */
```

Real-time list additions: `rr_dr60_set_battery_volts`, `rr_dr60_get_battery_volts`, the three battery functions.
