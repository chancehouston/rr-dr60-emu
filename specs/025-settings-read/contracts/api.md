# API contract: The Settings Read and Control Access by Index (spec 025, version 0.91.0)

Additive. No audio change; no struct, constant or existing function changes; the reading stays out of the frozen `--abi` rows.

## Rust (`rr_dr60`)

- `control_map::ControlReading { Switch(bool), Integer(u64), Continuous(f32), Choice(u32) }`.
- `control_map::settings_value(&Settings, index: u32) -> Result<ControlReading, Error>`; `control_map::set_settings_value(&mut Settings, index: u32, ControlReading) -> Result<(), Error>`.
- `control_map::setting_control(error::Setting) -> u32` (the validator's field to the control index).
- `Pipeline::control_value(index) -> Result<ControlReading, Error>`; `Pipeline::set_control_value(index, ControlReading) -> Result<(), Error>`.
- `Error::NotASetting { control }`, `Error::NotAPlainLiveControl { control }`; representability `Error::InvalidControlValue { control, value }` (NaN for a wrong-type reading).

## C (`include/rr_dr60.h`)

- `RrDr60Status rr_dr60_get_settings(RrDr60Pipeline *, RrDr60Settings *out)`: `out->struct_size` must equal `sizeof(RrDr60Settings)`.
- `RrDr60Status rr_dr60_control_value(RrDr60Pipeline *, uint32_t index, RrDr60ControlReading *out)`.
- `RrDr60Status rr_dr60_settings_control_value(const RrDr60Settings *, uint32_t index, RrDr60ControlReading *out)`.
- `RrDr60Status rr_dr60_set_settings_control_value(RrDr60Settings *, uint32_t index, const RrDr60ControlReading *)`.
- `RrDr60Status rr_dr60_set_control_value(RrDr60Pipeline *, uint32_t index, const RrDr60ControlReading *)`.
- `RrDr60Status rr_dr60_setting_field_control(uint32_t field, uint32_t *out_index)`.
- `RrDr60ControlReading` (24 bytes: `uint32_t struct_size`, `uint32_t value_type`, `uint64_t int_value`, `float float_value`, `uint32_t reserved`).
- Statuses 39 `RR_DR60_STATUS_NOT_A_SETTING`, 40 `RR_DR60_STATUS_NOT_A_PLAIN_LIVE_CONTROL`.
- Reachable setter statuses through `rr_dr60_set_control_value`: `INVALID_VOLUME`, `INVALID_SPEED_LEVEL`, `INVALID_BATTERY_VOLTS`; every unrepresentable reading `INVALID_ARGUMENT`.

## Swift (`RRDR60`)

- `ControlReading` (four cases; `Equatable`, `Sendable`), `init?(defaultOf:)`, `ControlIndex` (`init?(identifier:)`), `RrDr60ControlReading.sized`.
- Throwing wrappers: `controlValue`, `setControlValue`, `appliedSettings`, `RrDr60Settings.value(index:)` / `.setValue(index:_:)`, `controlIndex(forField:)`; `RRDR60Error.status(Int32)`.
