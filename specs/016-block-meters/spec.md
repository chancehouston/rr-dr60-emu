# Feature Specification: Per-Block Meters

**Feature Branch**: `016-block-meters`

**Created**: 2026-10-29

**Status**: Draft (awaiting the owner's spec sign-off)

**Input**: User description: "016 per-block meters. As an app showing the signal chain live, I want one read-out per processing call of what the modeled stages did in that call, so that every moving element on screen is a number the engine computed, not a guess from the audio. Product request E-A (docs/product/front-and-inside.md › Engine requests) and controls-discussion.md § 4 item 3; engine order decided 2026-10-29 (decisions.md: 014 → 016 → 015 …). The read-out covers the record side and the playback side. Record side (the one-pass and the record path): the input's peak; the microphone amplifier's peak after its clipper (stage 2); the limiter's gain reduction as the call's deepest value in dB and its hold charge as a fraction 0–1 of the limiting ceiling (stage 3); the peak after the codec's input filter and the number of samples the converter's full-scale clamp bounded (stage 4); the voice-activation state at the end of the call and the hang time remaining before it pauses (stage 5). Playback side (the playback path and the one-pass): the peak after the playback reconstruction (stage 10); the peak at the route output and the number of samples the optional output clamp bounded (the clip indicator spec 010's review queued; stages 11 – 13). Peaks are sample peaks in full-scale units at the point where they are read; gain in dB; the clamp counts are per call. A pull API: the caller reads a plain struct of the last call's figures after each processing call, in Rust and in C, with no allocation and no I/O; the figures describe exactly the samples that call processed (including the stop flush and the drain). The meters never change the output or its determinism: every existing golden file and smoke hash stays byte-identical, and the read-out itself is deterministic (same input, settings and seed ⇒ the same figures on every platform). Bypassed stages read as their rest values, stated per meter. A tap before a stage leaves that stage's meters at rest. The cost per block must be small (a budget in percent against 0.14, like specs 010 – 013). Not a control: nothing is settable. Free tier. Out of scope: spectra, a scope copy of a stage (E-B), limiter ceiling events (E-E), per-sample meter streams, callbacks."

## Decisions for sign-off

The owner reads this section first. Each decision gives the recommendation; the alternative is in the Assumptions section.

| # | Decision | Recommended |
|---|---|---|
| D1 | **Eleven meters, in two groups.** Record side: input peak, microphone-amplifier peak (stage 2), limiter gain reduction and hold charge (stage 3), filter peak and converter-clamp count (stage 4), voice-activation state and hang remaining (stage 5). Playback side: reconstruction peak (stage 10), route-output peak and output-clamp count (stages 11 – 13). That is product request E-A plus the stage 2 peak (built since the request) and the two clamp counts (the clip indicator spec 010's review queued). | Eleven |
| D2 | **One read-out, read after a call.** The caller reads one plain record of the last processing call's figures, both groups in it. A record call fills the record group and leaves the playback group at rest; a play call the reverse; the one-pass fills both. No new processing-call variants, no callback. | One read-out |
| D3 | **Units.** Peaks are the largest absolute sample value in full-scale units at the point read (0 when no sample passed). Gain reduction is a non-negative dB figure (0 = unity gain). Hold charge is 0 (discharged) to 1 (the charge at the limiter's deepest modeled attenuation). Hang remaining is in whole device samples (6 kHz, exact; ÷ 6000 for seconds). Counts are whole samples. | As stated |
| D4 | **Peaks before the clamps.** The stage 4 filter peak is read before the converter clamp and the route-output peak before the optional output clamp, so a reading above full scale shows what the clamp removed and the count beside it says how often. | Before |
| D5 | **The C form carries a size field.** The read-out is a new C structure with a caller-set size (the rule the settings structure already follows), so later meters can be appended without the ABI break spec 013's play report took. The existing block, record and play reports are unchanged. | Size field |
| D6 | **Cost budget 5 %** of the per-sample cost on the default engine at 48 kHz, measured the way specs 010 – 013 did (fastest of 45 alternating runs, a runner ceiling of 8 %). The meters are always on: there is no switch to turn them off. | 5 %, always on |

## Overview

An app that shows the signal chain while it runs ("the inside", [front-and-inside.md](../../docs/product/front-and-inside.md)) needs to animate what each stage is doing: how hard the limiter is holding, whether the converter clipped, whether the voice activation is about to pause, how loud the speaker is being driven. Today the engine reports only lengths, counts of events and frames, and the voice activation's paused flag. Everything else an app would have to guess from the audio, and it cannot guess the limiter's charge or a clamp count at all.

This feature adds **per-block meters**: after every processing call, the caller can read one small, plain record of figures the engine computed while processing that call's samples. The figures are a pull: nothing is pushed, nothing is allocated, and the output is untouched. A reader who sees only the read-out can draw every moving element of the chain from numbers the engine itself produced.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Draw the chain from the engine's own figures (Priority: P1)

An app feeds audio through the one-pass, the recorder or the player, and after each call reads the meters: the peaks at five points of the chain, the limiter's gain reduction and charge, the two clamp counts, and the voice activation's state and remaining hang. It animates each stage from those figures alone.

**Why this priority**: it is the whole value of the feature; the inside view cannot be built honestly without it.

**Independent Test**: for each meter, a stimulus chosen for it is processed in blocks and the read-out is compared with an oracle computed outside the engine: the peaks with the largest absolute value of the same call's output at the matching tap; the gain reduction with the sample-by-sample ratio of the limiter's output to a limiter-bypassed reference; the charge with the limiter's known state; the clamp counts with the number of samples the clamped output differs from a clamp-bypassed twin; the voice-activation figures with the splice events already reported.

**Acceptance Scenarios**:

1. **Given** the one-pass at a tap at or after a stage, **When** a block is processed, **Then** that stage's peak equals the largest absolute value of the same block's output at the tap after that stage, read sample for sample, at every supported host rate (the tap chain is the oracle).
2. **Given** a 1 kHz tone stepped 10 dB above the limiter's knee, **When** blocks are processed through the onset and the hold, **Then** the gain reduction rises from 0 to within 0.1 dB of the reference model's steady figure and the charge from 0 toward 1 along the way, and after a long silence both return to exactly 0.
3. **Given** a full-scale square wave with the converter clamp on, **When** a block is processed, **Then** the converter-clamp count equals the number of samples at which the clamped output differs from the clamp-bypassed twin, and the filter peak is above 1.0 while the output never is.
4. **Given** the player on the speaker route with the output clamp on and a loud recording, **When** a block is played, **Then** the output-clamp count equals the number of samples bounded (spec 010's identity, counted) and the route peak is the pre-clamp value.
5. **Given** a burst followed by silence in drop mode, **When** blocks are processed, **Then** the hang remaining counts down by exactly the block's device samples from the hang length to 0, the state reads "recording" until the block in which it pauses and "paused" from then, and the block in which the pause happens is the block whose splice event is reported.
6. **Given** the recorder, **When** a block is recorded, **Then** the record group is filled and the playback group reads rest values; after a play call the reverse; after the stop flush, the flush's figures are readable; during the drain, the playback figures fall toward rest.

---

### User Story 2 - The meters never change the sound (Priority: P2)

A maintainer adds the meters and every existing output stays bit-identical: every golden file, every smoke hash, at every rate and tap, and reading the meters costs almost nothing.

**Why this priority**: an observation that moves what it observes is worse than none; the inside view must be free to run at every frame.

**Independent Test**: the committed golden files and SHA guards are unmoved; a build timing comparison shows the cost within budget; a test proves the read-out allocates nothing and is bit-identical on every golden target.

**Acceptance Scenarios**:

1. **Given** the nine audio golden files of specs 001 – 013, **When** the test suite runs on the feature branch, **Then** every file and its SHA guard is byte-identical.
2. **Given** the default engine at 48 kHz, **When** the per-sample cost is compared with 0.14 in the same build, **Then** it is within the budget of D6.
3. **Given** the same input, settings and seed on every golden target, **When** the meters of every block are recorded, **Then** they are bit-identical across targets (a new golden file holds them).
4. **Given** a reader on another thread with no engine, or the engine between calls, **When** the read-out is taken, **Then** nothing is allocated and nothing blocks.

---

### User Story 3 - The same from C (Priority: P3)

The iOS app reads the meters through the C API after each call, into a structure it owns, with a size field so that a future addition does not break it.

**Why this priority**: the first app reaches the engine through C.

**Independent Test**: the C smoke program processes the stimulus of the Rust test and prints the meters of every block; the print equals the Rust test's; a too-small size is refused.

**Acceptance Scenarios**:

1. **Given** a C program, **When** it reads the meters after a one-pass, record or play call, **Then** it receives the Rust values field for field (exact for integers and the state; bit-identical for the floating figures).
2. **Given** a size smaller than this version's structure, **When** the read is attempted, **Then** it is refused with the existing invalid-argument status and nothing is written; a larger size (a later caller's) is accepted and the known fields are filled.
3. **Given** a null handle or a null out-pointer, **When** the read is attempted, **Then** it is refused and nothing is written.

### Edge Cases

- **Before any call, after `reset`, after `reconfigure`, after a zero-length call**: every figure is at rest (peaks 0, gain reduction 0, charge the limiter's actual charge, counts 0, state recording, hang remaining the full hang length). A zero-length call does not update the read-out of the last non-empty call; the rest values are what a fresh engine reads.
- **Rest values per meter** (what a bypassed stage or a stage after the tap reads): peaks 0; gain reduction 0 and charge 0 with the limiter bypassed; counts 0 with the clamp bypassed; state recording and hang remaining the full hang length with the voice activation bypassed (its counters do not run).
- **Charge persists across calls**: it is the limiter's state at the end of the call, not a per-call aggregate; with the limiter bypassed it reads 0.
- **A block the voice activation drops entirely** (drop mode, paused throughout): the stage 4 meters are still read (the filter and clamp run before the gate); the playback-side meters of the one-pass read 0 because no sample reached stage 10.
- **Mute mode**: muted samples are zeros past the gate; the playback-side peaks read what the zeros produce (the reconstruction's ringing, then 0).
- **The stop flush**: `stop_recording` feeds silence through the record path; the read-out after it holds the flush's figures (hiss-level peaks with the capsule on, the limiter releasing).
- **The drain**: play calls past the recording's end process the output chain's settle; the playback peaks fall toward 0 and the counts are 0 unless the settle itself is bounded.
- **The speed control**: the reconstruction peak is read after the resampler and stage 10 at the device rate, whatever the level.
- **A tap before stage 10 in the one-pass** (`AfterAgc`, `AfterRecord`, `AfterVas`, `AfterDecoder`, the microphone taps): the playback-side meters read rest; a tap at `AfterMicCapsule` leaves even the stage 2 peak at rest; `AfterAgc` and the microphone taps leave the stage 4 and 5 meters at rest. The route-output peak is read at `RouteOutput` only; at `AfterVolumeAmp`, `AfterSpeaker` and `AfterEarphone` it reads rest (the chain stops before the route output there), while the stage 10 peak is read at all four.
- **The recorder and the player ignore the tap**, as they do today; their meters are always the full groups.
- **Two engines**: each has its own read-out; the one-pass and the recorder/player of one engine share one read-out (D2), the last call's.
- **The input peak** is read after the engine's sanitizing of non-finite samples (spec 001), so it is always finite.

## Requirements *(mandatory)*

### Functional Requirements

**The read-out**

- **FR-001**: The engine MUST keep one **meter read-out**, a plain record of eleven figures in two groups, describing the samples of the most recent non-empty processing call (one-pass, record, play, or the stop flush), readable at any time without allocation, I/O or synchronization. Its initial value, and its value after `reset` and `reconfigure`, is the rest state of Edge Cases.
- **FR-002**: **Record group** (filled by the one-pass, the record call and the stop flush): (a) the **input peak**, the largest absolute sanitized input sample; (b) the **microphone peak**, the largest absolute sample after stage 2's clipper (after stage 1 with stage 2 bypassed; the input with both bypassed); (c) the **gain reduction**, the largest attenuation the limiter applied to any sample of the call, in non-negative dB; (d) the **hold charge**, the limiter's hold at the end of the call as a fraction 0 – 1 of the charge at its deepest modeled attenuation; (e) the **filter peak**, the largest absolute sample after stage 4's band-limit, before the converter clamp; (f) the **converter-clamp count**, the number of samples of the call the converter clamp bounded; (g) the **voice-activation state** at the end of the call, recording or paused; (h) the **hang remaining**, the device samples of silence still allowed before the voice activation pauses, the full hang length whenever the last sample was sound or the gate is paused or bypassed.
- **FR-003**: **Playback group** (filled by the one-pass at a tap at or after stage 10, the play call and the drain): (i) the **reconstruction peak**, the largest absolute sample after stage 10 (after the speed resampler and the decoder, at the device rate); (j) the **route peak**, the largest absolute sample at the selected route's output before the optional output clamp, at the host rate; (k) the **output-clamp count**, the number of samples of the call the output clamp bounded (0 with the option off, exactly spec 010's identity counted).
- **FR-004**: A call that does not run a stage (the stage bypassed, the tap before it, the other path's call) MUST leave that stage's meters at their rest values of Edge Cases for that call. The group a call does not run reads rest after that call.
- **FR-005**: Every peak MUST equal the largest absolute value of the same call's samples at the point read, computed from the same values the output is computed from, so that a peak at a tapped point equals the largest absolute value of that call's output at that tap exactly (sample for sample, after the same narrowing the output takes).
- **FR-006**: The gain reduction and the charge MUST be computed from the limiter's own gain and state: the gain reduction within 0.01 dB of the attenuation measured as the ratio of the limiter's output to a limiter-bypassed reference on the same sample, and the charge equal to the limiter's hold divided by the hold at its deepest modeled attenuation, clamped to 1.
- **FR-007**: The counts MUST equal the number of samples at which the clamped value differs from the unclamped one (a sample exactly at full scale is not counted).
- **FR-008**: The hang remaining and the state MUST agree with the voice activation's splice events: the call in which the hang remaining reaches 0 is the call that reports the pause's event, and the state flips in the same call.

**Access**

- **FR-009**: The read-out MUST be readable in Rust as a copyable value with public fields and in C as a plain structure with a caller-set size field (D5): a size smaller than this version's is refused with the existing invalid-argument status and nothing written; a larger size is accepted and the known fields filled. A null handle or out-pointer is refused. The existing block, record and play reports and the settings structure are unchanged; the header is regenerated.
- **FR-010**: Reading the read-out MUST be real-time safe (no allocation, no lock, no I/O, bounded work) and MUST NOT change the engine's state or its next output.

**Determinism, cost and guards**

- **FR-011**: The meters MUST NOT change any output: every audio golden file of specs 001 – 013 and its SHA guard MUST be byte-identical, every smoke hash unchanged, the stage and control manifests' canonical texts changed only in their version lines.
- **FR-012**: The read-out MUST be deterministic: same input, settings and seed give bit-identical figures on every golden target, in Rust and in C. A new golden file (`golden-meters-v1.json`) MUST hold the read-out of every block for a set of stimuli, configurations and the seven host rates, including entries for each path (one-pass, record, play, the stop flush, the drain).
- **FR-013**: The per-sample cost of the default engine at 48 kHz, meters always on, MUST be within D6's budget against 0.14, measured in the same build by the 45-alternating-run estimator of spec 012, with the runner ceiling in CI; the bypassed-chain cost (every stage off) within the same budget.
- **FR-014**: **Documentation.** The user guide MUST show how to read the meters in Rust and C and list each meter with its unit and rest value; the README's real-time list MUST include the read; the CHANGELOG MUST record the feature as version 0.15.0; the stage manifest's "Rust API items" of stages 2, 3, 4, 5, 10, 11 – 13 MUST name the read-out's fields (a re-bless of the canonical text); front-and-inside.md's E-A status MUST say built.
- **FR-015**: The meters are **not controls**: nothing is settable, the control manifest gains no entry, and no assumption is registered (the figures are read-outs of existing modeled behavior; the registers are cited through the stages they read).

### Out of Scope

- A scope copy of a stage's samples (E-B); limiter ceiling events with positions (E-E); per-sample meter streams; callbacks; spectra, RMS, loudness or any averaged figure (the app averages the peaks it reads); a switch to turn the meters off (D6); a count of hiss-only blocks; meters for stages 7 – 9 beyond what the play report already gives (frames, place); the volume knob's current position (a control's read-back, spec 015's domain).
- Sample-positioned control changes and the change log (spec 015).

### Key Entities

- **Meter read-out**: the eleven figures of FR-002 – FR-003 in two groups; one per engine; the last non-empty call's.
- **Rest state**: the read-out of a fresh engine: peaks 0, gain reduction 0, charge the limiter's state (0 when fresh), counts 0, state recording, hang remaining the full hang length.

| Meter | Group | Stage | Unit | Rest |
|---|---|---|---|---|
| input peak | record | input | full-scale, ≥ 0 | 0 |
| microphone peak | record | 2 (1 if 2 bypassed) | full-scale, ≥ 0 | 0 |
| gain reduction | record | 3 | dB, ≥ 0 | 0 |
| hold charge | record | 3 | fraction 0 – 1 | 0 |
| filter peak | record | 4 | full-scale, ≥ 0 | 0 |
| converter-clamp count | record | 4 | samples | 0 |
| voice-activation state | record | 5 | recording / paused | recording |
| hang remaining | record | 5 | device samples | hang length |
| reconstruction peak | playback | 10 | full-scale, ≥ 0 | 0 |
| route peak | playback | 12 or 13 | full-scale, ≥ 0 | 0 |
| output-clamp count | playback | route output | samples | 0 |

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every peak equals the harness's own largest absolute value of the matching tap's output, exactly, for every block of every stimulus at all seven host rates (five peaks × seven rates, 0 mismatches).
- **SC-002**: The gain reduction is within 0.01 dB of the measured attenuation and the steady figure on a 10 dB over-knee tone within 0.1 dB of the reference model; the charge reaches ≥ 0.99 of the model's steady charge ratio and returns to exactly 0 after 140 s of silence.
- **SC-003**: Each clamp count equals the differing-sample count of the clamp-bypassed twin for every block (0 mismatches at seven rates); the hang remaining and the state agree with every splice event (0 mismatches).
- **SC-004**: The nine audio golden files and their SHA guards are byte-identical; the new meters golden is reproduced on every golden target and by the C smoke program.
- **SC-005**: The per-sample cost at 48 kHz on the default engine is within 5 % of 0.14 (8 % on the CI runner), and the bypassed chain within the same budget.
- **SC-006**: Coverage stays at or above 80 % and within 1 point of the baseline (0.14: 97.81 % locally).

## Assumptions

- **D1 alternative:** E-A's seven figures only. Rejected because the stage 2 clipper (spec 012) is an audible stage the inside shows, and the clip indicator was queued by spec 010 for exactly this feature; a third alternative, meters for every tap, is more than the inside asked for and each would need its own oracle.
- **D2 alternative:** a new processing-call variant that takes a caller-provided record to fill (the wording of E-A). Rejected because it doubles the call surface in Rust and C for the same figures; a read after the call carries exactly the same information with no allocation. An app on a different thread than the audio callback copies the record out in the callback, as it does the block report.
- **D3 alternative:** gain in signed dB (≤ 0) and hang remaining in milliseconds. Non-negative "reduction" matches the request's wording and reads naturally on a meter; device samples are exact and the hang length is already in the settings in milliseconds for the conversion.
- **D4 alternative:** peaks after the clamps. Rejected because the peak would then saturate at 1.0 and say nothing the count does not.
- **D5 alternative:** appending the figures to the existing block, record and play reports. Rejected because those `repr(C)` reports have no size field and growing them is an ABI break (spec 013), and because one read-out serves all three calls.
- **D6 alternative:** a switch to turn the meters off. Rejected because the figures are a few compare-and-select operations per sample and a switch is one more setting, control-manifest entry and golden configuration; if the measured cost exceeds the budget, the plan reconsiders.
- The hold at the limiter's deepest modeled attenuation is the model's existing constant (spec 005), so the charge needs no new assumption.
- "Full-scale units" are the engine's existing dBFS scale (A-030: ±1.0 at the converter's full scale; spec 009's route scale, A-042, at the route output).
- **Spec-Kit process for this feature**, per the owner's 2026-10-29 cadence: owner sign-offs at this spec, the plan and the merge; reviews and phases otherwise proceed and stop only on a failed check or a new decision.
- No release is published; the CHANGELOG records the feature as 0.15.0, a minor version with new types and functions only.
