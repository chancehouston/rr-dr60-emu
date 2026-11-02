# Quickstart: validating spec 025

```
cargo test -p rr_dr60 --all-features control_map       # the mapping's exhaustiveness, accessors, conversion
cargo test -p rr_dr60_harness --test us1_settings_read --test us2_control_index --test settings_read_edge_cases -- --nocapture
cargo test -p rr_dr60_harness --test mutation settings_read
cargo test -p rr_dr60_harness --test ffi_parity --test alloc_free --test determinism settings_read
cargo test -p rr_dr60_harness --test golden_record      # the record's text unmoved
cargo +nightly miri test -p rr_dr60_ffi --lib -- abi_tests::settings_read abi_tests::control_reading settings_read::tests
crates/rr_dr60_ffi/tests/c/run_smoke.sh                 # smoke --settings against examples/settings_rows.rs
scripts/abi-promise.sh 12a3c45 && scripts/abi-promise.sh f5a02452728f3d0687c033689266a5bbdf88c6db
scripts/build-xcframework.sh && cd swift && swift test  # ControlReadingTests
```

Expected: every test green; the sixteen golden guards and every smoke hash unmoved; 26 of 26 settings pass the one-hot oracle in Rust and C; the five live controls by index log the same entries as their setters.
