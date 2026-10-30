# Implementation Plan: Sample-Timed Control Changes and the Change Log

**Branch**: `015-timed-changes` | **Date**: 2026-10-30 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/015-timed-changes/spec.md` (approved D1 – D9, 2026-10-30)

## Decisions and findings (owner: read this section)

The spec's D1 – D9 stand. The plan reviews (coach, hardware, product) are recorded below when applied. Plan choices, each traced to the research:

| # | Plan choice | Why |
|---|---|---|
| P1 | The stream state (the clock, the schedule, the last applied values, the pending plain change, a staging log) lives in `PlaybackPath`, the object both streams already are (R-01). | One implementation for the one-pass and the player; the apply point is inside the path. |
| P2 | The route and the volume are applied **lazily at the first produced sample at or past the position**, immediately before `chain.process` in `try_next_host`; the speed immediately before the `speed.next` call with device-output index d(P) (R-02). | That point is output sample P exactly; bit-identical to 0.15's entry-time apply for the plain setters (nothing observes the chain between); a dropped block applies nothing. |
| P3 | A sorted inline schedule of 64 entries per stream, ties by a sequence number; one `u64` compare against the earliest position per produced sample (R-03, R-09). | No allocation; position order (D4); the hot path is one compare. |
| P4 | Validation at the schedule call in a fixed order; distinct errors `ScheduleFull`, `ControlNotOnStream`, `InvalidControlValue`; the named-tap route refusal (R-04). | FR-001, FR-002; nothing stored on a refusal. |
| P5 | `OutputChain::reset` split into `reset_sections` (the route switch) and the full reset (R-05). | B4: spec 016's accumulators survive a mid-call route switch. |
| P6 | The log per stream from the stream's last applied values; entries staged in the path and drained by the `*_with_changes` variants; `BlockInfo`/`PlayInfo` gain `changes` (R-06, R-07). | D5; the cross-stream entry falls out of comparing with the stream's own values; the plain variants keep their form. |
| P7 | Lifecycle calls set the stream's last applied values without an entry; a plain setter is a pending pair at every call's entry, logged only if it moves the stream (R-07). | Coach 6, B2, B3. |
| P8 | The primary oracle is the plain setter at block size 1 in a reference engine; the 011 splice, the 009 route oracle and a device-index schedule for the 013 reference at 6 kHz; the irregular partition from `volume_ramp_checks` extended (R-08). | SC-001 – SC-002 without circularity (d(P) computed in the test). |
| P9 | Cost by `tests/changes_timing.rs` against the 0.15 API in a new `changes` mode of the A/B script, asserting 3 % (5 % on the runner); the pending-change figure reported (R-09). | D7. |
| P10 | `golden-changes-v1.json` with digests and full logs over the scenario set at seven rates; the C smoke `--changes` diffed against a Rust example (R-10). | FR-009, SC-003, SC-004 (the 64 + 3 entry). |
| P11 | Stages 9 and 11's API items name `Pipeline::schedule_change`; one canonical re-bless; the control manifest unchanged (R-11). | FR-012, FR-013. |
| P12 | `recording_counter()` in Rust and C; `speed_landing(P, rate)` published (R-12, contracts). | D9; the guide's lag arithmetic. |

**Finding F1 (R-02):** 0.15 applies the plain setters at the call's entry even when the call produces nothing; the lazy apply moves that to the next call's first produced sample with no observable difference (the chain is only read through `process`), which the nine SHA guards and the block-size-1 oracle prove. The one place where the two forms differ is the *log*: an entry is reported by the call that produced the sample, never by an empty call (spec Edge Cases).

**Finding F2 (R-02):** at rates above 6 kHz some host samples pull no device sample, so "the first device sample pulled for P" may be first pulled by a later host sample; d(P) still defines the landing and the block-size-1 setter lands at the same device index, so FR-007's identity holds for the speed with no look-ahead term.

**Finding F3 (R-03):** the entry is 40 bytes with a `u32` control and a `u32` reserved; 64 of them are 2.5 KB per stream inside the path; no `Box`.

## Summary

Each stream gets a clock, a sorted schedule and a staging log inside its `PlaybackPath`; changes land lazily at the production point of sample P (the route and the volume) or device sample d(P) (the speed); the plain setters become pending pairs applied at the same point, bit-identical to 0.15; every applied change is logged per stream with chained values and a tag; new `*_with_changes` variants, five small calls and a C twin; the oracles are the block-size-1 setter, the 011 splice, the 009 route run and the 013 reference; a golden, a C parity case and an asserted A/B; version 0.16.0.

## Requirement map

| Req. | Mechanism | Test |
|---|---|---|
| FR-001 the schedule call, validation, the tap refusal | R-04 | core `changes::refusals`; `changes_edge_cases` (every refusal; nothing stored) |
| FR-002 capacity, order, pending count, clearing, position | R-03, R-01 | core `changes::order_and_capacity`; `changes_edge_cases` (erase, erase_all, stop, finished, reset, reconfigure, start keeps) |
| FR-003 the landing rules, the meters intact | R-02, R-05 | `us1_changes` AS2 – AS5, AS7 – AS9; core `output::reset_sections_keeps_accumulators` |
| FR-004 plain setters unchanged; readings at landing | R-07 | the SHA guards; `us2_changes` AS2; `changes_edge_cases` (readings) |
| FR-005 the log, chained, the tag, the count, 67 | R-06 | `us2_changes` AS1, AS3, AS4; `changes_matrix` (64 + 3) |
| FR-006 same-value rules | R-06, R-07 | `us2_changes` AS5 |
| FR-007 identity with 0.15 | R-02 | the ten SHA guards; `us1_changes` AS6 |
| FR-008 partition independence; the block-size-1 oracle; the irregular partition | R-08 | `us1_changes` AS1; `changes_matrix` (seven rates, both routes, codec on/off for the speed) |
| FR-009 deterministic, real-time safe, the golden | R-03, R-06, R-10 | `alloc_free changes`; `determinism changes`; `golden_changes` on the six targets + iOS sim |
| FR-010 cost | R-09 | `scripts/ab-timing.sh f1449c2 changes`; `RR_DR60_CHANGES_COST_CEILING=0.05` on the label job |
| FR-011 Rust and C forms | contracts | FFI unit tests (statuses, NULLs, the count), `ffi_parity changes`, C smoke `--changes`; header drift |
| FR-012 docs and product notes | R-11 | review; `us2_cross_check canonical`; the guide's regions |
| FR-013 no assumption, manifest unchanged | R-11 | `us2_controls` (29 entries; no new `set_`); `check-traceability.sh` |
| SC-001 – SC-006 | as above | SC-001 AS1/matrix; SC-002 AS2 – AS4; SC-003 guards + golden + smoke + AS9; SC-004 us2 + the 64 + 3 entry; SC-005 the A/B; SC-006 llvm-cov |

## Technical Context

**Language/Version**: Rust 1.85 (edition 2024), unchanged.
**Primary Dependencies**: none new; `no_std` + `alloc` core.
**Storage**: N/A (the golden JSON in the harness).
**Testing**: `cargo test` (core unit tests, `us1_changes`, `us2_changes`, `changes_matrix`, `changes_edge_cases`, `mutation changes::`, `golden_changes`, `ffi_parity`, `alloc_free`, `determinism`), the C smoke loop, `scripts/ab-timing.sh … changes`, `cargo llvm-cov`.
**Target Platform**: the existing 8 targets; the output and the log bit-identical on all.
**Performance Goals**: ≤ 3 % per-sample cost at 48 kHz against 0.15 with an empty schedule (5 % on the runner); the pending-change cost reported.
**Constraints**: no allocation, lock or I/O in processing or scheduling; the ten audio golden files, every smoke hash, `RrDr60Settings` and every existing C symbol unchanged; the control manifest's text unchanged; a manifest API-item string within 120 characters.
**Scale/Scope**: about 450 lines of core (`changes.rs`: the schedule, the entry, d(P); the path's state and apply points; the transport's play loop and lifecycle calls; `reset_sections`; the errors), 200 of FFI, 1100 of harness (`changes_checks.rs` with the reference engine and the extended reference, five test files, `changes_timing.rs`, the golden generator, the example, the mutation/parity/alloc/determinism extensions), 80 of C smoke, 60 of script and workflow, docs.

No NEEDS CLARIFICATION remains.

## Constitution Check

| Principle | How this plan complies | Pre | Post |
|---|---|---|---|
| I. Portable Library First | `no_std` core, inline arrays, no I/O; C API additive (one struct, eight functions, two statuses), header regenerated and drift-checked. | ✅ | ✅ |
| II. Evidence-Based Fidelity | No modeled value changes; no assumption (an API convention, D8); the guide says so in its lead sentence. | ✅ | ✅ |
| III. Measured, Test-First Verification | The block-size-1 oracle, the splice, the route and the speed references are written first and fail before the lazy apply exists; the earlier guards prove no audio change; the A/B asserts the cost. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | Integer positions, a stable order, exact values; one compare per sample; no allocation (`alloc_free` covers the schedule call and the log). | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | The landing rules are per control at the stage the control manifest assigns; the stage manifest's API items name the call. | ✅ | ✅ |
| VI. Neutral, Honest Communication | The guide labels positioning as an emulator convention; no "provable", no assistant; the audible lag of a speed change is stated. | ✅ | ✅ |
| VII. Independently Testable Features | US1 per control and stream with its own oracle; US2 through the log alone; US3 through C alone. | ✅ | ✅ |

No violation; Complexity Tracking is empty.

## Project Structure

```text
specs/015-timed-changes/  plan.md, research.md, data-model.md, contracts/api.md, quickstart.md, tasks.md (next)
crates/rr_dr60/src/changes.rs            # Stream, ChangeEntry, Schedule, SCHEDULE_CAPACITY, MAX_CHANGES_PER_CALL, speed_landing, ChangeMutation (hooks)
crates/rr_dr60/src/paths.rs              # PlaybackPath: produced, schedule, last applied, pending plain, staging log, the lazy apply in try_next_host
crates/rr_dr60/src/transport.rs          # play: the device-output counter and the speed apply point; play_with_changes; start_playback / stop / erase / finished clearing; recording_counter
crates/rr_dr60/src/pipeline.rs           # schedule_change, pending_changes, stream_position, clear_schedule, process_with_events_and_changes, BlockInfo.changes, reset/reconfigure rules
crates/rr_dr60/src/stages/output.rs      # reset_sections
crates/rr_dr60/src/error.rs              # ScheduleFull, ControlNotOnStream, InvalidControlValue
crates/rr_dr60/src/manifest.rs           # stages 9 and 11 API items
crates/rr_dr60/src/lib.rs                # re-exports; hooks
crates/rr_dr60_ffi/src/lib.rs, cbindgen.toml, include/rr_dr60.h, tests/c/smoke.c (--changes), tests/c/run_smoke.sh
crates/rr_dr60_harness/src/changes_checks.rs   # the reference engine (block-size-1 setter), the extended irregular partition, the log expectation builder, the device-index speed reference
crates/rr_dr60_harness/src/speed_checks.rs     # reference_output generalized to a device-index schedule
crates/rr_dr60_harness/src/golden.rs, configs.rs   # ChangesGolden, generate_changes
crates/rr_dr60_harness/tests/us1_changes.rs, us2_changes.rs, changes_matrix.rs, changes_edge_cases.rs, golden_changes.rs, changes_timing.rs
crates/rr_dr60_harness/tests/mutation.rs, ffi_parity.rs, alloc_free.rs, determinism.rs
crates/rr_dr60_harness/examples/changes_rows.rs
crates/rr_dr60_harness/golden/golden-changes-v1.json (new), manifest-v1.txt (re-bless)
scripts/ab-timing.sh (the changes mode), .github/workflows/ab-timing.yml (the step, ceiling 0.05)
docs/guide.md (Timed changes section; real-time list), CHANGELOG.md, docs/product/{controls-discussion,front-and-inside,investigation-mode,decisions}.md, CLAUDE.md
```

## Reviews

The coach, the hardware-emulation reviewer and the product specialist review this plan next; their findings are applied above before the owner's plan sign-off. Tasks, analyze and implement then proceed to the merge, stopping only on a failed check or a new decision.
