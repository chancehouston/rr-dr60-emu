# Quickstart: validating the per-block meters (016)

```sh
cargo test -p rr_dr60 meters                       # core: rest state, the finalize rule, the charge landmarks from the limiter's constants, the VAS countdown
cargo test -p rr_dr60_harness --test us1_meters -- --nocapture    # US1: the tap oracles (peaks), the limiter ratio and charge, the clamp counts, the reference gate, the transport cases (AS1 – AS6) at the measurement rates
cargo test -p rr_dr60_harness --test meters_matrix -- --nocapture # the seven-rate report: every oracle per rate, the charge landmarks, the stop flush and drain figures
cargo test -p rr_dr60_harness --test meters_edge_cases            # zero-length calls, skipped flush, taps, bypassed chain, mute mode, reset/reconfigure
cargo test -p rr_dr60_harness --test mutation meters::            # the mutants: post-clamp reading, post-update gain, off-by-one hang, missing clear
cargo test -p rr_dr60_harness --test golden_meters                # golden-meters-v1.json on this target
cargo test -p rr_dr60_harness --test ffi_parity meters --test alloc_free meters --test determinism meters
crates/rr_dr60_ffi/tests/c/run_smoke.sh            # C smoke, incl. --meters against the Rust read-outs
cargo test -p rr_dr60_harness --test us2_cross_check canonical     # the stage manifest's notes (Meters named; stage 5's note within 120)
python3 tools/agc-model/limiter_model.py | grep -i charge          # the steady charge ratio and the landmarks the harness pins
```

Release-only (solo runs; quote the figures from these):

```sh
scripts/ab-timing.sh fe82f46 output_cost_mic   # the default record front against 0.14: budget 5 % (12 % on the runner)
scripts/ab-timing.sh fe82f46 output_cost       # the bypassed chain: within the same absolute ns
```

Expected: all green; every filter line reports a non-zero test count (`running N tests`, N > 0; tasks.md records each count); the ten earlier golden files and their SHA guards unmoved; `manifest-v1.txt` differs from main in the version line and the `Meters` tokens of nine notes; `control-manifest-v1.txt` in the version line only.

Bless on purpose only: `RR_DR60_BLESS=meters cargo test -p rr_dr60_harness --test golden_meters` (asserts, at bless time, the tap identities and the clamp-count identities on every entry; a CHANGELOG entry). The manifest re-bless, once: `RR_DR60_BLESS=manifest cargo test -p rr_dr60_harness --test us2_cross_check canonical` and `RR_DR60_BLESS=readme cargo test -p rr_dr60_harness --test us2_cross_check readme -- --test-threads=1`.

Before merge: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`, the MSRV build with `RUSTFLAGS="-D warnings"`, the header drift check, `scripts/check-traceability.sh`, `cargo llvm-cov --all-features --workspace --fail-under-lines 80`, the iOS simulator golden run.
