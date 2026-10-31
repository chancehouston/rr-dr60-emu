# Feature Specification: The Pre-1.0 C ABI Pass

**Feature Branch**: `022-abi-pass`

**Created**: 2026-10-31

**Status**: Complete on `main` (PR #55 merged 2026-10-31, merge commit 12a3c45; version 0.22.0). Spec approved by the owner 2026-10-31 (D1 – D12); plan reviewed 2026-10-31 (coach 1 – 11, hardware B1 – B9, product M1 – M9); D13 approved by the owner 2026-10-31 (the settings rule, found inverted by the plan's hardware review); plan approved 2026-10-31; implemented, reviewed (coach 1 – 10, hardware B1 – B6), converged and merged 2026-10-31

**Input**: User description: "022 the pre-1.0 C ABI pass (the owner's road to 1.0, decisions.md 2026-10-31). As an app developer integrating the C API (directly or through the Swift package), I want every struct the library fills to carry a size field with one stated compatibility rule, and the two array-element structs to be declared frozen with a start-up check, so that a later library can add a field to a report without breaking my binary, a stale binary is refused clearly instead of being written past its struct, and this is the last ABI break before 1.0."

## Decisions for sign-off

The owner reads this section first. Each decision gives the recommendation; the alternative is in the Assumptions section. The three draft reviews (coach 1 – 13, hardware B1 – B11, product M1 – M8) are applied at their defaults below and listed under Clarifications. **D8 – D12 came from the draft reviews; D13 from the plan reviews.**

| # | Decision | Recommended |
|---|---|---|
| D1 | **The eleven out-structs gain a size field.** `RrDr60BlockInfo`, `RrDr60RecordInfo`, `RrDr60StopOutcome`, `RrDr60PlayInfo`, `RrDr60EngineState`, `RrDr60MemoryStatus`, `RrDr60SpeechCodecInfo`, `RrDr60ManifestHeader`, `RrDr60StageEntry`, `RrDr60Control` and `RrDr60ControlOption` get `uint32_t struct_size` as their first field under the **compatible prefix-fill rule** every sized struct already follows (spec 018): the caller sets it to `sizeof`; below the smallest layout ever published for that struct → refused with `INVALID_ARGUMENT`, nothing written; a known smaller layout → exactly its prefix; larger (up to the cap of D8) → the whole struct with the tail zeroed. This version's layout is the smallest ever published for each of the eleven. The sizes (64-bit): BlockInfo 32 → 40, RecordInfo 40 → 48, StopOutcome 80 → 88 (D9), PlayInfo 40 → 48, EngineState 16 → 20, MemoryStatus 40 → 48, SpeechCodecInfo 24 → 28, ManifestHeader 64 → 72, StageEntry 80 → 80 (the field fills an existing pad), Control 112 → 120, ControlOption 16 → 24. | **Size fields on all eleven** (alternative: the per-call reports only) |
| D2 | **The two array-element structs are frozen, and the change entry's data field is renamed.** `RrDr60VasEvent` (16 bytes) and `RrDr60ChangeEntry` (40 bytes) are passed as caller arrays with a capacity, read back by the record writer's arrays and written singly by `rr_dr60_record_event` / `rr_dr60_record_change`: a per-element size cannot work, so their layouts are declared **frozen** (a new field would be a new struct with new functions), their sizes published as defines (`RR_DR60_VAS_EVENT_SIZE` 16, `RR_DR60_CHANGE_ENTRY_SIZE` 40), and the single-element writers and the record writer's input arrays fall under the frozen rule. In this one rebuild `RrDr60ChangeEntry.reserved` is **renamed `stream`** (the stream word it has carried since 0.20; the same offset and bytes, a source change only). This deviates from decisions.md's 2026-10-31 row, which named the change entry for a size field. | **Frozen, sizes published, `stream` named** (alternative: a stride argument on the twenty-one array-taking functions) |
| D3 | **One ABI version and a start-up check, 64-bit only.** `RR_DR60_ABI_VERSION` (1), `rr_dr60_abi_version(void)` returning the library's value, and `rr_dr60_abi_check(abi_version, pointer_size, settings_size, vas_event_size, change_entry_size)` returning `OK` only when the version equals the library's, the pointer size is 8, the settings size is at most the library's, and the two frozen sizes match; otherwise the new status `ABI_MISMATCH` (37). The eleven sized structs carry pointer-sized fields, so the C ABI is declared **64-bit pointer width only** (iOS; Android `arm64-v8a` and `x86_64`). The ABI version moves only on a change no size field can express: a frozen struct's bytes, a function signature, an enum value's meaning, a field reorder, the settings' input rule; appending a status, a control, a tap, a function, a define or a field to a sized struct never moves it. | **A version, a getter and a five-argument check** (alternative: the size defines alone) |
| D4 | **One breaking commit, version 0.22.0, the last *planned* ABI break before 1.0.** A `feat!:` commit; **0.22 is a hard break for every binary compiled against 0.21 or earlier** (exactly like 0.13 and 0.20: rebuild once); the protection is for callers compiled against 0.22 and later. The promise is defined by its mechanism, not by faith: from 0.22 a sized struct only grows under its size field, the frozen pair never changes, new statuses, controls, taps, defines and functions are appended; anything else moves `RR_DR60_ABI_VERSION`, is announced as breaking, and none is planned before 1.0. | **As stated** |
| D13 | **The settings rule, corrected and tightened.** The spec and the owner's sign-off described `RrDr60Settings`' input rule backwards: the library refuses a settings struct *smaller* than its own (`struct_size < sizeof` at `rr_dr60_create`, `reconfigure`, `settings_validate`; the 0.12 precedent refused a stale 112) and today **accepts a larger one**, reading its own prefix and silently dropping a newer caller's fields. Decision: `rr_dr60_create` / `reconfigure` / `settings_validate` refuse any `struct_size` **different from** the library's; `rr_dr60_abi_check` requires `settings_size == sizeof(RrDr60Settings)`; and `RrDr60Settings` is named as **the one exception to the growth promise** (an input the library validates whole: appending a setting is a rebuild for every earlier caller, reported by the check at start). A settings growth after 0.22 **moves `RR_DR60_ABI_VERSION`**, so the check is the signal an app gets before `create` fails. | **Equality + the carve-out** (alternative: a real input prefix rule, filling a smaller settings struct's missing tail with the defaults; it needs "every appended setting defaults to the neutral value" as a standing constraint, which spec 012's on-by-default microphone stages already violate) |
| D5 | **Nothing else moves.** The `rr_dr60` core crate's public API is unchanged (the `rr_dr60_ffi` crate's Rust-visible structs gain the field, their `Default` setting it to the full layout); no audio change (the fifteen golden guards and every smoke hash unmoved); the stage and control manifests unchanged; no assumption (the device has no C API). One C-only property is lost: `RrDr60SpeechCodecInfo` no longer shares the Rust struct's layout (its fields are pinned by offset instead). | **As stated** |
| D6 | **Cost, stated analytically and reported.** No existing A/B mode runs through the C API, so the budget cannot be asserted there: the pass adds, per call, one 32-bit read and compare and a copy of at most 88 bytes with an optional tail fill (under 0.01 % of the per-sample cost at 4096-sample blocks, about 0.1 % at 64); the C smoke prints the per-call wall time of `rr_dr60_process` over a 48 kHz loop as a reported figure, and the existing `ab-timing` modes (`playback`, `record`, `bypass`) stay as a no-regression check of the Rust path. | **As stated** (alternative: a new C-API timing mode) |
| D7 | **Records.** The guide's C / Swift section (the rule, the frozen pair, the check, the rebuild, 64-bit only, the zero-initializer note, the example setting `struct_size`); the CHANGELOG under Changed as a breaking change with the sizes table and the promise in D4's wording; decisions.md (one row, correcting the 2026-10-31 row's change-entry item); brief.md's distribution line ("64-bit targets only"); `swift/README.md`; CLAUDE.md. | **As stated** |
| D8 | **A cap on `struct_size`.** A value above `RR_DR60_MAX_STRUCT_SIZE` (4096; no published layout approaches it) is refused with `INVALID_ARGUMENT` on **every** sized out-struct, the seven existing ones included (spec 018's "no upper cap" rule on `RrDr60Meters` and spec 020's five amended in this break). An uninitialized C stack struct, the ordinary C idiom, then yields a refusal in all but a vanishing share of cases instead of a multi-gigabyte zero fill inside the audio callback. | **A 4096-byte cap** (alternative: no cap, the garbage size undefined behaviour) |
| D9 | **No embedded growable struct: the stop outcome is flattened.** An embedded sized struct shifts every field after it when it grows, so the prefix rule cannot hold for either struct. `RrDr60StopOutcome` carries the flush's seven fields inline (`flush_stored`, `flush_events`, `flush_paused`, `flush_stopped`, `flush_stop_reason`, `flush_frames`, `flush_frames_total`) with no nested `RrDr60RecordInfo` (80 → 88 bytes). The two spec 020 containers keep their embeds under a stated rule: `RrDr60RecordReadout` keeps `settings` last (the settings struct grows at its end, so the read-out's tail grows with it) and `RrDr60StopFields`, embedded before it and in `RrDr60RunData`, is declared **frozen** in layout (a new form is a new record-form version). | **Flatten the outcome; freeze `StopFields`** (alternative: move the embed last and keep it sized) |
| D10 | **Every byte of a filled struct is defined.** The eleven layouts contain padding (after `struct_size` on 8-aligned structs, after the `bool` fields, at some tails); the library builds each value in a zeroed buffer and writes its fields, so the padding reads 0 and nothing of the library's stack reaches the caller. The `bool` fields stay `bool` (the sizes table holds). | **Zeroed padding by construction** (alternative: `bool` → `uint32_t`, changing four sizes) |
| D11 | **`struct_size` reads back as the size the library filled** (its own layout, or the known smaller prefix): a caller newer than the library then sees which prefix is meaningful and reads zeros beyond it as "unknown to this library". The two precedents disagreed (`rr_dr60_meters` echoed the caller's value, the spec 020 five wrote the library's); `RrDr60Meters` adopts the written-back size in this break (spec 018's contract amended). | **Write back the filled size** (alternative: echo the caller's) |
| D12 | **The check order on the per-call reports.** When `out_info` is non-NULL its `struct_size` is read and validated immediately after the handle's NULL check and before every other argument and state check; a refused size returns `INVALID_ARGUMENT` and writes nothing. On any later error the report is still written under the rule, as in 0.21 (`produced = 0`, the current `paused`; the tail zeroed when larger); with a NULL or poisoned handle the report is written only if its size is valid and the status stays the handle's. The constant-data functions check NULL, then the size, then the index. | **As stated** |

## Clarifications

### Session 2026-10-31 (owner sign-off)

- The owner approved D1 – D12 as recommended (the eleven size fields, the frozen pair with the `stream` rename, the five-argument check and 64-bit only, one `feat!:` as the last planned break, the cap, the flattened outcome, zeroed padding, the read-back, the check order). The rest proceeds to the plan sign-off and then to the merge, stopping only on a failed check or a new decision.

### Session 2026-10-31 (the owner's plan sign-off)

- The owner approved **D13 as recommended** (equality at `rr_dr60_create` / `reconfigure` / `settings_validate` and in the ABI check; `RrDr60Settings` the one exception to the growth promise; a settings growth moves `RR_DR60_ABI_VERSION`) and the plan (P1 – P9 with the three plan reviews applied). The next owner stop is the merge.

### Session 2026-10-31 (the draft reviews: coach 1 – 13, hardware B1 – B11, product M1 – M8)

- **"A stale binary is refused" was false for pre-0.22 callers** (hardware B1, coach 3): a 0.21 binary never writes `struct_size`, so the library reads whatever lies at offset 0; the pass is a hard break for every earlier binary and protects 0.22-and-later callers from future growth (D4 reworded). The cap (D8) turns the common garbage case into a refusal.
- **The per-call reports are written on every error path today** (coach 1, hardware B5): the check order is now stated (D12), and the "always written" exception becomes "when the size is accepted"; the early returns fill through the same rule.
- **The embedded `RecordInfo` in `StopOutcome`** (coach 2, hardware B3): flattened (D9); `StopFields` frozen; `RecordReadout` keeps `settings` last.
- **Padding** (hardware B4): zeroed by construction (D10); a C smoke test paints a struct with 0xAA and reads the padding positions as 0.
- **The read-back value** (coach 5, hardware B6): the filled size (D11), Meters amended.
- **The frozen pair also covers the single-element writers and the record writer's arrays** (coach 4); the deviation from decisions.md's row is stated in D2; `reserved` → `stream` (coach 10, hardware B7, product M4).
- **The ABI check** (coach 7, hardware B9, product M2): a getter, the settings size, the pointer size; 64-bit only, with the Android targets named.
- **The cost claim was vacuous** (hardware B2, coach 12): D6 restated as an analytic bound plus a reported smoke figure; the existing modes are a no-regression check only.
- **The Rust API claim scoped to the core crate** (coach 9); `SpeechCodecInfo`'s layout-equality assertion becomes offset pins (hardware B8).
- **The sizes are 64-bit sizes** (coach 6, hardware B10): each struct's "smallest published layout" is a per-struct constant computed from the build target's `size_of`, listed in the plan's table; the golden guards are fifteen (the bypass golden arrived with 021).
- **The Swift package's obligations named** (product M1, coach 8): a `.sized` initializer on every sized struct, `RRDR60.checkABI()` run once lazily before the first wrapper call and callable at launch, both test targets setting the size, the raw `RRDR60C` module's trap documented.
- **Success criteria tightened** (coach 11): SC-001 per-call reports at the measurement rates, the rest once; SC-002 the right tuple, each argument wrong alone, all wrong, version 0 and `UINT32_MAX`; the smoke's size table is new.
- **Considered, not adopted**: a numeric version accessor and a status-name function (product M-engine: a `rr_dr60_status_name` saves every app a 38-name table; additive, backlog) (additive at any time; backlog), folding the library version into the check (every patch release would mismatch), a typed Swift engine wrapper (its own feature after 0.90.0, designed with the app).

### Session 2026-10-31 (the post-implementation reviews: coach 1 – 10, hardware B1 – B6)

- **D10 had one violation** the first gate could not see: the read-out's embedded settings were assigned as a whole struct inside a filler, so fifteen padding bytes (its own four and the settings' four runs) carried stack bytes in a debug build; fixed field by field, the padding tables derived from the field offsets (which also found `RrDr60StageEntry`'s pad at 52), the source scan reading the fillers, Miri running the real fill. **SC-004 restated**: the Miri run is no longer one-off; a nightly `miri` CI job runs the helper's tests on every PR (the hardware review's Q1, at its default).
- The Swift entry points **throw** the mismatch (US2 AS3 as written; the first implementation returned `nil`, indistinguishable from an unreadable record); the fallback reason `.libraryRefused(status:)` names a library refusal with every header comparison holding (the library's sizes are not exposed; a getter is backlog). The three read-side structs of spec 020 join the smoke and the parity (fifteen covered, as the plan said). The read-out input takes the cap; the header diff is checked line by line; the per-call matrix gains its poisoned stop/play and playback-in-progress rows; "2.1 ns" is the whole idle call, an upper bound; a 32-bit target fails to build rather than being "refused".

### Session 2026-10-31 (the plan reviews: coach 1 – 11, hardware B1 – B9, product M1 – M9)

- **The settings rule was described backwards** (hardware B1, Critical): the code refuses a *smaller* settings struct and accepts a larger one; D1's edge case, D3's "at most" and the assumption said the opposite, and D4's promise did not hold for the one struct every app passes first. **D13** (approved at the recommendation): equality at `create` and in the check, settings carved out of the growth promise, a settings growth moves the ABI version; AS2 and the create test take one above and one below.
- **The counts** (coach 3): fifteen out-structs through the helper, three inputs with their own read rules (`RunData` gets the cap on read; `StopFields` frozen, its size ignored on read as today, written as 40); the "spec 020 five writers" are three (two cited lines were a unit test).
- **The helper** (coach 2, hardware B3, B4): takes a per-struct ascending list of published layouts ending in `size_of`, fills the largest ≤ the caller's size and writes it back (`[56, 64]` for Meters, `[SIZE]` elsewhere); `build` assigns fields into a zeroed `MaybeUninit` buffer and never assigns the whole struct (a typed copy drops padding in the abstract machine); the byte copy reads the buffer; the size is read by `struct_size_at` (never `(*out).struct_size`). A one-off Miri run gates it (SC-004). `Default` on the fifteen is a manual zeroed value plus the size, and a source-scan test forbids `derive(Default)` and any whole-struct write to them (coach 5).
- **A pre-existing uninitialized-padding copy** (hardware B2): `RrDr60RecordReadout` has four padding bytes at 212 – 215 that today's `fill_prefix` copies from a typed value; the helper closes it; the painted check adds the readout's offsets (and Meters as a negative control).
- **The embedded settings in a readout prefix** (coach 4): its `struct_size` is the settings bytes the prefix holds (a post-fill fix-up), pinned by a doctored-layouts test.
- **The check order for a poisoned handle** (coach 1, 6; hardware B5): the handle's status wins with a bad size (nothing written), the idle report with a valid one; the three handle-taking status functions keep 0.18's order; the test matrix adds poisoned, `RecordingInProgress`, `NotRecording`, overlap × four sizes, and the engine's position / meters / memory unchanged across a refusal (coach 11).
- **The cost figure** (coach 7, hardware B6): "below 1 % at 64-sample blocks"; the smoke prints the 64, the 4096 and the `frames == 0` figures.
- **The gates** (coach 8, 9): the traceability audit gains the FFI crate's file for the new literals; SC-003's oracle is a `git diff --quiet` of the core crates; the header's `uintptr_t` for `usize`.
- **The defines** (coach 10): the four are literal consts with const assertions; the `size_of` pins stay private.
- **The parity's input** (coach 11): `smoke --abi` diffed against `examples/abi_rows.rs`, which the Swift parity test reads.
- **The lead message** (product M1, M6, M9): "rebuild once **and initialize every report**": the size fields are a run-time break for a caller that compiles unchanged; a migration block (M2); what a mismatch means (M3: a stale XCFramework or a mixed build, never a user condition: stop in debug, "this build is broken" in release; the Swift error carries the reason); the helpers stay in the `RRDR60` target and the README says to import it (M5); the "never" statements restated by their mechanism (M7); 64-bit on Android with the reason (M8); the records gain spec 001's contract, the guide's stale sentences and the README row (M4).
- **Swift** (hardware B9): `.sized` on the seventeen, never on `RrDr60Settings` (a zeroed settings is refused; it comes from `rr_dr60_settings_default`); `MemoryLayout<T>.size == sizeof` verified for the tail-padded shapes.

## Overview

The C API has grown struct by struct since spec 001. Seven structs carry a size field with one compatibility rule, and that rule has let later versions add fields to `RrDr60Settings` and `RrDr60Meters` without breaking earlier callers. Thirteen structs carry none: a field appended to one of them silently breaks every binary compiled against the earlier layout, which happened twice (`RrDr60PlayInfo` in 0.13, `RrDr60StopOutcome` in 0.20) and was noted each time as "the pre-1.0 ABI pass". This feature is that pass: the same rule on every struct the library fills, a cap that turns an uninitialized size into a refusal, no embedded struct that could shift its container, the two array-element structs declared frozen, one ABI version an app can check at start, and one last rebuild for every caller. 0.22 itself breaks every earlier binary; from 0.22 on a report can grow without breaking an app, a caller newer than the library is refused instead of overwritten, and the C surface is ready to be frozen at 1.0.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A report can grow without breaking my app (Priority: P1)

An app developer compiles against this version, fills every struct through the library's calls, and keeps working unchanged when a later version appends a field to a report: the new library fills the prefix the app knows and refuses an app whose struct is smaller than anything it ever published.

**Why this priority**: it is the pass's purpose.

**Independent Test**: for each of the eleven structs, a call with `struct_size` below this version's size is refused with `INVALID_ARGUMENT` and writes nothing; with the exact size the struct is filled, its padding zero and its `struct_size` the filled size; with a larger size up to the cap the struct is filled and the tail bytes are zero; above the cap refused; the Rust and C paths agree field for field.

**Acceptance Scenarios**:

1. **Given** any of the eleven structs, **When** a call is made with `struct_size` smaller than this version's size (0 included), **Then** the status is `INVALID_ARGUMENT`, no byte of the caller's struct is written, and the engine's state is unchanged (a processing call with a refused report processes nothing: the stream's position, the memory and the meters are untouched).
2. **Given** the exact size, **When** the call is made, **Then** every field equals the Rust path's value, every padding byte is 0 (a struct painted with 0xAA beforehand), `struct_size` reads back as this version's size, and the call behaves as in 0.21.
3. **Given** a larger size up to `RR_DR60_MAX_STRUCT_SIZE` (the struct followed by sentinel bytes), **When** the call is made, **Then** the struct is filled, `struct_size` reads back as this version's size, the bytes beyond it up to the caller's size are zero, and nothing beyond `struct_size` is touched.
4. **Given** a size above the cap (4097 and `UINT32_MAX`), **When** the call is made, **Then** the status is `INVALID_ARGUMENT` and nothing is written; on `RrDr60Meters` and the spec 020 five the same.
5. **Given** `RrDr60StopOutcome`, **When** filled, **Then** its seven flush fields equal the Rust outcome's flush report and no nested struct exists.
6. **Given** a per-call report and a NULL or poisoned handle, an overlapping buffer, or `frames == 0`, **When** called with a valid size, **Then** the report is written as in 0.21 (`produced = 0`, the current `paused`, the tail zeroed when larger) and the status is the error's; with an invalid size and a NULL handle nothing is written and the status is `NULL_POINTER`.

---

### User Story 2 - A stale app is told, once, at start (Priority: P2)

An app calls the ABI check once when it loads, with the version it was compiled against, its pointer size, its settings size and the sizes of the two frozen structs; a library that cannot serve it says so before any audio runs.

**Independent Test**: the check returns `OK` with this version and the published sizes, `ABI_MISMATCH` for each argument wrong alone and for all wrong, and never touches the engine.

**Acceptance Scenarios**:

1. **Given** `RR_DR60_ABI_VERSION`, 8, `sizeof(RrDr60Settings)` and the two frozen `sizeof`s, **When** `rr_dr60_abi_check` is called, **Then** it returns `OK`, and `rr_dr60_abi_version()` returns 1.
2. **Given** each argument wrong alone (version 0, 2 and `UINT32_MAX`; pointer size 4; a settings size one above the library's; a frozen size ± 1), and all wrong together, **When** called, **Then** it returns `ABI_MISMATCH` (status 37) and a live engine's schedule, log and meters are unchanged across the call; a settings size *below* the library's passes (an older caller the settings rule serves).
3. **Given** the Swift package, **When** its `RRDR60.checkABI()` runs (at launch by the app, and lazily before the first wrapper call), **Then** a mismatch surfaces as an error before any engine call; every sized struct has a `.sized` initializer the wrappers and both test targets use; the parity test compares the eleven structs through `.sized` with the C smoke's values and the two frozen sizes with the defines.
4. **Given** the two frozen structs, **When** their sizes are read, **Then** they equal the published defines (16 and 40) on every target, pinned by const assertions, and `RrDr60ChangeEntry.stream` is the former `reserved` at the same offset.

---

### User Story 3 - Nothing else moved (Priority: P3)

A maintainer checks that the pass changed only the C surface: every golden guard and smoke hash unmoved, the core Rust API identical, the cost as stated.

**Acceptance Scenarios**:

1. **Given** the fifteen golden guards and every smoke hash, **When** run, **Then** they are unmoved.
2. **Given** the `rr_dr60` core crate's public API, **When** diffed against 0.21, **Then** nothing changed; the `rr_dr60_ffi` crate's Rust structs differ by the size field (and `Default` sets it).
3. **Given** the existing A/B modes against 0.21, **When** run on the label, **Then** every path is within its budget (a no-regression check of the Rust path); the C smoke prints the per-call figure.
4. **Given** the header, **When** regenerated, **Then** it differs from 0.21 by the eleven size fields, the flattened outcome, the `stream` rename, the four defines, the version getter, the check function and the status.

---

### Edge Cases

- `struct_size` = 0 (a zero-initialized struct, which is what Swift's default initializer produces): refused with `INVALID_ARGUMENT`; the guide and the Swift wrappers say so.
- A `struct_size` above the cap: refused (D8); an uninitialized C stack struct reads garbage there and is refused in all but a vanishing share of cases; the guide's rebuild note tells C callers to initialize every report (`= { .struct_size = sizeof x }`).
- A refused size on a per-call report: checked before any sample, so the stream's position, the memory and the meters are unchanged.
- The constant-data structs hold string pointers: the prefix-fill copies the pointers as they are; a zeroed tail beyond the filled size holds NULL pointers, which the guide documents as "fields this library does not know".
- A caller newer than the library (a 0.23 header against the 0.22 library): the library fills its own 0.22 layout, writes back 0.22's size, zeroes the rest; the settings struct is an input validated whole, so a `struct_size` different from the library's is refused at `rr_dr60_create` (D13; before 0.22 a larger one was accepted and its tail ignored) and the ABI check reports the settings size at start.
- A 32-bit target: the ABI check returns `ABI_MISMATCH` for pointer size 4; the sized structs' sizes are 64-bit sizes, the frozen pair's hold everywhere.
- The single-element writers `rr_dr60_record_event` / `rr_dr60_record_change` keep their `bool` return and the frozen layouts.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The eleven out-structs MUST carry `uint32_t struct_size` as their first field and MUST follow the compatible prefix-fill rule: below the smallest published layout refused with `INVALID_ARGUMENT` and nothing written, a known smaller layout filled as its prefix, larger filled with a zeroed tail, above `RR_DR60_MAX_STRUCT_SIZE` (4096) refused; every refusal MUST happen before any processing; `struct_size` MUST read back as the size the library filled; every byte of a filled struct MUST be defined (padding zero). The cap and the read-back rule apply to the four existing out-structs too (`RrDr60Meters`, `RrDr60RecordReadout`, `RrDr60InputAlignment`, `RrDr60PlaceState`; spec 018's and 020's contracts amended), and the cap to the `RrDr60RunData` input on read; in a prefix fill of `RrDr60RecordReadout` the embedded `settings.struct_size` MUST be the settings bytes the prefix holds.
- **FR-002**: `RrDr60VasEvent` and `RrDr60ChangeEntry` MUST stay byte-identical to 0.21 and be documented as frozen (the single-element writers and the record writer's input arrays included); their sizes MUST be published as defines and pinned by const assertions; `RrDr60ChangeEntry.reserved` MUST be renamed `stream`.
- **FR-003**: `RR_DR60_ABI_VERSION` (1), `rr_dr60_abi_version()` and `rr_dr60_abi_check(abi_version, pointer_size, settings_size, vas_event_size, change_entry_size)` MUST be published; the check MUST return `OK` only for the library's version, pointer size 8, a settings size equal to the library's (D13) and the two frozen sizes, `ABI_MISMATCH` (37) otherwise, touching no engine state; the C ABI MUST be documented as 64-bit pointer width only.
- **FR-004**: `RrDr60StopOutcome` MUST carry the flush's fields inline with no nested struct; `RrDr60StopFields` MUST be documented as frozen; `RrDr60RecordReadout` MUST keep `settings` as its last field.
- **FR-005**: The per-call reports' check order MUST be: the size read and validated first; with a NULL or poisoned handle the handle's status is returned and the idle report written only when the size is valid; otherwise a refused size returns `INVALID_ARGUMENT` before any other check and processes nothing; on a later error the report is written under the rule. The constant-data functions check NULL, then the size, then the index; the handle-taking status functions (`rr_dr60_get_state`, `rr_dr60_memory_status`, `rr_dr60_meters`) keep 0.18's order: the handle, then NULL out, then the size.
- **FR-006**: The `rr_dr60` core crate's public API MUST be unchanged; the fifteen golden guards and every smoke hash MUST be unmoved; the stage and control manifests MUST be unchanged; no assumption MUST be added.
- **FR-007**: Every struct's size MUST be pinned by a const assertion against the build target's `size_of` and checked by the C smoke's new size table (64-bit); the C smoke MUST exercise the cases of US1 AS1 – AS4 for each of the eleven structs and the painted-padding check; the harness's parity test MUST compare the Rust and C paths for each.
- **FR-008**: The Swift package MUST provide a `.sized` initializer on every sized struct, `RRDR60.checkABI()` (run once lazily before the first wrapper call and callable at launch), both test targets MUST set the size, the parity test MUST cover the eleven structs and the frozen defines, and `swift/README.md` MUST state the raw module's zero-initializer trap.
- **FR-009**: The cost MUST be stated analytically in the records (below 1 % at 64-sample blocks) and the C smoke MUST print, as reported figures, the per-call wall time of `rr_dr60_process` at 64 and 4096 samples and of the `frames == 0` path (the size check and the idle write, the pass's added work); the existing `ab-timing` modes MUST stay within budget as a no-regression check.
- **FR-010**: The records: the guide's C / Swift section (the rule, the cap, the frozen pair, the check and what a mismatch means to an app, the rebuild **and the initialize-every-report edit, a run-time break**, a migration block, 64-bit only with `wasm32` and 32-bit Android named as refused, the zero-initializer note, the example setting `struct_size`) plus the guide's two stale sentences (controls "no size field", `PlayInfo` 0.13) and the record example's `flush_*` names, spec 001's C contract rules 2 and 5 amended with dated notes, the README's platforms row, the CHANGELOG under Changed as a breaking change with the sizes table and the promise in D4's wording, decisions.md's row (correcting the change-entry item of the 2026-10-31 row), brief.md's distribution line, `swift/README.md`, CLAUDE.md; version 0.22.0.

### Out of Scope

- Any change to the `rr_dr60` core crate's public API; any new field beyond the inserted size fields and the flattened flush; field reordering or packing beyond D9.
- The 1.0 release itself and the freeze it brings.
- A stride argument on the array-taking functions; a C-API timing mode; a numeric version accessor; a status-name function; a typed Swift engine wrapper (backlog).

### Key Entities

- **Sized struct**: a C struct whose first field is its caller-set size, filled under the prefix-fill rule with the cap, the read-back and the zeroed padding.
- **Frozen struct**: a C struct whose layout is promised never to change; a new form is a new type.
- **ABI version**: one integer that moves only on a change no size field can express.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All eleven structs pass US1 AS1 – AS4 and the painted-padding check in the C smoke and the parity test (the per-call reports at the measurement rates, the rest once); the two frozen sizes and every struct size are pinned on the build target.
- **SC-002**: The ABI check returns `OK` for the right tuple and `ABI_MISMATCH` for each argument wrong alone (the settings size one above **and** one below), for all wrong, for version 0 and `UINT32_MAX`; `rr_dr60_create` refuses a settings size one above and one below; the engine's schedule, log and meters unchanged across the call.
- **SC-003**: The fifteen golden guards and every smoke hash unmoved; `git diff --quiet 81f14dd -- crates/rr_dr60 crates/rr_dr60_detmath` empty; the header diff limited to the listed additions and the flattening.
- **SC-004**: The existing A/B modes within budget on the runner; the smoke's three per-call figures printed; a one-off Miri run of the helper's unit tests (the readout case included) clean, recorded in research.
- **SC-005**: The Swift suites green with `.sized` and `checkABI()`; coverage at or above 80 % and within 1 point of the baseline (0.21: 96.64 % lines locally, the gate figure of PR #53).

## Assumptions

- **Alternatives considered**: size fields on the per-call reports only (D1: one rule is easier to state than two); a stride argument on the array-taking functions (D2: twenty-one signatures and a per-call cost for structs that have not changed since their specs); keeping the name `reserved` (D2: a frozen struct with its data field misnamed forever); the size defines alone without a version (D3: a signature or meaning change would have no signal); several breaking commits (D4: one rebuild for every caller); a new C-API timing mode (D6: a 32-bit read and an 88-byte copy per call is below any budget's resolution); no cap (D8: a multi-gigabyte fill in the audio callback on an uninitialized struct); moving the embed last and keeping it sized (D9: the inner size has no consistent meaning inside an outer prefix); `bool` → `uint32_t` (D10: four sizes change for no gain once the padding is zeroed); echoing the caller's size (D11: a newer caller cannot tell a real zero from an unknown field).
- **The prefix-fill rule is spec 018's**, with the cap and the read-back added in this break; the settings struct is an input validated whole (D13: `struct_size` equal to the library's, else refused; before 0.22 only a smaller one was refused).
- **Fifteen structs are filled by the library** (the eleven, `RrDr60Meters`, `RrDr60RecordReadout`, `RrDr60InputAlignment`, `RrDr60PlaceState`) and three are inputs (`RrDr60Settings`, `RrDr60RunData`, `RrDr60StopFields`); the draft's "eighteen sized structs" counted both kinds (coach 3).
- **Swift's default initializer zeroes a C struct**, so the wrappers must set `struct_size`; a zero is refused by design, never silently filled.
- **The sizes are 64-bit sizes** (the reports carry pointer-sized counts); the frozen pair's are target-independent.
- **No device claim**: the C API is the emulator's surface; nothing here describes the device.
