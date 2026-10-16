# Quickstart: validating the microphone capsule and amplifier (stages 1–2)

From the repository root with `~/.cargo/bin` on `PATH`. Expected figures are the design script's print. The numbers below are plan-time estimates.

## 1. The design script

```sh
uv run tools/mic-chain/design_mic_chain.py           # every figure: nodal model, corners, hiss, clipper, bound, margins
uv run tools/mic-chain/design_mic_chain.py --check   # the bounds; exit 0
uv run tools/mic-chain/design_mic_chain.py --write   # regenerates crates/rr_dr60/src/stages/mic_coeffs.rs (byte-identical)
uv run tools/mic-chain/design_mic_chain.py --spice   # the ngspice cross-check (a gate before the bless; ngspice 47 installed)
```

Expected:
- **Corners:** about 818 Hz (680–980 over the grid, bounded above by about 1.0 kHz), 77 Hz and 12.9 kHz, a three-pole fit to the transfer at GSX.
- **DC check:** consistent with 1.7 / 1.1 / 2.6 V.
- **Per-rate fit:** worst error at most 0.1 dB at every rate, with every constraint satisfied.
- **ngspice:** within 10 % of the nodal input corner.
- **Hiss:** about −62 dBFS in the 300–3000 Hz band.
- **Clip limits:** about +7 dBFS (negative) and +16.5 dBFS (positive), with the onsets about 0.6 dB below them, in clipper units and at the engine input.
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
- **Streams:** identical for the same key; cross-correlation below 5/√N at lag 0 and 6/√N over lags ±64 across seeds and counters.
- **Release-only:** the 4σ and 5σ counts on 10⁸ draws (`cargo test -p rr_dr60 --release stages::mic -- --ignored`).
- **Also:** the P1 checkpoint's bypass identity and first-recording equivalence.

## 3. Story 2: clipping

```sh
cargo test -p rr_dr60_harness --test us2_mic -- --nocapture
```

Expected:
- **Clipper:** its static curve (through the probe) equal to the script's; exactly linear below the onsets; never above the limits. The engine-input onsets and limits are within ±0.5 dB with the phase-compensated stimulus.
- **Bound:** the output after stage 2 never exceeds L₊ + L₋, including at +40 dBFS and on adversarial squares.
- **Harmonics:** H2 and H3 within ±0.5 dB of the discrete model at every rate, with alias levels reported.
- **Recovery:** the clipper-off twin identity; within ±0.1 dB after the time printed for the stated stimulus.

## 4. Story 3: the whole engine

```sh
cargo test -p rr_dr60_harness --test golden --test golden_agc --test golden_vas --test golden_codec \
  --test golden_output --test golden_output_clamp --test golden_volume_ramp   # the seven files unmoved
cargo test -p rr_dr60_harness --test us1_mic whole_engine -- --nocapture       # equivalence, rest, rekey, taps
cargo test -p rr_dr60_harness --test mic_matrix vas_margin -- --nocapture      # 60 s per level and rate at AfterVas
cargo test -p rr_dr60_harness --test determinism mic && cargo test -p rr_dr60_harness --test alloc_free
cargo test -p rr_dr60_harness --test mutation mic
cargo test -p rr_dr60_harness --release --test timing mic_cost -- --ignored --nocapture --test-threads=1
scripts/ab-timing.sh                                  # bypassed against c9f9994 (0.11.0): ≤ 3 %
scripts/ab-timing.sh c9f9994 output_cost_mic          # on against 0.11.0: reported (the limiter uncharged at the end)
```

Expected:
- **Voice activation:** pauses once and never resumes at every level and rate; the level-5 margin is at least 10 dB.
- **Limiter:** the on/off ratio at most 1 and non-decreasing; two seeds' ratio trajectories within 4 f32 ULP.
- **Codec:** the decoded hiss is reported.
- **Cost:** `mic_cost` in-build at most 10 % (the CI ceiling is 25 %).
- **Mutants:** all five fail their rows.

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

1. **Without the new golden:** `cargo test --all-features -- --skip golden_mic`, fmt, clippy, the MSRV build with `-D warnings`, the header drift check, `--check` and the byte-identical `--write`.
2. **ngspice:** `--spice` within 10 %, recorded.
3. **Reviews:** the hardware review, then the coach review (one review-driven manifest re-bless is allowed).
4. **Bless:** `RR_DR60_BLESS=mic cargo test -p rr_dr60_harness --test golden_mic`, with the SHA pinned and `ci.yml` and the iOS script updated.
5. **With coverage:** `cargo llvm-cov --all-features --workspace --fail-under-lines 80`.
