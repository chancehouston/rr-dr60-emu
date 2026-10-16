# Contract: `golden-mic-v1.json`

The same JSON layout as the earlier golden files (an array of entries, sorted by configuration, stimulus and rate), with extra fields per entry:

```json
{
  "config": "mic_only",
  "stimulus": "silence_1s",
  "host_rate_hz": 48000,
  "seed": 0,
  "counter": 0,
  "hiss_offset_db": 0.0,
  "out_len": 48000,
  "out_hash": "…64 hex…"
}
```

## Stimuli

| Name | Definition |
|---|---|
| `silence_1s` | 1 s of 0.0 |
| `voiced_1s` | `stimulus::voiced(fs, 1.0, −20.0, 7)` |
| `loud_tone` | 1 kHz at +12 dBFS (amplitude 10^(12/20)), 0.25 s, starting on the host-grid peak phase (`stimulus::peak_phase`) |

## Configurations

`mic_only`, `mic_only_quiet`, `mic_capsule_only`, `default_mic` (data-model.md).

## Count and hashing

- **Count:** 3 × 4 × 7 = **84** entries.
- **Hash:** the SHA-256 of the output's little-endian `f32` bytes, as in the earlier files.

## Bless

- **Command:** `RR_DR60_BLESS=mic cargo test -p rr_dr60_harness --test golden_mic`.
- **Asserted at bless time:**
  - every `mic_only_quiet` entry on `silence_1s` is all zeros;
  - every `mic_only` entry on `silence_1s` has a band level within ±0.5 dB of the script's figure;
  - the entries at counter 0 for `default_mic` equal a record-then-play of the same input.
- **When:** blessed once, after the post-implementation reviews. Its SHA is pinned in `tests/golden_mic.rs` and added to `ci.yml` (check job, golden matrix, iOS simulator) and to `scripts/ios-device-golden.sh` in the same commit.
