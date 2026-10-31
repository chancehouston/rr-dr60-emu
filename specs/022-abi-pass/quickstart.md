# Quickstart: The Pre-1.0 C ABI Pass (spec 022)

| Scenario | Command | Expected |
|---|---|---|
| The FFI's own tests (sizes, minimums, the helper's cases, the check, the rename) | `cargo test -p rr_dr60_ffi --all-features` | green; every size pinned |
| The C smoke (the size table, the cases per struct, the painted padding, the check, the per-call figure) | `crates/rr_dr60_ffi/tests/c/run_smoke.sh` | green; every hash unchanged; the figure printed |
| Parity Rust ↔ C on the eleven structs | `cargo test -p rr_dr60_harness --test ffi_parity abi_pass_parity -- --nocapture` | green |
| The allocation and determinism suites (their constructions use `Default`) | `cargo test -p rr_dr60_harness --test alloc_free --test determinism` | green |
| The golden guards and smoke hashes | `cargo test -p rr_dr60_harness --test golden --test golden_agc --test golden_vas --test golden_codec --test golden_output --test golden_output_clamp --test golden_volume_ramp --test golden_mic --test golden_speed --test golden_meters --test golden_changes --test golden_record_changes --test golden_battery --test golden_record --test golden_bypass` | unmoved (fifteen) |
| The header | `cbindgen --config crates/rr_dr60_ffi/cbindgen.toml --crate rr_dr60_ffi --output crates/rr_dr60_ffi/include/rr_dr60.h` then `git diff` | the listed additions, the flattening and the rename only |
| The Swift package | `scripts/build-xcframework.sh && cd swift && swift test` | green; `.sized` everywhere; `checkABI()` passes |
| The iOS simulator goldens | `cargo test -p rr_dr60_harness --target aarch64-apple-ios-sim --test golden --test golden_bypass --test determinism` | green |
| The A/B label's existing modes (no regression) | the `ab-timing` label on the PR | within budget |
| The gate | `cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features && RUSTFLAGS="-D warnings" cargo build -p rr_dr60_ffi --release && cargo llvm-cov --all-features --workspace --fail-under-lines 80` | green; coverage within 1 point of 96.64 % |
