# Implementation Plan: Stage Manifest

**Branch**: `007-stage-manifest` | **Date**: 2026-10-12 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/007-stage-manifest/spec.md` (coach-reviewed and clarified; decisions in its Clarifications section).

## Summary

The library gains a **stage manifest**: constant data describing the fifteen signal-chain stages (number, identifier, name, a one-sentence description, status, path, confidence, source and assumption IDs, a coverage note, a controls note, a model version) under a header (library version, device rate, counts by status, a basis sentence, three links). Apps generate their "about this model" view from it. A harness **cross-check** reads the two registers, the signal-chain table and the CHANGELOG and fails when the manifest disagrees with them, and a small CI workflow runs that check on register-only changes. The same data is reachable from C. No audio processing changes; the three golden files stay byte-identical.

Technical approach:

- **Data in the core** (R-01): one `stage_table!` macro invocation in a new `manifest.rs` emits the public `static MANIFEST: Manifest` (plain `&'static str` fields, `#[non_exhaustive]` structs so fields can be added as a minor change) and a hidden twin table of NUL-terminated `&'static [u8]` strings for the FFI. Counts by status are a `const fn` over the entries; the FR-002/FR-003 form rules are `const` assertions, so a typo fails the build. No allocation, no I/O, no `unsafe`, no new dependency.
- **Content** (R-02): the spec's Key Entities table verbatim; statuses, confidences and model versions re-derived from the committed registers on 2026-10-12 and found consistent (5 assumed, 10 not modeled; bounds low/medium/low/low/medium for stages 3/4/5/8/10).
- **C surface** (R-03): five functions (`stage_count`, `header`, `stage`, `stage_source`, `stage_assumption`), two output structs, three `repr(u32)` enums, one status code (`NO_SUCH_STAGE` = 20); copy-out structs with process-lifetime string pointers, ID lists reachable by index through accessors. Header regenerated; nothing existing moves.
- **Cross-check** (R-04): `manifest_checks.rs` in the harness: line-based parsers for the three tables, a status classifier and headline-confidence reader for the assumption register, the `**Modeled by` marker as the oracle for "modeled", the denylist as one `const` block with a stem matcher, findings by rule, stage and ID; `tests/manifest.rs` runs it on the committed files and on seven doctored inputs.
- **One canonical rendering** (R-05): `golden/manifest-v1.txt` (compared in `manifest` and `determinism`, so every CI platform checks it), the Rust example and `smoke --manifest` print it and `run_smoke.sh` diffs them, and the README's fidelity table is generated between markers from the same data. Bless with `RR_DR60_BLESS=manifest` / `=readme`.
- **CI** (R-06): `.github/workflows/registers.yml`, one short job on changes to `docs/hardware/**`, `CHANGELOG.md`, `README.md` and the manifest files; `ci.yml` unchanged.
- **Records** (R-07, R-08): the identifier column and the "Modeled by" convention in `signal-chain.md`, the `measured` status in `assumptions.md`, the `General` type in `sources.md`, the CHANGELOG 0.7.0 entry, the README section and C-naming sentence, the release-checklist item, the traceability script's manifest block.

## Technical Context

**Language/Version**: Rust, edition 2024, MSRV 1.85 (001 R-01). `concat!`, `const fn` loops and `const` slice/str inspection used by the macro and the assertions are all stable well before 1.85. No Python.

**Primary Dependencies**: core: none new. Harness: `regex` (dev-time only, for the denylist and the register parsers' small patterns). FFI: none.

**Storage**: none. The manifest is `static` data in the library's read-only section (about 4 KB of strings plus the twin table). The harness commits one text file, `golden/manifest-v1.txt` (about 5 KB).

**Testing**:
- **Core** (`manifest.rs` unit tests): fifteen entries numbered 1..=15; `stage(0)` and `stage(16)` are `None`; `counts` sum to 15 and equal a runtime tally; the C twin table equals every string plus `\0`; `header.library_version == VERSION`; the `const` assertions compile.
- **Harness**: `tests/manifest.rs` (US1 scenarios, the cross-check on the committed registers, the seven doctored inputs, the wording rules, the canonical text, the README table, the parser units); `tests/ffi_parity.rs` (field-by-field C parity, the refusals); `tests/alloc_free.rs` (the five C functions and the Rust reads allocate nothing); `tests/determinism.rs` (the canonical text on every target); the C smoke test's manifest checks and the `--manifest` diff.
- **Golden**: the three JSON files untouched; `manifest-v1.txt` is a new golden text with its own bless switch.
- **Coverage**: gate stays at 80 %; the new module is fully exercised.

**Target Platform**: unchanged (8 targets). The manifest is identical on all of them (literals plus `CARGO_PKG_VERSION`); the golden-matrix and ios jobs prove it through `determinism`.

**Project Type**: library: a Rust crate plus a C ABI.

**Performance Goals**: none new; the feature adds nothing to a processing path. Reading the manifest is a pointer to a `static`. SC-005 is asserted by the unchanged SHA guards and the existing release timing bounds.

**Constraints**:
- Zero allocation, I/O or locking in every manifest read, Rust and C (FR-016, FR-017), asserted by `alloc_free`.
- Byte-identical golden files and SHA guards (FR-018); no change to `Settings` or any existing C symbol.
- Every string obeys FR-009 – FR-012 (sourced properties only, the denylist); the test is the gate, the technical review the judge of wording.
- No `unsafe` in the core (the twin table is `&[u8]`, not pointers); FFI `unsafe` blocks carry `// SAFETY:`.
- Lengths asserted in bytes in the core (stricter) and in characters in the harness (the spec's rule).

**Scale/Scope**: about 450 lines of core (`manifest.rs`: the macro, types, table, assertions, tests), about 250 lines of FFI (two structs, three enums, five functions, layout and refusal tests), about 700 lines of harness (`manifest_checks.rs` ≈ 400 with parsers, rules, renderers and the denylist; `tests/manifest.rs` ≈ 250; extensions to `ffi_parity`, `alloc_free`, `determinism`; the example), about 80 lines of C in `smoke.c`, a 25-line workflow, a 15-line script block, and the register, CHANGELOG, README, release-checklist and CLAUDE.md edits.

There are no open NEEDS CLARIFICATION items: the coach review and the clarify session resolved them (spec Clarifications 2026-10-12). The plan session settled two details the spec left to it: the C ID lists are reached through index accessors rather than pointer arrays (R-03), and the "modeled" marker is normalized to `**Modeled by` on row 5, which today reads "Modeled in" (R-02, R-07).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | How this plan complies | Pre-research | Post-design |
|---|---|---|---|
| I. Portable Library First | Core stays `no_std`, no I/O, no new dependency; the registers are read only by the harness. The C API grows by appended functions, structs, enums and one status code; the header is regenerated and drift-checked; the smoke test covers every new function (R-03). | ✅ | ✅ |
| II. Evidence-Based Fidelity | The manifest *is* the traceability surface: every entry cites its S-/A-IDs and the cross-check fails when a cited ID is missing or superseded, when a current ID is uncited, or when a status disagrees with the registers (R-04). The feature changes no modeled value. Status and content changes are CHANGELOG entries (FR-015) and re-blesses of the canonical text (R-05). The three register edits (identifier column, `measured` status, `General` type) are documentation conventions, not value changes (R-07). | ✅ | ✅ |
| III. Measured, Test-First Verification | The cross-check and the wording tests are written first and fail until `manifest.rs` exists; the doctored-input tests prove the guard itself; the canonical text and the README table are golden-style comparisons with explicit bless switches; the three audio golden files are untouched and their guards must not move. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | Constant data; reads are a `static` reference (Rust) or copies of static fields (C); `alloc_free` asserts zero allocations; `determinism` compares the rendering on every CI target (R-09). No processing path changes. | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | The manifest mirrors the fifteen-stage inventory one to one and names each stage's controls, taps and reports; the identifier column makes the table and the code share one key (R-07). One new module in the core; the check lives in the harness. | ✅ | ✅ |
| VI. Neutral, Honest Communication | Every string is tested against the denylist and the form rules; descriptions state sourced properties only; the basis sentence says no stage has been checked; the coverage note names what is left out; the README's fidelity table becomes a generated view of the same data so prose and engine cannot disagree (R-05). | ✅ | ✅ |
| VII. Independently Testable Features | US1 is tested with the manifest alone (no engine, no registers); US2 with the manifest plus the four texts and their doctored copies; US3 by C-versus-Rust parity. No story depends on another's harness code beyond `manifest_checks.rs`'s renderer. | ✅ | ✅ |
| Repo standards | rustdoc on every new public item with the spec rule; SemVer 0.7.0 (additive); Conventional Commits; no audio committed (one small text file). | ✅ | ✅ |

**Gate result**: PASS. Two complexity items are recorded below.

## Project Structure

### Documentation (this feature)

```text
specs/007-stage-manifest/
├── plan.md              # This file
├── research.md          # Phase 0: decisions R-01 – R-11
├── data-model.md        # Phase 1: the types, the fifteen entries, the register shapes, the canonical text
├── quickstart.md        # Phase 1: validation walk-through
├── contracts/
│   ├── rust-api.md      # the manifest module, stability rules, the doctest
│   ├── c-api.md         # status 20, three enums, two structs, five functions, smoke and parity additions
│   └── cross-check.md   # the register shapes, the rules and findings, the denylist, the doctored-input tests
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/rr_dr60/src/
├── lib.rs                      # `pub mod manifest; pub use manifest::{manifest, Manifest, Header, StatusCounts, StageEntry,
│                               #   StageStatus, StagePath, Confidence}`; crate docs paragraph + doctest
└── manifest.rs                 # NEW: types, `stage_table!`, MANIFEST, `__c` twin table, const assertions, unit tests (R-01, R-02)

crates/rr_dr60_ffi/
├── src/lib.rs                  # RrDr60Status::NoSuchStage = 20; RrDr60StageStatus, RrDr60StagePath, RrDr60Confidence;
│                               # RrDr60ManifestHeader, RrDr60StageEntry; five rr_dr60_manifest_* functions; layout + refusal tests (R-03)
├── cbindgen.toml               # the three enums added to [export] include; header comment's real-time-safe list
├── include/rr_dr60.h           # regenerated
└── tests/c/smoke.c, run_smoke.sh   # manifest checks; `--manifest` mode diffed against the Rust example

crates/rr_dr60_harness/
├── Cargo.toml                  # + regex
├── src/lib.rs                  # pub mod manifest_checks
├── src/manifest_checks.rs      # NEW: Registers, committed_registers, parsers, cross_check, wording_findings, DENYLIST_*,
│                               #      render, render_readme_table, Finding, Rule (R-04, R-05)
├── golden/manifest-v1.txt      # NEW: the canonical text (bless: RR_DR60_BLESS=manifest)
├── examples/manifest.rs        # NEW: prints render(manifest())
├── tests/manifest.rs           # NEW: US1, US2 (committed + doctored), wording, canonical text, README table, parser units
├── tests/ffi_parity.rs         # + manifest parity and refusals
├── tests/alloc_free.rs         # + manifest reads, Rust and C
└── tests/determinism.rs        # + canonical text equals golden/manifest-v1.txt

.github/workflows/registers.yml # NEW: the `registers` job (R-06)
scripts/check-traceability.sh   # + manifest ID existence block (R-08)
docs/hardware/signal-chain.md   # Identifier column; row 5 "Modeled by"; the convention paragraph (R-07)
docs/hardware/assumptions.md    # rules: `measured`, `adopted`, the classification and headline-confidence conventions
docs/hardware/sources.md        # the `General` type sentence
CHANGELOG.md                    # 0.7.0 Added + Documentation
README.md                       # manifest section (Rust + C read loops, identifiers as localization keys, C naming rule);
                                # fidelity table replaced by the generated table between markers (bless: RR_DR60_BLESS=readme)
docs/release-checklist.md       # "the README table equals the manifest's output"
CLAUDE.md                       # 007 status line; the two bless commands
Cargo.toml                      # version 0.7.0 (workspace + the three internal dependency versions)
```

**Structure Decision**: the existing three-crate workspace. The manifest is one module in the core because it is the library's own statement about itself; the cross-check and the renderers are harness code because they read files and allocate; the C surface follows the FFI crate's copy-out convention. The README table and the canonical text are "golden" artifacts with bless switches, like the three audio golden files, so the same habits apply.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| A hidden twin table of NUL-terminated strings in the core (`manifest::__c`) | C needs NUL-terminated strings valid for the life of the process; the core forbids `unsafe`, so it cannot hold raw pointers in a `static`, and the FFI crate must not retype the fifteen entries (drift) | `&'static CStr` fields on the public types (non-idiomatic Rust surface, runtime UTF-8 check on every read); a `Text` newtype (wraps every field for C's sake); generating the FFI table from the Markdown (the registers would become build inputs of a `no_std` crate) |
| A second workflow file (`registers.yml`) | `ci.yml` skips documentation-only changes workflow-wide, so no step inside it can run on a register edit; the spec requires the check to run then (FR-020) | Removing the docs skip from `ci.yml` (a 30-minute coverage job per register edit); a `paths-filter` step (the workflow never starts) |
