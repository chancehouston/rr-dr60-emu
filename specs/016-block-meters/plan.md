# Implementation Plan: Per-Block Meters

**Branch**: `016-block-meters` | **Date**: 2026-10-30 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/016-block-meters/spec.md` (approved D1 – D7, 2026-10-29)

## Decisions and findings (owner: read this section)

The spec's D1 – D7 stand. Plan choices, each traced to the research:

| # | Plan choice | Why |
|---|---|---|
| P1 | The accumulators live in the path objects (`RecordPath`, `RecordHalf`, `PlaybackPath`, `OutputChain`) and are finalized once per non-empty call into one `Meters` on the `Pipeline` (R-01, R-04). | Five call sites (one-pass, record, flush, play, drain) share one code path per figure; the transcendental work runs once per call. |
| P2 | The gain reduction is kept as the call's largest limiter entry voltage and converted once at finalization; the charge is `vc / v_limit` (R-02, R-07). | The applied gain is a monotone function of the entry voltage: one compare per sample, no per-sample `ln`. |
| P3 | Peaks accumulate as f64 compare-selects (`if a > p`) and are narrowed once per call with the output's rounding (R-02). | Exact against the tap oracle (narrowing is monotone); no NaN-aware max on the hot path (spec 010's lesson). |
| P4 | A `__test-hooks` trace of stage 4's pre-clamp stream is the filter-peak oracle at rates other than 6 kHz; the `AfterRecord` tap at 6 kHz with the clamp off is the hook-free one (R-06). | The device-rate stream is in no output buffer at six rates. |
| P5 | A harness reference gate plus the engine's own paused reports and the splice lengths are the VAS oracle (R-08). | Drop-mode events are reported at the resume; the state is the oracle, the events corroborate the paused count. |
| P6 | C: `RrDr60Meters` (56 bytes, size field, zero-filled tail, f32/u32/u64 fields) and one function `rr_dr60_meters`; no new status (R-05). | D5; the existing reports untouched. |
| P7 | Cost by the build A/B script against the 0.14 merge, 45 rounds, two runs (stages 1 – 2 on = the default front; off = the bypassed chain), with a 12 % runner ceiling on the `ab-timing` label (R-09). | No off switch (D6). |
| P8 | `golden-meters-v1.json` with per-block rows for 0.5 s stimuli and the C smoke `--meters` case (R-10). | Cross-target and Rust/C bit-exactness. |
| P9 | The stage manifest's notes name the token `Meters` (stage 5's `splices (VasEvent)` shortened to `VasEvent` to fit 120 characters); one canonical re-bless; no model version moves (R-11). | FR-011, FR-014. |

**Finding F1 (R-02):** the limiter computes the applied gain from the entry voltage; a meter that read the state after the update would be up to 2 dB wrong on onset blocks. The oracle compares onset blocks at 0.01 dB (spec FR-006), so this mutant is caught.

**Finding F2 (R-11):** stage 5's controls note is 118 of 120 characters; the shortening of `splices (VasEvent)` keeps every token the 014 tokenizer needs.

**Finding F3 (R-04):** `RecordPath::paused()` is `run_vas && is_paused()`; the read-out's state is the half's own gate state at finalization, and with `run_vas` false it is "recording" with the full hang by FR-002(h). Both read the same thing whenever the gate runs.

## Summary

One copyable `Meters` read-out on the engine, filled from accumulators in the paths at the end of every non-empty processing call; a C twin with a size field; a stage 4 trace hook and a reference gate as oracles; a new golden; a build A/B for the cost; no audio change; version 0.15.0.

## Requirement map

| Req. | Mechanism | Test |
|---|---|---|
| FR-001 one read-out, rest, last non-empty call | R-01, R-04; `Pipeline::meters`, `finalize_meters` | core `meters::rest_and_finalize_rule`; `meters_edge_cases` |
| FR-002 (a) – (h) | R-02 rows; `VasStage::hang_remaining` | `us1_meters` AS1, AS2, AS3, AS5; `meters_matrix` |
| FR-003 (i) – (k) | R-02 rows; the chain's route-output flag | `us1_meters` AS1, AS4 |
| FR-004 rest when not run; D7 points | R-03 (a meter rests iff its code did not run) | `meters_edge_cases` (the tap table, bypasses, the other path) |
| FR-005 peaks exact at the taps | R-02 (one narrowing per call), R-06 | `us1_meters` AS1 (four host-rate taps, `AfterRecord` at 6 kHz), `meters_matrix` (the hook at seven rates) |
| FR-006 gain reduction and charge | R-02, R-07; the model script's prints | `us1_meters` AS2; core `meters::charge_landmarks`; `meters_matrix` steady ratio pin |
| FR-007 counts | R-02 (f), (k) | `us1_meters` AS3, AS4 (the clamp-off twins) |
| FR-008 VAS agreement | R-08 | `us1_meters` AS5; `meters_matrix` |
| FR-009 Rust and C forms | R-05; offset assertions; `check_out_size` | FFI unit tests (small/large/null sizes), `ffi_parity meters`, C smoke `--meters` |
| FR-010 real-time safe, no state change | R-05 (a copy) | `alloc_free meters`; `meters_edge_cases` (read twice, output unchanged) |
| FR-011 no audio change | accumulators never feed back | the ten SHA guards; `us2_cross_check canonical` (version line + `Meters` tokens) |
| FR-012 deterministic; golden | R-10 | `golden_meters` (six golden targets + iOS sim), `determinism meters` |
| FR-013 cost | R-09 | `scripts/ab-timing.sh fe82f46 output_cost_mic` / `output_cost`; `RR_DR60_METERS_COST_CEILING` on the runner |
| FR-014 docs | R-12 | review; the guide's generated regions; `us2_cross_check` |
| FR-015 not a control | no setting, no entry | `us2_controls` (unchanged count 29), no register change |
| SC-001 – SC-006 | as above | SC-001 `us1_meters`/`meters_matrix` (0 mismatches); SC-002 AS2 + the pins; SC-003 AS3 – AS5; SC-004 golden + smoke; SC-005 the A/B; SC-006 llvm-cov |

## Technical Context

**Language/Version**: Rust 1.85 (edition 2024), unchanged.
**Primary Dependencies**: none new. The core stays `no_std` + `alloc`; `rr_dr60_detmath` for `exp`/`ln` at finalization.
**Storage**: N/A (the golden JSON in the harness).
**Testing**: `cargo test` (core unit tests, `us1_meters`, `meters_matrix`, `meters_edge_cases`, `mutation meters::`, `golden_meters`, `ffi_parity`, `alloc_free`, `determinism`), the C smoke loop, `scripts/ab-timing.sh`, `cargo llvm-cov`.
**Target Platform**: the existing 8 targets; the read-out bit-identical on all.
**Performance Goals**: ≤ 5 % per-sample cost at 48 kHz against 0.14 on the default front (12 % on the runner); the bypassed chain within the same absolute ns (D6).
**Constraints**: no allocation, lock or I/O in processing or reading; no per-sample transcendental; the ten audio golden files, every smoke hash, `RrDr60Settings` and every existing C symbol unchanged; never weaken a manifest limit to make a note fit.
**Scale/Scope**: about 250 lines of core (the accumulators across four files, `Meters`, finalization, the hook, `hang_remaining`), 120 of FFI, 700 of harness (`meters_checks.rs` with the reference gate, three test files, the golden generator, the mutation and parity extensions), 60 of C smoke, a print in the limiter script, docs.

No NEEDS CLARIFICATION remains.

## Constitution Check

| Principle | How this plan complies | Pre | Post |
|---|---|---|---|
| I. Portable Library First | `no_std` core, no I/O; C API additive (one struct, one function), header regenerated and drift-checked; the hook behind `__test-hooks`. | ✅ | ✅ |
| II. Evidence-Based Fidelity | No modeled value changes; no new assumption (the figures read existing modeled state; the charge's denominator is spec 005's engineering target, cited); the guide names the IDs each figure rests on. | ✅ | ✅ |
| III. Measured, Test-First Verification | Each oracle (tap equality, the ratio, the hook, the reference gate, the twins) is written first and fails before the accumulators exist; the golden guards the figures; the earlier golden guards prove no audio change. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | Compare-selects and one `exp`/`ln` per call through detmath; no allocation; `alloc_free` covers the read. | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | Each figure is tied to one stage's point; the stage manifest's notes name the read-out; the taps' rest rules follow the existing stage gating. | ✅ | ✅ |
| VI. Neutral, Honest Communication | The guide labels every figure's uncertainty by its assumption IDs; the manifest's strings change by one token; nothing claims a device reading. | ✅ | ✅ |
| VII. Independently Testable Features | US1 per figure with its own stimulus and oracle; US2 through the unmoved guards and the A/B; US3 through C alone. | ✅ | ✅ |

No violation; Complexity Tracking is empty.

## Project Structure

```text
specs/016-block-meters/  plan.md, research.md, data-model.md, contracts/api.md, quickstart.md, tasks.md (next)
crates/rr_dr60/src/pipeline.rs          # Meters (public), meters(), finalize_meters, the run_block finalization, stage4_trace
crates/rr_dr60/src/paths.rs             # RecordPath / RecordHalf / PlaybackPath accumulators, finalize, the trace
crates/rr_dr60/src/transport.rs         # record / stop_recording / play finalization
crates/rr_dr60/src/stages/limiter.rs    # v_limit accessor; gain-from-vc helper shared with gain_db
crates/rr_dr60/src/stages/vas.rs        # hang_remaining()
crates/rr_dr60/src/stages/output.rs     # route_peak / clamp_count in OutputChain::process
crates/rr_dr60/src/manifest.rs          # the Meters tokens (nine notes; stage 5 shortened)
crates/rr_dr60/src/lib.rs               # re-export Meters; hooks stage4_trace, limiter_v_limit
crates/rr_dr60_ffi/src/lib.rs, include/rr_dr60.h, tests/c/smoke.c (--meters), tests/c/run_smoke.sh
crates/rr_dr60_harness/src/meters_checks.rs   # ReferenceGate, the tap/ratio/twin oracles, the row renderer
crates/rr_dr60_harness/src/golden.rs    # MetersGolden, generate_meters, path_meters
crates/rr_dr60_harness/tests/us1_meters.rs, meters_matrix.rs, meters_edge_cases.rs, golden_meters.rs
crates/rr_dr60_harness/tests/mutation.rs, ffi_parity.rs, alloc_free.rs, determinism.rs, timing.rs (the ceiling env)
crates/rr_dr60_harness/golden/golden-meters-v1.json (new), manifest-v1.txt (re-bless)
scripts/ab-timing.sh (45 rounds for the two meters runs), .github/workflows/ab-timing.yml (two steps)
tools/agc-model/limiter_model.py        # the steady charge ratio and landmark prints
docs/guide.md (Meters section), README.md (real-time list), CHANGELOG.md, docs/product/front-and-inside.md, CLAUDE.md
```

## Reviews

Per the owner's cadence: the coach and the hardware-emulation reviewer review this plan, the product specialist reviews the guide's meter table wording; findings are applied without stopping unless one needs an owner decision; the owner signs off the plan next.
