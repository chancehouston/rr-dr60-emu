# Feature Specification: Volume Ramp (a smooth transition for volume position changes, stage 11)

**Feature Branch**: `011-volume-ramp`

**Created**: 2026-10-16

**Status**: Draft

**Input**: User description (abridged; the full text is in the specify session's transcript): "Volume ramp: a fixed-length ramp for volume position changes on the output chain (signal-chain stage 11), the follow-up queued by spec 009's post-implementation hardware review (T043 N2). Today a position change is an instantaneous gain step at the next block boundary; a large step (for example +20 dB during playback) produces an audible click, and the size of the discontinuity depends on the host's block size. The device's own volume control is a 50 kΩ analog potentiometer turned by a thumbwheel, which cannot jump between positions, so the emulated knob should move smoothly. A short, fixed-length, smooth transition from the old gain to the new one, starting at the next block, on both routes, in the one-pass processing and in the playback path. To settle: the length (fixed in time) and shape; no overshoot of either position's value; a change arriving mid-ramp starts from the current gain; position 0 still reaches exact zeros once the ramp ends; with no volume change the output bit-identical to version 0.10 at every tap (the six golden files unmoved); deterministic and real-time safe with a cost budget stated as a ratio; what reset, reconfigure, a route change, the stage-11 bypass and the start of a playback do to a ramp; whether the length is a setting; how the length traces to a source or an assumption, or is an emulator convenience. Tests compare against the two steady gains (no overshoot, monotonic, length within a tolerance, the click's high-frequency energy reduced against today's step) at the seven host rates. Out of scope: route-change clicks (spec 009 N3), the earphone follower's cut-off, a clip indicator."

## Overview

Since [spec 009](../009-output-chain/spec.md) the volume position is a live control: the host sets it at any time, including during playback, and it takes effect at the next block (009 FR-003, FR-009). The engine turns the position into each route's gain by the circuit's loaded law ([A-044](../../docs/hardware/assumptions.md)) once per block, so a change is an **instantaneous gain step at a block boundary**. On a playing signal a large step, for example from a quiet position to the top of the travel (about +20 dB), puts a discontinuity into the waveform that is heard as a click, and where the step lands depends on the host's block size. Spec 009's post-implementation hardware review recorded this (T043 N2) and queued a fixed-length ramp.

The device has no such step. Its volume control is a 50 kΩ linear potentiometer turned by a thumbwheel ([S-002](../../docs/hardware/sources.md)); the wiper moves continuously, so the gain passes through every intermediate position on its way. This feature makes the emulated knob do the same: a position change becomes a **short transition of fixed duration**, starting at the next block, during which the knob's position moves monotonically from where it was to where the host set it and the gain at every sample is the loaded law's gain at that intermediate position. A full-travel change and a small nudge take the same time; the transition is the same length at every host rate and does not depend on the block size.

The feature changes nothing when the volume does not change. The first block after construction, reset or reconfigure, and the first block of each playback, take the position at once (a playback starts from rest, [A-037](../../docs/hardware/assumptions.md)), so every configuration that sets its volume before it processes is bit-identical to version 0.10 at every tap, and the six golden files stay unmoved. What changes is the output during the transition after a mid-stream change, which spec 009's checks measured as a step at the next block.

The transition is not a limiter, a fade on route changes or a model of how fast a person turns the thumbwheel: the host decides when the position changes, and the engine moves the knob there in a fixed time. Route-change clicks (spec 009 N3) stay out of scope.

## Clarifications

### Session 2026-10-16 (specify: the three choices the description left open, answered by the user)

- Q: How long is the transition? → A: **20 ms**, fixed in time at every host rate (FR-002). It is slightly longer than a 60 Hz UI frame (16.7 ms), so a dragged thumbwheel sending one position per frame chains into one continuous motion with no flat steps between updates, and the knob trails a drag by at most 20 ms. A numerical check at the specify step (a 1 kHz tone, 0.1 → 1 linear in gain at 48 kHz, a 1 s Hann window, the energy more than 250 Hz from the tone) puts the splatter **about 32 dB** below the instantaneous step's (FR-004's floor is 20 dB; the plan computes the figure for the chosen shape and the loaded law's curve). Rejected: 10 ms (about 26 dB; a 60 Hz drag becomes short ramps with flat steps between) and 50 ms (about 40 dB, but it lags a UI visibly; a change to 0 waits 50 ms for the exact zeros).
- Q: Is the transition a modeled behavior or an emulator convenience? → A: **A modeled behavior of stage 11, with a new assumption A-045** (FR-010): the change sits inside a device stage and changes its tap, the device's wiper moves continuously (S-002), and the length is a default parameter value, which constitution II requires to trace to a registered assumption. Stage 11's model version moves to 0.11.0 and the canonical text and the README table are re-blessed once. Rejected: an emulator convenience like spec 010's output clamp, whose bound sits after the transducer templates and corresponds to nothing in the device; this transition does.
- Q: Is the transition fixed, or a host setting? → A: **Fixed and always on** (FR-011): no new setting and no C structure change; the instantaneous step stays reachable through the test hooks only, for the click comparison. The wider controls discussion the user queued may revisit it. Rejected: an on/off setting and a length setting with 0 meaning the step (each a C structure change with refusals and a test across values, without an app request).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Click-free volume changes during playback (Priority: P1)

An app developer binds the recorder's thumbwheel in the app to the engine's volume position. While a recording plays, the user flicks the wheel from near the bottom to the top. With version 0.10 the playback clicks at the change; with this feature the level rises smoothly over the transition and there is no click, whatever block size the app's audio callback uses. When the user drags the wheel, the app sends a new position every screen frame and the level follows the drag continuously. An audiophile turning the volume during a listening comparison hears the level change, not an artifact of the engine.

**Why this priority**: this is the whole feature: the one audible defect spec 009 left in the live control. It is a usable increment on its own.

**Independent Test**: can be fully tested with a fresh engine at each supported host rate in a **stage-11-only configuration** (every earlier stage bypassed, spec 009's output-chain-only configuration with the speaker template and the earphone output bypassed, read at the tap after stage 11), so the output is the volume gain applied to the input through the volume control's coupling corner and nothing else's behavior enters (constitution VII). The stimuli are synthetic steady tones; the **gain trajectory** is read as the per-sample ratio of the run's output to a reference run held at a fixed position on the same input (spec 002 R-11's method; the coupling corner sits before the gain, so the ratio is the gain ratio exactly), over the samples where the reference is not near a zero crossing. The memory, the export and the C API are not needed.

**Acceptance Scenarios**:

1. **Given** the stage-11-only configuration on the earphone route at each of the seven host rates, a steady full-scale tone and a position of 0.1, **When** the position is set to 1 between two blocks, **Then** the gain trajectory (the output divided by the output of a reference run at the same signal) starts at the old position's gain at the first sample of the next block, moves **monotonically** to the new position's gain, reaches it **exactly** after the transition length (FR-002) within one host sample, and stays there; no sample's gain lies outside the range between the two positions' gains.
2. **Given** the same configuration, **When** the same change is made with block sizes of 1, 64, 4096 host samples and an irregular partition, the change requested at the same sample position, **Then** the outputs are bit-identical from that sample on (the transition does not depend on the partition).
3. **Given** the same configuration and a full-scale 1 kHz tone, **When** the position changes from 0.1 to 1 (and, separately, from 1 to 0.1), **Then** the energy of the output's spectrum more than 250 Hz from the tone in a window around the change is **at least 20 dB below** that of the same change made as an instantaneous step (version 0.10's behavior), at every host rate; the plan computes the expected figure from the transition's shape and states the tolerance.
4. **Given** the speaker route in the stage-11-only configuration (the amplifier's knee on), **When** the same changes are made, **Then** the volume gain before the knee follows the same trajectory, the knee's output never exceeds its ceiling, and the trajectory checks of scenario 1 hold for a tone below the knee at both positions.
5. **Given** a transition in progress, **When** the host sets a new position, **Then** a new transition of the full length starts at the next block from the knob's **current** intermediate position, with no jump in the gain, and moves monotonically to the new position; **When** the host sets the position it is already moving to, **Then** the transition in progress continues unchanged.
6. **Given** a transition to position 0, **When** it ends, **Then** the volume gain is exactly zero, the tap after stage 11 reads exact zeros from the first sample after the transition, and every route output decays below −120 dBFS within 100 ms of the transition's end (spec 009 FR-009's bound, measured from the end of the transition).
7. **Given** a change of the position on either route with all the output stages on, **When** it is played, **Then** the route output contains no step discontinuity at the change (the splatter criterion of scenario 3 holds at the route output with the speaker template or the earphone output in place, reported per rate).

---

### User Story 2 - The transition through the whole engine (Priority: P2)

The developer who already uses the engine finds that the volume behaves the same in the one-pass processing and in the playback path, that it starts every playback and every reset at the set position without a transition, that a route change and a reconfigure behave as stated, that nothing else changed (every golden file, every tap with an unchanged volume), and that the transition costs little.

**Why this priority**: the guarantees of specs 006 – 010 (the one-pass equals record-then-play; the controls at the next block; the golden files; real-time safety) must hold with the transition, or it is not usable in the structured engine. Separable from P1: P1 proves the transition in one configuration, P2 proves it does not disturb the rest.

**Independent Test**: can be tested by running the existing equivalence, determinism, allocation-free and golden checks, by start-of-stream and route-change cases, and by a timing comparison, without any new signal measurement beyond US1's trajectory reader.

**Acceptance Scenarios**:

1. **Given** a fresh engine, a reset engine, a reconfigured engine, or the first block of a playback, **When** the host has set a position before that block, **Then** the first block runs at that position from its first sample, with no transition, bit-identical to version 0.10.
2. **Given** a constant volume (set before the first block and never changed), **When** any existing check runs, **Then** every tap is bit-identical to version 0.10 at every route, position and host rate; all six existing golden files are byte-identical and their SHA guards unchanged.
3. **Given** a transition in progress, **When** the route changes, **Then** the transition completes at once (the newly selected route's stages start from rest at the new position, spec 009 FR-009); **When** the engine is reset or reconfigured, or a playback ends and another starts, **Then** no transition carries over; **When** stage 11 is bypassed, **Then** the position (and any transition) has no effect, as in spec 009.
4. **Given** record-then-play and the one-pass processing, both routes and the seven host rates, **When** the volume is constant, **Then** they are bit-identical under spec 008's equivalence rule as in 0.10; **When** the same change is requested in both at the same output sample position, **Then** the two transitions are bit-identical from that sample on.
5. **Given** the output clamp of spec 010 on, **When** a transition runs, **Then** the route output stays bounded and equals the clamp-off output bounded sample for sample (spec 010 FR-005).
6. **Given** a transition, **When** the per-block processing runs, **Then** it allocates nothing and is deterministic across platforms and partitions; **When** the codec-bypassed one-pass cost per host sample at 48 kHz is measured on the speaker route in a release run with the timing tests one at a time, each side the fastest of nine alternating runs, **Then** with no transition in progress the cost rises by at most **3 %** against version 0.10, and with a transition running in every block by at most **15 %**; both routes reported, the nanoseconds stated beside the ratio.
7. **Given** a position refused as invalid (spec 009 FR-003), **When** the next block runs, **Then** no transition starts and the position is unchanged.

---

### User Story 3 - The transition is documented and traceable (Priority: P3)

A reader of the README, the register and the API documentation learns that a volume change takes a fixed time, how long, that it starts at the next block, that the start of a stream and of a playback take the position at once, that the reported position is the host's setting (not the knob's intermediate position), and where the transition's length comes from. The records say what is modeled on the device and what is an engine choice.

**Why this priority**: the project's traceability rule (constitution II) covers the transition's length like any default value; and app developers need the timing to align the app's own UI with the sound. Separable: the records can be checked without the audio.

**Independent Test**: the manifest cross-check, the traceability audit and the wording checks run against the updated register, README, CHANGELOG and API documentation; a reader test on the README.

**Acceptance Scenarios**:

1. **Given** the README, **When** the volume paragraph is read, **Then** it states the transition's length, that it is the same for any size of change and at every rate, that it starts at the next block, that the start of a stream and of a playback take the position at once, that the read-back position is the host's last setting, and how a dragging UI's updates chain into a continuous motion, in under five minutes of reading.
2. **Given** the register and the stage manifest, **When** stage 11's records are read, **Then** the transition traces to A-045 (and A-044's row names it), stage 11's manifest entry cites A-045 with model version 0.11.0, and the CHANGELOG and spec 009's FR-009 note record the change.
3. **Given** every string this feature adds, **When** the wording checks run, **Then** they pass spec 007's denylist and the claims policy (no claim that the transition protects hearing or equipment, no "accurate").

---

### Edge Cases

- **A change before the first block.** Construction, reset, reconfigure and the start of a playback take the position at once (no transition); a host that sets its position and then processes gets exactly 0.10's output.
- **A change to the same position.** Not a change: no transition starts, and a transition already moving to that position continues.
- **A change mid-transition.** A new full-length transition starts at the next block from the knob's current intermediate position; the gain never jumps and never leaves the range between the knob's current gain and the new target's.
- **Rapid updates (a dragging UI).** With updates arriving more often than the transition length, each update restarts a transition from the current position; the knob trails the latest setting by at most one transition length, and the gain stays continuous.
- **A refused position.** Unchanged position, no transition.
- **Position 0.** The gain reaches exactly zero at the transition's end; spec 009's exact zeros at the tap after stage 11 hold from then on.
- **The read-back position.** Reading the volume returns the host's last setting, not the intermediate knob position, so 009's read-after-write behavior is unchanged.
- **A route change during a transition.** The transition completes at once; the new route starts from rest at the target position. The route change's own click is spec 009 N3's and out of scope.
- **Stage 11 bypassed.** The position and any transition have no effect (unity gain at the codec's scale, 009 C4); a later reconfigure that re-enables stage 11 takes the position at once.
- **A transition longer than the remaining playback.** The playback's drain continues the transition; the end of a playback ends it (the next playback starts at the set position).
- **Recording.** The record path has no volume; the speaker is muted while recording (009); nothing here applies.
- **Low host rates.** At 6 kHz the transition spans fewer host samples than at 48 kHz; its duration is the same within one host sample.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: **The transition.** A change of the volume position MUST take effect as a **transition** that starts at the first sample of the next block: during it, the knob's position MUST move monotonically from its position at the change to the newly set position, and the volume gain at every sample on each route MUST be the loaded law's gain (A-044) at that intermediate position; at the transition's end the gain MUST equal exactly the new position's gain as computed in version 0.10, and stay there.
- **FR-002**: **Fixed length.** The transition MUST have the same duration for any size of change, at every supported host rate (the same duration in time within one host sample), and MUST NOT depend on the block size or partition. Its duration MUST be **20 ms** (A-045), that is round(0.02 × host rate) host samples: 120 at 6 kHz, 160 at 8 kHz, 320 at 16 kHz, 882 at 44.1 kHz, 960 at 48 kHz, 1764 at 88.2 kHz and 1920 at 96 kHz.
- **FR-003**: **No overshoot, no jump.** At no sample during a transition MAY the volume gain lie outside the range between the gain at the transition's start and the new position's gain (spec 009's edge case "volume changed mid-block"); a change arriving during a transition MUST start a new full-length transition from the knob's current intermediate position, with no discontinuity in the gain.
- **FR-004**: **Click reduction.** For a full-scale 1 kHz tone and a change between positions 0.1 and 1 in either direction, the energy of the output's spectrum more than 250 Hz from the tone, in a window around the change, MUST be at least 20 dB below that of the same change made as an instantaneous step, at every host rate, at the tap after stage 11; the plan computes the expected figure from the transition's shape and states the tolerance.
- **FR-005**: **Start at once.** The first block after construction, reset or reconfigure, and the first block of each playback, MUST run at the set position from its first sample with no transition; a reset, a reconfigure, the end of a playback and a route change MUST end any transition in progress (the route change at the new position).
- **FR-006**: **Unchanged without a change.** With a volume that does not change after the first block, every tap's output MUST be bit-identical to version 0.10's at every route, position and host rate; the six existing golden files MUST be byte-identical and their SHA guards unchanged. The plan MUST verify the precondition that no golden entry changes the volume after its first block.
- **FR-007**: **Both paths, both routes.** The transition MUST apply on the speaker and the earphone route, in the playback path and in the one-pass processing, which share the volume control as in spec 009; the same change requested at the same output sample position in record-then-play and in the one-pass MUST give bit-identical output from that sample on, and record-then-play MUST equal the one-pass under spec 008's rule with a constant volume.
- **FR-008**: **Exact zeros.** A transition to position 0 MUST end with the volume gain exactly zero; the tap after stage 11 MUST read exact zeros from the first sample after the transition, and every route output MUST decay below −120 dBFS within 100 ms of the transition's end (spec 009 FR-009, restated from the transition's end).
- **FR-009**: **Determinism, real-time safety and cost.** The transition MUST be deterministic (bit-identical on every supported platform and partition; no platform math function) and allocation-free; its per-sample work MUST be bounded. With no transition in progress the codec-bypassed one-pass cost per host sample at 48 kHz on the speaker route MUST rise by at most **3 %** against version 0.10; with a transition running in every block, by at most **15 %**; measured as US2 AS6 states, the nanoseconds reported beside the ratio (spec 010's lesson: budgets are ratios).
- **FR-010**: **Traceability.** The transition MUST be a **modeled behavior of stage 11**: the device's wiper moves continuously ([S-002](../../docs/hardware/sources.md)), and the transition's length and shape are an engine choice registered as a new assumption **A-045** (low confidence) with its rationale and a verification method; the code and the records MUST cite it. Stage 11's manifest entry MUST cite A-045 and its model version MUST move to 0.11.0; the canonical text and the README's manifest table MUST be re-blessed once for that change (and the header's library version), recorded in the CHANGELOG (spec 007 FR-015). A-044's row MUST name the transition.
- **FR-011**: **Control surface.** The transition MUST be **fixed and always on**: no new setting, no change to the Rust settings or the C settings structure (it stays 112 bytes) and no new C function; version 0.10's instantaneous step is reachable only through the test hooks, for FR-004's comparison. The volume control's own surface (setting, reading and refusing a position) MUST be unchanged; reading the volume MUST return the host's last setting.
- **FR-012**: **Its own golden file.** The transition MUST have its own golden file at the seven host rates: the stage-11-only configuration and the default engine, both routes, a steady tone with at least an upward change, a downward change, a change to 0, and a change arriving mid-transition, blessed once and reproducing on every golden-matrix platform.
- **FR-013**: **Records.** The README's volume paragraph, the API documentation of the volume control (Rust and C), the CHANGELOG and spec 009's FR-009 note MUST state the transition, its length, its start at the next block, the start-at-once rule and the read-back rule; spec 009's checks that assert a step at the next block MUST be restated against the transition (the old position up to the next block, the new position's gain exactly from the transition's end). The version MUST be 0.11.0.

### Out of Scope

Route-change clicks and a fade on route changes (spec 009 N3); a model of how fast a person turns the thumbwheel (the host decides when the position changes); the knob-angle-to-position mapping (A-044 leaves it unmodeled); a volume law change; the earphone follower's class-A cut-off (A-032); a clip indicator; making other settings live controls (the controls discussion the user queued).

### Key Entities

- **Volume position**: the host's setting, 0 to 1 (spec 009 FR-003); what reading the volume returns.
- **Knob position**: the position the gain is computed from at each sample; equal to the volume position except during a transition.
- **Transition**: the fixed-length movement of the knob position from its value at a change to the newly set volume position, starting at the next block.
- **Transition length**: the transition's duration in time (FR-002), the same at every host rate within one host sample.
- **Stage-11-only configuration**: every earlier stage bypassed, stage 11 on, the speaker template and the earphone output bypassed, read at the tap after stage 11.
- **Gain trajectory**: the per-sample ratio of a run's output to a reference run's at the same signal, the reader of the transition's shape (spec 002 R-11's method).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: At the seven host rates on both routes, a change from 0.1 to 1 and from 1 to 0.1 gives a gain trajectory that is monotonic, never outside the two positions' gains, reaches the new gain exactly after the transition length within one host sample, and is bit-identical across block partitions of 1, 64 and 4096 samples and an irregular one.
- **SC-002**: The click energy (FR-004) is at least 20 dB below the instantaneous step's at every host rate, the figure reported per rate beside the plan's computed expectation.
- **SC-003**: A change mid-transition never produces a gain discontinuity larger than the transition's own largest per-sample increment, at every host rate; a transition to 0 ends at exactly zero gain and the route output decays below −120 dBFS within 100 ms of its end.
- **SC-004**: The six existing golden files are byte-identical with unchanged SHA guards; every pre-existing check that does not change the volume mid-stream passes unchanged; the feature's golden file is blessed once and reproduces on every golden-matrix platform.
- **SC-005**: Record-then-play equals the one-pass bit for bit with a constant volume and from the change on with a change at the same output sample, on both routes, at the seven host rates; 100 % of the determinism and allocation-free cases pass with transitions running.
- **SC-006**: The cost rises by at most 3 % with no transition in progress and at most 15 % with a transition in every block (US2 AS6), nanoseconds reported beside the ratio.
- **SC-007**: The manifest cross-check, the traceability audit and the wording checks pass; the README, the API documentation, the CHANGELOG and the register carry the stated text.
- **SC-008**: A developer can find, from the README alone, how long a volume change takes, when it starts, and what the start of a stream does, in under 5 minutes.

## Assumptions

- **A-045** (new, low confidence): a volume position change moves the device's wiper continuously, which the engine models as a fixed 20 ms movement of the knob position from the old position to the new, the gain following the loaded law (A-044) at every sample. The device's own transition time is the user's thumb, which the engine does not model; the length is an engine choice for a block-based host (one 60 Hz UI frame plus margin). Verification: none can pin a human-turned knob's speed; a capture of the device's speaker output during a fast thumbwheel flick would show the gain trajectory's shape (continuous, no step), which is what the register states with confidence.
- The knob position is interpolated, not the gain: the gain during a transition is the loaded law's gain at an intermediate position (FR-001). The loaded law is monotonic in the position on both routes (its derivative is positive on 0 … 1), so a monotonic knob movement gives a monotonic gain and the no-overshoot rule (FR-003) follows. The transition's shape in time (how the knob position moves between its ends) is chosen by the plan to meet FR-003 and FR-004 without a platform math function.
- The transition starts at a block boundary, as every control change does since spec 006 (the controls are read once per block); a host that wants a change at a given sample splits its block there.
- A playback always starts from rest (A-037), so the first block of each playback takes the set position at once; the one-pass processing does the same after construction, reset and reconfigure.
- A route change already resets the route's sections (spec 009 FR-009); completing the transition at once there adds no new discontinuity of its own.
- No golden entry changes the volume after its first block (to be verified by the plan, FR-006); spec 009's mid-stream volume-change checks are the only existing checks whose expected output changes.
- The cost budget is stated as ratios against version 0.10's codec-bypassed speaker-route one-pass (about 57 – 68 ns per host sample on the development machine), because a fixed figure did not transfer to the x86-64 CI runner (spec 010).
- The version is 0.11.0; no release has been published.
