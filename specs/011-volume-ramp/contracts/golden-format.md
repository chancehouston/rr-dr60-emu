# Contract: golden files (feature 011; research R-09; FR-006, FR-012)

## The six earlier files

`golden-v1.json`, `golden-agc-v1.json`, `golden-vas-v1.json`, `golden-codec-v1.json`, `golden-output-v1.json` and `golden-output-clamp-v1.json` are byte-identical to their committed form and their SHA-256 guards unmoved: every generator sets its route and position before the first block, so a fresh engine takes the position at once (research R-04, R-06). Phase 1 runs the six golden tests before any core change and after it.

## `golden-volume-ramp-v1.json`

The `GoldenFile` form of `golden-output-v1.json` (`route` and `volume` present on every entry, `volume` being the starting position; `vas`, `frames` and `n_at_bound` absent). `GoldenEntry` gains one optional field, absent (and not serialized) in the earlier files:

| Field | Type | Meaning |
|---|---|---|
| `changes` | `[[u64, f32]]` | the scenario's position changes as `[sample, position]`, each at a 16-sample block boundary: the input sample at the start of the call, which equals the output sample (the generator asserts that every block up to the last change's transition produced its full length, so the VAS in drop mode dropped nothing there). `#[serde(default, skip_serializing_if = "Option::is_none")]`, so the six earlier files read and write the same bytes |

Entries: **5 scenarios × 4 configurations × 7 host rates = 140**, sorted by (stimulus, config, host_rate_hz); the stimulus name carries the scenario (`ramp_up`, `ramp_down`, `ramp_to_zero`, `ramp_retarget`, `ramp_drag`). Every change sample is rounded down to a multiple of 16 (the block size), written ⌊·⌋₁₆; c = ⌊rate ÷ 2⌋₁₆ (0.5 s), N = 0.02 × rate, G = 16 · round(rate ÷ 960) (the drag interval: 96, 128, 272, 736, 800, 1472, 1600 samples, 16.0 – 17.0 ms):

| Scenario | Starting position → changes |
|---|---|
| `ramp_up` | 0.1 → 1 at c |
| `ramp_down` | 1 → 0.1 at c |
| `ramp_to_zero` | 1 → 0 at c |
| `ramp_retarget` | 0.1 → 1 at c → 0.4 at c + ⌊N ÷ 2⌋₁₆ (about half a transition) |
| `ramp_drag` | 0.1 → 0.1 + 0.09 j at c + (j − 1)·G, j = 1 … 10 |

| Configuration | Route | Stimulus level | Tap |
|---|---|---|---|
| `stage11_only_speaker` | speaker | 1 kHz tone −30 dBFS | `AfterVolumeAmp` |
| `stage11_only_earphone` | earphone | 1 kHz tone 0 dBFS | `AfterVolumeAmp` |
| `default_output` | speaker | the voiced −20 dBFS stimulus | `RouteOutput` |
| `default_earphone` | earphone | the voiced −20 dBFS stimulus | `RouteOutput` |

1 s per entry, the one-pass in 16-sample blocks. Each entry holds the output SHA, `n_samples`, `head`, `rms_dbfs` as the earlier files, plus `changes`.

**At bless time only** (`RR_DR60_BLESS=volume_ramp`), the generator also asserts, for the two `stage11_only_*` configurations, the splice identity (research R-05 (a)): before the first change bit-identical to a run held at the starting position, and after the last change's transition sample N bit-identical to a run held at the last position.

Blessed **once**, last, after the post-implementation hardware review (`RR_DR60_BLESS=volume_ramp cargo test -p rr_dr60_harness --test golden_volume_ramp`), guarded by `GOLDEN_VOLUME_RAMP_V1_SHA256` in `tests/golden_volume_ramp.rs`; a CHANGELOG entry records the bless. CI runs `golden_volume_ramp` wherever the six others run (`ci.yml`: the check job, the golden matrix, the iOS simulator), and `scripts/ios-device-golden.sh` adds it.
