---

description: "Task list for the pre-1.0 C ABI pass (022)"
---

# Tasks: The Pre-1.0 C ABI Pass

**Input**: Design documents from `/specs/022-abi-pass/` (spec.md D1 – D13, plan.md P1 – P9, research.md R-01 – R-10, data-model.md, contracts/api.md, quickstart.md, records.md)

**Tests**: required (constitution III; spec FR-001 – FR-010, SC-001 – SC-005). Tests are written first in each story and fail before the implementation.

**Cadence** (owner, 2026-10-29): the phases run without stopping; stop only on a failed check or a new decision; the next owner stop is the merge.

## Format: `[ID] [P?] [Story] Description`

## Phase 1: Setup

- [x] T001 Bump the workspace to 0.22.0: `version = "0.22.0"` in `[workspace.package]` and the internal dependency entries of `Cargo.toml`; `RR_DR60_VERSION_MINOR = 22` and its test in `crates/rr_dr60_ffi/src/lib.rs`; regenerate `crates/rr_dr60_ffi/include/rr_dr60.h`; re-bless the stage manifest's version line (`RR_DR60_BLESS=manifest cargo test -p rr_dr60_harness --test us2_cross_check canonical`, then `RR_DR60_BLESS=readme … readme -- --test-threads=1`); the Swift version test if it pins the minor.
- [x] T002 [P] Record the baseline in a scratch note: the fifteen golden SHA guards (`crates/rr_dr60_harness/tests/golden*.rs`), every smoke hash in `crates/rr_dr60_ffi/tests/c/smoke.c`, the 1259-test count of `cargo test --all-features`, coverage 96.64 % locally (0.21), the A/B baseline 81f14dd, `git rev-parse 81f14dd:crates/rr_dr60` and `:crates/rr_dr60_detmath` (the core trees). They must all be unmoved at the end (FR-006, SC-003).

## Phase 2: Foundational (the helper, the sizes, the check; `crates/rr_dr60_ffi/src/lib.rs` throughout)

- [x] T003 Tests first (red) in the FFI crate's unit tests, `mod abi_tests`: `sizes_are_pinned` (the seventeen `size_of` figures of research R-03's table as const assertions: 40, 48, 88, 48, 20, 48, 28, 72, 80, 120, 24, 64, 376, 32, 32, 16, 40, plus `offset_of!` pins: `RrDr60StopOutcome.kept` 48 and `.padding_start_sample` 80, `RrDr60StageEntry.number` 4, `RrDr60Control.int_min` 64, `RrDr60RecordReadout.stop` 216 and `.settings` 376 − 120; the `RrDr60SpeechCodecInfo` per-field `offset_of!` pins replacing the layout-equality assertion at ~527), `published_lists_end_in_size_of` (every `PUBLISHED` ascending, last == `SIZE`; `METERS_PUBLISHED == [56, 64]`), `defines_are_literal_and_pinned` (`RR_DR60_VAS_EVENT_SIZE == 16 == size_of`, `RR_DR60_CHANGE_ENTRY_SIZE == 40 == size_of`, `RR_DR60_ABI_VERSION == 1`, `RR_DR60_MAX_STRUCT_SIZE == 4096`), `write_sized_meters_cases` (55 refused untouched, 56 → read-back 56, 60 → 56 with bytes 56..60 zero, 63 → 56, 64 → 64, 80 → 64 with a zero tail, 4097 and `u32::MAX` refused; the out buffer painted 0xAA before each call; research R-01), `write_sized_every_out_struct_four_cases` (the fifteen: below / exact + padding 0 + read-back / larger + zero tail / above the cap, each on a painted buffer; the padding positions per struct from the table incl. the readout's 212 – 215 and Meters' none), `readout_prefix_fixes_embedded_settings` (a doctored list `[368, 376]` at size 368: outer 368, `settings.struct_size` 112; research R-04), `no_out_struct_derives_default_and_no_whole_writes` (a source scan of `lib.rs`: none of the fifteen type names within a `#[derive(…Default…)]`; no `*out = ` / `put(` / `.write(` of those types outside `write_sized`; shown failing on a doctored string), `default_is_zeroed_plus_size` (each of the fifteen: `default()` bytes all zero except `struct_size == SIZE`).
- [x] T004 Then: `unsafe fn write_sized<T>(out: *mut T, published: &[u32], build: impl FnOnce(&mut T)) -> RrDr60Status` per research R-01 (`struct_size_at`, the cap, `MaybeUninit::zeroed`, field-only `build`, the largest published ≤ size, the read-back, the byte copy from the buffer, the zero tail); per-struct `const <NAME>_SIZE: u32 = size_of::<T>() as u32` (private) and `<NAME>_PUBLISHED: &[u32]`; `RR_DR60_MAX_STRUCT_SIZE`, `RR_DR60_VAS_EVENT_SIZE`, `RR_DR60_CHANGE_ENTRY_SIZE`, `RR_DR60_ABI_VERSION` as literal `pub const`s with rustdoc citing "spec 022 D8 / D2 / D3"; `fill_prefix` removed; `uint32_t struct_size` first on the eleven; `RrDr60StopOutcome` flattened (R-04); `RrDr60ChangeEntry.reserved` → `stream` with the frozen docs on both structs, `rr_dr60_record_event`, `rr_dr60_record_change`, `RrDr60RunData`'s arrays and `RrDr60StopFields`; manual `impl Default` on the fifteen (zeroed + size); the fillers `fill_into(&mut T)` replacing `c_info`, the `idle` literal, the three `from_core`, the manifest / control / codec-info / state / memory-status literals; the readout's embedded-settings fix-up; every "no size field (the pre-1.0 ABI pass)" doc phrase removed (~365, ~3801, the PlayInfo note); T003 green.
- [x] T005 Tests first (red): `abi_check_tuple` (OK for (1, 8, 120, 16, 40); mismatch for each argument wrong alone with the settings size 119 **and** 121; all wrong; version 0 and `u32::MAX`; no engine needed), `create_refuses_settings_size_119_and_121` (and `reconfigure`, `settings_validate`; 120 accepted; `RrDr60SettingField::StructSize`), `run_data_cap_on_read` (`struct_size` 4097 refused by `rr_dr60_record_write`'s run input; 47 refused as today), `stop_fields_size_ignored_on_read_written_as_40`; then `rr_dr60_abi_version()`, `rr_dr60_abi_check(abi_version: u32, pointer_size: usize, settings_size: usize, vas_event_size: usize, change_entry_size: usize) -> RrDr60Status` (R-05), `RrDr60Status::AbiMismatch = 37` with its header comment, `check_struct_size` / `validate_c` / `to_settings` refusing `size != SETTINGS_SIZE` (D13; the 0.12 "112 refused" tests and doc comments restated for `!=`), `run_in`'s cap; `cbindgen.toml`'s `include` for any new type; the header regenerated; T005 green.
- [x] T006 Tests first (red), the per-call matrix (`ffi-test-panic` for the poisoned handle): for each family `process` / `process_with_events` / `process_with_events_and_changes` / `record` / `record_with_changes` / `stop_recording` / `stop_recording_with_changes` / `play` / `play_with_changes` × sizes {valid, 0, size − 1, size + 16, 4097}: NULL handle (handle's status; idle written only when valid), poisoned handle (`InternalError`; same), live + `frames == 0`, overlap, `RecordingInProgress` on the one-pass, `NotRecording` on stop, `PlaybackInProgress` (idle written on valid; `InvalidArgument` and nothing written on invalid); `refused_report_processes_nothing` (snapshots of `rr_dr60_stream_position` on both streams, `rr_dr60_memory_status` and `rr_dr60_meters` byte-equal before and after a refused `process` / `record` / `play` with audio in the buffer); `status_functions_keep_0_18_order` (`get_state`, `memory_status`, `meters` on a poisoned handle with a bad size → `InternalError`); `constant_data_order` (NULL, size, index on `manifest_header` / `manifest_stage` / `control` / `control_option` / `speech_codec_info`: a bad index with a bad size reports the size); then the check order in `process_block` and the eight other entry points per research R-02 (`put_info` a thin call to the helper; every early return through it), the status functions on the helper with the cap and the read-back (`rr_dr60_meters`'s 56 – 63 branch now in `METERS_PUBLISHED`; its unit tests at ~5433 / ~5453 echoing the caller's 55 / 72 restated to the filled size), the five spec 020 readers (7100, 7510, 7664) on the helper; the contract comment in `cbindgen.toml`, the crate doc and the header: "always written when not NULL and its `struct_size` is accepted"; T006 green.
- [x] T007 The one-off Miri gate: `cargo +nightly miri test -p rr_dr60_ffi abi_tests` (install `miri` on nightly if absent: `rustup +nightly component add miri`); the result (clean, or the finding and its fix) recorded in research.md › Figures and restatements (SC-004).

**Checkpoint**: the FFI crate's own tests green; every size pinned; the helper proven on Meters' two layouts and the readout's padding; the check order and D13 in place.

## Phase 3: User Story 1 - A report can grow without breaking my app (P1)

**Goal**: the four cases per out-struct and the painted padding in the C smoke and the parity test; the Rust and C paths agree field for field.

**Independent test**: `crates/rr_dr60_ffi/tests/c/run_smoke.sh` and `cargo test -p rr_dr60_harness --test ffi_parity abi_pass_parity -- --nocapture`.

### Tests first (red until T010)

- [x] T008 [P] [US1] `crates/rr_dr60_ffi/tests/c/smoke.c` and `run_smoke.sh`: a `SIZED(T, name)` macro (`T name = { .struct_size = sizeof name }`) replacing every uninitialized out-struct declaration (count them: `grep -c` the pattern, record the count); `print_abi()` for `smoke --abi`: the 64-bit size table (every struct's `sizeof` against research R-03 under `sizeof(void *) == 8`) and one row per out-struct filled at its exact size (the fields as decimal columns, the row format of T009's `examples/abi_rows.rs`); `abi_cases()` in the default run: per out-struct with its fill route (BlockInfo `rr_dr60_process` on 64 samples; RecordInfo / StopOutcome a short recording; PlayInfo a playback; `get_state`, `memory_status`, `speech_codec_info`, `manifest_header`, `manifest_stage(3)`, `control(0)`, `control_option`, `meters` directly; the readout via `rr_dr60_record_read` on the record bytes the smoke already builds): size 0 and size − 1 refused with a 0xAA-painted struct unchanged byte for byte, the exact size filled with `struct_size` read back and every padding position 0 (the per-struct offsets; the readout's 212 – 215; Meters as the control), size + 16 filled with a zero 16-byte tail, 4097 and `UINT32_MAX` refused; the state-unchanged check (position / memory status / meters bytes across a refused `process`, `record`, `play`); the NULL-handle and `frames == 0` paths at a valid and an invalid size; the ABI check tuple, each wrong alone (settings 119 and 121), all wrong, 0 and `UINT32_MAX`; `rr_dr60_create` at 119 / 120 / 121; `outcome.flush_events` and `entry.stream` in the existing cases; the three timing prints (`rr_dr60_process` per call at 64 and 4096 samples, 48 kHz, 20 s; the `frames == 0` path over 10⁶ calls: "size check and idle write: N ns per call"); `run_smoke.sh` diffs `smoke --abi` against `cargo run -q -p rr_dr60_harness --example abi_rows`. Every hash unchanged.
- [x] T009 [P] [US1] `crates/rr_dr60_harness/examples/abi_rows.rs` (the size table and the fifteen rows from the Rust path, byte-identical to the smoke's print) and `crates/rr_dr60_harness/tests/ffi_parity.rs` `abi_pass_parity`: for each of the fifteen structs the four cases through the C functions against the Rust path's values (the per-call reports at `configs::measurement_rates()` on the voiced stimulus, the rest once), field for field incl. the read-back; the ABI check's tuple and variants; the create rule; the state snapshots; the `reserved` reads → `.stream` (the two spec 020 assertions); `alloc_free.rs` and `determinism.rs` unchanged in behavior (their `default()` constructions pass the size). Expected: 1 new test, 2 edits.

### Implementation

- [x] T010 [US1] Make T008 – T009 green (any restatement dated in spec.md's Clarifications); `crates/rr_dr60_ffi/tests/c/run_smoke.sh`; `cargo test -p rr_dr60_harness --test ffi_parity abi_pass_parity -- --nocapture`.

**Checkpoint**: US1 green; the smoke's size table, cases and three figures printed; the hashes unmoved.

## Phase 4: User Story 2 - A mismatched build is refused at start (P2)

**Goal**: the Swift package's `.sized`, `checkABI()` and parity; the records that an app developer reads.

**Independent test**: `scripts/build-xcframework.sh && cd swift && swift test`.

### Tests first (red until T013)

- [x] T011 [P] [US2] `swift/Tests/RRDR60Tests/ABITests.swift` (new): `checkABIPasses`, `sizedSetsMemoryLayoutSize` (each of the seventeen conformances: `struct_size == MemoryLayout<T>.size`; `RrDr60Settings` has no `.sized`), `zeroInitializedReportIsRefused` (`RrDr60BlockInfo()` → `INVALID_ARGUMENT` from `rr_dr60_process`), `mismatchReasonIsNamed` (the five comparisons re-run with one wrong input through the internal reason derivation), `abiRowsParity` (the fifteen structs filled through the C API in Swift against `examples/abi_rows.rs`'s output committed as a fixture or regenerated by the test script); `swift/Tests/RRDR60CTests/SmokeParityTests.swift` and `ReRenderTests` on `.sized` and `flush_events`.
- [x] T012 [P] [US2] `crates/rr_dr60_harness/tests/us2_abi.rs` (spec US2 AS1 – AS3 and US3 AS1 – AS4 in Rust): `as1_check_and_version` (pointer to T005's cases from the harness side, run once), `as2_create_rule`, `as3_frozen_pair_pinned` (the defines against `size_of`, the `stream` name), `us3_core_untouched` (`git diff --quiet 81f14dd -- crates/rr_dr60 crates/rr_dr60_detmath` via `std::process::Command`, skipped if git is absent), `us3_guards_unmoved` (the fifteen SHAs re-read from the golden tests' sources, as 021's `as1`), `us3_header_diff_listed` (the header against 81f14dd's: every differing line matches one of the listed additions, the flattening, the rename, the contract comment). Expected: 6 tests.

### Implementation

- [x] T013 [US2] `swift/Sources/RRDR60/Sized.swift` (`RRDR60Sized`, `.sized`, the seventeen conformances), `swift/Sources/RRDR60/ABI.swift` (`RRDR60`, `checkABI()`, `abiStatus`, `RRDR60Error.abiMismatch(reason:)`, `ABIMismatch`), `ReRender.swift` on `.sized` and the lazy check, `swift/README.md` per research R-06 (import `RRDR60` for the helpers; the bare `RRDR60C`; the trap; the mismatch meaning; 64-bit; the `.iOS(.v16)` note; the last line amended); `Package.swift` untouched except a comment; `scripts/build-xcframework.sh && cd swift && swift test`; T011 – T012 green.

**Checkpoint**: the Swift suites green with `.sized` and `checkABI()`; the parity rows agree in three languages.

## Phase 5: Polish and cross-cutting

- [x] T014 [P] `crates/rr_dr60_harness/tests/abi_edge_cases.rs`, one test per spec edge case (seven): `size_zero_refused`, `size_above_cap_refused`, `refused_report_leaves_state`, `constant_data_tail_is_null_pointers` (a larger `RrDr60StageEntry`: the bytes beyond 80 zero, read as NULL `const char *`), `newer_caller_prefix_and_settings_rule` (a 4-byte-larger BlockInfo filled with the read-back 40; settings 124 refused), `thirty_two_bit_pointer_refused` (the check with pointer size 4), `single_element_writers_keep_bool_and_layout`.
- [x] T015 [P] `scripts/check-traceability.sh`: `check_file crates/rr_dr60_ffi/src/lib.rs` so the new literals (4096, 16, 40, 1, 37) are audited with their "spec 022" citations; run it clean.
- [x] T016 [P] The records from records.md: `docs/guide.md` § C / Swift (the lead "rebuild once and initialize every report", the migration block, the rule, the inputs, the frozen pair by mechanism, the check and the mismatch meaning, 64-bit with `wasm32` and 32-bit Android refused, the zero-initializer note, the `SIZED` example line with its comment, the promise with "planned"; the stale sentences in § controls and § playback speed; the record example's `flush_*`), `README.md`'s Platforms row, `CHANGELOG.md` under Changed with product M6's lead and the sizes table, `docs/product/decisions.md`'s row (D13 included), `docs/product/brief.md`'s distribution line, `specs/001-pipeline-skeleton/contracts/c-api.md` rules 2 and 5, `specs/018-battery-state/contracts/api.md` and `specs/020-session-contract/contracts/api.md` dated notes, `CLAUDE.md`'s Spec 022 reports commands; `cargo test -p rr_dr60_harness --test us2_cross_check` (the guide blocks, the manifest).
- [x] T017 Record the figures in research.md › Figures and restatements: the sizes as pinned, the smoke's three timing figures, the `SIZED` count, the Miri result (T007); run the release-only A/B modes solo as a no-regression check: `scripts/ab-timing.sh 81f14dd playback`, `… record`, `… bypass` (reported).
- [x] T018 The full gate: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `RUSTFLAGS="-D warnings" cargo build -p rr_dr60_ffi --release` (feature-less), the iOS target build, the header drift check, `cargo test --all-features` (count recorded), the MSRV build with `-D warnings`, `crates/rr_dr60_ffi/tests/c/run_smoke.sh`, the iOS simulator golden run, `cd swift && swift test`, `cargo llvm-cov --all-features --workspace --fail-under-lines 80` detached (within 1 point of 96.64 %); the fifteen guards, every smoke hash and the core trees unmoved (T002).
- [x] T019 The post-implementation reviews: `speckit-coach` and `hw-emulation-reviewer` on the branch (read-only); apply their findings at the defaults; re-run the gate.
- [x] T020 Push the branch, open the PR with the `ab-timing` label (the body from a scratch note: D1 – D13, the sizes table, the three edits, the figures, the re-bless of the manifest's version line), CI green on all 12 jobs plus `registers`; the CLAUDE.md status entry (figures, lessons); `/speckit-converge`; the owner merges; the post-merge records on main (spec status, the engine order: 022 done → c1 next, the memory bookmark).

## Dependencies

- Phase 1 → Phase 2 (T003 → T004 → T005 → T006 → T007, sequential: one file) → US1 (T008 ∥ T009 → T010) → US2 (T011 ∥ T012 → T013) → Polish (T014 ∥ T015 ∥ T016 → T017 → T018 → T019 → T020).
- US2 depends on US1 only for `examples/abi_rows.rs` (T009).

## Parallel examples

- T008 (the C smoke) and T009 (the Rust example and parity) touch different files.
- T014, T015, T016 are independent.

## Implementation strategy

MVP = Phase 2 + US1 (the C contract proven in Rust and C); US2 adds the Swift surface; Polish the records and the gate. One `feat!:` commit series on the branch; the PR squashes nothing (the house keeps the history).

## Phase 6: Convergence

- [x] T021 Record the nightly `miri` CI job (`.github/workflows/ci.yml`, the hardware review's post-implementation Q1 at its default) in `specs/022-abi-pass/spec.md`'s Clarifications as a dated restatement of SC-004's "one-off" Miri run and in `specs/022-abi-pass/records.md` per plan: the Miri gate (R-01, SC-004) (unrequested)
