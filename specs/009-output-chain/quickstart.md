# Quickstart: validating the output chain (feature 009)

Every step runs from the repository root with `~/.cargo/bin` on `PATH`. The figures quoted are the plan's; the harness prints the measured ones. Record an "observed" line under each step when walking it (the 006 and 008 pattern).

## 1. The design script agrees with the committed coefficients

```sh
uv run tools/output-chain/design_output_chain.py --check
uv run tools/output-chain/design_output_chain.py --write && git diff --exit-code crates/rr_dr60/src/stages/output_coeffs.rs
```

Expected: the constants table (V_CLIP 1.327 V, gain 22.67 dB, VOLUME_REFERENCE 0.0976, K_SPK 10.25, knee 0.85 with its distortion bracket), the per-rate corners' −3 dB points (32, 159, 110, 190 Hz), the speaker template's peak frequency, 1 kHz and 2.5 kHz readings and 1 kHz group delay (≈ 0.27 ms), the `N_OUT` table (500 … 7 993), every bound green; the diff empty.

## 2. The earlier specs are untouched

```sh
cargo test -p rr_dr60_harness --test golden --test golden_agc --test golden_vas --test golden_codec
```

Expected: the four SHA guards green (every pre-009 configuration runs the chain bypassed).

## 3. Story 1: the two routes

```sh
cargo test -p rr_dr60_harness --test us1_output -- --nocapture
```

Expected, per route and rate: the earphone composite response within ±0.5 dB of the two-corner template from 20 Hz to 2.5 kHz with the fitted corners within ±10 %; the amplifier path's 22.7 dB ± 0.5 at position 1 and its two corners (32 and 159 Hz) within ±10 %; the speaker template within ±0.5 dB from 100 Hz to 2.5 kHz with a peak gain of exactly 1.0 and a 1 kHz group delay ≤ 0.5 ms; at 44.1 kHz and above the clip figures (peak never above 1.0; 10 % distortion between +1 and +3 dB over; 15 – 30 % at +6 dB; ≤ 1 % at −6 dB; ≤ 1 dB compression at the ceiling); the volume law within 0.1 dB at five positions; the default-position readings (speaker −1.0 … 0 dB, earphone ≈ −20.2 dB ± 1.5); exact zeros at the after-stage-11 tap on the earphone route at position 0 and the route output below −120 dBFS within 100 ms.

## 4. Story 2: the whole chain

```sh
cargo test -p rr_dr60_harness --test us1_output chain -- --nocapture
cargo test -p rr_dr60_harness --test output_matrix -- --nocapture
cargo test -p rr_dr60_harness --test determinism --test alloc_free --test mutation
```

Expected: record-then-play equals the one-pass bit for bit with the chain on (both routes, two volumes, seven rates; the skipped prefix asserted exactly zero); the one-pass at `Tap::AfterPlayback` equals the 0.8 default output; route and volume changes take effect at the next block; the drain table `⌈N₁₀·m/l⌉ + 2D + N_OUT`; the seven-rate matrix report; the determinism, allocation and mutation cases green.

## 5. Story 3: what the engine says

```sh
cargo test -p rr_dr60_harness --test us1_manifest --test us2_cross_check -- --nocapture
scripts/check-traceability.sh
cargo run -q -p rr_dr60_harness --example manifest | sed -n '/^11 /,/^13 /p'
```

Expected: rows 11 – 13 "assumed" with medium / low / low-or-medium, model version 0.9.0, the counts 5 not modeled / 10 assumed; the registers' A-010, A-031, A-032, A-042 – A-044 and the corrected S-006 row consistent with the manifest; the traceability audit OK.

## 6. Story 4: the same from C

```sh
cargo test -p rr_dr60_ffi
cargo test -p rr_dr60_harness --test ffi_parity
crates/rr_dr60_ffi/tests/c/run_smoke.sh
```

Expected: `RrDr60Settings` 104 bytes with the three bools at 96 – 98 and `struct_size = 96` refused; taps 5 – 8; statuses 23 and 24; `rr_dr60_set_volume` refusing 1.5 and NaN; the smoke loop's four route/volume cases matching the Rust hashes.

## 7. Gates, timing and the bless (last)

```sh
cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features
cargo test -p rr_dr60_harness --release --test timing output_cost -- --ignored --nocapture
cargo llvm-cov --all-features --workspace --fail-under-lines 80
RR_DR60_BLESS=output cargo test -p rr_dr60_harness --test golden_output     # once, after the post-implementation hardware review
cargo test -p rr_dr60_harness --test golden_output
```

Expected: the gate green; the chain's cost ≤ 25 % over the codec-bypassed one-pass at 48 kHz for both routes, the latencies unchanged; coverage within 1 point of 98.27 %; the 112 entries blessed once then reproduced.
