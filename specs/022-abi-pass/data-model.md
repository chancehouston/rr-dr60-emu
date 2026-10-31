# Data Model: The Pre-1.0 C ABI Pass (spec 022)

## Out-struct (fifteen: filled by the library through `write_sized`)

| Field | Type | Rule |
|---|---|---|
| `struct_size` | u32, first | Caller-set to `sizeof`. Below the first published layout or above `RR_DR60_MAX_STRUCT_SIZE` (4096) → `INVALID_ARGUMENT`, nothing written. Read back as the size the library filled (the largest published layout ≤ the caller's). |
| the fields | as today (+ the flattened flush in `RrDr60StopOutcome`) | Assigned one by one into a zeroed buffer: every padding byte 0. |
| the tail beyond the filled size | | Zero up to the caller's size. |

Per struct: `SIZE = size_of::<T>()` (private pin) and `PUBLISHED: &[u32]` ascending, ending in `SIZE` (const-asserted). The eleven new: `RrDr60BlockInfo`, `RrDr60RecordInfo`, `RrDr60StopOutcome` (flattened), `RrDr60PlayInfo`, `RrDr60EngineState`, `RrDr60MemoryStatus`, `RrDr60SpeechCodecInfo`, `RrDr60ManifestHeader`, `RrDr60StageEntry`, `RrDr60Control`, `RrDr60ControlOption` (`[SIZE]` each). The four existing: `RrDr60Meters` (`[56, 64]`), `RrDr60RecordReadout` (`[376]`, `settings` last; in a prefix fill the embedded `settings.struct_size` = the settings bytes the prefix holds), `RrDr60InputAlignment` (`[32]`), `RrDr60PlaceState` (`[32]`). `Default` = the zeroed value + `SIZE`, for constructions only; a source scan forbids `derive(Default)` and whole-struct writes.

## Input struct (three: read by the library)

- `RrDr60Settings` (120): `struct_size != sizeof` refused at `rr_dr60_create` / `reconfigure` / `settings_validate` (D13; before 0.22 only smaller was refused). The one exception to the growth promise.
- `RrDr60RunData` (80): `< 48` or `> 4096` refused on read.
- `RrDr60StopFields` (40): frozen; written as 40 by the library; ignored on read.

## Frozen struct (two)

`RrDr60VasEvent` (16: `output_position`, `input_length`), `RrDr60ChangeEntry` (40: `position`, `before`, `after`, `tag`, `control`, `stream` — the former `reserved`). Frozen: a different layout would be a new type with new functions, never a change to these. Sizes published as `RR_DR60_VAS_EVENT_SIZE` and `RR_DR60_CHANGE_ENTRY_SIZE` (literal defines, const-asserted). The single-element writers and the record writer's input arrays use them unchanged.

## The ABI check

```
RR_DR60_ABI_VERSION = 1
rr_dr60_abi_version() -> u32
rr_dr60_abi_check(abi_version, pointer_size, settings_size, vas_event_size, change_entry_size) -> OK | ABI_MISMATCH (37)
  OK iff abi_version == 1 && pointer_size == 8 && settings_size == sizeof(RrDr60Settings) && vas_event_size == 16 && change_entry_size == 40
```

What moves the version: a frozen struct's bytes; a function's signature or removal; an enum value's meaning; a field reorder; a settings growth or a change of the settings input rule. What does not: appended statuses, controls, taps, functions, defines; a field appended to an out-struct. A published list never loses an entry.

## The per-call report state machine

```
out NULL ─────────────────────────────────────────────► as 0.21 (nothing written; the call proceeds)
out non-NULL, handle NULL     ──size valid──► idle report written, status NULL_POINTER
                              ──size bad────► nothing written, status NULL_POINTER
out non-NULL, handle poisoned ──size valid──► idle report written, status INTERNAL_ERROR
                              ──size bad────► nothing written, status INTERNAL_ERROR
handle live, size bad ────────────────────────► INVALID_ARGUMENT, nothing written, nothing processed
handle live, size valid, later error ─────────► idle report written (tail zeroed), the error's status
handle live, size valid, success ─────────────► written (fields, padding 0, size read back, tail zeroed), OK
```

The handle-taking status functions (`get_state`, `memory_status`, `meters`): `live`, then NULL out, then the size. The constant-data functions: NULL, size, index.

## Swift

`RRDR60Sized` (`.sized` sets `struct_size = MemoryLayout<Self>.size`) on the seventeen (never `RrDr60Settings`); `RRDR60.checkABI() throws`, the lazy `abiStatus`; `RRDR60Error.abiMismatch(reason: ABIMismatch)` with `version(library:header:)`, `pointerSize`, `settingsSize`, `vasEventSize`, `changeEntrySize`.
