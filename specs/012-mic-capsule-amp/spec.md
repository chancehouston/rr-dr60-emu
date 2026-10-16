# Feature Specification: Microphone Capsule and Amplifier (signal-chain stages 1–2)

**Feature Branch**: `012-mic-capsule-amp`

**Created**: 2026-10-16

**Status**: Draft

**Input**: User description: "Feature 012: model signal-chain stages 1 and 2, the microphone capsule and the microphone amplifier, ahead of the record-path limiter. Today the engine's input is treated as the mic-amp output (A-036), so nothing models what happens between the sound at the microphone and the codec's input: the capsule's frequency response and self-noise, the amplifier's gain, its coupling high-pass corners (dominant ≈ 72 Hz, first order) and ≈ 13 kHz roll-off, its noise floor (the "hiss" floor), and its clipping on very loud sounds before the limiter can act. Evidence: S-002, A-005, A-006, A-029, A-030, A-036. The user wants the record path to start at the sound reaching the microphone. Each new stage must be independently bypassable and testable, deterministic (seeded noise), real-time safe, traceable to a source or a registered assumption, and every pre-012 configuration must stay bit-identical with the new stages bypassed (the earlier golden files unmoved). Out of scope: noise reduction (stage 6, to be researched deeper), clock and power (stages 14–15, to be researched deeper), handling noise and wind."

## Overview

### What the record path lacks today

The record path today starts at the limiter (stage 3). The engine treats its input as the **mic amplifier's output** at a fixed gain: 0 dBFS is the level that reaches the codec's full scale ([A-029](../../docs/hardware/assumptions.md), [A-030](../../docs/hardware/assumptions.md), [A-036](../../docs/hardware/assumptions.md)). So nothing models what happens between the sound arriving at the microphone and that point:

- **the hiss floor:** the electret capsule's self-noise, which every recording of a quiet room carries;
- **the band edges:** the amplifier's coupling high-pass, dominated by a first-order corner near 72 Hz, and the roll-off near 13 kHz;
- **clipping:** the amplifier clips on very loud sounds before the limiter can act on them.

The stage manifest lists stages 1 and 2 as not modeled.

### What this feature adds

This feature adds both stages to the record path, ahead of the limiter, so the record path starts at the sound reaching the microphone:

- **Stage 1, the microphone capsule:** the two-terminal electret RJM0016 ([S-002](../../docs/hardware/sources.md), A-005), contributing its self-noise and its response.
- **Stage 2, the microphone amplifier:**
  - the discrete differential-pair amplifier (S-002, [A-006](../../docs/hardware/assumptions.md));
  - its coupling corners and the codec input amplifier's roll-off (A-029);
  - its asymmetric clipping at its supply rails (A-029, as refined by spec 005).

### How it fits the existing record path

- **Order.** The two stages run in the device's order: capsule, then amplifier, then the existing limiter (whose attenuator sits after the amplifier on the schematic), then everything that follows today.
- **Limiter interaction.** The hiss passes through the limiter like any other sound, so a loud sound that the limiter pulls down pulls the hiss down with it.
- **Bypass.** Each stage can be bypassed on its own. With both bypassed, every tap is bit-identical to version 0.11 and every earlier golden file is unmoved.

### Evidence

The evidence is circuit-level, not a measurement. Known from the service manual: the parts, the topology and the resistor and capacitor values. **Not known:**

- the capsule's sensitivity, self-noise and response (A-005: low confidence);
- the amplifier's gain, which is the least certain number in the chain: A-029 records the sound level that reaches full scale as **80–100 dB SPL**.

So the hiss floor and the clip points are **modeled on** the circuit with typical part figures. They are documented as such, and they can be refined when a level-calibrated capture from a real unit arrives.

## Clarifications

### Session 2026-10-16 (specify)

- Q: What does the engine's input level mean once a microphone sits in front of it? → A: **Today's reference is kept** (the user's choice, option A): 0 dBFS at the input still reaches the codec's full scale, and the new stages add no gain at 1 kHz. The amplifier's uncertain absolute gain enters the engine only as documentation: the README and the registers state that full scale corresponds to **80–100 dB SPL at the microphone** (A-029, low confidence). App makers scale their own microphone input. The user asked that the hardware reviewer be consulted on this choice at the next review.
- Q: Is the capsule's hiss on by default, and at what level? → A: **On by default at the middle estimate** (the user's choice, option A): a capsule self-noise of 25–30 dB SPL against a full scale of 80–100 dB SPL puts the hiss between about −75 and −50 dBFS; the default is the middle, **about −62 dBFS in the 300–3000 Hz band** (the exact figure fixed by the plan's design script and registered as an assumption). It has its own switch and no level setting. The margin between the hiss's peaks and the voice-activation thresholds is reported, and voice activation stays paused in silence at all five sensitivity levels. The user asked that the hardware reviewer be consulted on this choice at the next review.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A quiet room sounds like the device's microphone (Priority: P1)

An app maker records a quiet room, or a soft voice in one, through the emulated recorder. Today the engine's own hiss floor is absent: a silent input gives a near-silent recording, and the only noise comes from the codec. With this feature the recording carries the device's microphone hiss, a steady broadband floor at a documented level. The capsule's response and the amplifier's band shape both the hiss and the sound: very low rumble is reduced below about 72 Hz, and content above about 13 kHz is rolled off before the limiter sees it.

**Why this priority**: The hiss floor is the most audible property of a small 1990s recorder's microphone path in quiet conditions. It is also where this device's recordings are listened to most closely, so it is the core of the feature.

**Independent Test**: With every later stage bypassed, read the signal after stage 2 at each host rate:
- feed silence and measure the hiss floor's level and spectrum;
- feed tones and an impulse and measure the band edges.

No other stage is needed.

**Acceptance Scenarios**:

1. **Given** the default configuration with stage 1's hiss on, **When** silence is processed at any supported host rate, **Then** the signal after stage 2 is a steady broadband noise:
   - its level in the speech band (300–3000 Hz) equals the documented hiss level within ±0.5 dB, measured over at least 10 s;
   - its spectrum within the band follows the documented shape within ±1 dB per third-octave band;
   - the in-band level is the same at every host rate within ±0.5 dB.
2. **Given** stage 1's hiss off and stage 2 on, **When** tones from 20 Hz to the host rate's band edge are processed, **Then** the response after stage 2:
   - is unity (0 dB ± 0.1 dB) at 1 kHz;
   - has a first-order high-pass corner at the documented frequency (72 Hz nominal) within ±5 %, together with the documented minor corners;
   - rolls off at the documented frequency (13 kHz nominal) within ±5 % at the host rates whose band reaches it.

   **Unity at 1 kHz** means the new stages add no gain at mid-band: the level an input reaches the limiter with is unchanged (Clarifications, Q1).
3. **Given** the same seed and the same input, **When** a recording is made twice, on any platform, in any block partition, **Then** the output is bit-identical. **Given** two different seeds, **Then** the two hiss sequences differ.
4. **Given** the hiss on, **When** silence or a quiet room is recorded with voice activation on, **Then** the hiss alone does not open voice activation at sensitivity levels 1–5: its peak level stays below the lowest threshold (−46 dBFS at level 5, A-022) by a documented margin.
5. **Given** the hiss on, **When** the full default engine records silence, **Then** the decoded playback carries the hiss as the speech codec reproduces it. The level is reported per host rate. It is not asserted against the hiss level, because the codec reshapes noise.
6. **Given** a loud sound that the limiter pulls down by several dB, **When** it is followed by silence, **Then** the hiss after the limiter is pulled down by the same gain and recovers as the limiter releases. The hiss sits ahead of the limiter, as on the device.

---

### User Story 2 - Very loud sounds overload the microphone amplifier (Priority: P2)

A sound loud enough to drive the amplifier past its supply rails clips there, before the limiter can act. One polarity reaches its rail before the other, because the amplifier's output swing is asymmetric about its bias point (A-029). The clipping is soft at its onset and hard at the rail, and it adds the harmonic distortion a real overload produces, which the limiter alone cannot.

**Why this priority**: Overload is the second audible behavior of the microphone path. It also bounds what reaches the limiter: today the limiter's attenuation range can, in principle, be exceeded by an input far above anything the device could see. It matters less than the hiss because it applies only to very loud input.

**Independent Test**: With every later stage bypassed and the hiss off, read the signal after stage 2. Feed 1 kHz tones and steps at rising levels, and asymmetric test signals.

**Acceptance Scenarios**:

1. **Given** a 1 kHz tone, **When** its level is raised step by step, **Then** the amplifier's output:
   - follows the input within 0.1 dB up to the documented onset of clipping for each polarity;
   - reaches the documented limit for each polarity and never exceeds it;
   - has its first polarity's onset at the documented level, within A-029's range of 7–16 dB above the limiter's knee, the second polarity's onset at its own documented level, and each within ±0.5 dB of the documented figure.
2. **Given** a tone 6 dB above the first clip onset, **When** it is processed, **Then** the total harmonic distortion after stage 2 is reported at each host rate, and the even harmonics (from the asymmetry) are present at a documented level within ±2 dB.
3. **Given** any input, however large, **When** it passes stage 2, **Then** the signal the limiter receives is bounded by the amplifier's limits. The limiter's attenuation range is therefore never exceeded by any input.
4. **Given** clipping in progress, **When** the input falls back below the onset, **Then** the output follows the input again from the next sample, with no recovery tail, unless the documented model includes one.

---

### User Story 3 - The microphone path in the whole engine (Priority: P2)

An app maker uses the engine as before: record then play, or the one-pass call. The new stages are part of the record path. Each can be bypassed on its own, its output can be read on its own, and everything else keeps its contracts: record-then-play equals the one-pass, the engine is deterministic and allocation-free, reset and reconfigure return the stages to rest, and the cost stays within budget.

**Why this priority**: The stages must fit the engine's existing guarantees, or app makers cannot rely on them. These checks are the same kind every earlier stage passed.

**Independent Test**: The existing whole-engine harnesses, run with the new stages on and off at the seven host rates: equivalence, determinism, allocation, mutation, timing and golden files.

**Acceptance Scenarios**:

1. **Given** both new stages bypassed, **When** any pre-012 configuration is processed, **Then** every tap is bit-identical to version 0.11, the earlier golden files and their guards are unmoved, and the C smoke loop's earlier hashes are unchanged.
2. **Given** the new stages on, **When** an input is recorded then played, **Then** the result equals the one-pass output under the existing coordinates. The hiss of a recording depends only on the seed and the samples fed, not on whether the one-pass or the record path ran.
3. **Given** a reset, a reconfigure, or the start of each recording, **When** processing resumes, **Then**:
   - the new stages' filters start at rest;
   - the hiss sequence restarts from the seed, so each recording's hiss is reproducible (A-037: no state carries between recordings).
4. **Given** each new stage, **When** it alone is switched off, **Then** that stage passes its input through unchanged, and the other stage is unaffected. The hiss can be switched off without switching off stage 1's response.
5. **Given** the new stages, **When** their outputs are requested as observation points, **Then** the signal after stage 1 and after stage 2 can each be read, with their latency reported.
6. **Given** the new stages on, **When** the cost per host sample at 48 kHz is measured (codec bypassed, a solo release run, the fastest of nine alternating runs), **Then** it rises by at most the documented budget against version 0.11, on the development machine and on the CI runner.
7. **Given** the new stages on, **When** the engine processes any block, **Then** it allocates nothing, and its per-sample work is bounded.

---

### User Story 4 - The microphone path is documented and traceable (Priority: P3)

A reader of the README, the stage manifest or the registers learns what stages 1 and 2 model, from which evidence, at what confidence, and what they do not model. Every figure the engine uses traces to a source or a registered assumption.

**Why this priority**: The project's fidelity and claims policies require it, but it follows the behavior it documents.

**Independent Test**: The registers cross-check, the manifest golden, the README statements test and the traceability audit.

**Acceptance Scenarios**:

1. **Given** the stage manifest, **When** stages 1 and 2 are read, **Then** each is "assumed", cites its sources and assumptions, names what it does not model, and carries model version 0.12.0. The counts become 12 assumed and 3 not modeled.
2. **Given** the README, **When** the microphone section is read, **Then** it states:
   - the hiss level and how to switch it off;
   - the band edges and the clip points;
   - the input level reference and the SPL range it corresponds to;
   - that the figures are modeled on the circuit with typical part values, not measured.
3. **Given** the assumption register, **When** A-005, A-006 and A-029 are read, **Then** they record what this feature adopted. Each new figure has its own registered assumption, with confidence and with how a capture from a real unit would check it. The hardware-evidence issue template lists those captures.

### Edge Cases

- **A silent input.** Today it gives exact zeros through the record path. With the hiss on, it gives the hiss. Checks of earlier features that rely on exact silence run with the hiss off, or with the stages bypassed.
- **DC and very low frequencies in the input.** The amplifier's coupling corners block DC, so a DC offset in the host's input no longer reaches the limiter or the clamp; today it does. The step response of each corner is part of the documented response.
- **Host rates whose band ends below 13 kHz** (6, 8 and 16 kHz). The roll-off lies above the band, so only its in-band effect is modeled. The response within each band follows the documented analog prototype within the stated tolerances.
- **Input far above full scale** (for example +40 dBFS). The amplifier bounds it (US2 AS3). The output after stage 2 is the clipped waveform, and nothing downstream sees more.
- **Non-finite input samples.** Handled as the engine handles them today at its first stage. The hiss generator's state is never affected by the input's values.
- **Very long recordings** (60 minutes). The hiss neither repeats audibly within a recording (the noise sequence's period exceeds any recording) nor drifts in level.
- **The sensitivity setting.** It does not reach stages 1–2: on the device it acts in the firmware after the codec (A-033). Changing it changes neither the hiss nor the clip points.
- **The limiter's polarity rule.** A-036's sensed polarity (positive samples at the limiter's input) is unchanged: the new stages are non-inverting as modeled. The acoustic polarity of the capsule and the amplifier stays unknown and documented as such.
- **Both new stages on, the hiss on, and the limiter bypassed.** The hiss still reaches the codec input filter and the clamp. Nothing assumes the limiter is present.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001: Order.** The record path MUST run the microphone capsule (stage 1), then the microphone amplifier (stage 2), then the existing limiter (stage 3) and the stages after it. Both the one-pass and the record path MUST include them.
- **FR-002: Input level reference.** With the new stages on, 0 dBFS at the engine's input MUST still be the level that reaches the codec's full scale through unity mid-band gain (Clarifications, Q1). The README and the registers MUST state the sound level at the microphone that this corresponds to, as the range A-029 records (80–100 dB SPL), low confidence.
- **FR-003: The hiss.**
  - **Source.** Stage 1 MUST add a steady broadband noise representing the capsule's self-noise (the amplifier's own noise being negligible beside it, A-029).
  - **Level and shape.** Its in-band level and its spectral shape MUST be documented figures, the same in the speech band at every host rate, generated from the engine's seed by a deterministic generator with no platform math function.
  - **Default.** It MUST be on by default, at the middle of the estimated range: about −62 dBFS in the 300–3000 Hz band, the exact figure documented and registered (Clarifications, Q2). There is no level setting.
  - **Switch.** The hiss MUST be switchable off independently of the rest of stage 1.
- **FR-004: The capsule's response.** Stage 1 MUST apply the capsule's documented response. Without capsule data, the default is flat in the device band (A-005, low confidence), and the documented response states that.
- **FR-005: The amplifier's band.** Stage 2 MUST apply:
  - the coupling high-pass corners of A-029 (the dominant first-order corner near 72 Hz and the minor corners);
  - the roll-off near 13 kHz;
  - unity gain at 1 kHz under FR-002's reference.

  The corner frequencies MUST be computed from the S-002 component values and documented.
- **FR-006: The amplifier's clipping.** Stage 2 MUST clip at the amplifier's documented limits, which are asymmetric (one polarity before the other), with a soft onset, within A-029's range of 7–16 dB above the limiter's knee for the first polarity. The output after stage 2 MUST never exceed either limit.
- **FR-007: Bypass and switches.** Each new stage MUST be bypassable on its own. A bypassed stage passes its input unchanged. The hiss MUST have its own switch (FR-003).
- **FR-008: Observation points.** The signal after stage 1 and after stage 2 MUST each be selectable as an output observation point, with its latency reported. No earlier observation point changes its meaning.
- **FR-009: State and reproducibility.**
  - The new stages' state MUST start at rest after construction, reset, reconfigure and the start of each recording.
  - The hiss sequence MUST restart from the seed at those points.
  - A recording's hiss MUST depend only on the seed and the number of samples processed since that restart.
  - Record-then-play MUST equal the one-pass under the existing coordinates.
- **FR-010: Bit-identity with the stages bypassed.** With both new stages bypassed, every observation point MUST be bit-identical to version 0.11 for every configuration, and the earlier golden files and their guards MUST stay unmoved.
- **FR-011: Determinism, real-time safety and cost.**
  - The new stages MUST be deterministic across platforms and block partitions and allocation-free per block, with bounded per-sample work.
  - With both on, the cost per host sample at 48 kHz (codec bypassed) MUST rise by at most 10 % against version 0.11, measured as each side's fastest of nine alternating solo release runs. The figure is reported on the development machine and on the CI runner.
- **FR-012: Host interfaces.** The new switches and observation points MUST be available to C callers as well as Rust callers. The version MUST be 0.12.0.
- **FR-013: Records.** The README's microphone section, the stage manifest (stages 1–2: assumed, model version 0.12.0), the assumption register (A-005, A-006, A-029 updated; a new assumption for each new figure), the signal-chain table, the CHANGELOG and the hardware-evidence issue template MUST state the behavior, its figures, its evidence and its confidence. Every literal in the implementation MUST trace to a source or an assumption.
- **FR-014: Golden file.** The feature MUST have its own golden file covering the new stages at the seven host rates, with the hiss on and off, blessed once after the reviews.

### Out of Scope

- **Noise reduction** (stage 6): claimed by one secondary account only; to be researched before any model.
- **The clock and the power supply** (stages 14–15): may not matter audibly; to be researched first.
- **Handling noise, wind, and the capsule's own acoustic overload** at very high sound levels.
- **Directivity:** the capsule is assumed omnidirectional.
- **The case's acoustics** around the capsule.
- **The microphone-sensitivity setting:** a firmware function after the codec, already covered by A-022 and A-033.
- **Unit-to-unit variation** of the capsule and the transistors.

### Key Entities

- **Microphone capsule (stage 1):** the electret element. Its response (flat by default) and its self-noise (the hiss), with a switch for the stage and a separate switch for the hiss.
- **Microphone amplifier (stage 2):** the discrete amplifier and the coupling network up to the codec input. Its band (high-pass corners, 13 kHz roll-off), its unity mid-band gain under FR-002's reference, and its asymmetric clip limits, with a switch.
- **Hiss sequence:** the deterministic noise derived from the engine's seed, restarted at the points in FR-009.
- **Observation points:** after stage 1 and after stage 2.
- **Level reference:** the statement of what an input level means, and the SPL range that reaches full scale (FR-002).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001: The hiss.** At the seven host rates, its level in the 300–3000 Hz band after stage 2 is within ±0.5 dB of the documented figure, and its third-octave shape is within ±1 dB of the documented shape.
- **SC-002: The band.** At the seven host rates, the response after stage 2 is 0 ± 0.1 dB at 1 kHz, and the documented corners are within ±5 % where each rate's band reaches them.
- **SC-003: Clipping.** The clip onsets and limits are within ±0.5 dB of the documented figures for each polarity at every host rate. No sample after stage 2 exceeds a limit, for any input, including +40 dBFS and non-sinusoidal signals.
- **SC-004: Bit-identity.** With both new stages bypassed, every earlier golden file and guard is unmoved, and every observation point of every pre-012 configuration is bit-identical to 0.11.
- **SC-005: Determinism.** Same seed and input give bit-identical output on every supported platform and partition; different seeds give different hiss.
- **SC-006: Cost.** At most +10 % per host sample at 48 kHz with both stages on (codec bypassed), on the development machine and the CI runner.
- **SC-007: Records.**
  - The manifest counts 12 assumed and 3 not modeled.
  - Every new figure has a registered assumption.
  - The README states the level reference and the SPL range, the hiss level and switch, the band and the clip points.
  - Coverage stays at or above 80 % and within 1 point of 0.11.

## Assumptions

- **Unity mid-band gain.** The new stages are designed to give unity gain at 1 kHz under FR-002's reference, so the levels at which the limiter, the clamp and voice activation act keep their present meaning. The amplifier's absolute gain is represented by the level reference, not by a gain applied to the input (Clarifications, Q1).
- **The hiss's shape.** The capsule's self-noise is modeled as broadband noise shaped by the amplifier's band. An electret's self-noise is roughly flat in the speech band. The low-frequency rise of real electret noise is largely removed by the 72 Hz corner.
- **The amplifier's own noise** is negligible beside the capsule's (A-029) and is not modeled separately.
- **The clip points** are chosen within A-029's range by a design script that prints every figure, as for earlier stages. They are modeled on the circuit's supply and bias voltages (S-002's measured DC voltages), not measured.
- **The capsule's response** is flat in the device band until capsule data or a capture says otherwise. The device's band (about 225–2550 Hz after the codec filter) hides most of an electret's response deviations.
- **No new evidence.** No physical unit and no capture is available; every new figure is low or medium confidence, and the records say so.
- **Version and interfaces.**
  - The version is 0.12.0.
  - The C settings structure grows for the new switches, a breaking change for C callers, as in earlier features.
  - New observation points are added at the end of the existing list.
