# RR-DR60 Signal Chain

The emulator models every stage between **sound arriving at the microphone** and **sound leaving the speaker (or earphone jack)**. This page is the inventory of those stages. For each stage it lists what we know, what we assume, and what the emulator must expose.

The IDs refer to [sources.md](sources.md) (`S-###`) and [assumptions.md](assumptions.md) (`A-###`). The service manual (S-002) and the codec family datasheet (S-005) were read in full on 2026-10-10; rows marked **(S-002)** are now sourced from the schematic.

```text
 RECORD PATH (analog, S-002 sheet 3.1)                                      (digital, DSP firmware)
 sound ─▶ [1 Mic capsule] ─▶ [2 Mic amp + attenuator] ─▶ [4 Codec: input amp, filters, ADC, µ-law]
                                        ▲ control                │ 6 kHz 8-bit PCM
                                        └── [3 AGC detector] ◀───┘ (senses the codec input)
                                                                 ▼
                                   [5 VAS gate] ─▶ [6 Noise reduction?] ─▶ [7 CELP encoder, 4 kbit/s] ─▶ [8 Flash 16 Mbit]

 PLAYBACK PATH
 [8 Flash] ─▶ [9 CELP decoder] ─▶ [10 Codec: µ-law, DAC, filter] ─▶ [11 Volume] ─┬▶ [NJM2113 power amp, 16 dB] ─▶ [12 Speaker 8 Ω]
                                                                                └▶ [13 Emitter follower, 470 Ω] ─▶ earphone 16 Ω
 GLOBAL
 [14 Clock: 8 MHz resonator → DSP PLL → codec BCLK ≈ 500 kHz, sync 6 kHz]   [15 Power: 2× AAA → 3.3 V DC-DC; 3.2 V analog rail]
```

| # | Stage | Known | Assumed | Character it contributes |
|---|-------|-------|---------|--------------------------|
| 1 | Mic capsule | Two-terminal electret RJM0016, biased through 1 kΩ from a filtered 2.7 V rail (S-002) | Omni; sensitivity and self-noise (A-005) | Bass/treble roll-off, self-noise, handling sensitivity |
| 2 | Mic amp | Differential pair Q1/Q3 with active load Q2; series 22 kΩ / shunt-transistor attenuator (the AGC's actuator); codec input amp gain ≈ 11 dB; coupling corners ≈ 72 Hz, roll-off ≈ 13 kHz (S-002) | Total gain ≈ 45–50 dB, full scale near 95–100 dB SPL, noise set by the capsule (A-029) | Hiss floor, soft clipping on very loud sounds |
| 3 | AGC | Peak-detecting limiter sensing the codec input: knee near the codec's full scale, attack ≈ 0.4 ms, release ≈ 10 s, always active (S-002, A-028, A-020) | Knee relative to full scale (±2 dB), attenuation law (A-028). **The emulator's [spec 002](../../specs/002-agc/spec.md) model (target −10 dBFS, 10:1 slope, release 1 s; A-017–A-019) predates the schematic and will be revised.** | A hard ceiling on loud sounds with a brief clipped onset, then gain held down for seconds; no noise pumping below the knee |
| 4 | Codec: anti-alias filter + ADC + µ-law | OKI MSM7702-02 (S-002, S-003): RC LPF + 8th-order BPF, 8-bit µ-law (S-002 "Digital filter; 8 bit µ-law", A-003 verified); **sampling 6 kHz** (S-002, A-026); PLL-clocked filters (S-005) | Band scales with the sync to ≈ 225–2550 Hz with the family's flat-to-0.2 dB template (A-027); levels and idle noise per A-030. **The emulator's [spec 001](../../specs/001-pipeline-skeleton/spec.md) model runs at 8 kHz with −3 dB edges at 300/3400 Hz (A-001, A-014) and will be revised.** Quantization not modeled yet. | Narrow telephone-band sound (top ≈ 2.5 kHz), µ-law quantization noise |
| 5 | VAS gate | Pauses recording when quiet; always on, no control of its own (S-001); **implemented in the DSP on the PCM stream, after the AGC** (S-002, A-033) | Threshold follows the 5-level sensitivity setting (A-008, A-021); whether the setting also changes a digital gain is open (A-033). Modeled in [spec 003](../../specs/003-vas/spec.md). | Clipped word onsets, abrupt splices, missing gaps |
| 6 | Noise reduction | Claimed only (S-004); nothing in the specification or the analog circuit (S-002) | Optional, off by default; if real, DSP firmware (A-009) | Possible gated or "swirly" artifacts |
| 7 | Speech encoder | **CELP, 4 kbit/s, on the Panasonic MN1931712BB DSP** (S-002) | Which CELP: frame length, look-ahead, codebooks (A-004) | "Watery"/buzzy codec artifacts; speech-like rendering of non-speech sounds |
| 8 | Flash storage | SDTB-16A, 16 Mbit; 99 recordings, 60 min (S-002, S-001) | Lossless storage of encoded frames | Capacity and recording-count limits (API-level) |
| 9 | Speech decoder | — | Matches the encoder (A-004) | Synthesis artifacts |
| 10 | Codec: DAC + reconstruction filter | MSM7702 receive path: D/A → 5th-order LPF (S-003); same 6 kHz clock | Response per A-027 (receive template); imaging residue above 3 kHz not modeled yet | Band-limiting, imaging residue |
| 11 | Volume / power amp | 50 kΩ volume control → NJM2113 BTL amp, gain ≈ 16 dB, clips near 1.0–1.25 V, muted while recording (S-002, S-006, A-031) | Clip shape | Distortion at high volume |
| 12 | Speaker | RAS3P13-U, 2.8 cm, 8 Ω, 110 mW max (S-002) | Small-driver response: strong bass roll-off, resonance (A-010) | Thin, "boxy", resonant playback |
| 13 | Earphone out | 2.5 mm, 16 Ω; **separate emitter follower from the volume control, bypassing the power amp** (S-002, A-012, A-032) | Corners ≈ 160 Hz and ≈ 33 Hz; no clipping | The cleanest tap: codec + volume |
| 14 | Clock | 8 MHz ceramic resonator → DSP → PLL → codec BCLK ≈ 500 kHz and 6 kHz sync; 32 kHz crystal for the microcontroller (S-002) | ±0.5 % per-unit deviation (A-011) | Slight pitch offset between units |
| 15 | Power | 2× AAA → 3.3 V DC-DC converter; switched 3.2 V analog rail; lithium memory back-up (S-002) | Battery effects not modeled in v1 (A-013) | — |

## Emulator requirements that apply to every stage

- **Bypassable and configurable**, with parameters that default to the assumed device values.
- **Tap-able**: callers can take output after any stage, e.g. "the codec only" or "the recording without the speaker".
- **Deterministic**: any randomness (noise) comes from a seeded PRNG.
- **Traceable**: every default value cites an `S-###` or `A-###`.
