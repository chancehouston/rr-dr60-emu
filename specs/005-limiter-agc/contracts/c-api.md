# Contract: C API changes (feature 005)

Breaking (0.4 → 0.5). Rules 1–4 of spec 003 (panics never cross; buffers written only on success except `out_info`; `struct_size` read first; the struct only grows at the end) still hold.

## `RrDr60Settings` (72 → 80 bytes)

| Offset | Field | Type | Change |
|---|---|---|---|
| 0 | `struct_size` | u32 | now 80 |
| 4 | `host_rate_hz` | u32 | — |
| 8 | `record_stage_enabled` | bool | — |
| 9 | `playback_stage_enabled` | bool | — |
| 12 | `tap` | u32 | `RR_DR60_TAP_AFTER_AGC` now means the host-rate limiter output, latency 0 |
| 16 | `seed` | u64 | — |
| 24 | `agc_enabled` | bool | — |
| 28 | `agc_knee_dbfs` | float | **renamed** from `agc_target_dbfs`; default 1.0; range −40 … 3 |
| 32 | `reserved_agc_1` | float | **renamed** from `agc_max_gain_db`; ignored; `rr_dr60_settings_default` writes 0 |
| 36 | `reserved_agc_2` | float | **renamed** from `agc_max_attenuation_db`; ignored; default 0 |
| 40 | `agc_attack_ms` | float | **new meaning**: dead time at 10 dB over the knee; default 3.0; range 1 … 50 |
| 44 | `agc_release_ms` | float | **new meaning**: time to −1 dB from 10 dB of attenuation; default 1370.0; range 200 … 10 000 |
| 48 | `vas_enabled` | bool | — |
| 52 | `vas_mode` | u32 | — |
| 56 | `vas_sensitivity` | u32 | — |
| 60 | `vas_threshold_dbfs` | float | default now −40.0 |
| 64 | `vas_hang_ms` | float | — |
| 68 | `vas_onset_ms` | float | — |
| 72 | `adc_clamp_enabled` | bool | **appended**; default true |
| 80 | (end, 8-byte alignment) | | `const _: () = assert!(size_of::<RrDr60Settings>() == 80)` |

(Offsets above are the plan's expectation from the 0.3 layout note "VAS fields at 48–68"; the implementation asserts the real ones in a unit test and the header comment states them.) A struct with `struct_size` 72 is rejected as `RR_DR60_STATUS_INVALID_ARGUMENT` / `RR_DR60_SETTING_FIELD_STRUCT_SIZE` after reading only `struct_size`, as 003 did for the 0.2 layout.

## `RrDr60SettingField`

| Value | Name | Change |
|---|---|---|
| 4 | `RR_DR60_SETTING_FIELD_AGC_KNEE_DBFS` | renamed from `…AGC_TARGET_DBFS` |
| 5, 6 | (retired) | never returned; the header comments say "retired in 0.5" |
| 7 | `…AGC_ATTACK_MS` | unchanged name |
| 8 | `…AGC_RELEASE_MS` | unchanged name |

## Functions

Unchanged signatures. `rr_dr60_settings_default` fills the new defaults; `rr_dr60_settings_validate` ignores the reserved fields; `rr_dr60_latency_samples` returns 0 with the "after AGC" tap; `rr_dr60_version_string` = "0.5.0". Output is bit-identical to the Rust API at every host rate (FR-015).

## Smoke test additions (`tests/c/smoke.c`)

- `struct_size == 80`; defaults: `agc_knee_dbfs == 1.0f`, `agc_attack_ms == 3.0f`, `agc_release_ms == 1370.0f`, `adc_clamp_enabled`, `vas_threshold_dbfs == -40.0f`, reserved fields 0.
- A −20 dBFS 1 kHz tone through the AGC-isolated configuration (tap after AGC) is returned bit-identical (memcmp) with latency 0.
- A +11 dBFS tone through the default record path at 6 kHz has max |y| == 1.0f exactly; with `adc_clamp_enabled = false` it exceeds 1.0f.
- `agc_knee_dbfs = 4.0f` → `RR_DR60_STATUS_INVALID_SETTING` naming `RR_DR60_SETTING_FIELD_AGC_KNEE_DBFS`; a struct with `struct_size = 72` → `STRUCT_SIZE`.
- The existing 48 kHz checks keep their meaning; the 1 kHz −20 dBFS level check now expects unity through the default pipeline within the template loss (±0.2 dB, A-015).
