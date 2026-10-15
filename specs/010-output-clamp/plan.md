# Implementation Plan: Optional Output Clamp (a host option at the route output)

**Branch**: `010-output-clamp` | **Date**: 2026-10-15 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/010-output-clamp/spec.md`

## Summary

A boolean host setting, `output.clamp_enabled`, off by default, that holds every sample at the route-output tap within [−1.0, +1.0] by replacement, on both routes, after the route's last stage and after any output-chain bypass; no other tap is touched. It is an emulator convenience for integer-buffer hosts, not a device stage: no manifest row, no new assumption, A-042 amended. Technically it is one bool on `OutputChain`, gated on the tap at construction, wrapping `process` with `f64::clamp` (research R-01); the identity `on == clamp(off)` holds exactly on the narrowed `f32` output and is the harness's only oracle (R-06). The C struct grows 104 → 108 with two explicit reserved fields so that 0.9's size is refused (R-03). Version 0.10.0; one new golden file (105 entries); the five earlier golden files and the stored form unmoved; the canonical text re-blessed once for its header's version.

## Technical Context

**Language/Version**: Rust, edition 2024, MSRV 1.85 (001 R-01). No Python: there is no design script (nothing to derive; the bound is A-042's full scale).

**Primary Dependencies**: none new.

**Storage**: unchanged (the memory, the stored form version 2, export and import untouched; FR-006).

**Testing**:
- **Core**: unit tests in `stages/output.rs` (the identity on a vector that crosses the bound, including 1.0 + 2⁻⁶⁰; ±∞ → ±1.0 and NaN → 0.0; the tap gating: `clamp` true only for `RouteOutput`; a bypassed chain at `RouteOutput` still bounded; `at_rest`/`n_out` unchanged), `settings.rs` (the presets both `false`; `Default`), `manifest.rs` (counts unchanged).
- **Harness**: `clamp_checks.rs` (R-06) and the tests `us1_clamp` (US1 and US2 at `measurement_rates()`), `clamp_matrix` (the seven-rate report), `clamp_edge_cases` (every spec edge case), `golden_clamp` (105 entries); `ffi_parity`, `alloc_free`, `determinism`, `timing` (`clamp_cost`), `mutation` (one case), `us2_cross_check` (the README section and the 0.10.0 entry's denylist pass; the header version) extended; `stimulus::square` with a unit test of its half-period rule.
- **Golden**: the five existing files untouched (SHA guards); `golden-clamp-v1.json` blessed once at the end (`RR_DR60_BLESS=clamp`).
- **Coverage**: gate 80 %; within 1 point of the 0.9 figure (98.18 % on CI).

**Target Platform**: unchanged: 8 targets. `f64::clamp` is an IEEE operation; the golden matrix proves bit-identity on six.

**Project Type**: library: a Rust crate plus a C ABI.

**Performance Goals**: ≤ 2 ns per host sample (about 3 %) added to the codec-bypassed speaker route's one-pass cost at 48 kHz, the median of five alternating on/off runs in a solo release run (SC-005; R-08); expected under 1 ns. Latencies and the drain unchanged at every rate.

**Constraints**:
- No reported latency change and no drain change (FR-005): the clamp is stateless; `n_out` untouched.
- Bit-identity at every rate, block partition and platform: the same `OutputChain` code in both `PlaybackPath` instances; the option read at construction, never per sample from the settings.
- No allocation in per-block calls; no `unsafe` outside `rr_dr60_ffi`; the C struct grows at the end (104 → 108) with explicit reserved fields.
- The five existing golden files and the manifest's rows, counts and entry strings do not change (FR-002, FR-004); the clamp golden is blessed exactly once; the canonical text once (the header version).
- Every string follows the claims policy (FR-004): "the output clamp", "an emulator convenience, not a device behavior"; "clip" stays the amplifier's word; never "accurate", "protects", "safe for", "the device clips here".

**Scale/Scope**: about 120 lines of core (`stages/output.rs` ≈ 60 with tests, `settings.rs` ≈ 30, `lib.rs` docs, `__test_hooks` one variant), about 80 lines of FFI (three fields, the conversion, the default, layout tests, the header), about 600 lines of harness (`clamp_checks.rs` ≈ 200, four test files ≈ 300, extensions ≈ 100, `configs`, `golden`, `stimulus::square`), `smoke.c` two cases and a refusal, `smoke_reference.rs` two hashes, docs (A-042, README, CHANGELOG, CLAUDE.md), one new golden file, `ci.yml` and `scripts/ios-device-golden.sh` (the test name), the contracts.

There are no open NEEDS CLARIFICATION items. The precondition the spec handed to the plan (which stimuli exceed the bound at which rates) was measured at plan time (R-04): all four overshooting stimuli exceed it at every rate, the reference-position tone never does.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | How this plan complies | Pre-research | Post-design |
|---|---|---|---|
| I. Portable Library First | No new dependencies; the core stays `no_std` + `alloc` (one `f64::clamp`); the C struct grows at the end with explicit reserved fields, no new function or status; the header regenerated and drift-checked; the smoke loop covers both routes with the option on (R-03, R-09). | ✅ | ✅ |
| II. Evidence-Based Fidelity | The bound is A-042's full scale, cited at the code constant; the option is an engine convenience with no hardware claim, so no new assumption is registered (R-07; coach C12); A-042 is amended (the option named; the earphone overshoot statement corrected) with its status line marked and a CHANGELOG entry; the default (off) keeps the modeled output unchanged. | ✅ | ✅ |
| III. Measured, Test-First Verification | The identity `on == clamp(off)` is the oracle for every stimulus, rate and route, written and seen to fail first (the chain without the clamp fails it on every overshooting stimulus); the option-off figures are reported, not toleranced (R-04, R-06); the five goldens guard the earlier specs unchanged and the option has its own file; mutation: a bound moved to ±0.5 fails the identity. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | One IEEE operation, no clock, no state, no allocation; the option read at construction; `alloc_free` and `determinism` extended (R-01, R-06). | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | The option is not a stage and adds no row; it is gated to the route-output tap so every per-stage tap keeps its meaning and the model's overshoot stays measurable (R-01; Complexity Tracking row 1). | ✅ | ✅ |
| VI. Neutral, Honest Communication | "An emulator convenience, not a device behavior"; the README states the default, what it bounds, what it does not, and the ×32767 conversion; the wording tests scan the README section and the CHANGELOG entry (R-07). | ✅ | ✅ |
| VII. Independently Testable Features | US1 by the `clamp_only` configuration (every stage bypassed, a synthetic above-full-scale stimulus: no stage's behaviour needed) plus the chain configurations reported; US2 by the equivalence, determinism and golden checks; US3 by the cross-check; US4 by C parity and the smoke loop; specs 001 – 009 keep their configurations with the option off. | ✅ | ✅ |
| Repo standards | rustdoc on every new public item; SemVer 0.10.0 (breaking: the C struct size); Conventional Commits; no audio committed. | ✅ | ✅ |

**Gate result**: PASS. One complexity item is recorded below.

## Project Structure

### Documentation (this feature)

```text
specs/010-output-clamp/
├── plan.md              # This file
├── research.md          # R-01 – R-10 (the placement and the identity proof, the settings field, the C layout, the measured precondition, stimuli and golden, harness, records, cost, smoke, risks)
├── data-model.md        # the setting, the chain's field, the taps, the C layout, the harness configurations and stimuli, the expected report, the golden
├── quickstart.md        # the validation runs per story and the gates
├── contracts/
│   ├── rust-api.md      # OutputSettings::clamp_enabled, the presets, the tap documentation, the test hook
│   ├── c-api.md         # RrDr60Settings 104 → 108 (reserved_010 at 99, output_clamp_enabled at 104, reserved_010_tail at 105), smoke cases, layout tests
│   └── golden-format.md # golden-clamp-v1.json: 3 stimuli × 5 configurations × 7 rates = 105, the twins, the asserted identity
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks output (not created by /speckit-plan)
```

### Source Code (repository root)

```text
crates/rr_dr60/src/
├── lib.rs                      # crate docs: one paragraph on the output clamp under the output-chain paragraph
├── settings.rs                 # OutputSettings { .., clamp_enabled } (false in DEVICE and BYPASSED); Tap docs: which taps are bounded
├── stages/output.rs            # OutputChain.clamp (set at new(): clamp_enabled && tap == RouteOutput); process = inner(y) then clamp; unit tests
└── lib.rs (__test_hooks)       # OutputMutation::ClampBound(f64) for the mutation test

crates/rr_dr60_ffi/
├── src/lib.rs                  # reserved_010 [u8; 5] at 99, output_clamp_enabled at 104, reserved_010_tail [u8; 3] at 105 (108 bytes); conversion; default; layout tests; 0.10.0
├── include/rr_dr60.h           # regenerated
├── tests/c/smoke.c             # two clamped ±1.0 square cases (both routes at position 1), the option-off exceedance, struct_size = 104 refused
└── tests/smoke_reference.rs    # the two pinned hashes

crates/rr_dr60_harness/
├── src/stimulus.rs             # square(freq_hz, amp, fs, len): half-period round(fs / (2·freq)), starting at +amp
├── src/configs.rs              # CLAMP_CONFIGS (five), bypass_route_output, clamp_twin(name); clamp_enabled false everywhere else
├── src/clamp_checks.rs         # NEW (R-06): identity, per_stage_taps_unclamped, bypassed_chain_bounded, equivalence, change timing
├── src/output_checks.rs        # chain_equivalence gains a configuration-name parameter (009 callers unchanged in behaviour)
├── src/golden.rs               # CLAMP_STIMULI, GOLDEN_CLAMP_CONFIGS, clamp_route_volume, generate_clamp (asserts the identity and the equivalence), committed_clamp, path_clamp
├── golden/golden-clamp-v1.json # NEW, blessed once (105 entries)
├── golden/manifest-v1.txt      # re-blessed once: library_version 0.10.0
├── tests/us1_clamp.rs          # NEW (US1, US2)
├── tests/clamp_matrix.rs       # NEW: the seven-rate report
├── tests/clamp_edge_cases.rs   # NEW
├── tests/golden_clamp.rs       # NEW
└── tests/ffi_parity.rs, alloc_free.rs, determinism.rs, timing.rs (clamp_cost), mutation.rs, us2_cross_check.rs  # extended

.github/workflows/ci.yml, scripts/ios-device-golden.sh   # golden_clamp beside golden_output
docs/hardware/assumptions.md (A-042 amended), README.md (output section; the record-path headroom note), CHANGELOG.md (0.10.0), CLAUDE.md, Cargo.toml (0.10.0 × 4)
```

**Structure Decision**: the existing three-crate workspace; no new module in the core (one field and a wrapper in `stages/output.rs`), one new check module and four test files in the harness, matching 009's layout one for one.

## Task order (for /speckit-tasks)

1. **Phase 1, Setup**: `stimulus::square` with its unit test; `configs::CLAMP_CONFIGS`, `bypass_route_output`, `clamp_twin`; `OutputSettings::clamp_enabled` (both presets `false`; the five golden tests green before anything else changes); `clamp_checks::identity` written red against the unchanged chain (it must fail on the overshooting stimuli).
2. **Phase 2, US1**: the core (`OutputChain.clamp`, the wrapper, the unit tests incl. ±∞/NaN and the tap gating) → `us1_clamp` (the identity on the primary and secondary stimuli, the taps, the bypassed chain, the reference-position tone's zero count), `clamp_matrix` (the table of R-04 re-printed), `clamp_edge_cases`. Checkpoint: the identity at the seven rates on both routes; the five SHA guards unmoved.
3. **Phase 3, US2**: the equivalence with the option on (both routes × two positions × seven rates), route and volume change timing with the option on, `determinism`, `alloc_free`, `mutation` (the `ClampBound` hook), `timing::clamp_cost`. Checkpoint: record-then-play equals the one-pass; the latencies unchanged.
4. **Phase 4, US3**: A-042 amended; the README output section and the record-path note; the CHANGELOG 0.10.0 entry (Added / Changed / Documentation parts between `0.10.0` markers); the version bump (Cargo.toml × 4) and **the one canonical-text re-bless**; `us2_cross_check` extended; the traceability audit. Checkpoint: `us1_manifest`, `us2_cross_check`, the audit.
5. **Phase 5, US4**: FFI (the three fields, the conversion, the default, the layout tests), the header, `smoke.c` and `smoke_reference.rs`, `ffi_parity`. Checkpoint: the C loop.
6. **Phase 6, Records and gates**: `ci.yml` and `scripts/ios-device-golden.sh`; the full gate (fmt, clippy, tests, MSRV with `-D warnings`, coverage, the header drift check); the solo `clamp_cost` run quoted; the post-implementation hardware review (`hw-emulation-reviewer`, Opus) and the coach review (Opus); **the golden bless, once, after the reviews**, its SHA pinned; CLAUDE.md's 010 line and the bless/report commands; the PR (ask first).

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| The clamp gated on the tap at construction, inside `OutputChain`, rather than a plain post-processing step in `paths.rs` | FR-003: only the route-output tap is bounded, and the one-pass returns the configured tap from the same `process` call; the chain already knows the tap, so the gate costs one bool and no second tap check | A clamp in `try_next_host` after `narrow_out` would need the tap re-derived there (a duplicate of the chain's gating) or would bound the per-stage taps too, breaking FR-003 |
| Two reserved fields in the C struct (104 → 108) for one bool | FR-008 (coach C3): a bool appended alone sits in 0.9's tail padding and leaves the size at 104, so a stale 0.9 caller would be accepted and its uninitialized byte 99 read as the option (undefined behaviour) | Validating `reserved_009` as zero and placing the bool there (changes a documented "ignored" field's contract and does not stop the next version's padding problem) |
