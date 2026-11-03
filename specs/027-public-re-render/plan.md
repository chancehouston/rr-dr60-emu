# Implementation Plan: The public re-render

**Branch**: `027-public-re-render` | **Date**: 2026-11-03 | **Spec**: [spec.md](spec.md) (approved by the owner 2026-11-03)

## Summary

The processing record's re-render, today a recipe in the unpublished harness and in Swift, becomes one public facility in the core:
- a `Plan` read from a record and editable before the run (the Rebuild);
- a streaming `ReRender` session that never schedules a row late, allocates only at construction, and returns a digest, a comparison with the original, the rebuilt stored form and a new processing record;
- a public `Pipeline::fill_to_remaining`;
- a C surface of two opaque handles, and a Swift session that replaces the Swift recipe.

No audio change; every C item appended; version 0.94.0.

## Technical context

Rust 2024, MSRV 1.85, `no_std` + `alloc` core (the session allocates at construction, like `Pipeline::new`); C API by cbindgen; Swift package. No new dependencies.

## Constitution check

- **Deterministic.** The session is the engine fed by a fixed rule; the result depends on the plan and the input only, never on the block lengths a host feeds (R-03's split rule makes that hold for dense schedules too).
- **Real-time and allocation.** The session is offline work, not on an audio thread, but `feed`, `meters` and `finish` allocate nothing (US3 AS1): every buffer is reserved in `new` (R-04).
- **Independently testable; an independent oracle.** The harness's recipe stays as written and is the oracle on every ordinary cell; the dense-row cell's oracle is the harness recipe at block size 1 (it tops the schedule up after every sample, so it is never late), and the harness recipe at block 4096 is the test's teeth (it fails there).
- **Claims policy.** The verdicts describe processing only; the guide carries product M6's four sentences, pinned by a cross-check.
- **C ABI (spec 022).** Two opaque handles, one sized outcome structure, appended statuses; the promise against five revisions.

## Research

- **R-01 The module.** `rr_dr60::rerender` (new): `Plan`, `ReRender`, `Fed`, `Outcome`, `Verdict`, `Reason`; `re_render(plan, samples)` the one-call convenience. The core's `record` module keeps the reader and the writer; `rerender` uses them.

- **R-02 The plan (D4).**
  - `Plan::from_record(bytes) -> Result<Plan, Error>`: `record::read` (spec 020's reader, its refusals), then the rows by `record::record_changes`; a row whose `stream` word is not the record stream's (3) is `Error::InconsistentRun` (B5). The plan holds the read-out (`RecordReadout`), the rows (a `Vec<ChangeEntry>`), the component and the 16-value store, `collect_run` (true), and an untouched copy for the `edited` compare.
  - Edits: `settings_mut()` returns a copy-in/copy-out `Settings` setter (`set_settings(s) -> Result`, validated by `Settings::validate` and refused when `host_rate_hz` differs: `Error::HostRateFixed`); `set_component(id)`; `set_parameter(index, value)` (validated as `set_component_parameter` validates it, without an engine: the component manifest's ranges); `rows_mut` is not exposed: `set_row(i, entry)`, `insert_row(entry)`, `remove_row(i)` validate the control (a record-side live control, spec 017's eight, the slot's 32 – 35) and value as `schedule_change` does, and keep the list sorted by position, stable for equal positions (insertion after the last equal one); `set_seed`, `set_counter`, `set_remaining_samples` (refused above the capacity's device samples at `new`, B5).
  - `edited()`: a field compare of the current against the untouched copy (settings by field, f32 by bits; the store by bits; the rows by every field; seed, counter, remaining, component).
  - Form 1 (0.20 – 0.23): `record::read` already gives an empty slot and the default store; the read-out's flags carry it (spec 024's reader); nothing new.

- **R-03 The split-and-top-up rule (D2, hardware B1).** The session keeps `next`, the index of the next unscheduled row. `feed(samples)`:
  1. `top_up()`: while `next < rows.len()` and `pending_changes(Record) < 64`, schedule `rows[next]`, `next += 1`.
  2. Let `limit` be the input still to take before the stop position. While samples remain and `limit > 0`: take `n = min(remaining, limit, CHUNK)`, and while rows remain unscheduled (the schedule is full), also `n = min(n, rows[next].position − input_position)`; record the `n` samples; `top_up()`; stop at a self-stop.
  - `n` is never 0: a full schedule holds 64 rows at positions at or after the current sample (a row at an earlier position has landed), so a 65th row at the current position would mean 65 rows at one position, which a plan refuses at `new` (`Error::ScheduleFull`: the engine's schedule could never have landed them at one sample, so no record carries it; an edited plan may).
  - Why it holds: a row is late only if it is scheduled after its position has been processed. Every chunk ends at or before the next unscheduled row's position, and the top-up runs after each chunk, so each row is pending before its sample. At most 64 rows are pending, and rows at one position are scheduled in order (the schedule keeps submission order at equal positions, spec 015).
  - `CHUNK` = 4096 host samples (the scratch buffers' size, R-04). Any host block length gives the same engine calls' *samples* in the same order; the engine is block-invariant (spec 001's partition identity), so the result is too.
  - Before the stop call, `top_up()` until the rows are scheduled or the schedule is full; rows that cannot be pending when the stop's flush runs are dropped by the engine at the end of the recording exactly as in the original (their positions are past the input), so the record's rows past the stop do not occur in a faithful plan; in an edited plan they are documented as dropped (the change log shows they never landed).

- **R-04 Buffers (D2, D5; hardware B2, coach 2).** At `new`:
  - The engine (`Pipeline::new_at_counter`), then the slot (every component's parameters at `parameter_offset + i`, then `set_component`), then the fill (R-05), all before the schedule and the start.
  - The event scratch: `max_events(CHUNK)` at the plan's shortest hang (the settings' hang and every row that sets `vas.hang_ms`; `RecordPath::max_events` takes the settings' hang and the pending rows, so the session computes the shortest over all rows itself through the same `stages::vas::device_samples`).
  - With `collect_run`: the run's events, sized `max_events(stop_position + flush, shortest hang)` where `flush` is the record path's latency plus 180 device samples' worth of host samples (the stop's padding bound, spec 008 FR-013) plus one `CHUNK`; refused above 16 Mi (`Error::RunTooLarge`). The run's changes: `rows.len() + MAX_CHANGES_PER_CALL` (every row lands at most once; the scratch room for one call).
  - The scratch change buffer: `MAX_CHANGES_PER_CALL` (one call's log).
  - Without `collect_run`: only the two scratch buffers.

- **R-05 The fill (D7; hardware B3, coach 5).** `Pipeline::fill_to_remaining(remaining)` (public, idle only): refused while recording or playing (`RecordingInProgress` / `PlaybackInProgress`), when the remaining capacity is already below `remaining` (`Error::DoesNotFit`), when no slot is left; otherwise `units = (remaining_now − remaining) / unit_samples`, and **the item is written into the memory directly** (`memory::fill_item(units)`, crate-private): with the codec bypassed `units` zero samples; with the codec on `units` copies of the frame the speech encoder emits for a frame of zeros from its rest state (encoded once at the call). Its span and its absent slot block are what `import` makes. No throwaway engine, no input vector, no exported bytes: the peak memory is the store already reserved, and the call costs one frame's encoding plus a memory write. The content is unspecified in the rustdoc; the oracle is behaviour (US4 AS3: equal remaining capacity and an equal digest for a recording made afterwards, against the harness's `fill_to_remaining`, and two fillers with different content giving one digest).

- **R-06 The stop rule (D6).** `feed` returns `Fed { taken, reached_stop, self_stopped }`; after a self-stop it takes nothing. `finish`: if the session self-stopped, no stop call; else if the input position is below the stop position, `Error::InputShort { needed }`; else `stop_recording_with_changes` into the run buffers. The stop fields come from the stop's outcome or the self-stopping call's report (`StopFields::from`), as the harness does.

- **R-07 The outcome (D5).** `Outcome { reason: Reason, digest: Option<u64>, edited: bool, matches_original: bool, verdict: Verdict, flags: u32, remaining_at_start: u64 }`; `Reason::{Kept, DiscardedEmpty}`; `Verdict::{Identical, Differs, Rebuilt}` (`Rebuilt` when `edited`). The session after `finish` becomes a `Finished` value holding the engine: `export_len`, `export`, `record_len`, `record_write` (the engine's own `processing_record_len` / `_write` with the collected `RunData`; `Error::RunNotCollected` without `collect_run`), `into_pipeline()` for a host that wants to play the rebuilt item. For an unedited plan at the record's library version the new record equals the original byte for byte: the slot block holds the same configuration, the digest is the same, and the run rows are the engine's own log and events (US1 AS6).

- **R-08 The meters (product M5).** `ReRender::meters()` is the engine's `meters()`: the read-out of the last processed chunk (spec 016's rules).

- **R-09 The input (D3).** `feed(&[i32])` converts in a 4096-sample stack chunk with `input::samples_from_i24`; a code outside −2²³ … 2²³ − 1 anywhere in the call refuses it before anything is taken (`Error::InvalidSample`, a pre-scan). `feed_f32(&[f32])` passes samples through (the engine sanitizes non-finite samples as always). The first feed fixes the kind; the other is `Error::FeedKindMixed`.

- **R-10 C (D8).**
  - Handles: `RrDr60ReRenderPlan` and `RrDr60ReRender`, each panic-guarded and poisonable like `RrDr60Pipeline`.
  - Plan: `rr_dr60_rerender_plan_from_record(bytes, len, **out)`, `_plan_destroy`, `_plan_set_control_value(plan, index, *RrDr60ControlReading)` (spec 025's readings, settings controls only; the live controls refused), `_plan_set_component`, `_plan_set_parameter`, `_plan_row_count`, `_plan_row`, `_plan_set_row`, `_plan_insert_row`, `_plan_remove_row` (rows as `RrDr60ChangeEntry`, frozen 40 bytes), `_plan_set_seed`, `_plan_set_counter`, `_plan_set_remaining_samples`, `_plan_set_collect_run`, `_plan_edited`.
  - Session: `rr_dr60_rerender_new(plan, **out)` (consumes the plan: the plan handle is destroyed whether or not `new` succeeds), `_feed_i24(r, *int32, n, *RrDr60ReRenderFed)`, `_feed_f32`, `_progress(r, *input_position, *stop_position)`, `_meters(r, *RrDr60Meters)` (spec 016's sized struct), `_finish(r, *RrDr60ReRenderOutcome)` (sized, published `[48]` on 64-bit), then `_export_len`, `_export`, `_record_len`, `_record_write` on the finished handle, `_destroy`.
  - Statuses appended after 48: `INPUT_SHORT`, `INVALID_SAMPLE`, `RUN_TOO_LARGE`, `FEED_KIND_MIXED`, `HOST_RATE_FIXED`, `RUN_NOT_COLLECTED`, `RERENDER_STATE` (a call in the wrong state: feed after finish, export before finish).
  - `RrDr60ReRenderFed` (sized `[24]`): taken, reached_stop, self_stopped. `RrDr60ReRenderOutcome`: struct_size, reason, verdict, edited, matches_original, flags, digest, has_digest, remaining_at_start.
  - Refusal orders: the handle, then NULL, then size, then the call's own checks (spec 022).
  - `abi-promise.sh` against 12a3c45, f5a0245, 97bbdc2, 2a170d4 and 3890bfe, by full ids; the fifth step in `ci.yml`.

- **R-11 Swift (D8).** `ReRenderSession` (a class owning the handle; `init(record:)`, the plan edits, `feed(i24:)`, `feed(f32:)`, `progress`, `meters`, `finish() -> Outcome`, `export()`, `processingRecord()`), and `ProcessingRecord.reRender(record:raw:block:)` rewritten on it (signature kept). The old `fill`, `restoreStore` and the refill loop are deleted (US4 AS2's source scan). `swift test` re-runs `ReRenderTests` unchanged plus the session tests.

- **R-12 Mutants (FR-010).** Behind `__test-hooks`, `RerenderMutation::{no_split, stop_after_self_stop, fill_not_floored, store_after_component, i24_scale, drop_event}`, each folded into a value read at `new` (no branch on the hot path, 015's lesson): no split (top up only between host blocks; fails `as5_dense_rows`); a stop call after a self-stop (`NotRecording`, or a different digest: fails `as1_sixteen_cells` self-stop cells); the fill not floored (a remaining off by up to 119: fails `as3_fill` and the codec self-stop cells); the store restored after the component (fails `as2_slot`: the component's state starts from the old store); `feed` converting with `x / (2²³ − 1)` instead of `input::samples_from_i24` (fails `as1_sixteen_cells`); one event dropped from the run (fails `as6_record_bytes` only: the teeth for coach 1).

- **R-13 Records.** The guide's "Processing record" section gains "The re-render, as an API": the session, the plan, the split rule, the outcome, product M6's four verdict sentences, the out-of-scope tap; the CHANGELOG under Added; the README's paragraph; the real-time lists name the session as not real-time but allocation-free after construction; version 0.94.0 (Cargo ×4, `RR_DR60_VERSION_MINOR` 94, the header, the manifest's version line, the Swift fixture).

## Traceability

| Item | R | Tests |
|---|---|---|
| D1, FR-001 | R-01, R-04 | `us1_rerender::{as1_sixteen_cells, as2_slot_and_form_1}` |
| D2, FR-002 | R-03, R-04, R-08 | `us1_rerender::{as3_partitions, as5_dense_rows}`, `us3_stream::{as1_alloc_free, as3_cancel}` |
| D3, FR-003 | R-09 | `us3_stream::as5_feed_edges`, `ffi_parity rerender` |
| D4, FR-004 | R-02 | `us2_rebuild::{as1_equals_direct, as3_refusals, as5_unreached_edit}` |
| D5, FR-005 | R-07 | `us1_rerender::as6_record_bytes`, `us2_rebuild::{as2_record_round_trip, as6_empty}` |
| D6, FR-006 | R-06 | `us2_rebuild::as4_self_stop_both_ways`, `us3_stream::as4_input_short` |
| D7, FR-007 | R-05 | `us4_surfaces::as3_fill` |
| D8, FR-008 | R-10, R-11 | `ffi_parity rerender`, `abi_tests rerender`, `swift test`, the promise steps |
| D9, FR-009 | — | the golden SHA guards, `us2_abi` |
| FR-010 | R-12 | `mutation rerender::` |
| SC-002 | R-03 | `us3_stream::as2_hour` (release) |

## Testing

Each acceptance scenario's named test, plus `alloc_free rerender`, `determinism rerender`, Miri on the C session's fills, the C smoke's `--rerender` against `examples/rerender_rows.rs`, the guide's cross-check of the four sentences.

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
