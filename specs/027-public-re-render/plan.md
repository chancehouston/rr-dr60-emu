# Implementation Plan: The public re-render

**Branch**: `027-public-re-render` | **Date**: 2026-11-03 | **Spec**: [spec.md](spec.md) (approved by the owner 2026-11-03; the plan reviews' restatements in its Clarifications) | Plan reviewed (coach 1 – 13, hardware P1 – P11, product M1 – M9, applied at the defaults below); **plan approved by the owner 2026-11-03**, with D7's direct-write fill confirmed.

## Summary

The processing record's re-render, today a recipe in the unpublished harness and in Swift, becomes one public facility in the core:
- a `Plan` read from a record, readable and editable before the run (the Rebuild);
- a streaming `ReRender` session that never schedules a row late, allocates only at construction, and returns a digest, a comparison with the original, the rebuilt stored form and a new processing record;
- a public `Pipeline::fill_to_remaining`;
- a C surface of two opaque handles, and a Swift session that replaces the Swift recipe.

No audio change; every C item appended; version 0.94.0.

## Technical context

Rust 2024, MSRV 1.85, `no_std` + `alloc` core (the session allocates at construction, like `Pipeline::new`); C API by cbindgen; Swift package. No new dependencies.

## Constitution check

- **Deterministic.** The result depends on the plan and the input only, never on the block lengths a host feeds (R-03's split rule makes that hold for dense schedules too).
- **Allocation.** The session is offline work, not on an audio thread, but `feed`, `meters` and `finish` allocate nothing (US3 AS1): every buffer is reserved in `new` (R-04).
- **An independent oracle.** The harness's recipe stays as written and is the oracle on every ordinary cell; the dense-row cell's oracle is the harness recipe at block size 1 (it tops the schedule up after every sample), and the harness recipe at block 4096 is the test's teeth (it fails there).
- **Claims policy.** The verdicts describe processing only; R-13's five sentences go in the guide, pinned by `us2_cross_check guide_rerender` and the house denylist.
- **C ABI (spec 022).** Two opaque handles, two sized structures with published offsets, appended statuses; the promise against five revisions.

## Research

- **R-01 The module.** `rr_dr60::rerender` (new): `Plan`, `ReRender`, `Fed`, `Outcome`, `Verdict`, `Reason`; `re_render(plan, samples)` the one-call convenience. It uses the `record` module's reader and the engine's writer.

- **R-02 The plan (D4).**
  - `Plan::from_record(bytes) -> Result<Plan, Error>`: `record::read` (spec 020's reader and refusals), then the rows by `record::record_changes`; a row whose `stream` word is not the record stream's (3) is `Error::InconsistentRun`. The plan holds the read-out, the rows (`Vec<ChangeEntry>`, scheduled later **with their recorded tags**), the component and the 16-value store, `collect_run` (true) and an untouched copy for `edited()`.
  - **Reads** (product M2): `settings()`, `component()`, `parameter(index)`, `rows()`, `seed()`, `counter()`, `remaining_samples()`, `collect_run()`, `edited()`, `flags()`, `readout()`.
  - **Edits**: `set_settings(s)` (validated by `Settings::validate`; a different `host_rate_hz` is `Error::HostRateFixed`); `set_component(id)`; `set_parameter(index, value)` (the component manifest's ranges, as `set_component_parameter` validates without an engine); `set_row(i, entry)`, `insert_row(entry)`, `remove_row(i)` (the control must be a record-side live control, spec 017's eight or the slot's 32 – 35, the value validated as `schedule_change` does; the list kept sorted, stable for equal positions; **a 65th row at one position is `Error::ScheduleFull`** at the edit); `set_seed`, `set_counter`, `set_remaining_samples` (above the capacity's device samples, refused at `new`; 0 refused at `new` with `MemoryFull`); `set_collect_run`.
  - `edited()`: a field compare against the untouched copy (f32 by bits). Spec 025's C write turns −0.0 into 0.0, so a read-and-write-back of a −0.0 field reads as edited while `matches_original` stays true; documented in the guide and asserted in `as5_rerender_unreached_edit`.
  - Form 1 (0.20 – 0.23): the reader gives an empty slot and the default store; the flags carry it.

- **R-03 The split-and-top-up rule (D2, hardware draft B1).** The session keeps `next`, the next unscheduled row. `feed(samples)`:
  1. `top_up()`: while `next < rows.len()` and `pending_changes(Record) < 64`, schedule `rows[next]`, `next += 1`.
  2. Let `limit` be the input still to take before the stop position. While samples remain, `limit > 0` and no self-stop: take `n = min(remaining, limit, CHUNK)`, and while rows remain unscheduled also `n = min(n, rows[next].position − input_position)`; record the `n` samples; `top_up()`.
  - `n` is never 0: a full schedule holds 64 rows at positions at or after the current sample, so a 65th row at the current position would be 65 at one position, refused at the edit (R-02) and absent from any record the engine wrote (every pending row at or before P lands at the top of P; hardware P10).
  - A row is late only if scheduled after its sample: every chunk ends at or before the next unscheduled row's position and the top-up follows each chunk. `CHUNK` = 4096 host samples. The engine is block-invariant (spec 001's partition identity), so any host block length gives the same result.
  - **At the stop** (hardware P3): `top_up()` once more; rows at positions from the stop position up to the input total land in the stop's flush, as they did in the original (a faithful record holds at most 64 of them: they were pending together at the original stop call). Rows the engine never reaches in an edited plan (past the input total, after an earlier self-stop, more than the schedule holds in the flush) are dropped, and the outcome's `rows_landed` counts the rows that landed; `rows_landed == rows.len()` is asserted on every faithful cell.
  - `Fed.taken` is the change in the input position (after a self-stop part-way through a chunk, the samples the engine took; hardware P11).

- **R-04 Construction and buffers (D2, D5, D6; hardware P7, coach 2).** `new(plan)`:
  1. Refusals before anything is built: imported origin (`Error::ImportedRecord`), an unknown component (`Error::UnknownComponent`), the remaining capacity 0 (`MemoryFull`) or above the capacity (`DoesNotFit`).
  2. The engine (`Pipeline::new_at_counter`), every published component's parameters at `parameter_offset + i`, then `set_component`, then the fill (R-05).
  3. The buffers, every one reserved here:
     - the event scratch for one call: `max_events(CHUNK)` at the plan's shortest hang (the settings' hang and every row that sets `vas.hang_ms`, through `stages::vas::device_samples`), + 4;
     - the stop's event scratch: `max_events` of the flush bound, `latency + 180·⌈m/l⌉ + 2·latency + 16` host samples, + 4 (the forced resume in drop mode and the closing event of an open muted region);
     - with `collect_run`: the run's events, `max_events(stop_position + flush bound, shortest hang) + 4`, refused above 16 Mi (`Error::RunTooLarge`); the run's changes, `rows.len() + MAX_CHANGES_PER_CALL` (every landed row logs one entry, a same-value landing a no-op entry, a dropped row none; hardware P10);
     - the scratch change log: `MAX_CHANGES_PER_CALL`.
  - **An overflow is an error, never a truncation**: after each engine call the session checks the call's event count against the room it handed in and returns `Error::InconsistentRun` (with a `debug_assert`), so an undersized bound shows at once, not at `record_write`.

- **R-05 The fill (D7 as restated; hardware P4, coach plan 1).** `Pipeline::fill_to_remaining(remaining)` (public, idle only): refused while recording or playing, when the remaining capacity is already below `remaining` (`DoesNotFit`) or no slot is left; `units = (remaining_now − remaining) / unit_samples`; `units == 0` adds no item; otherwise `memory::fill_item(units)` (crate-private) appends an item exactly as `import` does (its span, no slot block, counted against the items): `units` zero samples with the codec bypassed; with it on, `units` copies of the frame a standalone `SpeechEncoder::new()` emits for a frame of zeros (no side effect on the engine's encoder, which `start_recording` resets anyway). Export and import accept the item (the form's validation never inspects frame content). Peak memory is the store already reserved; the cost is one frame's encoding and a memory write. The oracle is behaviour (US4 AS3).

- **R-06 The stop rule (D6).** After a self-stop `feed` takes nothing. `finish(&mut self)`: if the session self-stopped, no stop call; else if the input position is below the stop position, `Error::InputShort { needed }` and the session stays feedable (`finish` may be called again; hardware P6); else `stop_recording_with_changes` into the run buffers. The stop fields come from the stop's outcome or the self-stopping call's report (`StopFields::from`), as the harness does. After a successful `finish`, `feed` is `Error::ReRenderState`.

- **R-07 The outcome (D5).** `Outcome { reason, digest: Option<u64>, edited, matches_original, verdict, flags, remaining_at_start, rows_landed }`; `Reason::{Kept, DiscardedEmpty}` (the second defensive only); `Verdict::{Identical, Differs, Rebuilt}`. After `finish` the session gives `export_len` / `export`, `record_len` / `record_write` (the engine's `processing_record_len` / `_write` with the collected `RunData`; `Error::RunNotCollected` without `collect_run`; 0 and OK after `DiscardedEmpty`) and `into_pipeline()`. For an unedited plan at the record's library version the new record equals the original byte for byte (hardware P9: the record has no item number; the configuration comes from the slot block written at the start; the `before` values chain from the same settings; the tags are kept; the stop fields do not depend on the chunking).

- **R-08 The meters and progress.** `meters()` is the engine's read-out of the **last engine call**, at most 4096 host samples (hardware P8): a host feeding longer blocks sees the last chunk. `progress()` returns the input position and the stop position; after a self-stop it stays below the stop position, so a host treats `reached_stop || self_stopped` as done (product M8). `new`'s time is short (R-05) and the guide says a host shows an indeterminate "Preparing" step (product M7).

- **R-09 The input (D3).** `feed(&[i32])` pre-scans the call: a code outside −2²³ … 2²³ − 1 refuses it before anything is taken (`Error::InvalidSample`); then converts in a 4096-sample stack chunk with `input::samples_from_i24`. `feed_f32(&[f32])` passes samples through. The first feed fixes the kind; the other is `Error::FeedKindMixed`.

- **R-10 C (D8; product M2 – M4, coach plan 8).**
  - Handles: `RrDr60ReRenderPlan` and `RrDr60ReRender`, each panic-guarded and poisonable like `RrDr60Pipeline`.
  - Plan: `rr_dr60_rerender_plan_from_record`, `_plan_destroy`, `_plan_clone`; reads `_plan_control_value(plan, index, *RrDr60ControlReading)` (settings controls), `_plan_component`, `_plan_parameter`, `_plan_row_count`, `_plan_row`, `_plan_seed`, `_plan_counter`, `_plan_remaining_samples`, `_plan_edited`, `_plan_flags`; edits `_plan_set_control_value` (settings controls only: the live controls and the slot's 31 – 35 are refused, the slot goes through `_plan_set_component` / `_plan_set_parameter`), `_plan_set_component`, `_plan_set_parameter`, `_plan_set_row`, `_plan_insert_row`, `_plan_remove_row` (rows as the frozen 40-byte `RrDr60ChangeEntry`), `_plan_set_seed`, `_plan_set_counter`, `_plan_set_remaining_samples`, `_plan_set_collect_run`.
  - Session: `rr_dr60_rerender_new(plan, **out)` **consumes the plan on success only** (product M3: after `RunTooLarge` the host sets `collect_run` false and retries); `_feed_i24`, `_feed_f32` (into a sized `RrDr60ReRenderFed`), `_progress`, `_meters` (spec 016's sized struct), `_finish` (a sized `RrDr60ReRenderOutcome`; after `INPUT_SHORT` the needed count is the stop position minus the input position from `_progress`), `_export_len`, `_export`, `_record_len`, `_record_write`, `_into_pipeline(r, **out)` (consumes a finished session, product M4), `_destroy`.
  - `RrDr60ReRenderFed` (published `[24]`): `uint32_t struct_size` 0, `bool reached_stop` 4, `bool self_stopped` 5, pad 6 – 7, `uint64_t taken` 8, `uint64_t reserved` 16.
  - `RrDr60ReRenderOutcome` (published `[56]`): `uint32_t struct_size` 0, `uint32_t reason` 4, `uint32_t verdict` 8, `uint32_t flags` 12, `bool edited` 16, `bool matches_original` 17, `bool has_digest` 18, pad 19 – 23, `uint64_t digest` 24, `uint64_t remaining_at_start` 32, `uint64_t rows_landed` 40, `uint64_t reserved` 48. Padding positions in `abi_tests`' offsets-derived table.
  - Statuses appended after 48: `INPUT_SHORT`, `INVALID_SAMPLE`, `RUN_TOO_LARGE`, `FEED_KIND_MIXED`, `HOST_RATE_FIXED`, `RUN_NOT_COLLECTED`, `RERENDER_STATE`, `IMPORTED_RECORD`, `UNKNOWN_COMPONENT`. The old smokes' "an unassigned status is refused" checks (spec 024 R8) do not test status values above their own range, so the promise needs no new tolerance; confirmed by running it.
  - Refusal orders: the handle, then NULL, then size, then the call's own checks (spec 022).
  - `abi-promise.sh` against 12a3c45, f5a0245, 97bbdc2, 2a170d4 and 3890bfe, by full ids; the fifth step in `ci.yml`.
  - `rr_dr60_fill_to_remaining(p, remaining)`.

- **R-11 Swift (D8; product M5, M6).** `ReRenderSession` (a non-`Sendable` final class owning the handle; `deinit` destroys it): `init(record:)`, the plan reads and edits by control identifier (`ControlIndex(identifier:)`, spec 025), `feed(i24:)`, `feed(f32:)`, `progress`, `meters`, `finish() throws -> ReRenderResult` (`outcome`, `storedForm: Data?`, `record: Data?`, copied eagerly; nil after `DiscardedEmpty` or without the run), `intoPipeline()`. `Verdict` and `Reason` Swift enums, the flags an `OptionSet`. `ProcessingRecord.reRender(record:raw:block:)` keeps its signature, runs on the session and checks `Task.checkCancellation()` between chunks (`CancellationError`). The old `fill`, `restoreStore`, `frameSamples` and refill loop are deleted (US4 AS2's source scan).

- **R-12 Mutants (FR-010; hardware P1, P2, P11).** Behind `__test-hooks`, `RerenderMutation::{no_split, self_stop_fields_dropped, fill_rounded_up, store_skipped, i24_scale, drop_event}`, each folded into a value read at `new` (no branch on the hot path):
  - `no_split`: top up only between host blocks → fails `us1_rerender::as5_rerender_dense_rows`;
  - `self_stop_fields_dropped` (a self-stopping call's stop fields not taken; restated from `stop_after_self_stop`, which only an error path caught) → the record writer refuses the run on a self-stop cell while the digest still matches;
  - `fill_rounded_up` → fails `us4_surfaces::as3_rerender_fill` at a codec-on target of 120·k + 7 (the self-stop cells' fills are whole frames, so they cannot see it);
  - `store_skipped` (the store left at its defaults) → fails `as2_rerender_slot_and_form_1`'s non-default-store cell (the order of store and component is equivalent at the start, so that mutant was dropped);
  - `i24_scale` (`x / (2²³ − 1)`) → fails `as1_rerender_sixteen_cells`' codec-off cells;
  - `drop_event` → fails `as6_rerender_record_bytes` only (the teeth for the run rows).
  Each mutant is shown failing before the gate relies on it.

- **R-13 Records.**
  - The guide's "Processing record" section gains "The re-render, as an API": the session, the plan, the split rule, the meters' last-call rule, the progress rule, the −0.0 note, the out-of-scope tap, and the five verdict sentences (spec 020's sentence for Identical kept as published, the others product M1/M6 reconciled):
    - *Identical:* "This recording re-renders identically from the included microphone audio with these settings (model 0.x)."
    - *Differs:* "Re-rendering from the included microphone audio with these settings gives a different result; the audio, the record or the library version is not the original's (see the flags)."
    - *Rebuilt, matching the original:* "The changed settings gave the same stored recording: the audio never reached the point where they act."
    - *Rebuilt:* "A new recording rebuilt from the original microphone audio with changed settings. The original recording is unchanged."
    - *Nothing kept:* "With these settings nothing was kept, so no recording was made."
  - `us2_cross_check guide_rerender` pins the sentences and runs the house denylist and the claims list on the section.
  - The CHANGELOG under Added; the README's paragraph; the real-time lists name the session as offline and allocation-free after construction; version 0.94.0 (Cargo ×4, `RR_DR60_VERSION_MINOR` 94, the header regenerated after every FFI edit, the stage and control manifests' version lines, the guide's copied blocks, the Swift fixture); CLAUDE.md's Useful commands line at the post-merge records; #76 closed then; a workbench issue for SC-004.
  - The golden guard count is read from `us2_abi` (seventeen golden files today: restate the spec's "sixteen guards" from that list at implementation, house rule).
  - The PR's `ab-timing` mode: `scripts/ab-timing.sh 3890bfe record` (memory and transport are touched), asserted no regression.

## Build order (test-first)

1. `fill_to_remaining` and `fill_item` with `us4_surfaces::as3_rerender_fill` (red, then green).
2. `Plan`: reads, edits, refusals, `edited()` (`us2_rebuild::as3_rerender_refusals`, core tests).
3. The session against the harness oracle: `us1_rerender` (the dense-row cell's red teeth on the harness at 4096 first), then `as6_rerender_record_bytes`.
4. US2 (`us2_rebuild`), then US3 (`us3_stream`, the hour last, release).
5. The six mutants, each shown failing.
6. C: the handles, `abi_tests`, Miri, `ffi_parity rerender`, the smoke's `--rerender` against `examples/rerender_rows.rs`, the header.
7. Swift: the session, the rewritten `reRender`, the source scan, `swift test`, the simulator.
8. Records and the version bump; the promise steps; the gate.

## Traceability

| Item | R | Tests |
|---|---|---|
| D1, FR-001 | R-01, R-04 | `us1_rerender::{as1_rerender_sixteen_cells, as2_rerender_slot_and_form_1, as4_rerender_differs}` |
| D2, FR-002 | R-03, R-04, R-08 | `us1_rerender::{as3_rerender_partitions, as5_rerender_dense_rows}`, `us3_stream::{as1_rerender_meters_and_footprint, as3_rerender_cancel_and_progress}` (the meters against a direct engine; progress) |
| D3, FR-003 | R-09 | `us3_stream::as5_rerender_feed_edges`, `ffi_parity rerender` |
| D4, FR-004 | R-02 | `us2_rebuild::{as1_rerender_equals_direct_and_as2_round_trip, as3_rerender_refusals, as5_rerender_unreached_edit}` |
| D5, FR-005 | R-07 | `us1_rerender::as6_rerender_record_bytes`, `us2_rebuild::{as1_rerender_equals_direct_and_as2_round_trip, as6_rerender_empty}`, `us3_stream::as5_rerender_feed_edges` (`RunTooLarge`, `collect_run`) |
| D6, FR-006 | R-04, R-06 | `us2_rebuild::{as3_rerender_refusals, as4_rerender_self_stop_both_ways}`, `us3_stream::{as4_rerender_input_short, as5_rerender_feed_edges}` |
| D7, FR-007 | R-05 | `us4_surfaces::as3_rerender_fill` |
| D8, FR-008 | R-10, R-11 | `ffi_parity rerender`, `abi_tests rerender`, `us4_surfaces::as2_rerender_no_swift_recipe`, `swift test`, the five promise steps |
| D9, FR-009 | R-13 | the golden SHA guards, `us2_abi` |
| FR-010 | R-12 | `mutation rerender::` |
| SC-001 | — | `us1_rerender`, `ffi_parity rerender`, `swift test ReRenderTests` |
| SC-002 | R-03 | `us3_stream::as2_rerender_hour` (release; the input generated in blocks, never one vector; the record path timed in the same solo run; the iPhone figure from the release checklist's device run, reported) |
| SC-003 | R-10 | the golden SHA guards, the five promise steps |
| SC-004 | R-13 | a workbench issue (its adoption feature) |
| The guide | R-13 | `us2_cross_check guide_rerender` |

## Testing

Each acceptance scenario's named test, plus `alloc_free rerender`, `determinism rerender`, Miri on the C session's fills, the C smoke's `--rerender`, the guide's cross-check. Test names in the harness files all contain `rerender` so a combined filter matches every binary (spec 015's lesson).

## Project structure

```text
crates/rr_dr60/src/rerender.rs                         # new: Plan, ReRender, Outcome
crates/rr_dr60/src/{memory.rs, transport.rs}           # fill_item, fill_to_remaining
crates/rr_dr60_ffi/src/rerender.rs                     # new: the two handles
crates/rr_dr60_harness/tests/{us1_rerender, us2_rebuild, us3_stream, us4_surfaces}.rs
crates/rr_dr60_harness/examples/rerender_rows.rs
swift/Sources/RRDR60/ReRender.swift                    # rewritten on the session
docs/guide.md, CHANGELOG.md, README.md
```
