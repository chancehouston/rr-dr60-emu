# Quickstart: validating feature 008 (the speech codec)

Every step is runnable from the repository root on the feature branch. Prerequisites as spec 006 (`~/.cargo/bin` on `PATH`; `cargo llvm-cov`; a C compiler for the smoke test). The two Python tools need `uv`; the calibration tool also needs the ITU-T G.723.1 reference software (a free download from the ITU, unpacked under `tools/celp-calibrate/ref/`; see `tools/celp-calibrate/README.md`) and a C compiler; `ffmpeg` is optional for a decode cross-check. The order below is the task order: the calibration (step 2) comes before any codec code.

## 1. The design constants (research R-02; contracts/codec-design.md)

```sh
uv run tools/celp-design/design_celp.py --check
uv run tools/celp-design/design_celp.py --write && git diff --exit-code crates/rr_dr60/src/codec/tables.rs
```

Expected: the budget table summing to 80 bits, the frame/subframe/look-ahead/delay figures (120 / 60 / 60 / 180), the per-rate latency table (one-pass 1923 host samples at 48 kHz, 1767 at 44.1 kHz), the pitch code map (127 codes, 127 unused), the companded LSP level tables with the spectral-distortion estimate, the gain tables with the required-versus-available energy at both ends of the level anchor; `--write` regenerates `crates/rr_dr60/src/codec/tables.rs` byte-identically (the `git diff` exits 0).

**T051 walk (2026-10-14):** observed `bit budget: 80 of 80 → OK` and the figure tables; `--write` left `tables.rs` byte-identical (`git diff --exit-code` 0).

## 2. Calibration, before any codec code (research R-09; clarify Q5) — plan time, not CI

```sh
cargo run -p rr_dr60_harness --example codec_stimuli -- --rate 8000 --out tools/celp-calibrate/out
uv run tools/celp-calibrate/calibrate.py tools/celp-calibrate/out      # needs tools/celp-calibrate/ref/ and a C compiler
cargo test -p rr_dr60_harness --release --test codec_matrix calibration -- --ignored --nocapture
cargo test -p rr_dr60_harness --test codec_matrix stimulus_digests
```

Expected: the tool builds `lbccodec`, runs it at 5.3 kbit/s on each stimulus and writes the decoded references (cross-checked with ffmpeg's decoder when present); the harness prints the G.723.1 figures for the same stimuli through the same metrics; `specs/008-celp-codec/calibration.md` records them, the margins, and the SHA-256 of each stimulus file, which the non-ignored `stimulus_digests` test asserts. Without the reference software the tool exits non-zero with the download hint and the committed `calibration.md` stands.

**T051 walk (2026-10-14):** observed the six stimuli written with the digests of `calibration.md`; the tool built `lbccodec` from the reference software (14 files) and wrote the six decoded references (`ffmpeg` not on `PATH`, the decode cross-check skipped); the reader printed the reference figures of `calibration.md` unchanged (lag 60, LSD 1.86 – 2.37 dB, segmental SNR 6.94 – 9.16 dB); `stimulus_digests` green.

## 3. Story 1: record through the codec and hear it back (FR-001 – FR-009, FR-015, FR-016)

```sh
cargo test -p rr_dr60 codec
cargo test -p rr_dr60_harness --test us1_codec -- --nocapture
cargo test -p rr_dr60_harness --test codec_matrix -- --nocapture
```

Expected: the core's codec tests pass (operators, LPC/LSP round trip, every quantizer table, the 17-bit codebook decode, the packer, 10 000 random frames decoding to finite output, the two fallbacks, the push-count look-ahead, silence, the −10 dBFS gain-range check); `us1_codec` reports at every rate the exact bit rate (4000), the delay lag 180 ± 2 in the codec-only configuration, the look-ahead probe (frame unchanged for every offset ≥ 60; the largest earlier offset that changes it reported), RMS within ±1.5 dB and third-octave levels within ±3 dB at −10/−20/−40 dBFS, LSD and segmental SNR (≥ 4 dB) with their figures, the frame read-outs, the VAS events and the after-VAS tap unchanged by the codec, the latency bound, and record-then-play bit-identical to the one-pass with the stop's silence appended and the output skipped by the delay; `codec_matrix` prints the seven-rate table.

**T051 walk (2026-10-14):** observed 72 core codec tests green; `us1_codec` 10 tests green at the seven rates in 22 s (every row PASS: bit rate 4000, lag 180 ± 2, the probe's reported offset 30, the latencies 181 / 321 / 642 / 1767 / 1923 / 3533 / 3846, the one-pass latency with the codec 30.2 – 40.1 ms against the 50 ms + bypassed bound); `codec_matrix` 2 tests green in 57 s (3 release-only cases ignored).

## 4. Story 2: frames in the memory, the stored form (FR-010 – FR-014)

```sh
cargo test -p rr_dr60 -- memory form transport
cargo test -p rr_dr60_harness --test us2_frames_memory -- --nocapture
```

Expected: the footprint 1 801 600 B at the default (≤ 2.5 MB); the self-stop on a frame boundary at a 2 s capacity with remaining exactly 0; a mid-frame stop padded to a whole frame with `silence_fed_samples` reported; the round trip bit-exact at seven rates; the corruption set refused (version 1 → unsupported form version; wrong model version → 22; the other encoding → 21; lag code 127 → damaged); the reconfigure refusal with recordings present.

**T051 walk (2026-10-14):** observed 64 core tests green; `us2_frames_memory` 6 tests green in 2.7 s with every row PASS at the seven rates.

## 5. Story 3: the manifest and the registers (FR-020)

```sh
cargo test -p rr_dr60_harness --test us1_manifest --test us2_cross_check -- --nocapture
scripts/check-traceability.sh
```

Expected: stages 7 and 9 assumed / low / 0.8.0 with their coverage and controls notes, stage 8 at 0.8.0, counts 8 / 7; the cross-check passes with A-004 adopted, A-040 and A-041 cited, S-008 – S-011 cited; the README's codec section and the CHANGELOG's 0.8.0 entry pass the denylist scan. The canonical text and the README table are re-blessed with the strings (twice on the branch), in this order:

**T051 walk (2026-10-14):** observed `us1_manifest` 11 and `us2_cross_check` 23 tests green (the denylist scan now covers the three `0.8.0` regions of the CHANGELOG, T045); `check-traceability: OK`. The re-bless commands were not re-run (no string changed in Phase 6).

```sh
RR_DR60_BLESS=manifest cargo test -p rr_dr60_harness --test us2_cross_check canonical
RR_DR60_BLESS=readme cargo test -p rr_dr60_harness --test us2_cross_check readme -- --test-threads=1
```

## 6. Story 4: the same from C (FR-019)

```sh
cargo test -p rr_dr60_ffi
crates/rr_dr60_ffi/tests/c/run_smoke.sh
cargo test -p rr_dr60_harness --test ffi_parity
```

Expected: `RrDr60Settings` 96 bytes, the report structs' new sizes, statuses 21 and 22; the smoke loop's frame bytes and playback `memcmp` equal; the bypassed handle refuses the frames with 21.

**T051 walk (2026-10-14):** observed 20 FFI unit tests green; `smoke: OK (rr_dr60 0.8.0, latency 1923 samples at 48000 Hz, 50 frames/s)`; `ffi_parity` 20 tests green in 2.2 s.

## 7. Goldens, determinism, timing and the gate (the codec bless is the last task, after the post-implementation hardware review)

```sh
cargo test -p rr_dr60_harness --test golden --test golden_agc --test golden_vas    # unchanged SHA guards
RR_DR60_BLESS=codec cargo test -p rr_dr60_harness --test golden_codec             # once, at the end
cargo test -p rr_dr60_harness --test determinism --test alloc_free --test mutation
cargo test -p rr_dr60_harness --release --test timing -- --ignored --nocapture
cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features
cargo llvm-cov --all-features --workspace --fail-under-lines 80
RUSTFLAGS="-D warnings" CARGO_TARGET_DIR=target/msrv cargo +1.85 build --workspace --all-features --all-targets
```

Expected: the three guards unmoved; 84 codec entries blessed once then reproduced; frame bytes identical across block partitions, in the C loop and in the release profile (the timing job checks one digest); the one-pass with the codec ≤ 8 µs per device sample, the worst 64-sample block at 48 kHz < 1.33 ms with the margin printed; the gate green.

**T051 walk (2026-10-14):** observed the three guards unmoved (`golden` 1, `golden_agc` 3, `golden_vas` 2 tests green); `determinism` 1, `alloc_free` 9, `mutation` 13 tests green; the release timing run (solo): 0.78 µs per device sample at 6 kHz and 1.23 µs at 48 kHz (budget 8), the worst 64-sample block 144 µs of 1333 (margin 9×), record + play 1.023× the one-pass, the `voiced_m20` frame-byte digest identical in debug and release (`c47e760b…1405`, 101 frames). The codec bless, the gate, coverage and the MSRV build are recorded under T052 / T056 in tasks.md.
