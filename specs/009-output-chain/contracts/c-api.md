# Contract: C API additions (feature 009; research R-06, R-07)

Rules of spec 001's contract apply (opaque handle, status codes, caller buffers, no panic across the boundary, the struct grows only at the end). The header is regenerated with cbindgen and drift-checked.

## `RrDr60Settings`: 96 → 104 bytes

| Offset | Field | Type | Notes |
|---|---|---|---|
| 0 – 88 | as 0.8.0 | | unchanged (`speech_codec_enabled` at 88; 89 – 95 remain tail padding and stay reserved: fields placed there would let a 0.8 caller's `struct_size = 96` pass with uninitialized bytes) |
| **96** | `output_volume_amp_enabled` | `bool` | default `true` (stage 11) |
| **97** | `output_speaker_enabled` | `bool` | default `true` (stage 12) |
| **98** | `output_earphone_enabled` | `bool` | default `true` (stage 13) |
| 99 – 103 | (tail padding) | | |

`struct_size` 96 is refused as `RR_DR60_SETTING_FIELD_STRUCT_SIZE` (a compile-and-run break like 0.5, 0.6 and 0.8; `rr_dr60_settings_default` fills 104 and sets `tap = RR_DR60_TAP_ROUTE_OUTPUT`). No new `RrDr60SettingField` (bools cannot be invalid).

## Enums, statuses, constant

- `RrDr60Tap`: `RR_DR60_TAP_ROUTE_OUTPUT = 5` (the new default), `RR_DR60_TAP_AFTER_VOLUME_AMP = 6`, `RR_DR60_TAP_AFTER_SPEAKER = 7`, `RR_DR60_TAP_AFTER_EARPHONE = 8` (appended; `RR_DR60_TAP_AFTER_PLAYBACK = 1` keeps its 0.8 meaning).
- `RrDr60Status`: `RR_DR60_STATUS_INVALID_VOLUME = 23`, `RR_DR60_STATUS_TAP_NOT_ON_ROUTE = 24` (appended).
- `RR_DR60_VOLUME_REFERENCE` (`float`, ≈ 0.0976f): the default volume position (A-043).

## Functions

| Function | Contract |
|---|---|
| `RrDr60Status rr_dr60_set_volume(RrDr60Pipeline*, float position)` | `INVALID_VOLUME` for a non-finite or out-of-range position (unchanged); real-time safe; any state, during playback too |
| `RrDr60Status rr_dr60_get_volume(const RrDr60Pipeline*, float* out_position)` | writes the position; `INVALID_ARGUMENT` for a NULL out pointer; real-time safe |
| `rr_dr60_process*` | return `TAP_NOT_ON_ROUTE` (nothing processed, `produced` 0) when the tap is a named route tap of the other route |

Both functions join the real-time-safe list in `cbindgen.toml`'s header comment.

## Smoke loop (`tests/c/smoke.c`)

Plays the exactly representable ±0.125 square wave through the speaker route at `RR_DR60_VOLUME_REFERENCE` and at 1.0 (the clipper engaged) and through the earphone route at both, comparing each playback's FNV-1a hash with the Rust path's pinned reference (`smoke_reference.rs` gains the four cases); exercises `rr_dr60_set_volume` with 1.5 and NaN (refused) and the route-tap refusal through `rr_dr60_process`.

## Layout tests

`size_of::<RrDr60Settings>() == 104`; the three offsets 96 – 98; `struct_size = 96` refused; every tap value 0 – 8 converts; the two statuses' values; `RR_DR60_VOLUME_REFERENCE` equals `rr_dr60::output::VOLUME_REFERENCE`.
