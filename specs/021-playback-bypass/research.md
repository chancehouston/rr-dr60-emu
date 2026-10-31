# Research: Live Playback-Stage Bypass (spec 021)

**Date**: 2026-10-31 | **Spec**: [spec.md](spec.md) (D1 – D11 approved) | **Baseline**: 0.20.0, merge commit f303c99

Every item: Decision, Rationale, Alternatives. Code references are to the tree at f303c99.

## R-01 The weight per sample: the expression, not a table

**Decision**: w = u·u·(3 − 2u) computed per sample from `u = k · inv_n` (k the phase index 0 … N, `inv_n = 1 / N` computed at construction), the same form spec 011's volume transition uses (`stages/output.rs:462, 496`) and spec 019 pins (`u * u * (3.0 - 2.0 * u)`). N = 0.02 × rate host samples for stages 11 – 13 (120 … 1920), 120 device samples for stage 10.

**Rationale**: a table would be 1920 f64 per stage at 96 kHz for a quantity that costs one multiply-add while a fade runs and nothing while idle; the expression is the pinned form the harness reference and spec 019's script already use, so the three blends (011 volume, 019 speed, 021 bypass) share one definition. Exactness: at k = N/2 (N is even at every rate: 120, 160, 320, 882, 960, 1764, 1920) u = 0.5 exactly (`k · inv_n` with inv_n = 1/N is exact for k = N/2 only when N is a power of two; for the others u = 0.5 within one ULP, so the core pins `w(N/2)` within 1 ULP of 0.5 and exactly 0.5 at the power-of-two rates: **restated** from the spec's "exactly": the pin is `(w − 0.5).abs() <= f64::EPSILON`), and at k = N the sample is selected, not computed (spec D3).

**Alternatives**: a per-rate table filled at construction (spec 019's 120-entry device-rate table; here a host-rate table up to 1920 entries per rate; no exactness gain); a linear weight (the mutant).

## R-02 The fade state and the reverse-from-phase arithmetic

**Decision**: per stage a `Fade { k: u32, dir: i8, n: u32, inv_n: f64 }` where `dir` ∈ {−1, 0, +1} (+1 toward bypassed, −1 toward on, 0 at rest) and the rest state is read from k (0 = on, N = bypassed). A landing of value v: if the stage is at rest at v, a no-op; else `dir = if v { +1 } else { −1 }` (from rest, or a reversal from the current k when a fade runs: k is kept, only `dir` flips). Per sample while `dir != 0`: `k = (k as i64 + dir) as u32`; if k == N: select x, dir = 0; if k == 0: select y, dir = 0; else `w = smooth(k · inv_n)`, `y + w·(x − y)`. The landing sample P computes k = 1 (the first blend sample is the landing sample, spec 011/019's indexing).

**Rationale**: the two-state switch's natural reverse is a direction flip at the current phase (spec D5); monotone within a direction by construction; a toggle at k = N/2 returns to 0 after N/2 samples; a toggle on the last blend sample (k = N just selected) sets dir = −1 and the next sample computes k = N − 1 (w just below 1: "starts from w = 1"). Two changes at one position: the first sets dir = +1 from k = 0, the second flips to −1 at k = 0 before any sample is computed, so the per-sample step would compute k = −1: the step clamps at 0 (select y, dir = 0): no blend, two entries (spec's same-position rule).

**Alternatives**: a phase `u: f64` incremented by ±inv_n (accumulates rounding; the integer k does not); a wait (spec 019 D4) or a restart from rest (spec 011).

## R-03 Stage 10's blend and its host-output settle; the device-rate trace hook

**Decision**: stage 10's blend sits in `PlaybackHalf::process` (`paths.rs:347`): `y = playback.process(d)` always runs when `run_playback`; the returned value is the fade's select/blend of y and d. The landing reaches the half through the player's existing pre-pull apply (`transport.rs:680`: `player.apply_if_due(&mut |level| speed.request(level))`): `Apply::Bypass(STAGE_10, on)` is handled inside `PlaybackPath::apply_if_due` by `self.half.set_bypass(on)` and takes effect on the next device sample pulled, which is d(P) = `changes::speed_landing(P, rate)` by spec 015's rule (the same reason the speed's request lands there). On the host output the blend passes the interpolator (a linear-phase kernel of delay D host samples, `paths.rs:1270` `D = [0, 40, 80, 219, 238, 437, 476]`): the first host sample that differs from the all-on run is P + 1 … 16 by rate (spec 015's speed-onset lesson), the audible onset about P + D, and the identity with the configured-off twin holds from P + ⌈119·rate ÷ 6000⌉ + 2D at the latest. **The harness measures the first differing and the first identical host sample per rate and the records quote the measured figures**, with the formula as the bound (hardware B2; spec 010's lesson).

The device-rate trace hook: `__test_hooks` `stage10_trace` (a `Vec<f32>` reserved at construction with 8192 entries like `speed_place_trace`, spec 020 R-04) records the half's **output** per device sample pushed during a play call (the existing `trace` records the speed stage's output, i.e. stage 10's input, so the pair gives x and y). The one-pass has no stage-10 bypass (D6), so the hook is the player's.

**Rationale**: the stage keeps running (D3), the half's `run_playback` stays construction-time (D4), and the player's apply point already precedes the pull. At 6 kHz `up` is `None` and the host output is the device stream (`pending`), so the identity on the host output holds from P + 119 there.

**Alternatives**: blending after the interpolator (would blend two host-rate streams and need a second interpolator: twice the cost and a different stage 10); a one-pass device-clock apply point (rejected, D6).

## R-04 "Runs on the stream": the refusal condition

**Decision**: `schedule_change` refuses with `Error::StageNotConfigured { control }` (C status 36) **any change** (0 or 1) of a stage switch whose stage does not run on the target stream: the player's `half.runs()` for stage 10 and `chain.runs_amp() / runs_speaker() / runs_earphone()` for 11 – 13 (the existing `run_*` flags, which already combine the construction setting with the one-pass's tap reach: `OutputChain::new(.., honour_tap)` `output.rs:258 – 278`; `PlaybackHalf::new` `paths.rs:340`). Restated from the spec's "a change that would switch on": a change *to off* of a non-running stage is refused too, because landing it would write `settings()` to off for a stage whose setting may be on (not reached by the tap), after which an on-change is refused: one rule, no trap. The route is not part of the rule: stage 12 runs on the one-pass and the player whatever the route (its blend's phase advances regardless, spec FR-008).

For the one-pass's mirroring (R-05): a value landed on the player for a stage the one-pass does not run is **not** mirrored (nothing to apply there); `settings()` still reflects the player's landing.

**Rationale**: the run flags are the one source of truth for "the stage runs here"; the refusal text names the fix ("configured off at construction or not reached by the tap; construct with the stage on and bypass live").

**Alternatives**: refuse on-from-off only (the spec's first wording; the trap above); land-and-log silently (the hardware review's "accepted, logged, changes nothing").

## R-05 Mirroring between the player and the one-pass; the bound 142

**Decision**: `StreamState` (`changes.rs:440`) gains `last_bypass: [bool; 3]` (stages 11, 12, 13), `landed_bypass: [Option<bool>; 3]`, and the cross-stream pending set `xpend: [Option<(bool, u64)>; 3]` (value, tag) with `next_apply` armed at the stream's next sample while any is pending (spec 017's `RecordCtl` pattern, `changes.rs:966 – 1071`). After every processing call the pipeline (as at `pipeline.rs:1099` for the record side) reads each stream's `landed_bypass` and sets the other stream's `xpend` for the stages that run there (R-04); `start_playback` adopts the engine's current values with no entry (`set_start_values` extended). The player's stage 10 has no mirror (D6). `apply_due` applies the cross-stream pending changes first (slot order), then the plain change, then the schedule, each logged with its tag. `MAX_CHANGES_PER_CALL` = 2·64 + 3 + 8 + 3 = **142** (`changes.rs:20`, its test at `changes.rs:400`, the C define and the docs).

**Rationale**: spec 017 D4's rule, so a Workbench toggle made during a one-pass monitoring session is heard on the next playback and vice versa; `settings()` is single-valued because the last landing anywhere is the value.

**Alternatives**: independent values per stream (spec 015's live controls; `settings()` ambiguous); mirroring through `settings` at the next call boundary without an entry (loses the log).

## R-06 The click reader at 48 kHz and the per-stage stimulus rows

**Decision**: `analysis::click_energy_in` keeps its signature and 2750 Hz top; a new `click_energy_band(y, rate, centre, window_s, exclusions_hz, top_hz)` takes the band's top, `click_energy_in` delegating with 2750. The asserted rows (6 kHz, the stage-only configurations, a −6 dBFS stored 1 kHz tone 1.5 s long, the landing at 0.75 s, the reader's 40 ms window centred on the blend's midpoint, ±250 Hz around 1 kHz excluded, the floor the reader on the all-on held run): stage 10 (`playback_only`), stage 11 speaker at the reference position and earphone at position 1 (`output_only_amp` at `AfterVolumeAmp`), stage 12 speaker (`output_only_speaker`), stage 13 earphone (`output_only_earphone`); the step realized by the `fade_step` mutant (R-09's mutants). The reported rows: stage 10 at 2.6 kHz (the tone written into the memory directly with `import`, since the record band-pass would remove it), stage 12 at 300 Hz, stage 13 at 100 Hz, stage 11's corners at 100 Hz (both routes at the neutral positions), stage 11's level-change rows (speaker at position 1, earphone at the reference position) printed as the level change in dB (20·log10 of the route scalar) beside their reader figures, every asserted row at 48 kHz with `top_hz = 0.45 × rate`, the voiced stimulus (`changes_checks`' stored recording) per stage, and the reversal at u = 1/2 for stage 12.

**Rationale**: spec 019's reader is the house instrument; the top as a parameter is the one change 48 kHz needs; the level-change rows are reported because the reader excludes a level step's sidebands by design (coach 6).

**Alternatives**: a new reader (no); asserting 48 kHz (the hardware review: the ratio there understates the step inside 2750 Hz; reported only).

## R-07 Stage 11's level figures

**Decision**: the guide's table is computed from the committed law: the bypass changes the level by −20·log10(k_route(p)) where `k_ear(p) = g_ear(p)` (`output.rs:68`, published as `earphone_gain_db`) and `k_spk(p) = g_spk(p) · CODEC_FULL_SCALE_V / V_CLIP_V` (`output.rs:62`; `speaker_gain_db` publishes g_spk, so the speaker figure is `speaker_gain_db(p) − 20·log10(CODEC_FULL_SCALE_V / V_CLIP_V)`). Figures (the harness prints them; a core test pins the table's rows to the functions within 0.01 dB): earphone at the reference position +19.2 dB, at position 1 0.0 dB; speaker at the reference position about 0 dB (k_spk(P_REF) is unity after the f32 rounding, spec 009), at position 1 −21.5 dB (above the knee the on-branch is clipped, so the figure is "at a level below the knee"), at position 0 the on-branch is silent. The neutral positions: the speaker route's reference position, the earphone route's position 1.

**Rationale**: D11; the app shows the delta from the two public functions plus one published constant (`output::SPEAKER_SCALE_DB`, new: 20·log10(CODEC_FULL_SCALE_V / V_CLIP_V), so the app needs no chain internals).

**Alternatives**: a level-kept bypass (rejected by the owner, D11).

## R-08 The hot-path cost plan

**Decision**: one `fading: bool` ("any fade running") and three rest bits packed with the existing `active`/`ramp_k` fields at the top of `OutputChain` (the hot cache line; spec 015/017's lesson), and the three per-stage `Fade` structs after them. In `inner()`: `if self.fading { self.fade_step() }` once per sample (cold when false), and at each stage point a select `if byp { x } else { y }` where `byp` is the rest bit, or the blend value while that stage fades. In the all-on state the added work is one bool test and three predictable selects per sample; the A/B budget is 3 % on both streams (`bypass` mode in `scripts/ab-timing.sh` against f303c99, `tests/bypass_timing.rs` written against 0.20's API, `RR_DR60_BYPASS_COST_CEILING` 0.05 on the label job); the bypassed steady state and a blend in progress are reported. Stage 10's half: one `dir != 0` test per device sample plus the rest select.

**Rationale**: spec 017's experience (+2.5 … +3.6 % before two folds, +0.9 % after) says the compare's cache line decides the cost, not its instruction count.

**Alternatives**: a per-sample match on an enum per stage (more branches); recomputing the chain twice during a blend (D3's rejected alternative).

## R-09 The golden and the mutants

**Decision**: `golden-bypass-v1.json` (`RR_DR60_BLESS=bypass`, `golden_bypass.rs`), 20 entries per rate × 7 = 140: per rate stage 10 on the player (a bypass-then-return schedule; a reversal schedule), stage 11 on both routes × both streams × the two schedules (8), stage 12 (speaker) and stage 13 (earphone) on both streams × two schedules (4 + 4), and two two-stage entries (11 + 12 at one position on the speaker route; 11 + 13 on the earphone route). Each entry: the configuration, route, stream, rate, the schedule, the produced count, the changes, and the output's digest; at bless time the endpoint identities (the all-on run before the landing, the configured-off twin over its length after the blend, the all-on run after the return) and the block-size-1 oracle are asserted on every entry. Six mutants in `BypassMutation` (`__test-hooks`): `weight_inverted` (x + w·(y − x)), `fade_step` (the switch at once: also the click reader's step reference), `fade_linear`, `stage_cold` (the bypassed stage's filter not run: the return differs), `count_ungated` (stage 11's clip count while w = 1), `phase_frozen_off_route` (a stage not on the active route holds its phase).

**Rationale**: the precedent of specs 015 – 020; the step mutant doubles as the reader's reference as in spec 019.

**Alternatives**: fewer rates (the goldens cover seven); a per-stage golden (one file is the precedent).

## R-10 The configured-off twin's length and the live engine's tail

**Decision**: the endpoint identity after a bypass compares the live engine with the configured-off twin over the twin's produced length; the live engine's extra drain samples (its drain is the all-on drain: stage 10's N₁₀ = 121 device samples and the chain's N_OUT when the twin has no chain stage running) are the warm stage's tail on silence, whose peak is printed per rate and asserted below −80 dBFS (the drain's design criterion is −100 dB on the state's envelope, spec 009 R-08, so the margin is large). The live engine's total length is asserted equal to the all-on no-schedule length (D4).

**Rationale**: coach 2; `derive_figures` (`paths.rs:1129`) is untouched.

**Alternatives**: allowing on-from-off with the drain sized for every stage (rejected, D4).

## R-11 The `reconfigure(settings())` trap and the guide's contract

**Decision**: `us2_bypass` has a test that bypasses stage 12 live, calls `reconfigure(settings())` and shows the on-change refused with `StageNotConfigured`, then shows the contract's path (`reconfigure` with the stage on, the bypass scheduled at position 0 of the next playback) producing the live bypass's output. The guide's section states the contract in three sentences (construct the A/B engine with every playback stage on; presets and restored sessions apply stage switches as scheduled changes at position 0, never as construction settings; `settings()` of a live engine is a read-out, not a construction recipe for a live engine), the refusal's text names the fix, and `Settings::new`'s default is noted as the all-on construction.

**Rationale**: coach 5, product M2.

**Alternatives**: exempting the four switches from `settings()` (two sources of truth; rejected with D2).

## Figures and restatements

*(Filled at implementation: the measured stage-10 settle per rate, the click table, the level table, the A/B.)*
