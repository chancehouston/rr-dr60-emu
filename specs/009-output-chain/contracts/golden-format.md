# Contract: golden files (feature 009; research R-12; FR-016)

## The four earlier files

`golden-v1.json`, `golden-agc-v1.json`, `golden-vas-v1.json` and `golden-codec-v1.json` are byte-identical to their committed form and their SHA-256 guards unmoved: every configuration they use sets `output: OutputSettings::BYPASSED` (`configs::settings` does so for every pre-009 name) and keeps `tap = AfterPlayback` where it was the default, so their entries are produced by exactly the 0.8 chain.

## `golden-output-v1.json`

Same `GoldenFile` form; `GoldenEntry` gains two optional fields, absent (and not serialized) in the earlier files:

| Field | Type | Meaning |
|---|---|---|
| `route` | `"speaker"` / `"earphone"` | the route set before the stimulus is processed |
| `volume` | `f32` | the volume position set before the stimulus is processed |

Entries: 4 stimuli × 4 configurations × 7 host rates = **112**, sorted by (stimulus, config, host_rate_hz):

| Stimulus | Content |
|---|---|
| `log_sweep_m20` | the harness's log sweep 20 Hz – 3 kHz at −20 dBFS, 2 s |
| `tone_1k_m10` | 1 kHz at −10 dBFS, 2 s |
| `voiced_m20` | the spec 008 voiced stimulus at −20 dBFS, 2 s |
| `step_over` | 1 kHz stepped from −20 dBFS to +6 dB over the ceiling (at the entry's volume), 2 s: the clipper engaged and released |

| Configuration | Route | Volume |
|---|---|---|
| `output_only_speaker` | speaker | `VOLUME_REFERENCE` |
| `output_only_earphone` | earphone | 1.0 |
| `default_output` | speaker | `VOLUME_REFERENCE` |
| `default_earphone` | earphone | 1.0 |

Each entry holds the one-pass output SHA, `n_samples`, `head`, `rms_dbfs` as the earlier files; for the two default configurations the generator also asserts that record-then-play of the same stimulus at the same route and volume equals the one-pass output under spec 008 FR-009's rule (the equivalence is asserted, not stored).

Blessed **once**, last, after the post-implementation hardware review (`RR_DR60_BLESS=output cargo test -p rr_dr60_harness --test golden_output`), guarded by `GOLDEN_OUTPUT_V1_SHA256` in `tests/golden_output.rs`; a CHANGELOG entry records the bless. CI runs `golden_output` wherever the four others run (the test job, the six golden-matrix targets, the iOS simulator job).
