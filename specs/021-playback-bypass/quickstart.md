# Quickstart: Live Playback-Stage Bypass (spec 021)

Prerequisites: the toolchain of CLAUDE.md; the tree at or after the feature's merge.

| Scenario | Command | Expected |
|---|---|---|
| The core: the fade's arithmetic, the reverse, the clamp at both ends, the pins | `cargo test -p rr_dr60 -- fade bypass` | green; `w(N/2)` within 1 ULP of 0.5 at seven rates |
| US1 at the measurement rates: the endpoint identities, the blend identity, the click rows, the partitions, the reversal | `cargo test -p rr_dr60_harness --test us1_bypass -- --nocapture` | green; the stage-10 settle, the click table and the level table printed |
| US2: the refusals, the lifecycle, the log, the mirror, the meters, the manifest, the trap | `cargo test -p rr_dr60_harness --test us2_bypass -- --nocapture` | green |
| Seven rates | `cargo test -p rr_dr60_harness --test bypass_matrix -- --nocapture` | green; the per-rate tables |
| Edge cases | `cargo test -p rr_dr60_harness --test bypass_edge_cases` | green |
| Mutants | `cargo test -p rr_dr60_harness --test mutation bypass::` | six caught |
| Golden | `cargo test -p rr_dr60_harness --test golden_bypass` | green; re-bless `RR_DR60_BLESS=bypass …` after an intended change (a CHANGELOG entry) |
| The pre-021 guards | `cargo test -p rr_dr60_harness --test golden --test golden_agc --test golden_vas --test golden_codec --test golden_output --test golden_output_clamp --test golden_volume_ramp --test golden_mic --test golden_speed --test golden_meters --test golden_changes --test golden_record_changes --test golden_battery --test golden_record` | unmoved |
| Controls and registers | `cargo test -p rr_dr60_harness --test us1_controls --test us2_controls --test us2_cross_check` | the four entries live; the guide section passes the forbidden-list scan |
| Parity, allocation, determinism | `cargo test -p rr_dr60_harness --test ffi_parity --test alloc_free --test determinism bypass` | green |
| C smoke | `crates/rr_dr60_ffi/tests/c/run_smoke.sh` | `--bypass` byte-identical to `examples/bypass_rows.rs` |
| Cost | `scripts/ab-timing.sh f303c99 bypass` (solo) | player and one-pass within 3 %; the bypassed state and a blend reported |
| Gate | `cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features && RUSTFLAGS="-D warnings" cargo build -p rr_dr60_ffi --release && cargo llvm-cov --all-features --workspace --fail-under-lines 80` | green; coverage within 1 point of 96.70 % |
