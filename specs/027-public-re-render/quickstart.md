# Quickstart: validating 027

```sh
cargo test -p rr_dr60 --all-features rerender                       # core: the plan's rows, the bit compare (2 tests)
cargo test -p rr_dr60_harness --test us1_rerender --test us2_rebuild --test us3_stream --test us4_surfaces -- --nocapture
cargo test -p rr_dr60_harness --test mutation rerender::
cargo test -p rr_dr60_harness --test ffi_parity --test alloc_free --test determinism rerender
cargo test -p rr_dr60_harness --test us2_cross_check guide_rerender
cargo test -p rr_dr60_ffi --all-features rerender                     # abi_tests
cargo +nightly miri test -p rr_dr60_ffi --lib -- abi_tests::rerender          # one-off: about 26 min (the real session); CI runs abi_tests::rerender_order
for r in 12a3c4560855a87990d0d47a48a49d99355cdcfd f5a02452728f3d0687c033689266a5bbdf88c6db 97bbdc24e2f358f6c8d89c75f5472a62985cf276 2a170d49340e09324020faef56e11e1845833002 3890bfe…; do scripts/abi-promise.sh $r; done   # 3890bfe by its full id
scripts/ab-timing.sh 3890bfe record
cargo test -p rr_dr60_harness --release --test us3_stream as2_rerender -- --ignored --nocapture --test-threads=1   # the hour and new on a full memory
crates/rr_dr60_ffi/tests/c/run_smoke.sh                              # smoke --rerender
scripts/build-xcframework.sh && (cd swift && swift test)              # ReRenderTests, ReRenderTests
```

Expected: every cell `Identical` at the measurement rates; the dense-row cell fails on the harness recipe at block 4096 and holds on the session; the six mutants fail their tests; the hour within 1.5× of the record path's cost.
