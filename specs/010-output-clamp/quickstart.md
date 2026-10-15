# Quickstart: validating the output clamp (feature 010)

Prerequisites: the toolchain of CLAUDE.md (`export PATH="$HOME/.cargo/bin:$PATH"`), branch `010-output-clamp`. Every command runs from the repository root. The expected figures are in [data-model.md](data-model.md) › Expected report; the contracts are in [contracts/](contracts/).

## 1. The earlier specs are untouched

```sh
cargo test -p rr_dr60_harness --test golden --test golden_agc --test golden_vas --test golden_codec --test golden_output
```

Expected: the five files byte-identical and their SHA guards unmoved (FR-002, SC-003). Run this after every phase.

## 2. Story 1: the bound at the route output

```sh
cargo test -p rr_dr60 output::                      # the core: the identity on a crossing vector, ±∞ / NaN, the tap gating
cargo test -p rr_dr60_harness --test us1_clamp -- --nocapture
cargo test -p rr_dr60_harness --test clamp_matrix -- --nocapture
cargo test -p rr_dr60_harness --test clamp_edge_cases
```

Expected: every identity row passes at the seven rates on both routes; `clamp_matrix` prints the option-off peaks and counts of data-model.md's table (the four overshooting stimuli above 1.0 at every rate, the reference-position tone never); the per-stage taps bit-identical with the option on and off; the bypassed chain bounded at the route output and not at `AfterPlayback`.

## 3. Story 2: the whole engine

```sh
cargo test -p rr_dr60_harness --test us1_clamp equivalence -- --nocapture
cargo test -p rr_dr60_harness --test determinism --test alloc_free clamp
cargo test -p rr_dr60_harness --release --test timing clamp_cost -- --ignored --nocapture   # solo; quote from this run
```

Expected: record-then-play equals the one-pass with the option on (both routes, two positions, seven rates); the five clamp configurations partition-invariant and allocation-free; `clamp_cost` prints the medians of five alternating on/off runs per route and the difference (≤ 2 ns per sample on the speaker route, SC-005); the latencies unchanged.

## 4. Story 3: what the engine says

```sh
cargo test -p rr_dr60_harness --test us1_manifest --test us2_cross_check -- --nocapture
scripts/check-traceability.sh
```

Expected: the manifest counts unchanged (5 not modeled, 10 assumed), no entry string changed, the canonical text's header reads 0.10.0 (re-blessed once: `RR_DR60_BLESS=manifest cargo test -p rr_dr60_harness --test us2_cross_check canonical`, a CHANGELOG line); the README output section and the 0.10.0 CHANGELOG entry pass the denylist; A-042 cites spec 010; the audit clean.

## 5. Story 4: the same from C

```sh
cargo test -p rr_dr60_ffi
cargo test -p rr_dr60_harness --test ffi_parity clamp
crates/rr_dr60_ffi/tests/c/run_smoke.sh
```

Expected: `size_of == 108`, the offsets 99 / 104 / 105, `struct_size = 104` refused; the C and Rust outputs bit-equal with the option on at the seven rates; the smoke loop's two clamped cases match the pinned hashes and every sample lies within [−1, 1]; the header drift check clean (`crates/rr_dr60_ffi/cbindgen.toml`).

## 6. Gates, timing and the bless (last)

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
RUSTFLAGS="-D warnings" cargo +1.85 build --workspace        # the MSRV gate
cargo llvm-cov --all-features --workspace --fail-under-lines 80
RR_DR60_BLESS=clamp cargo test -p rr_dr60_harness --test golden_clamp   # once, after the hardware review; pin the SHA
```

Expected: all green; coverage within 1 point of 0.9's figure (98.18 % on CI); the clamp golden blessed once (105 entries) and its guard pinned; `scripts/ios-device-golden.sh` and `ci.yml` list `golden_clamp`.
