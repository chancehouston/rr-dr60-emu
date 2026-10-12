# Implementation Plan: Stage Manifest

**Branch**: `007-stage-manifest` | **Date**: 2026-10-12 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/007-stage-manifest/spec.md` (coach-reviewed and clarified; the coach and hardware-emulation reviews of this plan are applied, decisions in the spec's Clarifications section).

## Summary

The library gains a **stage manifest**: constant data describing the fifteen signal-chain stages (number, identifier, name, a one-sentence description, status, path, confidence, source and assumption IDs, a coverage note, a controls note, a model version) under a header (library version, device rate, counts by status, a basis sentence, three links). Apps generate their "about this model" view from it. A harness **cross-check** reads the two registers, the signal-chain table and the CHANGELOG and fails when the manifest disagrees with them, and a small CI workflow runs that check on register-only changes. The same data is reachable from C. No audio processing changes; the three golden files stay byte-identical.

Technical approach:

- **Data in the core** (R-01): one `stage_table!` macro invocation in a new `manifest.rs` emits the public `static MANIFEST: Manifest` (plain `&'static str` fields, `#[non_exhaustive]` structs so fields can be added as a minor change) and a hidden twin table of NUL-terminated `&'static [u8]` strings for the FFI. Counts by status are a `const fn` over the entries; the FR-002/FR-003 form rules are `const` assertions in characters, so a typo fails the build. A unit test touches each modeled stage's bypass control by stage number, which anchors "modeled" in the API as well as in the table. No allocation, no I/O, no `unsafe`, no new dependency.
- **Content** (R-02): the spec's Key Entities table verbatim; statuses, confidences and model versions re-derived from the committed registers on 2026-10-12 and confirmed by both plan reviews (5 assumed, 10 not modeled; bounds low/medium/low/low/medium for stages 3/4/5/8/10); controls notes as verbatim Rust paths; descriptions with sourced properties only.
- **C surface** (R-03): five functions (`stage_count`, `header`, `stage`, `stage_source`, `stage_assumption`), two output structs, three `repr(u32)` enums, one status code (`NO_SUCH_STAGE` = 20); copy-out structs with process-lifetime string pointers, ID lists reachable by index through accessors, NULL-then-range check order, their own panic guard. Header regenerated; nothing existing moves.
- **Cross-check** (R-04): `manifest_checks.rs` in the harness: line-based parsers for the three tables, a status classifier and headline-confidence reader for the assumption register, the `**Modeled by` marker as the oracle for "modeled", the denylist as one `const` block with a stem matcher that catches every inflection FR-011 names, findings by rule, stage and ID; `tests/us2_cross_check.rs` runs it on the committed files and on seven doctored inputs.
- **One canonical rendering** (R-05): `golden/manifest-v1.txt` (compared in `us2_cross_check` and `determinism`, so every CI platform checks it); a self-contained Rust example and the smoke test's `print_manifest()` print it and `run_smoke.sh` diffs them; the README's Rust and C blocks are those two sources verbatim and its fidelity table is generated from the same data, all three equality-tested between markers. Bless with `RR_DR60_BLESS=manifest` / `=readme`.
- **CI** (R-06): `.github/workflows/registers.yml`, one short job with its own concurrency group on changes to `docs/hardware/**`, `CHANGELOG.md`, `README.md`, the manifest files and itself; `ci.yml` unchanged.
- **Records** (R-07, R-08): the identifier column and the "Modeled by" convention in `signal-chain.md`, the `measured` status in `assumptions.md`, the `General` type in `sources.md`, the CHANGELOG 0.7.0 entry (with the stage-4 confidence rationale), the README section (text-fragment links, the guard's scope, the C-naming sentence with the `get_` readers), the release-checklist item, the traceability script's manifest block, version 0.7.0 with `RR_DR60_VERSION_MINOR = 7`.

## Technical Context

**Language/Version**: Rust, edition 2024, MSRV 1.85 (001 R-01). `concat!`, `const fn` loops and `const` slice/str inspection used by the macro and the assertions are all stable well before 1.85. No Python.

**Primary Dependencies**: core: none new. Harness: `regex` in `[dependencies]` of the unpublished harness crate (the denylist matcher and the register parsers' small patterns; `manifest_checks.rs` is library code, so every job that builds the harness builds it, including the eight-target golden matrix; pure Rust, MSRV below 1.85). FFI: none.

**Storage**: none. The manifest is `static` data in the library's read-only section (about 4 KB of strings plus the twin table). The harness commits one text file, `golden/manifest-v1.txt` (about 5 KB).

**Testing**:
- **Core** (`manifest.rs` unit tests): fifteen entries numbered 1..=15; `stage(0)` and `stage(16)` are `None`; `counts` sum to 15 and equal a runtime tally; the C twin table equals every string plus `\0`; `header.library_version == VERSION`; the bypass-control anchor for every modeled stage; the `const` assertions compile; the crate-docs doctest.
- **Harness, one file per story** (Principle VII): `tests/us1_manifest.rs` needs no register and no engine (entries in order, the literal expected statuses, counts, identity across reads, the wording rules); `tests/us2_cross_check.rs` (the cross-check on the committed registers, the seven doctored inputs, the parser units including the denylist word lists, the canonical text, the README table and the two README code blocks, the example's output); `tests/ffi_parity.rs` (field-by-field C parity with `u32::from(number)`, the refusals); `tests/alloc_free.rs` (the five C functions and the Rust reads allocate nothing); `tests/determinism.rs` (the canonical text on every target); the C smoke test's manifest checks and the `--manifest` diff.
- **SC-001**: the two README code blocks equal the example file and the smoke function by equality test; their outputs are identical by the smoke diff.
- **Golden**: the three JSON files untouched; `manifest-v1.txt` is a new golden text with its own bless switch.
- **Coverage**: gate stays at 80 %; the new module is fully exercised; the FFI panic guards' unreachable `Err` arms carry the Constitution III exclusion comment.

**Target Platform**: unchanged (8 targets). The manifest is identical on all of them (literals plus `CARGO_PKG_VERSION`); the golden-matrix and ios jobs prove it through `determinism`.

**Project Type**: library: a Rust crate plus a C ABI.

**Performance Goals**: none new; the feature adds nothing to a processing path. Reading the manifest is a pointer to a `static`. SC-005 is asserted by the unchanged SHA guards and the existing release timing bounds; the PR quotes the timing rows against the 0.6 baseline (59.6 / 58.1 / 58.1 ns per sample).

**Constraints**:
- Zero allocation, I/O or locking in every manifest read, Rust and C (FR-016, FR-017), asserted by `alloc_free`.
- Byte-identical golden files and SHA guards (FR-018); no change to `Settings` or any existing C symbol; no task may touch `crates/rr_dr60_harness/golden/*.json`, and a moved SHA guard is a defect.
- Every string obeys FR-009 – FR-012 (sourced properties only, the denylist); the test is the gate and the string reviews (below) the judge of wording. Never lengthen a length limit, drop a denylist stem or weaken a parser rule to make a string pass: reword the string; a bound change is a spec Clarification.
- No `unsafe` in the core (the twin table is `&[u8]`, not pointers); FFI `unsafe` blocks carry `// SAFETY:`.
- Lengths are asserted in characters in the core (a `const fn` counting UTF-8 scalar starts) and checked in characters in the harness: one rule, the spec's.

**Scale/Scope**: about 480 lines of core (`manifest.rs`: the macro, types, table, assertions, tests), about 260 lines of FFI (two structs, three enums, five functions with their panic guard, layout and refusal tests), about 750 lines of harness (`manifest_checks.rs` ≈ 400 with parsers, rules, renderers and the denylist; `us1_manifest.rs` ≈ 80; `us2_cross_check.rs` ≈ 250; the example ≈ 30; extensions to `ffi_parity`, `alloc_free`, `determinism`), about 90 lines of C in `smoke.c`, a 30-line workflow, a 15-line script block, and the register, CHANGELOG, README, release-checklist and CLAUDE.md edits.

There are no open NEEDS CLARIFICATION items: the coach review, the clarify session and the two plan reviews resolved them (spec Clarifications 2026-10-12). The plan reviews corrected one clarify answer: the register pages have no row anchors, so hosts append a text fragment (`#:~:text=A-028`) instead; and they settled that the "modeled" marker is normalized to `**Modeled by` on row 5 (today "Modeled in"), that the C ID lists are reached through index accessors, and that the cross-check binds the manifest to the registers while the bypass-control unit test, the traceability script and review bind the code to them.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design and after the plan reviews.*

| Principle | How this plan complies | Pre-research | Post-design |
|---|---|---|---|
| I. Portable Library First | Core stays `no_std`, no I/O, no new dependency; the registers are read only by the harness. The C API grows by appended functions, structs, enums and one status code; the header is regenerated and drift-checked; the smoke test covers every new function (R-03). | ✅ | ✅ |
| II. Evidence-Based Fidelity | The manifest *is* the traceability surface: every entry cites its S-/A-IDs and the cross-check fails when a cited ID is missing or superseded, when a current ID is uncited, or when a status disagrees with the registers (R-04). The feature changes no modeled value. Status and content changes are CHANGELOG entries (FR-015) and re-blesses of the canonical text (R-05). The three register edits (identifier column, `measured` status, `General` type) are documentation conventions, not value changes (R-07). | ✅ | ✅ |
| III. Measured, Test-First Verification | The cross-check and the wording tests are written first and fail until `manifest.rs` exists; the doctored-input tests prove the guard itself; the canonical text, the README table and the README code blocks are golden-style comparisons with explicit bless switches; the three audio golden files are untouched and their guards must not move; the one unreachable arm (the FFI panic guard over static data) carries the exclusion comment. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | Constant data; reads are a `static` reference (Rust) or copies of static fields (C); `alloc_free` asserts zero allocations; `determinism` compares the rendering on every CI target (R-09). No processing path changes. | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | The manifest mirrors the fifteen-stage inventory one to one and names each stage's controls, taps and reports as verbatim API paths; every modeled stage has a non-empty controls note and a unit test touches its bypass control (R-01); the identifier column makes the table and the code share one key (R-07). One new module in the core; the check lives in the harness. | ✅ | ✅ |
| VI. Neutral, Honest Communication | Every string is tested against the denylist and the form rules; descriptions state sourced properties only; the basis sentence says no stage has been checked; the coverage note names what is left out; the README's fidelity table and code blocks become generated or copied views of the same data so prose and engine cannot disagree (R-05); the README states the guard's scope honestly. | ✅ | ✅ |
| VII. Independently Testable Features | US1 is tested with the manifest alone in `us1_manifest.rs` (no engine, no registers; scenario 5 as literal expected statuses); US2 with the manifest plus the four texts and their doctored copies in `us2_cross_check.rs`; US3 by C-versus-Rust parity, `alloc_free` and the smoke test. Each story has its own checkpoint in the phase order below. | ✅ | ✅ |
| Repo standards | rustdoc on every new public item with the spec rule; SemVer 0.7.0 (additive); Conventional Commits; no audio committed (one small text file). | ✅ | ✅ |

**Gate result**: PASS. Two complexity items are recorded below.

## Phase order and conventions for `/speckit-tasks`

1. **Setup**: version 0.7.0 (`Cargo.toml` workspace and the three internal dependency versions, `RR_DR60_VERSION_MINOR = 7`, header regenerated); record the 0.6 baseline (the three SHA guards, the release timing rows) for the PR.
2. **US1, core** (P1 checkpoint: `cargo test -p rr_dr60 manifest`, `--doc`, `--test us1_manifest` green; `examples/manifest.rs` prints): `manifest.rs` with the macro, the types, the fifteen entries, the `const` assertions, the twin table and the unit tests; `lib.rs` re-exports and the crate-docs paragraph; `us1_manifest.rs` written first and red; the example.
3. **US2, the drift guard** (P2 checkpoint: `--test us2_cross_check` green on the committed registers and red on each doctored input; `registers` workflow added): the three register edits first (identifier column, row 5 "Modeled by", the rules sentences), then `manifest_checks.rs`, `us2_cross_check.rs`, `golden/manifest-v1.txt` (first bless), the `determinism` extension, `registers.yml`, the traceability script block.
4. **US3, the C surface** (P3 checkpoint: `cargo test -p rr_dr60_ffi`, `--test ffi_parity --test alloc_free`, `run_smoke.sh` with the manifest diff green): the FFI types, functions and tests; `cbindgen.toml` and the header; `smoke.c`'s checks and `print_manifest()`; `run_smoke.sh`'s diff; the parity and allocation extensions.
5. **String reviews** (before any record is written): the hardware-emulation reviewer on the descriptions and coverage notes (FR-010, sourced properties only), the product specialist on the wording (FR-009, FR-011), the coach on the US1 increment. Findings are applied to `manifest.rs` and the golden text re-blessed; the strings land in the CHANGELOG only after these pass.
6. **Records**: README (the manifest section, the table and the two code blocks between markers, the text-fragment note, the guard's scope, the C-naming sentence), CHANGELOG 0.7.0 (with the stage-4 confidence rationale and the one sentence on why stage 10 stays 0.4.0), release checklist, CLAUDE.md, the signal-chain convention paragraph.
7. **Polish**: the full local gate (fmt, clippy, tests, llvm-cov, traceability, header drift), the quickstart walk recorded, the draft PR, CI, the final coach and hardware reviews; merge only on the user's word.

Conventions for every task: cite spec 007 FR/SC and S-/A-IDs in comments and test names; engineering-target constants labelled (research R-10); the golden JSON window is closed; never weaken a limit, a stem or a parser rule to make a string pass; story boundaries as in the Constitution Check VII row; `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all-features` before ticking a task; Conventional Commits on `007-stage-manifest`.

## Project Structure

### Documentation (this feature)

```text
specs/007-stage-manifest/
├── plan.md              # This file
├── research.md          # Phase 0: decisions R-01 – R-11
├── data-model.md        # Phase 1: the types, the fifteen entries, the register shapes, the canonical text
├── quickstart.md        # Phase 1: validation walk-through
├── contracts/
│   ├── rust-api.md      # the manifest module, stability rules, the doctest, the core unit tests
│   ├── c-api.md         # status 20, three enums, two structs, five functions, smoke and parity additions, the naming rule
│   └── cross-check.md   # the register shapes, the rules and findings, the denylist, the doctored-input tests
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/rr_dr60/src/
├── lib.rs                      # `pub mod manifest; pub use manifest::{manifest, Manifest, Header, StatusCounts, StageEntry,
│                               #   StageStatus, StagePath, Confidence}`; crate docs paragraph + doctest
└── manifest.rs                 # NEW: types, `stage_table!`, MANIFEST, `__c` twin table, const assertions (characters),
                                #      unit tests incl. the bypass-control anchor (R-01, R-02)

crates/rr_dr60_ffi/
├── src/lib.rs                  # RR_DR60_VERSION_MINOR = 7; RrDr60Status::NoSuchStage = 20; RrDr60StageStatus, RrDr60StagePath,
│                               # RrDr60Confidence; RrDr60ManifestHeader, RrDr60StageEntry; five rr_dr60_manifest_* functions with
│                               # their own catch_unwind; layout + refusal tests (R-03)
├── cbindgen.toml               # the three enums added to [export] include; header comment's real-time-safe list
├── include/rr_dr60.h           # regenerated
└── tests/c/smoke.c, run_smoke.sh   # manifest checks; print_manifest() between markers; `--manifest` mode diffed against the example

crates/rr_dr60_harness/
├── Cargo.toml                  # + regex
├── src/lib.rs                  # pub mod manifest_checks
├── src/manifest_checks.rs      # NEW: Registers, committed_registers, parsers, cross_check, wording_findings, DENYLIST_*,
│                               #      render, render_readme_table, Finding, Rule (R-04, R-05)
├── golden/manifest-v1.txt      # NEW: the canonical text (bless: RR_DR60_BLESS=manifest)
├── examples/manifest.rs        # NEW: self-contained; prints every field in the canonical form (the README's Rust block)
├── tests/us1_manifest.rs       # NEW: US1, register-free
├── tests/us2_cross_check.rs    # NEW: US2: committed + doctored, parser units, canonical text, README table and code blocks
├── tests/ffi_parity.rs         # + manifest parity and refusals
├── tests/alloc_free.rs         # + manifest reads, Rust and C
└── tests/determinism.rs        # + canonical text equals golden/manifest-v1.txt

.github/workflows/registers.yml # NEW: the `registers` job, own concurrency group (R-06)
scripts/check-traceability.sh   # + manifest ID existence block (R-08)
docs/hardware/signal-chain.md   # Identifier column; row 5 "Modeled by"; the convention paragraph (R-07)
docs/hardware/assumptions.md    # rules: `measured`, `adopted`, "never write verified for a measurement", the classification conventions
docs/hardware/sources.md        # the `General` type sentence
CHANGELOG.md                    # 0.7.0 Added + Documentation
README.md                       # manifest section; table + Rust block + C block between markers (bless: RR_DR60_BLESS=readme);
                                # text-fragment note; the guard's scope; the C naming rule
docs/release-checklist.md       # "the README table and code blocks equal the manifest's output"
CLAUDE.md                       # 007 status line; the two bless commands
Cargo.toml                      # version 0.7.0 (workspace + the three internal dependency versions)
```

**Structure Decision**: the existing three-crate workspace. The manifest is one module in the core because it is the library's own statement about itself; the cross-check and the renderers are harness code because they read files and allocate; the C surface follows the FFI crate's copy-out convention. The canonical text, the README table and the README code blocks are "golden" artifacts with bless switches, like the three audio golden files, so the same habits apply.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| A hidden twin table of NUL-terminated strings in the core (`manifest::__c`) | C needs NUL-terminated strings valid for the life of the process; the core forbids `unsafe`, so it cannot hold raw pointers in a `static`, and the FFI crate must not retype the fifteen entries (drift) | `&'static CStr` fields on the public types (non-idiomatic Rust surface, runtime UTF-8 check on every read); a `Text` newtype (wraps every field for C's sake); generating the FFI table from the Markdown (the registers would become build inputs of a `no_std` crate) |
| A second workflow file (`registers.yml`) | `ci.yml` skips documentation-only changes workflow-wide, so no step inside it can run on a register edit; the spec requires the check to run then (FR-020) | Removing the docs skip from `ci.yml` (a 30-minute coverage job per register edit); a `paths-filter` step (the workflow never starts) |
