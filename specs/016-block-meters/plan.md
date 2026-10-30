# Implementation Plan: Per-Block Meters

**Branch**: `016-block-meters` | **Date**: 2026-10-30 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/016-block-meters/spec.md` (approved D1 – D7, 2026-10-29)

## Decisions and findings (owner: read this section)

The spec's D1 – D7 stand. The three plan reviews (coach 1 – 13, hardware P1 – P12, product M1 – M7, 2026-10-30) are applied at their recommended defaults; the two items below are the ones a reviewer put to the owner, each taken at the default and open to reversal at the plan sign-off:

1. **Field names** (product M3): the stage 4 figures are `record_filter_peak` and `adc_clamp_count` (the count names the setting it reports, `adc_clamp_enabled`; a C field name is frozen after 0.15.0).
2. **The A/B fails CI when the meters exceed the budget** (hardware P1): the `meters` mode of the A/B script asserts the two budgets and exits non-zero on the `ab-timing` label job; no cap on the C `struct_size` for the zero-fill (P8 c; the caller's writable-bytes obligation is in the contract).

Plan choices, each traced to the research:

| # | Plan choice | Why |
|---|---|---|
| P1 | The accumulators live in the path objects (`RecordPath`, `RecordHalf`, `PlaybackPath`, `OutputChain`) and are finalized once per non-empty call into one `Meters` on the `Pipeline` (R-01, R-04). | Five call sites (one-pass, record, flush, play, drain) share one code path per figure; the transcendental work runs once per call. |
| P2 | The gain reduction is kept as the call's largest limiter entry voltage (inside the above-floor branch) and converted once at finalization; the charge is `vc / v_limit` (R-02, R-07). | The applied gain is a strictly decreasing function of the entry voltage (verified): one compare, no per-sample `ln`. |
| P3 | Peaks accumulate as f64 compare-selects (`if a > p`) and are narrowed once per call with the output's rounding (R-02). | Exact against the tap oracle (`narrow_out` is monotone and odd-symmetric, verified); no NaN-aware max on the hot path. |
| P4 | Two construction-time flags gate the playback group: `meter_playback` (the player, or the one-pass at a tap at or after stage 10) finalizes the reconstruction peak, `meter_route` (the chain's tap is the route output) gates the route figures; the record group follows the data (R-03). | The interpolator runs at `AfterRecord`/`AfterVas`/`AfterDecoder` too, and stage 10 bypassed is the identity (D7). |
| P5 | A `__test-hooks` trace of stage 4's pre-clamp stream is the record-filter-peak oracle at rates other than 6 kHz; the `AfterRecord` tap at 6 kHz with the clamp off is the hook-free one (R-06). | The device-rate stream is in no output buffer at six rates. |
| P6 | A harness reference gate written from spec 003's data model, the engine's paused reports and the splice lengths (device units in the recorder, the cumulative-floor host units in the one-pass) are the VAS oracle (R-08). | Drop-mode events are reported at the resume; the state is the oracle, the events corroborate the drop count. |
| P7 | C: `RrDr60Meters` (56 bytes, size field, zero-filled tail, f32/u32/u64 fields) and one function `rr_dr60_meters` on a `*mut` handle; no new status (R-05). | D5; the existing reports untouched; the poisoned-handle rule as every call. |
| P8 | Cost by a `meters_timing.rs` test (the default front with the codec bypassed, and every stage off) built against the 0.14 API, run in both builds by a new `meters` mode of the A/B script, 45 alternating rounds, asserted (5 %, and the same absolute nanoseconds for every stage off; 12 % on the runner) (R-09). | No off switch (D6); the script as it stood compared unlike builds and only printed. |
| P9 | `golden-meters-v1.json` with per-block rows for four 0.5 s stimuli × six configurations (two with a 50 ms hang so the gate pauses and resumes within the rows) × 7 rates, one-pass and record-then-play; the C smoke `--meters` case on the exactly representable `clamp_square`, floats printed as bit patterns, diffed against a Rust example (R-10). | Cross-target and Rust/C bit-exactness; a C stimulus must be exactly representable (spec 006's lesson). |
| P10 | A `MeterMutation` hook with four defects, each caught by a named oracle (R-12). | The quickstart's mutants need an injection mechanism, as every earlier mutation set has. |
| P11 | The stage manifest's notes name the token `Meters` (stage 5's `splices (VasEvent)` shortened to `VasEvent` to fit 120 characters); one canonical re-bless; no model version moves (R-11). | FR-011, FR-014. |

**Finding F1 (R-02):** the limiter computes the applied gain from the entry voltage; a meter that read the state after the update would be about one ramp step (up to 1 dB at 48 kHz) wrong on onset blocks. The ratio oracle compares onset blocks at 0.01 dB (spec FR-006) with a block-length rule that keeps every block's last sample qualifying (P5), so this mutant is caught.

**Finding F2 (R-11):** stage 5's controls note is 118 of 120 characters; the shortening keeps every token the 014 tokenizer needs (verified: 116 after `; Meters`; stage 11 reaches 119).

**Finding F3 (R-04):** `RecordPath::paused()` is `run_vas && is_paused()`; the read-out's state is the half's own gate state at finalization, "recording" with the full hang when the gate does not run (FR-002 h).

**Finding F4 (R-09):** the A/B script always ran the baseline's `output_cost` whatever the head test and never asserted; a 5 % budget run "against 0.14" would have measured stages 1 – 2 plus the meters against a bypassed baseline and failed by construction. The `meters` mode fixes the pair and asserts.

**Finding F5 (R-10):** the default 1.0 s hang never pauses within a 0.5 s stimulus; without the short-hang configurations the golden would carry no paused row.

## Summary

One copyable `Meters` read-out on the engine, filled from accumulators in the paths at the end of every non-empty processing call; a C twin with a size field; a stage 4 trace hook, a reference gate and a mutation hook as oracles; a new golden and a C parity case; a build A/B that asserts the cost; no audio change; version 0.15.0.

## Requirement map

| Req. | Mechanism | Test |
|---|---|---|
| FR-001 one read-out, rest, last non-empty call | R-01, R-04; `meters`, `rest_meters`, `finalize_meters` | core `meters::rest_and_finalize_rule`; `meters_edge_cases` |
| FR-002 (a) – (h) | R-02 rows; `VasStage::hang_remaining` | `us1_meters` AS1, AS2, AS3, AS5; `meters_matrix` |
| FR-003 (i) – (k) | R-02 rows; the `meter_route` flag | `us1_meters` AS1, AS4 |
| FR-004 rest when not run; D7 points | R-03 (the gates; a meter rests iff its point is off the call's path) | `meters_edge_cases` (the tap table, stage 10 off, the bypassed chain, the other path) |
| FR-005 peaks exact at the taps | R-02 (one narrowing per call), R-06 | `us1_meters` AS1 (four host-rate taps, `AfterRecord` at 6 kHz); `meters_matrix` (the hook at seven rates) |
| FR-006 gain reduction and charge | R-02, R-07; the model script's per-rate prints | `us1_meters` AS2 (the block-length rule asserted); core `meters::charge_landmarks` (and `v_limit` within 0.002 V of the script); `meters_matrix` per-rate pins; release-only `charge_flush` |
| FR-007 counts | R-02 (f), (k) | `us1_meters` AS3, AS4 (the clamp-off twins) |
| FR-008 VAS agreement | R-08 (both modes; the per-path event units) | `us1_meters` AS5; `meters_matrix` |
| FR-009 Rust and C forms | R-05; offset assertions; the size check | FFI unit tests (small/large/null sizes, the zero-fill), `ffi_parity meters`, C smoke `--meters` |
| FR-010 real-time safe, no state change | R-05 (a copy) | `alloc_free meters`; `meters_edge_cases` (read twice, output unchanged) |
| FR-011 no audio change | the accumulators never feed back | the nine SHA guards; `us2_cross_check canonical` (version line + `Meters` tokens); `us2_controls` (29 entries) |
| FR-012 deterministic; golden | R-10 | `golden_meters` (six golden targets + iOS sim); `determinism meters` (partition identities, two seeds) |
| FR-013 cost | R-09 | `scripts/ab-timing.sh fe82f46 meters`; `RR_DR60_METERS_COST_CEILING=0.12` in `ab-timing.yml` |
| FR-014 docs | R-13 | review; the guide's generated regions; `us2_cross_check` |
| FR-015 not a control | no setting, no entry, no register row | `us2_controls`; `scripts/check-traceability.sh` |
| SC-001 – SC-006 | as above | SC-001 `us1_meters`/`meters_matrix` (0 mismatches); SC-002 AS2 + the pins; SC-003 AS3 – AS5; SC-004 golden + smoke; SC-005 the A/B; SC-006 llvm-cov |

## Technical Context

**Language/Version**: Rust 1.85 (edition 2024), unchanged.
**Primary Dependencies**: none new. The core stays `no_std` + `alloc`; `rr_dr60_detmath` for `exp`/`ln` at finalization.
**Storage**: N/A (the golden JSON in the harness).
**Testing**: `cargo test` (core unit tests, `us1_meters`, `meters_matrix`, `meters_edge_cases`, `mutation meters::`, `golden_meters`, `ffi_parity`, `alloc_free`, `determinism`), the C smoke loop, `scripts/ab-timing.sh … meters`, `cargo llvm-cov`.
**Target Platform**: the existing 8 targets; the read-out bit-identical on all.
**Performance Goals**: ≤ 5 % per-sample cost at 48 kHz against 0.14 on the default front with the speech codec bypassed (the convention of specs 009 – 013; 12 % on the runner); every stage off within the same absolute nanoseconds (D6).
**Constraints**: no allocation, lock or I/O in processing or reading; no per-sample transcendental; the nine audio golden files, every smoke hash, `RrDr60Settings` and every existing C symbol unchanged; never weaken a manifest limit to make a note fit.
**Scale/Scope**: about 300 lines of core (the accumulators across four files, `Meters`, finalization, the flags, the hooks, `hang_remaining`), 130 of FFI, 900 of harness (`meters_checks.rs` with the reference gate, four test files, `meters_timing.rs`, the golden generator, the example, the mutation/parity/alloc/determinism extensions), 80 of C smoke, 60 of script and workflow, prints in the limiter script, docs.

No NEEDS CLARIFICATION remains.

## Constitution Check

| Principle | How this plan complies | Pre | Post |
|---|---|---|---|
| I. Portable Library First | `no_std` core, no I/O; C API additive (one struct, one function), header regenerated and drift-checked; the hooks behind `__test-hooks`. | ✅ | ✅ |
| II. Evidence-Based Fidelity | No modeled value changes; no new assumption (the figures read existing modeled state; the charge's denominator is spec 005's engineering target, cited as such); the guide names the IDs each figure rests on and words the landmarks as the model's hold law. | ✅ | ✅ |
| III. Measured, Test-First Verification | Each oracle (tap equality, the ratio, the hook, the reference gate, the twins, the partition identities) is written first and fails before the accumulators exist; the golden guards the figures; the earlier guards prove no audio change; the A/B asserts the cost. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | Compare-selects and one `exp`/`ln` per call through detmath; no allocation; `alloc_free` covers the read. | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | Each figure is tied to one stage's point and gated by the existing stage and tap rules; the stage manifest's notes name the read-out. | ✅ | ✅ |
| VI. Neutral, Honest Communication | The guide labels every figure's uncertainty by its assumption IDs and says "hold capacitor", never "detector"; the output clamp's count is documented as an emulator option; nothing claims a device reading. | ✅ | ✅ |
| VII. Independently Testable Features | US1 per figure with its own stimulus and oracle; US2 through the unmoved guards and the A/B; US3 through C alone. | ✅ | ✅ |

No violation; Complexity Tracking is empty.

## Project Structure

```text
specs/016-block-meters/  plan.md, research.md, data-model.md, contracts/api.md, quickstart.md, tasks.md (next)
crates/rr_dr60/src/pipeline.rs          # Meters (public), meters(), rest_meters(), finalize_meters, run_block finalization, the hooks
crates/rr_dr60/src/paths.rs             # RecordPath / RecordHalf / PlaybackPath accumulators, the meter_playback flag, finalize, stage4_trace
crates/rr_dr60/src/transport.rs         # record / stop_recording / play finalization
crates/rr_dr60/src/stages/limiter.rs    # v_limit accessor; the gain-from-vc helper shared with gain_db
crates/rr_dr60/src/stages/vas.rs        # hang_remaining()
crates/rr_dr60/src/stages/output.rs     # route_peak / clamp_count and the meter_route flag in OutputChain
crates/rr_dr60/src/manifest.rs          # the Meters tokens (nine notes; stage 5 shortened)
crates/rr_dr60/src/lib.rs               # re-export Meters; hooks stage4_trace, limiter_v_limit, MeterMutation
crates/rr_dr60_ffi/src/lib.rs, include/rr_dr60.h, tests/c/smoke.c (--meters), tests/c/run_smoke.sh
crates/rr_dr60_harness/src/meters_checks.rs   # ReferenceGate, the tap/ratio/twin oracles, the row renderer, narrow_out's mirror
crates/rr_dr60_harness/src/golden.rs, configs.rs, stimulus.rs   # MetersGolden, generate_meters, the two short-hang configurations, the four stimuli
crates/rr_dr60_harness/tests/us1_meters.rs, meters_matrix.rs, meters_edge_cases.rs, golden_meters.rs, meters_timing.rs
crates/rr_dr60_harness/tests/mutation.rs, ffi_parity.rs, alloc_free.rs, determinism.rs
crates/rr_dr60_harness/examples/meters_rows.rs
crates/rr_dr60_harness/golden/golden-meters-v1.json (new), manifest-v1.txt (re-bless)
scripts/ab-timing.sh (the meters mode), .github/workflows/ab-timing.yml (the step, ceiling 0.12)
tools/agc-model/limiter_model.py        # the per-rate steady figures, the charge ratio and the landmark prints
docs/guide.md (Meters section), README.md (real-time list), CHANGELOG.md, docs/product/front-and-inside.md (E-A built; E-F and the timeline map as later requests), CLAUDE.md
```

## Reviews

The coach, the hardware-emulation reviewer and the product specialist reviewed this plan on 2026-10-30; every finding is applied above. The owner signs off the plan next; tasks, analyze and implement then proceed to the merge, stopping only on a failed check or a new decision.
