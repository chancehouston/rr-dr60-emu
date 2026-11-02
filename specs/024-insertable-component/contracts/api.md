# API contract: An Insertable Component Slot and the Band Filter (spec 024, version 0.92.0)

Additive: no existing struct changes (D9). The empty slot is bit-identical to 0.91. `MAX_CHANGES_PER_CALL` 142 → **158** (the one-pass attains 146).

## Rust (`rr_dr60`)

- `components::{InsertionPoint, Component, ComponentParameter, ParameterValue, Scale, insertion_points(), components(), COMPONENT_PARAMETER_BUDGET}` (each parameter with `scale` and `control_index`, each component with `value` and `parameter_offset`, the point with `control_index`); `components::band_filter::{FilterType, frequency_ceiling_hz(rate)}`.
- `InsertionPointId::BeforeLimiter` (0); `ComponentId::{None = 0, BandFilter = 1}`.
- `Pipeline::set_component(point, component) -> Result<(), Error>` (idle only; kept by `reset`/`reconfigure`), `Pipeline::component(point) -> ComponentId`, `Pipeline::set_component_parameter(point, index, value) -> Result<(), Error>` (idle only, `ComponentWhileActive` while recording or playing; validated like the schedule; never a change entry; the restore path), `Pipeline::component_parameter_value(point, index) -> Result<f32, Error>` (the store, global index; the target during a transition).
- `schedule_change(Stream::Record | Stream::OnePass, input_position, 32 … 35, value, tag)`; `Stream::Playback` → `ControlNotOnStream`; out of range → `InvalidControlValue`.
- `Tap::AfterInsert`; `controls::CONTROL_COUNT` 36; `changes::MAX_CHANGES_PER_CALL` 158.
- `Error::{NoSuchInsertionPoint { point }, NoSuchComponent { component }, ComponentWhileActive}`.
- `record::FORM_VERSION` 2; `record::flags::UNKNOWN_COMPONENT`; `RecordReadout::{component, store}`.
- `__test_hooks`: `InsertMutation` (`param_step`, `type_instant`, `freq_linear`, `restart_from_target`, `state_kept_on_set`, `skip_store_mirror`), `insert_trace()` (the applied parameter trajectory, 8192 reserved), `insert_input_trace()` (the slot's f64 input, reserved at construction).

## C (`include/rr_dr60.h`)

- Defines: `RR_DR60_INSERTION_POINT_BEFORE_LIMITER` 0, `RR_DR60_COMPONENT_NONE` 0, `RR_DR60_COMPONENT_BAND_FILTER` 1, `RR_DR60_BAND_FILTER_LOW_PASS` 0 … `_NOTCH` 3, `RR_DR60_COMPONENT_PARAMETER_BUDGET` 16, `RR_DR60_TAP_AFTER_INSERT` 11, `RR_DR60_RECORD_FLAG_UNKNOWN_COMPONENT`.
- Statuses: 41 `NO_SUCH_INSERTION_POINT`, 42 `NO_SUCH_COMPONENT`, 43 `COMPONENT_WHILE_ACTIVE`.
- `rr_dr60_set_component(p, point, component)`, `rr_dr60_get_component(p, point, *out)`, `rr_dr60_set_component_parameter(p, point, index, double value)` (idle only), `rr_dr60_component_parameter_value(p, point, index, float *out_value)`, `rr_dr60_band_filter_frequency_ceiling_hz(rate, double *out_hz)`.
- `rr_dr60_insertion_point_count()`, `rr_dr60_insertion_point(index, RrDr60InsertionPoint *out)`, `rr_dr60_component_count()`, `rr_dr60_component(index, RrDr60Component *out)` (index 0 … count − 1; each entry carries its value), `rr_dr60_component_parameter(component_index, index, RrDr60ComponentParameter *out)` (with `scale`, `control_index`; a choice's options through `rr_dr60_control_option` at that index).
- `RrDr60RecordComponent` (80 bytes, sized), `rr_dr60_processing_record_component(bytes, len, RrDr60RecordComponent *out)`, `rr_dr60_record_write_with_component(readout, component, out, cap, *out_len)`; `RrDr60RecordReadout` and `rr_dr60_record_write` unchanged (the latter writes form 2 with an empty slot and the defaults).
- `rr_dr60_schedule_change` accepts controls 32 – 35 on the record and one-pass streams; `rr_dr60_control_value` reads 31 – 35; `rr_dr60_set_control_value(31, …)` sets the component, 32 – 35 write the store while idle (43 while active); `rr_dr60_settings_control_value(31 … 35)` `NOT_A_SETTING`.
- The promise: `scripts/abi-promise.sh` with 12a3c45, f5a0245 and 97bbdc2, byte-identical in every mode.

## Conventions stated with the slot

"An addition of the emulator, not a part of the original recorder. The slot sits after the microphone amplifier and before the limiter, so whatever the component does changes what the limiter and the voice-activated pause see and what the recorder stores. The band filter is one second-order section followed by a gain; a large gain or a sharp resonance drives the limiter and can keep the recorder from pausing. The filter's state starts at zero with each recording; on the one-pass a component change is immediate, so an app fades or mutes around it. The filter runs at the host rate, so near the top of the recorder's band its effect differs between host rates (the guide's table)."
