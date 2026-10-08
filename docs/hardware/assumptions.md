# Assumption Register

When exact hardware specifications are unknown, we make **reasonable, documented assumptions**. This register is the single place they live.

**Rules:**
- Every assumption has a stable ID (`A-###`). Never reuse or renumber an ID. Mark superseded ones instead.
- Code, tests, and specs cite the ID wherever the assumed value is used (e.g. `// A-001`).
- **Confidence:** **High** (strongly implied by primary sources), **Medium** (typical for this class of device or era, consistent with sources), **Low** (an educated guess, a placeholder until evidence arrives).
- **Status:** `assumed` → `verified` (with evidence) or `revised` (link the new value) or `superseded`.
- Changing an assumed value changes emulator output. Note it in [CHANGELOG.md](../../CHANGELOG.md).

| ID | Stage | Assumption | Rationale / sources | Confidence | How to verify | Status |
|----|-------|-----------|---------------------|-----------|---------------|--------|
| A-001 | ADC/DAC | Internal sample rate is **8 kHz**. | MSM7702 is a telephone voice-band codec (S-002, S-003), and 8 kHz is the standard rate for that class. | High | Datasheet clocking section; spectrum of a real capture (nothing above 4 kHz). | assumed |
| A-002 | ADC/DAC filters | Record and playback passband is roughly **300–3400 Hz**, with telephone-style (ITU-T G.712-like) band-pass filter shapes. | MSM7702 integrated filters (S-003). Wider frequency claims in S-004 are inconsistent with this codec. | High (band) / Medium (exact shape) | Datasheet filter templates; swept-sine capture through a real unit. | assumed |
| A-003 | ADC/DAC | PCM coding format (linear vs. µ-law/A-law companded, bit depth) is **TBD**. | Not yet extracted from S-003. | — | Read MSM7702 datasheet. | open |
| A-004 | Speech codec | Recordings are compressed with a **low-bitrate CELP-family speech codec**, in the region of ~4.8 kbps (e.g. comparable to FS-1016 CELP). | S-004 claims CELP. 60 min of audio in mid-1990s flash implies a very low bitrate (S-001). | Low | Identify the DSP/compression IC and flash size in S-002; analyze codec artifacts in real captures. | assumed |
| A-005 | Microphone | Built-in **omnidirectional electret condenser** capsule. | Typical for 1990s dictation recorders; mic preamp topology in S-002 fits an electret. | Medium | Service manual parts list; teardown photos. | assumed |
| A-006 | Mic preamp | Two-transistor discrete preamp (2SB1218A / 2SD1819A) contributes **gain, hiss, and soft clipping at high levels**. Values are TBD. | S-002 parts list. | Medium (topology) / Low (values) | Schematic in S-002; noise floor and clipping behavior in captures. | assumed |
| A-007 | AGC | The record path has **automatic gain control**, with attack and release time constants TBD. | S-004 claims auto-gain; standard for dictation recorders. | Low–Medium | Step-level test-tone captures (look for gain pumping). | assumed |
| A-008 | VAS | Voice Activated System **pauses recording** (drops audio, does not insert silence) when the input stays below a threshold. It has adjustable sensitivity, and the number of levels is TBD (S-004 suggests 5). | S-001 (pause behavior), S-004 (levels). | Medium (behavior) / Low (levels, thresholds, hang time) | Owner's manual VAS section; burst-tone captures. | assumed |
| A-009 | Noise reduction | Some form of noise reduction may exist in the record path. Its type is unknown. **Model as optional, off by default** until evidence appears. | Only S-004 claims it. | Low | S-002 block diagram; idle-noise captures. | assumed |
| A-010 | Speaker | A small dynamic speaker (RAS3P13-U) in a tiny enclosure. Assumed strong low-frequency roll-off, a resonance in the high hundreds of Hz, and audible distortion at high volume. | S-002 part number; typical of small pocket-recorder speakers. | Low | Speaker part data; acoustic measurement of a real unit. | assumed |
| A-011 | Clock | The 8 MHz ceramic resonator has ~±0.5 % tolerance, so per-unit sample-rate and pitch deviation is up to ~±0.5 %. **Nominal by default**, with a deviation parameter available. | S-002 (ceramic resonator); typical resonator tolerance. | Medium | Measure the pitch of a known tone recorded on real units. | assumed |
| A-012 | Output | A separate **2.5 mm earphone output** path bypasses the speaker. | S-004. | Medium | Owner's manual. | assumed |
| A-013 | Power | 3 V supply from 2× AAA. Battery-sag effects (level, distortion, noise) are **not modeled in v1**. | S-001, S-002. | High (supply) | — | assumed |
