# Feature Specification: The public re-render

**Feature Branch**: `027-public-re-render`

**Created**: 2026-11-03

**Status**: Draft, for the three draft reviews, then `/speckit-clarify` and the owner's sign-off.

**Input**: Engine issue #76, filed by the desktop workbench (rr-dr60-workbench, feature 005): spec 020's processing record is re-rendered by a recipe that lives only in the unpublished harness (`record_contract_checks::re_render_checked`, `filler`, `fill_to_remaining`) and in the Swift package (`ProcessingRecord.reRender`, `fill`). A Rust host has no public way to re-render a record and must mirror the recipe: an engine at the record's counter, a fill by import of a filler made on a throwaway engine, the slot and its store, the schedule refilled below 64 pending, the feed up to the stop position, the stop rule (no stop call after a self-stop), the digest of the last item. Each mirror can drift from the engine. Spec 026's clarification moved this here and named the app's need: **the Rebuild** (re-process a recording's kept microphone audio with changed settings) needs the rebuilt stored form and a new record, not only a digest.

**Timing**: the app's first release is delayed for non-technical reasons (owner, 2026-11-03), so this lands before it. Everything is appended (version 0.94.0); a host built on 0.93 runs unchanged.

## Decisions for sign-off

| # | Decision | Recommended |
|---|---|---|
| D1 | **One recipe, in the core** (#76). `rr_dr60::rerender` holds the recipe the harness and Swift carry today, step for step: the engine built at the record's host rate, settings, seed and counter; the memory filled by import to the record's remaining capacity at the start; the slot's component and every component's store; the record's change rows scheduled on the record stream and refilled below 64 pending; the kept microphone samples fed up to the stop's input position; the stop by the record's rule (a stop call, or none after a self-stop); the last item's digest. The harness's recipe stays as the **independent oracle** (it is not rewritten to call the core). The Swift `reRender` becomes a wrapper of the C call, so its own copy of the recipe is deleted. | **Core recipe; harness kept as the oracle; Swift wraps C** |
| D2 | **A streaming session, not one call** (product: an hour at 48 kHz is 173 M samples, 690 MB as `i32`). `ReRender::new(plan) → Result<ReRender, Error>` builds the engine and fills the memory (allocates, like `Pipeline::new`); `feed(&mut self, samples) → Result<Fed, Error>` takes any block length and returns how many samples were taken and whether the stop position (or a self-stop) is reached; `finish(self) → Result<Outcome, Error>`. Progress is the input position against the stop position; cancelling is dropping the session. No call allocates after `new`. A one-call `re_render(record, samples)` convenience is offered on top for short inputs and tests. | **Streaming session plus a one-call convenience** |
| D3 | **The input is the 24-bit feed point** (spec 020's rule: two apps re-rendering the same audio must feed identical samples). `feed` takes `i24` codes as `i32` and converts with `input::samples_from_i24`. A `feed_f32` variant takes engine samples for a host that keeps them losslessly as `f32` and documents that it re-renders identically only if those are the samples the recording was fed. | **i24 primary, f32 variant documented** |
| D4 | **The plan and the deltas** (the app's Rebuild, the workbench's comparisons). A `Plan` is built from a record (`Plan::from_record(bytes)`, which reads it) and is editable before `new`: the settings (validated as `reconfigure` validates them), the slot's component and store, the change rows (kept sorted; an edited row is validated as `schedule_change` validates it), the seed and the counter, the remaining capacity. An unedited plan is the faithful re-render; an edited one is a Rebuild. | **An editable plan; unedited = faithful** |
| D5 | **The outcome** (product M1 of spec 026). `finish` returns: the rebuilt item's digest; **the verdict** for a faithful plan, `Identical` or `Differs`, by the record's `recording_digest` (an edited plan's verdict is `Rebuilt`, no comparison); the read-out's flags carried (library or stage-model version differs, imported origin, unknown component); and access to the rebuilt item: its stored form (`export_len` / `export`) and **a new processing record** written from the session's own run (its events, change entries and stop fields, collected during `feed`), so a Rebuild is itself re-renderable. The new record is an ordinary engine record; it does not name the record it came from (the case file links them; no record-form change). | **Digest + verdict + flags + stored form + new record** |
| D6 | **Refusals and flags.** Refused at `Plan::from_record`: a record the reader refuses (spec 020's malformed, unknown form version, checksum). Refused at `new`: a record of imported origin (it has no run to replay), a record naming a component this library does not know (an empty slot would render a different session), settings or rows that do not validate. Refused at `finish`: an input that ended before the stop position (`InputShort`, with the samples still needed), since a shorter feed would differ silently. **Version differences are flags, not refusals**: the re-render runs and the verdict says whether it matched (spec 020: a record stays readable for ever; bit-identity holds at the record's library version). | **Refuse what cannot render; flag versions** |
| D7 | **`Pipeline::fill_to_remaining(remaining)` public** (#76 asks for it on its own). It fills the memory by import so the remaining capacity equals `remaining` device samples, floored to the stored unit (whole frames with the codec on), with the filler made as the recipe makes it today; refused while recording or playing, or when the memory is already fuller. Idle only; allocates. | **Public, idle only** |
| D8 | **C and Swift.** C: an opaque `RrDr60ReRender` handle with `rr_dr60_rerender_plan_from_record`, plan edit calls by control index (spec 025's readings) for the settings and by row for the changes, `rr_dr60_rerender_new`, `rr_dr60_rerender_feed_i24`, `rr_dr60_rerender_feed_f32`, `rr_dr60_rerender_progress`, `rr_dr60_rerender_finish` (a sized `RrDr60ReRenderOutcome`), the rebuilt item's `export_len`/`export` and `processing_record_len`/`write` through the session, and `rr_dr60_rerender_destroy`; new statuses for `InputShort` and the refusals; everything appended under spec 022's rules. Swift: `ProcessingRecord.reRender` keeps its signature and returns the digest through the C session; a new `ReRenderSession` type exposes the streaming calls, the verdict and the rebuilt item. | **A C session handle; Swift wraps it** |
| D9 | **Not a device behavior.** No assumption, no model version, no audio change (the sixteen golden guards and every smoke hash unmoved); the control and stage manifests unchanged but the version line. | **An emulator facility** |

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A host re-renders a record faithfully (Priority: P1)

A host holding a recording's processing record and its kept 24-bit microphone audio gets back the same recording, bit for bit, through one public engine facility, and is told so.

**Independent Test**: the core's re-render against the harness's independent recipe and against the recording's own digest, at seven rates.

1. **Given** spec 020's sixteen cells (codec × changes × drop/mute × stop call/self-stop), **When** each recording's record and input are re-rendered through the session, **Then** the digest equals the recording's and the harness recipe's, and the verdict is `Identical` (`us1_rerender::as1_sixteen_cells`).
2. **Given** spec 024's slot cells (the band filter, a non-default store with an empty slot), **Then** the same holds (`us1_rerender::as2_slot`).
3. **Given** a session fed in blocks of 1, 64, 4096 and an irregular schedule, **Then** every partition gives the same digest (`us1_rerender::as3_partitions`).
4. **Given** spec 020's named deltas applied to the input (a burst, one sample moved), **Then** the verdict is `Differs` (`us1_rerender::as4_differs`).
5. **Given** a schedule of more than 64 rows, **Then** the refill holds the digest (`us1_rerender::as5_refill`).

### User Story 2 - A host rebuilds a recording with changed settings (Priority: P1)

A host takes a record, changes a setting (a knee, the sensitivity, the band filter's frequency, a change row), and gets a new recording with its own stored form and record, which itself re-renders identically.

**Independent Test**: the rebuilt item against a direct recording made with the edited settings, and its new record's own re-render.

1. **Given** a plan with one edit (the knee to −30 dBFS; the band filter's frequency; one row moved; the seed), **When** it is rebuilt, **Then** its stored form equals a fresh engine's recording of the same input with the same edit applied (`us2_rebuild::as1_equals_direct`) and the verdict is `Rebuilt`.
2. **Given** the rebuilt item's new record and the same input, **Then** a faithful re-render of it is `Identical` (`us2_rebuild::as2_record_round_trip`).
3. **Given** edits that do not validate (a knee outside its range, a row with an unknown control, a row on the playback stream), **Then** each is refused at the edit with the setting's or the row's own error, and nothing is built (`us2_rebuild::as3_refusals`).

### User Story 3 - A host streams, shows progress and cancels (Priority: P2)

A host re-renders an hour-long recording without holding the whole input in memory, shows progress, and can stop early.

**Independent Test**: memory and allocation counts, the progress figure, and the early drop.

1. **Given** a session, **Then** `feed` and `finish` allocate nothing (the allocation counter), and the session's footprint is the engine's at the record's capacity plus a bounded run buffer (`us3_stream::as1_alloc_free`).
2. **Given** a 60-minute recording at 48 kHz (release only), **Then** the re-render reads `Identical` and runs at least 20× real time on the development machine (reported on the runner) (`us3_stream::as2_hour`).
3. **Given** a session dropped half-way, **Then** nothing leaks and a new session on the same record still reads `Identical` (`us3_stream::as3_cancel`).
4. **Given** an input that ends before the stop position, **Then** `finish` refuses with `InputShort` and the samples still needed (`us3_stream::as4_input_short`).

### User Story 4 - One recipe on every surface (Priority: P2)

Rust, C and Swift re-render through the same engine code.

**Independent Test**: parity and the source.

1. **Given** the sixteen cells, **Then** the C session and the Swift `ReRenderSession` give the Rust digest, verdict, stored form and new record byte for byte (`ffi_parity rerender`, `swift test ReRenderTests`).
2. **Given** the Swift package's sources, **Then** no fill, schedule refill or stop rule remains in Swift (a source scan) (`us4_surfaces::as2_no_swift_recipe`).
3. **Given** `Pipeline::fill_to_remaining`, **Then** it equals the harness's fill on every encoding and refuses while active or when fuller (`us4_surfaces::as3_fill`).

### Edge Cases

- **A record of imported origin**: refused at `new` (no run to replay).
- **A record from another library version**: re-rendered, flagged; the verdict decides.
- **A record whose recording self-stopped**: no stop call after the self-stop; the input past the stop position is ignored.
- **An input longer than the stop position**: the excess is ignored (`feed` reports it was not taken).
- **The codec on and a partial frame**: the fill floors to whole frames; the stop's padding is the record's.
- **An empty recording** (a record whose stop came before any kept sample): the session renders it and reads its digest, or refuses as the recorder would; to be pinned in the plan.
- **A delta that makes the memory self-stop earlier** (a larger store, a smaller capacity): the rebuilt item ends where the engine ends it; its new record says so.

## Requirements *(mandatory)*

- **FR-001**: D1's core recipe, matching the harness recipe on every cell (`us1_rerender`).
- **FR-002**: D2's streaming session, allocation-free after construction (`us3_stream`).
- **FR-003**: D3's feed point (`us1_rerender`, `us3_stream`).
- **FR-004**: D4's editable plan with the engine's own validation (`us2_rebuild`).
- **FR-005**: D5's outcome: digest, verdict, flags, stored form, new record (`us1_rerender`, `us2_rebuild`).
- **FR-006**: D6's refusals and flags (`us2_rebuild::as3_refusals`, `us3_stream::as4_input_short`, edge cases).
- **FR-007**: D7's public fill (`us4_surfaces::as3_fill`).
- **FR-008**: D8's C session and Swift wrapper, appended (`ffi_parity rerender`, `abi_tests`, `swift test`, the promise steps against 2a170d4 and 0.93's merge).
- **FR-009**: D9: no audio guard moves.

## Success Criteria *(mandatory)*

- **SC-001**: The workbench drops its re-render mirror (`workbench_core/src/records.rs`) for the engine's session with no change to its digests.
- **SC-002**: A faithful re-render reads `Identical` on every cell at seven rates through Rust, C and Swift.
- **SC-003**: An hour at 48 kHz re-renders at least 20× real time on the development machine, without the input in memory.
- **SC-004**: No audio guard moves; every earlier promise step passes.

## Assumptions

- The engine is at 0.93.0 (spec 026 merged, 3890bfe).
- Spec 020's bit-identity holds at the record's library version; a record from another version is re-rendered and judged by its digest.
- **Alternatives**:
  - **D1**: rewrite the harness to call the core (rejected: it would lose the independent oracle).
  - **D2**: one call over the whole input (kept only as a convenience: an hour does not fit comfortably on a phone).
  - **D3**: `f32` only (rejected: the feed-point rule is spec 020's guarantee).
  - **D4**: deltas as a fixed enum like the harness's (rejected: the Rebuild edits any setting).
  - **D5**: a digest only (rejected: the Rebuild needs the stored form and a record); a "rebuilt from" field in the record (rejected for now: a record-form change; the case file links them).
  - **D8**: Swift keeps its own recipe (rejected: the drift #77 showed).
