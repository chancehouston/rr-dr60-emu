# Feature Specification: Sample-Timed Control Changes and the Change Log

**Feature Branch**: `015-timed-changes`

**Created**: 2026-10-30

**Status**: Draft; awaiting the owner's sign-off on D1 – D8

**Input**: User description: "015 sample-timed control changes and the change log. As an app replaying a session, driving a preset or scripting a fade, I want to schedule a change of a live control (the route, the volume position, the playback speed level) at an exact sample position of the output stream, and to receive a log of every change the engine applied with the position it landed on, so that the same session reproduces bit for bit at any audio buffer size and a listener can see what changed when. Product request: controls-discussion.md § 2c gaps 1 and 2 and § 4 item 2; decisions.md 2026-10-29 (engine order 014 → 016 → 015 → 017 …; no AI control for now, so the feature stands on presets, sessions and replay); front-and-inside.md engine requests (the timeline map is noted). Today a live change is read once per call at the block boundary (spec 009 FR-009 for the route and the volume, spec 011's 20 ms transition from the next block, spec 013 FR-001 for the speed from the first device sample computed after it was set), so the same change gives different output under different buffer sizes; iOS changes the I/O buffer under the app. Scope: the three live controls on the playback path and the one-pass (the record-side values follow in spec 017 through the same mechanism, so the form must carry any control of the control manifest, spec 014). A change carries a position in the stream's own clock: the host output samples the player has produced since start_playback, or the one-pass since reset; the engine applies it at exactly that sample (the volume transition of spec 011 starts there, the route switches there with the new branch from rest as 011 FR-005 says, the speed level applies from the first device sample computed for host output at or after it under spec 013's latch and whole-place rules). A position already passed applies at the next sample and is logged where it landed. The existing setters keep their meaning (at the next call), so every existing golden file and smoke hash stays byte-identical and a change at a boundary of every partition equals today's behaviour. Pending changes live in a bounded schedule reserved at construction (no allocation in the processing path; a full schedule refuses the change with a distinct error; reset and reconfigure and start_playback clear it, stated). The change log: every applied change is reported to a caller buffer in the form the VAS events use (position, control, the value before, the value after), per call, with a length function that is always enough; no callbacks. Oracles: partition independence (bit-identical output at block sizes 1, 64, 4096 and an irregular partition for arbitrary change positions); spec 011's splice identity at the tap after stage 11 and spec 013's schedules against the harness reference for positioned changes; the log against the schedule. Determinism and real-time safety as every spec; a cost budget as a build A/B against 0.15 (idle, no pending change: 3 % on the development machine, 45 alternating runs, a runner ceiling via the ab-timing label; with changes pending, reported). Not a device behaviour: an engine convention (no new assumption unless the plan finds one; the control manifest's timing classes stay). Free tier. Out of scope: a programmed volume fade over N ms (a later option; the schedule gives a script the same), record-side values (017), the settings serialization and the session sidecar (020), the timeline map of meters to positions (020, on the stream clock 015 defines), AI control, callbacks, threading (the Swift package's queue carries the change form)."

## Decisions for sign-off

The owner reads this section first. Each decision gives the recommendation; the alternative is in the Assumptions section.

| # | Decision | Recommended |
|---|---|---|
| D1 | **Scope: the three live controls on two streams.** The route, the volume position and the playback speed level can be scheduled on the playback stream (the player) and on the one-pass stream. The record-side values (sensitivity, threshold, hang, the limiter's settings, the hiss offset) are spec 017's; the form of a change carries any control the control manifest numbers, so 017 adds controls, not a mechanism. | Three controls, two streams |
| D2 | **The clock is the stream's output samples.** A position is a count of host-rate output samples of the stream the change is scheduled on: for the playback stream, the samples the current playback has produced since it started (the recording's part, the speed control's tail and the drain, one count); for the one-pass stream, the samples produced since the engine was constructed, reset or reconfigured, the clock the voice-activation events already use. Each stream keeps its own schedule. | Output samples, one schedule per stream |
| D3 | **Where a change lands.** A change scheduled at position P is applied so that output sample P is the first sample it affects: the volume transition of spec 011 starts with sample P as its first transition sample; the route switches at sample P with the new branch from rest (spec 011 FR-005); the speed level applies from the first device-rate sample the engine computes to produce output sample P or any later sample, under spec 013's rules (a switch into level 3 waits for a whole place; the level is latched at the recording's end). A position at or before the stream's current position lands on the next sample produced and is logged where it landed. A change made through the existing setters (no position) lands on the first sample of the next call, exactly today's behaviour, and is logged there too. | As stated |
| D4 | **A bounded schedule.** Each stream's schedule holds up to **64** pending changes, reserved at construction; a 65th is refused with a distinct error and nothing is stored; a refused value (an invalid volume position or speed level) is refused with its existing error before it is stored. Changes are applied in the order they were submitted, two at the same position in submission order (so a later one wins). The one-pass schedule is cleared by `reset` and `reconfigure`; the playback schedule by `reset`, `reconfigure` and the end of a playback (finished or stopped), **not** by `start_playback`, so an app schedules a session's changes and then starts. The pending count is readable and a schedule can be cleared. | 64 per stream; order of submission; cleared as stated |
| D5 | **The change log.** Every applied change, scheduled or set without a position, is reported to a caller-provided buffer once, by the processing call that applied it, in the form the voice-activation events use: the position it landed on, the control (the control manifest's identifier), the value before and the value after. A length that is always enough is published (the schedule's capacity plus one per control for the plain setters). The processing calls without a buffer still apply the changes and discard the log. No callback. | As stated |
| D6 | **Nothing existing moves.** With no scheduled change, every output is bit-identical to 0.15: the ten audio golden files and their SHA guards, the smoke hashes, and a change at a position that is a boundary of every partition equals the plain setter's result. A positioned change is partition-independent: bit-identical output at block sizes 1, 64 and 4096 and an irregular partition. | Identity and partition independence asserted |
| D7 | **Cost budget 3 %** of the per-sample cost at 48 kHz (the codec-bypassed speaker route, specs 011 – 013's protocol) against 0.15 with the schedule empty, a build comparison (fastest of 45 alternating runs, solo) with a **5 % runner ceiling** through the `ab-timing` label; with changes pending, the cost is reported, not budgeted. | 3 % / 5 % |
| D8 | **An engine convention, not a device behaviour.** The device's knobs move whenever a hand moves them; positioning a change on a sample is a feature of the emulator's API. No assumption is registered, no stage's model version moves, the control manifest's timing classes stay "live" and its canonical text is unchanged; the stage manifest's API-item strings of the three controls' stages (9 and 11) name the schedule call (one canonical re-bless). | No assumption; API items re-blessed |

## Clarifications

### Session 2026-10-30 (owner sign-off)

- Q: Approve the spec with D1 – D8 as recommended? → A: _pending_.

## Overview

Since spec 009 the route and the volume position, and since spec 013 the playback speed level, are **live controls**: the host sets them at any time and they take effect at the next processing call (spec 009 FR-009; spec 011's 20 ms transition starts at the next block; spec 013 FR-001's level applies from the first device sample computed after it was set). Where a change lands therefore depends on the host's block size: the same `set_volume` made between two calls lands 64 samples apart at a 64-sample block size and 4096 apart at 4096. iOS changes the audio buffer size under an app (on a route change, for example), so a session replayed later, a preset applied by a script, or a recorded "what was changed when" cannot reproduce the output bit for bit, and the engine reports no control change at all, only the voice-activation splices.

This feature adds **sample-timed changes** and a **change log**. A change of a live control can carry a position in the stream's own clock, and the engine applies it so that exactly that output sample is the first one it affects, whatever the block size. Every applied change, positioned or not, is reported back to the caller with the position it landed on, in the form the voice-activation events already use. Together they make a session a list of events that reproduces bit for bit, and a timeline the app can show.

The plain setters keep their meaning and every existing output is unchanged. The record-side values become live through the same form in spec 017; the settings serialization and the session sidecar are spec 020's.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - The same session at any buffer size (Priority: P1)

An app replays a session: it starts a playback, schedules the route, volume and speed changes the session recorded at their positions, and plays it in blocks of whatever size the device gives it. The output is bit-identical to the output of the original session at a different block size.

**Why this priority**: it is the whole value of the feature: replay, presets and scripts are useless unless they reproduce.

**Independent Test**: a stimulus is played (or run through the one-pass) with a set of positioned changes at block sizes 1, 64 and 4096 and an irregular partition; the outputs are compared sample for sample; each change is also compared with the harness's existing oracles for that control at the change's position.

**Acceptance Scenarios**:

1. **Given** a playback (and separately the one-pass) with changes scheduled at arbitrary positions, including positions inside blocks, **When** the same stimulus is processed at block sizes 1, 64 and 4096 host samples and an irregular partition, **Then** the outputs are bit-identical at all seven host rates, on both routes.
2. **Given** a volume change scheduled at position P at the tap after stage 11, **When** the stream is processed, **Then** output samples before P equal a constant run at the old position, sample P is the first transition sample and sample P + N − 1 carries exactly the new position's gain (spec 011's splice identity, with P in place of the block boundary), at all seven host rates and on both routes.
3. **Given** a route change scheduled at position P, **When** the stream is processed, **Then** samples before P equal the old route's run and samples from P on equal a run on the new route started from rest at P (spec 011 FR-005), with a volume transition in progress at P ended as that rule says.
4. **Given** a playback with speed level changes scheduled at positions, with the speech codec bypassed and on, **When** it plays to the end, **Then** the output equals the harness's reference playback (spec 013's reference bank at the places the rule gives) bit for bit, the level of each stretch starting at the first device sample computed for output sample P or later, a switch into level 3 waiting for a whole place, the recording's length rule unchanged, and `finished` reported once at the end of the drain.
5. **Given** a change scheduled at a position already passed, or at the current position, **When** the next call runs, **Then** the change lands on the first sample that call produces and the log reports that position, not the requested one.
6. **Given** a change scheduled at a position that is a boundary of every partition (a multiple of 4096), **When** the stream is processed, **Then** the output equals the output of the plain setter called between the two calls at that boundary, bit for bit, at every partition.
7. **Given** two changes of the same control at the same position, **When** the stream is processed, **Then** both are applied in submission order and logged, and the output equals the second change's alone, except for the volume, where the second retargets the first's transition at the same sample (spec 011 FR-003's retarget rule).

---

### User Story 2 - A log of what changed when (Priority: P2)

An app shows a timeline of a session: every change the engine applied, with the output sample it landed on, the control, and the values before and after, read from the engine, not reconstructed from what the app sent.

**Why this priority**: the timeline and "the assistant changed this" are the product's reasons for the log; the engine alone knows where a change landed.

**Independent Test**: a stimulus is processed with scheduled changes and plain setter calls; the log of every call is collected and compared with the schedule and the setter calls: one entry per applied change, in order, with the landed position, the control's identifier and the values.

**Acceptance Scenarios**:

1. **Given** a schedule of changes, **When** the stream is processed in blocks of any size, **Then** each change appears in the log exactly once, in the call that produced the sample it landed on, with that position, the control's manifest identifier, the value before (the control's setting before the change) and the value after; the concatenated logs over the partition are identical at every block size.
2. **Given** a plain setter call between two processing calls, **When** the next call runs, **Then** the log holds one entry at the first sample that call produced, with the old and new values; several setter calls of one control between two processing calls log only the last (spec 011: only the last counts).
3. **Given** a log buffer shorter than the changes applied in a call, **When** the call runs, **Then** every change is still applied, the first entries fill the buffer, the report says how many there were, and the published length function returns a length that is always enough.
4. **Given** a processing call without a log buffer, **When** changes land in it, **Then** they are applied and nothing is reported (the existing calls are unchanged in form).
5. **Given** a change of the volume to the same position, **When** it is applied, **Then** it is logged with equal before and after values (it retargets a transition in progress, so it is a change).

---

### User Story 3 - The schedule is bounded and the same from C (Priority: P3)

The iOS app schedules changes and reads the log through the C API, and can rely on a fixed schedule capacity, a distinct error when it is full, a readable pending count and a clear call.

**Why this priority**: the first app reaches the engine through C; a bounded schedule is what keeps the processing path real-time safe.

**Independent Test**: the C smoke program schedules the Rust test's changes, processes its stimulus and prints the log; the print equals the Rust test's; a 65th change is refused; the pending count and the clear are checked.

**Acceptance Scenarios**:

1. **Given** a C program, **When** it schedules changes, processes blocks with a log buffer and reads the log, **Then** it receives the Rust entries field for field and the same output hash.
2. **Given** 64 pending changes on one stream, **When** a 65th is scheduled, **Then** it is refused with the schedule-full status and nothing changes; the other stream's schedule is unaffected.
3. **Given** a schedule with pending changes, **When** the pending count is read, the schedule is cleared, or `reset`, `reconfigure` or the end of a playback occurs, **Then** the count reads what D4 states and no cleared change is ever applied or logged.
4. **Given** an invalid value (a volume outside 0 … 1, a speed level outside 1 … 5), **When** it is scheduled, **Then** it is refused with the existing error for that control and nothing is stored.

### Edge Cases

- **Positions on the playback stream** count every sample the playback produces: the recording's part at the levels played, the speed control's tail and the drain. A change scheduled during the drain lands there (a route or volume change is audible in the settle; a speed change after the latch has no effect on this playback and is logged as applied).
- **A change past the end of a playback** is never reached; it stays pending until the playback ends, which clears it, and is never logged. A change past the current one-pass position stays pending until the stream reaches it.
- **Scheduling before `start_playback`**: allowed; the positions count from the playback's first sample. Changes left over from a previous playback do not exist (cleared at its end).
- **A change on the one-pass stream while a path is active**: accepted into the one-pass schedule (the stream is idle; it lands when the one-pass runs again). A speed change on the one-pass stream sets the level (it has no audible effect there; the player reads it, as today).
- **The route change ends a volume transition** in progress at that sample (spec 011 FR-005); the transition's own change was already logged when it started.
- **The speed change and the latch**: a change scheduled before the latch but whose first affected device sample falls after the place reaches K has no effect (spec 013 FR-007); it is logged where it landed with before and after values.
- **The speed change and the whole-place rule**: a switch into level 3 scheduled at P starts waiting at the first device sample computed for output P; the stored samples pass unaltered from the next whole place (at most 4 device samples later), as spec 013 FR-007 says of a plain change.
- **Volume positions and the knob**: the value before is the last set position (the control's reading), not the knob's position during a transition (spec 011 FR-011).
- **The zero-length call** produces no sample, so nothing lands and nothing is logged.
- **Several changes of different controls at the same position** are applied in submission order; the output does not depend on the order except where the rules of D3 make it (a route and a volume at the same sample: the new route starts at the new position, spec 011).
- **The schedule and the voice activation**: the one-pass clock is the output's, so a change scheduled at position P lands on output sample P whatever the gate dropped before it (the same clock as the splice events).
- **Determinism**: the schedule's state is part of the engine's; two engines fed the same calls, schedules and stimuli give the same output and log on every platform.
- **The C size rule**: the log entry is a plain structure; the schedule call takes the control's identifier and a value; a null handle or buffer pointer with a non-zero length is refused.

## Requirements *(mandatory)*

### Functional Requirements

**The schedule**

- **FR-001**: The engine MUST accept a **scheduled change** of a live control (the route, the volume position, the playback speed level) on either of its two streams, carrying the stream, the control, the value and a position in that stream's clock (D2): host output samples the stream has produced, the playback stream's count starting at 0 at `start_playback` and the one-pass stream's at 0 at construction, `reset` and `reconfigure` (the clock the voice-activation events use). The value is validated as the plain setter validates it, before anything is stored.
- **FR-002**: Each stream's schedule MUST hold up to **64** pending changes, reserved at construction and never allocated in the processing path; a change beyond the capacity MUST be refused with a distinct error (Rust and C) and nothing stored. The pending count of each stream MUST be readable; each schedule MUST be clearable by the host. The one-pass schedule is cleared by `reset` and `reconfigure`; the playback schedule by `reset`, `reconfigure` and the end of a playback (`finished` or `stop_playback`), and is kept by `start_playback`.
- **FR-003**: A scheduled change MUST be applied so that output sample P is the first sample it affects (D3): the volume's transition sample 1 is sample P (spec 011 FR-001, FR-002, with P in place of "the first sample of the next block"); the route is switched for sample P with the new branch from rest and a transition in progress ended (spec 011 FR-005); the speed level applies from the first device-rate sample computed to produce output sample P or later, under spec 013 FR-007's whole-place rule and the latch. A change whose position is at or before the stream's current position MUST land on the next sample produced. Changes MUST be applied in submission order, those at one position in that order.
- **FR-004**: The plain setters (`set_route`, `set_volume`, `set_playback_speed`) MUST keep their meaning: the change lands on the first sample of the next processing call of whichever stream runs next, and the last call of one setter between two processing calls is the one that lands. The transport calls (`start_playback`, `stop_playback`, recording) MUST be unchanged in their timing of the controls (spec 009 FR-009: `start_playback` takes the position and the route at once).

**The log**

- **FR-005**: Every applied change, scheduled or plain, MUST be reported exactly once, by the processing call that produced the sample it landed on, to a caller-provided buffer, as a **change entry**: the landed position (the stream's clock), the control's identifier (the control manifest's), the value before (the control's setting before the change) and the value after. The call MUST report how many changes it applied even when the buffer is shorter (the first entries written, as the voice-activation events do). A published length MUST be always enough: the schedule's capacity plus one per live control. Processing calls without a buffer MUST apply the changes and discard the entries.
- **FR-006**: A change to the same value MUST be applied and logged (for the volume it retargets a transition; for the route and the speed it is a no-op the log still shows); a change that can have no effect (a speed level after the latch) is applied and logged where it landed.

**Nothing existing moves**

- **FR-007**: With no scheduled change the output MUST be bit-identical to version 0.15: the ten audio golden files and their SHA guards byte-identical, every smoke hash unchanged; a change scheduled at a position that is a boundary of every partition MUST give the plain setter's output bit for bit.
- **FR-008**: A positioned change MUST be partition-independent: bit-identical output and identical concatenated logs at block sizes 1, 64 and 4096 and an irregular partition, at all seven host rates, on both routes, for every control, with the speech codec bypassed and on for the speed.
- **FR-009**: The schedule and the log MUST be deterministic (the same calls, schedules and stimuli give bit-identical output and entries on every supported platform) and real-time safe: no allocation, lock or I/O in any processing, scheduling, reading or clearing call; the per-sample work bounded (a change lookup per block or per sample that is a comparison against the next pending position). A new golden file (`golden-changes-v1.json`) MUST hold, per entry, the output digest and the full log for a set of schedules, configurations and the seven host rates, including entries for the plain setters, the three controls, the same-position cases, the passed-position case and the drain.
- **FR-010**: The per-sample cost at 48 kHz with the schedule empty MUST be within D7's budget against the 0.15 merge commit, measured as a build comparison (`scripts/ab-timing.sh`, 45 alternating rounds, solo) on the development machine and on the CI runner through the `ab-timing` label, with the runner ceiling; the cost with a pending change per block MUST be reported.

**Access and records**

- **FR-011**: The schedule call, the pending count, the clear, the change entry and the log-taking processing calls MUST exist in Rust and in C; the C entry is a plain structure, the control's identifier the manifest's index, the values as the control's own type carried in one numeric field (the plan fixes the form); the existing calls, reports and the settings structure are unchanged (120 bytes); a distinct status for the full schedule; the header regenerated. The log-taking calls are new variants beside the existing ones (no compile break).
- **FR-012**: **Documentation.** The user guide MUST explain the two stream clocks, how to schedule a change and read the log in Rust and C, the landing rules per control (D3), the capacity and the clearing rules (D4), and that positioning is an emulator convention (D8); the real-time list MUST include the new calls; the CHANGELOG MUST record the feature as version 0.16.0; the stage manifest's "Rust API items" of stages 9 and 11 MUST name the schedule call (one canonical re-bless); controls-discussion.md § 2c MUST say gaps 1 and 2 are built on the playback side and the one-pass; front-and-inside.md's engine requests MUST note the stream clock the timeline map (spec 020) will use.
- **FR-013**: The feature is **not a device behaviour**: no assumption is registered, no stage's model version moves, the control manifest's canonical text is unchanged (the three controls stay "live"), and no setting is added.

### Out of Scope

- A programmed volume fade over a duration (a later emulator option; a script schedules positions instead); the record-side values (spec 017, through this form); a settings serialization and the session sidecar (spec 020); the timeline map of the meters to positions (spec 020, on the clocks defined here); AI control; callbacks or a push of entries; threading (the Swift package's lock-free queue carries the change form into the audio callback); scheduling in time units (the app converts with the host rate); a schedule capacity setting; changes during recording of the record-side values.

### Key Entities

- **Stream**: the playback stream (one per playback, clock from `start_playback`) or the one-pass stream (clock from construction, `reset`, `reconfigure`); each has a schedule and a clock in host output samples.
- **Scheduled change**: stream, position, control, value; pending until applied or cleared.
- **Change entry**: position landed, control identifier, value before, value after; one per applied change, reported by the call that applied it.
- **Schedule**: a per-stream bounded list of pending changes, capacity 64, submission order.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: For every control on both streams, the output and the concatenated log are bit-identical across block sizes 1, 64, 4096 and an irregular partition at all seven host rates and on both routes: 0 mismatches.
- **SC-002**: A positioned volume change equals spec 011's splice oracle at the tap after stage 11 (sample P the first transition sample, sample P + N − 1 exactly the new gain); a positioned route change equals the new route's from-rest run from P; a positioned speed schedule equals the harness reference bit for bit with the codec bypassed and on: 0 mismatches at seven rates.
- **SC-003**: The ten audio golden files and their SHA guards are byte-identical; every change at an all-partition boundary equals the plain setter's output; the new golden is reproduced on every golden target and one entry by the C smoke program.
- **SC-004**: The log holds exactly one entry per applied change with the landed position and values on every scenario of US2; the published length is never exceeded over the golden set.
- **SC-005**: The per-sample cost at 48 kHz with an empty schedule is within 3 % of 0.15 on the development machine (5 % on the CI runner); with one pending change per block the figure is reported.
- **SC-006**: Coverage stays at or above 80 % and within 1 point of the baseline (0.15: 97.69 % locally).

## Assumptions

- **D1 alternative:** every control in `Settings` schedulable now. Rejected: the record-side values need their own landing rules (a sensitivity step moves a decision per device sample; a hiss offset needs a ramp) and the owner's order puts them in 017.
- **D2 alternatives:** positions in input samples (the one-pass output is shorter than the input in drop mode, so an input position maps to no output sample during a pause; the events use output positions); positions in time (the host rate converts); one schedule for both streams (ambiguous when idle, and a session's playback changes could not be scheduled before `start_playback`). The playback clock counts the drain so that a change can be positioned anywhere the stream produces samples.
- **D3 alternatives:** applying a passed position never (refuse it) or at the requested position by rewinding (impossible). The "next sample" rule matches what a late UI event does today and the log records the truth.
- **D4 alternatives:** an unbounded schedule (allocation in the processing path or at the schedule call, breaking the real-time rule); a capacity setting (a C structure change without an app request); clearing at `start_playback` (prevents schedule-then-start). 64 holds a session's changes for a block of any size: a UI sends at most one change per control per frame, and the app re-schedules per block if it keeps more.
- **D5 alternatives:** a callback (the engine must not call the host); a pull after the call like the meters (a call applying several changes would need a list anyway; the events' form already exists in Rust and C); not logging plain setter changes (the log would then miss the UI's changes and the timeline would be wrong).
- **D6, D7:** the protocol of specs 011 – 013 (the idle A/B against the previous merge commit, the 45-run estimator, the runner ceiling, the `ab-timing` label).
- **D8 alternative:** registering the convention as an assumption. Rejected: the registers describe the device; a scheduling API describes the emulator (spec 010's clamp set the precedent).
- The retarget rule of spec 011 (a retarget restarts from the knob at rest) stays; a scripted fade is a sequence of scheduled positions, each starting a 20 ms transition, and the "programmed fade" option is a later feature.
- **Spec-Kit process for this feature**, per the owner's 2026-10-29 cadence: owner sign-offs at this spec, the plan and the merge; reviews and phases otherwise proceed and stop only on a failed check or a new decision.
- No release is published; the CHANGELOG records the feature as 0.16.0, a minor version with new types and functions only.
