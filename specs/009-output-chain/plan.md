# Implementation Plan: Output Chain (volume and power amplifier, speaker, earphone; stages 11–13)

**Branch**: `009-output-chain` | **Date**: 2026-10-14 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/009-output-chain/spec.md` (the coach and hardware-emulation reviews of the draft applied; the clarify session taken at the defaults; all in its Clarifications section).

## Summary

The playback path and the one-pass processing gain the device's **output chain** at the host rate, after the resampler and before the narrowing: a **volume control** (a position 0 … 1, the control's wiper fraction with the circuit's loaded gain law, an engine-level control like the route, default at the reference position where codec full scale meets the amplifier's clip), the **amplifier path** on the speaker route (a 32 Hz coupling corner at the volume control's input, common to both routes, a 159 Hz corner at the amplifier's input, the loaded gain (22.7 dB at full travel) folded with the 1.15 V clip ceiling into one block-rate scalar so the reference position is unity, a symmetric parabolic knee from 0.85 of the ceiling), the **speaker template** (a 750 Hz / Q 1.8 second-order roll-off, a +4.5 dB shelf toward 3 kHz, peak-normalized to 1.0; a registered low-confidence guess with its own bypass), and the **earphone path** (two first-order corners near 120 and 280 Hz from the follower's loaded input and output couplings into the 16 Ω load, registered as ranges at low confidence; the follower's class-A cut-off registered as not modeled). Each route's output is scaled to its own full swing (A-042). The route selects the branch; a new default tap, the route output, resolves to the selected route's last stage; `AfterPlayback` keeps its 0.8 meaning. The drain grows by the chain's 58 ms settle (the −100 dB rule of spec 006). The four golden files hold (the earlier checks run with the chain bypassed); the chain gets its own golden file, blessed last. Version 0.9.0.

Technical approach:

- **Placement and controls** (R-01, R-05, R-06): `stages/output.rs` holds `OutputChain`, owned by `PlaybackPath` and run in `try_next_host` on the `f64` sample before `narrow_out`; `Pipeline` keeps `route` and `volume` and copies them into the chain at every processing call, so changes take effect at the next block; `Tap::RouteOutput` (default), `AfterVolumeAmp`, `AfterSpeaker`, `AfterEarphone` appended; `Error::InvalidVolume`, `Error::TapNotOnRoute`.
- **Design** (R-02 – R-04; contracts/output-chain-design.md): the constants `V_CLIP` 1.15 V (at the measured 2.8 V rail; 1.0 – 1.4 registered), `G` 13.6 (22.7 dB at full travel) with the loaded law, `P_REF` 0.1335, the corners 32 (common) / 159 (amplifier) / 120 and 280 (earphone, nominal) Hz, the template 750 Hz / 1.8 / +4.5 dB with a per-rate tolerance, the knee `k` 0.85; the circuit read from the full-resolution scans of S-002's sheet 3.1 at plan time; bilinear prewarped sections per host rate from `tools/output-chain/design_output_chain.py`, emitted as exact bit patterns into `stages/output_coeffs.rs`; a first-order section type beside the biquad; the script prints every derived figure and the Rust pins it.
- **Drain and equivalence** (R-08): the playback drain gains `N_OUT(rate)` (57.6 ms: the 32 Hz corner's response below −100 dB of its peak, spec 006's rule); the one-pass equivalence holds because the skipped prefix is exact zeros (asserted).
- **Interface** (R-05 – R-07; contracts/rust-api.md, c-api.md): `Settings::output: OutputSettings` (three bypasses), `set_volume`/`volume` (the loaded law recomputed per block), `output::VOLUME_REFERENCE`; C struct 96 → 104, taps 5 – 8, statuses 23 – 24, `rr_dr60_set_volume`/`rr_dr60_get_volume`, `RR_DR60_VOLUME_REFERENCE`.
- **Measurement** (R-10; contracts/golden-format.md): `output_checks.rs` (response against the design's discrete realization by DTFT, clip and distortion at ≥ 44.1 kHz driven by the volume, the loaded volume law and the scale, the chain equivalences, route and volume timing, refusals; the corners' −3 dB points pinned from the coefficients in core tests), five configurations, `golden-output-v1.json` (140 entries with `route` and `volume` fields; a `default_loud` configuration drives the clipper by volume) blessed once, last.
- **Records** (R-13): manifest rows 11 – 13 (counts 5 / 10), registers (A-031, A-032 adopted; A-010 revised; A-042 – A-044 new; S-006 already corrected), signal chain, README section, CHANGELOG 0.9.0, the field-measurement note's row.

## Technical Context

**Language/Version**: Rust, edition 2024, MSRV 1.85 (001 R-01). Python 3 for the design script (`uv run`; `numpy`, `scipy`; never runs in CI or at build time).

**Primary Dependencies**: none new. The harness keeps `sha2`, `serde_json`, `rustfft`.

**Storage**: unchanged (the memory, the stored form version 2 and export/import are untouched, FR-013).

**Testing**:
- **Core**: unit tests for `FirstOrder` (impulse response against the closed form; flush), the clipper (the five bracket figures at 48 kHz pinned to the script's; symmetry; never above 1.0; a C¹ joint), the loaded volume law (`K_SPK(p)`, `K_EAR(p)` at five positions against the script's figures; unity at `P_REF` within 1e-7; exactly 0.0 at position 0), `OutputChain` (route switch resets the right branch; the sections flush to exactly 0.0 on 400 ms of zeros; bypass semantics; the taps), the coefficient table against the script's printed figures (each section's −3 dB point from its coefficients within ±10 % of the registered figure, the template's peak and 1 kHz reading, the group delays, `N_OUT`, `P_REF`), the drain table, the settings and validation (`InvalidVolume`, `TapNotOnRoute`), the manifest's `bypass_control_anchor` arms.
- **Harness**: `output_checks.rs` (R-10) and the tests `us1_output` (stories 1 and 2 figures), `output_matrix` (the seven-rate report; distortion at the four rates ≥ 44.1 kHz), `output_edge_cases` (every spec edge case; a chain-bypassed export imported and played with the chain on, FR-013), `golden_output` (140 entries); `ffi_parity`, `alloc_free`, `determinism` (route/volume cases), `timing` (SC-007), `mutation` (a coefficient perturbation fails the template check) extended; `us1_manifest`, `us2_cross_check` re-blessed for the strings; the C smoke loop on both routes. Audio-heavy loops use `configs::measurement_rates()`.
- **Golden**: the four existing files untouched (SHA guards); `golden-output-v1.json` blessed once at the end (`RR_DR60_BLESS=output`).
- **Coverage**: gate 80 %; within 1 point of the 0.8 figure (98.27 % on CI).

**Target Platform**: unchanged: 8 targets. The chain is `f64` IEEE arithmetic with no platform math function; the golden matrix proves bit-identity on six.

**Project Type**: library: a Rust crate plus a C ABI; one offline Python tool.

**Performance Goals**: the chain adds ≤ 25 % to the codec-bypassed one-pass cost per 48 kHz host sample (≤ about 15 ns over about 60 ns; about 5 – 10 ns expected), measured in the release timing job for both routes (SC-007); reported latencies unchanged; the 1 kHz group delay per route ≤ 0.5 ms (about 0.28 ms on the speaker route from the template, under 0.1 ms per first-order corner); the loaded law's two divides per block are negligible.

**Constraints**:
- No reported latency change (FR-008): minimum-phase recursive sections only; the latency tables of 004 / 006 / 008 untouched.
- Bit-identity at every rate, block partition and platform: the same `OutputChain` code in both `PlaybackPath` instances; controls applied at block boundaries only.
- No allocation in per-block calls or the controls; the chain's state is fixed-size; coefficients are constants.
- The four existing golden files do not change (FR-016); the output golden is blessed exactly once.
- No `unsafe` outside `rr_dr60_ffi`; the C struct grows at the end (96 → 104); statuses and taps appended.
- Every string follows the claims policy (FR-018): "modeled on the circuit and the datasheet", "a template of a small speaker".

**Scale/Scope**: about 900 lines of core (`stages/output.rs` ≈ 350 with tests, `stages/first_order.rs` ≈ 120, `stages/output_coeffs.rs` generated ≈ 150, `paths.rs` ≈ 80 changed, `pipeline.rs` and `transport.rs` ≈ 80 changed, settings / error / validate / manifest ≈ 120), about 250 lines of FFI, about 1 000 lines of harness (`output_checks.rs`, a corner-fit helper, four test files, extensions), one Python tool (≈ 350 lines), docs, registers, CHANGELOG, README, one new golden file, the manifest text re-blessed.

There are no open NEEDS CLARIFICATION items. The re-read of S-002 sheet 3.1 was done at plan time from the archive's full-resolution scans (R-02) and corrected the gain (22.7 dB), the corners and the earphone topology; the clarify fallbacks are superseded. Four plan-time spec amendments are recorded in the spec's Clarifications (R-02, R-11): the circuit figures, the volume-0 decay time (50 → 100 ms, exact zeros only on the earphone route's after-stage-11 tap), the drain growth, and the tap naming.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | How this plan complies | Pre-research | Post-design |
|---|---|---|---|
| I. Portable Library First | No new dependencies; the core stays `no_std` + `alloc` (IEEE `f64` sections and a parabolic knee; a generated coefficient file); the design script lives under `tools/`; the C struct grows at the end (96 → 104), taps 5 – 8 and statuses 23 – 24 appended, two control functions added, the header regenerated and drift-checked, the smoke loop covers both routes (R-06, R-07). | ✅ | ✅ |
| II. Evidence-Based Fidelity | Every constant traces to S-002 / S-006 component values or to A-010, A-031, A-032 (corrected on the branch), A-042 – A-044 (new), each with confidence and a capture that would test it; the speaker is a registered template at low confidence with its own bypass; the default-output change is recorded in the CHANGELOG as the adoption of A-031 / A-032 and the revision of A-010; the script prints every figure and the Rust pins it (R-02 – R-04, R-13). | ✅ | ✅ |
| III. Measured, Test-First Verification | Each FR maps to an `output_checks` function or a core unit test (R-10), written and seen to fail first; the templates are computed independently in the harness; distortion is measured only where the host rate allows it (D8); the goldens guard the earlier specs unchanged and the chain with its own file; mutation: a perturbed coefficient fails the template check. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | IEEE `f64` operations only, no platform math, no clock; fixed-size state; controls read once per block; no allocation in any new call; `alloc_free` extended (R-01, R-04, R-09). | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | Three stages, three bypasses, taps after each, two route-specific configurations for isolated measurement; the earlier taps keep their meaning (R-06; Complexity Tracking rows 1 – 3). | ✅ | ✅ |
| VI. Neutral, Honest Communication | "Modeled on the circuit and the datasheet"; the speaker "a template of a small speaker"; the manifest's coverage notes name the unknowns and the audibility note; the README states the output-scale rule, the default-volume consequence and the host-rate caveat (R-13). | ✅ | ✅ |
| VII. Independently Testable Features | US1 by the output-chain-only configurations per route; US2 by the equivalence and golden checks; US3 by the cross-check; US4 by C parity and the smoke loop; specs 001 – 008 keep their configurations with the chain bypassed. | ✅ | ✅ |
| Repo standards | rustdoc on every new public item; SemVer 0.9.0 (breaking: C struct size, the default output and default tap); Conventional Commits; no audio committed. | ✅ | ✅ |

**Gate result**: PASS. Four complexity items are recorded below.

## Project Structure

### Documentation (this feature)

```text
specs/009-output-chain/
├── plan.md                        # This file
├── research.md                    # Phase 0: decisions R-01 – R-14
├── data-model.md                  # Phase 1: the chain, controls, taps, per-rate table, drain
├── quickstart.md                  # Phase 1: validation walk-through
├── contracts/
│   ├── output-chain-design.md     # constants, prototypes, the knee, the per-rate table (R-02 – R-04, R-08)
│   ├── rust-api.md                # settings, controls, taps, errors, constants
│   ├── c-api.md                   # RrDr60Settings 96 → 104, taps 5 – 8, statuses 23 – 24, two functions, one constant
│   └── golden-format.md           # golden-output-v1.json: route and volume fields; the unchanged four
└── tasks.md                       # Phase 2 (/speckit-tasks)

.github/workflows/ci.yml           # golden_output added where the golden tests are listed; the timing job prints the chain rows
```

### Source Code (repository root)

```text
crates/rr_dr60/src/
├── lib.rs                      # crate docs (the output chain, the route's sound, the scale); pub mod output (VOLUME_REFERENCE, constants)
├── settings.rs                 # OutputSettings { volume_amp_enabled, speaker_enabled, earphone_enabled }; Settings::output;
│                               # Tap::{RouteOutput (default), AfterVolumeAmp, AfterSpeaker, AfterEarphone}
├── error.rs                    # Error::InvalidVolume { position }, Error::TapNotOnRoute { tap, route }
├── stages/
│   ├── first_order.rs          # NEW: FirstOrder { b0, b1, a1, s } with the biquad's flush rule
│   ├── output.rs               # NEW: OutputChain (volume, amp path with FirstOrder + K_SPK + knee, speaker Biquad + shelf, earphone 2 × FirstOrder);
│   │                           #      set_controls(route, volume), process(y) -> f64 with the tap point, reset, runs_any(), N_OUT
│   └── output_coeffs.rs        # NEW, GENERATED by tools/output-chain/design_output_chain.py: per-rate sections, P_REF, K_SPK, KNEE, N_OUT, group delays
├── paths.rs                    # PlaybackPath { chain: OutputChain }; try_next_host runs the chain before narrow_out; drain += N_OUT; latency unchanged
├── pipeline.rs                 # volume: f32 beside route; set_volume/volume; controls copied into both paths per call; TapNotOnRoute check
├── transport.rs                # play/start_playback copy the controls; EngineState unchanged
├── validate.rs                 # nothing new (bools); the tap-route check lives in the processing calls
└── manifest.rs                 # rows 11, 12, 13; counts 5 / 10; bypass_control_anchor arms

crates/rr_dr60_ffi/
├── src/lib.rs                  # RrDr60Settings + 3 bools at 96 – 98 (104 bytes); RR_DR60_TAP_ROUTE_OUTPUT..AFTER_EARPHONE (5 – 8);
│                               # statuses 23 INVALID_VOLUME, 24 TAP_NOT_ON_ROUTE; rr_dr60_set_volume, rr_dr60_get_volume; RR_DR60_VOLUME_REFERENCE; layout tests; 0.9.0
├── cbindgen.toml               # the real-time-safe list gains the two functions
├── include/rr_dr60.h           # regenerated
└── tests/c/smoke.c             # plays the ±0.125 square wave through both routes at two volumes; hashes pinned from the Rust path

crates/rr_dr60_harness/
├── src/analysis.rs             # discrete_template_response (the only new helper: the design's bilinear sections evaluated at a frequency)
├── src/output_checks.rs        # NEW (R-10)
├── src/configs.rs              # output_only_speaker, output_only_earphone, default_output, default_earphone; `output` bypassed on every earlier config
├── src/golden.rs               # OUTPUT_STIMULI, OUTPUT_CONFIGS, route/volume fields, generate_output, committed_output, path_output
├── golden/golden-output-v1.json# NEW, blessed once (140 entries)
├── golden/manifest-v1.txt      # re-blessed with the strings
├── tests/us1_output.rs         # NEW (US1, US2)
├── tests/output_matrix.rs      # NEW: the seven-rate report
├── tests/output_edge_cases.rs  # NEW
├── tests/golden_output.rs      # NEW
└── tests/ffi_parity.rs, alloc_free.rs, determinism.rs, timing.rs, mutation.rs, us2_cross_check.rs  # extended

tools/output-chain/design_output_chain.py   # NEW: inputs V_CLIP, G, corners, template; prints every constant; --check; --write stages/output_coeffs.rs
docs/hardware/assumptions.md (A-010 revised; A-031, A-032 adopted; A-042 – A-044 new), signal-chain.md (rows 11 – 13 "Modeled by"; diagram),
field-measurement.md (the output-chain row); CHANGELOG.md (0.9.0); README.md (output section; generated table); CLAUDE.md
```

**Structure Decision**: the existing three-crate workspace. The chain is one stage module with one struct because the three stages share the volume, the route and the block-boundary control protocol, and because the playback path owns exactly one instance per path; the first-order section is its own small module beside the biquad because four of them run per sample.

## Task order (for /speckit-tasks)

1. **Phase 1, Setup**: the design script and `output_coeffs.rs` (`--check` green); `FirstOrder`; `configs::settings` sets the three bypasses off for every pre-009 configuration; the harness template helper; the registers carry the sheet 3.1 re-read's figures and the plan reviews' corrections (research R-02; done at plan time). **Checkpoint: the whole existing suite and the four SHA guards green with the chain bypassed everywhere.**
2. **Phase 2, US1**: `output_checks` and the core unit tests written red → `OutputChain` → the path integration, the controls, the taps, the drain → `us1_output`, `output_matrix`, `output_edge_cases`. Checkpoint: the per-route figures at the seven rates; distortion at the four rates.
3. **Phase 3, US2**: the equivalence cases with the chain on, the 0.8-equality case at `AfterPlayback`, route and volume timing, the FR-013 import case, `determinism`, `alloc_free`, `mutation`, the drain table. Checkpoint: record-then-play equals the one-pass on both routes × two volumes × seven rates, `AfterPlayback` equals the 0.8 output bit for bit, control changes land at the next block, the drain table pinned (the output golden waits for Phase 6).
4. **Phase 4, US3**: manifest rows 11 – 13; the registers; the signal chain; the README section; the field-measurement row. **The manifest and README re-bless sits here.** Checkpoint: `us1_manifest`, `us2_cross_check`, the traceability audit.
5. **Phase 5, US4**: FFI (struct, taps, statuses, functions, constant), the header, the smoke loop on both routes, `ffi_parity`. Checkpoint: the C loop.
6. **Phase 6, Records and gates**: **read S-003 pages 4 – 17 for the MSM7702's analog output level** (A-030; if it differs from the family figure, move the constant and re-derive `P_REF` before anything is blessed); timing (SC-007), coverage, CHANGELOG, `ci.yml`, CLAUDE.md; the post-implementation hardware review; **the output golden bless is the last task, after that review** (the template figures may still move). Then the PR (ask first).

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Controls applied at block boundaries (a copy per processing call into two path instances) | FR-009's "takes effect at the next block" and the per-sample path free of control reads; both paths must see the same controls | Reading the engine's fields per sample (a load and a branch per sample; a mid-block change the spec forbids); a single shared chain (the one-pass and the player run different streams and must keep separate states) |
| A fifth tap family and a route-following default (`RouteOutput`) | The default output must be the selected route's and `AfterPlayback` must keep its meaning for existing callers (R-06) | Redefining `AfterPlayback` (a silent semantic change for a C caller who set `tap = 1`) |
| The playback drain grows by `N_OUT` for both routes whenever any output stage runs | Spec 006 FR-008's "to settled" with corners that ring for 80 ms; one figure keeps the drain a constant of the configuration while the route can change mid-playback | A per-route drain (changes under the player's feet); resetting the chain instead of draining (not the device's behaviour) |
| A second filter section type (`FirstOrder`) beside `Biquad` | Three first-order sections per sample on either route; a biquad with zeroed coefficients costs five multiplies each (R-03, SC-007) | Reusing `Biquad` (about 2× the chain's cost for nothing); folding pairs of first-order sections into one biquad (loses the per-stage bypass and the per-corner fit) |
