# Implementation Plan: Processing Record (the session contract, record side)

**Branch**: `020-session-contract` | **Date**: 2026-10-31 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/020-session-contract/spec.md` (approved D1 – D12, 2026-10-31)

## Decisions and findings (owner: read this section)

The spec's D1 – D12 stand, with these restatements from the plan reviews (coach 1 – 23, hardware B1 – B16, product M1 – M12, 2026-10-31, applied at their defaults):

1. **The alignment arithmetic** (hardware B1, B2, B4; coach 2, 8): kept sample k aligns with host input **c(k) = ⌊(k + R(k)) · m ÷ l⌋ − L_pre**, R(k) the device samples removed at or before k (one conversion, the floor once), L_pre the record path's latency **to the encoder's input** (spec 006's figure without the codec's 180; a new public `record_latency_pre_encoder_samples()`, and `timeline` derives it from the settings so the maps stay pure); the direct-path window is c(k) ± the decimator's `delay_host_samples()` (host samples; the prototype's `num_taps` is at the l-times rate), a stated convention (stage 4 has no finite support); the inverse is `record_landing(p + L_pre)` minus the gaps. The draft's `+ L`, `record_latency_samples()` and `(num_taps − 1)/2` were wrong in sign, by the codec delay and by l.
2. **A plain stop-fields struct** (coach 1): `StopFields { how, stop_input_position, padding_start_sample, input_samples_total, silence_fed_samples }` is `Copy` and buildable by anyone, with `From<&StopOutcome>`, `From<&RecordInfo>` (the self-stop: `stop_input_position = input_samples_total`, hardware B8) and `From<&RecordReadout>`; `RunData.stop` and `kept_to_input` take it; `VasEvent::new` becomes public so a recipient can assemble the maps' inputs from a record.
3. **The run bounds** (hardware B3, B5; coach 4): entries `position < input_samples_total` (a change landing in the stop's flush is honest and reproduces); events `output_position ≤ stored device length + 60` with frames (the look-ahead), `≤ length` with samples; more events or entries than the totals is inconsistent ("more than counted"); no recording-number check (erase renumbers).
4. **The blocks live inside `Memory`** (hardware B7; coach 7): `blocks: Vec<Option<SlotBlock>>` parallel to `spans`, pushed at `open`, popped on both discard paths (`close(false)`, `discard_in_progress`), removed on erase, `None` on both imports, copied by `copied_to`, so `reconfigure` with the memory kept carries them and re-reserves to the new `max_recordings`.
5. **The stop fields' hybrid rule** (hardware B8): `run.stop = stop_call` whenever the host called the stop, even if its flush filled the memory; a self-stop's `stop_input_position` is its `input_samples_total` (nothing after the fill is fed) and its padding start is the length; a flush that fills skips the padding loop, so the padding start is the length then too.
6. **The form, readable without the engine** (product M1 – M4, M9; coach 6, 17 – 20): change lines carry the control **identifier** and the stream **word**; `event.fields` and `change.fields` lines precede the first of each; `run.stop = stop_call | memory_full`, `encoding = frames | samples`, `stored_unit_count`, `stored_codec_model_version` for both origins; under `origin = imported` the `host_rate_hz` and `stage.N` lines are absent (they would describe the writing engine); 24 `setting.` lines (the manifest's settings except `host_rate_hz` and `seed`, which identity carries); a Required column per origin; a line whose name version 1 does not define is `MalformedRecord`; version 0 is refused; the checksum covers the LF before its line and the record ends with exactly one LF; no comment syntax.
7. **The stream word is 0 none, 1 playback, 2 one-pass, 3 record** (product M4): a pre-0.20 entry's 0 is told apart from a playback entry; the writer's reasons "no stream" and "not the record stream" stay distinct.
8. **A pure writer** (coach 5): `record::write(readout, events, changes, out)` and `record::len`; the engine's writer fills a `RecordReadout` from the block and delegates; C `rr_dr60_processing_record_rewrite` reads and writes in one call; the round trip is `write → read → rewrite == bytes`.
9. **Batch reads** (product M5; coach 23): `record_events(bytes, first, out: &mut [VasEvent]) -> usize`, `record_changes(…)` and `record_unknown_setting(bytes, i)` beside the single-row cursors; the guide says to extract once.
10. **Map (c)'s inputs and state** (coach 9, hardware B14): `place_at(index, host_rate_hz, start_level, unit_count, entries)` uses the `playback_speed` entries only; `SpeedPlace` carries `own`, `tail` and `final_count` too; `PlaceState(i)` is the state after the requests landing on device output i are applied and before its step; ten corner cases (the six of the spec plus a deferred level-3 switch overtaken by the latch, a waited request equal to the old level, two entries landing on one device output, a request in the tail).
11. **The one-pass map is an alignment** (hardware B9, coach 9): `onepass_output_to_input(position, settings, events)` = position − `latency_samples()` + the removed host samples, clamped and reported.
12. **The matrix and its oracles** (coach 10 – 16, hardware B12; product M7): the fill is by **import** (a recorded filler consumes the counter and drops silence in drop mode; a test shows it failing); the onset detector is the same one applied to the VAS-bypassed, codec-bypassed twin, the latency convention is read on an impulse through `record_only`, the codec-on case cross-correlates decoded output (±2); the effective deltas are named (`mic.hiss_offset_db` +1 dB, `agc.knee_dbfs` −1 dB, `vas.threshold_dbfs` −3 dB, `speech_codec_enabled` toggled; one f32 ULP may not move a stored byte); the stimulus ends in voice and the test asserts the gate was open at the stop; one `golden-record-v1.json` with seven entries whose version lines are normalized at compare time (asserted separately against `VERSION` and the manifest); `ffi_parity`'s two `reserved == 0` assertions become `reserved == stream` rows; `record_contract_timing.rs` (0.19's API, copied into the baseline worktree) times the record path, the one-pass and playback at constant levels 3 and 5, each at 3 %; the harness files are `record_contract_checks.rs` and `record_contract_timing.rs` (`record_checks.rs` and `record_timing.rs` are spec 017's) and every filter is `record_contract`; no pre-sizing bound is published (`_len` first); `re_render --demo <dir>` writes its own inputs.
13. **AS5's one-pass clause** (hardware B6): the fresh engine's one-pass equals the original's next one-pass **after the original's `reset`** (the one-pass is rekeyed at construction, reset and reconfigure only); the guide's seed-and-counter rule says so.
14. **The Swift target** (coach 15): a library target `RRDR60` (depending on `RRDR60C`) with `ReRender.swift`; its test builds everything through the C API (a ±0.125 square, no fixture).
15. **The digest of a recording in memory** is two passes (the header's CRC, then FNV over header + payload), no allocation (hardware B16); the spec's codec window reads −40 … +30 ms re the frame start (B10).

Plan choices, each traced to the research:

| # | Plan choice | Why |
|---|---|---|
| P1 | Core modules `record` (the grammar, a pure writer and reader over caller buffers, the cursors and batch reads), `digest` (FNV-1a 64), `timeline` (the maps), `input` (the feed-point helper); no allocation in any of them (a counting `fmt::Write` sink for `_len`) (R-02, R-03, R-09). | D2, D3, FR-001 – FR-003 |
| P2 | `SlotBlock` inside `Memory`, parallel to the spans (finding 4); the totals incremented in `write_event` and at a record-stream landing; `Pipeline::new_at_counter` on `with_counter` (R-05). | D5, FR-004, FR-008 |
| P3 | `StopOutcome` + `stop_input_position`, `padding_start_sample`; `StopFields` (finding 2, 5); `ChangeEntry::reserved` → `stream` with the 0 – 3 numbering (R-07, R-08). | D9, FR-007 |
| P4 | The form of [record-form.md](contracts/record-form.md): identity, 24 settings, run, `event.fields`/`change.fields`, rows, checksum; shortest round-trip decimals (R-02). | D2, FR-001 |
| P5 | The reader: version first (0 or > 1 refused), the checksum, the shapes (an undefined name is malformed), the required fields per origin; flags for every difference; `recording_digest_matches` (R-02). | D4, FR-002 |
| P6 | The maps per finding 1, 10, 11 (R-01, R-04). | D8, FR-006 |
| P7 | `SpeedPlace` factored out of `SpeedStage` (finding 10), guarded by the goldens and the A/B; the independent walker as the fallback (R-04). *Implemented as the fallback walker (T018, 2026-10-31): `stages/speed.rs` gained only the `__test-hooks` place trace; the A/B read −0.02 % / +0.87 % on playback, the latter the hooks build's trace.* | D8(c) |
| P8 | `input::samples_from_i24`; `examples/re_render.rs` (`--demo <dir>` writes a record and a raw file; the fill by import); the Swift `RRDR60` target and `ReRender.swift` (R-06). | D12 |
| P9 | The matrix and oracles of finding 12 (R-06, R-01). | D6, FR-005, FR-006 |
| P10 | The C API of [api.md](contracts/api.md): fourteen functions, five new structs with `struct_size` (prefix-fill), statuses 31 – 35, `RrDr60StopOutcome` 64 → 80 bytes (R-08); the smoke's `--record` case. | FR-009 |
| P11 | Records per [records.md](records.md): the stage manifest's version line; the guide section scanned with the per-feature forbidden list; the CHANGELOG; the product notes; specs 015/017's pointers; the A/B mode `record_contract`; `golden_record` in `ci.yml`'s three golden lines; version 0.20.0. | FR-010 |

## Summary

One versioned text record per recording, written beside the stored form from a fixed per-slot block the memory fills at the recording's start plus the run the host collected; a reader with refusals and flags; a 64-bit FNV-1a digest of the stored form; an engine constructible at a recording counter; three pure timeline maps; the stream word and two stop fields. The re-render contract is checked by a seven-rate matrix. No audio changes.

## Requirement map

| Requirement | Research | Tests |
|---|---|---|
| FR-001 the writer, the inconsistent-run refusal | R-02, R-09 | `us1_record_contract` AS1, `record_contract_edge_cases::{flush_entry_is_consistent, lookahead_event_is_consistent, surplus_refused, self_stop_run_from_record_info}`, core `record::tests` |
| FR-002 the reader, refusals and flags, the round trip (write → read → rewrite) | R-02 | `us2_record_read` AS1 – AS5, `record_contract_matrix` (the doctored set), `golden_record` |
| FR-003 the digest | R-03 | core `digest::tests` (the three vectors), `us2_record_read` AS3 |
| FR-004 construct-at-counter, the slot block | R-05 | `us1_record_contract` AS5 (dirty engine; the one-pass after reset), core `counter_tests::{discarded_recording_leaves_no_block, reconfigure_grows_the_block_vector}`, `alloc_free record_contract_*` |
| FR-005 the matrix, the named deltas, the playback twin, the recorded-filler failure | R-06, R-07 | `us1_record_contract` AS2 – AS3, `record_contract_matrix`, `mutation record_contract::` |
| FR-006 the maps | R-01, R-04 | `us3_record_timeline` AS1 – AS3, `record_contract_matrix` (markers, the VAS-bypassed oracle, the impulse latency, the codec-on correlation, the trace on ten corner cases), a codec-on test that fails under `record_latency_samples()` |
| FR-007 the stream word, the stop fields | R-07, R-08 | `ffi_parity` (`reserved == stream`; one-pass 2, record 3), `record_contract_edge_cases`, spec 015/017 suites green |
| FR-008 no audio change, cost | R-05 | the thirteen guards, every smoke hash, `scripts/ab-timing.sh 01a61ca record_contract` (four figures at 3 %) |
| FR-009 the C API, Swift | R-08 | `ffi_parity record_contract_parity`, the smoke `--record`, `swift test` (`ReRenderTests`) |
| FR-010 the records | — | `us2_cross_check` (manifest; the guide scan `guide_record_contract`), the CHANGELOG |
| SC-001 – SC-007 | as above | the guards; the normalized golden; the matrix; the doctored set; the maps; AS5; coverage |

## Technical Context

**Language/Version**: Rust stable, edition 2024, MSRV 1.85; C header through cbindgen 0.29.4; Swift 6 package (a new `RRDR60` library target).
**Primary Dependencies**: none new (`core::fmt` and `str::parse` for the numbers).
**Storage**: the per-slot block inside the memory (reserved at construction); the record in the caller's buffer.
**Testing**: `cargo test --all-features`, the harness's seven-rate matrices, one normalized golden, the C smoke, the Swift tests, `cargo llvm-cov`.
**Target Platform**: the six golden targets, iOS device and simulator.
**Project Type**: library workspace.
**Performance Goals**: the record path, the one-pass and playback (levels 3 and 5) within 3 % of 0.19 (5 % on the runner); the writer and reader linear in the record's length, off the callback.
**Constraints**: no allocation in the writer, the reader, the maps or the processing path; deterministic bytes on every platform; panic-safe C.
**Scale/Scope**: a record of about 70 lines plus one line per event and entry.

## Constitution Check

- Portable core, no platform dependencies: yes.
- Stable C API, panic-safe, no Rust types: yes (P10).
- Real-time safe processing path: yes (the block at the recording's open, the totals on cold paths; the writer and reader off the callback).
- Deterministic: yes.
- Sample-rate agnostic: yes (the maps take the settings).
- Test-first with measurable tolerances: the matrix, the doctored set, the maps' one-device-sample tolerance (±2 with the codec on).
- Independently testable: each story has its own test binary.
- Coverage ≥ 80 % and within 1 point: SC-007.
- Simplicity: no new assumption; the record is data.

Post-design re-check: no violation; the `SpeedPlace` factoring was the one refactor planned on an audio path and was not made (the walker fallback, 2026-10-31): no audio-path code moved.

## Project Structure

### Documentation (this feature)

```
specs/020-session-contract/
├── plan.md, research.md, data-model.md, quickstart.md, records.md
├── contracts/api.md, contracts/record-form.md
└── tasks.md (next)
```

### Source Code (repository root)

```
crates/rr_dr60/src/
├── record.rs          # the grammar, write/len/read, StopFields, RecordReadout, cursors, batch reads (new)
├── digest.rs          # fnv1a64 (new)
├── timeline.rs        # kept_to_input, input_to_kept, onepass_output_to_input, place_at (new)
├── input.rs           # samples_from_i24 (new)
├── memory.rs          # SlotBlock, blocks parallel to spans
├── transport.rs       # the stop fields, the totals, processing_record_*, recording_digest
├── changes.rs         # ChangeEntry::stream
├── stages/speed.rs    # the __test-hooks place trace only (the walker lives in timeline.rs)
├── pipeline.rs        # new_at_counter, record_latency_pre_encoder_samples, VasEvent::new
└── error.rs           # five statuses
crates/rr_dr60_ffi/src/lib.rs, cbindgen.toml, include/rr_dr60.h, tests/c/smoke.c
crates/rr_dr60_harness/src/record_contract_checks.rs, examples/{re_render.rs, record_rows.rs},
  tests/{us1_record_contract, us2_record_read, us3_record_timeline, record_contract_matrix,
  record_contract_edge_cases, golden_record, record_contract_timing}.rs, golden/golden-record-v1.json
swift/Package.swift (RRDR60 target), swift/Sources/RRDR60/ReRender.swift, swift/Tests/RRDR60Tests/ReRenderTests.swift
scripts/ab-timing.sh (record_contract), .github/workflows/{ab-timing.yml, ci.yml}
docs/guide.md, CHANGELOG.md, docs/product/*, specs/015 and 017 pointers
```

## Complexity Tracking

None: no constitution deviation.
