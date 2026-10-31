# Implementation Plan: The Pre-1.0 C ABI Pass

**Branch**: `022-abi-pass` | **Date**: 2026-10-31 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/022-abi-pass/spec.md` (approved D1 – D12, 2026-10-31; D13 awaiting the owner)

## Decisions and findings (owner: read this section)

The spec's D1 – D12 stand. The three plan reviews (coach 1 – 11, hardware B1 – B9, product M1 – M9; 2026-10-31) are applied at their defaults; one finding is a **new decision for the owner**:

- **D13 (hardware B1, Critical): the settings rule was described backwards.** The code refuses a settings struct *smaller* than the library's and **accepts a larger one** (reading its prefix, dropping a newer caller's fields); the spec said the opposite, and D4's growth promise did not hold for the one struct every app passes first. Recommended: `rr_dr60_create` / `reconfigure` / `settings_validate` refuse any size different from the library's; the ABI check requires equality; `RrDr60Settings` is the stated exception to the growth promise; a settings growth moves `RR_DR60_ABI_VERSION`. Alternative: a real input prefix rule (a smaller struct's tail filled from the defaults), which would need every future setting to default to its neutral value, a constraint spec 012 already breaks.

Findings applied (restated in the spec's Clarifications):

1. **One helper, fifteen out-structs, three inputs** (coach 3, R-01, R-03): the eleven plus Meters, the readout, the alignment and the place state go through `write_sized(out, published, build)`; the helper takes each struct's ascending list of published layouts (`[56, 64]` for Meters), fills the largest that fits and writes it back; `build` assigns fields into a zeroed buffer and never a whole struct (hardware B3); `Default` is a manual zeroed value plus the size, for constructions only, with a source-scan test (coach 5). It closes a pre-existing copy of four uninitialized padding bytes in the record read-out (hardware B2), and a one-off Miri run gates it.
2. **The check order on a poisoned handle** (coach 1, 6; hardware B5): the handle's status wins; the idle report is written only with a valid size; the handle-taking status functions keep 0.18's order; the test matrix adds poisoned, `RecordingInProgress`, `NotRecording` and overlap × four sizes, and snapshots the engine's position, memory and meters across a refusal.
3. **The readout's embedded settings** (coach 4): in a prefix fill its `struct_size` is the settings bytes the prefix holds.
4. **The lead message** (product M1, M6): "rebuild once **and initialize every report**", a run-time break for a caller that compiles unchanged; a migration block; what a mismatch means; the helpers stay in the `RRDR60` target and the README says to import it; `.sized` never on `RrDr60Settings` (hardware B9).
5. **The figures and gates** (coach 7 – 11, hardware B6): below 1 % at 64-sample blocks, three smoke figures, `smoke --abi` against `examples/abi_rows.rs`, the core-crate `git diff` oracle, the FFI file in the traceability audit, the four defines as literals, `uintptr_t` in the contract.

Plan choices, each traced to the research:

| # | Plan choice | Why |
|---|---|---|
| P1 | `write_sized(out, published, build)` + per-struct `SIZE` / `PUBLISHED` consts with const assertions; the eleven structs gain `struct_size` first; the converters become field-assigning fillers; manual `Default`; the source-scan test; the Miri gate (R-01, R-03, R-10). | D1, D8, D10, D11; FR-001, FR-007; SC-004. |
| P2 | The per-call check order (the size first; the handle's status wins on NULL / poisoned) and the early returns through the helper; the status functions' order kept; the contract line amended in cbindgen.toml, the header, the crate doc and spec 001's contract (R-02). | D12; FR-005. |
| P3 | `RrDr60StopOutcome` flattened; `StopFields` frozen; `RecordReadout`'s `settings` last with the embedded-size fix-up and its doctored-list test; `RunData`'s cap on read (R-04, R-03). | D9; FR-001, FR-004. |
| P4 | The frozen pair: defines, const assertions, the `stream` rename, the docs on the single-element writers and `RunData` (R-03). | D2; FR-002. |
| P5 | `RR_DR60_ABI_VERSION`, `rr_dr60_abi_version`, `rr_dr60_abi_check` (settings equality), status 37, `RR_DR60_MAX_STRUCT_SIZE`; `create` / `reconfigure` / `settings_validate` refuse `!= SETTINGS_SIZE` (R-05). | D3, D8, **D13**; FR-003. |
| P6 | The C smoke's size table, the four cases per out-struct with each fill route, the painted padding (the readout's 212 – 215, Meters as the control), the state-unchanged check, the NULL / poisoned / zero-frames paths, the check and the create rule, the three per-call figures, every initializer via `SIZED`, `smoke --abi` against `examples/abi_rows.rs` (R-07, R-08). | FR-007, FR-009; SC-001, SC-002, SC-004. |
| P7 | `ffi_parity::abi_pass_parity` (the fifteen structs' four cases, the check, the create rule, the snapshots, the `abi_rows`); the `reserved` → `stream` reads; `alloc_free` / `determinism` unchanged in behavior; the core-crate `git diff` oracle (R-10). | FR-006, FR-007; SC-001, SC-003. |
| P8 | The Swift package: `RRDR60Sized` / `.sized` on the seventeen (never settings), `RRDR60.checkABI()`, the lazy status and the reasoned error, `ReRender.swift` and both test targets, a parity test reading `abi_rows`, `swift/README.md` (R-06). | FR-008; SC-005. |
| P9 | The records of records.md (the lead, the migration block, the mismatch meaning, spec 001's / 018's / 020's contracts, the guide's stale sentences, the README row, the traceability audit's FFI file), the version bump and the manifest re-bless (R-08, R-09). | D4 – D7, D13; FR-006, FR-009, FR-010. |

## Summary

A change confined to the C layer: every struct the library fills carries a caller-set size under one rule (a cap, zeroed padding, the filled size written back), the stop outcome no longer embeds a growable struct, the two array-element structs are frozen with their sizes published and the change entry's field named for what it holds, and one pure check lets an app refuse a mismatched build at start. 0.22 breaks every earlier C binary once; from 0.22 the C surface grows without breaking a caller, which is the promise 1.0 will freeze.

## Requirement map

| Requirement | Research | Tests |
|---|---|---|
| FR-001 the rule on the fifteen, the inputs | R-01, R-03, R-04 | FFI unit tests (the helper's cases per struct, Meters 55…4097, the readout's doctored list, the source scan, Miri once), the smoke's cases, `abi_pass_parity` |
| FR-002 the frozen pair, the rename | R-03, R-10 | const assertions, the smoke's table, `ffi_parity` |
| FR-003 the check, 64-bit, the settings rule (D13) | R-05 | FFI unit tests (the tuple, one above and one below, create 119/120/121), the smoke, `abi_pass_parity`, the Swift test |
| FR-004 the flattened outcome | R-04 | FFI unit tests, the smoke, `ffi_parity` |
| FR-005 the check order | R-02 | FFI unit tests (NULL, poisoned, zero frames, overlap, in-progress, not-recording × four sizes; the state snapshots), the smoke |
| FR-006 nothing else moved | — | the fifteen guards, the smoke hashes, `git diff --quiet 81f14dd -- crates/rr_dr60 crates/rr_dr60_detmath` |
| FR-007 pins, table, cases, parity | R-03, R-07 | as above |
| FR-008 Swift | R-06 | `swift test` (the parity and check tests) |
| FR-009 cost | R-08 | the smoke's three figures; the label's modes |
| FR-010 records | R-09 | `us2_cross_check` (the manifest version line, the guide blocks), the drift check, the traceability audit with the FFI file |

## Technical Context

**Language/Version**: Rust 1.85 (edition 2024); Swift 5.9+. **Dependencies**: none new. **Testing**: `cargo test`, the C smoke, `swift test`. **Target**: 64-bit pointer width (iOS; Android arm64-v8a, x86_64). **Performance**: per call one u32 read, two compares, a ≤ 88-byte zeroing and copy: below 1 % of a 64-sample block. **Constraints**: the core crate untouched; no allocation in a call; the header regenerated.

No NEEDS CLARIFICATION remains.

## Constitution Check

| Principle | How this plan complies | Pre | Post |
|---|---|---|---|
| I. Portable Library First | The C API's stable surface is the point of the pass; panic-safe, no Rust types; the header regenerated and drift-checked. | ✅ | ✅ |
| II. Evidence-Based Fidelity | No device claim; no assumption; the traceability audit cites the new literals. | ✅ | ✅ |
| III. Measured, Test-First Verification | Every rule pinned by const assertions and exercised per struct in the smoke and the parity test before the code changes; the padding rule gated once by Miri, the construction rule by a source scan. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | No allocation; the tail fill bounded by the cap; the per-call cost stated. | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | Untouched. | ✅ | ✅ |
| VI. Neutral, Honest Communication | The break stated as a break; the promise defined by its mechanism. | ✅ | ✅ |
| VII. Independently Testable Features | The FFI layer alone; each struct's cases independent. | ✅ | ✅ |

No violation; no Complexity Tracking entry.

## Project Structure

```text
specs/022-abi-pass/  plan.md, research.md, data-model.md, contracts/api.md, quickstart.md, records.md, tasks.md (next)
crates/rr_dr60_ffi/src/lib.rs      # write_sized, the fillers, the eleven size fields, the flattened outcome, stream, the defines, abi_version/abi_check, status 37, SIZE/PUBLISHED consts, manual Default, the settings equality, RunData's cap, the source scan
crates/rr_dr60_ffi/cbindgen.toml   # the contract comment; the consts included
crates/rr_dr60_ffi/include/rr_dr60.h  # regenerated
crates/rr_dr60_ffi/tests/c/smoke.c, run_smoke.sh  # the size table, the cases, the painted padding, the snapshots, the check, the three figures, SIZED, --abi
crates/rr_dr60_harness/examples/abi_rows.rs
crates/rr_dr60_harness/tests/ffi_parity.rs (abi_pass_parity, the stream reads), alloc_free.rs, determinism.rs
swift/Sources/RRDR60/{Sized.swift (new), ABI.swift (new), ReRender.swift}, swift/Tests/*, swift/README.md
docs/guide.md, README.md, CHANGELOG.md, docs/product/{decisions,brief}.md, specs/001-pipeline-skeleton/contracts/c-api.md, specs/018-battery-state/contracts/api.md, specs/020-session-contract/contracts/api.md, scripts/check-traceability.sh, CLAUDE.md
```

## Reviews

The coach (1 – 11), the hardware-emulation reviewer (B1 – B9) and the product specialist (M1 – M9) reviewed this plan on 2026-10-31; every finding is applied above and in the design documents at the reviewers' defaults, and the spec carries the restatements as a dated Clarifications session. D13 is the one new decision, for the owner's plan sign-off.
