# Contract: golden files (feature 010; research R-05; FR-002, FR-010)

## The five earlier files

`golden-v1.json`, `golden-agc-v1.json`, `golden-vas-v1.json`, `golden-codec-v1.json` and `golden-output-v1.json` are byte-identical to their committed form and their SHA-256 guards unmoved: every configuration they use carries `clamp_enabled = false` through `OutputSettings::BYPASSED` or `OutputSettings::DEVICE`, so their entries are produced by exactly the 0.9 chain.

## `golden-output-clamp-v1.json`

Same `GoldenFile` form as `golden-output-v1.json` (`route` and `volume` present on every entry; `vas` and `frames` absent). `GoldenEntry` gains one optional field, absent (and not serialized) in the earlier files:

| Field | Type | Meaning |
|---|---|---|
| `n_at_bound` | `u32` | the count of output samples exactly equal to +1.0 or −1.0; recomputed and compared on every run (it needs no twin), so each entry documents whether it clamped |

Entries: 3 stimuli × 6 configurations × 7 host rates = **126**, sorted by (stimulus, config, host_rate_hz):

| Stimulus | Content |
|---|---|
| `sine_1k_1p5` | 1 kHz sine of amplitude 1.5 (3.5 dB over full scale), 2 s; input above full scale is kept exactly with every stage bypassed (spec 001) |
| `square_1k_fs` | a ±1.0 square wave, a half-period of round(rate ÷ 2000) host samples, starting at +1.0, 2 s |
| `tone_1k_0` | the existing 1 kHz tone at 0 dBFS, 2 s |

| Configuration | Route | Volume | Option-off twin |
|---|---|---|---|
| `output_clamp_only` | speaker | 1.0 (without effect: every stage bypassed) | `bypass_route_output` (`bypass_all` with `tap = RouteOutput`) |
| `output_only_speaker_clamp` | speaker | 1.0 | `output_only_speaker` |
| `output_only_earphone_clamp` | earphone | 1.0 | `output_only_earphone` |
| `default_output_clamp` | speaker | 1.0 | `default_output` at 1.0 |
| `default_earphone_clamp` | earphone | 1.0 | `default_earphone` |
| `default_output_ref_clamp` | speaker | `VOLUME_REFERENCE` | `default_output` at the reference position (the non-overshooting case: `n_at_bound` expected 0 on both tones at every rate and on the square at every rate but 96 kHz, where 3 samples sit at the bound; T006) |

Each entry holds the one-pass output SHA, `n_samples`, `head`, `rms_dbfs` as the earlier files, plus `n_at_bound`.

**At bless time only** (`RR_DR60_BLESS=output_clamp`), the generator also asserts: for every entry, the identity `on[i] == off[i].clamp(-1.0, 1.0)` against the twin's output; for `default_output_ref_clamp`, zero twin samples beyond ±1.0 on `tone_1k_0` and `sine_1k_1p5` at every rate (the square reads 3 samples over at 96 kHz, T006; reported, not asserted); for the three `default_*` configurations, record-then-play equals the one-pass under spec 008 FR-009's rule. On a compare run the generator runs one one-pass per entry (the identity is proven on every run by `us1_output_clamp` and `output_clamp_matrix`), so the file costs about what `golden-output-v1.json` costs on every target.

Blessed **once**, last, after the post-implementation hardware review (`RR_DR60_BLESS=output_clamp cargo test -p rr_dr60_harness --test golden_output_clamp`), guarded by `GOLDEN_OUTPUT_CLAMP_V1_SHA256` in `tests/golden_output_clamp.rs`; a CHANGELOG entry records the bless. CI runs `golden_output_clamp` wherever the five others run (`ci.yml`: the check job line 81, the golden-matrix line 140, the iOS simulator line 152), and `scripts/ios-device-golden.sh` adds `--test golden_output --test golden_output_clamp` (009 never added `golden_output` there).
