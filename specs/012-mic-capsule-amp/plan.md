# Implementation Plan: Microphone Capsule and Amplifier (signal-chain stages 1–2)

**Branch**: `012-mic-capsule-amp` | **Date**: 2026-10-16 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/012-mic-capsule-amp/spec.md`

## Summary

Stages 1–2 run at the host rate inside the record path's front, between the input sanitizer and the limiter, so the one-pass, the recorder, the stop's flush and the frame completion all pass through them by construction (research R-05).

- **Stage 1 (the capsule):** deterministic Gaussian hiss, a ziggurat over xoshiro256++. Each recording gets its own stream, seeded by SplitMix64 from the engine seed and a recording counter. Its level is about −62 dBFS (AES17) in the 300–3000 Hz band after stage 2, plus a −18 … +12 dB offset setting (R-04).
- **Stage 2 (the amplifier):** five steps in the device's order (R-01 – R-03):
  1. the input high-pass, about 820 Hz, a bilinear first-order section;
  2. a gain normalization to 0 dB at 1 kHz;
  3. a memoryless asymmetric soft clipper: negative samples clip first at about +7 dBFS, positive ones at about +16.5 dBFS, using spec 009's knee;
  4. the output coupling, about 76 Hz, a bilinear high-pass;
  5. a magnitude-matched first-order roll-off at about 13 kHz.

  The output after stage 2 is bounded by L₊ + L₋.
- **The circuit model:** stated as a hybrid-π nodal network of S-002 sheet 3.1 in a new design script `tools/mic-chain/design_mic_chain.py`. A plan-time solve gives 820 Hz nominal (744–910 Hz over C1 ±10 % and hFE 150–400). The script prints every figure and writes the coefficients file byte-identically.
- **Configurations:** every pre-012 configuration is rebased on a 0.11-default configuration (stages bypassed), so the seven earlier golden files stay byte-identical (R-06).
- **Interfaces:** two new taps (latency 0), the C structure 112 → 120 bytes, version 0.12.0.
- **Cost:** two cross-build A/Bs against 0.11 (bypassed ≤ 3 %, on ≤ 10 %) plus an in-build report (R-09).

## Technical Context

**Language/Version**: Rust, edition 2024, MSRV 1.85 (001 R-01). Python 3.11 with numpy and scipy via `uv` for the design script (as 004, 008, 009, 011).

**Primary Dependencies**: none new. `rr_dr60_detmath` (`exp`, `ln`) is already a core dependency. ngspice is optional for the design script's `--spice` cross-check; it is not installed on the development machine, and nothing depends on it (R-01).

**Storage**: unchanged. The stored form stays version 2; stages 1–2 precede the memory.

**Testing**:
- **Core**: unit tests in `stages/mic.rs`:
  - each section's coefficients against the script's corner;
  - the clipper's onsets, limits and memorylessness;
  - the L₊ + L₋ bound on random and adversarial inputs (a square wave toggling between the limits);
  - the generator's period guard, seed-0 state, the 4σ count and the stream independence;
  - reset rekeying;
  - bypass identity;
  - offset validation.

  Also: `settings.rs` validation tests, and a `transport.rs` counter test (recording k carries stream k; construction, `reset` and `reconfigure` reset the counter).
- **Harness**:
  - `mic_checks.rs`: the prototype match by impulse, the hiss level, shape and offset, the clipper probe, the bound, the harmonics, the settling, streams and determinism, the voice-activation margin, the limiter oracle, the decoded hiss, the equivalence and the bypass identity;
  - tests `us1_mic` (US1, plus US3 AS1–AS3 at the P1 checkpoint), `us2_mic`, `mic_matrix` (the seven-rate report), `mic_edge_cases` and `golden_mic`;
  - extended: `determinism`, `alloc_free`, `mutation` (four mutants: no input high-pass, a symmetric clipper, a shared stream, a hiss level 3 dB off), `ffi_parity`, `timing` (`output_cost_mic`, `mic_cost`) and `us2_cross_check` (README statements).
- **Golden**: the seven existing files are untouched (SHA guards). `golden-mic-v1.json` (84 entries) is blessed once after the post-implementation reviews.
- **Coverage**: gate 80 %, within 1 point of 0.11's 98.02 % on CI. Under `cfg(coverage)` the audio-heavy loops use `measurement_rates()`; the 600 s and 60-minute checks are release-only.

**Target Platform**: unchanged (8 targets; the golden matrix proves bit-identity on six).

**Project Type**: library, a Rust crate plus a C ABI.

**Performance Goals**: FR-011 as planned in R-09. Each measurement is the fastest of nine alternating solo release runs at 48 kHz, codec bypassed, speaker route.
- Bypassed against 0.11: ≤ 3 %, by `scripts/ab-timing.sh`.
- On against 0.11: ≤ 10 %, by `scripts/ab-timing.sh <rev> output_cost_mic`.
- In-build on against bypassed: ≤ 10 %, asserted in `timing::mic_cost`, with the level spread reported.
- Both A/Bs on the development machine, and once on the CI runner through the PR's `ab-timing` label.
- Plan-time estimate: +8–10 % on.

**Constraints**:
- **Bit-identity to 0.11** with both stages bypassed (FR-010): bypass is a skipped branch decided at construction. The seven SHA guards are run before and after the core change in Phase 1.
- **Determinism:** basic IEEE operations only in the per-sample path; the ziggurat's tail uses `detmath`; no `f64::clamp` or platform math.
- **Real-time safety:** no allocation in the per-sample path; the ziggurat tables are `const`.
- **`unsafe`:** none outside `rr_dr60_ffi`.
- **The manifest:** stages 1–2 change status, assumptions, coverage notes (≤ 120 characters) and controls. One re-bless of the canonical text and the README table, with the version, after the wording review and the denylist run (Phase 1).
- **Wording:** "hiss", "the microphone's self-noise", "modeled on the circuit"; never "accurate", "authentic" or "measured". "Noise floor" only with a level attached.

**Scale/Scope**:
- **Core:** about 450 lines.
  - `stages/mic.rs`, about 300 lines with tests: `MicCapsule` (generator, ziggurat), `MicAmp` (sections, clipper), `MicFront`.
  - The generated `stages/mic_coeffs.rs`, about 250 lines of tables and per-rate constants.
  - `settings.rs` (`MicSettings`, validation, taps), `paths.rs` (the front), `transport.rs` (the counter), and docs.
- **FFI:** about 60 lines (the fields, assertions, taps, version, header).
- **Harness:** about 1,100 lines.
  - `mic_checks.rs`, about 450 lines;
  - five test files, about 450 lines;
  - `configs::settings_011` and the rebase;
  - `golden.rs`;
  - the extensions.
- **Design script:** about 500 lines.
- **Records:** README, seven new and seven amended assumptions, signal-chain, sources, CHANGELOG, issue template, CLAUDE.md.

There are no open NEEDS CLARIFICATION items. The spec's plan-time verification is done: the input corner is confirmed by an independent nodal solve (R-01: 820 Hz nominal, within the reviewer's 0.65–1.05 kHz). One wording deviation is recorded for the plan review: FR-011's "−20 dBFS noise" applies to the in-build measurement, while the cross-build A/Bs use `output_cost`'s fixed 0.11 stimulus (R-09).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | How this plan complies | Pre-research | Post-design |
|---|---|---|---|
| I. Portable Library First | No new dependency. The core stays `no_std` + `alloc`. The C structure grows with explicit reserved bytes and assertions; the old size is refused; two taps are appended; the header is regenerated and drift-checked; the smoke loop is rebased and gains new cases (R-06, R-10). | ✅ | ✅ |
| II. Evidence-Based Fidelity | Every figure traces to S-002 (topology, values, DC voltages) or a new registered assumption, A-046 – A-052, each with confidence and a capture protocol; seven assumptions are amended. The circuit model is stated, and the script prints the figures. "Modeled on", never "measured" (R-11). | ✅ | ✅ |
| III. Measured, Test-First Verification | Each property has a tolerance: prototype ±0.1 dB, hiss ±0.5 dB, shape ±1 dB, clip ±0.5 dB, the exact bound, harmonics ±2 dB (≥ 44.1 kHz), voice-activation margin ≥ 10 dB, limiter ratio 4 ULP. The checks are written and run red before the core (Phase 2's first task). Pins are to the script's print. Four mutants are listed. The seven earlier goldens are guarded and the feature has its own golden file. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | A seeded generator with no platform math; partition independence asserted; `alloc_free` and `determinism` extended; bounded per-sample work, with a rare ziggurat tail of bounded cost through `detmath` (R-04). | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | Two stages, each with its own switch and tap; the manifest's rows 1–2 move to "assumed"; the lumping of the output coupling and the roll-off ahead of the limiter is registered (A-047). | ✅ | ✅ |
| VI. Neutral, Honest Communication | The hiss is described as the capsule's self-noise at a stated level and confidence. No claim about what recordings contain. The README explains the voice-activation coupling and the host-gain arithmetic (R-07, R-11). | ✅ | ✅ |
| VII. Independently Testable Features | US1 and US2 run in the stages-only configurations with synthetic input (impulse, silence, tones) and the clipper probe. US3 uses the existing whole-engine harnesses. US4 uses the cross-check and the audit. The version bump, the registers' skeleton and the manifest re-bless sit in Phase 1, so no story depends on another. | ✅ | ✅ |
| Repo standards | rustdoc on every new public item; SemVer 0.12.0 (an output change and a C break, minor before 1.0); Conventional Commits; no audio committed. | ✅ | ✅ |

**Gate result**: PASS. Two complexity items are recorded below.

## Project Structure

### Documentation (this feature)

```text
specs/012-mic-capsule-amp/
├── plan.md              # This file
├── research.md          # R-01 – R-12
├── data-model.md        # settings, stage objects, stream keying, configurations, golden rows
├── quickstart.md        # the validation runs per story and the gates
├── contracts/
│   ├── rust-api.md      # MicSettings, the taps, mic:: constants, validation, docs
│   ├── c-api.md         # RrDr60Settings 112 → 120, the taps 9/10, version, smoke and parity
│   └── golden-format.md # golden-mic-v1.json: 3 stimuli × 4 configurations × 7 rates = 84
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks output (not created by /speckit-plan)
```

### Source Code (repository root)

```text
tools/mic-chain/design_mic_chain.py   # NEW: the nodal model (R-01), the DC self-check, --spice (optional),
                                      # sections, clipper, hiss anchor and sigma per rate, ziggurat tables,
                                      # voice-activation margins, bound, settling; --check, --write
tools/mic-chain/README.md             # NEW

crates/rr_dr60/src/
├── settings.rs                 # MicSettings { capsule_enabled, amp_enabled, hiss_offset_db } (DEVICE, BYPASSED);
│                               # Settings::mic; Tap::AfterMicCapsule, AfterMicAmp; the seed's docs
├── validate.rs                 # hiss_offset_db finite and in −18 … +12
├── mic.rs                      # NEW public module: HISS_OFFSET_RANGE_DB, FULL_SCALE_SPL_DB (documentation constants)
├── stages/mic.rs               # NEW: MicCapsule (xoshiro256++, SplitMix64 keying, ziggurat), MicAmp
│                               # (FirstOrder ×3, g_n, clipper), MicFront; unit tests
├── stages/mic_coeffs.rs        # NEW, generated by --write (byte-identical check)
├── stages/mod.rs               # pub(crate) mod mic, mic_coeffs
├── paths.rs                    # RecordPath: `front` (sanitize → stages 1–2 → limiter); front_only taps; rekey
├── transport.rs                # the recording counter; start_recording rekeys then increments; reset/reconfigure zero it
├── pipeline.rs                 # the one-pass taps; reset rekeys to counter 0; docs
├── manifest.rs                 # stages 1–2 entries (assumed, A-046 – A-052, controls, model 0.12.0)
└── lib.rs                      # pub mod mic; crate docs; __test_hooks::MicMutation (no_input_hp, symmetric_clip,
                                # shared_stream, hiss_db_delta) and a clipper probe

crates/rr_dr60_ffi/
├── src/lib.rs                  # RrDr60Settings + mic fields (112 → 120), assertions, taps 9/10, VERSION_MINOR 12, docs
├── include/rr_dr60.h           # regenerated
└── tests/c/smoke.c             # earlier cases set mic off; new mic-on cases with pinned hashes

crates/rr_dr60_harness/
├── src/configs.rs              # settings_011; settings_008 rebased; default_output/earphone/loud rebased;
│                               # mic_only, mic_only_quiet, mic_capsule_only, default_mic
├── src/mic_checks.rs           # NEW
├── src/golden.rs               # generate_mic, committed_mic, path_mic, MIC_STIMULI, GOLDEN_MIC_CONFIGS
├── src/smoke_reference.rs      # settings_011 rebase; SMOKE_MIC_* hashes
├── src/output_checks.rs        # Settings::new → settings_011
├── src/analysis.rs             # band_level_aes17, third_octave_levels (if not present), exceedance_count
├── golden/golden-mic-v1.json   # NEW, blessed once (84)
├── golden/manifest-v1.txt      # re-blessed once in Phase 1
├── tests/us1_mic.rs, us2_mic.rs, mic_matrix.rs, mic_edge_cases.rs, golden_mic.rs   # NEW
├── tests/output_edge_cases.rs  # Settings::new → settings_011
└── tests/determinism.rs, alloc_free.rs, mutation.rs, ffi_parity.rs, timing.rs, us2_cross_check.rs   # extended

scripts/ab-timing.sh (optional head-test argument), .github/workflows/ci.yml (golden_mic ×3),
scripts/ios-device-golden.sh, .github/ISSUE_TEMPLATE/hardware_evidence.yml (seven captures)
docs/hardware/assumptions.md (A-046 – A-052 new; A-005, A-006, A-019, A-022, A-028, A-029, A-036 amended),
docs/hardware/signal-chain.md (rows 1–2), docs/hardware/sources.md (S-002 summary), README.md (the microphone
section; the default-engine figures labelled), CHANGELOG.md (0.12.0), CLAUDE.md, Cargo.toml (0.12.0 × 4)
```

**Structure Decision**: the existing three-crate workspace. One new core stage module plus its generated coefficients, one public documentation module, one new check module and five test files in the harness, and one new design tool. This matches 009's layout.

## Task order (for /speckit-tasks)

1. **Phase 1, Setup.**
   - Version bump: `Cargo.toml` × 4, `RR_DR60_VERSION_MINOR = 12`, the header.
   - The design script: the nodal model and the DC self-check first, then the sections, clipper, hiss anchor, ziggurat tables, bound and margins, `--check` and `--write` producing `mic_coeffs.rs`. Its R-01 table is compared with research before anything else.
   - The register rows A-046 – A-052 as skeletons, with the figures from the script's print.
   - Stages 1–2's manifest entries, after the wording review and the denylist run on the new strings; **one re-bless** of the canonical text and the README table.
   - `MicSettings`, the taps and validation, inert: stages built but bypassed in every configuration until Phase 2.
   - `configs::settings_011` and the rebase of every pre-012 configuration, the direct `Settings::new` uses and the C smoke's earlier cases.
   - The seven SHA guards run green.
   - Checkpoint: the full suite green with the stages inert; bit-identity to 0.11 by construction.
2. **Phase 2, US1** (band and hiss).
   - Write `mic_checks` (prototype by impulse, hiss level, shape and offset, streams, 4σ) and run them **red**.
   - Then the core: `stages/mic.rs` sections, generator and keying, the front in `paths.rs`, the counter in `transport.rs`, the default on.
   - Then `us1_mic` and `mic_matrix` (US1 rows).
   - **P1 checkpoint**: also US3 AS1–AS3 (bypass identity, equivalence of the first recording, rest and rekey) and `determinism` / `alloc_free`, because the hiss is on by default from here.
3. **Phase 3, US2** (clipping).
   - The clipper with its probe hook; `us2_mic`; the bound with adversarial inputs; harmonics at ≥ 44.1 kHz with alias levels reported; settling.
   - Checkpoint: SC-003.
4. **Phase 4, US3 rest and US4.**
   - The voice-activation margins (60 s, measurement rates; the offset bisection), the limiter oracle and the decoded hiss.
   - `mutation` (four mutants), `timing::output_cost_mic` and `mic_cost`, `scripts/ab-timing.sh`'s head-test option and the two A/Bs on the development machine.
   - The README's microphone section and the default-engine figures labelled, rustdoc, the registers filled (A-022 re-derived), signal-chain, sources, the issue template, the CHANGELOG, `us2_cross_check`'s statements, the audit.
   - Checkpoint: SC-006, SC-007, SC-010.
5. **Phase 5, C.**
   - The fields, assertions, taps, header, `smoke.c`'s new cases and `smoke_reference.rs`'s hashes, `ffi_parity`'s cases.
   - Checkpoint: the C loop, SC-009.
6. **Phase 6, Records and gates**, in spec 010's gate order:
   1. the gate **without** `golden_mic` (fmt, clippy, tests, the MSRV test build with `-D warnings`, the header drift check, the script's `--check` and the byte-identical `--write`);
   2. the solo timing runs quoted;
   3. the post-implementation hardware review and the coach review, both on the session's model;
   4. **the golden bless, once, after the reviews**, its SHA pinned, in the same commit as `ci.yml` and `scripts/ios-device-golden.sh`;
   5. the full gate with coverage;
   6. CLAUDE.md; the PR (ask first); the `ab-timing` label; ask before the merge; then `/speckit-converge`.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| A ziggurat with generated tables (about 260 table entries) instead of a short formula | FR-003 requires Gaussian tails beyond ±5σ, deterministic and within the cost budget. The voice-activation margin depends on the tails (D10). | Box–Muller through `detmath` costs about 15–25 ns per sample, over the budget alone. Irwin–Hall(12) has light tails that fail the 4σ test and flatter the margin (R-04). |
| A recording counter in the split engine | The user chose a distinct deterministic stream per recording (Clarifications D9). The first recording must still equal the one-pass. | A shared stream makes every recording's hiss identical. Entropy from the OS breaks determinism (constitution IV). |
