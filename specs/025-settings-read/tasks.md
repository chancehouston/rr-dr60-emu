# Tasks: The Settings Read and Control Access by Index (spec 025)

**Input**: `spec.md` (D1 – D11 approved 2026-11-01), `plan.md` (P1 – P7 approved 2026-11-01), `research.md` (R-01 – R-10), `data-model.md`, `contracts/api.md`, `quickstart.md`, `records.md`.

**Cadence**: test-first per story; a checkpoint per story; the owner stops only at the merge (and on a failed check or a new decision).

## Phase 1: Setup

- [ ] T001 Version 0.91.0: `Cargo.toml` ×4 and `Cargo.lock`, `RR_DR60_VERSION_MINOR` 91 and its test in `crates/rr_dr60_ffi/src/lib.rs`, the header regenerated, the stage manifest's version line re-blessed (`RR_DR60_BLESS=manifest … canonical`, `RR_DR60_BLESS=readme … readme`), the Swift ABI fixture regenerated for the version line only.
- [ ] T002 The baseline note (scratchpad): the sixteen golden SHA guards, the smoke hashes, the test count (1354), coverage 96.73 % locally, the merge f5a0245.

## Phase 2: Foundational (the mapping)

- [ ] T003 `crates/rr_dr60/src/control_map.rs`, tests first in its `mod tests`: `targets_match_manifest_by_index` (identifiers equal for every index), `setting_variants_are_26`, `option_maps_match_manifest` (Tap, VasMode, Route, SpeedMode against `ControlOption.value`), `one_hot` (R-07: every settings index, an exhaustive destructure with literal identifiers, exactly one field changed), `representability` (types, options, finiteness and −0.0, the u8/u32/u64 widths: sensitivity 256 refused, 6 accepted), `setting_control_exhaustive` (every `error::Setting`), `mutants_change_what_they_name`; then the module: `ControlReading`, `const TARGETS`, `SettingAccess` (26) and `LiveControl` (5), `_exhaustive`, `settings_value` / `set_settings_value`, `setting_control`, the `__test-hooks` `MapMutation` and `__hooks::{settings_value_with, set_settings_value_with}`; `Error::{NotASetting, NotAPlainLiveControl}` in `error.rs`.
- [ ] T004 `crates/rr_dr60/src/pipeline.rs`: `control_value` (settings via `settings()`, the five via their getters) and `set_control_value` (R-04's direct conversions, the reachable statuses), core tests first (`control_value_defaults`, `set_control_value_statuses`, `set_control_value_neg_zero`).
- [ ] T005 `crates/rr_dr60/src/record.rs`: `get`/`set` on the mapping behind `SETTING_IDS`; a test that `seed`/`host_rate_hz`/`route` lines still raise the unknown-name flag; `setting_table_matches_the_manifest` restated; `golden_record` byte-identical.

**Checkpoint**: the core maps, reads and writes; every core test green; the record unmoved.

## Phase 3: User Story 1 - Read what is configured (P1)

- [ ] T006 [US1] `crates/rr_dr60_harness/tests/us1_settings_read.rs` (tests first): `us1_settings_read`, `us1_settings_read_landed`, `us1_trap_named`, `us1_committed_not_audible`, `us1_size_refused` at the measurement rates, through Rust and C.
- [ ] T007 [US1] `crates/rr_dr60_ffi/src/lib.rs`: `rr_dr60_get_settings` (R-03: `struct_size_at` equality, then `write_sized` with `fill_settings`), status order (R-06); `abi_tests` cases (the size refusals, painted buffer, poisoned handle).

**Checkpoint**: US1 green.

## Phase 4: User Story 2 - Every control by its index (P2)

- [ ] T008 [US2] `crates/rr_dr60_harness/tests/us2_control_index.rs` (tests first): `us2_defaults_by_index`, `us2_one_hot_c` (the painted 0xAA buffer, every byte outside the field kept), `us2_representability`, `us2_repair_by_index`, `us2_live_by_index` (the conversion and status rows; the log identity with the setter and a tag-1 schedule per stream at block size 1), `us2_class_refused`, `us2_device_as_modeled`, `us2_field_lookup`.
- [ ] T009 [US2] `crates/rr_dr60_ffi/src/lib.rs`: `RrDr60ControlReading` (24 bytes, const offsets, `READING_PUBLISHED = [24]`), `rr_dr60_control_value`, `rr_dr60_settings_control_value`, `rr_dr60_set_settings_control_value` (field-local unaligned raw-pointer access, switch bytes as u8), `rr_dr60_set_control_value`, `rr_dr60_setting_field_control` (raw u32), statuses 39 – 40 in `status_from_error`; the `#[cfg(test)]` whole-structure round-trip mutant; `abi_tests` (sizes, offsets, refusal matrix incl. poisoned, the switch byte 2 under Miri); the header regenerated; `cbindgen.toml` includes the new struct.

**Checkpoint**: US2 green.

## Phase 5: User Story 3 and polish

- [ ] T010 [P] `crates/rr_dr60_harness/tests/settings_read_edge_cases.rs` (the spec's edge cases, one per test); `tests/mutation.rs` `mod settings_read` (the mutants of R-07 with their named oracles); `tests/alloc_free.rs` `settings_read_allocate_nothing`; `tests/determinism.rs` `settings_read_same`; `tests/ffi_parity.rs` `settings_read_parity`; `tests/us2_abi.rs`'s unchanged constants and sizes (CONTROL_COUNT, MAX_CHANGES_PER_CALL, ABI_VERSION).
- [ ] T011 [P] The C smoke `--settings` (per index the identifier and changed byte offsets by `offsetof`; the reading's size and offsets; idle-cost prints) against `crates/rr_dr60_harness/examples/settings_rows.rs` (`offset_of!`); `run_smoke.sh`'s diff line; not in the promise script's mode loop.
- [ ] T012 [P] `swift/Sources/RRDR60/ControlReading.swift` (R-09) and `swift/Tests/RRDR60Tests/ControlReadingTests.swift`; `scripts/build-xcframework.sh && cd swift && swift test`.
- [ ] T013 `.github/workflows/ci.yml`: the second promise step (f5a02452728f3d0687c033689266a5bbdf88c6db) and the Miri filter; run both promise steps locally.
- [ ] T014 The records (records.md): the guide paragraph verbatim and a guide wording check in `us2_cross_check`, the C block re-blessed, the CHANGELOG, the README's C list, CLAUDE.md (status entry, reports commands), the traceability audit; the bench spec's pointer (ic-recorder-app).
- [ ] T015 The full gate: fmt, clippy, the feature-less `-D warnings` build, the iOS build, the header drift check, `cargo test --all-features` (count), the MSRV build, the C smoke, the simulator goldens, `swift test`, Miri, coverage (within 1 point of 96.73 %); the guards and smoke hashes unmoved (T002).
- [ ] T016 The post-implementation reviews (coach, hardware), applied at the defaults; the gate re-run.
- [ ] T017 The PR, CI green, `/speckit-converge`, the owner merges; the post-merge records; the memory bookmark; bench 001's plan resumes.

## Dependencies

Phase 1 → Phase 2 (T003 → T004 → T005) → US1 (T006 → T007) → US2 (T008 → T009) → Polish (T010 ∥ T011 ∥ T012 → T013 → T014 → T015 → T016 → T017).
