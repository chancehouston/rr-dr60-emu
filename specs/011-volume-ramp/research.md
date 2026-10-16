# Research: Volume Ramp (feature 011)

Every figure below is printed by `tools/volume-ramp/ramp_model.py` (`uv run tools/volume-ramp/ramp_model.py`; `--check` asserts FR-004's floor, the drag criterion, the absence of reversals and the drain ratio), unless a section says otherwise. The script is the reference the core and harness tests pin (005's lesson: a pin is to a figure the script prints).

## R-01 The mechanism: a per-sample knob inside `OutputChain`

**Decision.** The transition lives in `OutputChain` (`crates/rr_dr60/src/stages/output.rs`), the one type both `PlaybackPath` instances share, so the one-pass and the player ramp identically (FR-007). `set_controls(route, volume)` stays the once-per-block entry; it no longer writes the scalars directly when the position changes, it **starts a transition**:

- `set_controls` with a route change: the sections reset as today (`reset()`), which also marks the chain **fresh** (R-04); the new position is then taken at once.
- `set_controls` on a fresh chain, or with the chain inactive or stage 11 off (`!active || !run_amp`): the position is taken at once (the 0.10 path: both scalars from `k_spk(p)`, `k_ear(p)`), no transition.
- `set_controls` with the same target as the current one (an `f32` compare): nothing (a transition in progress continues).
- otherwise: a transition starts: `from = knob` (the knob position at the last processed sample, R-03), `to = p`, `k = 0`; the scalars are left as they are until the first sample of the block.

In `inner()`, after the `active` early return and before the route match, one predictable branch: `if self.ramp_k < self.ramp_n { self.ramp_step() }`. `ramp_step` advances `k`, computes the knob position along the shape, the **active route's** gain by the same function the block scalar uses (`k_spk(p) * gain_scale` or `k_ear(p)`), holds it within the two ends (R-03) and writes it to that route's scalar; at `k == N` it writes the target's block scalars computed exactly as 0.10 does and sets `knob = to`. The other route's scalar is set from the target at that point too (a route change completes the transition anyway, R-04). Nothing else in the per-sample path changes; the multiply, the knee and every filter read the scalar as before.

**Rationale.** The scalar was already read per sample (`self.k_spk * self.amp_hpf.process(u)`); updating it per sample during a transition is the smallest change that keeps every other line of the chain, and computing only the active route's gain halves the transition's cost (hardware review D6). Putting the knob in `PlaybackPath` or `Pipeline` would split the control's state from the scalars it drives.

**Alternatives rejected.** A per-block gain ramp precomputed into a buffer (an allocation or a fixed buffer sized for the largest block; the transition would then depend on the block size); a one-pole smoother on the scalar (no fixed length, an exponential tail, overshoot-free only in exact arithmetic); interpolating the gain or the gain in dB (spec Clarifications: undefined at 0, not the wiper's physics).

## R-02 The shape and the figures

**Decision.** The smoothstep `s(u) = u·u·(3 − 2u)`, `u = k · (1/N)` (the reciprocal computed once per rate; a multiply by a positive constant is monotone under round-to-nearest, so `u` is non-decreasing in `k`), `knob = from + (to − from)·s(u)`. Plain IEEE `f64` multiplies, adds and one divide (the loaded law), no platform math function; Rust does not contract to fused multiply-adds, so every platform computes the same bits (FR-009; 001 R-04).

**Figures** (the script, 1 kHz cosine peaking at the change's first sample, a Hann window of round(0.5 × rate) samples centred on the transition's midpoint, 20 – 2750 Hz excluding ±250 Hz of 1 kHz):

| Host rate | Speaker up / down | Earphone up / down | Speaker, linear position |
|---|---|---|---|
| 6 kHz | 34.46 / 34.46 dB | 41.23 / 41.23 dB | 19.79 dB |
| 8 kHz | 34.20 / 34.19 | 40.97 / 40.96 | 19.27 |
| 16 kHz | 33.94 / 33.94 | 40.71 / 40.71 | 18.79 |
| 44.1 kHz | 33.87 / 33.86 | 40.64 / 40.64 | 18.66 |
| 48 kHz | 33.86 / 33.86 | 40.64 / 40.64 | 18.66 |
| 88.2 kHz | 33.86 / 33.86 | 40.63 / 40.63 | 18.65 |
| 96 kHz | 33.86 / 33.86 | 40.63 / 40.63 | 18.65 |

FR-004's floor (30 dB) holds with 3.9 dB of margin on the speaker law; a linear position movement fails it by 10 – 11 dB, which makes the "linear shape" mutant (R-07) a sharp test. The drag of US1 AS8 (ten updates G = 16 · round(rate ÷ 960) samples apart, 16.0 – 17.0 ms on 16-sample block boundaries, positions 0.1 + 0.09 j; the golden's `ramp_drag`) reads **36.4 – 38.3 dB** (speaker) and **34.9 – 36.8 dB** (earphone) below the stepped drag at the seven rates, monotonic on both laws. A retarget half-way (SC-003) moves the gain by at most 0.0082 per sample at the switch (8 kHz), far below either transition's own largest increment (0.017 – 0.27, speaker law).

**Rationale.** The smoothstep is the lowest-order polynomial with zero slope at both ends (the spec's "starts and ends at rest"), costs four operations, and its residual corner (a jump in the second derivative) keeps the splatter more than 30 dB down. The raised cosine (36 / 43 dB) needs a cosine; the smootherstep (about 45 dB on a flick) makes a 60 Hz drag worse by about 13 dB in the 40 – 250 Hz band (hardware review D2) for no audible gain.

**How the Rust is pinned.** The harness's FR-004 rows assert ≥ 30 dB (the spec's floor) and the Rust figure within **±0.25 dB** of this table (the spec allows ±1 dB; a const table in `volume_ramp_checks.rs` copied from the script's print, with the script's command in the comment). The hardware reviewer ran the real chain at the seven rates on both routes (plan review): the Rust reduction matches the script **within 0.03 dB** (speaker 33.87 – 34.48, earphone 40.63 – 41.24). The residual 1 kHz phase at the tap is the coupling corners' lead, 1.65 – 1.82° on the earphone route and 9.9 – 10.9° on the speaker route (measured), and the reduction moves ≤ 0.05 dB over 0 – 45° of phase (0.5 dB at 90°). The records quote the harness's printed figures, not the script's (010's rule).

## R-03 Floating point: in range, monotonic by construction, exact at the end

**Decision.** Each transition sample's gain `g = law(knob)` is held within the two ends **and against the previous sample** (a running hold): upward `g = bound(g, g_prev, g_to)`, downward `g = bound(g, g_to, g_prev)`, where `g_prev` is the active route's scalar (which is the previous sample's gain) and `bound` is 010's two compares and selects (no `f64::clamp`, whose assertion would be a panic site); sample N writes `g_to` exactly; `knob` is set to `to` at N, so a later transition starts from the target's exact position. The running hold costs nothing over a range hold (the previous value is already in the scalar) and makes FR-003's monotonicity hold by construction.

**Evidence** (the script). The unbounded formula in `f64` on a grid of 21 × 21 positions (0, 0.05, … 1) at the seven rates on both laws (5880 transitions): no reversal, and 371 transitions have one sample a few ULP outside the ends' range (the knob rounding past `to` just before N). But a **retarget to a position within rounding of the knob** reverses: with the knob an `f64` within 1e-10, 1e-11, 1e-12 of an `f32` target the range hold alone reverses by one ULP in 24, 153 and 249 of 600 cases at N = 1920 (the hardware reviewer found it, plan review Q2; the script reproduces it); with the running hold, none. A core unit test retargets to `to = f32(knob) ± 1e-12 · knob` and asserts exact monotonicity.

**Exact zeros.** A transition to 0 ends with `k_spk(0) = 0.0` and `k_ear(0) = 0.0` exactly at sample N (0.10's scalars); the `+ 0.0` after the multiply keeps −0.0 out as today. Before N the gain is positive.

## R-04 Start and end rules

**Decision.** A `fresh: bool` on `OutputChain`, set by `new()` and `reset()`, cleared by the first sample `inner()` processes past the `active` check (one store per sample; a zero-length call never reaches it, so it does not consume "the first block", hardware review D9). `set_controls` on a fresh chain takes the position at once. Every "at once" path goes through one private `take_at_once(p)`, which writes both scalars from the 0.10 functions, sets `knob = from = to = p` and `target`, recomputes `g_to_*`, and sets `ramp_k = ramp_n` (no transition); it is called by the fresh, inactive, `!run_amp` and `volume_step` branches of `set_controls` and by `__mutate` (which today re-applies the reference position: under 0.11 a same-target call on a non-fresh chain would otherwise skip recomputing the scalars with a new `gain_scale`; plan review Q7). `reset()` sets `fresh` and `ramp_k = ramp_n`. N is computed as `rate · VOLUME_TRANSITION_MS / 1000` in integers, with a const assertion that every supported rate is a multiple of 50 (no `f64` product). Consequences, each a spec rule:

| Event | What resets the chain or skips the transition | Spec |
|---|---|---|
| construction (`Pipeline::new`, `reconfigure`, `PlaybackPath::new`) | `new()` → fresh | FR-005 |
| `Pipeline::reset` | `one_pass.reset()` and `player.reset()` → `chain.reset()` → fresh | FR-005 |
| start of a playback | `start_playback` calls `player.reset()` then `set_controls` → fresh, at once | FR-005, A-037 |
| route change (same gap as a position change or not) | `set_controls` resets the sections (009 FR-009) → fresh → the position at once | FR-005, US2 AS3 |
| stage 11 bypassed, or the one-pass at a tap before the chain | `!active || !run_amp` → at once (the scalars have no effect) | US2 AS3 |
| the end of a playback | the next playback starts with `player.reset()` | FR-005 |
| the drain | the player keeps calling `try_next_host` through the drain; the transition continues and completes inside it: the output chain's share of the drain, `N_OUT`, alone is 2.8786 – 2.8833 transition lengths (the script); a const assertion `100 · N_OUT[i] > 287 · N[i]` per rate, as FR-005 states | FR-005 |
| a change during the drain | a transition starts as usual; if the playback finishes first it is cut; the next playback starts at once | Edge Cases |

The one-pass after construction is fresh until its first processed sample, so a host that sets its volume and then processes gets 0.10's output exactly; every golden generator does this (R-06).

## R-05 The oracles and the measurement

**(a) The splice identity** (US1 AS1 (a), SC-001). In the stage-11-only configuration (`configs::stage11_only_{speaker,earphone}`: `bypass_all` with `volume_amp_enabled = true`, `tap = AfterVolumeAmp`) the tap is `k · hpf(x)` on the earphone route and `knee(k · hpf₁₅₉(hpf₃₂(x)))` on the speaker route: the filters precede the gain and the knee is memoryless, so the output before the change equals a run held at the old position bit for bit, and from transition sample N on equals a run held at the new position bit for bit; and the first sample that differs from the old-position run is transition sample 1, the last that differs from the new-position run is sample N − 1 (this kills the −1 length mutant, which reaches the target one sample early; the difference at N − 1 is about 12 f32 ULP at 96 kHz on the earphone route; plan review P2, Q3). The verified figure (plan review): the identity holds exactly on both routes at the seven rates. The **instantaneous step** at that tap is exactly the splice of the two runs at the change sample, so FR-004 needs no hook (coach C3).

**(b) The gain sequence** (US1 AS1 (b)). A core unit test in `output.rs` drives `OutputChain` one sample at a time, reads `scalars()` after each, and asserts: monotonic, within the two ends, sample N equal to the 0.10 scalar, every sample equal to `law(from + (to − from)·s(k·(1/N)))` held within the ends (the formula recomputed in the test), at the seven rates on both routes, for the grid of R-03's positions at 48 kHz and a few pairs at the other rates.

**(c) The ratio reader** (US1 AS1 (c)). The output divided by a reference run at position 1, over the samples where |reference| ≥ 0.25 of the tone's peak, monotonic within 4 f32 ULP; at 6 kHz a 1 kHz tone gives 6 samples per cycle, of which the threshold keeps 4.

**The click energy** (FR-004). The stimulus is a 1 kHz cosine of the route's level (earphone 0 dBFS; speaker −30 dBFS, below the knee's 0.85 at position 1: 11.83 × 0.0316 = 0.37) generated so that its peak falls on the change's output sample: the input cosine's phase is set from the path's reported latency (`latency_samples()`), ignoring the coupling corners' phase at 1 kHz (1.7 – 10.9°, which moves the reduction by at most 0.05 dB; R-02). The tone runs 0.5 s before the window starts. The change is made at a block boundary that is a multiple of every partition used (4096 · m). The step's energy is computed on the splice (a); the transition's on the run with the change; the window and band as R-02. `analysis::click_energy(y, rate, centre)` implements it once for every caller.

**The ceiling** (US1 AS4, plan review P3): `stage11_only_speaker` with a full-scale 1 kHz tone, the up, down and retarget changes at the seven rates: max |y| ≤ the knee's ceiling (009's knee bound), and the splice identity before and after the transition.

**Coordinates** (FR-007, plan review P12). In the one-pass the change is made at a block boundary at skip + c, with skip = 180 · rate ÷ 6000 host samples (an integer at all seven rates, 180 – 2880), so the one-pass and record-then-play see the change at the same output sample.

**At the route output** (US1 AS7, reported): spec 009's output-chain-only configurations, the same stimuli, the transition against the test hooks' step (R-07); the speaker template and the earphone corners shape both runs alike.

## R-06 Existing checks that change the volume mid-stream

Every other check sets its route and volume before the first block, so a fresh engine takes it at once and the output is 0.10's (R-04). Searched: every `set_volume` / `rr_dr60_set_volume` call in `crates/` (2026-10-16).

| Check | Today | 0.11 |
|---|---|---|
| `output_checks::chain_volume_change_timing` (009 FR-009, SC-005; 16 change points, 64-sample blocks, `AfterVolumeAmp`) | before equal to the 1.0 run, after equal to the 0.5 run from the next block | **restated**: before the change bit-identical to the 1.0 run; from transition sample N bit-identical to the 0.5 run (R-05 (a)); the stimulus lengthened 4096 → **8192** samples at every rate and at least 64 samples compared after N asserted (with 4096 the slice from N is empty for late change points: k = 63 at 6 kHz, 6 of 16 at 48 kHz, 8 of 16 at 96 kHz; plan review Q1) |
| `output_checks::volume_zero` (009 AS4, SC-004: 1.0 then 0 between calls): the decay row (< −120 dBFS within 100 ms) and the exact-zeros row (no sample other than +0.0 at the after-stage-11 tap) | measured from the change | **both restated** from transition sample N (the exact-zeros row goes red otherwise; plan review P6) |
| core `route_change_resets_sections_and_volume_change_does_not` | `scalars()` change at `set_controls` | **restated**: the scalars change from the first processed sample and reach the target at N; the sections untouched |
| `output_edge_cases::volume_change_lands_at_the_next_block_without_overshoot` (earphone block-gain, 1.0 → 0.5 at block 32 of 64-sample blocks) | every block's energy ratio equals the law's; its `assert_ne!` at the next block | **restated**: the blocks from ⌈N / 64⌉ after the change, on an 8192-sample stimulus (at 96 kHz only 2 blocks would remain of 4096); the `assert_ne!` still holds (s(1/N)·Δp > 0); the comment "the gain is block-constant by construction" rewritten |
| `pipeline.rs::controls_land_at_the_next_call` (6 kHz, a 64-sample second call) | the next call differs | **unchanged** (its `assert_ne!` holds: s(1/120)·Δp ≈ 1e-4) |
| `output_clamp_checks::change_timing`'s second row (option-off beyond the bound ≥ 1) and `alloc_free::output_clamp_cases`' second play loop | as named | **unchanged** |
| route-only mid-stream changes in `split_checks`, `split_edge_cases`, `determinism`, `output_clamp_edge_cases` | route changes | **unchanged** (a route change takes the position at once) |
| `output_clamp_checks::change_timing` (010; alternating 0.5 / 1.0) | the identity `on == clamp(off)` | **unchanged** (both twins ramp identically) |
| `alloc_free` (a change every 512-sample play block) | no allocation | **unchanged** (it now exercises transitions) |
| `ffi_parity` volume round-trips and route cases | C equals Rust | **unchanged**; a mid-playback change case is added (R-09) |
| `smoke.c` route cases (positions set before each `start_playback`) | pinned hashes | **unchanged** (each playback starts fresh) |
| `golden.rs` generators (all five files with a volume), `smoke_reference.rs`, `determinism`, `timing` | positions set before the first block | **unchanged** (the six SHA guards hold by rule; verified by running them in Phase 1) |

## R-07 Test hooks, mutants and a public constant

**Decision.**
- `__test_hooks::OutputMutation` gains `volume_step: bool` (false: as committed; true: `set_controls` takes every change at once, 0.10's step, for US1 AS7's reference and the cost comparison), `volume_shape_linear: bool` (the mutant: `s(u) = u`), and `volume_ramp_delta: i64` (added to N; the mutant ±1 fails the length checks). Through the existing `Pipeline::__with_output_mutation` and `__mutate`.
- Mutation tests (`mutation.rs`), **four configurations**: the step mutant fails FR-004 (0 dB reduction) and R-05 (a)'s first-differing-sample row; the linear mutant fails FR-004 on the speaker law (≈ 19 dB); the length mutant +1 fails the bit-identity from sample N, −1 fails the last-differing-sample row (N − 1). `__mutate` with a delta goes through `take_at_once` and recomputes `ramp_n` and `inv_n`, so no transition is left half-set.
- A public constant `output::VOLUME_TRANSITION_MS: u32 = 20` (A-045) and its C define `RR_DR60_VOLUME_TRANSITION_MS` (a literal const, so cbindgen emits it; 009's lesson), so an app can align its UI with the sound (US3) without hard-coding the figure; a unit test pins the define to the Rust constant, as `RR_DR60_VOLUME_REFERENCE` is pinned; FR-011 allows it (spec plan-review clarification); the manifest's controls note does not list it. Not a setting and not a function (FR-011 holds: no new setting, no new C function, the structure stays 112 bytes). `output::volume_transition_samples(host_rate_hz) -> Option<u32>` in Rust only (the harness's and the app's N).

**Rationale.** The step hook keeps 0.10's behavior reachable for comparison without a public setting (spec Clarifications Q3); the constant is documentation the compiler checks.

## R-08 Cost

**Decision.** Two measurements, codec bypassed, speaker route, 48 kHz, each side the fastest of nine alternating runs in a release run with the timing tests one at a time (spec 010's estimator and CI step):

1. **Worst case, asserted in `timing::volume_ramp_cost`** (FR-009 ≤ 15 %): 480-sample blocks, the position alternated between 0.1 and 1 before every block, so every sample is inside a transition (N = 960 > 480); against the `volume_step` hook at the same partition. The material is **noise at −20 dB** (`noise × 0.1`): on full-scale noise the cost depends on the position (63 ns per sample at 0.1 – 0.5, 68.5 ns at 1, the knee's upper branches; the hardware reviewer's measurement), and under the alternation the knob cycles between 0.4 and 0.7 (each block moves it half-way, s(0.5) = 0.5) while the step baseline spends half its samples at 1, which would inflate the baseline by about 4 %, the size of the effect; at −20 dB the cost is flat (63.0 – 63.4 ns at every position) and the VAS stays open. The test also prints the cost at constant positions 0.1, 0.55 and 1 with a spread check (≤ 2 %), the knob's 0.4 – 0.7 cycle, the full-scale figure (reported), the duty (100 %) and both routes. Runs on the x86-64 CI runner too (the CI timing step).
2. **Idle, an A/B of builds** (FR-009 ≤ 3 %): constant volume, 4096-sample blocks, `timing::output_cost` (009 SC-007) run solo, alternating the **0.10 build at `bd6d827`** (a worktree) and the branch build nine times each, the fastest of each quoted (`scripts/ab-timing.sh`, new). On the development machine in Phase 3, and **once on the CI runner before the merge** through a `workflow_dispatch` job (`.github/workflows/ab-timing.yml`, manual only), the figures quoted in the PR. `output_cost`'s own assertion (the chain ≤ 25 % over the bypassed chain, 18.9 % measured) guards the chain's total, **not** the 3 % idle budget.

The figure against 0.10 for the worst case is the product of the two ratios (the step hook already carries the idle code; plan review Q11), reported.

**Estimate** (hardware review D6): one divide (the law) and about eight other operations per transition sample, off the filters' recurrence, so the out-of-order core overlaps most of it: +2 – 5 ns on about 63 ns (3 – 8 %); idle under 1 ns.

## R-09 The golden file and the C parity

**Decision.** `golden-volume-ramp-v1.json`, the `GoldenFile` form with `route` and `volume` (the starting position) and one new optional field `changes` (the list of `[sample, position]` the scenario applies; `#[serde(default, skip_serializing_if = "Option::is_none")]`, so the six earlier files read and write the same bytes). A change's sample is the input sample at the start of a call; the generator asserts that every block up to the last change's transition produced its full length, so it equals the output sample (true for the voiced stimulus with the VAS in drop mode at the seven rates today, the hardware reviewer's check; asserted so a stimulus change cannot silently break it). Entries: **5 scenarios × 4 configurations × 7 rates = 140**:

| Scenario | Changes (rounded down to a multiple of 16, the golden's block size; the exact formulas in [contracts/golden-format.md](contracts/golden-format.md)) |
|---|---|
| `up` | 0.1 → 1 at 0.5 s |
| `down` | 1 → 0.1 at 0.5 s |
| `to_zero` | 1 → 0 at 0.5 s |
| `retarget` | 0.1 → 1 at 0.5 s, → 0.4 half a transition later |
| `drag` | ten updates about 1/60 s apart (16 – 17 ms, block-aligned) from 0.1 to 1 |

Configurations: `stage11_only_speaker` (−30 dBFS 1 kHz tone), `stage11_only_earphone` (0 dBFS), `default_output` and `default_earphone` (the one-pass default engine, a voiced −20 dBFS stimulus). 1 s per entry, 16-sample blocks (so the drag's interval stays near 16.7 ms at 6 kHz, where a 64-sample block is 10.7 ms). Blessed once, last, after the post-implementation hardware review (`RR_DR60_BLESS=volume_ramp`); at bless time the generator asserts the splice identity (R-05 (a)) for the stage-11-only entries. Added wherever the six other golden tests run (`ci.yml`, `scripts/ios-device-golden.sh`).

**C parity.** `ffi_parity` gains a case at the seven rates: a recorded tone played back through C and Rust with the position changed mid-playback (0.1 → 1 → 0.4 at fixed blocks), bit-identical; `smoke.c` gains one mid-playback change on the speaker route with a pinned hash (`SMOKE_SPEAKER_RAMP_HASH`).

## R-10 Records

- **A-045** (new row, `Volume / power amp`): the statement and the split confidence of the spec's Assumptions, the verification protocol (the flick capture, with the band energy taken relative to a steady segment of the same capture, since a real unit has no step reference; plan review Q10), the not-modeled list; status "assumed (engine convention; spec 011, 2026-10-16)", like A-043.
- **A-044**: one sentence: "a position change moves the knob over 20 ms (A-045; spec 011)"; status line "amended by spec 011".
- **Manifest stage 11**: `assumptions: ["A-031", "A-043", "A-044", "A-045"]`; `model_version: "0.11.0"`; the coverage note rewritten within 120 characters to name the wiper noise and the knob's speed ("the 159 Hz corner's shift with the knob; the clip level's spread; wiper contact noise; the knob's speed; the beeps", 116 characters; the denylist and the wording review run **before** the Phase 1 re-bless, so it happens once); the description and controls unchanged. One re-bless of the canonical text and the README table in Phase 1, with the version.
- **Signal-chain row 11**: the `**Modeled by` marker names the transition ("spec 009; the volume transition spec 011").
- **README**: the volume paragraph (US3 AS1's list); **CHANGELOG** 0.11.0 (the transition, A-045, A-044 amended, the stage-11 model version, the re-bless, the restated checks, the constant); **spec 009**: FR-009's note and T043 N2 marked closed by spec 011; the `hardware_evidence.yml` template gains the knob-flick capture; rustdoc of `set_volume` / `volume` and the C header's `rr_dr60_set_volume`; CLAUDE.md's status line and commands.
- `scripts/check-traceability.sh`'s tolerance scan gains `volume_ramp_checks.rs` (its literal tolerances: 30 dB, ±0.25 dB, 4 ULP).
- Wording: no "accurate", no claim that the transition protects hearing or equipment; "click" and "transition" are the words; "ramp" only in code identifiers and the feature name.

## R-11 Risks

- **The idle cost on x86** (010's lesson): the branch is predictable and the store is to a line the chain already writes; if `output_cost` moves on the runner, the `fresh` store can move to `try_next_host`'s caller or be replaced by a sample counter.
- **The 30 dB floor's margin** on the speaker route is 3.9 dB; the Rust figure is pinned ±0.25 dB to the script (the spec allows ±1 dB), so a drift of the measurement (window placement, phase) shows as a pin failure first.
- **Golden fixtures that change the volume mid-stream** in a way the search missed would fail their SHA guard in Phase 1 (the first gate run), before any core change lands.
