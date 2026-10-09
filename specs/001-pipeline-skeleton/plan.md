# Implementation Plan: Minimal End-to-End Pipeline Skeleton

**Branch**: `001-pipeline-skeleton` | **Date**: 2026-10-08 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/001-pipeline-skeleton/spec.md`

## Summary

This feature delivers the first runnable slice of the RR-DR60 emulator. A host feeds mono `f32` audio at 8, 16, 44.1, 48, 88.2 or 96 kHz in any block size and gets back the same number of samples at the same rate. Internally, the signal is converted to the device's 8 kHz rate (A-001), passed through the record and playback voice-band stages (signal-chain stages 4 and 10), and converted back. Each stage can be bypassed, the output can be tapped after the record stage, and the pipeline reports its latency.

Technical approach:

- **Workspace**: four Rust crates:
  - `rr_dr60`: the pure `no_std` + `alloc` core.
  - `rr_dr60_detmath`: deterministic math.
  - `rr_dr60_ffi`: the C API, with a cbindgen header.
  - `rr_dr60_harness`: the measurement harness and golden files.
- **Stages**: a fixed minimum-phase IIR filter at 8 kHz. It is a 5th-order Butterworth high-pass at 300 Hz plus a 6th-order elliptic low-pass at 3380 Hz, run as `f64` biquad sections in transposed direct form II. Coefficients are designed offline with scipy and committed as exact bit patterns.
- **Rate conversion**: linear-phase rational polyphase FIR (Kaiser-windowed sinc, 70 dB), designed at construction with in-house deterministic math.
- **Processing**: a per-sample state machine, giving block-size invariance.
- **Bit-identity**: basic IEEE operations only on any path that affects output.
- **Verification**: CI checks golden hashes on every target that has an automated runner (7 expected) on every PR. The iOS device, and Windows ARM64 if it has no hosted runner, are checked manually before each release.

Expected default latency is about 11.3 ms, against the 20 ms limit. See [research.md](research.md) for all decisions.

## Technical Context

**Language/Version**: Rust, edition 2024. MSRV 1.85; development on stable 1.99 (R-01). The offline filter-design tool uses Python 3 via `uv` with numpy and scipy (R-07).

**Primary Dependencies**:
- Core: `rr_dr60_detmath` only, an in-house crate.
- Harness: `rustfft`, `sha2`, `serde`, `serde_json`.
- Tools: cbindgen CLI (pinned), cargo-llvm-cov.

**Storage**: N/A. Golden references are one small JSON file (about 40 KB) committed in the harness crate (R-14).

**Testing**:
- `cargo test`: core unit tests including analytic filter checks, detmath accuracy tests, and the harness integration tests (measurement matrix, golden, partition, allocation, FFI parity).
- A C smoke test (clang against the staticlib).
- `cargo llvm-cov` for the 80% line-coverage gate.

**Target Platform**: Library for Linux, macOS and Windows on x86-64 and ARM64, iOS device (`aarch64-apple-ios`), and iOS simulator (`aarch64-apple-ios-sim`, plus `x86_64-apple-ios` build-only).

**Project Type**: Library: a Rust crate plus a C ABI. No app, CLI or file I/O (out of scope).

**Performance Goals**: At least 20× real time at 48 kHz on a developer laptop (SC-006). The estimate is about 200 multiply-adds per host sample (≈ 87 down + 87 up + 6 biquads), about 10M MAC/s at 48 kHz, so well over 100× real time.

**Constraints**:
- No allocation, locks or I/O in `process`.
- Per-sample work is constant.
- Default latency ≤ 20 ms.
- Bit-identical output across 8 targets and all block partitions.
- Memory under 1 MB per pipeline.
- No `unsafe` outside `rr_dr60_ffi`.

**Scale/Scope**: About 1.5–2.5k lines of Rust plus tests. 6 rates × 5 configurations in the measurement matrix. 96 golden entries.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | Requirement | How this plan complies | Pre-research | Post-design |
|---|---|---|---|---|
| I. Portable Library First | Pure-Rust core with no OS audio or file I/O. Stable, panic-safe C API with a cbindgen header. Tooling outside the core. | `rr_dr60` is `no_std` + `alloc` and `forbid(unsafe_code)`. `rr_dr60_ffi` uses `catch_unwind` on every entry point, exposes only `repr(C)` types, and has a committed cbindgen header with a CI drift check. The harness and design tool live outside the core. iOS packaging is left to the separate repo. | ✅ | ✅ |
| II. Evidence-Based Fidelity | Every default cites S/A. Assumptions registered. | Stage design cites A-002, A-014, A-015 and A-016. The device rate cites A-001. The generated coefficient file header lists the A-IDs. Engineering targets are labeled per FR-018. A-014 to A-016 are registered in `docs/hardware/assumptions.md`. A CHANGELOG entry is planned for the first release. | ✅ | ✅ |
| III. Measured, Test-First | Measurement tests written first and fail first. Golden files. fmt, clippy and tests pass. ≥ 80% coverage. | Harness checks map 1:1 to FR IDs (R-13). Golden hashes (R-14). The tasks phase orders each test before its implementation. The CI gate includes `cargo llvm-cov --fail-under-lines 80`. The generated coefficient file is data, so no exclusion is needed. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | Seeded PRNG. No clock or OS randomness. No allocation, locks or I/O per block. One engine for offline and real-time use. | Basic IEEE operations only, with libm banned by clippy (R-04). Per-sample state machine (R-09). Counting-allocator test (R-15). `no_std` excludes clock, I/O and locks. The seed is accepted but unused this slice (FR-009). | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | Each stage is configurable, bypassable, testable and tap-able. YAGNI. | The two stages have independent bypass flags and a tap point. Stages are separate modules with analytic unit tests. A fixed pipeline rather than a graph (R-11). No SPICE-level modeling. | ✅ | ✅ |
| VI. Neutral, Honest Communication | Describe signal behavior only. Say "modeled on" / "assumed". | Rustdoc, the README section and the quickstart describe band-limiting only and say "modeled on the MSM7702 (assumed)". There are no EVP claims either way. | ✅ | ✅ |
| VII. Independently Testable Features | P1 alone works. Tests don't depend on other stages. Per-story checkpoints. | US1 works without the US3 harness: it carries its own minimal tone and level tests. Per-stage checks run with the other stage bypassed and subtract the bypass-all baseline. The tasks phase will add a checkpoint per story. | ✅ | ✅ |
| Repo standards | rustdoc on public items, `// SAFETY:` comments, Conventional Commits, SemVer, no large audio. | `#![deny(missing_docs)]` in all published crates. Every `unsafe` block in the FFI crate carries a `// SAFETY:` comment. Version 0.1.0. Golden files are hashes. | ✅ | ✅ |

**Gate result**: PASS. No violations, so Complexity Tracking stays empty.

## Project Structure

### Documentation (this feature)

```text
specs/001-pipeline-skeleton/
├── spec.md
├── plan.md              # this file
├── research.md          # Phase 0 decisions R-01 … R-17
├── data-model.md        # Phase 1 entities and state
├── quickstart.md        # Phase 1 validation and run guide
├── contracts/
│   ├── rust-api.md      # public Rust API of rr_dr60
│   ├── c-api.md         # C ABI of rr_dr60_ffi (header shape, status codes, rules)
│   └── golden-format.md # golden-v1.json schema and bless procedure
├── checklists/requirements.md
└── tasks.md             # created later by /speckit-tasks
```

### Source Code (repository root)

```text
Cargo.toml                         # [workspace] members, shared lints, profile settings
clippy.toml                        # disallowed-methods: libm and mul_add (R-04)
rust-toolchain.toml                # channel = "stable", components fmt/clippy, iOS targets
.cargo/config.toml                 # aarch64-apple-ios-sim runner = scripts/ios-sim-runner.sh

crates/
├── rr_dr60/                       # core: no_std + alloc, forbid(unsafe_code)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                 # crate docs, re-exports; hidden `__test_hooks` (feature `__test-hooks`, R-13)
│       ├── settings.rs            # Settings, Tap, SUPPORTED_HOST_RATES (A-001 defaults)
│       ├── error.rs               # Error enum
│       ├── pipeline.rs            # Pipeline: per-sample engine, latency, reset, reconfigure
│       ├── sanitize.rs            # non-finite and subnormal handling (R-05)
│       ├── rate.rs                # host-rate table → (L, M), converter lengths
│       ├── resample/
│       │   ├── mod.rs
│       │   ├── design.rs          # Kaiser-windowed sinc via rr_dr60_detmath
│       │   ├── down.rs            # polyphase decimator (host → 8 kHz)
│       │   └── up.rs              # polyphase interpolator (8 kHz → host)
│       └── stages/
│           ├── mod.rs
│           ├── biquad.rs          # TDF-II f64 section with state flush
│           ├── voiceband.rs       # VoiceBandStage (stages 4 and 10), group delay at 1 kHz
│           └── voiceband_coeffs.rs  # GENERATED by tools/filter-design (A-014/A-015/A-016)
├── rr_dr60_detmath/               # no_std sin/cos/bessel_i0 using basic operations only (R-06)
│   ├── Cargo.toml
│   └── src/lib.rs
├── rr_dr60_ffi/                   # C ABI (staticlib, cdylib, rlib)
│   ├── Cargo.toml
│   ├── cbindgen.toml
│   ├── include/rr_dr60.h          # GENERATED by cbindgen, committed, drift-checked in CI
│   ├── src/lib.rs
│   └── tests/c/smoke.c            # C smoke test (CI: Linux and macOS)
└── rr_dr60_harness/               # publish = false
    ├── Cargo.toml
    ├── golden/golden-v1.json
    ├── src/
    │   ├── lib.rs
    │   ├── stimulus.rs            # tone, sweep, impulse, silence, DC, PCG32 noise
    │   ├── analysis.rs            # single-bin DFT, group delay, power ratio, min-phase reconstruction
    │   ├── checks.rs              # one fn per FR → MeasurementResult
    │   ├── configs.rs             # the 5 named configurations × 6 rates
    │   ├── golden.rs              # hash, compare, bless
    │   └── report.rs              # pass/fail table with FR and A-/S- IDs
    └── tests/
        ├── us1_voiceband.rs       # US1 acceptance scenarios 1–5
        ├── us2_bypass_tap.rs      # US2 acceptance scenarios 1–4
        ├── response_matrix.rs     # FR-005, FR-010, FR-011 over every rate and config
        ├── latency.rs             # FR-012, FR-013
        ├── edge_cases.rs          # NaN/Inf, over-range, DC, tail, reset, block size 0
        ├── determinism.rs         # block-partition invariance, seed independence
        ├── alloc_free.rs          # counting allocator (FR-022, SC-004)
        ├── ffi_parity.rs          # C API matches Rust API bit for bit; FR-024 errors
        ├── golden.rs              # FR-021, SC-003, SC-008
        └── timing.rs              # #[ignore], release: bounded work and ≥ 20× real time

tools/filter-design/
├── design_voiceband.py            # uv inline-script: design, verify, emit voiceband_coeffs.rs
└── README.md                      # how to regenerate; the CHANGELOG requirement

tools/detmath-refs/
└── gen_refs.py                    # added in implementation: mpmath references for the detmath tests (R-06)

scripts/
├── ios-sim-runner.sh              # cargo runner: xcrun simctl spawn booted <bin>
└── ios-device-golden.sh           # release gate: golden tests on a device via cargo-dinghy

.github/workflows/ci.yml           # matrix per R-16
```

**Structure Decision**: a Cargo workspace under `crates/`, with four crates. Each has one reason to exist:
- **core**: portability.
- **detmath**: deterministic math that the harness can share without widening the core's API.
- **ffi**: integration surface, needs `std` and `unsafe`.
- **harness**: test tooling, unpublished, reusable by later stage specs.

Offline tooling lives in `tools/` and CI helper scripts in `scripts/`.

## Implementation Notes for /speckit-tasks

- **Setup phase (before any story).** This phase creates:
  - The workspace with all 4 crates as compiling skeletons. The harness is a library skeleton plus empty test files, so US1's checkpoint can live there.
  - `clippy.toml`, `rust-toolchain.toml`, and the coefficient file, generated by running `tools/filter-design/design_voiceband.py`.
  - A **basic CI workflow** on ubuntu-latest and macos-latest: fmt, clippy `-D warnings`, test, and `cargo llvm-cov --fail-under-lines 80`. Constitution III requires the coverage gate as soon as the workspace exists.
- **Test-first order per story.** For each story: write its failing tests, implement, then hit the checkpoint.
  - **US1 (P1)**:
    - Settings, error, sanitize, detmath, voiceband stage.
    - Resamplers, built for **48 kHz and 44.1 kHz early**. The 80/441 polyphase path is the biggest technical risk, and US1 AS2 needs it.
    - The pipeline with the default config, then the minimal C API surface (AS5).
    - The remaining rates (8, 16, 88.2, 96 kHz) follow within US1.
    - Checkpoint: `cargo test -p rr_dr60_harness --test us1_voiceband` passes.
  - **US2 (P2)**: bypass flags, tap, the latency table per configuration, and reconfigure.
  - **US3 (P3)**: the full harness (matrix, golden, partition, allocation, timing) and the header drift check.
- **Separate final phase: cross-platform CI matrix** (FR-014, SC-003, R-16). This phase adds:
  - Linux ARM64 runners.
  - Intel macOS, or Rosetta.
  - Windows x86-64 and ARM64.
  - The iOS simulator runner script and iOS build-only targets.
  - The MSRV job.
  - The release timing job.
  - `scripts/ios-device-golden.sh` plus a release-checklist entry.

  The phase is kept separate because it is build infrastructure: it must not block the DSP stories, but the feature is not done until it passes.
- **Coverage.** Coverage counts harness library code, so its `report` and `golden` bless paths need tests or must be exercised by the golden test. The `catch_unwind` panic arms in `rr_dr60_ffi` are **not** excluded, because coverage exclusions need nightly. A test-only panic hook (cfg feature `ffi-test-panic`, enabled by `--all-features`) executes them and the poison and recover path (FR-024). A justification comment is added only if an arm still shows as uncovered.
- **Coefficient file first.** Run `uv run tools/filter-design/design_voiceband.py` before writing the stage tests. The analytic FR-010 tests in `rr_dr60` should fail against an all-pass placeholder before the generated file lands.
- **Documentation and records.**
  - README "Using the library" section.
  - CHANGELOG `[Unreleased]` entries: "Added: pipeline skeleton (stages 4 and 10)" and "Added assumptions A-014–A-016".
  - In CLAUDE.md, mark these open decisions resolved: `no_std` (yes, with alloc), API shape (fixed pipeline with settings), MSRV (1.85).
  - PR template: a golden-bless checkbox.

## Complexity Tracking

There are no constitution violations. Under Principle V ("new complexity MUST be justified"), each structural addition is listed with its reason:

| Addition | Why needed | Simpler alternative rejected because |
|---|---|---|
| 4 crates instead of 1 | The core must be `no_std` and `forbid(unsafe)` (I, IV); the C API needs `std` (`catch_unwind`) and `unsafe`; the harness is unpublished test tooling, reusable by later stage specs (VII). | One crate with features would put `unsafe` and `std` into the core build graph and publish test tooling as API. |
| `rr_dr60_detmath` (in-house sin, cos, Bessel I0) | FR-014 requires bit-identical output across 8 targets. Platform libm differs between OSes, and both the Kaiser taps and the golden stimuli need trig and Bessel functions (R-04, R-06). | Platform libm is not bit-stable. The `libm` crate has arch-specific paths. Committing precomputed resampler taps would mean several MB of generated source. |
| Offline Python and scipy filter-design tool | Elliptic and Butterworth design with analytic verification is standard in scipy. The output is committed as exact bit patterns, so Python never runs in CI or at runtime (R-07). | Writing an elliptic-filter designer in Rust is weeks of work for a design that is fixed. Hand-typed coefficients would have no provenance. |
| Polyphase rational resampler, including the 80/441 ratio | FR-002 requires 44.1 and 88.2 kHz, and FR-005 requires ≥ 60 dB alias rejection and ±0.1 dB flatness (R-08). | Linear or cubic interpolation fails FR-005. Third-party resamplers don't guarantee bit-identity or zero allocation. |
