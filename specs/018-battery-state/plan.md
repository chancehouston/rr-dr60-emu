# Implementation Plan: Battery State (the static law)

**Branch**: `018-battery-state` | **Date**: 2026-10-30 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/018-battery-state/spec.md` (approved D1 – D11, 2026-10-30)

## Decisions and findings (owner: read this section)

The spec's D1 – D11 stand. The plan reviews (coach, hardware, product) are applied at their defaults below once they arrive; nothing in this plan moves a spec decision.

Plan choices, each traced to the research:

| # | Plan choice | Why |
|---|---|---|
| P1 | A fourth live control beside the route, the volume and the speed: `Pipeline::{battery_volts, set_battery_volts}` (f32), `LIVE_BATTERY` in `changes.rs`, `Apply::Battery(f32)` to the chain, spec 015's `StreamState` carrying `last_battery` / `pending_battery` / `landed_battery`, the readings following landed values; kept through `reset` and `reconfigure` (R-01). | D1, FR-001; the same plumbing the volume uses, so every 015 oracle applies. |
| P2 | In `OutputChain` a second transition state on the factor b (`batt`, `batt_from`, `batt_to`, `batt_k`, `batt_n`), stepped once per sample while `batt_k < batt_n` along the smoothstep on b, no hold; taken at once in the volume's at-once cases plus a route change (R-02). | D4, FR-003; independent of the volume's `ramp_k` so two transitions never share a scalar. |
| P3 | The clipper generalized: `knee_b(x, k, b)` with thresholds `k·b`, `(2 − k)·b`, ceiling `b` and the parabola `b − d² ÷ (4·(1 − k)·b)`; the sample untouched below `k·b`; at b = 1.0 bit-identical to `knee` (R-03). | D3, FR-002, FR-005. |
| P4 | `battery::ceiling_factor(v) = (v − 1.8) ÷ 1.2`, computed once per change; `ceiling_db`; `ALKALINE_TRAJECTORY` as constant data from the design script's `--battery` print (R-04). | D2, D11, FR-006, FR-011. |
| P5 | The meters: `amp_clip_count` and `battery_ceiling` appended to `Meters` and `RrDr60Meters` (56 → 64 bytes) under the size rule, accumulated in the speaker branch where the knee is applied (R-05). | D10, FR-010. |
| P6 | The control manifest entry on stage 15; stage 15 → assumed at 0.18.0; stage 11's coverage note reworded; A-056 adopted; A-031 and S-017 (R-06). | D7, FR-008, FR-009. |
| P7 | Oracles: the hard-clipped tone at `VOLUME_6DB_OVER` (peak = b), the below-knee identity, the knee table shifted, the factor trace at ⌊N/4⌋, the splice, the partitions, the clamp matrix row (R-07). | D5, SC-001 – SC-004. |
| P8 | Cost by `battery_timing.rs` against the 0.17 API in a `battery` mode of the A/B script (3 %, 5 % runner); the non-default cost reported (R-08). | D6, FR-007. |
| P9 | `golden-battery-v1.json`, the C smoke `--battery` against `examples/battery_rows.rs`, the records (R-09). | FR-005, FR-009 – FR-012. |

## Summary

The speaker amplifier's clip ceiling becomes a live unit condition: a cell voltage whose factor b scales the clipper's thresholds (never the sample), moving over 20 ms on its own transition, with the clip count and the factor in the meters, the law and the alkaline trajectory published, the control on stage 15, A-056 adopted as the static law. At the default nothing moves.

## Requirement map

| Requirement | Research | Tests |
|---|---|---|
| FR-001 the control, the streams, the refusal | R-01 | `us2_battery` AS1 – AS2, `battery_edge_cases`, FFI unit tests |
| FR-002 the scaled clipper, the below-knee identity | R-03 | `us1_battery` AS2, `battery_matrix`, core `knee_b` pins |
| FR-003 the transition | R-02 | `us1_battery` AS4, core factor trace, `mutation battery::` |
| FR-004 the other paths unmoved | R-03 | `us1_battery` AS5 |
| FR-005 the goldens, the new golden | R-09 | the twelve guards, `golden_battery` |
| FR-006 the design-script figures | R-04 | core pins, `us1_battery` AS3 |
| FR-007 cost, determinism, allocation | R-08 | `battery_timing`, `scripts/ab-timing.sh 24ffab9 battery`, `alloc_free`, `determinism` |
| FR-008 the manifest entries | R-06 | `us1_controls`, `us2_controls`, `us1_manifest`, `us2_cross_check` |
| FR-009 the records | R-06, R-09 | `us2_cross_check` (registers), the CHANGELOG |
| FR-010 the meters | R-05 | `us2_battery` AS4, `meters` tests restated for the size |
| FR-011 the public law | R-04 | core, `ffi_parity battery_parity`, the C smoke |
| FR-012 spec 014's checks | R-06 | `us1_controls`, `us2_controls` |
| SC-001 – SC-006 | as above | `us1_battery`/`battery_matrix` 0 mismatches; the guards; the A/B; coverage |

## Technical Context

**Language/Version**: Rust 1.85 (edition 2024). **Dependencies**: none new; `no_std` + `alloc`. **Testing**: `cargo test` (core, `us1_battery`, `us2_battery`, `battery_matrix`, `battery_edge_cases`, `mutation battery::`, `golden_battery`, `ffi_parity`, `alloc_free`, `determinism`, the controls and manifest suites), the C smoke, the Swift parity, the A/B. **Target**: every supported host rate; the iOS targets. **Constraints**: real-time safe, deterministic, no allocation in the processing path; bit-identical at the default. **Scale**: one scalar control; two meter fields; one golden of 6 × 2 × 3 × 7 = 252 entries.

No NEEDS CLARIFICATION remains.

## Constitution Check

| Principle | How this plan complies | Pre | Post |
|---|---|---|---|
| I. Portable Library First | `no_std` core; the C API additive (the meters grow under the size rule, one status); header regenerated and drift-checked. | ✅ | ✅ |
| II. Evidence-Based Fidelity | A-056 adopted with its sources and confidence; every figure from the design script; the guide says "modeled on the circuit and the datasheet, not measured". | ✅ | ✅ |
| III. Measured, Test-First Verification | Every oracle written first; the twelve guards prove no audio change; the A/B asserts the cost. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | Plain IEEE arithmetic; one divide per transition sample; the factor computed once per change; `alloc_free` covers the calls. | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | The control on stage 15 with its note; the clipper's scaling in stage 11; the manifest re-blessed. | ✅ | ✅ |
| VI. Neutral, Honest Communication | The product rule; "not measured"; the claims wording of the product review. | ✅ | ✅ |
| VII. Independently Testable Features | US1 through the tap after stage 11; US2 through the manifest, the meters and C. | ✅ | ✅ |

No violation; no Complexity Tracking entry.

## Project Structure

```text
specs/018-battery-state/  plan.md, research.md, data-model.md, contracts/api.md, quickstart.md, records.md, tasks.md (next)
crates/rr_dr60/src/battery.rs            # ceiling_factor, ceiling_db, ALKALINE_TRAJECTORY, BATTERY_VOLTS_RANGE, DEFAULT_BATTERY_VOLTS
crates/rr_dr60/src/changes.rs            # LIVE_BATTERY, Apply::Battery, StreamState's battery fields
crates/rr_dr60/src/stages/output.rs      # knee_b, the battery transition state, set_battery, the clip accumulators
crates/rr_dr60/src/stages/output_coeffs.rs  # the trajectory rows and the law constants (generated)
crates/rr_dr60/src/paths.rs              # PlaybackPath: Apply::Battery routing, finalize_into's two fields
crates/rr_dr60/src/pipeline.rs           # battery_volts/set_battery_volts, the readings, Meters fields, schedule_change's fourth control
crates/rr_dr60/src/transport.rs          # start_playback's start values
crates/rr_dr60/src/controls.rs           # the battery_volts entry
crates/rr_dr60/src/manifest.rs           # stage 15 assumed, stage 11's coverage note
crates/rr_dr60/src/error.rs              # InvalidBatteryVolts
crates/rr_dr60_ffi/src/lib.rs, cbindgen.toml, include/rr_dr60.h, tests/c/smoke.c (--battery), tests/c/run_smoke.sh
crates/rr_dr60_harness/src/battery_checks.rs, golden.rs (BatteryGolden, generate_battery), configs.rs (battery configurations)
crates/rr_dr60_harness/tests/us1_battery.rs, us2_battery.rs, battery_matrix.rs, battery_edge_cases.rs, golden_battery.rs, battery_timing.rs
crates/rr_dr60_harness/tests/mutation.rs, ffi_parity.rs, alloc_free.rs, determinism.rs, us1_manifest.rs (the pinned notes), us1_controls.rs (the twelve live entries)
crates/rr_dr60_harness/examples/battery_rows.rs
crates/rr_dr60_harness/golden/golden-battery-v1.json (new), control-manifest-v1.txt, manifest-v1.txt (re-bless)
tools/output-chain/design_output_chain.py (--battery)
scripts/ab-timing.sh (battery mode), .github/workflows/ab-timing.yml, ci.yml / ios-device-golden.sh (the golden list)
docs/hardware/assumptions.md (A-056, A-031), docs/hardware/signal-chain.md (rows 11, 15), docs/guide.md, README.md, CHANGELOG.md, docs/product/*.md, CLAUDE.md
```

## Reviews

The coach, the hardware-emulation reviewer and the product specialist review this plan next; their findings are applied here and in the design documents before the owner's plan sign-off.
