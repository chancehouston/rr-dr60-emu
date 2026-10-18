---

description: "Task list for 013 Playback Speed (the device's five playback speeds)"
---

# Tasks: Playback Speed (the device's five playback speeds, in the playback path)

**Input**: Design documents from `specs/013-playback-speed/`:
- [plan.md](plan.md) (› "Task order")
- [spec.md](spec.md), with the spec-review, clarify and plan-review Clarifications
- [research.md](research.md) (R-01 – R-12)
- [data-model.md](data-model.md)
- [contracts/](contracts/) (rust-api, c-api, golden-format)
- [quickstart.md](quickstart.md)
- the design script `tools/playback-speed/speed_design.py`

**Tests**: REQUIRED. Constitution III requires tests to be written first and to fail before the implementation exists:
- **US1:** against a stub that plays every level as level 3;
- **US2:** against US1's core, which applies a level only at `start_playback`;
- **US3:** the README statements check before the README text;
- **C:** with a placeholder hash.

**Organization** (the plan's six phases): 1 Setup; 2 US1 (P1, MVP); 3 US2 (P2); 4 US3 (P3); 5 the C surface; 6 gates, reviews, the bless and the draft PR.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task).
- **[Story]**: the user story the task belongs to (US1 – US3).
- Paths are relative to the repository root.

## Conventions that apply to every task

- **Traceability.** Cite spec 013's FR, SC and assumption IDs in code comments, rustdoc and test names, for example `// A-054: the speed reading of "+/-40%" (spec 013 FR-002)` or `// 013 FR-005: the tail before the drain`. Every literal tolerance carries its citation (the traceability scan).
- **The figures come from the design script.** Every expected filter figure in a test is a constant `speed_design.py` prints, copied into `speed_checks.rs` or the core tests with the date of the print (spec 005's lesson); tests never pin a figure to the engine's own output, except the bank bit-pattern digest, which is a regression guard pinned from the Rust (research R-03).
- **The eight golden files do not move** (FR-006): `golden`, `golden_agc`, `golden_vas`, `golden_codec`, `golden_output`, `golden_output_clamp`, `golden_volume_ramp`, `golden_mic`, with their SHA guards; they run at the end of every phase. The three transport tests `playback_produces_ceil_k_m_over_l_then_drain`, `play_reports_frames_and_index` and `drain_zero_when_stage10_off_at_6k` stay byte-for-byte unchanged.
- **The oracles** (research R-08): (a) level 3 bit-identical to 0.12; (b) the stimulus itself (imported stored samples, `configs::speed_only`, 6 kHz, the playback output is the device stream); (c) the independent reference `speed_checks::reference_output` (the published bank, the four-sum order, the place rule computed by its own integer arithmetic), bit-exact, with the codec off (`speed_only`) and on (`speed_only_codec`); (d) the least-squares sine fit at exactly r × f.
- **The place** is an integer count of fifths everywhere (core and harness); never derived from `speed::ratio`.
- **Floating point.** `f64` in the per-sample path; basic IEEE operations; the dot product as four partial sums s_q (m ≡ q mod 4, m ascending) combined as (s₀ + s₁) + (s₂ + s₃); no `f64::clamp`; the harness bans `powf`/`sqrt`/`powi`/`f64::log10` (use `analysis::{db, from_db}`, `detmath::sqrt`).
- **Wording** (R-10): "the speed control", "playback speed", "level"; never "accurate", "authentic", "measured" (except "not measured") or "verified" (the denylist stem `verif`); "ramp" and "glide" never; no claim about what slowing reveals in a recording.
- **Story boundaries** (Principle VII): US1 uses imported stimuli and constant levels set before `start_playback`; US2 the reference and the whole-engine harnesses; US3 the cross-check and the audit.
- **Coverage time.** Audio-heavy loops use `configs::measurement_rates()`; FR-004's 6 kHz checks run at 6 kHz only; the release timing tests are `#[ignore]`.
- **Before ticking a task**, these pass (inside a story's test-first window, that story's new tests are the expected red, and `golden_speed` is red by design from T023 until T035): `cargo fmt --all -- --check`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo test --all-features`. The MSRV gate is `RUSTFLAGS="-D warnings" cargo +1.85 test --workspace --all-features --no-run`.
- **Commits.** Conventional Commits on branch `013-playback-speed`, after each task or logical group; `git add` by file. The branch is stacked on `research/noise-clock-power` (PR #31). **Nothing is merged and no PR is marked ready while the owner is away.**

---

## Phase 1: Setup (plan › Task order 1; research R-03, R-06, R-10)

**Purpose**: version 0.13.0, the registers, the manifest strings and the one re-bless, the public constants and the harness helpers. Nothing audible changes.

- [X] T001 Bump the workspace version to 0.13.0: `version = "0.13.0"` in `[workspace.package]` and the three internal dependency entries of `Cargo.toml`; `RR_DR60_VERSION_MINOR = 13` in `crates/rr_dr60_ffi/src/lib.rs` (and its unit test that asserts 12); regenerate `crates/rr_dr60_ffi/include/rr_dr60.h` (`cbindgen --config crates/rr_dr60_ffi/cbindgen.toml --crate rr_dr60_ffi --output crates/rr_dr60_ffi/include/rr_dr60.h`); fix every version string the smoke loop and `ffi_parity` pin.
- [X] T002 [P] Register **A-054** in `docs/hardware/assumptions.md` after A-058 (seven cells, no `|` inside a cell): the spec's Assumptions text (the five levels, S-001's factory level 3, the firmware command S-002, both readings of S-012's "+/-40%", linear spacing assumed, mechanisms (a) / (b) / (c) and what S-003 excludes, the weak lean toward (c) from S-018, (b) as the owner's labelled default 2026-10-17, the clean resampler as the emulator's choice); confidence "Low (ratios, mechanism, resampler quality) / High (the control exists and is firmware)"; verification capture P1 with research R-11's figures (a stored 2300 Hz tone at level 4 plays at 2760 Hz about 53.6 dB down under (b) as modeled, absent at level 5, about 0 dB under (a); f + f_image = 6 kHz at a fixed clock; the vowel's pitch; the durations) and the level-change capture (a sustained vowel, the dial turned during playback: an instant step, a frame-aligned change or a mute); status "assumed (spec 013)". Register **S-018** in `docs/hardware/sources.md` after S-017 (five cells): OKI EP0714089B1 and Lucent US5717823A (links from the research report's speed section), Type "Patent", Reliability "Secondary (for the class; nothing names the device)", key facts as research R-10.
- [X] T003 [P] Add the CHANGELOG 0.13.0 skeleton regions to `CHANGELOG.md` (`<!-- 0.13.0:start -->` … `<!-- 0.13.0:end -->` under Added, Changed and Documentation), with one placeholder line each naming spec 013, so the manifest's model version 0.13.0 appears in the CHANGELOG (007 FR-013 (g)).
- [X] T004 Update stage 9 and stage 10 in `crates/rr_dr60/src/manifest.rs` with research R-10's exact strings: stage 9 description "Rebuilds the voice-band stream from the stored CELP frames for playback, at one of five playback speeds (level 3 is normal)." (124 characters); sources `["S-001", "S-002", "S-003", "S-008", "S-009", "S-010", "S-011", "S-012", "S-018"]`; assumptions `["A-004", "A-040", "A-041", "A-054"]`; controls "speech_codec_enabled; set_playback_speed; Tap::AfterDecoder; PlayInfo.frames, PlayInfo.frame_index, PlayInfo.place" (114); coverage "the device's own decoder (frame length, look-ahead, codebooks, postfilter); its speed method (A-054); pitch-kept speeds" (119); model version "0.13.0"; stage 10 coverage "µ-law expansion, idle noise on the playback side, DAC imaging residue above 3 kHz". **First** run the denylist and the length checks on the changed strings (`cargo test -p rr_dr60_harness --test us2_cross_check wording`) and fix any hit by rewording (the cross-check checks the strings' form, not that the API items exist yet). Restate `crates/rr_dr60_harness/tests/us1_manifest.rs::codec_stages_us3_scenario_1` with a dated comment: stage 9's model version 0.13.0 (stage 7 stays 0.8.0), stage 9 cites A-054, its controls contain `set_playback_speed` and `PlayInfo.place` besides the pinned items (/speckit-analyze F2).
- [X] T005 Re-bless once: `RR_DR60_BLESS=manifest cargo test -p rr_dr60_harness --test us2_cross_check canonical` and `RR_DR60_BLESS=readme cargo test -p rr_dr60_harness --test us2_cross_check readme -- --test-threads=1`; run `us1_manifest`, `us2_cross_check` and `scripts/check-traceability.sh`. The commit message says the records run ahead of the code until Phase 2.
- [X] T006 [P] Add the public module `crates/rr_dr60/src/speed.rs` (contracts/rust-api.md): `LEVEL_COUNT: u8 = 5`, `DEFAULT_LEVEL: u8 = 3` (S-001), `RATIO_DENOMINATOR: u32 = 5`, `RATIO_NUMERATORS: [u32; 5] = [3, 4, 5, 6, 7]` (A-054), `LOOKAHEAD_SAMPLES: u32 = 28`, `MAX_TAIL_SAMPLES: u32 = 46` (research R-03, R-04), `pub fn ratio(level: u8) -> Option<f64>` (display only), rustdoc on each with its ID; `pub mod speed;` in `crates/rr_dr60/src/lib.rs`; `Error::InvalidSpeedLevel { level: u8 }` in `crates/rr_dr60/src/error.rs` with `Display` "invalid playback speed level {level}: expected 1 to 5" and the error tests extended. Unit tests: `ratio(0) == None`, `ratio(6) == None`, `ratio(3) == Some(1.0)`.
- [X] T007 [P] Harness helpers: `analysis::sine_fit(y, f, fs) -> (amplitude, residual)` (least squares on cos/sin at exactly f) and `analysis::spurious_peak_db(residual, fs, lo, hi)` (Kaiser β 20 window, normalized to a unit sine's peak; the window computed with the harness's allowed math) in `crates/rr_dr60_harness/src/analysis.rs`, each with a unit test (a known two-tone signal); a stored-samples import helper in `crates/rr_dr60_harness/src/stimulus.rs` (reuse an existing samples-form builder if the harness has one, else build the version-2 samples form of specs/006's contracts/recording-form.md with its CRC) returning the recording number; `stored_tone(f, n)`, `stored_impulse(j, n)`, `stored_end_loud(n)` and `stored_voiced(n)` (data-model.md › Stimuli: amplitude 0.5, −6 dBFS); `configs::{speed_only, speed_only_codec, speed_default, speed_default_no_codec}` in `crates/rr_dr60_harness/src/configs.rs` (data-model.md › Configurations).
- [X] T008 Phase checkpoint: the eight golden tests and SHA guards, the C smoke loop (`crates/rr_dr60_ffi/tests/c/run_smoke.sh`), `us1_manifest`, `us2_cross_check`, the audit and `uv run tools/playback-speed/speed_design.py --check` pass. Commit `chore(013): version 0.13.0, A-054, S-018, the stage-9 manifest entry and the speed constants`.

**Checkpoint**: the records and the constants exist; nothing audible has changed.

**Checkpoint notes (Phase 1, as built):**
- T007: the stored-recording helper is `configs::stored_recording(target, x)`: it records `x` at 6 kHz in `bypass_all` (every record stage off, so the stored samples equal the input, followed by the stop's flush of silence), exports and imports into the target engine (spec 006 FR-014). The harness had no stand-alone samples-form builder; this reuses the engine's own form, so no CRC code is duplicated.
- T007: `analysis::spurious_peak_db` zero-pads the FFT four times: without it a component between two bins read up to 0.51 dB low (the unit test's 2203 Hz tone at 6 kHz, 9000 samples), the unsafe direction for a −60 dB floor. `speed_design.py`'s `peak_db` was changed the same way; its worst spurious figures moved to −73.0 / −73.0 / −72.5 / −73.9 dB (research R-03 updated).
- T004: `us1_manifest::codec_stages_us3_scenario_1` restated as /speckit-analyze F2 says (stage 9 at 0.13.0, A-054, `set_playback_speed`, `PlayInfo.place`); the manifest's coverage and controls strings are research R-10's (119 and 114 characters).

---

## Phase 2: User Story 1: slow and fast playback at a chosen level (Priority: P1) 🎯 MVP

**Goal**: levels 1, 2, 4, 5 set before a playback play at r = k/5 with pitch moving, the tail before the drain; level 3 bit-identical to 0.12.

**Independent Test**: imported stimuli, constant levels, `configs::speed_only` at 6 kHz and the default path at the measurement rates; oracles (a), (b), (d) and the reference for the tail.

### Tests for User Story 1 (written first, run red)

- [X] T009 [US1] The stub surface (so the checks compile): `Pipeline { speed_level: u8 }` (default 3; kept by `reset` and `reconfigure`), `set_playback_speed` (refuses 0 and > 5 with `Error::InvalidSpeedLevel`, level unchanged) and `playback_speed` in `crates/rr_dr60/src/pipeline.rs`; `playback_length_samples` returning level 3's ⌈K · m ÷ l⌉ + drain for every level (stub) in `crates/rr_dr60/src/transport.rs`; `PlayInfo::place` (min(⌊last pulled index⌋, K) at level 3) in `transport.rs`; the design half of `crates/rr_dr60/src/stages/speed.rs` already real (`design_bank(k)` by `resample::design::kaiser_lowpass(281, fc ÷ 30000, kaiser_beta(70))` and `polyphase_split(h, 5)` with research R-03's edges, and `tail_len(K, k)` by data-model.md's rule) exposed through `__test_hooks::{speed_bank, speed_tail_len}` in `crates/rr_dr60/src/lib.rs`. The stub plays every level as level 3.
- [X] T010 [US1] Create `crates/rr_dr60_harness/src/speed_checks.rs` (research R-08): `reference_output(x, k_schedule)` (the independent reference: places in fifths by its own integer arithmetic with the latch-first and deferred-switch rules, the bank from `speed_bank`, the four-sum order, the tail by the kernel rule, then silence); checks for the lengths (`produced` minus `drain_samples()` equals ⌈(⌈5K ÷ k⌉ + T) · m ÷ l⌉ and `playback_length_samples` minus the drain; `finished` once), FR-004 (a) (fit at r × f, ±0.1 dB, residual ≤ −60 dB; tones every 50 Hz from 103 Hz to min(2250, 2250 ÷ r)), FR-004 (b), (c) (spurious ≤ −60 dB, 20 – 3000 Hz, tones every 100 Hz from 103 to 2903 Hz plus the script's worst-case tones (the script's own grid is 25 Hz), pinned to the script's per-level worst figures within 0.5 dB at the script's tones 2853 / 2853 / 2503 / 2203 Hz), the other-rate level comparison (±0.1 dB against level 3's playback of a stored tone at r × f at `measurement_rates()`), US1 AS5 (500 Hz and 1 kHz on the default path: frequency by the fit and level(r) − level(3) = H(r × f) − H(f) ± 0.2 dB, H from a codec-bypassed level-3 run), the impulse (stored j = 600 at levels 1, 2, 4 and j = 602 at level 5: the peak at the nearest output 1000 / 750 / 500 / 430, symmetric within 1e-12; the half-sample ties at levels 2 and 4 equal within 1e-12), the tail (a recording ending on a full-scale sample: the outputs after the own part equal the reference's tail bit for bit, then the drain or nothing), and level 3 (every play report field against 0.12's rules, research R-05). Constants from `speed_design.py` with the date of the print. Add `speed_checks.rs` to `scripts/check-traceability.sh`'s scan.
- [X] T011 [P] [US1] Create `crates/rr_dr60_harness/tests/us1_speed.rs` (AS1 – AS6 at `measurement_rates()`, FR-004 at 6 kHz) on `speed_checks`; run it: **red** on the lengths, the fits at r × f, the impulse and the reference at levels ≠ 3; green by construction at level 3 (record the red/green list in the commit message, not misread as all red).

### Implementation for User Story 1

- [X] T012 [US1] Implement `SpeedStage` in `crates/rr_dr60/src/stages/speed.rs` (data-model.md › SpeedStage and the state machine): banks for k = 3, 4, 6, 7 built at construction (levels 1 and 2 share a prototype: one bank, two references, or two copies, either is fine), the 128-entry `f32` ring, `pulled`, `end`, `p5`, `active`, `requested`, `latched` (set **first** when p5 ≥ 5K), `last_p5`, `own`, `tail`; `next(&mut self, src: &mut impl FnMut() -> Option<f32>) -> SpeedOut` with `Own(f64)` / `Tail(f64)` / `Silence`; level 3 pulls x[i] only (no look-ahead) and writes it to the ring; the four-sum dot product; `reset(level)`. In this phase the requested level is applied only by `reset` (live change is US2). Unit tests (`cargo test -p rr_dr60 stages::speed`): the banks' cutoff, energy and moment within 1e-12 of the script's (research R-03); phase sums within 1e-15 of 1; a digest of each bank's `f64::to_bits` (pinned from this Rust, a regression guard); ⌈5K ÷ k⌉ for K = 1, 120, 6000 (the script's table); `tail_len` worst cases 46 / 34 / 23 / 20 over K mod k; the at-most-4 switch count; no pull beyond x[i] at level 3; the latch at 5K; spurious at 2853 / 2503 / 2203 Hz within 0.5 dB of the script and the passband within 0.001 dB.
- [X] T013 [US1] Wire it into the player in `crates/rr_dr60/src/transport.rs` and `crates/rr_dr60/src/pipeline.rs` (research R-01, R-05): `Pipeline { speed: SpeedStage }` built in `new` and `reconfigure`; the source closure over the memory, the decoder and the cursor returning `Option<f32>`; `PlaybackPath::push_device_f64` in `crates/rr_dr60/src/paths.rs`; `Cursor::total` `u64::MAX` at `start_playback`, set to ⌈(K′ + T) · m ÷ l⌉ + drain at the first `Silence`, and **nothing pushed or emitted when `produced` already equals it**; `PlayInfo::place` = min(⌊last_p5 ÷ 5⌋, K) and `frame_index` = min(⌊place ÷ 120⌋, F − 1) (codec on; 0 bypassed); `start_playback` resets the stage with the host's level; `playback_length_samples` real (⌈(⌈5K ÷ k⌉ + T(K, k)) · m ÷ l⌉ + drain; `InvalidSpeedLevel` before `NoSuchRecording`). Rustdoc on every changed public item.
- [X] T014 [US1] Run the guards: the three named transport tests unchanged, the eight golden tests and SHA guards, the record-then-play ≡ one-pass tests (`cargo test -p rr_dr60_harness --test split_matrix --test us1_record_play --test us1_codec`), then `us1_speed` green at the seven rates (`-- --nocapture`, quote the 6 kHz FR-004 rows).
- [X] T015 [P] [US1] Create `crates/rr_dr60_harness/tests/speed_matrix.rs`: the seven-rate report (data-model.md › Report rows): own length against expected, fit and residual, worst spurious, impulse peak, tail, per level and rate; asserted where FR-004 asserts, reported elsewhere.
- [X] T016 [P] [US1] Create `crates/rr_dr60_harness/tests/speed_edge_cases.rs` with the constant-level rows: K = 1 at every level; a recording shorter than the look-ahead; the codec bypassed (`frame_index` 0, `place` progresses); DC and silence through every level (DC within 1e-15 of its stored value after the kernel fills; silence exact zeros); a refused level (0, 6, 255) unchanged; a level set while idle and while recording applies at the next playback; `playback_length_samples` refusals.

**Checkpoint**: SC-001, SC-002, SC-004. Commit `feat(013): the speed control at a constant level, the tail before the drain (US1)`.

**Checkpoint notes (Phase 2, as built):**
- T011 ran red against the stub as planned: all 7 US1 tests failed (level 3 too, through the stub's place of 0).
- **The total is fixed at the latch, not at the first silent sample** (deviation from research R-05 as planned). The recording's length K is known at `start_playback` (`SpeedStage::reset(level, K)`), so when the place first reaches K the stage computes K′ + T (the tail still to play counted from the place and the level) and the player fixes the total right after that output. Planned that way (total at the first `Silence`), a 6 kHz `speed_only` playback whose length is a whole number of calls would report `finished` one call later than 0.12, breaking FR-006's report identity; fixing it at the latch reproduces 0.12's timing exactly at level 3 and makes the drain-0 stop (plan review Q1) unnecessary: nothing past the total is ever requested. The `no_drain_zero_stop` mutant therefore has no mechanism to break; T021 replaces it.
- **The place is K once latched** (FR-009 "reach the recording's end"): the place of the last output computed, and K as soon as the place has reached K, so a level-3 playback's last report is K, not K − 1; `frame_index` = min(⌊place ÷ 120⌋, F − 1) is unchanged by this (F − 1 either way).
- T012: the spurious figures are pinned in the harness (`band_6k`, within 0.034 dB of the script at the worst tones), not in the core tests (the `no_std` core has no FFT); the core pins the banks' cutoff, energy and moment within 1e-12, the phase sums within 1e-15 and the bit-pattern digests.
- T014: the full suite green (62 binaries): the eight golden files and SHA guards, the three named transport tests unchanged, the record-then-play ≡ one-pass checks.

---

## Phase 3: User Story 2: changing the speed while a recording plays (Priority: P2)

**Goal**: live level changes continue from the place reached, bit-exact against the reference; the deferred switch into level 3; the latch; real-time safety and cost.

**Independent Test**: the reference (oracle c) in `speed_only` and `speed_only_codec`; the whole-engine harnesses.

### Tests for User Story 2 (written first, run red)

- [X] T017 [US2] Create `crates/rr_dr60_harness/tests/us2_speed.rs`: for every ordered pair (a, b) of distinct levels with b ≠ 3, at two change points (calls of rate ÷ 50 host samples at 6 kHz), and for the schedules into level 3 (from each of levels 1, 2, 4, 5 at each phase ph = 1 … 4), a change into another level during the deferred wait, a change in the last 4 outputs before K, and a change after the place reached K: the whole device output equals `reference_output` bit for bit in `speed_only` and `speed_only_codec`, the reported places match the reference's, `finished` once; rapid changes (a new level before every call through 3 → 1 → 5 → 2 → 4 → 3 …). Run it **red** against T012/T013's core (levels applied only at `start_playback`); record the red rows.

### Implementation for User Story 2

- [X] T018 [US2] The live change in `crates/rr_dr60/src/stages/speed.rs` and `transport.rs`: `play()` copies `speed_level` into the stage's `requested` at the start of each call; before each output: latch first (p5 ≥ 5K), then a non-3 request applies at once, a request for 3 waits for p5 ≡ 0 (mod 5), a different request during the wait applies at once and drops the pending switch; leaving level 3 pulls through x[i + 28]. `us2_speed` green; extend the core unit tests with the switch rules.
- [X] T019 [US2] Whole-engine rows in `us2_speed.rs`: the level kept by `reset` and `reconfigure`; `stop_playback` then `start_playback` starts from rest at the recording's start; erasing the recording being played stops the playback, the level kept; route, volume (with its transition) and the output clamp independent of the level; at every level the one-pass output, the record path, the stored bytes (`__test_hooks::stored_frames`) and the export bytes bit-identical to level 3's; `playback_latency_samples` and `drain_samples` the same at every level.
- [X] T020 [P] [US2] `crates/rr_dr60_harness/tests/determinism.rs` gains `speed_partitions` (a constant level non-3: partitions of 1, 64, 4096 and an irregular one bit-identical at 6 and 48 kHz) and `crates/rr_dr60_harness/tests/alloc_free.rs` gains playbacks at every level and with a change before every block, codec on and off.
- [X] T021 [P] [US2] Mutation: `__test_hooks::SpeedMutation { ratio_k7: Option<u32>, no_tail: bool, snap_into_level_3: bool, lookahead_delta: i32, latch_after_switch: bool, lookahead_at_level_3: bool }` and `Pipeline::__with_speed_mutation` in `crates/rr_dr60/src/lib.rs` / `pipeline.rs` / `stages/speed.rs`; `crates/rr_dr60_harness/tests/mutation.rs` gains `speed::` cases, each caught by a named check (ratio 7 → 6.9 by the lengths; no tail by the tail check; snap by the into-3 reference rows; look-ahead 27 by the impulse peak and the reference; latch after switch by the last-4-outputs row; no drain-0 stop by `drain_zero_when_stage10_off_at_6k`-style rows at 6 kHz in `speed_only`).
- [X] T022 [US2] Timing (research R-07): `timing::speed_cost` and `timing::speed_codec_worst_block` in `crates/rr_dr60_harness/tests/timing.rs` (`#[ignore]`, release, 45 strictly alternating runs, ceilings `RR_DR60_SPEED_COST_CEILING` default 15 on the development machine and `RR_DR60_SPEED_CODEC_RATIO_CEILING` default 1.5, the 6 kHz figure printed); `crates/rr_dr60_harness/tests/playback_timing.rs` (`#[ignore]`, only 0.12's public API: record 60 s of −20 dB noise with a codec-bypassed engine, play at the default level, print `playback, Speaker route, 48 kHz, 4096 blocks: <ns> ns per host sample`); `scripts/ab-timing.sh` gains the `playback` mode (copy `tests/playback_timing.rs` into the baseline worktree, `rounds=45` for this mode, parse the line, print the A/B against the 3 % budget); `.github/workflows/ab-timing.yml` gains the step `scripts/ab-timing.sh 3a4eefb playback` with the 5 % runner ceiling. Run solo (`--test-threads=1`) and the A/B; record the figures in the commit message.
- [X] T023 [US2] The golden generator (contracts/golden-format.md): `GoldenEntry.{speed_level, speed_changes, place, own_length}` (optional, skipped when absent, so the eight files read unchanged), `SPEED_*` names, `generate_speed(make, bless)`, `committed_speed`, `path_speed` in `crates/rr_dr60_harness/src/golden.rs` (calls of rate ÷ 50; each recording once per (stimulus, rate, codec); the bless-time assertions); `crates/rr_dr60_harness/tests/golden_speed.rs` with a placeholder SHA and the 194-entry count. **Red by design** until T035; the gate runs without it (`cargo test --all-features -- --skip golden_speed`).

**Checkpoint**: SC-003, SC-005. Commit `feat(013): live speed changes, mutants and the cost (US2)`.

**Checkpoint notes (Phase 3, as built):**
- T017 ran red as planned: 6 of 8 `us2_speed` tests failed against Phase 2's core (levels applied only at `start_playback`); `scope_and_latency` and `whole_engine_controls` were green by construction. T018 (`speed.request` at every play call) turned all 8 green: every schedule, codec off and on, bit-exact against the independent reference, places included. Call sizes of 121 – 124 put changes at every phase; one-sample calls put a change inside the deferred switch and in the last outputs before K.
- T021: the sixth mutant is `lookahead_at_level_3` (level 3 reading ahead), caught by a new row of `level3_identity` (each call's decoded frames against 0.12's ⌊place ÷ 120⌋ + 1); the planned `no_drain_zero_stop` had nothing left to break once the total is fixed at the latch (Phase 2's notes).
- T022: the first `speed_cost` run read +17.4 % at level 1 (+4.55 ns per host sample, about 36 ns per 57-tap product): the per-tap bounds tests in `x()`. A branch-free fast path for windows inside the stored samples (the same four partial sums, the same order: bit-identical, US1/US2 re-run green) brought it to **+8.3 … +8.6 % (+2.2 ns)** at 48 kHz; at 6 kHz, where every host sample is a device output, +130 % (+14.2 ns, reported). Codec on: level 5 at **1.195×** level 3 (28.36 → 33.89 ns); worst block with the level changed before every block 19.6 µs of 1333 (64 samples, 48 kHz), 153 µs of 85 333 (4096), 11.0 µs of 10 667 (64, 6 kHz), 291 µs of 682 667 (4096, 6 kHz). **The level-3 A/B** against `3a4eefb` (0.12's merge) first read **+3.63 %** (+0.91 ns per host sample; budget 3 %): the general `next()` path and a total check on every host sample. A steady-identity fast path in `next()` (one pull, one store; the same result as the general path, the eight goldens and US1/US2 re-run green) and the total check moved next to the device-sample pull brought it to **+1.52 % (25.07 → 25.45 ns)**. With level 3 faster, the other levels read **+9.8 … +10.3 % (+2.5 – 2.6 ns)** at 48 kHz and the codec-on level 5 **1.127×** (27.45 → 30.94 ns); the worst blocks 20.2 µs of 1333 (64, 48 kHz) and 242 µs of 85 333 (4096, 48 kHz). The script's number format was fixed (`.2f`).
- T023: the bless-time assertions were exercised once by a dry bless whose file was deleted (the lengths against the helper, the final place, schedule B's after-the-end change without effect, the speed-only entries against the reference): all hold; `golden_speed` stays red by design until T035.

---

## Phase 4: User Story 3: the control in the API and the records (Priority: P3)

**Goal**: the README, the rustdoc, the register and the signal-chain table carry the speed control.

**Independent Test**: the cross-check, the audit and the README statements check.

- [X] T024 [US3] Add `us2_cross_check::readme_speed_statements` in `crates/rr_dr60_harness/tests/us2_cross_check.rs` (the five levels and 0.6 … 1.4, "pitch", "modeled", the weak lean toward a pitch-keeping device, "playback only", the length helper, level 3 unchanged, the C names); run it **red**.
- [X] T025 [US3] Write the README's "Playback speed" section in `README.md` (research R-10) → T024 green; the rustdoc of `set_playback_speed`, `playback_speed`, `playback_length_samples`, `PlayInfo::place`, `PlayInfo::frame_index` and the `speed` module (the identity at level 3 stated, contracts/rust-api.md); the crate docs' one sentence in `crates/rr_dr60/src/lib.rs`.
- [X] T026 [P] [US3] Records: the signal-chain row 9 "**Modeled by** spec 013 …" sentence (research R-10) in `docs/hardware/signal-chain.md`; A-054's verification text complete; the CHANGELOG 0.13.0 regions filled (Added: the control, the length helper, `PlayInfo::place`, the constants; Changed: the C play report 32 → 40 bytes, an ABI break, rebuild against the new header; `frame_index` from the place; Documentation: A-054, S-018, the stage-9 manifest entry and the one re-bless).
- [X] T027 [US3] Run `us1_manifest`, `us2_cross_check` and `scripts/check-traceability.sh`; no second canonical re-bless unless a review changes a string.

**Checkpoint**: SC-007. Commit `docs(013): the speed control's records (US3)`.

---

## Phase 5: The C surface (FR-012; contracts/c-api.md)

- [X] T028 In `crates/rr_dr60_ffi/src/lib.rs`: `RrDr60Status::InvalidSpeedLevel = 25` (and the status mapping from `Error::InvalidSpeedLevel`); `rr_dr60_set_playback_speed`, `rr_dr60_get_playback_speed`, `rr_dr60_playback_length_samples` (the `rr_dr60_set_volume` / `rr_dr60_get_volume` / `rr_dr60_recording_length` patterns: `live`, `guarded`, NULL checks, `// SAFETY:` comments); the literal defines `RR_DR60_SPEED_LEVEL_COUNT`, `RR_DR60_SPEED_DEFAULT_LEVEL`, `RR_DR60_SPEED_RATIO_DENOMINATOR`, `RR_DR60_SPEED_RATIO_NUMERATOR_1 … _5`, pinned to `rr_dr60::speed` by a unit test; `RrDr60PlayInfo.place` at the end and `c_play_info` filling it; a size assertion of 40 bytes on 64-bit targets; regenerate the header; the header drift check.
- [X] T029 `crates/rr_dr60_ffi/tests/c/smoke.c` gains the speed case (contracts/c-api.md: a ±0.125 square recorded with the codec on at 48 kHz, played at level 5 then level 1, hashed; levels 0 and 6 refused; the length helper read) with `RR_DR60_SMOKE_SPEED_HASH` pinned in `crates/rr_dr60_harness/src/smoke_reference.rs` from the Rust run (placeholder first: run red, then pin); `crates/rr_dr60_harness/tests/ffi_parity.rs` gains set/get/refuse, the length helper and a playback with level changes bit-identical to the Rust API's.

**Checkpoint**: SC-006 (the C half). Commit `feat(ffi): the playback speed functions, status and play report field (013)`.

**Checkpoint notes (Phase 5, as built):** the smoke case reuses the block's existing recording of the full-scale square in 0.11's default (the codec on) rather than a new ±0.125 recording (contracts/c-api.md): it is exactly representable too, and the case adds no recording; hash `0x0dd90285502edcc3`, 79 776 samples (level 5, then level 1 from play block 10, volume 0.1, speaker route). `ffi_parity::speed_parity` plays a four-change schedule through `rr_dr60_play` and the Rust API: the output and every report (the place included) bit-identical; status 25 for levels 0, 6 and 9.

---

## Phase 6: Gates, reviews, the bless and the draft PR (plan › Task order 6)

- [X] T030 The gate **without** `golden_speed`: `cargo fmt --all -- --check`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo test --all-features -- --skip golden_speed`; the MSRV test build with `-D warnings`; the header drift check; `uv run tools/playback-speed/speed_design.py --check`; the C loop.
- [X] T031 The solo timing runs (`cargo test -p rr_dr60_harness --release --test timing speed_cost speed_codec_worst_block -- --ignored --nocapture --test-threads=1`) and `scripts/ab-timing.sh 3a4eefb playback`; quote the figures (nanoseconds and ratios, 6 and 48 kHz) for the PR and CLAUDE.md.
- [X] T032 The post-implementation hardware review (`hw-emulation-reviewer`, on the session's model): the code against research R-02 – R-05, the figures, the oracles; apply the findings at the recommended answers (the owner accepted defaults), recording them in the spec's Clarifications and tasks.md checkpoint notes.
- [X] T033 The coach review (`speckit-coach`, on the session's model) of the as-built branch against the spec, including the SC-008 reader test on the README; apply the findings likewise.
**Phase 6 notes (as built):** T030's gate green (67 binaries without `golden_speed`, the MSRV 1.85 build with `-D warnings`, the header drift check, `speed_design.py --check`, the C loop). T031: the solo figures of Phase 3 stand (level 3 +1.52 % against 0.12; the other levels +9.8 … +10.3 %; codec on 1.127×; the worst blocks under 1.5 % of real time). T032 (the hardware reviewer, "sound with fixes", B1 – B8) and T033 (the coach, "ready with fixes", R1 – R11) applied at the recommended answers, recorded in the spec's two new Clarifications sessions: the level-change step quantified and documented (kept instant), the golden's 6 kHz schedules made live (2 s stimuli, a schedule in calls of 121, a bless assertion; 196 entries), the place rule and the runner ceilings recorded in FR-009 / FR-010 / SC-005, the codec-on frame rule and the latch trigger made structural, the drain bound and level 3's per-call reports at the measurement rates, the other-rate spurious figures reported, the rustdoc and the design documents brought in line.

- [X] T034 Re-run T030's gate after the review fixes.
- [X] T035 **The golden bless, once, after the reviews**: `RR_DR60_BLESS=speed cargo test -p rr_dr60_harness --test golden_speed`; pin its SHA in `tests/golden_speed.rs`; add `golden_speed` to `.github/workflows/ci.yml` (the check job, the golden matrix, the iOS simulator job) and `scripts/ios-device-golden.sh`; add `RR_DR60_SPEED_COST_CEILING: "25"` and `RR_DR60_SPEED_CODEC_RATIO_CEILING: "2.0"` to the release timing job's environment; a CHANGELOG line for the bless; one commit.
- [X] T036 The full gate **with** coverage: `cargo llvm-cov --all-features --workspace --fail-under-lines 80` (within 1 point of 0.12's 97.96 %), and the iOS simulator golden run if available.
- [X] T037 CLAUDE.md: the 013 status line (the figures as the harness and the script print them, the lessons) and the commands (the reports, the bless, the A/B playback mode); the spec's Status line; the bookmark memory.
- [X] T038 Rebase onto `main` if PR #31 has merged (else keep the stack), push `013-playback-speed`, open a **draft** PR (to `main`, or to `research/noise-clock-power` if PR #31 is open) with the figures, the review notes and a checklist; when CI is green add the `ab-timing` label and quote the runner's A/B. **Do not mark the PR ready and do not merge**: the owner decides on return.

---

## Dependencies & Execution Order

- **Setup (T001–T008)** first. T002, T003, T006 and T007 are [P]; T004 needs T002 and T003; T005 needs T004; T008 last.
- **US1 (T009–T016)** depends on Setup. T009 → T010 → T011 (red) → T012 → T013 → T014; T015 and T016 [P] after T014.
- **US2 (T017–T023)** depends on US1. T017 (red) → T018 → T019; T020, T021 [P] after T018; T022 after T018; T023 after T019.
- **US3 (T024–T027)** depends on US1 and US2 (the behavior it documents). T024 (red) → T025; T026 [P]; T027 last.
- **C (T028–T029)** depends on US1 and US2.
- **Phase 6** depends on everything: T030 → T031 → T032 → T033 → T034 → T035 → T036 → T037 → T038.

## Parallel examples

- **Setup:** T002 (registers), T003 (CHANGELOG), T006 (core constants), T007 (harness helpers) together.
- **US1:** T015 (matrix) beside T016 (edge cases) after T014.
- **US2:** T020 (determinism, alloc) beside T021 (mutation).

## Implementation strategy

- **MVP = US1**: constant levels with the tail and the level-3 identity proof (T014).
- **Then US2**: the live change, the oracle, the cost.
- **Then US3, the C surface, the gates, the reviews and the single bless.**

The phase checkpoints are commit points. The owner is away: the PR is opened as a draft and nothing is merged or marked ready.

## Phase 7: Convergence

- [X] T039 Add to capture P1 in `.github/ISSUE_TEMPLATE/hardware_evidence.yml` the level-change capture A-054 names: a sustained vowel played back with the SPEED CONTROL dial turned during playback, captured at 48 kHz or more from the earphone jack, to show whether the device steps, changes at a frame boundary or mutes (the trigger for a crossfade, spec 013's post-implementation review B2), per A-054 verification / research R-10 (partial)
- [X] T040 Use `__test_hooks::speed_place_fifths` in `crates/rr_dr60_harness/tests/us2_speed.rs` (assert the place in fifths after each call against the independent reference's place sequence) or remove it from `crates/rr_dr60/src/lib.rs`, `crates/rr_dr60/src/stages/speed.rs` and `specs/013-playback-speed/contracts/rust-api.md`, per contracts/rust-api.md › Test hooks (unrequested)
