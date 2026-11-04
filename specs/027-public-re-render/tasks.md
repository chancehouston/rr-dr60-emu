---
description: "Tasks for engine feature 027, the public re-render"
---

# Tasks: The public re-render

Test-first. The traceability table is in plan.md; the build order is plan.md's.

## Phase 1: D7, the fill

- [X] T001 [US4] Test `us4_surfaces::as3_rerender_fill` (failing): `Pipeline::fill_to_remaining` against the harness's `fill_to_remaining` on both encodings (equal remaining, equal digest of a recording made afterwards), a codec-on target of 120·k + 7 (floored), two different fillers one digest, 0 units no item, the refusals (recording, playing, fuller, no slot).
- [X] T002 [US4] `memory::fill_item(units)` (an import-shaped item: zeros, or one encoder frame for silence from rest repeated) and `Pipeline::fill_to_remaining` (public, idle only) in `crates/rr_dr60/src/{memory.rs, transport.rs}`.

## Phase 2: D4, the plan

- [X] T003 [US2] Tests `us2_rebuild::as3_rerender_refusals` and core tests (failing): reads, edits, the refusals (host rate, a row's control and stream, a 65th row at one position, an unknown component, imported origin, remaining 0 or above the capacity), `edited()` by field compare incl. −0.0.
- [X] T004 [US2] `rr_dr60::rerender::Plan` in `crates/rr_dr60/src/rerender.rs` (R-02).

## Phase 3: D1, D2, D5, D6, the session

- [X] T005 [US1] Tests `us1_rerender::{as1_rerender_sixteen_cells, as2_rerender_slot_and_form_1, as3_rerender_partitions, as4_rerender_differs, as5_rerender_dense_rows, as6_rerender_record_bytes}` (failing; the dense-row cell first shown failing on the harness recipe at block 4096).
- [X] T006 [US1] `ReRender::{new, feed, feed_f32, progress, meters, finish}`, `Fed`, `Outcome`, `Verdict`, `Reason`, the export/record/into_pipeline reads, `re_render` (R-03 – R-09).
- [X] T007 [US2] Tests `us2_rebuild::{as1_rerender_equals_direct_and_as2_round_trip, as1_rerender_equals_direct_and_as2_round_trip, as4_rerender_self_stop_both_ways, as5_rerender_unreached_edit, as6_rerender_empty}`; fixes as they show.
- [X] T008 [US3] Tests `us3_stream::{as1_rerender_meters_and_footprint, as3_rerender_cancel_and_progress, as4_rerender_input_short, as5_rerender_feed_edges}` and the release `as2_rerender_hour`; `alloc_free rerender`, `determinism rerender`.

## Phase 4: FR-010, the mutants

- [X] T009 `RerenderMutation::{no_split, self_stop_fields_dropped, fill_rounded_up, store_skipped, i24_scale, drop_event}` behind `__test-hooks`; `mutation rerender::` shows each failing its test.

## Phase 5: D8, C and Swift

- [X] T010 C: `crates/rr_dr60_ffi/src/rerender.rs` (the two handles, R-10), the statuses, `RrDr60ReRenderFed` / `RrDr60ReRenderOutcome` with published offsets, `rr_dr60_fill_to_remaining`; `abi_tests rerender`, Miri, `ffi_parity rerender`, the header; `smoke --rerender` against `examples/rerender_rows.rs`.
- [X] T011 Swift: `ReRenderSession`, `ReRenderResult`, the rewritten `ProcessingRecord.reRender` with cancellation; the old recipe deleted; `us4_surfaces::as2_rerender_no_swift_recipe`; `swift test`, the simulator.

## Phase 6: records and the gate

- [X] T012 `ci.yml`'s fifth promise step (3890bfe, full id) and the Miri filter; version 0.94.0 (Cargo ×4, `RR_DR60_VERSION_MINOR`, header, the stage and control manifests' version lines, the guide's copied blocks, the Swift fixture); the guide section with the five sentences and `us2_cross_check guide_rerender`; the CHANGELOG; the README; the real-time lists; the golden guard count restated from `us2_abi`.
- [X] T013 The full gate (fmt, clippy, tests, the feature-less ffi build with `-D warnings`, llvm-cov, MSRV, the C smoke, the five promises, `scripts/ab-timing.sh 3890bfe record`, the Swift package, the simulator goldens); the post-implementation reviews (coach, hardware).

## Phase 7: Convergence

- [X] T014 Add an end-to-end cell with two band-filter parameter rows at one position (recorded, re-rendered `Identical`, their order kept in the new record) per Edge Cases "two rows at one position" / plan R-02 (partial)
- [X] T015 Add the Swift `ReRenderSession` reads and edits of the rows, the seed, the counter and the remaining capacity over the C plan calls, with a test, per D8 / plan R-11 (partial)
- [X] T016 File the workbench issue for dropping its re-render mirror once 0.94 merges per SC-004 / plan R-13 (missing)
