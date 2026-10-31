# Feature Specification: Speed-Change Crossfade

**Feature Branch**: `019-speed-crossfade`

**Created**: 2026-10-30

**Status**: Draft (the three draft reviews pending; owner sign-off pending)

**Input**: User description: "019 speed-change crossfade (spec 013's follow-up B2; the owner's engine order 018 → 019, decisions.md 2026-10-29; controls-discussion.md § 4 item 6 "removes the tick on a level change to or from levels 4 – 5; a constant level stays bit-identical", § 2b row "Speed crossfade"). As a listener and as an app developer, I want a playback speed-level change to blend from the old level into the new over a short fixed time instead of switching in one sample, so that a change to or from the fast levels no longer ticks on voice content above about 1.6 kHz, while a constant level stays bit-identical to 0.18. […] Out of scope: pitch-kept speed (c1), the live playback-stage bypass crossfade (spec 021), any change to the ratios or the banks' bands, record-side behavior."

## Decisions for sign-off

The owner reads this section first. Each decision gives the recommendation; the alternative is in the Assumptions section. The three draft reviews are applied at their defaults once they are in; the points the reviewers raise as owner questions are listed here.

| # | Decision | Recommended |
|---|---|---|
| D1 | **A level change blends, it no longer switches.** From the sample a change lands on (today's landing: the plain setter's first computed device sample, or spec 015's `speed_landing(P, rate)`), the output is for **N device samples** a blend of the **old level's value and the new level's value at the same place**: (1 − w_n)·y_old + w_n·y_new, with w a published weight rising from 0 to exactly 1 at sample N. The place advances at the **new** level's ratio from the change, as today: nothing restarts, skips or repeats, the pitch moves with the new level from the change, and the recording's length rule (the own part, the tail, the latch at K) is unchanged. What changes is only that no band is gated on or off within one sample. | Yes |
| D2 | **N = 20 ms of device samples (120 at 6 kHz), fixed.** The volume transition's length (A-045), restated as an **engine convention** for this change too: the device's own speed-change behavior is unknown until capture P1 (A-054 amended: the crossfade is the emulator's convention, not a device claim; P1 asks what a level change sounds like on a real unit). A longer blend would smear a deliberate change; a shorter one leaves the 2 kHz step audible. | Yes |
| D3 | **The weight is the smoothstep 3u² − 2u³ at u = n ÷ N** (spec 011's shape: the landing sample is blend sample 1, the target exact at N, monotonic, zero slope at both ends), applied as an **amplitude** blend whose two weights sum to 1 (the two branches are the same signal through two bands at the same place, so they add coherently and the level is kept). A linear weight and an instantaneous switch are the mutants. | Yes |
| D4 | **A request during a blend waits.** A new level requested while a blend is running is applied when the blend completes (at most 20 ms later), so the output never carries two blends or a step; the latest request wins (an intermediate request that is superseded before the blend ends never sounds). The change entry of a scheduled change is still logged at its position (the request); the audible onset follows at most 20 ms later, documented like spec 015's speed onset. The alternative, completing the running blend at once and starting the new one, trades the delay for a small step. | Yes |
| D5 | **Level 3 blends too.** Level 3 (normal speed) has values only at whole places; during a blend into or out of level 3 the level-3 branch is a **unity-passband interpolation** evaluated at the common place (the same family of banks as the other levels, with the slow levels' band so that it passes the whole stored band), so a blend between 3 and another level is a blend of two bands like any other. A **constant level 3 stays exactly the stored samples** (spec 013 FR-006: bit-identical to 0.12), because the interpolation branch runs only inside a blend. Spec 013's deferred switch into level 3 (the change waits for a whole place, at most four device samples) is kept: the blend starts at that whole place. | Yes |
| D6 | **A blend crossing the latch continues.** If the recording's own part ends (the place reaches K) during a blend, the blend continues through the tail with both branches reading zeros past K; the level is fixed at the latch as today and the final count follows the active (new) level. The blend never adds or removes samples. | Yes |
| D7 | **Always on, no setting.** The crossfade is how a speed change sounds in this engine; spec 013's golden schedule entries (level changes mid-playback) therefore change and the speed golden is **re-blessed once**, its constant-level entries byte-identical; every other golden file, every SHA guard of a file without a speed schedule, and every smoke hash are unmoved. The CHANGELOG records it as a **convention change visible in schedule outputs**, not an assumption change. The alternative, an emulator option off by default, would keep 013's golden byte-identical but leave the tick as the default sound. | Yes |
| D8 | **Oracles.** (a) Every constant-level output bit-identical to 0.18; (b) the blend equals the published weights applied to the two constant-level computations at the common places **to the bit**, the weights exact at ⌊N/4⌋ and 1.0 at N, the output bit-identical to the old level's run before the change and to the new level's constant-level computation from sample N on (spec 013's continuity oracle, now from N); (c) the click energy of a change against the instantaneous switch (spec 011 FR-004's reader: a 0.5 s window, 20 – 2750 Hz, the tone's own ±250 Hz excluded) reduced by **at least 30 dB** on stored −6 dBFS tones at 1 kHz, 2 kHz and 2.4 kHz for the level pairs spec 013's `change_step` table lists, and reported on the voiced stimulus; (d) spec 013's schedule tests bit-exact against an updated harness reference that applies the same blend from its own published coefficients; (e) partition independence of a scheduled change at every host rate; (f) the length rule, the latch and the deferred switch unchanged (013's tests green). | Yes |
| D9 | **Cost.** At a constant level the per-sample cost within **3 %** of 0.18 (merge commit 13defc7) by the solo A/B (45 alternating runs; a **5 % runner ceiling** on the `ab-timing` label); during a blend the stage does about twice its work (two banks of 57 taps) for 20 ms, reported with the worst block, within the real-time budget; deterministic; real-time safe (no allocation: the second bank is designed at construction like the five). No C API change, no control manifest change; stage 9's manifest notes name the crossfade at model version 0.19.0; version 0.19.0. | Yes |

## Clarifications

### Session 2026-10-30 (the draft)

- The step figures quoted are spec 013 B2's, pinned by `speed_checks::change_step` from `speed_design.py` (−77 … −99 dB at 1 kHz; −0.7 … −1.9 dB re the tone at 2 kHz to or from level 5; about 0 dB at 2.4 kHz; −40 dB between levels 2 and 4).
- "Bit-identical at a constant level" is the thirteen golden files' constant-level entries, every SHA guard of a file without a speed schedule, and every smoke hash; the speed golden's schedule entries are the one intended change (D7).

## Overview

Spec 013 made the playback speed a live control with five levels and chose, for the moment of a level change, to switch in one sample: the output after a change at a place is simply the new level's value there, which keeps the place exact and the oracle simple. The cost, quantified then and documented in the README and the guide, is a tick on content above about 1.6 kHz when a change involves the fast levels, because the fast levels' band is narrower than the slow levels' and the missing content is gated on or off at once. This feature replaces the switch by a short blend of the two bands at the same place. A constant level is untouched; only the 20 ms around a change sounds different.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Change the speed without a tick (Priority: P1)

A listener changes the playback speed while a recording plays, by the control or by a scheduled change, and hears the speed and pitch move without a click, including changes to or from the fast levels on voice.

**Why this priority**: it is the whole feature; everything else describes its edges.

**Independent Test**: a recorded −6 dBFS tone at 2 kHz played with a change from level 3 to level 5 at a known position: the output before the change equals the level-3 run, from blend sample N on equals the level-5 constant-level computation from the same place, and the click energy around the change is at least 30 dB below the instantaneous switch's.

**Acceptance Scenarios**:

1. **Given** a recording playing at a constant level, **When** no change is made, **Then** every output sample is bit-identical to 0.18's at that level (the own part, the tail and the drain), at all seven host rates.
2. **Given** a change from level a to level b landing at device sample s, **When** the N blend samples are read, **Then** each equals (1 − w_n)·y_a + w_n·y_b computed from the two levels' constant-level values at the common place to the bit, with w at ⌊N/4⌋ equal to the published smoothstep value and w_N = 1; the output before s equals level a's run; from s + N − 1 on it equals level b's constant-level computation from that place.
3. **Given** stored −6 dBFS tones at 1, 2 and 2.4 kHz and the level pairs of spec 013's step table, **When** each change is made with and without the blend, **Then** the blend's click energy (spec 011's reader) is at least 30 dB below the instantaneous switch's; on the voiced stimulus the reduction is reported.
4. **Given** a change scheduled at position P on the playback stream, **When** the playback is run at block sizes 1, 64 and 4096 and the irregular partitions, **Then** the outputs and the logs are identical, and the change entry sits at P.
5. **Given** a change into level 3 at a non-whole place, **When** it lands, **Then** the blend starts at the next whole place (at most four device samples later, spec 013's rule) and the constant level 3 after the blend is exactly the stored samples.
6. **Given** a change whose blend would cross the end of the recording's own part, **When** it is made, **Then** the blend continues through the tail, the total length equals spec 013's rule for the new level, and the drain is unchanged.

---

### User Story 2 - The change is bounded and described (Priority: P2)

An app developer relies on the blend's timing and cost being fixed and documented: a change's audible onset, what a second change during a blend does, what the engine says about itself, and that nothing else moved.

**Independent Test**: a second change requested during a blend lands when the blend completes, at most 20 ms later, and is logged at its own position; the manifest's stage 9 names the crossfade; the cost A/B at a constant level is within budget.

**Acceptance Scenarios**:

1. **Given** a change requested (or scheduled) while a blend is running, **When** the blend completes, **Then** the new level's blend begins at that sample, the superseded intermediate request (if any) never sounds, every request is logged as a change entry at its position, and the output stays monotonic in its blend weights (no step).
2. **Given** the control manifest and the stage manifest, **When** read, **Then** the control manifest is unchanged (no new entry), and stage 9's notes name the crossfade at model version 0.19.0 with A-054 amended.
3. **Given** the cost A/B against 0.18 at a constant level, **When** run solo, **Then** the per-sample cost is within 3 % (5 % on the runner), and the blend's extra work for 20 ms and the worst block are reported within the real-time budget.
4. **Given** the C API and the Swift package, **When** built and tested, **Then** nothing changed: the header is byte-identical, the parity tests pass (the C path blends exactly as the Rust path).

---

### Edge Cases

- A change to the level already active: no blend, no change in output; the entry is logged as today (before == after).
- A change requested in the same call as the playback's start: the start value rule of spec 015 (no blend, no entry).
- A change at the last device sample of the own part, or during the tail: the blend runs through the tail with zeros past K; nothing is added past the drain.
- Two changes scheduled at the same position: the later wins, one blend (spec 015's rule).
- A change into level 3 requested at a non-whole place while a blend is running: the blend completes, then the deferred-switch wait, then the blend into 3.
- A playback stopped during a blend: the next playback starts at the requested level with no blend (a stream's first sample).
- At 6 kHz the blend is 120 device samples = 120 host samples; at other host rates the blend's length in host samples is N·rate ÷ 6000 through the reconstruction and conversion, and the onset is reported like spec 015's speed onset.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: A level change MUST produce, from the device sample it lands on, N device samples that are the weighted blend of the old level's and the new level's values at the same place, the place advancing at the new level's ratio, with the published smoothstep weight (0 before, exactly 1 at sample N); before the landing the output MUST equal the old level's run and from sample N the new level's constant-level computation from the same place, bit for bit.
- **FR-002**: N MUST be 20 ms of device samples (120), a documented engine convention (A-054 amended; the device's behavior unknown until capture P1).
- **FR-003**: At a constant level every output MUST be bit-identical to 0.18.0: the own part, the tail and the drain, at every level including level 3's stored-sample identity, at all seven host rates; the thirteen golden files' constant-level entries, every SHA guard of a file without a speed schedule and every smoke hash unmoved; the speed golden's schedule entries re-blessed once.
- **FR-004**: A blend into or out of level 3 MUST use a unity-passband interpolation branch for level 3 at the common place, so that a constant level 3 outside a blend stays exactly the stored samples; spec 013's deferred switch into level 3 MUST be kept (the blend starts at the whole place).
- **FR-005**: A request during a blend MUST wait for the blend's end and then start its own blend; the latest request wins; every request is logged as a change entry at its position (spec 015's log); the audible onset of a waited request is documented (at most 20 ms after the blend's end, plus the playback path's latency).
- **FR-006**: The blend MUST NOT change the recording's length rule, the latch, the tail or the drain: `playback_length_samples` and spec 013's length tests hold; a blend crossing K continues through the tail with zeros past K.
- **FR-007**: The click energy of a change against the instantaneous switch MUST be reduced by at least 30 dB on stored −6 dBFS tones at 1, 2 and 2.4 kHz for the level pairs of spec 013's step table (spec 011 FR-004's reader), and reported on the voiced stimulus.
- **FR-008**: Scheduled changes MUST be partition-independent at every host rate (the outputs and the logs identical at block sizes 1, 64, 4096 and the irregular partitions); the setter oracle of spec 015 MUST hold.
- **FR-009**: Deterministic and real-time safe as every spec: the second bank designed at construction, no allocation in a call; the per-sample cost at a constant level within 3 % of 0.18 (5 % on the runner); the blend's cost and the worst block reported within the real-time budget.
- **FR-010**: The C API and the control manifest MUST be unchanged; stage 9's manifest notes MUST name the crossfade at model version 0.19.0; A-054 MUST be amended (the convention, the P1 question); the CHANGELOG MUST record the convention change (the speed golden's schedule entries moved; not an assumption change); the guide's playback-speed section, the README's tick sentence and the product notes (controls-discussion § 2b and § 4 item 6, decisions.md) MUST be updated; version 0.19.0.
- **FR-011**: Spec 013's harness reference MUST apply the same blend from its own published coefficients, so that 013's schedule tests stay bit-exact oracles.

### Out of Scope

- Pitch-kept speed (mechanism c1): a later option, decided by capture P1 or an app request.
- The live playback-stage bypass with a crossfade (spec 021).
- Any change to the five ratios, the banks' bands, the look-ahead or the length rule.
- Record-side behavior: the speed control is playback only.

### Key Entities

- **Blend**: the N device samples after a landing, each a weighted sum of the old and the new level's values at the common place; its weights are published and fixed.
- **Level-3 interpolation branch**: the unity-passband bank that gives level 3 a value at a non-whole place inside a blend only.
- **Waited request**: a level requested during a blend, applied at the blend's end.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: At a constant level every output is byte-identical to 0.18 at seven rates; thirteen golden files' constant-level entries and every SHA guard without a speed schedule unmoved.
- **SC-002**: The blend equals the published weights on the two constant-level computations to the bit at seven rates; w(⌊N/4⌋) = 0.15625 and w(N) = 1 exactly; the linear and the step mutants caught.
- **SC-003**: The click energy of a change is at least 30 dB below the instantaneous switch's on the 1, 2 and 2.4 kHz tone cases (the 2 kHz to-or-from-level-5 cases included); the voiced figure reported.
- **SC-004**: Spec 013's schedule tests are bit-exact against the updated reference; the length rule, the latch and the deferred switch unchanged; partition independence at seven rates.
- **SC-005**: Cost at a constant level within 3 % (5 % on the runner); the blend's cost reported.
- **SC-006**: Coverage stays at or above 80 % and within 1 point of the baseline (0.18: 97.28 % locally).

## Assumptions

- **Alternatives considered**: a longer or shorter blend (D2: 20 ms is the engine's one transition length); a linear weight or an equal-power weight (D3: the branches are coherent, so an amplitude blend summing to 1 keeps the level; equal-power would lift the middle by up to 3 dB); completing a running blend at once on a new request (D4: a small step; rejected for smoothness); snapping level 3 to a non-whole place (rejected by spec 013 D5); an emulator option off by default (D7: the default sound would keep the tick).
- **The convention is not a device claim**: A-054 is amended to say the crossfade is the emulator's convention for a level change; capture P1 (a level change on a real unit) decides what the device does.
- **The blend's two branches read the same stored samples**: no extra look-ahead beyond the banks' 28 samples; the level-3 interpolation branch shares that look-ahead.
