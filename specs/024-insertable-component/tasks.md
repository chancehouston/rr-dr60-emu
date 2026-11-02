---

description: "Task list for the insertable component slot and the band filter (024)"
---

# Tasks: An Insertable Component Slot and the Band Filter

**Input**: Design documents from `/specs/024-insertable-component/` (spec.md D1 – D12 with the plan reviews' restatements, plan.md P1 – P11 and R1 – R7, research.md R-01 – R-11 and the six mutants, data-model.md, contracts/api.md, quickstart.md, records.md)

**Tests**: required (constitution III; spec FR-001 – FR-013, SC-001 – SC-006). Tests are written first in each story and fail before the implementation.

**Cadence**: the owner is away ("run this on auto-pilot to the end", 2026-11-01): the phases run without stopping, every open choice at its recommended default, recorded in plan.md › R1 – R7; the plan's sign-off and the merge are the owner's.

## Format: `[ID] [P?] [Story] Description`

## Phase 1: Setup

- [x] T001 Bump the workspace to 0.92.0 (`Cargo.toml` ×4, `RR_DR60_VERSION_MINOR` 92 and its test in `crates/rr_dr60_ffi/src/lib.rs`, the header regenerated, the Swift ABI fixture regenerated, the stage manifest's version line re-blessed: `RR_DR60_BLESS=manifest cargo test -p rr_dr60_harness --test us2_cross_check canonical`, then `RR_DR60_BLESS=readme … readme -- --test-threads=1`).
- [x] T002 [P] Record the baseline in a scratch note: the sixteen golden SHA guards (fifteen audio + the record golden), every smoke hash in `crates/rr_dr60_ffi/tests/c/smoke.c`, the 1409-test count at feec054, coverage 96.69 % locally (0.91), the A/B baseline feec054, `MAX_CHANGES_PER_CALL` 142, the attained 142 / 64 / 69 (one-pass / recorder / player). The fifteen audio guards and every smoke hash must be unmoved at the end (FR-002, SC-003; the record golden moves once, plan R6).
- [x] T003 [P] The design script `tools/band-filter/design_band_filter.py` (research R-07): the section of R-02 in numpy f64 with the f32-narrowed parameter values; for the four types at the defaults and at the golden's parameter sets at the seven rates: the response at 20 Hz … 0.45 × rate (third-octave centres plus the corner), DC and Nyquist, the coefficients, the pole-radius impulse length `L = ⌈2·ln(1e8)/(−ln a2)⌉ + 0.1·rate`, the cross-rate table for corners 1 – 2.5 kHz; `--check` (the structural zeros exact, the unity points within 1e-10 in the coefficients, the impulse lengths); `--write` regenerates `crates/rr_dr60_harness/src/band_filter_ref.rs` byte-identical (the header citing research R-07 and the engineering targets).

## Phase 2: Foundational (the section, the slot, the store, the control state, the record)

- [x] T004 `crates/rr_dr60/src/error.rs`: `Error::{NoSuchInsertionPoint { point: u32 }, NoSuchComponent { component: u32 }, ComponentWhileActive}` with Display texts, statuses 41 / 42 / 43 in the C mapping; `crates/rr_dr60/src/settings.rs`: `Tap::AfterInsert` appended (C 11), `is_front` includes it; `crates/rr_dr60/src/manifest.rs`'s exhaustive `Tap` → stage match gains the tap (no stage; a dated restatement).
- [x] T005 `crates/rr_dr60/src/stages/band_filter.rs`, tests first in its `mod tests`: `structural_zeros_exact` (low at Nyquist, high at DC, band at both, from the coefficients), `unity_points` (within 1e-10), `mixes_are_constant_triples` (the four triples in {−1, 0, 1} on `(x, k·v1, v2)`), `blend_equals_two_sections` (two sections fed the same input and coefficients, state equal bit for bit; the blend equals `T_old·s + w·(T_new·s − T_old·s)`), `ramp_trajectory` (n = 1 at P, the target exact at P + N − 1, log of the applied frequency, log-Q, dB gain; within 1e-9), `ramp_clamped_endpoints` (both endpoints at min(f, 0.45 r); a move wholly above the ceiling flat), `second_landing_restarts_from_current`, `type_landing_during_blend_restarts` (freezes the current mix, restarts at 0; back to the origin reverses; the same type is a no-op), `flush_below_1e_30`, `recompute_only_while_ramping`; then the section (R-02: `g = detmath::sin/cos`, `a1, a2, a3`, the state update, the flush, the post-gain), the cached coefficients, three ramps, the blend (spec 021's smoothstep with selected ends), every literal tagged `engineering target (024 Dn)`.
- [x] T006 `crates/rr_dr60/src/stages/insert.rs`: `Insert { component, filter, n_transition, ceiling_hz, … }` (`new(rate, &store)`, `process(x) -> f64`, `set_component(c)` clearing the state and ending the ramps, `land(param, value, at_once)`, `reset()`), the `__test-hooks` `InsertMutation` (`param_step`, `type_instant`, `freq_linear`, `restart_from_target`, `state_kept_on_set`, `skip_store_mirror`; checks inside `Insert::process` only), `insert_trace` (the applied trajectory, 8192 reserved) and `insert_input_trace` (the slot's f64 input, reserved at construction).
- [x] T007 `crates/rr_dr60/src/components.rs` (tests first: `manifest_fields`, `indexed_enumeration`, `budget_offsets_no_overlap` (offsets + counts ≤ 16, no overlap), `control_indices_match_control_manifest`, `ceiling_function`): `InsertionPoint` (with `control_index` 31), `Component` (`value`, `parameter_offset`, indexed 0 … count − 1), `ComponentParameter` (`scale`, `control_index`), `insertion_points()`, `components()`, `COMPONENT_PARAMETER_BUDGET` 16, `band_filter::{FilterType, frequency_ceiling_hz}`; the sentences of research R-04 ("an addition of the emulator"); `lib.rs` exports.
- [x] T008 `crates/rr_dr60/src/changes.rs` (tests first: `max_changes_per_call_is_158_by_derivation`, `record_log_is_88`, `component_slots_map_32_to_35`, `begin_call_compares_the_store`, `new_and_adopt_seed_from_store_no_entry`): `RecordCtl` widened to 24 slots (masks `u32`, `pending_*`/`landed_tag` `[_; 24]`, `last_component: [f32; 16]`), `LIVE_COMPONENT`, `slot_of` mapping 32 – 35 → 8 – 11, `apply_due` over 0 … 24, `take_landed` returning the component values, `new(settings, &store)`, `adopt`, `begin_call(settings, &store, &tags24)`, `RECORD_LOG` 88, `MAX_CHANGES_PER_CALL` = 2·64 + 3 + 8 + `COMPONENT_PARAMETER_BUDGET` + 3; the earlier pins of research R-11 restated with dated comments (`changes.rs` 142 → 158, 72 → 88).
- [x] T009 `crates/rr_dr60/src/paths.rs` (tests first: `layout_offsets_unchanged` (`offset_of!(RecordPath, down)`, `ctl`), `empty_slot_no_arithmetic` (−0.0 kept), `front_tap_after_insert`, `reset_clears_insert`): `run_insert` in the padding after `run_limiter`, `insert: Insert` after `ctl`, `front_from` calling the slot between stage 2 and the limiter-input peak, `FrontStop::Insert`, `RecordPath::reset` → `Insert::reset`, `apply_due_cold` routing slots 8 – 23 to `Insert::land` (at once on the stream's first sample or with the slot empty), `adopt(settings, &store)`.
- [x] T010 `crates/rr_dr60/src/pipeline.rs` and `transport.rs` (tests first: `set_component_idle_only`, `set_component_kept_by_reset_and_reconfigure`, `set_component_parameter_idle_no_entry`, `component_parameter_value_reads_store`, `store_mirror_with_tag`, `reconfigure_moved_store_no_entry`, `start_recording_clears_filter_state`): `ComponentStore { component, values, tags }` in `Pipeline`; `set_component`, `component`, `set_component_parameter`, `component_parameter_value`; both record paths built and adopted from the store; the call epilogue writing landed component values and tags into the store; `schedule_change`'s validation for 32 – 35 (f32-narrowed range, the type integral, the playback stream `ControlNotOnStream`); `start_recording` adopt; `reconfigure` re-applying the ceiling.
- [x] T011 `crates/rr_dr60/src/controls.rs` and `control_map.rs` (tests first in `us1_controls`/`us2_controls` and the core: the five entries, the tap option, the component target, the 26 + 5 + 5 closure): entries 31 – 35 from the component manifest's constants, the `tap` entry's `after_insert` option, `CONTROL_COUNT` 36; `Target::Component` (`control_value` 31 – 35, `set_control_value(31)` → `set_component`, 32 – 35 → `set_component_parameter`, `settings_control_value` → `NotASetting`); re-bless `RR_DR60_BLESS=control_manifest cargo test -p rr_dr60_harness --test us1_controls canonical`; the 31-control pins restated (us1_controls, us2_pitch, us2_abi).
- [x] T012 `crates/rr_dr60/src/memory.rs` and `record.rs` (tests first: `form_2_round_trip`, `reads_form_1_as_empty_slot`, `unknown_component_flagged`, `unknown_parameter_counted`): `SlotBlock::{component, store}`, `FORM_VERSION` 2, the writer's component lines (every component's parameters), the reader (1 and 2), `flags::UNKNOWN_COMPONENT`, `RecordReadout::{component, store}`; the record re-render recipe (`crates/rr_dr60_harness/src/record_contract_checks.rs` and `examples/re_render.rs`) setting the component and the store before `start_recording`.

**Checkpoint**: the core sets, filters, ramps, blends, logs, mirrors and records; every core pin green; every earlier binary green except the record golden (re-blessed in T019).

## Phase 3: User Story 1 - A filter in the chain (P1)

**Independent test**: `cargo test -p rr_dr60_harness --test us1_insert -- --nocapture`.

### Tests first (red until T016)

- [x] T013 [P] [US1] `crates/rr_dr60_harness/src/insert_checks.rs`: the oracle configuration (stages 1 – 2 and the limiter bypassed, the tap `AfterInsert`), the reference section (detmath, the engine's operation order, the flush) on `insert_input_trace`, the DTFT response reader with research R-07's tolerance and the pole-radius impulse length, the placement identity, the B1 row (state-variable vs transposed direct form II, the stated metric), the click reader (spec 011's, the `param_step` reference, the floor printed), the type-blend row against `type_instant`, the meters' limiter-input peak oracle, the loud row, the gate bisection (release-only), the re-render cells incl. the second recording.
- [x] T014 [P] [US1] `crates/rr_dr60_harness/tests/us1_insert.rs` (US1 AS1 – AS5 at `configs::measurement_rates()`): `insert_as1_response_and_placement` (the four types, the zeros ≤ −120 dB, unity within 1e-6 dB, the placement identity bit-exact), `insert_as2_transitions_and_clicks` (the trace within 1e-9, the entry at P with its tag, ≥ 30 dB on ±12 dB gain steps and one-octave moves at Q 0.7071 with the floor, Q 10, full range, type blend and the B1 row reported), `insert_as3_empty_slot_and_toggle` (bit-identical to the empty twin; filter → none → filter; the re-render cells and named deltas), `insert_as4_lifecycle` (refused while active, no allocation, kept by reset/reconfigure, the store in force), `insert_as5_loud_and_gate` (gain reduction > 0, the clamp count > 0, the twin at 0; the limiter-input peak equals the slot's output peak), `insert_latency_unchanged`. Expected: 6 tests.
- [x] T015 [P] [US1] `crates/rr_dr60_harness/tests/insert_matrix.rs` (seven rates: every type and parameter set, the placement identity, the partitions at 1 / 64 / 4096 / irregular, the cross-rate table, the slot block's growth, `the_tight_bounds` (one-pass 146 and its record side 76 of 88, recorder 64, player 69, the 142-entry buffer), `gate_offset_bisection` ignored/release). Expected: 7 tests (1 ignored).

### Implementation

- [x] T016 [US1] Make T013 – T015 green; any restatement dated in spec.md's Clarifications; the reviewers' figures reproduced before any record quotes them.

**Checkpoint**: US1 green at the measurement rates; the figures printed.

## Phase 4: User Story 2 - The engine describes the slot (P2)

**Independent test**: `cargo test -p rr_dr60_harness --test us2_insert -- --nocapture`.

### Tests first (red until T020)

- [ ] T017 [P] [US2] `crates/rr_dr60_harness/tests/us2_insert.rs` (US2 AS1 – AS5): `insert_as1_manifests` (both ways against validation and the control manifest), `insert_as2_control_manifest_and_158` (the entries, 158 by derivation, a 142-entry buffer, every smoke mode's change count ≤ 142), `insert_as3_record_v2` (round trip, form 1 read, the deltas, the unknown component refused), `insert_as4_c_parity_pointer` (points at `ffi_parity insert` and the promise script), `insert_as5_refusals_and_ceiling` (the playback stream, out of range, the ceiling accepted and applied). Expected: 5 tests.
- [ ] T018 [P] [US2] The C surface in `crates/rr_dr60_ffi/src/lib.rs`: the functions, defines and statuses of contracts/api.md, the four sized structs (`RrDr60InsertionPoint`, `RrDr60Component`, `RrDr60ComponentParameter`, `RrDr60RecordComponent`) under `write_sized`, `rr_dr60_processing_record_component`, `rr_dr60_record_write_with_component`, the control map's C paths; `cbindgen.toml` `include`; the header regenerated; `abi_tests` (sizes, offsets, the painted cases, Miri on the four fillers); `tests/ffi_parity.rs` `insert_parity` (a ±0.125 square, the filter in the slot and a scheduled change, bit for bit; the statuses; the structs; the value read; the ceiling); the C smoke `smoke --insert` against `crates/rr_dr60_harness/examples/insert_rows.rs`; `run_smoke.sh`'s diff line; Swift: `RRDR60Sized` on the new structs, `ProcessingRecord.reRender` reading `RrDr60RecordComponent` and calling the setters, a Swift test.
- [ ] T019 [P] [US2] `crates/rr_dr60_harness/tests/golden_insert.rs` with `golden.rs`'s `InsertGolden` (research R-08's 84 entries; the bless-time assertions: the empty-slot twin, every change landing before the end; `RR_DR60_BLESS=insert`; ci.yml's golden lines); the record golden re-blessed once (`RR_DR60_BLESS=record`), its SHA moved with a dated comment in `golden_record.rs` and `us2_abi::us3_guards_unmoved`. Expected: 2 tests.

### Implementation

- [ ] T020 [US2] Make T017 – T019 green; bless the insert golden once after the matrix is green; record the SHAs in T002's note.

**Checkpoint**: US2 green; the golden blessed once; the fifteen audio guards unmoved.

## Phase 5: User Story 3 - Nothing else moved (P3)

- [ ] T021 [P] [US3] `crates/rr_dr60_harness/tests/insert_edge_cases.rs` (the spec's nine edge cases, one test each, every name containing "insert"); `tests/mutation.rs` `mod insert` (the six mutants with their named oracles); `tests/alloc_free.rs` `insert_allocate_nothing`; `tests/determinism.rs` `insert_same_calls_same_log`.
- [ ] T022 [P] [US3] `crates/rr_dr60_harness/tests/insert_timing.rs` (release-only: the filter's ns per host sample at 48 kHz, a transition every 200 ms, the worst 64-sample block with three ramps at 96 kHz asserted within real time); `scripts/ab-timing.sh <rev> insert` (the record path and the one-pass with the slot empty, 3 %, `RR_DR60_INSERT_COST_CEILING` 0.05) and its step in `.github/workflows/ab-timing.yml`; `configs::measurement_rates()` under `cfg(coverage)`.
- [ ] T023 [US3] Run the release-only timing solo and `scripts/ab-timing.sh feec054 insert`; record the figures in research.md › Figures to measure.

## Phase 6: Polish and cross-cutting

- [ ] T024 [P] ci.yml: the third promise step `scripts/abi-promise.sh 97bbdc24e2f358f6c8d89c75f5472a62985cf276`; run all three promise baselines locally.
- [ ] T025 The records from records.md (the guide's new section, the chain diagram, § Changes, § Settings read, the meters' limiter-input peak wording; README; CHANGELOG; decisions.md's row; the product notes by section; signal-chain.md's note; CLAUDE.md's Spec 024 reports and status entry); `scripts/check-traceability.sh` clean; `us2_cross_check` green (the denylist on every new sentence).
- [ ] T026 Run every manifest-pinning binary (us1_controls, us2_controls, us2_cross_check, us2_abi, us2_pitch, us2_bypass, bypass_matrix, pitch_matrix, ffi_parity), then the full gate: fmt, clippy, the feature-less `-D warnings` build, the iOS build, the header drift check, `cargo test --all-features` (count recorded), the MSRV build, the C smoke, Miri, the simulator goldens, `swift test`, coverage detached (within 1 point of 96.69 %); the fifteen audio guards and every earlier smoke hash unmoved (T002).
- [ ] T027 The post-implementation reviews: `speckit-coach` and `hw-emulation-reviewer` on the branch (read-only); apply their findings at the defaults; re-run the gate.
- [ ] T028 Push the branch, open the PR with the `ab-timing` label (the body: D1 – D12, R1 – R7 for the owner, the figures, the re-blesses), CI green; `/speckit-converge`; the owner merges; the post-merge records (spec status, CLAUDE.md, the memory bookmark).

## Dependencies

- Phase 1 (T001; T002 ∥ T003) → Phase 2 (T004 → T005 → T006 → T007 → T008 → T009 → T010 → T011 → T012) → US1 (T013 ∥ T014 ∥ T015 → T016) → US2 (T017 ∥ T018 ∥ T019 → T020) → US3 (T021 ∥ T022 → T023) → Polish (T024 ∥ T025 → T026 → T027 → T028).
- T003 (the script) precedes T013's oracle; T006's hooks precede T013's traces.

## Implementation strategy

MVP = Phase 2 + US1 (the slot and the filter pinned to the script); US2 adds the C surface, the record and the golden; US3 the cost and the edge cases; Polish the records and the gate.
