# Feature Specification: Pitch-Kept Playback Speed (an option)

**Feature Branch**: `023-pitch-kept-speed`

**Created**: 2026-10-31

**Status**: Draft; awaiting the three draft reviews and the owner's sign-off (D1 – D10)

**Input**: User description: "023 pitch-kept playback speed as an option (mechanism c1; the owner's road to 1.0, decisions.md 2026-10-31). As an app user reviewing a recording, I want to slow a playback down (or speed it up) at the device's five levels without the voice dropping or rising in pitch, selectable as an option beside the modeled default, so that a slowed passage stays intelligible; as an app developer I want it as one more live control with the same live-change, scheduling, logging, length and progress rules the speed level already has."

## Decisions for sign-off

The owner reads this section first. Each decision gives the recommendation; the alternative is in the Assumptions section. The three draft reviews (coach, hardware, product) are applied at their defaults before the sign-off and listed under Clarifications.

| # | Decision | Recommended |
|---|---|---|
| D1 | **One new live control, `playback_speed_mode`**, a choice with two options: `pitch_with_speed` (the default: the existing mechanism, the pitch moving with the speed) and `pitch_kept` (this feature). Kind **model internal** (the device's mechanism is A-054's open question; the app's rule of controls-discussion row 7 places it in the Workbench until a capture shows the device keeps pitch, when it becomes the device default by a CHANGELOG-recorded assumption change). Appended to the control manifest (index 30; no existing index moves). Live on the playback stream only, like the level: a plain setter and getter kept by `reset` and `reconfigure`, and a scheduled change in the output clock (spec 015); every change, plain or scheduled, is a change entry. | **A choice control, model internal, index 30** (alternatives: a boolean switch; a device control; a setting fixed at construction) |
| D2 | **The pitch-kept mechanism is time-domain time-scaling of the decoded stream at the 6 kHz device rate** (waveform-similarity overlap-add: a fixed analysis frame of about 20 ms, a search window of about ±5 ms, a raised-cosine overlap; designed and pinned by a script like spec 013's), at the same place in the chain as the existing mechanism (between the speech decoder and stage 10, before the reconstruction filter), the same five ratios 3/5 … 7/5; the fast levels compress the same way. **An engine convention, not a device behavior**: A-054 is amended with a note (the device's method, if it keeps pitch, is undocumented; S-018's decoder-domain patents are the alternative class, recorded), no new assumption. | **WSOLA on the decoded stream** (alternatives: a decoder-domain method of S-018's class; a spectral or formant method; both out of scope) |
| D3 | **Level 3 is the identity in both modes** (bit-identical to 0.22, every play report included), and **with the default mode every output is bit-identical to 0.22**: the sixteen golden guards and every smoke hash unmoved. | **Identity at level 3 and in the default mode** |
| D4 | **The place, length and progress rules are the level's.** At ratio r the recording's own part plays in ⌈K ÷ r⌉ device samples within the time-scaler's frame granularity (the exact bound stated by the design and pinned); `PlayInfo::place` advances r per output sample and reaches K at the end; `playback_length_samples(number, level)` holds for both modes with at most one frame's tail added in pitch-kept mode (stated); then the unchanged drain. | **The level's rules, one frame's tolerance** |
| D5 | **Changes.** A level change in pitch-kept mode takes effect at the time-scaler's next analysis frame (at most about 20 ms later), with no crossfade: the overlap-add output is continuous by construction (the click energy against the held level is asserted at least 30 dB below spec 019's instant switch, by spec 019's reader). A **mode change during playback is a 20 ms crossfade** (spec 019's blend from the common place, the old mechanism fading out at its own output). A change on a playback's first output switches at once (spec 011's rule). A request during a blend waits for its end (spec 019's rule). | **Frame-granular level changes, a blended mode change** |
| D6 | **Measurable properties**, at every host rate: on a sustained 1 kHz tone and on the harness's voiced stimulus, at every level, the fundamental's frequency in pitch-kept mode is within ±1 % (17 cents) of the stored one (and moves by the ratio in the default mode); the output level within ±1 dB of the stored level; spurious products at least 30 dB below the tone in 20 – 3000 Hz at 6 kHz (a transient-splice figure, reported per level); the length within the stated frame granularity; deterministic (the same calls give the same output at every rate); real-time safe (no allocation after construction: the frame buffers reserved at construction). | **As stated** |
| D7 | **Cost.** Pitch-kept mode at most **3 ×** the existing mechanism's per-sample cost at 48 kHz on the player (the cross-correlation search; reported per level, the worst 64-sample block within the real-time budget); the default mode within the 3 % A/B budget against 0.22 (merge commit 12a3c45). | **3 × budget, default within 3 %** |
| D8 | **One new golden** (`golden-pitch-kept-v1.json`: both modes × the five levels × seven rates on tones and the voiced stimulus, with mode and level changes), the control manifest re-blessed once (the appended entry), the stage manifest's stage 9 coverage note drops "pitch-kept speeds" and its model version moves to 0.23.0 (one re-bless of the canonical text, README table and guide blocks); A-054 amended. | **As stated** |
| D9 | **C.** A setter and a getter for the mode (a `uint32_t` mode: 0 the default, 1 pitch kept), a new status for an invalid mode, the control reachable through `rr_dr60_schedule_change` by its index. **No struct changes**: the first exercise of the 0.22 promise (a control, two functions, two defines and a status appended; every C caller compiled against 0.22 keeps working unchanged). | **Appended only** |
| D10 | **Version 0.23.0**; the records: the guide's playback-speed section (both modes, the convention, the figures, the Workbench placement), the README, the CHANGELOG under Added, decisions.md, brief.md's speed line, 1.0-feature-set.md's speed row and open items, CLAUDE.md. | **As stated** |

## Clarifications

*(The draft reviews' findings and the owner's sign-off are recorded here when they arrive.)*

## Overview

Since 0.13 the engine plays a recording at the device's five speeds, the pitch moving with the speed, because the modeled mechanism is a resampling of the decoded stream. Investigators reviewing a recording slow it down to hear a passage; at 0.6 the voice drops a major sixth, which changes what they hear. The device's own mechanism is unknown; the research leans weakly toward a pitch-keeping device. This feature adds the pitch-kept behavior as a selectable option beside the modeled default, as one more live control with the rules the speed level already has, so an app offers "slower, same pitch" without changing what the default engine sounds like. If a capture of a real unit shows the device keeps pitch, the option becomes the default by an assumption change; until then it is an emulator convention in the Workbench.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Slower, same pitch (Priority: P1)

A listener plays a recording at a slow level with the pitch-kept mode on and hears the voice at its own pitch, stretched; at a fast level, compressed; at level 3, unchanged.

**Independent Test**: play a stored 1 kHz tone and the voiced stimulus at each level in each mode; read the fundamental, the level, the spurious products, the length and the place.

**Acceptance Scenarios**:

1. **Given** a stored 1 kHz tone and the pitch-kept mode, **When** played at each level, **Then** the fundamental is within ±1 % of 1 kHz at every level and rate, the level within ±1 dB, and the spurious products at least 30 dB down (6 kHz, 20 – 3000 Hz).
2. **Given** the same tone and the default mode, **When** played at each level, **Then** the fundamental is the stored one times the ratio (within ±1 %) and the output equals 0.22's bit for bit.
3. **Given** the voiced stimulus, **When** played at level 1 and level 5 in pitch-kept mode, **Then** its pitch track stays within ±1 % of the stored track over the voiced frames, and the output's length is ⌈K ÷ r⌉ within the stated frame granularity.
4. **Given** either mode at level 3, **When** played, **Then** the output and every play report are bit-identical to 0.22.
5. **Given** a playback in pitch-kept mode, **When** `place` is read after each call, **Then** it advances r per output sample and reaches K at the end; `playback_length_samples` is within one frame of the observed total.

---

### User Story 2 - A live control like the level (Priority: P2)

An app sets the mode before or during a playback, schedules it at a position, and reads every change in the log.

**Acceptance Scenarios**:

1. **Given** a playback in the default mode, **When** the mode is set to pitch kept at a position, **Then** the output from that device sample is a 20 ms crossfade from the resampled stream to the time-scaled one, the click energy against the held mode at least 30 dB below an instant switch, and the entry is logged with its tag.
2. **Given** a playback in pitch-kept mode, **When** the level changes, **Then** the new ratio takes effect at the next analysis frame (at most about 20 ms), with no step in the output (the click reader), the entry logged.
3. **Given** `reset` or `reconfigure`, **When** the mode was set, **Then** it is kept (like the level); a plain set of the mode already in force is not logged; a scheduled set to it is a logged no-op.
4. **Given** the control manifest, **When** read, **Then** the entry at index 30 is `playback_speed_mode`, a choice of two, kind model internal, timing live, path playback, default the first option, with one neutral sentence, and the sentence passes the denylist.
5. **Given** an invalid mode value through C or Rust, **When** set or scheduled, **Then** it is refused with the new status and nothing changes.
6. **Given** a C caller built against 0.22's header, **When** it runs against 0.23, **Then** every call it makes works unchanged (the first exercise of the 0.22 promise); the parity rows for the mode agree between the Rust and C paths.

---

### User Story 3 - Nothing else moved (Priority: P3)

1. **Given** the sixteen golden guards and every smoke hash, **When** 0.23 is built, **Then** they are unmoved; the new golden is blessed once.
2. **Given** the A/B against 12a3c45, **When** run solo and on the runner, **Then** the default mode is within 3 %; pitch-kept mode is within 3 × the default's cost per level (reported), its worst 64-sample block within the real-time budget.
3. **Given** the stage manifest, **When** read, **Then** stage 9's coverage note no longer lists pitch-kept speeds and its model version is 0.23.0; the registers cross-check passes; A-054 carries the dated note.

### Edge Cases

- A mode change at the playback's first output switches at once (no blend).
- A mode change and a level change at the same position: the mode blend runs and the level applies inside the new mechanism at its next frame.
- A mode request during a mode blend waits for the blend's end (the latest wins), as spec 019's level requests do.
- The recording's end during a blend: the old mechanism's remainder past the end is cut by the drain (spec 019's rule).
- A very short recording (shorter than one analysis frame): pitch-kept mode plays it with the frame's edge handling; the length rule's tolerance absorbs it; stated.
- Level 3 in pitch-kept mode is the exact bypass (no time-scaler state advances), so a change away from and back to level 3 behaves as in the default mode.
- The one-pass stream has no playback speed and no mode (the control is refused on the one-pass and record streams, as the level is).
- The processing record's timeline map (c) replays the level's nominal place; the mode does not move the nominal place, so the walker is unchanged; the content alignment inside a frame is the time-scaler's (stated in the guide).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The engine MUST offer a playback speed mode with two values, the default (the pitch moving with the speed) and pitch kept, as a live control on the playback stream: a plain setter and getter kept by `reset` and `reconfigure`, and a scheduled change in the output clock; every change MUST be a change entry; an invalid value MUST be refused with a dedicated error.
- **FR-002**: In pitch-kept mode at levels 1, 2, 4 and 5 the output MUST be the decoded stream time-scaled by the level's ratio at the device rate with its pitch kept: on a sustained tone and on voiced material the fundamental within ±1 % of the stored one, the level within ±1 dB, spurious products at least 30 dB below a tone in 20 – 3000 Hz at 6 kHz.
- **FR-003**: Level 3 MUST be the identity in both modes, and the default mode MUST reproduce 0.22 bit for bit in every configuration.
- **FR-004**: The place, length and progress rules of the level MUST hold in pitch-kept mode within a stated frame granularity: the own part in ⌈K ÷ r⌉ device samples ± the stated bound, `place` reaching K, `playback_length_samples` within one frame.
- **FR-005**: A level change in pitch-kept mode MUST take effect within one analysis frame with the click energy against the held level at least 30 dB below an instant switch; a mode change during playback MUST be a 20 ms crossfade from the common place; a change on the first output MUST switch at once; a request during a blend MUST wait for its end.
- **FR-006**: The control manifest MUST carry the entry at index 30 (`playback_speed_mode`, a choice, model internal, live, playback) with a neutral sentence; the stage manifest's stage 9 MUST drop "pitch-kept speeds" from its coverage note at model version 0.23.0; A-054 MUST carry a dated note; no new assumption.
- **FR-007**: The output MUST be deterministic at every rate and the processing real-time safe (no allocation after construction).
- **FR-008**: The C API MUST gain the mode's setter, getter, two defines and a status, with no change to any struct; the control MUST be reachable by index through the schedule; the Rust and C paths MUST agree on the parity rows.
- **FR-009**: The cost MUST be reported per level and bounded: pitch-kept mode at most 3 × the default's per-sample cost at 48 kHz, the worst 64-sample block within the real-time budget; the default mode within the 3 % A/B budget against 12a3c45.
- **FR-010**: The records of D10, with the convention stated ("an engine convention, not a device behavior; the device's method is undocumented") wherever the mode is described; the sixteen golden guards and every smoke hash unmoved; the new golden blessed once.

### Out of Scope

- A spectral or formant-preserving method; a decoder-domain method (S-018's class).
- Changing the default mode or the five ratios (capture P1).
- A continuous tempo control beyond the five levels; the app's UI.

### Key Entities

- **Playback speed mode**: the live choice between the two mechanisms, with the level's lifecycle.
- **Time-scaler**: the pitch-kept mechanism's state (frame, search, overlap), reserved at construction.
- **Mode blend**: the 20 ms crossfade between the two mechanisms' outputs at a mode change.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: At seven rates and five levels, the pitch-kept fundamental within ±1 % on the tone and the voiced track, the level within ±1 dB, spurious ≥ 30 dB down at 6 kHz; the lengths within the stated bound; every figure printed by the harness.
- **SC-002**: The click energy of every level change in pitch-kept mode and of every mode change ≥ 30 dB below the instant switch on the asserted rows.
- **SC-003**: The sixteen golden guards and every smoke hash unmoved; level 3 and the default mode bit-identical to 0.22 in the matrix.
- **SC-004**: The A/B against 12a3c45: the default mode within 3 % (solo; the runner's ceiling 5 %); pitch-kept mode ≤ 3 × per level; the worst block within budget.
- **SC-005**: The control manifest, the stage manifest and the registers cross-check green after one re-bless each; a 0.22-built C smoke binary runs unchanged against 0.23 (the promise); coverage within 1 point of 96.73 %.

## Assumptions

- **Alternatives considered**: a boolean switch instead of a choice (a third mechanism would then need another control); a device control (the device's mechanism is unknown, so the kind is model internal until a capture decides); a construction-time setting (an investigator toggles it while listening); a decoder-domain method (the codec is a class match, so S-018's cycle-level method would be modeled on a decoder the device does not have); a spectral method (heavier, no better fit to the question); crossfading level changes in pitch-kept mode too (the overlap-add is already continuous; a blend would double the smoothing); a wider budget (3 × covers a ±5 ms search at 6 kHz with margin).
- **The device's mechanism is unknown** (A-054); this feature models neither reading of the evidence, it adds the behavior the research leans toward as an option and leaves the default alone.
- **The time-scaler's frame granularity** is the design's figure; the spec's bound is one frame unless the design pins tighter.
- **No device claim**: the option is an emulator convention; every description says so.
