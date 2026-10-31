# Implementation Plan: Live Playback-Stage Bypass

**Branch**: `021-playback-bypass` | **Date**: 2026-10-31 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/021-playback-bypass/spec.md` (approved D1 – D11, 2026-10-31)

## Decisions and findings (owner: read this section)

The spec's D1 – D11 stand, with these restatements from the plan's research (the plan reviews' findings are appended here when applied):

1. **The weight pin is within 1 ULP** (R-01): `u = k · inv_n` with inv_n = 1/N is exactly 0.5 at k = N/2 only when N is a power of two (N = 960 at 48 kHz is not), so the core pins `w(N/2)` within `f64::EPSILON` of 0.5, exactly 0.5 where N is a power of two; the end samples are selected, as the spec says.
2. **Any change of a non-running stage is refused**, not only an on-change (R-04): landing an off-change on a stage the one-pass's tap never reaches would write `settings()` to off and make the on-change refused later; one rule, no trap. The route is not part of the rule.
3. **The mirror skips a stage the other stream does not run** (R-05); `settings()` still reflects the landing.
4. **Stage 10's settle on the host output is measured and the formula is the bound** (R-03): the harness prints the first differing and the first identical host sample per rate.
5. **`output::SPEAKER_SCALE_DB`** (R-07): one new public constant so an app computes stage 11's bypass level change from the two public gain-law functions without the chain's internals.
6. **The click reader gains a band-top parameter** (R-06): `click_energy_band`, the 019 reader delegating; 6 kHz asserted, 48 kHz reported.

Plan choices, each traced to the research:

| # | Plan choice | Why |
|---|---|---|
| P1 | A `Fade { k, dir, n, inv_n }` per stage (11 – 13 in `OutputChain`, 10 in `PlaybackHalf`); the stage always runs when configured on; its contribution selected at rest and `y + w·(x − y)` while fading, w from the pinned expression per sample; a landing from rest starts, a landing during a fade flips the direction (R-01, R-02). | D3, D5; FR-002, FR-003, FR-005. |
| P2 | `Apply::Bypass(stage, bypassed)` through spec 015's `StreamState`; the four controls validated in `schedule_change` (exactly 0 or 1; the stream rule: 11 – 13 on the player and the one-pass, 10 on the player; the run-flag rule → `StageNotConfigured` 36); the landed value written into the engine's `Settings`; `reset` re-applies, `reconfigure(s)` takes `s` (R-04, R-11). | D2, D4; FR-001, FR-004. |
| P3 | The cross-stream mirror for 11 – 13 as spec 017's `RecordCtl` (`xpend` per stage, applied first, logged with the tag; `start_playback` adopts; the stage must run on the receiving stream); `MAX_CHANGES_PER_CALL` 142 (R-05). | D2; FR-001. |
| P4 | Stage 10: the half's output blended before the interpolator; the landing through the player's pre-pull apply at d(P); a `stage10_trace` hook; the host-output settle measured (R-03). | D1, D6; FR-003. |
| P5 | The hot path: `fading` and the rest bits beside `active`; one test per sample when idle; the A/B `bypass` mode against f303c99 on both streams at 3 % (`RR_DR60_BYPASS_COST_CEILING` 0.05 on the label job); the bypassed state and a blend reported (R-08). | D10; FR-010; SC-005. |
| P6 | The clip count gated per sample on `fade_amp.k < n`; the route and reconstruction peaks unchanged (they read after the blend) (R-02). | D7; FR-009. |
| P7 | The harness: `bypass_checks.rs` (the twins, the endpoint identities over the twin's length with the live tail's peak, the block-size-1 oracle, the device-rate trace for stage 10, the click table with `click_energy_band`, the level table against the public functions, the reversal, the partitions, the trap), tests `us1_bypass`, `us2_bypass`, `bypass_matrix`, `bypass_edge_cases`, `mutation bypass::` (six), `golden_bypass` (140 entries, bless-time identities), `ffi_parity`, `alloc_free`, `determinism`, `us2_cross_check::guide_bypass` (R-06, R-09, R-10). | D9; FR-006 – FR-008; SC-001 – SC-004. |
| P8 | The records of records.md: the control manifest's four timing words (one re-bless), the stage manifest's version line, the guide section with the app contract and the tables, the README lists, the CHANGELOG under Added, the product notes, `SPEAKER_SCALE_DB` and status 36 in the header; version 0.21.0 (R-07, R-11). | D10, D11; FR-011. |

## Summary

The four playback-side stage switches become live values scheduled on the player (all four) and the one-pass (stages 11 – 13) through spec 015's schedule, each landing a 20 ms smoothstep blend between the stage's processed output and its input while the stage keeps running, so a switch is click-free and reversible from any phase; a stage that does not run on the stream is refused, so the player's latency, drain and length never move; every pre-021 configuration is bit-identical to 0.20. An emulator convention, no assumption.

## Requirement map

| Requirement | Research | Tests |
|---|---|---|
| FR-001 the four live values, the streams, the log, `settings()`, the mirror | R-04, R-05 | `us2_bypass` AS1, AS3, AS4; `bypass_matrix`; `us1_controls`/`us2_controls` |
| FR-002 the blend | R-01, R-02 | core fade tests; `us1_bypass` AS2, AS3; `mutation bypass::{weight_inverted, fade_linear, fade_step}` |
| FR-003 the warm stage, the endpoint identities | R-03, R-10 | `us1_bypass` AS2 – AS4; `bypass_matrix`; `mutation bypass::stage_cold` |
| FR-004 the refusal; latency, drain, length fixed | R-04, R-10 | `us2_bypass` AS1, AS2, AS9 |
| FR-005 reverse from phase, the bounds, same-position | R-02 | core; `us1_bypass` AS7; `bypass_edge_cases` |
| FR-006 bit-identical to 0.20; the golden | R-09 | the fourteen guards; `golden_bypass` |
| FR-007 the click table | R-06 | `us1_bypass` AS5; `bypass_matrix` (6 kHz asserted, 48 kHz reported) |
| FR-008 partitions; the route rule | R-02, R-05 | `us1_bypass` AS6; `bypass_matrix`; `mutation bypass::phase_frozen_off_route` |
| FR-009 the meters | R-02 | `us2_bypass` AS5; `mutation bypass::count_ungated` |
| FR-010 cost, allocation, determinism | R-08 | `scripts/ab-timing.sh f303c99 bypass`; `alloc_free`; `determinism` |
| FR-011 the records | R-07, R-11 | `us2_cross_check` (manifest, guide); the CHANGELOG; the header drift check |
| SC-001 – SC-006 | as above | 0 mismatches; ≥ 30 dB on the asserted rows; the A/B; coverage |

## Technical Context

**Language/Version**: Rust 1.85 (edition 2024). **Dependencies**: none new; `no_std` + `alloc`. **Testing**: `cargo test` (core `fade`/`bypass` tests, the harness tests above), the C smoke, the A/B script. **Target**: every supported host rate; stages 11 – 13 at the host rate, stage 10 at the 6 kHz device rate. **Performance**: the all-on state within 3 % of 0.20 on both streams; a fade one multiply-add per fading stage per sample. **Constraints**: no allocation in a call; the latency, drain and length unchanged; the C API changes by a status, a define and a bound.

No NEEDS CLARIFICATION remains.

## Constitution Check

| Principle | How this plan complies | Pre | Post |
|---|---|---|---|
| I. Portable Library First | `no_std` core; the C API gains one status and one define, the header regenerated and drift-checked; no new function. | ✅ | ✅ |
| II. Evidence-Based Fidelity | No device claim: the device has no stage switches; the blend and the warm stage are stated as the emulator's conventions in the guide and the CHANGELOG; no assumption registered or amended; the traceability audit runs. | ✅ | ✅ |
| III. Measured, Test-First Verification | Every oracle written first: the endpoint identities, the blend identity, the click table with its reader defined, the measured stage-10 settle, six mutants, the golden's bless-time gates. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | Plain IEEE arithmetic in a fixed order; the fade state reserved at construction; one test per sample when idle; the A/B budget. | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | The blend sits at each stage's own point; the stage manifest's version line; the control manifest's timing words; every constant traced. | ✅ | ✅ |
| VI. Neutral, Honest Communication | "An emulator convention; the device has no such switches"; stage 11's level change stated with figures; the figures as the harness prints them. | ✅ | ✅ |
| VII. Independently Testable Features | US1 through the stage-only configurations and the twins; US2 through the log, the refusals and the manifest. | ✅ | ✅ |

No violation; no Complexity Tracking entry.

## Project Structure

```text
specs/021-playback-bypass/  plan.md, research.md, data-model.md, contracts/api.md, quickstart.md, records.md, tasks.md (next)
crates/rr_dr60/src/stages/output.rs        # Fade, the three fades, fading, the selects/blends at the stage points, the gated count, reset rules
crates/rr_dr60/src/stages/fade.rs (new)    # Fade { k, dir, n, inv_n }: step, land, weight (shared by output.rs and paths.rs)
crates/rr_dr60/src/paths.rs                # PlaybackHalf's fade and set_bypass; PlaybackPath::apply_if_due handles Apply::Bypass; the stage10 trace hook
crates/rr_dr60/src/changes.rs              # Apply::Bypass, StreamState's last/landed/xpend bypass fields, LIVE_STAGE_* consts, MAX_CHANGES_PER_CALL 142
crates/rr_dr60/src/pipeline.rs             # schedule_change's validation (0/1, the stream rule, the run-flag rule), the mirror after each call, settings reflection, start_playback adopt, reset
crates/rr_dr60/src/transport.rs            # the player's pre-pull apply routes Apply::Bypass(10) to the half
crates/rr_dr60/src/controls.rs             # the four entries' timing: Live
crates/rr_dr60/src/error.rs                # StageNotConfigured
crates/rr_dr60/src/output.rs               # SPEAKER_SCALE_DB
crates/rr_dr60/src/lib.rs                  # BypassMutation, stage10_trace hook; version 0.21.0
crates/rr_dr60_ffi/src/lib.rs, include/rr_dr60.h  # status 36, RR_DR60_SPEAKER_SCALE_DB, the bound
crates/rr_dr60_harness/src/analysis.rs     # click_energy_band
crates/rr_dr60_harness/src/bypass_checks.rs (new)
crates/rr_dr60_harness/src/golden.rs       # BypassGolden, generate_bypass
crates/rr_dr60_harness/tests/us1_bypass.rs, us2_bypass.rs, bypass_matrix.rs, bypass_edge_cases.rs, golden_bypass.rs, bypass_timing.rs (0.20 API)
crates/rr_dr60_harness/tests/mutation.rs (mod bypass), ffi_parity.rs, alloc_free.rs, determinism.rs, us1_controls.rs, us2_controls.rs, us2_cross_check.rs
crates/rr_dr60_harness/examples/bypass_rows.rs; crates/rr_dr60_ffi/tests/c/smoke.c (--bypass), run_smoke.sh
crates/rr_dr60_harness/golden/golden-bypass-v1.json, control-manifest-v1.txt, manifest-v1.txt
scripts/ab-timing.sh (bypass mode), .github/workflows/ab-timing.yml, .github/workflows/ci.yml (the golden line)
docs/guide.md, README.md, CHANGELOG.md, docs/product/{controls-discussion,brief,front-and-inside,decisions}.md, CLAUDE.md
```

## Reviews

*(The coach, the hardware-emulation reviewer and the product specialist review this plan next; their findings are applied above and in the design documents, and the spec carries the restatements as a dated Clarifications session.)*
