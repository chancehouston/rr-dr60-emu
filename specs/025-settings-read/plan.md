# Implementation Plan: The Settings Read and Control Access by Index

**Branch**: `025-settings-read` | **Date**: 2026-11-01 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/025-settings-read/spec.md` (approved D1 – D11, 2026-11-01)

## Decisions and findings (owner: read this section)

The spec's D1 – D11 stand; the plan's choices P1 – P7 below follow them. The plan reviews will be applied at their defaults and listed here.

| # | Plan choice | Why |
|---|---|---|
| P1 | A new core module `control_map.rs`: the exhaustive mapping control index → a setting accessor (26) or a live control (5), built from one `match` on the manifest identifier that is itself checked exhaustively against `Settings` (R-01); `ControlReading` (R-02). | D2, D3, D5; FR-002, FR-003, FR-005. |
| P2 | `Pipeline::control_value`, `Pipeline::set_control_value` through the five setters with spec 015 R-04's conversion (R-04); `settings()` unchanged in meaning (R-05). | D1, D2, D4; FR-001, FR-002, FR-004. |
| P3 | The processing record's `get`/`set` rebuilt on the mapping; its text form and its golden unmoved (R-08). | D5; FR-005. |
| P4 | The C surface: the six functions, `RrDr60ControlReading` (24 bytes), statuses 39 – 40, the field-local write through raw pointers, the in-place settings fill, the refusal order (R-03, R-06). | D1 – D4, D6, D7; FR-001 – FR-004, FR-006, FR-007. |
| P5 | The oracles: the one-hot test in Rust and C, the change-log identity, the mutants (R-07). | D5; FR-005; SC-001 – SC-004. |
| P6 | The Swift wrapper and its test (R-09). | D10; FR-010. |
| P7 | The records, the promise script's second revision, the version, the figures (R-10). | D8, D11; FR-008. |

## Summary

The engine gains a read of its configured settings and access to every control by the control manifest's index, on one exhaustive mapping inside the core that also backs the processing record. No audio path changes.

## Requirement map

| Requirement | Research | Tests |
|---|---|---|
| FR-001 the read | R-03, R-05 | `us1_settings_read` (AS1 – AS4), `ffi_parity settings_read`, the Miri gate |
| FR-002 the reading by index | R-02 | `us2_control_index` AS1 |
| FR-003 settings access | R-01, R-03 | `us2_control_index` AS2, AS4 (one-hot Rust and C, representability, repair) |
| FR-004 live by index | R-04 | `us2_live_by_index` AS3, AS5 |
| FR-005 one mapping | R-01, R-07, R-08 | core `control_map` tests, the one-hot oracle, `golden_record` byte-identical |
| FR-006 the field lookup | R-01 | `us2_control_index` (every field ↔ identifier) |
| FR-007 the refusal order | R-06 | `settings_read_edge_cases`, `abi_tests` |
| FR-008 nothing moved | R-10 | the golden guards, `us2_abi`, `scripts/abi-promise.sh` ×2 |
| FR-009 real time | R-03 | `alloc_free settings_read`, `determinism settings_read` |
| FR-010 Swift | R-09 | `swift test` (`ControlReadingTests`) |

## Technical Context

**Language/Version**: Rust 1.85 (edition 2024), `no_std` + alloc core; C11 for the smoke; Swift 6 for the package. **Dependencies**: none new. **Testing**: `cargo test`, the harness, the C smoke, Miri (the FFI writes), `swift test`. **Target**: the existing seven host rates and every CI target. **Performance**: constant work per call; no audio path change. **Constraints**: real-time safe, allocation-free, panic-guarded. No NEEDS CLARIFICATION remains.

## Constitution Check

- **I Portable library**: core changes are pure Rust; the C API stays panic-safe with no Rust-only types. Pass.
- **II Evidence-based fidelity**: no modeled value changes; no assumption. Pass.
- **III Measured, test-first**: the one-hot oracle and the change-log identity are written first; coverage within 1 point. Pass.
- **IV Deterministic, real-time safe**: constant work, no allocation, no audio change. Pass.
- **V Modular, traceable**: one module; the mapping tested against the manifest both ways. Pass.
- **VI Neutral communication**: the guide's paragraph passes the denylist. Pass.
- **VII Independently testable**: each story's tests run alone. Pass.

## Project Structure

```text
crates/rr_dr60/src/control_map.rs     # new: the mapping, ControlReading, the settings accessors
crates/rr_dr60/src/pipeline.rs        # control_value, set_control_value
crates/rr_dr60/src/record.rs          # get/set rebuilt on the mapping
crates/rr_dr60/src/error.rs           # NotASetting, NotAPlainLiveControl
crates/rr_dr60_ffi/src/lib.rs         # six functions, RrDr60ControlReading, statuses 39 – 40
crates/rr_dr60_ffi/include/rr_dr60.h  # regenerated
crates/rr_dr60_ffi/tests/c/smoke.c    # --settings, the C one-hot comparison
crates/rr_dr60_harness/tests/         # us1_settings_read, us2_control_index, us2_live_by_index, settings_read_edge_cases, mutation settings_read
crates/rr_dr60_harness/examples/settings_rows.rs
swift/Sources/RRDR60/ControlReading.swift, swift/Tests/RRDR60Tests/ControlReadingTests.swift
.github/workflows/ci.yml              # the promise script with f5a0245; the Miri list
```

## Complexity Tracking

None.
