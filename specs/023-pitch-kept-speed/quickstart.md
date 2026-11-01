# Quickstart: Pitch-Kept Playback Speed (spec 023)

| Scenario | Command | Expected |
|---|---|---|
| The core (the window pins, T_c, the anchor, the search's tie-break, the mode fade, the wait rule, the mutants) | `cargo test -p rr_dr60 stages::speed` | green |
| The design script | `uv run tools/playback-speed/speed_design.py --pitch-kept` / `--check` / `--write` | the figures; the margins; `speed_window.rs` byte-identical |
| US1 at the measurement rates | `cargo test -p rr_dr60_harness --test us1_pitch -- --nocapture` | the off-grid tone, the vowel, the glide, the lengths, the onset, the click counts printed |
| US2 | `cargo test -p rr_dr60_harness --test us2_pitch -- --nocapture` | the mode blend, the level change, the lifecycle, the manifest, the refusals, the wait rule |
| Seven rates | `cargo test -p rr_dr60_harness --test pitch_matrix -- --nocapture` | every cell; the shift digests against the reference |
| Edge cases, mutants, golden, parity, alloc, determinism | `cargo test -p rr_dr60_harness --test pitch_edge_cases --test golden_pitch_kept --test ffi_parity pitch --test alloc_free pitch --test determinism pitch` and `--test mutation pitch::` | green |
| The promise | `scripts/abi-promise.sh 12a3c45` | green |
| The C smoke | `crates/rr_dr60_ffi/tests/c/run_smoke.sh` (`smoke --pitch` against `examples/pitch_rows.rs`) | green; every hash unchanged |
| Release-only | `cargo test -p rr_dr60_harness --release --test pitch_timing -- --ignored --nocapture --test-threads=1`; `scripts/ab-timing.sh 12a3c45 pitch` | within budget |
| Re-bless | `RR_DR60_BLESS=pitch_kept …golden_pitch_kept`; `RR_DR60_BLESS=control_manifest …`; `RR_DR60_BLESS=manifest …`; `RR_DR60_BLESS=readme …` | once each |
| The gate | fmt, clippy, the feature-less build, the iOS build, the header drift, `cargo test --all-features`, MSRV, the simulator goldens, coverage | green; within 1 point of 96.73 % |
