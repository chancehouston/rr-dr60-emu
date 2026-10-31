# Feature Specification: Live Playback-Stage Bypass

**Feature Branch**: `021-playback-bypass`

**Created**: 2026-10-31

**Status**: Draft

**Input**: User description: "021 live playback-stage bypass with a crossfade (the owner's engine order 020 → 021, decisions.md 2026-10-29; controls-discussion.md § 2a row 6 'Stage bypass and A/B (with / without the speaker): playback-side stages live with a crossfade; record-side stages an offline re-render, not live', § 2b 'Playback-stage bypass, live (10, 11, 12, 13): both branches computed for a 20 ms crossfade; each bypassed branch keeps running to stay warm', § 4 item 8 '021: Live playback-stage bypass with a crossfade (M; Workbench A/B for stages 10 – 13)'). As a Workbench user listening to a playback, I want to switch each playback-side stage in and out while the recording plays, with no click, so that I can hear what each stage does to the same passage instead of stopping, reconfiguring and playing again."

## Decisions for sign-off

The owner reads this section first. Each decision gives the recommendation; the alternative is in the Assumptions section. The draft reviews (coach, hardware, product) are applied at their defaults below; their findings are listed under Clarifications.

| # | Decision | Recommended |
|---|---|---|
| D1 | **Which stages become live.** The four playback-side stage switches: stage 10 (the codec output filter, `playback_stage_enabled`), stage 11 (the volume control and power amplifier, `output.volume_amp_enabled`), stage 12 (the speaker template, `output.speaker_enabled`) and stage 13 (the earphone output, `output.earphone_enabled`). Stage 10 is included although its reported latency is not zero (0 … 1 host sample at 1 kHz; the drain carries its 121-sample settle), under D4. The output clamp stays a setting (D8). | **All four** (alternative: 11 – 13 only) |
| D2 | **The control is the setting itself, scheduled only** (spec 017's pattern): a change is placed with the existing scheduling call at a position in the output clock of the **playback stream** or the **one-pass stream**, by the control manifest's index (5, 22, 23, 24), lands at the top of host sample P (stage 10 at its device-rate landing d(P), the speed's rule), is logged as a change entry (before and after 0 or 1, the stream word), and `settings()` reflects it. No plain setter. The record stream refuses the four (control not on stream). The pending schedule and the landed values follow spec 015's lifecycle (kept by `start_playback`, cleared at the playback's end, by `reset` and by `reconfigure`). | **The setting, scheduled only** (alternative: a separate "bypass" live control per stage) |
| D3 | **A switch is a 20 ms crossfade between the stage's input and its output, per stage.** Each stage configured on keeps running through a bypass (its filter state stays warm; the deterministic per-sample work is unchanged); the stage's contribution to the signal is x + w·(y − x), x the stage's input, y its processed output, w the smoothstep 3u² − 2u³ over N = 0.02 × rate host samples (N = 120 device samples for stage 10), u = n ÷ N for n = 1 … N, with w = 0 "on" and w = 1 "bypassed"; the downstream stages and the per-stage taps receive the blended signal; the blend's last sample is the target branch itself (019's lesson: a + 1.0·(b − a) is not b in floating point). Bit-identical to the stage on at w = 0 and to the same engine configured with that stage off at w = 1. | **Per-stage input/output blend** (alternative: two whole-chain branches blended at the route output) |
| D4 | **A stage configured off stays off.** A scheduled change that would switch **on** a stage whose setting at construction is off is refused at scheduling (`StageNotConfigured`, a new status), so the player's reported latency, drain and length never depend on a live value: a bypassed stage's settle is simply not needed. A change to the value already in force lands as a no-op entry (before == after), as spec 015 logs it. | **Refuse on-from-off** (alternative: allow it with a drain sized for every stage on) |
| D5 | **A toggle during a blend reverses from the current phase**: the fade's phase u runs back along the same curve from where it is (the weight stays monotonic in time within each direction, the whole excursion finishes within 20 ms of the last toggle); no wait (unlike the speed, spec 019 D4) and no restart from rest (unlike the volume, spec 011). | **Reverse from phase** (alternatives: wait; restart from rest) |
| D6 | **The one-pass carries the four too**, at its own apply point (spec 015: the chain pair before the chain, stage 10 before the upsampler), so a monitoring toy can A/B. The record path is untouched; record-side "with / without" is spec 020's offline re-render. | **Both streams** (alternative: the playback stream only) |
| D7 | **The meters show what is heard**: while stage 11's blend is at w = 1 the amplifier's clip count (`amp_clip_count`) stops counting (the knee is not heard) and counts again from the first sample at w < 1; the route peak, the reconstruction peak and the output-clamp count read the blended signal as they do today. The battery ceiling meter is unchanged. | **Count only when heard** (alternative: count the running knee regardless) |
| D8 | **The output clamp stays a setting.** It is a pure sample map on the route output and an emulator convenience, not a stage; it has no filter state to keep warm and needs no fade. It may become live later if the app asks. | **Unchanged** (alternative: a fifth live switch) |
| D9 | **Oracles.** (a) Every pre-021 configuration bit-identical to 0.20 (merge commit f303c99): the thirteen audio golden guards, the record golden and every smoke hash unmoved. (b) The two endpoint identities per stage, route, stream and rate: before a landing the output equals the "on" run; from the blend's last sample on it equals the same engine configured with that stage off, bit for bit. (c) The blend identity: each blend sample equals x + w_n·(y − x) on the two constant computations, w(⌊N/4⌋) = 0.15625 exactly and w_N = 1, at the measurement rates on a trace of the stage's input and output. (d) Spec 015's partition identity (block sizes 1, 64, 4096 and irregular partitions) and the block-size-1 oracle. (e) The click energy of the crossfade against the step, per stage, on a −6 dBFS stored 1 kHz tone and on the voiced stimulus: at least 30 dB below the step where the step's energy is at least 20 dB above the reader's floor; reported where the stage's effect is small. (f) The reverse case: a toggle back at u = 1/2 returns to w = 0 within N/2 samples along the reversed curve, to the bit against the reference. (g) The refusal of D4; the no-op entry; the log chained with the stream word; the latency, drain and length unchanged by any schedule. (h) The meters' rules of D7. | **As listed** |
| D10 | **Cost, records, version.** The per-sample cost at a constant state (no blend running) within **3 %** of 0.20 by the solo A/B (45 alternating runs; a **5 % runner ceiling** on the `ab-timing` label), the blend's extra work and the worst block reported; no allocation in a call; deterministic. No new assumption (an emulator convention, not a device behavior: the device has no such switches); the control manifest's four entries read timing **live** (one re-bless); the stage manifest's version line only; the C API unchanged except one appended status (36); the guide gains a "Live stage bypass" section; the CHANGELOG under Added; the product notes (controls-discussion § 2a row 6, § 2b, § 4 item 8; brief.md's Workbench A/B); version **0.21.0**. | **As listed** |

## Clarifications

*(The draft reviews' findings and the owner's sign-off are recorded here as they arrive.)*

## Overview

Every playback-side stage of the engine can be bypassed today, but only by `reconfigure`, which rebuilds the engine and resets every path: an A/B of "with the speaker" against "without the speaker" on the same passage means stop, change, play again, and remember. The Workbench wants the switch while the recording plays, so that the listener hears the stage's contribution appear and disappear on the same syllable. A switch in one sample would click (a stage's output and its input differ by the stage's whole effect: a filter's phase, an amplifier's gain and clipping, a speaker template's resonance), so the switch is a short crossfade between the stage's input and its output, in the engine's one transition length of 20 ms (specs 011, 017, 018, 019). The stage keeps running while bypassed so that switching it back in is seamless, and nothing else about the engine moves: a constant state is bit-identical to 0.20, the player's latency, drain and length are unchanged, and the record path is untouched.

The four switches become live values in the pattern of spec 017's record-side values: the setting itself is scheduled on a stream (the playback stream and the one-pass), lands on a sample, is logged, and is reflected by `settings()`. A stage configured off at construction cannot be switched on live, because its settle would have to be carried in the drain and its latency in the reported figure; this keeps every figure the app already relies on fixed, and the Workbench's A/B runs on an engine with every stage on.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Hear a stage come and go without a click (Priority: P1)

A Workbench listener plays a recording and switches the speaker template out and back in, then the amplifier, then the earphone output on the earphone route, then the codec output filter, each while the passage plays. Each switch is heard as the stage's effect fading over 20 ms, with no click, and the stage's effect is exactly present or exactly absent outside the fade.

**Why this priority**: it is the whole feature; everything else bounds it.

**Independent Test**: a stored −6 dBFS 1 kHz tone played on the speaker route with stage 12 switched off at a known position and on again later: before the landing the output equals the all-on run; from the blend's last sample it equals the same engine configured with stage 12 off; each blend sample equals x + w·(y − x) on a trace of stage 12's input and output; the click energy around the switch is at least 30 dB below the step's.

**Acceptance Scenarios**:

1. **Given** any pre-021 configuration (every stage at its construction value, no bypass scheduled), **When** played, recorded and run one-pass at all seven host rates, **Then** every output sample is bit-identical to 0.20's: the thirteen audio golden guards, the record golden and every smoke hash unmoved.
2. **Given** a recording playing with every stage on and a change of stage k ∈ {10, 11, 12, 13} to bypassed landing at host sample P (stage 10 at its device-rate landing), **When** the output is read, **Then** before P it equals the all-on run; blend samples n = 1 … N equal x + w_n·(y − x) of the stage's input and processed output, with w(⌊N/4⌋) = 0.15625 and w_N = 1 exactly; from P + N − 1 on it equals the same engine configured with stage k off, bit for bit, on both routes (stage 13 on the earphone route, stages 11 – 12 on the speaker route, stage 10 on both), at the measurement rates.
3. **Given** a stage bypassed and a change back to on landing later, **When** read, **Then** the blend runs w from 1 to 0 by the same curve and from its last sample the output equals the all-on run, bit for bit (the stage ran warm throughout).
4. **Given** a −6 dBFS stored 1 kHz tone and the voiced stimulus, **When** each stage is switched by the engine and by a reference that steps the weight, **Then** the crossfade's click energy (a 40 ms reader centred on the blend's midpoint, the tone excluded) is at least 30 dB below the step's wherever the step's energy is at least 20 dB above the reader's floor, and is reported otherwise (a stage whose effect on the tone is small has a small step to begin with).
5. **Given** the same schedule at block sizes 1, 64 and 4096 and irregular partitions, **When** played, **Then** the outputs and the logs are identical; the block-size-1 oracle of spec 015 holds.
6. **Given** a toggle back while a blend is at phase u = 1/2, **When** read, **Then** the weight returns along the reversed curve and reaches 0 within N/2 samples, to the bit against the reference.

---

### User Story 2 - The switch is bounded, described and safe (Priority: P2)

An app developer relies on the switch doing nothing else: the latency, drain and length fixed, a configured-off stage refused, every landing logged, the meters telling what is heard, the control manifest saying which controls are live, and the cost within budget.

**Independent Test**: an engine configured with stage 12 off refuses a scheduled change to stage 12 on with the new status and plays bit-identically to 0.20; an engine with every stage on plays the same number of samples with any schedule as without; the log holds one entry per landing, chained, with the stream word.

**Acceptance Scenarios**:

1. **Given** an engine whose setting for stage k is off, **When** a change switching stage k on is scheduled on either stream, **Then** it is refused with `StageNotConfigured` (status 36), nothing is pending, and the playback is bit-identical to 0.20.
2. **Given** any schedule of the four switches on an engine with every stage on, **When** played to the end, **Then** `playback_latency_samples()`, the drain and the total played length equal the no-schedule figures at every rate.
3. **Given** changes landing on both streams, **When** the logs are read, **Then** each landing is one entry (position, control index, before, after as 0 or 1, the caller's tag, the stream word), chained (each before equals the previous after), and `settings()` reads the landed value; a change to the value in force is a no-op entry; a change on the record stream is refused (control not on stream).
4. **Given** stage 11 bypassed on the speaker route with a clipping-level tone, **When** the meters are read per block, **Then** `amp_clip_count` is 0 in blocks wholly at w = 1 and counts again from the first block with w < 1; the route peak reads the blended output.
5. **Given** the control manifest, **When** read, **Then** the four entries read timing **live**, their paths and kinds unchanged, the canonical text re-blessed once; the stage manifest moved in its version line only; no new assumption.
6. **Given** the cost A/B against 0.20 at a constant state, **When** run solo, **Then** the per-sample cost is within 3 % on the playback stream and the one-pass (5 % on the runner); the blend's extra work and the worst block are reported within the real-time budget; no call allocates.
7. **Given** the C API, **When** built, **Then** the header differs by the one appended status only, the parity test shows the C path landing and blending exactly as the Rust path, and the C smoke gains a bypass case.

---

### Edge Cases

- A change scheduled at position 0 of the playback stream (or placed between `start_playback` and the first play call) lands on the first output: spec 011's first-sample rule applies (the stage is switched at once, no blend, nothing has sounded yet); the entry is logged.
- Two changes of the same stage at the same position: the later wins (spec 015's rule).
- Changes of two different stages at the same position: both land; two independent blends run.
- A toggle back exactly at the blend's last sample: the new blend starts from w = 1 (the direction reversed at the end of the curve).
- Several toggles within one blend: each reverses the direction from the current phase; the output stays within [min, max] of the two branches at every sample.
- A playback stopped during a blend: the next playback starts at the landed values with no blend (a stream's first sample); the pending schedule is cleared with the playback.
- `reset` and `reconfigure` restore the settings' values and clear the schedule and the log, as spec 015 does.
- A bypass landing during the drain (past the recording's own part): the blend runs on the drain's samples; nothing is added past the drain.
- The earphone route with stage 13 bypassed: the route's output is stage 11's output on that route (the earphone follower's corners removed), as the configured-off path gives it today.
- Stage 11 bypassed on the speaker route: the volume position and the battery value no longer act (they live in stage 11); a volume or battery change landing during the bypass is logged and takes effect on the running (unheard) branch, so switching the stage back in hears the new value.
- Stage 10 bypassed at a host rate other than 6 kHz: the blend is 120 device samples before the upsampler; its onset in host samples is reported like spec 015's speed onset.
- The one-pass with the codec on: the four land at the one-pass's apply point; the codec is untouched.
- A change scheduled on the record stream for any of the four: refused (control not on stream).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The four playback-side stage switches (stage 10 `playback_stage_enabled`; stages 11 – 13 `output.volume_amp_enabled`, `output.speaker_enabled`, `output.earphone_enabled`) MUST be schedulable as live values on the playback stream and the one-pass stream through the existing scheduling call by their control-manifest indices, landing at the top of host sample P (stage 10 at the speed's device-rate landing d(P)), logged as change entries (before and after 0 or 1, the stream word), reflected by `settings()`, with no plain setter; the record stream MUST refuse them.
- **FR-002**: A landing MUST start a crossfade of N = 0.02 × rate host samples (120 device samples for stage 10) between the stage's input x and its processed output y: the stage's contribution is x + w_n·(y − x), w_n = 3u² − 2u³ at u = n ÷ N, n = 1 … N, from w = 0 (on) to w = 1 (bypassed) or back; the last blend sample MUST be the target branch itself; the downstream stages and the per-stage taps MUST receive the blended signal.
- **FR-003**: A stage configured on MUST keep running while bypassed (its state warm, the per-sample work unchanged), so that from the blend's last sample a bypass is bit-identical to the same engine configured with that stage off, and a return is bit-identical to the all-on run.
- **FR-004**: A change switching on a stage whose construction setting is off MUST be refused at scheduling with a new status, `StageNotConfigured` (36); the player's reported latency, drain and total length MUST NOT depend on any live value.
- **FR-005**: A toggle during a blend MUST reverse the fade's direction from its current phase along the same curve; several toggles MUST each reverse; the output MUST stay between the two branches at every sample; a stream's first sample switches at once (spec 011's rule).
- **FR-006**: Every pre-021 configuration MUST be bit-identical to 0.20.0 at all seven host rates: the thirteen audio golden guards, the record golden and every smoke hash unmoved; the feature's own golden covers the schedules.
- **FR-007**: The click energy of a switch MUST be at least 30 dB below an instantaneous step's under this spec's reader (a 40 ms Hann window centred on the blend's midpoint, 20 Hz to the band's top, the stored tone excluded) on a −6 dBFS stored 1 kHz tone for every stage and route where the step's energy is at least 20 dB above the reader's floor; the remaining rows and the voiced stimulus's figures MUST be reported.
- **FR-008**: Scheduled changes MUST be partition-independent at every host rate (outputs and logs identical at block sizes 1, 64, 4096 and irregular partitions); spec 015's block-size-1 oracle MUST hold.
- **FR-009**: The meters MUST report what is heard: `amp_clip_count` MUST NOT count while stage 11's blend is at w = 1 and MUST count from the first sample at w < 1; the route, reconstruction and clamp read-outs read the blended signal.
- **FR-010**: Deterministic and real-time safe as every spec: no allocation in a call, every blend's state reserved at construction; the per-sample cost at a constant state within 3 % of 0.20 (5 % on the runner) on the playback stream and the one-pass; the blend's cost and the worst block reported.
- **FR-011**: The control manifest's four entries MUST read timing live (one re-bless), the stage manifest MUST move in its version line only, no assumption MUST be added or amended (the records say "an emulator convention; the device has no such switches"); the C API MUST change by the appended status only; the guide MUST gain a "Live stage bypass" section; the CHANGELOG MUST record the feature under Added; the product notes MUST be updated; version 0.21.0.

### Out of Scope

- Live bypasses of record-side stages (1 – 5, 7): the offline re-render through spec 020's processing record.
- The speech codec (fixes the memory's encoding and the latency), the speed stage (specs 013, 019) and the output clamp (D8).
- A stage's internal parameters (the volume position and the battery value are already live; the others are settings).
- Any new stage, any change to a stage's coefficients, pitch-kept speed (the next spec).
- Switching on a stage configured off (D4; reconsider with a drain sized for every stage if the app asks).

### Key Entities

- **Stage switch**: one of the four playback-side settings, a live value on the playback and one-pass streams, logged by its control index.
- **Blend**: the N samples after a landing, each x + w·(y − x) of the stage's input and output; its phase u and direction are the per-stage state; w = 0 is "on", w = 1 "bypassed".
- **Warm stage**: a stage configured on whose filter runs regardless of its live value; only its contribution to the signal is blended.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every pre-021 configuration is byte-identical to 0.20 at seven rates; the thirteen audio golden guards, the record golden and every smoke hash unmoved; one new golden for the schedules, blessed once.
- **SC-002**: The two endpoint identities hold to the bit for every stage, route and stream at seven rates (the bypass equals the configured-off engine from the blend's last sample; the return equals the all-on run); the blend equals x + w·(y − x) to the bit at the measurement rates with w(⌊N/4⌋) = 0.15625 and w_N = 1; the step, the linear-weight and the "forgot to keep the stage warm" mutants caught.
- **SC-003**: The click energy is at least 30 dB below the step's on every asserted row of the 1 kHz table; the remaining rows and the voiced figures reported.
- **SC-004**: Partition independence and the block-size-1 oracle at seven rates; the refusal, the no-op entry, the log's chaining and the fixed latency, drain and length shown on both streams.
- **SC-005**: Cost at a constant state within 3 % (5 % on the runner) on both streams; the blend's cost reported.
- **SC-006**: Coverage stays at or above 80 % and within 1 point of the baseline (0.20: 96.70 % locally).

## Assumptions

- **Alternatives considered**: stages 11 – 13 only (D1: stage 10 is the one stage whose bypass an app would most want to hear, and its latency is fixed by D4); a separate per-stage "bypass" control beside the setting (D2: two sources of truth for one fact; spec 017's pattern keeps one); two whole-chain branches with and without the toggled stage, blended at the route output (D3: twice the chain's work for 20 ms, identical for a single toggle, and ill-defined for overlapping toggles of two stages; the per-stage blend composes); allowing on-from-off with a drain sized for every stage (D4: a length change for every chain-off configuration, moving goldens; and the reported latency would have to be the all-on figure for a configured-off engine); waiting for a blend's end or restarting from rest on a toggle (D5: a two-state switch has a natural reverse; a wait would delay the listener's gesture by up to 20 ms, a restart would jump); the playback stream only (D6: the one-pass monitoring toy is the Workbench's other A/B surface and the apply point exists); counting the warm knee's clips regardless (D7: the inside would show clipping nobody hears); a fifth live switch for the clamp (D8: a sample map with no state; later if asked).
- **Not a device behavior**: the RR-DR60 has no stage switches; the crossfade and the warm stage are the emulator's conventions, stated as such in the guide, the control manifest's sentences and the CHANGELOG; no assumption is registered or amended.
- **The 20 ms length** is the engine's one transition length (A-045 for the volume, spec 017's hiss, spec 018's battery, spec 019's speed), restated as a convention here; no new constant is published unless the plan finds a reuse of spec 019's define unsuitable.
- **A bypassed stage 11 silences the volume and the battery** on the speaker route, as the configured-off path does today; their live values keep landing on the warm branch.
- **The taps read the blended signal** (`AfterPlayback`, `AfterVolumeAmp`, `AfterSpeaker`, `AfterEarphone`): what the next stage receives is what a tap shows; a tap that showed the processed signal while the route carried the input would mislead the inside.
- **The app** uses a toggle per stage, one change per gesture; no new read-out is needed (the log gives the position and the manifest the timing).
