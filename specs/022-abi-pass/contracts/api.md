# API contract: The Pre-1.0 C ABI Pass (spec 022, version 0.22.0)

**Breaking for every C caller compiled against 0.21 or earlier: rebuild once.** The `rr_dr60` core crate's Rust API is unchanged.

## C (`include/rr_dr60.h`)

- The eleven out-structs gain `uint32_t struct_size` as their first field: `RrDr60BlockInfo` (40), `RrDr60RecordInfo` (48), `RrDr60StopOutcome` (88, the flush's seven fields inline: `flush_stored`, `flush_events`, `flush_paused`, `flush_stopped`, `flush_stop_reason`, `flush_frames`, `flush_frames_total`), `RrDr60PlayInfo` (48), `RrDr60EngineState` (20), `RrDr60MemoryStatus` (48), `RrDr60SpeechCodecInfo` (28), `RrDr60ManifestHeader` (72), `RrDr60StageEntry` (80), `RrDr60Control` (120), `RrDr60ControlOption` (24); 64-bit sizes.
- **The rule, every sized struct** (the eighteen): set `struct_size = sizeof` before the call; below the smallest published layout or above `RR_DR60_MAX_STRUCT_SIZE` (4096) → `RR_DR60_STATUS_INVALID_ARGUMENT`, nothing written; a known smaller layout → its prefix; larger → the whole struct, the tail zeroed; `struct_size` reads back as the size the library filled; every padding byte is zero. `RrDr60Settings` keeps its stricter input rule (larger refused at `rr_dr60_create`).
- **The per-call reports** (`rr_dr60_process*`, `rr_dr60_record*`, `rr_dr60_stop_recording*`, `rr_dr60_play*`): a non-NULL report's size is checked right after the handle's NULL check; a refused size processes nothing; with an accepted size the report is always written, on every error as in 0.21.
- **Frozen**: `RrDr60VasEvent` (`RR_DR60_VAS_EVENT_SIZE` 16) and `RrDr60ChangeEntry` (`RR_DR60_CHANGE_ENTRY_SIZE` 40; its `reserved` field is now `stream`, same offset and meaning since 0.20); `RrDr60StopFields` frozen in layout.
- `RR_DR60_ABI_VERSION` (1); `uint32_t rr_dr60_abi_version(void)`; `RrDr60Status rr_dr60_abi_check(uint32_t abi_version, size_t pointer_size, size_t settings_size, size_t vas_event_size, size_t change_entry_size)` → `OK` or `RR_DR60_STATUS_ABI_MISMATCH` (37). Call once at start with `RR_DR60_ABI_VERSION, sizeof(void *), sizeof(RrDr60Settings), sizeof(RrDr60VasEvent), sizeof(RrDr60ChangeEntry)`.
- The C ABI is 64-bit pointer width only (iOS; Android `arm64-v8a`, `x86_64`).
- **The promise from 0.22**: a sized struct only grows under its size field, the frozen pair never changes, new statuses, controls, taps, defines and functions are appended; anything else moves `RR_DR60_ABI_VERSION`, is announced as breaking, and none is planned before 1.0.

```c
RrDr60BlockInfo info = { .struct_size = sizeof info };
if (rr_dr60_abi_check(RR_DR60_ABI_VERSION, sizeof(void *), sizeof(RrDr60Settings),
                      sizeof(RrDr60VasEvent), sizeof(RrDr60ChangeEntry)) != RR_DR60_STATUS_OK) { /* this build of the engine does not match */ }
```

## Swift (`swift/`)

- `RRDR60Sized` with `.sized` on every sized struct: `var info = RrDr60BlockInfo.sized`. Swift's `RrDr60BlockInfo()` zero-initializes `struct_size`, which the library refuses by design.
- `try RRDR60.checkABI()` at launch; the `RRDR60` target's entry points run it lazily before their first library call and throw `RRDR60Error.abiMismatch(library:header:)`.
- The raw `RRDR60C` product keeps the C contract as is.
