# Feature Specification: The Pre-1.0 C ABI Pass

**Feature Branch**: `022-abi-pass`

**Created**: 2026-10-31

**Status**: Draft (awaiting the three draft reviews and the owner's sign-off on D1 – D7)

**Input**: User description: "022 the pre-1.0 C ABI pass (the owner's road to 1.0, decisions.md 2026-10-31). As an app developer integrating the C API (directly or through the Swift package), I want every struct the library fills to carry a size field with one stated compatibility rule, and the two array-element structs to be declared frozen with a start-up check, so that a later library can add a field to a report without breaking my binary, a stale binary is refused clearly instead of being written past its struct, and this is the last ABI break before 1.0."

## Decisions for sign-off

The owner reads this section first. Each decision gives the recommendation; the alternative is in the Assumptions section. The three draft reviews are applied at their defaults below and listed under Clarifications.

| # | Decision | Recommended |
|---|---|---|
| D1 | **The eleven out-structs gain a size field.** `RrDr60BlockInfo`, `RrDr60RecordInfo`, `RrDr60StopOutcome` (which embeds a `RrDr60RecordInfo`), `RrDr60PlayInfo`, `RrDr60EngineState`, `RrDr60MemoryStatus`, `RrDr60SpeechCodecInfo`, `RrDr60ManifestHeader`, `RrDr60StageEntry`, `RrDr60Control` and `RrDr60ControlOption` get `uint32_t struct_size` as their first field, under the **compatible prefix-fill rule** every sized struct already follows (spec 018): the caller sets it to `sizeof`; a value smaller than the smallest layout ever published for that struct is refused with `INVALID_ARGUMENT` and nothing is written; a known smaller layout receives exactly its prefix; a larger value receives the whole struct with the tail zeroed. This version's layout is the smallest ever published for each of the eleven, so today only "smaller → refused" and "larger → zero tail" apply. The embedded `RecordInfo` inside `StopOutcome` is written by the engine, its own size set. | **Size fields on all eleven** (alternative: the per-call reports only) |
| D2 | **The two array-element structs are frozen.** `RrDr60VasEvent` (16 bytes) and `RrDr60ChangeEntry` (40 bytes) are passed as caller arrays with a capacity, where a per-element size field cannot work; they stay as they are and are declared frozen: a new field would be a new struct with new functions, never a change to these. Their sizes are published as defines (`RR_DR60_VAS_EVENT_SIZE`, `RR_DR60_CHANGE_ENTRY_SIZE`). | **Frozen, sizes published** (alternative: a stride argument on every array-taking function) |
| D3 | **One ABI version and a start-up check.** `RR_DR60_ABI_VERSION` (1) and `rr_dr60_abi_check(abi_version, sizeof(RrDr60VasEvent), sizeof(RrDr60ChangeEntry))` returning `OK` or the new status `ABI_MISMATCH`, for an app to call once at start; the Swift package calls it when it loads. The ABI version moves only on a change no size field can express (a frozen struct, a function signature, an enum value's meaning). | **A version plus a one-call check** (alternative: the size defines alone) |
| D4 | **One breaking commit, version 0.22.0, the last planned ABI break before 1.0.** A `feat!:` commit; every C caller rebuilds once (the C smoke, the Swift package, whose wrappers set `struct_size` because Swift's zero-initializer leaves it 0); recorded in decisions.md and the guide's C section. | **As stated** |
| D5 | **Nothing else moves.** The Rust API is unchanged; no audio change (every golden guard and smoke hash unmoved); the stage and control manifests unchanged; no assumption (the device has no C API). | **As stated** |
| D6 | **Cost.** Reading the caller's size and the prefix copy on the per-call reports stay within the existing 3 % A/B budget against 0.21 (merge commit 81f14dd) under the `ab-timing` label's existing modes; no new mode. | **As stated** |
| D7 | **Records.** The guide's C / Swift section restated (the rule, the frozen pair, the start-up check, the rebuild); the CHANGELOG under Changed as a breaking change with every struct's old and new size; the smoke's struct-size table and the three cases per struct; the Swift package's wrappers and a parity test; CLAUDE.md. | **As stated** |

## Clarifications

*(The draft reviews' findings and the owner's sign-off are recorded here as they arrive.)*

## Overview

The C API has grown struct by struct since spec 001. Seven structs carry a size field with one compatibility rule, and that rule has let later versions add fields to `RrDr60Settings` and `RrDr60Meters` without breaking earlier callers. Thirteen structs carry none: a field appended to one of them silently breaks every binary compiled against the earlier layout, which happened twice (`RrDr60PlayInfo` in 0.13, `RrDr60StopOutcome` in 0.20) and was noted each time as "the pre-1.0 ABI pass". This feature is that pass: the same rule on every struct the library fills, the two array-element structs declared frozen, one ABI version an app can check at start, and one last rebuild for every caller. After it, a report can grow without breaking an app, a stale app is refused instead of overwritten, and the C surface is ready to be frozen at 1.0.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A report can grow without breaking my app (Priority: P1)

An app developer compiles against this version, fills every struct through the library's calls, and keeps working unchanged when a later version appends a field to a report: the new library fills the prefix the app knows and refuses an app whose struct is smaller than anything it ever published.

**Why this priority**: it is the pass's purpose.

**Independent Test**: for each of the eleven structs, a call with `struct_size` below this version's size is refused with `INVALID_ARGUMENT` and writes nothing; with the exact size the struct is filled; with a larger size the struct is filled and the tail bytes are zero; the Rust and C paths agree field for field.

**Acceptance Scenarios**:

1. **Given** any of the eleven structs, **When** a call is made with `struct_size` smaller than this version's size, **Then** the status is `INVALID_ARGUMENT`, no byte of the caller's struct is written, and the engine's state is unchanged (a processing call with a refused report processes nothing).
2. **Given** the exact size, **When** the call is made, **Then** every field equals the Rust path's value and the call behaves as in 0.21.
3. **Given** a larger size (the struct followed by sentinel bytes), **When** the call is made, **Then** the struct is filled, the bytes beyond this version's size are zero, and nothing beyond `struct_size` is touched.
4. **Given** `RrDr60StopOutcome`, **When** filled, **Then** its embedded `RecordInfo` carries its own size set by the engine, and the outer rule applies to the outer struct only.
5. **Given** every processing and playback call that fills a report, **When** `out_info` is NULL (where NULL is allowed today), **Then** the behavior is unchanged.

---

### User Story 2 - A stale app is told, once, at start (Priority: P2)

An app calls the ABI check once when it loads, with the version it was compiled against and the sizes of the two frozen structs; a library that cannot serve it says so before any audio runs.

**Independent Test**: the check returns `OK` with this version and the published sizes, `ABI_MISMATCH` with any other version or size, and never touches the engine.

**Acceptance Scenarios**:

1. **Given** `RR_DR60_ABI_VERSION` and the two `sizeof`s, **When** `rr_dr60_abi_check` is called, **Then** it returns `OK`.
2. **Given** a different version, or a different size for either struct, **When** called, **Then** it returns `ABI_MISMATCH` (a new status appended after the current last one).
3. **Given** the Swift package, **When** it loads, **Then** it calls the check and surfaces a mismatch as an error before any engine call; its wrappers set `struct_size` on every sized struct they pass, and its parity test runs the eleven structs through the C path against the Rust path.
4. **Given** the two frozen structs, **When** their sizes are read, **Then** they equal the published defines (16 and 40) and a const assertion pins each.

---

### User Story 3 - Nothing else moved (Priority: P3)

A maintainer checks that the pass changed only the C surface: every golden guard and smoke hash unmoved, the Rust API identical, the cost within budget.

**Acceptance Scenarios**:

1. **Given** the fourteen golden guards and every smoke hash, **When** run, **Then** they are unmoved.
2. **Given** the Rust public API, **When** diffed against 0.21, **Then** nothing changed.
3. **Given** the A/B label's existing modes against 0.21, **When** run, **Then** every path is within its budget (3 % locally, 5 % on the runner).
4. **Given** the header, **When** regenerated, **Then** it differs from 0.21 by the eleven size fields, the two defines, the version define, the check function and the status.

---

### Edge Cases

- `struct_size` = 0 (a zero-initialized struct, which is what Swift's default initializer produces): refused with `INVALID_ARGUMENT`; the guide and the Swift wrappers say so.
- A `struct_size` larger than any plausible struct (for example `UINT32_MAX`): the struct is filled and the tail zeroed up to `struct_size`, exactly as the rule says, since the caller promised that many writable bytes; no cap is introduced.
- A refused per-call report on `rr_dr60_process*` / `rr_dr60_record*` / `rr_dr60_play*`: the call processes nothing (the refusal is checked before any sample), so the stream's position and the memory are unchanged.
- The constant-data structs (`RrDr60Control`, `RrDr60ControlOption`, `RrDr60ManifestHeader`, `RrDr60StageEntry`) hold string pointers: the prefix-fill copies the pointers as they are; a zeroed tail holds NULL pointers, which the guide documents as "fields this library does not know".
- `rr_dr60_abi_check` with any size for a struct this version does not have is a mismatch (the check names only the two frozen structs).
- The settings struct's rule is unchanged (a larger `struct_size` is still refused there: a caller newer than the library).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The eleven out-structs MUST carry `uint32_t struct_size` as their first field and MUST follow the compatible prefix-fill rule: below the smallest published layout refused with `INVALID_ARGUMENT` and nothing written, a known smaller layout filled as its prefix, larger filled with a zeroed tail; every refusal MUST happen before any processing.
- **FR-002**: `RrDr60VasEvent` and `RrDr60ChangeEntry` MUST stay byte-identical to 0.21 and be documented as frozen; their sizes MUST be published as defines and pinned by const assertions.
- **FR-003**: `RR_DR60_ABI_VERSION` (1) MUST be published and `rr_dr60_abi_check(abi_version, vas_event_size, change_entry_size)` MUST return `OK` only for this version and the two published sizes, `ABI_MISMATCH` otherwise, touching no engine state.
- **FR-004**: The Rust public API MUST be unchanged; every golden guard and smoke hash MUST be unmoved; the stage and control manifests MUST be unchanged; no assumption MUST be added.
- **FR-005**: Every struct's size MUST be pinned by a const assertion in the C layer and checked by the C smoke; the C smoke MUST exercise the three cases for each of the eleven structs; the harness's parity test MUST compare the Rust and C paths for each.
- **FR-006**: The Swift package MUST set `struct_size` in its wrappers, call the ABI check at load, and pass a parity test on the eleven structs.
- **FR-007**: The per-sample cost under the A/B label's existing modes MUST stay within 3 % of 0.21 (5 % on the runner).
- **FR-008**: The records: the guide's C / Swift section (the rule, the frozen pair, the check, the rebuild, the zero-initializer note), the CHANGELOG under Changed as a breaking change listing every struct's old and new size, decisions.md's "last planned ABI break before 1.0", CLAUDE.md; version 0.22.0.

### Out of Scope

- Any Rust API change; any new field in any struct beyond the inserted size field; field reordering or packing.
- The 1.0 release itself and the freeze it brings.
- A stride argument on the array-taking functions (D2's alternative).

### Key Entities

- **Sized struct**: a C struct whose first field is its caller-set size, filled under the prefix-fill rule.
- **Frozen struct**: a C struct whose layout is promised never to change; a new form is a new type.
- **ABI version**: one integer that moves only on a change no size field can express.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All eleven structs pass the three cases in the C smoke and the parity test at every measurement rate; the two frozen sizes and every struct size are pinned.
- **SC-002**: The ABI check returns `OK` exactly once per combination tested (the right one) and `ABI_MISMATCH` for every wrong version or size.
- **SC-003**: The fourteen golden guards and every smoke hash unmoved; the Rust API diff empty; the header diff limited to the listed additions.
- **SC-004**: The A/B label's modes within budget on the runner.
- **SC-005**: The Swift suites green with the wrappers; coverage at or above 80 % and within 1 point of the baseline (0.21: 96.64 % locally).

## Assumptions

- **Alternatives considered**: size fields on the per-call reports only (D1: the constant-data and status structs grow too, and one rule is easier to state than two); a stride argument on the array-taking functions (D2: twenty-one signatures and a per-call cost for a struct that has not changed since its spec; freezing is honest and cheap, and a new form can always be a new struct); the size defines alone without a version (D3: a signature or meaning change would have no signal); several breaking commits (D4: one rebuild for every caller).
- **The prefix-fill rule is spec 018's**, already documented for `RrDr60Meters`; the settings struct keeps its stricter rule (a larger caller refused).
- **Swift's default initializer zeroes a C struct**, so the wrappers must set `struct_size`; a zero is refused by design, never silently filled.
- **No device claim**: the C API is the emulator's surface; nothing here describes the device.
