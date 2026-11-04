# Implementation Plan: The one-pass output map

**Branch**: `028-onepass-output-map` | **Date**: 2026-11-04 | **Spec**: [spec.md](spec.md) (approved by the owner 2026-11-04, D1 – D8) | Plan reviews (coach, hardware, product) next, then the owner's plan sign-off.

## Summary

Two engine reads on `Pipeline` close engine issue #82:
- `one_pass_input_at_output(position, events) -> InputAlignment`: the input behind a one-pass output, with spec 020's direct-path window;
- `one_pass_splice_output_position(event) -> u64`: where a reported splice or muted region reaches the output.

Each read takes the latency, the coder's delay and the gate's mode from the engine. The existing map keeps its behaviour, and its documentation, the `VasEvent` docs, the C header and the guide are corrected. Two C functions on the handle and two Swift statics are added. No audio change; version 0.95.0.

## Technical context

Rust 2024, MSRV 1.85, `no_std` + `alloc` core; C API by cbindgen; Swift package. No new dependencies. The reads are pure arithmetic over the caller's event slice and three values the engine already holds (`latency`, `settings`, the rate plan).

## Constitution check

- **Real-time safe.** No allocation, no lock; linear in the events passed (spec D5). The guide states the cost for a long list.
- **Deterministic.** Integer arithmetic on the engine's fixed values; no floating point in the map (the latency is already an integer).
- **Test-first with an independent oracle.** The labelled gap is measured on the audio (spec D7), not derived from the rule. Record-then-play through `kept_to_input` is the exact second oracle. The workbench rule is a regression pin only.
- **Traceability.** No new modeled behaviour and no new magic number: C is `paths::codec_delay_host_samples` (spec 008), the window constants are spec 020's (`codec::PAST_REACH`, `FRAME_SAMPLES`, `LOOKAHEAD_SAMPLES`, the decimator's half-length), and the stimulus levels and frequencies are test conventions.
- **Claims policy.** The new text describes the clocks only. The 30 ms before a splice is "the hang's end, the input below the gate's threshold that the gate kept", never "silence" (spec Assumptions, product M1). The house denylist runs on every new string.
- **C ABI (spec 022).** No new struct: the result is the existing sized `RrDr60InputAlignment` (spec 020), filled by `write_sized`. Two functions are appended and the promise runs against six revisions.

## Research

- **R-01 Where the values come from (D1).** `latency_samples()` gives L. C = `paths::codec_delay_host_samples(rate)` when `settings.one_pass_codec_runs()`, else 0 (`latency_samples() − one_pass_report_latency_samples()` gives the same, pinned by a test). Drop is `settings.one_pass_vas_runs() && settings.vas.mode == VasMode::Drop` (spec 026 D1's read: a front tap or the gate bypassed removes nothing). A crate-private `OnePassClock { l: u64, c: u64, drop: bool, half: u64, codec: bool, l_dev: u64, m_dev: u64 }` is built on demand from the engine, with no cached state, so `reconfigure` cannot leave it stale.

- **R-02 The centre (D2).** `R(o) = Σ e.input_length` over the events, in order, while `e.output_position + c ≤ o` (drop mode; 0 in mute mode). `raw = o + R(o)`; `centre = raw − l`, clamped to 0 when `raw < l`. With `c = 0` this is `timeline::onepass_output_to_input(o, l, events, drop)` bit for bit, and the core's test asserts that on random lists. The take-while is the existing map's: events past the position end the walk.

- **R-03 The window (D3, clarified).** The window has the same parts as `kept_to_input`:
  1. **The decimator's half-length** around the centre: `first = centre − half`, `last = centre + half`, with `clamped` when `centre < half`. `half` is `RatePlan::delay_host_samples()`.
  2. **With the coder running, the frame's reach.** The one-pass's kept device index behind output o is `k(o) = ⌊(o − c − l_post)·l_dev / m_dev⌋`, where `l_post` is the playback half's share of L. That expression is how `kept_to_input` reaches the frame. The plan uses a simpler and exact route, one the hardware review checks:
     - `frame_start = ⌊k / 120⌋ · 120` on the stream's kept clock. The encoder resets at construction, `reset` and `reconfigure`, so frames are anchored at kept sample 0.
     - `lo = frame_start − PAST_REACH` and `hi = frame_start + FRAME + LOOKAHEAD − 1` (kept device indices).
     - Each end goes to the report clock's output position, `p = ⌈x·m/l⌉` (the counters' own rule, `EventCounters::position`), and from there to the input by the existing map with `one_pass_report_latency_samples()` (the events' own clock; R-02 with `c = 0` and `l − c`).
     - `first = min(first, lo_input − half)` and `last = max(last, hi_input + half)`.

     `k(o)` itself is avoided: the centre's report-clock position is `o − c`, so `k` is the last kept index whose report-clock position is at or before `o − c`, and `k = ⌊(o − c − 1)·l/m⌋ + 1` inverts `⌈k·m/l⌉`. It is checked against the record-then-play twin within one device sample (US1 AS6).
  3. **No end clamp** (the one-pass has no stop position).

  A window across a splice skips the removed run, because each end is mapped through the events.

- **R-04 The splice read (D1, D3).** `one_pass_splice_output_position(e) = e.output_position + c`. In mute mode `e.output_position` is the region's first muted output on the report clock, so the same shift applies (hardware B3). With the coder off it returns `e.output_position`.

- **R-05 The engine methods.** `Pipeline::one_pass_input_at_output(&self, position: u64, events: &[VasEvent]) -> InputAlignment` and `Pipeline::one_pass_splice_output_position(&self, event: &VasEvent) -> u64`. Both are infallible, `#[must_use]` and real-time safe. The arithmetic lives in `timeline` as crate-private functions taking `OnePassClock`, so the core's unit tests can drive hand cases. Each rustdoc names its twin (`timeline::onepass_output_to_input`, `one_pass_report_latency_samples`) and the events contract (D5).

- **R-06 The documentation (D4).**
  - `timeline::onepass_output_to_input` gets the corrected sentence from spec D4.
  - The `VasEvent` field docs say the one-pass positions are on the clock before the speech coder, that the region or splice reaches the output C later (`one_pass_splice_output_position`), and that the recorder's device-sample positions are unaffected.
  - The C header's comments on `rr_dr60_onepass_output_to_input` and `RrDr60VasEvent`: lib.rs:8135 currently names `rr_dr60_latency_samples`, which becomes the corrected pair.
  - The guide's § the timeline maps (line ~694) keeps spec 020's codec-off figure and adds the codec-on figure this feature prints. § the latency the pause's reports align with (line ~808) gains "for the events' own positions; for output positions, `one_pass_input_at_output`". A new paragraph covers a marked region across a splice (product M4) and the cost (D5).
  - `us2_cross_check guide_onepass_map` pins the key phrases in each place and runs the denylist.

- **R-07 C (D6).**
  - `rr_dr60_one_pass_input_at_output(pipeline, position, events, count, out: *mut RrDr60InputAlignment)`, filled by `write_sized` with `RrDr60InputAlignment`'s published list.
  - `rr_dr60_one_pass_splice_output_position(pipeline, event: *const RrDr60VasEvent, out: *mut u64)`.
  - Both live in `host_reads.rs` beside spec 026's handle reads and use its `read_value` pattern and the panic guard.
  - Refusal order: the handle, then NULL (`out`, `event`; `events` NULL with `count > 0`), then size.
  - No new status.
  - The header is regenerated. The existing `rr_dr60_onepass_output_to_input` keeps its body, and only its comment changes.
  - A sixth promise step in ci.yml: `scripts/abi-promise.sh ad1ba8d401228c91480f12bd8ff3df68ac489e7f`.

- **R-08 Swift (D6).** `OnePassMap.inputAtOutput(_ pipeline: OpaquePointer, position: UInt64, events: [RrDr60VasEvent]) throws -> InputAlignment` and `OnePassMap.spliceOutputPosition(_ pipeline:, event:) throws -> UInt64`, in a new `OnePassMap.swift` beside `HostReads.swift`. They pass `withUnsafeBufferPointer` over the events and throw `RRDR60Error.status`. `InputAlignment` is the Swift view the record-timeline code already has, if there is one; if not, a small struct with the four fields.

- **R-09 The labelled-gap stimulus and readers (D7).** `rr_dr60_harness::onepass_map_checks` (new):
  - `labelled_gap(rate) -> (Vec<f32>, Labels)`:
    - three −20 dBFS 1 kHz bursts of 0.3 s, separated by gaps of 1.2 s;
    - each gap: f_a = 500 Hz at about −46 dBFS for hang + 200 ms after the burst's end, then f_b = 1500 Hz at the same level up to the next burst;
    - a leading 0.3 s of f_b;
    - built with `stimulus::tone` at `peak_phase`.
    
    `Labels` gives each input sample's region (burst j, f_a after j, f_b before j).
  - Settings: `configs::settings_011`-style front (the microphone and the output chain bypassed, `Tap::AfterPlayback`), hang 100 ms, level 3, in four cells (coder on/off × drop/mute).
  - The readers:
    - the Goertzel level of f_a and f_b over the middle two thirds of a span, with `analysis::db`;
    - the decoded level of a window (RMS, `analysis::db`);
    - the label of an input index.
    
    The harness bans `f64::sin`, `log10` and `powf`, so the code uses `stimulus::tone`, `analysis::db` and `detmath::sqrt`.
  - Figures are printed per rate and cell and asserted at spec D7's bounds: f_a at least 12 dB above f_b in every span; 100 % f_a labels for the new map; 0 % for the existing map with `latency_samples()` in the coder-on drop cell. The records quote only printed figures.

- **R-10 The record-then-play oracle (D7 v, US1 AS3, AS6).** Spec 008 FR-009: the one-pass output, skipped by C, equals record-then-play when the one-pass input is followed by the record-latency silence. For each one-pass output `o ≥ c`:
  - the player's output index is `o − c`;
  - its kept index is the device index the player's output reads, at the player's uniform latency: `k = ⌈(o − c − L_play)·l/m⌉`, with `L_play` = `playback_latency_samples()` at level 3;
  - its alignment is `kept_to_input(k, settings, recorder_events, stop)`.

  The check: the method's centre within one device sample (`⌈m/l⌉` host samples) of `kept_to_input`'s, and the window's ends within one device sample of its ends. The recorder's events are device-sample positions (spec 006), which `kept_to_input` expects; the one-pass's events are host positions, which the method expects. The two runs use the same stimulus and seed. The hiss is bypassed (spec 024's lesson: the one-pass hiss is keyed to the recording counter).

- **R-11 Mutants (FR-008).** Behind `__test-hooks`, `OnePassMapMutation::{no_c, c_twice, take_while_on_report_position, clamp_before_r, c_from_setting}`. Each is folded into the `OnePassClock` built for the call, so there is no branch in a hot path (the reads are not on a processing path). The failing tests:
  - `no_c`, `c_twice` and `take_while_on_report_position`: `us1_onepass_map::as1_onepass_map_labelled_gap`;
  - `clamp_before_r`: `onepass_map_edge_cases::clamp_order` (an event list whose first splice falls before the latency in a synthetic case);
  - `c_from_setting`: `us3_onepass_map::as1_onepass_map_no_codec` at `Tap::AfterLimiter` with the codec setting on.

- **R-12 Tests.**
  - The core: `timeline::onepass_map_tests` (hand cases, equality with the existing map at `c = 0` on random lists, the splice read, the window's parts).
  - The harness: `us1_onepass_map` (AS1 – AS6), `us2_onepass_map` (AS1 – AS3), `us3_onepass_map` (AS1 – AS3), `onepass_map_matrix` (seven rates × four cells, the figures), `onepass_map_edge_cases` (`synthetic_close_events`, `ends_paused`, `clamp_order`, `events_from_another_stream` (documented, asserted as "the reads use the current engine"), `shortest_hang_far_apart`), `mutation onepass_map::` (five), `alloc_free onepass_map`, `ffi_parity onepass_map`, `us2_cross_check guide_onepass_map`, and `us2_abi spec_028_appended_only` (the appended items, the frozen pair, the seventeen guards against `ce081b8`).
  - The C smoke: `smoke --onepass-map` diffed against `examples/onepass_map_rows.rs`, plus Miri on the sized fill (`abi_tests::onepass_map`).
  - Swift: `OnePassMapTests` against a fixture written by the example.
  - Under `cfg(coverage)` the audio loops use `measurement_rates()` and the per-rate figure table `all_rates()` (CLAUDE.md's rule).

- **R-13 The version and records.**
  - Version 0.95.0: `Cargo.toml` ×4, `RR_DR60_VERSION_MINOR`, the header, the Swift ABI fixture, and the stage manifest's version line re-blessed.
  - CHANGELOG `[Unreleased]` → `### Added` (the two reads) and `### Changed` (the documentation corrections), with a note that no audio changed.
  - The guide's copied blocks re-blessed.
  - No golden file and no smoke hash move: `us2_abi` reads the seventeen against `ce081b8`.

## Project structure

```text
crates/rr_dr60/src/timeline.rs          OnePassClock, onepass_map_*, the corrected rustdoc, unit tests
crates/rr_dr60/src/pipeline.rs          one_pass_input_at_output, one_pass_splice_output_position; VasEvent docs
crates/rr_dr60_ffi/src/host_reads.rs    the two C functions
crates/rr_dr60_ffi/src/lib.rs           the existing map's and RrDr60VasEvent's comments
crates/rr_dr60_ffi/include/rr_dr60.h    regenerated
crates/rr_dr60_ffi/tests/c/smoke.c      --onepass-map
crates/rr_dr60_harness/src/onepass_map_checks.rs
crates/rr_dr60_harness/examples/onepass_map_rows.rs
crates/rr_dr60_harness/tests/{us1,us2,us3}_onepass_map.rs, onepass_map_matrix.rs, onepass_map_edge_cases.rs
swift/Sources/RRDR60/OnePassMap.swift, swift/Tests/RRDR60Tests/OnePassMapTests.swift (+ fixture)
docs/guide.md, CHANGELOG.md, .github/workflows/ci.yml (sixth promise step)
```

## Complexity tracking

None: no new state on the processing path, no new struct, no new status.
