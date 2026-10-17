# Feature Specification: Microphone Capsule and Amplifier (signal-chain stages 1–2)

**Feature Branch**: `012-mic-capsule-amp`

**Created**: 2026-10-16

**Status**: Implemented on the branch `012-mic-capsule-amp` (2026-10-16; post-implementation reviews applied, golden blessed)

**Input**: User description: "Feature 012: model signal-chain stages 1 and 2, the microphone capsule and the microphone amplifier, ahead of the record-path limiter. Today the engine's input is treated as the mic-amp output (A-036), so nothing models what happens between the sound at the microphone and the codec's input: the capsule's frequency response and self-noise, the amplifier's gain, its coupling high-pass corners (dominant ≈ 72 Hz, first order) and ≈ 13 kHz roll-off, its noise floor (the "hiss" floor), and its clipping on very loud sounds before the limiter can act. Evidence: S-002, A-005, A-006, A-029, A-030, A-036. The user wants the record path to start at the sound reaching the microphone. Each new stage must be independently bypassable and testable, deterministic (seeded noise), real-time safe, traceable to a source or a registered assumption, and every pre-012 configuration must stay bit-identical with the new stages bypassed (the earlier golden files unmoved). Out of scope: noise reduction (stage 6, to be researched deeper), clock and power (stages 14–15, to be researched deeper), handling noise and wind."

## Overview

### What the record path lacks today

The record path today starts at the limiter (stage 3), and the engine treats its input as the **mic amplifier's output** at a fixed gain: 0 dBFS is the level that reaches the codec's full scale ([A-029](../../docs/hardware/assumptions.md), [A-030](../../docs/hardware/assumptions.md), [A-036](../../docs/hardware/assumptions.md)). Nothing models what happens between the sound arriving at the microphone and that point:

- **The hiss floor.** The electret capsule's self-noise, which every recording of a quiet room carries.
- **The band.** How the amplifier shapes the sound.
- **Clipping.** The amplifier clips very loud sounds before the limiter can act on them.

The stage manifest lists stages 1 and 2 as not modeled.

### A finding from the spec review

The spec review re-read the service manual's schematic (S-002, sheet 3.1) at full resolution and corrected the circuit summary in the registers. The microphone amplifier is a **two-stage transimpedance amplifier**:

- Q1 and Q3 form a differential pair.
- Q3 drives the PNP Q2 as a second stage, with R6 (100 kΩ) as its load.
- The output feeds back to Q1's base through R3 (100 kΩ).

The capsule's signal current reaches that feedback node through C1 (0.15 µF), and part of it is lost into the capsule's 1 kΩ bias resistor R2. That makes a **first-order high-pass near 0.82 kHz** (665–985 Hz; plan review), inside the device's band. Relative to 1 kHz, it cuts 300 Hz by about 7–8 dB and lifts 2.5 kHz by about 2 dB.

The registers had listed C1 as a 10–15 Hz corner (A-029), which this re-read corrects. The coupling corner after the amplifier is a **single** first-order corner near 77 Hz: C4, R7, C5 and R10 form one series loop while the limiter is idle, and the 7 Hz "minor corner" does not exist. Both of the corners the feature description named (72 Hz and 13 kHz) are inaudible in the device's 225–2550 Hz band. The input high-pass is not.

### What this feature adds

This feature adds both stages to the record path, ahead of the limiter, so the record path starts at the sound reaching the microphone:

- **Stage 1, the microphone capsule.** The two-terminal electret RJM0016 ([S-002](../../docs/hardware/sources.md), A-005) contributes its **self-noise, the hiss**:
  - deterministic Gaussian noise from the engine's seed;
  - its own stream for each recording;
  - a level offset setting;
  - a flat response, because nothing is known about the capsule's own response.
- **Stage 2, the microphone amplifier**, in the device's order:
  1. the input coupling high-pass near 0.82 kHz;
  2. the amplifier's gain, normalized to unity at 1 kHz;
  3. its asymmetric soft clipping at its supply rails, with negative samples clipping first;
  4. the output coupling near 77 Hz;
  5. the codec input amplifier's roll-off near 13 kHz.

### How it fits the existing record path

- **Order.** The record path becomes capsule → amplifier → the existing limiter → everything that follows today.
- **Limiter interaction.** The hiss passes through the limiter like any other sound. After a loud sound the limiter pulls the hiss down, and the hiss recovers as the limiter releases.
- **Bypass.** Each stage can be bypassed on its own. With both bypassed, every observation point is bit-identical to version 0.11, and every earlier golden file is unmoved.
- **Default output.** Both stages are on by default, so the library's default output changes, and the CHANGELOG records it as an output change.

### Evidence

The evidence is circuit-level, not a measurement. Known from the service manual: the parts, the topology, the resistor and capacitor values and the DC voltages. **Not known:**

- the capsule's sensitivity, self-noise and response (A-005: low confidence);
- the amplifier's open-loop gain, which places the input corner;
- the absolute gain, which places full scale between 80 and 100 dB SPL (A-029).

So the input corner, the hiss and the clip points are **modeled on** the circuit with typical part figures, printed by a design script and registered. They can be refined when a capture from a real unit arrives.

## Clarifications

### Session 2026-10-16 (specify)

- Q: What does the engine's input level mean once a microphone sits in front of it? → A: **Today's reference is kept** (the user's choice, option A):
  - 0 dBFS at the input still reaches the codec's full scale **at 1 kHz**, and the new stages are normalized to unity gain at 1 kHz.
  - The amplifier's uncertain absolute gain enters the engine only as documentation: full scale corresponds to **about 90 dB SPL at the microphone at 1 kHz (80–100, low confidence)**, A-029.
  - App makers scale their own microphone input; the README gives the arithmetic.
  - The hardware reviewer, consulted at the user's request, recommends keeping it: the gain is the least certain number in the chain; an SPL-calibrated input would bake that guess into every limiter, voice-activation and codec figure; and an opt-in "SPL at full scale" setting is only an input gain that can be added later without breaking anything.
- Q: Is the capsule's hiss on by default, and at what level? → A: **On by default at the middle estimate** (the user's choice, option A), about −62 dBFS in the 300–3000 Hz band.
  - The hardware reviewer, consulted at the user's request, recommends keeping it on, and found that the first derivation had two errors that cancel. Corrected:
    - small electret capsules of the period specify 32–36 dB SPL(A) of self-noise (S/N 58–62 dB at 94 dB SPL), not 25–30;
    - the 300–3000 Hz band level is about 6 dB below the A-weighted wideband level;
    - so 34 dB SPL(A) − 6 dB against a full scale of 90 dB SPL gives **about −62 dBFS (RMS, AES17: a full-scale sine reads 0 dBFS) in the 300–3000 Hz band at stage 2's output**.
  - The credible range is about **−80 to −48 dBFS**.
  - The exact figure, its 1 kHz third-octave level and its derivation are printed by the design script and registered.

### Session 2026-10-16 (spec review: the hardware reviewer's D1–D15 and the coach's C1–C17)

The user's answers to the reviewers' questions:

- **The input high-pass (D1).** It is **modeled and on by default** as part of stage 2. It is confirmed by a stated circuit model in the plan's design script first, and registered (high confidence that it exists, medium in the value, 665–985 Hz, after the plan review).
- **A hiss level offset (D3, D15).** **Added**, contrary to the specify choice of "no level setting": a bounded offset in dB, **−18 to +12 dB, default 0**.
  - It absorbs a future capture as a default change rather than a model change.
  - It lets a host whose own microphone already carries a floor turn the hiss down rather than off.
  - Above about +9 dB the voice activation stops pausing in a silent room at sensitivity level 5 (documented).
- **The hiss per recording (D9; the coach's C-question 4).** **Each recording has its own deterministic stream**, derived from the seed and a recording counter.
  - The counter starts at 0 at construction, reset and reconfigure, and advances by one at each recording start.
  - This is the hardware reviewer's recommendation. The coach recommended a shared stream, but two recordings of a silent room would then be bit-identical, and with the default seed every app's recordings would carry the same noise.

Taken at the reviewers' defaults:

- **The order inside stage 2 and the bound after it (C1, D2).** The clipper sits between the input high-pass and the output coupling, as on the device.
  - The clipper's own output never exceeds its limits.
  - After stage 2 the output can exceed them, because the output coupling recentres an asymmetric clip, as it does on the device, with a time constant of about 2 ms (plan review).
  - The bound after stage 2 is |y| ≤ L₊ + L₋: the coupling is 1 − LP with a non-negative low-pass impulse response of unity DC gain. That is about +19 dBFS, so the limiter's attenuation range is still never exceeded.
- **Where the output coupling and the 13 kHz roll-off sit (C1, D5).** Physically they lie after the limiter's attenuator: C5 and R10, and C10 across R15 in the codec input amplifier. They are lumped ahead of the limiter as linear filters moved past a slowly varying gain, registered. The limiter's detector senses after them on the device as well. Not modeled: the corner's shift while the limiter's shunt conducts (C5 and R10 alone, 159 Hz).
- **One switch for stage 1 (C12).** With the capsule's response flat, stage 1 consists of the hiss. Stage 1 on means hiss on; the offset sets its level. A second switch is added only when capsule data gives the response real content.
- **Clipper aliasing (C5, D8).** At the 6, 8 and 16 kHz host rates a clipped tone's harmonics fold back. This is documented, not modeled away. Harmonic levels are asserted at 44.1–96 kHz and reported, with the alias level, at the lower rates. The README recommends a host rate of at least 16 kHz for loud material.
- **Latency (D7; the coach's C14).** The new stages report **no latency**, as the output chain does (spec 009: minimum-phase filters, group delay reported, not added). Every existing latency figure and the one-pass coordinates are unchanged. Stage 2's 1 kHz group delay (about 0.1 ms) is reported per rate.

  The hardware reviewer preferred adding the group delay to the latency. The coach's form and spec 009's precedent were taken: the delay is a fraction of a millisecond, and adding it would move every documented latency figure.
- **The hiss's distribution (D10).** Gaussian to at least ±5σ, from a generator whose period exceeds the longest recording, seeded so that seed 0 does not degenerate. The stream is specific to this stage, so any later noise source leaves it unchanged.
- **The clip points (D6, C10).** Stated in dBFS at the engine input at 1 kHz, independent of the limiter's knee setting, and derived from S-002's DC voltages:
  - **negative samples are limited first, at about +7 dBFS** (+5 … +9): Q1 saturates;
  - **positive samples are limited at about +16.5 dBFS** (+16 … +17.5): Q2 saturates.

  These are the limits; the soft onsets lie a documented distance below them (plan review). The negative polarity is the one the limiter does not sense (A-036). *(restated after the post-implementation review, 2026-10-16: see Clarifications › Post-implementation reviews)*
- **Applied as written:** C2, C3, C4, C6–C9, C11, C13, C15–C17 and D4, D5, D11–D14.

### Session 2026-10-16 (plan review: the coach's P1–P16 and the hardware reviewer's Q1–Q15)

The user's answers:

- **The recording counter never rewinds** (hardware Q2, the user's choice):
  - It counts successful recording starts since construction, across `reset` and `reconfigure`, which keep the memory's recordings; a refused start consumes nothing.
  - The one-pass uses the stream of the counter's current value (the next recording's), from construction, `reset` or `reconfigure`. So the first recording after any of them still equals the one-pass, and no two recordings of one engine share a stream.
  - Determinism is "same seed and same call history".
- **The circuit-simulation cross-check runs** (coach P4, hardware Q13, the user's choice). ngspice is installed on the development machine, and the design script's `--spice` run is a gate before the golden bless. A disagreement above 10 % with the nodal model holds the bless.
- **The cost budget is asserted in-build** (hardware Q6, coach P8, the user's choice):
  - **Asserted:** stages 1–2 on against bypassed, in one build, on −20 dBFS noise, at most 10 % on the development machine; the CI runner reports it under a 25 % safety ceiling.
  - **Asserted:** the cross-build A/B against 0.11 with the stages bypassed, at most 3 %.
  - **Reported only:** the cross-build A/B with the stages on, because stage 2 changes the level reaching the limiter and the speaker amplifier, whose cost depends on level.

Taken at the reviewers' defaults:

- **Limits, not onsets** (coach P5, hardware Q4).
  - S-002's DC voltages give the clip **limits**: L₋ ≈ +7 dBFS (+5 … +9) for negative engine samples and L₊ ≈ +16.5 dBFS (+16 … +17.5) for positive ones.
  - The soft **onsets** lie a documented distance below them; with spec 009's knee at k = 0.85, about 0.6 dB below. The knee is justified by Q1's saturation curvature at a high loop gain. *(restated after the post-implementation review, 2026-10-16: see Clarifications › Post-implementation reviews)*
  - Static figures are checked in the clipper's own units through a probe that exposes the engine's own clipper input and output. Engine-input figures at 1 kHz use a start phase compensated by the input high-pass's phase, which the script prints.
- **The loop gain is high, not low** (hardware Q5). The impedance into the amplifier's input node is about 135–420 Ω, so the loop gain is about 240–740.
  - The input corner is set mainly by C1 with R2. It is bounded above by 1/(2π·C1·R2′) ≈ 1.0 kHz whatever the transistors.
  - Nominal **≈ 818 Hz**. Over C1 ±10 %, hFE, the Early voltage and Q3's current the range is **665–985 Hz**.
  - The full transfer to the codec input amplifier's output is three first-order corners (≈ 818 Hz, ≈ 77 Hz, ≈ 12.9 kHz), joint-fit by the script.
  - Below-clip distortion is negligible.
  - The capsule-current-to-full-scale chain now closes: full scale ≈ 82–92 dB SPL for capsules of −35 to −44 dBV/Pa, supporting 90 dB SPL.
  - The amplifier and R2 add a floor near −73 dBFS. With the capsule's S/N held at 58–62 dB, the hiss's credible range narrows to about −70 … −54 dBFS. The default (−62) is unchanged. *(restated after the post-implementation review, 2026-10-16: see Clarifications › Post-implementation reviews)*
- **Realization** (hardware Q1, coach P14). The three sections are a per-rate constrained fit to the prototype, made by the design script. Constraints:
  - the input high-pass carries the 1 kHz normalization;
  - the output coupling is 1 − LP′, with LP′ having a non-negative impulse response that sums to 1;
  - the roll-off has non-negative coefficients, a pole in [0, 1) and unity DC gain.

  `--check` asserts the constraints, which keep the L₊ + L₋ bound's proof, and the ±0.1 dB match. If the realized fit misses ±0.1 dB above 4 kHz at a rate, the tolerance there becomes ±0.25 dB (inaudible after the decimator), recorded as a deviation.
- **The output coupling recentres with a time constant of about 2 ms** (coach P6, hardware Q9). The time to return within ±0.1 dB depends on the overdrive and is printed for the stated stimulus.
- **The generator** (coach P3, hardware Q3, Q8):
  - **Retry cap.** The ziggurat's rejection loops are capped at 8 attempts each, returning the last candidate (probability of reaching the cap about 1e-16 per sample, registered; restated 2026-10-16: about 2.9e-13, set by the tail loop).
  - **Bit use.** The layer index and the mantissa use non-overlapping bits.
  - **Keying.** The key is two independent bijections: mix64(seed ^ stream constant) ^ mix64(counter ^ salt). *(restated after the post-implementation review, 2026-10-16: see Clarifications › Post-implementation reviews)*
  - **Advance count.** The source advances a variable but deterministic number of times per sample; the hiss is a function of the host samples processed.
- **Statistics** (coach P7, hardware Q7):
  - **Tails.** The 4σ and 5σ tail counts are core release-only tests on 10⁸ draws.
  - **Correlation.** Measured at the tap after stage 1 over 480 000 samples, with thresholds of 5/√N at lag 0 and 6/√N over lags ±64.
- **Harmonics** (hardware Q14). H2 and H3 are asserted within ±0.5 dB of the script's **discrete** model at all seven host rates. The alias level is reported as the discrete figure minus the continuous one.
- **Golden scope** (coach P12, hardware Q11):
  - **New configurations:** `default_mic_quiet` and a second-recording family (counter 1).
  - **New entries:** an entry at offset −12 dB.
  - **New stimulus:** a +24 dBFS tone that reaches both limits.
- **Interfaces** (coach P13):
  - the offset is validated whether or not the capsule is on;
  - Rust `Setting::MicHissOffsetDb` and the C setting field are appended;
  - no public full-scale SPL constant: the 90 dB SPL figure lives in the docs and A-029.
- **The README** tells hosts to set a per-session seed when recordings from different sessions must not share hiss (coach P16).
- **The order of work** (coach P1, P2, P10):
  - the C fields land inert in Phase 1;
  - every `Settings::new` in the core, the FFI and the doctests is audited and rebased at the default flip;
  - each story's checks are written red first;
  - one review-driven re-bless of the manifest text is allowed in Phase 6.

### Post-implementation reviews (2026-10-16; the hardware reviewer's B1–B8 and the coach's C1–C18)

The user's answers:

- **The clipper is refitted to the circuit simulation** (B1, the user's choice). The design script's `--spice` run gained a 1 kHz transient level sweep (0 … +30 dBFS). It showed the hard-rail estimates 2.4 dB low on the negative side and 1.0 dB low on the positive side, with the negative limit soft (Q1's base–collector junction conducting across R3) rather than a rail. Each polarity now has its own limit and knee, the minimax fit to the sweep with the onset held within 0.3 dB of the simulation's: **L₋ ≈ +9.15 dBFS, k₋ = 0.65; L₊ ≈ +17.10 dBFS, k₊ = 0.80** (worst error 0.45 / 0.29 dB). The sweep is a gate before the bless at ±1 dB. FR-006 and US2 are restated below; A-051 is revised.
- **The keying is nested** (B5, the user's choice): `mix64(mix64(seed ^ stream constant) ^ counter ^ salt)`. The XOR of two mixes was symmetric in its arguments, so families of (seed, counter) pairs shared a stream. Every hiss sample changes; the figures are re-read.
- **The hiss's default stays at −62 dBFS** (B2, the user's choice); its credible range is restated from the closed level chain as **about −66 … −53 dBFS** (FR-003), and the default corresponds to a capsule of about −42 dBV/Pa.
- **No impulse stimulus in the golden** (coach C17, the default).

Applied as records: the retry cap's probability (about 2.9e-13 per sample, B6), the 6 kHz H3 on Nyquist (B3), the onset rows tightened and a knee mutant (B4), the codec wording (B7), the evidence ask (B8), and the coach's C1–C16 and C18.

## User Scenarios & Testing *(mandatory)*

"Documented" figures in this spec are printed by the feature's design script and registered as assumptions. Tests pin to the script's print, never to the engine's own output.

### User Story 1 - The device's microphone sound: its band and its hiss (Priority: P1)

An app maker records through the emulated recorder. The sound now passes the device's microphone path:

- **Low end.** The input high-pass thins it below about 1 kHz: 300 Hz sits about 7–8 dB below 1 kHz.
- **High end.** It is lifted slightly toward 2.5 kHz.
- **Very low rumble.** Removed below about 77 Hz.
- **Above 13 kHz.** Rolled off before the limiter sees it.
- **Quiet passages** carry the capsule's hiss, a steady broadband floor at a documented level, shaped by the same band.

**Why this priority**: The band and the hiss are the most audible properties of the microphone path. The band changes every recording, and the hiss is what a quiet room sounds like on the device. Together they are the core of the feature.

**Independent Test**: With every later stage bypassed, read the signal after stage 2 at each host rate:
- feed an impulse and tones with the hiss off for the band;
- feed silence with the hiss on for the floor.

No other stage is needed.

**Acceptance Scenarios**:

1. **Given** stage 1 off and stage 2 on, **When** an impulse is processed, **Then** the magnitude response after stage 2:
   - matches the documented analog prototype (the circuit model's three first-order corners, normalized to 0 dB at 1 kHz) within ±0.1 dB;
   - holds that tolerance at every third-octave centre from 20 Hz to the lower of 20 kHz and 0.45 × the host rate, wherever the prototype is above −40 dB;
   - holds at every host rate.

   The prototype's −3 dB frequencies are reported per rate. Each realized section's −3 dB point is also read in unit tests against the script's print. If the fit misses ±0.1 dB above 4 kHz at a rate, the tolerance there is ±0.25 dB, recorded as a deviation (plan review). At the host rates whose band ends below 13 kHz, the roll-off's in-band effect is part of the matched prototype.
2. **Given** stage 1 on at offset 0 and silence in, **When** at least 10 s are processed at any host rate, **Then** the hiss after stage 2:
   - has a level in the 300–3000 Hz band (RMS, AES17) equal to the documented figure (about −62 dBFS) within ±0.5 dB;
   - has third-octave levels at the centres 315–2500 Hz each within ±1 dB of the documented shape (the white source through the band of AS1).
3. **Given** a hiss level offset of x dB within −18 … +12, **When** silence is processed, **Then** the hiss level is the default figure + x dB within ±0.5 dB. An offset outside the range is refused with the existing invalid-setting status.
4. **Given** the same seed, the same recording counter and the same input, **When** processed twice, on any platform and in any block partition, **Then** the output is bit-identical. The hiss after stage 1 (white) over 480 000 samples at each rate, for two seeds and for two consecutive counters, has a normalized cross-correlation below 5/√N at lag 0 and below 6/√N at every lag within ±64.
5. **Given** the generator, **When** 10⁸ draws are examined (a core release-only test), **Then**:
   - the count beyond ±4σ is within ±5 % of the Gaussian expectation (about 6 334);
   - the count beyond ±5σ is within ±40 % of its expectation (about 57);
   - forcing the retry cap through a hook returns a deterministic value;
   - the documented generator's period exceeds the longest recording at 96 kHz (3.5 × 10⁸ samples);
   - in a release-only check, every 10 s window of a 60-minute hiss is within ±0.5 dB of the figure.

---

### User Story 2 - Very loud sounds overload the microphone amplifier (Priority: P2)

A sound loud enough to overdrive the amplifier is limited there, before the limiter can act *(restated 2026-10-16 after the circuit simulation, B1)*:

- **Negative samples are limited first and softly,** toward about 9 dB above full scale (Q1's base–collector junction conducts across R3).
- **Positive samples are limited about 17 dB above full scale** (Q2 saturates at the rail).
- **The onsets are soft** and the limits held, adding the harmonic distortion of a real overload, including even harmonics from the asymmetry.
- **The output coupling recentres** the clipped waveform afterwards, with a time constant of about 2 ms, as on the device.

**Why this priority**: Overload is the microphone path's second audible behavior. It also bounds what reaches the limiter. It matters less than the band and the hiss because it applies only to very loud input.

**Independent Test**: With every later stage bypassed and the hiss off, read the clipper's own output (through a unit-level probe) and the signal after stage 2, feeding 1 kHz tones and steps at rising levels and asymmetric test signals.

**Acceptance Scenarios**:

1. **Given** the clipper's static curve, read through a probe that exposes the engine's own clipper input and output, **When** inputs are swept in the clipper's units, **Then**:
   - the output equals the input exactly below each polarity's onset;
   - it never exceeds that polarity's limit;
   - the limits (L₋ ≈ +9.15 dBFS, L₊ ≈ +17.10 dBFS; restated 2026-10-16, B1) and the onsets equal the script's figures, whatever the limiter's knee setting.

   **Given** a 1 kHz tone at the engine input, started on the phase the script prints (compensating the input high-pass's phase), **When** its level is raised step by step, **Then** the engine-input onsets and limits are within ±0.5 dB of the script's figures at every host rate.

   The **onset** of a polarity is the 1 kHz input level at which that polarity's output peak falls 0.1 dB below the linear peak. The **limit** is the polarity's largest output for any input. Which polarity clips first is documented relative to the engine input's sign and A-036's sensed polarity.
2. **Given** any input, including +40 dBFS tones, steps and non-sinusoidal signals, **When** it passes stage 2, **Then** no sample after stage 2 exceeds L₊ + L₋ (about +19 dBFS). The limiter's attenuation range is therefore never exceeded.
3. **Given** a tone 6 dB above the first onset, **When** it is processed, **Then** the 2nd and 3rd harmonic levels after stage 2 are within ±0.5 dB of the script's discrete model at every host rate. The alias level (the discrete figure minus the continuous one) is reported.
4. **Given** clipping in progress, **When** the input falls back below the onset, **Then**:
   - the clipper's output equals the linear path's from the next sample (the clipper is memoryless);
   - after the last clipped sample, the difference between stage 2's output and a clipper-off twin's is exactly the output coupling's and roll-off's decaying tail. It returns within ±0.1 dB of the linear response within the time the script prints for the stated stimulus: a 1 kHz tone at +13 dBFS for 50 ms, then −20 dBFS. The coupling's time constant is about 2 ms.

---

### User Story 3 - The microphone path in the whole engine (Priority: P2)

An app maker uses the engine as before, recording then playing or using the one-pass call. The new stages are part of the record path:

- each can be bypassed and observed on its own;
- the hiss passes the limiter, the voice activation and the speech codec as the device's would;
- every engine contract holds: record-then-play equals the one-pass, the engine is deterministic and allocation-free, and the cost stays within budget.

**Why this priority**: The stages must fit the engine's existing guarantees, or app makers cannot rely on them. With the hiss on by default from P1, the bit-identity, equivalence, determinism and allocation checks (AS1–AS3, AS8) are checked at the P1 checkpoint as well.

**Independent Test**: The existing whole-engine harnesses, run with the new stages on and off at the seven host rates: equivalence, determinism, allocation, mutation, timing and golden files.

**Acceptance Scenarios**:

1. **Given** both new stages bypassed, **When** any pre-012 configuration is processed (every earlier harness configuration, every earlier golden file's entries including their default entries, and the C smoke loop's earlier cases, all built on a 0.11-default configuration), **Then**:
   - every observation point is bit-identical to version 0.11;
   - the earlier golden files and their guards are unmoved;
   - the earlier smoke hashes are unchanged.
2. **Given** the new stages on, **When** the first recording after construction, reset or reconfigure is recorded and then played, **Then** it equals the one-pass output under the existing coordinates. Both use the counter's current value.
   - The stop's flush and frame completion feed zero-valued input through stages 1–2, so the hiss continues through them exactly as through the one-pass's trailing zeros.
   - Recording k since construction carries the stream for counter k. `reset` and `reconfigure` do not rewind the counter. A refused start does not advance it, and an empty recording discarded at the stop has consumed its stream.
3. **Given** a reset, a reconfigure, or the start of each recording, **When** processing resumes, **Then** the new stages' filters start at rest, and the hiss starts at the beginning of the stream for the current counter.
   - A recording's hiss depends only on the seed, the counter, the host rate and the number of host samples processed since its start.
   - A zero-length call processes none.
4. **Given** each new stage, **When** it alone is switched off, **Then** it passes its input unchanged and the other stage is unaffected.
5. **Given** the observation points after stage 1 and after stage 2, **When** selected, **Then** each can be read with latency 0.
   - After stage 1 carries the input plus unshaped hiss.
   - After stage 2 carries the shaped, clipped signal.
   - No earlier observation point changes its meaning or its latency.
6. **Given** the hiss on (offset 0) and silence in, **When** a seeded 60 s run is made at each sensitivity level and host rate with the voice activation on (drop mode), **Then**:
   - the recording keeps exactly the hang time of device samples (A-025), pauses once and never resumes, asserted from the events;
   - the report prints the RMS margin (threshold − hiss RMS at the voice-activation input, AES17) and the exceedance count;
   - the documented margin at level 5 is at least 10 dB (about 17 dB at offset 0 by the script; 16.8 dB in the harness).

   A 600 s run belongs to the release-only set. The offsets at which the gate stops pausing at levels 4 and 5 are reported.
7. **Given** the default configuration with the voice activation bypassed, **When** 10 s of silence are recorded and played, **Then** the decoded hiss level relative to the input hiss is reported per host rate (not asserted: the speech codec reshapes noise). Spectral levels are computed at 8 kHz after resampling both sides (the spec 008 lesson). With the voice activation on (default), the recording holds the hang time of hiss plus the stop feed, and its length is reported.
8. **Given** twin engines with the same seed, one with the limiter bypassed, **When** a loud 1 kHz burst above the knee is followed by silence, **Then** over the hiss-only segment the on/off sample ratio is at most 1 and non-decreasing. The limiter's state over the release equals, within 1e-6 relative, that of an engine with stage 1 off fed the first engine's own stage-1 signal during the burst and exact zeros after it, so the hiss is only scaled by the limiter and never drives its detector. *(Restated at implementation, 2026-10-16: the original "two seeds' ratio trajectories agree within 4 f32 ULP" cannot hold, because each seed's hiss moves the burst's peaks and so the charge at the burst's end; the seeds differ by about 700 – 5 800 ULP, now reported. Research R-08.)*
9. **Given** the new stages on, **When** the cost per host sample at 48 kHz is measured, **Then**:
   - **Conditions:** speaker route, codec bypassed, release build, timing tests one at a time; in one build each side the fastest of 45 strictly alternating runs, across builds the fastest of nine alternating rounds (the A/B script). *(Amended at implementation, 2026-10-16, the user's decision: the in-build comparison is the fastest of **45 strictly alternating runs** per side, because the fastest of nine was run-order sensitive on the development machine; the cross-build "on" figure is held to the same 10 % as corroboration. Research R-09 › As measured.)*
   - **In one build, on against bypassed,** on −20 dBFS noise: at most 10 %, asserted on the development machine and reported on the CI runner under a 25 % ceiling.
   - **Bypassed against the 0.11 build,** on spec 009's `output_cost` stimulus: at most 3 %, asserted on the development machine and run once on the CI runner through the PR's `ab-timing` label before the merge.
   - **On against the 0.11 build:** at most 10 % as corroboration of the in-build figure, checked when the A/B is run (not asserted by a test), with the limiter uncharged at the end.
10. **Given** the new stages on, **When** the engine processes any block, **Then** it allocates nothing and its per-sample work is bounded.

---

### User Story 4 - The microphone path is documented and traceable (Priority: P3)

A reader of the README, the stage manifest or the registers learns:
- what stages 1 and 2 model, from which evidence and at what confidence;
- what they do not model;
- how to scale a microphone into the engine.

Every figure the engine uses traces to a source or a registered assumption.

**Why this priority**: The project's fidelity and claims policies require it, but it follows the behavior it documents.

**Independent Test**: The registers cross-check, the manifest golden, the README statements test and the traceability audit.

**Acceptance Scenarios**:

1. **Given** the stage manifest, **When** stages 1 and 2 are read, **Then** each:
   - is "assumed" and carries model version 0.12.0;
   - cites its sources and assumptions;
   - names what it does not model.

   The counts become 12 assumed and 3 not modeled.
2. **Given** the README, **When** the microphone section is read, **Then** it states:
   - the input level reference (0 dBFS at 1 kHz = the codec's full scale ≈ 90 dB SPL, 80–100, low), with the host-gain arithmetic;
   - the input high-pass and how it thins the sound;
   - the hiss level, its offset and switch, and the advice to turn it down or off when the host's microphone already carries a floor at or above about −65 dBFS;
   - that the default voice activation keeps only the hang time of hiss in a silent room, and the offsets at which it stops pausing;
   - the clip points, and that loud material should use a host rate of at least 16 kHz;
   - that a host should set a per-session seed when recordings from different sessions must not share hiss;
   - that the figures are modeled on the circuit with typical part values, not measured.
3. **Given** the registers, **When** read, **Then** they record what this feature adopted and changed:
   - **Corrected:** A-005, A-006 (topology), A-029 (the corners, the self-noise figure, the level anchor), A-022 (its noise placement re-derived), A-019 and A-028 (the detector no longer sees the host's DC), A-036 (the input is acoustic-referred; stages 1–2 non-inverting by convention; the first-clipping polarity), signal-chain rows 1–2, and the S-002 summary in the sources register (R6; R1 is the record lamp's resistor).
   - **New assumptions, one per new figure:**
     - the input high-pass;
     - the output coupling and C4/C5's tolerance;
     - the stage-2 lumping ahead of the limiter;
     - the hiss anchor and shape;
     - the hiss's distribution and per-recording streams;
     - the amplifier's added noise and the unknown codec reference noise;
     - the clip limits, softness and polarity.
   - **Seed documentation.** The seed's documentation (Rust and C) no longer says the seed has no effect.
   - **Issue template.** The hardware-evidence issue template lists the captures that would check each new figure.

### Edge Cases

- **A silent input.** Until now it gave exact zeros through the record path. With the hiss on it gives the hiss. Checks of earlier features that rely on exact silence run on the 0.11-default configuration (US3 AS1).
- **DC and very low frequencies in the input.** The couplings block DC, so a DC offset in the host's input no longer reaches the limiter, the clamp or the voice activation (it did in 0.11). Each corner's step response is part of the documented response.
- **Host rates whose band ends below 13 kHz** (6, 8 and 16 kHz). The roll-off lies above the band; its in-band effect is matched by the prototype rule (US1 AS1).
- **Input far above full scale** (for example +40 dBFS). The clipper bounds it, and the output after stage 2 is bounded by L₊ + L₋ (US2 AS2).
- **Non-finite or subnormal input samples.** They become 0.0 at the engine's input, as today, so such a sample carries the hiss alone. The hiss generator's state never depends on the input's values.
- **Loud input, then silence, with the hiss off.** The stages' state reaches exactly zero with no subnormal output (spec 001's rule).
- **The sensitivity setting.** It does not reach stages 1–2: on the device it acts in the firmware after the codec (A-033). It changes neither the hiss nor the band nor the clip points, but its threshold's margin to the hiss is reported (US3 AS6).
- **The hiss offset at its top** (+12 dB, about −50 dBFS). Voice activation stops pausing in a silent room at levels 4–5. This is documented, not refused. *(Restated at implementation, 2026-10-16; R-07, A-022: level 5 stops pausing from about +9 dB; level 4 still pauses at +12 dB at five of the seven rates and stops from about +11.9 dB at 44.1 and 96 kHz, after the review's keying change.)*
- **A host microphone with its own floor.** Gaining up a phone microphone brings its self-noise and the room to a level comparable to the hiss. The README advises lowering the offset or switching stage 1 off.
- **A reset or reconfigure between recordings.** The recording counter keeps counting, so the next recording's hiss is new. A new engine starts at counter 0 again, so recordings from different sessions share hiss unless the host sets a per-session seed (documented in the README).
- **Both new stages on, the hiss on, the limiter bypassed.** The hiss still reaches the codec input filter and the clamp. Nothing assumes the limiter is present.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001: Order.** The record path MUST run stage 1 (the capsule), then stage 2 (the amplifier), then the existing limiter and the stages after it. Both the one-pass and the record path MUST include them.
- **FR-002: Input level reference.** With the new stages on, 0 dBFS at the engine's input MUST be the 1 kHz level that reaches the codec's full scale: the stages are normalized to unity gain at 1 kHz. The README and the registers MUST state:
  - that this corresponds to about 90 dB SPL at the microphone (80–100, low confidence, A-029);
  - the arithmetic for a host's microphone gain;
  - that the limiter's knee, the clip points and the voice-activation thresholds are input-referred one to one at 1 kHz only.
- **FR-003: The hiss.**
  - **Source.** Stage 1 MUST add Gaussian noise (to at least ±5σ) representing the capsule's self-noise.
  - **Default level.** It MUST be documented as about −62 dBFS (RMS, AES17) in the 300–3000 Hz band at stage 2's output (Clarifications), together with its 1 kHz third-octave level, its derivation and its credible range (about −66 … −53 dBFS through the closed level chain; restated 2026-10-16, B2, from the plan review's −70 … −54).
  - **Offset.** It MUST have a level offset of −18 to +12 dB, default 0. Values outside the range are refused with the existing invalid-setting status.
  - **Generator.** It MUST be generated deterministically from the engine's seed and the recording counter (FR-009), with no platform math function. The stream MUST be specific to this stage, with a period exceeding the longest recording at 96 kHz, and seed 0 MUST NOT degenerate it. The work per sample MUST be bounded: every rejection loop is capped, and the cap is registered (plan review).
- **FR-004: The capsule's response.** Flat (a new assumption, low confidence). Stage 1 consists of the hiss until capsule data gives the response content.
- **FR-005: The amplifier's band.** Stage 2 MUST apply three first-order corners, in the device's order around the clipper (FR-006):
  - the **input coupling high-pass**: C1 into R2 and the amplifier's virtual ground, about 818 Hz (665–985; bounded above by about 1.0 kHz);
  - the **output coupling**: the series loop C4–R7–C5–R10, about 77 Hz;
  - the **roll-off**: C10 across R15, about 12.9 kHz.

  It MUST normalize the gain to unity at 1 kHz. The corners MUST come from a stated circuit model of S-002's values and DC voltages, as a joint fit of three first-order corners to the transfer from the capsule's current to the codec input amplifier's output, printed by the design script. The input corner MUST be cross-checked by a circuit simulation before the golden bless. The output coupling and the roll-off sit after the limiter's attenuator on the device; they MUST be documented as lumped ahead of it (a new assumption).
- **FR-006: The amplifier's clipping.**
  - **Placement.** Between the input high-pass and the output coupling, stage 2 MUST apply a memoryless clipper with a soft onset.
  - **Limits.** Negative engine samples are limited first, at L₋ ≈ +9 dBFS (+7 … +10), and positive samples at L₊ ≈ +17 dBFS (+16 … +17.5). These are dBFS at the engine input at 1 kHz, from S-002's values and DC voltages through a circuit simulation, independent of the limiter's knee setting. Each polarity's soft onset, with its own knee, lies a documented distance below its limit, and the clipper is exactly linear below the onset. *(Restated 2026-10-16, B1: the limits were +7 (+5 … +9) and +16.5 (+16 … +17.5) from the DC voltages alone, with one knee; the circuit simulation's transient sweep is a gate before the bless, the model within ±1 dB at every level.)*
  - **Bounds.** The clipper's output MUST never exceed its limits, and the signal after stage 2 MUST never exceed L₊ + L₋.
- **FR-007: Switches.** Each new stage MUST be bypassable on its own; a bypassed stage passes its input unchanged. Stage 1 on means the hiss is on, at the offset's level.
- **FR-008: Observation points.** The signal after stage 1 and after stage 2 MUST each be selectable as an output observation point, with latency 0. No earlier observation point changes its meaning or its latency: `AfterAgc` stays the signal after stage 3, which now has stages 1–2 upstream when they run. Stage 2's 1 kHz group delay MUST be reported per host rate.
- **FR-009: State, streams and equivalence.**
  - **Rest.** The new stages' filters MUST start at rest after construction, reset, reconfigure and the start of each recording.
  - **Counter.** A recording counter MUST start at 0 at construction and never rewind. `reset` and `reconfigure` keep it. It advances by one at each successful recording start (a refused start consumes nothing). Each recording's hiss MUST be the stream for (seed, counter), from its beginning. The one-pass MUST use the stream for the counter's current value (the next recording's), from construction, `reset` or `reconfigure`.
  - **Dependence.** A recording's hiss MUST depend only on the seed, the counter, the host rate and the number of host samples processed since its start. A zero-length call processes none.
  - **The stop.** The stop's flush and frame completion (spec 006 FR-005, spec 008 FR-013) MUST feed zero-valued input through stages 1–2; "silence" in the stop report means zero input, not zero output.
  - **Equivalence.** The first recording after construction, reset or reconfigure MUST equal the one-pass under the existing coordinates.
- **FR-010: Bit-identity with the stages bypassed.**
  - With both new stages bypassed, every observation point MUST be bit-identical to version 0.11 for every configuration.
  - Every pre-012 harness configuration, including the earlier golden files' default entries and the C smoke loop's earlier cases, MUST be built on a 0.11-default configuration and stay byte-identical.
  - The library's default output changes (stages 1–2 on; a DC offset no longer passes), and the CHANGELOG MUST record it as an output change.
  - Figures quoted for "the default engine" in the README and the registers MUST be restated or labelled "0.11 default".
- **FR-011: Determinism, real-time safety and cost.**
  - The new stages MUST be deterministic across platforms and block partitions, allocation-free per block, and bounded in per-sample work.
  - Cost is measured per host sample at 48 kHz: speaker route, codec bypassed, release build, timing tests one at a time; in one build each side the fastest of 45 strictly alternating runs, across builds the fastest of nine alternating rounds. *(Amended at implementation, 2026-10-16, the user's decision: the in-build comparison is the fastest of **45 strictly alternating runs** per side, because the fastest of nine was run-order sensitive on the development machine; the cross-build "on" figure is held to the same 10 % as corroboration. Research R-09 › As measured.)*
  - **On against bypassed, in one build**, on −20 dBFS noise: it MUST rise by at most **10 %**. Asserted on the development machine; reported on the CI runner under a 25 % safety ceiling. The spread over levels and offsets is reported.
  - **Bypassed against the 0.11 build** (a cross-build A/B on spec 009's `output_cost` stimulus): it MUST rise by at most **3 %**. Checked on the development machine from the A/B script's print and recorded (research R-09; the script prints, no test asserts it), and run once on the CI runner through the PR's `ab-timing` label before the merge.
  - **On against the 0.11 build:** at most 10 % as corroboration of the in-build figure (checked when the A/B is run, not asserted by a test; it is not a budget of its own, because stage 2 changes the level reaching later stages whose cost depends on level). The limiter MUST be uncharged at the end of that run.
- **FR-012: Host interfaces.** The new switches, the offset and the observation points MUST be available to C callers as well as Rust callers.
  - **C structure.** The settings structure MUST grow from 112 bytes (8-byte aligned) with the new fields at recorded offsets and explicit reserved tails. 0.11's size is refused with the existing status.
  - **Taps.** The new observation points take the next C tap values.
  - **Stored form.** It is unchanged (version 2). Stages 1–2 precede the memory, so reconfiguring them needs no empty memory.
  - **Version.** The version MUST be 0.12.0.
- **FR-013: Records.** The README's microphone section, the stage manifest (stages 1–2: assumed, model version 0.12.0), the registers listed in US4 AS3, the signal-chain table, the CHANGELOG and the hardware-evidence issue template MUST state the behavior, its figures, its evidence and its confidence. Every literal in the implementation MUST trace to a source or an assumption.
- **FR-014: Golden file.** The feature MUST have its own golden file:
  - **Coverage:** the new stages at the seven host rates, with stage 1 on and off, on the default and the stages-only configurations; a second recording (counter 1); a non-zero offset; a tone that reaches both clip limits;
  - **Pinning:** the hiss sequences and the band pinned by hash;
  - **Blessing:** blessed once, after the post-implementation reviews.

### Out of Scope

- **Noise reduction** (stage 6): claimed by one secondary account only; to be researched before any model.
- **The clock and the power supply** (stages 14–15): may not matter audibly; to be researched first. This includes any tones from the switching converter in the idle noise.
- **Handling noise, wind, the capsule's acoustic overload** at very high sound levels, **directivity** (omnidirectional assumed) and the case's acoustics around the capsule.
- **The amplifier's distortion below clipping:** negligible at the amplifier's high loop gain (plan review).
- **The output coupling's shift under limiting** (159 Hz while the limiter's shunt conducts).
- **The codec's idle-channel noise** (A-030): not added; it lies below the hiss.
- **The microphone-sensitivity setting** (A-022, A-033).
- **Unit-to-unit variation** of the capsule, the capacitors (C4 and C5 vary by +80/−20 %) and the transistors. Ranges are documented; one value is modeled.
- **An SPL-calibrated input setting**: it can be added later without a breaking change.

### Key Entities

- **Microphone capsule (stage 1):** the electret element. Its hiss, at a documented level plus an offset, with a switch; its response is flat.
- **Microphone amplifier (stage 2):** the input high-pass, the gain normalized at 1 kHz, the asymmetric soft clipper, the output coupling and the roll-off, with a switch.
- **Hiss stream:** deterministic Gaussian noise for (seed, recording counter), restarted at the points in FR-009.
- **Recording counter:** counts successful recording starts since construction; never rewinds.
- **Observation points:** after stage 1 (input plus unshaped hiss) and after stage 2.
- **Configurations:** the stages-only configuration (stages 1–2 on, everything later bypassed) and the 0.11-default configuration (stages 1–2 bypassed) on which every pre-012 configuration is built.
- **Level reference:** 0 dBFS at 1 kHz is the codec's full scale, about 90 dB SPL (80–100, low).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001: The hiss.** At the seven host rates and offset 0:
  - its 300–3000 Hz level after stage 2 is within ±0.5 dB of the documented figure;
  - its third-octave shape (315–2500 Hz) is within ±1 dB;
  - each offset moves it by its value within ±0.5 dB.
- **SC-002: The band.** At the seven host rates, the response after stage 2 matches the documented prototype within ±0.1 dB at every third-octave centre where the prototype is above −40 dB, up to the lower of 20 kHz and 0.45 × the rate.
- **SC-003: Clipping.**
  - The clipper's onsets and limits are within ±0.5 dB of the documented figures for each polarity.
  - The clipper never exceeds its limits.
  - The output after stage 2 never exceeds L₊ + L₋, for any input, including +40 dBFS and non-sinusoidal signals.
- **SC-004: Bit-identity.** With both new stages bypassed, every earlier golden file and guard is unmoved and every observation point of every pre-012 configuration is bit-identical to 0.11.
- **SC-005: Determinism.**
  - Same seed, counter and input give bit-identical output on every supported platform and partition.
  - Different seeds or counters give hiss streams correlated below 5/√N at lag 0 and 6/√N over lags ±64 (US1 AS4).
- **SC-006: Cost.** Under FR-011's conditions: at most +10 % on against bypassed in one build, and at most +3 % bypassed against 0.11. The first asserted on the development machine, the second checked from the A/B script's print and recorded; both quoted from the CI runner.
- **SC-007: Records.**
  - The manifest counts 12 assumed and 3 not modeled.
  - Every new figure has a registered assumption.
  - The README states every item of US4 AS2.
  - Coverage stays at or above 80 % and within 1 point of 0.11.
- **SC-008: Equivalence and streams.** At the seven host rates with stages 1–2 on:
  - the first recording after construction, reset or reconfigure equals the one-pass bit for bit;
  - two engines with the same seed and the same call history carry bit-identical hiss in their k-th recordings, and no two recordings of one engine share a stream.
- **SC-009: The C surface.**
  - The C structure's new size and offsets are asserted, and 0.11's size is refused.
  - The new switches, the offset and the taps work from C.
  - The C smoke loop's new cases match the Rust path's pinned hashes.
- **SC-010: The voice-activation margin.** With the hiss at offset 0, voice activation pauses once and never resumes in a 60 s silent run at every sensitivity level and host rate, and the documented margin at level 5 is at least 10 dB.

## Assumptions

- **"Documented" means printed by the design script and registered.** Tests pin to the script's print, never to the engine's own output (spec 005's lesson).
- **The circuit model.** The input corner depends mainly on C1 and R2. The amplifier's open-loop gain, which depends on transistor parameters the schematic does not give, moves it within 665–985 Hz. The design script states the nodal model and the transistor figures used, and a circuit simulation with generic models cross-checks the corner. The value is low confidence; the existence of the corner is medium confidence (S-002 topology).
- **The hiss's shape.** White pressure noise at the capsule, shaped by stage 2's band. An electret's 1/f rise lies mostly below the input high-pass.
- **The amplifier's own noise** (mainly Q3, and R2's thermal noise; a floor near −73 dBFS) adds about 0.3 dB to the capsule's at the default and is folded into the figure. The codec's reference (SG) noise reaches the output with the signal's noise gain and is unknown. It is registered as an open question, with S-003's unread pages as the lead.
- **Below-clip linearity.** The amplifier is linear below its onsets; its low-level distortion is out of scope.
- **No new evidence.** No physical unit and no capture are available. Every new figure is low or medium confidence, and the records say so.
- **Version and interfaces.** The version is 0.12.0. The C settings structure grows (a breaking change for C callers, as in earlier features). New observation points are added at the end of the existing list.
