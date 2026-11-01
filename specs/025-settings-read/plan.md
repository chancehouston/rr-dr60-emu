# Implementation Plan: The Settings Read and Control Access by Index

**Branch**: `025-settings-read` | **Date**: 2026-11-01 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/025-settings-read/spec.md` (approved D1 – D11, 2026-11-01)

## Decisions and findings (owner: read this section)

The spec's D1 – D11 stand, with four narrowings from the plan reviews (coach 1 – 14, hardware B1 – B9, product M1 – M5; 2026-11-01), dated in the spec's Clarifications and applied at their defaults:

1. **The Rust one-hot oracle is a core test** (hardware B1, critical): the settings structures are `#[non_exhaustive]`, so only the core crate can destructure them exhaustively; the harness keeps the C one-hot (the C settings struct is plain).
2. **The control table is an array in manifest order, read in O(1)**, closed by three checks (identifiers equal by index, a compile-time destructure of `Settings`, a variant count of 26) (coach 5, hardware B2).
3. **The live setters are reached without an f64 round trip**, and the reachable statuses by index are `INVALID_VOLUME`, `INVALID_SPEED_LEVEL`, `INVALID_BATTERY_VOLTS` and `INVALID_ARGUMENT` (a NaN volume by index is `INVALID_ARGUMENT`, the plain setter's `INVALID_VOLUME`; documented) (coach 2, hardware B3, B4).
4. **The reading stays out of the frozen compatibility rows** (`--abi`, `abi_rows`, the Swift ABI fixture), so the promise script's byte diff with 0.22 and 0.90 holds; `--settings` prints it (coach 1).

Also applied: the field lookup on the raw `u32` and the core's `error::Setting` (no `SettingField` type exists); field accesses unaligned through raw pointers; the in-place read through the existing `write_sized`; the poisoned-handle order; the record's reader keeps flagging `seed`, `host_rate_hz` and `route` lines; exhaustive option maps; the mutation hooks' shape; throwing Swift wrappers around all six calls; the guide paragraph verbatim with the identifier rule. **Recorded, out of scope**: a C caller's `bool` byte of 2 is undefined behavior in create, validate and reconfigure (pre-existing since 0.1); a guide sentence now, the fix queued for the next ABI-touching feature.

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
| FR-001 the read | R-03, R-05 | `us1_settings_read.rs`: `us1_settings_read`, `us1_settings_read_landed`, `us1_trap_named`, `us1_committed_not_audible`, `us1_size_refused`; `ffi_parity settings_read`; the Miri gate |
| FR-002 the reading by index | R-02 | `us2_control_index.rs`: `us2_defaults_by_index` |
| FR-003 settings access | R-01, R-03 | core `control_map::one_hot` (Rust); `us2_one_hot_c`, `us2_representability`, `us2_repair_by_index` |
| FR-004 live by index | R-04 | `us2_live_by_index`, `us2_class_refused`, `us2_device_as_modeled` |
| FR-005 one mapping | R-01, R-07, R-08 | core `control_map` tests (identifiers by index, the variant count), the one-hots, `golden_record` byte-identical, the record's unknown-name test |
| FR-006 the field lookup | R-01 | `us2_field_lookup` (every header field value) |
| FR-007 the refusal order | R-06 | `settings_read_edge_cases`, `abi_tests` |
| FR-008 nothing moved | R-10 | the golden guards, `us2_abi`, `scripts/abi-promise.sh` ×2 |
| FR-009 real time | R-03 | `alloc_free settings_read`, `determinism settings_read` |
| FR-010 Swift | R-09 | `swift test` (`ControlReadingTests`: defaults, the one-hot through the wrappers) |
| SC-001 | R-07 | `us2_defaults_by_index`, the two one-hots |
| SC-002 | R-05 | `us1_settings_read*` |
| SC-003 | R-06 | `settings_read_edge_cases`, `abi_tests` |
| SC-004 | R-04, R-07 | `us2_live_by_index` |
| SC-005 | R-10 | the golden guards, `scripts/abi-promise.sh` ×2, Miri |
| SC-006 | R-10 | `cargo llvm-cov` |

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
crates/rr_dr60/src/control_map.rs     # also control_value, set_control_value (restated 2026-11-01)
crates/rr_dr60/src/record.rs          # get/set rebuilt on the mapping
crates/rr_dr60/src/error.rs           # NotASetting, NotAPlainLiveControl
crates/rr_dr60_ffi/src/settings_read.rs # six functions, RrDr60ControlReading (restated 2026-11-01)
crates/rr_dr60_ffi/src/lib.rs         # statuses 39 – 40, the module
crates/rr_dr60_ffi/include/rr_dr60.h  # regenerated
crates/rr_dr60_ffi/tests/c/smoke.c    # --settings, the C one-hot comparison
crates/rr_dr60_harness/tests/         # us1_settings_read.rs, us2_control_index.rs, settings_read_edge_cases.rs, mutation settings_read
crates/rr_dr60_harness/examples/settings_rows.rs
swift/Sources/RRDR60/ControlReading.swift, swift/Tests/RRDR60Tests/ControlReadingTests.swift
.github/workflows/ci.yml              # the promise script with f5a0245; the Miri list
```

## Complexity Tracking

None.
