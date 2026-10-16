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

FR-004's floor (30 dB) holds with 3.9 dB of margin on the speaker law; a linear position movement fails it by 10 – 11 dB, which makes the "linear shape" mutant (R-07) a sharp test. The drag of US1 AS8 (ten updates 1/60 s apart, positions 0.1 + 0.09 j) reads **37.8 – 38.5 dB** (speaker) and **36.5 – 37.0 dB** (earphone) below the stepped drag at the seven rates, monotonic on both laws.

**Rationale.** The smoothstep is the lowest-order polynomial with zero slope at both ends (the spec's "starts and ends at rest"), costs four operations, and its residual corner (a jump in the second derivative) keeps the splatter more than 30 dB down. The raised cosine (36 / 43 dB) needs a cosine; the smootherstep (about 45 dB on a flick) makes a 60 Hz drag worse by about 13 dB in the 40 – 250 Hz band (hardware review D2) for no audible gain.

**How the Rust is pinned.** The harness's FR-004 rows assert ≥ 30 dB and the Rust figure within ±1 dB of this table (a const table in `volume_ramp_checks.rs` copied from the script's print, with the script's command in the comment). The Rust signal differs from the script's model by the resampler pair's tone (common to both runs) and by `u = k·(1/N)` instead of `k/N` (sub-ULP); the ±1 dB covers both and the phase of the 1 kHz tone at the chain after the coupling corners (≤ 0.5 dB, hardware review D3).

## R-03 Floating point: in range, monotonic, exact at the end

**Decision.** Each transition sample's gain is held within `[min(g_from, g_to), max(g_from, g_to)]` (`g_from` the active route's scalar when the transition started, `g_to` the target's block scalar), by two compares and selects written as in 010's `bound` (no `f64::clamp`, whose assertion would be a panic site); sample N writes `g_to` exactly; `knob` is set to `to` at N, so a later transition starts from the target's exact position.

**Evidence.** The script evaluates the unbounded formula in `f64` on a grid of 21 × 21 positions (0, 0.05, … 1) at the seven rates on both laws (5880 transitions): **no reversal anywhere**, and **371 transitions have one sample** a few ULP outside the ends' range (the knob position rounding past `to` just before N). So the range hold alone keeps the sequence exactly monotonic and in range; a running hold against the previous sample is not needed. The core unit test checks this on the Rust sequence directly (R-05 oracle (b)).

**Exact zeros.** A transition to 0 ends with `k_spk(0) = 0.0` and `k_ear(0) = 0.0` exactly at sample N (0.10's scalars); the `+ 0.0` after the multiply keeps −0.0 out as today. Before N the gain is positive (the law at a positive position).

## R-04 Start and end rules

**Decision.** A `fresh: bool` on `OutputChain`, set by `new()` and `reset()`, cleared by the first sample `inner()` processes past the `active` check (one store per sample; a zero-length call never reaches it, so it does not consume "the first block", hardware review D9). `set_controls` on a fresh chain takes the position at once. Consequences, each a spec rule:

| Event | What resets the chain or skips the transition | Spec |
|---|---|---|
| construction (`Pipeline::new`, `reconfigure`, `PlaybackPath::new`) | `new()` → fresh | FR-005 |
| `Pipeline::reset` | `one_pass.reset()` and `player.reset()` → `chain.reset()` → fresh | FR-005 |
| start of a playback | `start_playback` calls `player.reset()` then `set_controls` → fresh, at once | FR-005, A-037 |
| route change (same gap as a position change or not) | `set_controls` resets the sections (009 FR-009) → fresh → the position at once | FR-005, US2 AS3 |
| stage 11 bypassed, or the one-pass at a tap before the chain | `!active || !run_amp` → at once (the scalars have no effect) | US2 AS3 |
| the end of a playback | the next playback starts with `player.reset()` | FR-005 |
| the drain | the player keeps calling `try_next_host` through the drain; the transition continues and completes inside it (`N_OUT ÷ N` = 2.8786 – 2.8833, the script; a const assertion `N_OUT[i] > 2·N[i]` per rate guards it with margin) | FR-005 |

The one-pass after construction is fresh until its first processed sample, so a host that sets its volume and then processes gets 0.10's output exactly; every golden generator does this (R-06).

## R-05 The oracles and the measurement

**(a) The splice identity** (US1 AS1 (a), SC-001). In the stage-11-only configuration (`configs::stage11_only_{speaker,earphone}`: `bypass_all` with `volume_amp_enabled = true`, `tap = AfterVolumeAmp`) the tap is `k · hpf(x)` on the earphone route and `knee(k · hpf₁₅₉(hpf₃₂(x)))` on the speaker route: the filters precede the gain and the knee is memoryless, so the output before the change equals a run held at the old position bit for bit, and from transition sample N on equals a run held at the new position bit for bit. The **instantaneous step** at that tap is exactly the splice of the two runs at the change sample, so FR-004 needs no hook (coach C3).

**(b) The gain sequence** (US1 AS1 (b)). A core unit test in `output.rs` drives `OutputChain` one sample at a time, reads `scalars()` after each, and asserts: monotonic, within the two ends, sample N equal to the 0.10 scalar, every sample equal to `law(from + (to − from)·s(k·(1/N)))` held within the ends (the formula recomputed in the test), at the seven rates on both routes, for the grid of R-03's positions at 48 kHz and a few pairs at the other rates.

**(c) The ratio reader** (US1 AS1 (c)). The output divided by a reference run at position 1, over the samples where |reference| ≥ 0.25 of the tone's peak, monotonic within 4 f32 ULP; at 6 kHz a 1 kHz tone gives 6 samples per cycle, of which the threshold keeps 4.

**The click energy** (FR-004). The stimulus is a 1 kHz cosine of the route's level (earphone 0 dBFS; speaker −30 dBFS, below the knee's 0.85 at position 1: 11.83 × 0.0316 = 0.37) generated so that its peak falls on the change's output sample: the input cosine's phase is set from the path's reported latency (`latency_samples()`), ignoring the coupling corners' phase at 1 kHz (≤ 0.5 dB, inside the ±1 dB pin). The tone runs 0.5 s before the window starts. The change is made at a block boundary that is a multiple of every partition used (4096 · m). The step's energy is computed on the splice (a); the transition's on the run with the change; the window and band as R-02. `analysis::click_energy(y, rate, centre)` implements it once for every caller.

**At the route output** (US1 AS7, reported): spec 009's output-chain-only configurations, the same stimuli, the transition against the test hooks' step (R-07); the speaker template and the earphone corners shape both runs alike.

## R-06 Existing checks that change the volume mid-stream

Every other check sets its route and volume before the first block, so a fresh engine takes it at once and the output is 0.10's (R-04). Searched: every `set_volume` / `rr_dr60_set_volume` call in `crates/` (2026-10-16).

| Check | Today | 0.11 |
|---|---|---|
| `output_checks::chain_volume_change_timing` (009 FR-009, SC-005; 16 change points, 64-sample blocks, `AfterVolumeAmp`) | before equal to the 1.0 run, after equal to the 0.5 run from the next block | **restated**: before the change bit-identical to the 1.0 run; from transition sample N bit-identical to the 0.5 run (R-05 (a)); the transition's samples monotonic between the two |
| `output_checks` volume-0 decay (009 AS4, SC-004: 1.0 then 0 between calls; < −120 dBFS within 100 ms) | measured from the change | **restated** from transition sample N |
| core `route_change_resets_sections_and_volume_change_does_not` | `scalars()` change at `set_controls` | **restated**: the scalars change from the first processed sample and reach the target at N; the sections untouched |
| `output_edge_cases` earphone block-gain check (1.0 → 0.5 at block 32 of 64-sample blocks) | every block's energy ratio equals the law's | **restated**: the blocks from ⌈N / 64⌉ after the change |
| `output_clamp_checks::change_timing` (010; alternating 0.5 / 1.0) | the identity `on == clamp(off)` | **unchanged** (both twins ramp identically) |
| `alloc_free` (a change every 512-sample play block) | no allocation | **unchanged** (it now exercises transitions) |
| `ffi_parity` volume round-trips and route cases | C equals Rust | **unchanged**; a mid-playback change case is added (R-09) |
| `smoke.c` route cases (positions set before each `start_playback`) | pinned hashes | **unchanged** (each playback starts fresh) |
| `golden.rs` generators (all five files with a volume), `smoke_reference.rs`, `determinism`, `timing` | positions set before the first block | **unchanged** (the six SHA guards hold by rule; verified by running them in Phase 1) |

## R-07 Test hooks, mutants and a public constant

**Decision.**
- `__test_hooks::OutputMutation` gains `volume_step: bool` (false: as committed; true: `set_controls` takes every change at once, 0.10's step, for US1 AS7's reference and the cost comparison), `volume_shape_linear: bool` (the mutant: `s(u) = u`), and `volume_ramp_delta: i64` (added to N; the mutant ±1 fails the length checks). Through the existing `Pipeline::__with_output_mutation` and `__mutate`.
- Mutation tests (`mutation.rs`): the step mutant fails FR-004 (0 dB reduction), the linear mutant fails FR-004 on the speaker law (≈ 19 dB), the length mutants fail SC-001's bit-identity from sample N.
- A public constant `output::VOLUME_TRANSITION_MS: u32 = 20` (A-045) and its C define `RR_DR60_VOLUME_TRANSITION_MS` (a literal const, so cbindgen emits it; 009's lesson), so an app can align its UI with the sound (US3) without hard-coding the figure. Not a setting and not a function (FR-011 holds: no new setting, no new C function, the structure stays 112 bytes). `output::volume_transition_samples(host_rate_hz) -> Option<u32>` in Rust only (the harness's and the app's N).

**Rationale.** The step hook keeps 0.10's behavior reachable for comparison without a public setting (spec Clarifications Q3); the constant is documentation the compiler checks.

## R-08 Cost

**Decision.** Two measurements, each the fastest of nine alternating runs in a release run with the timing tests one at a time (spec 010's estimator and CI step), codec bypassed, speaker route, 48 kHz:

1. **Worst case, asserted in `timing::volume_ramp_cost`** (FR-009 ≤ 15 %): 480-sample blocks, the position alternated between 0.1 and 1 before every block, so every sample is inside a transition (N = 960 > 480); against the `volume_step` hook at the same partition (0.10's arithmetic, the branch never taken). The duty (100 %) and both routes reported. Runs on the x86-64 CI runner too.
2. **Idle, reported** (FR-009 ≤ 3 %): constant volume, 4096-sample blocks; the only new per-sample work is the branch on `ramp_k < ramp_n` and the `fresh` store, which no in-binary comparison can isolate (the hook has the same branch). Measured as a **cross-commit A/B** on the development machine: `timing::output_cost` (009 SC-007) run solo, alternating the `main` build and the branch build nine times each (`scripts/ab-timing.sh`, new, a worktree of `main`), the fastest of each quoted; the existing `output_cost` assertion keeps guarding the chain's total on CI.

**Estimate** (hardware review D6): one divide (the law) and about eight other operations per transition sample, off the filters' recurrence, so the out-of-order core overlaps most of it: +2 – 5 ns on 57 – 68 ns (3 – 8 %); idle under 1 ns.

## R-09 The golden file and the C parity

**Decision.** `golden-volume-ramp-v1.json`, the `GoldenFile` form with `route` and `volume` (the starting position) and one new optional field `changes` (the list of `[output sample, position]` the scenario applies, absent in earlier files). Entries: **5 scenarios × 4 configurations × 7 rates = 140**:

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

- **A-045** (new row, `Volume / power amp`): the statement and the split confidence of the spec's Assumptions, the verification protocol (the flick capture), the not-modeled list; status "assumed (spec 011, 2026-10-16)".
- **A-044**: one sentence: "a position change moves the knob over 20 ms (A-045; spec 011)"; status line "amended by spec 011".
- **Manifest stage 11**: `assumptions: ["A-031", "A-043", "A-044", "A-045"]`; `model_version: "0.11.0"`; the coverage note rewritten within 120 characters to name the wiper noise and the knob's speed (e.g. "the 159 Hz corner's shift; the clip level's spread with battery and transistor; wiper noise and knob speed; the beeps", 117 characters; the denylist checked at the edit); the description and controls unchanged. One re-bless of the canonical text and the README table in Phase 1, with the version.
- **Signal-chain row 11**: the `**Modeled by` marker names the transition ("spec 009; the volume transition spec 011").
- **README**: the volume paragraph (US3 AS1's list); **CHANGELOG** 0.11.0 (the transition, A-045, A-044 amended, the stage-11 model version, the re-bless, the restated checks, the constant); **spec 009**: FR-009's note and T043 N2 marked closed by spec 011; the `hardware_evidence.yml` template gains the knob-flick capture; rustdoc of `set_volume` / `volume` and the C header's `rr_dr60_set_volume`; CLAUDE.md's status line and commands.
- Wording: no "accurate", no claim that the transition protects hearing or equipment; "click" and "transition" are the words; "ramp" only in code identifiers and the feature name.

## R-11 Risks

- **The idle cost on x86** (010's lesson): the branch is predictable and the store is to a line the chain already writes; if `output_cost` moves on the runner, the `fresh` store can move to `try_next_host`'s caller or be replaced by a sample counter.
- **The 30 dB floor's margin** on the speaker route is 3.9 dB; the Rust figure is pinned ±1 dB to the script, so a drift of the measurement (window placement, phase) shows as a pin failure first.
- **Golden fixtures that change the volume mid-stream** in a way the search missed would fail their SHA guard in Phase 1 (the first gate run), before any core change lands.
