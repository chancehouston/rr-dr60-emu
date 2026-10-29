---

description: "Task list for the control manifest (014)"
---

# Tasks: Control Manifest

**Input**: Design documents from `/specs/014-control-manifest/` (spec.md, plan.md, research.md, data-model.md, contracts/api.md, quickstart.md)

**Tests**: required (constitution III; spec FR-007 – FR-010, FR-016, SC-003). Tests are written first in each story and fail before the implementation.

**Cadence** (owner, 2026-10-29): the phases run without stopping; stop only on a failed check or a new decision; the next owner stop is the merge.

## Format: `[ID] [P?] [Story] Description`

## Phase 1: Setup

- [x] T001 Bump the workspace to 0.14.0: `version = "0.14.0"` in `[workspace.package]` and the internal dependency entries of `Cargo.toml`; `RR_DR60_VERSION_MINOR = 14` in `crates/rr_dr60_ffi/src/lib.rs`; regenerate `crates/rr_dr60_ffi/include/rr_dr60.h` (see `crates/rr_dr60_ffi/cbindgen.toml`). Then re-bless the version line only (FR-014): `RR_DR60_BLESS=manifest cargo test -p rr_dr60_harness --test us2_cross_check canonical` and `RR_DR60_BLESS=readme cargo test -p rr_dr60_harness --test us2_cross_check readme -- --test-threads=1`; confirm with `git diff` that `crates/rr_dr60_harness/golden/manifest-v1.txt` changes only in `library_version: 0.14.0` and the README/guide regions only in the version. Commit `chore: bump to 0.14.0`.
- [x] T002 [P] Record the baseline in a scratch note: the audio golden SHA guards (`golden.rs`, `golden_agc.rs`, `golden_vas.rs`, `golden_codec.rs`, `golden_output.rs`, `golden_output_clamp.rs`, `golden_volume_ramp.rs`, `golden_mic.rs`, `golden_speed.rs`), `cargo llvm-cov` line coverage from the last CI run on `main` (0.13: 97.96 % local / CI per CLAUDE.md), and `cargo test --all-features` test count. **Recorded:** the nine audio golden files' SHA-256 in the scratch note; the last `main` CI run reports no total, so the baseline is CLAUDE.md's 97.96 % (spec 012, CI).

## Phase 2: Foundational (comment work, R-06)

- [x] T003 One comment-only commit (no code token changes; `cargo test --all-features` unchanged) in `crates/rr_dr60/src/settings.rs`, `output.rs`, `speed.rs`, per research R-06 › Comment work: A-002 → A-027 on `record_stage_enabled` and `playback_stage_enabled` in `Settings::new`, in the two field docs, in the test comments, and rename the test `defaults_cite_a002_and_fr009` → `defaults_cite_a027_and_fr009`; add the trailing-comment tags exactly as R-06 lists (`hiss_offset_db` A-048; `capsule_enabled` A-052; `Speaker` variant `// S-001, A-032, A-042`; `VOLUME_REFERENCE` S-002, A-044, A-045 beside A-043; `DEFAULT_LEVEL` `// S-001, S-012, A-054`; `SUPPORTED_HOST_RATES` A-026; `seed` A-049; `clamp_enabled` A-042; `capacity_s` A-038; `speech_codec_enabled` A-040, A-041; `sensitivity` A-022, A-033). Each default line's trailing comment then carries exactly data-model.md's IDs column for its entry. `scripts/check-traceability.sh` passes. Commit `docs(core): cite the current assumptions on the default lines (014 R-06)`.

**Checkpoint**: every default line's tags equal data-model.md's IDs column.

## Phase 3: User Story 1 - Build the app's controls from the engine (P1) 🎯 MVP

**Goal**: the 29 entries, readable in Rust, bound to the engine's ranges, defaults and timing.

**Independent Test**: `cargo test -p rr_dr60 controls::` and `cargo test -p rr_dr60_harness --test us1_controls` pass with no registers read.

### Tests first

- [x] T004 [P] [US1] Core tests in `crates/rr_dr60/src/controls.rs` `#[cfg(test)]` (research R-03 › Core; written first in a scratch note, pasted in T006 so they are red first): `missing_or_extra(entries)` with an exhaustive destructuring of `Settings` and every sub-structure (no `..`), the mapped identifiers plus `route`, `volume`, `playback_speed` equal to the table's 29 exactly, plus a doctored-table case that drops one entry and asserts the named failure (SC-003 "missing entry"); `ranges_equal_setting_range` for every continuous and integer entry against `Setting::range()` and `MEMORY_*_RANGE`; `defaults_at_every_rate` against `Settings::new(r)` for all `SUPPORTED_HOST_RATES`, `output::VOLUME_REFERENCE`, `speed::DEFAULT_LEVEL`, `Route::default()`; `enum_options_exhaustive` (an exhaustive `match` on every `Tap`, `VasMode`, `Route` variant mapping to its option identifier and C value; `AFTER_STAGE_10` not listed); `c_twins_equal_strings_plus_nul`; `form_rules_called_at_runtime` (call the const form-rule functions so `llvm-cov` counts them).
- [x] T005 [P] [US1] Create `crates/rr_dr60_harness/tests/us1_controls.rs` and the check functions it needs in `crates/rr_dr60_harness/src/controls_checks.rs` (research R-03, R-07, R-09; FR-008, FR-012, FR-016): `twenty_nine_entries_in_order` (live controls first, then the `pub <field>:` order of `Settings` and sub-structures read from `crates/rr_dr60/src/settings.rs`); `edges` through `Settings::validate` and the live setters with R-03's just-outside rule (f32 bits ±1; at a 0.0 bound `f32::from_bits(0x8000_0001)` below and `from_bits(1)` above; integers ∓ 1; host rate each option ± 1 and 0; never construct a pipeline at `memory.capacity_s` = 86 400); `defaults` at all seven rates; `units` against `Setting`'s display text; `timing_behavior` (each live setter succeeds while recording and while playing; an idle change through `reconfigure` is refused while recording or playing; a memory-dependent change is refused with recordings present that do not fit); `read_on_other_thread_while_recording` (US1 AS4); `canonical` (render with `render_controls`, compare with `crates/rr_dr60_harness/golden/control-manifest-v1.txt`, bless with `RR_DR60_BLESS=control_manifest`); `denylist_on_names_and_descriptions` (reuse `manifest_checks`' denylist). Fails to compile until T006.

### Implementation

- [x] T006 [US1] Create `crates/rr_dr60/src/controls.rs` (data-model.md, contracts/api.md, research R-01, R-02): the types `ControlEntry`, `ControlKind { DeviceControl = 0, UnitCondition = 1, ModelInternal = 2, EmulatorOption = 3, HostParameter = 4 }`, `ControlTiming { Live = 0, Idle = 1, IdleMemoryDependent = 2, HostFixed = 3 }`, `ControlPaths` (bits `RECORD = 1`, `PLAYBACK = 2`, `ONE_PASS = 4`), `ControlValue`, `ControlOption`, enums `#[non_exhaustive]` `#[repr(u32)]`; `pub const CONTROL_COUNT: usize = 29; // FR-001`; `controls()`, `control(identifier)`; the `control_table!` macro emitting the table and the hidden `__c` twin; the 29 entries exactly as data-model.md's table (identifiers, names "≤ 40 chars", descriptions "one sentence, ≤ 160 chars, ends with '.', no digits", kinds, timing, paths, stages, values, units, IDs ascending); const assertions for the form rules ("identifier ASCII `[a-z0-9_.]`, ≤ 32 chars, unique"; a choice's default among its options; `min ≤ default ≤ max`); module docs with the claims-policy sentence and the four timing classes. Paste T004's tests. `pub mod controls;` and re-exports in `crates/rr_dr60/src/lib.rs`.
- [x] T007 [US1] Add `crates/rr_dr60_harness/examples/control_manifest.rs` printing `render_controls()`; bless the golden once (`RR_DR60_BLESS=control_manifest cargo test -p rr_dr60_harness --test us1_controls canonical`); extend `crates/rr_dr60_harness/tests/determinism.rs` to compare the canonical text with the golden and `crates/rr_dr60_harness/tests/alloc_free.rs` to read every entry with zero allocations.
- [x] T008 [US1] Run `cargo test -p rr_dr60 controls::` and `cargo test -p rr_dr60_harness --test us1_controls --test determinism --test alloc_free`; record each binary's test count (each > 0) in this file. **Recorded:** `controls::` 8; `us1_controls` 9; `determinism control_manifest` 1; `alloc_free` 1 (release).

**Checkpoint**: US1 green; the canonical golden blessed.

## Phase 4: User Story 2 - The control manifest cannot drift (P2)

**Goal**: drift against the stage manifest, the registers, the C header and the setters is caught and named.

**Independent Test**: `cargo test -p rr_dr60_harness --test us2_controls` passes on the committed tree and on each doctored case fails with the named entry.

- [ ] T009 [P] [US2] Create `crates/rr_dr60_harness/tests/us2_controls.rs` (SC-003's six harness cases plus the committed-tree passes) and add to `controls_checks.rs`: `notes(entries, manifest)` (spec FR-009 tokenizer exactly; research R-05's expected results asserted per stage); `setters(entries, sources, header)` (R-04's two regexes; every setter is a live entry, every live entry has a setter in both); `options_vs_header(entries, header_text)` (`RR_DR60_<TAP|VAS_MODE|ROUTE>_<IDENTIFIER uppercased> = <value>`); `ids(entries)` (R-06 reader: trailing `//` comment on the default line, scoped to its `impl XSettings { pub const DEVICE` block or `Settings::new`, plus `VOLUME_REFERENCE`, `DEFAULT_LEVEL`, the `Speaker` variant, `SUPPORTED_HOST_RATES`; lists equal); `strings(entries)` with R-10's kind rules ("a device control cites S-001 or S-002"; "a unit condition or model internal cites at least one A-###"). Doctored cases: a moved bound, a changed default, a superseded ID, a denylisted word, an entry whose stage disagrees with the notes, a setter without an entry (a doctored source text).
- [ ] T010 [US2] Extend `crates/rr_dr60_harness/src/manifest_checks.rs` and `crates/rr_dr60_harness/tests/us2_cross_check.rs` so the register cross-check covers the control manifest's IDs: each exists and is not superseded (FR-010); a doctored register with one cited ID superseded fails naming the control.
- [ ] T011 [US2] Extend `scripts/check-traceability.sh` to scan `crates/rr_dr60/src/controls.rs` outside `#[cfg(test)]` like `manifest.rs` (FR-011); confirm `registers.yml` path filters include `controls.rs`.
- [ ] T012 [US2] Run `cargo test -p rr_dr60_harness --test us2_controls --test us2_cross_check` and `scripts/check-traceability.sh`; record test counts.

**Checkpoint**: 7 of 7 doctored inputs caught (SC-003).

## Phase 5: User Story 3 - The same from C (P3)

**Goal**: the manifest through the C API, byte-identical canonical text.

**Independent Test**: `ffi_parity` and the C smoke `--controls` diff pass.

- [ ] T013 [P] [US3] Tests first in `crates/rr_dr60_harness/tests/ffi_parity.rs` and the FFI unit tests: every field of every entry, option and ID equal to Rust; NULL out → `RR_DR60_STATUS_NULL_POINTER` (checked first); index 29, option past `option_count`, ID past its count → `RR_DR60_STATUS_NO_SUCH_CONTROL` with nothing written; through `rr_dr60_settings_validate`, a tap, VAS mode or route one past the last C value → `INVALID_ARGUMENT` and `vas_sensitivity` = 261 → `INVALID_SETTING` (R-03).
- [ ] T014 [US3] Implement in `crates/rr_dr60_ffi/src/lib.rs` (contracts/api.md, R-08): `RrDr60Status::NoSuchControl = 26`; `RrDr60Control` with `reserved_014` and `offset_of` const assertions, `RrDr60ControlOption`, the kind/timing/value-type enums, the `RR_DR60_CONTROL_PATH_*` constants; `rr_dr60_control_count`, `rr_dr60_control`, `rr_dr60_control_option`, `rr_dr60_control_source`, `rr_dr60_control_assumption`, panic-guarded, NULL before range, `// SAFETY:` on every `unsafe`; regenerate the header; header drift check clean.
- [ ] T015 [US3] Extend `crates/rr_dr60_ffi/tests/c/smoke.c` with `--controls` printing the canonical text (f32 via `%.*f` with the fewest decimals 0 – 9 whose `strtof` equals the value) and `crates/rr_dr60_ffi/tests/c/run_smoke.sh` with a byte `diff` against the example's output; extend the `alloc_free` C-function checks.
- [ ] T016 [US3] Run `cargo test -p rr_dr60_ffi`, `cargo test -p rr_dr60_harness --test ffi_parity --test alloc_free`, `crates/rr_dr60_ffi/tests/c/run_smoke.sh`; record counts.

## Phase 6: Polish

- [ ] T017 [P] Documentation (FR-015): `docs/guide.md` gains a "Control manifest" section with the Rust and C reading examples, the four kinds and four timing classes, the free/Workbench rule and the presets rule and the unit-condition rule (research R-11), the stage-number join; `CHANGELOG.md` 0.14.0 entry (the feature, the comment work with A-002 → A-027, the version-line re-bless, the two new C structs on the pre-1.0 ABI-pass list); `docs/product/controls-discussion.md` section 1 points to the manifest as the current inventory; the README's "What exists today" line; `CLAUDE.md` status entry and commands.
- [ ] T018 [P] Swift package parity: rebuild `scripts/build-xcframework.sh` and run `cd swift && swift test` (no Swift change expected; confirms the header still builds).
- [ ] T019 Full gate: `cargo fmt --all -- --check`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo test --all-features`; the MSRV 1.85 build with `RUSTFLAGS="-D warnings"`; `scripts/check-traceability.sh`; the header drift check; every audio golden SHA guard unchanged (T002); `cargo llvm-cov --all-features --workspace --fail-under-lines 80` within 1 point of T002's baseline.
- [ ] T020 Reviews of the landed branch: coach (process, requirement coverage) and hardware-emulation reviewer (strings and IDs against the code); apply findings; stop only for a new decision.
- [ ] T021 Push the branch, open a PR, CI green on all jobs; then `/speckit-converge`; the owner merges.

## Dependencies

- T001 → everything (version). T003 before T009 (`ids` reads the comments) and before T006 (the table's IDs equal the comments).
- US1 (T004 – T008) → US2 (T009 – T012) and US3 (T013 – T016); US2 and US3 are independent of each other.
- Polish after all stories.

## Parallel opportunities

- T002 with T001. T004 with T005 (different files). After US1: US2 and US3 in parallel. T017 with T018.

## Implementation strategy

MVP = Phase 1 – 3 (US1): the table readable from Rust with its oracles. Then US2 (drift guards), US3 (C), Polish. Commit after each task or logical group, Conventional Commits.
