# Contract: AGC golden file (`golden-agc-v1.json`)

The format is identical to [001's golden format](../../001-pipeline-skeleton/contracts/golden-format.md) (`format: "rr_dr60-golden"`, `version: 1`). Only the contents are new.

- **Path**: `crates/rr_dr60_harness/golden/golden-agc-v1.json`, embedded at compile time like `golden-v1.json`.
- **`golden-v1.json`**: unchanged, byte for byte (FR-017, FR-018). Its entries are produced with the AGC bypassed.

## Stimuli (sorted)

All are bit-reproducible: detmath tones and PCG32 noise.

| Name | Content |
|---|---|
| `agc_noise_burst` | FR-009: noise band-limited to 300–3400 Hz, −70 dBFS RMS, seed `0x0D60`, plus 1 kHz bursts at −10 dBFS, 1 s on / 4 s off, 2 cycles (10 s) |
| `agc_start_m10` | 1 kHz at −10 dBFS from a freshly created pipeline, 0.5 s (start-at-maximum overshoot) |
| `agc_step` | 1 kHz: −40 dBFS 3 s, −10 dBFS 1 s, −40 dBFS 4 s (8 s) |

## Configurations (sorted)

| Name | AGC | Stage 4 | Stage 10 | Tap |
|---|---|---|---|---|
| `agc_only` | on (defaults) | — | — | AfterAgc |
| `default_agc` | on (defaults) | on | on | AfterPlayback |

That gives 3 × 2 × 6 rates = **36 entries**, about 15 KB.

## Blessing

```sh
RR_DR60_BLESS=agc cargo test -p rr_dr60_harness --test golden_agc
```

This rewrites only `golden-agc-v1.json`. Each bless needs a CHANGELOG entry. `RR_DR60_BLESS=1` (001's bless command) is not used in this feature. If `golden-v1.json` changes at all, that is a bug.
