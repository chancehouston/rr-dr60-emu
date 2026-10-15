# Contract: C API additions (feature 010; research R-03, R-09)

Rules of spec 001's contract apply (opaque handle, status codes, caller buffers, no panic across the boundary, the struct grows only at the end). The header is regenerated with cbindgen and drift-checked.

## `RrDr60Settings`: 104 → 112 bytes

The struct holds `seed: uint64_t`, so its alignment is 8 on every target; the size is a multiple of 8.

| Offset | Field | Type | Notes |
|---|---|---|---|
| 0 – 98 | as 0.9.0 | | unchanged (`output_earphone_enabled` at 98) |
| 99 – 103 | `reserved_010` | `uint8_t[5]` | the 0.9 layout's tail padding made explicit (write 0; ignored): a `bool` appended alone would sit at 99 and keep the size at 104, letting a 0.9 caller's `struct_size = 104` pass and byte 99 be read as the option from uninitialized memory (spec FR-008; coach C3) |
| **104** | `output_clamp_enabled` | `bool` | default `false`. The output clamp: the route-output sample held within [−1.0, +1.0] (FR-001); an emulator convenience, not a device behavior; the play calls always return the route output, so it always applies to them |
| 105 – 111 | `reserved_010_tail` | `uint8_t[7]` | this layout's tail, explicit (write 0; ignored) so that 0.11 can append at 112 and refuse 112 (a 3-byte tail would leave 108 – 111 as implicit padding and repeat the problem; coach P1, hardware D1) |

`struct_size = 104` is refused: by `rr_dr60_create` / `rr_dr60_reconfigure` with `RR_DR60_STATUS_INVALID_ARGUMENT` (`check_struct_size`), by `rr_dr60_settings_validate` naming `RR_DR60_SETTING_FIELD_STRUCT_SIZE` (a compile-and-run break like 0.5, 0.6, 0.8 and 0.9; `rr_dr60_settings_default` fills 112). `RR_DR60_VERSION_MINOR = 10`. No new `RrDr60SettingField`, status, tap, enum or function.

## Documentation in the header

`RrDr60Tap`: `RR_DR60_TAP_ROUTE_OUTPUT` "with `output_clamp_enabled`, the only tap that is bounded; the selected route's last per-stage tap then differs from it wherever a sample was bounded"; `AFTER_VOLUME_AMP`, `AFTER_SPEAKER`, `AFTER_EARPHONE`, `AFTER_PLAYBACK` "never bounded by the output clamp (the one-pass at this tap)". `output_clamp_enabled`: the sentences above plus "off by default; an emulator convenience, not a device behavior; `+1.0` maps to 32767 in a 16-bit conversion".

## Behaviour

`rr_dr60_process*` at `RR_DR60_TAP_ROUTE_OUTPUT` and every `rr_dr60_play*` call: with the option on, no output sample is outside [−1.0, +1.0]; the other taps are unchanged. `rr_dr60_settings_validate` accepts either value.

## Smoke loop (`tests/c/smoke.c`)

- Two new cases: a ±1.0 square wave (exactly representable; a half-period of 24 samples at 48 kHz) recorded and played through the speaker route and through the earphone route at position 1 with `output_clamp_enabled = true`, each playback's FNV-1a 64 hash compared with the Rust path's pinned reference and every sample checked in C to lie within [−1.0, +1.0].
- The same square with the option off on the speaker route checked to exceed 1.0 (the loop shows the option acting).
- `struct_size = 104` refused by `rr_dr60_create` with `RR_DR60_STATUS_INVALID_ARGUMENT`, and by `rr_dr60_settings_validate` with field `RR_DR60_SETTING_FIELD_STRUCT_SIZE`.

The references: `SMOKE_SPEAKER_CLAMP_HASH` and `SMOKE_EARPHONE_CLAMP_HASH` in `crates/rr_dr60_harness/src/smoke_reference.rs` (computed by the Rust path on the same exactly representable square; pinned by `ffi_parity::smoke_reference_hash_is_pinned`), mirrored as the `RR_DR60_SMOKE_SPEAKER_CLAMP_HASH` / `RR_DR60_SMOKE_EARPHONE_CLAMP_HASH` defines in `smoke.c`.

## Harness mapping

`configs::c_settings` maps `output.clamp_enabled` → `output_clamp_enabled`; the `c_default_*` twins keep it `false`.

## Layout tests

`const` assertions: `size_of::<RrDr60Settings>() == 112`, `offset_of!(reserved_010_tail) + 7 == size_of`, `align_of == 8`. Unit tests: `offset_of!(reserved_010) == 99`, `offset_of!(output_clamp_enabled) == 104`, `offset_of!(reserved_010_tail) == 105`; `struct_size = 104` refused on create, reconfigure and validate; the default's new field `false` and the reserved bytes zero; `rr_dr60_version()` reads `0.10.0` and `RR_DR60_VERSION_MINOR == 10`. `ffi_parity::c_api_output_clamp_matches_rust_at_all_rates`: the six clamp configurations through C and Rust, bit-equal, at the seven rates.
