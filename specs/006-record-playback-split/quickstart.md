# Quickstart: validating feature 006 (record and playback paths, modeled memory)

Every step is runnable from the repository root on the feature branch. Prerequisites as spec 005 (`~/.cargo/bin` on `PATH`; `cargo llvm-cov`; a C compiler for the smoke test).

## 1. The narrowing and the single re-bless (FR-009; research R-02, R-09)

```sh
cargo test -p rr_dr60_harness --test narrowing_delta -- --nocapture
cargo test -p rr_dr60_harness --test golden --test golden_agc --test golden_vas
```

**T055 walk (2026-10-12):** observed 100 / 112, 21 / 42 and 57 / 63 entries changed, 30.1 / 15.8 / 12.7 % of samples, max |Δ| 1.00 / 1.87 / 0.06 full-scale LSBs; exactly the 39 listed entries unchanged; the three golden tests pass with their SHA guards. Expected: the delta report (share of changed samples per file, maximum |Δ| ≤ 4 full-scale LSBs, measured 1.87; exactly the 39 entries the narrowing cannot move unchanged: the 21 `agc_only`, and at 6 kHz `bypass_all`, `playback_only`, `vas_only`, `vas_mute` and `record_only`; research R-02); after the re-bless commit the three golden tests pass with `library_version` 0.6.0 and the entry counts 112 / 42 / 63. The CHANGELOG's 0.6.0 entry quotes the report's figures.

## 2. Story 1: record, stop, play (FR-001 – FR-009)

```sh
cargo test -p rr_dr60 paths transport
cargo test -p rr_dr60_harness --test us1_record_play -- --nocapture
```

**T055 walk (2026-10-12):** observed `golden entries reproduced by record-then-play: 196 of 217`, every row PASS at the seven rates, the 48 → 44.1 kHz level 0.000 dB; 8 tests in 10.4 s. Expected (US1 alone, in one engine, no import): `golden_equivalence` reproduces 196 of 217 entries at the seven rates (the count printed); `live_equivalence` bit-identical prefixes with playback lengths exactly ⌈K·m/l⌉; `route_identity` and the two-item reset case; the state table (idle → recording → idle → playing → idle, `speaker_muted` only while recording, the in-progress refusals); the 48 → 44.1 kHz tone within ±0.1 dB / ±0.1 Hz (expected < 0.02 dB); partitions 4 801 / 997 and 1 / 1 identical; the drain's last 1 ms below −80 dBFS and `drain_samples()` = 121 / 242 / 483 / 1 328 / 1 444 / 2 653 / 2 888; the latency sums 1 / 81 / 162 / 444 / 483 / 887 / 966.

## 3. Story 2: the memory (FR-010 – FR-013)

```sh
cargo test -p rr_dr60 memory
cargo test -p rr_dr60_harness --test us2_memory -- --nocapture
```

**T055 walk (2026-10-12):** observed the self-stop exactly at the capacity at all seven rates, the default figures (0 / 0 / 21 600 000 / 99) and the footprint 86 401 600 B unchanged through the limit, erase, erase all, playback and clone; 7 tests pass, 2 release-only. Expected: at a 10 s capacity recording stops at exactly 60 000 stored samples with `stopped = MemoryFull` and the partial recording kept; drop-mode pauses cost nothing, mute-mode silence counts; the 3rd recording fills the slots and a 4th start is `NoRecordingSlots`; erase 3 of 5 renumbers the former 4 and 5 to 3 and 4 and returns the time; erase all empties; a start with nothing fed then stop is `DiscardedEmpty`; `footprint_bytes()` at the defaults is 86 401 600 (≤ 90 MB) and unchanged by any operation; a clone reports the same footprint.

## 4. Story 3: export and import (FR-014)

```sh
cargo test -p rr_dr60 form
cargo test -p rr_dr60_harness --test us3_export_import -- --nocapture
```

**T055 walk (2026-10-12):** observed 49 bit-identical imported playbacks (7 × 7), every corruption case refused with its stated reason and the memory unchanged, both mutants caught; 4 tests in 2 s. Expected: `export_len` = 36 + 4K; import into a fresh engine at each of the seven rates gives the same length and bit-identical playback; every member of the corruption set is refused with its stated reason (`UnknownForm`, `UnsupportedFormVersion`, `Damaged`, `DoesNotFit`) and `memory_status()` is unchanged before and after; the CRC-bit mutant fails the round trip.

## 5. Story 4: C parity and the smoke test (FR-020)

```sh
cargo test -p rr_dr60_ffi
cargo test -p rr_dr60_harness --test ffi_parity --test alloc_free
crates/rr_dr60_ffi/tests/c/run_smoke.sh
```

**T055 walk (2026-10-12):** observed 12 FFI unit tests, 20 parity tests and the allocation test pass; `smoke: OK (rr_dr60 0.6.0, latency 483 samples at 48000 Hz)` with the ±0.125 stimulus hash `0x81b812b9699dc614`. Expected: `RrDr60Settings` is 88 bytes with `memory_capacity_s` at 76 and `memory_max_recordings` at 80; an 80-byte `struct_size` is `STRUCT_SIZE`; statuses 6–19 and field numbers 14–15; the C record/export/import/play loop bit-identical to Rust; no allocation in record, play, start, stop, route or the figure reads; `smoke: OK (rr_dr60 0.6.0, latency 483 samples at 48000 Hz)`.

## 6. Edge cases, determinism, timing (FR-017, FR-018, SC-006)

```sh
cargo test -p rr_dr60_harness --test split_edge_cases --test split_matrix --test determinism --test mutation -- --nocapture
cargo test -p rr_dr60_harness --release --test timing --test split_matrix -- --ignored --nocapture
```

**T055 walk (2026-10-12):** observed 12 edge cases, the split matrix, 7 determinism and 10 mutation tests pass; the release run: ratio **1.024** (63.9 vs 62.4 ns/sample) and the 7 × 7 grid green (147 rows). Expected: the edge cases of the spec (memory fills inside a block and inside the flush, stop while paused, erase during playback stops it, route change mid-playback, reconfigure refusals, reset keeps the memory, bad numbers, tiny capacity rejected, empty import rejected); record-then-play identical across 100 partitions; the two split mutants caught; record + play within 25 % of the one-pass per-sample cost (ratio printed; expected ≤ 1.05); the 7 × 7 cross-rate grid.

## 7. Records and gates (FR-021; SC-001)

```sh
scripts/check-traceability.sh
grep -n "69.9\|66.7" docs/hardware/assumptions.md     # A-004 and A-038 agree on 2²⁴ bits
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo llvm-cov --all-features --workspace --fail-under-lines 80
```

**T055 walk (2026-10-12):** traceability OK; `69.9` twice and `66.7` once in the register (the explicit contrast "16 × 10⁶ would give 66.7 min"); fmt, clippy, the suite (37 binaries) and `cargo llvm-cov` at **98.58 %** of lines (99.12 % of functions; 0.5 was 98.99 %) all green; `cargo +1.85 test` green (T052). Then read the CHANGELOG's 0.6.0 entry and the README's "Recording and playback" section: a reader learns how to record, stop, play, read the state and the memory's figures, erase, export and import, which calls are real-time safe, the 86 MB reservation (virtual: pages are touched as recordings grow; commit charge on Windows), that the stored form is the engine's 32-bit samples until the codec, and that the route is a label until stages 11–13.
