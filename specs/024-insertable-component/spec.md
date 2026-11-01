# Feature Specification: An Insertable Component Slot and the Band Filter

**Feature Branch**: `024-insertable-component`

**Created**: 2026-11-01

**Status**: Draft (the owner's three choices of 2026-11-01 applied: a band filter first, one insertion point before the limiter on the record side, one slot per point); the three draft reviews next, then the owner's spec sign-off.

**Input**: User description: "024 an insertable component slot and the first free component, a band filter (the owner's request 2026-11-01: make our first insertable so the engine has that feature; include one basic fx plugin for free in the initial release; other plugins later). … Decisions for sign-off D1 – D12, the measurable properties and the oracles" (the full text in the session of 2026-11-01).

## Decisions for sign-off

The owner reads this section first. Each decision gives the recommendation; the alternatives are in the Assumptions section. The owner's three choices (the first component, the insertion point, one slot) are built in.

| # | Decision | Recommended |
|---|---|---|
| D1 | **One published insertion point, `before_limiter`.** The engine lists its insertion points as constant data (identifier, name, the path, a one-sentence description of where the point sits: "after the microphone amplifier, before the limiter, at the host rate"); one entry in 1.0. The slot is **empty by default**, and an empty slot is bit-identical to 0.23: every golden guard and every smoke hash unmoved. An **emulator convenience, not a device behavior**: no assumption, no stage-manifest row; the stage manifest's header gains one line, "insertion points: 1, components: 1". | **One point, empty by default** |
| D2 | **The component kinds are the engine's own, a closed set.** No third-party or scripted code. A **component manifest** as constant data: the kind's identifier, name, a one-sentence description of what it does to the sound, its parameter list (identifier, name, range, unit, default) and a cost class. One kind in 1.0: `band_filter`. | **A closed set with a manifest** |
| D3 | **The slot's kind is set while idle; its parameters are live.** `set_component(point, kind or none)` is refused while recording or playing and is kept by `reset` and `reconfigure` like the live controls; every kind's state is reserved at construction, so setting a kind never allocates. The parameters are **live on the record stream and the one-pass in the input clock**, scheduled like the record-side values of spec 017 (through the schedule by the control manifest's index, no plain setter), landing at input sample P with a **20 ms smoothstep transition of the parameter value** (the frequency in log-frequency, Q in log, the gain in dB), the filter's coefficients recomputed for every host sample of the transition. The kind and the parameters are **not settings**: the settings structure, its size and the ABI version stay unchanged (the 0.22 promise exercised again). | **Kind idle, parameters live, no settings change** |
| D4 | **The band filter**: one second-order section (a bilinear-transform biquad with prewarping) with a **type** (`low_pass`, `high_pass`, `band_pass`, `notch`), a **frequency** 20 – 20 000 Hz (clamped to 0.45 × the host rate at construction and at a landing; default 1000 Hz), a **Q** 0.5 – 10 (default 0.7071) and a **gain** −24 … +24 dB applied after the section (default 0), computed in double precision and narrowed where the limiter reads it. Its response equals the design formula within 0.1 dB and 2° from 20 Hz to 0.45 × the rate at the seven rates; the DC and Nyquist gains are exact by type; the transition's parameter trajectory equals the smoothstep within 1e-9. The defaults (low-pass, 1 kHz, 0.7071, 0 dB) are the state a slot holds the moment the kind is set. | **A single biquad with four parameters** |
| D5 | **The control manifest appends five entries**, no existing index moving: `insert.before_limiter` (the slot's kind, a choice `none` / `band_filter`, kind **emulator option**, timing **idle**, path record, stage 0), then `band_filter.type`, `band_filter.frequency_hz`, `band_filter.q`, `band_filter.gain_db` (emulator options, timing **live**, path record, stage 0, the input clock). `MAX_CHANGES_PER_CALL` grows by the four new live entries on the record streams (the derivation restated: 2 · 64 + 3 + 8 + 4 + 3). The stage manifest is unchanged but for the header line. | **Five entries appended** |
| D6 | **A tap after the slot**: `AfterInsert` appended (the slot's output at the host rate before the limiter; with the slot empty it equals `AfterMicAmp`); `AfterMicAmp` unchanged. The meters gain nothing: the limiter-input peak already reads after the slot, documented as such. | **One tap appended** |
| D7 | **The processing record carries the component**: the slot's kind and the four parameters as they were at the recording's start (in the slot block) and every parameter landing as a change entry with its clock, so a re-render reproduces the stored form's digest with a component in the slot; the stored form itself is unchanged (the component's output is what is stored); the reader flags an unknown kind. | **In the record, not in the stored form** |
| D8 | **Cost.** The empty slot within **1 %** of 0.23's merge (d6cf258) on the record path and the one-pass (solo; the runner's ceiling 3 %); the band filter's cost reported (one biquad per host sample: a few nanoseconds); a transition's per-sample recompute bounded and reported; the worst 64-sample block with a transition running within real time at 96 kHz. | **1 % empty, the filter reported** |
| D9 | **C.** `rr_dr60_set_component` / `rr_dr60_get_component` (a `uint32_t` point and kind: `RR_DR60_INSERTION_POINT_BEFORE_LIMITER` 0; `RR_DR60_COMPONENT_NONE` 0, `RR_DR60_COMPONENT_BAND_FILTER` 1), `rr_dr60_insertion_point_count` / `rr_dr60_insertion_point(index, out)`, `rr_dr60_component_count` / `rr_dr60_component(kind, out)` / `rr_dr60_component_parameter(kind, index, out)` as sized structs under spec 022's rule, statuses `NO_SUCH_INSERTION_POINT`, `NO_SUCH_COMPONENT`, `COMPONENT_WHILE_ACTIVE`; no change to any existing struct; the 0.22 promise exercised with 0.23's merge too (`scripts/abi-promise.sh` with d6cf258 beside 12a3c45 in CI); the Swift package's re-render carries the component. | **Appended only** |
| D10 | **One new golden** (`golden-insert-v1.json`: the four types × three parameter sets × seven rates on the voiced stimulus and a tone, codec on and bypassed, the record path and the one-pass, plus schedules with parameter transitions, blessed once with the empty-slot twin identities asserted at bless time); the control manifest re-blessed once (the five entries); the stage manifest's header re-blessed once; version 0.24.0. | **As stated** |
| D11 | **Free.** The slot and the band filter ship free in 1.0 (the owner, 2026-11-01); later components may come as paid packages (decisions.md 2026-10-15). The band filter's app card names it as an addition of the app, not a part of the original recorder. | **Free** |
| D12 | **Records**: the guide's new "Insertable components" section (the point, the kinds, the live parameters and their transition, the record, the C names, the claims wording), the README, the CHANGELOG under Added, decisions.md, brief.md 277 – 279, 1.0-feature-set.md 192 and § 5, the designer's UI brief (the workbench's "add component" is one slot before the limiter in 1.0), controls-discussion.md, CLAUDE.md. | **As stated** |

## Clarifications

### Session 2026-11-01 (the owner's choices before the draft)

- The first component is a **band filter** (a reverb and a three-band equalizer were the alternatives); the only insertion point in 1.0 is **before the limiter on the record side** (before the voice gate and before the output chain were offered); **one slot per point** (an ordered chain was the alternative).

## Overview

The engine's signal chain is fixed: fifteen stages in the device's order, each one modeled on the recorder's documentation or registered as an assumption, bypassable but never inserted, moved or removed (brief.md 277). The product direction since 2026-10-15 is that paid expansion adds components to the back-panel workbench, and that the places a component can go must come from the engine, not the app (brief.md 75). This feature builds the mechanism and ships its first component free: a **slot** at one published **insertion point**, before the limiter on the record side, that can hold one of the engine's own **components**, and a **band filter** as the first component. A component in the record path changes what the recorder hears and therefore what it stores; the processing record carries the component and its parameter changes so a session can be reproduced. Every description says what the component is: an addition of the app, not a part of the original recorder; it describes what the filter does to a tone, never what a listener might find.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A filter in the chain (Priority: P1)

An app user opens the workbench, puts the band filter into the slot before the limiter, hears the recorder record through it, adjusts its frequency while recording, and takes it out again.

**Independent Test**: set the kind, record a tone and the voiced stimulus with each filter type, read the response against the design formula, schedule a frequency change and read the transition, set the kind back to none and compare with a run that never had it.

**Acceptance Scenarios**:

1. **Given** an idle engine, **When** the band filter is set into `before_limiter` and a sweep is recorded, **Then** the stored recording's response (read at the tap after the slot) equals the design formula of the type, frequency, Q and gain within 0.1 dB and 2° from 20 Hz to 0.45 × the rate at every rate, and the DC and Nyquist gains are exact by type.
2. **Given** a recording in progress with the filter in the slot, **When** a frequency change is scheduled at input sample P, **Then** the parameter follows a 20 ms smoothstep from P in log-frequency (within 1e-9 of the formula on the trace), the output never steps (the click reader reads the transition at least 30 dB below an instant parameter step on a tone), and the entry is logged at P with its tag.
3. **Given** the slot set to none, **When** any stimulus is recorded or processed in one pass, **Then** the output is bit-identical to 0.23's at every rate and every configuration (the sixteen golden guards and every smoke hash unmoved; a twin run bit-identical).
4. **Given** a recording or a playback in progress, **When** `set_component` is called, **Then** it is refused and nothing changes; **given** an idle engine, the kind is set without allocating, is kept by `reset` and `reconfigure`, and is read back.
5. **Given** the filter with the gain at +24 dB on a loud stimulus, **When** recorded, **Then** the limiter and the clamp bound the result as they do for any loud input (no new overflow path; the limiter-input peak meter reads the slot's output).

---

### User Story 2 - The engine describes the slot (Priority: P2)

An app developer builds the workbench's "add component" from the engine's data and reproduces a session.

**Acceptance Scenarios**:

1. **Given** the insertion-point list, **When** read, **Then** it holds one entry, `before_limiter`, with its name, path and placement sentence; **given** the component manifest, it holds one kind, `band_filter`, with its four parameters' identifiers, names, ranges, units and defaults, all agreeing with the engine's validation and defaults and with the control manifest's entries.
2. **Given** the control manifest, **When** read, **Then** the five appended entries sit at the end with the stated kinds and timings; `MAX_CHANGES_PER_CALL` is the restated derivation; the registers cross-check passes after one re-bless.
3. **Given** a recording made with the filter and a scheduled parameter change, **When** its processing record is read and re-rendered, **Then** the digest is reproduced; **given** the record with the kind changed to none or a parameter moved, the re-render differs; **given** a record naming an unknown kind, the reader flags it and refuses the re-render.
4. **Given** the C API, **When** the parity rows are run on a square-wave stimulus, **Then** the Rust and C paths agree bit for bit with the filter in the slot and across a scheduled change; the refusals return their statuses; `scripts/abi-promise.sh` passes with 12a3c45 and with d6cf258.
5. **Given** the four parameters, **When** scheduled on the playback stream, **Then** they are refused (record-side values); **given** a value outside a range, it is refused with the setting's error family.

---

### User Story 3 - Nothing else moved (Priority: P3)

1. **Given** the A/B against d6cf258, **When** run solo and on the runner, **Then** the record path and the one-pass with the slot empty are within 1 % (3 % on the runner); the filter's cost and a transition's worst block are reported, the block within real time at 96 kHz.
2. **Given** the partition identities, **When** a recording with scheduled parameter changes is run at block sizes 1, 64, 4096 and irregular, **Then** the outputs and logs are identical.
3. **Given** the determinism and allocation-free suites, **When** extended by the slot, **Then** they pass.

### Edge Cases

- A parameter change scheduled while the slot is empty: logged as an entry (the value lands in the slot's parameter store) and applied the next time the kind is set; a kind set later starts from the stored parameters, not the defaults.
- A frequency above 0.45 × the host rate: clamped at construction and at a landing, the clamp reported by the control's effective range for the rate.
- A transition interrupted by a second landing within 20 ms: restarts from the parameter's current value (spec 011's rule).
- A kind set to the kind already in the slot: no change, no entry.
- `reconfigure` to another host rate: the slot's kind and parameters kept, the coefficients rebuilt for the new rate, the frequency re-clamped.
- The one-pass and the recorder share the slot's parameters as spec 017's values are shared (a landing on one stream becomes pending for the other).
- The notch at a frequency that a stored tone sits on: the tone attenuated by the design's depth, reported.
- A component in the slot during the stop's flush: the flush runs through it like the input does.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The engine MUST publish its insertion points and its component kinds as constant data (D1, D2), one point and one kind in 1.0, each entry with the fields of D1 and D2.
- **FR-002**: The slot MUST be empty by default, and an empty slot MUST leave every output bit-identical to 0.23 (the sixteen golden guards and every smoke hash unmoved).
- **FR-003**: The slot's kind MUST be settable only while the engine is idle, refused otherwise with a dedicated error, kept by `reset` and `reconfigure`, readable back, and MUST never allocate (D3).
- **FR-004**: The band filter MUST implement D4: four types, the three continuous parameters with their ranges and defaults, the response within 0.1 dB and 2° of the design formula at the seven rates, exact DC and Nyquist gains by type.
- **FR-005**: The four parameters MUST be live on the record stream and the one-pass in the input clock through the schedule only, landing at P with the 20 ms smoothstep of D3, every landing a change entry; the playback stream MUST refuse them; out-of-range values MUST be refused.
- **FR-006**: The control manifest MUST append the five entries of D5 with no existing index moving; `MAX_CHANGES_PER_CALL` MUST follow the restated derivation; the stage manifest's header MUST carry the count line.
- **FR-007**: `Tap::AfterInsert` MUST be appended and MUST equal `AfterMicAmp` with the slot empty.
- **FR-008**: The processing record MUST carry the slot's kind and parameters at the start and every landing, the re-render MUST reproduce the digest with a component in the slot, and an unknown kind MUST be flagged and refused (D7).
- **FR-009**: The C API MUST gain the functions, defines and statuses of D9 with no change to any existing struct; the promise script MUST pass with both 12a3c45 and d6cf258 in CI; the Swift re-render MUST carry the component.
- **FR-010**: The empty slot's cost MUST be within 1 % of d6cf258 on the record path and the one-pass (3 % on the runner); the filter's cost and the worst block MUST be reported, the block within real time at 96 kHz.
- **FR-011**: Every text describing the slot or the filter MUST call it an addition of the app and describe what it does to a tone (D11); the wording denylist applies.

### Out of Scope

- An ordered chain per point; insertion points on the playback side or before the voice gate (later specs: the point list is the extension seam).
- Third-party or scripted components; reverb, equalization, flanging (later components on the same mechanism).
- A component inside the memory or the stored form; a component on the playback path.

### Key Entities

- **Insertion point**: a published place in the chain where one slot sits (`before_limiter`).
- **Slot**: holds zero or one component; its kind set while idle; its parameters live.
- **Component kind**: an engine-owned processor with a manifest entry (`band_filter`).
- **Component manifest**: constant data describing every kind and its parameters.
- **Parameter transition**: the 20 ms smoothstep a landing follows, in the parameter's own scale.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: At seven rates and the four types, the filter's response within 0.1 dB and 2° of the design formula; the DC and Nyquist gains exact by type.
- **SC-002**: A scheduled parameter change lands at P, follows the smoothstep within 1e-9 on the trace, reads at least 30 dB below an instant step on the click reader, and is logged; the partition identities hold at four block sizes.
- **SC-003**: The sixteen golden guards and every smoke hash unmoved; the empty-slot twin bit-identical at every rate and configuration.
- **SC-004**: The re-render identity holds with a component in the slot in spec 020's cells; the named deltas (a parameter moved, the kind changed) fail it; an unknown kind is flagged.
- **SC-005**: The A/B against d6cf258 within 1 % (3 % on the runner); coverage within 1 point of 0.23's.
- **SC-006**: The control and component manifests agree with the engine both ways; the registers cross-check and the promise script green; every description passes the wording scan.

## Assumptions

- **Alternatives to D1**: several points at once (before the voice gate, before the output chain, before the limiter); rejected for 1.0 to keep one contract, with the list as the seam.
- **Alternatives to D2**: an open plug-in interface (rejected: no third-party code in the core, determinism and real-time safety could not be guaranteed).
- **Alternatives to D3**: the kind as a setting (rejected: it would grow the settings struct and move the ABI version under spec 022 D13); the kind live (rejected: a kind change mid-recording would need its own transition and a record rule); a plain setter for the parameters (rejected: spec 017's schedule-only rule for record-side values).
- **Alternatives to D4**: a one-pole filter (cheaper, no Q), a fourth-order section (more selective, a design of its own); a gain before the section (rejected: a post-gain keeps the section's numeric range).
- **Alternatives to the transition**: an instant coefficient step (a click on content near the cutoff; measured as the reference); coefficient interpolation instead of parameter interpolation (rejected: an unstable intermediate is possible for a biquad).
- **Alternatives to D11**: the filter in a paid package (rejected by the owner: one basic effect ships free).
