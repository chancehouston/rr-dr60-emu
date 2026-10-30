# Quickstart: validating the live record-side values (017)

```sh
cargo test -p rr_dr60 -- record_ctl retune set_values hiss_transition   # core: the record control state, clock_of, record_landing at seven rates, the retune's field list and vc continuity, the gate's set_values with counts kept, the hiss transition's end sample (P + N − 1) and at-once rule
cargo test -p rr_dr60_harness --test us1_record -- --nocapture     # US1 AS1 – AS8 at the measurement rates: the input-0 identity, the pre-landing identity (codec off per sample, on per frame), the limiter, the reference gate, the hang shortening, the hiss ratio, the passed position, the event bound
cargo test -p rr_dr60_harness --test us2_record -- --nocapture     # US2 AS1 – AS4: two engines, record-then-play with changes, the stop (fed silence, final position, self-stop), the cross-stream landing and settings()
cargo test -p rr_dr60_harness --test record_matrix -- --nocapture  # seven rates: the partition identity per value and stream, the k(P) pin (the stored sum), the 138-entry one-pass call and the 72-entry recorder call
cargo test -p rr_dr60_harness --test record_edge_cases             # front taps, capsule/limiter bypassed, reset/reconfigure, the adopt rule, the skipped-flush stop, same-position pairs, mute mode, refusals, the clocks, the two sides' shared rules
cargo test -p rr_dr60_harness --test mutation record::             # the six mutants of RecordMutation
cargo test -p rr_dr60_harness --test golden_record_changes         # golden-record-changes-v1.json (714 entries) on this target
cargo test -p rr_dr60_harness --test ffi_parity record_parity; cargo test -p rr_dr60_harness --test alloc_free record_changes; cargo test -p rr_dr60_harness --test determinism record_same   # one filter per binary (a shared filter matches no test in the others)
cargo test -p rr_dr60_harness --test us1_controls --test us2_controls   # the control manifest: eleven live entries (the eight through the schedule), canonical re-blessed once
crates/rr_dr60_ffi/tests/c/run_smoke.sh            # C smoke, incl. --record-changes diffed against examples/record_changes_rows.rs
cargo test -p rr_dr60_harness --test us1_changes --test us2_changes --test changes_matrix   # spec 015's suite with the 139 bound restated
```

Release-only (solo runs; quote the figures from these):

```sh
scripts/ab-timing.sh 06e40b0 record   # the record path and the one-pass with the schedules empty (budget 3 %) against 0.16; then runs record_pending_timing (one knee change per block; the worst block) and reports it; exits non-zero when over
```

Expected: all green; every filter line reports a non-zero test count; the eleven earlier golden files and their SHA guards unmoved; `control-manifest-v1.txt` differs from main in the eight timing words; `manifest-v1.txt` in its version line only.

Bless on purpose only: `RR_DR60_BLESS=record_changes cargo test -p rr_dr60_harness --test golden_record_changes` (asserts the input-0 and partition identities at bless time; a CHANGELOG entry); `RR_DR60_BLESS=control_manifest cargo test -p rr_dr60_harness --test us1_controls canonical` once for the timing words.

Before merge: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`, `RUSTFLAGS="-D warnings" cargo build -p rr_dr60_ffi --release` (feature-less), the MSRV build with `RUSTFLAGS="-D warnings"`, the header drift check, `scripts/check-traceability.sh`, `cargo llvm-cov --all-features --workspace --fail-under-lines 80`, the iOS simulator golden run, and the `ab-timing` label on the PR.
