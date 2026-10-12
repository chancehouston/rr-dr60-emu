# Implementation Plan: Record and Playback Paths with a Modeled Memory

**Branch**: `006-record-playback-split` | **Date**: 2026-10-11 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/006-record-playback-split/spec.md` (clarified; coach, clarify and hardware-review decisions in its Clarifications section).

## Summary

The one-pass engine (host input → limiter → conversion → stage 4 → clamp → VAS → stage 10 → conversion → host output) is split into the device's two paths with a modeled memory between them: a **record path** (limiter, conversion, stage 4, clamp, VAS) that appends device-rate samples to a recording, and a **playback path** (stage 10, conversion) that plays a stored recording toward a speaker or earphone route. Record-then-play is bit-identical to the one-pass processing, so the three golden files remain the oracle, and the one-pass calls stay.

Technical approach:

- **One narrowing first** (R-02; spec FR-009, hardware review B1): the one-pass processing rounds the device-rate sample after the VAS to 32-bit precision (the point where the recording is taken; the device's ADC is 8-bit µ-law there, A-003). It is one commit in the existing `DeviceChain`, with a single re-bless of all three golden files and the measured delta in the CHANGELOG, before the refactor and before any split code lands; the refactor into halves is then proven by the unchanged SHA guards. A test-hooks-only constructor keeps the un-narrowed path so the delta stays measurable.
- **Shared halves** (R-01): the device chain splits into a `RecordHalf` (stage 4, clamp, VAS; returns the narrowed `f32` kept sample or a drop) and a `PlaybackHalf` (stage 10). A `RecordPath` is the limiter, the decimator and a `RecordHalf`; a `PlaybackPath` is a `PlaybackHalf` and the interpolator. The one-pass `tick` is a `RecordPath` feeding a `PlaybackPath` sample by sample, so equivalence holds by construction; the one-pass keeps its own instances (its stream state is independent of recordings, FR-009).
- **Memory** (R-03): one `Vec<f32>` reserved once for the configured capacity (`try_reserve_exact`, an error rather than an abort on failure; 86.4 MB at the device's 60 minutes), a span table for up to the configured recording count, time counted in stored device samples. Erase drains a range (bounded memmove, no allocation); `Clone` preserves the capacity.
- **Transport** (R-05): a three-state machine (idle, recording, playing) on `Pipeline`, a route control that is not a setting, per-block reports for both paths, and thirteen refusal reasons as `Error` variants with distinct C status codes.
- **Sample counting** (R-06): the stop flush is the record latency of host silence; playback produces exactly ⌈K·m/l⌉ host samples for K stored samples, then drains ⌈N₁₀·m/l⌉ + 2D host samples of device silence (N₁₀ = 121, stage 10's −100 dB settling length; D the converter delay); both latencies in host samples, each rounded once (their sum equals the one-pass figure at all seven rates).
- **Byte form** (R-04): a 32-byte header (magic, form version 1, device rate, library version, sample count), little-endian `f32` samples, CRC-32; import validates form, version, length, CRC, count and that every sample is finite and non-subnormal before touching the memory.
- **Interface** (R-07; contracts/): new Rust calls on `Pipeline` and the matching C functions; `Settings::memory { capacity_s, max_recordings }`; the C struct grows 80 → 88 bytes by the two capacity fields; status codes 6–19; version 0.6.0.
- **Measurement** (R-08): a `split_checks` harness module: equivalence against the 196 applicable golden entries and against the one-pass processing in the same run at seven rates; the length rule; capacity scenarios at a small configured capacity; the export round trip and the corruption set; the state table; the cross-rate tone fit; the drain level; the latency sum; the footprint; record-plus-play timing in the release timing job.

## Technical Context

**Language/Version**: Rust, edition 2024, MSRV 1.85 (001 R-01). No Python this time.

**Primary Dependencies**: No new ones. The CRC-32 is 20 lines in the core (R-04); the harness keeps `sha2`, `serde_json`, `rustfft`.

**Storage**: The in-memory recording store (R-03): `Vec<f32>` with a reserved capacity of `capacity_s × 6000` samples (21.6 million, 86.4 MB, at the default), plus a span table. The three golden files are re-blessed once (R-09). No disk I/O in the core (the host persists the exported bytes).

**Testing**:
- **Core**: unit tests for the halves and paths (the one-pass equals record-then-play on synthetic input at 6 and 48 kHz; narrowing point; reset per start), the memory (append, limits at the exact sample, erase and renumbering, erase all, `Clone` capacity, the `try_reserve` failure path through an internal `Memory::with_capacity_samples(usize::MAX / 2)`, a deterministic capacity overflow, since any validated capacity is granted lazily on every 64-bit target, plan review P7), the byte form (round trip, each refusal), the transport (every transition and refusal), the drain length and N₁₀ (computed from the committed coefficients and asserted equal to the constant), latencies per rate, settings validation, FFI layout (88 bytes, offsets 76 and 80), status codes and field numbers.
- **Harness**: `split_checks.rs` (R-08) and the tests `us1_record_play`, `us2_memory`, `us3_export_import`, `split_edge_cases`, `split_matrix` (seven rates); `ffi_parity` extended with the record/play loop; `alloc_free` extended (record, play, stop, start, erase, export, import, route); `determinism` extended with a record-then-play partition case; `timing` extended (SC-006); the C smoke test extended (US4).
- **Golden**: the single re-bless in R-02's commit; afterwards the files are the oracle for SC-002 through `split_checks::golden_equivalence`.
- **Coverage**: gate stays at 80 %; the new modules are fully exercised by the unit tests.

**Target Platform**: Unchanged: 8 targets (001 FR-014). The store's reservation is virtual on every target (pages are touched as recordings grow; R-11).

**Project Type**: Library: a Rust crate plus a C ABI.

**Performance Goals**: record plus playback per host sample within 25 % of the one-pass cost at 48 kHz (SC-006; expectation within 10 %: the same operations plus one `f32` push and pop). Start, stop, erase, export and import are not per-block calls; erase moves at most the store's size (≈ 10 ms for 86 MB), export and import run the CRC over the recording (≈ 50 ms for 86 MB, bounded).

**Constraints**:
- Bit-identity (FR-007) at every rate and block partition: the same halves run in the same order with the same `f32` narrowing; `exp`/`ln` from detmath only (unchanged).
- No allocation in the per-block calls, in start, stop, route or the figure reads; the store and the span table reserved at construction or reconfigure; export and import use caller buffers (FR-017).
- The store never grows (SC-007); `Clone` preserves capacity; `reconfigure` keeps the recordings or refuses.
- Golden files change exactly once, in the narrowing commit, before any split code (FR-009).
- No `unsafe` outside `rr_dr60_ffi`; panics never cross the C boundary; new status codes only appended.

**Scale/Scope**: About 1 400 lines of core changes (`paths.rs` ≈ 350 with the halves and the two paths, `memory.rs` ≈ 300, `form.rs` ≈ 200 with the CRC and tests, `transport.rs` ≈ 350 for the state machine and the public calls, `pipeline.rs` reshaped ≈ 150, settings/error/validate ≈ 100), about 500 lines of FFI (functions, structs, mapping, tests, smoke test), about 1 200 lines of harness (`split_checks.rs`, five test files, extensions), docs, the register, CHANGELOG, README, three regenerated golden files.

There are no open NEEDS CLARIFICATION items. The specify, coach, clarify and hardware-review sessions resolved them (spec Clarifications). The plan session fixed two figures: stage 10's settling length at −100 dB relative to its own impulse-response peak is **121** device samples (the review's 112 was an estimate against a different reference; FR-008's text is amended to 121), and the record and playback latencies rounded separately sum exactly to the one-pass figure at all seven rates (R-06 table), so SC-008's "within one host sample" has margin.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | How this plan complies | Pre-research | Post-design |
|---|---|---|---|
| I. Portable Library First | No new dependencies; the core stays `no_std` + `alloc` (the CRC is in-crate); no I/O: the host persists the bytes. The C struct grows at the end by two fields (80 → 88), status codes and field numbers are only appended, the header is regenerated and drift-checked, the smoke test covers the loop (R-07). | ✅ | ✅ |
| II. Evidence-Based Fidelity | Every figure traces: capacity 60 min / 99 items (S-001), 16 Mbit (S-002), erase and numbering (S-001), the exclusive route (S-001), mute while recording (S-002), per-recording reset (A-037, engine decision recorded), accounting (A-038), stop-at-limit and empty items (A-039), the device rate (A-026), the narrowing's physical defence (A-003). The narrowing is a user-visible output change recorded in the CHANGELOG with the measured delta and one re-bless (R-02, R-09). A-004's row is made consistent (R-10). | ✅ | ✅ |
| III. Measured, Test-First Verification | Each FR maps to a `split_checks` function or a core unit test (R-08), written and seen to fail before the story's implementation (tasks enforce the order per story); the golden files are the equivalence oracle (SC-002) after one documented re-bless; the drain and the cross-rate check are measured, not asserted from the design; mutation: a one-sample shift in the stored recording and a wrong CRC are caught. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | The paths are the existing stages plus `f32` rounding (deterministic IEEE); the store is a reserved `Vec`; the per-block calls, start, stop and route do no allocation (asserted by `alloc_free`); allocation happens in `new`/`reconfigure` only, through `try_reserve_exact` so failure is an error, not an abort (R-03). | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | The halves keep every stage bypassable and the one-pass keeps every tap; the split paths ignore the tap (a recorded deviation, Complexity Tracking row 5); the memory and the transport are separate modules; the route is a labelled placeholder for stages 11–13. | ✅ | ✅ |
| VI. Neutral, Honest Communication | README and CHANGELOG say the stored form is the engine's, not the device's; the route is a label until stages 11–13; capacity above the device's is not the device's; the narrowing is inaudible but recorded; nothing about what recordings contain. | ✅ | ✅ |
| VII. Independently Testable Features | US1 is checked by record-then-play against the goldens and the one-pass with the memory at default capacity; US2 by the record path alone at a small capacity; US3 by the export round trip and the corruption set; US4 by C parity and the smoke test. Specs 001–005 keep their isolated configurations through the one-pass calls. | ✅ | ✅ |
| Repo standards | rustdoc on every new public item; SemVer 0.6.0 (breaking: C struct size, output narrowing); Conventional Commits; no audio committed (the harness generates stimuli; the form tests build bytes in code). | ✅ | ✅ |

**Gate result**: PASS. Six complexity items are recorded below, as Principle V requires.

## Project Structure

### Documentation (this feature)

```text
specs/006-record-playback-split/
├── plan.md              # This file
├── research.md          # Phase 0: decisions R-01 – R-13
├── data-model.md        # Phase 1: paths, memory, transport, settings, errors, per-rate table, byte form
├── quickstart.md        # Phase 1: validation walk-through
├── contracts/
│   ├── rust-api.md      # new Pipeline calls, types, errors, settings, test hooks
│   ├── c-api.md         # RrDr60Settings 80 → 88, new statuses 6–19, functions, structs, smoke test
│   ├── recording-form.md# the exported byte form (version 1) and its validation
│   └── golden-format.md # the single narrowing re-bless; the SC-002 reproduction rule
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/rr_dr60/src/
├── lib.rs                      # crate docs (two paths, memory, state, route, stored form); re-exports
├── settings.rs                 # MemorySettings { capacity_s, max_recordings } (+ DEVICE); Settings::memory; Route
├── error.rs                    # Error: 13 new variants (R-05); Setting::MemoryCapacityS, MemoryMaxRecordings
├── validate.rs                 # capacity_s 1 ..= 86_400, max_recordings 1 ..= 9_999
├── paths.rs                    # NEW: RecordHalf, PlaybackHalf, RecordPath, PlaybackPath, EventCounters (moved),
│                               #      narrow_device(), latency helpers, N10 and drain length (R-01, R-02, R-06)
├── memory.rs                   # NEW: Memory { store: Vec<f32>, spans, capacity }, Span, MemoryStatus, append/
│                               #      erase/erase_all/import_span, footprint, Clone (R-03)
├── form.rs                     # NEW: export_len/export/import of the version-1 byte form; crc32 (R-04)
├── transport.rs                # NEW: State, EngineState, RecordInfo, StopOutcome, PlayInfo, StopReason; the
│                               #      start/record/stop/play/erase/export/import/route methods on Pipeline (R-05)
├── pipeline.rs                 # Pipeline { settings, one_pass: OnePass, recorder: RecordPath, player: PlaybackPath,
│                               #      memory: Memory, transport }, the one-pass tick built from the halves, reset,
│                               #      reconfigure (keeps the memory or refuses), __with_device_narrowing hook
└── stages/                     # unchanged

crates/rr_dr60_ffi/
├── src/lib.rs                  # RrDr60Settings + memory_capacity_s (76), memory_max_recordings (80); 88 bytes;
│                               # RrDr60Status 6–19; RrDr60SettingField 14, 15; RrDr60Route, RrDr60EngineState,
│                               # RrDr60RecordInfo, RrDr60StopOutcome, RrDr60PlayInfo, RrDr60MemoryStatus;
│                               # 17 new functions (contracts/c-api.md); error mapping; layout tests
├── include/rr_dr60.h           # regenerated
└── tests/c/smoke.c             # record → stop → export → import into a second handle → play; memcmp with Rust

crates/rr_dr60_harness/
├── src/split_checks.rs         # NEW (R-08): golden_equivalence, live_equivalence, length_rule, cross_rate_tone,
│                               #      capacity scenarios, round_trip, corruption set, state table, drain_level,
│                               #      latency_sum, footprint
├── src/configs.rs              # memory capacity helper (small capacities for US2); `after_vas_playback_off`
├── src/golden.rs               # docs: the 0.6 narrowing re-bless; `applicable_entries()` for SC-002
├── golden/golden-v1.json       # re-blessed once (112)
├── golden/golden-agc-v1.json   # re-blessed once (42)
├── golden/golden-vas-v1.json   # re-blessed once (63)
├── tests/us1_record_play.rs    # NEW (US1)
├── tests/us2_memory.rs         # NEW (US2)
├── tests/us3_export_import.rs  # NEW (US3)
├── tests/split_edge_cases.rs   # NEW (edge cases; release-mode slow cases --ignored)
├── tests/split_matrix.rs       # NEW: the seven-rate matrix with --nocapture report
├── tests/golden*.rs            # SHA guards updated once; entry counts unchanged
├── tests/ffi_parity.rs, alloc_free.rs, determinism.rs, timing.rs, mutation.rs  # extended
└── tests/narrowing_delta.rs    # NEW: the measured FR-009 delta against the un-narrowed hook (report + bound)

docs/hardware/assumptions.md (A-037 – A-039 new; A-004 row 66.7 → 69.9 min, 2²⁴ bits), signal-chain.md (rows 8, 11–13);
CHANGELOG.md (0.6.0); README.md (Recording and playback section; fidelity table); CLAUDE.md; .github/ISSUE_TEMPLATE/hardware_evidence.yml
```

**Structure Decision**: the existing three-crate workspace. The core gains four modules rather than one large `pipeline.rs`: the halves and paths (shared by both engines), the memory (data), the byte form (serialization, no I/O) and the transport (state machine and public calls). `Pipeline` stays the single public handle, as the spec's one-engine state model and the C API's single opaque pointer want.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Three sets of path instances (one-pass, recorder, player) instead of one shared set | FR-009: the one-pass stream has its own state; a recording's reset (FR-004) must not reset a one-pass stream the host may resume later; the memory cost is a few kilobytes of filter state and ring buffers | Sharing the instances and resetting on every start: the one-pass stream would restart whenever the host recorded, a silent behaviour change for existing callers |
| A user-visible output change (the `f32` narrowing) with a golden re-bless, for a feature that adds no stage | A 4-byte store cannot reproduce 64-bit intermediates bit for bit (review B1, measured); the alternatives are a 173 MB default engine or giving up the bit-exact oracle | 64-bit store (rejected by the user: footprint); 1-LSB tolerance (rejected: the goldens stop being an oracle and the one-pass cannot be built on the paths) |
| A versioned byte form with a CRC inside the core | FR-014: bit-exact round trip, refusal of damaged and foreign input, a version field so encoded frames can replace samples later without an interface change; the host must not need to know the layout | Exposing raw samples for the host to serialize: every host invents a format, none carries the version or integrity check, and the codec change breaks them all |
| `Error` grows by fourteen variants and `RrDr60Status` by fourteen codes (6–19) | Each refusal reason in the spec is a distinct, testable outcome the app shows to a user; one "invalid state" code would hide which ("capacity too small" reuses `InvalidSetting` with the two new field names) | A single `InvalidState` error with a reason string: no strings in `no_std` without allocation, and C callers need codes |
| The record and playback paths ignore the tap setting | The recording must be the device's stored stream (after the VAS, FR-001) and playback must run stage 10 (FR-002); the one-pass keeps every tap, so nothing is lost | Honouring the tap on the paths: a recording that is not the device's, and a second meaning for one setting (clarify Q5) |
| A test-hooks-only un-narrowed path (`__with_device_narrowing`) stays in the core | FR-009's measured delta must remain reproducible after later changes; the flag and branch exist only under `cfg(feature = "__test-hooks")`, so the shipped build is unchanged | Deleting the hook after the re-bless: the CHANGELOG figure could never be re-measured, and the next narrowing question (µ-law, A-030) would rebuild it |
