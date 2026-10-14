# Contract: the codec golden file and the unchanged three (feature 008; FR-021; research R-09)

## The three existing files do not move

`golden-v1.json`, `golden-agc-v1.json`, `golden-vas-v1.json` and their SHA guards stay byte-identical: every configuration they describe runs with `speech_codec_enabled = false` (the harness's `configs::settings` sets it for every pre-008 configuration, as it bypasses the AGC for spec 001's and the VAS for specs 001–002's). Spec 006's `applicable_entries()` and the record-then-play equivalence keep their 196-of-217 rule with the codec bypassed (the memory in the `Samples` encoding).

## `golden/golden-codec-v1.json`

Format `rr_dr60-golden`, version 1, with two fields added to every entry:

| Field | Meaning |
|---|---|
| `stimulus` | `voiced_m20`, `unvoiced_voiced_m20` (the synthetic speech-like stimuli at −20 dBFS, 2 s each, seed 0x0D60), `tone_1k_m20`, `noise_seed_0d60` (spec 001's, 0.5 s) |
| `config` | `default_codec` (every stage on, tap after playback; the harness's `default` name stays the codec-bypassed 0.7 default of the three earlier files), `codec_only` (limiter, stages 4 and 10, clamp and VAS bypassed; tap after the decoder), `codec_only_vas` (as codec_only with the VAS on, drop mode) |
| `host_rate_hz` | the seven rates |
| `n_samples`, `sha256`, `head`, `rms_dbfs` | the one-pass output as today |
| `frames: u64` | frames of the record-then-play recording of the same stimulus |
| `frames_sha256` | SHA-256 of the concatenated 10-byte frames |
| `vas` | present for `codec_only_vas` (events and the paused flag) |

84 entries (4 × 3 × 7). The one-pass output is compared as today; the record-then-play recording's frame bytes are compared by digest and its playback must equal the one-pass output under FR-009's rule (the stop's silence appended to the one-pass input). The cross-platform golden matrix runs `golden_codec` on all six platforms, so the frame bytes are proven identical across targets.

Bless once at the end of implementation: `RR_DR60_BLESS=codec cargo test -p rr_dr60_harness --test golden_codec`, with a CHANGELOG entry. Any later change to the frame bytes is a model change (a CHANGELOG entry and, after 1.0, a kept decoder).
