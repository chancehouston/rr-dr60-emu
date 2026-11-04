# Implementation Plan: The one-pass output map

**Branch**: `028-onepass-output-map` | **Date**: 2026-11-04 | **Spec**: [spec.md](spec.md) (approved by the owner 2026-11-04, D1 – D8; D2, D3, D4 and D5 restated with the owner's answers to the plan reviews, 2026-11-04, in its Clarifications) | Plan reviewed (coach 1 – 17, hardware P1 – P8, product M1 – M6, applied below); the owner's plan sign-off next.

## Summary

Two engine reads on `Pipeline` close engine issue #82:
- `one_pass_input_at_output(position, events) -> InputAlignment`: the input behind a one-pass output, with spec 020's direct-path window;
- `one_pass_splice_output_position(event) -> u64`: where a reported splice or muted region is **heard** on the output.

Both are on the 1 kHz group-delay convention and switch at the heard point `S(e) = e.output_position + C + L_post`. They take every value from the engine.

The documentation of the existing map, of `VasEvent`, of spec 026's report latency, of the C header and of the guide is corrected (a `### Fixed (#82)` entry). Two C functions on the handle and two Swift statics are added. No audio change; version 0.95.0.

## Technical context

Rust 2024, MSRV 1.85, `no_std` + `alloc` core; C API by cbindgen; Swift package. No new dependencies. The reads are integer arithmetic over the caller's event slice and values the engine already holds.

## Constitution check

- **Real-time safe.** No allocation and no lock; the work is linear in the events passed (D5). Saturating arithmetic, so a crafted list cannot panic or wrap (hardware P6).
- **Deterministic.** Integer arithmetic only.
- **Test-first with an independent oracle.** Two measured oracles, neither derived from the rule:
  - the labelled gap, measured on the audio;
  - record-then-play through `kept_to_input`, an exact oracle that also found the L_post lead.

  The workbench's rule is a reported regression print.
- **Traceability.** No new modeled behaviour and no new magic number:
  - C is `paths::codec_delay_host_samples` (spec 008).
  - L_pre is `RecordPath::pre_encoder_latency` (spec 020).
  - L_post is L − C − L_pre, pinned equal to `playback_latency_samples()`.
  - The window constants are spec 020's.
  - The stimulus levels and frequencies are test conventions.
- **Claims policy** (product M6). The 30 ms before a splice is "the end of the hang: the last input the gate kept before it paused, below its threshold (the capsule's hiss, a quiet tone or quiet speech)". It is never called silence; silence (digital zeros) is what mute mode stores. The guide text says "pause reports" (the denylist's stem "event" is allowed only in code spans) and "the onset is not measured" (coach 8). `guide_onepass_map` runs `wording::first_hit` and `first_claim_hit` on the new paragraphs.
- **C ABI (spec 022).** No new struct: the result is the existing sized `RrDr60InputAlignment`, filled by `write_sized`. No new status. Two functions are appended, and the promise runs against six revisions.

## Research

- **R-00 Reproduce first** (coach 1). Before any core code, `onepass_map_matrix::onepass_map_lead_reproduction` prints the following, and implementation proceeds only if they match:
  - the record-then-play deviation of the signed draft's rule and of the heard-point rule, around each splice, at seven rates, coder on and off, in drop mode;
  - the mute-mode 5 ms window levels at 48 kHz.

  Expected, from the hardware review's scratch run:
  - the draft's rule wrong by `input_length` on `[pos + C, S(e))` at every rate from 8 kHz up (702 outputs over three splices at 48 kHz);
  - the heard-point rule with 0 failures in 28 cells;
  - mute mode at −46 dB before `S(e)` and −77 dB after it.

  The test keeps running as the record (reported, not asserted).

- **R-01 The clock** (D1; hardware P7). `OnePassClock` (crate-private, built per call from the engine; nothing cached, so `reconfigure` cannot leave it stale) has these fields:
  - `latency`: `latency_samples()`;
  - `c`: `paths::codec_delay_host_samples(rate)` when `settings.one_pass_codec_runs()`, else 0;
  - `l_pre`: `one_pass.rec.pre_encoder_latency()`;
  - `l_post`: `latency − c − l_pre`;
  - `drop`: `settings.one_pass_vas_runs() && settings.vas.mode == VasMode::Drop`;
  - `half`: `RatePlan::delay_host_samples()`;
  - `codec`: whether the coder runs;
  - `rate_l` and `rate_m`: the rate plan's ratio (renamed from `l`/`m` so `l` means one thing; coach 2).

  Pins:
  - `l_post == playback_latency_samples()`, reported per configuration (0 / 40 / 81 / 221 / 241 / 442 / 481 at the seven rates in the test configuration);
  - `c == latency_samples() − one_pass_report_latency_samples()`.

  The drop read is stricter than `timeline::drops`, which uses `vas.enabled`. The two differ only at taps where no events exist, so they agree there.

- **R-02 The centre** (D2, revised).
  - `S(e) = e.output_position.saturating_add(c + l_post)`.
  - `R(o)` is the sum, saturating, of `e.input_length` over the events in order while `S(e) ≤ o` (drop mode; 0 in mute mode). The take-while ends at the first event not yet heard.
  - `raw = o.saturating_add(R(o))`, and `centre = raw − latency`, clamped to 0 when `raw < latency`.

  Equivalently `onepass_output_to_input(o − c − l_post, l_pre, events, drop)` for `o ≥ c + l_post`. The core's test asserts the identity on random lists. At `S(e)` the centre is `pos + R_incl − l_pre`, spec 020's map (a) within one host sample.

- **R-03 The window** (D3; hardware P2, P3).
  1. `first = centre − half` and `last = centre + half`, saturating, with `clamped` when `centre < half`.
  2. With the coder running:
     - `q = o − c − l_post` (when `o < c + l_post`: `q = 0` and `clamped`);
     - `k = ⌊q·rate_l / rate_m⌋` (u128): the newest kept sample behind o, since ⌈k·m/l⌉ ≤ q ⇔ k ≤ q·l/m;
     - `frame_start = ⌊k/120⌋·120` (frames are anchored at kept sample 0 of each stream: the encoder is new at construction and `reconfigure`, and `OnePassCodec::reset` runs at `reset`; muted samples count as kept);
     - `lo = frame_start − PAST_REACH` (132; saturating, with `clamped` when `frame_start < 132`) and `hi = frame_start + FRAME + LOOKAHEAD − 1` (179);
     - each end x maps as `in(x) = onepass_output_to_input(⌈x·rate_m / rate_l⌉, l_pre, events, drop)` (u128): the events' own clock, through the latency to the encoder's input, as `kept_to_input` uses L_pre;
     - `first = min(first, in(lo) − half)` and `last = max(last, in(hi) + half)`.
  3. There is no end clamp (the one-pass has no stop position).

  A window across a splice skips the removed run.

- **R-04 The splice read** (D3, revised). `one_pass_splice_output_position(e) = S(e)`, the heard point in both modes. In mute mode, `e.output_position` is the region's first muted sample at the interpolator's input, so the same shift applies. The rustdoc names the leading edge `e.output_position + c`, where the resumed sound starts entering the interpolator.

- **R-05 The engine methods.** `Pipeline::one_pass_input_at_output(&self, position: u64, events: &[VasEvent]) -> InputAlignment` and `Pipeline::one_pass_splice_output_position(&self, event: &VasEvent) -> u64`. Both are infallible, `#[must_use]` and real-time safe. The arithmetic lives in `timeline` as crate-private functions taking `OnePassClock`.

  Each rustdoc names:
  - its twin (`timeline::onepass_output_to_input`);
  - the events contract (D5);
  - the heard point and the leading edge;
  - the bisection recipe for the inverse direction.

- **R-06 The documentation** (D4, revised; product M3 – M6; coach 8, 16).
  - **`timeline::onepass_output_to_input`**: the corrected text from spec D4, and its drop rule `one_pass_vas_runs()` and drop. The C comment at lib.rs:8135 says "`vas_enabled` and `vas_mode` drop" and names `rr_dr60_latency_samples`; both change.
  - **`VasEvent` fields and `RrDr60VasEvent`**: one-pass positions are where the sample after the splice (or the region's first muted sample) enters the playback half, heard C + L_post later (`one_pass_splice_output_position`). The recorder's device-sample positions are unaffected.
  - **`one_pass_report_latency_samples`** (spec 026): a dated note (2026-11-04, spec 028). Mapping a pause report's own position with it puts the input L_post earlier than spec 020's convention. The input of a pause is `one_pass_input_at_output(one_pass_splice_output_position(e), events)`.
  - **The guide, § the timeline maps (line ~694)**: "(`latency_samples`)" is replaced by the two reads and the heard point. Spec 020's codec-off figure stays, beside the figures this feature prints.
  - **The guide, § the latency the pause's reports align with (line ~808)**: the corrected guidance with the dated note.
  - **New guide paragraphs**:
    - **"From an input moment to the output"**: bisection over the never-decreasing centre, about log₂ of the output count in reads; an input inside a removed pause lands on that pause's `S(e)`.
    - **"A marked region on the one-pass output"**: map both ends; the input runs from the first centre to the last except the removed gaps (the pause reports whose `S(e)` falls inside the region); the window runs from the first `first` to the last `last`.
    - **"The cost"**: each read walks the pause reports it is given, at about N ns per report on the development machine (the matrix's print). One read is cheap at any length. Mapping every output of a long render costs outputs × reports, so do it off the audio thread or map only the positions you draw.
  - **The real-time lists** in the guide (line ~25), the README and the C crate doc name the two reads.
  - **`us2_cross_check guide_onepass_map`** pins the phrases in each place, fails if `onepass_output_to_input` and `latency_samples` share a guide sentence without `one_pass_input_at_output`, and runs the denylist.

- **R-07 C** (D6; coach 11).
  - `rr_dr60_one_pass_input_at_output(pipeline, position, events, count, out: *mut RrDr60InputAlignment)`, filled by `write_sized`.
  - `rr_dr60_one_pass_splice_output_position(pipeline, event: *const RrDr60VasEvent, out: *mut u64)`.
  - Both go in `host_reads.rs` with its handle pattern and panic guard.
  - Refusal order: the handle, then NULL (`out`, `event`; `events` NULL with `count > 0`), then size.
  - No new status. The header is regenerated after every FFI doc edit.
  - The existing `rr_dr60_onepass_output_to_input` keeps its body; only its comment changes.
  - ci.yml gains a sixth promise step (`scripts/abi-promise.sh ad1ba8d401228c91480f12bd8ff3df68ac489e7f`), and its `miri` step gains `abi_tests::onepass_map`.

- **R-08 Swift** (D6; product M1, coach 14). `OnePassMap.inputAtOutput(_ pipeline: OpaquePointer, position: UInt64, events: [RrDr60VasEvent]) throws -> RrDr60InputAlignment` (built with `.sized`) and `OnePassMap.spliceOutputPosition(_ pipeline: OpaquePointer, event: RrDr60VasEvent) throws -> UInt64`. They live in `OnePassMap.swift`, beside `HostReads.swift`.
  - Any status other than OK throws `RRDR60Error.status`.
  - An empty array passes a NULL base with count 0.
  - No Swift alignment view: the app handles one alignment type on both paths.

- **R-09 The labelled gap** (D7; hardware P4, coach 7, 12). `rr_dr60_harness::onepass_map_checks` (new).
  - **The stimulus**, built with `stimulus::tone` at `peak_phase`, with each region's composition asserted:
    1. a leading f_a (500 Hz, about −46 dBFS) for the hang + 200 ms, then f_b (1500 Hz) for 0.3 s;
    2. three bursts, 0.3 s of −20 dBFS 1 kHz, each followed by f_a for the hang + 200 ms and then f_b up to the next burst (1.2 s gaps);
    3. after the third burst, f_a for the hang + 200 ms, then `one_pass_drain_samples()` of silence.
  - **The splice counts**: three splices per drop cell and three regions per mute cell, asserted, with every span `[pos, S(e))` inside the render.
  - **The cells** (the microphone and the output chain bypassed, `Tap::AfterPlayback`, hang 100 ms, level 3):
    - coder on and off × drop and mute;
    - a fifth cell, the default engine at `RouteOutput` with the microphone on, coder on, in drop mode, with the hiss asserted at least 12 dB below f_a.
  - **The readers**, using `stimulus::tone`, `analysis::db` and `detmath::sqrt` (the harness bans `sin`, `log10`, `powf`):
    - Goertzel levels over the middle two thirds of a span (whole cycles of 500 / 1000 / 1500 Hz at every rate);
    - 5 ms windows for mute mode;
    - the first output above half the burst's settled peak for drop mode;
    - the label of an input index.
  - **The figures**, printed per rate and cell. The assertion bounds are set from the printed figures (hardware P4: the decoded −46 dBFS label varies about ±3 dB in 5 ms windows).

- **R-10 The record-then-play oracle** (D7 iii; hardware P3, coach 13).
  - **The one-pass input**: the stimulus, plus the record-latency silence, plus (`silence_fed_samples` + 120 + 1) device samples (spec 008 FR-009).
  - **The twin**: a recorder and player with the same settings and seed, the hiss bypassed (the one-pass hiss is keyed to the recording counter).
  - **The mapping**: for each output `o ≥ c + l_post`, `k = ⌊(o − c − l_post)·rate_l / rate_m⌋` (floor, not ceil: hardware P3), then `kept_to_input(k, settings, recorder_events, stop)`.
  - **The recorder's events**: the full list. The stop's held-open resume, one more event than the one-pass reports when the coder is on or in mute mode, lies past the compared region.
  - **Exclusions**: outputs where the twin is end-clamped (the flush, the padding) are excluded and counted.
  - **Tolerances**: the centre within ⌈m/l⌉ host samples, inclusive (44.1 kHz reaches it exactly); the window's ends within one device sample (measured: one host sample).

- **R-11 Mutants** (FR-008; hardware P5, coach 6). These live behind `#[cfg(feature = "__test-hooks")]`: `OnePassMapMutation` and its fold into `OnePassClock`, so the feature-less builds have no dead code (016's lesson). The reads are off the processing path.

  | Mutant | Changes | Fails |
  |---|---|---|
  | `no_c` | c = 0 in the whole clock (centre, window, splice read) | `us1_onepass_map::as1_onepass_map_labelled_gap`, `us2_onepass_map::as1_onepass_map_splice_drop` |
  | `c_twice` | c doubled | `us1_onepass_map::as2_onepass_map_splice_edges`, `as3_onepass_map_record_then_play` |
  | `no_l_post` | the signed draft's rule | `us1_onepass_map::as3_onepass_map_record_then_play`, `us2_onepass_map::as1_onepass_map_splice_drop` |
  | `take_while_on_report_position` | the existing map's switch at `pos` | `us1_onepass_map::as1_onepass_map_labelled_gap` |
  | `clamp_before_r` | an arithmetic mutant no render reaches (a splice needs pos ≥ the hang > L_pre) | `onepass_map_edge_cases::onepass_map_clamp_order` (synthetic) |
  | `c_from_setting` | c whenever `speech_codec_enabled` | `us2_onepass_map::as3_onepass_map_splice_no_codec`, `us3_onepass_map::as1_onepass_map_no_codec` at `Tap::AfterVas` with the codec setting on |

- **R-12 Tests** (coach 3, 4, 15; product M2, M5).
  - **The core**, `timeline::onepass_map_tests`:
    - hand cases;
    - the R-02 identity on random lists;
    - the splice read;
    - the window's parts;
    - `onepass_map_monotone` on random lists;
    - `onepass_map_saturates` (a `u64::MAX` event).
  - **The harness**:
    - `us1_onepass_map`: AS1 – AS7, AS7 being `as7_onepass_map_monotone`, including the bisection landing on `S(e)`;
    - `us2_onepass_map`: AS1 – AS3;
    - `us3_onepass_map`: AS1 – AS3;
    - `onepass_map_matrix`: `onepass_map_seven_rates` (every cell's figures), `onepass_map_lead_reproduction` (R-00), and `onepass_map_read_cost` (release, reported: the read's time at 0, 100 and 1000 reports at 48 kHz);
    - `onepass_map_edge_cases`: `onepass_map_synthetic_close_events`, `onepass_map_ends_paused`, `onepass_map_clamp_order`, `onepass_map_reads_current_settings`, `onepass_map_shortest_hang_far_apart`, `onepass_map_saturates`;
    - `mutation::onepass_map::*` (six);
    - `alloc_free::onepass_map_allocates_nothing`, behind the binary's static mutex;
    - `ffi_parity::onepass_map_parity`;
    - `us2_cross_check::guide_onepass_map`.
  - **The ABI pins**:
    - `us2_abi::spec_028_onepass_map_appended_only` reads `ad1ba8d` against the tree and asserts exactly the two functions added, with no status and no type;
    - `us2_abi::spec_027_appended_only` is restated to read `3890bfe` and `ad1ba8d` (two revisions; 023's lesson).
  - **The C side**: `abi_tests::onepass_map_*`, Miri on the sized fill, and `smoke --onepass-map` diffed against `examples/onepass_map_rows.rs`.
  - **Swift**: `OnePassMapTests` against a fixture the example writes, including the empty array.
  - **Before the gate**, every manifest-pinning and ABI-pinning binary is run. Under `cfg(coverage)`, the audio loops use `measurement_rates()` and the per-rate figure table uses `all_rates()`.

- **R-13 Records and what moves** (D8; coach 10, 17).
  - **Unmoved**:
    - the seventeen `golden-*.json` (`git diff --quiet ce081b8 -- 'crates/rr_dr60_harness/golden/golden-*.json'`);
    - `control-manifest-v1.txt`, byte-identical;
    - `us3_guards_unmoved`'s SHAs;
    - every smoke hash.
  - **Re-blessed once**: `manifest-v1.txt`'s version line and the guide's copied blocks.
  - **Regenerated**: the header and the Swift ABI fixture (`abi-rows-48000.txt`, version line).
  - **Edited**: the README's version row and a "one-pass output map (since 0.95)" paragraph.
  - **Version 0.95.0**: `Cargo.toml` ×4 and `RR_DR60_VERSION_MINOR`.
  - **The CHANGELOG** `[Unreleased]`:
    - `### Added`: the two reads, C and Swift;
    - `### Fixed (#82)`: the existing map's, the events', the report latency's and the guide's documentation;
    - "no audio change".
  - **The PR's `ab-timing` label run** as a no-regression check, with a solo `scripts/ab-timing.sh ce081b8 record_contract` reported.
  - **The post-merge records**:
    - close #82;
    - a workbench issue for SC-005 (adopt the reads; its figures move on L_post outputs toward the heard point);
    - CLAUDE.md's status entry and "Spec 028 reports".

## Traceability

| Item | R | Tests |
|---|---|---|
| D1, FR-001 | R-01, R-05 | `alloc_free::onepass_map_allocates_nothing`, `timeline::onepass_map_tests` |
| D2, FR-002 | R-00, R-02 | `us1_onepass_map::{as1_onepass_map_labelled_gap, as2_onepass_map_splice_edges, as3_onepass_map_record_then_play, as4_onepass_map_existing_and_workbench, as7_onepass_map_monotone}` |
| D3, FR-003 | R-03, R-04 | `us1_onepass_map::{as5_onepass_map_clamped, as6_onepass_map_window}`, `us2_onepass_map::*` |
| D4, FR-004 | R-06 | `us2_cross_check::guide_onepass_map`; the existing map's tests unchanged |
| D5, FR-005 | R-05, R-06 | `guide_onepass_map`, `onepass_map_edge_cases::{onepass_map_shortest_hang_far_apart, onepass_map_reads_current_settings}`, `onepass_map_matrix::onepass_map_read_cost` |
| D6, FR-006 | R-07, R-08 | `ffi_parity::onepass_map_parity`, `abi_tests::onepass_map_*`, `swift test OnePassMapTests`, `us2_abi::spec_028_onepass_map_appended_only`, six promise steps |
| D7, FR-007 | R-09, R-10 | `us1_/us2_/us3_onepass_map`, `onepass_map_matrix::onepass_map_seven_rates` |
| FR-008 | R-11 | `mutation onepass_map::` (six, each shown failing) |
| D8, FR-009 | R-13 | `us2_abi::us3_guards_unmoved`, the golden diff |
| SC-001 | R-09 | US1 AS1, AS3 including the default-engine cell |
| SC-002 | R-04 | US2 AS1, AS2 |
| SC-003 | R-02 | US3 AS1 – AS3 |
| SC-004 | R-07, R-08, R-13 | parity, the guards, the promise steps |
| SC-005 | R-13 | the workbench issue (reported) |

## Build order (test-first)

1. R-00, the reproduction. Stop if it does not match.
2. The core `onepass_map_tests`: red, then green.
3. US1 against the labelled gap and the twin.
4. US2 and US3.
5. The six mutants, each shown failing.
6. C: `abi_tests`, Miri, the smoke, the header.
7. Swift.
8. The documentation, the records and the version bump.
9. The gate.

## Gate

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`
- `RUSTFLAGS="-D warnings" cargo build -p rr_dr60_ffi --release`
- `cargo llvm-cov --all-features --workspace --fail-under-lines 80`, detached
- the MSRV 1.85 build with `-D warnings`
- the header drift check
- `scripts/check-traceability.sh`
- `crates/rr_dr60_ffi/tests/c/run_smoke.sh`
- the Miri filter
- `scripts/abi-promise.sh` at the six full ids
- `scripts/build-xcframework.sh && cd swift && swift test`
- `xcodebuild test -scheme RRDR60-Package -destination 'platform=iOS Simulator,name=iPhone 17,OS=26.0'`

## Project structure

```text
crates/rr_dr60/src/timeline.rs          OnePassClock, onepass_map_*, the corrected rustdoc, unit tests
crates/rr_dr60/src/pipeline.rs          the two reads; the VasEvent and report-latency docs
crates/rr_dr60_ffi/src/host_reads.rs    the two C functions
crates/rr_dr60_ffi/src/lib.rs           the existing map's and RrDr60VasEvent's comments
crates/rr_dr60_ffi/include/rr_dr60.h    regenerated
crates/rr_dr60_ffi/tests/c/smoke.c      --onepass-map
crates/rr_dr60_harness/src/onepass_map_checks.rs
crates/rr_dr60_harness/examples/onepass_map_rows.rs
crates/rr_dr60_harness/tests/{us1,us2,us3}_onepass_map.rs, onepass_map_matrix.rs, onepass_map_edge_cases.rs
swift/Sources/RRDR60/OnePassMap.swift, swift/Tests/RRDR60Tests/OnePassMapTests.swift (+ fixture)
docs/guide.md, README.md, CHANGELOG.md, .github/workflows/ci.yml (the sixth promise step, the Miri filter)
```

## Complexity tracking

None: no new state on the processing path, no new struct, no new status.
