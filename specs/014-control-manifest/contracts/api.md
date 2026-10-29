# Contract: Rust and C API (014)

Additive only: no existing item changes. Version 0.14.0.

## Rust (`rr_dr60::controls`)

```rust
pub const CONTROL_COUNT: usize = 29;
pub fn controls() -> &'static [ControlEntry];            // FR-001 order
pub fn control(identifier: &str) -> Option<&'static ControlEntry>;
pub struct ControlEntry { /* data-model.md */ }
pub enum ControlKind { DeviceControl, UnitCondition, ModelInternal, EmulatorOption, HostParameter }
pub enum ControlTiming { Live, Idle, IdleMemoryDependent, HostFixed }
pub struct ControlPaths(u8);  // RECORD, PLAYBACK, ONE_PASS; contains()
pub enum ControlValue { Switch{..}, Integer{..}, Continuous{..}, Choice{..} }
pub struct ControlOption { pub identifier: &'static str, pub value: u32 }
```

Enums are `#[non_exhaustive]` and `#[repr(u32)]` with the discriminants of data-model.md; adding a variant is a minor change.

## C

```c
/* RrDr60Status gains the variant RR_DR60_STATUS_NO_SUCH_CONTROL = 26 */
#define RR_DR60_CONTROL_PATH_RECORD   1
#define RR_DR60_CONTROL_PATH_PLAYBACK 2
#define RR_DR60_CONTROL_PATH_ONE_PASS 4

typedef struct RrDr60Control {
    const char *identifier, *name, *unit, *description;
    uint32_t kind, timing, paths, stage, value_type;   /* value_type: 0 switch, 1 integer, 2 continuous, 3 choice */
    uint32_t reserved_014;                            /* explicit padding before the u64s; offset_of asserted */
    uint64_t int_min, int_max, int_default;           /* integer; switch default in int_default (0/1) */
    float    float_min, float_max, float_default;     /* continuous */
    uint32_t option_count, choice_default, has_default; /* choice */
    uint32_t source_count, assumption_count;
} RrDr60Control;

typedef struct RrDr60ControlOption { const char *identifier; uint32_t value; } RrDr60ControlOption;

uint32_t      rr_dr60_control_count(void);
RrDr60Status  rr_dr60_control(uint32_t index, RrDr60Control *out);
RrDr60Status  rr_dr60_control_option(uint32_t index, uint32_t option, RrDr60ControlOption *out);
RrDr60Status  rr_dr60_control_source(uint32_t index, uint32_t i, const char **out);
RrDr60Status  rr_dr60_control_assumption(uint32_t index, uint32_t i, const char **out);
```

Rules: `out == NULL` → `RR_DR60_STATUS_NULL_POINTER`, checked first, as in 007; any index past its list → `NO_SUCH_CONTROL`, nothing written (unlike 007, which returns `INVALID_ARGUMENT` for an ID index: spec FR-013); strings NUL-terminated UTF-8, valid for the process; panic-guarded; no allocation. Fields of the unused value types are zero, except that a switch reports `int_min` 0, `int_max` 1 and its default in `int_default`; `has_default` is 0 only for a choice without a default (the host rate). The step is implied by `value_type` (1 for integer, 0 for continuous); there is no step field. No size field and no growth promise: both structs are on the pre-1.0 ABI-pass list (owner decision 4).
