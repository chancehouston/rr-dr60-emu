# Research: Microphone Capsule and Amplifier (stages 1–2)

**Spec**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md) | **Date**: 2026-10-16

All figures marked *plan-time* come from scratch computations made while planning: the hardware reviewer's `scratchpad/calc/{hiss,vas}.py` and the planner's `scratchpad/mic/nodal.py`. The design script `tools/mic-chain/design_mic_chain.py` (Phase 1) recomputes and prints each of them, and the tests pin to its print, never to these notes (spec Assumptions).

## R-01: The circuit model and the input high-pass (spec FR-005, Clarifications D1)

**Decision.** Model S-002 sheet 3.1's microphone amplifier as a small-signal **hybrid-π nodal network**:

- **The capsule:** a current source into node M, with R2 (1 kΩ) plus Q11's emitter resistance (≈ 65 Ω) to AC ground.
- **The input path:** C1 (0.15 µF) from M to Q1's base B; R3 (100 kΩ) from the output O back to B.
- **The differential pair:** Q1 (B / tail E / O) and Q3 (base at the codec's AC-grounded reference, collector C3); R5 (10 kΩ) from E.
- **The second stage:** R6 (100 kΩ) from C3 to the rail; Q2 (PNP) with its base at C3, its emitter at the rail and its collector at O.
- **The load:** the output coupling's series loop (C4 in series with C5, then 23 kΩ) at O.

Operating points come from the schematic's DC voltages:
- tail (1.1 V) / 10 kΩ = 110 µA;
- Q3's collector (2.6 V against the 3.2 V rail) through R6 gives ≈ 6 µA;
- so Q1 and Q2 carry ≈ 104 µA.

The design script solves the network at each frequency and prints three figures:
- the input corner (the −3 dB point below the plateau);
- the response at 300 Hz and 2550 Hz relative to 1 kHz;
- the transimpedance plateau.

It does this for the nominal parts and over a grid of C1 ±10 % and hFE 150–400 (the 2SD1819A ranks, `sources.md` › To find). The script also states its model in prose, as `limiter_model.py` does.

**Plan-time result** (`nodal.py`):

| hFE | C1 | Corner | 300 Hz re 1 kHz | 2550 Hz re 1 kHz | Plateau |
|---|---|---|---|---|---|
| 150 | 0.150 µF | 775 Hz | −6.81 dB | +1.66 dB | 78.5 kΩ |
| 250 | 0.135 µF | 910 Hz | −7.47 dB | +2.10 dB | 82.7 kΩ |
| **250** | **0.150 µF** | **820 Hz** | **−7.04 dB** | **+1.80 dB** | **82.7 kΩ** |
| 250 | 0.165 µF | 744 Hz | −6.63 dB | +1.56 dB | 82.7 kΩ |
| 400 | 0.150 µF | 847 Hz | −7.18 dB | +1.89 dB | 85.3 kΩ |

What this confirms:
- **The hardware reviewer's finding holds**: the reviewer's estimate was ≈ 0.85 kHz (0.65–1.05), and an independent solve gives 820 Hz nominal (744–910 over the grid).
- **The loop gain is low** (the plateau is about 83 % of R3), consistent with the reviewer's "loop gain about 6–30".

The model's default corner is the script's nominal figure. The registered range is the grid's span widened to the reviewer's 0.65–1.05 kHz, low confidence.

**Cross-check.** No circuit simulator is installed on the development machine (`ngspice`, `xyce`: not found). The script carries a netlist and a `--spice` option that runs ngspice when it is installed (`brew install ngspice`), with generic 2SD1819A/2SB1218A models. Without ngspice the option prints that it was skipped and why.

The plan does not depend on ngspice. Its independent checks are:
- the reviewer's estimate and the planner's solve, which agree;
- the script's DC self-check: an Ebers–Moll operating-point solve that must reproduce S-002's measured 1.7 / 1.1 / 2.6 V within the meter's 0.1 V rounding.

A disagreement beyond 10 % between the solve and ngspice, when run, holds the bless.

**Alternatives considered.** The reviewer's closed form 1/(2π·C1·(R2 + r_e + R3/(1+A_ol))): kept as a printed sanity check, not the model. A SPICE-only design: it would make the build tool depend on an external binary. An unmodeled corner: rejected by the user (Clarifications).

## R-02: Stage 2's realization, order and bound (FR-005, FR-006; C1, D2, D5)

**Decision.** At the host rate, in f64, in the device's order:

1. **The input high-pass** (≈ 820 Hz): a bilinear first-order section with the corner prewarped, as in spec 009's sections (`stages/first_order.rs`, state flushed below 1e-30).
2. **The gain normalization** `g_n`: one multiply that makes the whole stage-2 prototype 0 dB at 1 kHz (the high-pass alone reads about −2 dB there). It sits **before the clipper**, so the clip limits are in engine units at the clipper's input.
3. **The clipper** (R-03).
4. **The output coupling** (≈ 76 Hz): a bilinear first-order high-pass, prewarped.
5. **The roll-off** (≈ 13 kHz): a first-order low-pass, **magnitude-matched** at every host rate, so the response tracks the analog prototype within ±0.1 dB up to the lower of 20 kHz and 0.45 × fs.
   - The bilinear transform's warping near Nyquist would miss that tolerance at 44.1 and 48 kHz.
   - At 6, 8 and 16 kHz the pole lies above Nyquist; the matched section carries its in-band slope.
   - The script picks the realization per rate and checks it (`--check`).

**The bound after stage 2** (FR-006).
- A bilinear first-order high-pass equals 1 − LP, where LP is the bilinear low-pass with a non-negative impulse response summing to 1. So the coupling's output of any input lying in [−L₋, L₊] lies in [−(L₊ + L₋), L₊ + L₋].
- The roll-off's impulse response is non-negative with unity DC gain (the script asserts it), so it keeps that interval.
- The bound is therefore **L₊ + L₋** (≈ +19 dBFS), up to a few ULP of rounding. The script prints the realized sections' worst-case figure, and the test asserts the printed bound.

**The settling** after a clipped passage is the coupling's time constant, R·C with C4 in series with C5 and 23 kΩ: about **2.1 ms** to 1/e. The script prints the time to return within ±0.1 dB of the linear response (US2 AS4).

**The output coupling's corner** (D5) is one corner, not two:
- with the limiter idle, C4, R7, Q4's node, C5 and R10 form one series loop into the codec input amplifier's virtual ground: 1/(2π·23 kΩ·90.9 nF) = 76.1 Hz;
- the amplifier's closed-loop output impedance (about 2–3 kΩ; the script prints its estimate from R-01's network) lowers it toward 69 Hz;
- C4 and C5 are Y5V parts with +80/−20 % tolerance, so units vary over about 40–95 Hz (registered, not modeled);
- under deep limiting the corner moves to C5·R10 = 159 Hz (not modeled; spec Out of Scope).

**Alternatives considered.**
- Biquads: two sections where three first-order sections suffice. Spec 009 chose first-order sections for the same reason.
- One combined minimum-phase filter after the clipper: it cannot put the input high-pass before the clipper.
- An L1-norm bound (the coach's C1 form): looser (about 2·max(L) against L₊ + L₋), and unnecessary given the 1 − LP identity.

## R-03: The clipper (FR-006; D6, D8, C10)

**Decision.** A memoryless asymmetric soft clipper:

- y = L₊ · knee(x / L₊, k) for x ≥ 0;
- y = −L₋ · knee(−x / L₋, k) for x < 0;

reusing spec 009's parabolic knee (`stages/output.rs::knee`, C¹ at both joints, a hard ceiling at 1). The divisions are precomputed reciprocals, and the comparisons are branchless selects (spec 010's lesson on x86).

**The limits** come from the DC voltages (sheet 3.1), with 0 dBFS = 0.196 V peak at the amplifier's output (A-029, A-030):

| Engine samples | What clips | Headroom | Limit (plan-time) |
|---|---|---|---|
| Negative (clip first) | The output (1.7 V) falls until Q1 saturates near 1.15–1.3 V, with its emitter at 1.1 V | 0.4–0.55 V | **≈ +7 dBFS** (+5 … +9) |
| Positive | Q2 saturates (3.2 V − 0.1…0.2 V − 1.7 V) | 1.3–1.4 V | **≈ +16.5 dBFS** (+16 … +17.5) |

**The knee start** `k` is chosen so the onset (the 1 kHz input level at which a polarity's output peak falls 0.1 dB below the linear peak, spec US2 AS1) sits at a stated distance below each limit. The loop gain is low (R-01), so a gradual onset is plausible. The script prints `k`, both onsets and both limits; the spec's "about +7 / about +16.5 dBFS" ranges hold for both.

**Polarity.** Negative engine samples are the polarity the limiter does not sense (A-036), so a loud asymmetric sound is clipped on the side the limiter ignores and limited on the other. A-036 gains the sentence.

**Aliasing.** The clipper runs at the host rate. At 6 and 8 kHz a clipped 1 kHz tone's 4th–6th harmonics fold into the band. This is documented, and harmonic levels are asserted only at 44.1–96 kHz (spec Clarifications). Oversampling or ADAA was rejected: about 2–4× the clipper's cost against a +10 % budget, for an effect that only very loud input triggers.

## R-04: The hiss (FR-003; Clarifications Q2, D3, D9, D10)

**Anchor** (registered; printed by the script):
- capsule self-noise **34 dB SPL(A)** (32–36 for small electrets of the period: S/N 58–62 dB at 94 dB SPL, the WM-61A / WM-63 class; credible 26–38);
- full scale **90 dB SPL at 1 kHz** (80–100, A-029);
- the A-weighted-to-300–3000 Hz offset through stage 2's band, about −6 dB (the reviewer computed −5.9 with the tilt);
- giving **≈ −62 dBFS** (RMS, AES17) in the 300–3000 Hz band at stage 2's output.

The script integrates the band level through the realized stage-2 response at each rate and solves for the source's RMS σ_r per host rate. The source is white at the host rate, so σ_r grows with the rate. The script prints σ_r (exact `f64` bits, emitted into the coefficients file), the band level, the 1 kHz third-octave level and the third-octave shape at the centres 315–2500 Hz.

**Generator** (deterministic, no platform math):
- **Uniform source:** `xoshiro256++`, a 256-bit state with a period of 2²⁵⁶ − 1, far beyond 3.5 × 10⁸.
- **Seeding:** the state is seeded by **SplitMix64** from `seed ^ MIC_STREAM` and the recording counter, where `MIC_STREAM` is a stage-specific constant. A later stochastic stage takes its own constant and leaves this stream unchanged. SplitMix64 never produces an all-zero state, so seed 0 does not degenerate.
- **Gaussian:** a **ziggurat** (Marsaglia & Tsang 2000, 128 layers) whose tables the script emits as `f64` bit patterns. The rare tail and wedge paths use `rr_dr60_detmath::{exp, ln}`, so the samples are exactly Gaussian to well beyond ±5σ.
- **Cost:** about 1–2 ns per sample on the fast path.

**Level.** One multiply per sample by σ_r·10^(offset/20). That gain is computed at construction with `detmath::exp`, so a setting of 0 dB multiplies by σ_r exactly.

**Streams** (D9, the user's choice):
- the recording counter is kept by the split engine (`Transport`), set to 0 at construction, `reset` and `reconfigure`;
- `start_recording` rekeys the recorder's capsule to `(seed, counter)` and then increments the counter;
- the one-pass record path's capsule uses counter 0 from construction, `reset` and `reconfigure`;
- so the first recording equals the one-pass (FR-009), and recording k carries stream k.

**Tests.**
- The 4σ exceedance count is within ±30 % of the Gaussian expectation over 10 s at 48 kHz.
- The normalized cross-correlation is below 0.01 between seeds 0 and 1, and between counters 0 and 1.
- In a release-only check, every 10 s window of a 60-minute run at 48 kHz is within ±0.5 dB.
- Partition independence is asserted: the generator advances once per processed host sample.

**Alternatives considered.**
- Box–Muller through detmath: one `ln`, one `sqrt`, one `sin` and one `cos` per two samples, about 15–25 ns per sample, which exceeds the cost budget alone.
- Irwin–Hall(12): bounded at 6σ, but its 4σ tail is about 30 % light, which fails the exceedance test and flatters the voice-activation margin at 6 kHz (D10).
- The harness's PCG32: fine for stimuli, but a separate core generator keeps the harness free to change.

## R-05: Placement, taps, latency and state (FR-001, FR-007 – FR-009)

**Decision.**
- **Where the stages run.** Stages 1–2 run inside `RecordPath::limited` (renamed `front`), between `sanitize_in` and the limiter, so the one-pass, the recorder, the stop's flush and frame completion all pass through them by construction.
- **The stop.** It already calls the recorder with zero-valued host input (`feed_zeros`), so the hiss continues through it, as FR-009 requires.
- **Bypass.** A bypassed stage is a skipped branch decided at construction (no arithmetic, as the limiter's `run_limiter`), so bypassed stages are bit-identical to 0.11 by construction.
- **Taps.** `Tap::AfterMicCapsule` and `Tap::AfterMicAmp` are host-rate taps without conversion, voice activation or chain, like `Tap::AfterAgc` (the one-pass's `limiter_only` path generalized to `front_only(upto)`), with latency 0. `Tap::AfterAgc` now includes stages 1–2 when they run; its latency stays 0.
- **Latency.** Not reported, as for the output chain (spec 009 R-08). Stage 2's 1 kHz group delay is reported per rate by the script and by a harness row: about 0.10 ms, or 5 samples at 48 kHz.
- **Reset and reconfigure.** `reset` puts the sections at rest and rekeys the capsule (counter 0 for the one-pass; the transport resets its counter). `reconfigure` rebuilds everything, as today.

**Alternatives considered.** A separate front-end object before `RecordPath`: two call sites and a second state to reset, against one function that already exists.

## R-06: The 0.11-default configuration (FR-010; C7)

**Decision.** `configs::settings_011(rate)` is `Settings::new` with `mic = MicSettings::BYPASSED`. The existing base configurations are rebased on it:
- `settings_008`, and therefore `settings_007` and every named configuration;
- `"default_output" | "default_earphone" | "default_loud"`;
- the output-clamp and volume-ramp twins (through those names).

Direct `Settings::new` uses are replaced by `settings_011` in `output_checks.rs`, `smoke_reference.rs` and `tests/output_edge_cases.rs`.

The C smoke loop's existing cases set the two new fields off after `rr_dr60_settings_default`, so their pinned hashes stay. New smoke cases use the 0.12 default and pin new hashes.

The seven SHA guards (`golden-v1`, `agc`, `vas`, `codec`, `output`, `output-clamp`, `volume-ramp`) are run in Phase 1 before and after the core change.

**Default-engine figures that move** (restated or labelled "0.11 default" in Phase 4):
- A-042's 1.96–2.06 peaks;
- spec 011's route-output click figures;
- spec 008's "silence −78 dBFS and below";
- spec 009's "decoder output on silence is quiet";
- the README's "default" timing figures.

## R-07: The voice activation and the hiss (US3 AS6; D4)

**Plan-time simulation** (the reviewer's `vas.py`): the real state machine (hang 6000, onset 120, gap 32 device samples) on Gaussian hiss through an approximation of stage 4 at 6 kHz.

| Hiss (300–3000 Hz, AES17) | Level 5 (−46 dBFS) | Level 4 (−43) | Level 3 (−40) |
|---|---|---|---|
| −62 (default) | pauses after the 1 s hang, never resumes | same | same |
| −55 | pauses | pauses | pauses |
| −52 | **never pauses** | pauses | pauses |
| −50 | never pauses | pauses after 48 s | pauses |
| −48 | never pauses | **never pauses** | pauses after 1.8 s |

- **Rule of thumb:** the gate pauses reliably when threshold − hiss RMS ≥ about 9 dB, and fails below about 6–7 dB.
- **At the default** the RMS margin to level 5 is about 16 dB.
- **At the top offset** (+12 dB, about −50 dBFS) levels 4–5 never pause in a silent room. This is documented, not refused.

**The harness check** (US3 AS6), at each sensitivity level and host rate (measurement rates under coverage):
- a seeded 60 s silent run with the hiss on keeps exactly the hang time of device samples, pauses once, and never resumes, asserted from the events;
- the RMS margin at the voice-activation input and the exceedance count are printed;
- the offset at which each of levels 4 and 5 stops pausing is found by bisection and reported;
- the 600 s run is release-only.

**A-022** is re-derived in Phase 4:
- the hiss's peaks (about −51 dBFS over 600 s) replace "capsule self-noise peaks around −57 to −62 dBFS";
- room-noise placements move by the input high-pass's tilt.

## R-08: The limiter and the speech codec with the hiss (US3 AS7, AS8; D11, D12)

**The limiter** (AS8).
- **Setup.** Twin engines with the same seed, one with the limiter bypassed, are read at `Tap::AfterAgc`. A loud 1 kHz burst (+9 dBFS, 10 dB over the knee, starting on a host-grid peak, spec 005 R-03) is followed by 3 s of silence.
- **Oracle.** Over the silence the ratio of the two outputs equals the limiter's gain, sample by sample. The hiss is identical, and the limiter is a pure gain below its knee.
- **Assertion.** The ratio equals `limiter_state`-derived gain within 4 ULP, as in spec 011's ratio reader.

**The codec** (AS7).
- **Setup.** The default configuration with the voice activation bypassed records and plays 10 s of silence.
- **Measurement.** The decoded hiss's level relative to the input hiss is computed at 8 kHz after resampling both sides (spec 008's lesson) and reported per rate. It is expected a few dB low: analysis-by-synthesis renders noise low.
- **Placement.** The hiss sits about 20 dB above the fixed-gain quantizer's floor (≈ −82 dBFS, `design_celp.py`), so it is coded, not zeroed.
- **Default.** With the voice activation on, the recording's length (hang + stop feed) is reported.

## R-09: Cost (FR-011; C11)

**Decision.** Three measurements, each the fastest of nine alternating solo release runs at 48 kHz, with the codec bypassed, on the speaker route:

1. **Idle A/B against 0.11** (≤ 3 %).
   - `scripts/ab-timing.sh <0.11 rev>` unchanged: `output_cost` in both builds.
   - In the branch, `output_cost`'s configuration is rebased on `settings_011` (R-06), so it measures the stages **bypassed**.
2. **On A/B against 0.11** (≤ 10 %).
   - `scripts/ab-timing.sh <0.11 rev> output_cost_mic`: a new optional head-test argument.
   - The branch's new `timing::output_cost_mic` is `output_cost` with stages 1–2 on, printing the same line format, so the baseline's `output_cost` and the branch's `output_cost_mic` compare directly.
3. **In-build** `timing::mic_cost`, reported.
   - Stages on against bypassed, on −20 dBFS noise and on full-scale noise, and the spread across hiss offsets.
   - Its ratio ≤ 10 % is asserted (CI-safe), since the A/Bs need two builds.

**Deviation from the spec's wording.** FR-011 says "−20 dBFS noise". The cross-build A/Bs must use `output_cost`'s fixed 0.11 stimulus (uniform ±0.5). The mic stages' cost does not depend on level below the clip limits (branchless), and `mic_cost` reports the level spread to show it. This is recorded for the plan review to amend FR-011.

**Estimate.**
- Ziggurat plus xoshiro is about 2 ns.
- Three first-order sections are about 3 ns.
- The normalization and the clipper are about 1–2 ns.
- That totals about 6–7 ns on about 73 ns: **≈ +8–10 %**. The budget is achievable but not loose (D10).

**Fallback** if the A/B exceeds 10 %: fold the normalization into the input high-pass's coefficients, and merge the output coupling and the roll-off into one biquad. The change goes to the review before it is made.

## R-10: Interfaces (FR-012; C14)

**Rust.**
- `Settings::mic: MicSettings { capsule_enabled, amp_enabled, hiss_offset_db }`, with `MicSettings::DEVICE` (on, on, 0.0) and `MicSettings::BYPASSED`. The struct is `#[non_exhaustive]`.
- Validation: `hiss_offset_db` must be finite and within −18 … +12 dB, or `Error::InvalidSetting`.
- `Tap::AfterMicCapsule`, `Tap::AfterMicAmp`.
- `mic::HISS_OFFSET_RANGE_DB` and the documented constants (`mic::FULL_SCALE_SPL_DB` = 90.0 as documentation of the reference).

**C.**
- `RrDr60Settings` grows 112 → **120** bytes (8-aligned):
  - `mic_capsule_enabled` at 112 and `mic_amp_enabled` at 113;
  - `reserved_012 [u8; 2]` at 114;
  - `mic_hiss_offset_db: f32` at 116.
- Const assertions on size, alignment and offsets; `struct_size = 112` is refused with the existing invalid-argument status and the struct-size field, as spec 010 did for 104.
- Taps `RR_DR60_TAP_AFTER_MIC_CAPSULE = 9` and `RR_DR60_TAP_AFTER_MIC_AMP = 10`.
- `RR_DR60_VERSION_MINOR` 12.

**Stored form.** Unchanged (version 2). Reconfiguring stages 1–2 needs no empty memory.

**No live control.** The offset is a setting applied by `reconfigure`. Whether it becomes a live control belongs to the deferred controls discussion.

## R-11: Golden file and records (FR-013, FR-014)

**Golden file.** `golden-mic-v1.json`:
- **Stimuli:** silence 1 s; voiced −20 dBFS 1 s (`stimulus::voiced`); a loud 1 kHz tone at +12 dBFS for 0.25 s, starting on a host-grid peak.
- **Configurations:** `mic_only` (stages 1–2, tap after stage 2); `mic_only_quiet` (hiss off); `mic_capsule_only` (tap after stage 1); `default_mic` (the 0.12 default, voice activation bypassed).
- **Size:** 3 stimuli × 4 configurations × 7 host rates = **84 entries**, each with its configuration, counter and offset, and the output's hash and length.
- **Bless:** once, after the post-implementation reviews, with its SHA pinned and added to `ci.yml` (check, golden matrix, iOS simulator) and `scripts/ios-device-golden.sh` in the same commit.

**Manifest.** Stages 1 and 2 become `assumed`, model version 0.12.0, with their sources, assumptions, not-modeled notes (each ≤ 120 characters) and the Rust API items (`Settings::mic`, `MicSettings::*`, the taps). The canonical text and the README table are re-blessed once in Phase 1, after the wording and denylist runs on the new strings.

**Registers.**
- **New assumptions:** A-046 the input high-pass; A-047 the output coupling with C4/C5's tolerance and the lumping ahead of the limiter; A-048 the hiss anchor and shape; A-049 the generator and streams; A-050 the amplifier's added noise and the unknown codec reference noise; A-051 the clip limits, softness and polarity; A-052 the capsule's flat response.
- **Amended:** A-005, A-006, A-019, A-022, A-028, A-029, A-036.
- **Other records:** signal-chain rows 1–2; `sources.md`'s S-002 summary (R6; R1 is the record lamp's resistor); the hardware-evidence issue template (the reviewer's seven captures); the seed's rustdoc and its C comment.

## R-12: Risks

- **The input corner is wrong.** If a capture shows a flat low end, the corner is a setting change plus a re-bless, not a model change (A-046 registers the capture protocol).
- **The cost budget** (R-09 fallback).
- **The voice-activation coupling.** A future capture that raises the hiss can turn levels 4–5 into "never pause" (documented).
- **The ziggurat tables** are an implementation detail with exact bit patterns; a table regeneration is a model change and a re-bless.
- **Coverage time.** The 60 s voice-activation runs at seven rates are heavy under instrumentation, so they use `measurement_rates()` under `cfg(coverage)`, and the 600 s runs are release-only.
