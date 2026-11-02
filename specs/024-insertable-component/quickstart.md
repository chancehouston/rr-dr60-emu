# Quickstart: An Insertable Component Slot and the Band Filter (spec 024)

| Scenario | Command | Expected |
|---|---|---|
| The core (the section, the zeros, the mixes, the ramps, the blend, the store, the manifests, 158/146, the mutants) | `cargo test -p rr_dr60 --all-features -- band_filter insert components changes` | green |
| The design script | `uv run tools/band-filter/design_band_filter.py` / `--check` / `--write` | the response tables; the zeros and unity points; `band_filter_ref.rs` byte-identical |
| US1 at the measurement rates | `cargo test -p rr_dr60_harness --test us1_insert -- --nocapture` | the response, the placement identity, the transitions and clicks, the empty-slot twin, the lifecycle, the gate interaction printed |
| US2 | `cargo test -p rr_dr60_harness --test us2_insert -- --nocapture` | the manifests, 158, the record v2 round trip, the parity, the refusals |
| Seven rates | `cargo test -p rr_dr60_harness --test insert_matrix -- --nocapture` | every cell; the partitions; `the_tight_bounds` 158 / 146 |
| Edge cases, mutants, golden, parity, alloc, determinism | `cargo test -p rr_dr60_harness --test insert_edge_cases --test golden_insert --test ffi_parity --test alloc_free --test determinism insert` and `--test mutation insert::` | green |
| The record | `cargo test -p rr_dr60_harness --test golden_record --test us1_record_contract --test us2_record_read` | green after the one re-bless |
| The promise | `scripts/abi-promise.sh 12a3c45… && scripts/abi-promise.sh f5a0245… && scripts/abi-promise.sh 97bbdc2…` | green |
| The C smoke | `crates/rr_dr60_ffi/tests/c/run_smoke.sh` (`smoke --insert` against `examples/insert_rows.rs`) | green; every earlier hash unchanged |
| Miri | `cargo +nightly miri test -p rr_dr60_ffi --lib -- abi_tests` | green |
| Swift | `scripts/build-xcframework.sh && cd swift && swift test` | green (the re-render with the filter) |
| Release-only | `cargo test -p rr_dr60_harness --release --test insert_timing -- --ignored --nocapture --test-threads=1`; `scripts/ab-timing.sh feec054 insert` | within budget |
| Re-bless | `RR_DR60_BLESS=insert …golden_insert`; `RR_DR60_BLESS=record …golden_record`; `RR_DR60_BLESS=control_manifest …`; `RR_DR60_BLESS=manifest …`; `RR_DR60_BLESS=readme …` | once each |
| The gate | fmt, clippy, the feature-less `-D warnings` build, the iOS build, the header drift, `cargo test --all-features`, MSRV, the simulator goldens, coverage | green; within 1 point of 96.69 % |
