# Contract: golden files after the re-bless (feature 005)

Format unchanged (`rr_dr60-golden`, version 1; the VAS entries' `vas` field as spec 003). File names unchanged. `library_version` 0.5.0.

| File | Configurations | Stimuli | Rates | Entries | Change |
|---|---|---|---|---|---|
| `golden-v1.json` | bypass_all, default, playback_only, record_only (AGC and VAS bypassed; clamp on except in bypass_all) | impulse, noise_seed_0d60, sweep_log, tone_1k_m20 | 7 | 112 | **none** (byte-identical; research R-06) |
| `golden-agc-v1.json` | agc_only (tap after AGC, host rate, everything else off), default_agc (limiter, stages, clamp on; VAS off) | agc_onset, agc_rearm, agc_staircase | 7 | 42 | re-blessed once; stimuli replaced |
| `golden-vas-v1.json` | default_vas, vas_mute, vas_only | vas_burst_gap, vas_noise_gaps, vas_short_bursts | 7 | 63 | re-blessed once; bursts at −30 dBFS |

## AGC stimuli (new; 1 kHz at the host-grid peak phase `stimulus::peak_phase(1000, rate)`, research R-08/R-10, bit-reproducible)

| Name | Content |
|---|---|
| `agc_onset` | −20 dBFS 0.1 s, +11 dBFS 0.4 s, −20 dBFS 2.0 s (dead time, ceiling, release) |
| `agc_rearm` | +11 dBFS 0.2 s, −20 dBFS 2.0 s, +11 dBFS 0.2 s, −20 dBFS 0.6 s (arming) |
| `agc_staircase` | −20, 0, +3, +5, +8, +14 dBFS, 0.4 s each (the knee region and the ceiling; the clamp in `default_agc`). Levels chosen when the stimulus was planned at phase 0 (where 0 and +2 dBFS would both sit below the +1 dBFS onset on the 6 kHz grid, which reads a phase-0 1 kHz tone 1.25 dB low) and kept: at the peak phase no level is inert at any rate (research R-10) |

The 0.4 stimuli `agc_noise_burst`, `agc_start_m10` and `agc_step` are removed (they measured the compressor).

## VAS stimuli (burst level restated)

Definitions as spec 003 with the burst level at the threshold + 10 dB, now −30 dBFS; `vas_noise_gaps`' floor stays −70 dBFS RMS in the 225–2550 Hz band. `default_vas` now runs the limiter and the clamp; `vas_only` and `vas_mute` keep the VAS-isolated configuration (limiter off, stages off, clamp off, tap after VAS).

## Guards and procedure

`GOLDEN_V1_SHA256` unchanged (and asserted so). `GOLDEN_AGC_V1_SHA256` updated in the re-bless commit. One commit: CHANGELOG entry, `RR_DR60_BLESS=agc`, `RR_DR60_BLESS=vas`, guard update, full suite green. Any later golden change inside the feature is a defect.
