# Implementation Plan: Live Record-Side Values

**Branch**: `017-live-record-values` | **Date**: 2026-10-30 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/017-live-record-values/spec.md` (approved D1 – D10, 2026-10-30)

## Decisions and findings (owner: read this section)

The spec's D1 – D10 stand with two figures restated by the plan reviews (coach 1 – 12, hardware B1 – B11, product M1 – M6, 2026-10-30, applied at their defaults); the items a reviewer marked as the owner's, each taken at the default and open to reversal at the plan sign-off:

1. **The always-enough log length is 139, not 131** (coach 1, hardware B1): the cross-stream landings of D4 are logged too, up to eight in one call, so the bound is two schedules + the three plain setters + the eight landings; the one-pass attains 138 (the speed is playback-only), the recorder 72 (its staging log grows to 72), the player 67. D2 / FR-005 / SC-004 restated.
2. **The hiss transition keeps spec 011's indexing** (coach 3): the landing sample is transition sample 1 and the target is assigned at P + N − 1, as the knob's; the spec's "P + N" is restated (D3, FR-003, AS6).
3. **`start_recording` and `reset` apply the settings to the stages, not only record them** (hardware B2; spec 015's `start_playback` precedent): `RecordPath::adopt(&settings)`, so a value landed on the one-pass is in the recorder's stages when the next recording starts, without an entry.
4. **The mirror's tag asymmetry is documented, not aligned now** (coach 4, product M1): a record-side value landed on the other stream carries the landing's tag (D4), while the playback side's three carry 0 (spec 015, shipped); the guide says so and the two align at spec 020's ABI pass (a mirrored bit beside the stream word in `reserved`).
5. **The event bound takes the settings' hang too** (coach 2, hardware B3), so a hang landed on the recorder keeps `max_events` on the one-pass always enough before the landing.
6. **`RecordInfo` gains the final input position** (hardware B10) so a self-stop's position is readable; `rest_meters()` follows the settings' hang (no audio change).

Plan choices, each traced to the research:

| # | Plan choice | Why |
|---|---|---|
| P1 | The record-side state (`RecordCtl`: the input clock, a schedule, the last applied values, the cross-stream pending, the landed record, a 72-entry staging log) lives in `RecordPath` (`repr(C)`, hot scalars first), the object both record-side streams are; spec 015's playback state is untouched, so the one-pass carries one state per path, the two types sharing a stated rule list (R-01). | One implementation for the recorder, the stop's feed and the one-pass, the front taps included. |
| P2 | One apply point: the top of input sample P in `RecordPath::step` / `front_only`, before the front; the gate's values then reach device sample k(P) with no device-side point (R-02). | The gate processes nothing between host samples (the mirror of 015's speed argument). |
| P3 | `Limiter::retune` shares `derive` with `new`, stores the two overdrive indices, and carries `vc`, `vc_max`, the floor and the limit (R-03). | The input-0 identity is exact by construction; the gain law and the 016 charge are continuous; an attack or release change costs a few multiplies. |
| P4 | `VasStage::set_values` recomputes the threshold and the two lengths, keeps the counts; `max_events` takes the minimum of the stage's hang, the settings' hang and every pending hang change (R-04). | D3's counting rules; spec 003's event bound holds through a shortening on either stream. |
| P5 | `MicCapsule::set_offset` with a 20 ms smoothstep on the gain (the 011 form and indexing: the landing sample is transition sample 1, the target assigned at P + N − 1), at once when the clock reads 0 or the capsule is bypassed; the draws untouched (R-05). | D3, D9; one compare on the hot path. |
| P6 | The cross-stream landing through the shared settings: `begin_call` compares the settings with the path's last applied values and lands the difference at the call's first input sample, before the stream's own entries, with the landing's tag; `start_recording` and `reset` *adopt* the settings into the stages without an entry; `reconfigure` rebuilds (R-02). | D4; spec 015's rule and its `start_playback` precedent. |
| P7 | The stop: the path's clock counts every fed host sample; the schedule applies during the flush and `close_recording` clears it at every end (the skipped flush and the self-stop included); `StopOutcome` and `RecordInfo` gain the final position and the count; `stop_recording_with_changes` (R-07). | D5. |
| P8 | The one-pass's two schedules routed by `clock_of`; the log-taking variants drain the record path then the playback path; `MAX_CHANGES_PER_CALL` 139 (R-08). | D2 restated. |
| P9 | `clock_of`, `record_landing`, `input_position`, `NoSuchClock` (29) in Rust and C (R-09). | D10, FR-010. |
| P10 | Oracles: the input-0 identity, the pre-landing identity in the kept-sample index (per frame with the codec on), the 016 reference gate, the limiter's settle on pinned knee pairs and its continuity, the hiss ratio within 1 f32 ULP at the capsule tap, record-then-play, the stop, the cross-stream landing and the adopt rule, the event bound on both streams, the k(P) pin by the stored sum (R-10). | FR-006, FR-007. |
| P11 | Cost by `record_timing.rs` against the 0.16 API in a `record` mode of the A/B script (3 %, 5 % runner), the knee-per-block and worst-block figures reported (R-11). | D7. |
| P12 | `golden-record-changes-v1.json`, the C smoke `--record-changes`, the control manifest's timing words re-blessed once, the records of records.md (R-12, R-13). | FR-008, FR-010, FR-011. |

## Summary

The record path gains its own clock, schedule and log; eight record-side values land at the top of input sample P (the limiter retuned with its capacitor carried, the hiss gliding over 20 ms, the gate's values effective at device sample k(P) with its counts kept); each stream keeps its own applied values and the shared settings carry a landing to the other stream; the stop's feed applies and clears; the one-pass carries two schedules and the always-enough log length becomes 131; new helpers name each control's clock and the gate's landing sample; version 0.17.0; no audio change with no scheduled change.

## Requirement map

| Req. | Mechanism | Test |
|---|---|---|
| FR-001 the eight controls, validation | R-06 | `record_edge_cases::refusals`; FFI `record_tests` |
| FR-002 schedules, clearing, the input positions, the event bound | R-01, R-07, R-08, R-04 | `record_edge_cases::clearing_and_clocks`; `us1_record` AS8 |
| FR-003 the landing rules | R-02 – R-05 | `us1_record` AS1 – AS7; core `record_ctl` |
| FR-004 per-stream values, `settings()` | R-02 (begin_call) | `us2_record` AS4; `record_edge_cases` |
| FR-005 the log, 139 | R-07, R-08 | `us2_record` AS1 – AS3; `record_matrix` (138 on the one-pass, 72 on the recorder); the 015 suites restated |
| FR-006 identities | R-10 | `us1_record` AS1, AS2; `record_matrix`; `us2_record` AS2; the eleven guards |
| FR-007 the stage oracles | R-10 | `us1_record` AS3 – AS6 |
| FR-008 determinism, the golden, bounded work | R-11, R-12 | `determinism record` (the exported bytes), `alloc_free record`, `golden_record_changes`, `record_pending_timing` (the per-change figure) |
| FR-009 cost | R-11 | `scripts/ab-timing.sh 06e40b0 record`; `RR_DR60_RECORD_COST_CEILING=0.05` on the label job |
| FR-010 Rust and C, the manifest's timing | R-09, R-13 | FFI tests, `ffi_parity record_parity`, the smoke; `us1_controls`/`us2_controls` |
| FR-011 docs | R-13 | review; the guide's regions |
| FR-012 no assumption | R-13 | `check-traceability.sh`; the stage manifest's version line only |
| SC-001 – SC-006 | as above | `us1_record`/`record_matrix` 0 mismatches; AS3 – AS6; the guards + golden + smoke + record-then-play; the log scenarios + the tight 131; the A/B; llvm-cov |

## Technical Context

**Language/Version**: Rust 1.85 (edition 2024). **Dependencies**: none new; `no_std` + `alloc`. **Testing**: `cargo test` (core, `us1_record`, `us2_record`, `record_matrix`, `record_edge_cases`, `mutation record::`, `golden_record_changes`, `ffi_parity`, `alloc_free`, `determinism`, the 015 suites), the C smoke, `scripts/ab-timing.sh … record`, `cargo llvm-cov`. **Platform**: the existing 8 targets. **Performance**: ≤ 3 % per input sample at 48 kHz on the record path and the one-pass against 0.16 (5 % runner); the per-change work bounded (a retune about 5 µs). **Constraints**: no allocation, lock or I/O; the eleven golden files, every smoke hash, `RrDr60Settings` (120) and every existing C symbol unchanged; `MAX_CHANGES_PER_CALL` is the one constant that moves (67 → 139). **Scale**: about 500 lines of core (`RecordCtl` in `paths.rs`, `retune`, `set_values`, `set_offset`, the transport's counts and stop, `schedule_change`'s eight controls, the helpers, the errors), 200 of FFI, 1200 of harness (`record_checks.rs`, five test files, two timing tests, the golden generator, the example), 80 of C smoke, 60 of script and workflow, docs.

No NEEDS CLARIFICATION remains.

## Constitution Check

| Principle | How this plan complies | Pre | Post |
|---|---|---|---|
| I. Portable Library First | `no_std` core, inline arrays, no I/O; the C API additive (one constant change recorded), header regenerated and drift-checked. | ✅ | ✅ |
| II. Evidence-Based Fidelity | No modeled value changes; no assumption registered or amended; the guide cites S-001 for the device's procedure and labels the feature an emulator convention. | ✅ | ✅ |
| III. Measured, Test-First Verification | Every oracle written first; the eleven guards prove no audio change; the A/B asserts the cost. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | Integer positions, construction's arithmetic for the retune, the 011 smoothstep; one compare per input sample; `alloc_free` covers the calls. | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | Each value lands at its own stage by a stated rule; the control manifest's timing words say which are live. | ✅ | ✅ |
| VI. Neutral, Honest Communication | "Not a device behavior" with S-001's sentence; records.md's wording rules. | ✅ | ✅ |
| VII. Independently Testable Features | US1 per value with its own oracle; US2 through the log and the stored forms; US3 through C and the manifest. | ✅ | ✅ |

No violation. Complexity Tracking (principle V): two control-state types | the sides' applied values differ in type and the playback type is blessed by `golden-changes-v1.json` | a generic state would touch spec 015's hot path and golden for no 017 requirement; the two share a stated rule list and one cross-side test; revisit at spec 020.

## Project Structure

```text
specs/017-live-record-values/  plan.md, research.md, data-model.md, contracts/api.md, quickstart.md, records.md, tasks.md (next)
crates/rr_dr60/src/changes.rs            # MAX_CHANGES_PER_CALL 131, Clock, clock_of, record_landing, RecordValues, RecordCtl, RecordMutation
crates/rr_dr60/src/paths.rs              # RecordPath: ctl, the apply point in step / front_only, begin_call, apply_due, max_events with the pending hang
crates/rr_dr60/src/stages/limiter.rs     # derive, retune
crates/rr_dr60/src/stages/vas.rs         # set_values
crates/rr_dr60/src/stages/mic.rs         # MicCapsule::set_offset, the transition in process; MicFront::set_hiss_offset
crates/rr_dr60/src/transport.rs          # fed_input, start_recording keeps / stop and self-stop clear, record_with_changes, stop_recording_with_changes, StopOutcome fields
crates/rr_dr60/src/pipeline.rs           # schedule_change's eight controls, input_position, the one-pass's two schedules, the readings and tags in the epilogue, BlockInfo.changes both sides
crates/rr_dr60/src/controls.rs           # the eight entries timing: Live; index helpers
crates/rr_dr60/src/error.rs              # NoSuchClock
crates/rr_dr60_ffi/src/lib.rs, cbindgen.toml, include/rr_dr60.h, tests/c/smoke.c (--record-changes), tests/c/run_smoke.sh
crates/rr_dr60_harness/src/record_checks.rs    # the constant-configuration oracle, the reference-gate change, the limiter settle, the hiss ratio, the stop reader
crates/rr_dr60_harness/src/golden.rs           # RecordChangesGolden, generate_record_changes
crates/rr_dr60_harness/tests/us1_record.rs, us2_record.rs, record_matrix.rs, record_edge_cases.rs, golden_record_changes.rs, record_timing.rs, record_pending_timing.rs
crates/rr_dr60_harness/tests/mutation.rs, ffi_parity.rs, alloc_free.rs, determinism.rs, us1_changes.rs / changes_matrix.rs (the 131 restatement)
crates/rr_dr60_harness/examples/record_changes_rows.rs
crates/rr_dr60_harness/golden/golden-record-changes-v1.json (new), control-manifest-v1.txt (re-bless), manifest-v1.txt (version line)
scripts/ab-timing.sh (the record mode), .github/workflows/ab-timing.yml (the step, ceiling 0.05), ci.yml / ios-device-golden.sh (the golden list)
docs/guide.md, CHANGELOG.md, docs/product/{controls-discussion,brief,front-and-inside,investigation-mode,decisions}.md, CLAUDE.md
```

## Reviews

The coach (1 – 12), the hardware-emulation reviewer (B1 – B11) and the product specialist (M1 – M6) reviewed this plan on 2026-10-30; every finding is applied above and in research.md, data-model.md, contracts/api.md, quickstart.md and records.md. The owner signs off the plan next. Tasks, analyze and implement then proceed to the merge, stopping only on a failed check or a new decision.
