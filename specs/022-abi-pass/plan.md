# Implementation Plan: The Pre-1.0 C ABI Pass

**Branch**: `022-abi-pass` | **Date**: 2026-10-31 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/022-abi-pass/spec.md` (approved D1 – D12, 2026-10-31)

## Decisions and findings (owner: read this section)

The spec's D1 – D12 stand. Restatements from the plan's research (the plan reviews' findings are appended here when applied):

1. **One helper, eighteen structs** (R-01): the eleven new sized structs and the seven existing ones go through `write_sized` (the cap, the zeroed construction, the read-back, the prefix fill); `rr_dr60_meters` loses its "no upper cap" and its echo of the caller's size.
2. **The size check precedes `live(pipeline)`** on the per-call reports (R-02); every early return writes through the helper.
3. **The sizes come from `size_of` on the build target** (R-03); the table is 64-bit; `RrDr60StopOutcome`'s flattened size (88 expected) is confirmed by the const assertion at implementation.
4. **`Default` is the zeroed value plus the size** on every `rr_dr60_ffi` struct (R-10), so the harness's 158 `default()` constructions stay valid.
5. **The Swift check runs lazily** before the first wrapper call and is callable at launch (R-06); `.sized` on the eighteen structs.

Plan choices, each traced to the research:

| # | Plan choice | Why |
|---|---|---|
| P1 | `write_sized` + per-struct `SIZE` / `SIZE_MIN` consts with const assertions; the eleven structs gain `struct_size` first; `Default` = zeroed + size (R-01, R-03, R-10). | D1, D8, D10, D11; FR-001, FR-007. |
| P2 | The per-call check order and the early returns through the helper; the cbindgen/header contract line amended (R-02). | D12; FR-005. |
| P3 | `RrDr60StopOutcome` flattened; `StopFields` frozen; `RecordReadout`'s `settings` last (R-04). | D9; FR-004. |
| P4 | The frozen pair: defines, const assertions, the `stream` rename, the docs on the single-element writers and `RunData` (R-03). | D2; FR-002. |
| P5 | `RR_DR60_ABI_VERSION`, `rr_dr60_abi_version`, `rr_dr60_abi_check`, status 37, `RR_DR60_MAX_STRUCT_SIZE` (R-05). | D3, D8; FR-003. |
| P6 | The C smoke's size table, the cases per struct, the painted padding, the NULL/zero-frames paths, the check, the per-call figure, the 40 initializers (R-07). | FR-007, FR-009; SC-001, SC-002. |
| P7 | `ffi_parity::abi_pass_parity`; the `reserved` → `stream` reads; `alloc_free` / `determinism` unchanged in behavior (R-10). | FR-007; SC-001. |
| P8 | The Swift package: `RRDR60Sized` / `.sized`, `RRDR60.checkABI()` and the lazy status, `ReRender.swift` and both test targets, a parity test, `swift/README.md` (R-06). | FR-008; SC-005. |
| P9 | The records of records.md, the two amended contracts, the version bump and the manifest re-bless (R-08, R-09). | D4 – D7; FR-006, FR-009, FR-010. |

## Summary

A change confined to the C layer: every struct the library fills carries a caller-set size under one rule (a cap, zeroed padding, the filled size written back), the stop outcome no longer embeds a growable struct, the two array-element structs are frozen with their sizes published and the change entry's field named for what it holds, and one pure check lets an app refuse a mismatched build at start. 0.22 breaks every earlier C binary once; from 0.22 the C surface grows without breaking a caller, which is the promise 1.0 will freeze.

## Requirement map

| Requirement | Research | Tests |
|---|---|---|
| FR-001 the rule on the eighteen | R-01, R-03 | FFI unit tests (the helper's cases per struct), the smoke's cases, `abi_pass_parity` |
| FR-002 the frozen pair, the rename | R-03, R-10 | const assertions, the smoke's table, `ffi_parity` |
| FR-003 the check, 64-bit | R-05 | FFI unit tests, the smoke, `abi_pass_parity`, the Swift test |
| FR-004 the flattened outcome | R-04 | FFI unit tests, the smoke, `ffi_parity` |
| FR-005 the check order | R-02 | FFI unit tests (NULL handle, zero frames × valid/invalid size), the smoke |
| FR-006 nothing else moved | — | the fifteen guards, the smoke hashes, `cargo public-api`-style diff by review |
| FR-007 pins, table, cases, parity | R-03, R-07 | as above |
| FR-008 Swift | R-06 | `swift test` (the parity and check tests) |
| FR-009 cost | R-08 | the smoke's figure; the label's modes |
| FR-010 records | R-09 | `us2_cross_check` (the manifest version line), the drift check, the traceability audit |

## Technical Context

**Language/Version**: Rust 1.85 (edition 2024); Swift 5.9+. **Dependencies**: none new. **Testing**: `cargo test`, the C smoke, `swift test`. **Target**: 64-bit pointer width (iOS; Android arm64-v8a, x86_64). **Performance**: per call one u32 read and compare and a ≤ 88-byte copy. **Constraints**: the core crate untouched; no allocation in a call; the header regenerated.

No NEEDS CLARIFICATION remains.

## Constitution Check

| Principle | How this plan complies | Pre | Post |
|---|---|---|---|
| I. Portable Library First | The C API's stable surface is the point of the pass; panic-safe, no Rust types; the header regenerated and drift-checked. | ✅ | ✅ |
| II. Evidence-Based Fidelity | No device claim; no assumption; the traceability audit cites the new literals. | ✅ | ✅ |
| III. Measured, Test-First Verification | Every rule pinned by const assertions and exercised per struct in the smoke and the parity test before the code changes. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | No allocation; the tail fill bounded by the cap; the per-call cost stated. | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | Untouched. | ✅ | ✅ |
| VI. Neutral, Honest Communication | The break stated as a break; the promise defined by its mechanism. | ✅ | ✅ |
| VII. Independently Testable Features | The FFI layer alone; each struct's cases independent. | ✅ | ✅ |

No violation; no Complexity Tracking entry.

## Project Structure

```text
specs/022-abi-pass/  plan.md, research.md, data-model.md, contracts/api.md, quickstart.md, records.md, tasks.md (next)
crates/rr_dr60_ffi/src/lib.rs      # write_sized, the eleven size fields, the flattened outcome, stream, the defines, abi_version/abi_check, status 37, SIZE/SIZE_MIN consts, Default
crates/rr_dr60_ffi/cbindgen.toml   # the contract comment; the consts included
crates/rr_dr60_ffi/include/rr_dr60.h  # regenerated
crates/rr_dr60_ffi/tests/c/smoke.c, run_smoke.sh  # the size table, the cases, the check, the figure, the initializers
crates/rr_dr60_harness/tests/ffi_parity.rs (abi_pass_parity, the stream reads), alloc_free.rs, determinism.rs
swift/Sources/RRDR60/{Sized.swift (new), ABI.swift (new), ReRender.swift}, swift/Tests/*, swift/README.md
docs/guide.md, CHANGELOG.md, docs/product/{decisions,brief}.md, specs/018-battery-state/contracts/api.md, specs/020-session-contract/contracts/api.md, CLAUDE.md
```

## Reviews

*(The coach, the hardware-emulation reviewer and the product specialist review this plan next; their findings are applied above and in the design documents, and the spec carries the restatements as a dated Clarifications session.)*
