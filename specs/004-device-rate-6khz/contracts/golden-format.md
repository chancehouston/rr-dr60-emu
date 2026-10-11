# Contract: golden files after the re-bless

The format is unchanged: [001's golden format](../../001-pipeline-skeleton/contracts/golden-format.md) (`format: "rr_dr60-golden"`, `version: 1`) with [003's optional `vas` field](../../003-vas/contracts/golden-format.md). File names are unchanged ("v1" is the format version). What changes is the content, once (spec FR-013; research R-11).

| File | Configurations | Stimuli | Rates | Entries (was → now) | `library_version` |
|---|---|---|---|---|---|
| `golden-v1.json` | bypass_all, default, playback_only, record_only | impulse, noise_seed_0d60, sweep_log, tone_1k_m20 | 7 | 96 → **112** | 0.4.0 |
| `golden-agc-v1.json` | agc_only, default_agc | agc_noise_burst, agc_start_m10, agc_step | 7 | 36 → **42** | 0.4.0 |
| `golden-vas-v1.json` | default_vas, vas_mute, vas_only | vas_burst_gap, vas_noise_gaps, vas_short_bursts | 7 | 54 → **63** | 0.4.0 |

## Stimulus definitions that change

- `agc_noise_burst` and `vas_noise_gaps`: their band-limited noise is 225–2550 Hz (was 300–3400 Hz; spec FR-011, FR-012). Same seed, level, length and bursts.
- No other stimulus changes. `sweep_log` is defined relative to the host rate (20 Hz to 0.45·host) and keeps its definition. The VAS bursts stay at 1 kHz, −8 dBFS, phase 0 (research R-08: the phase rule applies to threshold bisections, not to bursts 10 dB above the threshold).

## Guards

- `golden_agc.rs`'s `GOLDEN_V1_SHA256` and `GOLDEN_AGC_V1_SHA256` are updated to the re-blessed files' hashes in the same commit as the re-bless. They continue to guard against unintended changes after this feature.
- `golden_vas.rs` asserts 63 entries (was 54), every entry with a `vas` field.

## Procedure (one commit)

```sh
uv run tools/filter-design/design_voiceband.py           # regenerate both coefficient sets
cargo test --all-features                                # everything but the golden files passes
RR_DR60_BLESS=1   cargo test -p rr_dr60_harness --test golden
RR_DR60_BLESS=agc cargo test -p rr_dr60_harness --test golden_agc
RR_DR60_BLESS=vas cargo test -p rr_dr60_harness --test golden_vas
shasum -a 256 crates/rr_dr60_harness/golden/golden-v1.json crates/rr_dr60_harness/golden/golden-agc-v1.json   # → update the two guards
cargo test --all-features                                # all green
```

Then the CHANGELOG entry (research R-13). Any later change to a golden file inside this feature is a defect to fix, not a second re-bless.

## Cross-platform

Unchanged policy: one set of files for all eight targets (001 FR-014); the CI golden matrix, the iOS simulator job and `scripts/ios-device-golden.sh` run all three files; at least 100 random partitions of each golden stimulus (determinism tests, now at 7 rates).
