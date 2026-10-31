# Implementation Plan: Pitch-Kept Playback Speed (an option)

**Branch**: `023-pitch-kept-speed` | **Date**: 2026-10-31 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/023-pitch-kept-speed/spec.md` (approved D1 – D11, 2026-10-31)

## Decisions and findings (owner: read this section)

The spec's D1 – D11 stand. The plan's restatements (the plan reviews' findings are appended here when applied):

1. **One stage, two mechanisms** (R-01, R-02): the time-scaler is a fixed-size state inside the speed stage, reading the same place and ring; the level-3 fast path is untouched.
2. **The tail is at most 120 device samples**, not 160 (R-06): the content past K is only the last frame's fade through the next hop, a pure function of the own part and the grid origin; the length helper is exact.
3. **The search costs about 81 multiply-adds per output** (R-03), 1.5 × the resampler's 57, inside the 3 × budget with margin.
4. **The mode blend freezes a copy of the old scaler** (R-05): a fixed-size copy, no allocation; the old branch plays its own frames out.
5. **The promise script reuses 0.22's smoke sources against the current library** (R-10), diffing each mode's output against the current smoke's.

| # | Plan choice | Why |
|---|---|---|
| P1 | `TimeScaler` in `stages/speed.rs`, the window table in `speed_window.rs` written by the design script, the synthesis and search of R-02/R-03, the anchor, the pulls by mode, the ring span 228. | D2, D4, D6; FR-002, FR-004, FR-007. |
| P2 | The place untouched; the frame grid anchored; level changes frame-fed with no fade; into-3 ends over the remaining overlap; out-of-3 anchors (R-04). | D4, D5; FR-004, FR-005. |
| P3 | The mode fade with the frozen old branch; the one wait rule; at level 3 at once (R-05). | D5; FR-005. |
| P4 | The exact tail and `playback_length_samples_in_mode` (R-06). | D4; FR-004. |
| P5 | The control, the stream state, the error, the setters, the schedule validation, the manifest notes, `MAX_CHANGES_PER_CALL` 143 (R-07, R-08). | D1, D8; FR-001, FR-006. |
| P6 | The design script's `--pitch-kept`, the harness reference and readers, the stimuli (R-09). | D6; FR-002; SC-001. |
| P7 | The C surface, the parity rows, `smoke --pitch`, the promise script in CI (R-10). | D9; FR-008; SC-005. |
| P8 | The timing tests and the `pitch` A/B mode (R-11). | D7; FR-009; SC-004. |
| P9 | The golden with shift digests, the seven mutants, the records (R-12). | D8, D10; FR-010; SC-002, SC-003, SC-006. |

## Summary

A second mechanism behind the speed stage, selected by a fifth live control: a waveform-similarity overlap-add on the decoded stream that keeps the pitch while the level changes the speed, with the place, the latch and the drain exactly as spec 013 left them, an exact length, frame-fed level changes, a blended mode change, and every figure pinned by a design script and a harness reference that share only the published rules.

## Requirement map

| Requirement | Research | Tests |
|---|---|---|
| FR-001 the control | R-07 | core `pipeline` tests, `us2_pitch` AS3 – AS5, `us1_controls`/`us2_controls`, `ffi_parity pitch` |
| FR-002 the time-scaling and its figures | R-02, R-03, R-09 | `us1_pitch` AS1 – AS3, `pitch_matrix`, `mutation pitch::{no_search, window_linear}` |
| FR-003 the identities | R-01 | `us1_pitch` AS4, the fifteen guards (`us2_pitch`), the golden's twins |
| FR-004 the place and the exact length | R-06 | `us1_pitch` AS5, core `lengths_and_tails_pitch_kept`, `pitch_edge_cases` (K = 1, 59, 119) |
| FR-005 the changes and the wait rule | R-04, R-05 | `us2_pitch` AS1, AS2, AS7, `mutation pitch::{restart_overlap, mode_step, grid_unanchored}` |
| FR-006 the manifests | R-08 | `us2_cross_check`, `us1_controls`, `us2_pitch` AS4 |
| FR-007 determinism, partitions, allocation | R-03, R-12 | `determinism pitch`, `pitch_matrix` partitions, `alloc_free pitch`, the golden's six targets |
| FR-008 the C surface and the promise | R-10 | `ffi_parity pitch`, `smoke --pitch`, `scripts/abi-promise.sh` |
| FR-009 the cost | R-11 | `pitch_timing`, `ab-timing pitch` |
| FR-010 the records | R-12 | `us2_cross_check` (the guide statements, A-054), review |

## Technical Context

**Language/Version**: Rust 1.85 (edition 2024), `no_std` + alloc core; Python 3 (uv) for the design script. **Dependencies**: none new. **Testing**: `cargo test`, the harness, the C smoke, Miri (the FFI only). **Performance**: the search once per 60 device outputs (≈ 4860 multiply-adds), ≈ 83 per output; the worst block decodes two codec frames. **Constraints**: no allocation after construction (the scaler fixed-size, the window boxed at construction); the ring 256; detmath only.

No NEEDS CLARIFICATION remains.

## Constitution Check

| Principle | How this plan complies | Pre | Post |
|---|---|---|---|
| I. Portable Library First | Pure Rust in the core; the C surface appended only; the header regenerated. | ✅ | ✅ |
| II. Evidence-Based Fidelity | An engine convention, stated everywhere; A-054 amended with a dated note; no magic numbers (N, Ss, Δ, Lc engineering targets cited to the research). | ✅ | ✅ |
| III. Measured, Test-First Verification | The figures pinned by the design script and the harness reference; tests first per story; a golden. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | Fixed-order sums, an outward tie-break, no allocation, bounded work per output. | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | Stage 9's notes updated; the control traced to A-054. | ✅ | ✅ |
| VI. Neutral, Honest Communication | The convention paragraph; the artifacts named; no claim about content. | ✅ | ✅ |
| VII. Independently Testable Features | The speed-only configuration; the default mode untouched. | ✅ | ✅ |

No violation; no Complexity Tracking entry.

## Project Structure

```text
specs/023-pitch-kept-speed/  plan.md, research.md, data-model.md, contracts/api.md, quickstart.md, records.md, tasks.md (next)
crates/rr_dr60/src/stages/speed.rs        # TimeScaler, the mode, the fade's old branch, the pulls, the tail
crates/rr_dr60/src/stages/speed_window.rs # the window table (generated)
crates/rr_dr60/src/{speed.rs (SpeedMode), changes.rs, pipeline.rs, transport.rs, error.rs, controls.rs, manifest.rs, lib.rs (hooks)}
crates/rr_dr60_ffi/src/lib.rs, include/rr_dr60.h, tests/c/smoke.c (--pitch)
crates/rr_dr60_harness/src/pitch_checks.rs, examples/pitch_rows.rs, tests/{us1_pitch, us2_pitch, pitch_matrix, pitch_edge_cases, golden_pitch_kept, pitch_timing}.rs, mutation.rs, alloc_free.rs, determinism.rs, ffi_parity.rs
tools/playback-speed/speed_design.py (--pitch-kept), scripts/abi-promise.sh, scripts/ab-timing.sh, .github/workflows/{ci,ab-timing}.yml
docs, product notes, registers, CLAUDE.md per records.md
```

## Reviews

*(The coach, the hardware-emulation reviewer and the product specialist review this plan next; their findings are applied above and in the design documents, and the spec carries the restatements as a dated Clarifications session.)*
