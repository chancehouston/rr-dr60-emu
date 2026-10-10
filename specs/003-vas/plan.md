# Implementation Plan: Voice Activated System (VAS) on the Record Path

**Branch**: `003-vas` | **Date**: 2026-10-09 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/003-vas/spec.md`

## Summary

This feature adds signal-chain stage 5, the VAS, to the pipeline. It is on by default in drop mode, matching the device (S-001, A-008), and it can be bypassed, tapped ("after VAS"), switched to mute mode, and tuned.

Technical approach:

- **Placement**: VAS runs at the 8 kHz device rate, after stage 4 and before stage 10. The AGC keeps running while VAS is paused (R-01).
- **Detector**: a sample-peak comparator against a sine-level threshold. A 32-sample bridge lets a sound "continue" through a waveform's troughs (R-02).
- **Threshold**: −18 dBFS at sensitivity 3, with 3 dB per level. Planning revised the spec's first-draft −24 dBFS with 6 dB steps, which never paused on AGC-raised quiet background and made level 1 record nothing (R-03).
- **State machine**: hang and onset counters in device samples. It starts in the recording state (R-04).
- **Variable-length output**: the interpolator is fed only kept samples and emits at most one host sample per input step, when its data is available.
  - With nothing dropped, this is **identical** to today's one-in-one-out schedule, so 001 and 002 output is unchanged.
  - With drops, the output stalls and catches up. It is partition-independent and allocation-free. A prototype verified it at all five resampled rates (R-05).
- **Events**: splice positions and removed lengths come from stream counters, using a cumulative-floor rule so integer lengths add up at 44.1 and 88.2 kHz (R-06).
- **Mute mode**: the same decisions, with paused samples replaced by +0.0 (R-09).
- **API**:
  - **Rust**: `process` and `process_in_place` return a `#[must_use] BlockInfo`, which keeps existing calls compiling. New: `process_with_events` with a caller-provided event slice, and `max_events` (R-07).
  - **C**: `rr_dr60_process` gains an `out_info` parameter, a deliberate compile break. New: `rr_dr60_process_with_events` and `rr_dr60_max_events`. The settings struct is now 72 bytes (R-08).
  - **Version**: 0.3.0.
- **Specs 001/002 intact**: all their configurations bypass VAS, and both golden files are guarded by SHA-256. A new `golden-vas-v1.json` stores the events (R-10).
- **Measurement**: by output length and event positions, not envelopes (R-11).
- **CI budget**: ≤ 60 s added to the debug suite, ≤ 60 s in the release-mode matrix (R-12).

## Technical Context

**Language/Version**: Rust, edition 2024. MSRV 1.85 (001 R-01).

**Primary Dependencies**: No new ones.
- Core: `rr_dr60_detmath`, for `exp` at construction only.
- Harness: `rustfft`, `sha2`, `serde`, `serde_json`, as before.

**Storage**: N/A. One new golden file, about 25 KB.

**Testing**:
- **Core**: VAS unit tests (state machine, detector, rounding, reset) and resampler tests for the emission schedule (no drops = identity schedule; drops = bounded; partition independence).
- **Harness**: integration tests `us1_vas`, `us2_vas_settings`, `vas_matrix`, `vas_edge_cases` and `golden_vas`, plus additions to `ffi_parity`, `alloc_free`, `determinism`, `mutation`, `timing` and `golden_agc` (the 002 file guard).
- **C**: the smoke test is extended.
- **Coverage**: the gate stays at ≥ 80 %.

**Target Platform**: Unchanged: 8 targets (001 FR-014).

**Project Type**: Library: a Rust crate plus a C ABI.

**Performance Goals**: ≥ 20× real time at 48 kHz with AGC and VAS on (SC-006). VAS adds about 10 operations per device sample, and the emission rule adds one compare per host sample (R-13).

**Constraints**:
- Zero added latency (FR-011).
- No allocation, locks or I/O while processing, including event reporting (FR-014).
- Bit-identical on 8 targets and for every block partition, including events.
- `golden-v1.json` and `golden-agc-v1.json` byte-identical.
- No `unsafe` outside `rr_dr60_ffi`.
- CI time budget per R-12.

**Scale/Scope**: About 0.7–1k lines of core and FFI code, plus about 1.5k lines of harness and tests.
- **Debug run**: defaults × 6 rates, plus every sensitivity level and the minimum and maximum of every other setting at 8 and 48 kHz.
- **Release-mode `--ignored` run**: the full matrix × 6 rates.
- **Golden file**: 54 new entries.

There are no open NEEDS CLARIFICATION items. Planning made two spec corrections, recorded in R-03 and R-06:
- A-022's values: −18 dBFS with 3 dB steps.
- FR-005: removed lengths add up within ±1 host sample (exactly at 8 kHz) for a stream that ends while recording, and each block reports whether it ends paused.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | How this plan complies | Pre-research | Post-design |
|---|---|---|---|
| I. Portable Library First | VAS lives in the `no_std` core with no new dependencies. The C API grows by appending fields (rule 5 unchanged) and adds two functions and three types, all panic-guarded. The header is regenerated. | ✅ | ✅ |
| II. Evidence-Based Fidelity | Behavior traces to S-001 (pause, no switch, sensitivity) and A-008, A-021 – A-025. The A-022 revision is recorded with its evidence (R-03). Detector and harness constants are labeled engineering targets (R-15). Mute mode is labeled an emulator option, not device behavior. The CHANGELOG records the new default output. | ✅ | ✅ |
| III. Measured, Test-First | Each harness check maps to a spec FR (R-11). Tests are written first and fail first. 001 and 002 golden files are unchanged and SHA-guarded, and a new VAS golden file includes events. The fmt, clippy, test and coverage gates are unchanged. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | Integer state machines only, with detmath used at construction. The emission rule and event positions come from stream counters, so they are partition-independent (R-05, R-06). Events go into a caller-provided slice, so nothing allocates (R-07). Allocation, op-count and determinism tests are extended. | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | Stage 5 is bypassable (no arithmetic), tap-able (`AfterVas`) and configurable (6 settings). The variable-length output is justified below. | ✅ | ✅ |
| VI. Neutral, Honest Communication | The docs describe dropped pauses, lost onsets and abrupt splices as "modeled on the owner's manual and assumed values", with no EVP claims either way (R-14). | ✅ | ✅ |
| VII. Independently Testable Features | Every VAS check uses the VAS-isolated configuration (AGC, stage 4 and stage 10 bypassed). The noise stimulus is the harness's own FIR. US1 carries its own minimal checks. The 001 and 002 checks run with VAS bypassed (FR-020). | ✅ | ✅ |
| Repo standards | rustdoc on all new public items. SemVer 0.3.0. Conventional Commits. No audio committed. | ✅ | ✅ |

**Gate result**: PASS. Two complexity items are recorded below, as Principle V requires.

## Project Structure

### Documentation (this feature)

```text
specs/003-vas/
├── spec.md
├── plan.md              # this file
├── research.md          # R-01 – R-15
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── rust-api.md      # delta vs 002
│   ├── c-api.md         # delta vs 002
│   └── golden-format.md # golden-vas-v1.json
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks (not created by /speckit-plan)
```

### Source Code (repository root)

```text
crates/rr_dr60/src/
├── settings.rs               # + VasSettings, VasMode, Tap::AfterVas, Settings.vas
├── error.rs                  # + Setting::Vas* variants
├── validate.rs               # + VAS range checks after the AGC fields
├── pipeline.rs               # BlockInfo, VasEvent, process/process_in_place return BlockInfo,
│                             #   process_with_events, max_events; chain gating; event bookkeeping
├── lib.rs                    # re-exports; crate docs list stage 5
├── resample/up.rs            # emission schedule: kept/emitted counters, try_next_host (R-05)
└── stages/
    └── vas.rs                # NEW: detector, state machine, reset (R-02 – R-04, R-09)

crates/rr_dr60_ffi/
├── src/lib.rs                # + VAS fields (72 bytes), RrDr60VasMode, TAP_AFTER_VAS, field values,
│                             #   RrDr60BlockInfo, RrDr60VasEvent, rr_dr60_process(out_info),
│                             #   rr_dr60_process_with_events, rr_dr60_max_events (R-08)
├── include/rr_dr60.h         # regenerated
└── tests/c/smoke.c           # + drop/mute/events/invalid sensitivity

crates/rr_dr60_harness/
├── src/
│   ├── configs.rs            # 001/002 configs → VAS bypassed; + vas_only, vas_mute, default_vas
│   ├── stimulus.rs           # + burst_gap, short_bursts
│   ├── vas_checks.rs         # NEW: one check per 003 FR (R-11)
│   └── golden.rs             # + optional `vas` field, VAS set, RR_DR60_BLESS=vas
├── golden/
│   ├── golden-v1.json        # UNCHANGED
│   ├── golden-agc-v1.json    # UNCHANGED
│   └── golden-vas-v1.json    # NEW
└── tests/
    ├── us1_vas.rs            # NEW
    ├── us2_vas_settings.rs   # NEW
    ├── vas_matrix.rs         # NEW (+ interplay report)
    ├── vas_edge_cases.rs     # NEW
    ├── golden_vas.rs         # NEW
    ├── golden_agc.rs         # + golden_agc_v1_unchanged (SHA-256)
    └── (001/002 tests)       # switched to VAS-bypassed settings; expectations unchanged;
                              #   callers of process() read BlockInfo where needed

.github/workflows/ci.yml                 # + golden_vas in check, golden-matrix, ios; vas tests in --ignored step
scripts/ios-device-golden.sh             # + golden_vas
docs/hardware/signal-chain.md            # row 5 → spec 003
docs/hardware/assumptions.md             # A-022 revised (done in planning)
CHANGELOG.md, README.md, CLAUDE.md       # R-14
```

**Structure Decision**: Keep the four-crate workspace. VAS is one new stage module next to `agc.rs`. The interpolator gains the emission schedule. The harness grows by one checks module and new test files. No new crates.

## Complexity Tracking

| Added complexity | Why needed | Simpler alternative rejected because |
|---|---|---|
| Variable-length output: an emission schedule in the interpolator and `BlockInfo` on every processing call (R-05, R-07) | The device removes paused audio (S-001, A-008), and the spec makes drop mode the default (FR-002, FR-004). | Fixed length only (mute) removes the defining artifact. Dropping after the interpolator still needs position mapping and leaves splices un-band-limited. Emitting every available sample can make a block's output exceed its input. |
| Event reporting through a caller-provided slice plus `max_events` (R-07, R-08) | FR-005 requires splice positions and removed lengths per block, with no allocation (FR-014). | An internal queue needs a fixed capacity that long offline blocks can overflow, and it adds hidden state. Iterators allocate or hold a borrow. |
