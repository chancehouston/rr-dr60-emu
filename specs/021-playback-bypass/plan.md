# Implementation Plan: Live Playback-Stage Bypass

**Branch**: `021-playback-bypass` | **Date**: 2026-10-31 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/021-playback-bypass/spec.md` (approved D1 – D11, 2026-10-31)

## Decisions and findings (owner: read this section)

The spec's D1 – D11 stand, with these restatements from the plan reviews (coach 1 – 11, hardware B1 – B12, product M1 – M9, 2026-10-31, applied at their defaults):

1. **The bit-exact endpoint identities hold where no stateful stage follows the switched one** (coach 1, hardware B1): the stage-only configurations, stage 12 on the speaker route and 13 on the earphone route, stage 11 alone, stage 10 with the chain off (and on its device-rate trace). In the default engine a downstream filter remembers the pre-landing history, so the live output converges to the twin's: asserted ≤ −100 dBFS from P + N − 1 + N_OUT, the max difference in f32 LSBs pinned once at the bless, the first bit-identical sample reported. The blend identity stays bit-exact everywhere (R-10).
2. **Any change of a stage that does not run on the stream is refused**, with the tap reach explicit (coach 2, hardware B4): `AfterVolumeAmp` reaches 11 only, `AfterSpeaker` 11 and 12, `AfterEarphone` 11 and 13, `RouteOutput` all; one predicate `OutputChain::reaches(stage)`; the route is not part of the rule; D4's no-op entry is for a running stage at its value (R-04).
3. **One new public function instead of a constant** (product M2, M3; hardware B3): `output::stage11_bypass_level_db(route, position)` / `rr_dr60_stage11_bypass_level_db`, so no app composes the speaker route's scalar (whose offset is −1.214 dB, a sign the first draft got wrong); a recorded deviation from D10's "the status and the bound only" (R-07).
4. **The midpoint pin stays exact** (hardware B5): (N/2)·fl(1/N) = 0.5 for all seven N; the first draft's "within 1 ULP" is withdrawn; the multiply form is kept (R-01).
5. **The fade step clamps first** (hardware B2), so the same-position pairs and a toggle at the last blend sample are well defined and pinned; the at-once rule reads the stream's first produced sample, never the chain's `fresh` flag, which a route switch sets (hardware B7); `start_playback` snaps the fades (R-02).
6. **The mirror is the one-pass's compare at its next call** (hardware B8): a one-pass call cannot run during a playback, so the player's only mirror is the `start_playback` adopt; the one-pass lands the player's switches at the first sample of its next call with the stored tags, for the stages it reaches; the bound 142 is not an ABI change (coach 7) (R-05).
7. **The hot path is one byte**: `bypass_state == 0` takes 0.20's path; the clip count's gate is the fade's verdict inside the cold path (restated at implementation: no cached flag); `OutputChain` becomes `#[repr(C)]` so the placement is real (hardware B10, coach 11) (R-08).
8. **No pre-quoted onset for stage 10** (hardware B6): the first differing and the first identical host sample are measured with the chain off; the live engine's extra drain is the selected input, asserted exactly zero where the analysis says so (hardware B9, coach 6) (R-03, R-10).
9. **`reset` re-applies the landed values to all four fades, the one-pass's stage 10 included** (coach Q1 at the default); `reconfigure(s)` rebuilds from `s`; the trap and its recipe are tested and in the guide (R-11).
10. **The app contract has eleven points** (product M1, M4 – M6, M8): the one-pass tap, presets refused rather than no-ops, the trap's recipe, the finger toggle on the audio thread, the loudness rule for stage 11 on the earphone route (the Workbench moves the volume to position 1 first; the engine does not compensate), the inert volume and battery while stage 11 is bypassed, stage 10's audibility and onset, the card's lifecycle and meter rules, the toggle order 12, 11, 13, 10, and the wording rule ("stage N bypassed (emulator)", never "raw", "clean", "unfiltered" or "true"; the forbidden-list scan gains the stems).
11. **Smaller items**: N comes from `volume_transition_samples` and `speed::CROSSFADE_SAMPLES`, no new literal (hardware B11); the 2.6 kHz row is recorded on `playback_only` (hardware B12); the block-size-1 case is a partition, not a setter oracle (coach 8); the CHANGELOG's lead names where audio changes (product M7); the tight-bounds row, the sixteen edge cases enumerated in tasks, the test names carrying "bypass" (coach 9, 10); records.md gains the Swift note, the header comment and the version bump (coach 11, product M9).

Plan choices, each traced to the research:

| # | Plan choice | Why |
|---|---|---|
| P1 | A `Fade { k, dir, n, inv_n }` per stage (11 – 13 in `OutputChain`, 10 in `PlaybackHalf`; `stages/fade.rs`); the stage always runs when configured on; its contribution selected at rest and `y + w·(x − y)` while fading, w from the pinned expression per sample (exactly 0.5 at N/2); the step clamps first; a landing from rest starts, during a fade flips the direction; at once on the stream's first produced sample (R-01, R-02). | D3, D5; FR-002, FR-003, FR-005. |
| P2 | `Apply::Bypass { stage, bypassed, at_once }` through spec 015's `StreamState`; the four controls validated in `schedule_change` (exactly 0 or 1; the stream rule: 11 – 13 on the player and the one-pass, 10 on the player; `reaches(stage)` / `half.runs()` → `StageNotConfigured` 36 for either value); the landed value written into the engine's `Settings`; `reset` re-applies to all four fades, `reconfigure(s)` takes `s` (R-04, R-11). | D2, D4; FR-001, FR-004. |
| P3 | The mirror for 11 – 13: the player adopts at `start_playback` (no entry); the one-pass compares `settings` to its `last_bypass` at its next call and lands the differences first with the stored tags, for the stages it reaches; `MAX_CHANGES_PER_CALL` 142, the tight bounds pinned (R-05). | D2; FR-001. |
| P4 | Stage 10: the half's output blended before the interpolator; the landing through the player's pre-pull apply at d(P); a `stage10_trace` hook; the first differing and first identical host sample measured with the chain off (R-03). | D1, D6; FR-003. |
| P5 | The hot path: one `bypass_state: u8` beside `active` in a `#[repr(C)]` chain; zero takes 0.20's path; the clip count gated by the fade's verdict in the cold path; the A/B `bypass` mode against f303c99 on both streams at 3 % (`RR_DR60_BYPASS_COST_CEILING` 0.05 on the label job); the bypassed state and a blend reported (R-08). | D10; FR-010; SC-005. |
| P6 | The clip count gated by the fade's verdict inside the cold path (`o_amp != Bypassed`; no cached flag, restated at implementation); the route and reconstruction peaks unchanged (they read after the blend) (R-08). | D7; FR-009. |
| P7 | The harness: `bypass_checks.rs` (the twins; the bit-exact endpoint identities where the switched stage is last, the convergence bound and the first-identical sample elsewhere; the live tail exactly zero where the analysis says so; the partitions including block size 1; the device-rate trace for stage 10; the click table with `click_energy_band`; the level table against `stage11_bypass_level_db`; the reversal; the trap; the stage-10 third-octave difference at 48 kHz), tests `us1_bypass`, `us2_bypass`, `bypass_matrix` (with `the_tight_bounds`), `bypass_edge_cases` (sixteen), `mutation bypass::` (six), `golden_bypass` (168 entries; the gates per entry's `exact` flag), `ffi_parity bypass_parity`, `alloc_free bypass_allocate_nothing`, `determinism bypass_same_calls_same_log`, `us2_cross_check::guide_bypass` (R-06, R-09, R-10). | D9; FR-006 – FR-008; SC-001 – SC-004. |
| P8 | The records of records.md: the control manifest's four timing words (one re-bless), the stage manifest's version line, the guide section with the eleven-point contract, the loudness rule and the tables, the README lists, the CHANGELOG under Added (the lead naming where audio changes), the product notes, `rr_dr60_stage11_bypass_level_db` and status 36 in the header; version 0.21.0 (R-07, R-11). | D10, D11; FR-011. |

## Summary

The four playback-side stage switches become live values scheduled on the player (all four) and the one-pass (stages 11 – 13) through spec 015's schedule, each landing a 20 ms smoothstep blend between the stage's processed output and its input while the stage keeps running, so a switch is click-free and reversible from any phase; a stage that does not run on the stream is refused, so the player's latency, drain and length never move; every pre-021 configuration is bit-identical to 0.20. An emulator convention, no assumption.

## Requirement map

| Requirement | Research | Tests |
|---|---|---|
| FR-001 the four live values, the streams, the log, `settings()`, the mirror | R-04, R-05 | `us2_bypass` AS1, AS3, AS4; `bypass_matrix`; `us1_controls`/`us2_controls` |
| FR-002 the blend | R-01, R-02 | core fade tests; `us1_bypass` AS2, AS3; `mutation bypass::{weight_inverted, fade_linear, fade_step}` |
| FR-003 the warm stage, the endpoint identities (exact where last; the convergence bound elsewhere) | R-03, R-10 | `us1_bypass` AS2 – AS4; `bypass_matrix`; `mutation bypass::stage_cold` |
| FR-004 the refusal (the tap reach); latency, drain, length fixed | R-04, R-10 | `us2_bypass` AS1 (incl. the one-pass at `AfterVolumeAmp`), AS2, AS9; core `reaches` at five taps |
| FR-005 reverse from phase, the bounds, same-position | R-02 | core; `us1_bypass` AS7; `bypass_edge_cases` |
| FR-006 bit-identical to 0.20; the golden | R-09 | the fourteen guards; `golden_bypass` |
| FR-007 the click table | R-06 | `us1_bypass` AS5; `bypass_matrix` (6 kHz asserted, 48 kHz reported) |
| FR-008 partitions; the route rule | R-02, R-05 | `us1_bypass` AS6; `bypass_matrix`; `mutation bypass::phase_frozen_off_route` |
| FR-009 the meters | R-02 | `us2_bypass` AS5; `mutation bypass::count_ungated` |
| FR-010 cost, allocation, determinism | R-08 | `scripts/ab-timing.sh f303c99 bypass`; `alloc_free`; `determinism` |
| FR-011 the records, the C surface | R-07, R-11 | `us2_cross_check` (manifest, guide); the CHANGELOG; the header drift check; `ffi_parity bypass_parity`; the smoke `--bypass` |
| SC-001 – SC-006 | as above | 0 mismatches; ≥ 30 dB on the asserted rows; the A/B; coverage |

## Technical Context

**Language/Version**: Rust 1.85 (edition 2024). **Dependencies**: none new; `no_std` + `alloc`. **Testing**: `cargo test` (core `fade`/`bypass` tests, the harness tests above), the C smoke, the A/B script. **Target**: every supported host rate; stages 11 – 13 at the host rate, stage 10 at the 6 kHz device rate. **Performance**: the all-on state within 3 % of 0.20 on both streams; a fade one multiply-add per fading stage per sample. **Constraints**: no allocation in a call; the latency, drain and length unchanged; the C API changes by a status, one function and a bound.

No NEEDS CLARIFICATION remains.

## Constitution Check

| Principle | How this plan complies | Pre | Post |
|---|---|---|---|
| I. Portable Library First | `no_std` core; the C API gains one status, one pure function (`rr_dr60_stage11_bypass_level_db`) and the moved bound, the header regenerated and drift-checked. | ✅ | ✅ |
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
crates/rr_dr60/src/output.rs               # stage11_bypass_level_db (the private scale constant from the design script)
crates/rr_dr60/src/lib.rs                  # BypassMutation, stage10_trace hook; version 0.21.0
crates/rr_dr60_ffi/src/lib.rs, include/rr_dr60.h  # status 36, rr_dr60_stage11_bypass_level_db, the bound
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

The coach (1 – 11), the hardware-emulation reviewer (B1 – B12) and the product specialist (M1 – M9) reviewed this plan on 2026-10-31; every finding is applied above and in research.md, data-model.md, contracts/api.md, quickstart.md and records.md, and the spec carries the restatements as a dated Clarifications session.
