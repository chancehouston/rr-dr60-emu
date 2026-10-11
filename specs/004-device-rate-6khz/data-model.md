# Data Model: 6 kHz Device Rate and Codec Filter Templates

**Feature**: [spec.md](spec.md) | **Research**: [research.md](research.md)

This revises the data models of [spec 001](../001-pipeline-skeleton/data-model.md), [spec 002](../002-agc/data-model.md) and [spec 003](../003-vas/data-model.md). No settings entity gains or loses a field. Only changed constants, the new per-stage templates and the restated figures are listed.

## Device rate (changed)

| Constant | Was | Now | Source |
|---|---|---|---|
| `DEVICE_RATE_HZ` | 8000 | **6000** | A-026 (S-002) |
| Device Nyquist frequency | 4 kHz | **3 kHz** | derived |

Every millisecond value in `AgcSettings` and `VasSettings` converts to device samples through this constant (R-01). Defaults in device samples: AGC detector delay 31 (unchanged in samples, 5.17 ms), VAS hang 6000 (was 8000), VAS onset 120 (was 160), VAS continuity window W 32 (unchanged, now 5.3 ms).

## Host rates and rate plans (changed)

| Constant | Was | Now |
|---|---|---|
| `SUPPORTED_HOST_RATES` | `[u32; 6]` = 8000, 16000, 44100, 48000, 88200, 96000 | **`[u32; 7]`** = **6000**, 8000, 16000, 44100, 48000, 88200, 96000 |
| Identity rate | 8000 | **6000** |

`RatePlan` (internal), `6000 / host = l / m`:

| Host | Plan | l | m | Prototype rate | Taps (R-03) | Delay per converter | Lead bound ⌈host/6000⌉ |
|---|---|---|---|---|---|---|---|
| 6 000 | Identity | — | — | — | — | 0 | 1 |
| 8 000 | Convert | 3 | 4 | 24 000 | 235 | 39 | 2 |
| 16 000 | Convert | 3 | 8 | 48 000 | 463 | 77 | 3 |
| 44 100 | Convert | 20 | 147 | 882 000 | 8 481 | 212 | 8 |
| 48 000 | Convert | 1 | 8 | 48 000 | 463 | 231 | 8 |
| 88 200 | Convert | 10 | 147 | 882 000 | 8 481 | 424 | 15 |
| 96 000 | Convert | 1 | 16 | 96 000 | 923 | 461 | 16 |

Taps and delays are the Kaiser estimates `rate.rs` computes (the code is the source of truth). Boundary constants: `CUTOFF_HZ` 3800 → **2775**, `TRANSITION_HZ` 400 → **450**, `STOPBAND_DB` 70 (unchanged).

- **Validation**: `Settings::validate` and `Pipeline::new` reject any rate not in the list with `Error::UnsupportedHostRate`, whose Display lists the seven rates (001 FR-002, spec FR-002).
- **Integer-ratio rates** (lengths exact in spec FR-012): 6000, 48 000, 96 000.

## Stage templates (new entities; A-027 corrected, A-016, S-005, S-007)

A **template** is the set of limits a band-limit stage must meet, plus the typical values it is designed toward. Loss is relative to the stage's gain at the reference frequency 765 Hz; group-delay distortion (GDD) is relative to the stage's minimum group delay over 225–2550 Hz.

### Record template (stage 4; transmit, scaled by 0.75)

| Property | Limit (tolerance) | Typical (design goal) | Source |
|---|---|---|---|
| Gain at 765 Hz | 0 ± 0.1 dB | — | A-015 (±0.1 engineering target) |
| Loss, 225–2250 Hz | −0.15 … +0.2 dB | +0.07 dB at 225 Hz | S-005 300/3000 Hz rows, A-027 |
| Loss at 2550 Hz | 0 … 0.8 dB | 0.4 dB | S-005 3400 Hz row, A-027 |
| Attenuation ≤ 45 Hz (above DC) | ≥ 20 dB | 26 dB | S-005 60 Hz row, A-027 |
| Attenuation at DC | ≥ 40 dB | — | A-014; S-007 (G.712) if its mask states DC, else engineering target |
| Loss at 2700, 2900 Hz | report only | — | no source |
| GDD at 375 Hz | ≤ 1.0 ms | 0.25 ms | S-005 500 Hz row ÷ 0.75, A-027 |
| GDD at 450 Hz | ≤ 0.47 ms | 0.15 ms | S-005 600 Hz row ÷ 0.75 |
| GDD 750–1950 Hz | ≤ 0.17 ms | ≤ 0.09 ms | S-005 1000–2600 Hz rows ÷ 0.75 |
| GDD at 2100 Hz | ≤ 1.0 ms | 0.09 ms | S-005 2800 Hz row ÷ 0.75 |
| Group delay at 1 kHz | ≤ 2 ms | — | A-016 |
| Group delay at 225 and 2550 Hz | > at 1 kHz | — | A-016 |
| Group delay at 2550 Hz | reported; ≤ 2 ms | — | engineering target (A-016's spirit; Clarifications) |
| Phase, 225–2550 Hz (at 6 kHz host) | within ±5° of minimum phase | — | A-016 (±5° engineering target) |
| Anti-alias (host rate, through the stage) | ≥ 30 dB from 3450 Hz to host Nyquist, total power incl. aliases | 32 dB | S-005 discrimination row, A-027 |

Design (R-04): Butterworth high-pass order 3 at 112.5 Hz + Chebyshev-II low-pass order 5, 32 dB stopband, stopband edge ≈ 2740 Hz (script-tuned to 0.4 dB at 2550 Hz); 5 biquads; `RECORD_SOS: [[f64; 5]; 5]`.

### Playback template (stage 10; receive, scaled by 0.75; low-pass only)

| Property | Limit (tolerance) | Typical (design goal) | Source |
|---|---|---|---|
| Gain at 765 Hz | 0 ± 0.1 dB | — | A-015 |
| Loss, DC–2250 Hz (incl. DC, 20, 45, 100 Hz) | −0.15 … +0.25 dB | −0.03 dB at 225 Hz, 0 at DC | S-005 receive 300/3000 Hz rows, A-027 (−0.15 engineering target) |
| Loss at 2550 Hz | 0 … 0.8 dB | 0.56 dB | S-005 receive 3400 Hz row |
| Low-frequency rejection | none (DC passes) | — | S-005 receive table (no 60 Hz row; GDD 0.00 below 1 kHz) |
| Loss at 2700, 2900 Hz | report only | — | no source |
| GDD 375–1950 Hz | ≤ 0.17 ms | 0.00 … 0.12 ms | S-005 receive rows ÷ 0.75 |
| GDD at 2100 Hz | ≤ 1.0 ms | 0.16 ms | S-005 receive 2800 Hz row ÷ 0.75 |
| Group delay at 1 kHz | ≤ 2 ms | — | A-016 |
| Group delay at 2550 Hz | > at 1 kHz; reported; ≤ 2 ms | — | A-016; 2 ms engineering target |
| Phase, 225–2550 Hz (at 6 kHz host) | within ±5° of minimum phase | — | A-016 |

Design (R-05): Chebyshev-II low-pass order 5, 32 dB stopband, stopband edge ≈ 2715 Hz (script-tuned to 0.56 dB at 2550 Hz); 3 biquads; `PLAYBACK_SOS: [[f64; 5]; 3]`.

### Stage types (changed)

| Type | Was | Now |
|---|---|---|
| `VoiceBandStage` | 6 sections, one coefficient set, `new()` | `VoiceBandStage<const N: usize>`; `VoiceBandStage::<5>::record()`, `VoiceBandStage::<3>::playback()` |
| `DeviceChain` | `record`, `playback`: same type | `record: VoiceBandStage<5>`, `playback: VoiceBandStage<3>` |
| Biquads per device sample (both stages on) | 12 | 8 |
| `Pipeline::__with_stage_sos` (test hooks) | `(Settings, [[f64;5];6])` | `(Settings, record: [[f64;5];5], playback: [[f64;5];3])` |

## Rate conversion boundary (changed; FR-003)

| Property | Was (001 FR-005) | Now |
|---|---|---|
| Flat within ±0.1 dB | 50–3600 Hz | 50–**2550** Hz |
| May be attenuated | 3600–4000 Hz | **2550–3000** Hz |
| ≥ 60 dB rejection incl. alias/image | from 4000 Hz | from **3000** Hz |
| Not checked at | 8 kHz host (identity) | **6 kHz** host (identity) |

## Test frequency sets (new; engineering targets)

- **Stage set**: DC, 20, 45, 100, 150, 225, 300, 375, 450, 600, 765, 1000, 1500, 1950, 2100, 2250, 2400, 2550, 2700, 2900 Hz.
- **Anti-alias set**: 3450, 4000, 5000, 7000, 10 000, 15 000, 20 000, 40 000 Hz, each only below the host Nyquist frequency.
- **Cascade check**: 50–2900 Hz (FR-009).

## Stimulus phase rule (new; R-08)

For a steady tone of frequency f that divides 6000, used to measure a sample-peak detector at host rate h: starting phase φ = (π/2 + 2π·f·D/h) mod (2π·f/6000), where D = the bypassed pipeline's reported latency at h divided by 2 (the decimator's delay in host samples; 0 at 6 kHz). Applied to the VAS threshold bisections at 1 kHz; not to the burst-gap stimuli (threshold + 10 dB) or the AGC tones.

## Restated figures (new entity: the CHANGELOG table, FR-010 to FR-013)

| Spec and clause | Was | Now |
|---|---|---|
| 001 FR-002 host rates / identity | 6 rates; 8 kHz identity | 7 rates; 6 kHz identity |
| 001 FR-004 device rate | 8 kHz | 6 kHz |
| 001 FR-005 boundary | flat to 3600, ≥ 60 dB from 4000 Hz | flat to 2550, ≥ 60 dB from 3000 Hz |
| 001 FR-006 shape | same nominal shape both stages | record band-pass, playback low-pass |
| 001 FR-010 stage template | −3 dB at 300/3400, ripple 400–3200, DC/60 Hz both stages, 14 dB at 4000, 25 dB from 4600 | record and playback templates above; DC/45 Hz record only; 4000 Hz point dropped; anti-alias ≥ 30 dB from 3450 |
| 001 FR-010 phase check rate | 8 kHz | 6 kHz |
| 001 FR-011 cascade range | 100–3900 Hz, 1 kHz gain | 50–2900 Hz, 765 Hz gain |
| 001 edge case DC | ≤ −60 dBFS default (two stages) | ≤ −60 dBFS default (record stage alone); playback-only passes DC |
| 001 latency, default at 48 kHz | 541 samples (11.3 ms) | ≈ 471 samples (≈ 9.8 ms); all rates in the CHANGELOG |
| 002 A-019 detector band | up to 4 kHz | up to 3 kHz |
| 002 FR-006 exact-gain method rate | 8 kHz | 6 kHz |
| 002 FR-008 tones | 300, 500, 1000, 2000, 2667, 3400 Hz; phases at 2000, 2667 | 225, 375, 750, 1000, 1500, 2000, 2550 Hz; phases at 1500, 2000 |
| 002 FR-009 noise band | 300–3400 Hz | 225–2550 Hz |
| 002 detector delay (harness) | 31/8000 s | 31/6000 s |
| 003 FR-004/FR-005 exact rates | 8, 16, 48, 96 kHz | 6, 48, 96 kHz |
| 003 FR-004 lead bound / settling | ⌈host/8000⌉ (12 at 96 kHz) | ⌈host/6000⌉ (16 at 96 kHz) |
| 003 FR-006 frequencies | 300, 1000, 3400 Hz | 225, 1000, 2550 Hz, 1 kHz phase-aligned |
| 003 FR-004/FR-010 bit-identity rate | 8 kHz | 6 kHz |
| 003 hang / onset in device samples | 8000 / 160 | 6000 / 120 |
| 003 W (continuity window) | 32 samples = 4 ms | 32 samples = 5.3 ms |
| Golden entries (001 / AGC / VAS) | 96 / 36 / 54 | 112 / 42 / 63 |

## Assumption status changes (FR-014)

| ID | Change |
|---|---|
| A-001 | superseded by A-026 (adopted by spec 004) |
| A-002 | superseded by A-027 (adopted by spec 004) |
| A-014 | superseded by A-027 (spec 004) |
| A-016 | kept; confidence High; status partially verified (S-005, MSM7717 family proxy); 6 kHz GDD figures recorded |
| A-019 | detector band 4 kHz → 3 kHz |
| A-026 | adopted (verified) |
| A-027 | adopted; receive row corrected to low-pass only; GDD limits per template recorded |
| S-007 (new source) | ITU-T G.712, cited for the origin of spec 001's 4000 and 4600 Hz points, and for the record stage's DC attenuation only if its mask states DC |
