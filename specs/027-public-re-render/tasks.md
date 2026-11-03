---
description: "Tasks for engine feature 027, the public re-render"
---

# Tasks: The public re-render

Test-first. The traceability table is in plan.md; the build order is plan.md's.

## Phase 1: D7, the fill

- [X] T001 [US4] Test `us4_surfaces::as3_fill` (failing): `Pipeline::fill_to_remaining` against the harness's `fill_to_remaining` on both encodings (equal remaining, equal digest of a recording made afterwards), a codec-on target of 120·k + 7 (floored), two different fillers one digest, 0 units no item, the refusals (recording, playing, fuller, no slot).
- [X] T002 [US4] `memory::fill_item(units)` (an import-shaped item: zeros, or one encoder frame for silence from rest repeated) and `Pipeline::fill_to_remaining` (public, idle only) in `crates/rr_dr60/src/{memory.rs, transport.rs}`.

## Phase 2: D4, the plan

- [ ] T003 [US2] Tests `us2_rebuild::as3_refusals` and core tests (failing): reads, edits, the refusals (host rate, a row's control and stream, a 65th row at one position, an unknown component, imported origin, remaining 0 or above the capacity), `edited()` by field compare incl. −0.0.
- [ ] T004 [US2] `rr_dr60::rerender::Plan` in `crates/rr_dr60/src/rerender.rs` (R-02).

## Phase 3: D1, D2, D5, D6, the session

- [X] T005 [US1] Tests `us1_rerender::{as1_sixteen_cells, as2_slot_and_form_1, as3_partitions, as4_differs, as5_dense_rows, as6_record_bytes}` (failing; the dense-row cell first shown failing on the harness recipe at block 4096).
- [X] T006 [US1] `ReRender::{new, feed, feed_f32, progress, meters, finish}`, `Fed`, `Outcome`, `Verdict`, `Reason`, the export/record/into_pipeline reads, `re_render` (R-03 – R-09).
- [ ] T007 [US2] Tests `us2_rebuild::{as1_equals_direct, as2_record_round_trip, as4_self_stop_both_ways, as5_unreached_edit, as6_empty}`; fixes as they show.
- [ ] T008 [US3] Tests `us3_stream::{as1_alloc_free, as3_cancel, as4_input_short, as5_feed_edges}` and the release `as2_hour`; `alloc_free rerender`, `determinism rerender`.

## Phase 4: FR-010, the mutants

- [ ] T009 `RerenderMutation::{no_split, stop_after_self_stop, fill_rounded_up, store_skipped, i24_scale, drop_event}` behind `__test-hooks`; `mutation rerender::` shows each failing its test.

## Phase 5: D8, C and Swift

- [ ] T010 C: `crates/rr_dr60_ffi/src/rerender.rs` (the two handles, R-10), the statuses, `RrDr60ReRenderFed` / `RrDr60ReRenderOutcome` with published offsets, `rr_dr60_fill_to_remaining`; `abi_tests rerender`, Miri, `ffi_parity rerender`, the header; `smoke --rerender` against `examples/rerender_rows.rs`.
- [ ] T011 Swift: `ReRenderSession`, `ReRenderResult`, the rewritten `ProcessingRecord.reRender` with cancellation; the old recipe deleted; `us4_surfaces::as2_no_swift_recipe`; `swift test`, the simulator.

## Phase 6: records and the gate

- [ ] T012 `ci.yml`'s fifth promise step (3890bfe, full id) and the Miri filter; version 0.94.0 (Cargo ×4, `RR_DR60_VERSION_MINOR`, header, the stage and control manifests' version lines, the guide's copied blocks, the Swift fixture); the guide section with the five sentences and `us2_cross_check guide_rerender`; the CHANGELOG; the README; the real-time lists; the golden guard count restated from `us2_abi`.
- [ ] T013 The full gate (fmt, clippy, tests, the feature-less ffi build with `-D warnings`, llvm-cov, MSRV, the C smoke, the five promises, `scripts/ab-timing.sh 3890bfe record`, the Swift package, the simulator goldens); the post-implementation reviews (coach, hardware).
