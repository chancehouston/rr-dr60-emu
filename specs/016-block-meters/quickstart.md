# Quickstart: validating the per-block meters (016)

```sh
cargo test -p rr_dr60 meters                       # core: rest state, the finalize rule, the charge landmarks and v_limit from the limiter's constants, the VAS countdown
cargo test -p rr_dr60_harness --test us1_meters -- --nocapture    # US1: the tap oracles (peaks), the limiter ratio and charge, the clamp counts, the reference gate and the event corroboration (AS1 – AS6) at the measurement rates
cargo test -p rr_dr60_harness --test meters_matrix -- --nocapture # the seven-rate report: every oracle per rate, the per-rate steady pins, the stop flush and drain figures
cargo test -p rr_dr60_harness --test meters_edge_cases            # zero-length calls, skipped flush, the tap table, stage 10 off, the bypassed chain, mute mode, reset/reconfigure, read twice
cargo test -p rr_dr60_harness --test mutation meters::            # the four mutants of MeterMutation
cargo test -p rr_dr60_harness --test golden_meters                # golden-meters-v1.json on this target
cargo test -p rr_dr60_harness --test ffi_parity --test alloc_free --test determinism meters
crates/rr_dr60_ffi/tests/c/run_smoke.sh            # C smoke, incl. --meters diffed against examples/meters_rows.rs
cargo test -p rr_dr60_harness --test us2_cross_check canonical     # the stage manifest's notes (Meters named; stage 5's note within 120)
cargo test -p rr_dr60_harness --test us2_controls                  # still 29 entries; nothing new is a control
python3 tools/agc-model/limiter_model.py | grep -i 'charge\|steady' # the per-rate steady figures and the landmarks the harness pins
```

Release-only (solo runs; quote the figures from these):

```sh
cargo test -p rr_dr60_harness --release --test us1_meters charge_flush -- --ignored --nocapture   # SC-002's exact-0 after the flush length, 6 kHz
scripts/ab-timing.sh fe82f46 meters   # the default front (budget 5 %) and every stage off (the same absolute ns) against 0.14; exits non-zero when over
```

Expected: all green; every filter line reports a non-zero test count (`running N tests`, N > 0; tasks.md records each count); the nine earlier golden files and their SHA guards unmoved; `manifest-v1.txt` differs from main in the version line and the `Meters` tokens of nine notes; `control-manifest-v1.txt` in the version line only.

Bless on purpose only: `RR_DR60_BLESS=meters cargo test -p rr_dr60_harness --test golden_meters` (asserts, at bless time, the tap and count identities on every entry and a paused row plus a resume in each short-hang entry; a CHANGELOG entry). The manifest re-bless, once: `RR_DR60_BLESS=manifest cargo test -p rr_dr60_harness --test us2_cross_check canonical` and `RR_DR60_BLESS=readme cargo test -p rr_dr60_harness --test us2_cross_check readme -- --test-threads=1`.

Before merge: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`, the MSRV build with `RUSTFLAGS="-D warnings"`, the header drift check, `scripts/check-traceability.sh`, `cargo llvm-cov --all-features --workspace --fail-under-lines 80`, the iOS simulator golden run, and the `ab-timing` label on the PR.
