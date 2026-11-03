# Feature Specification: The public re-render

**Feature Branch**: `027-public-re-render`

**Created**: 2026-11-03

**Status**: Draft reviewed 2026-11-03 (coach 1 – 13, hardware B1 – B10, product M1 – M9, applied at the defaults below); clarified 2026-11-03 (four questions, every answer the recommended one). **Spec approved by the owner 2026-11-03 (D1 – D9).** Plan reviewed and **approved by the owner 2026-11-03** (D7's direct-write fill confirmed). Tasks next.

**Input**: Engine issue #76, filed by the desktop workbench (rr-dr60-workbench, feature 005): spec 020's processing record is re-rendered by a recipe that lives only in the unpublished harness (`record_contract_checks::re_render_checked`, `filler`, `fill_to_remaining`) and in the Swift package (`ProcessingRecord.reRender`, `fill`). A Rust host has no public way to re-render a record and must mirror the recipe: an engine at the record's counter, a fill by import of a filler made on a throwaway engine, the slot and its store, the schedule refilled below 64 pending, the feed up to the stop position, the stop rule (no stop call after a self-stop), the digest of the last item. Each mirror can drift from the engine. Spec 026's clarification moved this here and named the app's need: **the Rebuild** (re-process a recording's kept microphone audio with changed settings) needs the rebuilt stored form and a new record, not only a digest.

Everything is appended (version 0.94.0); a host built on 0.93 runs unchanged.

## Clarifications

### Session 2026-11-03

- Q: A Rebuild whose worst-case pause count is huge (an edited hang near zero): refuse above a cap, allocate whatever it needs, or make Rebuilds digest-only? → A: **Refuse above the cap** (16 Mi events, `RunTooLarge`); the host can set `collect_run: false` for a digest-only session (D5).
- Q: "Listen here" on a past recording (a tap's audio during a re-render): in 027 or a later feature? → A: **A later feature** (Assumptions, out of scope).
- Q: Keep the `f32` feed beside the 24-bit feed point? → A: **Keep both**, one kind per session (D3).
- Q: Which engine does app 1.0 pin when the app resumes? → A: **The first release that contains 027** (a product decision recorded here; it changes nothing in 027).

### Session 2026-11-03 (the plan reviews: restatements for the plan sign-off)

- **D7's mechanism** (coach plan 1, hardware P4): the fill writes an item shaped like an import directly into the memory (zeros with the codec bypassed; with it on, one frame the speech encoder emits for silence from its rest state, repeated): the same span, the same absent slot block, counted against the 99 items, no throwaway engine, no input vector, no exported bytes. A fill of zero units adds no item. D7's intent (no single input vector, content unspecified, judged by behaviour) holds; "built by recording silence in fixed blocks" is replaced. **Confirmed by the owner at the plan sign-off, 2026-11-03.**
- **D5's empty outcome** (hardware P5): a Rebuild that keeps nothing cannot occur through the engine (the gate starts open, so any fed input keeps a sample), and a remaining capacity of 0 is refused at `new` (`MemoryFull`, the recorder's own refusal). `Reason::DiscardedEmpty` stays as a defensive value; US2 AS6 tests the refusal.
- **D4's per-position limit** (coach plan 3): an edit that would put more than 64 rows at one position is refused at the edit (`ScheduleFull`), since the engine could never land them at one sample.
- **D6's refusals at `new`** are named: imported origin `Error::ImportedRecord`, an unknown component `Error::UnknownComponent` (coach plan 2).
- **`finish` keeps the session** (hardware P6): after `InputShort` the host feeds the rest and calls `finish` again; `finish` is `&mut self`.
- **The outcome carries `rows_landed`** (hardware P3): the rows the engine applied; a faithful plan lands every row (asserted), an edited plan's rows the engine never reaches (past the input, after an earlier self-stop) are counted out.
- **The meters** read the last engine call, at most 4096 host samples (hardware P8, coach plan 4).
- **US4 AS4's mutants** become: no split; a stop call after a self-stop; the fill rounded up; the store left at its defaults; `feed` converting by `x / (2²³ − 1)`; one event dropped (hardware P1, P2, P11): the order of store and component is equivalent at the recording's start.

### Session 2026-11-03 (restatement)

- **Timing** (coach 12): spec 026 placed this feature after app 1.0, with app 1.0 pinned to v0.92.0. The owner changed that on 2026-11-03: the app's first release is delayed for non-technical reasons, so the planned engine work proceeds now and 027 lands before the app. Which engine version app 1.0 pins is decided when the app resumes.

## Decisions for sign-off

| # | Decision | Recommended |
|---|---|---|
| D1 | **One recipe, in the core** (#76). `rr_dr60::rerender` holds the recipe: the engine built at the record's host rate, settings, seed and counter; the memory filled by import to the record's remaining capacity at the start; every published component's stored parameters at its `parameter_offset + i`, then the slot's component (the Swift loop's rule; hardware B8: the harness restores only the first four values, which covers today's one component); the record's change rows scheduled on the record stream; the kept microphone samples fed up to the stop's input position; the stop; the last item's digest. The harness's recipe stays as the **independent oracle** (not rewritten to call the core). The Swift `reRender` becomes a wrapper of the C session; its own copy of the recipe is deleted. | **Core recipe; harness kept as the oracle; Swift wraps C** |
| D2 | **A streaming session.** `ReRender::new(plan) → Result<ReRender, Error>` builds the engine, fills the memory and reserves every buffer (allocates; idle work with no progress figure of its own, its duration and peak memory reported, US3); `feed(&mut self, samples) → Result<Fed, Error>` takes any block length and returns the samples taken and whether the stop position or a self-stop is reached; `finish(self) → Result<Outcome, Error>`. Progress is the input position against the stop position; cancelling is dropping the session. **No call allocates after `new`.** **Rows are never late** (hardware B1): when the schedule is full and rows remain, `feed` processes only up to the next unscheduled row's position, then tops the schedule up, so any block length gives the same result; before the stop, every remaining row the schedule can hold is scheduled (rows may land in the stop's flush). Rows at one position keep their record order. A one-call `re_render(plan, samples)` convenience is offered in Rust and Swift (C hosts stream). `ReRender::meters()` returns the engine's meter read-out after each `feed` (spec 016; product M5). | **Streaming, split-and-top-up, allocation-free after `new`** |
| D3 | **The input is the 24-bit feed point** (spec 020's rule). `feed` takes `i24` codes as `i32` and converts with `input::samples_from_i24`; a code outside −2²³ … 2²³ − 1 refuses the call with nothing taken (`InvalidSample`). `feed_f32` takes engine samples for a host that kept them losslessly (the workbench); it re-renders identically only for samples equal to `samples_from_i24` of the original codes, which the guide says. One session uses one of the two; mixing is refused. | **i24 primary; f32 for the workbench; no mixing** |
| D4 | **The plan and its edits.** `Plan::from_record(bytes)` reads the record (spec 020's reader; a record row on any stream but the record stream is a malformed run and refused here). The plan is editable before `new`: the settings (validated as `reconfigure` validates them), the slot's component and the component store (by parameter index), the change rows (an edited row is validated as `schedule_change` validates it; the list is kept sorted, stable for equal positions), the seed and the counter, the remaining capacity. **Refused edits** (hardware B5): the host rate (it changes what the input means); a remaining capacity above the capacity. A form-1 record (0.20 – 0.23) has no slot lines: its plan has an empty slot and the default store, flagged (hardware B10). An unedited plan is the faithful re-render. | **An editable plan, the engine's own validation, the identity-breaking edits refused** |
| D5 | **The outcome.** `finish` returns: the rebuilt item's digest; **`edited`** (the plan's values differ from `from_record`'s, a field compare, not "an edit call was made"); **`matches_original`** (the digest equals the record's `recording_digest`; always computed); the **verdict**, `Identical` / `Differs` for an unedited plan and `Rebuilt` for an edited one, with `matches_original` beside it (an edit the input never reaches rebuilds the same item; product M2, coach 4, hardware B6); the read-out's flags carried (library or stage-model version differs, form 1, unknown names); the remaining capacity actually reached by the fill (with the codec on it floors to whole frames); and the rebuilt item: its stored form (`export_len` / `export`) and **a new processing record** written from the session's own run (its events, change entries and stop fields, collected during `feed`, in buffers reserved at `new` from the engine's event bound at the stop position with the plan's shortest hang; a plan whose bound exceeds 16 Mi events is refused at `new` with `RunTooLarge`; `collect_run: false` in the plan gives a digest-only session that reserves nothing for the run; hardware B2, coach 2). For an unedited plan at the record's library version the new record equals the original byte for byte. **A Rebuild that keeps nothing** (with the codec on, `DiscardedEmpty`) returns an outcome with no item, no digest and no record, and that reason; not an error (hardware B7). The verdict describes processing only: it does not vouch for what the input contains (spec 020's rule); the guide carries one sentence per verdict (product M6). The new record does not name the record it came from (no record-form change; the case file links them). | **Digest, edited, matches_original, verdict, flags, item and new record; the empty outcome** |
| D6 | **One stop rule and the refusals.** For every plan: `feed` stops taking input at a self-stop, and `finish` issues the stop call unless the session self-stopped (coach 3; the Swift rule), so a Rebuild that self-stops earlier or later than the original ends where the engine ends it. Refused at `Plan::from_record`: what spec 020's reader refuses, and a malformed run. Refused at `new`: a record of imported origin (no run to replay), a component this library does not know, settings or rows that do not validate, `RunTooLarge`. Refused at `finish`: an input that ended before the stop position (`InputShort`, with the samples still needed) unless the session self-stopped. Input past the stop position is not taken. **Version differences are flags, not refusals**; the comparison says whether it matched. | **One stop rule; refuse what cannot render; flag versions** |
| D7 | **`Pipeline::fill_to_remaining(remaining)` public** (#76). It fills the memory by import so the remaining capacity equals `remaining` device samples, floored to the stored unit, idle only (refused while recording or playing, or when the memory is already fuller). The filler is built by recording silence in fixed blocks, never one input vector (hardware B3, coach 5); its content is unspecified (it never enters a later recording's digest: only the remaining capacity is written to the slot block). The oracle is behaviour: an equal remaining capacity and an equal digest for a recording made after the fill, against the harness's fill. | **Public, idle only, chunked, judged by behaviour** |
| D8 | **C and Swift.** C: two opaque handles, `RrDr60ReRenderPlan` (`rr_dr60_rerender_plan_from_record`, edits by control index with spec 025's readings, by parameter index for the store and by row for the changes, `rr_dr60_rerender_plan_destroy`; consumed by `new`) and `RrDr60ReRender` (`rr_dr60_rerender_new`, `_feed_i24`, `_feed_f32`, `_progress`, `_meters`, `_finish` into a sized `RrDr60ReRenderOutcome`, the item's `_export_len`/`_export` and `_record_len`/`_record_write`, `_destroy`); new statuses for `InputShort`, `InvalidSample`, `RunTooLarge` and the refusals; appended under spec 022's rules; the promise against 12a3c45, f5a0245, 97bbdc2, 2a170d4 and 3890bfe (0.93's merge). Swift: `ProcessingRecord.reRender` keeps its signature and returns the digest through the session; a new `ReRenderSession` exposes the plan, the streaming calls, the meters, the outcome and the item. | **Two C handles; Swift wraps them** |
| D9 | **Not a device behavior.** No assumption, no model version, no audio change (the sixteen golden guards and every smoke hash unmoved); the control and stage manifests unchanged but the version line. | **An emulator facility** |

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A host re-renders a record faithfully (Priority: P1)

A host holding a recording's processing record and its kept 24-bit microphone audio gets back the same recording, bit for bit, through one public engine facility, and is told so.

**Independent Test**: the core's session against the harness's independent recipe and the recording's own digest and record, at seven rates.

1. **Given** spec 020's sixteen cells (codec × changes × drop/mute × stop call/self-stop), **When** each record and input are re-rendered through the session, **Then** the digest equals the recording's and the harness recipe's, `matches_original` is true and the verdict is `Identical` (`us1_rerender::as1_sixteen_cells`).
2. **Given** spec 024's slot cells (the band filter; a non-default store with an empty slot) and a committed form-1 record, **Then** the same holds; the form-1 record is flagged (`us1_rerender::as2_slot_and_form_1`).
3. **Given** a session fed in blocks of 1, 64, 4096 and an irregular schedule, **Then** every partition gives the same digest and the same new record (`us1_rerender::as3_partitions`).
4. **Given** a burst added to the input of a cell, and the stop moved one frame earlier in the record, **Then** the verdict is `Differs` (`us1_rerender::as4_differs`).
5. **Given** more than 64 rows packed inside one 4096-sample block, fed at 4096 and at 1, **Then** both read `Identical`, and the harness recipe at block 4096 does not (the test's teeth; hardware B1) (`us1_rerender::as5_dense_rows`).
6. **Given** an unedited plan at the record's library version, **Then** the new record equals the original record byte for byte (the events and change rows, which the digest does not cover; coach 1) (`us1_rerender::as6_record_bytes`).

### User Story 2 - A host rebuilds a recording with changed settings (Priority: P1)

A host takes a record, changes a setting (a knee, the sensitivity, the band filter's frequency, a change row), and gets a new recording with its own stored form and record, which itself re-renders identically.

**Independent Test**: the rebuilt item against the harness recipe with the same delta and against a direct recording at the edited settings; its new record's own re-render.

1. **Given** a plan with one edit (the knee to −30 dBFS; the band filter's frequency; one row moved; the seed), **When** it is rebuilt, **Then** its digest equals the harness's `re_render(…, Delta::{Knee, ParameterMoved, EntryMoved, Seed})` and a direct recording at the edited settings, `edited` is true and the verdict is `Rebuilt` with `matches_original` false (`us2_rebuild::as1_equals_direct`).
2. **Given** the rebuilt item's new record and the same input, **Then** a faithful re-render of it is `Identical` (`us2_rebuild::as2_record_round_trip`).
3. **Given** edits that do not validate (a knee out of range, a row with an unknown control or another stream, the host rate, a remaining capacity above the capacity), **Then** each is refused with the engine's own error, nothing built (`us2_rebuild::as3_refusals`).
4. **Given** a smaller and a larger capacity than the original's, **Then** the rebuild self-stops earlier, or does not self-stop where the original did, and ends where the engine ends it (`us2_rebuild::as4_self_stop_both_ways`).
5. **Given** an edit the input never reaches (a playback-side setting; a same-value edit), **Then** `edited` follows the field compare and `matches_original` is true (`us2_rebuild::as5_unreached_edit`).
6. **Given** a plan with a remaining capacity of 0, **Then** `new` refuses with `MemoryFull` (an empty Rebuild cannot occur; restated, see Clarifications) (`us2_rebuild::as6_empty`).

### User Story 3 - A host streams, shows progress and cancels (Priority: P2)

A host re-renders an hour-long recording without holding the whole input in memory, shows progress, and can stop early.

**Independent Test**: allocation counts, the cost against the record path, the progress figure and the early drop.

1. **Given** a session, **Then** `feed`, `meters` and `finish` allocate nothing (the allocation counter), and `new`'s peak memory is reported per encoding (`us3_stream::as1_alloc_free`).
2. **Given** a 60-minute recording at 48 kHz (release only), **Then** the re-render reads `Identical`, `feed`'s cost per host sample is within 1.5× of the record path's own figure (at least 20× real time), and `new` on a nearly full memory is timed separately (`us3_stream::as2_hour`).
3. **Given** a session dropped half-way, **Then** nothing leaks and a new session on the same record still reads `Identical` (`us3_stream::as3_cancel`).
4. **Given** an input that ends before the stop position, **Then** `finish` refuses with `InputShort` and the samples still needed (a session-only check: the harness recipe feeds what it has) (`us3_stream::as4_input_short`).
5. **Given** an out-of-range i24 code, mixed `feed` and `feed_f32`, input past the stop position, a plan whose event bound exceeds the cap, and `collect_run: false`, **Then** each behaves as D2, D3, D5 and D6 say (`us3_stream::as5_feed_edges`).

### User Story 4 - One recipe on every surface (Priority: P2)

Rust, C and Swift re-render through the same engine code.

**Independent Test**: parity, the source and the fill.

1. **Given** the sixteen cells, **Then** the C session and the Swift `ReRenderSession` give the Rust digest, outcome, stored form and new record byte for byte (`ffi_parity rerender`, `swift test ReRenderTests`).
2. **Given** the Swift package's sources, **Then** no fill, schedule refill or stop rule remains in Swift (a source scan) (`us4_surfaces::as2_no_swift_recipe`).
3. **Given** `Pipeline::fill_to_remaining` and the harness's fill on both encodings, **Then** the remaining capacities and the digests of a recording made afterwards are equal, two different fillers give the same digest, and the call refuses while active or when fuller (`us4_surfaces::as3_fill`).
4. **Given** six mutants (no refill split; a stop call after a self-stop; the fill rounded up; the store left at its defaults; `feed` converting by `x / (2²³ − 1)`; one event dropped from the run), **Then** each fails a named test (`mutation rerender::`) *(restated, see Clarifications)*.

### Edge Cases

- **A record of imported origin**: refused at `new`.
- **A record from another library version**: re-rendered and flagged; `matches_original` decides.
- **A recording that self-stopped**: no stop call; input past the stop position is not taken.
- **A Rebuild that keeps nothing**: the empty outcome (D5).
- **A delta that makes the memory self-stop at another place**: the engine's own stop (D6).
- **Two rows at one position**: applied in the record's order.

## Requirements *(mandatory)*

- **FR-001**: D1's core recipe, matching the harness recipe on every cell (`us1_rerender`).
- **FR-002**: D2's streaming session, split-and-top-up, allocation-free after `new`, with the meters (`us1_rerender::as3_partitions`, `as5_dense_rows`, `us3_stream`).
- **FR-003**: D3's feed point (`us1_rerender`, `us3_stream::as5_feed_edges`).
- **FR-004**: D4's editable plan with the engine's own validation and the refused edits (`us2_rebuild`).
- **FR-005**: D5's outcome, including the new record byte for byte and the empty outcome (`us1_rerender::as6_record_bytes`, `us2_rebuild`).
- **FR-006**: D6's stop rule, refusals and flags (`us2_rebuild::as3_refusals`, `as4_self_stop_both_ways`, `us3_stream::as4_input_short`, `as5_feed_edges`).
- **FR-007**: D7's public fill (`us4_surfaces::as3_fill`).
- **FR-008**: D8's C handles and Swift wrapper, appended (`ffi_parity rerender`, `abi_tests`, `swift test`, the five promise steps).
- **FR-009**: D9: no audio guard moves (`us2_abi`'s guard list, the golden SHA guards).
- **FR-010**: every mutant of US4 AS4 fails a named test (`mutation rerender::`).

## Success Criteria *(mandatory)*

- **SC-001**: A faithful re-render reads `Identical` on every cell at seven rates through Rust, C and Swift.
- **SC-002**: An hour at 48 kHz re-renders within 1.5× of the record path's cost (at least 20× real time) on the development machine, without the input in memory; the figure on the reference iPhone is reported, not gated (product M4).
- **SC-003**: No audio guard moves; every promise step passes.
- **SC-004**: The workbench can drop its re-render mirror for the session with its seven-rate digests unchanged (checked in the workbench's adoption feature).

## Assumptions

- The engine is at 0.93.0 (spec 026 merged, 3890bfe).
- Spec 020's bit-identity holds at the record's library version; a record from another version is re-rendered and judged by its digest.
- **Out of scope**: a tap's audio during a re-render ("Listen here" on a past recording, app 1.1; product M1). The stored form is the session's only audio output; a later feature may add a one-pass session from a plan at a tap. A "rebuilt from" field in the record (product M7: the case file labels a shared Rebuild "Rebuilt from recording N"; revisit before case files are shared).
- **Alternatives**:
  - **D1**: rewrite the harness to call the core (rejected: it would lose the independent oracle).
  - **D2**: one call over the whole input (kept only as a convenience); topping up only between blocks (rejected: rows land late with dense schedules).
  - **D3**: `f32` only (rejected: the feed-point rule is spec 020's guarantee); `i24` only (rejected: the workbench keeps `f32`).
  - **D4**: deltas as a fixed enum like the harness's (rejected: the Rebuild edits any setting).
  - **D5**: a digest only (rejected: the Rebuild needs the stored form and a record); an unbounded run buffer (rejected: an edited hang can need millions of events).
  - **D8**: Swift keeps its own recipe (rejected: the drift #77 showed); one C handle for plan and session (rejected: the plan's lifetime differs).
