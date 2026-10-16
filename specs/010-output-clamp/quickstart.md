# Quickstart: validating the output clamp (feature 010)

Prerequisites: the toolchain of CLAUDE.md (`export PATH="$HOME/.cargo/bin:$PATH"`), branch `010-output-clamp`. Every command runs from the repository root. The expected figures are in [data-model.md](data-model.md) › Expected report; the contracts are in [contracts/](contracts/).

## 1. The earlier specs are untouched

```sh
cargo test -p rr_dr60_harness --test golden --test golden_agc --test golden_vas --test golden_codec --test golden_output
```

Expected: the five files byte-identical and their SHA guards unmoved (FR-002, SC-003). Run this after every phase.

## 2. Story 1: the bound at the route output

```sh
cargo test -p rr_dr60 output::                      # the core: the identity on a crossing vector, ±∞ / NaN / −0.0, the tap gating, the player always bounded
cargo test -p rr_dr60_harness --test us1_output_clamp identity -- --nocapture
cargo test -p rr_dr60_harness --test us1_output_clamp taps
cargo test -p rr_dr60_harness --test output_clamp_matrix -- --nocapture
cargo test -p rr_dr60_harness --test output_clamp_edge_cases
```

Expected: every identity row passes at the seven rates on both routes; `output_clamp_matrix` prints the option-off peaks, counts and "% bounded" of data-model.md's tables (1 s stimuli), the 18 stimulus × configuration cells, the quiet-material rows at the reference position (zero counts), and the speaker route's L1 bound per rate recomputed from the coefficients (1.75 – 2.29, within ±0.02; every speaker-route peak ≤ it); the per-stage taps bit-identical with the option on and off on all three stimuli, every earlier tap likewise; the bypassed chain bounded at the route output and not at `AfterPlayback`.

## 3. Story 2: the whole engine

```sh
cargo test -p rr_dr60_harness --test us1_output_clamp equivalence -- --nocapture
cargo test -p rr_dr60_harness --test us1_output_clamp change_timing
cargo test -p rr_dr60_harness --test us1_output_clamp stored_form
cargo test -p rr_dr60_harness --test us1_output_clamp latencies
cargo test -p rr_dr60_harness --test determinism output_clamp
cargo test -p rr_dr60_harness --test alloc_free output_clamp
cargo test -p rr_dr60_harness --test mutation output_clamp
cargo test -p rr_dr60_harness --release --test timing output_clamp_cost -- --ignored --nocapture   # solo; quote from this run
```

Expected: record-then-play equals the one-pass with the option on (both routes, two positions, seven rates; at position 1 the option-off run exceeds the bound, so the equivalence exercises it); the change-timing identity holds at every change point; the export bytes identical with the option on or off and an option-off export plays back bounded through an option-on engine; the latencies and the drain table unchanged; the six configurations partition-invariant and allocation-free; the three mutants fail the identity; `output_clamp_cost` prints, per route, the fastest of nine alternating on/off runs, the bounded share and the difference in ns and % (≤ 3 % on the speaker route, SC-005 as amended; run the timing tests one at a time).

## 4. Story 3: what the engine says

```sh
cargo test -p rr_dr60_harness --test us1_manifest --test us2_cross_check -- --nocapture
scripts/check-traceability.sh
```

Expected: the manifest counts unchanged (5 not modeled, 10 assumed), no entry string changed, the canonical text's header reads 0.10.0 (re-blessed once in Phase 1 with the version bump: `RR_DR60_BLESS=manifest cargo test -p rr_dr60_harness --test us2_cross_check canonical`, a CHANGELOG line); `readme_output_clamp_statements` finds every required phrase in the README output section and A-042 and none of the forbidden ones there or in the three 0.10.0 CHANGELOG regions, and both pass the manifest denylist; A-042 cites spec 010; the audit clean.

## 5. Story 4: the same from C

```sh
cargo test -p rr_dr60_ffi
cargo test -p rr_dr60_harness --test ffi_parity output_clamp
cargo test -p rr_dr60_harness --test ffi_parity smoke_reference_hash_is_pinned
crates/rr_dr60_ffi/tests/c/run_smoke.sh
cbindgen --config crates/rr_dr60_ffi/cbindgen.toml --crate rr_dr60_ffi --output crates/rr_dr60_ffi/include/rr_dr60.h && git diff --exit-code crates/rr_dr60_ffi/include/rr_dr60.h
```

Expected: `size_of == 112`, `align_of == 8`, the offsets 99 / 104 / 105, `struct_size = 104` refused with `INVALID_ARGUMENT` (create) and `STRUCT_SIZE` (validate); the C and Rust outputs bit-equal with the option on at the seven rates for the six configurations; the two clamp hashes pinned; the smoke loop's two clamped cases match the pinned hashes, every sample lies within [−1, 1], the option-off square exceeds 1.0; the header drift check clean.

## 6. Gates, timing and the bless (last)

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
RUSTFLAGS="-D warnings" cargo +1.85 test --workspace --all-features --no-run     # the MSRV gate, as ci.yml runs it
cargo llvm-cov --all-features --workspace --fail-under-lines 80
RR_DR60_BLESS=output_clamp cargo test -p rr_dr60_harness --test golden_output_clamp   # once, after the hardware review; pin the SHA
```

Expected: all green; coverage within 1 point of 0.9's figure (98.18 % on CI); the clamp golden blessed once (126 entries, the bless-time twin and equivalence assertions green) and its guard pinned; `scripts/ios-device-golden.sh` lists `golden_output` and `golden_output_clamp`, and `ci.yml` lists `golden_output_clamp` on its three golden lines.
