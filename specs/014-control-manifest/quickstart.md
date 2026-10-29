# Quickstart: validating the control manifest (014)

```sh
cargo test -p rr_dr60 controls::                 # core: form rules, exhaustive Settings/enum maps, ranges vs Setting::range, defaults at 7 rates
cargo test -p rr_dr60_harness --test us1_controls -- --nocapture   # US1: edge table, live setters, timing/unit/ID checks (FR-008, FR-016)
cargo test -p rr_dr60_harness --test us2_controls                  # US2: tokenizer, setter scan, registers, denylist, 7 doctored inputs (SC-003)
cargo test -p rr_dr60_harness --test ffi_parity --test alloc_free --test determinism
crates/rr_dr60_ffi/tests/c/run_smoke.sh          # C smoke, incl. --controls diff against the golden
cargo run -q -p rr_dr60_harness --example control_manifest        # prints the canonical text
```

Expected: all green; the canonical text equals `crates/rr_dr60_harness/golden/control-manifest-v1.txt`; the audio golden SHA guards unmoved; `manifest-v1.txt` differs from main only in `library_version: 0.14.0`.

Every line must report a non-zero test count (`running N tests`, N > 0): a filter that matches nothing passes silently (spec 012's lesson). tasks.md records each expected count.

Bless after an intended change: `RR_DR60_BLESS=control_manifest cargo test -p rr_dr60_harness --test us1_controls canonical` (a CHANGELOG entry). The version bump (FR-014) also needs, once: `RR_DR60_BLESS=manifest cargo test -p rr_dr60_harness --test us2_cross_check canonical` and `RR_DR60_BLESS=readme cargo test -p rr_dr60_harness --test us2_cross_check readme -- --test-threads=1`.

Before merge: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`, the MSRV build with `RUSTFLAGS="-D warnings"`, the header drift check, `scripts/check-traceability.sh`, `cargo llvm-cov --all-features --workspace --fail-under-lines 80`.
