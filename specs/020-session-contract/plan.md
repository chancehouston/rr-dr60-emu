# Implementation Plan: Processing Record (the session contract, record side)

**Branch**: `020-session-contract` | **Date**: 2026-10-31 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/020-session-contract/spec.md` (approved D1 – D12, 2026-10-31)

## Decisions and findings (owner: read this section)

The spec's D1 – D12 stand. Plan choices, each traced to the research:

| # | Plan choice | Why |
|---|---|---|
| P1 | A new core module `record` (the writer, the reader, the line grammar) and a module `digest` (FNV-1a 64), both pure functions over caller buffers: no allocation in either direction (the writer formats into the caller's `&mut [u8]` through a counting `fmt::Write` adaptor, the reader parses lines in place and fills a fixed-size `RecordReadout`); a cursor pair (`record_event(bytes, i)`, `record_change(bytes, i)`) re-scans the lines on demand (R-02, R-09). | D2, D3, FR-001 – FR-003 |
| P2 | A per-slot block `SlotBlock` (counter, a `Settings` copy, remaining samples at the start, two totals) in a `Vec<Option<SlotBlock>>` with capacity `max_recordings` reserved at construction (about 152 bytes per slot; 15 KB at 99), kept parallel to the memory's spans (`remove` on erase, `push(None)` on import, cleared with the memory); written at `start_recording`, the totals incremented in `write_event` and at a record-stream landing (cold paths); `Pipeline::new_at_counter` on the existing `with_counter` (R-05). | D5, FR-004, FR-008 |
| P3 | The stop report gains `stop_input_position` (the host samples fed before the stop was called: the input position at the stop's entry) and `padding_start_sample` (the recorder's `represented` with the codec on; the recording's length with it bypassed); the self-stop's outcome carries the same two (the position where the memory filled; no padding: the length); `ChangeEntry::reserved` → `stream`, set in `ChangeEntry::new` from the landing's stream (R-07, R-08). | D9, FR-007 |
| P4 | The writer's grammar: 37 header lines (identity + run), one `setting.<id>` line per control-manifest setting (the manifest's order, 26 lines), then `event.<i>` and `change.<i>` lines, then the checksum line; the real numbers through `core::fmt`'s shortest round-trip `Display` (f32 for the settings, f64 for before/after), parsed with `str::parse` (correctly rounded); the length bound is exact: the writer runs once in counting mode for `_len` and once writing (R-02, R-09). | D2, FR-001 |
| P5 | The reader validates the first line and the checksum before anything else, then every line's `name = value` shape, then the required fields; differences become flags in `RecordReadout.flags` (a bitset: `LIBRARY_DIFFERS`, `STAGE_MODEL_DIFFERS` with a per-stage mask, `HOST_RATE_DIFFERS`, `UNKNOWN_SETTINGS` with a count, `INCOMPLETE_RUN`, `IMPORTED_ORIGIN`, `DIGEST_MISMATCH` once checked against a recording with `recording_digest_matches(number, readout)`); form versions below the library's are read (version 1 is the only one today; the rule is written into the reader as "unknown = greater than ours") (R-02). | D4, FR-002 |
| P6 | The maps in a module `timeline`: (a) `kept_to_input` from the rate plan's integer conversion (`d(P)` of spec 015 inverted: kept k's centre host sample ⌈k·rate/6000⌉ + latency + Σ input lengths of events at or before k, in host units; the direct-path window ± the decimator's `(num_taps − 1)/2` host samples, widened with the codec on by the frame's analysis window 60 past … 60 ahead of the frame plus the 240-sample pitch memory: [120f − 240, 120f + 179] kept); (b) its inverse and the one-pass map in host units from the one-pass's events; (c) `place_at` over a `SpeedPlace` automaton factored out of `SpeedStage` (R-01, R-04). | D8, FR-006 |
| P7 | `SpeedStage` keeps its fields but moves the place arithmetic (`apply_request`, the fade's start, the deferred switch, the latch, `last_p5`) into `pub(crate) struct SpeedPlace` with `fn step(&mut self, request: Option<u32>) -> Step` returning what to compute (the branches' p5 and k, the fade index); the stage calls it per output; the public `timeline::place_at` replays it over a schedule; the speed and changes goldens and the 3 % A/B guard the refactor; if the A/B on level 5 reads above 3 %, the fallback is an independent walker in `timeline` pinned against `speed_device_outputs` (R-04). | D8(c) |
| P8 | The feed-point helper `input::samples_from_i24` (x ÷ 2²³ as `(x as f32) * (1.0 / 8_388_608.0)`, exact for every 24-bit integer) and the harness example `examples/re_render.rs` (reads a record and a raw file, builds the engine, fills the memory, schedules, feeds, stops, digests) with the Swift port in `swift/Sources/RRDR60/ReRender.swift` and a Swift test (R-06). | D12 |
| P9 | The re-render matrix: the harness builds the original from `configs::settings_011`-style configurations with the record stages on, records a stimulus (voice at −20 dBFS with gaps), collects the run from the calls, writes the record, re-renders through the recipe; the self-stop cell fills the second engine with a filler recording of `capacity − remaining` samples (bypass_all at 6 kHz, imported at the target rate: `stored_recording`'s pattern) before `start_recording` (R-06). | D6, FR-005 |
| P10 | The C API: eleven functions and four new structs with `struct_size` first (prefix-fill as spec 018's meters); statuses 31 – 35; `RrDr60StopOutcome` grows by two `uint64_t` at the end (no size field: the pre-1.0 ABI pass list; the CHANGELOG says so); the smoke's `--record` case diffed against `examples/record_rows.rs` (R-08). | FR-009 |
| P11 | Records: the stage manifest's version line only; the guide's "Processing record" section scanned by a new `us2_cross_check` test with the per-feature forbidden list; the CHANGELOG under Added; the product notes; specs 015/017's pointers; A/B mode `record_contract`; version 0.20.0. | FR-010 |

## Summary

One versioned text record per recording, written beside the stored form from a fixed per-slot block the engine fills at `start_recording` plus the run the host collected; a reader with refusals and flags; a 64-bit FNV-1a digest of the stored form; an engine constructible at a recording counter; three pure timeline maps; the stream word and two stop fields. The re-render contract is proven by a seven-rate matrix. No audio changes.

## Requirement map

| Requirement | Research | Tests |
|---|---|---|
| FR-001 the writer, the inconsistent-run refusal | R-02, R-09 | `us1_record_contract` AS1, `record_contract_edge_cases`, core `record::tests` |
| FR-002 the reader, refusals and flags, the round trip | R-02 | `us2_record_read` AS1 – AS5, `record_contract_matrix` (doctored set), `golden_record` |
| FR-003 the digest | R-03 | core `digest::tests` (the published FNV test vectors), `us2_record_read` AS3 |
| FR-004 construct-at-counter, the slot block | R-05 | `us1_record_contract` AS5 (dirty engine), core `counter_tests`, `alloc_free` |
| FR-005 the re-render matrix, the effective changes, the playback twin | R-06, R-07 | `us1_record_contract` AS2 – AS3, `record_contract_matrix`, `mutation record_contract::` |
| FR-006 the maps | R-01, R-04 | `us3_record_timeline` AS1 – AS3, `record_contract_matrix` (burst markers, VAS-bypassed oracle, the trace) |
| FR-007 the stream word, the stop fields | R-07, R-08 | `us2_changes` (015) unchanged, `record_contract_edge_cases`, `ffi_parity` |
| FR-008 no audio change, cost | R-05 | the thirteen guards, every smoke hash, `scripts/ab-timing.sh 01a61ca record_contract` |
| FR-009 the C API, Swift | R-08 | `ffi_parity record_parity`, the smoke `--record`, `swift test` |
| FR-010 the records | — | `us2_cross_check` (manifest, the guide scan), the CHANGELOG |
| SC-001 – SC-007 | as above | the guards; the golden records; the matrix; the doctored set; the maps; AS5; coverage |

## Technical Context

**Language/Version**: Rust stable, edition 2024, MSRV 1.85; C header through cbindgen 0.29.4; Swift 6 package.
**Primary Dependencies**: none new (the core stays `no_std` + `alloc`; `core::fmt` and `str::parse` for the numbers).
**Storage**: the per-slot block in the engine (reserved at construction); the record in the caller's buffer.
**Testing**: `cargo test --all-features`, the harness's seven-rate matrices, golden text records, the C smoke, the Swift tests, `cargo llvm-cov`.
**Target Platform**: the six golden targets, iOS device and simulator.
**Project Type**: library workspace.
**Performance Goals**: the processing path within 3 % of 0.19 (5 % on the runner); the writer and reader linear in the record's length, off the callback.
**Constraints**: no allocation in the writer, the reader, the maps or the processing path; deterministic bytes on every platform; panic-safe C.
**Scale/Scope**: a record of about 70 lines plus one line per event and entry (a 60-minute recording with thousands of events is a few hundred KB of text).

## Constitution Check

- Portable core, no platform dependencies: yes (text formatting through `core::fmt`).
- Stable C API, panic-safe, no Rust types: yes (P10).
- Real-time safe processing path: yes (the block is written at the recording's start, the totals on cold paths; the writer and reader are off the callback like export/import).
- Deterministic: yes (the record's bytes are a function of the engine's data; the digest of the stored form).
- Sample-rate agnostic: yes (the maps take the rate plan; the record carries the host rate).
- Test-first with measurable tolerances: the matrix, the doctored set, the maps' one-device-sample tolerance.
- Independently testable: each story has its own test binary.
- Coverage ≥ 80 % and within 1 point: SC-007.
- Simplicity: no new assumption; the record is data.

Post-design re-check: no violation; the `SpeedStage` factoring (P7) is the one refactor on an audio path, guarded by two goldens and the A/B.

## Project Structure

### Documentation (this feature)

```
specs/020-session-contract/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/api.md
├── contracts/record-form.md
└── tasks.md (next)
```

### Source Code (repository root)

```
crates/rr_dr60/src/
├── record.rs          # the writer, the reader, RunData, RecordReadout, the cursors (new)
├── digest.rs          # fnv1a64 (new)
├── timeline.rs        # kept_to_input, input_to_kept, onepass_output_to_input, place_at (new)
├── input.rs           # samples_from_i24 (new)
├── transport.rs       # SlotBlock, the stop fields, new_at_counter's plumbing
├── changes.rs         # ChangeEntry::stream
├── stages/speed.rs    # SpeedPlace factored out
├── pipeline.rs        # new_at_counter, processing_record_*, recording_digest
└── error.rs           # five statuses
crates/rr_dr60_ffi/src/lib.rs, cbindgen.toml, include/rr_dr60.h, tests/c/smoke.c
crates/rr_dr60_harness/src/record_checks.rs, examples/{re_render.rs, record_rows.rs}, tests/{us1_record_contract, us2_record_read, us3_record_timeline, record_contract_matrix, record_contract_edge_cases, golden_record, record_timing}.rs, golden/golden-record-v1-<rate>.txt
swift/Sources/RRDR60/ReRender.swift, swift/Tests/...
scripts/ab-timing.sh (record_contract), .github/workflows/ab-timing.yml
docs/guide.md, CHANGELOG.md, docs/product/*, specs/015 and 017 pointers
```

## Complexity Tracking

None: no constitution deviation.
