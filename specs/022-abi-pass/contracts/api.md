# API contract: The Pre-1.0 C ABI Pass (spec 022, version 0.22.0)

**Breaking (0.21 → 0.22) for C and Swift callers: rebuild once and initialize every report.** Three source edits: (1) set `struct_size = sizeof` on every struct the library fills (`RrDr60BlockInfo info = { .struct_size = sizeof info };`; Swift: `var info = RrDr60BlockInfo.sized`): a zero or uninitialized size is refused with `RR_DR60_STATUS_INVALID_ARGUMENT` **at run time, not at compile time**, so a 0.21 caller that compiles unchanged fails every report call until it is edited; (2) `outcome.flush.<x>` → `outcome.flush_<x>`; (3) `entry.reserved` → `entry.stream`. The `rr_dr60` Rust API and every output are unchanged.

## C (`include/rr_dr60.h`)

- The eleven out-structs gain `uint32_t struct_size` as their first field: `RrDr60BlockInfo` (40), `RrDr60RecordInfo` (48), `RrDr60StopOutcome` (88, the flush's seven fields inline: `flush_stored`, `flush_events`, `flush_paused`, `flush_stopped`, `flush_stop_reason`, `flush_frames`, `flush_frames_total`), `RrDr60PlayInfo` (48), `RrDr60EngineState` (20), `RrDr60MemoryStatus` (48), `RrDr60SpeechCodecInfo` (28), `RrDr60ManifestHeader` (72), `RrDr60StageEntry` (80), `RrDr60Control` (120), `RrDr60ControlOption` (24); 64-bit sizes.
- **The rule, every struct the library fills** (the eleven plus `RrDr60Meters`, `RrDr60RecordReadout`, `RrDr60InputAlignment`, `RrDr60PlaceState`): set `struct_size = sizeof` before the call; below the smallest published layout or above `RR_DR60_MAX_STRUCT_SIZE` (4096) → `RR_DR60_STATUS_INVALID_ARGUMENT`, nothing written; a known smaller layout → its prefix; larger → the whole struct, the tail zeroed; `struct_size` reads back as the size the library filled; every padding byte is zero. In a prefix fill of `RrDr60RecordReadout` the embedded `settings.struct_size` is the settings bytes the prefix holds.
- **The inputs**: `RrDr60Settings` is validated whole: a `struct_size` different from `sizeof(RrDr60Settings)` is refused at `rr_dr60_create`, `rr_dr60_reconfigure` and `rr_dr60_settings_validate` (before 0.22 a larger one was accepted and its tail ignored). It is the one exception to the growth promise: appending a setting is a rebuild for earlier callers and moves `RR_DR60_ABI_VERSION`. `RrDr60RunData`: below 48 or above 4096 refused on read. `RrDr60StopFields`: frozen in layout; its size is written as 40 and ignored on read.
- **The per-call reports** (`rr_dr60_process*`, `rr_dr60_record*`, `rr_dr60_stop_recording*`, `rr_dr60_play*`): a non-NULL report's size is checked first; with a NULL or poisoned handle the handle's status is returned and the idle report written only when the size is valid; otherwise a refused size processes nothing; with an accepted size the report is always written, on every error as in 0.21.
- **Frozen**: `RrDr60VasEvent` (`RR_DR60_VAS_EVENT_SIZE` 16) and `RrDr60ChangeEntry` (`RR_DR60_CHANGE_ENTRY_SIZE` 40; its `reserved` field is now `stream`, same offset and meaning since 0.20). A different layout would be a new type with new functions, never a change to these.
- `RR_DR60_ABI_VERSION` (1); `uint32_t rr_dr60_abi_version(void)`; `RrDr60Status rr_dr60_abi_check(uint32_t abi_version, uintptr_t pointer_size, uintptr_t settings_size, uintptr_t vas_event_size, uintptr_t change_entry_size)` → `OK` or `RR_DR60_STATUS_ABI_MISMATCH` (37). Call once at start with `RR_DR60_ABI_VERSION, sizeof(void *), sizeof(RrDr60Settings), sizeof(RrDr60VasEvent), sizeof(RrDr60ChangeEntry)`.
- **What a mismatch means**: the check guards against a library built from a different header than the app (a stale XCFramework, a mixed build), never against anything a user of the app can change: the library ships inside the app. Treat a failure as an assertion: in a debug build stop; in a release build show that the app's build is broken and must be updated, and do not create an engine.
- The C ABI is 64-bit pointer width only (iOS; Android `arm64-v8a` and `x86_64`). The check refuses 32-bit Android (`armeabi-v7a`, `x86`) and `wasm32`.
- **The promise from 0.22**: a struct the library fills only grows under its size field, the frozen pair does not change (a new layout is a new type), new statuses, controls, taps, defines and functions are appended; `RrDr60Settings` is the one exception (above). Anything else moves `RR_DR60_ABI_VERSION`, is announced as breaking, and none is planned before 1.0.

### Migrating a 0.21 caller

1. Rebuild against `rr_dr60.h` 0.22 (the XCFramework: `scripts/build-xcframework.sh`).
2. Every struct the library fills starts as `{ .struct_size = sizeof x }` (Swift: `.sized`). Grep for `RrDr60BlockInfo`, `RecordInfo`, `StopOutcome`, `PlayInfo`, `EngineState`, `MemoryStatus`, `SpeechCodecInfo`, `ManifestHeader`, `StageEntry`, `Control`, `ControlOption`; the structs that already had a size field are unchanged in use.
3. `outcome.flush.<x>` → `outcome.flush_<x>`; `entry.reserved` → `entry.stream`.
4. Arrays of `RrDr60VasEvent` and `RrDr60ChangeEntry` need no change (frozen).
5. Call `rr_dr60_abi_check(...)` once at start (Swift: `try RRDR60.checkABI()`); treat a failure as a build error of your app, not a user condition.
6. A `RrDr60Meters` caller that passed a `struct_size` above 4096 (none known) is now refused; the read-back is the filled size. A settings struct larger than the library's is now refused at `rr_dr60_create`.

```c
RrDr60BlockInfo info = { .struct_size = sizeof info };   /* every struct the library fills starts like this (0.22) */
if (rr_dr60_abi_check(RR_DR60_ABI_VERSION, sizeof(void *), sizeof(RrDr60Settings),
                      sizeof(RrDr60VasEvent), sizeof(RrDr60ChangeEntry)) != RR_DR60_STATUS_OK) { /* this build of the app is broken */ }
```

## Swift (`swift/`)

- `import RRDR60` for the helpers; `import RRDR60C` alone is the bare C API, where every report must be sized by hand (`info.struct_size = UInt32(MemoryLayout<RrDr60BlockInfo>.size)`).
- `RRDR60Sized` with `.sized` on every struct the library fills or reads except `RrDr60Settings` (which comes from `rr_dr60_settings_default`): `var info = RrDr60BlockInfo.sized`. Swift's `RrDr60BlockInfo()` zero-initializes `struct_size`, which the library refuses by design.
- `try RRDR60.checkABI()` at launch; the `RRDR60` target's entry points run it lazily before their first library call and throw `RRDR60Error.abiMismatch(reason:)` naming the failing comparison.
