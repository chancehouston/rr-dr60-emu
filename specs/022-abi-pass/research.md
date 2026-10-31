# Research: The Pre-1.0 C ABI Pass (spec 022)

**Date**: 2026-10-31 | **Spec**: [spec.md](spec.md) (D1 – D12 approved) | **Baseline**: 0.21.0, merge commit 81f14dd

Every item: Decision, Rationale, Alternatives. Code references are to `crates/rr_dr60_ffi/src/lib.rs` at 81f14dd.

## R-01 One helper for every sized out-struct

**Decision**: `unsafe fn write_sized<T: Copy>(out: *mut T, min: u32, build: impl FnOnce(&mut T)) -> RrDr60Status`: read the caller's `struct_size` at offset 0 (the first field of every `repr(C)` sized struct; the caller passed a non-NULL, readable pointer); `size < min` or `size > RR_DR60_MAX_STRUCT_SIZE` (4096) → `InvalidArgument`, nothing written; `let mut v = MaybeUninit::<T>::zeroed()` and the fields written by `build` on `&mut *v.as_mut_ptr()` (a typed `*p = value` would reintroduce the value's uninitialized padding, so `build` assigns fields of a zeroed value; every struct's `Default` is defined as the zeroed value with `struct_size = size_of::<T>()`, so `build` may start from `T::default()` **only** when `Default` is implemented as `zeroed()` plus the size, which the plan enforces); `struct_size = size_of::<T>() as u32` (D11); then `fill_prefix(out, &v, size, full)` as today: a known smaller layout gets exactly its prefix with `struct_size` written as that prefix's size (the 018 Meters rule: `from_core(m, size)` wrote the caller's size, which was equal to the prefix size in the only such case; now stated as "the filled size"), larger gets the full struct and a zeroed tail. `full` is `size_of::<T>() as u32` on the build target, never a literal. The seven existing sized structs move onto the helper: `rr_dr60_meters` (lines 2781 – 2793: the cap added, the read-back the filled size) and the spec 020 five (the cap added; they already write the library's size at 7100, 7510, 7664, 7771, 7783).

**Rationale**: D1, D8, D10, D11 in one place; one code path to test for the eighteen sized structs.

**Alternatives**: per-struct code (eighteen copies of the same arithmetic); `bool` → `uint32_t` to remove padding (D10's alternative: four sizes change for nothing once the padding is zeroed by construction).

## R-02 The per-call check order and the early returns

**Decision**: in `rr_dr60_process` / `_with_events` / `_with_events_and_changes` (through `process_block`, lines 1209 – 1318), `rr_dr60_record` / `_with_changes`, `rr_dr60_stop_recording` / `_with_changes` and `rr_dr60_play` / `_with_changes`: after the handle's NULL check and before `live(pipeline)`, the overlap check and `frames == 0`, when the report pointer is non-NULL its size is validated (`min`, the cap); a refusal returns `InvalidArgument` with nothing written and nothing processed. Every later early return keeps writing the report as in 0.21 (`idle`: `produced = 0`, the current `paused`; the `default()` sites at 1209, 1637, 1718, 1788), through `write_sized` so the tail is zeroed and the size written back. `put_info` (1027) becomes a thin call to the helper. With a NULL handle the status stays `NullPointer`; the report is written only if its size is valid (the handle check precedes the size check, the size check precedes the write). The constant-data functions (`rr_dr60_manifest_header`, `_stage`, `rr_dr60_control`, `_option`, `rr_dr60_speech_codec_info`, `rr_dr60_get_state`, `rr_dr60_memory_status`): NULL, then the size, then the index. The cbindgen.toml / header contract line "always written when not NULL" becomes "always written when not NULL and its `struct_size` is accepted".

**Rationale**: D12; the 0.21 "always written" promise is kept for every accepted size.

**Alternatives**: validating the size after the state checks (a poisoned handle would then write a struct whose size it never checked).

## R-03 The sizes table and the per-struct minimum

**Decision**: 64-bit sizes (the hardware review's scratch program verified them); each struct gets `const <NAME>_SIZE: u32 = size_of::<T>() as u32` and `const <NAME>_SIZE_MIN: u32` (this version's size for the eleven, pinned by a const assertion `SIZE == SIZE_MIN` that a later growth updates by naming the new minimum); the existing `METERS_SIZE_016` (56), `STOP_FIELDS_SIZE`, `RUN_DATA_SIZE`, `READOUT_SIZE` (376, including Settings 120), `ALIGNMENT_SIZE`, `PLACE_STATE_SIZE` keep their roles.

| Struct | 0.21 | 0.22 | Note |
|---|---|---|---|
| RrDr60BlockInfo | 32 | 40 | 4-byte pad after the size (first field `usize`) |
| RrDr60RecordInfo | 40 | 48 | |
| RrDr60StopOutcome | 80 | 88 | flattened (D9): 4 + 4 pad + 7 flush fields (8+8+1+1+4+8+8 with pads → 40) + kept/number/length/silence/stop/padding |
| RrDr60PlayInfo | 40 | 48 | |
| RrDr60EngineState | 16 | 20 | align 4 |
| RrDr60MemoryStatus | 40 | 48 | 4-byte tail pad |
| RrDr60SpeechCodecInfo | 24 | 28 | the Rust-layout equality assertion → offset pins |
| RrDr60ManifestHeader | 64 | 72 | |
| RrDr60StageEntry | 80 | 80 | the field fills the pad after `number` |
| RrDr60Control | 112 | 120 | `int_min` stays 8-aligned; `reserved_014` kept |
| RrDr60ControlOption | 16 | 24 | 4-byte tail pad |
| RrDr60VasEvent | 16 | 16 | frozen |
| RrDr60ChangeEntry | 40 | 40 | frozen; `reserved` → `stream` |

The exact flattened `StopOutcome` size is pinned by the const assertion at implementation (88 expected; the plan's figure is confirmed by `size_of`). On a 32-bit target the sized structs differ (pointer-sized counts), which the ABI check refuses (pointer size ≠ 8); the frozen pair holds everywhere.

**Rationale**: coach 6, hardware B10; `size_of` is the only honest source of a size.

## R-04 The flattened outcome and the frozen `StopFields`

**Decision**: `RrDr60StopOutcome { struct_size, flush_stored: usize, flush_events: usize, flush_paused: bool, flush_stopped: bool, flush_stop_reason: u32, flush_frames: usize, flush_frames_total: u64, kept: bool, number: u32, length_samples, silence_fed_samples, stop_input_position, padding_start_sample: u64 }` (field order as today with the flush's seven fields where the nested struct was); `From<&StopOutcome>` fills them from `outcome.flush`; the smoke's and the Swift tests' `outcome.flush.events` reads become `outcome.flush_events`; `RrDr60StopFields::from(&outcome)` in C reads the new fields (the C helper in the smoke, if any, updated). `RrDr60StopFields` is documented frozen (its layout is the record form's; a new form version is a new struct); `RrDr60RecordReadout` keeps `settings` last (Settings grows at its end, so the read-out's tail grows with it and its minimum stays 376).

**Rationale**: D9 (hardware B3, coach 2).

## R-05 The ABI check

**Decision**: `pub const RR_DR60_ABI_VERSION: u32 = 1;` (a literal, so cbindgen emits a define; the unit test pins it), `rr_dr60_abi_version() -> u32`, `rr_dr60_abi_check(abi_version: u32, pointer_size: usize, settings_size: usize, vas_event_size: usize, change_entry_size: usize) -> RrDr60Status`: `Ok` iff `abi_version == 1 && pointer_size == 8 && settings_size <= size_of::<RrDr60Settings>() && vas_event_size == 16 && change_entry_size == 40`, else `AbiMismatch` (37, appended after `StageNotConfigured`); pure, no handle. The version's semantics (the guide, the CHANGELOG): moves on a frozen struct's bytes, a function's signature or removal, an enum value's meaning, a field reorder, the settings' input rule, a shrinking minimum (never); appending a status, a control, a tap, a function, a define or a field to a sized struct does not move it. The settings rule: a caller whose settings struct is *larger* than the library's would be refused at `rr_dr60_create` anyway (a newer header against an older library), so the check reports it early; smaller passes (an older caller the settings rule serves).

**Rationale**: D3 (coach 7, hardware B9, product M2).

## R-06 The Swift package

**Decision**: in the `RRDR60` target: `public protocol RRDR60Sized { var struct_size: UInt32 { get set }; init() }` with `extension RRDR60Sized { public static var sized: Self { var s = Self(); s.struct_size = UInt32(MemoryLayout<Self>.size); return s } }` and conformances for the eighteen sized structs (the eleven and the seven); `public enum RRDR60 { public static func checkABI() throws; static let abiStatus: RrDr60Status = rr_dr60_abi_check(RR_DR60_ABI_VERSION, UInt(MemoryLayout<UnsafeRawPointer>.size), UInt(MemoryLayout<RrDr60Settings>.size), UInt(MemoryLayout<RrDr60VasEvent>.size), UInt(MemoryLayout<RrDr60ChangeEntry>.size)) }` with `checkABI()` throwing `RRDR60Error.abiMismatch(library: rr_dr60_abi_version(), header: RR_DR60_ABI_VERSION)`; `ProcessingRecord`'s entry points call `try RRDR60.checkABI()` first (the lazy run: SwiftPM has no load hook); `ReRender.swift` and both test targets replace `RrDr60X()` with `.sized`; `MemoryLayout<T>.size == .stride == sizeof` for every imported C struct (verified by the hardware review), so `.size` is right. `swift/README.md`: the raw `RRDR60C` module's zero-initializer trap, `.sized`, `checkABI()`, 64-bit only; `Package.swift`'s `.iOS(.v16)` left with a note (the library runs anywhere; the app's minimum is the app's).

**Rationale**: FR-008 (product M1, coach 8).

## R-07 The C smoke

**Decision**: a new `print_abi()` / `abi_cases()` section: the 64-bit size table (every struct's `sizeof` against the expected value, under `sizeof(void *) == 8`); per sized struct a generic case runner over a function pointer that fills it (the eleven plus Meters and the 020 five where a fill needs no recording: BlockInfo through `rr_dr60_process` on a 64-sample block, RecordInfo/StopOutcome through a short recording, PlayInfo through a playback, the status and constant-data structs directly): size 0 and size − 1 refused with a 0xAA-painted struct unchanged byte for byte, the exact size filled with `struct_size` read back and the padding positions 0, size + 16 filled with the 16-byte tail 0, 4097 and `UINT32_MAX` refused; the per-call paths with a NULL handle and `frames == 0` at a valid and an invalid size; the ABI check's tuple, each argument wrong alone, all wrong, version 0 and `UINT32_MAX`; the per-call wall time of `rr_dr60_process` over 20 s at 48 kHz in 4096-sample blocks, printed as "process call: N ns per call" (reported). All 40 uninitialized `RrDr60X name;` declarations become `= { .struct_size = sizeof name }` (or a `SIZED(T, name)` macro). The smoke's hashes do not move (no audio change).

## R-08 The cost

**Decision**: stated analytically (one unaligned u32 read and two compares, a copy of at most 88 bytes, an optional tail fill bounded by the cap; under 0.01 % of a 4096-sample block's ~70 ns per sample path, about 0.1 % at 64-sample blocks) and reported by the smoke's per-call figure; the existing `ab-timing` modes (`playback`, `record`, `bypass`) run on the label against 81f14dd as a no-regression check of the Rust path. No new mode (D6).

## R-09 The records and the two amended contracts

**Decision**: records.md lists them; spec 018's contracts/api.md and spec 020's contracts/api.md gain dated notes: the cap on every sized struct, the read-back as the filled size (Meters), `StopFields` frozen. decisions.md's new row corrects the 2026-10-31 row's "the change entry" item. The CHANGELOG lead (product M7's draft): "**Breaking (0.21 → 0.22) for C callers: rebuild once.**" with the sizes table and D4's promise wording.

## R-10 The harness constructions and the rename

**Decision**: every `rr_dr60_ffi` struct's `Default` is `zeroed()` with `struct_size = size_of::<T>()`, so the harness's `RrDr60BlockInfo::default()` (alloc_free.rs, ffi_parity.rs: 69 + 12 constructions) and the FFI unit tests' 77 constructions keep passing a valid size; the `reserved` reads in `ffi_parity` (the two assertions updated in spec 020 to `reserved == stream`) become `.stream`; `assert_eq!(info, RrDr60BlockInfo::default())`-style oracles compare the written-back size (equal to `default()`'s). `ffi_parity::abi_pass_parity` adds the eleven structs' three cases and the check.

## Figures and restatements

*(Filled at implementation: the sizes as `size_of` pins them, the smoke's per-call figure, the A/B label's figures.)*
