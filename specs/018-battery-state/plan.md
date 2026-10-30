# Implementation Plan: Battery State (the static law)

**Branch**: `018-battery-state` | **Date**: 2026-10-30 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/018-battery-state/spec.md` (approved D1 – D11, 2026-10-30)

## Decisions and findings (owner: read this section)

The spec's D1 – D11 stand, with these figures restated by the plan reviews (coach 1 – 14, hardware B1 – B15, product M1 – M9, 2026-10-30, applied at their defaults):

1. **The control is appended at manifest index 29** (coach 1, hardware B15): inserting it among the three live entries would shift every setting's index and move `golden-record-changes-v1.json`, whose rows store control indices; spec 014's order rule is restated as "the 0.14 entries in their order; later entries appended (indices are stable)", recorded for the pre-1.0 ABI pass.
2. **The trajectory oracle is the sample peak, bit-exact** (coach 2, hardware B1): on a tone 6 dB over the ceiling every clipped sample is the ceiling branch's `b` itself, so `max |y|` over the last second equals `ceiling_factor(v)` to the bit at every rate; the AES17 amplitude of a flat-topped tone reads about +1.9 dB above it and is reported, not asserted. The ⌊N/4⌋ check uses each rate's own u (882 does not divide by 4; hardware B3).
3. **The figures are pinned bit-exact to the script's f32 print** (hardware B2): the law returns f32, so "within 1e-9" is replaced by bit equality on the factor and 1e-5 on `ceiling_db`.
4. **The meters golden is not re-blessed** (coach 3, hardware B4): `meters_checks::row` keeps its eleven columns; a `battery_row` (the two new fields) serves the battery golden, the example and the C smoke.
5. **The meters' size rule becomes a compatible prefix fill** (hardware B5): a `struct_size` below 56 is refused, 56 … 63 is filled to the size (the 0.17 layout), 64 and above filled with the tail zeroed; spec 016's rule gets a dated amendment, so a 0.17 binary keeps working.
6. **`set_battery` is idempotent against its target** (hardware B6): a request equal to the current target returns early, so the per-call plain apply cannot restart a transition.
7. **The committed law form is the register's, `(volts − 1.8) ÷ 1.2` in f64 narrowed to f32** (hardware B7, coach 4): exactly 1.0 at 3.0 in f64 (3.0 − 1.8 is exact and equals the double nearest 1.2); the chain's factor is the widened f32, so the engine and the published law agree to the bit; the parabola keeps its stored denominator (a reciprocal is inexact at b = 1).
8. **The published law is total over finite inputs in both languages** (coach 8); refusal belongs to the setter (`INVALID_BATTERY_VOLTS`).
9. **The stage notes that fit** (coach 5, 6; hardware B14): stage 11's coverage "the 159 Hz corner's knob shift; the clip level's spread; the battery's sag; wiper noise; knob speed; the beeps" (109 characters, the five pinned words kept); stage 15's controls "set_battery_volts, battery_volts; Meters" and coverage "the 3.3 V converter and its rails; the cells' sag under load; supply noise; the low-battery indicator and cut-off".
10. **A NiMH constant joins the public data and the trajectory's rows are documented** (product M1, M2): `battery::NIMH_PAIR_VOLTS` (2.45 V, A-056); the rows are hours of **recording** under S-001's load and row 0 (2.96 V, loaded) is not the default: "fresh" is `DEFAULT_BATTERY_VOLTS`.
11. **The manifest sentence names the host device, not a sensor** (product M3, coach 11): "sensor" trips the denylist stem; "Sets the played unit's cell voltage: the lower the cells, the lower the speaker amplifier's clip ceiling; set by the user, never read from the host device." (153 characters, no digit); the guide carries the three words (the phone's battery, a sensor, a timer).
12. **Smaller items**: the clip count counts samples the knee or ceiling changed (`u > k·b`; hardware B12) and is finalized under the playback group's gate at taps from `AfterVolumeAmp` on the speaker route with stage 11 on (coach 7); `battery_ceiling` rests at the engine's factor (B11); `reset_sections` ends a battery transition and recomputes the thresholds, `__mutate` recomputes them too (B8, B9); the below-knee premise is asserted on the reconstruction peak (B10); the clamp engages on clipped material above about 2.32 – 2.49 V by host rate (B12); NiMH at 2.5 V is −4.7 dB (B13); the golden is 259 entries with the earphone and no-op twins asserted at the bless (coach 9); `battery_clipped_timing.rs` is the reported non-default cost (coach 10); the coverage baseline is CI's 97.35 % (coach 12); the records list is completed (coach 13, product M7); the trajectory constants are generated into `output_coeffs.rs` and re-exported by `battery.rs` (coach 14).

Plan choices, each traced to the research:

| # | Plan choice | Why |
|---|---|---|
| P1 | A fourth live control beside the route, the volume and the speed: `Pipeline::{battery_volts, set_battery_volts}` (f32), `LIVE_BATTERY` = 29 (appended) in `changes.rs`, `Apply::Battery(f32)` to the chain, spec 015's `StreamState` carrying `last_battery` / `pending_battery` / `landed_battery`, the readings following landed values; kept through `reset` and `reconfigure` (R-01). | D1, FR-001; the same plumbing the volume uses, so every 015 oracle applies. |
| P2 | In `OutputChain` a second transition state on the factor b (`batt`, `batt_from`, `batt_to`, `batt_k`, `batt_n`), stepped once per sample while `batt_k < batt_n` along the smoothstep on b, no hold; taken at once in the volume's at-once cases plus a route change (R-02). | D4, FR-003; independent of the volume's `ramp_k` so two transitions never share a scalar. |
| P3 | The clipper generalized: `knee_b(x, k, b)` with thresholds `k·b`, `(2 − k)·b`, ceiling `b` and the parabola `b − d² ÷ (4·(1 − k)·b)`; the sample untouched below `k·b`; at b = 1.0 bit-identical to `knee` (R-03). | D3, FR-002, FR-005. |
| P4 | `battery::ceiling_factor(v) = (v − 1.8) ÷ 1.2` (f64, narrowed; exactly 1.0 at 3.0), computed once per change; `ceiling_db`; `ALKALINE_TRAJECTORY` and `NIMH_PAIR_VOLTS` as constant data generated from the design script's `--battery` print (R-04). | D2, D11, FR-006, FR-011. |
| P5 | The meters: `amp_clip_count` and `battery_ceiling` appended to `Meters` and `RrDr60Meters` (56 → 64 bytes) under a compatible prefix-fill size rule, accumulated in the speaker branch where the knee is applied, finalized under the playback group's gate; `meters_checks::row` unchanged, a `battery_row` for the new golden (R-05). | D10, FR-010. |
| P6 | The control manifest entry on stage 15; stage 15 → assumed at 0.18.0; stage 11's coverage note reworded; A-056 adopted; A-031 and S-017 (R-06). | D7, FR-008, FR-009. |
| P7 | Oracles: the hard-clipped tone at `VOLUME_6DB_OVER` (the sample peak equals b to the bit), the below-knee identity with its premise asserted, the knee table shifted, the factor trace at ⌊N/4⌋, the splice, the partitions, the clamp matrix row (R-07). | D5, SC-001 – SC-004. |
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
| FR-007 cost, determinism, allocation | R-08 | `battery_timing`, `battery_clipped_timing` (reported), `scripts/ab-timing.sh 24ffab9 battery`, `alloc_free`, `determinism` |
| FR-008 the manifest entries | R-06 | `us1_controls`, `us2_controls`, `us1_manifest`, `us2_cross_check` |
| FR-009 the records | R-06, R-09 | `us2_cross_check` (registers), the CHANGELOG |
| FR-010 the meters | R-05 | `us2_battery` AS4, `meters` tests restated for the size |
| FR-011 the public law | R-04 | core, `ffi_parity battery_parity`, the C smoke |
| FR-012 spec 014's checks | R-06 | `us1_controls`, `us2_controls` |
| SC-001 – SC-006 | as above | `us1_battery`/`battery_matrix` 0 mismatches; the guards; the A/B; coverage |

## Technical Context

**Language/Version**: Rust 1.85 (edition 2024). **Dependencies**: none new; `no_std` + `alloc`. **Testing**: `cargo test` (core, `us1_battery`, `us2_battery`, `battery_matrix`, `battery_edge_cases`, `mutation battery::`, `golden_battery`, `ffi_parity`, `alloc_free`, `determinism`, the controls and manifest suites), the C smoke, the Swift parity, the A/B. **Target**: every supported host rate; the iOS targets. **Constraints**: real-time safe, deterministic, no allocation in the processing path; bit-identical at the default. **Scale**: one scalar control; two meter fields; one golden of 6 × 2 × 3 × 7 = 252 entries plus 7 scheduled-change entries = 259.

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
crates/rr_dr60/src/battery.rs            # ceiling_factor, ceiling_db, ALKALINE_TRAJECTORY, NIMH_PAIR_VOLTS (re-exported from the generated constants), BATTERY_VOLTS_RANGE, DEFAULT_BATTERY_VOLTS
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
crates/rr_dr60_harness/tests/us1_battery.rs, us2_battery.rs, battery_matrix.rs, battery_edge_cases.rs, golden_battery.rs, battery_timing.rs, battery_clipped_timing.rs
crates/rr_dr60_harness/tests/mutation.rs, ffi_parity.rs, alloc_free.rs, determinism.rs, us1_manifest.rs (the pinned notes), us1_controls.rs (the twelve live entries)
crates/rr_dr60_harness/examples/battery_rows.rs
crates/rr_dr60_harness/golden/golden-battery-v1.json (new), control-manifest-v1.txt, manifest-v1.txt (re-bless)
tools/output-chain/design_output_chain.py (--battery)
scripts/ab-timing.sh (battery mode), .github/workflows/ab-timing.yml, ci.yml / ios-device-golden.sh (the golden list)
docs/hardware/assumptions.md (A-056, A-031), docs/hardware/signal-chain.md (rows 11, 15), docs/guide.md, README.md, CHANGELOG.md, docs/product/*.md, CLAUDE.md
```

## Reviews

The coach (1 – 14), the hardware-emulation reviewer (B1 – B15) and the product specialist (M1 – M9) reviewed this plan on 2026-10-30; every finding is applied above and in research.md, data-model.md, contracts/api.md, quickstart.md and records.md, and the spec's restatements are dated in its Clarifications. The owner signs off the plan next.
