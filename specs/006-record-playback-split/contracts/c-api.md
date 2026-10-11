# Contract: C API changes (feature 006)

Breaking (0.5 → 0.6). Rules 1–5 of spec 003 hold; rule 2's `out_info` exception extends to `rr_dr60_record`, `rr_dr60_stop_recording` and `rr_dr60_play` (their info struct is always written when not NULL). Status codes and field numbers are only appended.

## `RrDr60Settings` (80 → 88 bytes)

| Offset | Field | Type | Change |
|---|---|---|---|
| 0 … 72 | as 0.5 | | unchanged (`adc_clamp_enabled` at 72) |
| 76 | `memory_capacity_s` | u32 | **appended**; default 3600; range 1 … 86 400 |
| 80 | `memory_max_recordings` | u32 | **appended**; default 99; range 1 … 9 999 |
| 88 | (end, 8-byte alignment) | | `const _: () = assert!(size_of::<RrDr60Settings>() == 88)` |

A struct with `struct_size` 80 is rejected as `RR_DR60_STATUS_INVALID_ARGUMENT` / `RR_DR60_SETTING_FIELD_STRUCT_SIZE` after reading only `struct_size`.

## `RrDr60SettingField`

| Value | Name |
|---|---|
| 14 | `RR_DR60_SETTING_FIELD_MEMORY_CAPACITY_S` |
| 15 | `RR_DR60_SETTING_FIELD_MEMORY_MAX_RECORDINGS` |

## `RrDr60Status` (appended)

| Value | Name | Rust error |
|---|---|---|
| 6 | `RECORDING_IN_PROGRESS` | `RecordingInProgress` |
| 7 | `PLAYBACK_IN_PROGRESS` | `PlaybackInProgress` |
| 8 | `NOT_RECORDING` | `NotRecording` |
| 9 | `NOT_PLAYING` | `NotPlaying` |
| 10 | `MEMORY_FULL` | `MemoryFull` |
| 11 | `NO_RECORDING_SLOTS` | `NoRecordingSlots` |
| 12 | `NO_SUCH_RECORDING` | `NoSuchRecording` |
| 13 | `DOES_NOT_FIT` | `DoesNotFit` |
| 14 | `UNKNOWN_FORM` | `UnknownForm` |
| 15 | `UNSUPPORTED_FORM_VERSION` | `UnsupportedFormVersion` |
| 16 | `DAMAGED` | `Damaged` |
| 17 | `STORED_EXCEEDS_CAPACITY` | `StoredExceedsCapacity` |
| 18 | `ALLOCATION_FAILED` | `AllocationFailed` |
| 19 | `BUFFER_TOO_SMALL` | `BufferTooSmall` |

## New enums and structs (all `repr(C)` / `repr(u32)`)

```c
typedef enum { RR_DR60_ROUTE_SPEAKER = 0, RR_DR60_ROUTE_EARPHONE = 1 } RrDr60Route;
typedef enum { RR_DR60_STATE_IDLE = 0, RR_DR60_STATE_RECORDING = 1, RR_DR60_STATE_PLAYING = 2 } RrDr60State;
typedef enum { RR_DR60_STOP_REASON_NONE = 0, RR_DR60_STOP_REASON_MEMORY_FULL = 1 } RrDr60StopReason;
typedef struct { uint32_t state; uint32_t route; uint32_t current_recording; /* 0 = none */ bool speaker_muted; } RrDr60EngineState;
typedef struct { size_t stored; size_t events; bool paused; bool stopped; uint32_t stop_reason; } RrDr60RecordInfo;
typedef struct { RrDr60RecordInfo flush; bool kept; uint32_t number; uint64_t length_samples; } RrDr60StopOutcome;
typedef struct { size_t produced; bool finished; } RrDr60PlayInfo;
typedef struct { uint64_t capacity_samples; uint64_t used_samples; uint64_t remaining_samples; uint32_t max_recordings; uint32_t count; uint32_t slots_remaining; } RrDr60MemoryStatus;
```

## New functions (all return `RrDr60Status` unless noted; NULL handle → `NULL_POINTER`; poisoned → `INTERNAL_ERROR`)

| Function | Notes |
|---|---|
| `rr_dr60_start_recording(p)` | |
| `rr_dr60_record(p, input, frames, out_info, events, events_capacity)` | `out_info` always written when not NULL; `events` may be NULL only with capacity 0; in-place has no meaning (no output) |
| `rr_dr60_max_record_events(p, frames, out_capacity)` | |
| `rr_dr60_stop_recording(p, out_outcome, events, events_capacity)` | `out_outcome` always written when not NULL |
| `rr_dr60_start_playback(p, number)` | |
| `rr_dr60_play(p, output, frames, out_info)` | `out_info` always written when not NULL; `frames == 0` is OK |
| `rr_dr60_stop_playback(p)` | |
| `rr_dr60_set_route(p, route)` / `rr_dr60_get_route(p, out_route)` | an invalid route value → `INVALID_ARGUMENT` |
| `rr_dr60_get_state(p, out_state)` | |
| `rr_dr60_memory_status(p, out_status)` | |
| `rr_dr60_recording_length(p, number, out_samples)` | |
| `rr_dr60_erase(p, number)` / `rr_dr60_erase_all(p)` | |
| `rr_dr60_export_len(p, number, out_len)` | |
| `rr_dr60_export(p, number, out_bytes, capacity, out_written)` | `BUFFER_TOO_SMALL` leaves `out_bytes` untouched and writes the needed length to `out_written` if not NULL |
| `rr_dr60_import(p, bytes, len, out_number)` | |
| `rr_dr60_record_latency_samples(p, out)` / `rr_dr60_playback_latency_samples(p, out)` / `rr_dr60_drain_samples(p, out)` | host samples |
| `rr_dr60_footprint_bytes(p, out)` | |

Real-time safe (header comment updated): `rr_dr60_start_recording`, `_record`, `_max_record_events`, `_stop_recording`, `_start_playback`, `_play`, `_stop_playback`, `_set_route`, `_get_route`, `_get_state`, `_memory_status`, `_recording_length`, `_export_len`, the three latency calls, `_footprint_bytes`, plus the 0.5 list. Not real-time safe: `rr_dr60_erase`, `_erase_all` (bounded memmove), `_export`, `_import` (CRC over the recording), plus `_create`, `_destroy`, `_reconfigure`.

`rr_dr60_reconfigure` returns `RECORDING_IN_PROGRESS`, `PLAYBACK_IN_PROGRESS` or `STORED_EXCEEDS_CAPACITY` as the Rust API does and otherwise keeps the memory. `rr_dr60_reset` keeps the memory and the route. `rr_dr60_process*` return the in-progress statuses while a path is active. `rr_dr60_version_string` = "0.6.0".

## Smoke test additions (`tests/c/smoke.c`)

- `struct_size == 88`; defaults `memory_capacity_s == 3600`, `memory_max_recordings == 99`; a struct with `struct_size = 80` → `STRUCT_SIZE`; `memory_capacity_s = 0` → `INVALID_SETTING` naming field 14.
- The loop at 48 kHz: start recording, record a −20 dBFS 1 kHz tone of 1 s in blocks of 480 with events, stop (`kept`, number 1, length 6 000 + the flush), state checks at each step (`speaker_muted` only while recording), `memory_status` (count 1), export into a buffer sized by `export_len`, create a second handle, import (number 1), play both to `finished`, `memcmp` the outputs with each other and with the Rust harness's reference bytes for the same stimulus (through `ffi_parity`); a refused `start_playback` while recording → `RECORDING_IN_PROGRESS`; `erase_all`; count 0.
