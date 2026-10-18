# Implementation Plan: Playback Speed (the device's five playback speeds, in the playback path)

**Branch**: `013-playback-speed` | **Date**: 2026-10-17 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/013-playback-speed/spec.md`

## Summary

The player gains a **speed control**: a device-rate resampler between the speech decoder (stage 9) and stage 10, run only by `play()`. Level k/5 (k = 3 … 7) plays at speed r = k/5; device output n sits at **place** p_n = n·k/5 stored samples, kept exactly as an integer count of fifths (research R-02). At levels 1, 2, 4 and 5 the value at a place is a 57-tap dot product (four partial sums in a fixed order) of one of five polyphase phases of a per-level 281-tap Kaiser prototype (70 dB, 30 kHz prototype rate, band edges per level, designed at construction by the existing deterministic designer, each phase normalized to unit DC by `resample::design::polyphase_split`) with the stored samples around the place, **centred on it** by reading 28 stored samples ahead (R-03), so the speed control adds no latency and a change of level leaves the place where it is. Level 3 passes the stored samples unaltered and reads nothing ahead, so every playback at level 3, every play report and the eight golden files are bit-identical to 0.12 (R-05). After the recording's own part (⌈K ÷ r⌉ device samples) the speed control plays its own tail (T ≤ 46 device samples, 0 at level 3) **before the unchanged drain**; the player's total length becomes dynamic, ⌈(K′ + T) · m ÷ l⌉ + drain, fixed (as built) at the latch, when the place reaches K, equal to 0.12's at level 3 (R-04, R-05). The level latches when the place reaches K; a change into level 3 waits until the place is whole (at most 4 outputs, R-02). The reference script `tools/playback-speed/speed_design.py` (written at plan time) prints every figure: passband deviation ≤ 0.0025 dB, worst spurious −72.5 … −73.9 dB against the 60 dB floor (the slow levels' passband 2550 Hz), tails 46 / 34 / 23 / 20 device samples, impulse peaks at the nearest output, and the banks' cutoff, energy and moment the core pins within 1e-12. New public items: `Pipeline::{set_playback_speed, playback_speed, playback_length_samples}`, `rr_dr60::speed` constants, `Error::InvalidSpeedLevel`, `PlayInfo::place`; C status 25, three functions, the play report grows 32 → 40 bytes; the settings struct stays 120. Version 0.13.0; stage 9's manifest entry cites A-054 at model version 0.13.0 (one re-bless); one new golden file (R-09).

## Technical Context

**Language/Version**: Rust, edition 2024, MSRV 1.85 (001 R-01); Python 3.11 with numpy via `uv` for the reference script (as 004, 008, 009, 011, 012).

**Primary Dependencies**: none new.

**Storage**: unchanged: the memory, the stored form version 2, export and import untouched (the speed control is after the memory).

**Testing**:
- **Core**: unit tests in `stages/speed.rs` (the bank's figures against the script: passband, spurious at 6 kHz on a few pinned tones, impulse peak and symmetry, tail length per level and K mod k, the place in fifths, the deferred switch into level 3, the latch at K, unit DC, `ceil(5K/k)`, no look-ahead at level 3), `transport.rs` (the dynamic total equals 0.12's at level 3 on the existing `playback_produces_ceil_k_m_over_l_then_drain` and `play_reports_frames_and_index` cases unchanged; the tail inside the drain; `PlayInfo::place`; the length helper), `pipeline.rs` (the control: set, read, refuse, kept by reset and reconfigure).
- **Harness**: `speed_checks.rs` (R-08, with the independent reference `reference_output`) and the tests `us1_speed` (US1 at `measurement_rates()`; FR-004 at 6 kHz, the ±0.1 dB level comparison at the measurement rates), `us2_speed` (US2: every schedule bit-exact against the independent reference in `speed_only` and `speed_only_codec`, the deferred switch, rapid changes, the latch, scope), `speed_matrix` (the seven-rate report: lengths, fits, spurious, impulse, tails, the level-change step), `speed_edge_cases`, `golden_speed`; `ffi_parity`, `determinism`, `alloc_free`, `mutation` (six mutants), `timing` (`speed_cost`, `speed_codec_worst_block`) extended; a new `playback_timing` test binary for the build A/B (R-07); `analysis::sine_fit`, `stimulus` import helpers, `configs::speed_only`.
- **Golden**: the eight existing files untouched (SHA guards); `golden-speed-v1.json` blessed once at the end, after the gate without it and before the gate with coverage (010's order).
- **Coverage**: gate 80 %; within 1 point of 0.12's figure (97.96 % on CI).

**Target Platform**: unchanged: 8 targets; the golden matrix proves bit-identity on six.

**Project Type**: library: a Rust crate plus a C ABI.

**Performance Goals**: FR-010: codec bypassed at 48 kHz on the speaker route, release, the fastest of 45 strictly alternating runs each side: level 3 at most **+3 %** against 0.12 (an A/B of the `3a4eefb` build, 0.12's merge, and the branch with the new `#[ignore]`d `playback_timing` binary overlaid on the baseline; 45 rounds; a 5 % ceiling on the CI runner by the PR's `ab-timing` label, `ab-timing.yml` gaining the step); levels 1, 2, 4, 5 at most **+15 %** against level 3 in one build (`timing::speed_cost`, `RR_DR60_SPEED_COST_CEILING = 25` on the runner), the 6 kHz figure reported; codec on, level 5 at most **1.5×** level 3 per host sample (`RR_DR60_SPEED_CODEC_RATIO_CEILING = 2.0` on the runner) and the worst 64- and 4096-sample block at 6 and 48 kHz with the level changed before every block below the block's real-time duration (`timing::speed_codec_worst_block`). Expected (R-07), against the play-only base of about 24 ns per host sample: +2 … +5 % at the non-3 levels; under 1 % at level 3.

**Constraints**:
- Bit-identity to 0.12 at level 3 (FR-006): no look-ahead at level 3, the dynamic total equal to 0.12's precomputed one with the drain-0 stop (R-05's proof; `playback_produces_ceil_k_m_over_l_then_drain`, `play_reports_frames_and_index` and `drain_zero_when_stage10_off_at_6k` unchanged), the eight SHA guards checked in Phase 1 and after the core change.
- Determinism: the banks built at construction by the existing deterministic `design::kaiser_lowpass` (detmath) and `polyphase_split`; the dot product as four partial sums in a fixed order in `f64`; the place an integer, never derived from `speed::ratio`; no platform math function; no `f64::clamp` (R-03).
- No allocation per block (the banks and the ring at construction); no `unsafe` outside `rr_dr60_ffi`; no new setting; the C settings struct stays 120 bytes.
- The manifest's stage 9 changes in its description, sources, assumptions, controls, coverage note (≤ 120 characters) and model version; stage 10 in its coverage note only; one re-bless of the canonical text and the README table with the version (Phase 1), after the wording review of the changed strings.
- Wording (R-10): "the playback speed", "the speed control", "level"; never "accurate" or "measured" (except "not measured"); no claim about what slowing reveals.

**Scale/Scope**: about 450 lines of core (`stages/speed.rs` ≈ 300 with tests; `speed.rs` ≈ 60, the public constants; `transport.rs` ≈ 80 changed: the source refactor, the dynamic total, `place`, the length helper; `pipeline.rs` the control, docs; `error.rs` one variant; `lib.rs` the module, the hooks), about 80 lines of FFI (three functions, the status, three defines, the play report's field, the header), about 900 lines of harness (`speed_checks.rs` ≈ 400, five test files ≈ 400, extensions, `golden.rs` speed generator, `configs`, `analysis::sine_fit`), `smoke.c` one case and `smoke_reference.rs` one hash, `tools/playback-speed/speed_design.py` (written at plan time), `scripts/ab-timing.sh` (a playback mode), docs (A-054, S-018, signal-chain row 9, README, CHANGELOG, CLAUDE.md), one new golden file, `ci.yml` and `scripts/ios-device-golden.sh`.

There are no open NEEDS CLARIFICATION items.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | How this plan complies | Pre-research | Post-design |
|---|---|---|---|
| I. Portable Library First | No new dependency; the core stays `no_std` + `alloc`; the C API gains three functions, a status and a field at the end of the play report (a compile break recorded in the CHANGELOG, as 008's); the settings struct unchanged; the header regenerated and drift-checked; the smoke loop gains a speed case (R-06). | ✅ | ✅ |
| II. Evidence-Based Fidelity | The speed control traces to S-001, S-002, S-003, S-012 and a new S-018 and to a new assumption A-054 (both readings, mechanism (b) as the labelled default, the clean resampler as the emulator's choice, capture P1 with its decision rule); stage 9's manifest entry cites them at model version 0.13.0; CHANGELOG entries (R-10). Every filter figure comes from the reference script, not the Rust (005's lesson). | ✅ | ✅ |
| III. Measured, Test-First Verification | Every behavior has a measurable check with a tolerance: the lengths (exact), the place (exact, fifths), the continuity oracle (bit-exact), level 3 (bit-exact), FR-004 (±0.1 dB, −60 dB, pinned to the script within 0.5 dB), the impulse peak (exact), the tail inside the drain; the checks are written and run red before the core (Phase 2's first task); six mutants; the eight goldens guard the earlier specs and the feature has its own file. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | Bounded per-sample work (57 multiply-adds at most, at most one frame decode per 120 stored samples, at most 29 extra stored samples pulled when leaving level 3), no allocation, no clock, no platform math; partition independence at a constant level asserted; `alloc_free` and `determinism` extended (R-03, R-04). | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | The speed control is part of stage 9's manifest entry (the DSP's playback firmware); level 3 is its bypass (the identity). It has no tap: the player honours none (spec 006); recorded in Complexity Tracking. | ✅ | ✅ (with the recorded item) |
| VI. Neutral, Honest Communication | "Modeled on" the documents; mechanism (b) labelled the owner's assumed default; the weak lean toward a pitch-keeping device stated; no claim about what slowing reveals (R-10). | ✅ | ✅ |
| VII. Independently Testable Features | US1 with imported synthetic stimuli and the speed-only configuration (codec, stage 10 and the output chain bypassed, 6 kHz), oracles the stimulus itself and level 3; US2 with constant-level playbacks from the same place as bit-exact oracles; US3 by the cross-check, the audit and the C parity; the version bump, A-054 and the manifest re-bless sit in Phase 1 so no story depends on another. | ✅ | ✅ |
| Repo standards | rustdoc on every new public item; SemVer 0.13.0 (a new control and a report field; minor before 1.0); Conventional Commits; no audio committed. | ✅ | ✅ |

**Gate result**: PASS. One complexity item (and one design note) is recorded below.

## Project Structure

### Documentation (this feature)

```text
specs/013-playback-speed/
├── plan.md              # This file
├── research.md          # R-01 – R-12 (placement, the place in fifths, the filter, the ring and the tail, level-3 identity and the dynamic total, the API, cost, oracles and measurement, golden, records, the P1 figures, risks)
├── data-model.md        # the control, SpeedStage's fields, the state machine, the per-output step, the transport's changes, configurations, stimuli, rows
├── quickstart.md        # the validation runs per story and the gates
├── contracts/
│   ├── rust-api.md      # Pipeline::{set_playback_speed, playback_speed, playback_length_samples}, rr_dr60::speed, Error::InvalidSpeedLevel, PlayInfo::place, the test hooks
│   ├── c-api.md         # status 25, three functions, three defines, RrDr60PlayInfo.place (32 → 40 bytes), VERSION_MINOR 13, the smoke case, parity
│   └── golden-format.md # golden-speed-v1.json: entries, fields, bless-time assertions
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks output (not created by /speckit-plan)
```

### Source Code (repository root)

```text
tools/playback-speed/speed_design.py   # NEW (plan time): every design figure; --check

crates/rr_dr60/src/
├── speed.rs                    # NEW public module: LEVEL_COUNT, DEFAULT_LEVEL, RATIO_DENOMINATOR, RATIO_NUMERATORS, ratio(), LOOKAHEAD_SAMPLES, MAX_TAIL_SAMPLES
├── stages/speed.rs             # NEW: SpeedStage (banks, ring, place, level, latch, tail), SpeedOut; unit tests
├── stages/mod.rs               # pub(crate) mod speed
├── transport.rs                # the source pull refactored; the dynamic total; PlayInfo.place; frame_index from the place; start_playback resets the speed control; playback_length_samples
├── pipeline.rs                 # Pipeline { speed: SpeedStage, speed_level: u8 }; set_playback_speed, playback_speed; new/reconfigure build the stage; rustdoc
├── error.rs                    # Error::InvalidSpeedLevel { level }
├── lib.rs                      # pub mod speed; __test_hooks::{SpeedMutation, speed_bank, speed_tail_len, speed_place_fifths}; crate docs
└── manifest.rs                 # stage 9: description, sources, A-054, controls, coverage, 0.13.0; stage 10: coverage

crates/rr_dr60_ffi/
├── src/lib.rs                  # RrDr60Status::InvalidSpeedLevel = 25; rr_dr60_set_playback_speed, rr_dr60_get_playback_speed, rr_dr60_playback_length_samples; RR_DR60_SPEED_*; RrDr60PlayInfo.place; VERSION_MINOR 13
├── include/rr_dr60.h           # regenerated
└── tests/c/smoke.c             # one speed case (RR_DR60_SMOKE_SPEED_HASH)

crates/rr_dr60_harness/
├── src/speed_checks.rs         # NEW (R-08)
├── src/analysis.rs             # sine_fit (least squares at a known frequency), spurious_peak_db
├── src/stimulus.rs             # import helpers: a recording from given device samples (the samples form)
├── src/configs.rs              # speed_only, speed_only_codec (stage 10 and the output chain bypassed; the codec off / on), speed_default, speed_default_no_codec
├── src/golden.rs               # SPEED_*; GoldenEntry.{speed_level, speed_changes, place, own_length}; generate_speed, committed_speed, path_speed
├── src/smoke_reference.rs      # SMOKE_SPEED_HASH
├── golden/golden-speed-v1.json # NEW, blessed once
├── golden/manifest-v1.txt      # re-blessed once in Phase 1
├── tests/us1_speed.rs, us2_speed.rs, speed_matrix.rs, speed_edge_cases.rs, golden_speed.rs, playback_timing.rs   # NEW
└── tests/ffi_parity.rs, determinism.rs, alloc_free.rs, timing.rs, mutation.rs, us2_cross_check.rs (readme_speed_statements)  # extended

scripts/ab-timing.sh (a playback mode, 45 rounds), .github/workflows/ab-timing.yml (the playback step), .github/workflows/ci.yml (golden_speed in three places; RR_DR60_SPEED_COST_CEILING, RR_DR60_SPEED_CODEC_RATIO_CEILING), scripts/ios-device-golden.sh, scripts/check-traceability.sh (speed_checks.rs)
docs/hardware/assumptions.md (A-054), docs/hardware/sources.md (S-018), docs/hardware/signal-chain.md (row 9), README.md (a playback-speed section), CHANGELOG.md (0.13.0), CLAUDE.md, Cargo.toml (0.13.0 × 4)
```

**Structure Decision**: the existing three-crate workspace; one new core stage (`stages/speed.rs`) owned by the `Pipeline` beside the player's decoder, used only by `play()`, and one small public constants module, matching 009's `output` / `stages::output` split; one new check module and six test files in the harness.

## Task order (for /speckit-tasks)

1. **Phase 1, Setup**: the version bump (`Cargo.toml` × 4, `RR_DR60_VERSION_MINOR = 13`, the header); **A-054's row** and **S-018's row** in the registers; stage 9's and stage 10's manifest strings with **the wording review and the denylist run on the changed strings first**; the CHANGELOG 0.13.0 regions as a skeleton; **one re-bless** of the canonical text and the README table (the records run ahead of the code until Phase 2: the Phase 1 commit says so); `rr_dr60::speed` constants and `Error::InvalidSpeedLevel` (inert); `analysis::sine_fit`, `spurious_peak_db`, the stimulus import helper and `configs::speed_only` with unit tests; the eight golden tests and the C smoke loop run. Checkpoint: `cargo test --all-features` green; the eight SHA guards unmoved; the cross-check and the audit green.
2. **Phase 2, US1**: a stub surface first (`set_playback_speed` / `playback_speed` storing the level, `playback_length_samples` returning level 3's length, `PlayInfo::place` from the level-3 cursor, `speed_bank` and `speed_tail_len` hooks returning the designed banks and T) so the checks compile; `speed_checks` (lengths, FR-004 (a) – (c), impulse, the independent reference and the tail, level-3 identity) written and run: **red** on the lengths, the fits at r × f, the impulse and the reference at levels ≠ 3; level 3 green by construction (stated, not misread as all red) → the core (`SpeedStage`, the transport refactor, the dynamic total with the drain-0 stop, the latch, `place`, the control, the length helper, unit tests against the script's pins) → `us1_speed`, `speed_matrix`, `speed_edge_cases` (the constant-level rows). Checkpoint: US1 at the seven rates; the eight SHA guards unmoved; the three named transport tests unchanged.
3. **Phase 3, US2**: `us2_speed` written first and run **red** against Phase 2's core with the live change not yet applied mid-playback (changes ignored until the next playback): every schedule against the independent reference in `speed_only` and `speed_only_codec`, the places, rapid changes, a change after the end; then the live change (the requested level per call, the deferred switch into level 3) → green; reset, reconfigure, stop, erase, route, volume, clamp; the one-pass and record path unchanged; `determinism::speed_partitions`, `alloc_free`, `mutation` (six mutants), `timing::{speed_cost, speed_codec_worst_block}`, the `playback_timing` binary and `scripts/ab-timing.sh`'s playback mode against `3a4eefb`, `ab-timing.yml`'s step; `golden.rs::generate_speed` and `tests/golden_speed.rs` written (**red by design** until the Phase 6 bless, excluded from the gate as 010 did). Checkpoint: the oracles bit-exact; the costs within budget.
4. **Phase 4, US3**: README, rustdoc, A-054's verification text complete, signal-chain row 9, the CHANGELOG regions filled, `readme_speed_statements`, the audit. Checkpoint: `us1_manifest`, `us2_cross_check`, the audit; no second canonical re-bless.
5. **Phase 5, C**: the status, the three functions, the defines, `RrDr60PlayInfo.place`, the header, `smoke.c`'s case and `smoke_reference.rs`'s hash, `ffi_parity`'s cases. Checkpoint: the C loop.
6. **Phase 6, Records and gates** (010's gate order; first the rebase onto `main` if PR #31 has merged, otherwise the PR targets `research/noise-clock-power`): the gate **without** `golden_speed` (fmt, clippy, tests, the MSRV test build with `-D warnings`, the header drift check, `speed_design.py --check`); the solo timing runs and the development-machine A/B quoted; the post-implementation hardware review and the coach review (the SC-008 reader test), both on the session's model; **the golden bless, once, after the reviews**, its SHA pinned, in the same commit as `ci.yml` and `scripts/ios-device-golden.sh`; then the full gate **with** coverage; CLAUDE.md's 013 line and the commands; push and a **draft** PR with the CI runner's A/B by the `ab-timing` label (the owner is away: the PR stays a draft and nothing is merged until the owner says so).

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| No tap after the speed control (constitution V's "tap-able") | The speed control runs only in the player, which honours no tap (spec 006: taps belong to the one-pass processing, which the speed control does not touch, FR-008); its output is observable through the speed-only configuration (stage 10 and the output chain bypassed), which plays the device stream at 6 kHz | A new `Tap` that the one-pass would have to ignore (a tap that is never honoured where taps apply); making the one-pass run the speed control (contradicts FR-008 and the device, which changes speed only in playback) |
| (A design note, not a principle deviation) A stage owned by `Pipeline` outside `PlaybackPath` | The speed control must sit between the stored-sample source (the memory and the decoder, owned by `Pipeline`) and `PlaybackPath::push_device`, and must pull stored samples ahead on demand; `PlaybackPath` is shared with the one-pass processing, which has no stored source | Inside `PlaybackPath` (it would need the memory and the decoder, or a push interface that cannot read ahead); a push-model resampler with a delay (adds latency and breaks FR-008 and the level-3 identity) |
