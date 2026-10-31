# Quickstart: validating the battery state (018)

```sh
cargo test -p rr_dr60 battery                                        # core: the law's exact 1.0 at 3.0, the six points, the seven rows and NiMH bit-exact to the script's f32 print (ceiling_db within 1e-5), knee_b == knee at b = 1 over a sweep, the factor trace (sample ⌊N/4⌋ at the smoothstep's value per rate, monotonic, exact at N), the at-once cases, the idempotent setter
cargo test -p rr_dr60_harness --test us1_battery -- --nocapture      # US1 AS1 – AS5: the default identity, the below-knee identity (premise asserted), the hard-clipped trajectory (sample peak = b to the bit) and the knee shift, the transition and partitions, the other paths
cargo test -p rr_dr60_harness --test us2_battery -- --nocapture      # US2 AS1 – AS5: kept through reset/reconfigure, refusals, C parity, the meters' clip count and factor, the published law
cargo test -p rr_dr60_harness --test battery_matrix -- --nocapture   # seven rates: the trajectory rows, the knee table shift (≥ 44.1 kHz), the clamp/peak row on the square at three values, the click energy (reported)
cargo test -p rr_dr60_harness --test battery_edge_cases
cargo test -p rr_dr60_harness --test mutation battery::              # the five mutants
cargo test -p rr_dr60_harness --test golden_battery                  # golden-battery-v1.json (259 entries: 252 + 7 scheduled) on this target
cargo test -p rr_dr60_harness --test ffi_parity battery_parity; cargo test -p rr_dr60_harness --test alloc_free battery; cargo test -p rr_dr60_harness --test determinism battery
cargo test -p rr_dr60_harness --test us1_controls --test us2_controls --test us1_manifest --test us2_cross_check   # the thirtieth entry, stage 15 assumed, the registers
crates/rr_dr60_ffi/tests/c/run_smoke.sh                              # incl. --battery against examples/battery_rows.rs
uv run tools/output-chain/design_output_chain.py --battery           # the law's print; --check the bounds; --write the generated constants (byte-identical)
```

Release-only (solo): `scripts/ab-timing.sh 24ffab9 battery` (the speaker route at the default, 3 %, `RR_DR60_BATTERY_COST_CEILING` 0.05 on the label job; then `battery_clipped_timing` at 2.4 V on a full-scale square, reported).

Expected: all green; every filter line > 0 tests; the twelve earlier golden files and their SHA guards unmoved (the meters golden included: its row keeps eleven columns); `manifest-v1.txt` and `control-manifest-v1.txt` re-blessed once (stage 15's status and notes, stage 11's coverage note, the new entry appended), the README table and the guide's blocks with them.

Bless on purpose only: `RR_DR60_BLESS=battery cargo test -p rr_dr60_harness --test golden_battery` (asserts the default, below-knee, earphone and no-op twins at bless time); `RR_DR60_BLESS=manifest`, `RR_DR60_BLESS=readme`, `RR_DR60_BLESS=control_manifest` once each (CHANGELOG entries).

Before merge: the gate of CLAUDE.md (fmt, clippy, tests, the feature-less `-D warnings` build, MSRV, the header drift check, traceability, coverage, the iOS simulator golden run) and the `ab-timing` label on the PR.
