# Contract: Rust and C API (018)

Additive but for the meters struct's growth (56 → 64 bytes under spec 016's size rule: a 0.17 caller's `struct_size = 56` is refused and recompiles) and `CONTROL_COUNT` 29 → 30. Version 0.18.0.

## Rust (`rr_dr60`)

```rust
pub mod battery {
    pub const BATTERY_VOLTS_RANGE: (f32, f32) = (2.2, 3.0);
    pub const DEFAULT_BATTERY_VOLTS: f32 = 3.0;
    /// The clip-ceiling factor of the modeled law (A-056): exactly 1.0 at the default.
    pub fn ceiling_factor(volts: f32) -> f32;
    /// 20·log10(ceiling_factor(volts)).
    pub fn ceiling_db(volts: f32) -> f32;
    /// A-056's alkaline reference trajectory under the device's load: (hours, volts).
    pub const ALKALINE_TRAJECTORY: [(f32, f32); 7];
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
#define RR_DR60_BATTERY_TRAJECTORY_ROWS 7
RrDr60Status rr_dr60_set_battery_volts(RrDr60Pipeline *p, float volts);      /* INVALID_BATTERY_VOLTS = 30 */
RrDr60Status rr_dr60_get_battery_volts(const RrDr60Pipeline *p, float *out_volts);
RrDr60Status rr_dr60_battery_ceiling_factor(float volts, float *out_factor); /* INVALID_ARGUMENT outside the range */
RrDr60Status rr_dr60_battery_ceiling_db(float volts, float *out_db);
RrDr60Status rr_dr60_battery_trajectory_row(uint32_t i, float *out_hours, float *out_volts); /* INVALID_ARGUMENT at or past 7 */
/* RrDr60Meters: uint32_t amp_clip_count (offset 56), float battery_ceiling (offset 60); sizeof 64. */
```

Real-time list additions: `rr_dr60_set_battery_volts`, `rr_dr60_get_battery_volts`, the three battery functions.
