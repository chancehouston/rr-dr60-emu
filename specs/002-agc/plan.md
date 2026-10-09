# Implementation Plan: Automatic Gain Control (AGC) on the Record Path

**Branch**: `002-agc` | **Date**: 2026-10-09 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/002-agc/spec.md`

## Summary

This feature adds signal-chain stage 3, the record-path AGC, to the 001 pipeline. It is on by default (A-020), and it can be bypassed, tapped ("after AGC") and tuned.

Technical approach:

- **Placement**: The AGC runs at the 8 kHz device rate, as the first element of the existing device chain, ahead of stage 4 (R-01).
- **Level detector**: The larger of (a) the analytic envelope from a committed 63-tap Hilbert FIR and (b) a 32-sample peak hold. This gives exact sine levels at every frequency and phase in the band, while still reacting immediately to onsets (R-02, R-03).
- **Gain computer**: A 10:1 static curve with hard limits (R-04). A one-pole smoother on the gain in dB, which gives the spec's exponential-in-dB attack and release. It starts at maximum gain (R-05).
- **Determinism**: The gain needs `ln` and `exp`, which come from the existing bit-identical `rr_dr60_detmath` crate (R-06).
- **Prototype result**: A float64 Python prototype of exactly this design met every spec tolerance with wide margins: static curve within 0.01 dB, attack 10.00 ms, release 1.001 s, release midpoint 47 %, and THD below 0.001 % at every frequency and phase tested.
- **Spec 001 stays intact**: Its checks and `golden-v1.json` run with the AGC bypassed and do not change. A new `golden-agc-v1.json` covers the AGC (R-10, R-12).
- **API**: The Rust API gains `AgcSettings`, `Tap::AfterAgc` and `Error::InvalidSetting`. The C struct grows at the end. The `struct_size ≥ sizeof` rule is unchanged, and a new `rr_dr60_settings_validate` names any bad field (R-08, R-09). Version 0.2.0.
- **Plan review (2026-10-09)** changed:
  - a gain flush, so the gain can never go subnormal (R-07);
  - the C struct-size rule, kept as in 0.1 (R-09);
  - distortion measured as THD+N, which catches folded harmonics (R-11);
  - named checks for every edge case (R-11);
  - CI wiring for the new golden file (R-12);
  - engineering-target labels for detector constants (R-15).

## Technical Context

**Language/Version**: Rust, edition 2024. MSRV 1.85 (001 R-01). The offline Hilbert design script uses Python 3 via `uv` with numpy and scipy, alongside `design_voiceband.py`.

**Primary Dependencies**: No new ones.
- Core: `rr_dr60_detmath`, which is now also used on the processing path for `ln`/`exp` (R-06).
- Harness: `rustfft`, `sha2`, `serde`, `serde_json`.

**Storage**: N/A. One new golden file, about 15 KB.

**Testing**:
- `cargo test`: new AGC unit tests in the core (static curve, coefficients, Hilbert response, reset) and harness integration tests `agc_matrix`, `golden_agc`, plus additions to `ffi_parity`, `alloc_free`, `determinism`, `mutation` and `edge_cases`.
- The C smoke test is extended with the AGC fields.
- `cargo llvm-cov` keeps the coverage gate at ≥ 80 %.

**Target Platform**: Unchanged: 8 targets (001 FR-014).

**Project Type**: Library: a Rust crate plus a C ABI.

**Performance Goals**: ≥ 20× real time at 48 kHz (SC-006). The AGC adds about 150 operations per *device* sample, about 1.2 M operations per second, compared with about 10 M multiply-adds per second already spent at 48 kHz (R-13).

**Constraints**:
- Zero added latency (FR-010).
- No allocation, locks or I/O in `process`.
- Bounded work per sample.
- Bit-identical on 8 targets and every block partition.
- `golden-v1.json` byte-identical.
- No `unsafe` outside `rr_dr60_ffi`.

**Scale/Scope**: About 0.6–1k lines of core and FFI code, plus about 1.5k lines of harness and tests. Measurement matrix:
- in the debug test run: defaults × 6 rates, and the US2 examples plus the minimum and maximum of each setting × 2 rates (8 and 48 kHz);
- in the release-mode `--ignored` job: the full set × 6 rates (R-11).

36 new golden entries.

There are no open NEEDS CLARIFICATION items. All were resolved in [research.md](research.md).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | How this plan complies | Pre-research | Post-design |
|---|---|---|---|
| I. Portable Library First | The AGC lives in the `no_std` core with no new dependencies. The C API grows by appending fields (rule 5 unchanged), adds one status, one enum and one `validate` function, all panic-guarded, and has a regenerated header. The design script stays in `tools/`. | ✅ | ✅ |
| II. Evidence-Based Fidelity | Every default cites A-017 to A-020, which are already registered. Detector and measurement constants are labeled engineering targets (R-15), serving A-019. Engineering targets are labeled as in spec FR-015. The CHANGELOG records the changed default output and the assumptions (R-14). | ✅ | ✅ |
| III. Measured, Test-First | Each harness check maps to one spec FR (R-11). Tests are written before the AGC code and fail first. Golden files are covered: v1 unchanged, plus a new AGC file. The fmt, clippy, test and coverage gates are unchanged. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | detmath `ln`/`exp` are bit-identical by construction (R-06). The platform libm stays banned. The gain is flushed so it can never go subnormal (R-07). State is fixed-size and inline. A per-sample state machine. Allocation and op-count tests are extended to AGC-on (R-13). | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | Stage 3 is bypassable (no arithmetic), tap-able (`AfterAgc`) and configurable (5 settings). The added detector complexity is justified below. | ✅ | ✅ |
| VI. Neutral, Honest Communication | Docs describe level control, pumping and noise rise as "modeled on an assumed AGC", with no EVP claims either way. They warn about the overshoot (R-14). | ✅ | ✅ |
| VII. Independently Testable Features | Every AGC check uses the AGC-isolated configuration (stages 4 and 10 bypassed). The noise stimulus uses a harness-built filter, not stage 4 (R-11). US1 carries its own minimal level and timing test. Spec 001's checks run with the AGC bypassed (FR-018). | ✅ | ✅ |
| Repo standards | rustdoc on all new public items. SemVer 0.2.0 for the breaking `Settings` derive change. Conventional Commits. No audio committed. | ✅ | ✅ |

**Gate result**: PASS. One complexity item is recorded below, as Principle V requires.

## Project Structure

### Documentation (this feature)

```text
specs/002-agc/
├── spec.md
├── plan.md              # this file
├── research.md          # R-01 – R-14
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── rust-api.md      # delta vs 001
│   ├── c-api.md         # delta vs 001
│   └── golden-format.md # golden-agc-v1.json
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks (not created by /speckit-plan)
```

### Source Code (repository root)

```text
crates/rr_dr60/src/
├── settings.rs               # + AgcSettings, Tap::AfterAgc, Settings.agc (drop Eq/Hash)
├── error.rs                  # + Error::InvalidSetting, Setting enum
├── validate.rs               # NEW: settings range checks (FR-011)
├── pipeline.rs               # DeviceChain gains the AGC stage before stage 4; tap logic
├── sanitize.rs               # narrow_out saturates to ±f32::MAX (R-07)
├── lib.rs                    # re-exports; crate docs mention stage 3
└── stages/
    ├── agc.rs                # NEW: detector, gain computer, smoother, reset (R-02 – R-05)
    └── agc_hilbert_coeffs.rs # NEW, generated: 63-tap Hilbert FIR bit patterns (R-03)

crates/rr_dr60_ffi/
├── src/lib.rs                # + AGC fields, struct_size rule, INVALID_SETTING, TAP_AFTER_AGC,
│                             #   RrDr60SettingField, rr_dr60_settings_validate (R-09)
└── include/rr_dr60.h         # regenerated

crates/rr_dr60_harness/
├── src/
│   ├── configs.rs            # 001 configs → AGC bypassed (doc comment); + agc_only,
│   │                         #   agc_isolated_after_playback, agc_tap_stages_on, default_agc
│   ├── stimulus.rs           # + level steps, tone bursts, band-limited noise (detmath FIR)
│   ├── analysis.rs           # + analytic envelope, gain trajectory, settling/midpoint, THD+N fit
│   ├── agc_checks.rs         # NEW: one check per 002 FR (R-11)
│   └── golden.rs             # generalized to multiple files; + AGC stimuli/configs
├── golden/
│   ├── golden-v1.json        # UNCHANGED
│   └── golden-agc-v1.json    # NEW
└── tests/
    ├── agc_matrix.rs         # NEW: FR-004 – FR-012 × 6 rates
    ├── golden_agc.rs         # NEW (+ golden_v1_unchanged: SHA-256 of golden-v1.json)
    ├── agc_edge_cases.rs     # NEW: start/reset, zero range + 10-min flush, DC/LF, >4 kHz, NaN, overshoot
    ├── us1_agc.rs            # NEW: US1 minimal standalone checks
    ├── us2_agc_settings.rs   # NEW: bypass/tap/settings/validation (US2)
    └── (001 tests)           # switched to AGC-bypassed settings; expectations unchanged;
                              #   ffi_parity/alloc_free tap mapping → exhaustive match

tools/filter-design/design_hilbert.py   # NEW (R-03)
.github/workflows/ci.yml                # + golden_agc, golden_v1_unchanged in check, golden-matrix, ios jobs
scripts/ios-device-golden.sh            # + golden_agc
crates/rr_dr60_detmath/src/lib.rs        # docs: now also used on the processing path (R-06)
docs/hardware/signal-chain.md           # row 3 → spec 002
CHANGELOG.md, README.md                 # R-14
```

**Structure Decision**: Keep 001's four-crate workspace. The AGC is one new stage module in the core, next to `voiceband.rs`. The harness grows by one checks module and new test files. No new crates.

## Complexity Tracking

| Added complexity | Why needed | Simpler alternative rejected because |
|---|---|---|
| Two-path detector: Hilbert analytic envelope plus a 32-sample peak hold (R-02) | FR-008 needs the sine level within ±0.5 dB and THD ≤ 1 % at any phase from 300 to 3400 Hz. A-019 needs a peak-responding detector with fast onsets. | Sample-peak only misreads 2000 Hz by up to 3 dB and 2667 Hz by up to 6 dB. An RMS detector contradicts A-019 and ripples. A Hilbert filter alone measured an 11.25 ms attack instead of 10 ms. |
| `ln`/`exp` per device sample on the processing path (R-06) | The gain is computed in dB (FR-004 to FR-006). | Lookup tables add code and accuracy questions. detmath is already bit-identical and fixed-cost. |
| C function `rr_dr60_settings_validate` and enum `RrDr60SettingField` (R-09) | US2 AS5 and FR-014: a C host must be able to name the invalid setting. | A status code alone can't name the field. A thread-local error string is hidden state and allocates. A separate query function could disagree with `create`. |
