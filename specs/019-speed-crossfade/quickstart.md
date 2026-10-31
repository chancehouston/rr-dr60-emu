# Quickstart: validating the speed-change crossfade (019)

```sh
cargo test -p rr_dr60 stages::speed                                   # core: the weight pins (w30 = 0.15625, w120 = 1 exactly, strictly increasing), the fade's start and end, the wait, level 3 without a branch, the ring span, the constant-level identity
cargo test -p rr_dr60_harness --test us1_crossfade -- --nocapture     # US1 AS1 – AS6: the constant-level identity, the blend identity to the bit (6 kHz; the device-rate trace at the other rates), the click table (the thirteen 2 and 2.4 kHz rows ≥ 30 dB under the 40 ms reader; the 1 kHz rows, the floor and the 0.5 s reader reported), the partitions, level 3, the end cut
cargo test -p rr_dr60_harness --test us2_crossfade -- --nocapture     # US2 AS1 – AS2: the wait cases against the reference (consecutive, superseded, equal, at the latch), the manifest's stage 9 and A-054's pinned phrases (AS3 is the A/B script; AS4 is `ffi_parity speed`)
cargo test -p rr_dr60_harness --test crossfade_matrix -- --nocapture  # seven rates: the blend identity (trace) and the splice per pair, the partitions, the lengths; at 6 kHz the click table under both readers, the voiced energies, the end cut (count, peak, energy) in both configurations
cargo test -p rr_dr60_harness --test crossfade_edge_cases
cargo test -p rr_dr60_harness --test mutation crossfade::             # four mutants
cargo test -p rr_dr60_harness --test us2_speed --test us1_speed --test speed_matrix --test speed_edge_cases   # spec 013 green against the updated reference
cargo test -p rr_dr60_harness --test golden_speed --test golden_changes                                        # the two re-blessed goldens on this target
cargo test -p rr_dr60_harness --test ffi_parity speed; cargo test -p rr_dr60_harness --test alloc_free crossfade; cargo test -p rr_dr60_harness --test determinism crossfade   # the new functions carry "crossfade" in their names
cargo test -p rr_dr60_harness --test us1_manifest --test us2_cross_check   # stage 9's notes, A-054, the canonical text
crates/rr_dr60_ffi/tests/c/run_smoke.sh                               # the speed case re-pinned
uv run tools/playback-speed/speed_design.py --crossfade               # the weights, the twelve pairs' reductions, the worst end cut; --check
```

Release-only (solo): `scripts/ab-timing.sh 13defc7 crossfade` (constant levels 3 and 5 at the default front against 0.18, both 3 %; `RR_DR60_CROSSFADE_COST_CEILING` 0.05 on the label job), then `cargo test -p rr_dr60_harness --release --test crossfade_fades_timing -- --ignored --nocapture` (a change every 200 ms: the worst block asserted against the real-time budget, the per-sample cost reported).

Expected: all green; every filter line > 0 tests; the eleven audio golden guards without a speed schedule unmoved; before the re-bless the `fade_step` build regenerates the committed speed golden and `speed_inside` entries bit for bit (the gate); `golden-speed-v1.json` and `golden-changes-v1.json` re-blessed once each with their constant-level and non-speed entries asserted byte-identical at bless time and the `speed_inside` counts moved as the wait predicts; the smoke's speed hash re-pinned once, its count unchanged; the stage manifest's canonical text, the README table and the guide blocks re-blessed once (stage 9's description and coverage note); the control manifest byte-identical; the header gains `RR_DR60_SPEED_CROSSFADE_MS` only.

Bless on purpose only: `RR_DR60_BLESS=speed cargo test -p rr_dr60_harness --test golden_speed`, `RR_DR60_BLESS=changes cargo test -p rr_dr60_harness --test golden_changes`, `RR_DR60_BLESS=manifest …`, `RR_DR60_BLESS=readme …` (CHANGELOG entries).

Before merge: the gate of CLAUDE.md and the `ab-timing` label on the PR.
