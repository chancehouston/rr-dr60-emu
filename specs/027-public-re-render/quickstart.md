# Quickstart: validating 027

```sh
cargo test -p rr_dr60 --all-features rerender                       # core: plan, split rule, fill
cargo test -p rr_dr60_harness --test us1_rerender --test us2_rebuild --test us3_stream --test us4_surfaces -- --nocapture
cargo test -p rr_dr60_harness --test mutation rerender::
cargo test -p rr_dr60_harness --test ffi_parity rerender
cargo test -p rr_dr60_harness --release --test us3_stream as2_hour -- --ignored --nocapture
crates/rr_dr60_ffi/tests/c/run_smoke.sh                              # smoke --rerender
scripts/build-xcframework.sh && (cd swift && swift test)              # ReRenderTests, ReRenderSessionTests
```

Expected: every cell `Identical` at the measurement rates; the dense-row cell fails on the harness recipe at block 4096 and holds on the session; the six mutants fail their tests; the hour within 1.5× of the record path's cost.
