# Quickstart: validating the sample-timed changes and the change log (015)

```sh
cargo test -p rr_dr60 changes                        # core: the schedule's order, capacity, refusals and the start-keeps/reset-clears rule; speed_landing at the seven rates (d(0) = 0, d(P) = P at 6 kHz, the rate refusals); reset_sections keeps the accumulators; the entry layout
cargo test -p rr_dr60_harness --test us1_changes -- --nocapture    # US1 AS1 – AS9 at the measurement rates: the block-size-1 setter oracle, the splice identity, the route oracle, the 6 kHz speed reference, passed and same-position cases, position order, the meters across a route switch
cargo test -p rr_dr60_harness --test us2_changes -- --nocapture    # US2 AS1 – AS6: the log against the schedule, the plain setters and lifecycle calls, the short buffer, the no-ops, the cross-stream entry
cargo test -p rr_dr60_harness --test changes_matrix -- --nocapture # the seven-rate report: partition identities per control and stream, the place pin of d(P), the speed's audible lag = playback_latency_samples ± 2 (asserted), the 64 + 3 call
cargo test -p rr_dr60_harness --test changes_edge_cases            # drain, past-the-end, schedule-then-start, erase/erase_all clearing, refusals, the clock vs. the events, the front taps, zero-production calls, the readings after landing, the named-tap refusal after a scheduled route
cargo test -p rr_dr60_harness --test mutation changes::            # the six mutants of ChangeMutation
cargo test -p rr_dr60_harness --test golden_changes                # golden-changes-v1.json on this target
cargo test -p rr_dr60_harness --test ffi_parity --test alloc_free --test determinism changes
crates/rr_dr60_ffi/tests/c/run_smoke.sh            # C smoke, incl. --changes diffed against examples/changes_rows.rs
cargo test -p rr_dr60_harness --test us2_cross_check canonical     # the stage manifest's API items of stages 9 and 11 name schedule_change
cargo test -p rr_dr60_harness --test us2_controls                  # still 29 entries; the setter scan sees no new set_ name
```

Release-only (solo runs; quote the figures from these):

```sh
scripts/ab-timing.sh f1449c2 changes   # the playback and the one-pass with an empty schedule (budget 3 %) against 0.15; one change per block reported; exits non-zero when over
```

Expected: all green; every filter line reports a non-zero test count (tasks.md records each); the ten earlier golden files and their SHA guards unmoved; `manifest-v1.txt` differs from main in the version line and the API items of stages 9 and 11; `control-manifest-v1.txt` unchanged (it has no version line).

Bless on purpose only: `RR_DR60_BLESS=changes cargo test -p rr_dr60_harness --test golden_changes` (asserts, at bless time, the partition identity and the block-size-1 oracle on every entry; a CHANGELOG entry). The manifest re-bless, once: `RR_DR60_BLESS=manifest cargo test -p rr_dr60_harness --test us2_cross_check canonical` and `RR_DR60_BLESS=readme cargo test -p rr_dr60_harness --test us2_cross_check readme -- --test-threads=1`.

Before merge: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`, `RUSTFLAGS="-D warnings" cargo build -p rr_dr60_ffi --release` (feature-less), the MSRV build with `RUSTFLAGS="-D warnings"`, the header drift check, `scripts/check-traceability.sh`, `cargo llvm-cov --all-features --workspace --fail-under-lines 80`, the iOS simulator golden run, and the `ab-timing` label on the PR.
