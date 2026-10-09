# Quickstart: validating the AGC (feature 002)

This guide shows how to check that feature 002 works, end to end. It uses the measurement harness and a few lines of host code. For API details, see [contracts/](contracts/). For the behavior and tolerances, see [spec.md](spec.md).

## Prerequisites

As for 001: Rust stable with `~/.cargo/bin` on `PATH`, `cbindgen` 0.29.4, `cargo-llvm-cov`, and `uv` (only to regenerate the Hilbert coefficients).

## 1. The project gate

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo llvm-cov --all-features --workspace --fail-under-lines 80
```

Expected: everything passes.

## 2. Spec 001 is untouched (FR-002, FR-017, FR-018, SC-003)

```sh
git diff --exit-code main -- crates/rr_dr60_harness/golden/golden-v1.json
cargo test -p rr_dr60_harness --test golden
```

Expected: no diff, and every 001 golden entry matches with the AGC bypassed. The `golden_v1_unchanged` test (in `golden_agc`) also checks the file's SHA-256 on every CI target.

## 3. The AGC measurement report (US1, US3, SC-002)

```sh
cargo test -p rr_dr60_harness --test agc_matrix -- --nocapture
```

Expected: one row per check × rate, all `PASS`, each citing `002/FR-0xx` and A-017 to A-020. Values to spot-check, at defaults and 48 kHz:

| Check | Expected |
|---|---|
| 1 kHz at −40 / −10 / 0 dBFS | −13 / −10 / −9 dBFS ± 1 dB |
| 1 kHz at −60 dBFS | −20 dBFS ± 1 dB |
| Attack / release (−40 ↔ −10 dBFS step) | 10 ms ± 2 ms / 1.0 s ± 0.2 s |
| Release at 25 % of its time | 35–65 % recovered |
| Noise in the last 1 s of a pause | −30 dBFS ± 2 dB |
| Added latency | 0 samples |

## 4. AGC golden file and mutation (FR-017, SC-008)

```sh
cargo test -p rr_dr60_harness --test golden_agc
cargo test -p rr_dr60_harness --test mutation
cargo test -p rr_dr60_harness --test agc_edge_cases
cargo test -p rr_dr60_harness --release --test agc_matrix -- --ignored   # full settings × 6 rates
```

Expected: 36 AGC golden entries match. The mutation test shows that +50 % release time, and separately +3 dB target, each fail both a tolerance check and a golden check.

## 5. Real-time safety and determinism (FR-013, SC-004)

```sh
cargo test -p rr_dr60_harness --test alloc_free --test determinism
cargo test -p rr_dr60 --features op-count
cargo test -p rr_dr60_harness --release --test timing -- --ignored
```

Expected: zero allocations over 1000+ blocks with the AGC on (at defaults and at extreme settings), bit-identical output across 100 random block partitions, a bounded per-sample operation count, and a flat ns/sample.

## 6. C interface (FR-014, US2 AS5–AS6)

```sh
cargo test -p rr_dr60_harness --test ffi_parity
```

Expected:
- C and Rust output are bit-identical for default, `agc_only` and extreme settings.
- `agc_attack_ms = 0` returns `RR_DR60_STATUS_INVALID_SETTING` from `rr_dr60_create`, and `rr_dr60_settings_validate` returns the same status and names `RR_DR60_SETTING_FIELD_AGC_ATTACK_MS`.
- A struct with `struct_size = 24` (the 0.1 layout) is rejected with `RR_DR60_STATUS_INVALID_ARGUMENT` (field `STRUCT_SIZE`).
- `RR_DR60_TAP_AFTER_AGC` with stages 4 and 10 on matches `agc_only` bit for bit.

## 7. Listen (optional, by hand)

Host code (Rust):

```rust
let mut s = rr_dr60::Settings::new(48_000);     // AGC on by default (A-020)
// s.agc.enabled = false;                       // bypass
// s.tap = rr_dr60::Tap::AfterAgc;              // AGC only
// s.agc.release_ms = 3000.0;                   // slower recovery
let mut p = rr_dr60::Pipeline::new(s)?;
```

Feed it speech that has pauses. Expected: pauses fill with rising background hiss over about 1 s, and the level dips after loud words, then recovers. A loud first word may briefly exceed full scale. Limit or clip the output before converting it to 16-bit.
