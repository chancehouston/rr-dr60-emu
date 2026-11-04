---
description: "Tasks for engine feature 028, the one-pass output map"
---

# Tasks: The one-pass output map

Test-first. The traceability table and the build order are in plan.md.

## Phase 1: R-00, the reproduction

- [ ] T001 `onepass_map_checks` (the labelled-gap stimulus, the labels, the readers, the record-then-play twin; R-09, R-10) in `crates/rr_dr60_harness/src/onepass_map_checks.rs`, and `onepass_map_matrix::onepass_map_lead_reproduction` (reported) computing both rules in the harness from the engine's public figures: the draft's and the heard point's record-then-play deviation around each splice at seven rates, coder on and off, drop mode; the mute-mode 5 ms window levels at 48 kHz. Stop if the figures do not match the plan's.

## Phase 2: US1 – US3, the core reads (D1 – D5)

- [ ] T002 [US1] Core tests `timeline::onepass_map_tests` (failing): hand cases, the R-02 identity with `onepass_output_to_input(o − c − l_post, l_pre, …)` on random lists, the splice read, the window's parts, `onepass_map_monotone`, `onepass_map_saturates`; the pins `l_post == playback_latency_samples()` and `c == latency − report latency`.
- [ ] T003 [US1] Harness tests (failing) `us1_onepass_map::{as1_onepass_map_labelled_gap, as2_onepass_map_splice_edges, as3_onepass_map_record_then_play, as4_onepass_map_existing_and_workbench, as5_onepass_map_clamped, as6_onepass_map_window, as7_onepass_map_monotone}` in `crates/rr_dr60_harness/tests/us1_onepass_map.rs`.
- [ ] T004 [US1] `OnePassClock` and the crate-private map functions in `crates/rr_dr60/src/timeline.rs`; `Pipeline::{one_pass_input_at_output, one_pass_splice_output_position}` in `crates/rr_dr60/src/pipeline.rs` (R-01 – R-05), saturating.
- [ ] T005 [US2] [US3] Tests `us2_onepass_map::{as1_onepass_map_splice_drop, as2_onepass_map_splice_mute, as3_onepass_map_splice_no_codec}`, `us3_onepass_map::{as1_onepass_map_no_codec, as2_onepass_map_mute, as3_onepass_map_no_pause}`, `onepass_map_edge_cases::*` (six), `onepass_map_matrix::{onepass_map_seven_rates, onepass_map_read_cost}`, `alloc_free::onepass_map_allocates_nothing`; fixes as they show.

## Phase 3: FR-008, the mutants

- [ ] T006 `OnePassMapMutation::{no_c, c_twice, no_l_post, take_while_on_report_position, clamp_before_r, c_from_setting}` behind `__test-hooks`, folded into the clock; `mutation::onepass_map::*` shows each failing its named test (R-11).

## Phase 4: D6, C and Swift

- [ ] T007 C: `rr_dr60_one_pass_input_at_output`, `rr_dr60_one_pass_splice_output_position` in `crates/rr_dr60_ffi/src/host_reads.rs`; the header; `abi_tests::onepass_map_*`, Miri, `ffi_parity::onepass_map_parity`; `smoke --onepass-map` against `examples/onepass_map_rows.rs`; `us2_abi::spec_028_onepass_map_appended_only` and `spec_027_appended_only` restated to two revisions.
- [ ] T008 Swift: `swift/Sources/RRDR60/OnePassMap.swift`, `OnePassMapTests` with the fixture (incl. the empty array); `swift test`, the simulator.

## Phase 5: D4, D8, records and the gate

- [ ] T009 The documentation (R-06): the existing map's rustdoc and C comment, `VasEvent` and `RrDr60VasEvent`, the dated note on `one_pass_report_latency_samples`, the guide's two sections and three new paragraphs, the real-time lists; `us2_cross_check::guide_onepass_map`.
- [ ] T010 Version 0.95.0 (Cargo ×4, `RR_DR60_VERSION_MINOR`, header, the stage manifest's version line, the guide's copied blocks, the Swift fixture); ci.yml's sixth promise step and the Miri filter; the CHANGELOG (`### Added`, `### Fixed (#82)`); the README; the golden diff against ce081b8.
- [ ] T011 The full gate (plan.md › Gate) and `scripts/ab-timing.sh ce081b8 record_contract` (reported); the post-implementation reviews (coach, hardware).
