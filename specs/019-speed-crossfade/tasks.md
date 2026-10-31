---

description: "Task list for the speed-change crossfade (019)"
---

# Tasks: Speed-Change Crossfade

**Input**: Design documents from `/specs/019-speed-crossfade/` (spec.md, plan.md, research.md R-01 – R-09, data-model.md, contracts/api.md, quickstart.md, records.md)

**Tests**: required (constitution III; spec FR-001 – FR-011, SC-001 – SC-006). Tests are written first in each story and fail before the implementation.

**Cadence** (owner, 2026-10-29): the phases run without stopping; stop only on a failed check or a new decision; the next owner stop is the merge.

## Format: `[ID] [P?] [Story] Description`

## Phase 1: Setup

- [X] T001 Bump the workspace to 0.19.0: `version = "0.19.0"` in `[workspace.package]` and the internal dependency entries of `Cargo.toml`; `RR_DR60_VERSION_MINOR = 19` and its test in `crates/rr_dr60_ffi/src/lib.rs`; regenerate `crates/rr_dr60_ffi/include/rr_dr60.h`. Re-bless the version line only: `RR_DR60_BLESS=manifest cargo test -p rr_dr60_harness --test us2_cross_check canonical` and `RR_DR60_BLESS=readme … readme -- --test-threads=1`; the CHANGELOG's 0.19.0 placeholder entry under Changed (research R-09's lead); commit.
- [X] T002 [P] Record the baseline in a scratch note: the thirteen audio golden SHA guards (`golden*.rs`), the smoke hashes incl. `SMOKE_SPEED_HASH` 0x0dd9_0285_502e_dcc3 and `SMOKE_SPEED_SAMPLES` 79 776, the 1055-test count of `cargo test --all-features`, coverage 97.28 % locally, the 0.18 merge commit `13defc7` as the A/B baseline; `uv run tools/playback-speed/speed_design.py --check` green.
- [X] T003 The design script: `tools/playback-speed/speed_design.py --crossfade` prints the weight table's pinned values from the pinned expression `u * u * (3.0 - 2.0 * u)`, u = n / 120 (w₃₀ = 0.15625, w₁₂₀ = 1.0, strictly increasing, w₁), the twenty rows of `SCRIPT_STEPS`' pairs on stored −6 dBFS tones with the step/crossfade click-energy ratio under the 40 ms and the 0.5 s readers (both output tones ± 250 Hz excluded, an exclusion above 3000 Hz folded to 6000 − f; the step reader centred on s, the crossfade's on s + 60) and the constant-run floor, the worst end cut per pair (count, peak dB re the tone), the ring span 153; `--check` asserts w₃₀ and w₁₂₀ exact, every 2 and 2.4 kHz row ≥ 30 dB under the 40 ms reader and the span ≤ 256. The scratch `xfade_sim.py` is folded in; the printed figures are recorded in research R-09.

## Phase 2: Foundational

- [X] T004 Core unit tests first (red until T005), in `crates/rr_dr60/src/stages/speed.rs`'s `mod tests` and a new `mod crossfade_tests`: `weights_are_the_smoothstep` (the table's w[29] == 0.15625 and w[119] == 1.0 bit-exact, strictly increasing, w[0] > 0, the pinned expression), `fade_starts_at_the_request_and_ends_at_n` (a 3 → 5 change at a known place: 119 blended outputs equal `y_old + w·(y_new − y_old)` from two reference stages at the two places, output 120 equals the level-5 stage's value exactly, output 121 on equals the constant level-5 stage from the common place), `fade_out_of_level_3_reads_whole_places`, `fade_into_level_3_waits_for_a_whole_place_then_blends`, `request_during_a_fade_waits` (the second request applies at n = N; the latest wins; an equal request starts nothing; a request at the latch is dropped), `fade_continues_through_the_tail` (both branches zero past K; the final count unchanged), `constant_level_is_untouched` (every level, a 4000-sample stored tone: the outputs equal a 0.18-form run, i.e. `fade_step` on, sample for sample), `ring_span_is_within_256` (a 1 ↔ 5 fade with the debug assertion live), `fade_step_is_0_18` (no fade; a request inside 20 ms applies at once), `fade_no_wait_restarts`, `fade_len_delta_moves_the_end`, `fade_linear_at_n_30`.
- [X] T005 Implement the stage (research R-01 – R-05): `Fade { old_k, old_p5, n }`, `fade: Option<Fade>`, `weights: Box<[f64]>` (120 entries from the pinned expression at construction; `fade_len_delta` changes the length), `RING = 256`, `dot(&self, bank: &[f64], ph, base)`, `apply_request` gated on `fade.is_none()` (the wait) and starting a fade on a change (`fade_step`: no fade, apply at once), the blend in `next_general` (n < N the blend, n = N the new branch's value; `old_p5 += step(old_k)`; `pull_through(max(base_old, base_new))`; `fade = None` at n = N), the fast path's explicit `fade.is_none()`, `fade_no_wait` (the new request applies at once, the running fade's new branch becomes the old branch, n restarts), `fade_linear`, debug assertions on the ring; `crates/rr_dr60/src/speed.rs` `CROSSFADE_SAMPLES = 120`, `CROSSFADE_MS = 20`; `crates/rr_dr60/src/lib.rs` `SpeedMutation::{fade_step, fade_linear, fade_len_delta, fade_no_wait}`, `__test_hooks::crossfade_weights(level) -> Vec<f64>` and `speed_device_outputs(pipeline) -> Vec<f64>` (the player records the speed stage's device outputs of the last play call under the hooks feature; `crates/rr_dr60/src/paths.rs` / `transport.rs`). `cargo test -p rr_dr60 stages::speed` green; the thirteen golden guards: every guard but `golden_speed` and `golden_changes` green (those two red by design until T017).

**Checkpoint**: the stage blends; the constant-level identity holds by construction.

## Phase 3: User Story 1 - Change the speed without a tick (P1)

**Goal**: the blend identity, the splice, the click table, the partitions, level 3, the end cut.

**Independent test**: `cargo test -p rr_dr60_harness --test us1_crossfade -- --nocapture`.

### Tests first (red until T005 + T006)

- [X] T006 [P] [US1] `crates/rr_dr60_harness/src/analysis.rs`: `click_energy_in(y, rate, centre, window_s, exclusions_hz: &[f64])` (a Hann window of `round(window_s · rate)` samples, 20 – 2750 Hz, each exclusion ± 250 Hz folded at the Nyquist, a union); `click_energy` delegates with 0.5 s and `[1000.0]`; spec 011's figures pinned unchanged (`volume_ramp_matrix` green). `crates/rr_dr60_harness/src/speed_checks.rs`: `reference_output_at` models the wait and the fade (`(old_k, old_p5, n)`; `value_at` for both branches; `y_old + w·(y_new − y_old)` for n < N, `y_new` at n = N; the weights from the pinned expression, asserted equal to `crossfade_weights`); spec 013's `us2_speed`, `us1_speed`, `speed_matrix`, `speed_edge_cases` green against it.
- [X] T007 [P] [US1] Create `crates/rr_dr60_harness/src/crossfade_checks.rs` (research R-06): `blend_identity(make, rate, pair)` (6 kHz: the engine's `speed_only` / `speed_only_codec` playback with a change at a device index against the reference, 0 mismatches; other rates: the `speed_device_outputs` trace against the reference), `splice(make, rate, pair)` (bit-identical to the old constant run before s and to the new constant-level computation from s + N − 1), `click_table(make)` (the twenty `SCRIPT_STEPS` rows at 6 kHz: the `fade_step` mutant's output as the switch; the 40 ms reader with both tones excluded, the step centred on s, the crossfade on s + 60; the thirteen 2 and 2.4 kHz rows asserted ≥ 30 dB, the seven 1 kHz rows reported with both absolute energies; the 0.5 s reader and a constant-run floor row reported; each within 0.5 dB of the script's print), `voiced_click(make)` (both absolute energies, reported), `wait_cases(make, rate)` (consecutive requests, a superseded request, a request equal to the target, a request at the latch: the engine equals the reference bit for bit; the log holds every entry with chained targets), `end_cut(make)` (per pair at the latest landing that starts a fade: the cut count, peak dB re the stimulus and energy, in both configurations; reported), `partitions(make, rate)` (block sizes 1, 64, 4096 and the irregular partitions through spec 015's schedule: identical outputs and logs), `lengths_unchanged(make, rate)` (013's `lengths` at every constant level, and with a fade in flight). `pub mod crossfade_checks;` in `src/lib.rs`.
- [X] T008 [P] [US1] Create `crates/rr_dr60_harness/tests/us1_crossfade.rs` (spec US1 AS1 – AS6) at `configs::measurement_rates()`: `as1_constant_levels_bit_identical` (every level, both configurations, against the `fade_step` build: 0 mismatches), `as2_blend_identity_and_splice`, `as3_click_table_and_voiced` (6 kHz), `as4_partitions`, `as5_level_3` (the deferred whole place; the constant level 3 exactly the stored samples), `as6_end_cut`. Six tests.
- [X] T009 [P] [US1] Create `crates/rr_dr60_harness/tests/crossfade_matrix.rs`: `blend_and_splice_at_every_rate` (`configs::all_rates()`, the four pairs 3 → 5, 5 → 1, 1 → 3, 2 → 4), `partitions_and_lengths_at_every_rate`, `click_table_at_6k` (both readers, the floor, the voiced energies), `end_cut_at_6k`. Four tests.
- [X] T010 [US1] Run T008, T009: `us1_crossfade` 6, `crossfade_matrix` 4, all green; record the figures (the twenty rows under both readers, the floor, the voiced energies, the worst end cut) in research R-09 against the script's print.

**Checkpoint**: US1 green at seven rates; the click reduction measured.

## Phase 4: User Story 2 - The change is bounded and described (P2)

**Goal**: the wait's semantics, the manifest, the records' pins, the C define, the re-pins, the cost.

**Independent test**: `cargo test -p rr_dr60_harness --test us2_crossfade -- --nocapture`; `--test us1_manifest --test us2_cross_check`.

- [X] T011 [P] [US2] Create `crates/rr_dr60_harness/tests/us2_crossfade.rs` (spec US2 AS1 – AS2): `as1_wait_cases` (the four wait cases at the measurement rates against the reference; `playback_speed()` reads the latest target; the log's chained entries), `as2_manifest_and_register` (stage 9's description names the blend, its coverage note the sound of a level change and A-054, model version 0.19.0; the control manifest unchanged; A-054's row carries "engine convention" and "capture P1"). Two tests; AS3 is the A/B script, AS4 `ffi_parity speed`.
- [X] T012 [P] [US2] Create `crates/rr_dr60_harness/tests/crossfade_edge_cases.rs` (spec › Edge cases): a change to the active level (no fade, the entry logged), a change in the start call and one scheduled at position 0 (start values, no blend, no entry), consecutive-position requests (the first starts, the last of the rest waits), two changes at one position (the later wins), a change at the last own sample and during the tail, a playback stopped during a fade (the next starts at the latest request, no blend), the one-pass ignores the speed (bit-identical to 0.18), a change into level 3 during a fade (the fade, then the wait, then the blend). ≥ 8 tests.
- [X] T013 [US2] The manifest and the registers, one commit: `crates/rr_dr60/src/manifest.rs` stage 9's description "…at one of five playback speeds (level 3 is normal); a level change blends over 20 ms, an engine convention", its coverage note "the device's own decoder (frame length, look-ahead, codebooks); its speed method and the sound of a level change (A-054)", model version 0.19.0 (the const assertions check the lengths; `us1_manifest`'s pinned tokens kept); `docs/hardware/assumptions.md` A-054 amended as records.md's hypothesis wording (P1's new capture), `.github/ISSUE_TEMPLATE/hardware_evidence.yml` P1 line; `crates/rr_dr60_harness/tests/us2_cross_check.rs` pins "engine convention" and "capture P1" in A-054's row; re-bless the manifest canonical text and the README table / guide blocks; `cargo test -p rr_dr60_harness --test us1_manifest --test us2_cross_check` green.
- [X] T014 [US2] The C define: `pub const RR_DR60_SPEED_CROSSFADE_MS: u32 = 20;` in `crates/rr_dr60_ffi/src/lib.rs` with a unit test pinning it to `rr_dr60::speed::CROSSFADE_MS`, the header regenerated (one define added; the drift check); `crates/rr_dr60_harness/tests/ffi_parity.rs` `speed_crossfade_parity` (the smoke's speed case Rust versus C bit for bit, plus a change inside 20 ms on both paths), `crates/rr_dr60_harness/tests/alloc_free.rs` `crossfade_allocate_nothing` (a change during a fade and a producing call), `crates/rr_dr60_harness/tests/determinism.rs` `crossfade_same_calls_same_output` (two engines, a schedule with a waited request, at 6000 and 48 000). Swift: `scripts/build-xcframework.sh` then `cd swift && swift test`.
- [X] T015 [US2] Run T011, T012: `us2_crossfade` 2, `crossfade_edge_cases` ≥ 8, green; record the counts.

**Checkpoint**: the wait is described and pinned; C agrees.

## Phase 5: Mutants, the re-pins, the smoke and the cost

- [X] T016 [P] `crates/rr_dr60_harness/tests/mutation.rs` `mod crossfade`: four tests, each asserting the named oracle fails on the mutant and passes on the default: `fade_step` by the blend identity and the click table (the switch reads < 30 dB), `fade_linear` by the weight at n = 30 (0.25 vs 0.15625 through the blend identity), `fade_len_delta` (±1) by the splice at s + N − 1, `fade_no_wait` by the wait cases. `cargo test -p rr_dr60_harness --test mutation crossfade::` 4 green.
- [X] T017 The re-pins (research R-07), in this order: (a) the gate: with `fade_step` on, `golden::generate_speed` and the changes golden's entries with a speed change regenerate the committed files bit for bit (`golden::speed_re_bless_gate` / `changes_re_bless_gate`, run at bless time while the committed file is still 0.18's by its SHA-256, self-skipping afterwards; ran once 2026-10-30: 20 of 196 and 21 of 98 entries moved); (b) `RR_DR60_BLESS=speed cargo test -p rr_dr60_harness --test golden_speed` with the bless-time assertions: every entry without a schedule byte-identical to the committed file, every scheduled entry's changes land before the end and its blend completes before K; pin the new SHA; (c) `RR_DR60_BLESS=changes cargo test -p rr_dr60_harness --test golden_changes` with the assertions: every non-`speed_inside` entry byte-identical, the `speed_inside` produced counts moved by the wait as predicted; pin the new SHA; (d) `smoke_reference::SMOKE_SPEED_HASH` re-pinned from the Rust path, `SMOKE_SPEED_SAMPLES` asserted unchanged, `smoke.c`'s defines following; `crates/rr_dr60_ffi/tests/c/run_smoke.sh` green. CHANGELOG entries for the three re-pins.
- [X] T018 [P] Cost (research R-08; FR-009): create `crates/rr_dr60_harness/tests/crossfade_timing.rs` (the 0.18 API: a recorded 30 s stimulus played at a constant level 3 and a constant level 5 on the default front, the codec bypassed, 48 kHz, fastest of 45: `level3_ns=`, `level5_ns=`) and `crates/rr_dr60_harness/tests/crossfade_fades_timing.rs` (head-only: a level change every 200 ms cycling 3 → 5 → 1 → 3; `fades_ns=` reported; the worst 4096-sample block asserted ≤ 85 333 µs); the `crossfade` mode in `scripts/ab-timing.sh` (both figures asserted within `RR_DR60_CROSSFADE_COST_CEILING`, default 0.03) and the step in `.github/workflows/ab-timing.yml` (ceiling 0.05). Run `scripts/ab-timing.sh 13defc7 crossfade` solo; record the figures in research R-08 and the CHANGELOG; PASS required.

## Phase 6: Polish

- [X] T019 [P] Documentation (FR-010; records.md): `docs/guide.md`'s playback-speed paragraph (the measured reduction, the onset sentence, the end cut, the constants), the real-time list unchanged; `CHANGELOG.md` 0.19.0 under Changed with the figures of T010, T017 and T018; the product notes (controls-discussion § 2b rows and § 4 item 6, decisions.md's row, investigation-mode.md § 5's bullet, brief.md's speed line, monetization.md's parenthetical) with the merge date left as "2026-10-30 (PR pending)"; CLAUDE.md's "Spec 019 reports" commands; records.md's wording rules observed.
- [X] T020 Full gate: `cargo fmt --all -- --check`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo test --all-features` (record the count against T002); `RUSTFLAGS="-D warnings" cargo build -p rr_dr60_ffi --release` and `--target aarch64-apple-ios`; the MSRV 1.85 build with `RUSTFLAGS="-D warnings"`; `scripts/check-traceability.sh`; the header drift check; the eleven audio golden SHA guards without a speed schedule unchanged (T002); `cargo llvm-cov --all-features --workspace --fail-under-lines 80` (within 1 point of 97.28 %); the iOS simulator golden run (`golden_speed`, `golden_changes`, `determinism`); every quickstart line's test count > 0; `uv run tools/playback-speed/speed_design.py --check`.
- [X] T021 Post-implementation reviews: the speckit-coach and the hw-emulation-reviewer on the implementation and the records; apply the findings; record them in spec.md's Clarifications as a dated session.
- [ ] T022 Push the branch, open a PR with the `ab-timing` label, CI green on all 12 jobs plus `registers`; the CLAUDE.md status entry (figures, lessons); `/speckit-converge`; the owner merges; post-merge records (the merge date in the spec, CHANGELOG and product notes; CLAUDE.md's engine order; the memory bookmark).

## Dependencies

- T001 → everything. T003 → T004 → T005 → every story.
- US1: T006, T007, T008, T009 (parallel) → T010.
- US2 needs US1: T011, T012 (parallel) → T013 → T014 → T015.
- Phase 5 after US2: T016, T018 parallel; T017 after T005 and T006 (the gate needs the mutant and the reference).
- Phase 6: T019 after T010, T017, T018; T020 → T021 → T022.

## Parallel opportunities

- T002 with T001. T006, T007, T008, T009 together. T011 with T012. T016 with T018. T019 with T020's early steps.

## Implementation strategy

MVP = Phase 1 – 3 (the stage blends, the identities and the click table hold at 6 kHz). Then US2 (the wait's description, the manifest, C), the re-pins, the cost and the records.

## Phase 7: Convergence

- [X] T023 Add an edge case to `crates/rr_dr60_harness/tests/crossfade_edge_cases.rs`: a change scheduled inside a blend that runs into the latch is dropped without sound but logged with chained target levels, the output equal to the reference, per FR-005 "a waited request reaching the latch is dropped, its entry logged" (partial)
