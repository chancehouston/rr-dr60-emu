# Quickstart: Host reads

```sh
cargo test -p rr_dr60 --all-features -- wording meters host_reads drain   # core
cargo test -p rr_dr60_harness --test us1_host_reads -- --nocapture
cargo test -p rr_dr60_harness --test us2_drain -- --nocapture
cargo test -p rr_dr60_harness --test us3_meters_manifest -- --nocapture
cargo test -p rr_dr60_harness --test us4_wording -- --nocapture
cargo test -p rr_dr60_harness --test us2_cross_check canonical readme
uv run tools/band-filter/design_band_filter.py --drain
uv run tools/filter-design/design_voiceband.py --drain
uv run tools/mic-chain/design_mic_chain.py --drain
crates/rr_dr60_ffi/tests/c/run_smoke.sh        # smoke --host-reads against examples/host_reads_rows.rs
cd swift && swift test                          # ReRenderTests
```

Each test binary is run by name (spec 016's lesson: a shared filter matches every binary).
