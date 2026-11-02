# Implementation Plan: An Insertable Component Slot and the Band Filter

**Branch**: `024-insertable-component` | **Date**: 2026-11-01 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/024-insertable-component/spec.md` (approved D1 – D12 as clarified, 2026-11-01: `MAX_CHANGES_PER_CALL` 158 with a 16-parameter budget, statuses 41 – 43, version 0.92.0, baselines 0.91.0 / feec054, free for good, the engine side before engine 1.0)

## Decisions and findings (owner: read this section)

The spec's D1 – D12 stand. Two findings of the plan restate a signed-off decision and need the owner's plan sign-off:

- **N1 (restates D9's "no change to any existing struct"): the C record readout grows by an appended tail.** A C or Swift caller reads a processing record through `RrDr60RecordReadout` and writes one through `rr_dr60_record_write(readout)`; D7 puts the component and its four parameters in the record, so without a change C could neither read nor write them. Recommended: append `insert_before_limiter` (`uint32_t`, the component's value) and `component_parameters[16]` (`float`, the store by the component's parameter order, the 16-parameter budget) at the end of the struct, under spec 022's published-layout rule (`READOUT_PUBLISHED` `[376, 448]`): a caller compiled against 0.91 passes 376 and receives the 0.91 prefix exactly, and a 376-byte readout handed to the pure writer writes form version 2 with an empty slot. This is the mechanism spec 022 built for this case (spec 018's meters grew the same way); the promise script's byte-identical oracle holds. Alternative: a separate sized struct `RrDr60RecordComponent` with `rr_dr60_processing_record_component` and `rr_dr60_record_write_with_component` (D9 kept literally; two more functions, and the Swift re-render reads two structs).
- **N2 (restates D3's "two sections reserved per slot"): one filter state, two output mixes.** In the state-variable form all four types are linear combinations of one state's three signals (input, band, low), so a type change's 20 ms blend is a blend of two output mixes of one state: `y = mix_old·s + w·(mix_new·s − mix_old·s)`. Two sections fed the same input from the same state at P compute the same state, so this is the spec's blend with half the arithmetic and no state copy. Recommended: one state, the blend over the mixes; the `type_instant` mutant (the mix switched at P) is still the click reference. Alternative: two sections as the draft says (twice the per-sample cost during a type blend, a state copy at P).

Findings applied without a decision (restated in the spec at the plan's sign-off):

1. **The parameters are stored as single-precision values** (research R-06): the schedule narrows a parameter's value to `f32` as spec 017's record-side values are, so the store, the change entries, the record and the C reading agree bit for bit; the type and the component are whole numbers in the same `f64` entry.
2. **The word "slot"** names the component's place (the spec's term) and spec 020's per-recording "slot block" stays; in code the place is `Insert` (`stages/insert.rs`) so the two never share an identifier.
3. **The control manifest's `tap` entry gains the option `after_insert` (11)**: the tap is a setting, so its choice list grows, a re-bless the spec's D10 already counts.
4. **The one-pass's front taps**: `Tap::AfterInsert` joins `is_front()`; at that tap the limiter does not run and the limiter-input peak reads the slot's output (D6).
5. **The component in the control map** (spec 025): indices 31 – 35 are neither settings fields nor plain live controls. `control_value` reads them (31 the component, 32 – 35 the store), `set_control_value(31, …)` calls `set_component`, 32 – 35 return `NotAPlainLiveControl` (status 40, the schedule's), and `rr_dr60_settings_control_value` returns `NOT_A_SETTING` (39).

| # | Plan choice | Why |
|---|---|---|
| P1 | The slot in `RecordPath::front_from` between stage 2 and the limiter-input peak, one `run_insert` flag beside `run_limiter` in the path's hot line; `FrontStop::Insert` for the new tap; the stop's flush through the same path (R-01). | D1, D6, D8; FR-002, FR-007, FR-010. |
| P2 | `stages/band_filter.rs`: the topology-preserving state-variable section in `f64` with `g = sin/cos` from `detmath`, the four output mixes, the post-gain, the ceiling applied at the coefficient (R-02). | D4; FR-004, FR-012. |
| P3 | The transitions: a smoothstep in log-frequency, log-Q and dB from P, recomputing the coefficients per sample only while one runs; a second landing restarts from the current value; the type blend over two mixes of one state (N2) (R-02, R-03). | D3, D4; FR-005. |
| P4 | `components.rs` (the insertion-point list and the component manifest as constant data), the control manifest's entries 31 – 35 and the tap option, the control map's component target (R-04). | D1, D2, D5; FR-001, FR-006. |
| P5 | The parameter store in the engine; `set_component` idle-only, kept by `reset`/`reconfigure`; the record side's control state widened to 8 + 16 slots, `RECORD_LOG` 64 + 8 + 16, `MAX_CHANGES_PER_CALL` 158 with the attained 146 pinned (R-06, R-11). | D3, D5; FR-003, FR-005, FR-006. |
| P6 | The record form version 2: the slot block and the writer carry the component and the store; the reader reads 1 and 2; a new flag for an unknown component; the re-render sets the component first (R-05). | D7; FR-008. |
| P7 | `tools/band-filter/design_band_filter.py` and its generated harness table; the DTFT oracle and the placement identity; the hardware review's B1 simulation as a reported row (R-07). | D4; FR-004; SC-001. |
| P8 | `golden-insert-v1.json` (about 80 entries) with the empty-slot twin at bless time (R-08). | D10; SC-003. |
| P9 | The C surface (N1), cbindgen, `smoke --insert`, the parity rows, the promise script against three baselines, the Swift re-render (R-09). | D9; FR-009. |
| P10 | The `insert` A/B mode and `insert_timing` (R-10). | D8; FR-010; SC-005. |
| P11 | The six mutants, the records (records.md). | D10, D12; FR-011. |

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
| FR-013 no latency | R-01 | `us1_insert` AS1 (the impulse at sample 0) |

## Technical Context

**Language/Version**: Rust 1.85 (edition 2024), `no_std` + alloc core; Python 3 (uv) for the design script. **Dependencies**: none new. **Testing**: `cargo test`, the harness, the C smoke, Miri (the FFI only), the Swift package. **Performance**: the empty slot one predictable branch per host sample; the filter about ten multiply-adds per host sample; a transition adds one `sin`, one `cos` and up to three `exp` per host sample for 20 ms. **Constraints**: no allocation after construction (one section state, the store and the transition state fixed size); `detmath` only; the settings structure unchanged.

No NEEDS CLARIFICATION remains.

## Constitution Check

| Principle | How this plan complies | Pre | Post |
|---|---|---|---|
| I. Portable Library First | Pure Rust in the core; the C surface appended (the readout's tail under spec 022's rule, N1); the header regenerated. | ✅ | ✅ |
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

The coach, the hardware-emulation reviewer and the product specialist review this plan next; their findings are applied at their defaults and listed here before the owner's plan sign-off.
