# Implementation Plan: Optional Output Clamp (a host option at the route output)

**Branch**: `010-output-clamp` | **Date**: 2026-10-15 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/010-output-clamp/spec.md`

## Summary

A boolean host setting, `output.clamp_enabled`, off by default, that holds every sample at the route-output tap within [−1.0, +1.0] by replacement, on both routes, after the route's last stage and after any output-chain bypass; no other tap is touched, and the play calls (which always return the route output) are always bounded. It is an emulator convenience for integer-buffer hosts, not a device stage: no manifest row, no new assumption, A-042 amended with every overshoot figure labelled by its configuration and the speaker route's provable bound (the template's impulse-response L1 norm, 1.75 – 2.29 by rate). Technically it is one gate on `OutputChain`, set on the tap at construction, wrapping `process` with a hand-written bound (research R-01); the identity `on == clamp(off)` holds exactly on the narrowed `f32` output (proven and fuzzed) and is the harness's only oracle (R-06). The C struct grows 104 → **112** (alignment 8) with two explicit reserved fields so that 0.9's size is refused and no tail padding returns (R-03). Version 0.10.0; one new golden file (126 entries, `golden-output-clamp-v1.json`); the five earlier golden files and the stored form unmoved; the canonical text re-blessed once for its header's version. The coach's (P1–P17) and the hardware reviewer's (D1–D10) plan reviews are applied; the spec's Clarifications record the session.

## Technical Context

**Language/Version**: Rust, edition 2024, MSRV 1.85 (001 R-01). No Python: there is no design script (nothing to derive; the bound is A-042's full scale; the L1 bound is recomputed in the harness from the committed coefficients).

**Primary Dependencies**: none new.

**Storage**: unchanged (the memory, the stored form version 2, export and import untouched; FR-006; checked by `stored_form_unchanged`).

**Testing**:
- **Core**: unit tests in `stages/output.rs` (the identity on a vector that crosses the bound, including 1.0 + 2⁻⁶⁰ and −0.0; `bound`: ±∞ → ±1.0, NaN → 0.0; the tap gating: the gate true only for `RouteOutput`; a bypassed chain at `RouteOutput` still bounded; the player bounded whatever the tap; `at_rest` / `n_out` unchanged), `settings.rs` (the presets both `false`; `Default`), `manifest.rs` (counts unchanged).
- **Harness**: `output_clamp_checks.rs` (R-06) and the tests `us1_output_clamp` (US1 and US2 at `measurement_rates()`), `output_clamp_matrix` (the seven-rate report, 1 s stimuli, the 18 cells, the quiet-material rows, the L1 bound), `output_clamp_edge_cases` (every spec edge case), `golden_output_clamp` (126 entries); `ffi_parity`, `alloc_free`, `determinism`, `timing` (`output_clamp_cost`), `mutation` (three mutants), `us2_cross_check` (`readme_output_clamp_statements`; the header version) extended; `stimulus::square` with a unit test of its half-period rule; `analysis::impulse_l1`.
- **Golden**: the five existing files untouched (SHA guards); `golden-output-clamp-v1.json` blessed once at the end (`RR_DR60_BLESS=output_clamp`), the twin identity and the equivalence asserted at bless time only.
- **Coverage**: gate 80 %; within 1 point of the 0.9 figure (98.18 % on CI).

**Target Platform**: unchanged: 8 targets. The bound is two compares and two selects; the golden matrix proves bit-identity on six.

**Project Type**: library: a Rust crate plus a C ABI.

**Performance Goals**: ≤ 2 ns per host sample (about 3 %) added to the codec-bypassed speaker route's one-pass cost at 48 kHz on full-scale noise at position 1, the median of five alternating on/off runs in a solo release run (SC-005; R-08); expected under 1 ns. Latencies and the drain unchanged at every rate.

**Constraints**:
- No reported latency change and no drain change (FR-005): the bound is stateless; `n_out` untouched.
- Bit-identity at every rate, block partition and platform: the same `OutputChain` code in both `PlaybackPath` instances; the option read at construction, never per sample from the settings.
- No allocation in per-block calls; no `unsafe` outside `rr_dr60_ffi`; no `f64::clamp` in the per-sample path (its assertion); the C struct grows at the end (104 → 112) with explicit reserved fields and three const assertions.
- The five existing golden files and the manifest's rows, counts and entry strings do not change (FR-002, FR-004); the clamp golden is blessed exactly once; the canonical text once (the header version, Phase 1).
- Every new name carries the `output_clamp` prefix ("clamp" alone already names the ADC's full-scale clamp, A-030).
- Every string follows the claims policy (FR-004): "the output clamp", "an emulator convenience, not a device behavior", "bounds" / "replaces"; "clip" stays the amplifier's word; never "accurate", "protects", "safe for", "hearing", "loudness limit", "the device clips here", "prevents clipping"; the word "verified" stays out of the CHANGELOG entry (the denylist stem `verif`).

**Scale/Scope**: about 150 lines of core (`stages/output.rs` ≈ 80 with tests, `settings.rs` ≈ 30, `lib.rs` docs and two hook fields), about 90 lines of FFI (three fields, the conversion, the default, the assertions and layout tests, the tap docs, the header), about 800 lines of harness (`output_clamp_checks.rs` ≈ 280, four test files ≈ 350, extensions ≈ 120, `configs`, `golden` with `n_at_bound`, `stimulus::square`, `analysis::impulse_l1`), `smoke.c` three cases, `smoke_reference.rs` two hashes, docs (A-042, README in two places, CHANGELOG, CLAUDE.md, one test doc comment), one new golden file, `ci.yml` (three lines) and `scripts/ios-device-golden.sh` (two test names), the contracts.

There are no open NEEDS CLARIFICATION items. The precondition the spec handed to the plan (which stimuli exceed the bound at which rates) was measured at plan time by the planner and the hardware reviewer (R-04): the four overshooting stimuli exceed it at every rate, the chain-only reference-position tone never does, and the 0.9 default's speaker route reaches about 2.05 at position 1, bounded by the L1 norm; the 0.9 default at the reference position is measured in Phase 1 for the sixth golden configuration.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | How this plan complies | Pre-research | Post-design |
|---|---|---|---|
| I. Portable Library First | No new dependencies; the core stays `no_std` + `alloc` (one hand-written bound); the C struct grows at the end with explicit reserved fields and const assertions, no new function or status; the header regenerated and drift-checked; the smoke loop covers both routes with the option on (R-03, R-09). | ✅ | ✅ |
| II. Evidence-Based Fidelity | The bound is A-042's full scale, cited at the code constant; the option is an engine convenience with no hardware claim, so no new assumption is registered (R-07; coach C12; hardware "keep"); A-042 is amended (the option named; every figure with its configuration; the L1 bound; the mechanism corrected; the distortion statement) with its status line marked and a CHANGELOG entry; the default (off) keeps the modeled output unchanged. | ✅ | ✅ |
| III. Measured, Test-First Verification | The identity `on == clamp(off)` is the oracle for every stimulus, rate and route, written and run red before the core change (Phase 2's first task); the option-off figures are reported, not toleranced (R-04, R-06); the L1 bound is recomputed by a test (the 008 lesson); the five goldens guard the earlier specs unchanged and the option has its own file with `n_at_bound`; mutation: three mutants fail the identity. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | Two compares and two selects, no clock, no state, no allocation, no assertion in the per-sample path; the option read at construction; `alloc_free` and `determinism` extended (R-01, R-06). | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | The option is not a stage and adds no row; it is gated to the route-output tap so every per-stage tap keeps its meaning and the model's overshoot stays measurable (R-01; Complexity Tracking row 1). | ✅ | ✅ |
| VI. Neutral, Honest Communication | "An emulator convenience, not a device behavior"; the README leads with headroom scaling as the fidelity-preserving choice, states the default, what the option bounds and does not, the ×32767 conversion, the distortion the option adds and the true-peak caveat; the overshoot figures carry their configurations; a wording test with required and forbidden phrases (R-07). | ✅ | ✅ |
| VII. Independently Testable Features | US1 by the `output_clamp_only` configuration (every stage bypassed, a synthetic above-full-scale stimulus: no stage's behaviour needed) plus the chain configurations reported; US2 by the equivalence (engaged at position 1), the change-timing identity, the stored form, determinism and golden checks; US3 by the cross-check; US4 by C parity and the smoke loop; the version bump and the canonical re-bless sit in Phase 1 so no story depends on another; specs 001 – 009 keep their configurations with the option off. | ✅ | ✅ |
| Repo standards | rustdoc on every new public item; SemVer 0.10.0 (breaking: the C struct size); Conventional Commits; no audio committed. | ✅ | ✅ |

**Gate result**: PASS. Two complexity items are recorded below.

## Project Structure

### Documentation (this feature)

```text
specs/010-output-clamp/
├── plan.md              # This file
├── research.md          # R-01 – R-10 (the placement, the bound function and the identity proof; the settings field; the C layout 104 → 112; the measured precondition and the L1 bound; stimuli, golden and naming; harness; records and wording; cost; smoke; risks)
├── data-model.md        # the setting, the chain's fields, the taps, the C layout, the configurations and stimuli, the expected report, the golden
├── quickstart.md        # the validation runs per story and the gates
├── contracts/
│   ├── rust-api.md      # OutputSettings::clamp_enabled, the presets, the tap and player documentation, the test hook fields
│   ├── c-api.md         # RrDr60Settings 104 → 112 (reserved_010 at 99, output_clamp_enabled at 104, reserved_010_tail [7] at 105), statuses, smoke cases, layout tests
│   └── golden-format.md # golden-output-clamp-v1.json: 3 stimuli × 6 configurations × 7 rates = 126, n_at_bound, the twins asserted at bless time
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks output (not created by /speckit-plan)
```

### Source Code (repository root)

```text
crates/rr_dr60/src/
├── lib.rs                      # crate docs: one paragraph on the output clamp; __test_hooks::OutputMutation { .., clamp_bound: f64 (1.0), clamp_positive_only: bool }
├── settings.rs                 # OutputSettings { .., clamp_enabled } (false in DEVICE and BYPASSED); Tap docs: which taps are bounded; the player sentence
└── stages/output.rs            # OutputChain { clamp, bound, positive_only }; fn bound(y, b); process = inner(y) then the bound; __mutate; unit tests

crates/rr_dr60_ffi/
├── src/lib.rs                  # reserved_010 [u8; 5] at 99, output_clamp_enabled at 104, reserved_010_tail [u8; 7] at 105 (112 bytes; three const assertions); conversion; default; RR_DR60_VERSION_MINOR = 10; tap docs; layout tests
├── include/rr_dr60.h           # regenerated
└── tests/c/smoke.c             # two clamped ±1.0 square cases (both routes at position 1), the option-off exceedance, struct_size = 104 refused (INVALID_ARGUMENT / STRUCT_SIZE)

crates/rr_dr60_harness/
├── src/smoke_reference.rs      # SMOKE_SPEAKER_CLAMP_HASH, SMOKE_EARPHONE_CLAMP_HASH
├── src/stimulus.rs             # square(freq_hz, amp, fs, len): half-period round(fs / (2·freq)), starting at +amp; unit test
├── src/analysis.rs             # impulse_l1 (the discrete sections' impulse-response L1 norm over 2 s)
├── src/configs.rs              # bypass_route_output; OUTPUT_CLAMP_CONFIGS (six); output_clamp_twin; output_clamp_route_volume; c_settings maps clamp_enabled
├── src/output_clamp_checks.rs  # NEW (R-06): identity, per_stage_taps_unclamped, bypassed_chain_bounded, equivalence, change_timing, stored_form_unchanged, latencies_and_drain_unchanged, l1_bound
├── src/output_checks.rs        # chain_equivalence gains a configuration-name parameter (009 callers unchanged in behaviour)
├── src/golden.rs               # OUTPUT_CLAMP_STIMULI, GOLDEN_OUTPUT_CLAMP_CONFIGS, GoldenEntry.n_at_bound, generate_output_clamp (bless-time assertions), committed_output_clamp, path_output_clamp
├── golden/golden-output-clamp-v1.json   # NEW, blessed once (126 entries)
├── golden/manifest-v1.txt      # re-blessed once in Phase 1: library_version 0.10.0
├── tests/us1_output_clamp.rs   # NEW (US1, US2): identity_*, taps_*, equivalence_*, change_timing_*, stored_form_*, latencies_*
├── tests/output_clamp_matrix.rs# NEW: the seven-rate report
├── tests/output_clamp_edge_cases.rs  # NEW
├── tests/golden_output_clamp.rs      # NEW
├── tests/output_edge_cases.rs  # the 009 overshoot test's doc comment corrected (D8)
└── tests/ffi_parity.rs, alloc_free.rs (output_clamp_cases), determinism.rs (output_clamp_partitions), timing.rs (output_clamp_cost), mutation.rs, us2_cross_check.rs (readme_output_clamp_statements)  # extended

.github/workflows/ci.yml (lines 81, 140, 152: golden_output_clamp), scripts/ios-device-golden.sh (golden_output, golden_output_clamp)
docs/hardware/assumptions.md (A-042 amended), README.md (the output section; the record-path paragraph at line 338), CHANGELOG.md (0.10.0, three regions), CLAUDE.md, Cargo.toml (0.10.0 × 4)
```

**Structure Decision**: the existing three-crate workspace; no new module in the core (two fields, a function and a wrapper in `stages/output.rs`), one new check module and four test files in the harness, matching 009's layout one for one.

## Task order (for /speckit-tasks)

1. **Phase 1, Setup**: **the version bump first** (`[workspace.package]` and the three internal dependency entries in `Cargo.toml`; `RR_DR60_VERSION_MINOR = 10`; the header regenerated; the canonical text re-blessed for the version line only, `RR_DR60_BLESS=manifest …`; the three `<!-- 0.10.0:start/end -->` CHANGELOG regions opened as a skeleton), then `stimulus::square` with its unit test, `analysis::impulse_l1`, `configs::bypass_route_output`, `OUTPUT_CLAMP_CONFIGS`, `output_clamp_twin`, `output_clamp_route_volume`, `OutputSettings::clamp_enabled` (both presets `false`, the field inert), the `default_output_ref_clamp` twin's counts measured and recorded in data-model.md. Checkpoint: `cargo test --all-features` green with the option present and inert; the five golden tests green; the canonical text re-blessed once.
2. **Phase 2, US1**: `output_clamp_checks::identity` written and run **red** against the unchanged chain (it fails on the overshooting stimuli) → the core (`bound`, the gate, the two hook fields, the unit tests incl. ±∞ / NaN / −0.0, the tap gating, the player) → `us1_output_clamp` (the identity on the primary and secondary stimuli, the taps on all three stimuli, the earlier taps, the bypassed chain, the reference configuration's zero counts), `output_clamp_matrix` (R-04's tables re-printed with the 18 cells, the quiet rows, "% bounded", the L1 bound), `output_clamp_edge_cases`. Checkpoint: the identity at the seven rates on both routes; the five SHA guards unmoved.
3. **Phase 3, US2**: the equivalence with the option on (four named positions, engaged at position 1), the change-timing identity, `stored_form_unchanged`, `latencies_and_drain_unchanged`, `determinism::output_clamp_partitions`, `alloc_free::output_clamp_cases`, `mutation` (the three mutants), `timing::output_clamp_cost`. Checkpoint: record-then-play equals the one-pass; the export bytes identical; the latencies unchanged.
4. **Phase 4, US3**: A-042 amended (R-07); the README output section (headroom scaling first, then the option) and the record-path paragraph; the `output_edge_cases.rs` doc comment; the CHANGELOG regions filled; `readme_output_clamp_statements`; the traceability audit. Checkpoint: `us1_manifest`, `us2_cross_check`, the audit; no second canonical re-bless.
5. **Phase 5, US4**: FFI (the three fields, the const assertions, the conversion, the default, the tap docs, the layout tests), the header, `smoke.c` (three cases) and `smoke_reference.rs` (two hashes), `configs::c_settings`, `ffi_parity`. Checkpoint: the C loop.
6. **Phase 6, Records and gates**: `ci.yml` and `scripts/ios-device-golden.sh`; the full gate (fmt, clippy, tests, the MSRV test build with `-D warnings`, coverage, the header drift check); the solo `output_clamp_cost` run quoted; the post-implementation hardware review (`hw-emulation-reviewer`, Opus) and the coach review (Opus; the SC-008 reader test); **the golden bless, once, after the reviews**, its SHA pinned; CLAUDE.md's 010 line and the bless/report commands; the PR (ask first).

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| The bound gated on the tap at construction, inside `OutputChain`, rather than a bool in `PlaybackPath` applied after `narrow_out` | FR-003: only the route-output tap is bounded, and the one-pass returns the configured tap from the same `process` call; the chain already owns the tap decision (`honour_tap`, `tap`), so the gate costs one bool beside it | A second copy of the decision in `PlaybackPath` would be equally cheap but would split FR-003's rule across two types |
| Two reserved fields in the C struct (104 → 112) for one bool, plus three const assertions | FR-008 (coach C3, P1; hardware D1): the struct is 8-aligned (`seed: u64`); a bool appended alone sits in 0.9's tail padding and leaves the size at 104, and a short tail would leave 108 – 111 as implicit padding one version later; both let a stale caller be accepted and an uninitialized byte be read as the option (undefined behaviour) | Validating `reserved_009` as zero and placing the bool there (changes a documented "ignored" field's contract and does not stop the next version's padding problem) |
