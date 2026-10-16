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
| `loud_tone` | 1 kHz at +12 dBFS (amplitude 10^(12/20)), 0.25 s, starting on the phase the design script prints (the input high-pass's phase compensated) |
| `very_loud_tone` | the same at +24 dBFS, reaching both clip limits |

## Configurations

`mic_only`, `mic_only_quiet`, `mic_capsule_only` (at offset −12 dB), `default_mic`, `default_mic_quiet` (data-model.md), and the `default_mic_rec1` family: `silence_1s` recorded as the engine's second recording (counter 1) and played back, at the seven rates.

## Count and hashing

- **Count:** 4 × 5 × 7 + 7 = **147** entries.
- **Hash:** the SHA-256 of the output's little-endian `f32` bytes, as in the earlier files.

## Bless

- **Command:** `RR_DR60_BLESS=mic cargo test -p rr_dr60_harness --test golden_mic`.
- **Asserted at bless time:**
  - every `mic_only_quiet` entry on `silence_1s` is all zeros;
  - every `mic_only` entry on `silence_1s` has a band level within ±0.5 dB of the script's figure;
  - the entries at counter 0 for `default_mic` equal a record-then-play of the same input;
  - every `default_mic_rec1` entry differs from the matching counter-0 entry.
- **When:** blessed once, after the post-implementation reviews. Its SHA is pinned in `tests/golden_mic.rs` and added to `ci.yml` (check job, golden matrix, iOS simulator) and to `scripts/ios-device-golden.sh` in the same commit.
