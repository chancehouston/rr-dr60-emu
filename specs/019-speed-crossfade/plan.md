# Implementation Plan: Speed-Change Crossfade

**Branch**: `019-speed-crossfade` | **Date**: 2026-10-30 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/019-speed-crossfade/spec.md` (approved D1 – D9, 2026-10-30)

## Decisions and findings (owner: read this section)

The spec's D1 – D9 stand. The plan's choices, each traced to the research:

| # | Plan choice | Why |
|---|---|---|
| P1 | The crossfade lives inside `SpeedStage`: a `fade: Option<Fade>` state (`old_k`, `old_p5`, `n`) beside the existing place and level; a request is applied only while no fade runs (the wait), applying a change starts a fade; per output while a fade runs the old branch's value at its own place and the new branch's at the common place are blended as `y_old + w_n·(y_new − y_old)` in f64; `dot` takes the bank as a parameter (R-01). | D1, D4, D5; FR-001, FR-004, FR-005. |
| P2 | N = `speed::CROSSFADE_SAMPLES` = 120 device samples, `CROSSFADE_MS` = 20 published beside `VOLUME_TRANSITION_MS`; the weights from a 120-entry f64 table filled at construction from the smoothstep at u = n ÷ 120 (no divide per sample; the core pins w₃₀ = 0.15625 and w₁₂₀ = 1.0 exactly and strict monotonicity) (R-02). | D2, D3; FR-002; SC-002. |
| P3 | The ring grows 128 → 256 `f32` (the two branches' places diverge by up to 96 stored samples; plus 57 taps and the 28-sample look-ahead = 181); the pulls stay bounded by the new branch's look-ahead, which leads (R-03). | D1, D9; FR-009. |
| P4 | The latch, `final_count`, the tail and the drain follow the new branch's place unchanged; a fade continues through the tail and is cut by the drain; the steady-identity fast path requires no fade (R-04). | D6; FR-003, FR-006. |
| P5 | Four mutants in `SpeedMutation`: `fade_step` (the 0.18 switch; also the click reader's switch reference), `fade_linear`, `fade_len_delta`, `fade_no_wait` (R-05). | D8; SC-002, SC-003. |
| P6 | The harness reference models the wait and the fade from the published banks and weights; a new click reader `analysis::click_energy_in` (window length and exclusion list as parameters; spec 011's reader delegates); `crossfade_checks.rs` with the blend identity, the splice, the twelve-pair click table at 6 kHz, the wait cases, the end cut and the partitions (R-06). | D8; FR-007, FR-008, FR-011; SC-001 – SC-004. |
| P7 | Three re-pins at bless time with the identities asserted (the speed golden's constant-level entries, the changes golden's non-speed entries, the smoke's speed case from `smoke_reference`); the eleven other audio guards unmoved (R-07). | D7; FR-003; SC-001. |
| P8 | Cost: the `playback` A/B mode against 13defc7 (level 3; 3 %, `RR_DR60_CROSSFADE_COST_CEILING` 0.05 on the label job) and a head-only `crossfade_timing.rs` (a change every 200 ms; the blend's cost and the worst block, reported) (R-08). | D9; FR-009; SC-005. |
| P9 | The design script's `--crossfade` print (the weights, the twelve pairs' reductions under both readers, the worst end cut) pins the figures; the records of records.md (A-054 amended, P1 reworded, stage 9's notes within 120 characters, the CHANGELOG as a convention change, the guide's paragraph, the product notes) (R-09). | FR-002, FR-010. |

## Summary

A playback speed-level change becomes a 20 ms time-domain crossfade inside the speed stage: the old level's constant-level computation fades out at its own place and ratio while the new level's fades in from the common place, so no band or content is gated within one sample; a request during a fade waits for its end. A constant level is bit-identical to 0.18; three goldens re-pin their schedule entries; the convention is the emulator's (A-054 amended; capture P1 decides the device's).

## Requirement map

| Requirement | Research | Tests |
|---|---|---|
| FR-001 the blend, the splice | R-01, R-02 | `us1_crossfade` AS2, `crossfade_matrix`, core weight pins, `mutation crossfade::` |
| FR-002 N = 20 ms, the convention | R-02, R-09 | core pins; the A-054 register cross-check |
| FR-003 constant levels bit-identical; the three re-pins | R-07 | the eleven guards, `golden_speed` / `golden_changes` bless-time identities, `smoke_reference` |
| FR-004 level 3 without a branch | R-01 | `us1_crossfade` AS5, core `switch_into_level_3` tests |
| FR-005 the wait | R-01, R-06 | `us2_crossfade` AS1, `crossfade_edge_cases`, the reference |
| FR-006 the length rule, the end cut | R-04 | `us1_crossfade` AS6, `crossfade_matrix` (the worst cut), 013's `lengths` |
| FR-007 the click reduction | R-06 | `us1_crossfade` AS3, `crossfade_matrix` (twelve pairs, both readers), the voiced figure |
| FR-008 partitions, the setter oracle | R-06 | `us1_crossfade` AS4, `crossfade_matrix` |
| FR-009 determinism, real-time, cost | R-03, R-08 | `alloc_free`, `determinism`, `scripts/ab-timing.sh 13defc7 crossfade`, `crossfade_timing` |
| FR-010 the records | R-09 | `us2_cross_check` (registers, manifest), the CHANGELOG |
| FR-011 the reference, the weight pins | R-05, R-06 | `us2_speed` (013) green, core `weights_are_the_smoothstep` |
| SC-001 – SC-006 | as above | 0 mismatches; ≥ 30 dB; the A/B; coverage |

## Technical Context

**Language/Version**: Rust 1.85 (edition 2024). **Dependencies**: none new; `no_std` + `alloc`. **Testing**: `cargo test` (core `stages::speed`, `us1_crossfade`, `us2_crossfade`, `crossfade_matrix`, `crossfade_edge_cases`, `mutation crossfade::`, `golden_speed`, `golden_changes`, `us2_speed`, `ffi_parity`, `alloc_free`, `determinism`), the C smoke, the A/B script, the design script. **Target**: every supported host rate; the stage runs at the 6 kHz device rate. **Performance**: a constant level within 3 % of 0.18; a fade two dots per device sample for 120 samples. **Constraints**: no allocation in a call (the table and the ring at construction); the C API and the control manifest unchanged.

No NEEDS CLARIFICATION remains.

## Constitution Check

| Principle | How this plan complies | Pre | Post |
|---|---|---|---|
| I. Portable Library First | `no_std` core; no C API change; the header byte-identical. | ✅ | ✅ |
| II. Evidence-Based Fidelity | The crossfade is registered as the emulator's convention in A-054 (no device claim); the device's plausible behavior (a frame-boundary switch) and capture P1 recorded. | ✅ | ✅ |
| III. Measured, Test-First Verification | Every oracle written first; the blend identity bit-exact; the click reduction asserted with a reader defined for the case; the re-pins assert their identities at bless time. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | Plain IEEE arithmetic in a fixed order; a weight table and a larger ring at construction; bounded pulls. | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | Inside stage 9's speed stage; the manifest notes name the blend; every constant from the design script. | ✅ | ✅ |
| VI. Neutral, Honest Communication | "An engine convention, not a device behavior" in every record; the figures as the harness prints them. | ✅ | ✅ |
| VII. Independently Testable Features | US1 at 6 kHz through `speed_only` against the harness reference; US2 through the log and the manifest. | ✅ | ✅ |

No violation; no Complexity Tracking entry.

## Project Structure

```text
specs/019-speed-crossfade/  plan.md, research.md, data-model.md, contracts/api.md, quickstart.md, records.md, tasks.md (next)
crates/rr_dr60/src/speed.rs                 # CROSSFADE_SAMPLES, CROSSFADE_MS (public constants)
crates/rr_dr60/src/stages/speed.rs          # Fade state, the weight table, dot(bank, …), RING 256, the wait, the four mutants
crates/rr_dr60/src/lib.rs                   # SpeedMutation's four fields; a weights hook
crates/rr_dr60/src/manifest.rs              # stage 9's notes at 0.19.0
crates/rr_dr60_harness/src/analysis.rs      # click_energy_in (the 011 reader delegates)
crates/rr_dr60_harness/src/speed_checks.rs  # reference_output_at models the wait and the fade; value_at unchanged
crates/rr_dr60_harness/src/crossfade_checks.rs (new)
crates/rr_dr60_harness/src/golden.rs        # bless-time identities for the speed and changes re-pins
crates/rr_dr60_harness/src/smoke_reference.rs  # the speed case's hash and sample count re-pinned
crates/rr_dr60_harness/tests/us1_crossfade.rs, us2_crossfade.rs, crossfade_matrix.rs, crossfade_edge_cases.rs, crossfade_timing.rs
crates/rr_dr60_harness/tests/mutation.rs (mod crossfade), ffi_parity.rs, alloc_free.rs, determinism.rs, us1_manifest.rs (stage 9 pins)
crates/rr_dr60_harness/golden/golden-speed-v1.json, golden-changes-v1.json (re-blessed once each), manifest-v1.txt
crates/rr_dr60_ffi/tests/c/smoke.c          # RR_DR60_SMOKE_SPEED_HASH / _SAMPLES re-pinned
tools/playback-speed/speed_design.py (--crossfade)
scripts/ab-timing.sh (crossfade mode), .github/workflows/ab-timing.yml
docs/hardware/assumptions.md (A-054), .github/ISSUE_TEMPLATE/hardware_evidence.yml (P1), docs/guide.md, CHANGELOG.md, docs/product/*.md, CLAUDE.md
```

## Reviews

Pending: the coach, the hardware-emulation reviewer and the product specialist review this plan next; their findings are applied here and in research.md, data-model.md, contracts/api.md and quickstart.md before the owner's plan sign-off.
