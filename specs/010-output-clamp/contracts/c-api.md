# Contract: C API additions (feature 010; research R-03, R-09)

Rules of spec 001's contract apply (opaque handle, status codes, caller buffers, no panic across the boundary, the struct grows only at the end). The header is regenerated with cbindgen and drift-checked.

## `RrDr60Settings`: 104 → 108 bytes

| Offset | Field | Type | Notes |
|---|---|---|---|
| 0 – 98 | as 0.9.0 | | unchanged (`output_earphone_enabled` at 98) |
| 99 – 103 | `reserved_010` | `uint8_t[5]` | the 0.9 layout's tail padding made explicit (write 0; ignored): a `bool` appended alone would sit at 99 and keep the size at 104, letting a 0.9 caller's `struct_size = 104` pass and byte 99 be read as the option from uninitialized memory (spec FR-008; coach C3) |
| **104** | `output_clamp_enabled` | `bool` | default `false`. The output clamp: the route-output sample held within [−1.0, +1.0] (FR-001); an emulator convenience, not a device behavior |
| 105 – 107 | `reserved_010_tail` | `uint8_t[3]` | this layout's tail, explicit (write 0; ignored) so that 0.11 can append at 108 and refuse 108 |

`struct_size = 104` is refused as `RR_DR60_SETTING_FIELD_STRUCT_SIZE` (a compile-and-run break like 0.5, 0.6, 0.8 and 0.9; `rr_dr60_settings_default` fills 108). No new `RrDr60SettingField`, status, tap, enum or function.

## Behaviour

`rr_dr60_process*`, `rr_dr60_play*`: with the option on, no output sample is outside [−1.0, +1.0] at `RR_DR60_TAP_ROUTE_OUTPUT`; the other taps are unchanged. `rr_dr60_settings_validate` accepts either value.

## Smoke loop (`tests/c/smoke.c`)

Two new cases: a ±1.0 square wave (exactly representable; a half-period of 24 samples at 48 kHz) recorded and played through the speaker route and through the earphone route at position 1 with `output_clamp_enabled = true`, each playback's FNV-1a 64 hash compared with the Rust path's pinned reference (`smoke_reference.rs` gains `RR_DR60_SMOKE_SPEAKER_CLAMP_HASH` and `RR_DR60_SMOKE_EARPHONE_CLAMP_HASH`) and every sample checked to lie within [−1.0, +1.0] in C; the same square with the option off on the speaker route checked to exceed 1.0 (so the loop shows the option acting); `struct_size = 104` refused with `RR_DR60_STATUS_INVALID_SETTINGS` naming `STRUCT_SIZE`.

## Layout tests

`size_of::<RrDr60Settings>() == 108`; `offset_of!(reserved_010) == 99`, `offset_of!(output_clamp_enabled) == 104`, `offset_of!(reserved_010_tail) == 105`; `struct_size = 104` refused; the default's new field `false` and the reserved bytes zero; `rr_dr60_version()` reads `0.10.0`. `ffi_parity::c_api_output_clamp_matches_rust_at_all_rates`: the five clamp configurations through C and Rust, bit-equal, at the seven rates.
