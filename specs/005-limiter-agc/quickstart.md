# Quickstart: validating feature 005 (limiter AGC, ADC clamp, VAS threshold)

Every step is runnable from the repository root on the feature branch. Prerequisites as spec 004 (`~/.cargo/bin` on `PATH`; `cargo llvm-cov`; a C compiler for the smoke test; Python 3 for the reference model).

## 1. The reference model (research R-04)

```sh
python3 tools/agc-model/limiter_model.py
```

Expected (about 8 s; **T051 walk 2026-10-11: matches**, 8.5 s, at the −1 dBFS knee after the post-implementation review): the static curve with unity gain through −1 dBFS and the ceiling at −1.0 dBFS above it (below full scale: no steady clipping), the knee at about 0 dBFS; the self-consistent ceiling for three saturation currents (+0.2 / −0.8 / −2.0 dBFS); the discretized static rows at 6 and 48 kHz; a reference dead time of 2.17 ms at detector gain 300 and the +1 dBFS reference knee, the default attack setting mapping to a gain of about 166 at the default knee and realizing about 3.2 ms (3.3 ms on the 6 kHz grid), about 26 ms at 2 dB over the knee and 0.27 ms when armed, with no over-attenuation at any host rate; the armed dead time versus the gap (0.19 ms at 1 s to 2.3 ms at 20 s); the knee-aware mapping holding the dead time near 3 ms from −40 to +3 dBFS; the release to −1 dB in 1.36 s from 10 dB and scaling one-to-one with the setting; the golden onset stimulus's 2.42 ms / 1.475 s; the unity floor 0.171 V; the 3 dB polarity difference; the 5.5 dB bandwidth expectation; and the clipped-onset duty of about 79 % then 0 %.

## 2. Story 1: hear the limiter (FR-003 – FR-009)

```sh
cargo test -p rr_dr60 stages::limiter stages::clamp
cargo test -p rr_dr60_harness --test us1_agc --test agc_matrix -- --nocapture
```

Expected: bit-exact passthrough below the knee at all seven rates; the ceiling within ±0.5 dB from knee + 1.5 dB to +20 dBFS; the knee at 0 ± 2 dBFS; dead time 3 ± 1.5 ms (at least 3× longer at 2 dB over the knee), armed ≤ 0.5 ms, no over-attenuation; release 1.37 s ± 40 %; polarity 3 ± 0.5 dB; the 6 kHz bandwidth row at 16 kHz and above; the clamp rows; THD+N report rows. **T051 walk 2026-10-11 (re-run at the −1 dBFS knee):** `agc_matrix` prints 563 default rows at seven rates (dead time 3.33 ms at 6 kHz, 2.50 at 8 kHz, 3.18–3.25 above; knee +0.02 dBFS; release 1.36 s; polarity 2.95–3.00 dB; bandwidth drop 5.4–5.5 dB; the armed-versus-gap report rows) and the 290-row settings matrix at 6 and 48 kHz (the knee cases also run the dead-time and release rows; the ramp and armed rows, and the 2 dB-over ratio at knee +3 dBFS, are report rows there).

## 3. Story 2: the record chain still records speech (FR-012)

```sh
cargo test -p rr_dr60_harness --test chain --test vas_matrix --test us2_vas_settings -- --nocapture
```

Expected: the three chain scenarios pass at 6 and 48 kHz; the bisected thresholds read −34, −37, −40, −43, −46 dBFS ± 1 dB; the rewritten interplay report shows the identity below the knee.

## 4. Story 3: settings, tap, C parity (FR-001, FR-010, FR-011, FR-015)

```sh
cargo test -p rr_dr60 settings pipeline
cargo test -p rr_dr60_harness --test us2_agc_settings --test agc_edge_cases --test ffi_parity --test alloc_free
crates/rr_dr60_ffi/tests/c/run_smoke.sh
```

Expected: the settings matrix (knee −10 / +3 dBFS, attack 1 / 10 / 50 ms, release 0.2 / 4 / 10 s) within the FR-011 tolerances (the matrix itself runs in `agc_matrix`, step 2; `us2_agc_settings` AS1/AS2 run its knee −10 dBFS and release 4 s cases); the "after AGC" tap equals the limiter output with latency 0 (AS3 pins the golden onset stimulus's dead time 2.417 ms and release 1.475 s at 48 kHz to the reference model); out-of-range values named; C bit-identical at seven rates for every matrix case; the smoke test's 80-byte struct, defaults, clamp and knee checks (`smoke: OK (rr_dr60 0.5.0, latency 483 samples at 48000 Hz)`).

## 5. Golden files, determinism, mutation (FR-014, FR-016, SC-004, SC-007)

```sh
cargo test -p rr_dr60_harness --test golden --test golden_agc --test golden_vas --test determinism --test mutation
```

Expected: `golden-v1.json` byte-identical (its SHA guard unchanged); the AGC and VAS files match after the two re-blesses (the limiter, then the knee move); 100 partitions at 48 kHz; the three mutants (knee **−3** dB, since +3 would leave FR-010's range; release ×2; VAS threshold +3 dB) caught by a tolerance check and a golden check each. **T051 walk 2026-10-11: all green** (golden 112 / 42 / 63 entries).

## 6. Records (FR-013, FR-019; SC-001, SC-008)

```sh
scripts/check-traceability.sh
grep -rn "Hilbert\|AGC_HILBERT\|design_hilbert" crates tools CLAUDE.md README.md   # only the harness's FFT Hilbert-transform helper (analysis.rs) and the "retired" note in agc_checks.rs
```

Then read the CHANGELOG's 0.5.0 entry and the README's AGC section: a reader learns what the limiter does to quiet and loud input, the input-level anchor, how to bypass it or move its knee, and that it is modeled on the circuit, not measured.

## 7. Release-mode matrices and gates

```sh
cargo test -p rr_dr60_harness --release --test agc_matrix --test agc_edge_cases --test vas_matrix --test vas_edge_cases -- --ignored
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo llvm-cov --all-features --workspace --fail-under-lines 80
cargo test -p rr_dr60_harness --release --test timing -- --ignored --nocapture
```

Expected: all green; ≥ 20× real time (expected faster than 0.4's 368×); coverage ≥ 80 % and not down by more than 1 point. **T046/T051 walk 2026-10-11:** release matrices 55 s; timing 361× / 359× / 358× (default / default_agc / default_vas, 59.6 / 58.1 / 58.1 ns per sample); coverage 98.99 % lines (004: 98.74 %), 6.4 min locally.
