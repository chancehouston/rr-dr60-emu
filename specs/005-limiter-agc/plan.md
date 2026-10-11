# Implementation Plan: Limiter-Type AGC per the Schematic, ADC Full-Scale Clamp, VAS Threshold Re-anchored

**Branch**: `005-limiter-agc` | **Date**: 2026-10-11 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/005-limiter-agc/spec.md`

## Summary

This feature replaces the spec 002 compressor with the limiter the service manual's schematic shows (A-028 adopted; A-017, A-018 superseded), adds the codec's full-scale saturation as its own bypassable stage (A-030), and re-anchors the VAS threshold to −40 dBFS at level 3 (A-022 revised). The limiter is modeled as the circuit's control loop in the control-voltage domain, run at the host rate before the rate conversion; every figure comes from a committed reference model of the schematic's component values (`tools/agc-model/limiter_model.py`), with low confidence until captures exist. The AGC and VAS golden files are re-blessed once; the spec 001 golden file does not change (verified at plan time: the record stage cannot push any 001 stimulus above full scale).

Technical approach:

- **Placement** (R-01): the AGC leaves `DeviceChain` and runs on the host-rate input before the decimator. The "after AGC" tap returns the limiter's output directly with latency 0; every other tap and the default output keep spec 004's latency. The identity-rate and converting paths differ only in whether the decimator follows.
- **Model** (R-02, R-03): three state-free parameters derived from the settings at construction (detector threshold, charge gain, discharge scale) and one state variable, the hold-capacitor voltage. Per host sample: the shunt's gain from the capacitor voltage through the exponential base–emitter law; the output; the half-wave detector on the output's sensed polarity against the threshold, with the codec amp's rail limiting what it sees; the charge (current-gain limited, then collector-resistor limited) and the discharge (10 MΩ plus the follower's base current); and an analytic cap that stops the charge at the voltage where the current sample would no longer exceed the threshold, which is what the continuous loop does and what keeps a 6 kHz host from over-attenuating. Gain exactly 1.0 and no multiply while the capacitor is below the shunt's turn-on floor, so sub-knee input passes bit-exact; the capacitor flushes to 0.0 below a floor so reset, partition and long-silence states are exact.
- **Numbers** (R-04): the reference model prints the static curve (ceiling at the threshold, −1 dB point at +1.98 dBFS), the dead time (2.4 ms at 10 dB over the knee for a detector current gain of 300; 1.4–4.4 ms over 150–600; 50 ms at 2 dB over; 0.01 ms when armed), the release (1.37 s to −1 dB from 10 dB of attenuation, 1.69 s from 17 dB; −6.2 dB at 0.3 s and −2.9 dB at 0.75 s) and the arming decay (1.1 → 0.5 V in 7.9 s). The settings map onto the model: knee → detector threshold; attack dead time → detector current gain (inverse); release → discharge scale.
- **ADC clamp** (R-06): a one-line stage after the record stage, its own `adc_clamp_enabled` setting (default on), bypassed by `bypass_all`; verified not to touch the 001 golden stimuli.
- **VAS** (R-07): the default threshold constant becomes −40 dBFS; the harness's burst level follows (−30 dBFS); the interplay report and the input-referred statement are rewritten; three chain-level scenarios are added.
- **Measurement** (R-08): the harness measures the exact per-sample gain at the host-rate tap at every rate (the reference-ratio method and the Hilbert detector are retired): static curve and knee bisection, dead time and arming, release trajectory, polarity with the asymmetric stimulus, bandwidth with a 6 kHz tone at 48 kHz, the clamp, THD+N reports, the settings matrix, and the chain scenarios; mutation fixtures for the knee, the release and the VAS threshold.
- **Interface** (R-09): `AgcSettings { enabled, knee_dbfs, attack_ms, release_ms }` with new meanings and defaults, `Settings::adc_clamp_enabled`, `Setting::AgcKneeDbfs`, `Setting::AdcClamp`-free (a bool needs no validation); the C struct keeps its offsets, renames the target slot to the knee, marks two slots reserved, appends the clamp enable (72 → 80 bytes), keeps the field-enum numbers stable; version 0.5.0.
- **Golden and records** (R-10, R-11): three new AGC stimuli (onset, re-arm, staircase), the VAS stimuli at the new burst level, 42 + 63 entries re-blessed in one commit with the CHANGELOG entry; register rows A-007, A-017, A-018, A-019, A-022, A-028, A-029 updated and A-034 – A-036 added; spec 002 and 003 header notes; README, signal-chain, tools README, CLAUDE.md, issue template.

## Technical Context

**Language/Version**: Rust, edition 2024, MSRV 1.85 (001 R-01). Python 3.10+ (standard library only) for the reference model.

**Primary Dependencies**: No new ones. Core: `rr_dr60_detmath` (`exp` and `ln` on the processing path, as spec 002 already allowed). Harness: unchanged. The Hilbert design script's scipy dependency goes away with the script.

**Storage**: N/A. Two golden files regenerated (42 and 63 entries); `golden-v1.json` byte-identical.

**Testing**:
- **Core**: limiter unit tests at 6 kHz and 48 kHz (bit-exact passthrough below the knee, ceiling, dead time from a discharged and from an armed state, release points, polarity, clamp, settings mapping, reset and partition exactness, flush-to-zero); pipeline tests for the tap, latency and stage order; settings validation; FFI tests for the renamed and appended fields and the stable enum numbers.
- **Harness**: `agc_checks.rs` rewritten (R-08); `vas_checks.rs` threshold and chain scenarios; `golden.rs` stimuli; `configs.rs` clamp; new mutation fixtures; matrices at seven rates; C smoke test additions.
- **Reference model**: `tools/agc-model/limiter_model.py` prints the design figures; a core unit test pins the Rust model's dead time, release and ceiling to those figures within the spec tolerances so a drift in either is caught.
- **Coverage**: gate stays at 80 %.

**Target Platform**: Unchanged: 8 targets (001 FR-014).

**Project Type**: Library: a Rust crate plus a C ABI.

**Performance Goals**: ≥ 20× real time at 48 kHz (SC-005). The limiter costs about one `exp`, one conditional `ln` (the charge cap) and 20 flops per host sample; the Hilbert FIR (32 MACs per device sample) goes away (R-12).

**Constraints**:
- Latency unchanged at every tap but "after AGC" (0); default ≤ 20 ms (SC-006).
- Bit-identical on 8 targets and across block partitions (FR-014): `exp`/`ln` from detmath only; the capacitor state flushes to 0.0 below a floor.
- No allocation, locks or I/O while processing; no `unsafe` outside `rr_dr60_ffi`.
- The AGC and VAS golden files change exactly once, in one commit (FR-016); `golden-v1.json` never.
- The spec's figures carry ±1.5 ms / ±40 % tolerances; the Rust model must land inside them at every host rate, including the coarse 6 kHz grid (R-03).

**Scale/Scope**: About 500 lines of core changes (new `limiter.rs`, pipeline tap and stage order, settings, clamp, tests), about 150 lines of FFI changes, about 900 lines of harness changes (AGC checks rewritten, VAS chain scenarios, stimuli, configs, mutation, golden), the reference model script (about 150 lines), docs, register, CHANGELOG, two regenerated golden files, removal of the Hilbert coefficient file, its tests and its design script.

There are no open NEEDS CLARIFICATION items. The pre-specification review fixed five decisions and the clarify session four more (spec Clarifications). One figure in the spec is corrected by this plan and recorded in the spec's plan-review session: the polarity test's expected gain difference is 3 dB, not 2.5 dB (R-08).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | How this plan complies | Pre-research | Post-design |
|---|---|---|---|
| I. Portable Library First | No new dependencies; the core stays `no_std` + `alloc`. The C struct grows at the end, keeps every existing offset, keeps the field-enum numbers; the header is regenerated; the smoke test covers the new fields (R-09). | ✅ | ✅ |
| II. Evidence-Based Fidelity | Every component value traces to S-002; every transistor estimate to A-028/A-034/A-035/A-036 with its confidence; the level scale to A-030/A-029; the threshold to A-022 (revised). The register, CHANGELOG, README and signal-chain record the change; the spec 002 model is retired explicitly, not silently (R-11). The reference model is committed so the numbers are reproducible. | ✅ | ✅ |
| III. Measured, Test-First Verification | Each FR-003 to FR-012 check maps to a harness function measuring the exact gain at the host-rate tap (R-08); core unit tests pin the model to the reference script's figures; golden files re-blessed once, on purpose, with guards and a CHANGELOG entry; three mutations caught (SC-007). | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | Per-sample math uses detmath `exp`/`ln` and basic ops; no allocation; the state is one `f64` plus the derived parameters; flush-to-zero gives exact state equality (R-03). | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | The limiter and the clamp are separate stages, each bypassable, the limiter tap-able; the clamp's enable is a setting (Clarifications). Added complexity is recorded below. | ✅ | ✅ |
| VI. Neutral, Honest Communication | README, CHANGELOG and register say "modeled on the circuit, not measured against a unit", give the input-level anchor, list what is not modeled (R-14), and describe the clipped onset as a model property with low confidence. | ✅ | ✅ |
| VII. Independently Testable Features | US1 is checked at the host-rate tap alone; US2 by output lengths and events through the default pipeline; US3 by the settings matrix and C parity. Spec 001, 003 and 004 checks keep their isolated configurations. | ✅ | ✅ |
| Repo standards | rustdoc on changed public items; SemVer 0.5.0 (breaking); Conventional Commits; no audio committed. | ✅ | ✅ |

**Gate result**: PASS. Three complexity items are recorded below, as Principle V requires.

## Project Structure

### Documentation (this feature)

```text
specs/005-limiter-agc/
├── plan.md              # This file
├── research.md          # Phase 0: decisions R-01 – R-15
├── data-model.md        # Phase 1: model parameters and state, settings, stimuli, changed figures
├── quickstart.md        # Phase 1: validation walk-through
├── contracts/
│   ├── rust-api.md      # AgcSettings, Settings::adc_clamp_enabled, Setting, Tap::AfterAgc, test hooks
│   ├── c-api.md         # RrDr60Settings 72 → 80 bytes, renamed/reserved fields, enum numbers, smoke test
│   └── golden-format.md # the re-blessed AGC and VAS files
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/rr_dr60/src/
├── settings.rs                 # AgcSettings {enabled, knee_dbfs, attack_ms, release_ms}; Settings::adc_clamp_enabled;
│                               # Setting::AgcKneeDbfs; validation ranges; docs
├── pipeline.rs                 # limiter at the host rate before the decimator; AfterAgc tap = limiter output, latency 0;
│                               # DeviceChain: record → clamp → VAS → playback; op-count test
├── lib.rs                      # crate docs (limiter, clamp, level scale)
└── stages/
    ├── mod.rs                  # `pub(crate) mod limiter; pub(crate) mod clamp;` (agc + agc_hilbert_coeffs removed)
    ├── limiter.rs              # NEW: the control-voltage model (R-02, R-03), parameters from settings, tests
    ├── clamp.rs                # NEW: ±1.0 saturation stage (R-06), tests
    ├── agc.rs                  # REMOVED (spec 002 compressor)
    ├── agc_hilbert_coeffs.rs   # REMOVED
    └── vas.rs                  # unchanged logic; doc comment on the threshold scale

crates/rr_dr60_ffi/
├── src/lib.rs                  # RrDr60Settings: agc_knee_dbfs (was agc_target_dbfs), reserved_agc_1/2, agc_attack_ms,
│                               # agc_release_ms (new meanings), adc_clamp_enabled appended; 80 bytes; field enum numbers kept
├── include/rr_dr60.h           # regenerated
└── tests/c/smoke.c             # limiter knee check, clamp check, reserved-field tolerance, 80-byte struct_size

crates/rr_dr60_harness/
├── src/agc_checks.rs           # REWRITTEN: static curve, knee bisection, dead time, arming, release, polarity,
│                               # bandwidth, clamp, THD+N report, settings matrix (R-08)
├── src/vas_checks.rs           # BURST_ABOVE_DB unchanged (bursts now −30 dBFS); input-referred statement and
│                               # interplay report rewritten; chain scenarios (R-07)
├── src/stimulus.rs             # asymmetric stimulus; speech-like burst pattern over a floor; staircase
├── src/configs.rs              # adc clamp in the configurations; `agc_only` = AfterAgc tap; `clamp_off` variants
├── src/golden.rs               # agc_onset, agc_rearm, agc_staircase; VAS bursts at −30 dBFS; docs
├── golden/golden-agc-v1.json   # re-blessed (42 entries)
├── golden/golden-vas-v1.json   # re-blessed (63 entries)
├── tests/us1_agc.rs            # REWRITTEN (US1 scenarios)
├── tests/us2_agc_settings.rs   # REWRITTEN (US3 scenarios)
├── tests/agc_edge_cases.rs     # REWRITTEN (edge cases)
├── tests/agc_matrix.rs         # defaults at 7 rates; settings matrix at 6 and 48 kHz; release-mode full matrix
├── tests/chain.rs              # NEW: the FR-012 chain scenarios (US2)
├── tests/mutation.rs           # knee +3 dB, release ×2, VAS threshold +3 dB
├── tests/golden_agc.rs, golden_vas.rs, determinism.rs, alloc_free.rs, ffi_parity.rs  # updated
└── tests/vas_*.rs              # threshold −40 dBFS; bisections unchanged in form

tools/
├── agc-model/limiter_model.py  # NEW: the reference model (R-04)
└── filter-design/design_hilbert.py  # REMOVED; README updated

docs/hardware/assumptions.md, signal-chain.md; CHANGELOG.md; README.md; CLAUDE.md;
.github/ISSUE_TEMPLATE/hardware_evidence.yml; specs/002-agc/spec.md and specs/003-vas/spec.md (header notes)
```

**Structure Decision**: the existing three-crate workspace. The limiter is a new stage module at the host-rate side of the pipeline; the clamp a new device-rate stage; the spec 002 stage and its coefficient file are deleted rather than kept behind a feature flag (nothing is released, and keeping two AGC models would double the matrix for no user).

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| A control-loop model with four circuit constants and an analytic charge cap instead of a two-coefficient smoother | The spec's dead time, arming, flat ceiling and fast-then-slow release all come from one capacitor state and the exponential shunt law; a dB-domain smoother cannot produce a dead time or arming (reviewer D2) | Re-parameterising the spec 002 smoother: no dead zone, no arming, exponential-in-dB release (wrong shape); four tuned constants instead of four traced ones |
| The AGC runs at the host rate, outside `DeviceChain`, with its own tap path | The real detector is pre-filter; a 6 kHz loop over-attenuates; the exact gain becomes observable at every rate (spec Clarifications) | Keeping it at the device rate: a documented deviation on bandwidth, a reference-ratio measurement that cannot resolve a 1 ms ramp through a 10 ms kernel, and the 6 kHz discretization hazard |
| The C struct renames one slot and reserves two instead of appending four new fields | Keeps every existing offset and the field-enum numbers, so a 0.4 caller fails to compile (renamed field) and fails at runtime (struct_size 72 ≠ 80) rather than silently misreading; no dead 12 bytes carried forever | Appending `agc_knee_dbfs` and leaving `agc_target_dbfs` in place: two "level" fields of which one is ignored, confusing in the header and in docs |
