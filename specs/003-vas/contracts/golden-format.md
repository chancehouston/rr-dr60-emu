# Contract: VAS golden file (`golden-vas-v1.json`)

The format is [001's golden format](../../001-pipeline-skeleton/contracts/golden-format.md) (`format: "rr_dr60-golden"`, `version: 1`), with one optional field per entry.

- **Path**: `crates/rr_dr60_harness/golden/golden-vas-v1.json`, embedded at compile time.
- **Unchanged files**: `golden-v1.json` and `golden-agc-v1.json` stay byte-for-byte identical (FR-019, FR-020). Their entries are produced with VAS bypassed. `golden_agc.rs` checks both by SHA-256.

## Entry additions

The numbers in this example are illustrative.

```json
{
  "stimulus": "vas_burst_gap",
  "config": "vas_only",
  "host_rate_hz": 48000,
  "n_samples": 141120,
  "sha256": "…",
  "head": ["…"],
  "rms_dbfs": -9.2,
  "vas": {
    "events": [[96000, 191040]],
    "paused_at_end": false
  }
}
```

- **`n_samples`**: the total produced, which is now the output length, not the input length.
- **`sha256`**: covers only the produced samples.
- **`vas`**: present in VAS-file entries only. It is skipped when serializing older entries, so older files read unchanged.
  - `events`: `[output_position, input_length]` pairs in order.
  - `paused_at_end`: whether VAS was paused at the end of the stimulus.
- **Comparison**: compare fails if `events` or `paused_at_end` differ.

## Stimuli (sorted)

All are bit-reproducible: detmath tones and PCG32 noise. Bursts are 1 kHz at −8 dBFS, 10 dB above the default threshold, so they stay below full scale. Gaps are digital silence unless stated otherwise.

| Name | Content |
|---|---|
| `vas_burst_gap` | burst 1 s, gap 0.6 s (under the hang time), burst 0.5 s, gap 3 s, burst 1 s, gap 2.5 s (8.6 s; ends paused) |
| `vas_noise_gaps` | the 002 band-limited noise (seed `0x0D60`) at −40 dBFS throughout, plus bursts: 1 s on, 3 s off, 2 cycles (8 s) |
| `vas_short_bursts` | gap 2 s, then bursts of 10 ms, 30 ms and 200 ms, each followed by 2 s of silence (6.24 s) |

## Configurations (sorted)

| Name | AGC | Stage 4 | VAS | Stage 10 | Tap |
|---|---|---|---|---|---|
| `default_vas` | on | on | on (drop) | on | AfterPlayback |
| `vas_mute` | — | — | on (mute) | — | AfterVas |
| `vas_only` | — | — | on (drop) | — | AfterVas |

That gives 3 × 3 × 6 rates = **54 entries**. Stimuli are processed in one block, as before. Partition equivalence, including events, is tested by `determinism`.

## Blessing

```sh
RR_DR60_BLESS=vas cargo test -p rr_dr60_harness --test golden_vas
```

This rewrites only `golden-vas-v1.json`. Each bless needs a CHANGELOG entry. The 001 and 002 bless commands are not used in this feature.
