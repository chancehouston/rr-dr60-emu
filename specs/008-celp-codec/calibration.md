# Calibration of the codec tolerances (feature 008; research R-09; clarify Q5)

**Date**: 2026-10-12. **Reference coder**: ITU-T G.723.1 at 5.3 kbit/s (S-008), the documented ACELP coder of the same family and bit density as the model (0.667 bit per sample), run through the **ITU-T reference implementation**, fixed-point C source version 5.2 (the "Software Package Release 2 (June 2006)" attached to Recommendation G.723.1 (05/2006), main body and Annex A, VAD/CNG off; built with `cc -O2 -x c -std=gnu89 -D__alpha`, the branch of its `TYPEDEF.H` with 16-bit `short` and 32-bit `int` words; the `__sun` branch makes a 64-bit word on this machine and the coder hangs). ffmpeg's G.723.1 encoder has no 5.3 kbit/s mode (`g723_1enc.c`: "Use bitrate 6300 instead of 5300"), so it was not used; its decoder cross-check was skipped (ffmpeg not on `PATH`).

The figures below are **the reference coder's**, measured by the harness's own metrics (`analysis.rs`: `third_octave_levels`, `lpc_lsd`, `segmental_snr`, `level_track`, `xcorr_lag`; the same code the codec checks use), on the harness's stimuli at 8 kHz. They set the tolerances the model must meet (figure plus a stated margin, never looser than the spec's floors). They say nothing about the RR-DR60's coder.

## Stimuli (8000 Hz, 16-bit little-endian, 10 s, seed 0x0D60; `examples/codec_stimuli.rs`)

| File | SHA-256 |
|---|---|
| `voiced_m10.raw` | `aa1412f569ee1785e34612acbce3e404a89975670605b7455d34bdca650512cd` |
| `voiced_m20.raw` | `6ea93374468266ce22f1eae4ac4872a49284292599ca9d3b12316861147727d7` |
| `voiced_m40.raw` | `f123678dfaa5701caf24106ce042e41ef91132d13f1f84022927238536c465f6` |
| `unvoiced_voiced_m10.raw` | `6bb76c9be4bedceb8f287eda966e60f4c30692664a1ef347d0976dd644d7a122` |
| `unvoiced_voiced_m20.raw` | `bc2cfa6e15e6a55c20bf6cd5b16dcc3d909e0fffcca86c6440c8250e1628b6a0` |
| `unvoiced_voiced_m40.raw` | `a701f3469d9947f8d6917bac16eebc13438355777c932de1b112789c4502b3c0` |

The test `codec_matrix::stimulus_digests` asserts these digests; a stimulus change invalidates this file visibly.

## Reference figures (`cargo test -p rr_dr60_harness --release --test codec_matrix calibration -- --ignored --nocapture`)

| Stimulus | Lag (samples at 8 kHz) | RMS error (dB) | LSD (dB) | Segmental SNR (dB) | Level track (dB) | Third-octave deviations 315 … 2500 Hz (dB) |
|---|---|---|---|---|---|---|
| voiced −10 dBFS | 60 | −0.01 | 1.86 | 9.09 | 0.75 | +0.6 +0.5 +0.4 +0.3 +0.1 −1.0 −1.1 −0.4 −0.0 −0.8 |
| voiced −20 dBFS | 60 | 0.00 | 1.93 | 9.05 | 0.79 | +0.5 +0.6 +0.4 +0.3 +0.1 −1.0 −1.2 −0.3 +0.1 −0.9 |
| voiced −40 dBFS | 60 | −0.04 | 1.95 | 9.16 | 1.00 | +0.5 +0.5 +0.3 +0.2 +0.1 −1.1 −1.2 −0.4 −0.1 −1.0 |
| unvoiced-voiced −10 dBFS | 60 | −0.13 | 2.34 | 6.94 | 5.77 | +0.5 +0.5 +0.5 +0.0 −0.6 −3.0 −2.3 −1.9 −4.3 −2.9 |
| unvoiced-voiced −20 dBFS | 60 | −0.16 | 2.37 | 6.94 | 5.68 | +0.5 +0.5 +0.6 +0.1 −0.5 −2.9 −2.3 −1.8 −4.6 −3.1 |
| unvoiced-voiced −40 dBFS | 60 | −0.17 | 2.31 | 7.68 | 5.76 | +0.5 +0.5 +0.5 −0.0 −0.5 −3.2 −2.5 −2.0 −4.4 −2.9 |

The lag of 60 samples is G.723.1's 7.5 ms look-ahead (S-008), a sanity check of the measurement. Third-octave deviations are measured at 8 kHz band edges; the model's are measured at 6 kHz with the same centres.

## Tolerances for the model (spec FR-005, FR-006; `codec_checks.rs`)

| Check | Spec floor | Reference (worst over the three levels) | Margin | **Tolerance asserted** |
|---|---|---|---|---|
| RMS level, voiced | ±1.5 dB | 0.04 dB | — | **±1.5 dB** (the floor; the reference is far inside it) |
| Third-octave 315 – 2000 Hz, voiced | ±3 dB | 1.2 dB | — | **±3 dB** (the floor) |
| Log-spectral distance, voiced | set here | 1.95 dB | +1.0 dB | **≤ 3.0 dB** |
| Segmental SNR, voiced | ≥ 4 dB | 9.05 dB | −2.0 dB | **≥ 7.0 dB** |
| Level tracking, unvoiced-voiced (100 ms windows) | ±3 dB | 5.77 dB | +1.0 dB | **≤ 6.8 dB** |

**Spec amendment to record (FR-006)**: the spec's ±3 dB level-tracking figure was an uncalibrated engineering guess that the reference coder itself fails by almost 3 dB (its rendering of the shaped-noise segment sits lower than the input in the 1000 – 2500 Hz bands, as the third-octave deviations show). The calibrated figure replaces it, as research R-13 anticipated for figures the reference coder also fails. The other floors stand, and the reference's figures confirm the hardware review's expectation of 5 – 10 dB segmental SNR for coders of this class.

## Reproduction

```sh
cargo run -p rr_dr60_harness --example codec_stimuli -- --rate 8000 --out tools/celp-calibrate/out
uv run tools/celp-calibrate/calibrate.py tools/celp-calibrate/out      # needs tools/celp-calibrate/ref/ (see its README)
cargo test -p rr_dr60_harness --release --test codec_matrix calibration -- --ignored --nocapture
```
