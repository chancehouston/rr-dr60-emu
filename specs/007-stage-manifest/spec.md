# Feature Specification: Stage Manifest

**Feature Branch**: `007-stage-manifest`

**Created**: 2026-10-12

**Status**: Draft

**Input**: User description: "Stage manifest: the library exports a static, read-at-start-up description of the RR-DR60 signal chain it models, so an app's "about this model" or "inside" view is generated from the engine and cannot drift from the project's hardware registers (product request E-D, docs/product/front-and-inside.md, decided 2026-10-11). For each of the 15 stages in docs/hardware/signal-chain.md (mic capsule, mic amp, AGC, codec anti-alias filter + ADC + µ-law, VAS gate, noise reduction, speech encoder, flash storage, speech decoder, codec DAC + reconstruction filter, volume / power amp, speaker, earphone out, clock, power) the manifest gives: a stable numeric ID and a short machine identifier; the stage's name; a one-sentence neutral English description of what the stage does to sound; its modeling status, one of sourced (modeled on a document), assumed (modeled on a registered assumption), not modeled (passes through or is a label), or checked against a real unit (none today); the source IDs (S-###) and assumption IDs (A-###) it traces to; a confidence level; which signal path it belongs to (record, playback, or both/none) and which engine tap or setting, if any, exposes it; and the library version that last changed its model. The manifest also carries a header: the library version, the device rate, a count of stages by status (so an app can say "0 of 15 stages checked against a real unit"), and a neutral one-line summary of the modeling basis ("modeled on the service manual and codec datasheets"). All strings are short, neutral, English, and follow the claims policy (no "verified", "detects", "activity", "event" wording); apps localize on their side. The manifest is constant data compiled into the library: no allocation, no I/O, no real-time call; readable from Rust and from the C API with no Rust-only types, with each string valid for the life of the process. It must be kept consistent with the registers by a test that cross-checks every stage's IDs against docs/hardware/assumptions.md and docs/hardware/sources.md and against the signal-chain table, so a register change without a manifest change fails the build's tests, and the traceability audit script covers it. Out of scope: per-stage live meters (E-A), scope taps (E-B), limiter events (E-E), any change to the audio processing or golden files, localization, and app UI."

## Overview

The project's honesty rests on two registers: the sources ([docs/hardware/sources.md](../../docs/hardware/sources.md), `S-###`) and the assumptions ([docs/hardware/assumptions.md](../../docs/hardware/assumptions.md), `A-###`), tied to the fifteen stages of the [signal chain](../../docs/hardware/signal-chain.md). Today an app that wants to tell its users what the emulator rests on has to copy that information out of the Markdown by hand, and the copy goes stale the moment a feature adopts, revises or supersedes an assumption. The app built on this engine wants to generate its "inside" view and its status header from the engine itself ("Modeled on the service manual and codec datasheets. 0 of 15 stages checked against a real unit."; [docs/product/front-and-inside.md](../../docs/product/front-and-inside.md), product request E-D, decided 2026-10-11), and every other library customer has the same need.

This feature adds a **stage manifest** to the library: a constant, read-once description of the fifteen stages, one entry per stage, plus a header that summarizes the whole. Each entry says what the stage does to sound, whether and on what basis the engine models it, which source and assumption IDs it traces to, how confident the project is, which path it sits on, which engine controls touch it, and which library version last changed its model. The manifest changes nothing about the sound: the three golden files stay byte-identical.

What makes the manifest trustworthy is that it **cannot drift**. A test cross-checks every entry against the two registers and the signal-chain table, so a register change that is not carried into the manifest fails the build's tests, and the manifest can never cite an ID that does not exist or that has been superseded. Status changes are user-visible (constitution Principle II) and are recorded in the CHANGELOG like any other assumption change.

## Clarifications

### Session 2026-10-11 (product consult, before specification)

- Q: May the engine carry user-facing strings? → A: **Yes, short neutral English strings**; apps localize on their side, keyed on the stage identifiers (front-and-inside.md, decided 2026-10-11).
- Q: Is this a real-time call? → A: **No.** The manifest is read at start-up; it is constant data and never changes during a run.
- Q: Which tier? → A: **Free**; it benefits every library customer.
- Q: Does the status header count stages or features? → A: **Stages**, fifteen of them, numbered as in the signal-chain table.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Generate the "about this model" view from the engine (Priority: P1)

An app developer wants an "about this model" or "inside" view that lists every stage of the emulated recorder with its name, what it does to sound, whether it is modeled and on what basis, the source and assumption IDs behind it, the project's confidence, and the engine version that last changed it, topped by a header such as "Modeled on the service manual and codec datasheets. 0 of 15 stages checked against a real unit." They read the manifest once at start-up and render it; they never type a status or an ID into the app.

**Why this priority**: this is the product request itself (E-D) and the whole value of the feature: a status view that comes from the engine. Everything else guards or extends it.

**Independent Test**: can be fully tested by reading the manifest with no engine constructed and no audio processed, and checking that it has exactly fifteen entries numbered 1 to 15 in signal-chain order, that every field of every entry is populated according to the rules below, that the header's counts agree with the entries, and that every string obeys the wording rules; it delivers a complete, renderable status list.

**Acceptance Scenarios**:

1. **Given** a program that has linked the library and constructed nothing, **When** it reads the manifest, **Then** it receives a header and fifteen stage entries, numbered 1 to 15 in the order of the signal-chain table, each with a non-empty identifier, name and description, a status, a path, a confidence, its source and assumption IDs, its controls note and its model version.
2. **Given** the manifest at this version, **When** the header's counts are read, **Then** they say: sourced 0, assumed 5, not modeled 10, checked against a real unit 0, summing to 15, and the header's library version equals the library's own version and its device rate is 6000 Hz (A-026).
3. **Given** the manifest, **When** it is read twice, or on two different platforms, or after any amount of audio has been processed, **Then** every field is identical every time.
4. **Given** the stage entries, **When** their descriptions are checked, **Then** each is one English sentence that says what the stage does to sound, ends with a full stop, is at most 160 characters, and contains none of the words the claims policy avoids (FR-011).
5. **Given** the stage entries, **When** their statuses are compared with the registers, **Then** every stage whose model rests on at least one current assumption reads "assumed", no stage reads "sourced" or "checked against a real unit" today, and every stage the engine passes through or represents only as a label reads "not modeled".

---

### User Story 2 - The manifest cannot drift from the registers (Priority: P2)

A maintainer adopts, revises or supersedes an assumption in a later feature, or adds a source. If they forget to carry the change into the manifest, the build's tests fail and name the stage and the ID, so the apps' status views never show a stale or superseded basis. The traceability audit also covers the manifest.

**Why this priority**: without this guard the manifest is just another copy of the registers that goes stale; with it, the manifest is the one copy that is checked. It is what the product note means by "cannot drift".

**Independent Test**: can be fully tested with the registers and the signal-chain table as they are, plus doctored copies of them, without any engine or audio: the cross-check passes on the real files and fails, naming the discrepancy, on each doctored copy (an ID removed, an ID superseded, a new assumption not cited, a stage renamed, a stage row added or removed).

**Acceptance Scenarios**:

1. **Given** the registers and the signal-chain table as committed, **When** the cross-check runs, **Then** it passes: every ID the manifest cites exists in its register and is not superseded; every current (not superseded) assumption and every source is cited by at least one stage; the fifteen stage numbers and names match the signal-chain table's rows.
2. **Given** a copy of the assumption register with one current assumption's status changed to superseded, **When** the cross-check runs against it, **Then** it fails and names the stage entry that still cites the ID.
3. **Given** a copy of the assumption register with a new assumption row (`A-040`) added, **When** the cross-check runs against it, **Then** it fails and names the uncited ID.
4. **Given** a copy of the signal-chain table with a stage renamed, **When** the cross-check runs against it, **Then** it fails and names the stage number whose name differs.
5. **Given** a stage entry whose confidence is higher than the lowest headline confidence among the current assumptions it cites, **When** the cross-check runs, **Then** it fails and names the stage.
6. **Given** the traceability audit script, **When** it runs, **Then** it also checks that every `S-###` and `A-###` written in the manifest's source exists in the registers, and passes on the committed tree.

---

### User Story 3 - The same from C (Priority: P3)

The iOS app, through its C bridge, reads the same manifest: the header, the stage count, and each stage by its number, with every string valid for the life of the process so it can be kept by reference, and with statuses, paths and confidence levels as named constants rather than strings to compare.

**Why this priority**: the first app is an iOS app consuming the C API; without this story the manifest exists but the product cannot use it. It is ordered after the drift guard because the content must be right before it is exposed twice.

**Independent Test**: can be fully tested by a C program that reads the header and all fifteen stages through the C API and prints them, compared field by field with the Rust manifest, plus the out-of-range refusals; it delivers the same status list to a C or Swift host.

**Acceptance Scenarios**:

1. **Given** a C program linked against the library, **When** it asks for the stage count, the header and each stage by number 1 to 15, **Then** it receives the same values as the Rust manifest, field for field, with the ID lists enumerable one ID at a time.
2. **Given** a C program, **When** it asks for stage 0 or stage 16, **Then** it is refused with a distinct status code and nothing is written to its output.
3. **Given** a C program that keeps a pointer to a manifest string, **When** it uses that pointer at any later time, including after engines have been created and destroyed, **Then** the string is still valid and unchanged.
4. **Given** the manifest's strings, **When** read through the C API, **Then** they are UTF-8, terminated the way C expects, and byte-identical to the Rust strings.

---

### Edge Cases

- **A stage that is modeled in part.** Stage 4 (codec input) models the band-limit, the 6 kHz rate and the full-scale clamp but not µ-law quantization or idle noise; stage 8 (memory) models capacity and numbering but stores the engine's own samples. The status reflects the model of the stage as a whole: it is "assumed" as soon as any modeled behavior rests on a current assumption, and the description describes the device's stage, not the model's coverage. The register IDs and the signal-chain table carry the detail; the manifest does not carry a per-stage coverage note (see Assumptions).
- **A stage with no current assumption but a source.** No such stage exists today (every stage cites at least one current assumption). If one arises, its status is "sourced" and its confidence "high"; the cross-check allows it.
- **A stage on both paths.** The memory (8), the clock (14) and the power supply (15) belong to both paths; the path field says "both". No stage is on neither path.
- **A stage with no engine control.** Stages 1, 2, 6, 7, 9, 14 and 15 have no setting and no tap; the controls note is empty, not "none".
- **A stage exposed only as a label.** Stages 11, 12 and 13 are reached only through the route selection and the mute-while-recording report of spec 006; their controls notes say so and their status is "not modeled".
- **Superseded assumptions.** A-001, A-002, A-014, A-017 and A-018 are superseded and must not be cited by any stage; the cross-check fails if they are.
- **A register row whose stage wording differs from the table's.** The registers' "Stage" column is free text ("ADC/DAC", "Output", "Memory / transport"). The cross-check does not match stage names against that column; it checks existence, supersession, complete citation and the confidence bound. Stage names are checked against the signal-chain table only.
- **The library version in the header.** It is the library's own version, so it moves with every release without a manifest edit; the per-stage model versions move only when a stage's model changes.
- **Reading before, during and after audio processing.** The manifest is constant data; reading it allocates nothing, performs no I/O and is allowed from any thread at any time, including while an engine is recording or playing on another thread.
- **An out-of-range stage number.** Stage 0 and stages above 15 are refused; in Rust the lookup returns nothing, in C a distinct status code; the indexed form of the fifteen entries never fails.
- **Non-ASCII characters.** Names and descriptions may contain the characters the registers use (µ, Ω, ≈, en dashes); identifiers are ASCII only.
- **Localization.** The strings are English; an app that localizes keys its translations on the stage identifier, which is stable, never on the English text, which may change in a minor version.

## Requirements *(mandatory)*

### Functional Requirements

**The manifest**

- **FR-001**: The library MUST provide a **stage manifest**: constant data, available without constructing an engine, consisting of a **header** and exactly **fifteen stage entries** numbered 1 to 15 in the order of the signal-chain table ([docs/hardware/signal-chain.md](../../docs/hardware/signal-chain.md)), one per table row.
- **FR-002**: Each **stage entry** MUST carry: a **number** (1–15, stable for the life of the library; renumbering is a breaking change); an **identifier** (ASCII lowercase letters, digits and underscores, at most 24 characters, unique, stable; renaming is a breaking change); a **name** (at most 48 characters); a **description** (one English sentence of at most 160 characters, ending with a full stop, that says what the stage does to sound on the device); a **status** (FR-004); a **path** (record, playback or both); a **confidence** (FR-006); the **source IDs** and **assumption IDs** it traces to (FR-005), each list in ascending ID order; a **controls note** (FR-007); and a **model version** (FR-008).
- **FR-003**: The **header** MUST carry: the **library version** (equal to the library's own version string); the **device rate** in hertz (6000, A-026); the **stage count** (15); **counts of stages by status** (one per status of FR-004, summing to the stage count); a **basis sentence** (one neutral English sentence of at most 160 characters stating what the model rests on and that no stage has been checked against a real unit while that is true); and the **evidence link** (the address of the project's hardware-evidence guidance, so an app can offer "Own one? Help check it").
- **FR-004**: **Status** MUST be exactly one of four values, with these meanings and this rule: **checked against a real unit**: at least one measurement of a real RR-DR60 has confirmed the stage's model within its stated tolerance (applies to no stage today); **sourced**: the engine models the stage and every modeled figure traces to a source with no current assumption open (applies to no stage today); **assumed**: the engine models the stage and at least one modeled behavior or figure rests on a current (not superseded) assumption; **not modeled**: the engine does not change the signal at this stage, or represents it only as a label or a pass-through. A stage is "modeled" when the engine changes the signal there, gates it, stores it or limits it according to a registered description (stages 3, 4, 5, 8 and 10 today).
- **FR-005**: **IDs.** Each stage MUST cite every current source and assumption that its row in the signal-chain table or its model rests on, and MUST NOT cite a superseded assumption or an ID that does not exist. Across all fifteen entries, every current assumption and every source in the registers MUST be cited by at least one stage, so that nothing in the registers is orphaned from the manifest. The initial citations are the table in Key Entities.
- **FR-006**: **Confidence** MUST be one of **high**, **medium**, **low** or **not applicable**, where "not applicable" is used exactly for stages whose status is "not modeled". For a modeled stage the stated confidence MUST NOT exceed the lowest **headline confidence** (the first level named in the register's Confidence column) among the current assumptions the stage cites; a stage with no current assumption is "high".
- **FR-007**: The **controls note** MUST name, in the engine's public vocabulary, the settings, taps, state reads and reports through which a host can configure, bypass or observe the stage, and MUST be empty for a stage the engine offers no control or read-out for. It is a short English string (at most 120 characters), not a machine-readable list.
- **FR-008**: The **model version** MUST be the library version in which the stage's modeled behavior last changed (3: 0.5.0; 4: 0.5.0; 5: 0.5.0; 8: 0.6.0; 10: 0.4.0), and MUST be empty for a stage that is not modeled. Adding the manifest itself does not count as a model change.

**Honesty and wording**

- **FR-009**: Every string in the manifest MUST follow the claims policy: it describes only what the stage does to the signal, it says "modeled on" or names the assumption rather than claiming accuracy, and it makes no claim, for or against, about what recordings contain.
- **FR-010**: A description MUST contain a numeric figure only when that figure is **sourced** (traceable to an `S-###` row); assumed figures stay in the registers so the strings do not move when an assumption is revised.
- **FR-011**: No manifest string MAY contain, as whole words in any letter case, the words or phrases the product note avoids: "verified", "verify", "detect", "detects", "detection", "senses", "sensing", "listening for", "signal present", "anomaly", "anomalous", "spike", "activity", "event", nor the words "paranormal", "EVP", "ghost", "spirit", "entity", "voice from" or "communication". "Checked against a real unit" is the phrase for the strongest status. A test MUST enforce the list, which lives in one place so the product team can extend it.
- **FR-012**: The header's basis sentence MUST read, while no stage is checked: "Modeled on the RR-DR60 service manual and the codec family datasheets; no stage has been checked against a real unit." When the first stage becomes checked, the sentence changes with it and the change is a CHANGELOG entry.

**Consistency with the registers**

- **FR-013**: A **cross-check test** MUST read the committed [sources.md](../../docs/hardware/sources.md), [assumptions.md](../../docs/hardware/assumptions.md) and [signal-chain.md](../../docs/hardware/signal-chain.md) and verify, failing with the stage number and ID named: (a) every cited ID exists in its register; (b) no cited assumption is superseded; (c) every current assumption and every source is cited by at least one stage (FR-005); (d) the fifteen stage numbers and names equal the table's rows, and the table has exactly fifteen rows; (e) each modeled stage's confidence respects FR-006; (f) the header's counts equal the tally of the entries' statuses, and the stage count is 15; (g) every stage with status "not modeled" has an empty model version and confidence "not applicable", and every modeled stage has a non-empty model version that is a released or the current version; (h) every string passes FR-002's length and form rules and FR-011's word list. The test MUST also be shown to fail on doctored copies of each file (US2 scenarios 2–5), so that the guard itself is tested.
- **FR-014**: The **traceability audit script** MUST additionally verify that every `S-###` and `A-###` written in the manifest's source exists in the registers, and MUST pass on the committed tree.
- **FR-015**: A change to any stage's status, IDs, confidence or model version MUST be recorded in the CHANGELOG under the version that makes it, as Principle II requires for assumption changes; the README's fidelity-status table MUST state that the manifest is the canonical per-stage status and MUST agree with it at every release.

**Access**

- **FR-016**: The manifest MUST be readable in Rust: the whole manifest at once, the header, all fifteen entries in order, and a lookup by stage number that returns nothing for 0 or above 15. Reading MUST NOT allocate, perform I/O, take a lock or depend on any engine, and the data MUST be identical on every supported platform and at every moment of a process's life.
- **FR-017**: The manifest MUST be readable through the **C API** with no Rust-only types: the stage count, the header, and a stage by number, each copied into a caller-provided plain structure; strings as UTF-8, NUL-terminated, valid for the life of the process; status, path and confidence as named integer constants; the source and assumption ID lists enumerable one ID at a time by a C caller. Stage numbers 0 and above 15 MUST be refused with a distinct status code and no write. The C header MUST be regenerated and the existing C functions MUST be unchanged.
- **FR-018**: The audio processing MUST NOT change: the three golden files and their SHA guards stay byte-identical, the settings structures are unchanged, and the engine's timing figures are unaffected.
- **FR-019**: **Documentation.** The README MUST show how to read the manifest in Rust and in C and MUST name the stage identifiers as the keys for localization; the CHANGELOG MUST record the feature as version 0.7.0; the signal-chain page MUST note that the manifest mirrors its table and is checked against it.

### Out of Scope

- Per-stage live meters (E-A), a scope copy at a stage (E-B) and limiter ceiling events (E-E).
- Any change to the audio processing, the settings or the golden files.
- Localization of the strings; the engine carries English only.
- App UI, the "inside" rendering, and the app's own copy.
- The text of the assumptions and sources themselves (titles, rationales, verification methods): the manifest carries IDs, and hosts link to the registers for the detail.
- A machine-readable list of controls per stage (the controls note is prose; a structured form can follow E-A if the app needs it).

### Key Entities

- **Manifest**: the header plus the fifteen stage entries; constant; versioned with the library.
- **Header**: library version, device rate, stage count, counts by status, basis sentence, evidence link.
- **Stage entry**: number, identifier, name, description, status, path, confidence, source IDs, assumption IDs, controls note, model version.
- **Status**: checked against a real unit, sourced, assumed, not modeled (FR-004).
- **Path**: record, playback, both.
- **Confidence**: high, medium, low, not applicable (FR-006).

The initial content, as of this version (the descriptions are the first draft and are subject to the technical and claims reviews; the IDs, statuses, paths, confidences and model versions follow the rules above applied to the registers as committed):

| # | Identifier | Name | Path | Status | Sources | Assumptions | Confidence | Model version | Description |
|---|---|---|---|---|---|---|---|---|---|
| 1 | `mic_capsule` | Microphone capsule | record | not modeled | S-002 | A-005 | not applicable | | Turns sound pressure into an electrical signal with a small omnidirectional electret capsule. |
| 2 | `mic_amp` | Microphone amplifier | record | not modeled | S-002 | A-006, A-029 | not applicable | | Amplifies the microphone signal ahead of the codec and carries the limiter's attenuator. |
| 3 | `limiter` | Limiter (AGC) | record | assumed | S-001, S-002 | A-007, A-019, A-020, A-028, A-034, A-035, A-036 | low | 0.5.0 | Holds loud sounds just below the codec's full scale after a brief clipped onset, then lets the level recover over about a second. |
| 4 | `codec_input` | Codec input: filter, ADC and µ-law | record | assumed | S-002, S-003, S-005, S-007 | A-003, A-015, A-016, A-026, A-027, A-030 | medium | 0.5.0 | Narrows the signal to the codec's voice band, samples it at 6 kHz as 8-bit µ-law and clips anything beyond full scale. |
| 5 | `vas` | Voice-activated pause (VAS) | record | assumed | S-001, S-002 | A-008, A-021, A-022, A-023, A-024, A-025, A-033 | low | 0.5.0 | Pauses the recording while the input stays quiet and resumes when sound returns, leaving the quiet gaps out. |
| 6 | `noise_reduction` | Noise reduction | record | not modeled | S-004 | A-009 | not applicable | | Reduces background noise according to one secondary account; the service manual describes no such stage. |
| 7 | `encoder` | Speech encoder (CELP, 4 kbit/s) | record | not modeled | S-002, S-004 | A-004 | not applicable | | Compresses the voice-band stream into 4 kbit/s CELP frames for storage. |
| 8 | `memory` | Flash memory | both | assumed | S-001, S-002 | A-037, A-038, A-039 | low | 0.6.0 | Stores up to 99 recordings and 60 minutes in total; recording stops when either limit is reached. |
| 9 | `decoder` | Speech decoder | playback | not modeled | S-002 | A-004 | not applicable | | Rebuilds the voice-band stream from the stored CELP frames. |
| 10 | `codec_output` | Codec output: DAC and reconstruction filter | playback | assumed | S-003, S-005, S-007 | A-015, A-016, A-026, A-027 | medium | 0.4.0 | Converts the stream back to an analog signal and smooths it with the codec's reconstruction low-pass filter. |
| 11 | `volume_amp` | Volume control and power amplifier | playback | not modeled | S-001, S-002, S-006 | A-031 | not applicable | | Sets the playback level and drives the speaker through a small power amplifier that is muted while recording. |
| 12 | `speaker` | Speaker | playback | not modeled | S-002 | A-010 | not applicable | | Turns the amplified signal into sound through a 2.8 cm, 8 Ω speaker in a small enclosure. |
| 13 | `earphone` | Earphone output | playback | not modeled | S-001, S-002 | A-012, A-032 | not applicable | | Feeds the volume-controlled signal to the 2.5 mm earphone jack, bypassing the power amplifier. |
| 14 | `clock` | Clock | both | not modeled | S-002 | A-011 | not applicable | | An 8 MHz resonator clocks the DSP, which derives the codec's 6 kHz sampling clock from it. |
| 15 | `power` | Power supply | both | not modeled | S-002 | A-013 | not applicable | | Two AAA cells feed a 3.3 V converter for the digital section and a switched rail for the analog recording circuit. |

Controls notes (initial): 3 "agc settings (enabled, knee, attack, release); tap after AGC"; 4 "record stage enable; ADC clamp enable; tap after record"; 5 "vas settings (enabled, mode, sensitivity, threshold, hang, onset); tap after VAS; VAS splices and the paused flag in block reports"; 8 "memory settings (capacity, max recordings); memory figures, recording list, erase, export and import"; 10 "playback stage enable; tap after playback"; 11 "route selection; speaker muted while recording (state read)"; 12 "route selection (speaker)"; 13 "route selection (earphone)"; all others empty.

Header (initial): library version = the library's; device rate 6000; stage count 15; counts sourced 0, assumed 5, not modeled 10, checked 0; basis sentence per FR-012; evidence link = the README's "Hardware evidence" issue address.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An app developer, using only the README, can produce the complete status list (the header and all fifteen stages with every field) in Rust and in C with the documented calls and no other source of information, and the two lists are identical field for field.
- **SC-002**: On the committed registers the cross-check passes, and on each of the five doctored copies (an ID removed, an assumption superseded, an assumption added, a stage renamed, a confidence raised above its bound) it fails and names the stage and the ID: 5 of 5 detected.
- **SC-003**: 100 % of the manifest's strings pass the wording rules (FR-002 lengths and form, FR-010, FR-011); the prohibited-word list is a single list that a test applies to every string.
- **SC-004**: The manifest is identical on every CI platform and every read: a digest of all its strings and figures is pinned and compared on each platform; reading the whole manifest allocates nothing (0 allocations in the allocation-counting test).
- **SC-005**: The three golden files and their SHA guards are byte-identical before and after the feature, and the engine's timing figures are within the existing noise of the baseline.
- **SC-006**: The header reads "0 of 15 stages checked against a real unit" worth of data today: checked 0, sourced 0, assumed 5, not modeled 10, sum 15, and the signal-chain table has exactly 15 rows.
- **SC-007**: Workspace line coverage stays at or above 80 % and within 1 point of the baseline (98.58 %).

## Assumptions

- **The status is per stage, not per behavior.** A stage with a partly sourced, partly assumed model (stage 4: a sourced rate, an assumed template) reads "assumed"; the registers carry the finer detail. This keeps the four statuses honest ("sourced" means nothing is assumed) and keeps the header's counts simple. A per-behavior breakdown can be added later as more entries without changing the fifteen stage entries.
- **The memory counts as modeled.** Stage 8 does not change the sound, but the engine models its capacity, numbering and stop-at-limit behavior on registered descriptions (A-037 – A-039), so its status is "assumed", not "not modeled". The route (stages 11–13) is a label only and those stages stay "not modeled".
- **Confidence is the lowest headline level.** The registers often state split confidences ("High (topology) / Low (gain)"). The manifest takes the first-named level of each current assumption and the stage's confidence may not exceed the lowest of them; "Low to medium" reads as low. The maintainers may state a lower confidence than the rule allows, never a higher one.
- **Descriptions describe the device, not the model's coverage.** What the engine does and does not model at a stage is in the status, the IDs and the registers; the description stays stable across model changes so localized copies stay valid.
- **Figures in strings are sourced figures only** (6 kHz, 8-bit µ-law, 4 kbit/s, 99 recordings, 60 minutes, 2.8 cm, 8 Ω, 2.5 mm, 8 MHz, 3.3 V, two AAA cells: S-001, S-002). Assumed figures (the knee, the band edges, the thresholds) stay out of the strings.
- **Model versions name releases, not merges.** They are the version strings of the CHANGELOG entries that changed the stage's model; no release has been published, so they are the CHANGELOG's own version numbers.
- **The evidence link is a stable address** under the project's repository; if it moves, the header changes and the change is a CHANGELOG entry.
- **Breaking-change rule.** The stage numbers and identifiers are part of the public surface: renumbering or renaming is a breaking change. Changing a name, description, status, confidence, IDs, controls note or model version is a minor change recorded in the CHANGELOG.
- No release has been published; the CHANGELOG records the feature as version 0.7.0. The C API grows by new functions and types only; nothing existing changes.
