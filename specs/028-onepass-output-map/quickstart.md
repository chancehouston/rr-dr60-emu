# Quickstart: validating the one-pass output map

- The core: `cargo test -p rr_dr60 --all-features -- onepass_map`.
- The stories and the matrix: `cargo test -p rr_dr60_harness --test us1_onepass_map --test us2_onepass_map --test us3_onepass_map --test onepass_map_matrix --test onepass_map_edge_cases onepass_map -- --nocapture` (every binary must report a non-zero count; R-00's reproduction first: the labelled-gap figures per rate and cell: f_a over f_b in each span, the label shares for both maps, the record-then-play deviation).
- Mutants, allocation, parity, documentation: `cargo test -p rr_dr60_harness --test mutation --test alloc_free --test ffi_parity --test us2_cross_check onepass_map`.
- The ABI: `cargo test -p rr_dr60_harness --test us2_abi spec_02` (028's pin and 027's restated pin); `scripts/abi-promise.sh ad1ba8d401228c91480f12bd8ff3df68ac489e7f`; Miri `cargo +nightly miri test -p rr_dr60_ffi --lib -- abi_tests::onepass_map`.
- C and Swift: `crates/rr_dr60_ffi/tests/c/run_smoke.sh` (`smoke --onepass-map` against `cargo run -q -p rr_dr60_harness --example onepass_map_rows`); `scripts/build-xcframework.sh && cd swift && swift test` (`OnePassMapTests`).
- Expected: every figure within spec D7's bounds; the existing map's label share 0 % on `[pos, S(e))`; the record-then-play centre within ⌈m/l⌉ in every cell; the seventeen golden files unmoved.
