# Implementation Plan: Control Manifest

**Branch**: `014-control-manifest` | **Date**: 2026-10-29 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/014-control-manifest/spec.md`

## Decisions and findings (owner: read this section)

No plan decision needs the owner unless the reviews raise one. Choices made, all following spec 007's proven pattern:

| # | Plan choice | Why |
|---|---|---|
| P1 | A new core module `controls.rs` with one table macro that emits the Rust table and its C twin (research R-01). | The 007 pattern: Rust and C cannot drift; no `unsafe` in the core. |
| P2 | Values as switch / integer (u64) / continuous (f32) / choice with each option's C value (R-02). | The seed is a u64; the tap's C numbers are not in Rust order. |
| P3 | Two oracles for ranges and defaults: a core test against the engine's own range table and `Settings::new` at all 7 rates, plus a harness edge table through the public validation and setters (R-03). | Catches drift of the constants and of the behavior an app sees. |
| P4 | New live controls are caught by scanning the source for `pub fn set_` and the C header for `rr_dr60_set_` (R-04). | The compiler cannot enumerate methods. |
| P5 | A canonical text golden (`control-manifest-v1.txt`) is the cross-platform and Rust/C oracle (R-07). | The 007 pattern. |
| P6 | C: five functions, three enums, a path bit set, status 26 (R-08). | The 007 shape, already used by the Swift package. |

**Finding F1 (no decision needed):** four code comments that FR-016 reads are wrong or missing: `record_stage_enabled` and `playback_stage_enabled` cite **A-002, which is superseded** (by A-027 in spec 004); `mic.hiss_offset_db` cites nothing though it rests on A-048; `Route`'s default cites nothing though S-001 is its basis. The plan corrects the comments (no code or audio change) and the CHANGELOG notes it (R-06).

**Finding F2:** the version bump re-blesses the stage manifest's canonical text and the README's generated regions in one line each (spec FR-014 as amended by the coach review).

## Summary

Constant data in the core describing all 29 controls (3 live, 26 settings), bound by tests to the engine's validation, defaults, stage manifest notes, registers and denylist, readable from Rust and C; no audio change; version 0.14.0.

## Technical Context

**Language/Version**: Rust 1.85 (edition 2024), unchanged.
**Primary Dependencies**: none new (the harness keeps `regex`).
**Storage**: N/A.
**Testing**: `cargo test` (core unit tests, harness story tests, `ffi_parity`, `alloc_free`, `determinism`), the C smoke loop, `cargo llvm-cov`.
**Target Platform**: the existing 8 targets; the canonical text is identical on all.
**Project Type**: library (Rust crate plus C ABI).
**Performance Goals**: none; no processing path changes. Reading is a pointer to a `static`.
**Constraints**: zero allocation, I/O or locking in every read; audio golden files and SHA guards unmoved; `Settings` and every existing C symbol unchanged; never weaken a length limit or denylist stem to make a string pass.
**Scale/Scope**: about 450 lines of core (`controls.rs`), 250 of FFI, 500 of harness (`controls_checks.rs`, `us1_controls.rs`, `us2_controls.rs`, the example, extensions to three existing tests), 60 of C smoke, plus four comment fixes and docs.

No NEEDS CLARIFICATION remains.

## Constitution Check

| Principle | How this plan complies | Pre | Post |
|---|---|---|---|
| I. Portable Library First | `no_std` core, no I/O, no dependency; C API additive, header regenerated and drift-checked. | ✅ | ✅ |
| II. Evidence-Based Fidelity | Every entry cites the IDs its default's comment cites (R-06); superseded IDs fail the register cross-check; no modeled value changes. | ✅ | ✅ |
| III. Measured, Test-First Verification | Tests are written first per story: the edge table, the exhaustive map, the tokenizer and the seven doctored inputs fail before the table exists. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | Constant data; `alloc_free` covers every read; nothing in a processing path. | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | Each entry names its stage; FR-009 binds it to the stage manifest's controls notes both ways. | ✅ | ✅ |
| VI. Neutral, Honest Communication | Descriptions pass 007's denylist, contain no digits (no assumed figures), and go through a string review. | ✅ | ✅ |
| VII. Independently Testable Features | One test file per story; US1 needs no registers, US2 no engine, US3 only the C API. | ✅ | ✅ |

## Project Structure

```text
specs/014-control-manifest/  plan.md, research.md, data-model.md, contracts/api.md, quickstart.md, tasks.md (next)
crates/rr_dr60/src/controls.rs              # new: types, control_table!, the 29 entries, const assertions, unit tests
crates/rr_dr60/src/lib.rs                   # pub mod controls; re-exports
crates/rr_dr60/src/settings.rs, transport?  # comment fixes (F1) only
crates/rr_dr60_ffi/src/lib.rs, include/rr_dr60.h, tests/c/smoke.c
crates/rr_dr60_harness/src/controls_checks.rs   # tokenizer, setter scan, ID-from-comment reader, renderer
crates/rr_dr60_harness/tests/us1_controls.rs, us2_controls.rs
crates/rr_dr60_harness/examples/control_manifest.rs
crates/rr_dr60_harness/golden/control-manifest-v1.txt (new), manifest-v1.txt (version line)
docs/guide.md, README.md (generated regions), CHANGELOG.md, docs/product/controls-discussion.md, CLAUDE.md
scripts/check-traceability.sh               # covers controls.rs (FR-011)
```

## Reviews

Per the owner's cadence: the coach and the hardware-emulation reviewer review this plan; their findings are applied without stopping unless one needs an owner decision. The 29 descriptions get a string review (product specialist) before the canonical bless.
