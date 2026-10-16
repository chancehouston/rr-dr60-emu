# Quickstart: validating the microphone capsule and amplifier (stages 1–2)

From the repository root with `~/.cargo/bin` on `PATH`. Expected figures are the design script's print. The numbers below are plan-time estimates.

## 1. The design script

```sh
uv run tools/mic-chain/design_mic_chain.py           # every figure: nodal model, corners, hiss, clipper, bound, margins
uv run tools/mic-chain/design_mic_chain.py --check   # the bounds; exit 0
uv run tools/mic-chain/design_mic_chain.py --write   # regenerates crates/rr_dr60/src/stages/mic_coeffs.rs (byte-identical)
uv run tools/mic-chain/design_mic_chain.py --spice   # optional: runs ngspice if installed, else says it skipped
```

Expected:
- **Input corner:** about 820 Hz (744–910 over the grid).
- **DC self-check:** reproduces 1.7 / 1.1 / 2.6 V within 0.1 V.
- **Output coupling:** about 69–76 Hz.
- **Hiss:** about −62 dBFS in the 300–3000 Hz band.
- **Clip limits:** about +7 dBFS (negative) and +16.5 dBFS (positive).
- **Bound:** L₊ + L₋ ≈ +19 dBFS.
- **Level-5 margin:** about 16 dB.

## 2. Story 1: the band and the hiss

```sh
cargo test -p rr_dr60 stages::mic                    # core: sections, generator, keying, bypass, validation
cargo test -p rr_dr60_harness --test us1_mic -- --nocapture
cargo test -p rr_dr60_harness --test mic_matrix -- --nocapture
```

Expected at the seven rates:
- **Band:** the prototype is matched within ±0.1 dB at each third-octave centre above −40 dB.
- **Hiss:** within ±0.5 dB of the figure; the third-octave shape 315–2500 Hz within ±1 dB; each offset moves the level by its value within ±0.5 dB.
- **Streams:** identical for the same key; cross-correlation below 0.01 across seeds and counters; the 4σ count within ±30 %.
- **Also:** the P1 checkpoint's bypass identity and first-recording equivalence.

## 3. Story 2: clipping

```sh
cargo test -p rr_dr60_harness --test us2_mic -- --nocapture
```

Expected:
- **Clipper:** its onsets and limits within ±0.5 dB of the figures; it never exceeds them.
- **Bound:** the output after stage 2 never exceeds L₊ + L₋, including at +40 dBFS and on adversarial squares.
- **Harmonics:** H2 and H3 within ±2 dB at 44.1–96 kHz, with alias levels reported at 6, 8 and 16 kHz.
- **Recovery:** within ±0.1 dB after the printed settling time.

## 4. Story 3: the whole engine

```sh
cargo test -p rr_dr60_harness --test golden --test golden_agc --test golden_vas --test golden_codec \
  --test golden_output --test golden_output_clamp --test golden_volume_ramp   # the seven files unmoved
cargo test -p rr_dr60_harness --test us1_mic whole_engine -- --nocapture       # equivalence, rest, rekey, taps
cargo test -p rr_dr60_harness --test mic_matrix vas_margin -- --nocapture      # 60 s per level and rate
cargo test -p rr_dr60_harness --test determinism mic && cargo test -p rr_dr60_harness --test alloc_free
cargo test -p rr_dr60_harness --test mutation mic
cargo test -p rr_dr60_harness --release --test timing mic_cost -- --ignored --nocapture --test-threads=1
scripts/ab-timing.sh <0.11 rev>                      # bypassed: ≤ 3 %
scripts/ab-timing.sh <0.11 rev> output_cost_mic      # on: ≤ 10 %
```

Expected:
- **Voice activation:** pauses once and never resumes at every level and rate; the level-5 margin is at least 10 dB.
- **Limiter:** the ratio oracle within 4 ULP.
- **Codec:** the decoded hiss is reported.
- **Mutants:** all four fail their rows.

Release-only: the 600 s and 60-minute runs (`--release --test mic_matrix -- --ignored`).

## 5. Story 4: the records

```sh
cargo test -p rr_dr60_harness --test us1_manifest --test us2_cross_check
scripts/check-traceability.sh
```

## 6. C

```sh
crates/rr_dr60_ffi/tests/c/run_smoke.sh
cargo test -p rr_dr60_harness --test ffi_parity
```

## 7. Gates (Phase 6 order)

1. **Without the new golden:** fmt, clippy, all tests except `golden_mic`, the MSRV build with `-D warnings`, the header drift check, `--check` and the byte-identical `--write`.
2. **Reviews:** the hardware review, then the coach review.
3. **Bless:** `RR_DR60_BLESS=mic cargo test -p rr_dr60_harness --test golden_mic`, with the SHA pinned and `ci.yml` and the iOS script updated.
4. **With coverage:** `cargo llvm-cov --all-features --workspace --fail-under-lines 80`.
