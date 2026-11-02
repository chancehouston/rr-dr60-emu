# Implementation Plan: An Insertable Component Slot and the Band Filter

**Branch**: `024-insertable-component` | **Date**: 2026-11-01 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/024-insertable-component/spec.md` (approved D1 – D12 as clarified, 2026-11-01: `MAX_CHANGES_PER_CALL` 158 with a 16-parameter budget, statuses 41 – 43, version 0.92.0, baselines 0.91.0 / feec054, free for good, the engine side before engine 1.0)

## Decisions and findings (owner: read this section)

The spec's D1 – D12 stand. The three plan reviews (coach 1 – 23, hardware B1 – B17 with simulations, product M1 – M8; 2026-11-01) are applied at their defaults. **The owner was away ("run this on auto-pilot to the end"), so the plan's sign-off is taken at the merge**; the choices below that restate the signed spec or add to its surface are listed first for that sign-off. **Signed off by the owner 2026-11-02: R1 – R8 as built; R4 kept as 16 for all components together.**

**Taken at the default, for the owner's review at the merge:**

- **R1 (hardware B3, coach 2, product M2): an idle-only parameter setter.** `set_component_parameter(point, index, value)` (and its C twin; `set_control_value(32 – 35)` reaches it): refused while recording or playing (43), validated like the schedule, never a change entry. Without it a re-render cannot put a recording's filter values into a fresh engine, and an app cannot set "400 Hz, then Record". D3 said "no plain setter"; this is an idle restore like `set_component`, not a live setter.
- **R2 (hardware B2, coach 3): the filter's state is cleared at each recording's start**, as stage 2's is (`start_recording` resets the record path). The spec's edge case said it persists "like the amplifier's", but the amplifier's does not; a carried state would break the re-render identity on every recording after the first. Restated in the spec; the one-pass keeps it across calls.
- **R3 (coach 1): plan N1 is withdrawn; D9 holds.** The record readout is unchanged; a new sized struct `RrDr60RecordComponent` and two functions (`rr_dr60_processing_record_component`, `rr_dr60_record_write_with_component`) carry the component. Growing the readout would have failed the ABI promise script, whose `--abi` rows print each header's struct sizes.
- **R4 (hardware B7, coach 8): the parameter store is global**: each component's parameters at a fixed published offset, never reused (the band filter 0 – 3), all components together within the 16-parameter budget, as the owner's clarification chose. **The product review (M1) recommends the other reading**, 16 per component in the slot, because a reverb, a three-band EQ and a flanger together need about 21 – 26 parameters, so the budget fills at the third later component and the next one moves the ABI version. Not taken (it reverses the owner's answer); **the owner's question at the merge**: keep "16 for all components together", or change to "16 per component in the slot" (the number 158 unchanged either way; the change is cheap now, an ABI move after 1.0). **Owner 2026-11-02: keep 16 for all components together.**
- **R5 (plan N2, sound per the hardware review): one state, two constant mixes.** All four types are constant mixes `(m0, m1, m2) ∈ {−1, 0, 1}` of `(x, k·v1, v2)` of one state, so a type blend over two mixes equals the spec's two-section blend bit for bit. Restates the spec's "two sections" sentences (D3, Clarifications coach 1 and question 1, US1 AS2, Key Entities, FR-012).
- **R6 (hardware B5): the response oracle's impulse length** comes from the section's pole radius, not the spec's `8 Q / (π f) + 0.1 s` (which leaves −70 dB tails and ignores warping near the ceiling); **(coach 6)** the fifteen audio guards stay unmoved and the record golden moves once (form 2), where the spec said "the sixteen guards unmoved".
- **R8 (implementation): the ABI promise and an appended enum value.** The earlier smokes check that tap 11 is refused; `Tap::AfterInsert` assigns it, so `scripts/abi-promise.sh` tolerates exactly that check and its knock-on `struct_size = 4` (the run must complete: exit 1, its summary, the count equal to the tolerated lines; a self-test of the filter runs in CI). Appending an enum value is compatible by design; an old check that a then-unassigned value is refused is not part of spec 022's promise. Alternative: no C tap for the slot (D9 restated).
- **R7 (product M7): engine strings say "an addition of the emulator"** (the library serves other apps); app copy keeps "an addition of the app"; the gain sentence says "can keep" the recorder from pausing.

Findings applied without a decision: the placement identity on an f64 trace of the slot's input (B1, coach 4); the type blend's restart rule (B4); the transition on the applied, clamped frequency (B11, coach 11); the notch and stopband oracle (B9, coach 12); the named click reader (coach 13); the B1 row against transposed direct form II (B10); D6 and AS5 oracles and the gate bisection, no per-type figure quoted before the harness prints it (coach 14, B12); the pins of earlier specs restated (coach 15); spec 014's checks extended (coach 16); the 142 assertion (coach 17); the transition indexing and at-once cases (coach 18); the golden enumerated, 84 entries (coach 19); the records' wording and gaps (coach 20, B15, M8); the filter-name rule (coach 21); the requirement map (coach 22); the out-type and latency oracle (coach 23); `control_index`, `scale` and indexed enumeration in the manifests (M3, M4); a guide sentence on the one-pass's immediate component change (M6); the subnormal flush (B14); the cross-rate table (B16); traceability tags (B17); the store as the cross-stream carrier with its tags and seeding (B8, coach 9); the layout: the flag in `run_limiter`'s padding, `insert` after `ctl` (B13). The free-for-good sweep of the product docs is its own docs change on PR #68's branch (M8).

| # | Plan choice | Why |
|---|---|---|
| P1 | The slot in `RecordPath::front_from` between stage 2 and the limiter-input peak, one `run_insert` flag beside `run_limiter` in the path's hot line; `FrontStop::Insert` for the new tap; the stop's flush through the same path (R-01). | D1, D6, D8; FR-002, FR-007, FR-010. |
| P2 | `stages/band_filter.rs`: the topology-preserving state-variable section in `f64` with `g = sin/cos` from `detmath`, the four output mixes, the post-gain, the ceiling applied at the coefficient (R-02). | D4; FR-004, FR-012. |
| P3 | The transitions: a smoothstep in log-frequency, log-Q and dB from P, recomputing the coefficients per sample only while one runs; a second landing restarts from the current value; the type blend over two mixes of one state (N2) (R-02, R-03). | D3, D4; FR-005. |
| P4 | `components.rs` (the insertion-point list and the component manifest as constant data), the control manifest's entries 31 – 35 and the tap option, the control map's component target (R-04). | D1, D2, D5; FR-001, FR-006. |
| P5 | The parameter store in the engine; `set_component` idle-only, kept by `reset`/`reconfigure`; the record side's control state widened to 8 + 16 slots, `RECORD_LOG` 64 + 8 + 16, `MAX_CHANGES_PER_CALL` 158 with the attained 146 pinned (R-06, R-11). | D3, D5; FR-003, FR-005, FR-006. |
| P6 | The record form version 2: the slot block and the writer carry the component and the store; the reader reads 1 and 2; a new flag for an unknown component; the re-render sets the component and the store first (R1); C through `RrDr60RecordComponent` (R3) (R-05). | D7; FR-008. |
| P7 | `tools/band-filter/design_band_filter.py` and its generated harness table; the DTFT oracle and the placement identity; the hardware review's B1 simulation as a reported row (R-07). | D4; FR-004; SC-001. |
| P8 | `golden-insert-v1.json` (about 80 entries) with the empty-slot twin at bless time (R-08). | D10; SC-003. |
| P9 | The C surface (four new sized structs, no existing struct changed), cbindgen, `smoke --insert`, the parity rows, the promise script against three baselines, the Swift re-render (R-09). | D9; FR-009. |
| P10 | The `insert` A/B mode and `insert_timing` (R-10). | D8; FR-010; SC-005. |
| P11 | The six named mutants (research › The six mutants), the pins restated (R-11), the records (records.md). | D10, D12; FR-011. |

## Summary

A slot at one published place in the record path, between the microphone amplifier and the limiter at the host rate, that holds none (the default, bit-identical to 0.91) or the engine's band filter: one state-variable section with four types, a frequency, a Q and a gain, whose parameters are scheduled live in the input clock with a 20 ms transition and carried by the processing record so a session reproduces.

## Requirement map

| Requirement | Research | Tests |
|---|---|---|
| FR-001 the manifests | R-04 | core `components::`, `us2_insert` AS1, `us2_cross_check` |
| FR-002 the empty slot | R-01 | `us1_insert` AS3, the sixteen guards, the golden's twin |
| FR-003 the component's lifecycle | R-06 | core `pipeline` tests, `us1_insert` AS4, `insert_edge_cases` |
| FR-004 the filter | R-02, R-07 | core `stages::band_filter`, `us1_insert` AS1, `insert_matrix` |
| FR-005 the live parameters | R-02, R-03, R-06 | `us1_insert` AS2, `us2_insert` AS5, `mutation insert::` |
| FR-006 the control manifest and 158 | R-04, R-11 | `us1_controls`/`us2_controls`, core `changes::` (158, 146) |
| FR-007 the tap | R-01 | `us1_insert` AS3 |
| FR-008 the record | R-05 | `us2_insert` AS3, `golden_record`, `record_contract_*` |
| FR-009 the C surface | R-09 | `ffi_parity insert`, `smoke --insert`, `abi_tests`, `scripts/abi-promise.sh` ×3, Swift |
| FR-010 the cost | R-10 | `insert_timing`, `ab-timing insert` |
| FR-011 the wording | R-04 | `us2_cross_check` (the denylist) |
| FR-012 determinism, partitions, allocation | R-02 | `determinism insert`, `alloc_free insert`, `insert_matrix` partitions, the golden's six targets |
| FR-013 no latency | R-01 | `us1_insert` AS1 (the impulse at sample 0; the record and one-pass latencies unchanged at seven rates) |
| US1 AS5, D6 | R-07 | `us1_insert` AS5 (the loud row), the meters' limiter-input peak, `insert_matrix::gate_offset_bisection` |
| US2 AS5 the ceiling | R-02 | `us2_insert` AS5 |
| SC-004 the named deltas | R-05 | `us1_insert` AS3 re-render cells (none in the record with the filter in the engine; a parameter moved; the second recording) |
| US3 AS3 the footprint | R-05 | `insert_matrix` (the slot block's growth, reported) |

## Technical Context

**Language/Version**: Rust 1.85 (edition 2024), `no_std` + alloc core; Python 3 (uv) for the design script. **Dependencies**: none new. **Testing**: `cargo test`, the harness, the C smoke, Miri (the FFI only), the Swift package. **Performance**: the empty slot one predictable branch per host sample; the filter about ten multiply-adds per host sample; a transition adds one `sin`, one `cos` and up to three `exp` per host sample for 20 ms. **Constraints**: no allocation after construction (one section state, the store and the transition state fixed size); `detmath` only; the settings structure unchanged.

No NEEDS CLARIFICATION remains.

## Constitution Check

| Principle | How this plan complies | Pre | Post |
|---|---|---|---|
| I. Portable Library First | Pure Rust in the core; the C surface appended, no existing struct changed; the header regenerated. | ✅ | ✅ |
| II. Evidence-Based Fidelity | An emulator convenience, not a device behavior: no assumption, no stage-manifest row; every figure an engineering target cited to the spec (D4's ranges, the 20 ms of A-045's convention, the 0.45 ceiling). | ✅ | ✅ |
| III. Measured, Test-First Verification | The response pinned by a design script; the placement identity bit-exact; tests first per story; a golden. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | `detmath` only, fixed-order arithmetic, no allocation, bounded work per sample. | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | The slot is a published insertion point with its own tap, bypassed (empty) by default. | ✅ | ✅ |
| VI. Neutral, Honest Communication | Every sentence names the filter an addition of the app and says what it does to a tone; the denylist applies. | ✅ | ✅ |
| VII. Independently Testable Features | The oracle configuration (stages 1 – 2 and the limiter bypassed) tests the filter alone. | ✅ | ✅ |

No violation; no Complexity Tracking entry.

## Project Structure

```text
specs/024-insertable-component/  plan.md, research.md, data-model.md, contracts/api.md, quickstart.md, records.md, tasks.md (next)
crates/rr_dr60/src/stages/band_filter.rs   # the section, the mixes, the transitions, the type blend
crates/rr_dr60/src/stages/insert.rs        # the slot: none or the band filter; the run flag
crates/rr_dr60/src/components.rs           # insertion points, the component manifest, the ceiling function
crates/rr_dr60/src/{paths.rs, changes.rs, pipeline.rs, transport.rs, memory.rs, record.rs, controls.rs, control_map.rs, settings.rs (Tap), error.rs, lib.rs (hooks)}
crates/rr_dr60_ffi/src/lib.rs, include/rr_dr60.h, tests/c/smoke.c (--insert)
crates/rr_dr60_harness/src/{insert_checks.rs, band_filter_ref.rs (generated)}, examples/insert_rows.rs,
  tests/{us1_insert, us2_insert, insert_matrix, insert_edge_cases, golden_insert, insert_timing}.rs, mutation.rs, alloc_free.rs, determinism.rs, ffi_parity.rs
tools/band-filter/design_band_filter.py, scripts/ab-timing.sh, .github/workflows/{ci,ab-timing}.yml
swift/Sources/RRDR60 (the re-render), docs, product notes, CLAUDE.md per records.md
```

## Reviews

The coach (1 – 23), the hardware-emulation reviewer (B1 – B17, with simulations in the session scratchpad: `sim.py`, `click.py`, `click2.py`, `zeros*.py`) and the product specialist (M1 – M8) reviewed this plan on 2026-11-01; every finding is applied above and in the design documents at the reviewers' defaults (where coach 1 and hardware B6 disagreed on N1, coach 1's promise-script evidence decided), and the spec carries the restatements as a dated Clarifications session. The plan's sign-off is the owner's at the merge (R1 – R7).
