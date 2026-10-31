# Research: Speed-Change Crossfade (019)

## R-01: The fade inside the stage

**Decision.** `SpeedStage` gains `fade: Option<Fade>`, `Fade { old_k: u32, old_p5: u64, n: u32 }`. `apply_request` runs only while `fade.is_none()` (and not latched): a request for a level other than the active one starts a fade (`old_k = active`, `old_p5 = p5`, `n = 0`, then `active = requested`); a request equal to the active level does nothing; the deferred switch into level 3 waits for a whole place as today (so a blend into 3 starts at a whole place). Per output while a fade runs: `n += 1`; `y_new` is the active level's value at `p5` as today; `y_old` is the old branch's constant-level value at `old_p5` (`x(i_old)` for `old_k == 5`, else `dot(bank(old_k), ph_old, i_old + D)`); `y = y_old + w[n − 1]·(y_new − y_old)`; `old_p5 += old_k`; when `n == N` the fade ends (`fade = None`), the next output being the new branch alone. The `dot` signature takes the bank slice; the ring and the pulls are shared. The "steady identity" fast path (level 3, no request pending, no fade) is unchanged, so a constant level 3 stays bit-identical; at the other levels a constant level never enters the fade branch, so FR-003 holds by construction (no fade → the 0.18 code path, sample for sample).

**Rationale.** D1, D4, D5; the hardware review's simulation (the same-place blend aliases; the time-domain crossfade reads 44.7 – 53.8 dB); the coach's finding that a 2550 Hz bank is not the identity at whole places (no level-3 branch is needed under D1).

**Alternatives.** The same-place blend (rejected: aliasing, the pitch step kept); restarting a fade on a new request (D4's alternative; the `fade_no_wait` mutant); a crossfade outside the stage at the host rate (rejected: the stage's place arithmetic is where both branches are exact).

## R-02: N and the weights

**Decision.** `speed::CROSSFADE_SAMPLES: u32 = 120` (20 ms of device samples, A-045's length restated as an engine convention) and `speed::CROSSFADE_MS: u32 = 20` (published; a C define follows only if a reviewer asks, since the C API is otherwise unchanged). The weights are a `[f64; 120]` table filled at construction: `w[n − 1] = s(n / 120)`, `s(u) = u·u·(3 − 2u)`, computed in f64 from the integer n; the table costs nothing per sample and is bit-identical to computing per sample. Pins: `w[29] == 0.15625` exactly (u = 0.25: 0.0625·2.5 = 0.15625, exact in binary), `w[119] == 1.0` exactly, `w` strictly increasing, `w[0] > 0`. The design script prints the same table's two pinned values.

**Rationale.** D2, D3; SC-002.

**Alternatives.** Per-sample computation (equivalent; the table avoids a divide); equal-power weights (unneeded: the dip of an amplitude blend between incoherent branches is a few milliseconds).

## R-03: The ring and the pulls

**Decision.** `RING` 128 → 256. The two branches' places diverge by at most |k_old − k_new|·N ÷ 5 = 4·120 ÷ 5 = 96 stored samples over a fade (1 ↔ 5); the old branch's kernel reaches 57 taps back from its base and the new branch's look-ahead leads by 28: the span is at most 96 + 57 + 28 = 181 < 256. The pulls are driven by whichever branch reads the highest index (the new branch when faster, the old when the new is slower and the old leads); `pull_through` stays at most 2 + 29 per output (the per-output advance is at most 7 fifths, and a fade's start pulls at most the look-ahead once). The mask form (`& (RING − 1)`) needs the power of two.

**Rationale.** Hardware B4; FR-009.

**Alternatives.** 192 (not a power of two; the mask would become a modulo); keeping 128 and bounding the fade (would truncate 1 ↔ 5 fades).

## R-04: The latch, the tail and the end cut

**Decision.** The latch, `final_count`, the tail (`Tail` outputs) and the `Silence` boundary follow the new branch's `p5` exactly as in 0.18; a fade in progress continues through the tail (both branches read zeros past K through `x`) and is cut when the player stops requesting (the drain follows). The worst cut is the old branch's remaining fade samples at that moment: when the new level is faster the new branch reaches K first; the matrix reports the worst number of cut blend samples per level pair for a change at the latest landing that still starts a fade, and the mostly-silent content of the cut (the stop's fed silence, ≥ 60 device samples, spec 008).

**Rationale.** D6, FR-006; the length rule stays spec 013's.

**Alternatives.** Extending the final count by the fade's remainder (rejected by D6: it changes `playback_length_samples` and the length tests for a mostly silent remainder).

## R-05: The mutants

**Decision.** `SpeedMutation` gains `fade_step: bool` (the weight is 1 from blend sample 1: the 0.18 switch, bit-identical to 0.18's schedule outputs, which also realizes the click reader's "instantaneous switch" reference), `fade_linear: bool` (w = u), `fade_len_delta: i32` (N ± d, the table recomputed at construction) and `fade_no_wait: bool` (a request during a fade applies at once, restarting the fade from the blended output's branches: D4's rejected alternative). Each is caught by a named oracle: `fade_step` by the blend identity and the click table; `fade_linear` by the weight at ⌊N/4⌋ (0.25 vs 0.15625); `fade_len_delta` by the splice at s + N − 1 and the weight pins; `fade_no_wait` by the wait cases (the harness reference models the wait).

**Rationale.** D8, SC-002; the house pattern (every mutant has a named oracle that fails on it and passes on the default).

## R-06: The harness reference, the reader and the checks

**Decision.** `speed_checks::reference_output_at` gains the wait and the fade: it keeps `(old_k, old_p5, n)` beside the place, applies a request only when no fade runs, and blends with `value_at` for both branches and the published weights (read through a `__test_hooks::crossfade_weights()` hook or recomputed from the formula; the core pins both agree), narrowing after the blend. Spec 013's `us2_speed` schedules then stay bit-exact oracles with the reference updated once. The new reader `analysis::click_energy_in(y, rate, centre, window_s, exclusions_hz)`: a Hann window of `round(window_s·rate)` samples centred on `centre`, the power of the bins in 20 – 2750 Hz excluding each exclusion ± 250 Hz; `click_energy` delegates with 0.5 s and `[1000.0]` (spec 011's figures unchanged, pinned by its tests). `crossfade_checks.rs`: `blend_identity(rate, pair)` (the engine's playback through `speed_only` at 6 kHz and the default at every rate against the reference: 0 mismatches), `splice(rate, pair)` (bit-identical to the old constant run before s and to the new constant-level computation from s + N − 1), `click_table()` (the twelve pairs of 013's step table on stored −6 dBFS tones at 1, 2 and 2.4 kHz at 6 kHz: the `fade_step` mutant's output as the switch, the 40 ms reader centred on s + N/2 with both output tones excluded, asserted ≥ 30 dB; the 0.5 s reader reported), `voiced_click()` (reported), `wait_cases()` (consecutive requests, a superseded request, a request equal to the target, a request at the latch: the engine equals the reference bit for bit and the log holds every entry), `end_cut()` (reported), `partitions(rate)` (block sizes 1, 64, 4096, the irregular partitions through spec 015's schedule: identical outputs and logs), `lengths_unchanged(rate)` (013's `lengths` at every level, a fade in flight).

**Rationale.** D8; FR-007, FR-008, FR-011; hardware B1 (the 011 reader is unusable here), coach 3 and 5.

## R-07: The three re-pins

**Decision.** `generate_speed(bless)` asserts at bless time that every entry without a schedule is byte-identical to the committed file (the constant-level entries) and that every scheduled entry's changes land before the end; `generate_changes(bless)` asserts that every entry but the `speed_inside` scenarios is byte-identical; `smoke_reference::SMOKE_SPEED_HASH` and `SMOKE_SPEED_SAMPLES` are re-pinned from the Rust path (the sample count is unchanged by the fade, which adds no samples: assert it), and `smoke.c`'s defines follow. The eleven other audio golden guards are unmoved (their configurations have no speed schedule), as are every other smoke hash.

**Rationale.** D7, FR-003; coach 2.

## R-08: Cost

**Decision.** A `crossfade` mode in `scripts/ab-timing.sh` reusing `tests/playback_timing.rs` (the 0.12 API: level 3, the default front) against 13defc7 (3 %; `RR_DR60_CROSSFADE_COST_CEILING` 0.05 on the label job); `crossfade_timing.rs` head-only: a recorded 30 s stimulus played with a level change every 200 ms cycling 3 → 5 → 1 → 3 (every fade 120 device samples of two dots), the per-sample cost and the worst 4096-sample block reported. Expected: the constant-level cost moves by one `fade.is_none()` test on both paths (under 1 %); a fade's extra work is 57 multiply-adds per device sample for 120 samples.

**Rationale.** D9, FR-009.

## R-09: The design script and the records

**Decision.** `speed_design.py --crossfade` prints the weight table's pinned values (w₃₀ = 0.15625, w₁₂₀ = 1.0), the twelve pairs' click reductions under the 40 ms and the 0.5 s readers (the simulation of the hardware review, now the script's own: the harness pins each within 0.5 dB), the worst end cut per pair and the ring span (181). The records follow records.md: A-054 amended (the crossfade is the emulator's convention; the device's natural change quantum is the 20 ms CELP frame, so a frame-boundary switch is the plausible device behavior; capture P1 reworded: a sustained 2.3 kHz tone and a vowel with the dial turned, 48 kHz, read for a step, a fade or a gap and for 20 ms alignment), the issue template's P1 line, stage 9's `controls` and `coverage` notes within 120 characters ("… a 20 ms blend at a level change (engine convention)"; coverage adds "the change's own sound"), the manifest canonical text, README table and guide blocks re-blessed once, the CHANGELOG as a convention change visible in schedule outputs and not a device behavior, the guide's playback-speed paragraph, the product notes.

**Rationale.** FR-002, FR-010; product M3, M4; hardware B8.
