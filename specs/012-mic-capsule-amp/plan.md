# Implementation Plan: Microphone Capsule and Amplifier (signal-chain stages 1–2)

> **As built (2026-10-16, after the post-implementation reviews):** this file records the design as planned. Where it differs from the implementation, the spec's Clarifications › Post-implementation reviews, research R-03 › As built, R-04 and R-09 › As measured hold: the clipper has one limit and one knee per polarity (`Clipper { l_pos, l_neg, k_pos, k_neg, … }`, L₋ ≈ +9.15 dBFS, k₋ = 0.65, L₊ ≈ +17.10 dBFS, k₊ = 0.80); the keying is nested, `mix64(mix64(seed ^ MIC_STREAM) ^ counter ^ MIC_COUNTER_SALT)`; `MicFront` holds the capsule and the amplifier; `reset` keys the one-pass to the counter's current value (FR-009), never back to 0; the registers run A-046 – A-053; the golden file holds 147 entries with the earlier files' `n_samples` and `sha256` fields; the in-build cost protocol is 45 strictly alternating runs.

**Branch**: `012-mic-capsule-amp` | **Date**: 2026-10-16 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/012-mic-capsule-amp/spec.md`

## Summary

Stages 1–2 run at the host rate inside the record path's front, between the input sanitizer and the limiter, so the one-pass, the recorder, the stop's flush and the frame completion all pass through them by construction (research R-05).

- **Stage 1 (the capsule):** deterministic Gaussian hiss, a capped ziggurat over xoshiro256++. Each recording gets its own stream, keyed from the engine seed and a recording counter that never rewinds. Its level is about −62 dBFS (AES17) in the 300–3000 Hz band after stage 2, plus a −18 … +12 dB offset setting (R-04, R-05).
- **Stage 2 (the amplifier):** four steps in the device's order (R-01 – R-03):
  1. the input high-pass, about 818 Hz, carrying the 0 dB normalization at 1 kHz;
  2. a memoryless asymmetric soft clipper, exactly linear below the onset: negative samples are limited first at L₋ ≈ +7 dBFS, positive ones at L₊ ≈ +16.5 dBFS, with spec 009's knee (k = 0.85);
  3. the output coupling, about 77 Hz, in the bilinear form;
  4. the roll-off, about 12.9 kHz.

  The three sections are a per-rate constrained fit within ±0.1 dB of the prototype (worst 0.075 dB at 48 kHz). The output after stage 2 is bounded by L₊ + L₋.
- **The circuit model:** stated as a hybrid-π nodal network of S-002 sheet 3.1 in a new design script `tools/mic-chain/design_mic_chain.py`. It solves the transfer to the codec input amplifier's output: 818 / 77 / 12 939 Hz nominal, input corner 665–985 Hz over C1 ±10 %, hFE, the Early voltage and Q3's current. It is cross-checked by ngspice (installed) before the bless. The script prints every figure and writes the coefficients file byte-identically.
- **Configurations:** every pre-012 configuration is rebased on a 0.11-default configuration (stages bypassed), so the seven earlier golden files stay byte-identical (R-06).
- **Interfaces:** two new taps (latency 0), the C structure 112 → 120 bytes, version 0.12.0.
- **Cost:** in-build on against bypassed ≤ 10 % (asserted); the cross-build A/B against 0.11 with the stages bypassed ≤ 3 % (asserted); the cross-build "on" A/B reported (R-09).

## Technical Context

**Language/Version**: Rust, edition 2024, MSRV 1.85 (001 R-01). Python 3.11 with numpy and scipy via `uv` for the design script (as 004, 008, 009, 011).

**Primary Dependencies**: none new in the build. `rr_dr60_detmath` (`exp`, `ln`) is already a core dependency. ngspice 47 (Homebrew, installed 2026-10-16) runs the design script's `--spice` cross-check, a gate before the golden bless (R-01). It is design-time only.

**Storage**: unchanged. The stored form stays version 2; stages 1–2 precede the memory.

**Testing**:
- **Core**: unit tests in `stages/mic.rs`:
  - each section's coefficients against the script's corner;
  - the clipper's onsets, limits and memorylessness;
  - the L₊ + L₋ bound on random and adversarial inputs (a square wave toggling between the limits);
  - the generator's seed-0 state, the keying, the retry cap through a hook, and (release-only, 10⁸ draws) the 4σ and 5σ counts;
  - reset rekeying;
  - bypass identity;
  - offset validation.

  Also: `settings.rs` validation tests, and a `transport.rs` test for every row of R-05's counter table. The counter never rewinds.
- **Harness**:
  - `mic_checks.rs`: the prototype match by impulse, the hiss level, shape and offset, the clipper probe, the bound, the harmonics, the settling, streams and determinism, the voice-activation margin, the limiter oracle, the decoded hiss, the equivalence and the bypass identity;
  - tests `us1_mic` (US1, plus US3 AS1–AS3 at the P1 checkpoint), `us2_mic`, `mic_matrix` (the seven-rate report), `mic_edge_cases` and `golden_mic`;
  - extended: `determinism`, `alloc_free`, `mutation` (five mutants: no input high-pass, a symmetric clipper, swapped polarity, a shared stream, a hiss level 3 dB off), `ffi_parity`, `timing` (`output_cost_mic`, `mic_cost`) and `us2_cross_check` (README statements).
- **Golden**: the seven existing files are untouched (SHA guards). `golden-mic-v1.json` (147 entries) is blessed once after the post-implementation reviews.
- **Coverage**: gate 80 %, within 1 point of 0.11's 98.02 % on CI. Under `cfg(coverage)` the audio-heavy loops use `measurement_rates()`; the 600 s and 60-minute checks are release-only.

**Target Platform**: unchanged (8 targets; the golden matrix proves bit-identity on six).

**Project Type**: library, a Rust crate plus a C ABI.

**Performance Goals**: FR-011 as amended at the plan review (R-09). Each measurement is the fastest of nine alternating solo release runs at 48 kHz, codec bypassed, speaker route.
- In-build on against bypassed, on −20 dBFS noise: ≤ 10 %, asserted in `timing::mic_cost` on the development machine, and under a 25 % ceiling on CI.
- Bypassed against 0.11 (`c9f9994`): ≤ 3 %, by `scripts/ab-timing.sh`, on the development machine and once on CI through the `ab-timing` label.
- On against 0.11: reported, by `scripts/ab-timing.sh c9f9994 output_cost_mic`, with the limiter uncharged at the end.
- Plan-time estimate: +8–10 % on.

**Constraints**:
- **Bit-identity to 0.11** with both stages bypassed (FR-010): bypass is a skipped branch decided at construction. The seven SHA guards are run before and after the core change in Phase 1.
- **Determinism and bounded work:** basic IEEE operations only in the per-sample path; the ziggurat's tail uses `detmath`; every rejection loop is capped at 8 attempts; no `f64::clamp` or platform math.
- **Real-time safety:** no allocation in the per-sample path; the ziggurat tables are `const`.
- **`unsafe`:** none outside `rr_dr60_ffi`.
- **The manifest:** stages 1–2 change status, assumptions, coverage notes (≤ 120 characters) and controls. One re-bless of the canonical text and the README table, with the version, after the wording review and the denylist run (Phase 1). One review-driven re-bless is allowed in Phase 6.
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

There are no open NEEDS CLARIFICATION items. The plan reviews (the coach's P1–P16, the hardware reviewer's Q1–Q15) are applied, and the spec was amended accordingly (its Clarifications, plan-review session). The input corner is confirmed by two independent nodal solves (R-01: 818–820 Hz nominal), with the ngspice cross-check scheduled before the bless.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | How this plan complies | Pre-research | Post-design |
|---|---|---|---|
| I. Portable Library First | No new dependency. The core stays `no_std` + `alloc`. The C structure grows with explicit reserved bytes and assertions; the old size is refused; two taps are appended; the header is regenerated and drift-checked; the smoke loop is rebased and gains new cases (R-06, R-10). | ✅ | ✅ |
| II. Evidence-Based Fidelity | Every figure traces to S-002 (topology, values, DC voltages) or a new registered assumption, A-046 – A-052, each with confidence and a capture protocol; seven assumptions are amended. The circuit model is stated, and the script prints the figures. "Modeled on", never "measured" (R-11). | ✅ | ✅ |
| III. Measured, Test-First Verification | Each property has a tolerance: prototype ±0.1 dB, hiss ±0.5 dB, shape ±1 dB, clip ±0.5 dB, the exact bound, harmonics ±0.5 dB of the discrete model, voice-activation margin ≥ 10 dB, the limiter ratios 4 f32 ULP, tail counts ±5 % / ±40 % on 10⁸ draws. **Each story's checks are written and run red before its implementation**: US1 in Phase 2, US2 in Phase 3 against a linear stand-in, US3 in Phase 4a, the C cases with placeholder hashes in Phase 5. Pins are to the script's print. Five mutants are listed. The seven earlier goldens are guarded and the feature has its own golden file. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | A seeded generator with no platform math; partition independence asserted; `alloc_free` and `determinism` extended. The work per sample is bounded: each ziggurat rejection loop is capped at 8 attempts (probability of reaching the cap about 1e-16, registered in A-049), and the tail uses `detmath` (R-04). | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | Two stages, each with its own switch and tap; the manifest's rows 1–2 move to "assumed"; the lumping of the output coupling and the roll-off ahead of the limiter is registered (A-053). The nodal model and the ngspice run are **design-time only**, justified by FR-005. The runtime model is three first-order sections and a memoryless clipper, not a circuit simulation. | ✅ | ✅ |
| VI. Neutral, Honest Communication | The hiss is described as the capsule's self-noise at a stated level and confidence. No claim about what recordings contain. The README explains the voice-activation coupling and the host-gain arithmetic (R-07, R-11). | ✅ | ✅ |
| VII. Independently Testable Features | US1 and US2 run in the stages-only configurations with synthetic input (impulse, silence, tones) and the clipper probe. US3 uses the existing whole-engine harnesses. US4 uses the cross-check and the audit. **Each story has its own checkpoint** (Phases 2, 3, 4a, 4b). The version bump, the C fields, the registers' skeleton and the manifest re-bless sit in Phase 1, so no story depends on another. | ✅ | ✅ |
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
tools/mic-chain/design_mic_chain.py   # NEW: the nodal model and joint corner fit (R-01), the DC consistency check,
                                      # --spice (ngspice gate), the per-rate constrained fit (R-02), clipper figures
                                      # in both unit systems and the phase compensation (R-03), hiss anchor and sigma
                                      # per rate, ziggurat tables, keying constants (R-04), voice-activation margins,
                                      # bound, settling, discrete harmonics; --check, --write
tools/mic-chain/rr_dr60_mic.cir       # NEW: the ngspice netlist and generic transistor models
tools/mic-chain/README.md             # NEW

crates/rr_dr60/src/
├── settings.rs                 # MicSettings { capsule_enabled, amp_enabled, hiss_offset_db } (DEVICE, BYPASSED);
│                               # Settings::mic; Tap::AfterMicCapsule, AfterMicAmp; the seed's docs
├── validate.rs                 # hiss_offset_db finite and in −18 … +12
├── mic.rs                      # NEW public module: HISS_OFFSET_RANGE_DB (no public SPL constant)
├── stages/mic.rs               # NEW: MicCapsule (xoshiro256++, mix64 keying, capped ziggurat), MicAmp
│                               # (FirstOrder ×3 with the normalization in HP_in, the select-form clipper), MicFront; unit tests
├── stages/mic_coeffs.rs        # NEW, generated by --write (byte-identical check)
├── stages/mod.rs               # pub(crate) mod mic, mic_coeffs
├── paths.rs                    # RecordPath: `front` (sanitize → stages 1–2 → limiter); front_only taps; rekey
├── transport.rs                # the recording counter (never rewinds; R-05's table); start_recording rekeys after open() then increments;
│                               # StopOutcome::silence_fed_samples docs
├── pipeline.rs                 # the one-pass taps; reset rekeys to counter 0; docs
├── manifest.rs                 # stages 1–2 entries (assumed, A-046 – A-052, controls, model 0.12.0)
├── test_util.rs                # settings_011 for the core's tests (the audit, R-06)
└── lib.rs                      # pub mod mic; crate docs; __test_hooks::MicMutation (no_input_hp, symmetric_clip,
                                # swapped_polarity, shared_stream, hiss_db_delta, clipper_off, force_retry_cap) and the
                                # clipper probe (the engine's own clip_in / clip_out per sample)

crates/rr_dr60_ffi/
├── src/lib.rs                  # RrDr60Settings + mic fields (112 → 120), assertions, taps 9/10, VERSION_MINOR 12, docs
├── include/rr_dr60.h           # regenerated
└── tests/c/smoke.c             # earlier cases set mic off; new mic-on cases with pinned hashes

crates/rr_dr60_harness/
├── src/configs.rs              # settings_011; settings_008 rebased; default_output/earphone/loud rebased;
│                               # mic_only, mic_only_quiet, mic_capsule_only, default_mic, default_mic_quiet
├── src/mic_checks.rs           # NEW
├── src/golden.rs               # generate_mic, committed_mic, path_mic, MIC_STIMULI, GOLDEN_MIC_CONFIGS
├── src/smoke_reference.rs      # settings_011 rebase; SMOKE_MIC_* hashes
├── src/output_checks.rs        # Settings::new → settings_011
├── src/analysis.rs             # band_level_aes17, a 20 Hz – 20 kHz third-octave centre list (the existing third_octave_levels
│                               # covers 315–2500 Hz), exceedance_count, normalized cross-correlation over lags
├── golden/golden-mic-v1.json   # NEW, blessed once (147)
├── golden/manifest-v1.txt      # re-blessed once in Phase 1
├── tests/us1_mic.rs, us2_mic.rs, mic_matrix.rs, mic_edge_cases.rs, golden_mic.rs   # NEW
├── tests/output_edge_cases.rs  # Settings::new → settings_011
└── tests/determinism.rs, alloc_free.rs, mutation.rs, ffi_parity.rs, timing.rs, us2_cross_check.rs   # extended

scripts/ab-timing.sh (optional head-test argument; default baseline c9f9994), .github/workflows/ab-timing.yml (the bypassed
and the output_cost_mic steps against c9f9994), .github/workflows/ci.yml (golden_mic ×3; --test mic_matrix in the release
--ignored step),
scripts/ios-device-golden.sh, .github/ISSUE_TEMPLATE/hardware_evidence.yml (seven captures)
docs/hardware/assumptions.md (A-046 – A-053 new; A-005, A-006, A-019, A-022, A-028, A-029, A-036 amended),
docs/hardware/signal-chain.md (rows 1–2), docs/hardware/sources.md (S-002 summary), README.md (the microphone
section; the default-engine figures labelled), CHANGELOG.md (0.12.0), CLAUDE.md, Cargo.toml (0.12.0 × 4)
```

**Structure Decision**: the existing three-crate workspace. One new core stage module plus its generated coefficients, one public documentation module, one new check module and five test files in the harness, and one new design tool. This matches 009's layout.

## Task order (for /speckit-tasks)

1. **Phase 1, Setup** (everything inert: the default stays bypassed until Phase 2).
   - The version bump: `Cargo.toml` × 4, `RR_DR60_VERSION_MINOR = 12`.
   - **The design script**, in this order:
     1. the nodal model and the joint corner fit, compared with research R-01 before anything else;
     2. the DC consistency check and `--spice` run with ngspice (a disagreement above 10 % stops the work);
     3. the per-rate constrained fit;
     4. the clipper figures;
     5. the hiss anchor and σ per rate;
     6. the ziggurat tables and keying constants;
     7. the margins, bound, settling and harmonics;
     8. `--check` and `--write`, producing `mic_coeffs.rs`.
   - The register rows A-046 – A-053 as skeletons with the script's figures.
   - Stages 1–2's manifest entries, after the wording review and the denylist run on the new strings; **one re-bless** of the canonical text and the README table.
   - Settings, taps and validation:
     - `MicSettings`, the taps, `Setting::MicHissOffsetDb` and validation;
     - **`MicSettings::DEVICE` defined but `Settings::new` still using `BYPASSED`**.
   - **The C fields** (inert):
     - 112 → 120, the assertions, `to_settings` and `rr_dr60_settings_default`;
     - `RrDr60SettingField::MicHissOffsetDb`, taps 9/10;
     - the header regenerated;
     - `smoke.c`'s earlier cases set both switches off.
   - `configs::settings_011` and the harness rebase.
   - **Checkpoint:** the full suite, the C loop and `ffi_parity` green; nothing audible has changed.
2. **Phase 2, US1** (band and hiss).
   - **Red first:** `mic_checks` (the prototype by impulse, the hiss level, shape and offset, streams and cross-correlation), `us1_mic` and the core's statistics tests, all run red.
   - **The core:**
     - `stages/mic.rs` (sections, generator, keying, retry cap);
     - the front in `paths.rs`;
     - the counter in `transport.rs` with R-05's table tests;
     - **the default flipped on** (`Settings::new` uses `DEVICE`).
   - **At the flip:**
     - the `test_util::settings_011` audit of every `Settings::new` in the core, the FFI and the doctests (R-06);
     - **the seven SHA guards, the C loop, `ffi_parity` and the full suite green** (the bit-identity proof);
     - US3 AS1–AS3, `determinism` and `alloc_free` (the hiss is on by default from here).
   - **Checkpoint:** SC-001, SC-002, SC-004, SC-005, SC-008.
3. **Phase 3, US2** (clipping).
   - **Red first:** `us2_mic` and the US2 rows of `mic_checks` against a linear stand-in (the hook returns its input), so the onset, limit, bound and harmonic rows fail.
   - **The clipper:** its select form, the probe and the clipper-off twin; the phase-compensated stimuli; the bound on adversarial inputs; the harmonics against the discrete model; the settling.
   - **Checkpoint:** SC-003.
4. **Phase 4a, US3** (the whole engine).
   - **Red first:** the voice-activation margins at `AfterVas` (60 s; the offset bisection), the limiter ratios, the decoded hiss.
   - **Then:**
     - `mutation` (five mutants);
     - `timing::mic_cost` (asserted ≤ 10 %; the CI ceiling 25 %) and `output_cost_mic` (the limiter-state assertion);
     - `scripts/ab-timing.sh`'s head-test option and default baseline `c9f9994`, and `ab-timing.yml`'s two steps;
     - the two A/Bs on the development machine, with their figures recorded against the budgets.
   - **Checkpoint:** SC-006, SC-010.
5. **Phase 4b, US4** (records).
   - **README:**
     - the microphone section, including the per-session seed;
     - the default-engine figures found by grep, restated or labelled "0.11 default".
   - **Rustdoc:** including `silence_fed_samples` and the seed.
   - **Registers:** filled (A-022 re-derived; A-029's level chain closed).
   - **Other records:** signal-chain, sources, the issue template, the CHANGELOG.
   - **Checks:** `us2_cross_check`'s statements, the audit.
   - **Checkpoint:** SC-007.
6. **Phase 5, C.**
   - **Red first:** the new `smoke.c` cases with placeholder hashes.
   - **Then:** the hashes in `smoke_reference.rs` and the new `ffi_parity` cases.
   - **Checkpoint:** the C loop, SC-009.
7. **Phase 6, Records and gates** (spec 010's gate order).
   1. **The gate without `golden_mic`:** `cargo test --all-features -- --skip golden_mic`, fmt, clippy, the MSRV test build with `-D warnings`, the header drift check, `--check` and the byte-identical `--write`.
   2. **The ngspice gate**, recorded.
   3. **The solo timing runs**, quoted.
   4. **The post-implementation reviews:** the hardware review and the coach review, both on the session's model. One review-driven manifest re-bless is allowed.
   5. **The golden bless, once, after the reviews**, its SHA pinned, in the same commit as `ci.yml` and `scripts/ios-device-golden.sh`.
   6. **The full gate with coverage.**
   7. **Finish:** CLAUDE.md; the PR (ask first); the `ab-timing` label; ask before the merge; then `/speckit-converge`.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| A ziggurat with generated tables (about 260 table entries) instead of a short formula | FR-003 requires Gaussian tails beyond ±5σ, deterministic and within the cost budget. The voice-activation margin depends on the tails (D10). | Box–Muller through `detmath` costs about 15–25 ns per sample, over the budget alone. Irwin–Hall(12) has light tails that fail the 4σ test and flatter the margin (R-04). |
| A recording counter in the split engine that never rewinds | The user chose a distinct deterministic stream per recording (spec D9) and a counter that survives `reset` and `reconfigure` (plan review). The first recording after any of them must still equal the one-pass. | A shared stream makes every recording's hiss identical; a rewinding counter repeats hiss among recordings kept in memory; entropy from the OS breaks determinism (constitution IV). |
