# Sources

Evidence about the Panasonic RR-DR60 and its components. Every modeled behavior must trace to a source here (`S-###`) or to an assumption in [assumptions.md](assumptions.md) (`A-###`).

**Reliability scale:** **Primary** (manufacturer documentation, datasheets, measurements of real units) > **Secondary** (reputable technical writing) > **Anecdotal** (forum posts, marketing, community lore).

| ID | Source | Type | Reliability | Key facts extracted |
|----|--------|------|-------------|---------------------|
| S-001 | [Panasonic RR-DR60 Operating Instructions (ManualsLib)](https://www.manualslib.com/manual/304890/Panasonic-Rr-Dr60.html) | Owner's manual | Primary | Up to 99 recordings, ~60 min total recording time; IC flash memory; Voice Activated System (VAS) pauses recording when no sound is detected; 2× AAA batteries; ~10 h recording / ~8 h playback battery life. |
| S-002 | [Panasonic RR-DR60 Service Manual (ManualsLib)](https://www.manualslib.com/manual/2950781/Panasonic-Rr-Dr60.html) | Service manual | Primary | Codec IC **OKI MSM7702-02MS**; mic amplifier transistors **2SB1218A** and **2SD1819A**; 8 MHz ceramic resonator (RSXY8M00M07T); 32 kHz crystal (RSXC32K7L04T); speaker part **RAS3P13-U**; LCD RSL5213-T; 3 V supply. *Not yet fully reviewed:* specs table, block diagram, schematic, flash/DSP IC part numbers. |
| S-003 | OKI MSM7702 datasheet ([mirror](https://mpdf.jiepei.com/MSM7702-9605035.html)) | Datasheet | Primary | Single-channel voice-band CODEC for 300–3400 Hz signals with integrated A/D and D/A filters; single 2.7–3.8 V supply; low-power, designed for digital telephone terminals. *Not yet fully reviewed:* coding format (linear vs. companded), filter templates, gain/noise specs. |
| S-004 | [Higgypop: "Panasonic RR-DR60: A Legendary EVP Recorder"](https://www.higgypop.com/news/panasonic-rr-dr-60/) | Article | Secondary / anecdotal | Launched mid-1990s; claims CELP speech codec; claims onboard noise reduction and auto-gain; 2.5 mm earphone socket; frequency-range claims vary widely (some contradicted by S-003). Contains clear errors (e.g. "samples six times per second"), so use only as leads. |

## To find / review next

- [ ] Full read of S-002: block diagram, schematic, the IC list (which chip does speech compression? flash size?), and the specifications table.
- [ ] Full read of S-003: PCM format, filter response templates, idle-channel noise, gain settings.
- [ ] The flash memory capacity. With the 60 min recording time this gives the codec bitrate (see A-004).
- [ ] Real-unit captures: test tones, sweeps, noise, and speech, recorded and played back through a working unit. See the "Hardware evidence" issue template.
