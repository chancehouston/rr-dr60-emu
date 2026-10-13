# Quickstart: validating feature 008 (the speech codec)

Every step is runnable from the repository root on the feature branch. Prerequisites as spec 006 (`~/.cargo/bin` on `PATH`; `cargo llvm-cov`; a C compiler for the smoke test). The two Python tools need `uv`; the calibration tool also needs `ffmpeg` on `PATH` (`brew install ffmpeg`).

## 1. The design constants (research R-02; contracts/codec-design.md)

```sh
uv run tools/celp-design/design_celp.py --check
```

Expected: the budget table summing to 80 bits, the frame/subframe/look-ahead/delay figures (120 / 60 / 30 / 150), the per-rate latency table (one-pass 1683 host samples at 48 kHz), the pitch code map (127 codes, 127 unused), the LSP step table and the gain tables; `--write` regenerates `crates/rr_dr60/src/codec/tables.rs` byte-identically (checked by `git diff --exit-code`).

## 2. Story 1: record through the codec and hear it back (FR-001 – FR-009, FR-015, FR-016)

```sh
cargo test -p rr_dr60 codec
cargo test -p rr_dr60_harness --test us1_codec -- --nocapture
cargo test -p rr_dr60_harness --test codec_matrix -- --nocapture
```

Expected: the core's codec tests pass (operators, LPC/LSP round trip, every quantizer table, the 17-bit codebook decode, the packer, causality, silence); `us1_codec` reports at every rate the exact bit rate (4000), the delay lag 150 ± 2 in the codec-only configuration, the look-ahead probe = 30, RMS within ±1.5 dB and third-octave levels within ±3 dB at −10/−20/−40 dBFS, LSD and segmental SNR (≥ 4 dB) with their figures, the frame read-outs, and record-then-play bit-identical to the one-pass with the stop's silence appended; `codec_matrix` prints the seven-rate table.

## 3. Story 2: frames in the memory, the stored form (FR-010 – FR-014)

```sh
cargo test -p rr_dr60 memory form transport
cargo test -p rr_dr60_harness --test us2_frames_memory -- --nocapture
```

Expected: the footprint 1 801 600 B at the default (≤ 2.5 MB); the self-stop on a frame boundary at a 2 s capacity with remaining exactly 0; a mid-frame stop padded to a whole frame with `silence_fed_samples` reported; the round trip bit-exact at seven rates; the corruption set refused (version 1 → unsupported form version; wrong model version → 22; the other encoding → 21; lag code 127 → damaged); the reconfigure refusal with recordings present.

## 4. Story 3: the manifest and the registers (FR-020)

```sh
cargo test -p rr_dr60_harness --test us1_manifest --test us2_cross_check -- --nocapture
scripts/check-traceability.sh
```

Expected: stages 7 and 9 assumed / low / 0.8.0 with their coverage and controls notes, stage 8 at 0.8.0, counts 8 / 7; the cross-check passes with A-004 adopted, A-040 and A-041 cited, S-008 – S-011 cited; the canonical text and README table re-blessed once (`RR_DR60_BLESS=manifest …`, `RR_DR60_BLESS=readme … -- --test-threads=1`).

## 5. Story 4: the same from C (FR-019)

```sh
cargo test -p rr_dr60_ffi
crates/rr_dr60_ffi/tests/c/run_smoke.sh
cargo test -p rr_dr60_harness --test ffi_parity
```

Expected: `RrDr60Settings` 96 bytes, the report structs' new sizes, statuses 21 and 22; the smoke loop's frame bytes and playback `memcmp` equal; the bypassed handle refuses the frames with 21.

## 6. Calibration (research R-09; clarify Q5) — plan time, not CI

```sh
cargo run -p rr_dr60_harness --example codec_stimuli -- --rate 8000 --out tools/celp-calibrate/out
uv run tools/celp-calibrate/calibrate.py tools/celp-calibrate/out      # needs ffmpeg
cargo test -p rr_dr60_harness --release --test codec_matrix calibration -- --ignored --nocapture
```

Expected: the harness prints the G.723.1 (5.3 kbit/s) figures for the same stimuli through the same metrics; the tolerances in `codec_checks.rs` equal those figures plus the margins written in `specs/008-celp-codec/calibration.md`. Without ffmpeg the tool exits non-zero with the install hint and the committed `calibration.md` stands.

## 7. Goldens, determinism, timing and the gate

```sh
cargo test -p rr_dr60_harness --test golden --test golden_agc --test golden_vas    # unchanged SHA guards
RR_DR60_BLESS=codec cargo test -p rr_dr60_harness --test golden_codec             # once, at the end
cargo test -p rr_dr60_harness --test determinism --test alloc_free --test mutation
cargo test -p rr_dr60_harness --release --test timing -- --ignored --nocapture
cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features
cargo llvm-cov --all-features --workspace --fail-under-lines 80
RUSTFLAGS="-D warnings" CARGO_TARGET_DIR=target/msrv cargo +1.85 build --workspace --all-features --all-targets
```

Expected: the three guards unmoved; 84 codec entries blessed once then reproduced; frame bytes identical across block partitions and in the C loop; the one-pass with the codec ≤ 8 µs per device sample, the worst 64-sample block at 48 kHz < 1.33 ms with the margin printed; the gate green.
