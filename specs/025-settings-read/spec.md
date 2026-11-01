# Feature Specification: The Settings Read and Control Access by Index

**Feature Branch**: `025-settings-read`

**Created**: 2026-11-01

**Status**: Draft; the three draft reviews next, then `/speckit-clarify`, then the owner's sign-off.

**Input**: The owner's decision of 2026-11-01 in the test bench's `/speckit-clarify` (ic-recorder-app, spec 001, question 1: "engine first"): the engine gains a read of the settings it is running with and access to every control by the control manifest's index, so the bench can prove an Apply took effect from the engine's own read and build its controls without a hand-written table. The first engine feature after 0.90.0 (a 0.9x minor: 0.91.0); spec 024 stays held.

## Decisions for sign-off

The owner reads this section first. Each decision gives the recommendation; the alternatives are in the Assumptions section.

| # | Decision | Recommended |
|---|---|---|
| D1 | **The applied-settings read.** `rr_dr60_get_settings(pipeline, out)` writes the settings the engine is running with into a caller's settings structure (the Rust `Pipeline::settings()` already exists). It reflects every landed live value as `settings()` does since spec 017 (a scheduled record-side value or stage switch that has landed is in it), so it is the engine's own answer to "what is applied". The settings structure keeps spec 022's rule: its `struct_size` must equal the settings size, else the call is refused and nothing is written. Real-time safe. | **One read, the existing Rust semantics** |
| D2 | **A reading of any control on a running engine.** `control_value(index)` returns the current value of the control at the manifest's index: a setting from the applied settings, a live control from its own read (route, volume position, speed level, speed mode, battery voltage), the host rate from the settings. The result is a **control reading** carrying the value in the manifest's value type: a switch (true or false), an integer (an unsigned 64-bit value, exact: the seed spans the whole range), a continuous value (single precision, as the setting holds it), or a choice (the engine value of the option). In C a sized structure under spec 022's rule (`RrDr60ControlReading`: `struct_size`, the value type, a `double` for continuous and switch values, a `uint64_t` for integer and choice values). An index past the manifest is refused. | **One reading per control, exact integers** |
| D3 | **Settings access by control index, on a settings structure.** Two pure functions with no engine handle: `settings_control_value(settings, index)` reads a control's value from a settings structure, and `set_settings_control_value(settings, index, reading)` writes it. The write checks **representability only** (the reading's value type matches the control's; a choice value is one of its options; a continuous value is finite; a switch is true or false) and leaves **ranges to the validator** (`rr_dr60_settings_validate`), so an out-of-range value reaches the validator and is refused there with the field named. A control that is not a setting (the five plain-setter live controls) is refused with a dedicated status (`NOT_A_SETTING`). | **Pure, representability here, ranges in validate** |
| D4 | **Setting a live control by index.** `set_control_value(pipeline, index, reading)` sets one of the five live controls that have a plain setter (route, volume, speed level, speed mode, battery voltage), with exactly that setter's validation and timing; every other control is refused with a dedicated status (`NOT_A_PLAIN_LIVE_CONTROL`): an idle setting changes through a settings structure and `reconfigure`, a schedule-only live value through the schedule. | **The five, by index** |
| D5 | **One mapping in the engine.** The processing record's own setting table (spec 020: the identifier → field accessor behind the record's slot block) becomes the one mapping from control index to settings field, used by the record, the new functions and the control manifest's tests; a test proves for every control that the index, the identifier, the value type, the default in the default settings and the record's line agree, and an engine setting added without a mapping fails the existing exhaustive-destructuring test. | **One table** |
| D6 | **No audio change, no moved constant.** Every golden guard and every smoke hash unmoved; the control and stage manifests unchanged but for the stage manifest's version line; no existing struct changes; appended: `rr_dr60_get_settings`, `rr_dr60_control_value`, `rr_dr60_settings_control_value`, `rr_dr60_set_settings_control_value`, `rr_dr60_set_control_value`, the `RrDr60ControlReading` structure, the value-type enumeration's reuse (spec 014's `RrDr60ControlValueType`), the statuses **39** `NOT_A_SETTING` and **40** `NOT_A_PLAIN_LIVE_CONTROL` (spec 024, held, renumbers if it resumes later); the ABI version stays 1; `scripts/abi-promise.sh` passes with 12a3c45 (0.22) and f5a0245 (0.90). Version 0.91.0. | **Appended only** |
| D7 | **Real-time safety.** Every new function is real-time safe (no allocation, no lock, constant work); the reads may be called from any thread that may call the engine. | **As stated** |
| D8 | **The Swift package** gains `RrDr60ControlReading.sized` and a small typed wrapper (`ControlReading` with the four cases) so the bench needs no C struct handling. | **A typed wrapper** |
| D9 | **Records**: the guide's control-manifest section (the five functions, the representability-versus-range rule, an app's generated control surface in a few lines), the CHANGELOG under Added, the README's list of C functions, CLAUDE.md; the bench's spec 001 points to the engine version. | **As stated** |

## Clarifications

### Session 2026-11-01 (the owner's request)

- The test bench (ic-recorder-app spec 001) needs to prove an Apply from the engine's own read and to build its controls from the manifest without a hand-written accessor table; the owner chose to add both in the engine before bench 001 continues.

## Overview

Since spec 014 the engine describes its 31 controls (identifier, kind, timing, value type, range, default) and since spec 020 its processing record writes every setting by name, but the C API gives an app no way to read the settings an engine is running with or to reach a control by its manifest index. An app that builds its controls from the manifest therefore keeps its own table from identifier to settings field and cannot verify that a reconfigure applied what it asked. This feature adds the read and the access by index, built on the mapping the engine already keeps for the processing record, so a generated control surface needs no app-side table and every value an app shows comes from the engine.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Read what is applied (Priority: P1)

An app developer applies a settings set and reads back from the engine what it is running with.

**Independent Test**: reconfigure with a set differing in every setting, read the applied settings, compare field by field; land a scheduled record-side value and a stage switch, read again.

**Acceptance Scenarios**:

1. **Given** an engine reconfigured with a settings set, **When** the applied settings are read, **Then** they equal the set field by field, in Rust and through C, at every host rate.
2. **Given** a scheduled record-side value and a playback-stage switch that have landed, **When** read, **Then** the read carries the landed values (as `settings()` does).
3. **Given** a settings structure with a wrong `struct_size`, **When** passed to the read, **Then** it is refused and nothing is written.

---

### User Story 2 - Every control by its index (Priority: P2)

An app developer generates a control surface from the manifest and reaches every control by its index.

**Independent Test**: for every index, read the control's value on a default engine, read and write it on a settings structure, set the five live controls, and check the refusals.

**Acceptance Scenarios**:

1. **Given** a default engine, **When** every control is read by index, **Then** each reading's value type is the manifest's and its value is the manifest's default (the host rate's is the engine's rate).
2. **Given** a settings structure, **When** every setting is written by index with a value of its type and read back, **Then** the value round-trips and the structure's field holds it; **given** a reading of the wrong type, a choice value outside the options or a non-finite continuous value, **Then** the write is refused; **given** an out-of-range value of the right type, **Then** the write succeeds and the validator refuses the structure naming the field.
3. **Given** one of the five plain-setter live controls, **When** set by index, **Then** the engine's read returns the value, with the same validation and timing as its setter; **given** an idle setting or a schedule-only value, **Then** setting it by index is refused with its status.
4. **Given** the one mapping, **When** tested, **Then** for every control the index, the identifier, the value type, the default and the processing record's line agree, and every settings field is reachable by exactly one index.

---

### User Story 3 - Nothing else moved (Priority: P3)

1. **Given** the sixteen golden guards and every smoke hash, **When** built, **Then** unmoved; the control manifest's canonical text unchanged.
2. **Given** a C caller compiled against 0.22 or 0.90, **When** linked to 0.91, **Then** the promise script passes.
3. **Given** the allocation and determinism suites, **When** extended by the new functions, **Then** they pass.

### Edge Cases

- An index equal to or past the control count: refused (`NO_SUCH_CONTROL`, the existing status).
- The seed's full range (0 … 2⁶⁴ − 1): exact through the integer field, never through a double.
- A continuous value that is finite but not representable in single precision exactly: written as the nearest single-precision value (the setting's own type), read back as that value.
- `tap` and `vas.mode` (choices): the option's engine value; a value that is not an option refused.
- The host rate by index on a settings structure: written like any setting; `reconfigure` decides.
- A live control read during a 20 ms volume or battery transition: the target value (the read the setter's getter returns).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The engine MUST offer a read of its applied settings (D1) in Rust and C, reflecting landed live values, refusing a wrong `struct_size` without writing.
- **FR-002**: The engine MUST offer a control reading by manifest index on a running engine (D2) for every control, in the manifest's value type, integers exact.
- **FR-003**: The engine MUST offer reading and writing a setting by control index on a settings structure (D3), checking representability only and refusing non-settings with `NOT_A_SETTING`.
- **FR-004**: The engine MUST offer setting the five plain-setter live controls by index (D4) and refuse every other control with `NOT_A_PLAIN_LIVE_CONTROL`.
- **FR-005**: One mapping MUST serve the processing record and the new functions (D5), proven by a test over every control.
- **FR-006**: No audio change, no moved constant, appended C items only, the promise script green with 12a3c45 and f5a0245 (D6); version 0.91.0.
- **FR-007**: Every new function MUST be real-time safe (D7) and allocation-free.
- **FR-008**: The Swift package MUST offer the typed wrapper of D8.

### Key Entities

- **Applied settings**: the settings the engine runs with, landed live values included.
- **Control reading**: one control's value in its value type (switch, integer, continuous, choice).
- **Control mapping**: control index ↔ settings field (or live control), the one table.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 31 of 31 controls read by index on a default engine equal to the manifest's defaults; every setting round-trips by index on a settings structure.
- **SC-002**: The applied-settings read equals the reconfigured set in every field at seven rates, landed values included.
- **SC-003**: Every refusal (a wrong size, a wrong type, a non-option, a non-setting, a non-plain live control, an index past the count) returns its status and changes nothing.
- **SC-004**: The sixteen golden guards and every smoke hash unmoved; the promise script green with both revisions.
- **SC-005**: Coverage within 1 point of 0.90.0's.

## Assumptions

- **Alternatives to D1**: a read by identifier string only (rejected: an app holds the index from the manifest); a prefix-fill like the reports (rejected: spec 022 D13 validates the settings structure whole).
- **Alternatives to D2**: separate typed reads per value type (more functions, the same data); a `double` for every value (rejected: the seed exceeds a double's exact integers).
- **Alternatives to D3**: range checks in the write (rejected: the validator is the one place ranges are checked, and the bench needs out-of-range values to reach it); writes by identifier string (the processing record's form; the index is what the manifest hands an app).
- **Alternatives to D4**: no live set by index (the app would keep a table of five setters; small, but the bench's request was no table).
- Spec 024 (held) took statuses 39 – 41 in its draft; if it resumes after 025 merges, it renumbers to follow 025's.
