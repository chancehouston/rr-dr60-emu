# Quickstart: validating the battery state (018)

```sh
cargo test -p rr_dr60 battery                                        # core: the law's exact 1.0 at 3.0, the six points and the seven rows within 1e-9 of the script, knee_b == knee at b = 1 over a sweep, the factor trace (sample N/4, monotonic, exact at N), the at-once cases
cargo test -p rr_dr60_harness --test us1_battery -- --nocapture      # US1 AS1 – AS5: the default identity, the below-knee identity, the hard-clipped trajectory and the knee shift, the transition and partitions, the other paths
cargo test -p rr_dr60_harness --test us2_battery -- --nocapture      # US2 AS1 – AS5: kept through reset/reconfigure, refusals, C parity, the meters' clip count and factor, the published law
cargo test -p rr_dr60_harness --test battery_matrix -- --nocapture   # seven rates: the trajectory rows, the knee table shift (≥ 44.1 kHz), the clamp/peak row on the square at three values, the click energy (reported)
cargo test -p rr_dr60_harness --test battery_edge_cases
cargo test -p rr_dr60_harness --test mutation battery::              # the five mutants
cargo test -p rr_dr60_harness --test golden_battery                  # golden-battery-v1.json (259 entries) on this target
cargo test -p rr_dr60_harness --test ffi_parity battery_parity; cargo test -p rr_dr60_harness --test alloc_free battery; cargo test -p rr_dr60_harness --test determinism battery
cargo test -p rr_dr60_harness --test us1_controls --test us2_controls --test us1_manifest --test us2_cross_check   # the thirtieth entry, stage 15 assumed, the registers
crates/rr_dr60_ffi/tests/c/run_smoke.sh                              # incl. --battery against examples/battery_rows.rs
uv run tools/output-chain/design_output_chain.py --battery           # the law's print; --check the bounds; --write the generated constants (byte-identical)
```

Release-only (solo): `scripts/ab-timing.sh 24ffab9 battery` (the speaker route at the default, 3 %; the clipped non-default figure reported).

Expected: all green; every filter line > 0 tests; the twelve earlier golden files and their SHA guards unmoved; the meters golden re-blessed once for its two new columns (digests unchanged); `manifest-v1.txt` and `control-manifest-v1.txt` re-blessed once (stage 15's status and note, the new entry).

Bless on purpose only: `RR_DR60_BLESS=battery cargo test -p rr_dr60_harness --test golden_battery`; `RR_DR60_BLESS=meters …` once; `RR_DR60_BLESS=manifest`, `RR_DR60_BLESS=readme`, `RR_DR60_BLESS=control_manifest` once each (CHANGELOG entries).

Before merge: the gate of CLAUDE.md (fmt, clippy, tests, the feature-less `-D warnings` build, MSRV, the header drift check, traceability, coverage, the iOS simulator golden run) and the `ab-timing` label on the PR.
