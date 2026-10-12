# Quickstart: validating feature 006 (record and playback paths, modeled memory)

Every step is runnable from the repository root on the feature branch. Prerequisites as spec 005 (`~/.cargo/bin` on `PATH`; `cargo llvm-cov`; a C compiler for the smoke test).

## 1. The narrowing and the single re-bless (FR-009; research R-02, R-09)

```sh
cargo test -p rr_dr60_harness --test narrowing_delta -- --nocapture
cargo test -p rr_dr60_harness --test golden --test golden_agc --test golden_vas
```

Expected: the delta report (share of changed samples per file, maximum |Δ| ≤ 2 output LSBs; `bypass_all` and `agc_only` entries unchanged); after the re-bless commit the three golden tests pass with `library_version` 0.6.0 and the entry counts 112 / 42 / 63. The CHANGELOG's 0.6.0 entry quotes the report's figures.

## 2. Story 1: record, stop, play (FR-001 – FR-009)

```sh
cargo test -p rr_dr60 paths transport
cargo test -p rr_dr60_harness --test us1_record_play -- --nocapture
```

Expected (US1 alone, in one engine, no import): `golden_equivalence` reproduces 196 of 217 entries at the seven rates (the count printed); `live_equivalence` bit-identical prefixes with playback lengths exactly ⌈K·m/l⌉; `route_identity` and the two-item reset case; the state table (idle → recording → idle → playing → idle, `speaker_muted` only while recording, the in-progress refusals); the 48 → 44.1 kHz tone within ±0.1 dB / ±0.1 Hz (expected < 0.02 dB); partitions 4 801 / 997 and 1 / 1 identical; the drain's last 1 ms below −80 dBFS and `drain_samples()` = 121 / 242 / 483 / 1 328 / 1 444 / 2 653 / 2 888; the latency sums 1 / 81 / 162 / 444 / 483 / 887 / 966.

## 3. Story 2: the memory (FR-010 – FR-013)

```sh
cargo test -p rr_dr60 memory
cargo test -p rr_dr60_harness --test us2_memory -- --nocapture
```

Expected: at a 10 s capacity recording stops at exactly 60 000 stored samples with `stopped = MemoryFull` and the partial recording kept; drop-mode pauses cost nothing, mute-mode silence counts; the 3rd recording fills the slots and a 4th start is `NoRecordingSlots`; erase renumbers (the former 3 becomes 2) and returns the time; erase all empties; a start with nothing fed then stop is `DiscardedEmpty`; `footprint_bytes()` at the defaults is 86 401 600 (≤ 90 MB) and unchanged by any operation; a clone reports the same footprint.

## 4. Story 3: export and import (FR-014)

```sh
cargo test -p rr_dr60 form
cargo test -p rr_dr60_harness --test us3_export_import -- --nocapture
```

Expected: `export_len` = 36 + 4K; import into a fresh engine at each of the seven rates gives the same length and bit-identical playback; every member of the corruption set is refused with its stated reason (`UnknownForm`, `UnsupportedFormVersion`, `Damaged`, `DoesNotFit`) and `memory_status()` is unchanged before and after; the CRC-bit mutant fails the round trip.

## 5. Story 4: C parity and the smoke test (FR-020)

```sh
cargo test -p rr_dr60_ffi
cargo test -p rr_dr60_harness --test ffi_parity --test alloc_free
crates/rr_dr60_ffi/tests/c/run_smoke.sh
```

Expected: `RrDr60Settings` is 88 bytes with `memory_capacity_s` at 76 and `memory_max_recordings` at 80; an 80-byte `struct_size` is `STRUCT_SIZE`; statuses 6–19 and field numbers 14–15; the C record/export/import/play loop bit-identical to Rust; no allocation in record, play, start, stop, route or the figure reads; `smoke: OK (rr_dr60 0.6.0, latency 483 samples at 48000 Hz)`.

## 6. Edge cases, determinism, timing (FR-017, FR-018, SC-006)

```sh
cargo test -p rr_dr60_harness --test split_edge_cases --test split_matrix --test determinism --test mutation -- --nocapture
cargo test -p rr_dr60_harness --release --test timing --test split_matrix -- --ignored --nocapture
```

Expected: the edge cases of the spec (memory fills inside a block and inside the flush, stop while paused, erase during playback stops it, route change mid-playback, reconfigure refusals, reset keeps the memory, bad numbers, tiny capacity rejected, empty import rejected); record-then-play identical across 100 partitions; the two split mutants caught; record + play within 25 % of the one-pass per-sample cost (ratio printed; expected ≤ 1.05); the 7 × 7 cross-rate grid.

## 7. Records and gates (FR-021; SC-001)

```sh
scripts/check-traceability.sh
grep -n "69.9\|66.7" docs/hardware/assumptions.md     # A-004 and A-038 agree on 2²⁴ bits
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo llvm-cov --all-features --workspace --fail-under-lines 80
```

Then read the CHANGELOG's 0.6.0 entry and the README's "Recording and playback" section: a reader learns how to record, stop, play, read the state and the memory's figures, erase, export and import, which calls are real-time safe, the 86 MB reservation, that the stored form is the engine's 32-bit samples until the codec, and that the route is a label until stages 11–13.
