# CLAUDE.md — rr-dr60-emu

Guidance for Claude Code (and other AI agents) working in this repository. Keep this file current. When a decision changes, update it here.

## Mission

**rr-dr60-emu** is an open-source, portable software emulator of the complete audio signal chain of the **Panasonic RR-DR60** digital voice recorder (mid-1990s IC recorder). The chain runs from the sound reaching the microphone to the sound leaving the speaker. Feed it audio and you get back what an RR-DR60 would have recorded and played back.

- **Primary deliverable:** a portable Rust library (crate `rr_dr60`) with a stable C API.
- **Secondary (later, separate repo):** an iOS app that consumes this library, either directly through the C API / XCFramework or as a port. Design decisions here must keep that path easy, but iOS app code does **not** live in this repo.

## Audience

1. **Paranormal / EVP app creators.** The RR-DR60 is legendary among paranormal investigators as an "EVP recorder". These developers want to embed an authentic RR-DR60 sound in their apps.
2. **Audiophiles and vintage-gear enthusiasts** interested in the device's sonic character.

Both groups need clear docs, honest accuracy statements, and an API that is easy to drop into an app.

## Tone and claims policy (important)

Stay **neutral and technical.** Acknowledge the device's reputation in the EVP community and serve those users well, but describe only what the signal chain physically does: band-limiting, noise, codec artifacts, AGC pumping, VAS gating, and so on.
- Never claim the emulator (or the original device) captures, enhances, or reveals paranormal phenomena. Never claim the opposite either. Don't debunk; just describe.
- Never market accuracy we haven't measured. Say "modeled on…" or "assumed…" until verified.

## Fidelity policy

- **Target for v1: measured character match.** Match measurable properties within stated tolerances: frequency response, noise floor, AGC attack/release, VAS thresholds, codec artifacts, distortion, and output level. Bit-exactness is a non-goal unless the original codec firmware is ever recovered.
- **Reasonable assumptions are allowed when exact specs are unknown.** Every assumption must be recorded in [docs/hardware/assumptions.md](docs/hardware/assumptions.md) with an ID (`A-###`), rationale, confidence, and how it could be verified. Code and specs reference the ID, e.g. `// A-004: assumed 8 kHz sample rate`.
- Every modeled behavior must trace to either a **source** in [docs/hardware/sources.md](docs/hardware/sources.md) or a registered **assumption**. Untraceable "magic numbers" are bugs.
- When new evidence arrives (captures from real units, datasheets), update the assumption's status. Don't silently change behavior.
- We currently have **no physical unit and no reference recordings**. Community-contributed evidence is welcome; see the "Hardware evidence" issue template.

## Scope: the signal chain

The full component-by-component inventory, with known vs. assumed status, lives in [docs/hardware/signal-chain.md](docs/hardware/signal-chain.md). Summary:

mic capsule → mic preamp → AGC → anti-alias filter + ADC (OKI MSM7702 codec) → VAS (voice-activated gating) → noise reduction (claimed) → speech compression → flash storage → decompression → DAC + reconstruction filter → volume / power amp → speaker (or 2.5 mm earphone output).

Each stage should be independently configurable, bypassable, and testable. Exposing intermediate taps is desirable, so app creators can use e.g. "codec only".

## Engineering principles

- **Portable core:** pure Rust, no platform or OS audio dependencies in the core crate. File I/O (WAV) and CLI tools live outside the core (separate crate or feature).
- **C API:** a thin, stable `extern "C"` layer (header generated with cbindgen) is the integration surface for Swift/iOS and other languages. It must be panic-safe (never unwind across FFI) and must use no Rust-only types.
- **Offline and real-time:** one block-based engine serves both batch file processing and live streaming. The processing path must be **real-time safe**: no allocation, locks, I/O, or unbounded work per block. Allocate at construction or configuration time.
- **Deterministic:** same input + same config + same seed ⇒ bit-identical output on every platform. Noise sources use a seeded PRNG. Never use wall-clock time or OS randomness in DSP.
- **Sample-rate agnostic I/O:** accept common host rates (44.1/48 kHz, etc.). Internally model the device's native rate (assumed 8 kHz) with resampling at the boundaries.
- **Test-first with measurable tolerances:** DSP behavior is verified by measurement tests (sweeps, tones, impulses, noise), with tolerances stated in the spec. Golden-file regression tests guard against unintended changes.
- **Independently testable:** every feature, user story, and stage can be tested on its own. Other stages are bypassed or replaced with test doubles or synthetic input.
- **Coverage:** at least 80% workspace line coverage (`cargo llvm-cov`), and no PR may lower it by more than 1 point. Coverage is a backstop; the measurement tests prove correctness.
- **Simplicity:** model what's audible or measurable. Don't simulate circuits at SPICE level unless a spec justifies it.

## Workflow: Spec-Driven Development (GitHub Spec-Kit)

All features go through Spec-Kit. The project principles live in [.specify/memory/constitution.md](.specify/memory/constitution.md), and they override this file if the two ever conflict.

1. `/speckit-specify` → 2. `/speckit-clarify` → 3. `/speckit-plan` → 4. `/speckit-checklist` (optional) → 5. `/speckit-tasks` → 6. `/speckit-analyze` → 7. `/speckit-implement` → 8. `/speckit-converge`

- Specs live in `specs/NNN-feature-name/` (sequential numbering).
- **Coach:** use the `speckit-coach` agent ([.claude/agents/speckit-coach.md](.claude/agents/speckit-coach.md)) for "what's next?" and "poke holes in my spec".
- Keep specs tech-agnostic (WHAT/WHY). Rust, crate layout, and API shapes belong in the plan.
- A suggested first feature: a minimal end-to-end pipeline skeleton (input → device-rate band-limit → output) with the measurement test harness. Then add stages one at a time, each as its own spec.

## Repository standards (public GitHub repo)

- License: **MIT** ([LICENSE](LICENSE)), © 2026 Chance Houston.
- Community files: [README.md](README.md), [CONTRIBUTING.md](CONTRIBUTING.md), [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) (Contributor Covenant), [SECURITY.md](SECURITY.md) (GitHub private vulnerability reporting, no email published), [CHANGELOG.md](CHANGELOG.md) (Keep a Changelog), and issue/PR templates in `.github/`.
- **Semantic Versioning.** The C API and the Rust public API are both versioned surfaces.
- **Conventional Commits** for commit messages (`feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`, `ci:`).
- Before declaring work done, run: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`, and (once set up) `cargo llvm-cov --all-features --workspace --fail-under-lines 80`.
- Public items need rustdoc comments. Avoid `unsafe` outside the FFI layer, and give every `unsafe` block a `// SAFETY:` comment.
- Don't commit large audio files. Keep reference audio small, or document how to generate it.

## Toolchain

- Rust stable (installed via rustup) with targets `aarch64-apple-ios`, `aarch64-apple-ios-sim`, `x86_64-apple-ios` (pinned in `rust-toolchain.toml`). MSRV is **1.85** (edition 2024).
- `~/.cargo/bin` must be on `PATH`. rustup adds `. "$HOME/.cargo/env"` to `~/.zshenv`, but tools and sessions started before the install won't see it.
- `cargo-llvm-cov` (coverage gate) and `cbindgen` 0.29.4 (C header; pinned in CI as `CBINDGEN_VERSION`): `cargo install cargo-llvm-cov cbindgen --locked`.
- Full Xcode (not just the Command Line Tools) plus an iOS simulator runtime, for iOS builds and simulator tests. `.cargo/config.toml` runs `aarch64-apple-ios-sim` tests in a simulator.
- `uv` + `specify` CLI for Spec-Kit (`uv tool upgrade specify-cli` to update).

## Current status

- 2026-10-08: Repo bootstrapped. Spec-Kit initialized, community files added, hardware research started.
- Published at https://github.com/chancehouston/rr-dr60-emu (public). Constitution amended to v1.1.0 (independently testable features, 80% coverage gate).
- 2026-10-09: **Feature 001 (pipeline skeleton) is complete on `main`** (PRs #2, #3 and #4 merged; CI green on all 11 jobs). `/speckit-converge` follow-ups are on branch `001-convergence`. The one remaining manual gate is the iOS-device golden check before the first release (`scripts/ios-device-golden.sh`, `docs/release-checklist.md`). Workspace crates:
  - `rr_dr60`: the `no_std` core.
  - `rr_dr60_detmath`: deterministic math.
  - `rr_dr60_ffi`: the C API and `include/rr_dr60.h`.
  - `rr_dr60_harness`: the measurement harness and golden files (unpublished).
- 2026-10-09: **Feature 002 (record-path AGC, signal-chain stage 3) is complete on `main`** (PR #6 merged; CI green on all 11 jobs; `/speckit-converge` found zero gaps). The AGC is on by default (A-017 – A-020); version 0.2.0. Spec 001 checks run with the AGC bypassed, and `golden-v1.json` is unchanged; the AGC has its own `golden-agc-v1.json`. Measurement lesson (002 research R-11): measure gain timing at host rates as output ÷ an AGC-bypassed reference, not with an FFT envelope.
- CI (`.github/workflows/ci.yml`) runs on pull requests and on pushes to `main`. Feature branches get CI only through an open PR.
- Useful commands:
  - Re-bless golden files after an *intended* output change: `RR_DR60_BLESS=1 cargo test -p rr_dr60_harness --test golden` (spec 001 file) or `RR_DR60_BLESS=agc cargo test -p rr_dr60_harness --test golden_agc` (AGC file). Add a CHANGELOG entry.
  - Regenerate the filter coefficients: `uv run tools/filter-design/design_voiceband.py` (voice band) and `uv run tools/filter-design/design_hilbert.py` (AGC detector).
  - Regenerate the C header: see `crates/rr_dr60_ffi/cbindgen.toml`.
  - Measurement reports: `cargo test -p rr_dr60_harness --test response_matrix -- --nocapture` (filters) and `--test agc_matrix -- --nocapture` (AGC). Slow AGC tests: `cargo test -p rr_dr60_harness --release --test agc_matrix --test agc_edge_cases -- --ignored`.
  - Traceability audit: `scripts/check-traceability.sh`.

## Open decisions (resolve via specs/plan)

- Distribution for iOS: Swift Package wrapping an XCFramework (likely) vs. source port to Swift.

Resolved by spec 001 (see `specs/001-pipeline-skeleton/research.md`):
- `no_std`: **yes**. The core is `no_std` + `alloc` (R-02), which also rules out clock, I/O and locks (FR-016).
- API shape: a **fixed pipeline with a `#[non_exhaustive]` `Settings` struct** (R-11). Presets can be layered on top later.
- MSRV: **1.85** (R-01), checked by the `msrv` CI job.
