# Feature Specification: The Settings Read and Control Access by Index

**Feature Branch**: `025-settings-read`

**Created**: 2026-11-01

**Status**: Draft reviewed 2026-11-01 (coach 1 – 13, hardware B1 – B9, product M1 – M5, applied at the defaults below); clarified 2026-11-01 (five questions, every answer the recommended one); **spec approved by the owner 2026-11-01 (D1 – D11)**; plan reviewed (coach 1 – 14, hardware B1 – B9, product M1 – M5) and **approved by the owner 2026-11-01**; tasks next.; **implemented 2026-11-01** (T001 – T014; the post-implementation reviews coach 1 – 10 and hardware B1 – B7 applied at the defaults; the gate next).

**Input**: The owner's decision of 2026-11-01 in the test bench's `/speckit-clarify` (ic-recorder-app, spec 001, question 1: "engine first"): the engine gains a read of the settings it is running with and access to every control by the control manifest's index, so the bench can confirm an Apply from the engine's own read and build its controls without a hand-written table. The first engine feature after 0.90.0 (a 0.9x minor: 0.91.0); spec 024 stays held.

## Decisions for sign-off

The owner reads this section first. Each decision gives the recommendation; the alternatives are in the Assumptions section.

| # | Decision | Recommended |
|---|---|---|
| D1 | **The applied-settings read.** `rr_dr60_get_settings(pipeline, out)` writes the engine's settings (the Rust `Pipeline::settings()`) into a caller's settings structure. **What it means**: the last value set or landed on any stream for every setting (spec 017's mirror; spec 021's switches: the target during a blend); each stream uses a landed value from its next produced sample or adopts it at its next start; a scheduled change that has not landed is not in it. It is a statement about what the engine is configured to do, not about what has sounded: a read confirms a `reconfigure` exactly; for a live value the confirmation that it took effect is the change log's entry (spec 015). **The size rule**: the output's `struct_size` must equal the settings size (spec 022 D13), refused otherwise before anything is written; the structure is filled in place in a buffer zeroed first, field by field (spec 022 D10), its padding zero under the Miri gate. **Spec 021's trap stays**: reconfiguring with a read taken after a playback-stage switch has landed configures that stage off, and the guide's recipe applies. | **One read; committed, not audible** |
| D2 | **A reading of any control on a running engine.** `rr_dr60_control_value(pipeline, index, out)` returns the current value of the control at the manifest's index: a setting from the read of D1, a live control exactly as its getter returns it (the last valid set or landed value: during a volume or battery transition, a deferred change into speed level 3, or a request waiting for a crossfade, the requested value, not the knob's position or the level heard), the host rate from the settings. The result is a **control reading** in the manifest's value type, a sized structure under spec 022's rule, **24 bytes**: `uint32_t struct_size` (0), `uint32_t value_type` (4, spec 014's `RrDr60ControlValueType`), `uint64_t int_value` (8: a switch as 0 or 1, an integer, a choice's engine value), `float float_value` (16: a continuous value, the same type as the setting, as spec 014's `RrDr60Control` carries it), `uint32_t reserved` (20, zero); the unused value field is zero on output and ignored on input; const assertions pin the offsets. As an input a reading's `struct_size` below 24 or above `RR_DR60_MAX_STRUCT_SIZE` is refused and only its first 24 bytes are read. | **One 24-byte reading** |
| D3 | **Settings access by control index, on a settings structure.** Two pure functions with no engine handle: `rr_dr60_settings_control_value(settings, index, out)` and `rr_dr60_set_settings_control_value(settings, index, reading)`. They read or write **only the mapped field**, through the raw pointer at its offset, never forming a reference to the whole structure (a structure holding an invalid value elsewhere, e.g. `tap = 99`, can still be repaired by index; the reserved slots and the padding are never touched); the input reading is copied before the write (the pointers may alias); a switch byte other than 0 or 1 is refused on read. The write checks **representability only**: the reading's value type is the control's; a switch is 0 or 1; a choice is one of the control's options; a continuous value is finite (−0.0 written as 0.0); **an integer fits the Rust field's width** (`u8` for the sensitivity, `u32` for the memory fields, `u64` for the seed; one rule in both languages). Ranges stay with `rr_dr60_settings_validate`, which names the field: a sensitivity of 6 is written and then refused by the validator, a sensitivity of 256 is refused at the write. A control that is not a setting (the five plain-setter live controls) is refused with `NOT_A_SETTING`. The host rate is written like any setting (one of its options; `reconfigure` accepts a rate change; its timing, host-fixed, is the app's to respect). | **Field-local, representability here, ranges in validate** |
| D4 | **Setting a live control by index.** `rr_dr60_set_control_value(pipeline, index, reading)` sets one of the five live controls that have a plain setter (route, volume, speed level, speed mode, battery voltage) through **that setter**, with its validation and timing: the reading converts to the same value `schedule_change` takes (spec 015 R-04's rule for continuous values), so a set by index, the plain setter and a schedule at the current position give identical change-log entries. Every other control is refused with `NOT_A_PLAIN_LIVE_CONTROL` (an idle setting changes through a settings structure and `reconfigure`, a schedule-only value through the schedule). | **The five, through their setters** |
| D5 | **One mapping, exhaustive, with an independent oracle.** A new mapping covers all 26 settings (the processing record's 24 lines plus `host_rate_hz` and `seed`, which the record carries as identity lines; the record's form unchanged) and routes the five live controls to their getters and setters; it destructures `Settings` exhaustively, so a new settings field fails to compile until it is mapped; the record's table is rebuilt on it. **The oracle is independent**: a hand-written harness test, for each settings index, writes a non-default value unique to that index (a switch flipped, a continuous value of the index + 0.5, an integer of the index + 2 within the field's width, a non-default option), destructures `Settings` exhaustively with each field paired to a literal identifier in the test's source, and asserts that exactly one field changed and that its literal is the manifest's identifier for the index (this catches two swapped fields of the same type and default, e.g. attack and release, speaker and earphone); the same through C on `rr_dr60_settings_default`, comparing the bytes with a structure set by hand by field name (the C compiler names the field), which catches writes into reserved slots or padding. | **One exhaustive mapping; a one-hot oracle** |
| D6 | **A validator refusal mapped to its control.** `rr_dr60_setting_field_control(field, out_index)` returns the control index for a `RrDr60SettingField` (refused with `NO_SUCH_CONTROL` for `NONE` and `STRUCT_SIZE`), so an app highlights the row the validator names; a test maps every field to the manifest identifier of the same name. | **One lookup** |
| D7 | **Refusals, in order, nothing written on any**: a null pointer (`NULL_POINTER`); a refused `struct_size` (`INVALID_ARGUMENT`); an index at or past the control count (`NO_SUCH_CONTROL`); a control of the wrong class (`NOT_A_SETTING`, `NOT_A_PLAIN_LIVE_CONTROL`); an unrepresentable reading (`INVALID_ARGUMENT`, the Rust `Error::InvalidControlValue`); then, for D4, the setter's own status (`INVALID_VOLUME`, `INVALID_SPEED_LEVEL`, `INVALID_BATTERY_VOLTS`, `INVALID_SPEED_MODE`; the route's). New statuses **39** `NOT_A_SETTING` and **40** `NOT_A_PLAIN_LIVE_CONTROL` (spec 024, held, renumbers if it resumes later). | **As stated** |
| D8 | **No audio change, no moved constant.** Every golden guard and every smoke hash unmoved; the control manifest's canonical text byte-identical; the stage manifest re-blessed for its version line only; no existing struct changes; `CONTROL_COUNT`, `RR_DR60_MAX_CHANGES_PER_CALL`, `RR_DR60_ABI_VERSION` and every existing struct size unchanged (asserted by the ABI test); the C items appended: the six functions of D1 – D4 and D6, `RrDr60ControlReading`, the two statuses; `ci.yml` runs `scripts/abi-promise.sh` with 0.22's merge (12a3c45) and a second step with 0.90's (f5a0245, the full id). Version 0.91.0. | **Appended only** |
| D9 | **Threads and real time.** The engine functions are real-time safe (no allocation, no lock, constant work) and, like every engine call, never run concurrently with another call on the same handle; the pure settings-structure functions take no handle and are safe on any thread with their own buffers. Every entry point is panic-guarded. | **As stated** |
| D10 | **The Swift package** gains `RrDr60ControlReading.sized`, a typed `ControlReading` (`switch(Bool)`, `integer(UInt64)`, `continuous(Float)`, `choice(UInt32)`), `ControlReading(defaultOf: index)` from the manifest entry, and `ControlIndex(identifier:)` (the manifest index for an identifier or nil, a loop over the manifest); the guide says presets and sessions store identifiers, never indices. | **A typed wrapper and the lookup** |
| D11 | **Records**: the guide's control-manifest section gains the paragraph "Reading and setting controls by index" (the product review's text, checked against the wording denylist: "read back" and "confirm", never "verify"); the guide's C block re-blessed; the CHANGELOG under Added; the README's list of C functions; the Swift ABI fixture regenerated if the ABI rows gain the new structure; the traceability audit; CLAUDE.md; the bench's spec 001 points to 0.91.0. Figures: the idle cost of each new call as the C smoke prints it (spec 022's timing prints); SC-006 quotes 0.90's coverage. | **As stated** |

## Clarifications

### Session 2026-11-01 (implementation restatements; the post-implementation reviews: coach 1 – 10, hardware B1 – B7)

- **D3, the read** (hardware B1, coach 8): the read by index refuses only a stored `bool` byte other than 0 or 1 (the reading cannot carry it); every other stored value is returned as stored (a non-option choice such as `tap = 99`, a non-finite float, an integer wider than the engine's field), so a repair screen can show it; the write or the validator then refuses it. The draft implementation also refused a non-option choice on read; one rule now, stated in the guide.
- **D9, the panic guards** (coach 1): the two engine calls that run core code (`rr_dr60_control_value`, `rr_dr60_set_control_value`) run it under the handle's guard (a panic poisons the handle, as `rr_dr60_process`), the write's core check under the constant-data guard, and the field-to-control lookup is constant data with no panic path (compile-time indices); `engine_calls_are_panic_guarded` forces a panic.
- **US2 AS2**: the Rust one-hot is the core test `control_map::tests::one_hot` (the harness cannot destructure the `#[non_exhaustive]` settings structures).
- **Placement**: `Pipeline::control_value` / `set_control_value` are in `control_map.rs`; the C functions are in `crates/rr_dr60_ffi/src/settings_read.rs` (re-exported from `lib.rs`); `cbindgen.toml` needed no entry (the reading is referenced by the functions).
- **R-07's mutants**: "the setter's validation bypassed" is the wide-integer wrap on the speed level by index (259 → 3), caught by the representability row; the harness has five mutants with named oracles, the whole-structure round trip is the FFI crate's.
- **The record's −0.0** keeps its sign on the record's round trip; a set by index stores 0.0.
- **D11**: the guide's paragraph adds the idle/live recipe (`reconfigure` when idle, `schedule_change` for live-timed settings during a stream), the transitions and waits that read the requested value, the confirmation rule (a set to the current value logs nothing) and −0.0; the README has no list of C functions, so only its version line moved.

### Session 2026-11-01 (the plan reviews: coach 1 – 14, hardware B1 – B9, product M1 – M5; dated restatements)

- **D5's Rust one-hot oracle is a core test**: the settings structures are `#[non_exhaustive]`, so only the core crate can destructure them exhaustively; the harness keeps the C one-hot.
- **D6's lookup takes the raw field value** (`uint32_t`) and the core's `error::Setting`; 0, 3, the retired 5 and 6, and unknown values return `NO_SUCH_CONTROL`.
- **D7's setter statuses by index** are `INVALID_VOLUME`, `INVALID_SPEED_LEVEL` and `INVALID_BATTERY_VOLTS`; a non-option route or mode, a level above 255 and a non-finite value are `INVALID_ARGUMENT` (`INVALID_SPEED_MODE` and a route status are unreachable by index); a NaN volume by index is `INVALID_ARGUMENT`, the plain setter's `INVALID_VOLUME`, documented.
- **D10's Swift wrapper covers all six calls** (throwing wrappers, `ControlIndex`, `init?(defaultOf:)` nil for the host rate).
- **D11: the reading stays out of the frozen `--abi` rows, the ABI example and the Swift ABI fixture** (the promise script diffs them byte for byte with 0.22 and 0.90); the new `--settings` mode prints it.
- **Recorded, out of scope**: a C caller's `bool` byte other than 0 or 1 is undefined behavior in create, validate and reconfigure (since 0.1); the guide says so now and the fix is queued for the next ABI-touching feature.

### Session 2026-11-01 (/speckit-clarify)

- Q: How should the reading structure store a fractional value, as a single-precision float or a double? → A: **An integer field and a single-precision float field** (D2 as drafted).
- Q: Should the applied-settings read mean what the engine is configured to do, with the change log confirming a live value took effect? → A: **Yes, configured; the log confirms** (D1 as drafted).
- Q: Should an integer too large for the engine's field be refused at the write or clamped? → A: **Refused at the write** (D3 as drafted).
- Q: Should setting a control by index on a running engine be limited to the five live controls with a plain setter? → A: **Yes, only the five** (D4 as drafted).
- Q: Should the engine translate the validator's field into the control's index? → A: **Yes** (D6 as drafted).

### Session 2026-11-01 (the owner's request)

- The test bench (ic-recorder-app spec 001) needs to confirm an Apply from the engine's own read and to build its controls from the manifest without a hand-written accessor table; the owner chose to add both in the engine before bench 001 continues.

### Session 2026-11-01 (the draft reviews: coach 1 – 13, hardware B1 – B9, product M1 – M5)

- **The mapping** (coach 1, hardware B5): the record's table covered 24 of 26 settings with a string match that a new field does not break; D5 makes the mapping exhaustive and its oracle independent (the one-hot test, the C byte comparison).
- **The reading's layout** (coach 2, hardware B7): an integer field and a single-precision float field (the control manifest's own form), switches in the integer field; the alternative, a double and an integer, was declined at `/speckit-clarify`.
- **Integer width and the conversion rule** (coach 3, hardware B2, B3): representability includes the Rust field's width, one rule in both languages; continuous values follow spec 015 R-04.
- **Refusal order** (coach 4); **the read's semantics and spec 021's trap** (coach 5, hardware B4); **the live getters' exact meaning** (coach 6); **the field-local C write** (hardware B1); **the settings read built in place** (coach 8, hardware B6); **threads** (coach 10, hardware B8); **the promise script's two revisions and the unchanged constants** (coach 9); **the field-to-control lookup** (coach 11, D6); **the host rate** (coach 12); **the records and the figures** (coach 13); **named tests and mutants** (coach 7, hardware B5: in the plan); **the identifier lookup, the default reading, the guide's wording** (product M1, M2, M4); **no settings-to-text call now** (product M3: a session restores from the processing record's read-out, presets in the app's own form keyed by identifier).

## Overview

Since spec 014 the engine describes its 31 controls (identifier, kind, timing, value type, range, default) and since spec 020 its processing record writes every setting by name, but the C API gives an app no way to read the settings an engine is configured with or to reach a control by its manifest index. An app that builds its controls from the manifest therefore keeps its own table from identifier to settings field and cannot confirm that a reconfigure applied what it asked. This feature adds the read and the access by index on one exhaustive mapping inside the engine, so a generated control surface needs no app-side table and every value an app shows comes from the engine.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Read what is configured (Priority: P1)

An app developer applies a settings set and reads back from the engine what it is configured with.

**Independent Test**: reconfigure with a set differing in every setting, read the applied settings through Rust and C, compare field by field; land a scheduled record-side value and a stage switch, read again; set a live value without a processing call and read.

**Acceptance Scenarios**:

1. **Given** an engine reconfigured with a settings set, **When** the applied settings are read, **Then** they equal the set field by field in Rust and through C (into a painted buffer whose padding reads zero) at the measurement rates, and the next recording's processing record agrees (`us1_settings_read`).
2. **Given** a scheduled record-side value and a playback-stage switch that have landed, **When** read, **Then** the read carries the landed values equal to the change log's `after` (`us1_settings_read_landed`); **given** reconfiguring with that read after the switch landed, **Then** the stage is configured off, as spec 021's trap states (`us1_trap_named`).
3. **Given** a plain `set_volume` and no processing call yet, **When** the volume is read, **Then** the read returns the new value while the change log is still empty (`us1_committed_not_audible`).
4. **Given** a settings structure with a `struct_size` other than the settings size, **When** passed to the read, **Then** it is refused and nothing is written (`us1_size_refused`).

---

### User Story 2 - Every control by its index (Priority: P2)

An app developer generates a control surface from the manifest and reaches every control by its index.

**Independent Test**: the one-hot oracle over every settings index in Rust and C; every control read on a default engine; the five live controls set by index against their setters; every refusal.

**Acceptance Scenarios**:

1. **Given** a default engine, **When** every control is read by index, **Then** each reading's value type is the manifest's and its value is the manifest's default (the host rate's is the engine's rate), and `ControlReading(defaultOf:)` equals it (`us2_defaults_by_index`).
2. **Given** a settings structure, **When** each setting is written by index with its one-hot value, **Then** exactly the field the test names by identifier changed, in Rust and in C (the core test `control_map::tests::one_hot`, `us2_one_hot_c`); **given** a reading of the wrong type, a choice value outside the options, a non-finite continuous value or an integer wider than the field, **Then** the write is refused with `INVALID_ARGUMENT` and nothing changes; **given** an out-of-range value within the width (sensitivity 6), **Then** the write succeeds and the validator refuses naming the field, and `rr_dr60_setting_field_control` maps that field to the control's index (`us2_representability`).
3. **Given** one of the five live controls, **When** set by index, **Then** the engine's read returns the value and the change log's entries are identical to the plain setter's and to a schedule at the current position (`us2_live_by_index`); **given** an idle setting or a schedule-only value, **Then** setting it by index is refused with `NOT_A_PLAIN_LIVE_CONTROL` (`us2_class_refused`).
4. **Given** a structure holding `tap = 99`, **When** the `tap` index is written with a valid option, **Then** the structure becomes valid and no other byte changed (`us2_repair_by_index`).
5. **Given** a non-default engine, **When** reconfigured with the default settings and each of the five live controls set by index to its manifest default, **Then** all 31 readings equal the defaults of AS1 (`us2_device_as_modeled`).

---

### User Story 3 - Nothing else moved (Priority: P3)

The engine's output, constants and existing structures are unchanged; old C callers keep working.

**Independent Test**: the golden guards, the smoke hashes, the ABI test, the promise script with two revisions, the allocation and determinism suites.

1. **Given** the sixteen golden guards and every smoke hash, **When** built, **Then** unmoved; the control manifest's canonical text byte-identical.
2. **Given** a C caller compiled against 0.22 or 0.90, **When** linked to 0.91, **Then** the promise script passes with both revisions.
3. **Given** the allocation and determinism suites, **When** extended by the new functions, **Then** they pass; the Miri gate covers the new fills and the field-local writes.

### Edge Cases

- An index equal to or past the control count: `NO_SUCH_CONTROL`.
- The seed's full range (0 … 2⁶⁴ − 1): exact through the integer field (the seed 2⁵³ + 1 round-trips).
- A continuous value of 0.1 written: read back as the single-precision value 0.10000000149011612; −0.0 written as 0.0.
- `tap` and `vas.mode` (choices): the option's engine value; a value that is not an option refused.
- A live control read during a transition, a deferred speed change or a waiting request: the requested value (D2).
- A switch byte other than 0 or 1 in a caller's structure: refused on read, never interpreted.
- An input reading's unused field non-zero: ignored.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The engine MUST offer the applied-settings read of D1 in Rust and C with D1's meaning, size rule and in-place fill.
- **FR-002**: The engine MUST offer a control reading by manifest index on a running engine (D2) for every control, in the 24-byte reading, integers exact.
- **FR-003**: The engine MUST offer reading and writing a setting by control index on a settings structure (D3), field-local, representability only (the Rust field width included), refusing non-settings with `NOT_A_SETTING`.
- **FR-004**: The engine MUST offer setting the five plain-setter live controls by index through their setters (D4), refusing every other control with `NOT_A_PLAIN_LIVE_CONTROL`.
- **FR-005**: One exhaustive mapping MUST serve the processing record and the new functions (D5), proven by the one-hot oracle in Rust and C.
- **FR-006**: The engine MUST offer the field-to-control lookup of D6.
- **FR-007**: Every refusal MUST follow D7's order and write nothing.
- **FR-008**: No audio change, no moved constant, appended C items only, the promise script green with 12a3c45 and f5a0245 in CI (D8); version 0.91.0.
- **FR-009**: Every new function MUST be real-time safe, allocation-free and panic-guarded, with D9's thread rule.
- **FR-010**: The Swift package MUST offer D10's wrapper and lookups.

### Key Entities

- **Applied settings**: the engine's settings, the last value set or landed for each.
- **Control reading**: one control's value in its value type (switch, integer, continuous, choice), 24 bytes in C.
- **Control mapping**: control index ↔ settings field (26) or live getter and setter (5), exhaustive.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 31 of 31 controls read by index on a default engine equal to the manifest's defaults; 26 of 26 settings pass the one-hot oracle in Rust and in C.
- **SC-002**: The applied-settings read equals the reconfigured set in every field at the measurement rates, landed values included and equal to the log's `after`.
- **SC-003**: Every refusal of D7 returns its status in order and changes nothing.
- **SC-004**: The five live controls set by index give change-log entries identical to their plain setters'.
- **SC-005**: The sixteen golden guards and every smoke hash unmoved; the promise script green with both revisions; the Miri gate green.
- **SC-006**: Coverage within 1 point of 0.90.0's (96.73 % lines locally).

## Assumptions

- **Alternatives to D1**: a read by identifier string only (rejected: an app holds the index from the manifest); a prefix-fill like the reports (rejected: spec 022 D13 validates the settings structure whole).
- **Alternatives to D2's layout**: a `double` and a `uint64_t` (the hardware reviewer's; a finite double above the single-precision maximum would then need its own refusal); a union (rejected: spec 022's padding oracle and Swift).
- **Alternatives to D3**: range checks in the write (rejected: the validator is the one place ranges are checked, and the bench needs out-of-range values to reach it); a whole-structure round trip (rejected: an invalid value elsewhere would block the write, and the reserved slots would be rewritten); saturating a too-wide integer (rejected: the read-back would differ from the write).
- **Alternatives to D4**: no live set by index (the app would keep a table of five setters); `set_control_value` also writing idle and schedule-only values into the engine's settings (rejected: their timing belongs to `reconfigure` and the schedule).
- **Alternatives to D5**: the record's existing table as the mapping (rejected by the reviews: 24 of 26 settings and not exhaustive).
- **Alternatives to D10**: no identifier lookup (every app would loop over the manifest itself); a settings-to-text call (deferred: the processing record's read-out restores a session).
- Spec 024 (held) took statuses 39 – 41 in its draft; if it resumes after 025 merges, it renumbers to follow 025's.
