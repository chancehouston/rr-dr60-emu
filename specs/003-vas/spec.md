# Feature Specification: Voice Activated System (VAS) on the Record Path

**Feature Branch**: `003-vas`

**Created**: 2026-10-09

**Status**: Draft

**Input**: User description: "Voice Activated System (VAS) for the RR-DR60 record path, signal-chain stage 5 (A-008, A-021, S-001). It acts on the band-limited, AGC-regulated signal after stage 4 and pauses recording while the input stays below a threshold. Paused audio is dropped from the recording, not replaced by silence, so the recorded output can be shorter than the input. On the real device VAS has no switch and no sensitivity control of its own: it is part of normal recording, and its effective threshold follows the five-level microphone sensitivity setting (factory level 3; S-001, A-021). Behaviors to specify with measurable tolerances: the threshold level and how it depends on the microphone sensitivity level; how long the signal must stay below the threshold before recording pauses (hang time); how quickly recording resumes when sound returns, including how much of a word's onset is lost; what happens at each splice (abrupt join or short fade) and how the caller can find where splices occurred; and whether VAS is on by default in the emulator. The stage can be bypassed, can be tapped ("after VAS"), defaults to assumed device values, is deterministic for any block split, and does no allocation during processing. The caller must be able to tell how much output each input block produced. Each numeric default must cite a source or a new registered assumption, low confidence until real-unit captures exist. Tests use synthetic stimuli (tone bursts, gaps of varying length) with other stages bypassed. Out of scope: mic capsule, preamp hiss, noise reduction, speech codec, PCM quantization (A-003), flash capacity limits (99 items / 60 min), playback stages, playback speed control."

## Overview

This feature adds signal-chain stage 5, the Voice Activated System (VAS), to the pipeline from [spec 001](../001-pipeline-skeleton/spec.md) and [spec 002](../002-agc/spec.md). The owner's manual (S-001) says: "Recording automatically pauses when no sound is detected. This avoids blank portions in recordings. The function works better the lower the microphone sensitivity level." The device has no VAS switch and no VAS sensitivity control (A-008). Its character comes from what it removes: pauses vanish from the recording, the first part of a sound after a pause is lost (clipped word onsets), and the remaining sounds are joined abruptly (splices).

Unlike every earlier stage, VAS changes the **length** of the signal. Audio dropped while paused never reaches the output, so a block of input can produce less output, or none.

There is no physical unit and no reference recording. Every numeric default is a registered, low-confidence assumption (A-022 to A-025). The spec fixes **which** properties are measured and **how**; the values can be revised as evidence arrives.

Terms used in this spec (others are defined in specs 001 and 002):

- **VAS input**: the signal entering stage 5, i.e. after stage 4 (record band-limit) in the device's signal-chain order.
- **Recording / paused**: the VAS state. While recording, the VAS input passes to the output. While paused, it is dropped.
- **Threshold**: the VAS input level, as the level of a steady 1 kHz sine (AES17, as in spec 002), that separates "sound" from "no sound". A steady tone at least 3 dB above the threshold counts as sound; one at least 3 dB below counts as no sound.
- **Sensitivity level**: the device's microphone sensitivity setting, 1 to 5, factory level 3 (S-001, A-021). Higher levels record quieter sounds.
- **Hang time**: how long the VAS input must stay below the threshold, continuously, before recording pauses. Audio during the hang time is kept.
- **Onset time**: how long the VAS input must stay above the threshold, continuously, before a paused recording resumes. Audio during the onset time is dropped: this is the lost word onset.
- **Splice**: a point in the output where paused (dropped) audio was removed, so audio from before and after the pause meet. Each splice carries its output position and the length of input removed.
- **Device time**: time measured on the input signal. Hang time and onset time are in device time.
- **VAS-isolated configuration**: VAS on; AGC (stage 3), record band-limit (stage 4) and playback band-limit (stage 10) all bypassed. All VAS measurements use it unless stated otherwise (Principle VII).
- **Burst-gap stimulus**: a 1 kHz tone at the threshold + 10 dB, switched on and off. Its "on" segments ("bursts") and "off" segments (digital silence, "gaps") have lengths chosen per test.
- **Output mode**: what happens to paused audio. **Drop** (default, device-faithful): it is removed, and the output gets shorter. **Mute**: it is replaced by digital silence, and the output keeps the input's length. Both modes make the same pause and resume decisions.
- **Muted region**: in mute mode, a stretch of output that corresponds to paused audio. Each muted region stands where a splice would be in drop mode.

## Clarifications

### Session 2026-10-09

- Q: Is VAS on by default in the emulator? → A: On. The device has no VAS switch (S-001, A-008), so on is device-faithful, as with the AGC in spec 002 (A-020). The default output changes again and becomes variable in length, which the CHANGELOG records. Specs 001 and 002 keep their output by running with VAS bypassed (FR-002, FR-020).
- Q: Should there also be a fixed-length option for real-time hosts? → A: Yes. Drop is the default; a **mute** output mode replaces paused audio with digital silence, keeps one output sample per input sample, and reports the muted regions (FR-004, FR-005, FR-012).
- Q: What does the sensitivity level control? → A: Only the VAS threshold, in this feature (FR-007). A possible gain effect ahead of the AGC (A-021) waits for the mic preamp stage (stage 2), so the AGC's behavior and spec 002's checks are unchanged.
- Q: Should each reported splice also say how much audio was removed there? → A: Yes. Each splice reports its output position and the length of input removed there, in input samples, so hosts can map output time back to input time (FR-005).
- Q: Should the VAS decide from the signal's peaks or its average (RMS) level? → A: Peaks, like the AGC's detector (A-019). Noise and speech therefore keep recording at a lower average level than a steady sine (FR-006, A-022).

### Session 2026-10-09 (plan review)

The coach's plan review found places where the spec promised more than the design can deliver. The fixes below were applied to the spec, research, data model and contracts together.

- Q: How is a muted region reported? → A: Once, by the block in which it ends, with its start position and length. A region still open at the end of a block shows up through the block's paused flag (FR-005).
- Q: Does the golden file need the default configuration in mute mode? → A: No. Golden checks cover drop and mute in the VAS-isolated configuration and drop in the default configuration; mute in the default configuration differs only by stage 10 (FR-019).
- Implementation finding (T006, 2026-10-09): right after a pause ends, the output can lead by up to one device sample (m/l host samples) for a few samples while the conversion catches up, so the ±1 length rule holds once recording has continued for one device sample after the last pause (FR-004, FR-005; research R-05).
- Corrections without a user decision: resume happens only on a sound sample, and FR-009 checks a burst just under the onset time; mute lengths match drop's removed lengths within ±1 host sample at 44.1 and 88.2 kHz; mute equals drop only at the VAS output; the noise checks need hang times ≥ 0.5 s; the mute exact-zero allowance is one pipeline latency after each region's start; the burst level is the threshold + 10 dB; FR-004 and FR-005 share one length tolerance.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Hear the RR-DR60's voice-activated recording (Priority: P1)

An app developer feeds the pipeline a recording with speech separated by long pauses. The output contains the speech with the long pauses removed: each pause is cut down to the hang time, the start of each sound after a pause is clipped by the onset time, and the remaining audio is joined at abrupt splices. The developer can tell, for each block they process, how many output samples it produced and where any splices fell.

**Why this priority**: VAS is the next audible stage of the record path, and its artifacts (vanished pauses, clipped onsets, abrupt joins) are among the best-known parts of the RR-DR60 sound. It is also the first stage that changes signal length, which shapes the API for every later stage. Alone, it is a usable increment.

**Independent Test**: In the VAS-isolated configuration at 48 kHz, process burst-gap stimuli and steady tones and check FR-004 to FR-009 by output length and splice positions. This story includes the minimal measurement it needs, so it does not depend on the harness work in Story 3.

**Acceptance Scenarios**:

1. **Given** the VAS-isolated configuration with default settings, **When** a steady 1 kHz tone at the threshold + 3 dB is processed for 10 s, **Then** the output has exactly as many samples as the input, with no splice.
2. **Given** the same configuration, **When** a burst of 1 s, then a gap of 5 s, then a burst of 1 s is processed, **Then** the output keeps the first burst, the first hang time of the gap, and the second burst minus the onset time, joined at one splice, and its length is 2 s + hang time − onset time, within ±1 ms.
3. **Given** the same configuration, **When** a burst of 1 s, a gap shorter than the hang time, and a burst of 1 s are processed, **Then** nothing is dropped: the output has as many samples as the input.
4. **Given** the same configuration, **When** only digital silence is processed, **Then** the output is exactly zero for the first hang time and nothing after it (the stream starts in the recording state, A-025).
5. **Given** a pipeline processing in blocks, **When** any stimulus is processed, **Then** for each block the caller learns how many output samples it produced (never more than its input) and the output position of every splice in it.

---

### User Story 2 - Tune, bypass, or tap the VAS (Priority: P2)

An app developer wants to keep the RR-DR60's other character without VAS, to tune it, or to use it in a real-time app that must output one sample per input sample. They can bypass VAS, choose mute mode instead of drop mode, take the output right after it ("after VAS"), choose the sensitivity level, and change the threshold, hang time and onset time within documented limits. The same controls are available through the language-neutral interface.

**Why this priority**: Principle V requires every stage to be bypassable, tap-able and configurable. The defaults are low-confidence guesses. Bypass also keeps the output of specs 001 and 002 available unchanged, at fixed length.

**Independent Test**: For each setting in FR-012, check the stated effect with burst-gap stimuli and steady tones, through both the native and the language-neutral interface. Check that a VAS-bypassed pipeline reproduces the spec 001 and spec 002 golden files bit for bit.

**Acceptance Scenarios**:

1. **Given** a pipeline with VAS bypassed, **When** any spec 001 or spec 002 golden stimulus is processed in its configuration, **Then** the output is bit-identical to the stored golden reference, and every block produces exactly as many samples as it consumed.
2. **Given** a pipeline with the tap set to "after VAS", **When** any stimulus is processed, **Then** the output equals that of the same pipeline with stage 10 bypassed, sample for sample.
3. **Given** sensitivity level 1 and then level 5, **When** steady tones are processed, **Then** the threshold is 6 dB above and 6 dB below the level-3 threshold respectively, each ±1 dB (A-022).
4. **Given** a hang time of 3 s, **When** a burst, a 10 s gap and a burst are processed, **Then** 3 s ± 1 ms of the gap is kept.
5. **Given** mute mode in the VAS-isolated configuration, **When** a burst of 1 s, a gap of 5 s and a burst of 1 s are processed, **Then** the output has exactly as many samples as the input, the muted region covers the same device-time span that drop mode removes (the gap after the hang time, plus the onset time of the second burst; its length in host samples equals drop mode's removed length, within ±1 at 44.1 and 88.2 kHz), and the samples outside it equal drop mode's output.
6. **Given** an out-of-range setting (e.g. sensitivity level 6), **When** the host creates or reconfigures the pipeline, **Then** it is rejected with an error that names the setting, and an existing pipeline is left unchanged.
7. **Given** a host that uses only the language-neutral interface, **When** it processes a burst-gap stimulus with non-default VAS settings in each output mode, **Then** its output, per-block output counts, splice positions and muted regions match the native interface exactly.

---

### User Story 3 - The harness measures the VAS (Priority: P3)

A maintainer runs the measurement harness. It now drives the VAS with steady tones near the threshold, burst-gap stimuli with gaps around the hang time, short bursts around the onset time, and silence, at every supported host rate and every sensitivity level. It checks every tolerance in this spec, reports each with its requirement and A-/S- IDs, and guards the VAS output, output lengths and splice positions with golden files.

**Why this priority**: Principle III requires measurement tests and golden-file regression. These checks verify Stories 1 and 2.

**Independent Test**: Run the harness. Deliberately change the default hang time by 20 % or the threshold by 3 dB, and confirm that both the relevant tolerance check and the golden check fail.

**Acceptance Scenarios**:

1. **Given** the pipeline meets this spec, **When** the harness runs, **Then** every VAS check passes at all six host rates and names its requirement and A-/S- IDs.
2. **Given** any VAS stimulus processed in one block and in randomly sized blocks (including sizes 0 and 1), **When** the outputs are concatenated, **Then** they are bit-identical, with identical splice positions.
3. **Given** the default pipeline (AGC and VAS on), **When** a stimulus of speech-level bursts separated by long gaps of low-level noise is processed, **Then** the harness reports, without a pass/fail tolerance, how much of each gap was kept. This documents how the AGC's noise rise (spec 002 FR-009) interacts with the VAS threshold.

---

### Edge Cases

- **Start of stream, reset and reconfigure**: a new, reset or reconfigured pipeline starts in the recording state, as if recording had just been started (A-025). Silence from the start is kept for one hang time and then dropped.
- **Everything dropped**: in drop mode, a block processed while paused produces zero output samples. This is valid, not an error. In mute mode it produces a full block of silence.
- **Sounds shorter than the onset time**: a sound above the threshold for less than the onset time does not resume a paused recording, and none of it reaches the output.
- **Gap exactly at the hang time**: a gap equal to the hang time (within one device sample) does not pause recording.
- **Level hovering at the threshold**: recording pauses only after the hang time of continuous no-sound and resumes only after the onset time of continuous sound, so there is at most one pause per hang time + 2 device samples, whatever the onset time.
- **Non-finite input (NaN, ±Inf)**: treated as 0.0, as in spec 001. Neither the output nor the VAS state becomes non-finite.
- **Long silence**: hours of silence produce one hang time of zeros and then nothing, with no drift and no growth in memory.
- **Long sound**: there is no limit on recording length. Flash capacity (99 items / 60 min, S-001) is out of scope.
- **Tap before VAS**: with the tap "after AGC" or "after record stage", VAS does not run, the output has exactly as many samples as the input, and the VAS settings have no effect.
- **Mute mode and the playback stage**: in mute mode, the playback band-limit stage (10) runs over the silence, so its filter ring-out from the last kept sample continues into the start of a muted region. That ring-out is not removed; the exact-zero rule in FR-004 applies to the VAS output (tap "after VAS").
- **Output across a splice**: the samples on either side of a splice are passed through unchanged (no fade, no crossfade, A-024). Any discontinuity at the join is part of the modeled character.
- **Host-rate conversion across a splice**: the conversion back to the host rate runs on the joined signal, so a splice is smoothed only by the band-limiting of that conversion. The reported splice position is the output sample where the first sample after the pause would land.
- **Configuration changes while streaming**: not supported, as in specs 001 and 002. Reconfiguring resets VAS to its starting state.

## Requirements *(mandatory)*

### Functional Requirements

**Placement, bypass, tap and output length**

- **FR-001**: The pipeline MUST include a VAS stage (signal-chain stage 5) that acts on the signal after the record band-limit stage (stage 4) and before the playback band-limit stage (stage 10), matching the device's signal-chain order. Its decision uses the VAS input only.
- **FR-002**: VAS MUST be bypassable. A bypassed VAS MUST pass its input through unchanged (bit-exact) and drop nothing. With VAS bypassed, every configuration from specs 001 and 002 MUST produce output bit-identical to theirs, including their golden references. VAS MUST be on by default, in drop mode, because the device has no VAS switch (S-001, A-008).
- **FR-003**: The caller MUST be able to choose the tap "after VAS", in addition to the existing taps. With it, stage 10 does not run. With a tap before stage 5 ("after AGC", "after record stage"), VAS does not run.
- **FR-004**: **Output length.** For every processed block the pipeline MUST report how many output samples it produced, which is never more than the number of input samples in the block.
  - **Drop mode** (default): paused VAS input is removed, not replaced. Over a whole stream that ends while recording, the output length MUST equal the input length minus the dropped duration converted to the host rate, within ±1 host sample (exactly at 8, 16, 48 and 96 kHz). This holds once recording has continued for one device sample (⌈host rate / 8000⌉ + 1 host samples) after the last pause; at any other moment the difference is at most one device sample (12 host samples at 96 kHz). When nothing has been dropped, the output length equals the input length exactly.
  - **Mute mode**: paused VAS input is replaced by digital silence. Every block MUST produce exactly as many output samples as it consumed. The pause and resume decisions MUST be identical to drop mode's for the same input and settings. Outside the muted regions, the VAS output (tap "after VAS", or the VAS-isolated configuration) MUST be bit-identical to drop mode's at the 8 kHz host rate; stages after VAS (stage 10) may differ after each region, because their filter memory differs between the modes. Inside a muted region the VAS output MUST be exactly 0.0 at the 8 kHz host rate; at other host rates it MUST be exactly 0.0 except within one pipeline latency plus one sample after the region's start, where the rate conversion still sees kept samples. A region shorter than that may contain no exact zeros.
- **FR-005**: **Splice and mute reporting.** For every block the caller MUST be able to learn the output position of every splice (drop mode) that the block produced. In mute mode, each muted region MUST be reported once, by the block in which it ends, with its start position and length; a region still open at the end of a block is signalled by the block's paused flag. Each splice MUST also report the length of input removed there, in host-rate input samples; over a whole stream that ends while recording, the removed lengths MUST add up to the input length minus the output length, within ±1 host sample (exactly at 8, 16, 48 and 96 kHz), with the same settling condition as FR-004. The caller MUST also be able to tell, for every block, whether VAS is paused at its end, since audio dropped by a pause that has not ended yet has no splice. Positions are reported against the stream's output, so they are independent of how the input was split into blocks. In mute mode, the muted regions MUST cover exactly the device-time spans that drop mode removes; in host samples, each region's length MUST equal the corresponding removed length exactly at 8, 16, 48 and 96 kHz, and within ±1 host sample at 44.1 and 88.2 kHz.

**Measurable behavior (VAS-isolated configuration, default settings, every supported host rate)**

- **FR-006**: **Threshold.** At sensitivity level 3, a steady 1 kHz tone at least 3 dB above the threshold MUST be kept in full, and one at least 3 dB below it MUST be dropped after the hang time. Default threshold at level 3: −18 dBFS (A-022). The same MUST hold for tones at 300, 1000 and 3400 Hz, with the threshold the same within ±1 dB at each frequency. The detector is **peak-responding** (A-022): band-limited noise (300–3400 Hz, as in spec 002 FR-009) at a level 5 dB below the threshold MUST be kept in full, because its peaks read about 8–9 dB above its level, and the same noise 15 dB below the threshold MUST be dropped after the hang time.
- **FR-007**: **Sensitivity.** Each sensitivity step changes the threshold by 3 dB: level 1 = level-3 threshold + 6 dB, level 2 = + 3 dB, level 4 = − 3 dB, level 5 = − 6 dB, each ±1 dB (A-022). Higher levels record quieter sounds (S-001). In this feature the sensitivity level changes only the VAS threshold: it MUST NOT change the level of the recorded signal or the AGC's behavior. A gain effect ahead of the AGC (A-021) is out of scope until the mic preamp stage is modeled.
- **FR-008**: **Hang time.** With the burst-gap stimulus, a gap longer than the hang time MUST be kept for exactly the hang time and dropped after it, and a gap shorter than or equal to the hang time MUST be kept in full. Default hang time: 1.0 s (A-023). Tolerance: ±1 ms, checked with gaps of 0.5× the hang time, the hang time − 5 ms, the hang time + 5 ms, and 3× the hang time.
- **FR-009**: **Onset time.** After a pause, a burst longer than the onset time MUST resume recording, and the first onset time of the burst MUST be dropped. A burst shorter than the onset time MUST be dropped entirely. Default onset time: 20 ms (A-024). Tolerance: ±1 ms, checked with bursts of 0.5×, 1.5× and 50× the onset time, and of the onset time − 2 ms when the onset time is at least 3 ms (with an onset time of 0, only a 50 ms burst, which must be kept in full). A burst shorter than the onset time MUST NOT resume recording in the silence after it. There is no look-ahead and no pre-roll: VAS never recovers audio from before the moment it resumes.
- **FR-010**: **Splices.** At each splice, the last kept sample before the pause and the first kept sample after it MUST be adjacent in the output, with no inserted, faded or altered samples (A-024). Every sample that is kept MUST be bit-identical to the VAS input sample it came from (at the 8 kHz host rate, where the rate conversion is the identity).
- **FR-011**: **Latency.** VAS MUST add no latency. The pipeline's reported latency MUST be the same with VAS on or bypassed, and it applies to kept audio.

**Settings**

- **FR-012**: The caller MUST be able to set the following when creating or reconfiguring the pipeline. Values outside these ranges MUST be rejected with an error that names the setting.

  | Setting | Default | Valid range |
  |---|---|---|
  | VAS on / bypassed | on (S-001, A-008) | — |
  | Output mode | drop (S-001, A-008) | drop, mute |
  | Sensitivity level | 3 (S-001, A-021) | 1 to 5 |
  | Threshold at level 3 | −18 dBFS (A-022) | −60 to 0 dBFS |
  | Hang time | 1.0 s (A-023) | 0.05 to 10 s |
  | Onset time | 20 ms (A-024) | 0 to 200 ms |

  An onset time of 0 means recording resumes on the first sample above the threshold.
- **FR-013**: For non-default settings, FR-006 to FR-009 MUST hold with the defaults replaced by the configured values, with the same tolerances, except that FR-006's noise checks apply only for hang times of at least 0.5 s (noise crosses the threshold intermittently, so shorter hang times pause inside it). The harness MUST check the defaults, every sensitivity level, and the minimum and maximum of each other setting with the rest at default.

**Determinism, real-time safety and interface**

- **FR-014**: The determinism, real-time safety and reset requirements of specs 001 and 002 (001 FR-014 to FR-017, 002 FR-013) MUST hold with VAS on, for every valid setting. For every block partition, in both output modes, the concatenated output, the splice positions and the muted regions MUST be bit-identical. Processing MUST NOT allocate, lock or do I/O, including when reporting output counts, splices and muted regions.
- **FR-015**: All VAS settings, per-block output counts, splice positions and muted regions MUST be available through the language-neutral interface, with output bit-identical to the native interface. Invalid VAS settings MUST be reported as error values naming the setting, following 001 FR-024 and 002 FR-014.
- **FR-016**: The AGC (stage 3) MUST keep running while VAS is paused, since it sits upstream of VAS in the device (A-025). VAS has no effect on the AGC.

**Traceability and harness**

- **FR-017**: Every default value MUST cite its A-/S- ID in the spec, code and tests. The following are emulator engineering targets, not device properties, and are labeled as such: the ±3 dB threshold margin and ±1 dB threshold tolerances (FR-006, FR-007); the ±1 ms timing tolerances and the gap and burst multiples (FR-008, FR-009); the ±1 host sample length tolerance (FR-004); the test frequencies and the 5 dB / 15 dB noise margins (FR-006); the setting ranges (FR-012); the 20 % / 3 dB mutation sizes (SC-007); the mute-mode edge allowance of one pipeline latency plus one sample (FR-004); the ±1 host sample mute-length tolerance at 44.1 and 88.2 kHz (FR-005); the 0.5 s minimum hang time for noise checks (FR-013); the onset − 2 ms burst (FR-009); the burst level of the threshold + 10 dB (definitions); the 10 s tone in US1 AS1 and the hang time + 1 s tones (research R-11); and the 5 minutes in SC-001.
- **FR-018**: The measurement harness MUST check FR-002 to FR-016 and the edge cases above, with each result reporting its measured value, tolerance, requirement and A-/S- IDs (as 001 FR-020). Checks with exact expectations MAY be plain test assertions whose messages state the measured value.
- **FR-019**: The harness MUST add golden-file checks for the VAS-isolated configuration in both output modes and for the default configuration in drop mode, covering output samples, output lengths, splice positions and muted regions, using small seeded stimuli that include burst-gap stimuli with gaps on both sides of the hang time. All spec 001 and 002 golden references MUST stay unchanged: none are re-blessed. If the default pipeline's output changes, the CHANGELOG MUST record it.
- **FR-020**: All spec 001 and spec 002 acceptance scenarios, tolerance checks, edge cases and golden-file configurations MUST be evaluated with VAS bypassed, and are otherwise unchanged. Where spec 002 says "default pipeline" or "default settings", this means spec 002's defaults (AGC on) with VAS bypassed.

### Out of Scope

Microphone (stage 1); mic preamp hiss, clipping and AC coupling (stage 2); noise reduction (stage 6); speech encoder/decoder and frame-based timing (stages 7, 9); PCM quantization and companding (A-003); flash storage capacity and item limits (stage 8, S-001); playback speed control (S-001); volume, power amp, speaker and earphone path (stages 11–13); the REC lamp; VAS look-ahead or pre-roll; changing settings while streaming.

### Key Entities

- **VAS stage**: signal-chain stage 5. Holds its settings and its state (recording or paused, and how long the input has been above or below the threshold). Has a bypass flag.
- **VAS settings**: on/bypassed, output mode (drop or mute), sensitivity level, threshold at level 3, hang time, onset time. They extend the pipeline settings, and every default cites an A- or S-ID.
- **Block result**: for each processed block, the number of output samples produced and the output positions of any splices (drop mode) or muted-region edges (mute mode).
- **Splice**: an output position where dropped audio was removed (drop mode), with the length of input removed there.
- **Muted region**: a stretch of output replaced by silence (mute mode).
- **Tap point**: extended with "after VAS".
- **Measurement result** and **golden reference**: as in specs 001 and 002, extended with output lengths and splice positions.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An app developer can bypass VAS, switch it to mute mode, change the sensitivity level, or read the per-block output counts and splice positions, in under 5 minutes using only the project's documentation.
- **SC-002**: 100 % of the VAS tolerance checks (FR-004 to FR-011, FR-013) pass at all six supported host rates.
- **SC-003**: With VAS bypassed, the spec 001 and 002 golden references match with zero differences. The new VAS golden references pass unchanged on all eight platform targets from 001 FR-014, and across at least 100 random block partitions of each VAS golden stimulus.
- **SC-004**: Zero memory allocations are observed while processing 1000+ blocks of varying size with VAS on, including blocks that are entirely dropped and blocks with several splices.
- **SC-005**: VAS adds 0 samples of latency.
- **SC-006**: With AGC and VAS on, processing still runs at least 20× faster than real time (engineering target) at 48 kHz on a typical developer laptop.
- **SC-007**: A deliberate change of the default hang time by 20 %, or of the threshold by 3 dB, is caught by the harness: both the tolerance check and the golden check fail.
- **SC-008**: A review finds zero untraceable numbers in this feature. Every value cites an A-/S- ID or is labeled an engineering target.

## Assumptions

The new assumptions are registered in [docs/hardware/assumptions.md](../../docs/hardware/assumptions.md) with this spec. All are low confidence until burst-and-gap captures from a real unit exist.

- **A-022** (new): VAS threshold. The VAS input's peak level (as with the AGC's detector, A-019) is compared with a threshold of −18 dBFS (sine level) at sensitivity level 3, and each sensitivity step moves it by 3 dB (level 1 highest, level 5 lowest). The detector is frequency-independent within the device band. Rationale: −18 dBFS sits below speech regulated by the AGC (about −13 to −9 dBFS, spec 002), and far enough above quiet background raised by the AGC's maximum gain (−30 dBFS for −70 dBFS input) that the background's peaks almost never cross it, so a quiet room pauses at the factory level. With 3 dB steps, level 1 records only inputs above about −30 dBFS ("only the louder sounds", S-001) and level 5 records inputs down to about −64 dBFS. Plan research R-03 revised the first draft's −24 dBFS and 6 dB steps, which never paused on that background and made level 1 record nothing. Confidence: Low.
- **A-023** (new): VAS hang time 1.0 s. Typical of voice-activated dictation recorders: long enough to keep pauses between words, short enough to remove pauses between sentences. Confidence: Low.
- **A-024** (new): VAS onset time 20 ms, with no pre-roll, and abrupt splices with no fade. Recorders of the era had no buffer to recover the start of a sound, which produces the clipped word onsets the device is known for. Confidence: Low.
- **A-025** (new): VAS starts in the recording state when recording starts, and the AGC keeps running while VAS is paused (the AGC is upstream in the analog path). Confidence: Low.
- **A-008** and **A-021** (existing) are refined by A-022 to A-025.
- The device's speech codec works in frames, so real splices probably fall on frame boundaries. Frame-based timing belongs to the codec stages and is out of scope; this feature splices at device-sample resolution.
- The REC lamp, which flashes while paused (S-001), is not modeled. The per-block results give hosts the same information.
- Mute mode is an emulator convenience for real-time hosts, not a device behavior. The device only drops (S-001, A-008); mute mode uses the same decisions and replaces the dropped audio with silence.
- The sensitivity level moves only the VAS threshold in this feature. A-021's possible gain effect ahead of the AGC waits for the mic preamp stage (stage 2). Spec 002's out-of-scope note that no mic sensitivity switch was known is superseded by S-001; the AGC itself is unchanged.
- The default pipeline's output changes (VAS on). No release has been published, so this is acceptable; the CHANGELOG records it.
