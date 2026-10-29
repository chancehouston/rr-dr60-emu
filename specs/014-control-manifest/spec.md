# Feature Specification: Control Manifest

**Feature Branch**: `014-control-manifest`

**Created**: 2026-10-29

**Status**: Draft

**Input**: User description: "014 control manifest. As an app developer building on the engine, I want the engine to describe every adjustable setting and live control as constant, machine-readable data, so that the app's controls, the "inside" view's cards and the free/Workbench line all come from one cross-checked source and cannot drift from what the engine actually accepts. Each entry gives: a stable identifier; its kind, one of four (device control, unit condition, model internal, emulator option); its timing class; which path it applies to; its unit, range, step and default; the signal-chain stage it belongs to; the source and assumption IDs it traces to; and one neutral sentence describing what changing it does to the sound. Covers the 3 live controls and every construction-time setting. Owner decisions 2026-10-29: the four kinds adopted; record-side values change between recordings only for now; no AI control for now; free stays free. Must be checkable against the engine's validation, the stage manifest (spec 007), the denylist and the registers. Available to Rust and C callers. No audio change. Brief: docs/product/controls-discussion.md section 1 and section 4 item 1."

## Decisions for sign-off

The owner reads this section first. Each decision gives the recommendation; the alternative is in the Assumptions section.

| # | Decision | Recommended |
|---|---|---|
| D1 | **The engine carries the kind, not the price tier.** Each entry says which of the four kinds it is. The free/Workbench line ("device controls and unit conditions are free", decisions.md 2026-10-29) is an app rule applied to the kind, documented in the README, not a field in the MIT engine. | Kind only |
| D2 | **Seed and host rate are a fifth value, "host parameter"**, outside the four kinds: neither is something a user adjusts (the seed picks a hiss realization for reproducibility; the host rate is fixed by the app's audio session). | Fifth value |
| D3 | **The memory figures are emulator options.** The device's capacity is fixed (60 min, 99 recordings, S-001); changing it is something the device cannot do. The device value is the default. | Emulator option |
| D4 | **Timing classes are four:** live (changes while running, real-time safe), idle (applied between recordings or playbacks), idle with an empty memory (the speech codec, which fixes the stored encoding), and fixed by the host (the host rate). Record-side values are idle, per the owner's 2026-10-29 decision. | Four classes |
| D5 | **No serialized (JSON) form, no AI framing.** Structured access only, as spec 007 decided; the descriptions are written for app users and developers. | As stated |
| D6 | **The stage manifest's controls notes stay** as they are; the new manifest is cross-checked against them, so the two cannot disagree. | Keep both |

## Overview

Today an app that wants a control for each engine setting copies its range, default and meaning out of the code and the brief by hand ([controls-discussion.md](../../docs/product/controls-discussion.md) section 1 is such a copy, already dated). The copy drifts the moment a range or default moves.

This feature adds a **control manifest**: constant data in the library, one entry per adjustable setting and live control (29 today: 3 live controls and 26 construction-time settings). Each entry says what the control is, what kind it is, when it may change, what values the engine accepts, what it defaults to, which stage it belongs to, what evidence it rests on, and, in one neutral sentence, what changing it does to the sound. Tests bind the manifest to the engine's own validation and defaults, to the stage manifest and to the registers, so it cannot drift. Nothing about the sound changes.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Build the app's controls from the engine (Priority: P1)

An app developer reads the control manifest once at start-up and builds every slider, switch and picker from it: the label key, the range, the step, the default, the unit, and whether it may move while playing. They never type a range or default into the app.

**Why this priority**: it is the whole value of the feature.

**Independent Test**: read the manifest with no engine constructed and check that it lists every setting and live control, each with every field populated by the rules below, and that every string obeys the wording rules.

**Acceptance Scenarios**:

1. **Given** a program that has constructed nothing, **When** it reads the control manifest, **Then** it receives 29 entries in a stable order, each with an identifier, a name, a kind, a timing class, a path, a stage, a value type, a unit, a range, a step, a default, its source and assumption IDs and a description.
2. **Given** each entry, **When** its default is compared with the engine's defaults at every supported host rate, **Then** they are equal (the host rate's default is "set by the host").
3. **Given** each numeric entry, **When** a value at its minimum and at its maximum is offered to the engine, **Then** both are accepted, and the nearest representable values just outside are refused, by construction-time validation for settings and by the live setter for live controls.
4. **Given** the manifest, **When** it is read twice, on two platforms, or while an engine is recording or playing on another thread, **Then** every field is identical, and reading allocates nothing.

---

### User Story 2 - The control manifest cannot drift (Priority: P2)

A maintainer adds a setting, moves a range or default, or changes a stage's controls note in a later feature. If the control manifest is not updated with it, the build's tests fail and name the entry.

**Why this priority**: without the guard the manifest is one more stale copy.

**Independent Test**: the checks pass on the committed tree and fail, naming the entry, on each doctored input below.

**Acceptance Scenarios**:

1. **Given** the committed tree, **When** the checks run, **Then** they pass: every setting field and live control has exactly one entry; every range, step and default agrees with the engine; every entry's identifier appears in its stage's controls note in the stage manifest; every cited ID exists and is not superseded; every string passes the wording rules.
2. **Given** a new setting field added to the engine without an entry, **When** the library is built with its tests, **Then** it fails to compile or a test fails, naming the field.
3. **Given** a doctored entry with a range edge moved by one step, or a default changed, **When** the checks run, **Then** they fail and name the entry.
4. **Given** a doctored entry citing a superseded or nonexistent ID, or a string containing a denylisted word, **When** the checks run, **Then** they fail and name the entry and the ID or word.

---

### User Story 3 - The same from C (Priority: P3)

The iOS app reads the same manifest through the C API: the count and each entry by index, with every string valid for the life of the process and every enumerated field comparable without string comparison.

**Why this priority**: the first app reaches the engine through C.

**Independent Test**: a C program prints every entry and the output equals the Rust manifest field for field; index 29 is refused.

**Acceptance Scenarios**:

1. **Given** a C program, **When** it reads the count and every entry by index, **Then** it receives the Rust manifest's values field for field.
2. **Given** a C program, **When** it asks for an index at or past the count, **Then** it is refused with a distinct status code and nothing is written.

### Edge Cases

- **A setting with no stage.** The tap, the seed and the host rate belong to no single stage; their stage is "none" (number 0).
- **A setting on two stages.** The speech codec switch covers stages 7 and 9; the entry names the first stage (7) and the stage manifest's controls note for either stage must mention it.
- **Booleans and choices.** A switch has range off–on; a choice (VAS mode, route, tap) lists its options by stable identifier, with the default among them.
- **Integer versus continuous values.** Sensitivity and speed levels step by 1; continuous values (dBFS, ms, volume position) have step 0, meaning continuous.
- **The tap's eleven options.** Listed by their stable identifiers; the recorder and playback ignore the tap, which the description says.
- **A range that depends on another setting.** None today; the hiss offset's range applies whether or not the capsule is on (spec 012). If a dependent range appears, it is a breaking change to this feature's model and needs a spec.
- **Speed and route are kept by reset and reconfigure; volume is too.** That is a property of the live class, stated once in the documentation, not a field.

## Requirements *(mandatory)*

### Functional Requirements

**The manifest**

- **FR-001**: The library MUST provide a **control manifest**: constant data, available without constructing an engine, with one entry per construction-time setting field and per live control (29 today), in a stable order: live controls first, then the settings in the order of the settings structure.
- **FR-002**: Each entry MUST carry: an **identifier** (the setting's canonical dotted path, e.g. `vas.sensitivity`, or the live control's name, e.g. `volume`; ASCII, stable; renaming is a breaking change); a **name** (at most 40 characters); a **kind** (FR-003); a **timing class** (FR-004); a **path** (record, playback, one-pass, or a combination); a **stage number** (1–15, or 0 for none); a **value type** (switch, integer, continuous, choice); a **unit** (empty when unitless); a **minimum**, **maximum** and **step** for numeric types, or the **options** for a choice; a **default**; the **source IDs** and **assumption IDs** it traces to, ascending; and a **description** (FR-006).
- **FR-003**: **Kind** MUST be one of: **device control** (a control the owner's manual gives the user), **unit condition** (a physical state of one unit), **model internal** (a value the device has that its user cannot reach), **emulator option** (something the device cannot do), or **host parameter** (D2: set by the host for reproducibility or by its audio session). The initial kinds are the table in Key Entities.
- **FR-004**: **Timing class** MUST be one of: **live** (changes while the engine runs, real-time safe), **idle** (applied only when nothing is recording or playing), **idle, empty memory** (also needs an empty memory), or **host-fixed** (chosen at construction by the host).
- **FR-005**: The **default** MUST equal the engine's default at every supported host rate; the **minimum** and **maximum** MUST equal the inclusive bounds the engine's validation accepts; the **step** MUST be 1 for integer types and 0 for continuous types; the options of a choice MUST be exactly the values the engine accepts.
- **FR-006**: The **description** MUST be one English sentence of at most 160 characters, ending with a full stop, saying what changing the control does to the sound or to the recording. It MUST follow the claims policy and pass the stage manifest's denylist (spec 007 FR-011), and MUST NOT state an assumed figure (the figures live in the numeric fields and the registers).

**Consistency**

- **FR-007**: A test MUST fail, naming the field, when a construction-time setting field or a live control exists without an entry, or an entry exists for nothing.
- **FR-008**: Tests MUST verify FR-005 for every entry: default equality at all supported host rates; acceptance at the minimum and maximum; refusal just outside each bound; the options of each choice.
- **FR-009**: Every entry with a stage number 1–15 MUST be named in that stage's controls note in the stage manifest, and every setting or live control named in a stage's controls note MUST have an entry with that stage (the speech codec switch: stage 7 or 9).
- **FR-010**: The register cross-check (spec 007 FR-013) MUST cover the control manifest's IDs: each exists and is not superseded. It is not required that every assumption be cited here.
- **FR-011**: The traceability audit script MUST cover the control manifest's source like the stage manifest's.

**Access and scope**

- **FR-012**: The manifest MUST be readable in Rust (all entries, and a lookup by identifier that returns nothing for an unknown one) with no allocation, I/O or synchronization, identical on every platform.
- **FR-013**: The manifest MUST be readable through the C API with no Rust-only types and no allocation: the count and an entry by index; strings UTF-8, NUL-terminated, valid for the life of the process; enumerated fields comparable without string comparison; an index at or past the count refused with a distinct status code and no write. Existing C functions are unchanged; the header is regenerated.
- **FR-014**: The audio processing MUST NOT change: every golden file and SHA guard byte-identical, the settings structures unchanged.
- **FR-015**: **Documentation.** The README's user guide MUST show how to read the control manifest in Rust and C and MUST state the app rule of D1 (free = device controls and unit conditions); the CHANGELOG MUST record the feature as version 0.14.0; controls-discussion.md section 1 MUST point to the manifest as the current inventory.

### Out of Scope

- New controls, new live controls, sample-positioned changes (spec 015), meters (spec 016).
- A price tier field (D1), a serialized form (D5), localization (apps key translations on the identifier).
- Per-value explanations or presets; the app's own copy.

### Key Entities

- **Control manifest**: the ordered list of entries; constant; versioned with the library.
- **Control entry**: the fields of FR-002.

Initial kinds and timing (from controls-discussion.md section 1, with D2 – D4 applied):

| Identifier | Kind | Timing | Stage |
|---|---|---|---|
| `route` | device control | live | 11 |
| `volume` | device control | live | 11 |
| `playback_speed` | device control | live | 9 |
| `vas.enabled` | emulator option | idle | 5 |
| `vas.mode` | emulator option | idle | 5 |
| `vas.sensitivity` | device control | idle | 5 |
| `vas.threshold_dbfs`, `vas.hang_ms`, `vas.onset_ms` | model internal | idle | 5 |
| `agc.enabled` | emulator option | idle | 3 |
| `agc.knee_dbfs`, `agc.attack_ms`, `agc.release_ms` | model internal | idle | 3 |
| `mic.capsule_enabled`, `mic.amp_enabled` | emulator option | idle | 1, 2 |
| `mic.hiss_offset_db` | unit condition | idle | 1 |
| `record_stage_enabled`, `adc_clamp_enabled` | emulator option | idle | 4 |
| `speech_codec_enabled` | emulator option | idle, empty memory | 7 |
| `playback_stage_enabled` | emulator option | idle | 10 |
| `output.volume_amp_enabled` | emulator option | idle | 11 |
| `output.speaker_enabled` | emulator option | idle | 12 |
| `output.earphone_enabled` | emulator option | idle | 13 |
| `output.clamp_enabled` | emulator option | idle | 0 |
| `memory.capacity_s`, `memory.max_recordings` | emulator option | idle | 8 |
| `tap` | emulator option | idle | 0 |
| `seed`, `host_rate_hz` | host parameter | idle, host-fixed | 0 |

The stage numbers follow the stage manifest's controls notes as committed (`set_playback_speed` is in stage 9's note, `set_volume` and `set_route` in stage 11's). The stage 5 note names the VAS fields in a short form (`vas.enabled, mode, sensitivity, …`); the plan defines how FR-009 matches such forms, or rewrites the note in full paths (a manifest re-bless, no audio change).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 29 of 29 settings and live controls have an entry, and a new setting without one fails the build's tests.
- **SC-002**: For 100 % of entries, the default, bounds, step and options agree with the engine at all seven host rates.
- **SC-003**: Each of the doctored inputs of US2 (a missing entry, a moved bound, a changed default, a superseded ID, a denylisted word) is caught and named: 5 of 5.
- **SC-004**: The README examples print every entry, and the Rust and C outputs are identical.
- **SC-005**: Every golden file is byte-identical; reading the manifest allocates nothing; coverage stays at or above 80 % and within 1 point of the baseline.

## Assumptions

- **D1 alternative:** a tier field in the engine. Rejected because it puts commercial policy in the MIT engine, where a competitor's wrapper would ignore it anyway, and because the tier rule is already a function of the kind.
- **D2 alternative:** classify the seed and host rate as emulator options, or leave them out. Leaving them out would break FR-007's completeness check; calling them emulator options would put them on the app's Workbench, where they do not belong.
- **D3 alternative:** "device control" with the device value as the only valid value. Rejected because the engine accepts a wide range and the kind should describe that range.
- **The 29 count** is today's: 3 live controls and 26 settings (controls-discussion.md section 1a – 1b). A later feature that adds a setting adds an entry in the same change.
- **Spec-Kit process for this feature**, per the owner's 2026-10-29 cadence: owner sign-offs at this spec, the plan and the merge; reviews and phases otherwise proceed and stop only on a failed check or a new decision.
- No release is published; the CHANGELOG records the feature as 0.14.0, a minor version with new functions and types only.
