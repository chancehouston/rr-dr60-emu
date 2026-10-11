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
| 3 | AGC | Peak-detecting limiter sensing the codec input: a half-wave detector charges a 1 µF hold capacitor through 390 Ω, discharged through 10 MΩ, driving a shunt transistor ahead of the codec; always active (S-002, A-028, A-020) | Knee −1 dBFS ± 2 dB, just below full scale (A-028); dead time ≈ 3 ms at 10 dB over the knee (A-034); release ≈ 1.4 s, with an arming decay of 10 s (A-035, A-028); positive polarity sensed (A-036). **Modeled by [spec 005](../../specs/005-limiter-agc/spec.md) (2026-10-11) at the host rate, before the rate conversion, replacing the spec 002 compressor (A-017 – A-019 superseded or revised): modeled on the circuit (S-002), not measured against a unit; whether a real unit limits just below full scale (the model, a clean sine) or flat-tops at it is the open question (A-028).** | Nothing below the knee (no noise pumping); a hard ceiling on loud sounds with a few milliseconds of clipped onset, then recovery over about 1.4 s |
| 4 | Codec: anti-alias filter + ADC + µ-law | OKI MSM7702-02 (S-002, S-003): RC LPF + 8th-order BPF, 8-bit µ-law (S-002 "Digital filter; 8 bit µ-law", A-003 verified); **sampling 6 kHz** (S-002, A-026); PLL-clocked filters (S-005) | Band scales with the sync to ≈ 225–2550 Hz with the family's flat-to-0.2 dB transmit template (A-027); levels and idle noise per A-030. **Modeled by [spec 004](../../specs/004-device-rate-6khz/spec.md) (2026-10-11): the device rate is 6 kHz and the stage is the S-005 transmit template scaled to that clock (a band-pass, unity gain at 765 Hz, minimum-phase, A-015/A-016), as a filter design that meets the scaled loss and group-delay limits; modeled on the datasheet, not measured against a unit. Not modeled: the device's partial aliasing of 3000–3450 Hz input into the top of its band (the emulator's rate conversion boundary rejects everything from 3 kHz by ≥ 60 dB), µ-law quantization and idle noise (A-003, A-030). The 2550–3000 Hz roll-off is a design choice not constrained by any source, reported at 2700 and 2900 Hz. Full-scale clamp after the filter ([spec 005](../../specs/005-limiter-agc/spec.md) FR-008, A-030): samples beyond ±1.0 saturate, which gives loud onsets their clipped start; modeled on the codec's full-scale code (S-005), not measured.** | Narrow telephone-band sound (top ≈ 2.5 kHz), µ-law quantization noise |
| 5 | VAS gate | Pauses recording when quiet; always on, no control of its own (S-001); **implemented in the DSP on the PCM stream, after the AGC** (S-002, A-033) | Threshold follows the 5-level sensitivity setting (A-008, A-021); whether the setting also changes a digital gain is open (A-033). **Modeled in [spec 003](../../specs/003-vas/spec.md): A-022 – A-025.** | Clipped word onsets, abrupt splices, missing gaps |
| 6 | Noise reduction | Claimed only (S-004); nothing in the specification or the analog circuit (S-002) | Optional, off by default; if real, DSP firmware (A-009) | Possible gated or "swirly" artifacts |
| 7 | Speech encoder | **CELP, 4 kbit/s, on the Panasonic MN1931712BB DSP** (S-002) | Which CELP: frame length, look-ahead, codebooks (A-004) | "Watery"/buzzy codec artifacts; speech-like rendering of non-speech sounds |
| 8 | Flash storage | SDTB-16A, 16 Mbit; 99 recordings, 60 min (S-002, S-001) | Lossless storage of encoded frames | Capacity and recording-count limits (API-level) |
| 9 | Speech decoder | — | Matches the encoder (A-004) | Synthesis artifacts |
| 10 | Codec: DAC + reconstruction filter | MSM7702 receive path: D/A → 5th-order LPF (S-003); same 6 kHz clock | Response per A-027 (receive template: a low-pass only, passes DC). **Modeled by [spec 004](../../specs/004-device-rate-6khz/spec.md) (2026-10-11): the S-005 receive template scaled to the 6 kHz clock (minimum-phase low-pass, unity gain at 765 Hz), modeled on the datasheet, not measured against a unit. Not modeled: the DAC imaging residue above 3 kHz (S-005 out-of-band spurious ≤ −35 dBm0; the emulator's boundary removes it). The 2550–3000 Hz roll-off is a design choice not constrained by any source, reported at 2700 and 2900 Hz.** | Band-limiting, imaging residue |
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
