# Implementation Plan: Volume Ramp (a smooth transition for volume position changes, stage 11)

**Branch**: `011-volume-ramp` | **Date**: 2026-10-16 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/011-volume-ramp/spec.md`

## Summary

A volume position change becomes a **20 ms transition** of the knob position (A-045), starting at the next block: the knob moves along a smoothstep from where it is to the new position, and the active route's gain at every sample is the loaded law (A-044) at the knob's position, held within the two ends, the last sample exactly 0.10's scalar. Technically it is a few fields and one predictable branch in `OutputChain` (research R-01): `set_controls` starts a transition instead of writing the scalars, and `inner()` advances it per sample; a `fresh` flag set by `new()` / `reset()` and cleared by the first processed sample makes construction, reset, reconfigure, a playback start and a route change take the position at once (R-04), so every check that sets its volume before processing, all six golden files and the C smoke hashes are unchanged. The reference script `tools/volume-ramp/ramp_model.py` prints every design figure: FR-004's click reduction is 33.9 – 34.5 dB on the speaker law and 40.6 – 41.2 dB on the earphone law (floor 30 dB; a linear position movement reads 18.7 – 19.8 dB; the real chain matches within 0.03 dB, the plan review), the drag 34.9 – 38.3 dB; a range and running hold make every transition monotonic by construction (R-02, R-03). No setting and no C struct change; a public constant `output::VOLUME_TRANSITION_MS = 20` and its C define (R-07). Version 0.11.0; stage 11's manifest entry cites A-045 at model version 0.11.0 (one re-bless); one new golden file, 140 entries (R-09).

## Technical Context

**Language/Version**: Rust, edition 2024, MSRV 1.85 (001 R-01); Python 3.11 with numpy via `uv` for the reference script (as 004, 008, 009).

**Primary Dependencies**: none new.

**Storage**: unchanged (the memory, the stored form version 2, export and import untouched: the transition is after the memory).

**Testing**:
- **Core**: unit tests in `stages/output.rs` (the gain sequence: monotonic, within the ends, exactly 0.10's scalar at N, equal to the formula under the range and running hold; a retarget within rounding of the knob monotonic, on R-03's grid at 48 kHz and pairs at the other six rates; N per rate; fresh-chain, route-change, inactive-chain and bypassed-stage rules; retarget from the current knob; a zero-length call keeps the chain fresh; position 0 exact at N; the restated `route_change_resets_sections_and_volume_change_does_not`; the `N_OUT > 2·N` const assertion), `paths.rs` / `pipeline.rs` / `transport.rs` (start-at-once after `new`, `reset`, `reconfigure`, `start_playback`).
- **Harness**: `volume_ramp_checks.rs` (R-05) and the tests `us1_volume_ramp` (US1 and US2 at `measurement_rates()`), `volume_ramp_matrix` (the seven-rate report: FR-004 table against R-02 ±0.25 dB, drag, route output), `volume_ramp_edge_cases` (every spec edge case), `golden_volume_ramp` (140 entries); the restated 009 checks (R-06); `ffi_parity`, `determinism`, `timing` (`volume_ramp_cost`), `mutation` (four mutant configurations) extended; `analysis::click_energy`; `stimulus::tone_peaking_at`.
- **Golden**: the six existing files untouched (SHA guards); `golden-volume-ramp-v1.json` blessed once at the end, after the gate without it and before the gate with coverage.
- **Coverage**: gate 80 %; within 1 point of 0.10's figure (97.96 % on CI).

**Target Platform**: unchanged: 8 targets; the golden matrix proves bit-identity on six.

**Project Type**: library: a Rust crate plus a C ABI.

**Performance Goals**: FR-009 as amended at the plan review: with every sample in a transition (480-sample blocks, alternating 0.1 / 1, noise at −20 dB where the cost is flat across positions) at most **15 %** over the step hook at the same partition, asserted in `timing::volume_ramp_cost` on both machines; idle at most **3 %**, an A/B of the 0.10 build (`bd6d827`) and the branch build with `timing::output_cost` (`scripts/ab-timing.sh`), on the development machine and once on the CI runner before the merge (`ab-timing.yml`, `workflow_dispatch`); the figure against 0.10 the product of the two (R-08). `output_cost`'s own assertion does not guard the 3 %. Expected +2 – 5 ns ramping, under 1 ns idle. Latencies and the drain unchanged at every rate.

**Constraints**:
- Bit-identity to 0.10 at every tap whenever the volume does not change after the first processed block (FR-006): the fresh rule (R-04), the six SHA guards checked in Phase 1 before and after the core change.
- Determinism: plain `f64` multiplies, adds and one divide per transition sample; `u = k · (1/N)`; no platform math function, no `f64::clamp` in the per-sample path (R-02, R-03).
- No allocation; no `unsafe` outside `rr_dr60_ffi`; no new setting, status or C function; the C struct stays 112 bytes (FR-011).
- The manifest's stage 11 changes in its assumptions, coverage note (≤ 120 characters) and model version only; one re-bless of the canonical text and the README table, with the version (Phase 1).
- Wording (R-10): "the volume transition"; never "accurate", never a protection claim; "ramp" only in identifiers and the feature name.

**Scale/Scope**: about 200 lines of core (`stages/output.rs` ≈ 160 with tests: fields, `set_controls`, `ramp_step`, the constant and `volume_transition_samples`; `lib.rs` three hook fields and docs; rustdoc on `set_volume`, `set_route`, `Tap::AfterVolumeAmp`), about 20 lines of FFI (the define, `VERSION_MINOR`, the comment, the header), about 700 lines of harness (`volume_ramp_checks.rs` ≈ 300, four test files ≈ 300, restated checks, `golden.rs` with `changes`, `configs::stage11_only_*`, `analysis::click_energy`, `stimulus::tone_peaking_at`, extensions), `smoke.c` one case and `smoke_reference.rs` one hash, `tools/volume-ramp/ramp_model.py` (written at plan time), `scripts/ab-timing.sh` and `.github/workflows/ab-timing.yml` (manual), `scripts/check-traceability.sh` (one line), docs (A-044, A-045, signal-chain row 11, README, CHANGELOG, CLAUDE.md, spec 009's two notes, the issue template), one new golden file, `ci.yml` and `scripts/ios-device-golden.sh`.

There are no open NEEDS CLARIFICATION items. The spec's two plan-time verifications are done: no golden generator changes the volume after its first block (R-06; Phase 1 re-runs the six guards), and the list of checks that change it mid-stream with their restated expectations is R-06's table.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | How this plan complies | Pre-research | Post-design |
|---|---|---|---|
| I. Portable Library First | No new dependency; the core stays `no_std` + `alloc`; no C struct change, no new function or status, one new define; the header regenerated and drift-checked; the smoke loop gains a mid-playback case (R-07, R-09). | ✅ | ✅ |
| II. Evidence-Based Fidelity | The transition is modeled stage-11 behavior traced to S-002 (the continuous wiper) and a new assumption A-045 with split confidence, rationale and a verification protocol; A-044 amended; stage 11's manifest entry cites A-045 at model version 0.11.0; CHANGELOG entries (R-10). The 20 ms and the shape are registered as an engine convention, not a measurement. | ✅ | ✅ |
| III. Measured, Test-First Verification | Every behavior is a measurable property with a tolerance: the splice identity (exact), the gain sequence (exact), the ratio reader (4 ULP), FR-004 (≥ 30 dB, ±0.25 dB of the script), the drag (≥ 30 dB), the exact zeros and the −120 dBFS bound; the checks are written and run red against 0.10's step before the core change (Phase 2's first task); the script's figures are printed, not taken from the Rust (005's lesson); mutation: four mutant configurations; the six goldens guard the earlier specs and the feature has its own file. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | Bounded per-sample work (one divide, a few multiplies, two compares), no allocation, no clock, no platform math; partition independence asserted; `alloc_free` and `determinism` extended (R-01 – R-03). | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | The transition sits inside stage 11 and inherits its bypass (`volume_amp_enabled`); `Tap::AfterVolumeAmp` exposes it; no new stage (Complexity Tracking row 1). | ✅ | ✅ |
| VI. Neutral, Honest Communication | "Modeled on" the wiper's continuity; the 20 ms called an engine convention; the not-modeled list (wiper noise, lift-off, detents, the knob's speed) in A-045 and the manifest; no protection claim (R-10). | ✅ | ✅ |
| VII. Independently Testable Features | US1 in the stage-11-only configuration with synthetic tones and constant-position runs as oracles (no other stage's behavior; the resampler pair cancels); US2 by the existing equivalence, determinism, golden and timing harnesses; US3 by the cross-check and the audit; the version bump, A-045 and the manifest re-bless sit in Phase 1 so no story depends on another. | ✅ | ✅ |
| Repo standards | rustdoc on every new public item; SemVer 0.11.0 (a behavior change of a control, minor before 1.0); Conventional Commits; no audio committed. | ✅ | ✅ |

**Gate result**: PASS. One complexity item is recorded below.

## Project Structure

### Documentation (this feature)

```text
specs/011-volume-ramp/
├── plan.md              # This file
├── research.md          # R-01 – R-11 (mechanism, shape and figures, floating point, start/end rules, oracles and measurement, the restated checks, hooks and the constant, cost, golden and C parity, records, risks)
├── data-model.md        # the control surface, OutputChain's new fields, the state machine, the per-sample step, configurations, stimuli, rows
├── quickstart.md        # the validation runs per story and the gates
├── contracts/
│   ├── rust-api.md      # VOLUME_TRANSITION_MS, volume_transition_samples, the set_volume / set_route / tap docs, the three test-hook fields
│   ├── c-api.md         # struct unchanged (112), RR_DR60_VOLUME_TRANSITION_MS, VERSION_MINOR 11, the smoke case, parity
│   └── golden-format.md # golden-volume-ramp-v1.json: 5 scenarios × 4 configurations × 7 rates = 140, `changes`, bless-time splice assertions
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks output (not created by /speckit-plan)
```

### Source Code (repository root)

```text
tools/volume-ramp/ramp_model.py   # NEW (plan time): every design figure; --check

crates/rr_dr60/src/
├── lib.rs                      # __test_hooks::OutputMutation { .., volume_step, volume_shape_linear, volume_ramp_delta }; crate docs: one sentence on the transition
├── output.rs                   # VOLUME_TRANSITION_MS, volume_transition_samples (re-exported from stages::output)
├── pipeline.rs                 # rustdoc: set_volume, volume, set_route; start-at-once tests
├── settings.rs                 # Tap::AfterVolumeAmp docs
└── stages/output.rs            # OutputChain { knob, from, to, target, ramp_k, ramp_n, inv_n, up, g_to_spk, g_to_ear, fresh, hooks }; set_controls; ramp_step; reset sets fresh; N_OUT > 2·N const assertion; unit tests

crates/rr_dr60_ffi/
├── src/lib.rs                  # RR_DR60_VOLUME_TRANSITION_MS; RR_DR60_VERSION_MINOR = 11; rr_dr60_set_volume comment
├── include/rr_dr60.h           # regenerated
└── tests/c/smoke.c             # one mid-playback change case (RR_DR60_SMOKE_SPEAKER_RAMP_HASH); the define checked

crates/rr_dr60_harness/
├── src/volume_ramp_checks.rs   # NEW (R-05): splice, ratio_reader, partitions, click_reduction, retarget, zeros, drag, route_output, whole_engine, equivalence, drain
├── src/analysis.rs             # click_energy (Hann round(0.5·fs), 20 – 2750 Hz, |f − 1 kHz| > 250 Hz)
├── src/stimulus.rs             # tone_peaking_at (a 1 kHz cosine whose peak falls on a given output sample through the path latency)
├── src/configs.rs              # stage11_only_speaker, stage11_only_earphone; c_settings unchanged
├── src/output_checks.rs        # restated: chain_volume_change_timing, the volume-0 decay (R-06)
├── src/smoke_reference.rs      # SMOKE_SPEAKER_RAMP_HASH
├── src/golden.rs               # VOLUME_RAMP_SCENARIOS, GOLDEN_VOLUME_RAMP_CONFIGS, GoldenEntry.changes, generate_volume_ramp, committed_volume_ramp, path_volume_ramp
├── golden/golden-volume-ramp-v1.json   # NEW, blessed once (140 entries)
├── golden/manifest-v1.txt      # re-blessed once in Phase 1 (version, stage 11)
├── tests/us1_volume_ramp.rs    # NEW (US1, US2)
├── tests/volume_ramp_matrix.rs # NEW: the seven-rate report
├── tests/volume_ramp_edge_cases.rs   # NEW
├── tests/golden_volume_ramp.rs # NEW
├── tests/output_edge_cases.rs  # restated: the earphone block-gain check
└── tests/ffi_parity.rs, determinism.rs (volume_ramp_partitions), timing.rs (volume_ramp_cost), mutation.rs, us2_cross_check.rs (readme_volume_statements)  # extended

scripts/ab-timing.sh (NEW), .github/workflows/ci.yml (golden_volume_ramp in three places), scripts/ios-device-golden.sh, .github/ISSUE_TEMPLATE/hardware_evidence.yml (the knob-flick capture)
docs/hardware/assumptions.md (A-045 new, A-044 amended), docs/hardware/signal-chain.md (row 11 Modeled by), README.md (the volume paragraph), CHANGELOG.md (0.11.0), CLAUDE.md, specs/009-output-chain/spec.md (FR-009 note, T043 N2 closed), Cargo.toml (0.11.0 × 4)
```

**Structure Decision**: the existing three-crate workspace; no new core module (fields and two methods in `stages/output.rs`, the constant on the public `output` module), one new check module and four test files in the harness, matching 010's layout.

## Task order (for /speckit-tasks)

1. **Phase 1, Setup**: the version bump (`Cargo.toml` × 4, `RR_DR60_VERSION_MINOR = 11`, the header); **A-045's row** and A-044's amendment in the register; stage 11's manifest entry (A-045, the coverage note, model version 0.11.0) with **the wording review and the denylist run on the changed strings first**; the CHANGELOG 0.11.0 regions as a skeleton; **one re-bless** of the canonical text and the README table (the records run ahead of the code until Phase 2: the Phase 1 commit says so); `output::VOLUME_TRANSITION_MS` and `volume_transition_samples` (inert); the three hook fields (inert); `configs::stage11_only_*`, `analysis::click_energy`, `stimulus::tone_peaking_at` with unit tests; the six golden tests and the C smoke loop run. Checkpoint: `cargo test --all-features` green; the six SHA guards unmoved; the cross-check and the audit green.
2. **Phase 2, US1**: `volume_ramp_checks` (splice with the first and last differing samples, ceiling, ratio reader, partitions, click reduction against R-02's table, retarget, zeros, drag) written and run against 0.10's step: **red** on FR-004, the drag, the first/last-differing rows and the zeros from N; the splice before the change, the partitions, the ratio reader and the ceiling are green by construction (stated, not misread as all red) → the core (`OutputChain` fields, `set_controls`, `ramp_step`, `fresh`, the hook fields live, the unit tests) → `us1_volume_ramp` (US1), `volume_ramp_matrix`, `volume_ramp_edge_cases`; the restated 009 checks (R-06). Checkpoint: US1 at the seven rates; the six SHA guards unmoved.
3. **Phase 3, US2**: the whole-engine cases (start-at-once after `new` / `reset` / `reconfigure` / `start_playback`, a zero-length call, route change, bypass, refused position, clamp identity, drain, a change during the drain, the one-pass after a playback), the equivalence from the change (a block boundary at skip + c), `determinism::volume_ramp_partitions`, `alloc_free`, `mutation` (four mutant configurations), `timing::volume_ramp_cost`, `scripts/ab-timing.sh` against `bd6d827` and `ab-timing.yml`. Checkpoint: record-then-play equals the one-pass; the costs within budget.
4. **Phase 4, US3**: README, rustdoc, A-045's verification protocol text complete, signal-chain row 11, spec 009's notes, the issue template, the CHANGELOG regions filled, `readme_volume_statements`, the audit. Checkpoint: `us1_manifest`, `us2_cross_check`, the audit; no second canonical re-bless.
5. **Phase 5, C**: the define, the comment, the header, `smoke.c`'s case and `smoke_reference.rs`'s hash, `ffi_parity`'s case. Checkpoint: the C loop.
6. **Phase 6, Records and gates** (010's gate order: `cargo llvm-cov` stops at the first failing test and the new golden is red until blessed): the gate **without** `golden_volume_ramp` (fmt, clippy, tests, the MSRV test build with `-D warnings`, the header drift check); the solo `volume_ramp_cost` run and the development-machine A/B quoted; the post-implementation hardware review and the coach review (the SC-008 reader test), both on the session's model; **the golden bless, once, after the reviews**, its SHA pinned, in the same commit as `ci.yml` and `scripts/ios-device-golden.sh`; then the full gate **with** coverage; CLAUDE.md's 011 line and the bless/report commands; the PR (ask first), with the CI runner's A/B (`ab-timing.yml`) run and quoted before the merge.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Per-sample state (the knob and a counter) in a control that was per-block | FR-001 – FR-003: the transition is defined per sample and must not depend on the partition; the chain already reads the scalar per sample, so the state sits beside it and the idle path adds one branch and one store | A per-block gain step of 1/N (still a staircase at large blocks, partition-dependent); a buffer of precomputed gains (an allocation, or a fixed size tied to the largest block) |
