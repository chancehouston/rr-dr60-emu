---

description: "Task list for 006 Record and Playback Paths with a Modeled Memory"
---

# Tasks: Record and Playback Paths with a Modeled Memory

**Input**: Design documents from `specs/006-record-playback-split/`: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/](contracts/) (rust-api, c-api, recording-form, golden-format), [quickstart.md](quickstart.md).

**Tests**: REQUIRED. Constitution III says measurement tests are written first and must fail before the implementation exists. Every phase lists its tests before its implementation tasks. "Fail first" has three shapes here: the narrowing commit's tests fail against the un-narrowed one-pass (Phase 2A); the refactor's proof is that nothing fails (Phase 2B: the SHA guards do not move); the story tests fail because the calls, modules and C functions do not exist until their implementation tasks land.

**Organization**: Setup → Foundational A (the one `f32` narrowing with its measurement and the single golden re-bless, FR-009) → Foundational B (the refactor into shared halves, settings, errors; goldens unchanged) → US1 (P1, MVP: record, stop, play, state, bit-identical to the one-pass) → US2 (P2: the memory's limits, erase, status) → US3 (P3: export and import; then the seven-rate matrix and the cross-cutting harness) → US4 (P4: the C API) → Records → CI → Polish and the final reviews.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: The user story the task belongs to (US1 – US4)
- Paths are relative to the repository root.

## Conventions that apply to every task

- **Traceability**: cite spec 006 FR, SC and A-/S-IDs in code comments, rustdoc and test names, e.g. `// S-001: 60 min / 99 items`, `// S-002: 16 Mbit flash; amp muted while recording`, `// A-026: 6 kHz device rate`, `// A-037: reset per recording (engine decision)`, `// A-038: capacity in stored device time`, `// A-039: stop at the limit; empty items not created`, `// A-003: 8-bit µ-law at this point (narrowing defence)`. Constants that are not device properties are commented `// engineering target (006 FR-0xx)` or `(006 R-xx)`; the list is research R-12. Harness results cite requirements as `006/FR-0xx`.
- **Determinism**: only basic IEEE operations and integer arithmetic on the processing paths (the narrowing is `narrow_out`, a round-to-nearest cast); `rr_dr60_detmath` as before. No platform libm (`clippy.toml`). The CRC is integer arithmetic.
- **Real-time safety**: the per-block `record` and `play`, `start_*`, `stop_*`, `set_route`, the state and figure reads never allocate, format or do I/O; the store and the span table are reserved in `new`/`reconfigure` with `try_reserve_exact`; `erase*` move memory but do not allocate; `export`/`import` use caller buffers. `alloc_free` asserts it (T051).
- **The golden window**: it is **open only inside the uncommitted work of T008 – T010** (the narrowing). The re-bless happens exactly once, in T010, with the CHANGELOG figures, and the three files, their SHA guards and `library_version` land in that one commit with nothing else. **From that commit on the window is closed**: `golden`, `golden_agc`, `golden_vas` and their guards must stay green through every later task, and no task may touch `crates/rr_dr60_harness/golden/*.json`. A moved hash is a defect, never a re-bless.
- **Never widen a spec tolerance** to make a test pass. If a derived figure (N₁₀, a latency, a drain length, the delta bound) disagrees with the plan, the plan is wrong: stop, recompute (research R-02, R-06), and record the correction in the spec's Clarifications.
- **Story boundaries (Principle VII)**: US1's checks run record-then-play **in one engine** and compare with the committed goldens and the one-pass, never through `import`; US2 uses the record path at a small capacity and US1's playback as the only public read of a recording's content; US3 compares exports and imports; US4 compares C with Rust. No story test depends on another story's harness code beyond `split_checks.rs`'s shared runner, `stimulus.rs` and `configs.rs`.
- **Before ticking a task**: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all-features` must pass (inside T008 – T010's work, the three golden tests are the expected red until the re-bless).
- **Commits**: Conventional Commits, on branch `006-record-playback-split`. T008 + T009 + T010 are one `feat!:` commit. Merge to `main` only through a green PR, and only after the user says so.

---

## Phase 1: Setup

**Purpose**: Version, the baseline numbers the CHANGELOG and the PR need, and the plan's derived figures pinned where the tests will read them.

- [x] T001 Bump the workspace version to 0.6.0: `version = "0.6.0"` in `[workspace.package]` and the three internal dependency entries of `Cargo.toml`; `RR_DR60_VERSION_MINOR = 6` in `crates/rr_dr60_ffi/src/lib.rs`; regenerate `crates/rr_dr60_ffi/include/rr_dr60.h` (`cbindgen --config crates/rr_dr60_ffi/cbindgen.toml --crate rr_dr60_ffi --output crates/rr_dr60_ffi/include/rr_dr60.h`). Golden comparison ignores `library_version`, so every test still passes.
- [x] T002 [P] Record the 0.5 baseline for the PR and the CHANGELOG (research R-11): wall time of `cargo test --all-features`; the `ns/sample` rows of `cargo test -p rr_dr60_harness --release --test timing -- --ignored --nocapture` (59.6 / 58.1 / 58.1 expected); the default latency table of `cargo test -p rr_dr60_harness --test latency -- --nocapture` (1/81/162/444/483/887/966); the three current SHA guards (`GOLDEN_V1_SHA256` in `crates/rr_dr60_harness/tests/golden.rs`, `GOLDEN_AGC_V1_SHA256` in `golden_agc.rs`, the VAS guard in `golden_vas.rs`). Keep them in a scratch note until T056.
- [x] T003 [P] Pin the plan's derived figures as a commented constant block at the top of a new `crates/rr_dr60/src/paths.rs` test module stub (research R-06; data-model › Per-rate figures), with the line `// derived 2026-10-11 from rate.rs and voiceband_coeffs.rs (006 research R-06); the tests below recompute each`: D = 0/40/80/219/238/437/476; record latency 1/41/81/223/242/445/485; playback latency 0/40/81/221/241/442/481; one-pass 1/81/162/444/483/887/966; drain 121/242/483/1328/1444/2653/2888; N₁₀ = 121; τ_rec = 0.5387, τ_play = 0.3133; ‖h₁₀‖₁ = 2.65, max interpolator branch ‖b‖₁ = 2.33. (The file is created here with the constants and `#![allow(dead_code)]`; T014 fills it.)

**Checkpoint**: the workspace builds at 0.6.0 and every test passes; the baseline and the pinned figures are recorded. *(Reached: commit 46a5cdb (2026-10-11).)*

---

## Phase 2A: Foundational — the narrowing and the single re-bless (FR-009; research R-02, R-09; contracts/golden-format.md)

**Purpose**: Make the one-pass processing round the device-rate stream after the VAS to `f32` in the **existing** `DeviceChain`, measure what it changes, and re-bless the three golden files once. One commit, nothing else in it.

### Tests (write first, must fail against the un-narrowed one-pass)

- [x] T004 [P] Narrowing unit tests in `crates/rr_dr60/src/pipeline.rs` `#[cfg(test)]` (FR-009; research R-02): `narrowing_point_is_after_the_vas`: at 6 kHz with every stage on, feed a `bypass_all`-style input of values that are not `f32`-exact after stage 4 (a −20 dBFS tone) and assert that the sample stage 10 receives equals `f64::from(narrow_out(post_vas))` (observe through the `__with_device_narrowing(settings, false)` hook of T005: the two pipelines' outputs differ, and the narrowed one equals a manual re-run that rounds the post-VAS sample); `bypass_all_at_6k_is_bit_identical_to_input` (no arithmetic before the narrowing); `after_agc_tap_never_narrows` (output equals the limiter's `f32` output as today); mute mode's muted samples narrow to +0.0 exactly.
- [x] T005 [P] L1-norm pin test in `crates/rr_dr60/src/stages/voiceband.rs` `#[cfg(test)]` (research R-02, review P2): compute ‖h₁₀‖₁ from `PLAYBACK_SOS` by running a unit impulse through the cascade for 4 000 samples (sum of |h|) and assert 2.65 ± 0.01; compute the largest interpolator branch L1 norm from `resample::converters` at 48 kHz and assert 2.33 ± 0.01 (in `crates/rr_dr60/src/resample/mod.rs` tests); state in the test comment that the `narrowing_delta` bound (4 full-scale LSBs) is derived as 0.5 ulp × (‖h₁₀‖₁ + max‖b‖₁ + … ) ≈ 2.05 worst case.
- [x] T006 [P] `crates/rr_dr60_harness/tests/narrowing_delta.rs` (FR-009; contracts/golden-format.md; review P1, P2): for every entry of the three golden sets at the seven rates, run the stimulus through `Pipeline::__with_device_narrowing(settings, false)` and `(…, true)`; report per file the share of entries and of samples that differ and the maximum |Δ| in **full-scale LSBs** (2⁻²³ × max(1, peak |un-narrowed output|)); assert max ≤ 4; assert that **exactly** the four `bypass_all` entries at 6 kHz and the 21 `agc_only` entries are unchanged and every other entry is allowed to change (a narrowing at the wrong point, e.g. before the clamp or at the interpolator's output, would move that set); print the figures in the form the CHANGELOG quotes. It fails until T008 because the hook does not exist. *(Measured at T009: the unchanged set is 39, not 25; the rule and the correction are in research R-02 and the spec's Clarifications.)*
- [x] T007 Add `#[test] fn narrowing_delta_report_is_stable()` to T006's file: run twice, identical report (determinism of the measurement).

### Implementation (T008 – T010 are ONE commit: `feat!: narrow the device-rate stream after the VAS to f32; re-bless the golden files once (006 FR-009)`)

- [x] T008 In `crates/rr_dr60/src/pipeline.rs`: add `fn narrow_device(d: f64) -> f32 { crate::sanitize::narrow_out(d) }` (rustdoc: the point where the recording is taken; A-003's 8-bit µ-law at this point as the inaudibility defence, not a fidelity claim); in `DeviceChain::process`, after the VAS decision and before stage 10, round the kept sample (`Keep`, `Resume`, and the muted `0.0`) through `narrow_device` and widen back with `f64::from` for stage 10; add the `__test-hooks`-gated field `narrow: bool` (default true) with the branch under `cfg(feature = "__test-hooks")` only, and `Pipeline::__with_device_narrowing(settings, enabled) -> Result<Self, Error>`; export the hook through `lib.rs`'s `__test_hooks` as needed. Shipped build: no field, no branch. *(Built as spec Clarifications T014: `RecordOut` carries the 64-bit sample plus a `Mute` variant, `RecordPath::push` narrows, and the un-narrowed flag lives on `OnePass`; the narrowing point is unchanged.)*
- [x] T009 Run `cargo test -p rr_dr60_harness --test narrowing_delta -- --nocapture` and paste the printed figures (per file: entries changed / total, share of samples, max |Δ| in full-scale LSBs) into the CHANGELOG `## [Unreleased]` `### Changed` entry for 0.6.0 (the entry's full text is T057's; here only the narrowing paragraph and the table row "Golden entries: 112 / 42 / 63, re-blessed once for the f32 narrowing; max |Δ| … ; the 6 kHz bypass_all and agc_only entries byte-identical"), and into research R-02's "Measurement" bullet as **Measured (T009, date)**.
- [x] T010 Re-bless: `RR_DR60_BLESS=1 cargo test -p rr_dr60_harness --test golden`, `RR_DR60_BLESS=agc … --test golden_agc`, `RR_DR60_BLESS=vas … --test golden_vas`; update `GOLDEN_V1_SHA256`, `GOLDEN_AGC_V1_SHA256` and the VAS guard; confirm entry counts 112 / 42 / 63 and `library_version` 0.6.0; `cargo test --all-features` fully green. Commit T008 + T009 + T010 as the one `feat!:` commit. **The golden window is closed here.**

**Checkpoint**: the full suite green at the re-bless commit; `narrowing_delta` reports the committed figures; `git show --stat HEAD` lists only `pipeline.rs`, `lib.rs`, `voiceband.rs`/`resample/mod.rs` tests, `narrowing_delta.rs`, the three golden files, the three guard lines, `CHANGELOG.md`, `research.md`. *(Reached: commit 830d924 (2026-10-11, the single `feat!:` narrowing commit).)*

---

## Phase 2B: Foundational — the shared halves, settings and errors (research R-01, R-05, R-07; data-model › Paths, Settings, Errors)

**Purpose**: Refactor the one-pass onto the halves and paths every story builds on, proven by the unchanged SHA guards; add the settings and error surface. No behaviour change.

### Tests (write first)

- [x] T011 [P] Settings tests in `crates/rr_dr60/src/settings.rs` `#[cfg(test)]` (FR-010; contracts/rust-api.md): `MemorySettings::DEVICE == { capacity_s: 3600, max_recordings: 99 }` citing S-001; `Settings::new(48_000).memory == MemorySettings::DEVICE`; `Route::default() == Route::Speaker`; validation in `crates/rr_dr60/src/validate.rs` tests: `capacity_s` 0 and 86 401 → `InvalidSetting { MemoryCapacityS }`, 1 and 86 400 accepted; `max_recordings` 0 and 10 000 → `InvalidSetting { MemoryMaxRecordings }`, 1 and 9 999 accepted (engineering targets, 006 FR-010).
- [x] T012 [P] Error tests in `crates/rr_dr60/src/error.rs` `#[cfg(test)]`: every new variant (`RecordingInProgress`, `PlaybackInProgress`, `NotRecording`, `NotPlaying`, `MemoryFull`, `NoRecordingSlots`, `NoSuchRecording { number }`, `DoesNotFit`, `UnknownForm`, `UnsupportedFormVersion { version }`, `Damaged`, `StoredExceedsCapacity`, `AllocationFailed`, `BufferTooSmall { needed }`) has a `Display` that names the reason in the spec's words and includes its field; `Setting::MemoryCapacityS` and `MemoryMaxRecordings` display their ranges.
- [x] T013 [P] Paths unit tests in `crates/rr_dr60/src/paths.rs` `#[cfg(test)]` (research R-01, R-06; review P10): `record_path_then_playback_path_equals_one_pass_at_6k_and_48k` (a synthetic stream through `RecordPath::push` → `PlaybackPath::push_device`/`try_next_host` versus `Pipeline::process`, bit-identical, with the VAS on in drop and mute mode and with everything bypassed); `record_path_owns_counters` (`take_event()` returns each event once; positions in device samples when `l = m = 1`); `identity_rate_pending_sample` (at 6 kHz `push_device` then one `try_next_host` returns it once, a second returns `None`); `playback_length_is_ceil_k_m_over_l` for K = 1 … 50 at the seven rates; `drain_len_table` equals the T003 constants; `record_and_playback_latency_table` equals the constants and their sum equals `latency_samples()` at every rate; `n10_from_committed_coefficients` computes 121 from `PLAYBACK_SOS` (first index after which |h| < 1e-5 · peak) and asserts the constant; `tau_rec_tau_play` 0.5387 / 0.3133 ± 1e-3.

### Implementation

- [x] T014 Fill `crates/rr_dr60/src/paths.rs` (created as a constants stub by T003; FR-001, FR-002; research R-01; data-model › Paths): move `EventCounters` and `DeviceOut` logic here; `RecordHalf { record, run_record, clamp, vas, run_vas, mute }` with `process(f64) -> RecordOut` (`Keep(f32)`, `Resume(f32)`, `Drop`; the `f32` through `narrow_device`, moved here from `pipeline.rs`); `PlaybackHalf { playback, run_playback }` with `process(f32) -> f64`; `RecordPath { limiter, run_limiter, down: Option<PolyphaseDown>, half, counters }` with `new(settings, honour_tap: bool, device_positions: bool)`, `push(f32) -> Option<f32>`, `take_event()`, `reset()`, `latency_host_samples()` (= round(D + τ_rec·host/6000) or 0 when stage 4 is off); `PlaybackPath { half, up: Option<PolyphaseUp>, pending: Option<f32> }` with `push_device`, `try_next_host`, `reset`, `latency_host_samples()`, `drain_len_host_samples()` (⌈N₁₀·m/l⌉ + 2D, N₁₀ = 0 when stage 10 is off); constants `N10: usize = 121` with its rustdoc derivation; the `__test-hooks` narrowing flag moves into `RecordHalf`. Register `mod paths;` in `lib.rs`. *(Built as spec Clarifications T014: `RecordOut` carries the 64-bit sample plus a `Mute` variant, `RecordPath::push` narrows, and the un-narrowed flag lives on `OnePass`; the narrowing point is unchanged.)*
- [x] T015 Rewrite the one-pass in `crates/rr_dr60/src/pipeline.rs` on the paths: `struct OnePass { rec: RecordPath, play: PlaybackPath, tap_after_agc: bool }`; `tick(x)` as research R-01 (limiter-only at the AfterAgc tap; else `rec.push` → `play.push_device`, then `play.try_next_host().map(narrow_out)`); `run_block` drains `rec.take_event()`; `latency_samples()` keeps 001 R-10's single rounding (computed from the plan and the halves' group delays, not as the sum of the two path figures); `max_events` unchanged; `reset`/`reconfigure` unchanged in behaviour; delete `DeviceChain`. **Proof**: `cargo test --all-features` green with all three SHA guards unchanged; `cargo test -p rr_dr60_harness --test narrowing_delta` prints the T009 figures unchanged.
- [x] T016 Settings and errors: `MemorySettings` + `DEVICE`, `Settings::memory`, `Route` in `crates/rr_dr60/src/settings.rs`; `Setting::MemoryCapacityS`, `MemoryMaxRecordings` with ranges 1 ..= 86 400 and 1 ..= 9 999 in `crates/rr_dr60/src/error.rs` and `validate.rs`; the fourteen `Error` variants with `Display`; re-exports in `lib.rs` (`MemorySettings`, `Route`; the transport types come in T022). Settings validation order: host rate → AGC → VAS → memory.
- [x] T017 FFI compiles against the new settings without the new calls yet: `crates/rr_dr60_ffi/src/lib.rs` gains `memory_capacity_s` (offset 76) and `memory_max_recordings` (80), `struct_size` 88, defaults, `to_settings`, `RrDr60SettingField` 14 and 15, the status codes 6–19 appended to `RrDr60Status` and mapped in `error_to_status_and_field`; the layout test asserts 88 and the two offsets; `settings_layout_is_80_bytes…` renamed and restated; a `struct_size = 80` struct rejected as `STRUCT_SIZE`; regenerate the header; `crates/rr_dr60_ffi/tests/c/smoke.c` updated for `struct_size == 88` and the two defaults (its loop comes in T047). Harness `configs.rs`: a `memory_capacity(settings, secs, max)` helper.

**Checkpoint**: `cargo test --all-features` green; the three SHA guards and `narrowing_delta`'s figures unchanged by the refactor (T015's proof); clippy and fmt clean; `scripts/check-traceability.sh` OK. Commits: `refactor(core): shared record/playback halves and paths behind the one-pass (006 R-01)`, `feat(core): memory settings, route, the transport error variants (006 FR-010, FR-020)`. *(Reached: commit 86caa40 and eee9677 (2026-10-11).)*

---

## Phase 3: User Story 1 — Record, stop, play (Priority: P1) 🎯 MVP

**Goal**: `start_recording` / `record` / `stop_recording` / `start_playback` / `play` / `stop_playback` / `set_route` / `state`, with the memory at its default capacity and its limits unexercised; playback bit-identical to the one-pass processing and to the committed goldens; the drain; the latencies.

**Independent Test**: `cargo test -p rr_dr60_harness --test us1_record_play -- --nocapture` (one engine per case; no import; no capacity limit).

### Tests for User Story 1 (write first, must fail: the calls do not exist)

- [x] T018 [P] [US1] Memory core unit tests in a new `crates/rr_dr60/src/memory.rs` `#[cfg(test)]` (research R-03; data-model › Memory): `reserves_capacity_once` (`store.capacity() == capacity_samples` after `new`, after `append` to the brim, after `close`); `open_append_close_numbering` (two recordings → spans contiguous, Σ len == store.len(), numbers 1 and 2); `close_without_input_discards` (`close(fed_any = false)` → `DiscardedEmpty`, span popped); `clone_keeps_capacity` (both `Vec` capacities equal after `clone()`); `try_reserve_failure_is_an_error` through `Memory::with_capacity_samples(usize::MAX / 2, 99)` → `Err(AllocationFailed)` (review P7); `footprint_bytes_formula` = `store.capacity() × 4 + spans.capacity() × 16`.
- [x] T019 [P] [US1] Transport unit tests in a new `crates/rr_dr60/src/transport.rs` `#[cfg(test)]` (FR-003 – FR-008, FR-015, FR-019; FR-004's reset per start; data-model › Transitions): every row of the transitions table as one test each or a table-driven test: the refusals in their check order, the states after each call, `speaker_muted` only while recording, `current` = count + 1 while recording and the number while playing, `set_route` during playback takes effect next block and survives `reset` and `reconfigure`, `reset` in any state (recording in progress discarded without flush, store truncated; playback stopped without drain), `reconfigure` refused while active, the one-pass calls refused while active with the pipeline unchanged (clone-compare), `start_recording` resets the recorder (record a +9 dBFS burst, stop, record a −20 dBFS tone: item 2 equals a fresh one-pass of the tone, A-037), `start_playback` resets the player, empty `record(&[])` does not set `fed_any`, after `stopped = MemoryFull` a `stop_recording` is `NotRecording` (use a 1 s capacity).
- [x] T020 [P] [US1] Flush, drain, length and report unit tests in `crates/rr_dr60/src/transport.rs` tests (FR-005, FR-006, FR-007, FR-008; research R-06): `stop_flushes_record_latency_of_zeros` (the stored count grows by the flush's device samples; with `!fed_any` no flush); `playback_produces_ceil_k_m_over_l_then_drain` at the seven rates (`finished` exactly after `drain_samples()` more samples, in blocks of 1, 7 and 4 096); `drain_zero_when_stage10_off_at_6k`; `mute_region_open_at_stop_gets_a_synthesized_event` (position = its start, length = stored − start; review P5b); `events_are_device_positions_from_the_recording_start`.
- [x] T021 [P] [US1] `crates/rr_dr60_harness/src/split_checks.rs` (research R-08) and `crates/rr_dr60_harness/tests/us1_record_play.rs` (US1 scenarios 1–6; FR-007, FR-008, FR-015, FR-016; SC-002, SC-005, SC-008): the shared runner `record_then_play(make, settings, input, record_block, play_block) -> (Vec<f32> pre_drain, Vec<f32> drain, Vec<VasEvent> device_events, bool last_paused, StopOutcome)`; `golden_equivalence(rate)` over `golden::applicable_entries()` (new in `golden.rs`: the 154 default-tap entries as they are, the 42 `vas_only`/`vas_mute` with `playback_stage_enabled = false`, `agc_only` excluded; asserts the count 196 of 217 across rates) comparing the SHA-256 of the first `n_samples` pre-drain samples with the entry's `sha256`, and for the VAS entries the events converted with running sums (position ⌈kept·m/l⌉; drop length ⌊cum·m/l⌋ − ⌊cum_prev·m/l⌋; mute length ⌈end·m/l⌉ − ⌈start·m/l⌉) and the last `record` call's `paused` (review P4); `live_equivalence(rate)` against the one-pass of input + `record_latency + ⌈m/l⌉` zeros (prefix rule) with length exactly ⌈K·m/l⌉, two partitions (4 801 / 997 and 1 / 1); `length_rule(rate)` (K = 1 … 50 recorded at 6 kHz with every stage bypassed, engine reconfigured to `rate` before playback); `route_identity(rate)`; `two_items(rate)` (A-037); `cross_rate_tone(48_000, 44_100)` (−20 dBFS 1 kHz 5 s, VAS off, `analysis::fit` over the last second, ±0.1 dB / ±0.1 Hz; reconfigure keeps the memory); `drain_level(rate)` (0 dBFS 1 kHz 1 s, VAS off, stopped on a `peak_phase` positive peak; last 1 ms of the drain ≤ −80 dBFS; `drain_samples()` equals the produced drain; `finished` only then); `latency_sum(rate)` (sum within 1 of `latency_samples()`; the playback path's 1 kHz group delay against `playback_latency_samples()` ±1, stage 10 on and off; review P8's argmax |y| of a K = 1 playback is report-only, see the spec's Clarifications, T025); `state_table(rate)` (every (state, call) pair with its reason, state unchanged after a refusal). Each a `MeasurementResult` with `006/FR-xxx` and trace IDs.

### Implementation for User Story 1

- [x] T022 [US1] `crates/rr_dr60/src/memory.rs` (FR-010, FR-017; research R-03): `Memory`, `Span`, `MemoryStatus`; `new(settings) -> Result<Self, Error>` via `with_capacity_samples(capacity_samples, max_recordings)` using `Vec::new()` + `try_reserve_exact` (map `TryReserveError` to `AllocationFailed`); `open`, `append -> bool`, `close(fed_any) -> StopResult`, `span(n)`, `status()`, `footprint_bytes()`, manual `Clone` preserving both capacities, debug assertions of the invariants; `#[cfg(any(test, feature = "__test-hooks"))] fn capacities(&self) -> (usize, usize)` (review P6). `erase*` and `import_samples` come in US2/US3 but their signatures may be stubbed as `todo!()`-free `unimplemented` returning `Err(...)` only if a test needs the symbol; prefer adding them in their stories.
- [x] T023 [US1] `crates/rr_dr60/src/transport.rs` (research R-05; data-model › Transport): `State`, `Route`, `EngineState`, `StopReason`, `RecordInfo`, `StopResult`, `StopOutcome`, `PlayInfo` (all `#[non_exhaustive]`, `Copy`, `Default` where sensible, infos `#[must_use]`); the player cursor (span index, offset, drained count); the recorder flags (`fed_any`); and `impl Pipeline { start_recording, record, max_record_events, stop_recording, start_playback, play, stop_playback, set_route, route, state, memory_status, recording_length, record_latency_samples, playback_latency_samples, drain_samples, footprint_bytes }` as contracts/rust-api.md, with the check orders of the transitions table; `record` appends through `RecordPath::push` and reports `RecordInfo` (the `MemoryFull` stop at the exact sample belongs to US2 but the plumbing, `append -> false` → close + `stopped`, is written here); `stop_recording` flushes `record_latency_samples()` zeros unless `!fed_any`, synthesizes an open mute region's event, closes; `play` pulls stored samples through `PlaybackHalf` into the interpolator with the `try_next_host` loop, then drains `drain_samples()` of device zeros, then `finished` and idle.
- [x] T024 [US1] Wire `crates/rr_dr60/src/pipeline.rs`: `Pipeline { settings, one_pass, recorder: RecordPath (honour_tap false, device positions), player: PlaybackPath, memory: Memory, transport: Transport, route: Route }`; `new` builds all and returns `AllocationFailed` from the memory; `reset` (any state; discard without flush; stop playback; paths reset; memory and route kept); `reconfigure` (idle only → `RecordingInProgress`/`PlaybackInProgress`; keeps the memory when the capacity is unchanged, re-reserves and copies when it grew, `StoredExceedsCapacity` when the stored recordings or count would not fit; route kept); the one-pass calls refuse while active; rustdoc for the two paths, the state model, the real-time-safe list; `lib.rs` re-exports and crate docs (two paths, memory, state, route, stored form, "capacity above 60 minutes is not the device's").
- [x] T025 [US1] `golden::applicable_entries()` in `crates/rr_dr60_harness/src/golden.rs` and the `split_checks` runner's event conversion helpers; make T021's tests green at the seven rates; `cargo test -p rr_dr60_harness --test us1_record_play -- --nocapture` prints the 196-entry count, the lengths, the drain figures and the latency table.

**Checkpoint (MVP)**: `cargo test -p rr_dr60 paths memory transport` and `cargo test -p rr_dr60_harness --test us1_record_play` green at seven rates; the three golden tests and their guards unchanged; clippy, fmt, traceability clean. Demo for the user: the `us1_record_play` report (196 of 217 goldens reproduced, drain levels, latency sums). Coach review of the MVP before Phase 4. Commit: `feat(core): record and playback paths with a modeled memory — record, stop, play, state, route (006 US1)`.

---

## Phase 4: User Story 2 — A memory that fills and empties like the device's (Priority: P2)

**Goal**: the limits at the exact sample with their reasons, paused time costing nothing in drop mode and memory in mute mode, numbering and erase, the figures, the configurable capacity.

**Independent Test**: `cargo test -p rr_dr60_harness --test us2_memory -- --nocapture` (record path only, capacity 10 s and 3 recordings).

### Tests for User Story 2 (write first, must fail: erase and the exact-sample stop do not exist)

- [x] T026 [P] [US2] Memory limit and erase unit tests in `crates/rr_dr60/src/memory.rs` tests (FR-011 – FR-013; A-038, A-039; S-001): `append_returns_false_exactly_at_capacity`; `open_refused_when_full_or_no_slot` (`MemoryFull` before `NoRecordingSlots` when both); `erase_renumbers_and_returns_time` (five recordings, erase 3 → four, the former 4 and 5 at 3 and 4, their samples unchanged, remaining grows by the erased length, Σ len invariant); `erase_all_empties_and_keeps_capacity`; `status_figures_exact` (device samples and the derived seconds). *(Coach review of US1, finding 3: also assert that a close with input fed but nothing stored is `DiscardedEmpty`; the memory unit test `close_with_input_but_nothing_stored_discards` already pins it.)* *(Done; `erase` returns the erased span, so the test also pins the moved-down starts and that erasing 0 or a number above the count is `None`.)*
- [x] T027 [P] [US2] Transport limit tests in `crates/rr_dr60/src/transport.rs` tests (FR-012, FR-019): `recording_stops_at_the_exact_capacity_sample_inside_a_block` (capacity 1 s at 6 kHz: the block that crosses reports `stored` = the remainder and `stopped = Some(MemoryFull)`, the state is idle, the recording is kept with length 6 000); `flush_cut_by_capacity`; `count_limit_only_prevents_start`; `erase_during_playback_of_the_played_item_stops_it`, `erase_of_a_lower_number_renumbers_the_played_item`; `reconfigure_keeps_recordings_and_refuses_when_they_do_not_fit` (`StoredExceedsCapacity`; a larger capacity re-reserves and copies: `capacities()` reflect the new size, playback bit-identical before and after). *(Done. The lower-number case also proves the player's cursor follows the store's move: the interrupted playback is bit-identical to an uninterrupted one.)*
- [x] T028 [P] [US2] `crates/rr_dr60_harness/tests/us2_memory.rs` with the checks in `split_checks.rs` (US2 scenarios 1–8; SC-003, SC-007): `capacity_time(rate)` (10 s capacity, 25 s of −20 dBFS 1 kHz, VAS off: stop at exactly 60 000 stored samples, reason, partial kept, 0 remaining); `capacity_time_default_6k` marked `#[ignore]` (the 60-minute case, release job only; review P9); `capacity_vas(rate)` (drop mode: 25 s of which the VAS pauses 20 s stops only if stored time reaches 10 s; mute mode: stops at 10 s of input); `capacity_count(rate)` (3 recordings → `NoRecordingSlots` on the 4th start while time remains); `erase_renumber(rate)`; `empty_discarded(rate)` (start, stop without input → `DiscardedEmpty`; and the counter-case: start, record 1 s of silence, stop → kept, length ≥ 6 000 + flush, A-025); `expanded_capacity(rate)` (600 min / 999 configured: a recording past 60 min continues); `footprint()` (`footprint_bytes()` ≤ 90 000 000 at the defaults, equal to `capacities().0 × 4 + capacities().1 × 16`, unchanged after record-to-limit at 10 s capacity, erase, erase all and `clone()`). *(Done, two adjustments: the silent counter-case feeds 0.5 s, not 1 s, so the stored zeros sit inside the 1 s hang (A-023) and the length bound is exact; `expanded_capacity(rate)` creates the 600 min / 999 engine, checks its figures and reservation and records 1 s at the seven rates, while the recording past 60 minutes is `expanded_capacity_past_device` at 6 kHz, `#[ignore]` with the default-capacity case; `ci.yml`'s release job now lists `--test us2_memory`; the `__test_hooks::memory_capacities` accessor was added for `footprint()`.)*

### Implementation for User Story 2

- [x] T029 [US2] `Memory::erase(n)`, `erase_all()`, the exact-capacity `append` semantics and `status()` figures in `crates/rr_dr60/src/memory.rs` (research R-03: `drain` of the span's range, start adjustment of the later spans, no allocation). *(Done; `erase(number) -> Option<Span>`, `erase_all()`.)*
- [x] T030 [US2] Transport: `erase`/`erase_all` on `Pipeline` with the playing-item rule and the renumbering of `current`; the `MemoryFull` stop inside `record` and inside the flush; `start_recording`'s `MemoryFull`/`NoRecordingSlots` order; (`reconfigure`'s keep/copy/refuse was completed at T024, by move when the capacity is unchanged; only T027's test remains) in `crates/rr_dr60/src/transport.rs` and `pipeline.rs`; rustdoc that `erase*` are not real-time safe (bounded memmove) and that after a memory-full stop the engine is idle. *(Done; erasing a lower number than the played item also moves the cursor's span start down with the store.)*
- [x] T031 [US2] Make T028 green; add the `vas` configurations with a 10 s capacity to `crates/rr_dr60_harness/src/configs.rs` if the checks need named ones. *(Done; no new named configuration was needed: the checks use `configs::memory_capacity` on `default_agc` / `default_vas`.)*

**Checkpoint**: `us2_memory` green at seven rates (the default-capacity case `--ignored`); goldens unchanged; clippy, fmt, traceability clean. Commit: `feat(core): memory limits, erase and renumbering, status, reconfigure keeps recordings (006 US2)`. *(Reached: commit df157b5 (2026-10-12).)*

---

## Phase 5: User Story 3 — Recordings that survive the session and can be shared (Priority: P3)

**Goal**: `export_len`, `export`, `import` of the version-1 byte form; the round trip; the corruption set; then the seven-rate matrix and the cross-cutting harness extensions.

**Independent Test**: `cargo test -p rr_dr60_harness --test us3_export_import -- --nocapture`.

### Tests for User Story 3 (write first, must fail: `form.rs` does not exist)

- [x] T032 [P] [US3] Form unit tests in a new `crates/rr_dr60/src/form.rs` `#[cfg(test)]` (FR-014; contracts/recording-form.md; review P3): `crc32_check_value` (`crc32(b"123456789") == 0xCBF4_3926`); `header_layout` (magic at 0, version 1 at 8, flags at 10, 6000 at 12, library version at 16/18/20, K at 24, total 36 + 4K); `round_trip_bit_exact`; each refusal in validation order with a minimal input: 7 bytes → `UnknownForm`, wrong magic → `UnknownForm`, version 2 → `UnsupportedFormVersion { 2 }`, flags 1 → `UnsupportedFormVersion { 1 }`, a 20-byte sequence with valid magic and version → `Damaged` (no panic: the length check precedes reading K), length not ≡ 0 mod 4 → `Damaged`, K ≠ (len − 36)/4 → `Damaged`, K = 2⁶² with a short body → `Damaged` without overflow, device rate 8000 → `Damaged`, K = 0 → `Damaged`, CRC flipped → `Damaged`, a NaN / an Inf / a subnormal sample with a recomputed CRC → `Damaged`; `export_into_short_buffer` → `BufferTooSmall { needed }` with the buffer untouched. *(Done; also pinned: a sequence truncated inside the version field (8–11 bytes) is `Damaged`, not `UnknownForm`; the library version fields never refuse; K = 0 exports (import refuses it).)*
- [x] T033 [P] [US3] Transport import/export tests in `crates/rr_dr60/src/transport.rs` tests: `import_counts_against_capacity` (`DoesNotFit` for time and for slots, memory unchanged); `import_while_playing_appends_and_does_not_disturb_playback`; `import_while_recording_refused`; `export_of_a_bad_number`. *(Done; export is allowed while recording, since it reads a closed recording.)*
- [x] T034 [P] [US3] `crates/rr_dr60_harness/tests/us3_export_import.rs` with `split_checks::round_trip(rate)` (export → import into a fresh engine at each of the seven rates → same length, bit-identical playback; US1 scenario 5's first two clauses: length and bytes unchanged by the playback rate, equal to the fresh engine's import) and `corruption_set(rate)` (truncation at every 4 096-byte boundary and at the last byte; a single-byte flip at 64 positions spread over header, payload and CRC; foreign bytes with a valid magic; version 2; flags 1; the 20-byte header; K = 2⁶²; NaN sample with valid CRC; subnormal sample; K = 0; oversized for the remaining time; oversized for the slots): each refused with the stated reason and `memory_status()` identical before and after; the CRC-bit mutant and the drop-first-sample mutant (`split_checks::mutants`) each fail their check (SC-002, SC-004 teeth). *(Done; the two mutants are `split_checks::mutants::{flip_crc_bit, drop_first_sample}` applied between export and import by `round_trip_with`; the corruption set has 11 truncations, 64 flips, the foreign sequence (seeded, expected reason derived from its version bytes) and the named cases; the report prints the count per rate.)*

### Implementation for User Story 3

- [x] T035 [US3] `crates/rr_dr60/src/form.rs` (research R-04): a `const fn` CRC-32 table (`static CRC_TABLE: [u32; 256]`), `crc32(&[u8]) -> u32`, `export_len(k)`, `export(samples, version, out) -> Result<usize, Error>`, `validate(bytes) -> Result<&[u8] (the sample bytes), Error>` in the contract's order with no read past the slice and no `36 + 4·K` multiplication (compare `K` as `u64` with `(len − 36) / 4`), `samples_from_le_bytes` into the store (finite and non-subnormal check per sample before any write). No allocation; `no_std`. *(Done; `LIB_VERSION` parsed from the `CARGO_PKG_VERSION_*` env at compile time.)*
- [x] T036 [US3] `Pipeline::export_len`, `export`, `import` in `crates/rr_dr60/src/transport.rs` and `Memory::import_samples(&[u8] decoded) `/`import_len_check` in `memory.rs` (append within capacity, a new span, the number returned); the `RecordingInProgress` refusal; rustdoc: the stored form is the engine's, version 1, will become encoded frames (A-004), the host never parses it. *(Done; `Memory::import_samples(k, iter)` decodes straight into the reserved store.)*
- [x] T037 [US3] Make T034 green; `split_checks::mutants` in `crates/rr_dr60_harness/src/split_checks.rs` implemented as harness-side transforms of the exported bytes and the stored samples (never in the core). *(Done; the mutants are harness-side byte transforms, with the harness's own CRC-32 to re-form them.)*
- [x] T038 [US3] `crates/rr_dr60_harness/tests/split_matrix.rs` (FR-022; SC-002 – SC-008 at seven rates, `-- --nocapture` report; the 7 × 7 `cross_rate_tone` grid and `capacity_time_default_6k` behind `#[ignore]` for the release job) and `crates/rr_dr60_harness/tests/split_edge_cases.rs` (every spec Edge Case not already covered: memory fills inside the flush; stop while paused; route change mid-playback; reconfigure refusals and the capacity-too-small construction; reset keeps the memory; bad numbers; empty import; host-rate change between record and playback; the one-pass refused while active). *(Done. `split_matrix` runs the sixteen per-rate checks plus the 48 → 44.1 kHz case and the footprint in one report (the default-capacity case stays in `us2_memory` `--ignored`, not duplicated); `cross_rate_grid` is the 7 × 7 release case. `split_edge_cases` has twelve tests.)*
- [x] T039 [P] [US3] (FR-017, FR-018, SC-006) Extend `crates/rr_dr60_harness/tests/determinism.rs` (a record-then-play case over 100 partitions at 48 kHz and the seven rates, events included), `tests/alloc_free.rs` (no allocation in `start_recording`, `record`, `stop_recording`, `start_playback`, `play`, `stop_playback`, `set_route`, `state`, `memory_status`, `recording_length`, `export_len`, the latency and drain reads; `erase` allocates nothing either), `tests/mutation.rs` (the two split mutants), `tests/timing.rs` (SC-006: record + play versus the one-pass on the same 60 s stream, five interleaved repeats, min of each, ratio ≤ 1.25, release `--ignored`, the ratio printed). *(Done; `alloc_free` runs the transport inside the single measuring test (300 s capacity: 85 s of input at 6 kHz recorded twice and imported once); `mutation.rs`'s `split` module tests the same two mutants against the golden SHA, `us3_export_import` against the round trip. Release figures 2026-10-12 for T056: SC-006 ratio **1.024** (one-pass 62.4, record + play 63.9 ns/sample); the one-pass 60.4 / 59.2 / 59.3 ns/sample at 4096 blocks.)*

**Checkpoint**: `us3_export_import`, `split_matrix`, `split_edge_cases`, `determinism`, `alloc_free`, `mutation` green; the release-mode `--ignored` matrix and timing run once and the figures noted for T056; goldens unchanged; clippy, fmt, traceability clean. Coach review of the branch before Phase 6. Commits: `feat(core): versioned export and import of recordings with CRC-32 (006 US3)`, `feat(harness): split matrix, edge cases, determinism, allocation, mutation and timing for the two paths (006 FR-022)`. *(Reached: commit 2efd781 and 276c0b9 (2026-10-12); the coach review's fixes in ee17405.)*

---

## Phase 6: User Story 4 — The same from C (Priority: P4)

**Goal**: every capability through the C API with distinct status codes; the smoke test's loop; bit-identical to Rust.

**Independent Test**: `cargo test -p rr_dr60_ffi`, `cargo test -p rr_dr60_harness --test ffi_parity`, `crates/rr_dr60_ffi/tests/c/run_smoke.sh`.

### Tests for User Story 4 (write first, must fail: the functions do not exist)

- [x] T040 [P] [US4] FFI unit tests in `crates/rr_dr60_ffi/src/lib.rs` `#[cfg(test)]` (contracts/c-api.md): statuses 6–19 by value and name; `RrDr60Route`/`RrDr60State`/`RrDr60StopReason` values; the five new structs' sizes and that every field is fixed-width; `error_to_status_and_field` maps each new `Error` variant; an invalid route value → `INVALID_ARGUMENT`; the info structs are written on every early return (NULL output, poisoned handle, `frames == 0`) as rule 2's amendment says. *(Coach review of US1, finding 1: `rr_dr60_process` already reports `RECORDING_IN_PROGRESS` / `PLAYBACK_IN_PROGRESS` while a transport is active, pinned by `process_is_refused_while_a_transport_is_active`; keep it in the status list.)* *(Coach review of US2–US3: statuses 6–19 and `error_to_status_and_field` exist since T017; keep the by-value-and-name test and `process_is_refused_while_a_transport_is_active`; pin `rr_dr60_export(p, n, NULL, 0, out_written)` → `NULL_POINTER` (size queries go through `rr_dr60_export_len`) and `rr_dr60_import` while playing → OK with the number appended.)* *(Done; the poisoned-handle early returns are exercised by setting the handle's flag directly inside the crate's own tests.)*
- [x] T041 [P] [US4] Extend `crates/rr_dr60_harness/tests/ffi_parity.rs`: the record/stop/export/import/play loop at the seven rates through the C functions versus the Rust calls, bit-identical outputs and identical `RecordInfo`/`PlayInfo`/`StopOutcome`/`MemoryStatus`/`EngineState` values; the state table's refusals as C codes; `BUFFER_TOO_SMALL` leaves the buffer untouched and reports the needed length; a `ffi-test-panic` case while recording: the handle is poisoned, `rr_dr60_reset` recovers it with the recording discarded. *(Done; `rr_dr60_export`'s `BUFFER_TOO_SMALL` also tested with a NULL `out_written`; the smoke reference is `harness::smoke_reference` (a 1 kHz square wave at −20 dBFS, exactly representable in C and Rust; FNV-1a 64 of the Rust playback pinned at `0xc3e904643c8824d7`, 49 692 samples), so the C program compares its playback to the Rust path's without a file exchange.)*
- [x] T042 [P] [US4] Extend `crates/rr_dr60_ffi/tests/c/smoke.c` (US4 scenarios 1–3; contracts/c-api.md › Smoke test additions): `struct_size == 88`, the two defaults, an 80-byte `struct_size` → `STRUCT_SIZE`, `memory_capacity_s = 0` → `INVALID_SETTING` field 14; the loop at 48 kHz (start, record a −20 dBFS 1 kHz tone 1 s in 480-sample blocks with events, stop → kept number 1, `rr_dr60_get_state` at each step with `speaker_muted` only while recording, `memory_status` count 1, `export_len` → buffer → `export`, a second handle → `import` → number 1, both played to `finished`, `memcmp` equal and equal to the reference bytes `ffi_parity` writes to a temp file or embeds; `start_playback` while recording → `RECORDING_IN_PROGRESS`; after a memory-full stop in a 1 s-capacity handle, `stop_recording` → `NOT_RECORDING`; `erase_all` → count 0). *(Coach review of US2–US3: add one `BUFFER_TOO_SMALL` call with a 10-byte buffer to exercise `out_written`.)* *(Done; the stimulus is the square wave above, not a `sin()` tone, so both languages feed identical bits; the smoke prints `smoke: OK (rr_dr60 0.6.0, latency 483 samples at 48000 Hz)`.)*

### Implementation for User Story 4

- [x] T043 [US4] The C surface in `crates/rr_dr60_ffi/src/lib.rs`: `RrDr60Route`, `RrDr60State`, `RrDr60StopReason`, `RrDr60EngineState`, `RrDr60RecordInfo`, `RrDr60StopOutcome`, `RrDr60PlayInfo`, `RrDr60MemoryStatus`; the 21 functions (17 rows) of contracts/c-api.md with `catch_unwind`, `// SAFETY:` comments, NULL and poison handling, the always-written info structs, `rr_dr60_export`'s `out_written` on `BUFFER_TOO_SMALL`; `rr_dr60_reconfigure` and `rr_dr60_reset` per the contract; the header comment's real-time-safe list updated in `crates/rr_dr60_ffi/cbindgen.toml`; regenerate `include/rr_dr60.h`. *(Coach review of US2–US3: the header comments say that `rr_dr60_erase` of the played item stops playback, that `RrDr60MemoryStatus` is in device samples (÷ 6000 for seconds, A-026; C has no seconds helpers), and that `rr_dr60_export` with a NULL buffer is `NULL_POINTER`.)* *(Done; the test-only panic hook also fires in `rr_dr60_record` and `rr_dr60_play`; `rr_dr60_import`'s `out_number` may be NULL; the core's test-hook-only items (`settling_len`, its imports, `RecordPath::host_rate_hz`) are feature-gated so the FFI's release build is warning-free.)*
- [x] T044 [US4] Make T040 – T042 green; `crates/rr_dr60_ffi/tests/c/run_smoke.sh` unchanged in form (it builds against the staticlib). *(Done; `run_smoke.sh` unchanged.)*

**Checkpoint**: `cargo test -p rr_dr60_ffi`, `ffi_parity`, `alloc_free`, the smoke test green (`smoke: OK (rr_dr60 0.6.0, latency 483 samples at 48000 Hz)`); the header drift check (`cbindgen` 0.29.4) clean; goldens unchanged. Commit: `feat(ffi): record, playback, memory, route and export/import through the C API; statuses 6-19; settings 88 bytes (006 US4)`. *(Reached: commit bc5ca61 (2026-10-12).)*

---

## Phase 7: Records (FR-021; SC-001; research R-10)

- [x] T045 [P] `docs/hardware/assumptions.md`: add **A-037**, **A-038**, **A-039** as the spec's Assumptions state them (A-037 low–medium confidence, the engine decision and the capture; A-038 medium with the item-count note and the 2²⁴ arithmetic; A-039 medium, the stop-at-limit assumption and the empty-item engine decision); correct **A-004**'s "16 Mbit ÷ 4 kbit/s ≈ 66.7 min" to "2²⁴ bit ÷ 4 000 bit/s ≈ 69.9 min (the manual's 60 min leaves about 14 % for overhead, A-038)"; cite A-026 in A-038. *(Done.)*
- [x] T046 [P] `docs/hardware/signal-chain.md`: row 5 (the recorder reports VAS events in device samples from the recording's start); row 8 "**Modeled by spec 006 (2026-10-11) for capacity only**: 60 min / 99 items counted in stored device time (A-038); the stored form is the engine's 32-bit samples until the codec (A-004)"; rows 11–13 "spec 006 reserves the speaker/earphone **route** (exclusive, S-001); the stages are not modeled"; the chain diagram gains the memory between the VAS and stage 10. *(Done; the diagram gained an EMULATOR line rather than a change to the device diagram, which already shows the flash between 7 and 9.)*
- [x] T047 [P] `CHANGELOG.md` 0.6.0 under `## [Unreleased]`: `### Added` (the two paths and their calls, the memory with the device's figures and the configurable capacity, the state model and the route, the byte form version 1, the C functions, structs and statuses, the harness: `split_checks`, five test files, `narrowing_delta`); `### Changed` (the narrowing paragraph and figures from T009 completed; the C struct 80 → 88 with the two fields and `struct_size` 80 rejected; `reconfigure` refused while active and keeping the memory; version 0.6.0; a "what a user gets" paragraph: record, stop, play, the 86 MB virtual reservation, the drain, "capacity above 60 minutes is not the device's"); `### Documentation` (A-004 corrected; A-037 – A-039). Breaking list for C callers. *(Done; the 0.6.0 figures: SC-006 ratio 1.024, one-pass 60.4 / 59.2 / 59.3 ns/sample at 4096 blocks against 59.6 / 58.2 / 58.0 before, path latencies 242 + 241 = 483 at 48 kHz.)*
- [x] T048 [P] `README.md`: a "Recording and playback" section (the state model and the calls in order with a short Rust example; the memory's figures and that the store is reserved at construction, about 86 MB at the defaults, virtual until written, commit charge on Windows; the stored form's status and version and that the host never parses it; which calls are real-time safe; the route's status; mute mode's silence counts; after a memory-full stop the engine is idle; no CRC timing quoted); the fidelity table gains the flash row ("capacity only; S-001, A-038") and the route note on rows 11–13; the "Real-time safe" bullet lists the new calls. *(Done; the fidelity table gained the flash row and the route note on the speaker/earphone row; the Rust example is illustrative, not a doctest (the crate's own example is).)*
- [x] T049 [P] `CLAUDE.md`: the 006 status line (branch, PR, what the split is, the narrowing and the one re-bless with its figures, the figures table pointers, the measurement lessons found during implementation) and the useful commands (`us1_record_play`, `split_matrix -- --nocapture`, the release-mode `--ignored` runs, `narrowing_delta`); `.github/ISSUE_TEMPLATE/hardware_evidence.yml`: the four captures (limiter carry-over between items; total minutes with one long item versus 99 short ones, with and without VAS pauses; behaviour when the 60 minutes run out mid-recording; the first 0.5 s of several items). *(Done; CLAUDE.md gained the spec 006 commands line; the issue template gained four captures for A-037 – A-039.)*
- [x] T050 [P] Crate docs: `crates/rr_dr60/src/lib.rs` (the two paths, the memory, the state, the route, the stored form, the level and length notes kept), `pipeline.rs` rustdoc example extended with the record/stop/play loop; `crates/rr_dr60_ffi/src/lib.rs` module docs (rules, the real-time list). *(Done; the `pipeline.rs` doctest runs the record/stop/play loop and pins played = drain + 8 × length at 48 kHz.)*

**Checkpoint**: `scripts/check-traceability.sh` OK; every new constant cites an ID or an engineering target; `grep -rn "66.7" docs/` finds nothing. *(Reached: commit 330a46f (2026-10-12).)*

---

## Phase 8: CI and cross-platform golden files (SC-002; FR-018)

- [ ] T051 Open the **draft** PR `006-record-playback-split → main` with the plan's summary, the baseline (T002) against the new timing (T039's ratio), the 196-of-217 figure and the narrowing figures; wait for the 11 jobs (`check` ×2, `coverage`, `golden` ×6, `ios`, `msrv`); the golden matrix proves the re-blessed files and the record-then-play determinism case on six desktop targets and the iOS simulator. Record the job times in this task. *(Pushed 2026-10-12 at 330a46f; draft PR #18 https://github.com/chancehouston/rr-dr60-emu/pull/18; CI job times to be recorded here when the run finishes.)*
- [x] T052 [P] Local gates: `cargo llvm-cov --all-features --workspace --fail-under-lines 80` (expect ≥ 97 %; note the figure against 0.5's 98.99 %); `cargo +1.85 test --workspace --all-features` (MSRV: `try_reserve_exact`, `const fn` table, `div_ceil` are all 1.85); the release-mode `--ignored` runs (`split_matrix`, `timing`, the AGC and VAS matrices) green. *(Done 2026-10-12: `cargo llvm-cov` **98.58 %** of lines, 99.12 % of functions (0.5: 98.99 %; within a point); `cargo +1.85 test --workspace --all-features` green (the harness doctest needs a clean target dir under 1.85, since the shared one holds stable-built rlibs: `CARGO_TARGET_DIR=target/msrv`); the release-mode `--ignored` runs (AGC and VAS matrices and edge cases, the 60-minute and 61-minute recordings, the 7 × 7 grid, SC-006 at 1.024) all green; the suite's wall time 119 s warm for 37 binaries.)*

---

## Phase 9: Polish, the reviews and the merge

- [x] T053 [P] Stale-text sweep: `grep -rn "DeviceChain\|one-pass only\|single pass" crates docs README.md CLAUDE.md` and fix every mention that no longer holds; `grep -rn "80 bytes\|struct_size = 80" crates/rr_dr60_ffi README.md` → 88; the spec 001/003/005 header notes gain one line each where a figure is restated (001: the narrowing and the golden re-bless; 003: the stop flush replaces "append one latency of silence" for hosts of the split; 005: none unless found). *(Done 2026-10-12: the sweep found no stale `DeviceChain` / one-pass-only mention; the only `80` mentions are the intended 0.5-layout rejection tests; spec 001 gained a "Narrowed by spec 006" note and spec 003 an "Extended by spec 006" note; spec 005 needed none.)*
- [x] T054 [P] Literal audit: every numeric literal added by this feature in `crates/rr_dr60/src/{paths,memory,form,transport}.rs` and `crates/rr_dr60_harness/src/split_checks.rs` has its trace comment (research R-12); `scripts/check-traceability.sh` extended to `split_checks.rs`'s tolerances if its pattern does not already cover them. *(`scripts/check-traceability.sh` scans `split_checks.rs` since the US1 coach review; the core and harness literals of US1 are already tagged.)* *(Done 2026-10-12: every named constant in `paths`, `memory`, `form`, `transport` and `split_checks` carries its trace; the remaining inline literals are structural (the byte widths of `f32`/`u16`, the CRC's 8 bits and 256-entry table, the span table's +1 for the recording in progress, loop counters), not modeled values.)*
- [x] T055 [P] Quickstart walk: run every command of `quickstart.md` steps 1–7 and record the observed figures next to each expected line (`**T055 walk (date): …**`), as 005's T051 did; fix the quickstart where the expectation was wrong and the spec where a figure moved (through its Clarifications). *(Done 2026-10-12: every step's observed figures recorded next to the expectation; no expectation was wrong.)*
- [x] T056 Coverage and timing numbers into the CHANGELOG and README (T047/T048 placeholders): the per-sample ratio, the coverage figure, the suite wall time. *(Done 2026-10-12: the README's speed bullet reads about 350× / 60 ns and the 1.024× record-and-play cost; the CHANGELOG 0.6.0 entry carries the ratio, the per-sample figures and the baseline; coverage 98.58 % lines in the PR table and the quickstart walk.)*
- [x] T057 Spec and plan status: `spec.md` Status stays Draft until the merge; research.md gains the **Implementation** notes per decision as 005's did (what was measured, what moved and why); tasks ticked with dates and the commits named at each checkpoint. *(Done 2026-10-12: research R-01, R-03 – R-13 each carry an Implementation note; the checkpoints name their commits; spec Status stays Draft until the merge.)*
- [ ] T058 Ask the speckit-coach for a final review of the branch against spec 006, then ask the hw-emulation-reviewer for a post-implementation pass (the narrowing figures, the drain and length rules at the seven rates, the memory's accounting, the byte form, the C surface), apply their fixes, and **ask the user before marking the PR ready and before merging**. After the merge: `docs: 006 merged` commit on `main` (spec Status, CLAUDE.md status line, tick this task), as 004 and 005 did. *(2026-10-12: final coach review "ready with fixes" (eleven findings: stale figures, the 99-slot case, tone, the PR body; applied in 9af086d and the PR body) and hardware-emulation review "sound with fixes" (B1–B7: the µ-law comparison restated as 12 bits finer, the smoke stimulus moved to ±0.125 and its hash re-pinned at 0x81b812b9699dc614, the narrowing derivation's 2.54 worst case, the iOS residency and 6 kHz drain sentences in the README, the 2²⁴ convention, the unchanged-set comment; two captures added to the issue template). Both reviewers keep every design decision. The user's go-ahead for ready and for merge is still to come.)*

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (Phase 1)**: no dependencies. T003's constants feed T013.
- **Foundational A (Phase 2A)**: depends on Setup. T004 – T007 before T008; T008 → T009 → T010 in one commit. Blocks everything after it (the goldens must be settled before any refactor).
- **Foundational B (Phase 2B)**: depends on T010. T011 – T013 before T014 – T017. Blocks every story.
- **US1 (Phase 3)**: depends on Phase 2B. T018 – T021 before T022 – T025.
- **US2 (Phase 4)**: depends on US1 (the transport exists). T026 – T028 before T029 – T031.
- **US3 (Phase 5)**: depends on US1 (export needs recordings; `round_trip` plays back) and on US2 for `DoesNotFit` (T033) and the mutants' capacity cases; T032 – T034 before T035 – T037; T038 – T039 after T037 (they exercise all three stories).
- **US4 (Phase 6)**: depends on US1 – US3 (the full Rust surface). T040 – T042 before T043 – T044.
- **Records (Phase 7)**: T045 – T046 any time after T010; T047 – T050 after T044 (they describe the final surface).
- **CI (Phase 8)**: T051 after T044 and T047 – T050; T052 in parallel with T051.
- **Polish (Phase 9)**: after everything; T058 last.

### Task-level dependencies

| Task | Depends on |
|---|---|
| T006, T007 | T004 (the hook's name), T005 (the bound's derivation) |
| T008 | T004 – T007 written |
| T009 | T008 |
| T010 | T009 |
| T013 | T003 |
| T014 | T010, T013 |
| T015 | T014 |
| T016 | T011, T012 |
| T017 | T016 |
| T022 | T018, T016 |
| T023 | T019, T020, T022 |
| T024 | T023 |
| T025 | T021, T024 |
| T029 | T026, T022 |
| T030 | T027, T029, T024 |
| T031 | T028, T030 |
| T035 | T032 |
| T036 | T033, T035, T029 |
| T037 | T034, T036 |
| T038, T039 | T037 |
| T043 | T040, T017, T036 |
| T044 | T041, T042, T043 |
| T047, T048 | T044, T009 |
| T051 | T044, T047 – T050 |
| T056 | T039, T052 |
| T058 | T051 – T057 |

### Within each story

Tests first (they fail because the calls or modules do not exist), then the core implementation, then the harness checks go green, then the checkpoint.

## Parallel Examples

### Phase 2A

- T004, T005, T006 in parallel (three files); T007 with T006; then T008 → T009 → T010 strictly in sequence inside one commit.

### Phase 2B

- T011, T012, T013 in parallel; T014 → T015; T016 in parallel with T014; T017 after T016.

### User Story 1

- T018, T019, T020, T021 in parallel (four files); T022 → T023 → T024 → T025.

### User Story 2

- T026, T027, T028 in parallel; T029 → T030 → T031.

### User Story 3

- T032, T033, T034 in parallel; T035 → T036 → T037; then T038 and T039 in parallel.

### User Story 4

- T040, T041, T042 in parallel; T043 → T044.

### Records

- T045 – T050 all in parallel once their inputs exist.

## Implementation Strategy

### MVP first (User Story 1)

1. **Phases 1 – 2A**: version, baseline, the derived constants; the narrowing's tests; the one `feat!:` commit with the measured figures and the re-bless. Stop at the checkpoint: the golden window is closed.
2. **Phase 2B**: the halves refactor proven by the unchanged guards; settings, errors, the C struct growth. Stop at the checkpoint.
3. **Phase 3**: record, stop, play, state, route, drain, latencies, with the memory at its default capacity. Stop at the checkpoint, demo the `us1_record_play` report, and review with the coach. This is the MVP.

### Incremental delivery

1. **US2**: limits, erase, status, reconfigure keeping recordings. Validate by the exact stop sample, reasons and figures.
2. **US3**: the byte form, the round trip and the corruption set; then the matrix and the cross-cutting harness.
3. **US4**: the C surface and the smoke test's loop.
4. **Records, CI, Polish**, the two reviews, then the merge, with the user's approval.

### Risk notes

- **The narrowing commit is the only golden change.** Everything after it must keep the guards; the refactor's correctness is literally that `narrowing_delta` and the three golden tests print and pass the same. If a refactor moves a hash, find the operation that changed order; do not re-bless.
- **Lengths and the drain are exact rules** (⌈K·m/l⌉; ⌈N₁₀·m/l⌉ + 2D). An off-by-one is a defect in the player's pull loop or in the identity-rate `pending`, not a tolerance question.
- **The default capacity in tests**: never record to the default limit in a debug test (10–25 s); the one such case is `--ignored` and release-only. Constructing default pipelines is cheap (virtual reservation).
- **Event conversion at 44.1 and 88.2 kHz**: running sums, never per-event rounding (review P4). If a golden VAS entry's events disagree, check the cumulative floor before anything else.
- **The flush and the VAS**: the flush's zeros are inside the hang time, so they are stored; a test that expects the flush to be dropped is wrong.
- **Honesty**: "the engine's stored form", "modeled for capacity only", "the route is a label until stages 11–13", "virtual reservation", "inaudible but recorded". Never "the device's memory format".

## Notes

- `[P]` tasks touch different files and have no unfinished dependencies.
- Commit after each task or logical group, using Conventional Commits; T008 + T009 + T010 are one `feat!:` commit.
- Every value cites an A-ID, an S-ID or an engineering target (FR-021).
- Task numbering: T045 – T046 may run any time after T010; T047 – T050 after T044; T051 – T052 CI; T053 – T058 polish and the reviews.
