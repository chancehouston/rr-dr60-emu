# Quickstart: validating feature 005 (limiter AGC, ADC clamp, VAS threshold)

Every step is runnable from the repository root on the feature branch. Prerequisites as spec 004 (`~/.cargo/bin` on `PATH`; `cargo llvm-cov`; a C compiler for the smoke test; Python 3 for the reference model).

## 1. The reference model (research R-04)

```sh
python3 tools/agc-model/limiter_model.py
```

Expected: the static curve with unity gain through +1 dBFS and the ceiling at about +1 dBFS above it, the knee at about +2 dBFS, dead times of about 2.4 ms (+10 dB, detector gain 300) and 50 ms (+2 dB), an armed dead time near 0, the release to −1 dB in about 1.37 s from 10 dB, and the 3 dB polarity difference.

## 2. Story 1: hear the limiter (FR-003 – FR-009)

```sh
cargo test -p rr_dr60 stages::limiter stages::clamp
cargo test -p rr_dr60_harness --test us1_agc --test agc_matrix -- --nocapture
```

Expected: bit-exact passthrough below the knee at all seven rates; the ceiling within ±0.5 dB from knee + 1.5 dB to +20 dBFS; the knee at +2 ± 2 dBFS; dead time 3 ± 1.5 ms, armed ≤ 0.5 ms, no over-attenuation; release 1.3 s ± 40 %; polarity 3 ± 0.5 dB; the 6 kHz bandwidth row at 16 kHz and above; the clamp rows; THD+N report rows.

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

Expected: the settings matrix (knee −10 / +4 dBFS, attack 0.5 / 10 / 50 ms, release 0.2 / 4 / 10 s) within the FR-011 tolerances; the "after AGC" tap equals the limiter output with latency 0; out-of-range values named; C bit-identical at seven rates; the smoke test's 80-byte struct, defaults, clamp and knee checks.

## 5. Golden files, determinism, mutation (FR-014, FR-016, SC-004, SC-007)

```sh
cargo test -p rr_dr60_harness --test golden --test golden_agc --test golden_vas --test determinism --test mutation
```

Expected: `golden-v1.json` byte-identical (its SHA guard unchanged); the AGC and VAS files match after the single re-bless; 100 partitions at 48 kHz; the three mutants (knee +3 dB, release ×2, VAS threshold +3 dB) caught by a tolerance check and a golden check each.

## 6. Records (FR-013, FR-019; SC-001, SC-008)

```sh
scripts/check-traceability.sh
grep -rn "Hilbert\|AGC_HILBERT\|design_hilbert" crates tools CLAUDE.md README.md   # only historical mentions (CHANGELOG, specs 002/004)
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

Expected: all green; ≥ 20× real time (expected faster than 0.4's 368×); coverage ≥ 80 % and not down by more than 1 point.
