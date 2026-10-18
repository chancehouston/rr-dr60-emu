# Quickstart: validating spec 013 (playback speed)

Prerequisites: Rust stable with `~/.cargo/bin` on `PATH`; `uv` for the reference script; for the C loop a C compiler; for the A/B a clean working tree (it builds a baseline worktree).

## Design figures

```sh
uv run tools/playback-speed/speed_design.py          # every figure (research R-03, R-04, R-11)
uv run tools/playback-speed/speed_design.py --check  # the bounds: passband ≤ 0.1 dB, spurious ≤ −60 dB, tails ≤ 50, the switch ≤ 4
```

## US1: constant levels

```sh
cargo test -p rr_dr60 stages::speed                                   # the core: banks against the script, the place, the tail, the latch
cargo test -p rr_dr60_harness --test us1_speed -- --nocapture         # lengths, FR-004 at 6 kHz, the impulse, the tail, level 3, AS5
cargo test -p rr_dr60_harness --test speed_matrix -- --nocapture      # the seven-rate report
```

Expected: lengths exact (⌈(⌈5K ÷ k⌉ + T) · m ÷ l⌉ before the drain); at 6 kHz fits within ±0.1 dB, spurious at or below −60 dB and within 0.5 dB of the script; impulse peaks at 1000 / 750 / 500 / 430 (j = 600, and 602 at level 5); level 3 bit-identical; the eight golden guards unmoved.

## US2: live changes

```sh
cargo test -p rr_dr60_harness --test us2_speed -- --nocapture         # every ordered pair bit-exact against the independent reference
cargo test -p rr_dr60_harness --test speed_edge_cases
cargo test -p rr_dr60_harness --test determinism speed
cargo test -p rr_dr60_harness --test alloc_free
cargo test -p rr_dr60_harness --test mutation speed::                 # six mutants caught
cargo test -p rr_dr60_harness --release --test timing speed_cost speed_codec_worst_block -- --ignored --nocapture --test-threads=1
scripts/ab-timing.sh 3a4eefb playback                                 # level 3 against 0.12: ≤ 3 % (solo; 45 alternating rounds)
```

## US3: records

```sh
cargo test -p rr_dr60_harness --test us1_manifest --test us2_cross_check -- --nocapture
scripts/check-traceability.sh
```

## C

```sh
crates/rr_dr60_ffi/tests/c/run_smoke.sh
cargo test -p rr_dr60_harness --test ffi_parity speed
```

## Golden (Phase 6, once, after the reviews)

```sh
RR_DR60_BLESS=speed cargo test -p rr_dr60_harness --test golden_speed
```

## Gates

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features                       # before the bless (T035): -- --skip golden_speed
RUSTFLAGS="-D warnings" cargo +1.85 test --no-run --all-features
cargo llvm-cov --all-features --workspace --fail-under-lines 80
```
