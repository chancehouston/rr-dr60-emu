# Feature Specification: Live Record-Side Values

**Feature Branch**: `017-live-record-values`

**Created**: 2026-10-30

**Status**: Draft; awaiting the owner's sign-off on D1 – D9

**Input**: User description: "017 live record-side values. As an app recording a session, I want the record path's values (the microphone sensitivity, the voice activation's threshold, hang and onset, the limiter's knee, attack and release, the capsule's hiss offset) to change during a recording and during the one-pass at a sample position, through the schedule and the change log spec 015 built, so that a user can adjust the recorder while it records and the session reproduces bit for bit. Product request: controls-discussion.md § 2b (the per-control engine work), § 4 item 4; decisions.md 2026-10-29 (record-side values change between recordings first; live record-side changes only through sample-timed events, spec 017; never live: the codec, the memory, the host rate and latency-bearing bypasses); the engine order 014 → 016 → 015 → 017. Spec 015 reserved `Stream::Record` and a change form that carries any control the manifest numbers."

## Decisions for sign-off

The owner reads this section first. Each decision gives the recommendation; the alternative is in the Assumptions section.

| # | Decision | Recommended |
|---|---|---|
| D1 | **Eight values become live**, on the recorder and the one-pass: the microphone sensitivity (a device control), the voice activation's threshold, hang and onset and the limiter's knee, attack and release (model internals), and the capsule's hiss offset (a unit condition). Everything else on the record side stays idle (`reconfigure` only): the stage switches and bypasses (the limiter's, the voice activation's and its mode, the microphone's two, the record stage, the converter clamp), the speech codec, the memory, the host rate, the seed and the tap. | Eight values |
| D2 | **The record side has its own clock: input samples.** A record-side change's position counts the host **input** samples the stream has processed: for the recorder (`Stream::Record`, reserved by spec 015) the samples fed since `start_recording`; for the one-pass (`Stream::OnePass`) the input samples processed since construction, `reset` or `reconfigure`. The playback-side controls keep spec 015's output clock on the same one-pass stream; a control's side says which clock its positions and entries use (the record-side controls act before the voice activation drops samples, so an output position maps to no input sample during a pause). The input positions are readable beside the output ones. | Input clock for the record side |
| D3 | **Where a value lands.** The host-rate values land on exactly input sample P: the limiter's derived coefficients are recomputed for sample P with the hold capacitor's state carried (a resistor changed while the circuit runs); the hiss gain moves from its old to its new level over a **20 ms** transition starting at P (the hiss stream itself is unchanged; a stepped noise level is audible). The device-rate values (sensitivity, threshold, hang, onset) take effect at the first device sample the record band-limit derives from input sample P or later (the decimator's rule, the mirror of spec 015's d(P)): the threshold moves a per-sample decision, so a step is click-free; the hang and onset counters take the new length at once, their running counts kept (a pause already under way completes against the new hang). A position at or before the stream's input position lands on the next input sample. | As stated |
| D4 | **No new plain setters.** A record-side value changes live only through `schedule_change` (a position at or below the current input position is the "set now"), or idle through `reconfigure` as today. `settings()` reflects a landed value (the live value lives in the settings), so `reconfigure(settings())` is a no-op. The control manifest's eight entries move from timing "idle" to "live" (a canonical re-bless); their kinds, and so the free tier line, are unchanged. | Schedule only; the manifest's timing moves |
| D5 | **The recorder's schedule.** `Stream::Record`'s schedule (64 changes, position order, spec 015's rules) is kept by `start_recording` (schedule, then start), applies during `record` calls, and is cleared by `stop_recording` after the flush (a change inside the flush's silence lands there; one past it is dropped without an entry), by `reset` and by `reconfigure`. The record calls gain a log-taking variant (`record_with_changes`); the stop's entries ride on its flush report. On the one-pass the existing log-taking call carries record-side entries too, their positions in the input clock. | As stated |
| D6 | **Nothing existing moves.** With no record-side change scheduled every output is bit-identical to 0.16: the eleven golden files and their SHA guards, the smoke hashes. The primary oracle: a change scheduled at input sample 0 of a fresh stream equals the constant configuration with the new value, bit for bit (every value, both streams, seven rates); a change at P leaves every sample before its landing equal to the old constant run; partition independence at block sizes 1, 64, 4096 and the irregular partition; record-then-play with changes equals the one-pass with the same changes under spec 008 FR-009's rule. | Asserted |
| D7 | **Cost budget 3 %** of the per-sample cost at 48 kHz against the 0.16 merge commit with the schedule empty, on the record path and on the one-pass (the codec-bypassed default front), a build comparison (45 alternating runs, solo) with a **5 % runner ceiling** through the `ab-timing` label; with one change per block reported. | 3 % / 5 % |
| D8 | **Not a device behavior.** The device sets its sensitivity in a menu (S-001) and whether a change takes effect during a recording is not known (A-021); the other seven values are the model's constants. Changing them while recording is a feature of the emulator's API: no assumption is registered, no stage's model version moves, no new setting; the stage manifest's canonical text moves in its version line only. | No assumption |
| D9 | **The hiss transition is the volume's shape at the gain**: a smoothstep over 20 ms of the hiss gain (linear amplitude), the same length and curve as spec 011's knob movement, an engine convention under A-045's family. | 20 ms smoothstep of the gain |

## Clarifications

### Session 2026-10-30 (owner sign-off)

- Q: Approve the spec with D1 – D9 as recommended? → A: _pending_.

## Overview

Since spec 015 the three playback-side controls land on an exact sample of their stream and every applied change is logged. The record side's values still change only idle: `reconfigure` rebuilds the paths, so a user who turns the sensitivity down while a recording runs today has to stop, reconfigure and start again, and a session that replays a recording cannot reproduce a change made mid-recording.

This feature makes eight record-side values **live** through the same schedule and log: a change carries a position in the stream's **input** clock (the samples the recorder or the one-pass has processed), lands on exactly that input sample (or the first device sample derived from it, for the voice activation's values), is logged with the position it landed on, and leaves everything else as it is. The limiter's capacitor keeps its charge across a knee or release change, as a circuit would; the voice activation's running counts are kept and compared against the new lengths; the hiss level glides over 20 ms rather than stepping.

The owner's 2026-10-29 decision stands: the codec, the memory, the host rate and the latency-bearing bypasses are never live; the stage switches stay idle. Changing a value while recording is an emulator convenience: the device's menu setting is not known to act mid-recording (A-021).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Adjust the recorder while it records (Priority: P1)

An app lets the user turn the sensitivity down (or any of the eight values) while a recording runs, by scheduling the change at the input sample the user's gesture corresponds to; the recording continues, the change lands on that sample at any buffer size, and the result equals what a recording made with the new value from the start would be from that sample on.

**Why this priority**: it is the feature; the brief ranks the sensitivity second of all controls for group-hunt users and investigators.

**Independent Test**: for each value, on the recorder and the one-pass, a change at input sample 0 of a fresh stream is compared with the constant configuration (bit-identical), a change at P with the old constant run before its landing, and the output at the four partitions with each other; the log against the schedule.

**Acceptance Scenarios**:

1. **Given** a fresh recorder (and separately a fresh one-pass) with a value scheduled at input position 0, **When** a stimulus is recorded, **Then** the stored samples (the one-pass output) are bit-identical to a stream configured with that value from the start, for each of the eight values at seven rates; the entry's position is 0 and its before/after values are the old and new settings.
2. **Given** a value scheduled at input position P inside a block, **When** the stream runs at block sizes 1, 64, 4096 and the irregular partition, **Then** the outputs and the concatenated logs are bit-identical, and every stored (output) sample derived from input samples before P equals the old constant run's.
3. **Given** the limiter's knee lowered at P while a tone 10 dB over the old knee plays, **When** the stream runs, **Then** from the landing the attenuation deepens toward the new knee's steady value within the model's release-shaped time, the hold capacitor's voltage is continuous across P (no reset), and the output equals a run with the new knee from the start once both have settled (within the spec 005 tolerances).
4. **Given** the sensitivity raised by two levels (the threshold 6 dB lower) at P during a quiet passage that the old threshold would have paused, **When** the stream runs, **Then** the gate's decision changes from the first device sample derived from input samples at or after P, the hang count already running is compared against the new length, no output sample moves before that device sample, and the splice events agree with a harness reference gate given the same change.
5. **Given** the hang shortened at P during a pause already running (the count past the new length), **When** the stream runs, **Then** the gate pauses at the first device sample after the landing, exactly as the reference gate does.
6. **Given** the hiss offset raised by 12 dB at P, **When** the stream runs with the input silent, **Then** the hiss level moves from the old to the new over exactly 20 ms of input samples starting at P along the smoothstep (the sample at P + N − 1 at the new gain), the hiss stream's samples are the same draws as without the change (the gain alone moves), and the level before P equals the old run's.
7. **Given** a record-side change scheduled at a position at or below the stream's input position, **When** the next call runs, **Then** it lands on the next input sample and the entry says so.

---

### User Story 2 - The session reproduces (Priority: P2)

A recording made with changes mid-way is exported; a session replays the same input with the same schedule and gets the same recording; the one-pass with the same changes equals the recording played back.

**Why this priority**: replay is the reason the changes are positioned and logged.

**Independent Test**: record with changes twice from the same input and schedule (bit-identical stored forms); the one-pass with the same changes against record-then-play under spec 008 FR-009's rule; the schedule survives `start_recording` and is cleared by the stop.

**Acceptance Scenarios**:

1. **Given** the same input, settings, seed, recording counter and schedule, **When** two engines record, **Then** the exported recordings are byte-identical and the logs equal.
2. **Given** a record-side schedule, **When** the one-pass runs it and a recorder records it and plays it back, **Then** the outputs are bit-identical under spec 008 FR-009's rule (the one-pass fed the record-latency silence; the output skipped by the codec's delay), with the codec bypassed and on.
3. **Given** changes scheduled before `start_recording`, **When** the recording starts and runs, **Then** they land at their input positions; **Given** changes past the fed input, **When** `stop_recording` runs, **Then** those inside the flush's silence land and are logged in the stop's report and those past it are dropped without an entry; the schedule is empty afterwards.
4. **Given** a landed change, **When** `settings()` is read, **Then** it shows the new value, and `reconfigure(settings())` changes no output from a fresh stream's.

---

### User Story 3 - Validation, the manifest and C (Priority: P3)

The app schedules record-side changes through the C API with the same refusals as the idle path, reads the input position, and the control manifest tells it which controls are live.

**Independent Test**: every refusal (a value outside the range, a control that is not live, the wrong stream) stores nothing; the control manifest's eight entries read "live" and the canonical text matches; the C smoke's scheduled recording equals the Rust example's rows.

**Acceptance Scenarios**:

1. **Given** a value outside its range (a sensitivity 0 or 6, a threshold above 0 dBFS, a hang below 50 ms, a hiss offset above +12 dB, a knee above +3 dBFS, a non-finite value), **When** it is scheduled, **Then** it is refused with the setting's existing error (`InvalidSetting`, naming the field) and nothing is stored.
2. **Given** a control that stays idle (a switch, the codec, the memory, the host rate, the seed, the tap, the voice activation's mode), **When** it is scheduled on any stream, **Then** it is refused with `ControlNotOnStream` and nothing is stored; a record-side control on the playback stream likewise.
3. **Given** the control manifest, **When** it is read, **Then** the eight entries' timing is "live" and every other entry's is unchanged; the canonical text is re-blessed once.
4. **Given** a C program, **When** it schedules record-side changes on the record stream and the one-pass, records with `rr_dr60_record_with_changes` and reads the input positions, **Then** it receives the Rust entries and outputs bit for bit.

### Edge Cases

- **Two clocks on the one-pass**: a route or volume change's position is an output index, a record-side change's an input index; both are reported where they landed in their own clock, and the entry's control tells the app which; `stream_position` reads the output clock and `input_position` the input clock.
- **The recorder's clock** starts at 0 at `start_recording`; the stop's flush extends it by the record latency's silence; `input_position(Stream::Record)` reads it (0 while idle).
- **The sensitivity and the threshold** both move the gate's level (A-022: 3 dB per level from the level-3 threshold); a change of either lands by D3; the manifest keeps both entries.
- **A hang or onset change while recording (not paused)** changes only the lengths; a running silent count above the new hang pauses at the next non-sound device sample.
- **The hiss offset with the capsule bypassed**: accepted, logged, no audible effect (as the idle setting).
- **A limiter change with the limiter bypassed**: accepted, logged, no effect until a `reconfigure` turns the stage on with the landed value.
- **A record-side change on the one-pass at a front tap** (before the stage): the limiter's values land (stage 3 runs at `AfterAgc`); the voice activation's values land on an inactive gate (no effect, logged); the input clock counts at every tap.
- **The one-pass input clock and `reset`**: both clocks restart; the schedule is cleared; the landed values stay (they live in the settings).
- **The hiss transition at the recording's end**: a transition the stop's flush cuts completes inside the flush's silence or is cut by the recording's end, as the volume's at a playback's end.
- **Changes at one input position**: applied in submission order; for the limiter the last coefficients win; for the hiss two transitions at one sample start one from the gain at rest; for the gate the last lengths win.
- **The level-3 anchor and the sensitivity at once**: both at one position: the threshold is the pair's result.
- **The record-side entries' `before`** is the stream's last applied value (each stream keeps its own, as spec 015), which after `reset` or `reconfigure` is the settings' value.
- **Determinism**: positions are integers, the landing rules exact, the hiss transition the smoothstep of spec 011 in the same arithmetic.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The engine MUST accept a scheduled change of each of the eight record-side controls (`vas.sensitivity`, `vas.threshold_dbfs`, `vas.hang_ms`, `vas.onset_ms`, `agc.knee_dbfs`, `agc.attack_ms`, `agc.release_ms`, `mic.hiss_offset_db`, by the control manifest's indices) on `Stream::Record` and `Stream::OnePass`, the position in the stream's **input** clock (D2), the value validated exactly as `validate` validates the setting (the setting's `InvalidSetting` error, nothing stored); any other setting, and a record-side control on the playback stream, MUST be refused with `ControlNotOnStream`.
- **FR-002**: The record stream's schedule MUST follow spec 015's rules (64 pending, position order, ties in submission order, `pending_changes`, `clear_schedule`): kept by `start_recording`, applied during `record` and the stop's flush, cleared by `stop_recording` (after the flush), `reset` and `reconfigure`. The input positions MUST be readable: `input_position(Stream::Record)` (0 while idle) and `input_position(Stream::OnePass)`.
- **FR-003**: A host-rate value (the limiter's three, the hiss offset) MUST land on input sample P exactly: the limiter's coefficients derived from the new value apply to sample P with the hold capacitor's voltage unchanged; the hiss gain moves from its current value to the new one over N = 0.02 × host rate input samples from P along spec 011's smoothstep (sample P + N − 1 exactly at the new gain; a retarget restarts from the gain at rest), the noise draws unchanged. A device-rate value (the gate's four) MUST take effect at the first device sample derived from input sample P or later (the decimator's due rule), with the gate's running counts kept and compared against the new lengths. A position at or below the stream's input position lands on the next input sample.
- **FR-004**: `settings()` MUST reflect a landed value; `reconfigure(settings())` MUST then change no output from a fresh stream's. No new setter is added; the idle path is unchanged.
- **FR-005**: Every landed record-side change MUST be logged as a change entry (spec 015's form, the position in the input clock, before = the stream's last applied value, after, tag) by the call that processed the input sample it landed on: `record_with_changes(input, events, changes)` and the existing `process_with_events_and_changes` (whose `changes` buffer now carries both sides); the stop's flush entries in the stop's report (`StopOutcome` gains the entries' count and the log-taking variant `stop_recording_with_changes`). `MAX_CHANGES_PER_CALL` stays always enough (the plain setters are playback-side).
- **FR-006**: With no record-side change scheduled the output MUST be bit-identical to 0.16: the eleven audio golden files and their SHA guards, every smoke hash. A change at input position 0 of a fresh stream MUST equal the constant configuration with that value bit for bit (every value, both streams, seven rates); a change at P MUST leave every output sample derived from input before its landing equal to the old constant run's; outputs and logs MUST be partition-independent (1, 64, 4096, the irregular partition); the one-pass with a schedule MUST equal record-then-play with the same schedule under spec 008 FR-009's rule, the codec bypassed and on.
- **FR-007**: The gate's decisions after a change MUST equal a harness reference gate given the same device samples and the same change (spec 003's data model with the new lengths applied at the landing sample); the limiter's attenuation after a change MUST settle to the new constant run's within spec 005's tolerances; the hiss transition MUST read the smoothstep within spec 011's tolerance on the gain.
- **FR-008**: Deterministic and real-time safe as spec 015: no allocation, lock or I/O in any call; integer positions; the same inputs, settings, seed, counter and schedule give bit-identical outputs and logs on every platform. A new golden file (`golden-record-changes-v1.json`) MUST hold, per entry, the output digest and the full log over the eight values, both streams, the flush case and the seven rates.
- **FR-009**: The per-sample cost at 48 kHz with the schedule empty MUST be within D7's budget against the 0.16 merge commit on the record path and the one-pass (the codec-bypassed default front), as a build comparison on the development machine and the CI runner (`ab-timing` label, the runner ceiling); with one change per block reported.
- **FR-010**: Rust and C: `input_position`, `record_with_changes`, `stop_recording_with_changes` (the C forms with the count as an out-parameter; the existing report structures unchanged); the control manifest's eight entries at timing "live" (one canonical re-bless; the kinds unchanged); the header regenerated.
- **FR-011**: Documentation: the user guide's section gains the record side (the input clock, the eight values and their landing rules, the hiss transition, the flush rule, the two clocks of the one-pass, "not a device behavior": the sensitivity is a menu setting whose mid-recording behavior is unknown, A-021); the real-time list; the CHANGELOG as version 0.17.0; the control manifest's text re-blessed; controls-discussion.md § 2b's status; the stage manifest's version line only.
- **FR-012**: Not a device behavior: no assumption registered, no stage's model version moves, no new setting; the control manifest's kinds (and so the free / Workbench line) unchanged.

### Out of Scope

- Live switches and bypasses (latency-bearing or enabling); the speech codec, the memory, the host rate, the seed, the tap, the voice activation's mode; a between-recordings real-time-safe apply (`reconfigure` stays the idle path); the battery state (spec 018); the playback-stage bypass (021); the session snapshot and sidecar (020); the timeline map of input to output positions (020); plain setters for the eight values (D4); any claim about the device's behavior when its sensitivity changes mid-recording.

### Key Entities

- **Record stream**: `Stream::Record`, the recorder; clock = input samples fed since `start_recording`; a schedule; last applied values per record-side control.
- **Input clock** of the one-pass: input samples processed since construction, `reset`, `reconfigure`; beside spec 015's output clock.
- **Record-side change**: a scheduled change of one of the eight controls; lands by FR-003; logged as a change entry.
- **Hiss transition**: N = 0.02 × rate input samples of the smoothstep on the hiss gain.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: For each of the eight values on both streams at seven rates: a change at input 0 is bit-identical to the constant configuration; a change at P leaves the pre-landing output identical to the old run; outputs and logs identical at the four partitions: 0 mismatches.
- **SC-002**: The gate after a change equals the reference gate on every device sample (0 mismatches; the events equal); the limiter settles to the new constant run within spec 005's tolerances; the hiss transition's gain within spec 011's tolerance of the smoothstep and exactly the new gain at P + N − 1.
- **SC-003**: The eleven golden files and their SHA guards are byte-identical; the new golden is reproduced on every golden target; the C smoke's scheduled recording equals the Rust example's rows; record-then-play with changes equals the one-pass (codec off and on).
- **SC-004**: The log holds one entry per landed change with the input position, the chained values and the tag; the stop's flush entries are reported; a change past the flush is dropped without an entry.
- **SC-005**: The idle cost at 48 kHz is within 3 % of 0.16 on the record path and the one-pass (5 % on the runner); one change per block reported.
- **SC-006**: Coverage stays at or above 80 % and within 1 point of the baseline (0.16: 97.49 % locally).

## Assumptions

- **D1 alternative:** every record-side setting live, the switches included (rejected: a bypass changes the latency or enables a stage from rest, which the owner excluded on 2026-10-29); only the sensitivity (rejected: the limiter's and gate's internals are the Workbench's reason to exist, and the one-pass monitoring toy needs them).
- **D2 alternatives:** the output clock for the record side too (rejected: the record stages act before the gate drops samples, so a change during a pause would have no position); the recording's stored-sample clock (the VAS events') for the recorder (rejected: the limiter and the hiss act at the host rate before the decimation, and the app's gesture is in input time); one clock per stream with the record side mapped (rejected: the map is spec 020's).
- **D3 alternatives:** resetting the limiter's capacitor at a knee change (rejected: unphysical and a click); resetting the gate's counts at a hang change (rejected: a pause under way would restart its hang); stepping the hiss gain (rejected: a stepped noise level is audible, the brief's own note).
- **D4 alternative:** eight plain setters (rejected: spec 015's schedule at the current position is the same call, the setter scan of spec 014 would grow, and the idle path already exists).
- **D5 alternative:** clearing the record schedule at `start_recording` (rejected: schedule-then-start, as the player).
- **D7:** the protocol of specs 011 – 016.
- **D8 alternative:** registering an assumption about the device's sensitivity mid-recording (rejected: we assert nothing about the device; A-021 already records the unknown).
- **D9 alternative:** a linear-in-dB glide (rejected: spec 011's smoothstep on the linear gain is the engine's one transition shape; a dB glide needs an exponential per sample).
- **Spec-Kit process for this feature**, per the owner's 2026-10-29 cadence: owner sign-offs at this spec, the plan and the merge; reviews and phases otherwise proceed and stop only on a failed check or a new decision.
- No release is published; the CHANGELOG records the feature as 0.17.0, a minor version with new functions and the manifest's timing words.
