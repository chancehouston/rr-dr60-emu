# Feature Specification: Per-Block Meters

**Feature Branch**: `016-block-meters`

**Created**: 2026-10-29

**Status**: Spec and plan approved by the owner (2026-10-29, 2026-10-30); implementing

**Input**: User description: "016 per-block meters. As an app showing the signal chain live, I want one read-out per processing call of what the modeled stages did in that call, so that every moving element on screen is a number the engine computed, not a guess from the audio. Product request E-A (docs/product/front-and-inside.md › Engine requests) and controls-discussion.md § 4 item 3; engine order decided 2026-10-29 (decisions.md: 014 → 016 → 015 …). The read-out covers the record side and the playback side. Record side (the one-pass and the record path): the input's peak; the microphone amplifier's peak after its clipper (stage 2); the limiter's gain reduction as the call's deepest value in dB and its hold charge as a fraction 0–1 of the limiting ceiling (stage 3); the peak after the codec's input filter and the number of samples the converter's full-scale clamp bounded (stage 4); the voice-activation state at the end of the call and the hang time remaining before it pauses (stage 5). Playback side (the playback path and the one-pass): the peak after the playback reconstruction (stage 10); the peak at the route output and the number of samples the optional output clamp bounded (the clip indicator spec 010's review queued; stages 11 – 13). Peaks are sample peaks in full-scale units at the point where they are read; gain in dB; the clamp counts are per call. A pull API: the caller reads a plain struct of the last call's figures after each processing call, in Rust and in C, with no allocation and no I/O; the figures describe exactly the samples that call processed (including the stop flush and the drain). The meters never change the output or its determinism: every existing golden file and smoke hash stays byte-identical, and the read-out itself is deterministic (same input, settings and seed ⇒ the same figures on every platform). Bypassed stages read as their rest values, stated per meter. A tap before a stage leaves that stage's meters at rest. The cost per block must be small (a budget in percent against 0.14, like specs 010 – 013). Not a control: nothing is settable. Free tier. Out of scope: spectra, a scope copy of a stage (E-B), limiter ceiling events (E-E), per-sample meter streams, callbacks."

## Decisions for sign-off

The owner reads this section first. Each decision gives the recommendation; the alternative is in the Assumptions section.

| # | Decision | Recommended |
|---|---|---|
| D1 | **Eleven meters, in two groups.** Record side: input peak, limiter-input peak (after stages 1 – 2), limiter gain reduction and hold charge (stage 3), filter peak and converter-clamp count (stage 4), voice-activation state and hang remaining (stage 5). Playback side: reconstruction peak (after stage 10), route-output peak and output-clamp count (stages 11 – 13). That is product request E-A plus the peak after the microphone stages (built since the request) and the two clamp counts (the clip indicator spec 010's review queued). | Eleven |
| D2 | **One read-out, read after a call, on the engine's thread.** The caller reads one plain record of the last non-empty processing call's figures, both groups in it. A record call fills the record group and leaves the playback group at rest; a play call the reverse; the one-pass fills both. No new processing-call variants, no callback; an app on another thread copies the record out in its audio callback, as it does the block report. | One read-out |
| D3 | **Units.** Peaks are the largest absolute sample value in full-scale units at the point read (0 when no sample passed). Gain reduction is a non-negative dB figure (0 = unity gain). Hold charge is 0 (discharged) to 1 (the model's deepest attenuation, 40 dB; the knee sits near 0.73, see FR-006). Hang remaining is in whole device samples (6 kHz, exact; ÷ 6000 for seconds): the full hang length when the last sample was sound or the gate is bypassed, **0 while paused**. The converter-clamp count is in device samples, the output-clamp count in host samples. The reconstruction peak is read **at the host rate**, after the conversion and before the output chain. | As stated |
| D4 | **Peaks before the clamps.** The stage 4 filter peak is read before the converter clamp and the route-output peak before the optional output clamp, so a reading above full scale shows what the clamp removed and the count beside it says how often. | Before |
| D5 | **The C form carries a size field.** The read-out is a new C structure with a caller-set size (the rule the settings structure already follows), so later meters can be appended without the ABI break spec 013's play report took; a larger caller size has its tail zero-filled. The existing block, record and play reports are unchanged. | Size field |
| D6 | **Cost budget 5 %** of the per-sample cost on the default engine at 48 kHz against 0.14, as a build comparison (fastest of 45 alternating runs, solo), with a **12 % runner ceiling** (the runner's observed ratio of 2.5×); the bypassed chain within the **same absolute cost in nanoseconds** as the default's budget. The meters are always on: there is no switch to turn them off. | 5 % / 12 % / same ns, always on |
| D7 | **Peaks are points, the rest are stages.** A peak reads the largest sample at its point whenever the call reaches that point, whatever ran before it (the limiter-input peak reads the sanitized input with stages 1 – 2 bypassed; the route peak reads the route output with the chain bypassed). The stage-owned meters (gain reduction, charge, the counts, the state, the hang) read rest when their stage does not run. | Points |

## Clarifications

### Session 2026-10-29 (owner sign-off)

- Q: Approve the spec with D1 – D7 as recommended? → A: **Yes**, all seven as recommended. The clarify step is covered by these decisions and the two reviews below.

### Session 2026-10-29 (speckit-coach review of the draft, findings 1 – 16; hw-emulation-reviewer B1 – B14)

Applied at the recommended defaults, before the owner's sign-off:

- **The peak oracle** (coach 1, B3, B6): the one-pass taps deliver host-rate samples after the conversion and after the clamps, so "equal to the tap's output" holds only for host-rate points with the clamp bypassed. The reconstruction peak moved to the host rate after the conversion (its tap oracle is then exact and the figure is comparable with the route peak; a device-rate sample peak of a tone reads phase, not level, the 009 lesson). The filter peak stays at the device rate before the converter clamp: exact against the tap at 6 kHz with the clamp bypassed, and against the harness's own copy of the stage's stream at the other rates (FR-005, SC-001).
- **The voice activation** (coach 2, 3, B1, B2): a drop-mode splice event is reported at the resume, never at the pause; the state oracle is the existing `paused` report, and the events corroborate the paused count. The countdown reads 0 at the last kept non-sound sample; the pause lands on the next non-sound sample, possibly in the next call (FR-002, FR-008). Hang remaining reads 0 while paused (D3; the gate's counter is frozen then, so "full" would mislead).
- **Peaks are points** (coach 6, B4): D7; the microphone peak renamed the limiter-input peak and read at stage 2's output (the tap's point), not inside the stage after the clipper.
- **The hold charge** (coach 4, 12, B7): rest 0 everywhere; it is state, not a per-call aggregate (a silent block reads the decaying charge); the scale is the model's existing 40 dB cap (spec 005 R-03), with the landmarks in FR-006 and the model script printing the steady ratio SC-002 pins.
- **Threading** (coach 13, B5): the read-out is read between calls on the thread that owns the engine; FR-010 and US2 AS4 restated.
- **Transport calls** (coach 5, 7, B11): a zero-length call, a skipped flush and the start/stop/erase calls leave the read-out unchanged; the flush reports the gate's own state machine while the stop holds the gate open, as `RecordInfo.paused` does.
- **Cost** (coach 8, B9): a build comparison, not an in-build twin (there is no off switch); the figures in D6.
- **Records** (coach 9, 10, 14, 15, 16; B10, B12, B13, B14): the stage manifest's API-item strings change (FR-011, FR-014); count units stated; a larger C size zero-filled; the tap list completed; the golden holds short stimuli in full and digests otherwise; C field types stated in FR-009; the scenario stimuli bypass the stages that would confound them (AS2, AS3).
- **Gain-reduction oracle** (coach 11, B8): compared on samples whose reference magnitude is at least −60 dBFS, onset blocks included; the model's steady figure is a mean and the meter the deepest, the in-cycle ripple inside the 0.1 dB tolerance.

### Session 2026-10-30 (plan reviews: coach 1 – 13, hardware P1 – P12, product M1 – M7; applied at the defaults)

- **Field names** (M3): the stage 4 figures are `record_filter_peak` and `adc_clamp_count` (the setting they report is `adc_clamp_enabled`); the prose keeps "record-filter peak" and "converter-clamp count".
- **FR-008's event corroboration, by path** (P4): the recorder's splice `input_length` equals the reference gate's dropped device samples D since the flip; the one-pass places events in host output samples with the cumulative-floor rule, so there it equals ⌊D·m/l⌋ − ⌊D_prev·m/l⌋. The per-block state and hang are checked against the reference gate, not against a count the read-out does not carry.
- **SC-002's "exactly 0 after the flush length"** (coach 9) runs release-only (`--ignored`) at 6 kHz, as spec 005's state-equality tests do; the steady figures are pinned per host rate from the model's print (P6).
- **Edge case narrowed** (coach 11): "a block the voice activation drops entirely leaves the one-pass's playback group at 0" holds with the speech codec bypassed (with it on, earlier frames still leave the decoder's delay line); the test uses the codec bypassed.
- **Out of scope, added** (M2 b): a timeline map of each figure to input and recording sample positions (investigation-mode.md § 5), which belongs with the sample-positioned changes (spec 015) or the session contract (spec 020).
- Q: Approve the plan with the two defaults (the stage 4 field names; the A/B asserts and fails CI over budget)? → A: **Yes** (owner plan sign-off, 2026-10-30); tasks, analyze and implement proceed to the merge, stopping only on a failed check or a new decision.
- **Restated at implementation** (post-implementation coach review, 2026-10-30): FR-011's control-manifest clause (the text carries no version line, so it is unchanged); FR-014's real-time list is the guide's; SC-004's C smoke reproduces one entry's one-pass rows, the plan's design (P9).
- **The cost protocol** (coach 1 – 2, P1, P10): "the default engine" is the codec-bypassed default front, as specs 009 – 013 measure; the bypassed chain is every stage off; both figures come from one timing test built against the 0.14 API and run in both builds by the A/B script, which asserts the budgets and fails the CI job when over.

## Overview

An app that shows the signal chain while it runs ("the inside", [front-and-inside.md](../../docs/product/front-and-inside.md)) needs to animate what each stage is doing: how hard the limiter is holding, whether the converter clipped, whether the voice activation is about to pause, how loud the speaker is being driven. Today the engine reports only lengths, counts of events and frames, and the voice activation's paused flag. Everything else an app would have to guess from the audio, and it cannot guess the limiter's charge or a clamp count at all.

This feature adds **per-block meters**: after every processing call, the caller can read one small, plain record of figures the engine computed while processing that call's samples. The figures are a pull: nothing is pushed, nothing is allocated, and the output is untouched. A reader who sees only the read-out can draw every moving element of the chain from numbers the engine itself produced.

The figures of a call describe the samples that call **processed**, not the samples it output: the stage 4 and 5 figures and the record group of the recorder describe device samples that leave the output up to the path's latency later (FR-014 documents each meter's lead).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Draw the chain from the engine's own figures (Priority: P1)

An app feeds audio through the one-pass, the recorder or the player, and after each call reads the meters: the peaks at five points of the chain, the limiter's gain reduction and charge, the two clamp counts, and the voice activation's state and remaining hang. It animates each stage from those figures alone.

**Why this priority**: it is the whole value of the feature; the inside view cannot be built honestly without it.

**Independent Test**: for each meter, a stimulus chosen for it is processed in blocks and the read-out is compared with an oracle computed outside the engine: the host-rate peaks with the largest absolute value of the same call's output at the matching tap (the clamp bypassed); the filter peak with the tap at 6 kHz and with the harness's copy of the stage's stream at the other rates; the gain reduction with the sample-by-sample ratio of the limiter's output to a limiter-bypassed reference; the charge with the limiter's known state; the clamp counts with the number of samples the clamped output differs from a clamp-bypassed twin; the voice-activation figures with the existing paused report and a harness reference gate, corroborated by the splice events.

**Acceptance Scenarios**:

1. **Given** the one-pass at the tap after a host-rate point (`AfterMicAmp`, `AfterPlayback`, `RouteOutput` with the output clamp off), **When** a block is processed, **Then** that point's peak equals the largest absolute value of the same block's output at that tap exactly, at every supported host rate; the input peak equals the largest absolute value of the sanitized input; and the filter peak equals the largest absolute value of the block's `AfterRecord` output at 6 kHz with the converter clamp off, and the harness's stage-stream copy at the other rates.
2. **Given** a 1 kHz tone stepped 10 dB above the limiter's knee (starting on a host-grid peak, stages 1 – 2 bypassed), **When** blocks are processed through the onset and the hold, **Then** the gain reduction on every block, onset blocks included, is within 0.01 dB of the deepest measured attenuation among the block's qualifying samples, its steady value is within 0.1 dB of the reference model's steady figure, the charge settles within 1 % of the model's steady ratio, and after the spec 005 flush length of silence both read exactly 0.
3. **Given** a full-scale square wave with stages 1 – 3 bypassed and the converter clamp on, **When** a block is processed, **Then** the converter-clamp count equals the number of device samples at which the clamped stream differs from the clamp-bypassed twin, and the filter peak is above 1.0 while the output never is.
4. **Given** the player on the speaker route with the output clamp on and a loud recording, **When** a block is played, **Then** the output-clamp count equals the number of host samples bounded (spec 010's identity, counted) and the route peak is the pre-clamp value, equal to the clamp-off twin's route peak.
5. **Given** a burst followed by silence in drop mode, **When** blocks are processed, **Then** the hang remaining counts down from the hang length by the device samples the conversion produced in each block, reads 0 at the block that keeps the last non-sound sample, the state reads "recording" through that block and "paused" from the block that processes the next non-sound device sample, which is the block whose block report first reads paused; the hang remaining reads 0 while paused; and at the resume the splice event's removed length equals the device samples the read-out reported as paused.
6. **Given** the recorder, **When** a block is recorded, **Then** the record group is filled and the playback group reads rest values; after a play call the reverse; after `stop_recording` the flush's figures are readable, with the state and hang reporting the gate's own state machine even while the stop holds the gate open; during the drain, the playback figures fall toward rest.

---

### User Story 2 - The meters never change the sound (Priority: P2)

A maintainer adds the meters and every existing output stays bit-identical: every golden file, every smoke hash, at every rate and tap, and reading the meters costs almost nothing.

**Why this priority**: an observation that moves what it observes is worse than none; the inside view must be free to run at every frame.

**Independent Test**: the committed golden files and SHA guards are unmoved; a build comparison shows the cost within budget; a test proves the read-out allocates nothing and is bit-identical on every golden target.

**Acceptance Scenarios**:

1. **Given** the nine audio golden files of specs 001 – 013, **When** the test suite runs on the feature branch, **Then** every file and its SHA guard is byte-identical.
2. **Given** the default engine at 48 kHz, **When** its per-sample cost is compared with the 0.14 merge commit's build, **Then** it is within the budget of D6; the bypassed chain within the same absolute cost.
3. **Given** the same input, settings and seed on every golden target, **When** the meters of every block are recorded, **Then** they are bit-identical across targets (a new golden file holds them).
4. **Given** the engine between calls, **When** the read-out is taken twice, **Then** the read allocates nothing, makes no blocking call, returns the same values both times and leaves the next output unchanged; a copy taken inside an audio callback is a plain value another thread can read without touching the engine.

---

### User Story 3 - The same from C (Priority: P3)

The iOS app reads the meters through the C API after each call, into a structure it owns, with a size field so that a future addition does not break it.

**Why this priority**: the first app reaches the engine through C.

**Independent Test**: the C smoke program processes the stimulus of the Rust test and prints the meters of every block; the print equals the Rust test's; a too-small size is refused.

**Acceptance Scenarios**:

1. **Given** a C program, **When** it reads the meters after a one-pass, record or play call, **Then** it receives the Rust values field for field (exact for the integers and the state; bit-identical for the floating figures).
2. **Given** a size smaller than this version's structure, **When** the read is attempted, **Then** it is refused with the existing invalid-argument status and nothing is written; a larger size (a later caller's) is accepted, the known fields are filled and the bytes beyond this version's structure are zeroed.
3. **Given** a null handle or a null out-pointer, **When** the read is attempted, **Then** it is refused and nothing is written.

### Edge Cases

- **Before any call, after `reset` and after `reconfigure`**: the rest state (the table in Key Entities). **A zero-length call**, a `stop_recording` whose flush is skipped (nothing was fed), `start_recording`, `start_playback`, `stop_playback` and `erase` leave the read-out unchanged: it is always the last non-empty processing call's.
- **Rest values per meter**: peaks 0; gain reduction 0 and charge 0 with the limiter bypassed or not reached; counts 0 with the clamp bypassed or not reached; state recording and hang remaining the full hang length with the voice activation bypassed or not reached.
- **The charge is state**: after any non-empty call that ran the limiter it is the hold at the end of that call, whatever the call contained (a silent block reads the decaying charge, not 0).
- **A block the voice activation drops entirely** (drop mode, paused throughout): the stage 4 meters are still read (the filter and clamp run before the gate); the playback-side meters of the one-pass read 0 because no sample reached stage 10 (with the speech codec bypassed; with it on, earlier frames still leave the decoder's delay line).
- **Mute mode**: muted samples are zeros past the gate; the playback-side peaks read what the zeros produce (the reconstruction's ringing, then 0).
- **The stop flush**: `stop_recording` feeds silence through the record path; the read-out after it holds the flush's figures (hiss-level peaks with the capsule on, the limiter releasing). In drop mode the stop holds the gate open; the state and hang still report the gate's own state machine, as `RecordInfo.paused` does, so the state can read paused while every flush sample is kept.
- **The drain**: play calls past the recording's end process the output chain's settle; the playback peaks fall toward 0 and the counts are 0 unless the settle itself is bounded.
- **The speed control**: the reconstruction peak is read after the resampler, stage 10 and the conversion to the host rate, whatever the level.
- **Taps in the one-pass.** `AfterMicCapsule`: only the input peak is read (nothing after stage 1 runs; the limiter-input peak, read at stage 2's output, is rest). `AfterMicAmp`: the input and limiter-input peaks. `AfterAgc`: those plus the stage 3 meters. `AfterRecord`: plus the stage 4 meters; stage 5 at rest (the gate does not run there). `AfterVas` and `AfterDecoder`: the whole record group; the playback group rest. `AfterPlayback`, `AfterVolumeAmp`, `AfterSpeaker`, `AfterEarphone`: plus the reconstruction peak; the route peak and the output-clamp count rest (the chain stops before the route output). `RouteOutput`: everything.
- **The recorder and the player ignore the tap**, as they do today; their meters are always the full groups.
- **The output chain bypassed**: the route output still exists and is still clamped (spec 010), so the route peak and the count are read.
- **Two engines**: each has its own read-out; the one-pass and the recorder/player of one engine share one read-out (D2), the last call's.
- **The input peak** is read after the engine's sanitizing of non-finite samples (spec 001), so it is always finite; no meter is ever NaN.

## Requirements *(mandatory)*

### Functional Requirements

**The read-out**

- **FR-001**: The engine MUST keep one **meter read-out**, a plain record of eleven figures in two groups, describing the samples of the most recent non-empty processing call (one-pass, record, play, or the stop flush), readable between calls on the thread that owns the engine, without allocation or I/O. Its initial value, and its value after `reset` and `reconfigure`, is the rest state of Key Entities.
- **FR-002**: **Record group** (filled by the one-pass, the record call and the stop flush): (a) the **input peak**, the largest absolute sanitized input sample; (b) the **limiter-input peak**, the largest absolute sample entering stage 3 (stage 2's output; stage 1's with stage 2 bypassed; the sanitized input with both bypassed); (c) the **gain reduction**, the largest attenuation the limiter applied to any sample of the call, in non-negative dB; (d) the **hold charge**, the limiter's hold at the end of the call as a fraction 0 – 1 of the hold at its deepest modeled attenuation; (e) the **filter peak**, the largest absolute device sample after stage 4's band-limit, before the converter clamp; (f) the **converter-clamp count**, the device samples of the call the converter clamp bounded; (g) the **voice-activation state** at the end of the call, recording or paused; (h) the **hang remaining**, the further non-sound device samples the gate will still keep before pausing: the hang length minus the non-sound samples since the last sound, saturating at 0 (0 means the next non-sound device sample pauses the gate); the full hang length when the last sample was sound or the gate is bypassed or not reached; 0 while paused.
- **FR-003**: **Playback group** (filled by the one-pass at a tap at or after stage 10, the play call and the drain): (i) the **reconstruction peak**, the largest absolute host sample after stage 10 and the conversion to the host rate, before the output chain (the chain's input; at 6 kHz the device rate); (j) the **route peak**, the largest absolute host sample at the selected route's output before the optional output clamp, also with the output chain bypassed; (k) the **output-clamp count**, the host samples of the call the output clamp bounded (0 with the option off; exactly spec 010's identity counted).
- **FR-004**: A peak MUST be read whenever the call reaches its point, whatever stages ran before it (D7). A stage-owned meter (c, d, f, g, h, k) MUST read its rest value for a call that does not run its stage (bypassed, the tap before it, the other path's call). The group a call does not run reads rest after that call.
- **FR-005**: Every peak MUST equal the largest absolute value of the samples that passed its point in that call, computed from the same values the output is computed from, with one narrowing to the output's precision per call (the narrowing is monotone, so this equals the largest absolute narrowed sample). For the host-rate points (a, b, i, j) at a tap at that point, the peak equals the largest absolute value of that call's output at the tap exactly, with the clamp after the point bypassed; the filter peak (e) equals the `AfterRecord` output's at 6 kHz with the converter clamp bypassed.
- **FR-006**: The gain reduction and the charge MUST be computed from the limiter's own gain and state: the gain reduction is the deepest gain applied in the call (the gain each sample received, not the state after it), within 0.01 dB of the attenuation measured as the ratio of the limiter's output to a limiter-bypassed reference on the same sample over samples whose reference magnitude is at least −60 dBFS; the charge equals the hold divided by the model's hold at its deepest modeled attenuation (40 dB, spec 005 R-03, settings-independent), clamped to 1. Landmarks the guide states: about 0.13 is the unity floor (below it the gain is exactly 1), about 0.73 the knee (1 dB), 0.84 at 10 dB, 0.90 at 20 dB, 1.0 at 40 dB.
- **FR-007**: The counts MUST equal the number of samples at which the clamped value differs from the unclamped one (a sample exactly at full scale is not counted).
- **FR-008**: The state and the hang remaining MUST agree with the engine's existing reports: the call after which the state first reads paused is the call whose block, record or flush report first reads paused, and the state returns to recording in the call whose report returns to not paused; in drop mode the splice event reported at the resume has a removed length equal to the device samples the read-out reported as paused since the flip; in mute mode the region's start lies in the call in which the state first read paused. The countdown and the state are checked against a harness reference gate fed the same device samples.

**Access**

- **FR-009**: The read-out MUST be readable in Rust as a copyable value with public fields and in C as a plain structure with a caller-set size field (D5): a size smaller than this version's is refused with the existing invalid-argument status and nothing written; a larger size is accepted, the known fields filled and the tail zeroed. A null handle or out-pointer is refused. C field types: single-precision for the peaks (the narrowed value is exactly one), the gain reduction and the charge (a cast of the deterministic double); 32-bit unsigned counts; a 64-bit unsigned hang; the state an integer flag. The existing block, record and play reports and the settings structure are unchanged; the header is regenerated.
- **FR-010**: Reading the read-out MUST be real-time safe (no allocation, no lock, no I/O, bounded work) and MUST NOT change the engine's state or its next output. Concurrent access to one engine is the host's responsibility, as for every other call.

**Determinism, cost and guards**

- **FR-011**: The meters MUST NOT change any output: every audio golden file of specs 001 – 013 and its SHA guard MUST be byte-identical, every smoke hash unchanged, the control manifest's canonical text unchanged (it carries no version line), the stage manifest's changed only in its version line and the API-item strings FR-014 names; no stage's model version moves (a read-out is not a modeled behavior).
- **FR-012**: The read-out MUST be deterministic: same input, settings and seed give bit-identical figures on every golden target, in Rust and in C (the dB figure through the deterministic math crate, once per call). A new golden file (`golden-meters-v1.json`) MUST hold, per entry, the read-out of every block for stimuli of at most 0.5 s and a digest of the whole sequence plus named blocks otherwise, over a set of stimuli, configurations and the seven host rates, including entries for each path (one-pass, record, play, the stop flush, the drain).
- **FR-013**: The per-sample cost of the default engine at 48 kHz, meters always on, MUST be within D6's budget against the 0.14 merge commit, measured as a build comparison (`scripts/ab-timing.sh`, 45 alternating rounds, solo) on the development machine and on the CI runner through the `ab-timing` label, with the runner ceiling; the bypassed chain (every stage off) within the same absolute cost.
- **FR-014**: **Documentation.** The user guide MUST show how to read the meters in Rust and C and list each meter with its point, unit, rest value, its lead over the audible output (the stage 4 and 5 figures and the recorder's group describe device samples that leave the output up to the path's latency later), the charge landmarks of FR-006, and the assumptions each figure rests on (the limiter's on A-028, A-034, A-035; the hang on A-023; the counts on A-030 and A-042). The user guide's real-time list (the README's list moved there 2026-10-19) MUST include the read; the CHANGELOG MUST record the feature as version 0.15.0; the stage manifest's "Rust API items" of stages 1 – 5 and 10 – 13 MUST name the read-out (one verbatim path each; a re-bless of the canonical text); front-and-inside.md's E-A status MUST say built.
- **FR-015**: The meters are **not controls**: nothing is settable, the control manifest gains no entry, and no assumption is registered (the figures are read-outs of existing modeled behavior; the registers are cited through the stages they read).

### Out of Scope

- A scope copy of a stage's samples (E-B); limiter ceiling events with positions (E-E); per-sample meter streams; callbacks; spectra, RMS, loudness or any averaged figure (the app averages the peaks it reads); a switch to turn the meters off (D6); a count of hiss-only blocks; meters for stages 7 – 9 beyond what the play report already gives (frames, place); the volume knob's current position (a control's read-back, spec 015's domain); a timeline map of each figure to input and recording sample positions (with spec 015 or 020).
- Sample-positioned control changes and the change log (spec 015).

### Key Entities

- **Meter read-out**: the eleven figures of FR-002 – FR-003 in two groups; one per engine; the last non-empty call's.
- **Rest state**: the read-out of a fresh engine, the table's Rest column.

| Meter | Group | Point | Unit | Rest |
|---|---|---|---|---|
| input peak | record | the sanitized input | full-scale, ≥ 0 | 0 |
| limiter-input peak | record | stage 2's output (stages 1 – 2 as run) | full-scale, ≥ 0 | 0 |
| gain reduction | record | stage 3 | dB, ≥ 0 | 0 |
| hold charge | record | stage 3 | fraction 0 – 1 | 0 |
| filter peak | record | stage 4, before the clamp | full-scale, ≥ 0 | 0 |
| converter-clamp count | record | stage 4 | device samples | 0 |
| voice-activation state | record | stage 5 | recording / paused | recording |
| hang remaining | record | stage 5 | device samples | hang length |
| reconstruction peak | playback | after stage 10, at the host rate | full-scale, ≥ 0 | 0 |
| route peak | playback | the route output, before the clamp | full-scale, ≥ 0 | 0 |
| output-clamp count | playback | the route output | host samples | 0 |

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The four host-rate peaks equal the clamp-bypassed tap output's largest absolute value for every block at all seven host rates, and the filter peak equals it at 6 kHz and the harness's stage-stream copy at the other six rates: 0 mismatches.
- **SC-002**: The gain reduction is within 0.01 dB of the measured attenuation on every block, onset included, and its steady value on a 10 dB over-knee tone within 0.1 dB of the reference model's steady figure (the model's print, pinned); the charge settles within 1 % of the model's printed steady ratio and returns to exactly 0 after the spec 005 flush length of silence.
- **SC-003**: Each clamp count equals the differing-sample count of the clamp-bypassed twin for every block (0 mismatches at seven rates); the state and the hang remaining agree with the paused reports, the reference gate and every event's removed length (0 mismatches).
- **SC-004**: The nine audio golden files and their SHA guards are byte-identical; the new meters golden is reproduced on every golden target, and one entry's one-pass rows by the C smoke program (`--meters`, diffed against the Rust example `meters_rows`).
- **SC-005**: The per-sample cost at 48 kHz on the default engine is within 5 % of 0.14 (12 % on the CI runner), and the bypassed chain within the same absolute cost.
- **SC-006**: Coverage stays at or above 80 % and within 1 point of the baseline (0.14: 97.81 % locally).

## Assumptions

- **D1 alternative:** E-A's seven figures only. Rejected because the microphone stages (spec 012) are audible stages the inside shows, and the clip indicator was queued by spec 010 for exactly this feature; a third alternative, meters for every tap, is more than the inside asked for and each would need its own oracle.
- **D2 alternative:** a new processing-call variant that takes a caller-provided record to fill (the wording of E-A). Rejected because it doubles the call surface in Rust and C for the same figures; a read after the call carries exactly the same information with no allocation.
- **D3 alternatives:** gain in signed dB (≤ 0); hang remaining in milliseconds, or the full hang length while paused; the reconstruction peak at the device rate. Non-negative "reduction" matches the request's wording and reads naturally on a meter; device samples are exact and the hang length is already in the settings in milliseconds for the conversion; a "full" bar on a paused gate misleads; a device-rate peak has no exact tap oracle at six of seven rates and reads phase, not level, on a tone (the 009 lesson).
- **D4 alternative:** peaks after the clamps. Rejected because the peak would then saturate at 1.0 and say nothing the count does not.
- **D5 alternative:** appending the figures to the existing block, record and play reports. Rejected because those `repr(C)` reports have no size field and growing them is an ABI break (spec 013), and because one read-out serves all three calls.
- **D6 alternatives:** a switch to turn the meters off (rejected: the figures are a few compare-and-select operations per sample and a switch is one more setting, control-manifest entry and golden configuration); an in-build comparison (impossible without an off twin); 8 % on the runner (tight against the project's own data: spec 012 read 5.1 % here and 12.7 % there); 5 % of the bypassed chain (a smaller absolute budget on a much cheaper path where the meters still run in full).
- **D7 alternative:** the limiter-input peak strictly stage 2's, rest when bypassed. Rejected because "the level entering the limiter" is the figure the inside wants, and the route peak with the chain bypassed is still a real output.
- The hold at the limiter's deepest modeled attenuation is the model's existing constant (spec 005 R-03), so the charge needs no new assumption; the model script gains a print of the steady charge ratio the harness pins.
- "Full-scale units" are the engine's existing dBFS scale (A-030: ±1.0 at the converter's full scale; spec 009's route scale, A-042, at the route output).
- **Spec-Kit process for this feature**, per the owner's 2026-10-29 cadence: owner sign-offs at this spec, the plan and the merge; reviews and phases otherwise proceed and stop only on a failed check or a new decision.
- No release is published; the CHANGELOG records the feature as 0.15.0, a minor version with new types and functions only.
