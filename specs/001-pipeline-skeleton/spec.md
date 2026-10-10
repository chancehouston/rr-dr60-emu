# Feature Specification: Minimal End-to-End Pipeline Skeleton

**Feature Branch**: `001-pipeline-skeleton`

**Created**: 2026-10-08

**Status**: Implemented, merged to `main` (PRs #2–#5)

## Clarifications

### Session 2026-10-08

- Q: Can bypass/tap settings change between blocks while streaming, or only at configuration time? → A: Only at configuration time. Reconfiguring resets state and may allocate; live switching is out of scope (FR-008).
- Q: Should the voice-band filters be minimum-phase (hardware-like, low delay) or linear-phase? → A: Minimum-phase, hardware-like (A-016; FR-006, FR-010).
- Q: With both stages bypassed, does audio still pass through the 8 kHz device rate, or come out completely unchanged? → A: The 8 kHz device rate is always applied. There is no option to bypass it (FR-004).
- Q: Which platforms must produce bit-identical output against one set of golden files? → A: All Tier-1 targets: Linux/macOS/Windows on x86-64 and ARM64, plus iOS device and simulator (FR-014, FR-021, SC-003).
- Q: Which host sample rates must be supported and fully pass the harness? → A: 8, 16, 44.1, 48, 88.2 and 96 kHz; all others are rejected (FR-002, FR-005, SC-002).
- Q: Is the language-neutral (C) integration surface part of this slice? → A: Yes, as a minimal interface: create, process, latency, reset, reconfigure, destroy, with errors returned as values (FR-023, FR-024).
- Q: When must the iOS-device golden check pass, given that CI can't run it? → A: Before each release. All other targets are checked on every pull request (FR-014, SC-003).

**Input**: User description: "Minimal end-to-end pipeline skeleton for the RR-DR60 emulator. A host application feeds mono audio at a common host sample rate (at minimum 44.1 kHz and 48 kHz), in blocks of any size, and receives mono output at the same rate. The output has passed through the device's voice band twice: once on the record side (anti-alias filter + ADC, signal-chain stage 4) and once on the playback side (DAC + reconstruction filter, stage 10). Both are modeled at the device's native internal rate (A-001, assumed 8 kHz) with a telephone-style 300-3400 Hz passband (A-002). Each of the two stages can be bypassed independently, and the caller can take the output after either stage. The pipeline reports its fixed latency in samples. The same input, settings and seed must produce identical output on every platform and for every block size. Processing a block must never allocate memory, wait on locks or do I/O. The feature includes a measurement harness that drives the pipeline with tones, sine sweeps, impulses and silence and checks frequency response, out-of-band rejection, passband level and latency against tolerances stated in the spec, plus golden-file regression checks. Every default value cites an S-### or A-### ID. Explicitly out of scope for this slice: microphone, preamp, AGC, VAS, noise reduction, speech codec, flash storage, volume/amp, speaker, earphone path, PCM quantization/companding (A-003 is still open), clock deviation (A-011, nominal only), stereo/multichannel, and any file-format or command-line tooling."

## Overview

This is the first working slice of the emulator. It gives host applications something they can use right away: audio that has been band-limited the way the RR-DR60's voice-band codec (OKI MSM7702, S-002 / S-003) is assumed to band-limit it, once when recording and once when playing back. It also sets up the frame that every later stage (mic, AGC, VAS, speech codec, speaker, …) plugs into: the internal device-rate domain, bypass and tap controls, latency reporting, determinism, real-time safety, and the measurement harness that proves each stage's behavior.

Terms used in this spec:

- **Host rate**: the sample rate the calling application uses for input and output (e.g. 48 kHz).
- **Device rate**: the emulated device's internal sample rate, 8 kHz (A-001).
- **Rate conversion boundary**: the conversion from host rate to device rate on the way in and from device rate back to host rate on the way out. It is an emulator mechanism, not a modeled hardware stage.
- **Record band-limit stage**: signal-chain stage 4 (anti-alias filter + ADC), modeled as a voice-band filter at the device rate.
- **Playback band-limit stage**: signal-chain stage 10 (DAC + reconstruction filter), modeled as a voice-band filter at the device rate.
- **Tap**: the point in the chain where the caller takes the output.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Hear audio through the RR-DR60 voice band (Priority: P1)

An app developer (e.g. building an EVP-recorder or vintage-gear app) creates a pipeline at their app's host rate with default settings, feeds it mono audio in whatever block sizes their audio system delivers, and gets back mono audio at the same rate that has been band-limited by both the record and playback stages, sounding like telephone-band audio.

**Why this priority**: This is the smallest increment that delivers audible value and proves the end-to-end path (host rate in → device rate → host rate out). Every later stage depends on it.

**Independent Test**: Create a pipeline with defaults at 48 kHz, through both the native and the language-neutral interface. Process the stimuli in acceptance scenarios 1–5 and check the stated outcomes. This story includes the minimal tone generation and level measurement it needs, so it does not depend on the full harness in Story 3 (Principle VII).

**Acceptance Scenarios**:

1. **Given** a default pipeline at 48 kHz, **When** the host processes a 1 kHz tone at −20 dBFS, **Then** the steady-state output is a 1 kHz tone at −20 dBFS ± 0.2 dB (passband level, A-015).
2. **Given** a default pipeline at 44.1 kHz, **When** the host processes a 50 Hz tone and a 3800 Hz tone, **Then** each produces steady-state output at least 20 dB below the input level (A-002, A-014; both stages contribute attenuation).
3. **Given** a default pipeline, **When** the host submits a block of N samples, **Then** it receives exactly N output samples for that block.
4. **Given** a default pipeline that has only ever received digital silence, **When** the host processes more silence, **Then** every output sample is exactly zero.
5. **Given** a host written in another language (e.g. C or Swift) that uses only the language-neutral interface, **When** it creates a default pipeline, processes a 1 kHz tone, queries latency, resets and destroys the pipeline, **Then** the output matches that of the native interface bit for bit, and invalid calls (unsupported rate, missing buffer) return an error value without crashing.

---

### User Story 2 - Choose stages and tap point (Priority: P2)

An app developer wants only part of the character, e.g. "what the device recorded" (record side only) or only the playback filtering. They can bypass the record stage, bypass the playback stage, or take the output right after the record stage.

**Why this priority**: The constitution requires every stage to be bypassable and tap-able (Principle V). It also lets each stage be measured on its own (Principle VII), which the harness in Story 3 depends on for per-stage tolerances.

**Independent Test**: For each of the configurations in FR-007, process a sine sweep and check that the measured response matches the response expected for that configuration.

**Acceptance Scenarios**:

1. **Given** a pipeline with the playback stage bypassed, **When** a sweep is processed, **Then** the response matches the record stage's response alone within the per-stage tolerances (FR-010).
2. **Given** a pipeline with the tap set to "after record stage", **When** a sweep is processed, **Then** the output equals the output of the configuration "playback stage bypassed" sample for sample, and the playback stage's bypass setting has no effect.
3. **Given** a pipeline with both stages bypassed, **When** a sweep is processed, **Then** the response is flat (±0.1 dB) from 50 Hz to 3600 Hz and the output contains only the effect of the rate conversion boundary.
4. **Given** any configuration, **When** the host asks for the latency, **Then** the reported value matches the measured delay of that configuration (FR-012).

---

### User Story 3 - Measurement harness proves the behavior (Priority: P3)

A maintainer or contributor runs the automated measurement harness. It drives the pipeline with tones, sine sweeps, impulses and silence at each supported host rate and configuration, checks every tolerance in this spec, and compares the output against golden reference files to catch unintended changes.

**Why this priority**: The constitution requires measurement tests and golden-file regression (Principle III). The harness is also the reusable tool every later stage's spec will use. The requirements it checks belong to Stories 1 and 2, so it ranks after them as a deliverable, but its tests are written first.

**Independent Test**: Run the harness against the pipeline. It reports pass/fail per tolerance. Deliberately change one stage's band edge by 100 Hz and confirm the harness fails the response check and the golden check.

**Acceptance Scenarios**:

1. **Given** the pipeline meets this spec, **When** the harness runs, **Then** every check passes and each result names the requirement and the A-/S- ID it verifies.
2. **Given** a deliberate change that shifts any output sample, **When** the harness runs, **Then** the golden-file check fails and reports which signal and configuration differ.
3. **Given** a stimulus processed in one block and the same stimulus processed in randomly sized blocks (including size 0 and 1), **When** the harness compares the outputs, **Then** they are bit-identical.

---

### Edge Cases

- **Block size 0**: accepted, produces no output, and does not change the pipeline's state.
- **Block size 1 and very large blocks** (e.g. 10 minutes of audio in one call): accepted; output is bit-identical to any other partition of the same input (FR-014).
- **Unsupported host rate**: rejected when the pipeline is created, with an error that names the supported rates. No partially working pipeline is returned.
- **Non-finite input samples (NaN, ±Inf)**: treated as 0.0. The output never contains NaN, Inf or subnormal values, and the pipeline does not need a reset to recover.
- **Input beyond ±1.0 (over full scale)**: processed linearly without clipping, since quantization and clipping are out of scope (A-003). The output may exceed ±1.0.
- **DC input**: a constant 0 dBFS DC input produces steady-state output at or below −60 dBFS in the default configuration. This follows from FR-010's DC attenuation of at least 40 dB per stage (A-014).
- **End of stream / tail**: to flush the tail, the host feeds at least 0.5 s of silence. The reported latency (1 kHz group delay) is not enough on its own, because minimum-phase stages ring longer near the band edges. After any input of at most 0 dBFS stops, every output sample more than 0.5 s later is below −120 dBFS (engineering target, FR-018).
- **Reset**: the host can return the pipeline to its freshly created state (same as creating a new one with the same settings) without allocating memory.
- **Configuration changes while streaming**: not supported. The host reconfigures the pipeline between streams, which resets its state (FR-008).

## Requirements *(mandatory)*

### Functional Requirements

**Input, output and rates**

- **FR-001**: The pipeline MUST accept mono audio as floating-point samples with nominal full scale ±1.0, and MUST return mono audio at the same host rate.
- **FR-002**: The pipeline MUST support exactly these host rates: 8 kHz, 16 kHz, 44.1 kHz, 48 kHz, 88.2 kHz and 96 kHz. Every supported rate MUST pass the full measurement harness. Any other rate MUST be rejected when the pipeline is created. At an 8 kHz host rate, the rate conversion boundary is the identity (no conversion), so with both stages bypassed the output equals the input exactly, except that non-finite and subnormal input samples become 0.0 (see Edge Cases).
- **FR-003**: The pipeline MUST accept blocks of any length, including 0 and 1, with no maximum block length. For each block of N input samples it MUST return exactly N output samples.
- **FR-004**: Internally, both band-limit stages MUST operate at the device rate of 8 kHz (A-001). The rate conversion boundary MUST always be applied, even when both stages are bypassed, because the internal device rate is a property of the device (A-001). There is no setting to bypass the device rate. A fully bypassed pipeline is the reference "empty device" baseline for testing later stages.
- **FR-005**: The rate conversion boundary MUST NOT noticeably color the result: with both stages bypassed, the response from 50 Hz to 3600 Hz MUST be flat within ±0.1 dB; content between 3600 Hz and 4000 Hz MAY be attenuated; any input from 4000 Hz up to the host Nyquist frequency MUST be attenuated by at least 60 dB, and any alias or image products MUST be at least 60 dB below the stimulus. (These are emulator engineering targets, not device properties, so they cite no A-/S- ID.) Checks at or above the host Nyquist frequency do not apply. At 8 kHz that means checks at 4000 Hz and above are skipped, here and in FR-010, so the stages' upper stopband is verified only at the other five rates.

**Stages, bypass and tap**

- **FR-006**: The record band-limit stage (stage 4) and the playback band-limit stage (stage 10) MUST each apply a telephone-style band-pass response with the tolerances in FR-010 (A-002, A-014), unity passband gain (A-015), and the same nominal shape on both sides (A-014). Each stage MUST be minimum-phase, as in analog or codec filters, not linear-phase (A-016). This is verified by the phase checks in FR-010.
- **FR-007**: The caller MUST be able to choose, independently: record stage on/bypassed, playback stage on/bypassed, and tap "after record stage" or "after playback stage". Defaults: both stages on, tap after playback stage (A-002: the device band-limits on both record and playback). A bypassed stage MUST pass its input through unchanged (bit-exact, no gain or filtering).
- **FR-008**: Bypass and tap settings (and all other settings) are fixed when the pipeline is created or configured. They MUST NOT change during block processing. Changing them requires reconfiguring the pipeline. Reconfiguring MAY allocate memory, is not real-time safe, and MUST leave the pipeline in the same state as a freshly created pipeline with the new settings (including the newly reported latency). Glitch-free switching while streaming is out of scope for this slice.
- **FR-009**: The pipeline MUST accept a seed as part of its settings so that future stochastic stages follow the determinism contract. In this slice no stage is stochastic, so the output MUST NOT depend on the seed.

**Measurable behavior (per stage, with the other stage bypassed, at every supported host rate)**

- **FR-010**: Each band-limit stage MUST meet these tolerances (A-002, A-014). The 1 kHz gain is absolute (output level / input level); all other magnitude limits are relative to the 1 kHz gain. The stage's own response is the configuration's measured response minus the fully bypassed configuration's measured response (magnitude in dB, phase and group delay), which removes the rate conversion boundary.
  - Gain at 1 kHz: 0 dB ± 0.1 dB (A-015).
  - Lower −3 dB point: 300 Hz ± 50 Hz. Upper −3 dB point: 3400 Hz ± 50 Hz.
  - Passband ripple from 400 Hz to 3200 Hz: within ±0.5 dB.
  - Attenuation at DC: at least 40 dB.
  - Attenuation from above DC up to 60 Hz: at least 20 dB.
  - Attenuation at 4000 Hz: at least 14 dB.
  - Attenuation from 4600 Hz up to the host Nyquist frequency: at least 25 dB, counting the total output power, including any alias or image products.
  - Group delay at 1 kHz: at most 2 ms (A-016).
  - Group delay at 400 Hz and at 3200 Hz: each greater than the group delay at 1 kHz (A-016).
  - Phase from 400 Hz to 3200 Hz: within ±5° of the minimum-phase response computed from the stage's measured magnitude (A-016; the ±5° figure is an engineering target). This check is performed at the 8 kHz host rate, where the rate conversion boundary is the identity, so the stage response is observed directly. The stage coefficients are identical at every host rate, and the other FR-010 checks confirm this at the other rates.
- **FR-011**: With both stages on, the measured response at every test frequency from 100 Hz to 3900 Hz MUST equal R_rec + R_play − R_base within ±0.3 dB, wherever that value is above −40 dB. Here R_rec and R_play are the measured responses with only the record or only the playback stage on, and R_base is the fully bypassed response, all in dB. Subtracting R_base once stops the rate conversion boundary being counted twice. (±0.3 dB and −40 dB are engineering targets.) The 1 kHz gain MUST be 0 dB ± 0.2 dB (A-015).

**Latency**

- **FR-012**: The pipeline MUST report its latency as a whole number of host-rate samples for its current configuration and host rate. The reported latency MUST equal the measured group delay at 1 kHz within ±1 sample. It MUST be constant for the life of a given configuration (it does not vary with block size, input or time).
- **FR-013**: With default settings, the total latency MUST NOT exceed 20 ms at any supported host rate, so the pipeline is usable for live monitoring.

**Determinism and real-time safety**

- **FR-014**: The same input, settings and seed MUST produce bit-identical output on every supported platform, and for every way of splitting the input into blocks. Supported platforms for this guarantee: Linux, macOS and Windows on both x86-64 and ARM64, plus iOS device (ARM64) and iOS simulator. One set of golden references MUST serve all of them; per-platform golden files are not allowed. Golden checks MUST pass on every pull request on every target that has automated runners. The iOS-device check MUST pass before each release.
- **FR-015**: Processing a block MUST NOT allocate or free memory, wait on locks, perform I/O, or do work that is not bounded by a constant per sample. All resources MUST be acquired when the pipeline is created or configured.
- **FR-016**: The pipeline MUST NOT use wall-clock time, OS randomness or any other non-deterministic input.
- **FR-017**: The pipeline MUST provide a reset that returns it to the freshly created state without allocating memory.

**Traceability**

- **FR-018**: Every default value and every modeled tolerance in this feature MUST cite an S-### or A-### ID in the spec, in code and in tests. Values that are emulator engineering targets, not device properties, MUST be labeled as such. In this spec they are: FR-005 (all values), the ±5° phase tolerance in FR-010, ±0.3 dB and −40 dB in FR-011, ±1 sample in FR-012, ±0.3 dB for the measured vs. analytic stage response (US2 AS1), FR-013 (20 ms), the tail decay (−120 dBFS within 0.5 s), SC-006 (20×), and the interface behavior in FR-023 and FR-024.

**Measurement harness**

- **FR-019**: The feature MUST include an automated measurement harness that generates its own stimuli (tones, logarithmic sine sweeps, impulses, silence, DC) in code, runs them through every configuration in FR-007 at every supported host rate, and checks FR-002, FR-003, FR-005, FR-007, FR-010, FR-011, FR-012, FR-013, FR-023 and FR-024 (through the language-neutral interface), and the edge cases above.
- **FR-020**: Each harness check MUST report the measured value, the tolerance, the requirement it verifies and the A-/S- IDs involved.
- **FR-021**: The harness MUST include golden-file regression checks: a small set of fixed, seeded stimuli processed through the default and each single-stage configuration, compared bit-exactly with stored reference output. Reference files MUST be small (no large audio committed) and MUST be updated only on purpose, with a CHANGELOG entry.
- **FR-022**: The harness MUST include a block-partition check (one block vs. randomly sized blocks from a seeded generator, including sizes 0 and 1) and an automated check that no memory is allocated during processing of at least 1000 blocks. The "no locks, no I/O, bounded work" parts of FR-015 are verified by code review and by a check that processing time per sample does not grow with block size or with stream length.

**Integration surface**

- **FR-023**: The pipeline MUST be usable from hosts in other languages (C, Swift, and others) through a stable, language-neutral interface. That interface MUST offer: create (with settings), process a block, query latency, reset, reconfigure, destroy, and query the library version. It MUST expose only plain data types and MUST behave identically to the native interface, bit for bit.
- **FR-024**: Every failure through the language-neutral interface (unsupported rate, missing or invalid buffer, invalid settings) MUST be reported as an error value. A failure MUST never crash the host process or leave the pipeline unrecoverable. Ordinary errors (invalid arguments, unsupported rate) leave the pipeline unchanged and usable. After an internal error, the pipeline MAY refuse processing until `reset` or `reconfigure` succeeds, and either one MUST restore it to a working state. The interface is a versioned public surface under Semantic Versioning.

### Out of Scope

Microphone (stage 1), mic preamp (2), AGC (3), VAS (5), noise reduction (6), speech encoder/decoder (7, 9), flash storage (8), volume / power amp (11), speaker (12), earphone path (13), PCM quantization and companding (A-003 is still open), clock deviation (A-011: nominal 8 kHz only), modeling of DAC imaging residue above 4 kHz, battery effects (A-013), glitch-free changes to bypass or tap settings while streaming (FR-008), stereo or multichannel audio, and any file-format or command-line tooling.

### Key Entities

- **Pipeline**: one emulator instance. Holds its settings, host rate, internal state and reported latency. Processes blocks of mono audio.
- **Settings**: host rate; record stage on/bypassed; playback stage on/bypassed; tap point; seed. Every default cites an A-/S- ID.
- **Stage**: one modeled signal-chain element (here stage 4 and stage 10), with a bypass flag and a measurable response defined by this spec.
- **Measurement result**: one harness check: stimulus, configuration, host rate, measured value, tolerance, pass/fail, and the requirement and A-/S- IDs it covers.
- **Golden reference**: stored expected output for a named stimulus, configuration and host rate.
- **Language-neutral interface**: the stable, versioned surface other languages use to create and drive a pipeline (FR-023, FR-024).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An app developer can go from nothing to hearing telephone-band output from their own audio with the default settings in under 15 minutes, using only the project's documentation.
- **SC-002**: 100% of the tolerance checks in FR-010 to FR-013 pass at all six supported host rates (8, 16, 44.1, 48, 88.2, 96 kHz), in every configuration.
- **SC-003**: The golden-file checks pass unchanged on all eight platform targets in FR-014. They run on every pull request on all targets with automated runners. The iOS-device run is documented and repeatable, and passes before each release. Output is also bit-identical across at least 100 random block partitions of each golden stimulus.
- **SC-004**: Zero memory allocations are observed while processing 1000+ blocks of varying size in any configuration.
- **SC-005**: Default-configuration latency is 20 ms or less, and reported latency matches measured latency within ±1 sample (engineering target) in 100% of configurations and rates.
- **SC-006**: Processing runs at least 20× faster than real time (engineering target) at 48 kHz on a typical developer laptop, leaving headroom for later stages.
- **SC-007**: Every default value and modeled tolerance in this feature traces to an S-/A- ID or is labeled as an engineering target. A review finds zero untraceable numbers.
- **SC-008**: A deliberate 100 Hz shift of either band edge is caught by the harness (both the response check and the golden check fail).

## Assumptions

- **A-001** (internal rate 8 kHz) and **A-002** (300–3400 Hz telephone-style band) are taken as given for this slice, from the existing register.
- **A-014** (new, registered with this spec): the stage 4 and stage 10 response tolerances in FR-010 are modeled on an ITU-T G.712-like voice-band template, and both stages share the same nominal shape. Confidence: Medium (band) / Low (exact template). Verify with the MSM7702 datasheet filter templates (S-003) or swept-sine captures from a real unit.
- **A-015** (new, registered with this spec): both band-limit stages have unity (0 dB) passband gain in the emulator. The device's absolute record and playback levels (codec gain settings, mic and speaker sensitivity) belong to other stages and later specs. Confidence: Low. Verify with level-calibrated captures from a real unit.
- **A-016** (new, registered with this spec): the MSM7702's record and playback filters are minimum-phase, not linear-phase, as is typical of telephone codec filters built to G.712, which limits group-delay distortion rather than requiring zero. Confidence: Medium. Verify with the MSM7702 datasheet (group delay specs, S-003) or impulse/step captures through a real unit.
- With both stages bypassed, the output still passes through the 8 kHz device-rate domain (FR-004). Callers who want the untouched host-rate signal should not run it through the pipeline.
- DAC imaging residue (energy above 4 kHz on playback, listed in the signal-chain table for stage 10) is not modeled in this slice. The playback stage's output is confined to the device band.
- Clock deviation (A-011) is fixed at nominal: exactly 8 kHz.
- No physical unit or reference recordings exist yet, so "accuracy" in this slice means meeting the stated tolerances, not matching a real device. Documentation will say "modeled on" / "assumed".
- The host is responsible for format conversion (e.g. integer PCM to floating point), channel down-mixing, and file I/O.
