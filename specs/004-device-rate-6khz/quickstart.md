# Quickstart: validate feature 004 (6 kHz device rate and codec filter templates)

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md)

Runnable checks that prove the rate and band change end to end. Design details are in [research.md](research.md), the templates in [data-model.md](data-model.md), and the API deltas in [contracts/](contracts/).

## Prerequisites

- The toolchain from `CLAUDE.md` (Rust stable, `~/.cargo/bin` on `PATH`, `cargo-llvm-cov`, `cbindgen` 0.29.4) and `uv`.
- Branch `004-device-rate-6khz`.

## 1. The filters meet their templates on paper (FR-005, FR-006, FR-008)

```sh
uv run tools/filter-design/design_voiceband.py --check
uv run tools/filter-design/design_hilbert.py --check
```

Expected: two margin tables (record and playback), every row PASS, the chosen Chebyshev-II stopband edges printed with the 2550 Hz loss within ±0.1 dB of 0.4 dB (record) and 0.56 dB (playback), the 1950 Hz group-delay distortion near 0.08 and 0.11 ms (limit 0.17), and the group delay at 2550 Hz under 2 ms (committed design: edges 2735 and 2725 Hz, 0.306 and 0.478 dB at 2550 Hz, group delay 1.43 and 1.58 ms at 2550 Hz; the rule takes the highest edge inside the ±0.1 dB window, so the loss lands below the typical). The loss at 2700 and 2900 Hz is printed as report-only rows. The Hilbert script reports PASS at 6 kHz; `git diff --stat crates/rr_dr60/src/stages/agc_hilbert_coeffs.rs` shows only header-comment lines changed (the bit patterns are identical).

## 2. Story 1: the stages at the device rate (FR-005 – FR-009)

```sh
cargo test -p rr_dr60 stages::voiceband stages::vas stages::agc
cargo test -p rr_dr60_harness --test us1_voiceband --test response_matrix -- --nocapture
```

Expected: the unit tests re-verify both templates from the committed coefficients, and the VAS and AGC unit tests pass with their 1 kHz tones phase-aligned to the 6 kHz grid (plan R-08). The response matrix prints, for all seven rates and each single-stage configuration, the loss at every frequency of the test set, the group-delay distortion rows, the minimum-phase check at 6 kHz, the anti-alias rows (≥ 30 dB; expect ≥ 60 dB) and the two report-only rows at 2700 and 2900 Hz. Every toleranced row passes.

## 3. Story 2: host rates and the boundary (FR-002 – FR-004)

```sh
cargo test -p rr_dr60 rate resample
cargo test -p rr_dr60_harness --test us2_bypass_tap --test edge_cases --test latency -- --nocapture
```

Expected: 6 kHz is the identity (bypassed output equals input; latency 0 with both stages bypassed); the boundary is flat within ±0.1 dB to 2550 Hz and rejects ≥ 60 dB from 3000 Hz at the six converting rates; a 7 kHz request is rejected naming the seven rates; the latency table shows every default under 20 ms (483 samples at 48 kHz; 1, 81, 162, 444, 483, 887 and 966 at the seven rates) and matching the measured 1 kHz group delay within ±1 sample.

## 4. Story 3: the AGC and VAS laws are unchanged (FR-011, FR-012)

```sh
cargo test -p rr_dr60_harness --test agc_matrix --test us1_agc --test us2_agc_settings --test agc_edge_cases
cargo test -p rr_dr60_harness --test vas_matrix --test us1_vas --test us2_vas_settings --test vas_edge_cases -- --nocapture
```

Expected: every AGC check passes at seven rates with the restated tones (225 … 2550 Hz) and noise band; every VAS check passes with the restated frequencies and the phase-aligned 1 kHz bisection, lengths exact at 6, 48 and 96 kHz. The `interplay_report` still prints the floor sweep.

## 5. Golden files, determinism, mutation (FR-013, SC-003, SC-004, SC-007)

```sh
cargo test -p rr_dr60_harness --test golden --test golden_agc --test golden_vas --test determinism --test mutation
```

Expected: the three re-blessed files match (112, 42 and 63 entries), the SHA guards accept the new 001 and AGC files, 100 partitions of each golden stimulus agree, and each of the three toward-band shifted-edge fixtures (record high-pass +100 Hz, record and playback low-pass stopband edges −100 Hz) fails both its response check and the golden check.

## 6. C API parity, including 6 kHz (FR-017)

```sh
cargo test -p rr_dr60_harness --test ffi_parity --test alloc_free
crates/rr_dr60_ffi/tests/c/run_smoke.sh
```

Expected: C and Rust bit-identical at all seven rates; zero allocations while processing at every rate; the smoke test's 6 kHz identity check and the 7 kHz rejection pass.

## 7. Records (FR-013, FR-014, FR-018; SC-001, SC-008)

```sh
scripts/check-traceability.sh
grep -rn '8000\|8_000\|8 kHz' crates tools README.md docs/hardware/signal-chain.md | grep -v 'superseded\|was 8\|8 kHz host' || echo "no stray 8 kHz references"
git diff main --stat -- docs/hardware specs/001-pipeline-skeleton/spec.md specs/002-agc/spec.md specs/003-vas/spec.md CHANGELOG.md README.md
```

Expected: traceability OK; no stray 8 kHz references outside historical text; the register shows A-001, A-002 and A-014 superseded, A-016 upgraded, A-027 corrected; sources.md has S-007; each earlier spec has its one-line pointer; the CHANGELOG entry lists every row of the restated-figures table in data-model.md and the new latency per rate.

## 8. Gates

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo llvm-cov --all-features --workspace --fail-under-lines 80
cargo test -p rr_dr60_harness --release --test timing -- --ignored --nocapture
```

Expected: all green; the timing test shows ≥ 20× real time at 48 kHz for `default_vas` (expect faster than 0.3: fewer biquads, fewer device samples, shorter polyphase branches).
