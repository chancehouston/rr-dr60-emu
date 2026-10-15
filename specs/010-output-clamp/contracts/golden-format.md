# Contract: golden files (feature 010; research R-05; FR-002, FR-010)

## The five earlier files

`golden-v1.json`, `golden-agc-v1.json`, `golden-vas-v1.json`, `golden-codec-v1.json` and `golden-output-v1.json` are byte-identical to their committed form and their SHA-256 guards unmoved: every configuration they use carries `clamp_enabled = false` through `OutputSettings::BYPASSED` or `OutputSettings::DEVICE`, so their entries are produced by exactly the 0.9 chain.

## `golden-clamp-v1.json`

Same `GoldenFile` form as `golden-output-v1.json` (`route` and `volume` present on every entry; `vas` and `frames` absent). Entries: 3 stimuli × 5 configurations × 7 host rates = **105**, sorted by (stimulus, config, host_rate_hz):

| Stimulus | Content |
|---|---|
| `sine_1k_1p5` | 1 kHz sine of amplitude 1.5 (3.5 dB over full scale), 2 s; input above full scale is kept exactly with every stage bypassed (spec 001) |
| `square_1k_fs` | a ±1.0 square wave, a half-period of round(rate ÷ 2000) host samples, starting at +1.0, 2 s |
| `tone_1k_0` | the existing 1 kHz tone at 0 dBFS, 2 s |

| Configuration | Route | Volume | Option-off twin |
|---|---|---|---|
| `clamp_only` | speaker | 1.0 (without effect: every stage bypassed) | `bypass_route_output` (every stage bypassed, `tap = RouteOutput`) |
| `output_only_speaker_clamp` | speaker | 1.0 | `output_only_speaker` |
| `output_only_earphone_clamp` | earphone | 1.0 | `output_only_earphone` |
| `default_output_clamp` | speaker | 1.0 | `default_output` at 1.0 |
| `default_earphone_clamp` | earphone | 1.0 | `default_earphone` |

Each entry holds the one-pass output SHA, `n_samples`, `head`, `rms_dbfs` as the earlier files. The generator asserts, for every entry, the identity `on[i] == off[i].clamp(-1.0, 1.0)` against the twin's output, and for the two `default_*_clamp` configurations that record-then-play equals the one-pass under spec 008 FR-009's rule (asserted, not stored).

Blessed **once**, last, after the post-implementation hardware review (`RR_DR60_BLESS=clamp cargo test -p rr_dr60_harness --test golden_clamp`), guarded by `GOLDEN_CLAMP_V1_SHA256` in `tests/golden_clamp.rs`; a CHANGELOG entry records the bless. CI runs `golden_clamp` wherever the five others run (the check job, the six golden-matrix targets, the iOS simulator job) and `scripts/ios-device-golden.sh` adds it.
