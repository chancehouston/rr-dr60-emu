---

description: "Task list for 002 Automatic Gain Control (AGC) on the Record Path"
---

# Tasks: Automatic Gain Control (AGC) on the Record Path

**Input**: Design documents from `specs/002-agc/`: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: REQUIRED. Constitution III says measurement tests are written first and must fail before the implementation exists. Every phase lists its tests before its implementation tasks.

**Organization**: Setup → Foundational (settings, validation, harness helpers, keeping spec 001 intact) → US1 (P1, MVP: the AGC stage) → US2 (P2: tune, bypass, tap, C interface) → US3 (P3: full measurement harness and golden files) → CI → Polish.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: The user story the task belongs to (US1, US2, US3)
- Paths are relative to the repository root.

## Conventions that apply to every task

- **Traceability**:
  - Cite spec 002 FR, SC and A-IDs in code comments, rustdoc and test names, e.g. `// A-017: 10:1 regulation slope`.
  - Detector and measurement constants are commented `// engineering target (002 R-15)`. Spec tolerances are commented `// engineering target (002 FR-015)`.
  - Harness results cite requirements as `002/FR-0xx`.
- **Determinism** (001 R-04, 002 R-06):
  - In `rr_dr60`, use only basic IEEE operations, plus `rr_dr60_detmath::{ln, exp}`. No platform libm. `clippy.toml` already enforces this.
  - ln 10 is `core::f64::consts::LN_10`.
- **Real-time safety**: no allocation, formatting or I/O in `process`, `process_in_place` or `reset`. AGC state is fixed-size and inline.
- **Spec 001 must stay intact**:
  - `crates/rr_dr60_harness/golden/golden-v1.json` must never change.
  - Spec 001's expected values and tolerances must never change. Only its settings change, to bypass the AGC (FR-018).
- **Before ticking a task**: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all-features` must pass. The only exception is a test written to fail first, and only until its implementation task is done.
- **Commits**: Conventional Commits, on branch `002-agc`. Merge to `main` only through a green PR, and only after the user says so.

---

## Phase 1: Setup

**Purpose**: Version bump and the generated Hilbert coefficients that the AGC detector needs.

- [ ] T001 Bump the workspace version to 0.2.0:
  - `version = "0.2.0"` in `[workspace.package]` and in the three internal dependency entries of `Cargo.toml`.
  - `RR_DR60_VERSION_MINOR = 2` in `crates/rr_dr60_ffi/src/lib.rs`.
  - Regenerate `crates/rr_dr60_ffi/include/rr_dr60.h` (see `crates/rr_dr60_ffi/cbindgen.toml`).
  - Golden comparison ignores `library_version`, so the 001 golden test must still pass.
- [ ] T002 [P] Create `tools/filter-design/design_hilbert.py`, modeled on `design_voiceband.py`: a `uv` inline-script header (numpy, scipy), `--check` mode, and a non-zero exit on failure.
  - **Design**: `scipy.signal.remez(63, [250, 3750], [1], type="hilbert", fs=8000)`.
  - **Checks**: the taps are antisymmetric, and every even-index tap is exactly 0. |H| is within ±0.01 dB at every 10 Hz step from 300 to 3400 Hz.
  - **Output**: `crates/rr_dr60/src/stages/agc_hilbert_coeffs.rs`, containing `pub(crate) const AGC_HILBERT: [f64; 63]` as `f64::from_bits(0x…)` literals.
  - **Header comment**: cites A-019, research.md R-03 and R-15, and the regeneration command.
- [ ] T003 Run `uv run tools/filter-design/design_hilbert.py` and commit the generated `crates/rr_dr60/src/stages/agc_hilbert_coeffs.rs`. Declare it in `crates/rr_dr60/src/stages/mod.rs`; until US1 uses it, add `#[allow(dead_code)]` with a comment pointing to T023.

**Checkpoint**: the workspace builds at 0.2.0, and `uv run tools/filter-design/design_hilbert.py --check` passes.

---

## Phase 2: Foundational (blocking prerequisites)

**Purpose**: The settings and validation surface, the C struct fields, the harness helpers, and moving every spec 001 test onto AGC-bypassed settings. Once this phase is done, the pipeline accepts AGC settings but does not process them yet: no AGC stage exists until US1. All 001 tests stay green.

### Tests (write first, must fail)

- [ ] T004 [P] Settings tests in the `#[cfg(test)]` module of `crates/rr_dr60/src/settings.rs`. Assert:
  - **Defaults**: `AgcSettings::DEVICE` and `AgcSettings::default()` are `enabled: true` (A-020), `target_dbfs: -10.0`, `max_gain_db: 40.0`, `max_attenuation_db: 20.0` (A-017), `attack_ms: 10.0` and `release_ms: 1000.0` (A-018).
  - `Settings::new(48_000).agc == AgcSettings::DEVICE`.
  - `Tap::AfterAgc` exists, and `Tap::default()` is still `AfterPlayback`.
- [ ] T005 [P] Validation tests in the test module of the new `crates/rr_dr60/src/validate.rs`. For each AGC field, using the data-model ranges quoted verbatim:
  - `target_dbfs` "−30.0 … 0.0"
  - `max_gain_db` "0.0 … 60.0"
  - `max_attenuation_db` "0.0 … 40.0"
  - `attack_ms` "1.0 … 100.0"
  - `release_ms` "50.0 … 10000.0"

  Assert:
  - **Boundaries**: both inclusive bounds are accepted. The next representable f32 outside each bound, NaN, +Inf and −Inf are rejected with `Error::InvalidSetting { setting: Setting::… }` naming that field.
  - **When bypassed**: validation still happens with `enabled = false`.
  - **Rate first**: an unsupported host rate is reported before any AGC error.
  - **Combinations**: a release shorter than the attack (attack 100, release 50) is accepted (spec FR-012).
  - **Display**: `Setting`'s `Display` contains the field name (e.g. `agc.attack_ms`) and its range.
- [ ] T006 [P] Pipeline validation tests in the test module of `crates/rr_dr60/src/pipeline.rs`:
  - `Pipeline::new` with `attack_ms = 0.0` returns `Err(Error::InvalidSetting { setting: Setting::AgcAttackMs })`.
  - `reconfigure` with invalid AGC settings returns the error, and leaves `settings()`, `latency_samples()` and the processing state unchanged. Check this by comparing output with a clone of the pipeline that was never reconfigured.
- [ ] T007 [P] Saturation test in `crates/rr_dr60/src/sanitize.rs`:
  - `narrow_out(1e300)` returns `f32::MAX`, and `narrow_out(-1e300)` returns `f32::MIN`. Both are finite (research.md R-07).
  - Existing behavior for normal values and subnormals is unchanged.
- [ ] T008 [P] Hilbert coefficient tests in `crates/rr_dr60/src/stages/agc_hilbert_coeffs.rs`, in a `#[cfg(test)]` module that the generator writes or that is appended next to it in `stages/mod.rs`:
  - Evaluate the frequency response analytically, with detmath `sin`/`cos`, at 300, 500, 1000, 2000, 8000/3 and 3400 Hz. |H| must be within ±0.01 dB (engineering target, 002 R-15).
  - Antisymmetry: h[k] = −h[62−k]. Even-index taps are exactly 0.
- [ ] T009 [P] Harness helper tests in `crates/rr_dr60_harness/src/analysis.rs` and `crates/rr_dr60_harness/src/stimulus.rs` test modules:
  - `analytic_envelope` recovers a constant 0.5 amplitude for a 1 kHz sine to within 0.01 dB away from the ends.
  - `settle_index(trajectory, final, excursion)` returns the last index outside 2/27 of the excursion. Check it on a synthetic exponential with a known τ, where the answer is τ·ln 13.5 ± 1 sample.
  - `midpoint_fraction` gives 1 − e^(−ln 13.5 / 4) ≈ 0.478 ± 0.01 for an exponential.
  - `thd_n` (least-squares fit of the fundamental's sine and cosine terms; residual power including DC, relative to the fundamental) returns below −100 dB for a pure tone, and −40 dB ± 0.1 dB for a tone plus an added 1 % 3rd harmonic. The same holds at 2000 Hz at an 8 kHz rate, where that harmonic folds onto the fundamental's own frequency; this proves the method can see folded harmonics.
  - `level_dbfs_aes17` returns 0.0 for a full-scale sine and −70.0 ± 0.05 for `bandlimited_noise(…, -70.0)`.
  - `step(levels, durations)` and `tone_bursts` produce the exact sample counts requested.
  - Every new stimulus is bit-reproducible (detmath and PCG32 only): two calls give identical bits.
- [ ] T010 [P] Write `crates/rr_dr60_harness/tests/golden_agc.rs` with a first test, `golden_v1_unchanged`. It computes the SHA-256 of `include_bytes!("../golden/golden-v1.json")` and asserts it equals a constant recorded from the file as committed on `main`, which has not changed since spec 001. The test fails only if someone edits the 001 golden file (FR-017, FR-018).

### Implementation

- [ ] T011 Implement in `crates/rr_dr60/src/settings.rs` (contracts/rust-api.md):
  - `#[non_exhaustive] pub struct AgcSettings { enabled, target_dbfs, max_gain_db, max_attenuation_db, attack_ms, release_ms }`, deriving `Clone, Copy, Debug, PartialEq`.
  - `pub const DEVICE`, and `Default`.
  - `Settings.agc`. `Settings` derives `Clone, Copy, Debug, PartialEq`, with **no `Eq`/`Hash`**.
  - `Tap::AfterAgc`, with rustdoc citing A-017, A-018, A-020 and spec FR-003/FR-011.
  - Rustdoc on `AgcSettings` states the overshoot warning: output can exceed ±1.0 by up to `max_gain_db`; limit or clip before converting to integer formats. Makes T004 pass.
- [ ] T012 Implement in `crates/rr_dr60/src/error.rs` and the new `crates/rr_dr60/src/validate.rs`:
  - The `#[non_exhaustive] pub enum Setting` (with `Display`), and `Error::InvalidSetting { setting }` with `Display`.
  - `pub(crate) fn validate(&Settings) -> Result<(), Error>`: rate first, then the AGC fields in struct order, inclusive ranges, non-finite rejected.
  - Re-export `AgcSettings` and `Setting` from `crates/rr_dr60/src/lib.rs`.
  - Call `validate` at the start of `Pipeline::new`, which `reconfigure` already goes through. Makes T005 and T006 pass.
- [ ] T013 [P] Make `narrow_out` in `crates/rr_dr60/src/sanitize.rs` saturate ±Inf results to ±`f32::MAX` (R-07). Makes T007 pass.
- [ ] T014 Update `DeviceChain::new` in `crates/rr_dr60/src/pipeline.rs` with the stage gating `run_record = settings.record_stage_enabled && settings.tap != Tap::AfterAgc` and `run_playback = settings.playback_stage_enabled && settings.tap == Tap::AfterPlayback` (data-model.md). The latency calculation is unchanged, and with `AfterAgc` it reports the boundary delay only. Add a unit test: with `Tap::AfterAgc` and both stages enabled, output equals the both-stages-bypassed output.
- [ ] T015 [P] Implement the helpers that T009 tests in `crates/rr_dr60_harness/src/analysis.rs`: `analytic_envelope` (FFT Hilbert via `rustfft`), `gain_trajectory_exact` (y/x where |x| ≥ 0.1·A, engineering target R-15), `settle_index`, `midpoint_fraction`, `thd_n`, `level_dbfs_aes17`. Analysis code is outside the R-04 ban, as in 001.
- [ ] T016 [P] Implement the stimuli that T009 tests in `crates/rr_dr60_harness/src/stimulus.rs`: `step`, `tone_bursts`, and `bandlimited_noise(seed, len, fs, level_dbfs)`. The noise is PCG32 through a Kaiser-windowed sinc band-pass FIR, 300–3400 Hz, designed with detmath, then scaled by the AES17 convention (RMS = 10^(L/20)/√2). It never uses `rr_dr60`'s filters (Principle VII).
- [ ] T017 Move every spec 001 harness configuration onto AGC-bypassed settings in `crates/rr_dr60_harness/src/configs.rs`:
  - `settings()` sets `s.agc.enabled = false` for `default`, `record_only`, `playback_only`, `tap_after_record` and `bypass_all`. Add a doc comment saying each name means "the spec 001 configuration with the AGC bypassed (spec 002 FR-018)".
  - Add the new names: `agc_only` (AGC on, both stages off, `Tap::AfterAgc`), `agc_isolated_after_playback` (AGC on, both stages off, `AfterPlayback`), `agc_tap_stages_on` (AGC on, both stages on, `AfterAgc`), and `default_agc` (`Settings::new`).
  - Keep `CONFIGS` (001's list) unchanged, and add `AGC_CONFIGS`.
  - Extend `configs_match_data_model_table`.
- [ ] T018 Extend `RrDr60Settings` in `crates/rr_dr60_ffi/src/lib.rs`:
  - Append `agc_enabled: bool, agc_target_dbfs: f32, agc_max_gain_db: f32, agc_max_attenuation_db: f32, agc_attack_ms: f32, agc_release_ms: f32` in that order (contracts/c-api.md offsets 24–44, size 48). The derive drops `Eq`.
  - `rr_dr60_settings_default` fills the device defaults.
  - Add `RrDr60Tap::AfterAgc = 2`. `to_settings` maps tap 2 and copies the AGC fields. The `struct_size ≥ sizeof` rule is unchanged.
  - Until T031, map `Error::InvalidSetting` to `InvalidArgument`.
  - Add a `const _: () = assert!(size_of::<RrDr60Settings>() == 48);`.
  - Regenerate the header.
- [ ] T019 Update the 001 tests that build settings by hand so they use AGC-bypassed settings and map every tap exhaustively:
  - In `crates/rr_dr60_harness/tests/ffi_parity.rs` (`c_settings`) and `crates/rr_dr60_harness/tests/alloc_free.rs` (around line 85), replace the `if tap == AfterRecord {0} else {1}` mapping with an exhaustive `match` including `Tap::AfterAgc => 2`, and copy `agc_enabled` and the AGC fields from the Rust settings.
  - Check `us1_voiceband.rs`, `us2_bypass_tap.rs`, `edge_cases.rs`, `timing.rs`, `determinism.rs`, `memory.rs`, `mutation.rs` and `crates/rr_dr60_ffi/tests/c/` for `Settings::new` or `rr_dr60_settings_default` used as "the 001 default". Each must set the AGC off (`agc_enabled = false` in C).
  - No expected value or tolerance may change.

**Checkpoint**:
- `cargo test --all-features` is green, including every spec 001 test, with unchanged expectations.
- `golden_v1_unchanged` passes, and `git diff main -- crates/rr_dr60_harness/golden/golden-v1.json` is empty.
- The pipeline accepts and validates AGC settings, but has no AGC stage yet.

---

## Phase 3: User Story 1 - Hear the RR-DR60's auto-gain character (Priority: P1) 🎯 MVP

**Goal**: The AGC stage exists and is on by default. Quiet input comes out louder, loud input quieter, the level pumps after loud sounds, and noise rises in pauses.

**Independent Test**: `cargo test -p rr_dr60 stages::agc` and `cargo test -p rr_dr60_harness --test us1_agc` pass. They use only the core and the Phase 2 helpers, not the US3 harness.

### Tests for User Story 1 (write first, must fail)

- [ ] T020 [P] [US1] Unit tests in the `#[cfg(test)]` module of the new `crates/rr_dr60/src/stages/agc.rs`. They drive `AgcStage` directly at 8 kHz with detmath tones:
  - **Static curve** (FR-004, FR-005, A-017): 1 kHz tones in, steady output peak level out, measured over the last 0.25 s after 20 × attack + detector delay. Expected −60 → −20, −40 → −13, −10 → −10, 0 → −9 and +5 → −8.5 dBFS, each ±1 dB. The prototype achieved within 0.01 dB.
  - **Start at maximum** (FR-013, A-019): for the first non-zero input sample, y/x = 10^(40/20) within 1e-9 relative.
  - **Silence** (FR-007, US1 AS4): 10 s of 0.0 gives exactly 0.0 out. After a loud tone and then 10 s of silence, G is within 0.1 dB of `max_gain_db`.
  - **Timing** (FR-006, A-018): use the exact gain trajectory y/x on the step −40 → −10 → −40 dBFS. Attack must be 10 ms ± 2 ms (prototype 10.00 ms) and release 1.0 s ± 0.2 s (prototype 1.001 s). The midpoint at 25 % of the measured release must be in 35–65 % (prototype 47 %).
  - **Gain flush** (R-07): with `max_attenuation_db = 0` and the other settings at default, a 0 dBFS tone for 10 minutes. The tone is above the target, so the clamp gives G<sub>t</sub> = −0.0 and G decays from 40 toward 0. Assert that G is never subnormal, that it becomes exactly 0.0, and that from then on the output equals the input bit for bit. Run it `#[ignore]` in debug if slow; release-mode CI runs it.
  - **Reset** (FR-013): after any input, `reset()` makes the next output bit-identical to a fresh stage.
- [ ] T021 [P] [US1] Write `crates/rr_dr60_harness/tests/us1_agc.rs`, covering spec US1 AS1–AS5 at 48 kHz with the Phase 2 helpers:
  - **AS1, AS2**: `agc_only`, levels by AES17 single-bin DFT.
  - **AS3**: the step stimulus, using `analytic_envelope` divided by the input amplitude, with latency removed.
  - **AS4**: silence in gives exact zeros out.
  - **AS5**: `default_agc` gives −13 dBFS ± 1.2 dB for −40 dBFS in.
  - **FR-010**: `latency_samples()` is equal with the AGC on and bypassed, for every tap at every rate.

### Implementation for User Story 1

- [ ] T022 [US1] Implement `AgcStage` in `crates/rr_dr60/src/stages/agc.rs`, following data-model.md › Per-sample processing exactly:
  - **State**: a ring of the last 63 samples.
  - **e²** = max(the peak hold over k = 0…31 of x[n−k]², x[n−31]² + h[n]², 1e-20), with h from `AGC_HILBERT`'s 32 odd taps.
  - **L** = (10 / LN_10) · `detmath::ln(e²)`.
  - **G<sub>t</sub>** = clamp(−0.9·(L − T), −A<sub>max</sub>, +G<sub>max</sub>).
  - **Smoother**: one-pole with α<sub>a</sub> or α<sub>r</sub>, where α = 1 − `detmath::exp`(−1 / (τ·8000)), τ<sub>a</sub> = attack / ln 13.5 and τ<sub>r</sub> = (release − 31/8000 s) / ln 13.5, computed at construction. Then G ← `flush_state(G)`.
  - **Output**: y = x · `detmath::exp`(G · LN_10 / 20).
  - **Start and reset**: G starts at G<sub>max</sub>, and `reset()` restores the initial state.
  - **Traceability**: label constants per 002 R-15, and cite A-017, A-018, A-019.

  Makes T020 pass.
- [ ] T023 [US1] Integrate the stage in `crates/rr_dr60/src/pipeline.rs`:
  - `DeviceChain` gets `agc: AgcStage` and `run_agc = settings.agc.enabled`. `process` runs AGC → stage 4 → stage 10.
  - `reset` also resets the AGC.
  - A bypassed AGC does no arithmetic (FR-002).
  - Update the `Pipeline` rustdoc: stage 3 is listed, plus the overshoot warning.
  - Remove the `#[allow(dead_code)]` from T003.
- [ ] T024 [US1] Update the `op-count` test in `crates/rr_dr60/src/pipeline.rs`. Add a fixed AGC term to the per-sample bound: 32 Hilbert multiply-adds plus the counted `ln`/`exp` and smoother operations. Count AGC operations under `cfg(feature = "op-count")` in `agc.rs`. Assert that the work is still independent of how the input is split into blocks, with the AGC on.
- [ ] T025 [P] [US1] Update the crate docs in `crates/rr_dr60/src/lib.rs`, which list the modeled stages: add stage 3, "modeled on an assumed AGC (A-017–A-020)", with no EVP claims either way. Update the `crates/rr_dr60_detmath/src/lib.rs` docs: `ln`/`exp` now also run on the processing path (002 R-06).
- [ ] T026 [US1] Run `cargo test -p rr_dr60_harness --test us1_agc` and fix until green. Confirm that every 001 test and `golden_v1_unchanged` are still green.

**Checkpoint (US1 / MVP)**: T020 and T021 pass, all 001 tests pass unchanged, and the default pipeline now includes the AGC. Stop and demo (quickstart.md § 7). This is the MVP.

---

## Phase 4: User Story 2 - Tune, bypass, or tap the AGC (Priority: P2)

**Goal**: Developers can bypass, tap and tune the AGC, from Rust and from C, and invalid settings are named.

**Independent Test**: `cargo test -p rr_dr60_harness --test us2_agc_settings --test ffi_parity` passes, and the C smoke test prints `smoke: OK`.

### Tests for User Story 2 (write first, must fail)

- [ ] T027 [P] [US2] Write `crates/rr_dr60_harness/tests/us2_agc_settings.rs`, covering spec US2 AS1–AS5:
  - **AS1**: regenerate 001's golden entries (`golden::generate` with the 001 configs, AGC bypassed) and compare them with `golden::committed()`. Zero differences.
  - **AS2 and FR-003**: `agc_tap_stages_on`, `agc_isolated_after_playback` and `agc_only` give bit-identical output on the step stimulus at every rate.
  - **AS3**: `release_ms = 3000.0` gives a measured release of 3 s ± 20 % at 8 kHz, by the exact trajectory.
  - **AS4**: `target_dbfs = -20.0` with a −30 dBFS tone gives −21 dBFS ± 1 dB.
  - **AS5**: for each AGC field, an out-of-range value returns `Error::InvalidSetting` naming it, from both `new` and `reconfigure`, and a failed `reconfigure` leaves the pipeline unchanged.
- [ ] T028 [P] [US2] Extend `crates/rr_dr60_harness/tests/ffi_parity.rs` (US2 AS5, AS6, FR-014, contracts/c-api.md):
  - **Parity**: C and Rust output are bit-identical for `default_agc`, `agc_only`, `agc_tap_stages_on`, and each extreme setting (the minimum and maximum of every field).
  - **Invalid field**: `agc_attack_ms = 0` makes `rr_dr60_create` return `RR_DR60_STATUS_INVALID_SETTING`, and `rr_dr60_settings_validate` returns the same status with field `AGC_ATTACK_MS`.
  - **Old struct size**: `struct_size = 24` gives `INVALID_ARGUMENT` / `STRUCT_SIZE`.
  - **Agreement**: with several bad fields at once (bad struct size and bad tap; bad rate and bad attack), the status from `validate` equals the status from `create`, and the named field follows the documented order (struct size → tap → host rate → AGC fields).
  - **NULL**: a NULL `settings` gives `NULL_POINTER`, and a NULL `out_field` is accepted.
  - **reconfigure**: invalid AGC settings leave the handle unchanged.
- [ ] T029 [P] [US2] Extend the C smoke test in `crates/rr_dr60_ffi/tests/c/`:
  - set `agc_release_ms = 3000`, create, process and destroy;
  - set `agc_attack_ms = 0` and check `INVALID_SETTING` plus the field from `rr_dr60_settings_validate`;
  - use `RR_DR60_TAP_AFTER_AGC`.
  - Print `smoke: OK`.
- [ ] T030 [P] [US2] Add a rustdoc doc-test on `AgcSettings` in `crates/rr_dr60/src/settings.rs` that bypasses the AGC, sets `release_ms = 3000.0`, sets `Tap::AfterAgc`, and builds a pipeline (SC-001).

### Implementation for User Story 2

- [ ] T031 [US2] Implement the C validation in `crates/rr_dr60_ffi/src/lib.rs` (contracts/c-api.md, research.md R-09):
  - `RrDr60Status::InvalidSetting = 5`.
  - `#[repr(u32)] RrDr60SettingField`: `NONE` 0, `HOST_RATE` 1, `TAP` 2, `STRUCT_SIZE` 3, `AGC_TARGET_DBFS` 4, `AGC_MAX_GAIN_DB` 5, `AGC_MAX_ATTENUATION_DB` 6, `AGC_ATTACK_MS` 7, `AGC_RELEASE_MS` 8.
  - One internal function, `validate_c(&RrDr60Settings) -> Result<Settings, (RrDr60Status, RrDr60SettingField)>`: struct size, then tap, then `to_settings`, then the core validator. The core `Error::InvalidSetting { setting }` maps to the matching field and `InvalidSetting`. `UnsupportedHostRate` maps to `HOST_RATE`.
  - `rr_dr60_settings_validate(settings, out_field)` wraps it, panic-guarded, with a `// SAFETY:` comment on each pointer use.
  - `rr_dr60_create` and `rr_dr60_reconfigure` call `validate_c`.
  - Regenerate the header and confirm the CI header-drift check passes locally.

  Makes T028 and T029 pass.
- [ ] T032 [P] [US2] Add an "AGC (signal-chain stage 3)" section to `README.md`:
  - **Content**: what it does, "modeled on an assumed AGC (A-017–A-020)", and the defaults table.
  - **Rust and C snippets**: bypass, change the release, tap after the AGC.
  - **Overshoot warning**: up to +40 dB at the defaults and +60 dB at most; limit or clip before converting to 16-bit.
  - **Inputs above 0 dBFS** are valid.
  - No EVP claims either way (Principle VI).
- [ ] T033 [US2] Run `us2_agc_settings`, `ffi_parity` and `crates/rr_dr60_ffi/tests/c/run_smoke.sh`, and fix until green.

**Checkpoint (US2)**: `us1_agc`, `us2_agc_settings` and `ffi_parity` pass, and the smoke test prints `smoke: OK`. US2 is validated independently of US3.

---

## Phase 5: User Story 3 - The harness measures the AGC (Priority: P3)

**Goal**: Every AGC tolerance in the spec is checked at every host rate and reported with its IDs, and the AGC output is guarded by golden files.

**Independent Test**: `cargo test -p rr_dr60_harness --test agc_matrix --test agc_edge_cases --test golden_agc --test mutation` passes, and the release-mode `--ignored` matrix passes. A deliberate +50 % release or +3 dB target fails both a tolerance check and a golden check.

### Harness checks and tests (write first; each must fail against a deliberately wrong AGC)

- [ ] T034 [US3] Create `crates/rr_dr60_harness/src/agc_checks.rs`, one public function per requirement (research.md R-11). Each takes `(rate, &AgcSettings, Make)`, returns `Vec<MeasurementResult>` citing `002/FR-0xx` plus A-IDs, and labels engineering targets:
  - **`check_envelope_self_test`**: at 8 kHz, the envelope method agrees with the exact y/x within 0.25 ms on attack and release, and within 0.1 dB on steady levels.
  - **`check_fr004_regulation`**: points 3 dB inside the regulated range from (T − G<sub>max</sub>/0.9) to (T + A<sub>max</sub>/0.9), on the regulation line ± 1 dB.
  - **`check_fr005_limits`**: a sweep from the lower knee − 10 dB to the upper knee + 10 dB in steps of at most 5 dB, with points 3 dB beyond each knee at G<sub>max</sub> ± 1 dB or −A<sub>max</sub> ± 1 dB.
  - **`check_fr006_timing`**:
    - the step stimulus: 0.5 s low, then ≥ 20 × attack (minimum 0.2 s) high, then ≥ 1.5 × release + 0.5 s low;
    - step placement per FR-012;
    - tolerances: default ±2 ms / ±0.2 s, otherwise ±20 % or ±1 ms;
    - the shape check: 35–65 % at 25 % of the measured release.
  - **`check_fr007_silence`**: digital silence gives exactly 0, and the gain reaches maximum.
  - **`check_fr008_frequency_thdn`**: 300, 500, 1000, 2000, 8000/3 and 3400 Hz at −30, −10 and 0 dBFS, with 4 phases at 2000 and 8000/3 Hz. Level within ±0.5 dB of 1 kHz, and THD+N ≤ 1 %.
  - **`check_fr009_noise_rise`**: the stimulus from T016. The last 1 s of each pause is at −30 dBFS ± 2 dB (AES17). The first 50 ms after each burst, counted after the boundary latency, is at least 20 dB lower.
  - **`check_fr010_latency`**.
- [ ] T035 [US3] Write `crates/rr_dr60_harness/tests/agc_matrix.rs`:
  - **Normal run**: (a) the defaults at all 6 rates; (b) the US2 examples (release 3 s, target −20) plus the minimum and maximum of each setting with the others at default, at 8 kHz and 48 kHz. Prints the report with `--nocapture`.
  - **`#[ignore]` test `full_matrix_all_rates`**: the full settings set at all 6 rates, for the release-mode job.
  - **Runtime**: measure the normal-run time and record it in the test's module docs. If it exceeds 60 s in debug, move cases to the ignored test.
  - **First run**: before T022 exists, or against a stage with 0.5 × release, at least one check per function must fail. Record that in the PR.
- [ ] T036 [P] [US3] Write `crates/rr_dr60_harness/tests/agc_edge_cases.rs`, covering spec Edge Cases and research.md R-11, at 8 kHz and 48 kHz unless stated:
  - **Start**: a fresh pipeline at 8 kHz has a first-sample gain of `max_gain_db` ± 0.01 dB.
  - **Reset and reconfigure**: after a loud tone, `reset()` and `reconfigure(same settings)` each give output bit-identical to a fresh pipeline.
  - **Zero gain range**: max gain = max attenuation = 0 gives output = input within ±0.01 dB at −60, −10 and +10 dBFS.
  - **10-minute flush**, in release mode as `#[ignore]`: through the pipeline at 8 kHz with `max_attenuation_db = 0` and a 0 dBFS tone. Once the gain has flushed to 0, output equals input bit for bit, and there are never any subnormals.
  - **DC**: a = 0.1 gives steady output a · 10^(9/20) ± 1 dB (G<sub>t</sub> = 9 dB at defaults).
  - **Low frequency**: a 50 Hz tone at −20 dBFS gives a mean gain at least 20 dB below the silence gain.
  - **Above 4 kHz**, at 48 kHz: 1 kHz at −40 alone vs. plus 6 kHz at −10. The 1 kHz output level agrees within 0.1 dB.
  - **Non-finite**: NaN, +Inf and −Inf mid-stream give output bit-identical to the same stream with 0.0 in their place.
  - **Overshoot**: `agc_start_m10` peaks about +30 dBFS, unclipped and finite, and an input of 1e36 gives finite output.
- [ ] T037 [US3] Generalize `crates/rr_dr60_harness/src/golden.rs`:
  - `GoldenSet { file, stimuli, configs }`, with the 001 set unchanged.
  - The AGC set: stimuli `agc_noise_burst`, `agc_start_m10`, `agc_step` (contracts/golden-format.md, exact durations); configs `agc_only` and `default_agc`.
  - `committed_agc()` embeds `golden/golden-agc-v1.json`.
  - Blessing: `RR_DR60_BLESS=agc` rewrites only the AGC file. `RR_DR60_BLESS=1` keeps its 001 meaning, and no task in this feature runs it.
- [ ] T038 [US3] Add the `golden_agc_matches` test to `crates/rr_dr60_harness/tests/golden_agc.rs`. Bless `crates/rr_dr60_harness/golden/golden-agc-v1.json` with `RR_DR60_BLESS=agc cargo test -p rr_dr60_harness --test golden_agc`: 36 entries, about 15 KB. Add a CHANGELOG line for the new golden file.
- [ ] T039 [P] [US3] Extend `crates/rr_dr60_harness/tests/mutation.rs` (SC-008) with two mutants built through `Make`, with no hidden hooks: `release_ms × 1.5`, and `target_dbfs + 3.0`. For each, assert that `check_fr006_timing` or `check_fr004_regulation` reports a failure, and that `golden::compare` against `committed_agc()` reports a difference.
- [ ] T040 [P] [US3] Extend `crates/rr_dr60_harness/tests/determinism.rs` and `crates/rr_dr60_harness/tests/alloc_free.rs` (FR-013, SC-003, SC-004):
  - **Determinism**: the AGC golden stimuli in 100 seeded random block partitions, including sizes 0 and 1, are bit-identical, for `agc_only`, `default_agc` and the attack-minimum and release-maximum settings.
  - **Allocation**: zero allocations and deallocations over 1000+ blocks for the same configurations, through both the Rust and C paths.
- [ ] T041 [P] [US3] Add `default_agc` to the release-mode timing test in `crates/rr_dr60_harness/tests/timing.rs`, keeping max/min ≤ 3.0 (SC-006 ≥ 20× real time at 48 kHz).
- [ ] T042 [US3] Run the US3 tests and the ignored matrix in release mode (`cargo test -p rr_dr60_harness --release -- --ignored`) and fix until green.

**Checkpoint (US3)**: `cargo test --all-features` and the ignored release-mode tests pass, and the report lists every 002 FR with its IDs. All three stories have now been validated independently.

---

## Phase 6: CI and cross-platform golden files (SC-003)

- [ ] T043 Update `.github/workflows/ci.yml`:
  - **`check` job**: add `--test golden_agc` next to `--test golden`, and add `cargo test -p rr_dr60_harness --release --test agc_matrix --test agc_edge_cases -- --ignored` after the timing step.
  - **`golden-matrix` and `ios` jobs**: add `--test golden_agc` to the test commands.
  - Update the header comment.
- [ ] T044 [P] Add `--test golden_agc` to the dinghy command in `scripts/ios-device-golden.sh`. In `docs/release-checklist.md`, note that the iOS-device run now covers both golden files.
- [ ] T045 Push `002-agc` and open a **draft PR**, only after asking the user. Confirm every CI job is green, including `golden_agc` on the desktop targets and the iOS simulator.

**Checkpoint**: CI is green on every automated target. The iOS-device check stays a manual release gate.

---

## Phase 7: Polish & cross-cutting concerns

- [ ] T046 [P] Update `CHANGELOG.md` `[Unreleased]`:
  - **Added**: the AGC (stage 3, spec 002), `AgcSettings`, `Setting`, `Tap::AfterAgc`, `Error::InvalidSetting`, the C AGC fields, `RR_DR60_TAP_AFTER_AGC`, `RR_DR60_STATUS_INVALID_SETTING`, `RrDr60SettingField`, `rr_dr60_settings_validate`, and `golden-agc-v1.json`.
  - **Changed**:
    - Default output now includes the AGC.
    - `Settings` and the C `RrDr60Settings` are no longer `Eq`/`Hash`.
    - The C struct is now 48 bytes.
    - `narrow_out` saturates instead of producing Inf.
    - detmath now runs on the processing path.
  - **Assumptions**: A-017 to A-020.
- [ ] T047 [P] Update `docs/hardware/signal-chain.md`: in row 3, add "**Modeled in [spec 002](../../specs/002-agc/spec.md): A-017–A-020.**". Leave the A-017 to A-020 status in `docs/hardware/assumptions.md` as `assumed`.
- [ ] T048 [P] Add a note to `specs/001-pipeline-skeleton/research.md` R-06: "Amended by spec 002 R-06: `ln`/`exp` also run on the processing path."
- [ ] T049 [P] Update the "Current status" section of `CLAUDE.md` with feature 002, the new test names (`agc_matrix`, `agc_edge_cases`, `golden_agc`), and the AGC bless command.
- [ ] T050 Run every step of `specs/002-agc/quickstart.md` § 1–6 and the coverage gate `cargo llvm-cov --all-features --workspace --fail-under-lines 80`. Fix anything that fails.
- [ ] T051 Ask the speckit-coach for a final review, then ask the user before merging the PR.

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (Phase 1)**: no dependencies. T002 → T003.
- **Foundational (Phase 2)**: depends on Setup. It blocks every story.
- **US1 (Phase 3)**: depends on Phase 2.
- **US2 (Phase 4)**: depends on Phase 2.
  - T027's AS3 and AS4 and T028's parity cases need the AGC stage (T022, T023) to show non-trivial output. Run US2 after US1, or write its tests in parallel with US1 and expect them to fail until T023.
- **US3 (Phase 5)**: depends on US1 (it measures the stage). It is independent of US2, except that T040's C path uses the T018 fields.
- **CI (Phase 6)**: depends on US3 (T038, the golden file).
- **Polish (Phase 7)**: depends on everything else.

### Task-level dependencies

| Task | Depends on |
|---|---|
| T011 | T004 |
| T012 | T005, T006, T011 |
| T013 | T007 |
| T014 | T011 |
| T015, T016 | T009 |
| T017 | T011 |
| T018 | T011, T001 |
| T019 | T017, T018 |
| T022 | T003, T008, T020 |
| T023 | T022, T014 |
| T024 | T023 |
| T026 | T021, T023 |
| T031 | T012, T018, T028 |
| T035 | T034 |
| T038 | T037, T023 |
| T039 | T034, T038 |
| T043 | T038 |
| T045 | T043, and the user's go-ahead |

### Within each story

Tests come before implementation, and the story checkpoint passes before the next story starts (Principle VII).

---

## Parallel Examples

### Phase 2 (tests, then implementation)

```text
T004, T005, T006, T007, T008, T009, T010   # all different files
then T011 → T012; in parallel: T013, T015, T016
then T014, T017, T018 → T019
```

### User Story 1

```text
T020 (core unit tests) ∥ T021 (harness us1_agc) ∥ T025 (docs)
then T022 → T023 → T024 → T026
```

### User Story 2

```text
T027 ∥ T028 ∥ T029 ∥ T030 ∥ T032
then T031 → T033
```

### User Story 3

```text
T034 → T035; in parallel: T036, T037 → T038 → T039; T040, T041
then T042
```

---

## Implementation Strategy

### MVP first (User Story 1)

1. Phases 1–2: version, coefficients, settings, validation, helpers, and the move of all 001 tests to AGC-bypassed settings. Everything green, `golden-v1.json` untouched.
2. Phase 3: the AGC stage. Stop at the checkpoint, demo (quickstart § 7), and review with the coach.

### Incremental delivery

1. **US2**: control from Rust and C. Validate it independently.
2. **US3**: the full measurement matrix, edge cases, golden files and mutation tests.
3. **CI**: wiring for all targets.
4. **Polish**, then the merge, with the user's approval.

### Risk notes

- **The prototype was float64 Python** (research.md R-02, R-05). If Rust results differ by more than rounding, compare against the prototype numbers in R-05 before changing tolerances. Never widen a spec tolerance to make a test pass. Raise it with the user instead.
- **Debug-mode runtime** of the AGC matrix: T035 measures it, and slow cases move to the ignored release-mode job.
- **The 10-minute flush test** is slow in debug; it is `#[ignore]` and runs in release-mode CI.
- **The C struct grows to 48 bytes**: any downstream C code compiled against 0.1 must be rebuilt. 0.1 was never released.

## Notes

- `[P]` tasks touch different files and have no unfinished dependencies.
- Commit after each task or logical group, using Conventional Commits.
- Every value cites an A-ID, an S-ID or an engineering target (SC-007).
