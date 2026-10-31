# Implementation Plan: Speed-Change Crossfade

**Branch**: `019-speed-crossfade` | **Date**: 2026-10-30 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/019-speed-crossfade/spec.md` (approved D1 – D9, 2026-10-30)

## Decisions and findings (owner: read this section)

The spec's D1 – D9 stand, with these restatements from the plan reviews (coach 1 – 12, hardware B1 – B10, product M1 – M6, 2026-10-30, applied at their defaults):

1. **The last blend sample is the new branch itself** (hardware B1, coach 1): `a + 1.0·(b − a)` is not `b` in floating point for about 44 % of pairs, so blend samples n = 1 … N − 1 use `y_old + w_n·(y_new − y_old)` and sample N emits `y_new` directly (w_N = 1 is that rule); the splice identity from s + N − 1 then holds by construction and the reference uses the identical rule.
2. **The step mutant is "no fade, apply at once"** (hardware B2): it reproduces 0.18 bit for bit, including requests inside 20 ms, so it is the click table's switch reference **and** a bless-time gate: before the re-bless, the committed `golden-speed-v1.json` and the changes golden's `speed_inside` entries are regenerated with `fade_step` on and must match the committed files.
3. **Waited requests lengthen schedules whose requests are under 20 ms apart** (hardware B3): the length rule is unchanged for a constant level and for the new branch's trajectory; a waited request lands up to 120 device samples later than 0.18's, so the `speed_inside` entries' produced counts move (asserted as expected at bless time) and the smoke's count is unchanged only because its one change stands alone (asserted).
4. **The ring span is 153, the pulls follow the leading branch** (hardware B4, coach 6): the kernel is centred (i − 28 … i + 28), so either way round the span is 96 + 57 = 153 < 256; the highest index read is at most i_new + 124 (the old branch leads when the old level is faster), the lowest at least i_new − 124; `pull_through(max(base_old, base_new))` while a fade runs; debug assertions guard the ring; after a fade into level 3 from a faster level the general path runs for about 124 outputs (`pulled > i`; the same output, spec 013 FR-006; the cost noted).
5. **The fast path tests `fade.is_none()` explicitly** (hardware B5); the old branch advances by `step(old_k)` (the ratio mutant).
6. **The click reader's cases and floor** (coach 5, 7; hardware B6): the asserted cases are spec 013's thirteen step-table rows at 2 and 2.4 kHz (the seven 1 kHz rows, whose switch steps are −73 … −99 dB, are reported with both absolute energies); the fade's reading sits at the reader's leakage floor (about −41 dB re the tone), so the ratio is "the switch against the floor" with about 15 dB of margin; a constant-run floor row is printed; the step reader is centred on s and the fade reader on s + N ÷ 2; an exclusion above the device Nyquist folds to 6000 − f; overlapping exclusions are a union; the voiced figure is reported as both absolute energies (voice fills the band; no exclusion applies).
7. **Bit-exactness is at 6 kHz; a device-rate trace serves the other rates** (coach 3): spec 013's bit-exact tests run at 6 kHz (`speed_only`, `speed_only_codec`); at the six other rates the blend passes stage 10 and the resampler, so the blend identity is checked on a `__test_hooks` trace of the speed stage's device outputs per play call, plus the partition identity on the host output.
8. **The cost A/B measures the general path too** (coach 4): a new `crossfade_timing.rs` against 0.18's API plays a constant level 3 **and** a constant level 5 (copied into the 13defc7 worktree), the `crossfade` mode asserts both at 3 % (`RR_DR60_CROSSFADE_COST_CEILING` 0.05 on the label job) and a workflow step runs it; the head-only fade-every-200-ms test asserts its worst block against the real-time budget (1000 × samples ÷ kHz µs) and reports the per-sample cost.
9. **Stage 9's notes** (coach 2, product M4): the coverage note is at 119 of 120 characters and the controls note at 114, so the blend goes into the stage's **description** ("…at one of five playback speeds (level 3 is normal); a level change blends over 20 ms, an engine convention") and the coverage note is rewritten whole, keeping its pinned tokens: "the device's own decoder (frame length, look-ahead, codebooks); its speed method and the sound of a level change (A-054)"; the lengths checked by the manifest's const assertions.
10. **One C define** (product M1): `RR_DR60_SPEED_CROSSFADE_MS 20` beside `RR_DR60_VOLUME_TRANSITION_MS` (the app debounces its stepper with it); the header gains one line, pinned to the core constant; `CROSSFADE_SAMPLES` stays Rust-only.
11. **Claims wording** (product M2, M3, M6; hardware B10): A-054's amendment reads as a hypothesis ("What the device does at the moment of a level change is unknown: no document describes it. Because the decoder works in 20 ms frames, a switch at a frame boundary is one candidate; an instant step and a brief mute remain candidates. Capture P1 decides."); the guide says the instant switch's step is replaced by a 20 ms blend and quotes the harness's click-energy reduction per pair, never "no longer ticks"; D3's dip is "a brief beat, about −3 dB average power at the midpoint"; the CHANGELOG's category is Changed with the lead "an audio change at level changes only, not an assumption change".
12. **Smaller items**: `fade_no_wait` is "the new request applies at once; the running fade's new branch becomes the old branch, its weight restarting at n = 1" (hardware B7, coach 8); the weight expression is pinned as `u * u * (3.0 - 2.0 * u)` with `u = f64::from(n) / 120.0` in the core, the reference and the script (B8; the expanded form differs on 64 of 120 entries); the end cut's report carries the cut's peak amplitude and energy per pair in both configurations, D6's "mostly silence" holding with the codec on only (B9, coach 10); the registers cross-check pins the phrases "engine convention" and "capture P1" in A-054 (coach 11); the bless-time check adds "every scheduled entry's blend completes before K" (coach 12); the quickstart's filters match the new test names (`*_crossfade*`), US2 AS3 points at the A/B script and AS4 at `ffi_parity` (coach 9); the records gain investigation-mode's § 5 bullet, brief.md's engine-facts clause and monetization.md's stale parenthetical (product M5).

Plan choices, each traced to the research:

| # | Plan choice | Why |
|---|---|---|
| P1 | The crossfade lives inside `SpeedStage`: a `fade: Option<Fade>` state (`old_k`, `old_p5`, `n`) beside the existing place and level; a request is applied only while no fade runs (the wait), applying a change starts a fade; per output while a fade runs the old branch's value at its own place and the new branch's at the common place are blended as `y_old + w_n·(y_new − y_old)` in f64 for n = 1 … N − 1, and sample N is `y_new` itself; `dot` takes the bank as a parameter; the pulls follow the leading branch (R-01). | D1, D4, D5; FR-001, FR-004, FR-005. |
| P2 | N = `speed::CROSSFADE_SAMPLES` = 120 device samples, `CROSSFADE_MS` = 20 published beside `VOLUME_TRANSITION_MS`; the weights from a 120-entry f64 table filled at construction from the smoothstep at u = n ÷ 120 (no divide per sample; the core pins w₃₀ = 0.15625 and w₁₂₀ = 1.0 exactly and strict monotonicity) (R-02). | D2, D3; FR-002; SC-002. |
| P3 | The ring grows 128 → 256 `f32` (the two branches' places diverge by up to 96 stored samples; plus the 57-tap centred kernel = 153 either way round); the pulls follow whichever branch leads (R-03). | D1, D9; FR-009. |
| P4 | The latch, `final_count`, the tail and the drain follow the new branch's place unchanged; a fade continues through the tail and is cut by the drain; the steady-identity fast path requires no fade (R-04). | D6; FR-003, FR-006. |
| P5 | Four mutants in `SpeedMutation`: `fade_step` (no fade, the request applied at once: 0.18 bit for bit; the click reader's switch reference and the re-bless gate), `fade_linear`, `fade_len_delta`, `fade_no_wait` (R-05). | D8; SC-002, SC-003. |
| P6 | The harness reference models the wait and the fade from the published banks and weights; a new click reader `analysis::click_energy_in` (window length and exclusion list as parameters, exclusions folded at the Nyquist; spec 011's reader delegates); a device-rate trace hook for the other rates; `crossfade_checks.rs` with the blend identity, the splice, the click table at 6 kHz (thirteen rows asserted, seven reported, the floor row), the wait cases, the end cut (count, peak, energy) and the partitions (R-06). | D8; FR-007, FR-008, FR-011; SC-001 – SC-004. |
| P7 | Three re-pins, gated: the `fade_step` build regenerates the committed speed golden and the `speed_inside` entries bit for bit before the re-bless; at bless time the constant-level entries and the non-speed entries are asserted byte-identical, every scheduled entry's blend completes before K, and the `speed_inside` counts move as expected; the smoke's speed case re-pinned from `smoke_reference` with its count unchanged; the eleven other audio guards unmoved (R-07). | D7; FR-003; SC-001. |
| P8 | Cost: `crossfade_timing.rs` against 0.18's API (constant level 3 and constant level 5) in a `crossfade` A/B mode against 13defc7 (3 %, `RR_DR60_CROSSFADE_COST_CEILING` 0.05 on the label job; a workflow step) and a head-only `crossfade_fades_timing.rs` (a change every 200 ms; the worst block asserted against the real-time budget, the per-sample cost reported) (R-08). | D9; FR-009; SC-005. |
| P9 | The design script's `--crossfade` print (the weights, the twenty rows' reductions under both readers and the floor, the worst end cut) pins the figures; `RR_DR60_SPEED_CROSSFADE_MS` in C; the records of records.md (A-054 amended as a hypothesis, P1 reworded, stage 9's description carrying the blend and its coverage note rewritten within 120 characters, the CHANGELOG under Changed as an audio change at level changes only, the guide's paragraph with the measured reduction, the product notes incl. investigation-mode, brief and monetization) (R-09). | FR-002, FR-010. |

## Summary

A playback speed-level change becomes a 20 ms time-domain crossfade inside the speed stage: the old level's constant-level computation fades out at its own place and ratio while the new level's fades in from the common place, so no band or content is gated within one sample; a request during a fade waits for its end. A constant level is bit-identical to 0.18; three goldens re-pin their schedule entries; the convention is the emulator's (A-054 amended; capture P1 decides the device's).

## Requirement map

| Requirement | Research | Tests |
|---|---|---|
| FR-001 the blend, the splice | R-01, R-02 | `us1_crossfade` AS2, `crossfade_matrix`, core weight pins, `mutation crossfade::` |
| FR-002 N = 20 ms, the convention | R-02, R-09 | core pins; the A-054 register cross-check |
| FR-003 constant levels bit-identical; the three re-pins | R-07 | the eleven guards, `golden_speed` / `golden_changes` bless-time identities, `smoke_reference` |
| FR-004 level 3 without a branch | R-01 | `us1_crossfade` AS5, core `switch_into_level_3` tests |
| FR-005 the wait | R-01, R-06 | `us2_crossfade` AS1, `crossfade_edge_cases`, the reference |
| FR-006 the length rule, the end cut | R-04 | `us1_crossfade` AS6, `crossfade_matrix` (the worst cut), 013's `lengths` |
| FR-007 the click reduction | R-06 | `us1_crossfade` AS3, `crossfade_matrix` (twelve pairs, both readers), the voiced figure |
| FR-008 partitions, the setter oracle | R-06 | `us1_crossfade` AS4, `crossfade_matrix` |
| FR-009 determinism, real-time, cost | R-03, R-08 | `alloc_free crossfade`, `determinism crossfade`, `scripts/ab-timing.sh 13defc7 crossfade` (levels 3 and 5), `crossfade_fades_timing` (the worst block asserted) |
| FR-010 the records | R-09 | `us2_cross_check` (registers, manifest), the CHANGELOG |
| FR-011 the reference, the weight pins | R-05, R-06 | `us2_speed` (013) green, core `weights_are_the_smoothstep` |
| SC-001 – SC-006 | as above | 0 mismatches; ≥ 30 dB; the A/B; coverage |

## Technical Context

**Language/Version**: Rust 1.85 (edition 2024). **Dependencies**: none new; `no_std` + `alloc`. **Testing**: `cargo test` (core `stages::speed`, `us1_crossfade`, `us2_crossfade`, `crossfade_matrix`, `crossfade_edge_cases`, `mutation crossfade::`, `golden_speed`, `golden_changes`, `us2_speed`, `ffi_parity`, `alloc_free`, `determinism`), the C smoke, the A/B script, the design script. **Target**: every supported host rate; the stage runs at the 6 kHz device rate. **Performance**: a constant level within 3 % of 0.18; a fade two dots per device sample for 120 samples. **Constraints**: no allocation in a call (the table and the ring at construction); the C API and the control manifest unchanged.

No NEEDS CLARIFICATION remains.

## Constitution Check

| Principle | How this plan complies | Pre | Post |
|---|---|---|---|
| I. Portable Library First | `no_std` core; the C API gains one define (`RR_DR60_SPEED_CROSSFADE_MS`), pinned; the header regenerated and drift-checked. | ✅ | ✅ |
| II. Evidence-Based Fidelity | The crossfade is registered as the emulator's convention in A-054 (no device claim); the device's behavior at a change stated as unknown with the candidates (a frame-boundary switch, a step, a mute) and capture P1 recorded. | ✅ | ✅ |
| III. Measured, Test-First Verification | Every oracle written first; the blend identity bit-exact; the click reduction asserted with a reader defined for the case; the re-pins assert their identities at bless time. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | Plain IEEE arithmetic in a fixed order; a weight table and a larger ring at construction; bounded pulls. | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | Inside stage 9's speed stage; the manifest notes name the blend; every constant from the design script. | ✅ | ✅ |
| VI. Neutral, Honest Communication | "An engine convention, not a device behavior" in every record; the figures as the harness prints them. | ✅ | ✅ |
| VII. Independently Testable Features | US1 at 6 kHz through `speed_only` against the harness reference; US2 through the log and the manifest. | ✅ | ✅ |

No violation; no Complexity Tracking entry.

## Project Structure

```text
specs/019-speed-crossfade/  plan.md, research.md, data-model.md, contracts/api.md, quickstart.md, records.md, tasks.md (next)
crates/rr_dr60/src/speed.rs                 # CROSSFADE_SAMPLES, CROSSFADE_MS (public constants)
crates/rr_dr60/src/stages/speed.rs          # Fade state, the weight table, dot(bank, …), RING 256, the wait, the four mutants
crates/rr_dr60/src/lib.rs                   # SpeedMutation's four fields; a weights hook; the device-rate trace hook
crates/rr_dr60_ffi/src/lib.rs, include/rr_dr60.h  # RR_DR60_SPEED_CROSSFADE_MS (one define, pinned)
crates/rr_dr60/src/manifest.rs              # stage 9's notes at 0.19.0
crates/rr_dr60_harness/src/analysis.rs      # click_energy_in (the 011 reader delegates)
crates/rr_dr60_harness/src/speed_checks.rs  # reference_output_at models the wait and the fade; value_at unchanged
crates/rr_dr60_harness/src/crossfade_checks.rs (new)
crates/rr_dr60_harness/src/golden.rs        # bless-time identities for the speed and changes re-pins
crates/rr_dr60_harness/src/smoke_reference.rs  # the speed case's hash and sample count re-pinned
crates/rr_dr60_harness/tests/us1_crossfade.rs, us2_crossfade.rs, crossfade_matrix.rs, crossfade_edge_cases.rs, crossfade_timing.rs (the A/B's, 0.18 API), crossfade_fades_timing.rs (head-only)
crates/rr_dr60_harness/tests/mutation.rs (mod crossfade), ffi_parity.rs, alloc_free.rs, determinism.rs, us1_manifest.rs (stage 9 pins)
crates/rr_dr60_harness/golden/golden-speed-v1.json, golden-changes-v1.json (re-blessed once each), manifest-v1.txt
crates/rr_dr60_ffi/tests/c/smoke.c          # RR_DR60_SMOKE_SPEED_HASH / _SAMPLES re-pinned
tools/playback-speed/speed_design.py (--crossfade)
scripts/ab-timing.sh (crossfade mode), .github/workflows/ab-timing.yml
docs/hardware/assumptions.md (A-054), .github/ISSUE_TEMPLATE/hardware_evidence.yml (P1), docs/guide.md, CHANGELOG.md, docs/product/{controls-discussion,decisions,brief,investigation-mode,monetization}.md, CLAUDE.md
```

## Reviews

The coach (1 – 12), the hardware-emulation reviewer (B1 – B10) and the product specialist (M1 – M6) reviewed this plan on 2026-10-30; every finding is applied above and in research.md, data-model.md, contracts/api.md, quickstart.md and records.md, and the spec carries the restatements as a dated Clarifications session.
