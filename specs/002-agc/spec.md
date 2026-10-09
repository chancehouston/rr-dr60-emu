# Feature Specification: Automatic Gain Control (AGC) on the Record Path

**Feature Branch**: `002-agc`

**Created**: 2026-10-09

**Status**: Draft

**Input**: User description: "Automatic gain control (AGC) for the RR-DR60 record path, signal-chain stage 3 (A-007). It sits before the anti-alias filter + ADC (stage 4) and raises quiet input and lowers loud input toward a target level, producing the audible "pumping" and the rise of background noise in quiet passages. Behaviors to specify with measurable tolerances: target output level for steady tones; maximum gain and maximum attenuation; steady-state input-vs-output level curve; attack time and release time measured with level-step tones (e.g. 0 dB to -30 dB to 0 dB); behavior on silence (gain stays at maximum or holds); no added distortion above a stated limit for steady tones. The stage can be bypassed, can be tapped ("after AGC"), defaults to assumed device values, is deterministic for any block split, and does no allocation during processing. Each numeric default must cite a source or a new registered assumption (target level, gain range, attack, release, detector type), all low confidence until real-unit captures exist. Tests use synthetic stimuli with other stages bypassed. Out of scope: mic capsule, preamp hiss, VAS, noise reduction, codec, quantization (A-003), speaker."

## Overview

This feature adds signal-chain stage 3, the record-path automatic gain control, to the pipeline built in [spec 001](../001-pipeline-skeleton/spec.md). The RR-DR60 is claimed to have auto-gain (S-004, A-007), as is standard for dictation recorders. An AGC raises quiet input and lowers loud input toward a target level. Its audible side effects are the character this stage contributes: level "pumping" after loud sounds, and background noise that rises during quiet passages.

There is no physical unit and no reference recording, so every numeric default here is a registered, low-confidence assumption (A-017 to A-020). The spec fixes **which** properties are measured and **how**. The values can be revised as evidence arrives, without changing the shape of the spec.

Terms used in this spec (others are defined in spec 001):

- **Level**: the RMS level of a signal in dBFS, using the convention that a sine wave with peak amplitude 1.0 is 0 dBFS (AES17). A sine with peak 0.316 is therefore −10 dBFS.
- **Gain**: output level minus input level, in dB.
- **Target level**: the AGC's reference level. A steady input at the target level comes out at the target level (gain 0 dB). Inputs above or below it come out only slightly above or below it (see regulation slope).
- **Maximum gain / maximum attenuation**: the limits of the AGC's gain range: at most +G<sub>max</sub> dB, at least −A<sub>max</sub> dB.
- **Regulation slope**: inside the regulated range, the output level changes by 1 dB for every 10 dB change of input level (10:1 compression, A-017). The **regulation line** is output = target + (input − target) / 10.
- **Regulated range**: the input levels for which the gain is between its limits, from (target − G<sub>max</sub>/0.9) to (target + A<sub>max</sub>/0.9). With the defaults, that is about −54.4 to +12.2 dBFS.
- **Static curve**: the steady-state output level as a function of input level for a steady 1 kHz tone. All static targets in this spec apply to sine tones. The AGC regulates peaks (A-019), so noise and speech, which have higher peaks for the same RMS, come out at a lower RMS level than the regulation line.
- **Settling band**: 2/27 (7.4 %) of the output level's total dB excursion after a step. The excursion is the difference between the output level just after the step and the final steady output level. For the default step stimulus the excursion is 27 dB, so the band is 2 dB. The band scales with the step, so attack and release times do not depend on step size.
- **Attack time**: after an upward level step, the time from the step until the output level stays within the settling band of its final steady value.
- **Release time**: after a downward level step, the time from the step until the output level stays within the settling band of its final steady value.
- **Step stimulus**: a 1 kHz tone that switches between −40 dBFS and −10 dBFS (a 30 dB step), starting at −40 dBFS for at least 0.5 s, then −10 dBFS for at least 20× the attack time (minimum 0.2 s), then −40 dBFS for at least 1.5× the release time plus 0.5 s, so that the output settles after each step. FR-012 gives the step for non-default settings.
- **Gain trajectory shape**: during attack and release, the gain in dB approaches its final value approximately exponentially (a single time constant in the dB domain): fast at first, then slower (A-018).
- **AGC-isolated configuration**: AGC on, record band-limit stage (4) and playback band-limit stage (10) both bypassed. All AGC measurements use it, unless stated otherwise (Principle VII).

## Clarifications

### Session 2026-10-09

- Q: During long quiet stretches, should the gain keep rising to maximum, hold briefly first, or freeze below a silence threshold? → A: It keeps rising at the release rate up to maximum gain. There is no hold and no noise gate (FR-007, A-019).
- Q: When a pipeline is created or reset, should the AGC start at maximum gain or at unity gain? → A: At maximum gain, as if after silence, with no setting to change it. A loud first sound overshoots for about one attack time (Edge Cases, FR-013, A-019).
- Q: After a loud sound stops, should the gain recover at a steady dB-per-second rate, or fast at first and then slower (capacitor-like)? → A: Fast at first, then slower. The harness checks both the release time and a midpoint: at 25 % of the release time, 35–65 % of the dB change has been recovered (FR-006, A-018). After the spec review this was made precise: the gain approaches its final value exponentially in the dB domain, for attack as well as release.
- Q: Within its working range, should every steady input come out at nearly the same level, or should louder inputs stay somewhat louder? → A: Gentle slope: output rises 1 dB per 10 dB of input (10:1), passing through the target at gain 0 dB, checked to ±1 dB (FR-004, A-017).
- Q: Should the AGC's brief overshoot above full scale be left as is, hard-clipped, or soft-limited? → A: Left as is, without clipping or limiting (as in spec 001). The documentation warns hosts to limit or clip it themselves (Edge Cases, A-003).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Hear the RR-DR60's auto-gain character (Priority: P1)

An app developer creates a pipeline with default settings and feeds it audio that mixes quiet and loud passages. Quiet passages come out louder and loud passages come out quieter. After a loud sound, the level dips and then recovers ("pumping"). During long pauses the background noise rises. Nothing needs to be configured: the AGC is on by default, because the device's AGC is assumed to be always active (A-020).

**Why this priority**: AGC is the next audible stage of the record path, and it sits in front of everything that follows it (VAS thresholds act on the post-AGC level). On its own, without the other stories, it is a usable increment: the default pipeline now sounds more like the device.

**Independent Test**: In the AGC-isolated configuration at 48 kHz, process the static-curve tones and the step stimulus, and check FR-004 to FR-008. Then create a default pipeline and check acceptance scenario 5. This story includes the minimal level measurement it needs, so it does not depend on the harness work in Story 3.

**Acceptance Scenarios**:

1. **Given** the AGC-isolated configuration with default settings, **When** steady 1 kHz tones at −40, −10 and 0 dBFS are processed, **Then** the steady-state output levels are −13, −10 and −9 dBFS, each ± 1 dB (the regulation line, A-017).
2. **Given** the same configuration, **When** a steady 1 kHz tone at −60 dBFS is processed, **Then** the steady-state output level is −20 dBFS ± 1 dB (maximum gain of +40 dB, A-017).
3. **Given** the same configuration, **When** the step stimulus steps up from −40 to −10 dBFS, **Then** the attack time is 10 ms ± 2 ms (A-018). **When** it steps back down, **Then** the release time is 1.0 s ± 0.2 s (A-018).
4. **Given** the same configuration, **When** only digital silence has ever been processed, **Then** every output sample is exactly zero.
5. **Given** a default pipeline (all stages on, tap after the playback stage), **When** a 1 kHz tone at −40 dBFS is processed, **Then** the steady-state output level is −13 dBFS ± 1.2 dB. This combines the AGC tolerance with the band-limit stages' 1 kHz gain tolerance (A-015).

---

### User Story 2 - Tune, bypass, or tap the AGC (Priority: P2)

An app developer wants to adjust the auto-gain character, or remove it. They can bypass the AGC, take the output right after it ("after AGC"), or change its target level, gain range, attack time and release time within documented limits. The same controls are available to hosts in other languages through the language-neutral interface.

**Why this priority**: The constitution requires every stage to be bypassable, tap-able and configurable (Principle V). The defaults are low-confidence guesses, so letting developers tune them is how the emulator stays useful before evidence arrives. Bypass also keeps spec 001's behavior available exactly as it was.

**Independent Test**: For each setting in FR-011, check the stated effect with the step stimulus and static-curve tones, through both the native and the language-neutral interface. Check that an AGC-bypassed pipeline reproduces spec 001's golden files bit for bit.

**Acceptance Scenarios**:

1. **Given** a pipeline with the AGC bypassed, **When** any spec 001 golden stimulus is processed in its spec 001 configuration, **Then** the output is bit-identical to the stored spec 001 golden reference.
2. **Given** a pipeline with the tap set to "after AGC", **When** any stimulus is processed, **Then** the output equals that of the AGC-isolated configuration sample for sample, and the settings of stages 4 and 10 have no effect.
3. **Given** a pipeline with the release time set to 3 s, **When** the step stimulus steps down, **Then** the release time is 3 s ± 20 %.
4. **Given** a pipeline with the target level set to −20 dBFS, **When** a 1 kHz tone at −30 dBFS is processed, **Then** the steady-state output is −21 dBFS ± 1 dB (the regulation line for a −20 dBFS target).
5. **Given** an out-of-range setting (e.g. attack time 0 ms), **When** the host creates or reconfigures the pipeline, **Then** it is rejected with an error that names the setting. Through the language-neutral interface this is an error value, and an existing pipeline is left unchanged.
6. **Given** a host that uses only the language-neutral interface, **When** it creates a pipeline with non-default AGC settings and processes the step stimulus, **Then** the output matches the native interface bit for bit.

---

### User Story 3 - The harness measures the AGC (Priority: P3)

A maintainer runs the measurement harness. It now drives the AGC with level-step tones, static-curve tones, a noise-and-burst signal and silence, at every supported host rate. It checks every tolerance in this spec, reports each result with its requirement and A-/S- IDs, and guards the AGC's output with golden files.

**Why this priority**: Principle III requires measurement tests and golden-file regression. These checks verify Stories 1 and 2, so the harness deliverable ranks after them, but its tests are written first.

**Independent Test**: Run the harness. Deliberately change the default release time by 50 % or the target level by 3 dB, and confirm that the harness fails both the relevant tolerance check and the golden check.

**Acceptance Scenarios**:

1. **Given** the pipeline meets this spec, **When** the harness runs, **Then** every AGC check passes at all six host rates and names its requirement and A-/S- IDs.
2. **Given** the noise-and-burst stimulus (FR-009), **When** it is processed in the AGC-isolated configuration, **Then** the noise rises during pauses and drops during bursts as FR-009 states.
3. **Given** any AGC stimulus processed in one block and in randomly sized blocks (including sizes 0 and 1), **When** the outputs are compared, **Then** they are bit-identical.

---

### Edge Cases

- **Start of stream and reset**: a new or reset pipeline starts at maximum gain, as if it had been silent (A-019). A loud first sound therefore overshoots for about one attack time before the gain comes down.
- **Overshoot beyond ±1.0**: the attack overshoot is passed through linearly, without clipping or limiting. For the step stimulus it is about 27 dB above the final level, peaking near +17 dBFS for about one attack time. The worst case is a loud first sound after creation, reset or long silence: it overshoots by up to the maximum gain, i.e. +40 dB (×100 amplitude) with the defaults and +60 dB (×1000) at the top of the setting range. Quantization and clipping belong to the ADC stage and are out of scope (A-003), as in spec 001. The user documentation MUST warn hosts about this overshoot and advise them to limit or clip the output themselves when converting to fixed-point formats, stating the worst case above.
- **Input above 0 dBFS**: valid AGC input, processed like any other level. The AGC sits in front of the ADC in the device, so its input is not limited to digital full scale. With the defaults, maximum attenuation is reached only by inputs above about +12.2 dBFS (FR-005). The documentation says so.
- **Long silence**: the gain rises to maximum and stays there. It does not drift, and processing hours of silence or near-silence creates no subnormal or non-finite values.
- **Non-finite input (NaN, ±Inf)**: treated as 0.0, as in spec 001. Neither the output nor the AGC's internal state become non-finite, and no reset is needed to recover.
- **DC input**: DC counts toward the AGC's level like any other signal. AC coupling in front of the AGC belongs to the mic preamp (stage 2), which is out of scope.
- **Out-of-band low frequencies**: content below 300 Hz (e.g. 50 Hz hum or handling rumble) drives the AGC even though stage 4 later removes it. The audible effect is pumping driven by inaudible energy. This is intended, because the AGC sits before the band-limit filter (signal-chain order).
- **Content above 4 kHz**: does not affect the AGC gain. The AGC acts on the device-band signal (A-019).
- **Gain limits reached**: below the regulated range the output follows the input at maximum gain (FR-005). Above it, the output follows the input at maximum attenuation.
- **Gain range of zero**: with maximum gain and maximum attenuation both set to 0 dB, the gain is pinned to unity, and the output equals the input within ±0.01 dB at every level.
- **Configuration changes while streaming**: not supported, as in spec 001 (FR-008 of 001). Reconfiguring resets the AGC to its starting state.

## Requirements *(mandatory)*

### Functional Requirements

**Placement, bypass and tap**

- **FR-001**: The pipeline MUST include an AGC stage (signal-chain stage 3) that processes the signal before the record band-limit stage (stage 4), matching the device's signal-chain order.
- **FR-002**: The AGC MUST be on by default (A-020). It MUST be bypassable. A bypassed AGC MUST pass its input through unchanged (bit-exact). With the AGC bypassed, every configuration from spec 001 MUST produce output bit-identical to spec 001's, including its golden references.
- **FR-003**: The caller MUST be able to choose the tap "after AGC", in addition to spec 001's taps ("after record stage", "after playback stage"). With the tap "after AGC", the output MUST equal that of the AGC-isolated configuration sample for sample. The default tap stays "after playback stage".

**Measurable behavior (AGC-isolated configuration, default settings, every supported host rate)**

- **FR-004**: **Regulation.** For a steady 1 kHz tone at any input level in the regulated range, at least 3 dB from either end of it, the steady-state output level MUST be within ±1 dB of the regulation line, output = target + (input − target) / 10. Defaults: target −10 dBFS, slope 10:1, maximum gain +40 dB, maximum attenuation 20 dB, so the regulated range is about −54.4 to +12.2 dBFS (A-017). The slope is fixed (not a setting).
- **FR-005**: **Gain limits.** For a steady 1 kHz tone at least 3 dB below the regulated range, the steady-state gain MUST be G<sub>max</sub> ± 1 dB. At least 3 dB above it, the gain MUST be −A<sub>max</sub> ± 1 dB (A-017). The static curve MUST be checked from 10 dB below the lower end of the regulated range to 10 dB above its upper end, in steps of at most 5 dB (about −64.4 to +22.2 dBFS with the defaults). Inputs above 0 dBFS are valid AGC input (Edge Cases).
- **FR-006**: **Timing.** With the step stimulus, the attack time MUST be 10 ms ± 2 ms and the release time MUST be 1.0 s ± 0.2 s (A-018). **Shape:** during both attack and release, the gain in dB MUST approach its final value approximately exponentially (single time constant in the dB domain, A-018). This is verified on release: at 25 % of the measured release time after the downward step, the output level MUST have recovered 35–65 % of its total dB excursion (a steady dB-per-second recovery would reach only about 23 %, an exponential one about 48 %). Output level over time is measured with a time resolution of at most min(2 ms, attack time / 4) for attack and at most min(20 ms, release time / 20) for release. The measurement method (e.g. the sample-by-sample gain at the 8 kHz host rate, where the rate conversion boundary is the identity) is a plan decision.
- **FR-007**: **Silence.** During silence or near-silence, the gain MUST rise toward maximum gain at the release rate and stay there. There is no hold period and no noise gate or gain freeze below any threshold, so background noise is boosted up to maximum gain (A-019). Digital silence in MUST produce exactly zero out.
- **FR-008**: **Frequency independence and distortion.** For steady tones at 300, 500, 1000, 2000, 2667 and 3400 Hz, at input levels of −30, −10 and 0 dBFS, with the 2000 Hz and 2667 Hz tones each tested at 4 or more different starting phases (frequencies that fall on few samples per cycle at the device rate, where a peak detector is most phase-sensitive), the steady-state output level MUST be within ±0.5 dB of the 1 kHz value, and total harmonic distortion MUST be at most 1 % (−40 dB).
- **FR-009**: **Noise rise in pauses.** Stimulus: white noise band-limited to 300–3400 Hz at −70 dBFS RMS throughout (its peaks read about 8–9 dB higher, still below the regulated range), plus 1 kHz bursts at −10 dBFS, each 1 s on and 4 s off, from a fixed seed. In the last 1 s of each pause, the output noise level MUST be −30 dBFS ± 2 dB (input noise plus maximum gain). In the first 50 ms after each burst ends, the output noise level MUST be at least 20 dB below that.
- **FR-010**: **Latency.** The AGC MUST add no latency: it does not look ahead. The pipeline's reported latency MUST be the same with the AGC on or bypassed.

**Settings**

- **FR-011**: The caller MUST be able to set the following when creating or reconfiguring the pipeline. Values outside these ranges MUST be rejected with an error that names the setting.

  | Setting | Default | Valid range |
  |---|---|---|
  | AGC on / bypassed | on (A-020) | — |
  | Target level | −10 dBFS (A-017) | −30 to 0 dBFS |
  | Maximum gain | +40 dB (A-017) | 0 to +60 dB |
  | Maximum attenuation | 20 dB (A-017) | 0 to 40 dB |
  | Attack time | 10 ms (A-018) | 1 ms to 100 ms |
  | Release time | 1.0 s (A-018) | 50 ms to 10 s |

- **FR-012**: For non-default settings, FR-004, FR-005 and FR-006 MUST hold with the defaults replaced by the configured values. Timing tolerances are ±20 % or ±1 ms, whichever is larger. The harness MUST check the defaults, and the minimum and maximum of each setting with the other settings at default. Timing checks use the −40 ↔ −10 dBFS step when both ends lie at least 3 dB inside the regulated range. Otherwise they use a step centered in the regulated range, 6 dB smaller than its width and at most 30 dB. They are skipped when the regulated range is narrower than 12 dB. Any combination of valid values is allowed, including a release time shorter than the attack time.

**Determinism, real-time safety and interface**

- **FR-013**: Spec 001's determinism, real-time safety and reset requirements (001 FR-014 to FR-017) MUST hold with the AGC on, for every valid setting. That means bit-identical output on every supported platform and for every block partition, no allocation, locks or I/O while processing, and a reset that returns the AGC to its starting state (maximum gain, A-019) without allocating. A new, reset or reconfigured pipeline MUST start at the configured maximum gain. There is no setting to choose a different starting gain.
- **FR-014**: All AGC settings MUST be available through the language-neutral interface, with output bit-identical to the native interface. Invalid AGC settings MUST be reported as error values, following 001 FR-024.

**Traceability and harness**

- **FR-015**: Every default value MUST cite its A-/S- ID in the spec, code and tests. The following are emulator engineering targets, not device properties, and are labeled as such: the ±1 dB regulation tolerance (FR-004); the ±1 dB gain-limit tolerance and the 3 dB knee exclusion (FR-005); the sweep span (±10 dB beyond the regulated range) and 5 dB grid (FR-005); the step stimulus levels and hold durations, and the 3 dB / 6 dB / 30 dB / 12 dB step-placement rules (definitions, FR-012); the ±2 ms and ±0.2 s timing tolerances, the ±20 % / ±1 ms tolerance for non-default settings, the time-resolution rules, and the 25 % / 35–65 % shape check (FR-006, FR-012); the test frequencies, levels and phase count, and the ±0.5 dB and 1 % limits (FR-008); the FR-009 stimulus and its ±2 dB / 20 dB / 50 ms limits; the setting ranges (FR-011); the ±0.01 dB unity-gain tolerance (Edge Cases); the 2/27 settling band (definitions); and the 5 minutes in SC-001.
- **FR-016**: The measurement harness MUST check FR-002 to FR-012, FR-014, FR-018 and the edge cases above, with each result reporting its measured value, tolerance, requirement and A-/S- IDs (as 001 FR-020).
- **FR-017**: The harness MUST add new golden-file checks for the AGC-isolated configuration and for the new default configuration (AGC on), using small, seeded stimuli that include the step stimulus and the FR-009 noise-and-burst stimulus. All spec 001 golden references MUST stay unchanged: none are re-blessed. The change of the default pipeline's output MUST be recorded in the CHANGELOG.
- **FR-018**: All spec 001 acceptance scenarios, tolerance checks, edge cases and golden-file configurations MUST be evaluated with the AGC bypassed, and are otherwise unchanged. Where spec 001 says "default pipeline" or "default settings", this means spec 001's defaults with the AGC bypassed.

### Out of Scope

Microphone (stage 1), mic preamp hiss, gain, clipping and AC coupling (stage 2), VAS (stage 5), noise reduction (stage 6), speech encoder/decoder (stages 7, 9), PCM quantization and companding (A-003), volume / power amp, speaker and earphone path (stages 11–13), clock deviation (A-011), battery effects (A-013), a user-facing "mic sensitivity" switch (no evidence one exists), AGC look-ahead, and changing AGC settings while streaming.

### Key Entities

- **AGC stage**: signal-chain stage 3. Holds its settings and its current gain. Has a bypass flag and a measurable static curve and timing defined by this spec.
- **AGC settings**: on/bypassed, target level, maximum gain, maximum attenuation, attack time, release time. They extend spec 001's pipeline settings, and every default cites an A-ID.
- **Tap point**: extended with "after AGC".
- **Measurement result** and **golden reference**: as in spec 001, with new AGC stimuli: static-curve tones, the step stimulus and the noise-and-burst stimulus.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An app developer can bypass the AGC, or change its release time, in under 5 minutes using only the project's documentation.
- **SC-002**: 100 % of the AGC tolerance checks (FR-004 to FR-010, FR-012) pass at all six supported host rates.
- **SC-003**: With the AGC bypassed, spec 001's golden references match with zero differences. The new AGC golden references pass unchanged on all eight platform targets from 001 FR-014, and across at least 100 random block partitions of each AGC golden stimulus.
- **SC-004**: Zero memory allocations are observed while processing 1000+ blocks of varying size with the AGC on, at default and at extreme settings.
- **SC-005**: The AGC adds 0 samples of latency, so default-configuration latency stays at or below spec 001's 20 ms limit.
- **SC-006**: With the AGC on, processing still runs at least 20× faster than real time (engineering target) at 48 kHz on a typical developer laptop.
- **SC-007**: A review finds zero untraceable numbers in this feature. Every value cites an A-/S- ID or is labeled an engineering target.
- **SC-008**: A deliberate change of the default release time by 50 %, or of the target level by 3 dB, is caught by the harness: both the tolerance check and the golden check fail.

## Assumptions

All four new assumptions are registered in [docs/hardware/assumptions.md](../../docs/hardware/assumptions.md) with this spec. They are low confidence until level-step captures from a real unit exist.

- **A-017** (new): AGC level regulation. Target −10 dBFS, 10:1 regulation slope (output rises 1 dB per 10 dB of input, so loud inputs stay somewhat louder), maximum gain +40 dB, maximum attenuation 20 dB. Since the mic and preamp are not modeled, levels are relative to the digital input, not to sound pressure. Confidence: Low.
- **A-018** (new): AGC timing. Attack time 10 ms, release time 1.0 s (as defined above, for a 30 dB step). During attack and release the gain in dB approaches its final value approximately exponentially (single time constant in the dB domain): fast at first, then slower. (A simple RC envelope detector recovers at a roughly steady dB rate instead; the dB-domain shape is the assumed audible character, to be checked against captures.) Fast attack and slow release are typical of voice-recorder AGCs and produce audible pumping. Confidence: Low.
- **A-019** (new): AGC detector. Peak-responding (rectified-envelope, fast attack / slow release), no hold period, no look-ahead. It responds to the device-band signal (content up to 4 kHz), including DC and low frequencies. The gain starts at maximum, as if after silence. This is typical of the analog AGCs of the era. Confidence: Low.
- **A-020** (new): The device's AGC is always active, with no user control. S-004 mentions auto-gain, and no switch is known. Confidence: Low–Medium. Verify against the owner's manual (S-001).
- **A-007** (existing) is refined by A-017 to A-020.
- The default pipeline now includes the AGC, which changes its output. No release has been published yet, so this is acceptable. It is recorded in the CHANGELOG. Spec 001's checks and golden references are kept unchanged by running them with the AGC bypassed (FR-017, FR-018); new golden references cover the new default.
- The host-to-device rate conversion stays as in spec 001. The AGC acts on the device-band signal, so content above 4 kHz in the host input never reaches it (A-019).
