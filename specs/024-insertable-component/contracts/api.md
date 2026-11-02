# API contract: An Insertable Component Slot and the Band Filter (spec 024, version 0.92.0)

Additive except the record readout's appended tail (plan N1, spec 022's rule). The empty slot is bit-identical to 0.91. `MAX_CHANGES_PER_CALL` 142 → **158** (the one-pass attains 146).

## Rust (`rr_dr60`)

- `components::{InsertionPoint, Component, ComponentParameter, ParameterValue, insertion_points(), components(), COMPONENT_PARAMETER_BUDGET}`; `components::band_filter::{FilterType, frequency_ceiling_hz(rate)}`.
- `InsertionPointId::BeforeLimiter` (0); `ComponentId::{None = 0, BandFilter = 1}`.
- `Pipeline::set_component(point, component) -> Result<(), Error>` (idle only; kept by `reset`/`reconfigure`), `Pipeline::component(point) -> ComponentId`, `Pipeline::component_parameter_value(point, index) -> Result<f64, Error>` (the store; the target during a transition).
- `schedule_change(Stream::Record | Stream::OnePass, input_position, 32 … 35, value, tag)`; `Stream::Playback` → `ControlNotOnStream`; out of range → `InvalidControlValue`.
- `Tap::AfterInsert`; `controls::CONTROL_COUNT` 36; `changes::MAX_CHANGES_PER_CALL` 158.
- `Error::{NoSuchInsertionPoint { point }, NoSuchComponent { component }, ComponentWhileActive}`.
- `record::FORM_VERSION` 2; `record::flags::UNKNOWN_COMPONENT`; `RecordReadout::{component, component_parameters}`.
- `__test_hooks`: `InsertMutation` (six), `insert_trace()` (the parameter trajectory, 8192 reserved).

## C (`include/rr_dr60.h`)

- Defines: `RR_DR60_INSERTION_POINT_BEFORE_LIMITER` 0, `RR_DR60_COMPONENT_NONE` 0, `RR_DR60_COMPONENT_BAND_FILTER` 1, `RR_DR60_BAND_FILTER_LOW_PASS` 0 … `_NOTCH` 3, `RR_DR60_COMPONENT_PARAMETER_BUDGET` 16, `RR_DR60_TAP_AFTER_INSERT` 11, `RR_DR60_RECORD_FLAG_UNKNOWN_COMPONENT`.
- Statuses: 41 `NO_SUCH_INSERTION_POINT`, 42 `NO_SUCH_COMPONENT`, 43 `COMPONENT_WHILE_ACTIVE`.
- `rr_dr60_set_component(p, point, component)`, `rr_dr60_get_component(p, point, *out)`, `rr_dr60_component_parameter_value(p, point, index, *out_value)`, `rr_dr60_band_filter_frequency_ceiling_hz(rate, *out_hz)`.
- `rr_dr60_insertion_point_count()`, `rr_dr60_insertion_point(index, RrDr60InsertionPoint *out)`, `rr_dr60_component_count()`, `rr_dr60_component(component, RrDr60Component *out)`, `rr_dr60_component_parameter(component, index, RrDr60ComponentParameter *out)`: three new sized structs (`struct_size` first, spec 022's rule).
- `RrDr60RecordReadout`: appended `uint32_t insert_before_limiter; uint32_t reserved_024; float component_parameters[16];` (376 → 448; a 376-byte caller receives the 0.91 prefix; the pure writer given 376 bytes writes form 2 with an empty slot).
- `rr_dr60_schedule_change` accepts controls 32 – 35 on the record and one-pass streams; `rr_dr60_control_value` reads 31 – 35; `rr_dr60_set_control_value(31, …)` sets the component, 32 – 35 `NOT_A_PLAIN_LIVE_CONTROL`.
- The promise: `scripts/abi-promise.sh` with 12a3c45, f5a0245 and 97bbdc2, byte-identical in every mode.

## Conventions stated with the slot

"An addition of the app, not a part of the original recorder. The slot sits after the microphone amplifier and before the limiter, so whatever the component does changes what the limiter and the voice-activated pause see and what the recorder stores. The band filter is one second-order section followed by a gain; a large gain or a sharp resonance drives the limiter and can keep the recorder from pausing."
