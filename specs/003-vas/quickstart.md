# Quickstart: validate feature 003 (VAS)

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md)

Runnable checks that prove the VAS works end to end. API details are in [contracts/](contracts/), and the state machine is in [data-model.md](data-model.md).

## Prerequisites

- The toolchain from `CLAUDE.md` (Rust stable, `~/.cargo/bin` on `PATH`, `cargo-llvm-cov`, `cbindgen` 0.29.4).
- Branch `003-vas`.

## 1. Specs 001 and 002 are intact (FR-002, FR-020)

```sh
cargo test -p rr_dr60_harness --test golden --test golden_agc
```

Expected: both pass. `golden_v1_unchanged` and `golden_agc_v1_unchanged` confirm that the committed 001 and 002 golden files are byte-identical to `main`.

## 2. Story 1: hear voice-activated recording (FR-004 – FR-010)

```sh
cargo test -p rr_dr60 stages::vas
cargo test -p rr_dr60_harness --test us1_vas -- --nocapture
```

Expected: pass. The output lists, at 48 kHz:
- a tone at the threshold + 3 dB is kept in full;
- for burst 1 s, gap 5 s, burst 1 s, the output length is 2 s + 1.0 s − 20 ms (±1 ms), with one splice;
- a gap under 1 s is kept;
- silence gives 1 s of zeros, then nothing.

## 3. Story 2: tune, bypass, tap, mute; C parity (FR-012 – FR-015)

```sh
cargo test -p rr_dr60_harness --test us2_vas_settings --test ffi_parity
crates/rr_dr60_ffi/tests/c/run_smoke.sh
```

Expected: pass, and the smoke test prints `smoke: OK`. Mute mode keeps the length and reports the same spans that drop mode removes. Sensitivity levels 1 and 5 move the threshold by +6 and −6 dB. An out-of-range sensitivity is named.

## 4. Story 3: the full harness (SC-002, SC-003, SC-007)

```sh
cargo test -p rr_dr60_harness --test vas_matrix --test vas_edge_cases --test golden_vas --test mutation --test determinism --test alloc_free -- --nocapture
cargo test -p rr_dr60_harness --release --test vas_matrix --test vas_edge_cases -- --ignored
```

Expected: every VAS check passes at all six rates and names its `003/FR-0xx` and A-/S- IDs. The mutation tests catch a 20 % hang change and a 3 dB threshold change. The second command runs the full settings matrix in release mode within the R-12 budget (≤ 60 s).

## 5. Default-pipeline interplay (US3 AS3)

```sh
cargo test -p rr_dr60_harness --test vas_matrix interplay -- --nocapture
```

Expected: a report, with no pass/fail, of how much of each −70 dBFS noise gap the default pipeline (AGC + VAS) keeps.

## 6. Gates

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo llvm-cov --all-features --workspace --fail-under-lines 80
scripts/check-traceability.sh
```

## 7. Hear it (manual demo)

Process any speech WAV with long pauses through the default pipeline, writing `output[..produced]` for each block. The pauses longer than 1 s shrink to 1 s, and the start of each phrase after a pause loses about 20 ms. The harness has no WAV I/O in the core (Principle I), so use any host that calls the library, or `process_in_place` in a small test binary.
