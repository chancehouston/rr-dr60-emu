# RR-DR60 Signal Chain

The emulator models every stage between **sound arriving at the microphone** and **sound leaving the speaker (or earphone jack)**. This page is the inventory of those stages. For each stage it lists what we know, what we assume, and what the emulator must expose.

The IDs refer to [sources.md](sources.md) (`S-###`) and [assumptions.md](assumptions.md) (`A-###`).

```text
 RECORD PATH
 sound ─▶ [1 Mic capsule] ─▶ [2 Mic preamp] ─▶ [3 AGC] ─▶ [4 Anti-alias LPF + ADC] ─▶ [5 VAS gate]
                                                                                        │
          ┌─────────────────────────────────────────────────────────────────────────────┘
          ▼
       [6 Noise reduction?] ─▶ [7 Speech encoder] ─▶ [8 Flash storage]

 PLAYBACK PATH
 [8 Flash storage] ─▶ [9 Speech decoder] ─▶ [10 DAC + reconstruction filter] ─▶ [11 Volume / power amp] ─┬▶ [12 Speaker] ─▶ sound
                                                                                                          └▶ [13 Earphone out 2.5 mm]
 GLOBAL
 [14 Clock (8 MHz resonator)]   [15 Power (3 V, 2× AAA)]
```

| # | Stage | Known | Assumed | Character it contributes |
|---|-------|-------|---------|--------------------------|
| 1 | Mic capsule | Built-in mic (S-001) | Omni electret (A-005) | Bass/treble roll-off, self-noise, handling sensitivity |
| 2 | Mic preamp | Discrete transistors 2SB1218A / 2SD1819A (S-002) | Gain, hiss, soft clip (A-006) | Hiss floor, clipping on loud sounds |
| 3 | AGC | Claimed (S-004) | Exists; time constants TBD (A-007) | "Pumping", noise rising in quiet passages |
| 4 | Anti-alias filter + ADC | OKI MSM7702 voice-band codec (S-002, S-003) | 8 kHz rate (A-001); 300–3400 Hz band (A-002); coding format TBD (A-003). **Modeled in [spec 001](../../specs/001-pipeline-skeleton/spec.md): G.712-like minimum-phase band-pass, A-014/A-015/A-016.** Quantization (A-003) not modeled yet. | Telephone-band sound, quantization noise |
| 5 | VAS gate | Pauses recording when quiet (S-001) | Thresholds and levels TBD (A-008) | Clipped word onsets, abrupt splices, missing gaps |
| 6 | Noise reduction | Claimed only (S-004) | Optional, off by default (A-009) | Possible "swirly" or gated artifacts |
| 7 | Speech encoder | Compression is implied by the 60 min capacity (S-001) | CELP-family, ~4.8 kbps (A-004) | Robotic or "watery" codec artifacts; speech-like rendering of non-speech noise |
| 8 | Flash storage | IC flash, 99 recordings, 60 min (S-001) | Lossless storage of encoded frames | Capacity and recording-count limits (API-level) |
| 9 | Speech decoder | — | Matches encoder (A-004) | Synthesis artifacts |
| 10 | DAC + reconstruction filter | MSM7702 (S-002, S-003) | Same band as stage 4 (A-002). **Modeled in [spec 001](../../specs/001-pipeline-skeleton/spec.md): same filter as stage 4, A-014/A-015/A-016.** DAC imaging residue above 4 kHz is not modeled yet. | Band-limiting, imaging residue |
| 11 | Volume / power amp | Volume control exists (S-001) | Small amp; clipping at high volume | Distortion at high volume |
| 12 | Speaker | RAS3P13-U (S-002) | Small dynamic speaker, strong bass roll-off, resonance (A-010) | Thin, "boxy", resonant playback |
| 13 | Earphone out | 2.5 mm jack (S-004) | Bypasses the speaker (A-012) | Cleaner than speaker playback |
| 14 | Clock | 8 MHz ceramic resonator, 32 kHz crystal (S-002) | ±0.5 % per-unit deviation (A-011) | Slight pitch offset between units |
| 15 | Power | 3 V, 2× AAA (S-001) | Battery effects not modeled in v1 (A-013) | — |

## Emulator requirements that apply to every stage

- **Bypassable and configurable**, with parameters that default to the assumed device values.
- **Tap-able**: callers can take output after any stage, e.g. "the codec only" or "the recording without the speaker".
- **Deterministic**: any randomness (noise) comes from a seeded PRNG.
- **Traceable**: every default value cites an `S-###` or `A-###`.
