# Research: The Pre-1.0 C ABI Pass (spec 022)

**Date**: 2026-10-31 | **Spec**: [spec.md](spec.md) (D1 – D12 approved; D13 awaiting the owner) | **Baseline**: 0.21.0, merge commit 81f14dd | **Plan reviews applied**: coach 1 – 11, hardware B1 – B9, product M1 – M9 (2026-10-31)

Every item: Decision, Rationale, Alternatives. Code references are to `crates/rr_dr60_ffi/src/lib.rs` at 81f14dd.

## R-01 One helper for every struct the library fills

**Decision**: `unsafe fn write_sized<T>(out: *mut T, published: &[u32], build: impl FnOnce(&mut T)) -> RrDr60Status`.
- `published` is a per-struct `const`, ascending, ending in `size_of::<T>() as u32` (a const assertion pins the last entry; `METERS_PUBLISHED = [56, 64]`, `READOUT_PUBLISHED = [376]`, every other `[SIZE]`).
- The size is read by the existing `struct_size_at(out)` (line 6576: `cast::<u32>().read_unaligned()`), **never** `(*out).struct_size` (forming `&*out` of a 64-byte `T` over a caller's 56 bytes is UB).
- `size < published[0]` or `size > RR_DR60_MAX_STRUCT_SIZE` (4096) → `InvalidArgument`, nothing written.
- `let mut buf = MaybeUninit::<T>::zeroed()`; `build(&mut *buf.as_mut_ptr())` **assigns fields only** (a field place write touches that field's bytes; a whole-struct assignment `*p = T { .. }` or `*p = T::default()` is a typed copy that does not preserve padding in the abstract machine, so the helper's contract forbids it); the filled size `f` = the largest `published` entry ≤ `size`; `struct_size` written as `f` (D11); then a byte copy of `f` bytes from `buf.as_ptr().cast::<u8>()` and `write_bytes(0)` of `size − f`. The existing `fill_prefix` (6588) refuses `size < full` and takes a typed `&T`; it is replaced by the helper (its safety comment "no uninitialized padding" is false for the readout, R-04).
- **Closes a pre-existing bug** (hardware B2): `RrDr60RecordReadout` has four padding bytes at 212 – 215 (`reserved_2` ends at 212, `stop` is 8-aligned at 216) that `rr_dr60_processing_record_read` (7098 – 7100) copies from a typed stack value.
- The converters become fillers: `fn fill_into(&self, t: &mut T)` for `c_info` (1035), the `idle` site (1217), the three `from_core`, the manifest and control literals.
- `Default` on the fifteen out-structs is a manual impl: the zeroed value with `struct_size = size_of::<T>()` (for the harness's and the unit tests' constructions, R-10), never used by a writer. A source-scan test (the 014 pattern) asserts no out-struct derives `Default` and no `extern "C"` function assigns a whole value to one of the fifteen types (`*out =`, `put(`).
- A **one-off Miri run** at implementation (`cargo +nightly miri test -p rr_dr60_ffi write_sized`, the readout case included) gates the uninit-read question the 0xAA smoke cannot (it passes by luck on today's compilers); recorded here with its result.

**Rationale**: D1, D8, D10, D11 in one place; the only test that proves D11's known-smaller branch is Meters at 55 / 56 / 60 / 63 / 64 / 80 / 4097 (read-back 56 / 56 / 56 / 56 / 64 / 64 and the two refusals).

**Alternatives**: `min: u32` alone (cannot express three published layouts; after 1.0 the normal case); per-struct code; `bool` → `uint32_t`.

## R-02 The per-call check order and the early returns

**Decision**: in `process_block` (1207 – 1320) and the `rr_dr60_record*`, `rr_dr60_stop_recording*`, `rr_dr60_play*` families: when the report pointer is non-NULL, its size is read and validated **first**; a NULL or poisoned handle returns the handle's status (`NullPointer` / `InternalError`), writing the idle report only when the size is valid; otherwise a refused size returns `InvalidArgument` before `live(pipeline)`, the overlap check, `frames == 0`, `RecordingInProgress` / `PlaybackInProgress`, `TapNotOnRoute`, `NotRecording` / `NotPlaying`, and processes nothing. Every later early return writes the idle report through the helper (`produced = 0`, the current `paused`, the tail zeroed). The constant-data functions (`rr_dr60_manifest_header`, `_stage`, `rr_dr60_control`, `_option`, `rr_dr60_speech_codec_info`): NULL, then the size, then the index. The handle-taking status functions (`rr_dr60_get_state`, `rr_dr60_memory_status`, `rr_dr60_meters`) keep 0.18's order: `live` (NULL / poisoned), then NULL out, then the size. The contract line "always written when not NULL" becomes "always written when not NULL and its `struct_size` is accepted" (cbindgen.toml, the header, the FFI crate doc, spec 001's contract rule 2 with a dated note).

**Tests**: per family × {valid, smaller, larger, > cap}: a poisoned handle (`ffi-test-panic`, `rr_dr60__test_force_panic` at 1262 / 1651 / 1802 / 1881), `frames == 0`, overlap, `RecordingInProgress` on the one-pass, `NotRecording` on stop; the engine's `stream_position` (both streams), `memory_status` and `meters` snapshotted before and after a refused `process` / `record` / `play` and compared byte for byte (AS1's "processes nothing").

## R-03 The sizes table, the published lists and the frozen pair

**Decision**: 64-bit sizes, each verified by the hardware review's scratch program; each struct's `SIZE` is `size_of::<T>() as u32` (private, the Rust pin) and its `PUBLISHED` list is this version's size alone for the eleven.

| Struct | 0.21 | 0.22 | Note |
|---|---|---|---|
| RrDr60BlockInfo | 32 | 40 | 4-byte pad after the size (first field `usize`) |
| RrDr60RecordInfo | 40 | 48 | |
| RrDr60StopOutcome | 80 | 88 | flattened (R-04): `kept` at 48, `padding_start_sample` at 80 |
| RrDr60PlayInfo | 40 | 48 | |
| RrDr60EngineState | 16 | 20 | align 4 |
| RrDr60MemoryStatus | 40 | 48 | 4-byte tail pad |
| RrDr60SpeechCodecInfo | 24 | 28 | the Rust-layout equality assertion → per-field `offset_of!` pins (+4) |
| RrDr60ManifestHeader | 64 | 72 | |
| RrDr60StageEntry | 80 | 80 | the field fills the pad; `number` at 4 |
| RrDr60Control | 112 | 120 | `int_min` at 64; `reserved_014 + 4 == int_min` still holds |
| RrDr60ControlOption | 16 | 24 | 4-byte tail pad |
| RrDr60Meters | 64 | 64 | `[56, 64]` |
| RrDr60RecordReadout | 376 | 376 | `[376]`; padding 212 – 215 (R-01) |
| RrDr60InputAlignment | 32 | 32 | |
| RrDr60PlaceState | 32 | 32 | |
| RrDr60VasEvent | 16 | 16 | frozen (also 16 on 32-bit) |
| RrDr60ChangeEntry | 40 | 40 | frozen (also 40 on 32-bit); `reserved` → `stream` |

The three inputs: `RrDr60Settings` (120; R-05's rule), `RrDr60RunData` (80; `< 48` **or `> 4096`** refused on read, the cap added for one rule), `RrDr60StopFields` (40; frozen; `struct_size` written as 40 by the library, ignored on read as today, stated in the docs). The four defines are **literal** consts (cbindgen emits a define only for a literal: the 014 lesson): `RR_DR60_ABI_VERSION` 1, `RR_DR60_VAS_EVENT_SIZE` 16, `RR_DR60_CHANGE_ENTRY_SIZE` 40, `RR_DR60_MAX_STRUCT_SIZE` 4096, each with `const _: () = assert!(X as usize == size_of::<…>())` where a type exists. cbindgen emits `usize` as `uintptr_t`, so the contract writes the check's signature that way. A 32-bit target is refused by the check (pointer size ≠ 8); so are `wasm32` and 32-bit Android (`armeabi-v7a`, `x86`); `bool` width and endianness are fixed on every target Rust and Clang share.

## R-04 The flattened outcome, the frozen `StopFields`, the readout's embedded settings

**Decision**: `RrDr60StopOutcome { struct_size, flush_stored: usize, flush_events: usize, flush_paused: bool, flush_stopped: bool, flush_stop_reason: u32, flush_frames: usize, flush_frames_total: u64, kept: bool, number: u32, length_samples, silence_fed_samples, stop_input_position, padding_start_sample: u64 }` (88); the filler reads `outcome.flush`; the smoke's and the Swift tests' `outcome.flush.events` become `outcome.flush_events`. `RrDr60StopFields` is documented frozen (its layout is the record form's; a new form is a new struct). `RrDr60RecordReadout` keeps `settings` last; **in a prefix fill the embedded `settings.struct_size` is the settings bytes the prefix holds** (`f − offset_of!(RrDr60RecordReadout, settings)`), a post-fill fix-up in the readout's filler (the only out-struct with an embedded sized struct), pinned now by a unit test that fills through the helper with a doctored list `[368, 376]` at size 368 and asserts `settings.struct_size == 112` and the outer `struct_size == 368`.

**Rationale**: D9 (hardware B3, coach 2); coach 4 (the inner size would lie by one level otherwise). The alternative, refusing a stale readout size when Settings grows, would make D9's "settings last so the tail grows" false.

## R-05 The ABI check and the settings rule (D13)

**Decision**: `RR_DR60_ABI_VERSION` 1, `rr_dr60_abi_version() -> u32`, `rr_dr60_abi_check(abi_version: u32, pointer_size: usize, settings_size: usize, vas_event_size: usize, change_entry_size: usize) -> RrDr60Status`: `Ok` iff `abi_version == 1 && pointer_size == 8 && settings_size == size_of::<RrDr60Settings>() && vas_event_size == 16 && change_entry_size == 40`, else `AbiMismatch` (37, appended); pure, no handle.

**The settings rule (D13)**: today `check_struct_size` (727 – 733), `validate_c` (848) and `to_settings` (864) refuse `size < SETTINGS_SIZE` and **accept a larger struct**, reading the 120-byte prefix and dropping a newer caller's fields (the spec's draft said the opposite). In this break they refuse `size != SETTINGS_SIZE` (0.22 is a rebuild for every caller, so no shipped binary depends on the accept path). `RrDr60Settings` is the one exception to the growth promise: an input validated whole, so appending a setting is a rebuild for earlier callers, **moves `RR_DR60_ABI_VERSION`** and is reported by the check at start. The five arguments suffice: every other sized struct is verified per call by its own size; the frozen pair has no size field; `sizeof(RrDr60Meters)` would add nothing under the prefix rule.

What moves the version (the guide, the CHANGELOG): a frozen struct's bytes, a function's signature or removal, an enum value's meaning, a field reorder, a settings growth, the settings input rule, a shrinking published list (which does not happen). What does not: appended statuses, controls, taps, functions, defines; a field appended to an out-struct.

**Tests**: the right tuple; each argument wrong alone, the settings size one above **and** one below; all wrong; version 0 and `UINT32_MAX`; `rr_dr60_create` at 119 and 121 refused, 120 accepted; the engine's schedule, log and meters unchanged across the check.

**Alternative (D13)**: a real input prefix rule (a smaller settings struct's tail filled from the defaults; larger refused): then `<=` is right and the promise holds for settings too, at the cost of a standing constraint that every appended setting defaults to the neutral value, which spec 012's on-by-default microphone stages already break.

## R-06 The Swift package

**Decision**: in the `RRDR60` target (`Sized.swift`, `ABI.swift`): `public protocol RRDR60Sized { var struct_size: UInt32 { get set }; init() }` with `static var sized` setting `UInt32(MemoryLayout<Self>.size)` on the **seventeen** (the fifteen out-structs, `RrDr60RunData`, `RrDr60StopFields`; **never `RrDr60Settings`**: a zeroed settings is refused, it comes from `rr_dr60_settings_default`); `public enum RRDR60 { public static func checkABI() throws; static let abiStatus = rr_dr60_abi_check(RR_DR60_ABI_VERSION, UInt(MemoryLayout<UnsafeRawPointer>.size), UInt(MemoryLayout<RrDr60Settings>.size), UInt(MemoryLayout<RrDr60VasEvent>.size), UInt(MemoryLayout<RrDr60ChangeEntry>.size)) }` (`static let` is dispatch_once: once, thread-safe); `RRDR60Error.abiMismatch(reason: ABIMismatch)` with cases `version(library:header:)`, `pointerSize`, `settingsSize`, `vasEventSize`, `changeEntrySize`, derived by re-running the five comparisons in Swift (the C check stays one status); `ProcessingRecord`'s entry points call `try RRDR60.checkABI()` first (lazy: SwiftPM has no load hook); `ReRender.swift` and both test targets use `.sized`. `MemoryLayout<T>.size == .stride == sizeof` for every tail-padded shape (hardware B9 verified EngineState 20, MemoryStatus 48, ControlOption 24, BlockInfo 40). `swift/README.md`: "`import RRDR60` for the helpers (`.sized`, `checkABI()`, the re-render); `import RRDR60C` alone is the bare C API, where every report must be sized by hand"; the raw module's zero-initializer trap; what a mismatch means; 64-bit only; the `.iOS(.v16)` note. The helpers stay in `RRDR60` (product M5's cheaper option).

## R-07 The C smoke

**Decision**: `print_abi()` / `abi_cases()` and a `--abi` print mode diffed against `examples/abi_rows.rs` (the sizes table and the fifteen filled structs as rows; the Swift parity test reads the same rows): the 64-bit size table (every struct's `sizeof` against the expected value under `sizeof(void *) == 8`); per out-struct the fill route (BlockInfo `rr_dr60_process` on a 64-sample block; RecordInfo / StopOutcome a short recording; PlayInfo a playback; the status and constant-data structs directly; Meters; the readout via `rr_dr60_record_read` on the bytes the smoke already builds, no recording) and the four cases: size 0 and size − 1 refused with a 0xAA-painted struct unchanged byte for byte; the exact size filled with `struct_size` read back and every padding position 0 (the eleven's positions, the readout's 212 – 215, Meters as the negative control); size + 16 filled with a zero tail; 4097 and `UINT32_MAX` refused; the engine's position / memory / meters bytes unchanged across a refused per-call report; the per-call paths with a NULL handle and `frames == 0` at a valid and an invalid size; the ABI check's tuple and its variants; `rr_dr60_create` at 119 / 120 / 121. Timing prints (reported): `rr_dr60_process` per call at 64 and 4096 samples at 48 kHz over 20 s, and the `frames == 0` path over 10⁶ calls ("size check and idle write: N ns per call"). Every uninitialized `RrDr60X name;` declaration (count at implementation; about 46) becomes `= { .struct_size = sizeof name }` via a `SIZED(T, name)` macro. The hashes do not move.

## R-08 The cost

**Decision**: analytic: one unaligned u32 read, two compares, a ≤ 88-byte zeroing, the field writes, a ≤ 88-byte copy, a tail fill bounded by the cap (a caller asking for 4096 pays its own ~4 KB memset): about 20 – 40 ns per call, **below 1 % of a 64-sample block at 48 kHz** (~4.5 µs) and below 0.01 % at 4096; reported by the smoke's three figures; the existing `ab-timing` modes (`playback`, `record`, `bypass`) on the label against 81f14dd as a no-regression check of the Rust path, which the C layer cannot move. No new mode (D6).

## R-09 The records and the amended contracts

**Decision**: records.md lists them. The lead everywhere (product M1, M6): "**Breaking (0.21 → 0.22) for C and Swift callers: rebuild once and initialize every report.**" with the three source edits (the size fields, a **run-time** break for a caller that compiles unchanged; `flush.<x>` → `flush_<x>`; `reserved` → `stream`), the sizes table, "no audio change; the Rust API unchanged", the promise with "planned" kept and the "never" statements written as their mechanism (M7), D13's carve-out. A "Migrating a 0.21 caller" block (M2) in the contract and the guide; what a mismatch means (M3). Spec 001's `contracts/c-api.md` rules 2 and 5, spec 018's and 020's contracts, gain dated notes. decisions.md's new row corrects the 2026-10-31 row's change-entry item and records D13. brief.md: "64-bit targets only (iOS is 64-bit; on Android `arm64-v8a` and `x86_64`, which Play serves to every current device; 32-bit-only devices are not offered the app)". The traceability audit (`scripts/check-traceability.sh`) gains `crates/rr_dr60_ffi/src/lib.rs` so the new literals are audited (coach 8).

## R-10 The harness constructions and the rename

**Decision**: the manual `Default` (R-01) keeps the harness's `RrDr60BlockInfo::default()` constructions valid (alloc_free.rs, ffi_parity.rs; the FFI unit tests); the `reserved` reads become `.stream`; `ffi_parity::abi_pass_parity` covers the fifteen structs' four cases (below, exact + padding + read-back, larger, above the cap), the check, the create rule, the state-unchanged snapshots, and reads `examples/abi_rows.rs`'s rows as the smoke does.

## Figures and restatements

*(Filled at implementation: the sizes as `size_of` pins them, the smoke's three figures, the Miri run's result, the A/B label's figures.)*
