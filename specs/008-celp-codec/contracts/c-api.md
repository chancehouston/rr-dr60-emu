# Contract: C API additions (feature 008; research R-08)

Rules of spec 001's contract apply (opaque handle, status codes, caller buffers, no panic across the boundary, the struct grows only at the end). The header is regenerated with cbindgen and drift-checked.

## `RrDr60Settings`: 88 → 96 bytes

| Offset | Field | Type | Notes |
|---|---|---|---|
| 0 – 83 | as 0.7.0 | | unchanged |
| 84 | `reserved_008` | `u32` | ignored; write 0 (fills the 0.7 tail padding so the next field sits at 88; `repr(C)` would otherwise place a `bool` at 84 and keep the size at 88) |
| **88** | `speech_codec_enabled` | `bool` | default `true` |
| 89 – 95 | (tail padding) | | |

`struct_size` 88 is refused as `RR_DR60_SETTING_FIELD_STRUCT_SIZE` (a compile-and-run break like 0.5 and 0.6; `rr_dr60_settings_default` fills 96). No new `RrDr60SettingField` (a bool cannot be invalid).

## Enums and statuses

- `RrDr60Tap`: `RR_DR60_TAP_AFTER_DECODER = 4` (appended).
- `RrDr60Encoding`: `RR_DR60_ENCODING_FRAMES = 1`, `RR_DR60_ENCODING_SAMPLES = 2` (the form's field values).
- `RrDr60Status`: `RR_DR60_STATUS_ENCODING_DIFFERS = 21`, `RR_DR60_STATUS_UNSUPPORTED_CODEC_MODEL_VERSION = 22` (appended; 17 keeps its code with the broadened meaning).

## Report structs (layout changes recorded in the CHANGELOG)

| Struct | New fields (appended) | Size on 64-bit |
|---|---|---|
| `RrDr60BlockInfo` | `frames: usize` | 24 → 32 |
| `RrDr60RecordInfo` | `frames: usize`, `frames_total: u64` | 24 → 40 (the plan's "32 → 48" miscounted the 0.6 size; corrected at T043) |
| `RrDr60StopOutcome` | `silence_fed_samples: u64` | 40 → 64 (`flush` grows by 16, the new field adds 8) |
| `RrDr60PlayInfo` | `frames: usize`, `frame_index: u64` | 16 → 32 |
| `RrDr60SpeechCodecInfo` (new) | `frame_samples, frame_ms, bits_per_frame, delay_samples: u32; model_version[3]: u16; reserved: u16` | 24 |

Each layout is const-asserted against the Rust type.

## Functions

| Function | Contract |
|---|---|
| `RrDr60Status rr_dr60_speech_codec_info(RrDr60SpeechCodecInfo* out)` | handle-less; constants; `NULL` → `NULL_POINTER`; real-time safe |
| `RrDr60Status rr_dr60_get_encoding(RrDr60Pipeline* p, uint32_t* out)` | the engine's configured encoding as an `RrDr60Encoding` value; takes the handle as every other read does (a mutable handle, the library's single-pointer convention); real-time safe |
| existing record / play / stop / export / import | the new fields and statuses; `rr_dr60_import` returns 21 / 22 for the new refusals and 15 for a version 1 sequence |

## Smoke test (`tests/c/smoke.c`)

Record the exactly representable ±0.125 square wave through the default settings (codec on), read `frames_total`, stop and read `silence_fed_samples`, export, import into a second handle (same encoding), play both, `memcmp` the frame bytes (from the exported form) and the playback output with the Rust harness's reference values; then a bypassed-codec handle importing the frames must get 21.
