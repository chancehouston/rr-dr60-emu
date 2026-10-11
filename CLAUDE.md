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
- **Reasonable assumptions are allowed when exact specs are unknown.** Every assumption must be recorded in [docs/hardware/assumptions.md](docs/hardware/assumptions.md) with an ID (`A-###`), rationale, confidence, and how it could be verified. Code and specs reference the ID, e.g. `// A-026: 6 kHz device rate (S-002)`.
- Every modeled behavior must trace to either a **source** in [docs/hardware/sources.md](docs/hardware/sources.md) or a registered **assumption**. Untraceable "magic numbers" are bugs.
- When new evidence arrives (captures from real units, datasheets), update the assumption's status. Don't silently change behavior.
- We currently have **no physical unit and no reference recordings**. Community-contributed evidence is welcome; see the "Hardware evidence" issue template.

## Scope: the signal chain

The full component-by-component inventory, with known vs. assumed status, lives in [docs/hardware/signal-chain.md](docs/hardware/signal-chain.md). Summary:

mic capsule → mic preamp → AGC (a limiter at the codec's full scale) → anti-alias filter + ADC (OKI MSM7702 codec) + full-scale clamp → VAS (voice-activated gating) → noise reduction (claimed) → speech compression → flash storage → decompression → DAC + reconstruction filter → volume / power amp → speaker (or 2.5 mm earphone output).

Each stage should be independently configurable, bypassable, and testable. Exposing intermediate taps is desirable, so app creators can use e.g. "codec only".

## Engineering principles

- **Portable core:** pure Rust, no platform or OS audio dependencies in the core crate. File I/O (WAV) and CLI tools live outside the core (separate crate or feature).
- **C API:** a thin, stable `extern "C"` layer (header generated with cbindgen) is the integration surface for Swift/iOS and other languages. It must be panic-safe (never unwind across FFI) and must use no Rust-only types.
- **Offline and real-time:** one block-based engine serves both batch file processing and live streaming. The processing path must be **real-time safe**: no allocation, locks, I/O, or unbounded work per block. Allocate at construction or configuration time.
- **Deterministic:** same input + same config + same seed ⇒ bit-identical output on every platform. Noise sources use a seeded PRNG. Never use wall-clock time or OS randomness in DSP.
- **Sample-rate agnostic I/O:** accept common host rates (6/8/16/44.1/48/88.2/96 kHz). Internally model the device's native rate (6 kHz, verified by S-002, A-026; adopted by spec 004) with resampling at the boundaries; 6 kHz is the identity host rate.
- **Test-first with measurable tolerances:** DSP behavior is verified by measurement tests (sweeps, tones, impulses, noise), with tolerances stated in the spec. Golden-file regression tests guard against unintended changes.
- **Independently testable:** every feature, user story, and stage can be tested on its own. Other stages are bypassed or replaced with test doubles or synthetic input.
- **Coverage:** at least 80% workspace line coverage (`cargo llvm-cov`), and no PR may lower it by more than 1 point. Coverage is a backstop; the measurement tests prove correctness.
- **Simplicity:** model what's audible or measurable. Don't simulate circuits at SPICE level unless a spec justifies it.

## Workflow: Spec-Driven Development (GitHub Spec-Kit)

All features go through Spec-Kit. The project principles live in [.specify/memory/constitution.md](.specify/memory/constitution.md), and they override this file if the two ever conflict.

1. `/speckit-specify` → 2. `/speckit-clarify` → 3. `/speckit-plan` → 4. `/speckit-checklist` (optional) → 5. `/speckit-tasks` → 6. `/speckit-analyze` → 7. `/speckit-implement` → 8. `/speckit-converge`

- Specs live in `specs/NNN-feature-name/` (sequential numbering).
- **Coach:** use the `speckit-coach` agent ([.claude/agents/speckit-coach.md](.claude/agents/speckit-coach.md)) for "what's next?" and "poke holes in my spec".
- **Technical reviewer:** use the `hw-emulation-reviewer` agent ([.claude/agents/hw-emulation-reviewer.md](.claude/agents/hw-emulation-reviewer.md)) for deep reviews of plans, progress and decisions: hardware plausibility, DSP correctness, measurement validity, and evidence gaps. It is read-only. The coach covers process and requirement quality; this agent covers technical substance.
- **Product specialist:** use the `product-specialist` agent ([.claude/agents/product-specialist.md](.claude/agents/product-specialist.md)) for the product side of the app built on this engine and of the engine itself: users and positioning, free tier vs. in-app purchase, store compliance, trademark and claims wording, and which engine capabilities the app needs (returned as ready-to-paste feature descriptions). It may write only under `docs/product/`; the other three lenses stay with the coach and the reviewer.
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
- 2026-10-09: **Feature 001 (pipeline skeleton) is complete on `main`** (PRs #2–#5 merged, including the `/speckit-converge` follow-ups; CI green on all 11 jobs). The one remaining manual gate is the iOS-device golden check before the first release (`scripts/ios-device-golden.sh`, `docs/release-checklist.md`). Workspace crates:
  - `rr_dr60`: the `no_std` core.
  - `rr_dr60_detmath`: deterministic math.
  - `rr_dr60_ffi`: the C API and `include/rr_dr60.h`.
  - `rr_dr60_harness`: the measurement harness and golden files (unpublished).
- 2026-10-09: **Feature 002 (record-path AGC, signal-chain stage 3) is complete on `main`** (PR #6 merged; CI green on all 11 jobs; `/speckit-converge` found zero gaps). The AGC is on by default (A-017 – A-020); version 0.2.0. Spec 001 checks run with the AGC bypassed, and `golden-v1.json` is unchanged; the AGC has its own `golden-agc-v1.json`. Measurement lesson (002 research R-11): measure gain timing at host rates as output ÷ an AGC-bypassed reference, not with an FFT envelope.
- 2026-10-10: **Hardware evidence pass.** The service manual (S-002) and the codec family datasheet (S-005) were read in full; see `docs/hardware/sources.md` and the register rows A-026–A-033. Headline facts: **6 kHz sampling** (not 8), **4 kbit/s CELP** on a Panasonic MN1931712BB DSP, 16 Mbit flash, µ-law confirmed, a **limiter-type AGC** at the codec's full scale with a ~10 s release (not a 10:1 compressor), PLL-clocked codec filters (band ≈ 225–2550 Hz at 6 kHz), an earphone path that bypasses the power amp, and the sensitivity setting / VAS implemented in DSP firmware after the AGC. The code still models 8 kHz, 300–3400 Hz and the spec 002 AGC; planned follow-ups: a codec-rate/band feature (A-026, A-027) and an AGC revision feature (A-028), each a CHANGELOG-recorded assumption change. The MSM7702's own datasheet pages 4–17 are still unread (S-003 leads in sources.md).
- 2026-10-10: **Feature 003 (record-path VAS, signal-chain stage 5) is complete on `main`** (PR #11 merged; CI green on all 11 jobs). The VAS is on by default in drop mode (A-008, A-022 – A-025), so the default output can be shorter than the input: every processing call returns `BlockInfo`, and `process_with_events`/`max_events` report splices (or muted regions in mute mode) without allocating. The C `rr_dr60_process` gained `out_info` (compile break); the settings struct is 72 bytes; version 0.3.0. Specs 001 and 002 run with the VAS bypassed and their golden files are SHA-guarded; the VAS has `golden-vas-v1.json` (54 entries with events). Tests: `us1_vas`, `us2_vas_settings`, `vas_matrix`, `vas_edge_cases`, `golden_vas`. Measurement lessons (003 R-11): measure the VAS by **output length and events**, never by an amplitude envelope; at resampled rates the last kept samples are still inside the conversion when the input ends, so append one latency of silence to a stimulus that must end "while recording"; and a burst that starts at a zero crossing resumes one device sample late, so nominal-arithmetic comparisons need one device sample of slack. Known from the evidence pass (A-022, A-033): the default threshold was about −58 dBFS input-referred through the spec 002 compressor (restated by spec 005: −40 dBFS at level 3, input-referred one to one); the sensitivity setting's placement (threshold vs. a digital gain) is open for the stage-2 feature.
- 2026-10-11: **Feature 004 (6 kHz device rate and the codec filter templates) is complete on `main`** (PR #13 merged; CI green on all 11 jobs). The device rate is 6 kHz (A-026 adopted, A-001 superseded), the two band-limit stages are the codec family's transmit band-pass and receive low-pass templates scaled to that clock (A-027 adopted; A-002, A-014 superseded; the playback stage passes DC), 6 kHz is a supported host rate and the identity, 8 and 16 kHz are conversions, `SUPPORTED_HOST_RATES` is `[u32; 7]` (breaking), default latency at 48 kHz 541 → 483 samples; version 0.4.0. All three golden files were re-blessed once; the restated figures of specs 001–003 live in the CHANGELOG's 0.4.0 entry. Measurement lessons (004 research R-07, R-08): evaluate band-edge group delay at exact frequencies with the DTFT, never from adjacent FFT bins; and a tone with a whole number of device samples per period (1 kHz has 6) that feeds a sample-peak detector (the VAS threshold bisections, and the AGC's peak-hold branch for the first 31 device samples after a step) must start at `stimulus::aligned_phase`, or it reads up to 1.25 dB low. The design script picks the highest stopband edge within ±0.1 dB of the datasheet typical, so the stages' 2550 Hz loss is 0.31/0.48 dB against typicals of 0.4/0.56: say "modeled on the datasheet scaled to the measured clock", never "measured".
- 2026-10-11: **Feature 005 (limiter-type AGC, ADC clamp, VAS threshold re-anchored) is complete on `main`** (PR #15 merged; CI green on all 11 jobs). The record-path AGC is now the **limiter** the service manual's schematic shows (A-028 adopted and reinterpreted; A-017/A-018 superseded; A-019, A-022 revised; A-034 dead time, A-035 release, A-036 polarity new): unity below a knee at **−1 dBFS** (bit-exact; +1 dBFS until the post-implementation review showed the hard 0.6 V threshold is the attack-phase Vbe and the steady-state ceiling sits just below full scale), a hard ceiling above it after a charge dead time (≈ 3 ms at 10 dB over, ≈ 20–25 ms at 2 dB over, ≤ 0.5 ms for ≈ 3 s after a loud sound; the arming decays with a 10 s time constant), a fast-then-slow release ≈ 1.4 s, a half-wave detector on positive samples; it runs at the **host rate before the decimator**, and `Tap::AfterAgc` is now the host-rate limiter output with latency 0. The codec's **full-scale clamp** is a new stage after the record filter (`adc_clamp_enabled`, A-030; the default output can still overshoot ±1.0 on clipped audio because the playback stage reconstructs flat tops). The VAS threshold is **−40 dBFS** at level 3 (A-022 revised; input-referred one to one). Settings: `agc.{knee_dbfs, attack_ms (dead time), release_ms}`; C struct 72 → 80 bytes with `agc_knee_dbfs` at 28, two reserved slots, `adc_clamp_enabled` at 72; version 0.5.0. The AGC and VAS golden files were re-blessed twice on the branch (the limiter, then the knee move); `golden-v1.json` is byte-identical. Everything is **modeled on the circuit (S-002) with estimated transistor parameters, not measured**; the open question is whether a real unit limits just below full scale (the model) or clips at it; the SPL anchor is 80–100 dB SPL for full scale (A-029, ±10 dB). The reference model `tools/agc-model/limiter_model.py` prints every design figure. Measurement lessons (005 research R-03, R-04, R-08): stimuli that feed the limiter at the host-rate tap start at `stimulus::peak_phase` (a host sample on the positive peak; spec 004's device-grid `aligned_phase` misses the 8 kHz peak by 15°); the dead time is the first sample whose attenuation exceeds **half the steady value** (the same relative rule for every step size); the attack setting floors near 1 ms (the R12 charge-current limit); the gain is exactly unity again about 18 s after a burst and the control flushes to 0.0 about 140 s after it (state-equality tests feed > 140 s or assert `limiter_state() == 0.0`); the FR-005 ramp and armed figures **scale with the charge rate**, so they are reported for non-default settings; a release-scaled quiet gap must be rounded to whole 1 kHz cycles so the next burst starts on a peak; the static tones stay 5 s long under coverage instrumentation (a 1 s tone's "last second" contains the onset, whose dead time grows without bound toward the knee, and the knee bisection then reads its upper bound); FR-005's "ramp ≤ 1 ms" is an above-the-rail figure (a step that stays below the codec input amp's rail waits up to one cycle for the last dB); stimulus levels in the limiter checks are relative to the knee ("10 dB over" is +9 dBFS), and a core pin to the reference script must be to a figure the script prints, never to the Rust's own output. Timing at 48 kHz: 59.6 / 58.1 / 58.1 ns per sample (361× / 359× / 358×); coverage 98.99 %.
- CI (`.github/workflows/ci.yml`) runs on pull requests and on pushes to `main`. Feature branches get CI only through an open PR. Changes that touch only documentation (Markdown, `docs/`, `specs/`, `.specify/`, `.claude/`, issue templates, LICENSE) skip CI entirely; the coverage job alone takes 15–30 min on the shared runner.
- Useful commands:
  - Re-bless golden files after an *intended* output change: `RR_DR60_BLESS=1 cargo test -p rr_dr60_harness --test golden` (spec 001 file), `RR_DR60_BLESS=agc cargo test -p rr_dr60_harness --test golden_agc` (AGC file) or `RR_DR60_BLESS=vas cargo test -p rr_dr60_harness --test golden_vas` (VAS file). Add a CHANGELOG entry.
  - Regenerate the filter coefficients: `uv run tools/filter-design/design_voiceband.py` (both codec templates: record band-pass and playback low-pass at 6 kHz; `--check` prints the margin tables). The limiter's reference model (the figures the spec 005 unit tests pin): `python3 tools/agc-model/limiter_model.py`.
  - Regenerate the C header: see `crates/rr_dr60_ffi/cbindgen.toml`.
  - Measurement reports: `cargo test -p rr_dr60_harness --test response_matrix -- --nocapture` (both stage templates, boundary, latency, anti-alias, cascade and minimum phase at seven rates; 413 rows), `--test latency -- --nocapture` (the per-rate latency table), `--test agc_matrix -- --nocapture` (the limiter: 563 default rows at seven rates and the 290-row settings matrix at 6/48 kHz) and `--test chain -- --nocapture` (the limiter-plus-VAS chain scenarios). Slow AGC tests: `cargo test -p rr_dr60_harness --release --test agc_matrix --test agc_edge_cases -- --ignored`. VAS: `cargo test -p rr_dr60_harness --test vas_matrix -- --nocapture` (including the US3 AS3 floor-sweep `interplay_report`); slow VAS tests: `cargo test -p rr_dr60_harness --release --test vas_matrix --test vas_edge_cases -- --ignored`.
  - Traceability audit: `scripts/check-traceability.sh`.

## Open decisions (resolve via specs/plan)

- Distribution for iOS: Swift Package wrapping an XCFramework (likely) vs. source port to Swift.

Resolved by spec 001 (see `specs/001-pipeline-skeleton/research.md`):
- `no_std`: **yes**. The core is `no_std` + `alloc` (R-02), which also rules out clock, I/O and locks (FR-016).
- API shape: a **fixed pipeline with a `#[non_exhaustive]` `Settings` struct** (R-11). Presets can be layered on top later.
- MSRV: **1.85** (R-01), checked by the `msrv` CI job.
