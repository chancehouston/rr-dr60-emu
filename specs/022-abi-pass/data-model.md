# Data Model: The Pre-1.0 C ABI Pass (spec 022)

## Sized out-struct (eighteen after the pass)

| Field | Type | Rule |
|---|---|---|
| `struct_size` | u32, first | Caller-set to `sizeof`. `< MIN` or `> RR_DR60_MAX_STRUCT_SIZE` (4096) → `INVALID_ARGUMENT`, nothing written. Read back as the size the library filled. |
| the fields | as today (+ the flattened flush in `RrDr60StopOutcome`) | Written into a zeroed value: every padding byte 0. |
| the tail beyond the filled size | | Zero up to the caller's size. |

Per struct: `SIZE = size_of::<T>()` (the build target), `SIZE_MIN` (this version's for the eleven; 56 for Meters; the 020 five's 0.20 sizes; 376 for the read-out), both pinned.

The eleven: `RrDr60BlockInfo`, `RrDr60RecordInfo`, `RrDr60StopOutcome` (flattened), `RrDr60PlayInfo`, `RrDr60EngineState`, `RrDr60MemoryStatus`, `RrDr60SpeechCodecInfo`, `RrDr60ManifestHeader`, `RrDr60StageEntry`, `RrDr60Control`, `RrDr60ControlOption`. The seven existing: `RrDr60Settings` (its stricter input rule unchanged: larger refused at `rr_dr60_create`; the cap applies), `RrDr60Meters`, `RrDr60StopFields` (frozen in layout, sized as before), `RrDr60RunData`, `RrDr60RecordReadout` (`settings` last), `RrDr60InputAlignment`, `RrDr60PlaceState`.

## Frozen struct (two)

`RrDr60VasEvent` (16: `output_position`, `input_length`), `RrDr60ChangeEntry` (40: `position`, `before`, `after`, `tag`, `control`, `stream` — the former `reserved`). Layouts promised never to change; sizes published as `RR_DR60_VAS_EVENT_SIZE` and `RR_DR60_CHANGE_ENTRY_SIZE`; a new form is a new struct with new functions. The single-element writers and the record writer's input arrays use them unchanged.

## The ABI check

```
RR_DR60_ABI_VERSION = 1
rr_dr60_abi_version() -> u32
rr_dr60_abi_check(abi_version, pointer_size, settings_size, vas_event_size, change_entry_size) -> OK | ABI_MISMATCH (37)
  OK iff abi_version == 1 && pointer_size == 8 && settings_size <= sizeof(RrDr60Settings) && vas_event_size == 16 && change_entry_size == 40
```

What moves the version: a frozen struct's bytes; a function's signature or removal; an enum value's meaning; a field reorder; the settings' input rule; a shrinking minimum (never). What does not: appended statuses, controls, taps, functions, defines; a field appended to a sized struct.

## The per-call report state machine

```
out_info NULL ──────────────────────────────────────────────► as 0.21 (nothing written; the call proceeds)
out_info non-NULL, handle NULL ──size valid──► written (produced 0, paused false), status NULL_POINTER
                                ──size bad────► nothing written, status NULL_POINTER
handle live, size bad ──────────────────────► INVALID_ARGUMENT, nothing written, nothing processed
handle live, size valid, later error ───────► written as 0.21's idle report (tail zeroed), the error's status
handle live, size valid, success ───────────► written (fields, padding 0, size read back, tail zeroed), OK
```

## Swift

`RRDR60Sized` (protocol; `.sized` sets `struct_size = MemoryLayout<Self>.size`) on the eighteen; `RRDR60.checkABI() throws` and the lazy `abiStatus`; `RRDR60Error.abiMismatch(library:header:)`.
