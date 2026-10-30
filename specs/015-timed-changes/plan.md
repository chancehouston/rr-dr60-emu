# Implementation Plan: Sample-Timed Control Changes and the Change Log

**Branch**: `015-timed-changes` | **Date**: 2026-10-30 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/015-timed-changes/spec.md` (approved D1 – D9, 2026-10-30)

## Decisions and findings (owner: read this section)

The spec's D1 – D9 stand. The three plan reviews of 2026-10-30 (coach 1 – 18, hardware P1 – P11, product M1 – M9) are applied at their recommended defaults; the items below are the ones a reviewer marked as the owner's or that changed a plan choice, each taken at the default and open to reversal at the plan sign-off:

1. **One apply point per stream, the speed included** (hardware P2): a speed change is requested at the top of host sample P like the chain controls; the device-index d(P) stays a published mapping and a harness pin, never engine logic (it removes a counter, a compare, a never-pulled hole at the end of a playback, and the draft's F2).
2. **The one-pass's front taps keep the clock and the log** (hardware P1, coach 2): the apply point moves out of the resampler path into the path's emit, so a stream at `AfterAgc` still counts, lands and logs (the chain is inactive there; nothing audible changes).
3. **A scheduled volume is rounded to single precision** (coach 6): a double `0.1` from C or Swift is accepted as `set_volume(0.1f)` is; the entry's `after` is the stored `f32` widened. The strict alternative (refuse a value not exactly an `f32`) is the owner's to prefer.
4. **The stage manifest's two API-item lines are trimmed to fit** (coach 1) — **restated at implementation:** both trims drop a token an earlier spec's test pins (008 FR-020's `speech_codec_enabled` on stage 9; the 009/011/013 pins on both), so the lines keep their 0.15 text, the canonical moves in its version line only, and the guide and the control manifest carry the call (spec Clarifications).
5. **The audible lag of a speed change is `playback_latency_samples()`** (product M1, coach 16) — **restated at implementation:** the first *bit* difference lies within two device periods after P (asserted per rate), while the perceptual lag is the configuration's playback latency (documented and printed, not asserted as a first difference).
6. **The C stream is an enum `RrDr60Stream`** (product M2, coach 11), the spec 014 pattern; `speed_landing` is fallible (coach 7: no division by zero across the FFI); the control index is `u32` on the whole surface (coach 10).

Plan choices, each traced to the research:

| # | Plan choice | Why |
|---|---|---|
| P1 | The stream state (the clock, the schedule, the last applied values, the pending plain change, a staging log) lives in `PlaybackPath`, the object both streams already are (R-01). | One implementation for the one-pass and the player; the apply point is inside the path. |
| P2 | Every change is applied **at one point per stream: the top of host sample `produced`** (the path's `emit`, reached by the one-pass's normal and front-tap branches and by each `play` iteration before any pull), when `produced >= next_apply` (R-02). | That point is output sample P exactly for the chain controls and the request before the pull d(P) for the speed; bit-identical to 0.15's entry-time apply (nothing observes the chain between; no `speed.next` runs between); a dropped block applies nothing. |
| P3 | A sorted inline schedule of 64 entries per stream with a cached earliest position, ties by a `u64` sequence; the plain pending change folded into the same cached `next_apply`, so the hot path is one predictable `u64` compare per produced sample and nothing per device pull (R-03, R-09). | No allocation; position order (D4); the schedule survives `PlaybackPath::reset` (`start_playback` keeps it) and is cleared explicitly (R-01). |
| P4 | Validation at the schedule call in a fixed order; errors `ReservedStream`, `ControlNotOnStream`, `InvalidControlValue`, `ScheduleFull` beside the existing ones; the volume narrowed to `f32` and −0.0 normalized; the named-tap route refusal (R-04). | FR-001, FR-002; nothing stored on a refusal; equality in the log is numeric. |
| P5 | `OutputChain::reset` split into `reset_sections` (the route switch) and the full reset (R-05). | B4: spec 016's accumulators survive a mid-call route switch. |
| P6 | The log per stream from the stream's last applied values; entries staged in the path and drained by the `*_with_changes` variants; `BlockInfo`/`PlayInfo` gain `changes` (R-06, R-07). | D5; the cross-stream entry falls out of comparing with the stream's own values; the plain variants keep their form. |
| P7 | Lifecycle calls set the stream's last applied values without an entry; a plain change is pending at every call's entry, applied unconditionally at the first produced sample (as 0.15 did at entry) and logged only if it moves the stream; a change of either origin on one stream reaches the other as a tag-0 entry; the readings move at landing through a `landed` record (R-07). | Spec coach 6, B2, B3; hardware P4, P7; coach 14. |
| P8 | The primary oracle is the plain setter at block size 1 in a reference engine; the 011 splice, the 009 route oracle and a device-index schedule for the 013 reference at 6 kHz; d(P) pinned independently by the place reading; the lag asserted; a new irregular partition of 1 … 4096 that is not cut at the scheduled positions (R-08). | SC-001 – SC-002 without circularity. |
| P9 | Cost by `tests/changes_timing.rs` against the 0.15 API in a new `changes` mode of the A/B script, asserting 3 % (5 % on the runner); the pending-change figure reported (R-09). | D7. |
| P10 | `golden-changes-v1.json` with digests and full logs over the scenario set at seven rates; the C smoke `--changes` diffed against a Rust example (R-10). | FR-009, SC-003, SC-004 (the 64 + 3 entry). |
| P11 | The stage manifest's lines keep their text (restated: the pinned tokens leave no room within 120 characters); the canonical moves in its version line only; the control manifest unchanged; the guide's section is "Scheduled changes and the change log" and the records are the copy in records.md (R-11). | FR-012, FR-013; coach 1; product M8, M9. |
| P12 | `recording_counter()` in Rust and C; `speed_landing(P, rate)` published as a fallible mapping; the control index resolved by identifier in the smoke and the guide (R-12, contracts; product M4). | D9; the guide's lag arithmetic. |

**Finding F1 (R-02):** 0.15 applies the plain setters at the call's entry even when the call produces nothing; the lazy apply moves that to the next call's first produced sample with no observable difference (the chain is only read through `process`), which the nine SHA guards and the block-size-1 oracle prove. The one place where the two forms differ is the *log*: an entry is reported by the call that produced the sample, never by an empty call (spec Edge Cases).

**Finding F2 (R-02, hardware P2):** at most one device pull happens per host sample at every rate, and after host P − 1 the pull count is exactly d(P), so a request at the top of host sample P is the request before the pull d(P); no device-index machinery is needed, and a d(P) that is never pulled (the last ⌈m ÷ l⌉ − 1 host samples before a playback's total at the converting rates) is still applied and logged.

**Finding F3 (R-03):** the schedule entry and the change entry are 40 bytes each; 64 + 67 of them are about 5 KB per stream inside the path; no `Box`.

**Finding F4 (R-02, hardware P1):** the one-pass's front-tap branch never reaches the resampler, so an apply point inside `try_next_host` would leave those streams with a clock stuck at 0; the apply point is the path's emit.

## Summary

Each stream gets a clock, a sorted schedule and a staging log inside its `PlaybackPath`; every change lands at one point per stream, the top of host sample P (the speed's request then reaches device sample d(P)); the plain setters become pending changes applied at the same point, bit-identical to 0.15; every applied change is logged per stream with chained values and a tag; new `*_with_changes` variants, five small calls and a C twin; the oracles are the block-size-1 setter, the 011 splice, the 009 route run and the 013 reference; a golden, a C parity case and an asserted A/B; version 0.16.0.

## Requirement map

| Req. | Mechanism | Test |
|---|---|---|
| FR-001 the schedule call, validation, the tap refusal | R-04 | `changes_edge_cases::refusals_store_nothing` (every refusal; nothing stored); FFI `changes_tests` |
| FR-002 capacity, order, pending count, clearing, position | R-03, R-01 | core `changes::schedule_*` (order, capacity); `changes_edge_cases::clearing_rules` (start keeps; erase, erase_all, stop, finished, reset, reconfigure) and `the_clock_and_the_front_taps` |
| FR-003 the landing rules, the meters intact | R-02, R-05 | `us1_changes` AS2 – AS5, AS7 – AS9; core `output::reset_sections_keeps_accumulators`; `changes_matrix` (the place pin, the lag) |
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
**Scale/Scope**: about 450 lines of core (`changes.rs`: the schedule, the entry, `speed_landing`; the path's state, `next_wide`/`emit` and `apply_due`; the transport's play loop and lifecycle calls; `reset_sections`; the errors), 200 of FFI, 1100 of harness (`changes_checks.rs` with the reference engine and the extended reference, five test files, `changes_timing.rs`, the golden generator, the example, the mutation/parity/alloc/determinism extensions), 80 of C smoke, 60 of script and workflow, docs.

No NEEDS CLARIFICATION remains.

## Constitution Check

| Principle | How this plan complies | Pre | Post |
|---|---|---|---|
| I. Portable Library First | `no_std` core, inline arrays, no I/O; C API additive (one struct, eight functions, two statuses), header regenerated and drift-checked. | ✅ | ✅ |
| II. Evidence-Based Fidelity | No modeled value changes; no assumption (an API convention, D8); the guide says so in its lead sentence. | ✅ | ✅ |
| III. Measured, Test-First Verification | The block-size-1 oracle, the splice, the route and the speed references are written first and fail before the lazy apply exists; the earlier guards prove no audio change; the A/B asserts the cost. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | Integer positions, a stable order, exact values; one compare per sample; no allocation (`alloc_free` covers the schedule call and the log). | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | The landing rules are per control at the stage the control manifest assigns; the control manifest's entries lead to the call (the stage manifest's lines keep their text, restated). | ✅ | ✅ |
| VI. Neutral, Honest Communication | The guide labels positioning as an emulator convention; no "provable", no assistant; the audible lag of a speed change is stated. | ✅ | ✅ |
| VII. Independently Testable Features | US1 per control and stream with its own oracle; US2 through the log alone; US3 through C alone. | ✅ | ✅ |

No violation; Complexity Tracking is empty.

## Project Structure

```text
specs/015-timed-changes/  plan.md, research.md, data-model.md, contracts/api.md, quickstart.md, records.md (the copy to paste), tasks.md (next)
crates/rr_dr60/src/changes.rs            # Stream, ChangeEntry, Schedule, SCHEDULE_CAPACITY, MAX_CHANGES_PER_CALL, speed_landing, ChangeMutation (hooks)
crates/rr_dr60/src/paths.rs              # PlaybackPath: produced, schedule, next_apply, last applied, pending plain, landed, staging log; next_wide / emit / apply_due; clear_schedule
crates/rr_dr60/src/transport.rs          # play: apply_due per host iteration (the speed closure); play_with_changes; start_playback keeps / stop / erase / erase_all / finished clear; recording_counter
crates/rr_dr60/src/pipeline.rs           # schedule_change, pending_changes, stream_position, clear_schedule, process_with_events_and_changes, BlockInfo.changes, reset/reconfigure clearing; tick's front branch counts and applies
crates/rr_dr60/src/stages/output.rs      # reset_sections
crates/rr_dr60/src/error.rs              # ScheduleFull, ControlNotOnStream, ReservedStream, InvalidControlValue
crates/rr_dr60/src/lib.rs                # re-exports; hooks
crates/rr_dr60_ffi/src/lib.rs, cbindgen.toml, include/rr_dr60.h, tests/c/smoke.c (--changes), tests/c/run_smoke.sh
crates/rr_dr60_harness/src/changes_checks.rs   # the reference engine (block-size-1 setter), IRREGULAR (1 … 4096, not cut at the positions), the log expectation builder
crates/rr_dr60_harness/src/speed_checks.rs     # reference_output generalized to a device-index schedule
crates/rr_dr60_harness/src/golden.rs, configs.rs   # ChangesGolden, generate_changes
crates/rr_dr60_harness/tests/us1_changes.rs, us2_changes.rs, changes_matrix.rs, changes_edge_cases.rs, golden_changes.rs, changes_timing.rs
crates/rr_dr60_harness/tests/mutation.rs, ffi_parity.rs, alloc_free.rs, determinism.rs
crates/rr_dr60_harness/examples/changes_rows.rs
crates/rr_dr60_harness/golden/golden-changes-v1.json (new), manifest-v1.txt (re-bless)
scripts/ab-timing.sh (the changes mode), .github/workflows/ab-timing.yml (the step, ceiling 0.05)
docs/guide.md (Scheduled changes section; real-time list), CHANGELOG.md, docs/product/{controls-discussion,front-and-inside,investigation-mode,decisions,brief}.md, CLAUDE.md
```

## Reviews

The coach (1 – 18), the hardware-emulation reviewer (P1 – P11) and the product specialist (M1 – M9) reviewed this plan on 2026-10-30; every finding is applied above and in research.md, data-model.md, contracts/api.md, quickstart.md and records.md. The owner signed off the plan on 2026-10-30 with the six defaults; tasks, analyze and implement proceed to the merge, stopping only on a failed check or a new decision.
