---

description: "Task list for 003 Voice Activated System (VAS) on the Record Path"
---

# Tasks: Voice Activated System (VAS) on the Record Path

**Input**: Design documents from `specs/003-vas/`: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: REQUIRED. Constitution III says measurement tests are written first and must fail before the implementation exists. Every phase lists its tests before its implementation tasks.

**Organization**: Setup → Foundational (settings, validation, BlockInfo and the `#[must_use]` sweep, the emission schedule, keeping specs 001 and 002 intact) → US1 (P1, MVP: the VAS stage in drop mode, with events) → US2 (P2: tune, bypass, tap, mute, C interface) → US3 (P3: full measurement harness and golden files) → CI → Polish.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: The user story the task belongs to (US1, US2, US3)
- Paths are relative to the repository root.

## Conventions that apply to every task

- **Traceability**:
  - Cite spec 003 FR, SC and A-/S-IDs in code comments, rustdoc and test names, e.g. `// A-023: hang time 1.0 s`.
  - Detector and harness constants are commented `// engineering target (003 R-15)`. Spec tolerances are commented `// engineering target (003 FR-017)`.
  - Harness results cite requirements as `003/FR-0xx`.
- **Determinism**: in `rr_dr60`, use only basic IEEE operations and integer arithmetic, plus `rr_dr60_detmath::exp` at construction. No platform libm (`clippy.toml` enforces this).
- **Real-time safety**: no allocation, formatting or I/O in `process`, `process_in_place`, `process_with_events`, `max_events` or `reset`. VAS state is fixed-size and inline.
- **Specs 001 and 002 must stay intact**:
  - `crates/rr_dr60_harness/golden/golden-v1.json` and `golden-agc-v1.json` must never change.
  - Spec 001 and 002 expected values and tolerances must never change. Only their settings change, to bypass VAS (FR-020).
- **Story boundaries (Principle VII, plan › Story boundaries)**: `us1_vas.rs` and `us2_vas_settings.rs` use plain assertions and must not import `crates/rr_dr60_harness/src/vas_checks.rs`, which belongs to US3.
- **Before ticking a task**: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all-features` must pass. The only exception is a test written to fail first, and only until its implementation task is done.
- **Commits**: Conventional Commits, on branch `003-vas`. Merge to `main` only through a green PR, and only after the user says so.

---

## Phase 1: Setup

**Purpose**: Version bump and the guard that keeps spec 002's golden file unchanged.

- [X] T001 Bump the workspace version to 0.3.0:
  - `version = "0.3.0"` in `[workspace.package]` and in the three internal dependency entries of `Cargo.toml`.
  - `RR_DR60_VERSION_MINOR = 3` in `crates/rr_dr60_ffi/src/lib.rs`.
  - Regenerate `crates/rr_dr60_ffi/include/rr_dr60.h` (see `crates/rr_dr60_ffi/cbindgen.toml`).
  - Golden comparison ignores `library_version`, so the 001 and 002 golden tests must still pass.
- [X] T002 [P] Add `golden_agc_v1_unchanged` to `crates/rr_dr60_harness/tests/golden_agc.rs`: the SHA-256 of `include_bytes!("../golden/golden-agc-v1.json")` equals a constant recorded from the file as committed on `main`, as `golden_v1_unchanged` does (FR-019, FR-020; research R-10).

**Checkpoint**: the workspace builds at 0.3.0, and both golden-file guards pass.

---

## Phase 2: Foundational (blocking prerequisites)

**Purpose**: The VAS settings and validation surface, `BlockInfo` on every processing call (with the call-site sweep), the variable-length emission schedule, the C struct fields, the harness helpers, and moving every spec 001 and 002 test onto VAS-bypassed settings. When this phase is done, the pipeline accepts VAS settings and can carry variable-length output, but no VAS stage exists yet, so every block still produces as many samples as it consumes. All 001 and 002 tests stay green.

### Tests (write first, must fail)

- [X] T003 [P] Settings tests in the `#[cfg(test)]` module of `crates/rr_dr60/src/settings.rs`. Assert:
  - **Defaults**: `VasSettings::DEVICE` and `VasSettings::default()` are `enabled: true` (S-001, A-008), `mode: VasMode::Drop`, `sensitivity: 3` (S-001, A-021), `threshold_dbfs: -18.0` (A-022), `hang_ms: 1000.0` (A-023) and `onset_ms: 20.0` (A-024).
  - `VasMode::default()` is `Drop`.
  - `Settings::new(48_000).vas == VasSettings::DEVICE`.
  - `Tap::AfterVas` exists, and `Tap::default()` is still `AfterPlayback`.
- [X] T004 [P] Validation tests in the test module of `crates/rr_dr60/src/validate.rs`. For each VAS field, use the data-model ranges quoted verbatim:
  - `sensitivity` "1 … 5"
  - `threshold_dbfs` "−60.0 … 0.0"
  - `hang_ms` "50.0 … 10000.0"
  - `onset_ms` "0.0 … 200.0"

  Assert:
  - **Boundaries**: both inclusive bounds are accepted. For the floats, the next representable f32 outside each bound, NaN, +Inf and −Inf are rejected. For `sensitivity`, 0 and 6 are rejected. Each rejection is `Error::InvalidSetting { setting: Setting::Vas… }` naming that field.
  - **When bypassed**: validation still happens with `enabled = false`, and in mute mode.
  - **Order**: host rate, then the AGC fields, then the VAS fields in struct order (`sensitivity`, `threshold_dbfs`, `hang_ms`, `onset_ms`).
  - **Combinations**: an onset longer than the hang (onset 200, hang 50) is accepted.
  - **Display**: each new `Setting` `Display` contains the field name (e.g. `vas.hang_ms`) and its range.
- [X] T005 [P] Pipeline tests in the test module of `crates/rr_dr60/src/pipeline.rs`:
  - `Pipeline::new` with `vas.sensitivity = 6` returns `Err(Error::InvalidSetting { setting: Setting::VasSensitivity })`.
  - `reconfigure` with an invalid VAS setting returns the error and leaves `settings()`, `latency_samples()` and the processing state unchanged. Check by comparing output with an un-reconfigured clone.
  - **`BlockInfo` without a VAS stage**: for every tap and rate, `process` and `process_in_place` return `produced == input.len()`, `events == 0` and `paused == false`.
  - **Stage gating**: with `Tap::AfterVas` and stage 10 enabled, output equals the stage-10-bypassed output.
- [X] T006 [P] Emission-schedule tests in the test module of `crates/rr_dr60/src/resample/up.rs` (research R-05). Drive the decimator and interpolator directly at all five non-identity plans:
  - **No drops**: feeding every device sample, the new emission rule emits exactly one host sample per input step, with values bit-identical to the current `next_host` path, over 200 000 steps.
  - **With drops**: with seeded random pauses (some one device sample long), no step emits more than one sample. Over the run, input length − output length = ⌊dropped·m/l⌋ within ±1 (exact at 16, 48 and 96 kHz).
  - **Ring buffer**: the writer never leads the newest-read index by more than today's maximum (1 for l = 1, 2 at 44.1 and 88.2 kHz).
  - **Partition independence**: the emission times don't depend on any block split, because the rule is per step.
- [X] T007 [P] Harness stimulus tests in `crates/rr_dr60_harness/src/stimulus.rs`:
  - `burst_gap(freq, level_dbfs, segments: &[(on: bool, secs)], fs)` and `short_bursts(...)` produce the exact sample counts requested.
  - Their bursts start at phase 0 and are digital silence (+0.0) in gaps.
  - Two calls give identical bits.

### Implementation

- [X] T008 Implement in `crates/rr_dr60/src/settings.rs` (contracts/rust-api.md):
  - **Types**:
    - `#[non_exhaustive] pub enum VasMode { #[default] Drop, Mute }`;
    - `#[non_exhaustive] pub struct VasSettings { enabled, mode, sensitivity: u8, threshold_dbfs, hang_ms, onset_ms }`, deriving `Clone, Copy, Debug, PartialEq`, with `pub const DEVICE` and `Default`;
    - `Settings.vas`;
    - `Tap::AfterVas`.
  - **Rustdoc**: cites S-001, A-008 and A-021 – A-025. It says mute mode is an emulator option, not device behavior, and that in drop mode the output can be shorter than the input, so hosts must read `BlockInfo::produced`.
  - Makes T003 pass.
- [X] T009 Implement in `crates/rr_dr60/src/error.rs` and `crates/rr_dr60/src/validate.rs`:
  - **New `Setting` variants**: `VasSensitivity`, `VasThresholdDbfs`, `VasHangMs` and `VasOnsetMs`, with ranges and `Display` names as in T004, each commented `// engineering target (003 FR-012)`.
  - **Validator**: checks the VAS fields after the AGC fields, with inclusive ranges and non-finite values rejected.
  - **Re-exports**: `VasSettings` and `VasMode` from `crates/rr_dr60/src/lib.rs`.
  - Makes T004 and the validation parts of T005 pass.
- [X] T010 Add `BlockInfo` and `VasEvent` to `crates/rr_dr60/src/pipeline.rs` and re-export both from `lib.rs` (contracts/rust-api.md).
  - **`BlockInfo`**: `#[must_use] #[non_exhaustive] pub struct BlockInfo { produced: usize, events: usize, paused: bool }`.
  - **`VasEvent`**: `#[non_exhaustive] pub struct VasEvent { output_position: u64, input_length: u64 }`.
  - **Signatures**: change `process` to return `Result<BlockInfo, Error>` and `process_in_place` to return `BlockInfo`. Both still produce one sample per input here.
  - **Sweep**: update every call site (research R-07; 67 call sites in 19 files):
    - core: `pipeline.rs`, `settings.rs` doc-tests;
    - FFI: `crates/rr_dr60_ffi/src/lib.rs`;
    - harness: `crates/rr_dr60_harness/src/{analysis,checks,golden,agc_checks}.rs`;
    - harness tests: `crates/rr_dr60_harness/tests/{agc_edge_cases,alloc_free,determinism,edge_cases,ffi_parity,timing,us1_agc,us1_voiceband,us2_agc_settings,us2_bypass_tap}.rs`;
    - the `README.md` snippet.

    Harness helpers that size output by input length must truncate to `produced` (e.g. `y.truncate(info.produced)`), so later VAS tests can reuse them. `clippy -D warnings` must be clean.
  - Makes the `BlockInfo` part of T005 pass.
- [X] T011 Implement the emission schedule in `crates/rr_dr60/src/resample/up.rs` (research R-05, data-model › Emission schedule state):
  - **Counters**: add `kept` and `emitted` counters and `fn try_next_host(&mut self) -> Option<f64>`. It emits only when `kept ≥ ⌊emitted·l/m⌋ + 1`, computed with integer phase bookkeeping.
  - **Pipeline**: change `Pipeline::tick` in `crates/rr_dr60/src/pipeline.rs` to write output only when a sample is emitted, and to count `produced`.
  - **Identity plan**: emits each kept device sample immediately.
  - **Reset**: `reset()` clears the counters.
  - **Constraint**: no output may change. Every 001 and 002 test and golden file must stay green.
  - Makes T006 pass.
- [X] T012 Update `DeviceChain::new` in `crates/rr_dr60/src/pipeline.rs` with the stage gating from research R-01: `run_vas = vas.enabled && tap ∈ {AfterVas, AfterPlayback}` and `run_playback = playback_stage_enabled && tap == AfterPlayback`. The VAS stage itself arrives in T023; until then `run_vas` gates nothing. Latency is unchanged (FR-011). Makes the gating part of T005 pass.
- [X] T013 [P] Implement the stimuli that T007 tests in `crates/rr_dr60_harness/src/stimulus.rs`: `burst_gap` and `short_bursts`, using detmath tones only. These are Foundational (plan › Story boundaries).
- [X] T014 Move every spec 001 and 002 harness configuration onto VAS-bypassed settings in `crates/rr_dr60_harness/src/configs.rs`:
  - **Existing names**: `settings()` sets `s.vas.enabled = false` for every 001 name and every 002 name, including `default_agc`, which is built from `Settings::new`. Add a doc comment: each means "the spec 001/002 configuration with VAS bypassed (spec 003 FR-020)".
  - **New names**: `vas_only` (VAS on in drop mode, AGC off, stages 4 and 10 off, `Tap::AfterVas`), `vas_mute` (as `vas_only`, mode `Mute`) and `default_vas` (`Settings::new`).
  - **Lists**: keep `CONFIGS` and `AGC_CONFIGS` unchanged, and add `VAS_CONFIGS`.
  - Extend `configs_match_data_model_table`.
- [X] T015 Extend `RrDr60Settings` and the process call in `crates/rr_dr60_ffi/src/lib.rs` (contracts/c-api.md):
  - **Settings fields**: append `vas_enabled: bool, vas_mode: u32, vas_sensitivity: u32, vas_threshold_dbfs: f32, vas_hang_ms: f32, vas_onset_ms: f32` at offsets 48–68, size 72. Add `const _: () = assert!(size_of::<RrDr60Settings>() == 72);`. `rr_dr60_settings_default` fills the device defaults.
  - **New types**: `RrDr60VasMode { Drop = 0, Mute = 1 }`, `RrDr60Tap::AfterVas = 3`, and `RrDr60SettingField` values 9–13 (`VAS_MODE`, `VAS_SENSITIVITY`, `VAS_THRESHOLD_DBFS`, `VAS_HANG_MS`, `VAS_ONSET_MS`).
  - **Conversion rules**:
    - `to_settings` returns the failing field (so an invalid `vas_mode` reports `VAS_MODE`, not `TAP`);
    - `vas_sensitivity` is converted with `u8::try_from(…).unwrap_or(u8::MAX)`;
    - the core-error mapping has an arm for each of the four VAS `Setting` variants.
  - **`RrDr60BlockInfo`**: add it, and add the `out_info` parameter to `rr_dr60_process`. On every early return (error, poisoned handle, `frames == 0`), `*out_info` = `{0, 0, current paused}`, with `paused = false` for a NULL handle.
  - Regenerate the header.
- [X] T016 Update the 001 and 002 tests that build settings by hand so they bypass VAS, and map every tap exhaustively:
  - `crates/rr_dr60_harness/tests/ffi_parity.rs` (`c_settings`) and `crates/rr_dr60_harness/tests/alloc_free.rs`: add `Tap::AfterVas => 3` to the exhaustive `match`, and copy the VAS fields.
  - Check `us1_voiceband.rs`, `us2_bypass_tap.rs`, `edge_cases.rs`, `timing.rs`, `determinism.rs`, `memory.rs`, `mutation.rs`, `us1_agc.rs`, `us2_agc_settings.rs`, `agc_edge_cases.rs`, `crates/rr_dr60_harness/src/agc_checks.rs` and `crates/rr_dr60_ffi/tests/c/` for `Settings::new` or `rr_dr60_settings_default` used as a 001 or 002 default. Each must set VAS off (`vas_enabled = false` in C).
  - Update `crates/rr_dr60_ffi/tests/c/smoke.c` for the new `rr_dr60_process` signature.
  - No expected value or tolerance may change.

**Checkpoint**:
- `cargo test --all-features` is green, including every spec 001 and 002 test, with unchanged expectations.
- `golden_v1_unchanged` and `golden_agc_v1_unchanged` pass, and `git diff main -- crates/rr_dr60_harness/golden/` is empty.
- The pipeline accepts and validates VAS settings and returns `BlockInfo`, but has no VAS stage yet.

---

## Phase 3: User Story 1 - Hear the RR-DR60's voice-activated recording (Priority: P1) 🎯 MVP

**Goal**: The VAS stage exists, on by default in drop mode. Long pauses shrink to the hang time, onsets after a pause lose the onset time, splices are abrupt, and each block reports its produced count, its splices and whether it ends paused.

**Independent Test**: `cargo test -p rr_dr60 stages::vas` and `cargo test -p rr_dr60_harness --test us1_vas` pass. They use only the core, the Phase 2 configurations and stimuli, and plain assertions, not the US3 harness.

### Tests for User Story 1 (write first, must fail)

- [X] T017 [P] [US1] Unit tests in the `#[cfg(test)]` module of the new `crates/rr_dr60/src/stages/vas.rs`. They drive `VasStage` directly at 8 kHz with detmath tones at −8 dBFS (threshold + 10 dB) and digital silence. Each case states its expected decision counts:
  - **Threshold** (FR-006, A-022): a steady 1 kHz tone at −15 dBFS (+3 dB) is kept for 10 s with no pause. At −21 dBFS (−3 dB), exactly H = 8000 samples are kept, then all are dropped. At 300 and 3400 Hz, the lowest kept level is within ±1 dB of the 1 kHz value.
  - **Sensitivity** (FR-007): the effective threshold at levels 1 and 5 is −12 and −24 dBFS (±0.05 dB, by bisection at 1 kHz phase 0).
  - **Hang** (FR-008, A-023): with silent gaps of H − 40, H, H + 1 and H + 40 samples, the kept gap is min(gap, H), with no pause for gap ≤ H.
  - **Onset** (FR-009, A-024; plan review finding 1):
    - after a pause, bursts of 80, 144 (O − 16, i.e. onset − 2 ms), 159, 160, 161, 240 and 8000 samples;
    - bursts of ≤ 160 samples leave no resume and nothing kept, including in the silence after them;
    - longer bursts keep exactly burst − 160 samples.
    - With `onset_ms = 0`, the first sound sample is kept. With `onset_ms = 2`, a one-sample click does not resume.
  - **W bridge** (R-02): a 300 Hz tone at the threshold + 3 dB, after a pause, resumes after exactly O + 1 samples from its first sound sample. A burst train with a 33-sample gap restarts the run.
  - **Start, reset** (A-025): a fresh stage keeps H samples of silence, then pauses. `reset()` makes the next decisions identical to a fresh stage.
  - **Counters** (plan review finding 16): 10 minutes of silence leaves `since_sound` at W + 1, with no overflow.
- [X] T018 [P] [US1] Pipeline event tests in the test module of `crates/rr_dr60/src/pipeline.rs`, with `vas_only`-equivalent settings built inline:
  - **8 kHz**: for burst 1 s, gap 5 s, burst 1 s, `produced` sums to 8000 + 8000 + 8000 − 160, with one event at `output_position` 16000 and `input_length` 32160 (40000 − 8000 dropped gap + 160 onset).
  - **48 kHz**: the same event's `input_length` is within ± one device sample (6 host samples) of exactly 6 × the 8 kHz value, labeled an engineering target (FR-017; research R-11 › Measured: the decimator's edge smear gives 6 × 32 161, not 6 × 32 160). The sum of removed lengths plus total output equals total input exactly (m/l is an integer).
  - **`process_with_events`**: with a too-short slice (length 0), `BlockInfo.events` still counts 1, and nothing is written. `max_events(frames)` ≥ the events of any block in a 60 s seeded burst-gap run at every rate with `hang_ms = 50` (hundreds of splices; kept short for the R-12 budget).
  - **`paused`**: true at the end of a block that ends in a dropped stretch.
- [X] T019 [P] [US1] Write `crates/rr_dr60_harness/tests/us1_vas.rs`, covering spec US1 AS1–AS5 at 48 kHz with `configs::settings("vas_only", 48_000)`, the Phase 2 stimuli and plain assertions (no `vas_checks`):
  - **AS1**: a 1 kHz tone at −15 dBFS for 10 s. `produced` equals the input length, with no event.
  - **AS2**: for burst 1 s, gap 5 s, burst 1 s, the output length is (2 s + 1.0 s − 20 ms) × 48 000 ± 48 samples, with exactly one splice.
  - **AS3**: a gap of 0.9 s. Output length equals input, with no event.
  - **AS4**: 5 s of silence. Exactly 48 000 zero samples come out, then nothing, and the final block reports `paused`.
  - **AS5**: processed in random block sizes (seeded, including 0 and 1), each block's `produced ≤` its length, and the concatenated output and events equal the one-block run.
  - **FR-011**: `latency_samples()` is equal with VAS on and bypassed, for every tap at every rate.

### Implementation for User Story 1

- [X] T020 [US1] Implement `VasStage` in `crates/rr_dr60/src/stages/vas.rs`, following data-model.md › Per-sample processing exactly, including `if sound and run ≥ O + 1` for resume and `since_sound` saturating at W + 1:
  - **Interface**: `fn process(&mut self, x: f64) -> VasDecision`, where `VasDecision` is `Keep`, `Drop` or `Resume` (the first kept sample after a pause).
  - **Derived at construction**: A<sub>thr</sub> = `detmath::exp`((threshold_dbfs + 3·(3 − sensitivity)) · LN_10 / 20), H = round(hang_ms · 8), O = round(onset_ms · 8). Each cites A-022 – A-024.
  - **Constants**: W = 32, labeled `// engineering target (003 R-15)`.
  - **Declare** the module in `crates/rr_dr60/src/stages/mod.rs`.
  - Makes T017 pass.
- [X] T021 [US1] Integrate the stage in `crates/rr_dr60/src/pipeline.rs` (research R-05, R-06):
  - **Chain**: `DeviceChain` gets `vas: VasStage`, which runs after stage 4 when `run_vas`. Dropped samples never reach stage 10 or the interpolator, and the AGC runs on every sample (FR-016).
  - **Bookkeeping** with stream counters:
    - on `Resume`, compute the splice position as the first output n with ⌊n·l/m⌋ ≥ the kept index;
    - the removed length is ⌊D·m/l⌋ minus its value at the previous splice;
    - record the event in the block where it is emitted.
  - **`BlockInfo`**: set `paused` at the end of each block.
  - **`reset` and `reconfigure`**: reset VAS and all counters.
  - **Remove** any placeholder gating from T012.
- [X] T022 [US1] Implement `process_with_events` and `max_events` in `crates/rr_dr60/src/pipeline.rs` (contracts/rust-api.md):
  - **Events**: write the first `events.len()` events in order, and count all of them.
  - **`max_events`**: = ⌊frames·l/m⌋ / (H + 2) + 2, computed in u128 and labeled `// engineering target (003 R-15)`.
  - **Delegation**: `process` and `process_in_place` call the same core with an empty event slice.
  - **`Pipeline` rustdoc**: list stage 5, state that by default the output can be shorter than the input, and update the example to read `produced` (moved here from T024, so `pipeline.rs` is edited by one task at a time).
  - Makes T018 pass.
- [X] T023 [US1] Update the `op-count` test in `crates/rr_dr60/src/pipeline.rs`: add a fixed VAS term per device sample and the emission compare per host sample. Assert that work is still independent of block partition, with VAS on in a burst-gap run.
- [X] T024 [P] [US1] Update the crate docs in `crates/rr_dr60/src/lib.rs` (this file only; the `Pipeline` rustdoc is part of T022). List stage 5 as "modeled on the owner's manual and assumed values (A-008, A-021 – A-025)", with no EVP claims either way, and state that by default the output can be shorter than the input.
- [X] T025 [US1] Run `cargo test -p rr_dr60 stages::vas` and `cargo test -p rr_dr60_harness --test us1_vas`, and fix until green. Confirm every 001 and 002 test and both golden guards are still green.
- [X] T052 [US1] Event-capacity edge tests (added 2026-10-10, MVP review; research R-07), in the test module of `crates/rr_dr60/src/pipeline.rs`:
  - **Worst case for `max_events`**: `onset_ms = 0`, bursts of one loud sample separated by gaps of exactly H + 1 silent device samples (the densest possible splice train), at every rate, in blocks of varied size including 1 and `frames` large enough to hold several splices; `info.events ≤ max_events(n)` for every block, and the total equals the expected splice count.
  - **Short slices**: a block containing two splices processed with slices of length 0, 1 and 2: `info.events == 2` each time, the first `len` events are written in order, and the later ones are counted but not written.
  - Both use plain assertions and cite FR-005 and R-07; update the `max_events` rustdoc to "+ 1 (+ 2 at 44.1 and 88.2 kHz)" while there.

**Checkpoint (US1 / MVP)**: T017–T019 pass (T052 was added after the checkpoint and runs before Phase 4), all 001 and 002 tests pass unchanged, and the default pipeline now drops pauses. Stop and demo with a burst-gap stimulus (quickstart § 2). This is the MVP.

---

## Phase 4: User Story 2 - Tune, bypass, tap, or mute the VAS (Priority: P2)

**Goal**: Developers can bypass, tap, tune and mute VAS from Rust and C. Invalid settings are named, and C results match Rust bit for bit.

**Independent Test**: `cargo test -p rr_dr60_harness --test us2_vas_settings --test ffi_parity` passes, and the C smoke test prints `smoke: OK`.

### Tests for User Story 2 (write first, must fail)

- [X] T026 [P] [US2] Write `crates/rr_dr60_harness/tests/us2_vas_settings.rs`, covering spec US2 AS1–AS6 with plain assertions:
  - **AS1**: regenerate 001's and 002's golden entries (`golden::generate`, `golden::generate_agc`, VAS bypassed) and compare with `golden::committed()` and `golden::committed_agc()`. Zero differences, and every block `produced == len`.
  - **AS2**: `Tap::AfterVas` with stage 10 on equals stage 10 off, sample for sample, at every rate.
  - **AS3**: at sensitivity 1 and 5, the lowest kept 1 kHz level (bisection to 0.05 dB) is −12 and −24 dBFS ± 1 dB.
  - **AS4**: `hang_ms = 3000`. For burst, 10 s gap, burst, the kept gap is 3 s ± 1 ms at 8 and 48 kHz.
  - **AS5** (VAS-isolated, mute): for burst 1 s, gap 5 s, burst 1 s:
    - the output length equals the input;
    - one muted-region event, whose length equals drop mode's removed length (exact at 8 and 48 kHz, ±1 at 44.1 kHz);
    - at 8 kHz, samples outside the region equal drop mode's output;
    - inside the region, samples are exactly 0.0 at 8 kHz, and at 48 kHz after the first `latency_samples() + 1` samples of the region.
  - **AS6**: for each VAS field, an out-of-range value returns `Error::InvalidSetting` naming it, from both `new` and `reconfigure`, and a failed `reconfigure` leaves the pipeline unchanged.
- [X] T027 [P] [US2] Extend `crates/rr_dr60_harness/tests/ffi_parity.rs` (US2 AS7, FR-015, contracts/c-api.md):
  - **Parity**: C and Rust give bit-identical output, `BlockInfo` sequences and events for `vas_only`, `vas_mute`, `default_vas`, and the minimum and maximum of every VAS field. Use `rr_dr60_process_with_events` with capacity from `rr_dr60_max_events`.
  - **Invalid fields**:
    - `vas_sensitivity = 6` and `= 259` → `INVALID_SETTING` / `VAS_SENSITIVITY`;
    - `vas_mode = 7` → `INVALID_ARGUMENT` / `VAS_MODE`;
    - `vas_onset_ms = NaN` → `VAS_ONSET_MS`;
    - `validate` and `create` agree.
  - **`out_info` on early returns**: `frames == 0`, NULL buffers and a poisoned handle each give `{0, 0, paused}`.
  - **NULL events**: `events = NULL` with capacity 0 is accepted.
  - **Old struct size**: `struct_size = 48` → `INVALID_ARGUMENT` / `STRUCT_SIZE`.
- [X] T028 [P] [US2] Extend the C smoke test in `crates/rr_dr60_ffi/tests/c/smoke.c`:
  - drop mode with a burst-gap buffer, reading `produced`, getting events through `rr_dr60_max_events` and `rr_dr60_process_with_events`;
  - mute mode (`produced == frames`);
  - `RR_DR60_TAP_AFTER_VAS`;
  - `vas_sensitivity = 259` reported by `rr_dr60_settings_validate`.
  - Print `smoke: OK`.
- [X] T029 [P] [US2] Add a rustdoc doc-test on `VasSettings` in `crates/rr_dr60/src/settings.rs` (SC-001): it bypasses VAS, switches to mute mode, sets sensitivity 5, processes a block, and reads `produced` and events with `process_with_events`.

### Implementation for User Story 2

- [X] T030 [US2] Implement mute mode (research R-09). **Placement (MVP review, 2026-10-10)**: the change lives in `crates/rr_dr60/src/pipeline.rs`, not in `stages/vas.rs`. The stage's decisions (`Keep` / `Drop` / `Resume`) are already mode-independent; the mode decides what the pipeline does with a `Drop`.
  - **Pipeline**: add a `DeviceOut::Mute` outcome. `DeviceChain` stores the output mode (today it returns `DeviceOut::Drop` before stage 10 and knows no mode): in mute mode a `VasDecision::Drop` runs stage 10 on +0.0 when `run_playback` and returns `DeviceOut::Mute(d)`, which feeds onward into the interpolator and counts toward `kept`, so the emission schedule stays one-in-one-out. `dropped` and `removed_reported` are untouched in mute mode.
  - **Regions**: `EventCounters` gains `mute_start: Option<u64>`, cleared by `reset`. At the first muted sample (kept index j₀) record the region start ⌈j₀·m/l⌉; on `Resume` (kept index j₁) emit `VasEvent { output_position: start, input_length: ⌈j₁·m/l⌉ − start }` through `pending`, so each region is reported once, by the block in which it ends (data-model › VasEvent). The block's `paused` flag shows a region still open.
  - **Data model**: add the new field to data-model.md › Event counters (and its reset note), so the table matches the code.
  - **Stage**: no change to `stages/vas.rs`.
- [X] T031 [US2] Implement the C functions in `crates/rr_dr60_ffi/src/lib.rs` (contracts/c-api.md, research R-08):
  - `RrDr60VasEvent`, `rr_dr60_process_with_events` and `rr_dr60_max_events`;
  - panic-guarded, with a `// SAFETY:` comment on each pointer use;
  - validation order: struct size → tap → `vas_mode` → host rate → AGC fields → VAS fields.
  - Regenerate the header and confirm the CI header-drift check passes locally.
  - Makes T027 and T028 pass.
- [X] T032 [P] [US2] Add a "VAS (signal-chain stage 5)" section to `README.md` (research R-14):
  - **Content**: what it does, "modeled on the owner's manual and assumed values (A-008, A-021 – A-025)", and the defaults table. Say clearly that by default the output can be **shorter than the input**, so hosts must use `produced`. Explain drop vs. mute and when to use each. State the input-referred threshold (about −58 dBFS at level 3, through the AGC's static curve; A-022) and that the default therefore pauses only on near-silence; make no claim that a quiet room pauses.
  - **Snippets**: Rust and C, for bypass, mute, sensitivity, and reading events.
  - No EVP claims either way (Principle VI).
- [X] T033 [US2] Run `us2_vas_settings`, `ffi_parity` and `crates/rr_dr60_ffi/tests/c/run_smoke.sh`, and fix until green.

**Checkpoint (US2)**: `us1_vas`, `us2_vas_settings` and `ffi_parity` pass, and the smoke test prints `smoke: OK`. US2 is validated independently of US3.

---

## Phase 5: User Story 3 - The harness measures the VAS (Priority: P3)

**Goal**: Every VAS tolerance in the spec is checked at every host rate and reported with its IDs. Golden files guard the VAS output, lengths, splices and muted regions.

**Independent Test**: `cargo test -p rr_dr60_harness --test vas_matrix --test vas_edge_cases --test golden_vas --test mutation` passes, and the release-mode `--ignored` matrix passes. A +20 % hang or +3 dB threshold fails both a tolerance check and a golden check.

### Harness checks and tests (write first; each must fail against a deliberately wrong VAS)

- [ ] T034 [US3] Create `crates/rr_dr60_harness/src/vas_checks.rs`, with one public function per requirement (research R-11). Each takes `(rate, &Case, Make)`, returns `Vec<MeasurementResult>` citing `003/FR-0xx` plus A-/S-IDs, and labels engineering targets:
  - **`check_rate_self_test`**: kept lengths at the host rate agree with 8 kHz within the FR-008/009 tolerances.
  - **`check_fr004_length`**: output length vs. input − ⌊dropped·m/l⌋, within ±1 (exact at 8 kHz).
  - **`check_fr005_events`**:
    - removed lengths sum to input − output (±1, exact at 8 kHz) for a stream ending while recording;
    - mute-region lengths equal drop's removed lengths (±1 at 44.1 and 88.2 kHz);
    - `paused` is correct at the end.
  - **`check_fr006_threshold`**:
    - bisection to 0.05 dB at 300, 1000 and 3400 Hz, within ±1 dB of each other, reporting the 1 kHz offset per rate;
    - keep at +3 dB and drop at −3 dB;
    - noise at T − 5 dB kept and T − 15 dB dropped, only when `hang_ms ≥ 500` (FR-013).
  - **`check_fr007_sensitivity`**: every level at 3 dB steps, ±1 dB.
  - **`check_fr008_hang`**: gaps of 0.5 H, H − 5 ms, H + 5 ms and 3 H, ±1 ms.
  - **`check_fr009_onset`**: bursts of 0.5 O, O − 2 ms (when O ≥ 3 ms), 1.5 O and 50 O, ±1 ms, with no resume for the short bursts.
  - **`check_fr010_splices`**: at 8 kHz, every kept sample is bit-identical to the VAS-bypassed output at its input index, which follows from the events.
  - **`check_fr011_latency`**.
- [ ] T035 [US3] Write `crates/rr_dr60_harness/tests/vas_matrix.rs` (research R-12):
  - **Normal run**:
    - (a) defaults at all 6 rates;
    - (b) every sensitivity level, and the minimum and maximum of every other setting with the rest at default, at 8 and 48 kHz;
    - "kept in full" checks use H + 1 s tones.
  - **`#[ignore]` test `full_matrix_all_rates`**: the full set at all 6 rates, for the release-mode job.
  - **`interplay_report`** (US3 AS3, the floor sweep; research R-11): the default pipeline (`default_vas`), groups of 1 kHz bursts at −20 dBFS input, 300 ms on / 150 ms off, four per group, separated by 5 s gaps of band-limited noise, run once per noise floor at −40, −50, −60 and −70 dBFS RMS (engineering targets, FR-017). It prints, per floor, the kept length of each gap, and a table of the input-referred threshold at each sensitivity level implied by spec 002's static curve (A-017, A-022), with no tolerance.
  - **Runtime**: measure the normal-run and release-mode times and record them in the module docs. Each must be ≤ 60 s (R-12), or move cases to the ignored test.
  - **First run**: against a stage with resume allowed on non-sound samples, `check_fr009_onset` must fail. Record that in the PR.
- [ ] T036 [P] [US3] Write `crates/rr_dr60_harness/tests/vas_edge_cases.rs`, covering spec Edge Cases at 8 and 48 kHz unless stated:
  - **Start and reset**: start, reset and reconfigure each keep exactly H of silence, then pause.
  - **Everything dropped**: a block entirely inside a pause gives `produced == 0`. In mute mode it gives a full block of zeros.
  - **Short sounds**: sounds shorter than the onset time leave no output, including the onset − 2 ms burst.
  - **Gap exactly H**: no splice.
  - **Hovering**: a tone hovering at the threshold ± 0.5 dB with random dropouts gives at most one pause per H + 2 device samples.
  - **Non-finite**: NaN, +Inf and −Inf give decisions and output identical to 0.0 in their place.
  - **Long silence** (`#[ignore]`, release): 1 hour at 8 kHz gives H zeros, then nothing, with `produced == 0` per block.
  - **Long sound** (`#[ignore]`, release): a 10-minute 1 kHz tone at the threshold + 10 dB at 8 kHz keeps `produced == input` for every block, with no event (spec Edge Cases).
  - **Tap before VAS**: `Tap::AfterAgc` and `Tap::AfterRecord` give fixed length, and VAS settings have no effect.
  - **Mute and stage 10**: with stage 10 on, the output keeps its length, and ring-out at a region's start is allowed.
  - **AGC runs while paused** (FR-016, A-025): with AGC and VAS on (tap "after VAS", stage 4 off), every kept sample is bit-identical to the VAS-bypassed output at its input index. **Bit-exact only at 8 kHz**, where the conversion is the identity; at the other rates the interpolator's history differs after each splice, so either run this check at 8 kHz only or compare within a stated tolerance (FR-017) at the other rates.
- [ ] T037 [US3] Extend `crates/rr_dr60_harness/src/golden.rs` (contracts/golden-format.md):
  - **Entry field**: an optional `vas: Option<VasGolden { events: Vec<[u64; 2]>, paused_at_end: bool }>`, with `#[serde(default, skip_serializing_if = "Option::is_none")]`.
  - **Hash scope**: `n_samples` and `sha256` cover only produced samples.
  - **The VAS set**: stimuli `vas_burst_gap`, `vas_noise_gaps` (−70 dBFS noise) and `vas_short_bursts`, with exact durations from the contract; configs `default_vas`, `vas_mute` and `vas_only`.
  - **Embedding**: `committed_vas()` embeds `golden/golden-vas-v1.json`.
  - **Comparison**: fails if `events` or `paused_at_end` differ.
  - **Blessing**: `RR_DR60_BLESS=vas` rewrites only the VAS file.
  - Confirm `golden-v1.json` and `golden-agc-v1.json` still read and compare unchanged.
- [ ] T038 [US3] Write `crates/rr_dr60_harness/tests/golden_vas.rs` with `golden_vas_matches`. Bless `crates/rr_dr60_harness/golden/golden-vas-v1.json` with `RR_DR60_BLESS=vas cargo test -p rr_dr60_harness --test golden_vas`: 54 entries. Add a CHANGELOG line for the new golden file.
- [ ] T039 [P] [US3] Extend `crates/rr_dr60_harness/tests/mutation.rs` (SC-007) with two mutants built through `Make`, with no hidden hooks: `hang_ms × 1.2` and `threshold_dbfs + 3.0`. For each, assert that `check_fr008_hang` or `check_fr006_threshold` reports a failure, and that `golden::compare` against `committed_vas()` reports a difference.
- [ ] T040 [P] [US3] Extend `crates/rr_dr60_harness/tests/determinism.rs` and `crates/rr_dr60_harness/tests/alloc_free.rs` (FR-014, SC-003, SC-004; research R-12):
  - **Determinism**:
    - 100 seeded random partitions (including sizes 0 and 1) of each VAS golden stimulus with `vas_only` at 8 and 48 kHz;
    - 10 partitions for `vas_mute`, `default_vas` and the other rates;
    - concatenated output, events and final `paused` are bit-identical to the one-block run.
  - **Allocation**: zero allocations over 1000+ blocks for the same configurations, including blocks that are entirely dropped and blocks with several splices (`hang_ms = 50`), through Rust `process_with_events` and C `rr_dr60_process_with_events`.
- [ ] T041 [P] [US3] Add `default_vas` to the release-mode timing test in `crates/rr_dr60_harness/tests/timing.rs`, keeping max/min ≤ 3.0 and ≥ 20× real time at 48 kHz (SC-006).
- [ ] T042 [US3] Run the US3 tests and the ignored matrix in release mode (`cargo test -p rr_dr60_harness --release --test vas_matrix --test vas_edge_cases -- --ignored`), and fix until green.

**Checkpoint (US3)**: `cargo test --all-features` and the ignored release-mode tests pass, and the report lists every 003 FR with its IDs. All three stories have now been validated independently.

---

## Phase 6: CI and cross-platform golden files (SC-003)

- [ ] T043 Update `.github/workflows/ci.yml`:
  - **`check` job**: add `--test golden_vas` next to `--test golden --test golden_agc`, and add `--test vas_matrix --test vas_edge_cases` to the release-mode `--ignored` step.
  - **`golden-matrix` and `ios` jobs**: add `--test golden_vas` to the test commands.
  - Update the header comment.
- [ ] T044 [P] Add `--test golden_vas` to the dinghy command in `scripts/ios-device-golden.sh`. In `docs/release-checklist.md`, note that the iOS-device run now covers all three golden files.
- [ ] T045 Push `003-vas` and open a **draft PR**, only after asking the user.
  - Confirm every CI job is green, including `golden_vas` on the desktop targets and the iOS simulator.
  - Record the `check` job's wall time against the 002 baseline (R-12 budget).

**Checkpoint**: CI is green on every automated target, within the R-12 budget. The iOS-device check stays a manual release gate.

---

## Phase 7: Polish & cross-cutting concerns

- [ ] T046 [P] Update `CHANGELOG.md` `[Unreleased]`:
  - **Added**: VAS (stage 5, spec 003); `VasSettings`, `VasMode`, `Tap::AfterVas`, `BlockInfo`, `VasEvent`, `process_with_events` and `max_events`; the C VAS fields, `RrDr60VasMode`, `RR_DR60_TAP_AFTER_VAS`, `RrDr60BlockInfo`, `RrDr60VasEvent`, `rr_dr60_process_with_events` and `rr_dr60_max_events`; and `golden-vas-v1.json`.
  - **Changed**:
    - **Default output now drops pauses**, so it can be shorter than the input (the line was added on 2026-10-10; extend it, don't duplicate it). State the input-referred threshold (about −58 dBFS at level 3) and that the default pauses only on near-silence; no quiet-room claim.
    - `process` and `process_in_place` return `BlockInfo`.
    - `rr_dr60_process` takes `out_info`.
    - The C struct is 72 bytes.
  - **Assumptions**: A-021 (manual), A-022 – A-025, and A-020's correction.
- [ ] T047 [P] Update `docs/hardware/signal-chain.md`: row 5 already links spec 003 (evidence pass, 2026-10-10); add the IDs "A-022 – A-025" to that link and keep its A-033 note. Leave the A-022 to A-025 status in `docs/hardware/assumptions.md` as `assumed`.
- [ ] T048 [P] Update the "Current status" section of `CLAUDE.md` with feature 003, the new test names (`us1_vas`, `us2_vas_settings`, `vas_matrix`, `vas_edge_cases`, `golden_vas`), the VAS bless command, and the measurement lesson: measure VAS by output length and events (003 R-11).
- [ ] T049 Traceability audit (SC-008, FR-017): list every numeric literal in the code and tests added by this feature, **including test literals** (the `#[cfg(test)]` modules of `stages/vas.rs`, `pipeline.rs` and `resample/up.rs`, and every new file under `crates/rr_dr60_harness/tests/`; e.g. the burst lengths 80/144/159/160/161/240/8000 in T017 must say "O − 80, O − 16 (onset − 2 ms), O − 1, O, O + 1, 1.5 O, 50 O (FR-009)"). Check each one has an A-/S- ID or an engineering-target comment (`// engineering target (003 FR-017)` or `(003 R-15)`), and fix any that don't. Record the check in the PR description. Search: `git diff main -- crates/ | grep -E '^\+.*[0-9]+\.[0-9]+|^\+.*\b[0-9]{2,}\b' | grep -v -E 'A-0|S-0|engineering target|FR-0|R-[0-9]'`. Also run `scripts/check-traceability.sh`.
- [ ] T050 Run every step of `specs/003-vas/quickstart.md` § 1–6 and the coverage gate `cargo llvm-cov --all-features --workspace --fail-under-lines 80`. Fix anything that fails.
- [ ] T051 Ask the speckit-coach for a final review, then ask the user before merging the PR.
---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (Phase 1)**: no dependencies.
- **Foundational (Phase 2)**: depends on Setup. It blocks every story.
- **US1 (Phase 3)**: depends on Phase 2.
- **US2 (Phase 4)**: depends on Phase 2.
  - T026 AS4/AS5 and T027's parity cases need the VAS stage (T020, T021) to show non-trivial output. Run US2 after US1, or write its tests in parallel with US1 and expect them to fail until T021.
- **US3 (Phase 5)**: depends on US1 (it measures the stage) and on T030 for its mute checks. Its C allocation path uses T031.
- **CI (Phase 6)**: depends on US3 (T038, the golden file).
- **Polish (Phase 7)**: depends on everything else.

### Task-level dependencies

| Task | Depends on |
|---|---|
| T008 | T003 |
| T009 | T004, T005, T008 |
| T010 | T008 |
| T011 | T006, T010 |
| T012 | T008 |
| T013 | T007 |
| T014 | T008 |
| T015 | T008, T001 |
| T016 | T014, T015 |
| T020 | T017 |
| T021 | T020, T011, T012 |
| T022 | T021, T018 |
| T023 | T022 |
| T025 | T019, T022 |
| T030 | T021 |
| T031 | T015, T022, T027 |
| T033 | T026 – T032 |
| T035 | T034 |
| T037 | T021, T030 |
| T038 | T037 |
| T039 | T034, T038 |
| T043 | T038 |
| T045 | T043, and the user's go-ahead |
| T050 | T049 |
| T052 | T022 |

### Within each story

Tests come before implementation, and the story checkpoint passes before the next story starts (Principle VII).

---

## Parallel Examples

### Phase 2 (tests, then implementation)

```text
T003, T004, T005, T006, T007   # all different files
then T008 → T009, T010 → T011; in parallel: T012, T013, T014
then T015 → T016
```

### User Story 1

```text
T017 (stage unit tests) ∥ T018 (pipeline events) ∥ T019 (harness us1_vas) ∥ T024 (docs)
then T020 → T021 → T022 → T023 → T025 → T052
```

### User Story 2

```text
T026 ∥ T027 ∥ T028 ∥ T029 ∥ T032
then T030 → T031 → T033
```

### User Story 3

```text
T034 → T035; in parallel: T036, T037 → T038 → T039; T040, T041
then T042
```

---

## Implementation Strategy

### MVP first (User Story 1)

1. **Phases 1–2**: the version, both golden guards, settings, validation, `BlockInfo` and the call-site sweep, and the emission schedule (output unchanged). Then move all 001 and 002 tests to VAS-bypassed settings. Everything green, golden files untouched.
2. **Phase 3**: the VAS stage in drop mode, with events. Stop at the checkpoint, demo, and review with the coach.

### Incremental delivery

1. **US2**: mute mode, and control from Rust and C. Validate it independently.
2. **US3**: the full measurement matrix, edge cases, golden file, mutation and determinism tests.
3. **CI**: wiring for all targets, within the R-12 budget.
4. **Polish**, then the merge, with the user's approval.

### Risk notes

- **The emission schedule (T011)** changes the hot path for every configuration. If any 001 or 002 test or golden file changes, stop: R-05 says no output may change when nothing is dropped. Compare against the prototype in research R-05 before changing anything else.
- **The `#[must_use]` sweep (T010)** touches 19 files. Do it in one commit, with no behavior change, so the diff is easy to review.
- **Default output length changes for every host**: the README (T032) and CHANGELOG (T046) must say so prominently.
- **Never widen a spec tolerance** to make a test pass. Raise it with the user instead.

## Notes

- `[P]` tasks touch different files and have no unfinished dependencies.
- Commit after each task or logical group, using Conventional Commits.
- Every value cites an A-ID, an S-ID or an engineering target (SC-008).
