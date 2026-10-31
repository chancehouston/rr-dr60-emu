# Quickstart: The Pre-1.0 C ABI Pass (spec 022)

| Scenario | Command | Expected |
|---|---|---|
| The FFI's own tests (sizes, published lists, the helper's cases incl. Meters 55…4097 and the readout's doctored list, the check, the create rule 119/120/121, the per-call matrix incl. poisoned, the source scan, the rename) | `cargo test -p rr_dr60_ffi --all-features` | green; every size pinned |
| The one-off Miri gate on the helper | `cargo +nightly miri test -p rr_dr60_ffi write_sized` | clean; the result recorded in research.md |
| The C smoke (the size table, the four cases per out-struct, the painted padding incl. the readout's 212 – 215, the state-unchanged check, the ABI check, the create rule, the three per-call figures) | `crates/rr_dr60_ffi/tests/c/run_smoke.sh` | green; every hash unchanged; `smoke --abi` byte-identical to `examples/abi_rows.rs` |
| Parity Rust ↔ C on the fifteen structs | `cargo test -p rr_dr60_harness --test ffi_parity abi_pass_parity -- --nocapture` | green |
| The allocation and determinism suites (their constructions use `Default`) | `cargo test -p rr_dr60_harness --test alloc_free --test determinism` | green |
| The golden guards and smoke hashes | `cargo test -p rr_dr60_harness --test golden --test golden_agc --test golden_vas --test golden_codec --test golden_output --test golden_output_clamp --test golden_volume_ramp --test golden_mic --test golden_speed --test golden_meters --test golden_changes --test golden_record_changes --test golden_battery --test golden_record --test golden_bypass` | unmoved (fifteen) |
| The core crates untouched (SC-003) | `git diff --quiet 81f14dd -- crates/rr_dr60 crates/rr_dr60_detmath && echo untouched` | `untouched` |
| The header | `cbindgen --config crates/rr_dr60_ffi/cbindgen.toml --crate rr_dr60_ffi --output crates/rr_dr60_ffi/include/rr_dr60.h` then `git diff` | the listed additions, the flattening and the rename only |
| The Swift package | `scripts/build-xcframework.sh && cd swift && swift test` | green; `.sized` everywhere; `checkABI()` passes; the parity test reads `abi_rows` |
| The iOS simulator goldens | `cargo test -p rr_dr60_harness --target aarch64-apple-ios-sim --test golden --test golden_bypass --test determinism` | green |
| The traceability audit | `scripts/check-traceability.sh` | green with the FFI file included |
| The A/B label's existing modes (no regression) | the `ab-timing` label on the PR | within budget |
| The gate | `cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features && RUSTFLAGS="-D warnings" cargo build -p rr_dr60_ffi --release && cargo llvm-cov --all-features --workspace --fail-under-lines 80` | green; coverage within 1 point of 96.64 % |
