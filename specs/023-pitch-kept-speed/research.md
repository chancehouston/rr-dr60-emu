# Research: Pitch-Kept Playback Speed (spec 023)

**Date**: 2026-10-31 | **Spec**: [spec.md](spec.md) (D1 – D11 approved) | **Baseline**: 0.22.0, merge commit 12a3c45

Every item: Decision, Rationale, Alternatives. Code references are to the tree at 12a3c45.

## R-01 Where the time-scaler lives

**Decision**: inside `stages/speed.rs`'s `SpeedStage`, beside the resampler, as a fixed-size `TimeScaler` struct (no heap of its own; the window table a `Box<[f64]>` of 120 built at construction with `detmath::cos`, shared): fields `origin: u64` (the device output index the frame grid is anchored at), `n: u32` (the output's index inside the current synthesis hop, 0 … 59), `cur_a: i64`, `cur_s: i32` (the current frame's nominal analysis position, a whole stored sample, and its chosen shift), `prev_a`, `prev_s` (the previous frame's). The stage keeps `mode: Mode` and `requested_mode: Mode` beside `active`/`requested`, and the place arithmetic (`p5`, `step`, `latch`, the deferred switch into level 3, `final_count`) is untouched (D4). The pull window becomes `i + N + Δ` (160) in pitch-kept mode (`lookahead()` by mode); the ring span 228 ≤ 256 (hardware B3).

**Rationale**: the place is the clock in both modes (D4) and the transport's pull model, the latch and spec 020's walker read it; one stage, two mechanisms behind one `next()`. The steady identity fast path at level 3 is untouched (the mode test sits after its conditions: `fade.is_none()` already excludes a blend; `mode` is read only in `next_general`).

**Alternatives**: a separate stage after the resampler (two places to keep in step); a decoder-domain method (the codec is a class match).

## R-02 The synthesis

**Decision**: output o inside hop k (n = 0 … 59): `y = x[prev_a + prev_s + Ss + n] · w[Ss + n] + x[cur_a + cur_s + n] · w[n]`, both reads through `x()` (zeros outside [0, K)), w the periodic Hann of length N = 120 (`w[n] = 0.5 − 0.5·cos(2πn/N)`, so `w[n] + w[n + 60] = 1` within 2 ULP, pinned; `w[30]` pinned to the detmath value). At the hop boundary (n = 60) the frame advances: `prev ← cur`, `cur_a ← place at this output` (⌊p5 ÷ 5⌋, whole by construction since every hop is a multiple of 5 fifths: hardware B3), `cur_s ← the search`. The anchor: at the first output the scaler computes after leaving level 3 or at a mode change, `origin = o`, `prev_a = a₀ − Ss`, `prev_s = 0`, `cur_a = a₀`, `cur_s = 0` (the first frame has no continuation to match), so outputs 0 … 59 are `x[a₀ + n]·(w[60 + n] + w[n]) = x[a₀ + n]`: a full-gain onset (hardware B4c), no fade-in.

**Rationale**: one multiply-add pair per output plus the search once per hop; continuity at a ratio change is by construction (the previous frame's position is the actual one, not the nominal); the level-3 twin at the onset is exact on the first hop.

## R-03 The search

**Decision**: at each hop boundary, over shifts s in the outward order 0, −1, +1, −2, +2, … , −Δ, +Δ (Δ = 40), the plain cross-correlation `c(s) = Σ_{n<Lc} x[cur_a + s + n] · x[prev_a + prev_s + Ss + n]` (Lc = 60: the overlap region), summed as four partial sums in a fixed order (the `dot` pattern of 013 R-03), the shift with the strictly largest c (a tie keeps the earlier candidate, so the smaller displacement; silence gives 0). 81 candidates × 60 = 4860 multiply-adds per hop, once per 60 outputs: 81 per output on average (plus 2 for the synthesis), against the resampler's 57 (hardware B6: 1.5 ×). The pulls: `pull_through(cur_a + Δ + N)` before the search.

**Rationale**: hardware B3 (Δ ≥ P_max/2 for 75 Hz), B7 (determinism: no normalization, no `sqrt`, a fixed order, an outward tie-break).

**Alternatives**: normalized correlation (a division and a `sqrt` per candidate); Δ = 60 (2.2 ×); a fractional refinement (out of scope).

## R-04 The level change and the frame grid (D5)

**Decision**: the request lands in the place at d(P) exactly as today (`apply_request` changes `active`); in pitch-kept mode it starts **no** crossfade (the `Fade` is only for mechanism (b)'s level changes and for mode changes); the next hop boundary reads the new ratio through `cur_a = place`. The grid is anchored at `origin` and advances by Ss outputs per hop regardless of the host's blocks (partition-independent). A change into level 3 keeps the deferred switch (a whole place) and, once applied, the scaler plays its current frame's remaining overlap onto the identity output (the identity branch is `x[i]`; the remaining n′ = 60 − n outputs blend `x[prev_a + prev_s + Ss + n]·w[Ss + n] + x[i]·w[n]` with `i` the place: a continuous end, R-02's formula with the new frame the identity at shift 0), then the plain identity. A change out of level 3 anchors a new grid at d(P) (no wait).

**Oracle**: the change window's reading (019's 40 ms reader on the residual against the held-level run, both tones excluded) within 1 dB of the held window's; the mutant `restart_overlap` (the previous frame dropped at the change: `prev_a = cur_a − Ss` re-anchored) fails it.

## R-05 The mode change (D5)

**Decision**: a `Fade` whose old branch is the old mechanism frozen at its level: `Fade { old_k, old_p5, n, old_mode, old_scaler: TimeScaler }` (a copy of the scaler state when the old mode is pitch kept; a fixed-size struct, no allocation); the new branch starts at the common place: pitch kept → the scaler anchored at d(P) with shift 0; the default → the resampler at the place. The blend `y_old + w·(y_new − y_old)` over 120 outputs as spec 019. At level 3 (both branches the identity) the mode applies at once with no fade and logs its entry. The one wait rule: `apply_request` and `apply_mode_request` both return while `fade.is_some()`; the latest request per control waits; past the latch both are dropped (logged by the stream state as 019 D4). The pulls follow the leading branch (the larger base of the two).

**Oracle**: 019's click reader with both tones (f·r and f) excluded, the baseline the instant-switch mutant `mode_step`; the voiced figure reported; the time offset at the blend's end ≤ Δ stated.

## R-06 The length (D4)

**Decision**: the latch is 013's (the place reaches K). In pitch-kept mode the remaining outputs after the latch are counted by the same loop idea: the current hop m (its start o_m = origin + Ss·⌊(o − origin) ÷ Ss⌋) continues to its end, the next hop's frame has a ≥ K (zeros), so the content ends when the previous frame's tail has faded through the next hop: `T_c = 2·Ss − (o_L − o_m)` with o_L the latch output, in (60, 120]; so **T_c ≤ 120** (tighter than the spec's 160 bound; the shift cannot extend content past K since `x()` is zero there). `playback_length_samples_in_mode(number, level, mode)`: pitch kept at level 3 → the default's figure (the identity); else `own = ⌈5K ÷ k⌉`, `T_c` from `origin = 0` (a playback from the start at a constant level): `o_m = 60·⌊own ÷ 60⌋`, `T_c = 120 − (own − o_m)` when own mod 60 ≠ 0, else 60; total `⌈(own + T_c)·m ÷ l⌉ + drain`. `finished` arrives on it (asserted at seven rates). K = 1, 59, 119: own 2 / 99 / 199 at level 1, T_c 118 / 81 / 101; asserted.

**Rationale**: the house standard since 013 is exact (coach 4, hardware B4).

## R-07 The control (D1)

**Decision**: `controls.rs`: entry 30 `playback_speed_mode`, name "Playback speed mode", kind `ModelInternal`, timing `Live`, paths playback, stage 9, `ControlValue::Choice { options: [pitch_with_speed 0, pitch_kept 1], default: Some(0) }`, unit "", sources S-001, S-012, assumptions A-054, the description of US2 AS4; `CONTROL_COUNT` 31; `LIVE` grows to five (`battery_volts`, then the mode: the test's list); entry 2's description reworded (D8). `changes.rs`: `LIVE_SPEED_MODE = 30`, `Apply::SpeedMode(Mode)`, `StreamState::{last_mode, pending_mode, landed_mode}` as the battery's pattern, `MAX_CHANGES_PER_CALL` 143 (= 2·64 + 4 + 8 + 3: four plain live controls on the playback stream), the player's attained 69. `pipeline.rs`: `set_playback_speed_mode(mode: SpeedMode) -> Result<(), Error>` (never fails for the enum; the C path maps 0/1 or `InvalidSpeedMode`), `playback_speed_mode()`, kept by `reset`/`reconfigure` beside `speed_level`; `schedule_change` validation: exactly 0.0 or 1.0 (`InvalidControlValue`), the playback stream only (`ControlNotOnStream`); `error.rs`: `Error::InvalidSpeedMode { mode: u32 }` (C status 38). `transport.rs`: `apply_if_due` routes `Apply::SpeedMode` to `speed.request_mode(mode)` beside the level; `playback_length_samples_in_mode`. `rr_dr60::speed::{SpeedMode, MODE_COUNT}` public.

## R-08 The manifest (D8)

**Decision**: stage 9's controls note: `speech_codec_enabled; set_playback_speed; playback_speed_mode; PlayInfo.frames, PlayInfo.frame_index, PlayInfo.place` (116 ≤ 120; `Tap::AfterDecoder` to the rustdoc; `PlayInfo.frames` kept for `us1_manifest::codec_stages_us3_scenario_1`); coverage note "the device's own decoder (frame length, look-ahead, codebooks); its speed method, whether it keeps pitch, and the sound of a level change (A-054)" (count at implementation; ≤ 120, else "its speed method and whether it keeps pitch (A-054)"); model version 0.23.0; the control manifest and stage manifest re-blessed once each; the README table and guide blocks by the readme bless.

## R-09 The design script and the harness reference

**Decision**: `tools/playback-speed/speed_design.py --pitch-kept`: prints the window table (bits), the per-level T_c table for K in {1, 59, 119, 600, 6000}, the off-grid tone figures (1234.5 Hz: level and spurious per level; 2345 Hz; the two-tone), the vowel's and the glide's pitch figures with the reader's own error, the click-train counts, and the per-frame shift sequences for the golden stimuli; `--check` pins the margins (spurious ≥ 28 dB at 1234.5 Hz, the vowel within 1 %); `--write` regenerates the window table into `crates/rr_dr60/src/stages/speed_window.rs` (byte-identical). The harness `pitch_checks.rs`: `reference_output(x, schedule)` sharing only the published rules (the window bits, N, Ss, Δ, Lc, the outward tie-break, the anchor rule, the mode blend) so every schedule is bit-exact testable (013 R-09); the readers: `pitch_track(x, rate)` (normalized autocorrelation, 40 ms windows every 10 ms, a lag window ±20 % of the expected f0 for the glide, parabolic interpolation, voiced where the normalized peak ≥ 0.5), `spurious_offgrid` (013's `spurious_peak_db` on the sine-fit residual over the own part's middle, the first and last frame excluded), `click_count` (peaks above half the stimulus's click amplitude, min 20 ms apart). Stimuli: `stimulus::tone` at 1234.5 and 2345 Hz, a two-tone 700 + 1900, a steady vowel at 130 Hz (the glide generator at a fixed f0), the glide, a click train (one 1-sample click per 100 ms at 0.5).

## R-10 The 0.22 promise script (D9)

**Decision**: `scripts/abi-promise.sh <rev>`: `git show <rev>:crates/rr_dr60_ffi/include/rr_dr60.h` and `…/tests/c/smoke.c` into a scratch dir, `cc -std=c11 -I<scratch> smoke.c target/release/librr_dr60_ffi.a` (the current library), run the default mode and every `--x` mode, diff each `--x` output against the current-built smoke's, assert exit 0 and the ABI check line; CI: a step in the `check` job (ubuntu) after the header drift check, `scripts/abi-promise.sh 12a3c45`. The 0.22 smoke's `--controls` prints the library's 31 entries on both sides (the data is the library's), so the diff holds; the 0.22 smoke's own CHECKs that pin counts (e.g. "29 controls"? none: it reads `rr_dr60_control_count()`) are checked at the first run.

## R-11 Cost (D7)

**Decision**: `pitch_timing.rs` (release-only): per level 1, 2, 4, 5 the player's ns per host sample at 48 kHz, codec bypassed, speaker route, 4096 blocks, both modes, 45 alternating runs × 3; the increment over level 3 in pitch-kept mode ≤ 3 × the default's increment (asserted; `RR_DR60_PITCH_COST_CEILING` 4.0 on the label job); the worst 64-sample block at 6 and 48 kHz with the codec on (a block decoding two frames at the scaler's first output) against 1000 × 64 ÷ kHz µs; `scripts/ab-timing.sh 12a3c45 pitch` for the default mode (3 %, `RR_DR60_PITCH_DEFAULT_COST_CEILING` 0.05) and the workflow step.

## R-12 The golden and the records

**Decision**: `golden-pitch-kept-v1.json` per spec D8 with a `shifts` digest per entry (FNV over the per-frame shifts from the `__test-hooks` `pitch_shift_trace`); bless-time assertions: the default-mode twin bit-identical, the lengths equal to the helper, every scheduled change before the end, the shift digest equal to the harness reference's. Records per records.md; the mutants (`__test_hooks::PitchMutation`): `no_search` (shift forced 0: the off-grid tone's level and spurious), `restart_overlap` (R-04), `onset_fade` (the first frame's prev half zeroed: the first 60 outputs' level), `window_linear` (a triangular window: the vowel's level ripple), `tie_break_outward_off` (the first maximum in ascending order: the shift digest on silence-with-noise), `grid_unanchored` (the grid restarts at each play call: the partition identity), `mode_step` (the instant mode switch: the mode click table).

## Figures and restatements

*(Filled at implementation.)*
