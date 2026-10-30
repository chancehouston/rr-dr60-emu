# Quickstart: validating the live record-side values (017)

```sh
cargo test -p rr_dr60 record_ctl                     # core: the record control state, clock_of, record_landing at seven rates, the retune's field list and vc continuity, the gate's set_values with counts kept, the hiss transition's end assignment and at-once rule
cargo test -p rr_dr60_harness --test us1_record -- --nocapture     # US1 AS1 – AS8 at the measurement rates: the input-0 identity, the pre-landing identity (codec off per sample, on per frame), the limiter, the reference gate, the hang shortening, the hiss ratio, the passed position, the event bound
cargo test -p rr_dr60_harness --test us2_record -- --nocapture     # US2 AS1 – AS4: two engines, record-then-play with changes, the stop (fed silence, final position, self-stop), the cross-stream landing and settings()
cargo test -p rr_dr60_harness --test record_matrix -- --nocapture  # seven rates: the partition identity per value and stream, the k(P) pin, the 64 + 64 + 3 call
cargo test -p rr_dr60_harness --test record_edge_cases             # front taps, capsule/limiter bypassed, reset/reconfigure, same-position pairs, mute mode, refusals, the clocks
cargo test -p rr_dr60_harness --test mutation record::             # the six mutants of RecordMutation
cargo test -p rr_dr60_harness --test golden_record_changes         # golden-record-changes-v1.json on this target
cargo test -p rr_dr60_harness --test ffi_parity --test alloc_free --test determinism record
cargo test -p rr_dr60_harness --test us1_controls --test us2_controls   # the control manifest: eight entries live, canonical re-blessed
crates/rr_dr60_ffi/tests/c/run_smoke.sh            # C smoke, incl. --record-changes diffed against examples/record_changes_rows.rs
cargo test -p rr_dr60_harness --test us1_changes --test us2_changes --test changes_matrix   # spec 015's suite with the 131 bound restated
```

Release-only (solo runs; quote the figures from these):

```sh
scripts/ab-timing.sh 06e40b0 record   # the record path and the one-pass with the schedules empty (budget 3 %) against 0.16; one knee change per block and the worst block reported; exits non-zero when over
```

Expected: all green; every filter line reports a non-zero test count; the eleven earlier golden files and their SHA guards unmoved; `control-manifest-v1.txt` differs from main in the eight timing words; `manifest-v1.txt` in its version line only.

Bless on purpose only: `RR_DR60_BLESS=record_changes cargo test -p rr_dr60_harness --test golden_record_changes` (asserts the input-0 and partition identities at bless time; a CHANGELOG entry); `RR_DR60_BLESS=control_manifest cargo test -p rr_dr60_harness --test us1_controls canonical` once for the timing words.

Before merge: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`, `RUSTFLAGS="-D warnings" cargo build -p rr_dr60_ffi --release` (feature-less), the MSRV build with `RUSTFLAGS="-D warnings"`, the header drift check, `scripts/check-traceability.sh`, `cargo llvm-cov --all-features --workspace --fail-under-lines 80`, the iOS simulator golden run, and the `ab-timing` label on the PR.
