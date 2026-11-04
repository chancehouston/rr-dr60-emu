# Survey: specifications of other Panasonic recorders and their parts, as candidate emulator components

Compiled 2026-11-04 (engine 0.95.0) at the owner's request: a breadth survey of the highest-quality specifications of other Panasonic voice recorders and of the parts inside recorders of the period, rated by **implementability** (does the document alone let an engineer model the stage in code?). The purpose is exploration toward either a new whole-device emulation or a composite "new" recorder assembled from well-documented stages, each landing as an insertable component (spec 024's slot) or as a selectable device model. Three research agents wrote the three parts below; this front section is the fold-in.

**This is not the source register.** Nothing here has passed the register's review ([`../sources.md`](../sources.md)); cite a register row, not this file, in specs and code. It extends [`research-sources.md`](research-sources.md) and does not repeat it. Every URL in the parts was fetched on 2026-11-04 unless marked **not verified**; figures are quoted as the sources state them; an agent's own reading of a schematic says so.

## Findings

1. **Four Panasonic IC-recorder siblings of 1999–2001 have full service manuals** on the Internet Archive, in the same collection and 600 dpi format as the RR-DR60's S-002: RR-QR80 (AD9909210C5), RR-QR100/200 (AD0107131C2), RR-QR400 (AD0107132C5) and the SD-card RR-XR320 (AD0102040C8). Each has a complete schematic, a parts list with every R and C value, IC pin tables and a block diagram. The QR80 and XR320 schematics were checked at full resolution and every value is legible. Their analog stages (NJM2107 mic preamp with a switched sensitivity, playback low-pass, volume and power amplifier, the XR320's switchable playback "by-pass filter", a probable FET-shunt ALC on the QR80 with a ≈ 2.2 s release) are **implementable from the documents alone**. Their codec is identified (OKI ML2301GA, ML2302) and only partly documented: the ML2302 digest gives modes, 4.0–12.8 kHz rates, a 14-bit converter and a voice-level detector, not the ADPCM2 algorithm, the filter response or the detector threshold; no ML2301 document exists online. The codec clock is 16 kHz in HQ and 8 kHz in SP/LP (the QR100/200/400 waveform charts). The QR80's two codec oscillators (8.19 / 11.06 MHz, ratio ≈ 1.35) behind a selector suggest a clock-switched fast playback with the pitch rising, unlike the RR-DR60; an inference, no document states it.
2. **The OKI ADPCM recorder family is implementable end to end as a codec component**: the MSM6588 / MSM6688 datasheets (3- or 4-bit ADPCM at 4.0 / 5.3 / 6.4 / 8.0 kHz, 12-bit converters, a 4th-order switched-capacitor low-pass at 0.4·fs with a plotted curve, the full-scale window, voice-triggered start thresholds) plus Dialogic's 1988 OKI-ADPCM note with the step tables. That is the public 4-bit mode the ML2301/ML2302 siblings would run under one assumption (their ADPCM2 variant is unpublished; MAME's untested MSM9810 decoder and two OKI patents are the leads). The OKI MSM7570 / MSM7540 give G.726 / G.721 with full transmit and receive loss masks and tolerances, and the MSM7570 documents a VOX with comfort noise.
3. **Panasonic's own service CD-ROMs (TSN "Technical Service Navigation", 2000–2002) are on the Internet Archive** and hold born-digital service manuals of tape voice recorders, with **vector schematics whose part values and DC voltages extract as text**: RQ-L31E (2002; rated 5: 4.8 / 1.6 cm/s, wow and flutter 0.12 % WRMS, record/playback response 250–6300 Hz, DC bias with magnet erase, an adjustment table with test-tape frequencies and a 3000 ± 50 Hz speed target), RQ-L30 (2001), RN-305 / RN-405 microcassette (2000; 2.4 / 1.2 cm/s, clean schematics, no mechanism figures). The ISO images need `pycdlib`; macOS `bsdtar` and `hdiutil` cannot read them.
4. **Three recorder ICs make whole tape stages implementable**: the Sanyo **LA4168M** (pre-amp, ALC with a 40 dB minimum / 67 dB typical width starting at −74 dBm, voice switch at −85 dBm with 6 dB hysteresis, every gain and clip level, BTL power amp, motor governor K = 50 with drift coefficients; used in the RN-305/405, RN-502, RQ-L30/L31), the Panasonic **AN7086S** (VAS at 14 µVrms, ALC width 1.5 %, 350 mW) and the **AN6612** governor (≤ 10 rpm/V, −0.035 %/°C, example motor constants; the pair used in the RQ-L305 / RQ-L340).
5. **Transducers and amplifiers with fit-able data**: Panasonic WM-61A (flat), WM-034 (+3 dB near 5 kHz, −10 dB at 16 kHz), WM-55D103 noise-cancelling capsule (two curves at 1 and 50 cm: a distance-dependent low cut); the Dayton CE28MB-8 28 mm speaker with full Thiele–Small parameters (a modern part at the RR-DR60's speaker size) and the CUI CLS0281MP 28 mm Mylar cone's chart-recorder curve (Fo 850 Hz) as a period-typical cross-check; TI LM4871 / LM4880, Philips TDA7050 amplifiers with curves. Nisshinbo publishes a 2026-07-29 NJM2113 datasheet newer than the register's S-006.
6. **Gaps common to every family**: no S/N with a stated weighting (Panasonic's 35–38 dB IC-recorder figures and the tape "WRMS" wow-and-flutter figures come without methods, one JIS exception), no response at the slow tape speeds, no head or motor data beyond the AN6612's example, no teardown of any Panasonic recorder, no FCC filings (Part 15 devices without an FCC ID), no document for the RR-DR60's own capsule (RJM0016), no sibling using its MN1931712BB DSP, and no 2004-and-later IC recorder whose codec is identified (Panasonic in-house part codes).

## Candidates, by what the documents support

| Candidate | Shape | Implementable now | Needs one assumption | Describable only | Fit to the engine |
|---|---|---|---|---|---|
| **RR-XR320** (2001) | whole device | mic preamp, sensitivity switch, playback low-pass, by-pass filter, volume, power amp; 16 kHz HQ clock; figures 400–6000 Hz HQ, 200 mW | the ML2302 as 4-bit OKI ADPCM at a documented rate; the 0.4·fs LPF from the MSM6588 family; the VAC threshold | ADPCM2, the detector | a second device model: a new rate (16 / 8 kHz), a waveform codec in place of stages 7 and 9, a switched playback filter as a component |
| **RR-QR80** (1999) | whole device or donor | mic preamp (NJM2107, values), the FET-shunt ALC (≈ 2.2 s release; attack by the source impedance), power amp (NJM2135) | the ALC's reading as a limiter (the schematic says "switching"); the clock-switched speed control | the ML2301 | the ALC as a component before the limiter (the slot's position); a pitch-rising speed mechanism as a third speed mode |
| **RR-QR100/200/400** (2001) | whole device | as the QR80 plus the documented clock per mode (16 / 8 / 8 kHz) | the codec | — | as the XR320 |
| **OKI ADPCM recorder codec** (MSM6588 / 6688 + Dialogic tables) | component (stages 7 and 9) | the whole coder: 3/4-bit ADPCM at 4.0–8.0 kHz, the 0.4·fs LPF, the converter window, voice-triggered start | — | — | a codec component; the memory's stored form gains a second encoding (one encoding per memory, spec 008) |
| **G.726 codec** (MSM7570 masks) | component | the standard plus the loss masks with tolerances; VOX with comfort noise | — | — | a codec component of another class (cordless phone) |
| **RQ-L31E** (2002) mini-cassette | whole device | mic amp, ALC, voice switch, EQ amp, power amp and clip, governor, DC bias, two speeds, the adjustment targets, W&F 0.12 % WRMS, 250–6300 Hz | the W&F spectrum (the method is unstated), tape noise and head response | — | a tape device model: a new stage class (speed modulation, bias/EQ, tape noise); the ALC and voice switch map onto stages 3 and 5 |
| **RN-305 / RN-405** (2000) microcassette | whole device | the electronics (LA4168M + schematic), 2.4 / 1.2 cm/s | every mechanism figure | — | as the RQ-L31 with less evidence |
| **LA4168M stage** | component | ALC, voice switch, power amp, governor (from the datasheet alone) | — | — | an ALC-plus-gate component; a different limiter character than A-028 |
| **WM-034 / WM-55D103 / WM-61A** capsules | component (stage 1) | the response curves (±1 dB by eye), sensitivity, S/N | the hiss level per capsule | — | capsule variants with their own A-IDs; WM-55D103 adds a talker-distance parameter |
| **28 mm speaker from Thiele–Small** (CE28MB-8, CLS0281MP) | component (stage 12) | the parameters and a curve | enclosure and baffle | — | a physically derived replacement for the A-010 template |
| **ISD ChipCorder** (ISD2500 / 4004) | device class | the 5-pole filters per rate (3.4 / 2.7 / 2.3 / 1.7 kHz), THD | the storage noise | — | a third recorder class of the era |
| RR-US450 / US570 (2007 / 2009) | donor | the analog front ends | — | the codec | front-end components only |
| RN-109, RN-Z36, RQ-330, RQ-2102 and others | — | — | — | figures only | not without the service manuals |

## How this could proceed (for the owner's decision, nothing started)

- **The cheapest first component** with the most complete evidence is the **OKI ADPCM codec** (candidate 4): algorithm, rates and filter all documented, a bit-plausible model rather than a class match, and the natural codec of the 1999–2001 siblings. It would need a second stored-form encoding in the memory and its own golden.
- **The cheapest first device** is the **RR-XR320** or **RR-QR80**: the analog stages derive from the schematic as the RR-DR60's did (`tools/mic-chain` and `tools/output-chain` are the pattern), the codec rides on candidate 4 under one stated assumption, and Panasonic's figures (band, S/N, output) are the check targets. The engine would need a device rate other than 6 kHz (today a constant, A-026) or a per-model rate.
- **The tape class** (RQ-L31E, RN-305/405) has the best single documents of all (vector schematics, adjustment targets) but needs stage classes the engine does not have (speed modulation, bias and EQ, tape noise) and its mechanism figures come without methods.
- Before any of this becomes a spec: a reading pass over the QR100/200/400 and XR320 schematics at full resolution, the RR-QR240 service manual (elektrotanya archive, not opened; order no. AD9909225C5 from the TSN supplement), the two OKI ADPCM2 patents, and the RQ-L305 / RQ-L340 ManualsLib schematic pages.

## Access tips learned (to merge into `research-sources.md` when a part of this is taken up)

- The Internet Archive's `advancedsearch.php?q=identifier%3Amanual_RR*PANASONIC*&fl[]=identifier&rows=300&output=json` route lists the eleven Panasonic RR service-manual items; read schematics from the `_jp2.zip` at full resolution (the OCR text carries no schematic values).
- The TSN service CD-ROM ISOs on the Internet Archive need `pycdlib` to extract; `bsdtar` and `hdiutil` fail. PyMuPDF reads the Japanese owner's manuals that `pypdf` cannot.
- `datasheetbank.com/pdf-view/…` and `datasheetq.com/pdf-view/…` return real PDFs to `curl` with a browser user agent while WebFetch gets 403.
- Dead: `servlib.com` no longer resolves; the elektrotanya archives have no previews (RAR only); ROHM/LAPIS codec PDF paths and the NJM2070 vendor paths return 404; TDA1308 could not be verified (NXP 404, alldatasheet 403).

---


## Part A. Panasonic / Matsushita IC recorders as whole devices


Breadth survey, 2026-11-04. It extends `docs/hardware/research/research-sources.md` § 2 and § 13 and the register `docs/hardware/sources.md`. Items already listed there get a one-line pointer only. Every URL below was fetched or downloaded on 2026-11-04 unless marked **not verified**. Figures are quoted as the source states them. Where a statement is my own reading of a schematic or an inference, it says so.

Rating scale (owner's refinement: **implementability**, meaning whether the document alone lets an engineer model a stage):
- **5**: full legible schematic with part values plus the codec/DSP identified with a datasheet or algorithm description, plus measured figures.
- **4**: full legible schematic with part values and measured spec figures, but the codec only partly documented.
- **3**: schematic or parts list present but incomplete, illegible or with unidentified (in-house-coded) ICs; spec figures present.
- **2**: spec sheet or owner's manual with measured figures (response, S/N, output, modes) only.
- **1**: feature lists, listings, unverified or anecdotal.

---

### 1. Summary

1. **The best-documented siblings are the 1999–2001 OKI-codec generation: RR-QR80 (1999), RR-QR100/200 and RR-QR400 (2001), and the SD-card RR-XR320 (2001).** All four have full service manuals on the Internet Archive in the same `manual_*_SM_PANASONIC_EN` collection as the RR-DR60's S-002. Each has 600 dpi page images (`*_jp2.zip`), a full schematic, an electrical parts list with R/C values, terminal-function tables and a block diagram. For RR-QR80 and RR-XR320 I checked that the schematic values are legible at full resolution.
2. **Their codec is identified: OKI ML2301GA (RR-QR80/100/200/400) and OKI ML2302 (RR-XR320).** These are OKI "recording and playback LSIs" with OKI ADPCM/ADPCM2, a built-in 14-bit ADC/DAC, an LPF and voice-level detection (VAC). The ML2302 has a 24-page datasheet digest (FEDL2302DIGEST-05, 2004). No ML2301 document was found. Their character differs from the RR-DR60's 4 kbit/s CELP at 6 kHz: waveform coding with no CELP artifacts. The service manuals give the codec sample clock as **16 kHz (HQ), 8 kHz (SP, LP)** for the QR100/200/400. The RR-XR320 page states "16kHz Sampling Frequency (HQ Mode)".
3. **Implementable from documents alone (analog stages):** for RR-QR80 and RR-XR320 (and by the same format very likely RR-QR100/200/400, not inspected at full resolution):
   - the microphone preamplifier: an NJM2107 op-amp, with every resistor and capacitor value printed;
   - the mic-sensitivity switch (a transistor switching the gain network);
   - an input-side FET stage that reads like a diode-rectified ALC (RR-QR80: Q14 2SK1958 with D6/D7, C19 1 µF, R12 2.2 MΩ; my reading, drive not fully traced);
   - the playback low-pass and power amplifier (NJM2107/NJM2135 with values);
   - on RR-XR320, a FET-switched playback "BY-PASS FILTER";
   - the plug-in-power mic input (0.56 mV), earphone and speaker figures.
4. **Implementable in part (codec):** the 4-bit OKI ADPCM algorithm is public (the Dialogic/OKI ADPCM specification, used by ffmpeg, libsndfile and spandsp). OKI's **ADPCM2** is not published. MAME carries an "ADPCM2" decoder for the OKI MSM9810, marked "not thoroughly tested", and cites Oki patents (JP3398457B2, JPH0918425A) as the likely description. Whether the ML2301/ML2302's ADPCM2 is the same is **unknown**. The codec's internal LPF response and the VAC threshold are not given in the digest. So the codec is describable, plus implementable if the 4-bit OKIADPCM mode is assumed.
5. **The speed control is probably different from the RR-DR60's.** RR-QR80 has two codec master oscillators (8.19 MHz and 11.06 MHz) behind a TC7WH157 selector, a "Fast clock oscillator control" microcontroller pin and a "CODEC clock select" pin. That suggests fast playback by raising the codec clock (11.06/8.19 ≈ 1.35, pitch rising with speed). This is an inference from the schematic, not a documented statement.
6. **Describable only (codec unidentified):** RR-QR160E/170/180, RR-US395 (2004–2006), RR-US050 (2005), RR-US450/470/475 (2007) and RR-US570/590 (2009). Their service manuals exist (ManualsLib, archive.org), but the main LSI carries Panasonic in-house codes (e.g. `C2HBBJ000042` "CODEC" on RR-QR160E). Their analog front ends are modelable from the schematics.
7. **The later MP3/linear-PCM models are spec-sheet only:** RR-US300, RR-XR800, RR-XS3xx/4xx, RR-RS150/250, RR-SR30, RR-XP00x. Their character is a generic MP3 recorder, so they are of low value for emulation.
8. **Most promising, in order:**
   - **(a) RR-XR320**, as a whole device: ML2302 datasheet, legible schematic, distinct 400–6000 Hz HQ band, a switched playback filter, a 3-step speed switch.
   - **(b) RR-QR80**, as a whole device or as a component donor: its ALC and mic amp, the clock-switch speed control, and the RR-DR60's direct predecessor-era sibling (1999).
   - **(c) RR-QR100/200/400**: a documented sample clock per mode (a waveform chart) and Digital VAS.
   - **(d) RR-US450/RR-US570**, as analog-front-end donors only.
9. Panasonic's published measured figures (response, S/N, output) are consistent across the families (HQ ≈ 300–5000 Hz, S/N 35–38 dB, built-in mic) and serve as check targets.
10. **No teardown, FCC filing (these are Part 15B/DoC devices) or ML2301 datasheet was found.** No sibling uses the RR-DR60's MN1931712BB DSP, as far as any document found shows.

---

### 2. Table of models

SM = service manual, OM = owner's manual, SS = spec sheet / product page. The rating is the best implementability rating of the available documents (scale above).

| Model | Years (doc ©) | Codec / DSP (as the SM names it) | Rates / modes / response | Storage | Notable features | Docs | Rating |
|---|---|---|---|---|---|---|---|
| RR-DR60 | 1998 | MN1931712BB + MSM7702 (S-002/S-003) | 6 kHz CELP | 16 Mbit | (the emulated device) | already listed (S-001, S-002, S-012, S-013) | — |
| RR-QR80 | 1999 (SM AD9909210C5) | **OKI ML2301GA** (IC2, "CODEC"); MN101C16ACB3 system controller; 8.19 MHz + 11.06 MHz codec clocks via TC7WH157; NJM2107 mic amp; NJM2135 power amp | 450 Hz – 5.0 kHz (−6 dB, ext mic → HP, HQ); S/N 37 dB (built-in mic, HQ); 110 mW | HN29W6411TT (64 Mbit); 60 min (US page) | LP/SP/HQ; MIC SENS H/L; "VAS voice detection signal input" pin; SLOW–FAST playback; ALC-like FET stage (my reading) | SM 27 pp (IA, 600 dpi; ManualsLib 28 pp); OM (JP, shared with QR240); US page (already listed) | **4** |
| RR-QR240 | 1999–2000 | not verified (SM on elektrotanya as RAR, no preview); same OM and same response spec as QR80 | 450–5000 Hz (−6 dB, HQ); S/N 37 dB (OM) / 58 dB (JP spec sheet, already listed) | 32 MB | LP/SP/HQ, timer, VOS, SLOW–FAST | SM (elektrotanya RAR, not opened); JP OM; spec sheet (already listed) | 2 (3 if the RAR is the SM) |
| RR-QR100 / QR200 | 2001 (SM AD0107131C2) | **OKI ML2301GA** (IC2; parts list calls it "IC, MICRO COMPUTER"); MN101C16ACD1/ACE; 8.19 / 11.06 MHz ceramic oscillators; NJM2107 mic amp + NJM2107 power amp + C0ABZA000033 power amp | 300–4,300 Hz (HQ); S/N 38 dB (HQ); 200 mW; codec VCK **16 kHz HQ / 8 kHz SP / 8 kHz LP** (waveform chart) | 64 Mbit (QR100) / 128 Mbit (QR200) | LP 100/200 min, SP 31/65, HQ 15/32 min; Digital VAS; 4 folders × 99 | SM 45 pp (IA; ManualsLib 46 pp); OM (IA, ManualsLib); US pages (already listed) | **4** |
| RR-QR400 | 2001 (SM AD0107132C5) | **OKI ML2301GA**; MN101C16ACF; same clocks | 300–4,300 Hz (HQ); S/N 38 dB; 200 mW (SM) / 140 mW EIAJ (JP OM); VCK 16 / 8 / 8 kHz | 256 Mbit | LP 420, SP 131, HQ 65 min; VAS (VOS in JP) | SM 38 pp (IA); JP OM; US page (already listed) | **4** |
| RR-XR320 (JP RR-XR330) | 2001 (SM AD0102040C8) | **OKI ML2302** (IC401, "CODEC"); MN102H60GRA system control; MN66817SDLD (IC101); NJM2107 ×2 (mic amp, power amp); 16.3 MHz + 32.7 kHz clocks | 400–6000 Hz (HQ); 200 mW (SM) / 140 mW (JP OM); HQ "16kHz Sampling" (US page); files: HQ 472 KB/min, SP 236 KB/min (SM's software section; LP OCR-garbled) | SD card | "ADPCM2 codec" VM1 files (SD Voice Editor); PLAYBACK SPEED switch SLOW/NOR/FAST; FET-switched "BY-PASS FILTER" in playback; VAS input pin | SM 45 pp (IA, 600 dpi, incl. PC software manual); OM (ManualsLib, IA/manualzilla); JP OM RR-XR330; US page (already listed); ML2302 datasheet digest | **4–5** |
| RR-QR120 / US320 / US321 | c. 2002 | not found | — | — | — | OM (IA `manual_RRQR120_OM_PANASONIC_EN`, ManualsLib) | 1 |
| RR-QR150 | c. 2003 | not found (SM on elektrotanya RAR, not verified) | 350–4300 Hz (HQ); S/N 38 dB (HQ); 140 mW (JEITA) | — | 3-step speed; MIC SENS H/L; VOS | JP OM (verified) | 2 |
| RR-QR160E | 2004 (SM AD0402034C2) | `C2HBBJ000042` (IC2; pin table: "chip select [CODEC(IC2)]"); MN101C38CLH; 4.096 MHz codec clock | 240–5,100 Hz (HQ) / 230–3,400 Hz (FQ/SP); S/N 36 dB (HQ); 300 mW | `C3FBQC000014` flash | HQ / FQ / SP; mic sens H/L; level meter | SM 45 pp (IA, 600 dpi) | 3 |
| RR-QR170 / QR180 / RR-US395 | 2006 (SM AD0602003CE, AD0604004CE) | not identified (in-house codes) | 240–5,100 Hz (HQ) / 230–3,400 Hz (FQ/SP); S/N 36 dB; 300 mW; earphone 0.5 mW | flash | — | SM 29 pp (ManualsLib, E/P versions); SM 6 pp (IA `RRQR170P`, ManualsLib; schematic but no electrical parts list) | 3 |
| RR-US050 / 070 / 090 | 2005 (SM AD0502008C1) | not identified | 280–5,200 Hz (HQ) / 270–3,400 Hz (FQ/SP); S/N 35 dB (stereo HQ); 350 mW (SM) / 270 mW JEITA (JP OM) | — | stereo; 5-step speed control "at a natural pitch" (already listed); playback EQ "余分なノイズをカット" | SM 9 pp (ManualsLib; elektrotanya P/PC, not verified); JP OM (verified); OM (IA) | 3 |
| RR-QR005 / RR-US007 / US009 | c. 2005 | not found | 300–4000 Hz (HQ); S/N 38 dB; 60 mW | — | single-cell; 3-step speed | JP OM (verified) | 2 |
| RR-US450 / US470 / US475 | 2007 (SM MD0702017CE, MD0703037AE) | MN101C93KMA; mic amp `C0ABAB000112`; NAND `C3FBTC000030` | 300–5,000 Hz (HQ) / 300–3,400 Hz (FQ/SP); S/N 37 dB; 300 mW | 128 MB | — | SM 40 pp (US450P, ManualsLib; schematic 4 sheets, parts list with values); SM 20 pp (US475E, supplement) | 3 |
| RR-US570 / US590 | 2009 (SM, code PSG0902009CE) | IC201 unnamed; speaker amp `C1AB00003089` | not in the extracted text | NAND flash | doctor mode, firmware update; block diagram, voltage/waveform charts | SM 63 pp (ManualsLib) | 3 |
| RR-XS400 / XS410 | 2011 (SM AD1107004CE) | not given | — | — | — | SM 3 pp (supplement only, ManualsLib) | 1 |
| RR-US300 (JP) | c. 2010 | — | MP3 44.1 kHz HQ 128 kbps / 16 kHz LP 32 kbps; 300–21,000 Hz / 300–6,700 Hz (−10 dB); S/N 41 dB (JIS A) | — | 21-step speed 0.5–2.0× | JP OM (verified) | 2 |
| RR-XR800 | c. 2008 | — | PCM-44.1 and MP3 32–320 kbps; 30–22,000 Hz (PCM) | — | speed control; VOS | JP OM (verified); OM 60 pp (ManualsLib) | 2 |
| RR-XS3xx/4xx, RR-RS150/250, RR-SR30, RR-XP007–009, RR-US330, RR-QR210/220 | 2009–2015 | — | linear PCM / MP3 (e.g. SR30: 44.1 kHz, 100–19,000 Hz, S/N 44 dB) | — | — | JP spec pages and OMs (search results; one OM checked) | 2 |

---

### 3. Sources

#### 3.1 Service manuals: Internet Archive (`manual_*_SM_PANASONIC_EN` collection)

These are the same collection and format as S-002. Access is the same as in research-sources.md "RR-DR60 service manual". The item has `<ID>_jp2.zip` (600 dpi page images, 4959 × 7017 px for an A4 page), `<ID>_djvu.txt` (OCR, usable for parts lists and pin tables) and a PDF. Read schematics from the jp2 at full resolution with crops. The OCR text does not carry schematic values. To find the items, run `https://archive.org/advancedsearch.php?q=identifier%3Amanual_RR*PANASONIC*&fl[]=identifier&rows=300&output=json`, which returns 11 Panasonic RR items.

1. **Panasonic RR-QR80 Service Manual, Order No. AD9909210C5** (© 1999). *Service manual; Primary; read (OCR text in full; schematic pages 25–27 inspected at full resolution).*
   - URL: [archive.org/details/manual_RRQR80_SM_PANASONIC_EN](https://archive.org/details/manual_RRQR80_SM_PANASONIC_EN). Also on ManualsLib as a 28-page viewer: [manualslib 2614883](https://www.manualslib.com/manual/2614883/Panasonic-Rr-Qr80.html) (listed on the brand page; the document page itself was not opened).
   - Contains (27 pages): specifications (110 mW; 450 Hz–5.0 kHz −6 dB ext mic → HP, HQ; S/N 37 dB built-in mic HQ; mic jack 0.56 mV; earphone 200 mV 16 Ω; speaker 2.8 cm 8 Ω); operation check mode; disassembly; schematic notes (voltage legend, "No mark: Recording, ( ): Playback", as on S-002); schematic; both PCB sides; block diagram; the terminal function of IC4 (MN101C16ACB3); the replacement parts list with every R and C value.
   - ICs: IC2 ML2301GA, IC3 TC7WU04FUT2L, IC4 MN101C16ACB3, IC5 TC7W74FUT2L, IC6/IC12 XC61AN2002MR/1802MR (reset), IC7 TC7WH157FUTL, IC8 XC6368B101MR (DC-DC), IC9 HN29W6411TT (flash), IC13 NJM2107FTE1 (mic amp), IC14 NJM2135RTE1 (power amp). X1 8.19 MHz, X2 32.768 kHz, X3 11.06 MHz.
   - Pin table: "IREQ0 VAS voice detection signal input" (pin 38), "CODEC (IC2) clock select signal" (pin 25), "Fast clock oscillator control" (pin 62), "Microphone sensitivity setting" (pin 63), "Power amp active select signal".
   - Schematic, read at full resolution: the built-in mic and mic jack feed R11 2.2K / C16 0.022 / R62 10K → C14 0.1 → NJM2107 (+IN biased by R68/R49 1M, R70 100K); feedback R61 120K, C15 0.47, R10 2.2K, with Q2 DTC144TUA (R9 1K) switching the gain (sensitivity). At the R62 node a Q14 2SK1958T1 FET to ground, gate network C19 1 µF ∥ R12 2.2 MΩ with D6/D7 MA110TX: reads as a rectifier-driven FET-shunt ALC. This is my reading; the schematic labels Q14 "SWITCHING" and the drive source was not traced. Playback: VR1 50K volume, C8 0.33, R65 18K ∥ C62 1000P ∥ C10 100P around the amp, C4 3300P / R4 6.8K, Q5/Q6 earphone-detect switching.
   - Quality: **4**. Every analog stage from mic to speaker is derivable (gains, corners, the ALC time constants from C19·R12 ≈ 2.2 s, with the attack set by the source impedance). The codec chip is identified, but its datasheet is not found (see 3.4).

2. **Panasonic RR-QR100 / RR-QR200 Service Manual, Order No. AD0107131C2** (© 2001). *Service manual; Primary; read (OCR text); schematic not inspected at full resolution.*
   - URL: [archive.org/details/manual_RRQR200_SM_PANASONIC_EN](https://archive.org/details/manual_RRQR200_SM_PANASONIC_EN); ManualsLib 46 pp [manualslib 2983288](https://www.manualslib.com/manual/2983288/Panasonic-Rr-Qr100.html) (listed on the brand page; not opened).
   - Contains (45 pages): specifications (200 mW; 300–4,300 Hz HQ; S/N 38 dB HQ; earphone 3.6 mW 16 Ω); separate schematics and PCB diagrams for QR100 and QR200; block diagram; **voltage/waveform chart with clock frequencies per mode** ("Pulse 40(/CS) 8 ms / 16 ms / 8 ms; Clock 46(VCK) 16 kHz / 8 kHz / 8 kHz" for HQ / SP / LP; SC 500 kHz); terminal functions ("IRQ0 VAS voice detection", "Mic. sensitivity 'H' set up", "Play power amp. drive"); parts list with descriptions (IC2 ML2301GA "IC, MICRO COMPUTER"; IC9 64M bit or 128M bit memory; IC13 NJM2107 MIC AMP; IC14 C0ABZA000033 POWER AMP; IC16 NJM2107 POWER AMP; X1 8.19 MHz ceramic, X3 `H2D102500001` ceramic; the schematic labels 11.06 MHz). Recording times LP 100/200, SP 31/65, HQ 15/32 min.
   - Quality: **4** (same reasoning as RR-QR80; adds the sample-clock-per-mode figure).

3. **Panasonic RR-QR400 Service Manual, Order No. AD0107132C5** (© 2001). *Service manual; Primary; read (OCR text).*
   - URL: [archive.org/details/manual_RRQR400_SM_PANASONIC_EN](https://archive.org/details/manual_RRQR400_SM_PANASONIC_EN). Elektrotanya also lists an RR-QR400 RAR (search result only, not verified).
   - Contains (38 pages): as the QR100/200 manual. IC9 `C3FBRC000005` "256M bit MEMORY"; LP 420, SP 131, HQ 65 min; 300–4,300 Hz HQ, S/N 38 dB, 200 mW; VCK 16/8/8 kHz.
   - Quality: **4**.

4. **Panasonic RR-XR320 Service Manual, Order No. AD0102040C8** (© 2001). *Service manual; Primary; read (OCR text; schematic sheet 4 inspected at full resolution).*
   - URL: [archive.org/details/manual_RRXR320_SM_PANASONIC_EN](https://archive.org/details/manual_RRXR320_SM_PANASONIC_EN).
   - Contains (45 pages): specifications (200 mW; 400–6000 Hz HQ; earphone 3 mW 16 Ω; mic 0.56 mV); a troubleshooting flowchart; operation check mode (FAST PLAY / SLOW PLAY displays); schematic across four sheets; PCB; block diagram; terminal functions of IC301 MN102H60GRA ("FAST", "SLOW" speed inputs, "VASIN VAS signal input", "MICS Microphone sensitivity setting", "FILTER A Bi-pass filter output", "REC LEVEL Recording level signal input", "BATT VOL"); parts list (IC101 MN66817SDLDR, IC202 CY62136VL70B SRAM, IC401 **ML2302**, IC501/502 NJM2107FTE1, IC601 XC6368B101MR; 16.3 MHz and 32.7 kHz oscillators). It also bundles the SD Voice Editor manual: "only VM1 files (ADPCM2 codec) can be played"; "WAV files with a sampling frequency of 8 kHz are converted to SP VM1 files and WAV files with sampling frequencies of 11, 16, or 22 kHz are converted to HQ VM1 files"; file size per minute HQ 472 KB, SP 236 KB (LP value OCR-garbled).
   - Schematic, read at full resolution: IC502 NJM2107FTE1 "AUDIO POWER AMP" (R535/R534 22K, C508 330P, C507 0.1, R537 100); Q501 2SK1958T1 "BY-PASS FILTER" switching C512 0.018 µF / C529 2200P through R525 56 Ω behind R503 3.3K, gate via R520 10K from the controller's FILTER A pin; R536 33K / C528 6800P / C521 0.022 / R501 120K / C522 220P / C504 1000P / R505 270K networks. Every value is legible.
   - Quality: **4–5**. The analog chain is fully implementable, the codec has a datasheet digest (3.4), and the band edge and file rates are stated.

5. **Panasonic RR-QR160E Service Manual, Order No. AD0402034C2** (© 2004). *Service manual; Primary; read (OCR text).*
   - URL: [archive.org/details/manual_RRQR160E_SM_PANASONIC_EN](https://archive.org/details/manual_RRQR160E_SM_PANASONIC_EN).
   - Contains (45 pages): specifications (300 mW; 240–5,100 Hz HQ / 230–3,400 Hz FQ/SP; S/N 36 dB HQ); an operation check that records "a sound source signal (1KHz, −72dB) from External Microphone" for the level meter; schematic; PCB; block diagram; terminal functions ("LSICS chip select [CODEC(IC2)]", "4MCLK CODEC(IC2) Clock select", "SENSE microphone sensitivity high"); parts list (IC2 `C2HBBJ000042`, IC4 MN101C38CLH, IC8 `C0DBAGC00020` DC-DC converter, IC9 `C3FBQC000014`, IC13 `C0ABAB000108`; a 4.096 MHz codec clock in the waveform chart).
   - Quality: **3**. The codec is an unidentified in-house part.

6. **Panasonic RR-QR170P / RR-QR180P / RR-US395PC Service Manual, Order No. AD0604004CE** (© 2006). *Service manual (supplement); Primary; read.*
   - URL: [archive.org/details/manual_RRQR170P_SM_PANASONIC_EN](https://archive.org/details/manual_RRQR170P_SM_PANASONIC_EN); [ManualsLib 2856558](https://www.manualslib.com/manual/2856558/Panasonic-Rr-Us395pc.html).
   - Contains (5–6 pages): specifications (300 mW; 240–5,100 Hz HQ; 230–3,400 Hz FQ/SP; S/N 36 dB HQ; earphone 0.5 mW; 28 mm 8 Ω speaker); a schematic; accessories only in the parts list.
   - Quality: **2–3**.

#### 3.2 Service manuals: ManualsLib (document pages opened)

7. **RR-US395E / US395P / QR170E / QR180E SM, Order No. AD0602003CE** (© 2006). *SM; Primary; partly read (preview text).*
   - URL: [manualslib 4325798](https://www.manualslib.com/manual/4325798/Panasonic-Rr-Us395e.html).
   - Contains (29 pages): test mode, troubleshooting, voltage chart, block diagram ("FLASH MEMORY (REC)"), main schematic, PCB, parts list (resistor values seen); ICs not named in the preview.
   - Quality: **3**.

8. **RR-US450P / PC SM, Order No. MD0702017CE** (© 2007). *SM; Primary; partly read.*
   - URL: [manualslib 4325801](https://www.manualslib.com/manual/4325801/Panasonic-Rr-Us450p.html).
   - Contains (40 pages): voltage and waveform chart, wiring, block diagram, schematic 1–4, PCB, parts list with values (e.g. "C108 F1G1C103A048 0.01 16V"); 300–5,000 Hz HQ, 300–3,400 Hz FQ/SP, S/N 37 dB, 300 mW, 128 MB; IC203 `C0ABAB000112` mic amp, MN101C93KMA, NAND `C3FBTC000030`.
   - Quality: **3**. The analog front end is modelable; the codec is unidentified.

9. **RR-US475E SM, Order No. MD0703037AE** (© 2007). *SM supplement; Primary; partly read.*
   - URL: [manualslib 1258660](https://www.manualslib.com/manual/1258660/Panasonic-Rr-Us475e.html).
   - Contains (20 pages): specifications (300 mW; 300–5,000 Hz HQ; S/N 37 dB; 0.56 mV plug-in-power mic; 20 mm 8 Ω speaker; 128 MB) and parts; it refers to the RR-US470P/PC SM, Order No. MD0702016CE (not found).
   - Quality: **2**.

10. **RR-US050PC SM, Order No. AD0502008C1** (© 2005). *SM; Primary; partly read.*
    - URL: [manualslib 4325815](https://www.manualslib.com/manual/4325815/Panasonic-Rr-Us050pc.html).
    - Contains (9 pages): 350 mW; 280–5,200 Hz HQ / 270–3,400 Hz FQ/SP; S/N 35 dB HQ; schematic 1–4 (analog and digital circuit).
    - Quality: **3**.

11. **RR-US570PP / US570E / US590P SM** (© 2009; the code PSG0902009CE appears on the last contents page). *SM; Primary; partly read.*
    - URL: [manualslib 2219908](https://www.manualslib.com/manual/2219908/Panasonic-Rr-Us570pp.html).
    - Contains (63 pages): doctor mode, firmware update, voltage and waveform charts, an IC201 block diagram, block diagram, schematics 1/4–4/4, PCB, parts lists (IC302 speaker amp `C1AB00003089`).
    - Quality: **3**.

12. **RR-XS400PP / XS410PP SM, Order No. AD1107004CE** (© 2011). *SM supplement; Primary; read.*
    - URL: [manualslib 840387](https://www.manualslib.com/manual/840387/Panasonic-Rr-Xs400pp.html).
    - Contains (3 pages): specifications, controls and the parts list only; no schematic.
    - Quality: **1**.

#### 3.3 Service manuals: elektrotanya (download pages; files not opened)

13. **elektrotanya.com RR-QR and RR-US service files.** *SM archives; probably Primary; listing pages only.*
    - Fetched: [RR-QR240 AUDIO](https://elektrotanya.com/panasonic_rr-qr240_audio.rar/download.html) (RAR 1.1 MB, "No preview item") and [RR-QR80 AUDIO](https://elektrotanya.com/panasonic_rr-qr80_audio.rar/download.html) (RAR 1.0 MB, no preview).
    - Search results only, **not verified**: RR-QR160E, RR-QR150P/PC, RR-QR230P, RR-QR270E/P (PDF 37 pp), RR-QR120P/PC, RR-QR400, RR-QR170E/180E/US395E/US395P, RR-QR100/200, RR-US500P, RR-US050P/PC, RR-US380PC.
    - Access: tick a box to reveal the download; "personal usage only". The **RR-QR240 RAR is the one item of value not available elsewhere** (a 32 MB, 1999–2000 OKI-generation sibling).
    - Quality: unknown; **3** expected if the RAR holds a full SM.

14. **servlib.com** (RR-US065E, RR-US550PP/E, RR-US510E, RR-US590E SM viewers, per search results). **Not verified**: the domain did not resolve (NXDOMAIN) on 2026-11-04. ManualsHive (RR-US475E SM pages) and manualmachine (RR-US050P SM) appeared in results and were not opened.

#### 3.4 Codec and part datasheets

15. **OKI ML2302 "Recording and Playback LSI with Built-in 2-Bit ADPCM2 Supported FIFO", FEDL2302DIGEST-05, issued Dec. 27, 2004** (24 pp; OKI Semiconductor). *Datasheet digest; Primary (for RR-XR320's IC401); read.*
    - URLs: [sourcengine PDF](https://assets.sourcengine.com/datasheets/b5159133-cb1d-49fa-a177-f451c8683fc3.pdf) (downloaded; 28 pp with OKI's name-change cover page); [jiepei mirror, page 1](https://pdf.jiepei.com/ml2302tb-14226846.html?page=1) (lowercase path, as for S-003).
    - Contains:
      - "2-bit OKIADPCM2; 4/5/6/7-bit OKIADPCM2; 4-bit OKIADPCM; 8/16-bit straight PCM; 8-bit OKI Non-linear PCM";
      - "Source oscillation frequency 16.384 MHz"; fsam "4.0 to 12.8 kHz (OKIADPCM2)", "4.0 to 25.6 kHz (8-bit straight PCM)";
      - "Built-in voice level detection function (VAC)" (pin VOXO), with a command "Level detection, noise injection";
      - an electronic volume "(0 dB to –44 dB: –2 dB step, –44 dB to –80 dB: –4 dB step)";
      - "Built-in 14-bit A/D converter and 14-bit D/A converter";
      - "Built-in low pass filter (LPF) (recording side: analog filter, playback side: digital filter)";
      - two mic op-amps; the fsam-by-algorithm tables; the FIFO buffering times.
    - Missing: no ADPCM2 algorithm, no LPF response, no VAC threshold (a "digest": "For full specifications, please contact…").
    - Note: the RR-XR320 SM predates this issue (2001), so the part revision may differ.
    - Quality: **3** (it identifies the codec class and its parameters, but not enough to implement ADPCM2 or the filters).

16. **OKI ML2301 (RR-QR80/100/200/400 IC2)**: **no document found** (searched in standard and extended modes, alldatasheet, the jiepei pattern). Treat the ML2302 digest as the nearest sibling. Not verified that the ML2301 offers the same algorithms.

17. **OKI ADPCM / ADPCM2 algorithm references.**
    - [MAME `okiadpcm.cpp`](https://raw.githubusercontent.com/mamedev/mame/master/src/devices/sound/okiadpcm.cpp) and `okiadpcm.h` (BSD-3-Clause; fetched from GitHub raw; git.redump.net answered 403). *Open-source emulation; Secondary; read (comments).*
      - Contains `oki_adpcm_state` (the MSM6295/5205 4-bit ADPCM, 49-step table) and `oki_adpcm2_state` "used by external ADPCM generators with compatible specs to the OKI MSM9810. TODO: not thoroughly tested", with `s_index_shift = { -2, -2, -2, -2, 2, 6, 9, 11 }`.
      - The header comments list Oki patents, among them JP3398457B2 ("this may cover the 'adpcm2' method") and JPH0918425A ("looks like ADPCM2 maybe?").
      - Quality: **3** for 4-bit OKIADPCM (well established), **1–2** for ADPCM2 (untested, and not shown to match the ML230x).
    - [Dialogic IMA ADPCM (MultimediaWiki)](https://wiki.multimedia.cx/index.php/Dialogic_IMA_ADPCM): **not opened** (search summary only). It describes the OKI/Dialogic 4-bit variant (a 49-entry step table, 12-bit clamp).
    - Patents JP3398457B2 and JPH0918425A: **not opened**. These are the next read for ADPCM2.

18. **JRC NJM2107 single operational amplifier, Ver. 2012-10-30** (5 pp). *Datasheet; Primary (part class); read (page 1).*
    - URL: [szlcsc mirror](https://atta.szlcsc.com/upload/public/pdf/source/20170523/1495533899115.pdf). (The LCSC link returned a JavaScript page.)
    - Contains: "Operating Voltage (V+/V- = ±1.0V to ±3.5V)", "Low Output Saturation (4VP-P at single 5V supply)", bipolar; pin functions.
    - Use: the mic amp and power amp of RR-QR80/100/200/400 and RR-XR320 are this part. Its rail swing on 3 V sets the clip level of both stages.
    - Quality: **4** for that stage.

#### 3.5 Owner's manuals and spec pages

19. **Panasonic Japan manual index, ICレコーダー取扱説明書** (57 models, 43 PDFs). *Index; Primary; read.*
    - URL: [av.jpn.support.panasonic.com/support/audio/manual/rr.html](https://av.jpn.support.panasonic.com/support/audio/manual/rr.html).
    - PDFs follow `http://dl-ctlg.panasonic.com/jp/manual/rr/<name>.pdf` or `https://panasonic.jp/p-db/contents/manualdl/<id>.pdf`.
    - Downloaded and read (specs page): `rr_qr240_qr80`, `rr_qr400`, `rr_xr330`, `rr_xr800`, `rr_qr150`, `rr_us009_us007_qr005`, `rr_us050_070_090`, `rr_us300`.
    - **Text extraction needs PyMuPDF.** pypdf failed on six of eight files. Figures as quoted in the table:
      - QR240/QR80: "実用最大出力 90mW (EIAJ); 周波数特性 450Hz–5,000Hz (−6dB, 外部マイク–インサイドホン, HQ); SN 比 37dB (内蔵マイク, HQ)"; SLOW–FAST speed.
      - QR400: 140 mW EIAJ; 300–4300 Hz HQ; 38 dB.
      - XR330: "周波数特性(EIAJ) 400 Hz – 6,000 Hz (HQ)"; 3-step speed.
      - US050/070/090: 280–5200 Hz HQ / 270–3400 Hz FQ/SP; S/N 35 dB stereo HQ; playback EQ "余分なノイズをカットする"; speed control.
      - US300: "サンプリング周波数 44.1 kHz [HQ 128 kbps] 16 kHz [LP 32 kbps]; 録音方式 MP3"; 300–21,000 / 300–6,700 Hz (−10 dB); S/N 41 dB (JIS A); 21-step speed 0.5–2×.
      - XR800: PCM-44.1 / MP3 32–320 kbps; 30–22,000 Hz.
    - Quality: **2** each. These are the only Panasonic sources that state the speed-step counts per model.

20. **Panasonic Japan spec pages** for RR-XS460, RR-RS150, RR-US330, RR-XP009, RR-XS350, RR-RS250, RR-QR210, RR-XS360 (`https://panasonic.jp/other/products/<MODEL>/spec.html`) and the RR-SR30 OM (`https://panasonic.jp/manualdl/p-db/rr/rr_sr30.pdf`). *Spec pages; Primary; search summaries only, **not verified** by fetch.* Modern MP3/PCM figures (e.g. RR-SR30 44.1 kHz, 100–19,000 Hz, S/N 44 dB JIS A). Quality: **2**; low emulation value.

21. **Internet Archive owner's-manual items.** *OMs; Primary; existence verified through the IA search API, contents not read.*
    - `manuallib-id-2654690` (QR400/200/100), `-2654695` (QR240/QR80), `-2654691` (QR120/US321/US320), `-2654694` (QR81), `-2654692/93` (QR160/QR170), `-2654696` (US006), `-2654702` (US050), `-2654703`–`2654707` (US350/351/360/361/380/395), `-2654711` (US490/470/450), `-2654712/13` (US550), `-2654714` (US591/571/551), `-2654715` (US570), `-2654716` (US590);
    - `manual_RRQR120_OM_PANASONIC_EN`, `manual_RRUS050_OM_PANASONIC_ENGLISH`, `manual_RRUS351_OM_PANASONIC_EN`;
    - `manualzilla-id-7252910` (RR-XR320) and others;
    - `panasonic-rr-us-551`, `panasonic-rr-us-500-ic-recorder`.
    - Quality: **1–2**.

22. **ManualsLib owner's manuals** (brand page [panasonic/voice-recorder](https://www.manualslib.com/brand/panasonic/voice-recorder.html), fetched). It lists OMs for RR-QR120/150/153/160/170/180/230/240/270, RR-US006/050/065/300/310/320/321/350/351/360/361/380/395/430/455/470/490/500/510/550/551/570/571/590/591/750/950, RR-XR320/800 and RR-XS350/400/410/420/450, and the SMs in 3.1–3.2. Quality: **2**.

#### 3.6 Already listed (pointers only)

- RR-QR80, RR-QR240, RR-QR100/200/400 and RR-XR320 US product pages; the RR-QR240 JP spec sheet (S/N 58 dB, 450–5000 Hz); RR-US310, RR-US090/070/050 and RR-QR280 announcements; the ITmedia RR-US050 review: research-sources.md § 2, items 1–9.
- The partner spec-sheet path probe (only RR-DR60 and RR-QR240 exist): research-sources.md "Access tips".

---

### 4. Searched without result, and dead ends

- **ML2301 datasheet:** "ML2301 OKI" (standard and extended), alldatasheet, and the jiepei URL pattern returned only ML2302, ML2308, ML2500 and an unrelated ML67Q2301 ARM MCU.
- **An ADPCM2 algorithm description from OKI:** none. The ML2302 digest omits it; only MAME's untested MSM9810 decoder and two patents (unread) point to it.
- **RR-DR50 / RR-DR55 / RR-DR70:** no page anywhere (consistent with research-sources.md's dead end). No Panasonic model other than the RR-DR60 is documented with the MN1931712BB DSP or a CELP coder.
- **Teardowns or die photos of any Panasonic IC recorder:** none (iFixit, TechInsights and EDN results concern other products). A 2010 notice of malware in the flash of RR-US571/RR-XS600 (ScanNetSecurity) is not chip evidence.
- **FCC filings:** none found. These recorders are Part 15 unintentional radiators (the OMs carry a Part 15 statement, no FCC ID), so no grantee ACJ filing is expected.
- **servlib.com:** NXDOMAIN on 2026-11-04 (it previously hosted RR-US065E/510E/550/590E SM viewers).
- **elektrotanya:** no previews for the RAR files; the contents are not verified.
- **The ManualsLib text layer** gives scrambled spec tables and no schematic content. Use the IA jp2 images for any schematic work.
- **The RR-US470P/PC base SM** (Order No. MD0702016CE, cited by the RR-US475E supplement): not found online.
- **pypdf** extracted no text from most 2000s Panasonic JP OMs; PyMuPDF works.
- **The RR-QR160E-generation in-house codes** (`C2HBBJ000042`, `C0ABAB000108`, `C0ABAB000112`): no cross-reference to a manufacturer part was found (not searched exhaustively).

---

## Part B. Parts of 1990s–2000s digital voice recorders


Compiled 2026-11-04 for rr-dr60-emu. This is a breadth survey and a first step toward swappable components. **It is not the source register.** Nothing here has passed the register's review (`docs/hardware/sources.md`), so cite a register row, not this file, in specs and code.

It extends `docs/hardware/research/research-sources.md` and does not repeat it. Already listed there or in the register: the MSM7702 (S-003), MSM7717 (S-005), MSM7578/7579 (§3.3), the 1987 OKI Voice Synthesis LSI Data Book (§3.4, recorded there as "not read"; partly read below), the MN1932801/MN1933211 and MN1931712 catalog traces (§4), the MN101C51F manual (§4.5), the DS2132A and DSP Group TAD chips (§8), the NJM2113 (S-006) and the MC34119 (S-015), and the Torex/power parts (§9).

**Verification.** "Verified" means the URL was fetched on 2026-11-04 and the document opened (curl + PyMuPDF text extraction, with the curve pages rendered and read as images). The figures quoted were read from the documents, not from search snippets, unless an entry says otherwise. A URL that could not be opened is marked "not verified" with the reason.

**Rating scale (implementability first, per the owner's refinement).** Each rating asks one question: does the document alone let an engineer model the part?
- **5:** Implementable. Codec: the algorithm or a standard reference, the companding, the bit rates, and filter templates with tolerances. Transducer: a readable response curve plus sensitivity and impedance, or Thiele–Small parameters. Amplifier: gain, the clip level against supply, and THD/response curves.
- **4:** Mostly implementable. One element is a reasonable assumption (for example a curve read by eye, or an unstated filter order).
- **3:** Partly implementable. The headline numbers are enough for a coarse model, and the rest must be assumed.
- **2:** Describable only. The functions and rates are given, but the algorithm or curves are not.
- **1:** A listing or a summary only.

---

### 1. Summary

1. **Implementable from the documents found:**
   - The **OKI ADPCM recorder family** (MSM6588 and MSM6688 datasheets, plus Dialogic's 1988 OKI-ADPCM algorithm note). These give the 3/4-bit ADPCM algorithm with its step tables, the sampling rates (4.0/5.3/6.4/8.0 kHz), a 4th-order switched-capacitor LPF at 0.4·fs with a plotted curve, 12-bit converters, the full-scale window, and a voice-triggered start at stated thresholds.
   - The **OKI MSM7570 / MSM7540 ADPCM codecs** (G.726 / G.721 plus full transmit and receive loss templates).
   - Three **Panasonic WM capsules** with readable response curves: WM-61A (flat), WM-034 (+3 dB near 5 kHz), and the WM-55D103 noise-cancelling capsule (a distance-dependent low-frequency cut).
   - The **Dayton CE28MB-8** 28 mm speaker (full Thiele–Small parameters).
   - The **TI LM4871 / LM4880 / LM386** and **Philips TDA7050** amplifiers (gain, power and clip against supply, and THD curves).
2. **Partly implementable:**
   - The ISD ChipCorder ISD2500/ISD4004: analog multilevel storage, a 5-pole anti-alias filter, and a stated −3 dB pass band per sampling rate, but no noise model.
   - The Panasonic EAS-4P15SA/45P30S speakers (40–45 mm, small curves only).
   - The CUI/Same Sky CLS0281MP 28 mm Mylar speaker (a chart-recorder curve; no Thiele–Small data).
   - The KA2209 dual amplifier (tables only).
3. **Describable only:**
   - The OKI MSM6788 sub-band (SBC) recorder: five bands, 96-sample frames, 7.5–16 kbit/s at 6/8 kHz, but no band filters or bit allocation.
   - The OKI MSM6322 17-step pitch converter.
   - The NEC µPD7759 (playback only).
   - The Samsung KS5911 and Holtek HT8659 ADM recorders (summaries only).
   - The Toshiba TC8830F (no datasheet found).
4. **The five most promising components:**
   - **(a) An "OKI ADPCM recorder" codec component.** It replaces the CELP stages 7 and 9 with a sibling-class device of the period: 4-bit ADPCM at 6.4 or 8 kHz, or 3-bit at 5.3 kHz, behind the 0.4·fs LPF. The algorithm, the rates and the filter are all documented, so it could be bit-plausible (Dialogic tables), not only a class match.
   - **(b) A G.726 32 kbit/s "cordless-phone grade" codec component** from the MSM7570. The ITU standard is public and the datasheet gives the loss masks with tolerances.
   - **(c) Alternative mic capsules:** WM-034 (presence lift), WM-55D103 (proximity-dependent cut), and WM-61A (flat reference). Each curve is readable enough to fit within about ±1 dB.
   - **(d) A 28 mm speaker from Thiele–Small parameters** (CE28MB-8: Fs 623 Hz, Qts 1.63, Re 7.54 Ω, Le 0.45 mH). It is physically derived, unlike the current template (A-010). The CLS0281MP curve is a period-typical Mylar cross-check.
   - **(e) An ISD-style analog-storage recorder:** sample-and-hold at 8/6.4/5.3/4 kHz, with 5-pole filters at 3.4/2.7/2.3/1.7 kHz. It is a different recorder class of the same era, but its storage noise would be an assumption.
5. Caveat: none of these parts is known to be in the RR-DR60. They are siblings or contemporaries. "Which recorders used it" is mostly unknown. The datasheets name application classes (answering machines, voice memo, cordless phones, toys), not models.

---

### 2. Tables by part family

#### 2.1 Speech codec / voice-recorder LSIs

| Part | Maker | Function | What the document gives | Used in (if known) | URL (status) | Rating |
|---|---|---|---|---|---|---|
| MSM6588 / 6588L | OKI | ADPCM solid-state recorder (serial voice register) | 3/4-bit ADPCM; fs 4.0/5.3/6.4/8.0 kHz at 4.096 MHz (×2 at 8.192 MHz); 12-bit ADC/DAC; mic amp; **4th-order SC LPF, −40 dB/oct, fc = 0.4·fs, curve at fs = 8 kHz** (flat to ≈3.2 kHz, ≈−60 dB notch near 9 kHz, ≈−50 to −53 dB floor above); 6588L full scale ¼–¾ VDD; voice-triggered start ±VDD/128, /64, /32 (±24/48/96 mV at 3.072 V); op-amp open-loop gain ≥ 40 dB; application circuits; 106 pp. No step table in the sheet. | Answering machines, voice memo (application class; no model named) | https://assets.sourcengine.com/datasheets/1add7da6-9183-41e9-a2f2-28d5204d6b1e.pdf (verified, FEDL6588-6588L-04, 2002); 1998 copy https://datasheetq.com/pdf-view/MSM6588-OKI.pdf (verified, 101 pp.) | 4 (5 with the Dialogic note) |
| MSM6688 / 6688L | OKI | ADPCM solid-state recorder (dedicated DRAM register, up to 32 Mbit) | Same LPF family (−40 dB/oct, 0.4·fs); 3/4-bit ADPCM; voice detection levels ±VDD/64…/16 (5 V) or /128…/32 (3 V); recording-time formula; 159 pp. | Not known | https://datasheetbank.com/pdf-view/MSM6688L-OKI.pdf (verified, E2D0026-27-43, Jan. 1998) | 4 |
| MSM6788 | OKI | SBC solid-state recorder | Five sub-bands; 96-sample frames; 16.0/12.6/10.0 kbit/s at 8 kHz, 12.0/9.5/7.5 at 6 kHz; −40 dB/oct LPF; recording-time formula. **No band-split filters, no bit allocation.** | Not known | 1994 OKI Voice Synthesis ICs data book, printed pp. 815–954 (verified, see 3.1) | 2 |
| MSM6388, MSM6258, MSM5218, MSM6310 | OKI | Earlier ADPCM/PCM record-playback LSIs | Rates, bit depths, memory types (typical-characteristics table, data book p. 8) | MSM6258: Sharp X68000 (general knowledge, not from the book) | 1994 data book (verified) | 2–3 (not read individually) |
| MSM6322 | OKI | 17-step voice pitch converter | 8-bit PCM; input 8 kHz, output 4–16 kHz; mic amp, LPF, line amp; 4 MHz; 4–6 V | Voice changers | 1994 data book pp. 9–10 and 1055 (verified) | 2 |
| MSM7570-01 (and 7570L) | OKI | Multi-function ADPCM codec | **ITU-T G.726 32/24/16 kbit/s**; A/µ-law PCM; transmit loss T1–T6 with limits (0–60 Hz ≥ 25 dB; 300–3000 Hz −0.15…+0.20; 3300 Hz −0.15…+0.80; 3400 Hz 0…0.80; 3968.75 Hz ≥ 13 dB) and receive R1–R5; idle noise; **VOX: transmit voice detect, receive background-noise generation**; side tone; DTMF; 26 pp. | Digital cordless phones | https://datasheetbank.com/pdf-view/MSM7570-01-OKI.pdf (verified, E2U0030-28-82, Aug. 1998) | 5 |
| MSM7540 / 7560 | OKI | Single-rail ADPCM codec | **G.721 (1988) 32 kbit/s**; µ/A-law or 14-bit linear PCM; loss templates T1–T6 / R1–R5; idle noise; 15 pp. | Cordless phones | https://datasheetbank.com/pdf-view/MSM7540-OKI.pdf (verified, E2U0026-28-83, Aug. 1998) | 5 |
| MSM7704-01 | OKI / LAPIS | 2-channel single-rail PCM codec | 300–3400 Hz; built-in PLL; 2.7–3.8 V; µ/A-law variants; 19 pp. (text layer thin; AC tables not extracted) | Echo-canceller front ends | https://datasheetq.com/pdf-view/MSM7704-01-LAPIS.pdf (verified, FEDL7704-01-01, 2012) | 3 (tables not read) |
| MSM7533 / 7534 | OKI | 2-channel 5 V PCM codec | 300–3400 Hz; µ/A-law; BCLK list; 18 pp. | — | https://pdf.jiepei.com/msm7534-9756080.html (verified, page 1 only, E2U0020-28-81, Aug. 1998) | 3 |
| ISD2532/40/48/64 and ISD2560/75/90/120 | ISD (Winbond, later Nuvoton) | Analog multilevel-storage ChipCorder | fs 8.0/6.4/5.3/4.0 kHz → typical −3 dB pass band 3.4/2.7/2.3/1.7 kHz; 5-pole active anti-alias and smoothing filters; low cut set by an external C with the internal 10 kΩ | Toys, answering machines | https://pdf.jiepei.com/isd2590t-29157262.html?page=10 (verified, page 10, 42 pp.); ISD April 1998 sheet https://www.farnell.com/datasheets/305422.pdf (verified, 5 pp., text mostly unextractable) | 3 |
| ISD4004-08/10/12/16M | ISD / Nuvoton | 3 V multilevel ChipCorder, 8–16 min | fs and pass band 8.0/3.4, 6.4/2.7, 5.33/2.3, 4.0/1.7 kHz; 5-pole AAF; **THD 1 % typ / 2 % max** at 32 mVpp 1 kHz; ANA IN max 32 mVpp; 3 kΩ input impedance sets the low cut with the coupling C | Voice memo products of 1998–2005 (class) | https://www.nuvoton.com/export/resource-files/en-us--DS_ISD4004_DataSheet_EN_Rev1.7.pdf (verified) | 3 |
| µPD7759 | NEC | ADPCM speech synthesis (playback only) | "rp-ADPCM" (repeating phoneme); fs 4/5/6/8 kHz; 8–32 kbit/s; 26 pp. scan | Arcade games (MAME driver) | https://bitsavers.mirrorservice.org/components/nec/speech/uPD7759_ADPCM_Speech_Synthesis_LSI.pdf (verified, NEC Electronics Europe, 12/85 V1.0) | 2 |
| MSM9842 | OKI | Playback LSI with FIFO (ADPCM2/ADPCM/PCM) | fs 4.0–32 kHz; 14-bit DAC; −40 dB/oct LPF; volume steps | — | https://assets.sourcengine.com/datasheets/d2b3ca5a-6c52-49f4-9575-5f0873264ff7.pdf (verified, preliminary, 1997) | 2 |
| ML22xx | LAPIS / ROHM | Modern speech playback LSI (HQ-ADPCM) | Out of period; playback only | — | https://fscdn.rohm.com/lapis/en/products/databook/datasheet/ic/speech/FEDL227XX.pdf (HTTP 200, not opened) | 1 |
| KS5911 | Samsung | ADM voice record/play with DRAM | Per summary: ADM, 8/11/16/32 kbit/s, voice-start / silence-stop auto mode | — | https://datasheetbank.com/preview/KS5911-Samsung (**not verified**: 403) | 1 |
| HT8658 / HT8659 | Holtek | ADM voice recorder (DRAM) | Per search summary only | — | https://html.alldatasheet.com/html-pdf/146445/HOLTEK/HT8659/215/1/HT8659.html (**not verified**: 403) | 1 |
| TC8830F / TC8830AF | Toshiba | Voice record/playback LSI (ADM per distributors) | No datasheet found | Arcade (MAME header) | https://git.redump.net/mame/tree/src/devices/sound/tc8830f.h?h=xtal (not opened) | 1 |

#### 2.2 Algorithm references

| Document | Gives | URL (status) | Rating |
|---|---|---|---|
| Dialogic "Dialogic ADPCM Algorithm", 00-1366-001, © 1988 (PDF built 1998) | The OKI/Dialogic 4-bit ADPCM: encoder and decoder flow, Table 1 M(L(n)) index adjustments, Table 2 the 49 step sizes, reset conditions (step 16, estimate 0), VOX file format; 7 pp. | https://people.ece.cornell.edu/land/courses/ece4760/Speech/adpcm.pdf (verified) | 5 |
| OKI 1987 Voice Synthesis LSI Data Book, introduction | The ADPCM recursion as OKI states it: dn = Xn − X̂n−1, Ln = dn/Δn, qn = (Ln + ½)Δn, Δn+1 = Δn·M(Ln); bit meanings (sign, 4Δ, 2Δ, Δ); "straight" vs "compressed" ADPCM (unvoiced elimination, waveform repetition). No M(Ln) table. | https://ftp.mirrorservice.org/sites/www.bitsavers.org/components/oki/_dataBooks/1987_OKI_Voice_Synthesis_LSI_Data_Book.pdf (verified; research-sources §3.4 had it unread) | 3 |
| ITU-T G.726 / G.721 | The MSM7570/7540 algorithm (standard; not fetched here) | itu.int (not fetched) | — |

#### 2.3 Electret microphone capsules (Panasonic WM series)

| Part | Type | What the sheet gives | URL (status) | Rating |
|---|---|---|---|---|
| WM-61A | 6 × 3.4 mm back electret, omni | **Readable curve 20 Hz–20 kHz**: flat (0 dB) from 50 Hz to about 5 kHz, a small ripple about −1 dB near 7–12 kHz; sensitivity −35 ± 4 dB re 1 V/Pa at 2 V / 2.2 kΩ; S/N > 62 dB; < 2.2 kΩ; 20–16 000 Hz; −3 dB at 1.5 V; application circuit (2.2 kΩ, 1 µF) | https://uploadcdn.oneyac.com/attachments/files/brand_pdf/panasonic/WM-61A.pdf (verified, Panasonic PDF built 2009) | 5 |
| WM-034B/034DB/034C/034D | 9.7 mm front electret, omni, "for telephone" | **Curve**: flat to about 3 kHz, **+3 dB peak near 5 kHz**, then falling to about −10 dB at 16 kHz; sensitivity grades X/Y/Z −46/−42/−38 ± 3 dB (4.5 V, 2.2 kΩ) and M…U −44…−36 ± 2 dB (1.5 V, 1.5 kΩ); S/N > 60 dB | https://www.tedss.com/DataSheets/2099/2099000243.pdf (verified, Panasonic, 2000–2001) | 5 |
| WM-52B / WM-54B | 9.7 mm omni; WM-54B "suitable for tape recorders" | Curve flat to about 10 kHz, then −3 dB toward 16 kHz; grades −46…−40 dB; S/N > 60 dB; 1.5 V / 2.5 V | https://radioizba.ru/cat/techdata/WM-54B.pdf (verified, Panasonic 2004, "Discontinued" overprint) | 4 |
| WM-55D103 | 9.7 × 5 mm **noise-cancelling (pressure-gradient)** | **Two curves**: at 1 cm flat at about +10 dB; at 50 cm about −17 dB below 100 Hz, rising to about +8 dB at 2 kHz and flat above; −54 ± 4 dB at 50 cm; 680 Ω; 100–10 000 Hz; S/N > 58 dB | https://static.datasheets.com/doc/8032666-panasonic-wm-55d103-ds.pdf (verified) | 5 |
| WM-64MN | Thin omni, gold contacts | Sheet with curve (not rendered) | https://cdn.eicom.ru/media/PDF/949797.pdf (verified, Panasonic SC Device Solutions, 2005) | 4 (curve not read) |
| WM-60A | Omni back electret | Sensitivity −46 (X) / −44 (T) dB per search summary | https://dtsheet.com/doc/248347/panasonic-wm-60a (**not verified**: 403) | — |
| RJM0016 | The RR-DR60's own capsule (S-002 parts list) | No document found | — | — |

#### 2.4 Small speakers

| Part | Maker | What the sheet gives | URL (status) | Rating |
|---|---|---|---|---|
| CE28MB-8 | Dayton Audio | 28 mm, 8 Ω: **Re 7.54 Ω, Le 0.45 mH, Fs 623 Hz, Qms 2.22, Qes 6.16, Qts 1.63, Mms 0.087 g, Cms 0.748 mm/N, Sd 4.52 cm², BL 0.642 T·m, Vas 0.0215 l, Xmax 0.1 mm**; SPL 81 dB @ 2.83 V/1 m; response and impedance/phase plots (measured at 3.5 in., 1 V) | https://www.daytonaudio.com/images/resources/285-147--dayton-audio-ce28mb-8-spec-sheet.pdf (verified, 2022) | 5 (modern part; a physics model, not a period part) |
| CLS0281MP | CUI Devices (now Same Sky) | 28 mm, 8 Ω, 0.2 W, Mylar (PET in the 2024 sheet); Fo 850 Hz (700–1000); response Fo–6 kHz; **chart-recorder response curve 10 Hz–20 kHz** (peak about +12 dB re plateau near 1 kHz, a plateau 2–5 kHz, falling from about 6 kHz); "Discontinued" | https://www.cuidevices.com/product/resource/cls0281mp.pdf (verified; redirects to the Same Sky 2024 sheet; the 2021 CUI copy on the same path was fetched earlier) | 4 |
| EAS-4P15SA, EAS-45P30S (and table EAS-6P22S, -8P36S, -4D02CO, -G7D504A2, -G9D550B2, -G9D541A2, -G12D531E2) | Panasonic | 45 mm, 8 Ω, 0.1 W, fo 530 Hz, "fo–3k"; **response curves 20 Hz–20 kHz** (small; 45P30S peaks near 2.5 kHz and falls sharply above 4 kHz); SPL 92–93 dB/1 W/0.5 m | https://media.digikey.com/pdf/Data%20Sheets/Panasonic%20Electronic%20Components/General_Speaker.pdf (verified, Panasonic, 2000/2004) | 3 |
| EAS-3P127A | Panasonic | 36 mm, 8 Ω, 500 mW, 550 Hz–4 kHz (distributor) | https://datasheet.lcsc.com/lcsc/2312251854_PANASONIC-EAS-3P127A_C19579902.pdf (**not verified**: served HTML) | 1 |
| AS02808MR-R | PUI Audio | 28 mm, 8 Ω, 500 Hz–20 kHz (listing) | https://www.mouser.com/datasheet/2/334/AS02808MR-R-25860.pdf (**not verified**: Mouser served HTML) | 1 |

#### 2.5 Low-voltage audio amplifiers and earphone drivers

| Part | Maker | What the sheet gives | URL (status) | Rating |
|---|---|---|---|---|
| LM4871 | TI (ex-National) | 3 W BTL with shutdown; THD+N vs frequency and vs output power; output power vs supply and vs load; gain set by resistors; 30 pp. | https://www.ti.com/lit/ds/symlink/lm4871.pdf (verified, SNAS002F, Feb. 2000, rev. 2013) | 5 |
| LM4880 | TI (ex-National) | Dual 250 mW (headphone / earphone class) with shutdown; curves as above; 24 pp. | https://www.ti.com/lit/ds/symlink/lm4880.pdf (verified, SNAS103C, Nov. 1995, rev. 2013) | 5 |
| LM386 | TI (ex-National) | Low-voltage amplifier, gain 20–200, curves; 4 V minimum (above the 2-cell range) | https://www.ti.com/lit/ds/symlink/lm386.pdf (verified, SNAS545D, rev. 2023) | 4 |
| TDA7050 / TDA7050T | Philips | 1.6 V minimum; **fixed 26 dB gain**, differential input; BTL or stereo; output power vs supply (Figs 2–3) at 10 % THD into 32 Ω; noise output voltage; application diagrams | https://archive.org/download/manualsonline-id-4cce6949-09ab-407e-8a13-81b0eedd8e61/4cce6949-09ab-407e-8a13-81b0eedd8e61.pdf (verified, 8 pp.); TDA7050T https://www.rcscomponents.kiev.ua/datasheets/TDA7050T.pdf (verified, 9 pp., 1994 data, PDF 1998) | 4 |
| KA2209 | Samsung | Dual low-voltage amplifier, 1.8–9 V; gain 44 dB; Po 0.11 W at 3 V / 4 Ω stereo, 0.35 W bridge; THD 0.5 %; RR 24–30 dB; noise 0.5 mV (TDA2822 class) | 1986 Samsung Linear IC Data Book, printed pp. 78–80: https://archive.decromancer.ca/bitsavers.org/components/samsung/_Databooks/1986_Samsung_Linear_IC_Data_Book.pdf (verified, OCR scan) | 3 |
| NJM2113 (newer revision) | Nisshinbo (ex-JRC) | Ver. 2026-07-29, 6 pp. with "Typical characteristics"; a newer issue than S-006's 2014 sheet | https://www.nisshinbo-microdevices.co.jp/en/pdf/datasheet/NJM2113_E.pdf (verified) | already S-006 (re-check the curves against S-006) |
| MSC1191 / MSC1192 | OKI | Speaker drive amplifier for the voice LSIs, 2–6 V, 250 mA, standby pin | 1994 OKI data book p. 11 and printed p. 1073 (verified; data pages not read) | 2 |
| NJM2070 | JRC / Nisshinbo | 1.8–15 V class-AB amplifier (distributor summaries) | **not verified**: the vendor PDF paths return 404 | 1 |
| TDA1308 | Philips / NXP | Class-AB headphone driver | **not verified**: the NXP PDF returns 404, alldatasheet 403 | — |
| MC34119 | Motorola | already S-015 | — | — |

---

### 3. Entries (research-sources.md format)

#### 3.1 OKI voice-recorder LSIs and data books

1. **OKI "Voice Synthesis LSI Data Book" 1994** (OKI Semiconductor; 1123-page OCR scan, "Paper Capture" text layer).
   URL: [bitsavers mirror](https://ftp.mirrorservice.org/sites/www.bitsavers.org/components/oki/_dataBooks/1994_OKI_Voice_Synthesis_ICs.pdf).
   *Data book; Primary; none; partly read (contents, product tables pp. 3–13, the MSM6788 and MSM6322 introductions, the ADPCM regeneration text in the MSM6650 section).*
   Provides: contents list. Voice synthesis: MSM5205, MSM6585, MSM6295, MSM6372–6376, MSM6378A/6379, MSM6652–6656, MSM6650. **Voice recorder**: MSM5218 p. 415, MSM6258/6258V 433, MSM6310 491, MSM6388 509, MSM6588 565, MSM6688 663, MSM6788 815. Voice register/ROM: MSM6389, 6587, 6586, 6595–6597. Interface: MSM6690/6691/6791. **Pitch control**: MSM6322 1055, MSM6722 1069. **Speaker amplifier**: MSC1191/1192 1073. Demo boards. A typical-characteristics table of the recorders (coding, memory, maximum time, fs ranges, clocks, built-in amplifiers). MSM6788: SBC in five bands, 96-sample frames, 16.0/12.6/10.0 kbit/s at 8 kHz and 12.0/9.5/7.5 kbit/s at 6 kHz, LPF −40 dB/oct. MSM6322: 17-step pitch conversion, 8-bit PCM, input 8 kHz, output 4–16 kHz. Page numbers are printed pages; the PDF index is offset by about +5.
   Access: a 45 MB download; the OCR is imperfect (Δ comes out as "6" or "c."), so render the pages for tables.
   **Rating 3 overall**: the best single map of OKI's recorder line; the individual standalone datasheets below are cleaner.

2. **OKI MSM6588/6588L "ADPCM Solid-State Recorder (for Serial Registers)", FEDL6588-6588L-04** (OKI; issued Jan. 25, 2002; 106 pp.).
   URL: [sourcengine PDF](https://assets.sourcengine.com/datasheets/1add7da6-9183-41e9-a2f2-28d5204d6b1e.pdf). Older 1998 copy (E2D0025-27-42 per search; 101 pp. opened): [datasheetq](https://datasheetq.com/pdf-view/MSM6588-OKI.pdf).
   *Datasheet; Primary (for that chip; a sibling-class device, not in the RR-DR60); none; partly read (features, analog characteristics p. 20, LPF and full-scale p. 42, voice-triggered start p. 43; the LPF page rendered).*
   Provides: standalone and microcontroller modes. 3-bit ADPCM at 5.3/8.0 kHz in standalone mode; 3/4-bit selectable at 4.0/5.3/6.4/8.0 kHz (4.096 MHz) or ×2 (8.192 MHz). 12-bit A/D and D/A. Mic amplifier. **"built-in fourth order LPF using switched capacitor filter technology … –40 dB/oct … cut-off frequency is preset to 0.4 times the sampling frequency"**, with a curve at fs = 8 kHz: 0 dB to about 3.2 kHz, a steep fall to a notch near −60 dB at about 9 kHz, rising to about −50 dB by 40 kHz. Full scale 0–VDD (6588) and ¼–¾ VDD (6588L). Voice-triggered start (a recording begins when \|ADIN − ½VDD\| exceeds VVDS): ±VDD/64, /32, /16 (5 V) or ±VDD/128, /64, /32 (3 V; ±24/48/96 mV at 3.072 V). Op-amp open-loop gain ≥ 40 dB (0–4 kHz). DA relative error ≤ 5 mV. Maximum 262 s at 3-bit / 5.3 kHz. Application circuits.
   Not given: the ADPCM step table (use the Dialogic note, 3.2.1), S/N, or THD.
   **Rating 4** (5 with the Dialogic tables). A complete sibling-class recorder front end: LPF, converters, a voice-start threshold and a codec.

3. **OKI MSM6688/6688L "ADPCM Solid-State Recorder IC", E2D0026-27-43** (OKI; Jan. 1998, previous May 1997; 159 pp.).
   URL: [datasheetbank PDF](https://datasheetbank.com/pdf-view/MSM6688L-OKI.pdf).
   *Datasheet; Primary (sibling class); none; partly read (differences table, contents, LPF text).*
   Provides: the same −40 dB/oct LPF at 0.4·fs; up to 32 Mbit of dedicated registers (MSM6684B 4 Mbit, MSM6685 8 Mbit); voice detection ±VDD/64, /32, /16 (5 V) and ±VDD/128, /64, /32 (3 V); the recording time = memory ÷ (fs × bits).
   Access: the system `grep` (aliased to ugrep on the research machine) missed text in this PDF's extraction; use `/usr/bin/grep -a`.
   **Rating 4.**

4. **OKI MSM7570-01 "Multi-Function ADPCM CODEC", E2U0030-28-82** (OKI; Aug. 1998, previous Nov. 1996; 26 pp.).
   URL: [datasheetbank PDF](https://datasheetbank.com/pdf-view/MSM7570-01-OKI.pdf).
   *Datasheet; Primary; none; partly read (features, AC characteristics).*
   Provides: a G.726 ADPCM codec at 32/24/16 kbit/s with an integrated PCM codec; A/µ-law selectable; 12.288/19.2 MHz master clock. Transmit frequency response: LOSS T1 0–60 Hz ≥ 25 dB; T2 300–3000 Hz −0.15…+0.20 dB; T3 1020 Hz reference; T4 3300 Hz −0.15…+0.80; T5 3400 Hz 0…0.80; T6 3968.75 Hz ≥ 13 dB. Receive R1–R5 (not transcribed). Idle channel noise (P-message weighted). Two input amplifier stages. **VOX: transmit voice-signal detect; receive background-noise generation.** Side tone; DTMF; tone generators; a sounder driver. MSM7570L is the 2.7–3.6 V variant (per search; [manualmachine](https://manualmachine.com/datasheet/msm7570l01tsk/4154731-datasheet-oki), **not verified**: 403).
   **Rating 5.** With the ITU-T G.726 text, the codec, the masks and the VOX behavior are all specifiable; its VOX and comfort noise are a documented period example of the features the VAS stage touches.

5. **OKI MSM7540/7560 "Single Rail ADPCM CODEC", E2U0026-28-83** (OKI; Aug. 1998, previous Nov. 1996; 15 pp.).
   URL: [datasheetbank PDF](https://datasheetbank.com/pdf-view/MSM7540-OKI.pdf).
   *Datasheet; Primary; none; partly read (features, loss rows located).*
   Provides: "Complies completely with 1988's version ITU-T G.721 (32 kbps)"; µ-law/A-law, or 14-bit linear (MSM7540); 10.368 MHz crystal; LOSS T1–T6 / R1–R5; idle noise.
   **Rating 5.**

6. **OKI MSM7704-01 "2ch Single Rail CODEC", FEDL7704-01-01** (LAPIS; June 25, 2012; 19 pp.).
   URL: [datasheetq PDF](https://datasheetq.com/pdf-view/MSM7704-01-LAPIS.pdf).
   *Datasheet; Primary; none; partly read (header; the AC tables did not extract as text).*
   Provides (from the header and the search summary): two channels, 300–3400 Hz, built-in PLL, 2.7–3.8 V, -01/-02/-03 companding variants, an echo-canceller front end. A two-channel cousin of the MSM7702.
   **Rating 3** until the tables are rendered.

7. **OKI MSM7533/7534 "2ch Single Rail CODEC", E2U0020-28-81** (OKI; Aug. 1998; 18 pp.).
   URL: [jiepei page 1](https://pdf.jiepei.com/msm7534-9756080.html) (the `?page=N` pattern of the access tips should apply).
   *Datasheet; Primary; none; partly read (page 1 via the fetch tool).*
   Provides: 300–3400 Hz, 5 V, µ/A-law variants, the same BCLK list as the MSM7702 family. **Rating 3.**

8. **OKI MSM9842 "Voice Synthesis LSI with Built-in FIFO"** (OKI; preliminary; PDF built Nov. 1997; 35 pp.).
   URL: [sourcengine PDF](https://assets.sourcengine.com/datasheets/d2b3ca5a-6c52-49f4-9575-5f0873264ff7.pdf).
   *Datasheet; Primary; none; partly read (contents and features).*
   Provides: ADPCM2 (4–8 bit), 4-bit ADPCM, 8/16-bit PCM, 8-bit nonlinear PCM; fs 4.0/6.4/8.0/12.8/16.0/32.0 kHz; 14-bit DAC; LPF −40 dB/oct; 8-step volume 0–21 dB. Playback only. **Rating 2.**

#### 3.2 Algorithm references

1. **Dialogic Corporation, "Dialogic ADPCM Algorithm", application note 00-1366-001** (© 1988; PDF built Aug. 1998; 7 pp.).
   URL: [Cornell course mirror](https://people.ece.cornell.edu/land/courses/ece4760/Speech/adpcm.pdf). Alternative listing: [academia.edu](https://www.academia.edu/6620645/Dialogic_ADPCM_Algorithm) (not opened).
   *Application note; Primary for the algorithm (Dialogic and OKI share it; the note does not name OKI's chips); none; read (structure; the tables located, not transcribed here).*
   Provides: the VOX file format; the encoder and decoder block diagrams; the 4-bit code-to-difference computation; Table 1 (the index adjustment M(L(n))) and Table 2 (49 calculated step sizes); the initial and reset conditions.
   **Rating 5.** With the MSM6588 sheet's rates and LPF, an OKI-class ADPCM recorder is implementable end to end. The 3-bit variant's table is not in this note; open-source decoders (MAME's `okiadpcm`, spandsp `oki_adpcm.c`) carry a 3-bit table, but those are secondary.

2. **OKI 1987 Voice Synthesis LSI Data Book, introduction** (already listed as research-sources §3.4, "not read"; now **partly read**).
   URL: as listed there (opened 2026-11-04; 214 pp.).
   Provides: the ADPCM recursion and bit semantics quoted in table 2.2; "Oki straight ADPCM" vs "compressed ADPCM" (unvoiced elimination and waveform repetition, about ⅓ the bit rate); the MSM5205 and MSM5218 data. **Rating 3** (no M(Ln) table).

#### 3.3 Other recorder-class LSIs

1. **ISD/Winbond ISD2560/75/90/120 ChipCorder datasheet** (jiepei mirror; 42 pp.; undated).
   URL: [jiepei page 10](https://pdf.jiepei.com/isd2590t-29157262.html?page=10). The 1994 *Silicon Chip* note [ISD2590P](https://siliconchip.com.au/Issue/1994/February/Data+On+The+ISD2590P+Voice+Recorder+IC) was not opened.
   *Datasheet; Primary; none; partly read (page 10 via the fetch tool).*
   Provides: durations 60/75/90/120 s ↔ fs 8.0/6.4/5.3/4.0 kHz ↔ typical filter pass band 3.4/2.7/2.3/1.7 kHz (3 dB point).

2. **ISD "Single-Chip Voice Record/Playback Devices, 32-, 40-, 48-, and 64-Second Durations" (ISD2532/40/48/64)** (ISD, April 1998; 5 pp.).
   URL: [Farnell PDF](https://www.farnell.com/datasheets/305422.pdf).
   *Datasheet; Primary; none; partly read (the text layer is mostly unextractable symbol encoding; the "Typical Filter Pass Band" row was located).* Render to read. **Rating 3.**

3. **Nuvoton ISD4004 series datasheet, Rev 1.7** (Nuvoton; ex-ISD/Winbond).
   URL: [nuvoton.com PDF](https://www.nuvoton.com/export/resource-files/en-us--DS_ISD4004_DataSheet_EN_Rev1.7.pdf).
   *Datasheet; Primary; none; partly read (block diagram, pin text, the pass-band and THD rows).*
   Provides: multilevel analog storage (960K–3840K cells); 5-pole active anti-aliasing and smoothing filters; fs and pass band 8.0/3.4, 6.4/2.7, 5.33/2.3, 4.0/1.7 kHz; THD 1 % typ, 2 % max (32 mVpp, 1 kHz); input 32 mVpp single-ended (16 mVpp differential); a 3 kΩ input impedance with the coupling C sets the low cut; AutoMute.
   **Rating 3**: the filters and rates are implementable (the 5-pole shape assumed Butterworth or Chebyshev); the storage noise is not specified.

4. **NEC µPD7759 "ADPCM Speech Synthesis LSI" product description** (NEC Electronics (Europe) GmbH; 12/85 V1.0; 26 pp. scan).
   URL: [bitsavers mirror](https://bitsavers.mirrorservice.org/components/nec/speech/uPD7759_ADPCM_Speech_Synthesis_LSI.pdf). Related: [µPD7755/7756 user's manual](https://bitsavers.mirrorservice.org/components/nec/speech/uPD7755_uPD7756_Users_Manual.pdf) (not opened).
   *Datasheet; Primary; none; partly read (outline, features).*
   Provides: rp-ADPCM (ADPCM with a repeating phoneme), external ROM up to 1 Mbit, fs 4/5/6/8 kHz (product description) or 5/6/8 kHz (later sheet, per search), 8–32 kbit/s. Synthesis only. **Rating 2.**

5. **Samsung KS5911** and **Holtek HT8658/HT8659** (ADM voice recorders with DRAM).
   URLs: [datasheetbank KS5911 preview](https://datasheetbank.com/preview/KS5911-Samsung) (403); [alldatasheet HT8659](https://html.alldatasheet.com/html-pdf/146445/HOLTEK/HT8659/215/1/HT8659.html) (403).
   *Datasheets; not read (search summaries only).* Per the summaries: ADM; 8/11/16/32 kbit/s; an auto mode that starts on voice and stops on silence. Worth a retry: ADM is a distinct, low-complexity coder class with a recognizable slope-overload sound. **Rating 1** until opened.

#### 3.4 Microphone capsules

1. **Panasonic WM-61A "Omnidirectional Back Electret Condenser Microphone Cartridge"** (Panasonic Corporation; PDF built 2009-04-02; 1 page plus a distributor page).
   URL: [oneyac mirror](https://uploadcdn.oneyac.com/attachments/files/brand_pdf/panasonic/WM-61A.pdf). Others: [dtsheet](https://dtsheet.com/doc/173142/panasonic-wm-61a) (not opened), [DigiKey listing](https://www.digikey.ca/en/products/detail/panasonic-industry/WM-61A/252843) (obsolete; not opened).
   *Datasheet; Primary; none; read (rendered).*
   Provides: a typical frequency response curve on a 20 Hz–20 kHz, −30…+20 dB grid: flat 0 dB from about 50 Hz, a shallow dip of about −1 dB between about 7 and 12 kHz, ending near 15 kHz. Sensitivity −35 ± 4 dB (0 dB = 1 V/Pa, 1 kHz) at Vs 2.0 V, RL 2.2 kΩ; impedance < 2.2 kΩ; 20–16 000 Hz; maximum 10 V, standard 2 V; maximum current 0.5 mA; within −3 dB at 1.5 V; S/N > 62 dB; the FET schematic with a 2.2 kΩ load and a 1 µF coupling.
   **Rating 5.** A flat reference capsule. A hobbyist measurement (Szekeres, [pitt.edu](https://sites.pitt.edu/~szekeres/mic/mic.htm), Anecdotal, read) reports ±0.5 dB to about 7 kHz and a +2 dB hump near 13 kHz on the WM-61A's successor class.

2. **Panasonic WM-034B/034DB/034C/034D "Omnidirectional Electret Condenser Microphone Cartridge"** (Panasonic Electronic Components; PDF 2000-09-15, modified 2001; 1 page).
   URL: [tedss.com PDF](https://www.tedss.com/DataSheets/2099/2099000243.pdf). Others: [jiepei](https://pdf.jiepei.com/wm-034b-8925948.html) (not opened), [datasheetq](https://datasheetq.com/pdf-html/692626/Panasonic/1page/WM-034B.html) (403).
   *Datasheet; Primary; none; read (rendered).*
   Provides: "Useful for almost any type telephone"; a curve flat (0 dB) from 20 Hz to about 3 kHz, a **presence peak of about +3 dB near 5 kHz**, then a roll-off to about −10 dB at 16 kHz. Sensitivity grades (B/C at 4.5 V, 2.2 kΩ: X −46, Y −42, Z −38 ± 3 dB; DB/D at 1.5 V, 1.5 kΩ: M −44 … U −36 ± 2 dB). Impedance < 2.2 kΩ [1.5 kΩ]; S/N > 60 dB.
   **Rating 5.** A contrast to WM-61A: a telephone-type capsule with a top-octave lift and fall.

3. **Panasonic WM-52B/54B** (Panasonic; PDF 2004; "Discontinued" overprint).
   URL: [radioizba mirror](https://radioizba.ru/cat/techdata/WM-54B.pdf).
   *Datasheet; Primary; none; read (rendered).*
   Provides: "WM-54B series are low-voltage 2.5V operation microphones and suitable for tape recorders, telephone devices, and toys". The curve is flat to about 10 kHz with a −3 dB droop by 16 kHz. Sensitivity grades −46 … −40 dB; S/N > 60 dB.
   **Rating 4** (the overprint obscures the curve's left end).

4. **Panasonic WM-55D103 "Noise Cancelling Back Electret Condenser Microphone Cartridge"** (Panasonic; undated; 1 page).
   URL: [datasheets.com static PDF](https://static.datasheets.com/doc/8032666-panasonic-wm-55d103-ds.pdf). Listing: [DigiKey](https://www.digikey.co.uk/en/products/detail/panasonic-industry/WM-55D103/138179) (obsolete; not opened).
   *Datasheet; Primary; none; read (rendered).*
   Provides: two curves. **L = 1 cm**: about +10 dB, flat from 50 Hz to 10 kHz. **L = 50 cm**: about −17 dB at 50–100 Hz, rising (about first order) through −10 dB at 300 Hz to about +7 dB at 2 kHz, joining the 1 cm curve at about +9 dB above 5 kHz. −54 ± 4 dB at 50 cm (1.5 V, 680 Ω); 100–10 000 Hz; S/N > 58 dB.
   **Rating 5.** A pressure-gradient capsule whose response depends on talker distance. A swappable "close-talk" mic component is fully specified by the two curves (a first-order gradient model with a distance parameter).

5. **Panasonic WM-64MN** (Panasonic SC Device Solutions; PDF 2005; 1 page).
   URL: [eicom mirror](https://cdn.eicom.ru/media/PDF/949797.pdf).
   *Datasheet; Primary; none; partly read (text only; curve not rendered).* **Rating 4** pending the render.

#### 3.5 Speakers

1. **Dayton Audio CE28MB-8 specification sheet** (Dayton Audio; 2022-01-24; 1 page).
   URL: [daytonaudio.com PDF](https://www.daytonaudio.com/images/resources/285-147--dayton-audio-ce28mb-8-spec-sheet.pdf).
   *Spec sheet; Primary (modern part); none; read (text).*
   Provides: the full Thiele–Small set (see table 2.4), response and impedance/phase plots.
   **Rating 5 for implementability.** A lumped electro-mechanical model of a 28 mm 8 Ω driver of the RR-DR60's size (S-001: 2.8 cm 8 Ω) can be built from it directly. It is a modern poly-cone part, so its use is as a physically grounded template, not as the device's speaker.

2. **CUI Devices CLS0281MP speaker datasheet** (CUI Devices 2021-02-08; reissued by Same Sky 2024-09-11; 3 pp.; "Discontinued").
   URL: [cuidevices.com resource](https://www.cuidevices.com/product/resource/cls0281mp.pdf) (now served as the Same Sky sheet). Mirror of a copy: [hkjdwchip](https://hkjdwchip.com/image//files/cuidevices-cls0281mp-datasheets-1062.pdf) (verified).
   *Datasheet; Primary (modern part); none; read (rendered page 2).*
   Provides: 28 mm, 8 Ω (6.8–9.2 at 1.5 kHz), 0.2 W rated / 0.3 W maximum; Fo 850 Hz (700–1000); Mylar (2021) / PET (2024) cone; the response Fo–6 kHz. A chart-recorder curve (50 dB potentiometer range, RMS, 100 mm/s writing speed) on a 10 Hz–20 kHz log axis: rising from about 300 Hz, a resonance peak near 1 kHz, a plateau about 10 dB lower from 2 to 5 kHz, a fall from about 6 kHz to a trough near 10 kHz, and small peaks near 15–20 kHz.
   **Rating 4.** The curve can be digitized within about ±1–2 dB. It is the closest period-typical response shape found for a 28 mm Mylar driver.

3. **Panasonic "General Speaker" catalog sheet** (Panasonic Electronic Components; PDF 2000-09-28, modified 2004; 4 pp.).
   URL: [DigiKey media](https://media.digikey.com/pdf/Data%20Sheets/Panasonic%20Electronic%20Components/General_Speaker.pdf).
   *Catalog; Primary; none; read pp. 1–2 (rendered).*
   Provides: a specification table for nine EAS parts (power, impedance, fo, range, SPL dB/1 W/0.5 m, mass); curves for EAS-4P15SA (fo 530 Hz; a peak near 1 kHz; a ragged top from about 5 kHz) and EAS-45P30S (a broad 500 Hz–3 kHz plateau and a sharp fall above about 4 kHz). The smallest is 45 mm.
   **Rating 3** (small plots; no 28 mm part).

#### 3.6 Amplifiers

1. **TI LM4871 "3W Audio Power Amplifier with Shutdown Mode", SNAS002F** (National Feb. 2000; TI rev. May 2013; 30 pp.).
   URL: [ti.com](https://www.ti.com/lit/ds/symlink/lm4871.pdf). *Datasheet; Primary; none; partly read (titles of the typical-performance curves).* **Rating 5.**
2. **TI LM4880 "Dual 250 mW Audio Power Amplifier with Shutdown Mode", SNAS103C** (National Nov. 1995; TI rev. May 2013; 24 pp.).
   URL: [ti.com](https://www.ti.com/lit/ds/symlink/lm4880.pdf). *Datasheet; Primary; none; partly read.* A period-correct stereo earphone driver with full curves. **Rating 5.**
3. **TI LM386, SNAS545D** (rev. Aug. 2023; 34 pp.). URL: [ti.com](https://www.ti.com/lit/ds/symlink/lm386.pdf). *Partly read.* **Rating 4** (its 4 V minimum is outside 2-cell operation).
4. **Philips TDA7050 "Low voltage mono/stereo power amplifier"** (Philips Semiconductors; 8 pp.) and **TDA7050T** (1994 product specification; PDF 1998; 9 pp.).
   URLs: [archive.org (manualsonline)](https://archive.org/download/manualsonline-id-4cce6949-09ab-407e-8a13-81b0eedd8e61/4cce6949-09ab-407e-8a13-81b0eedd8e61.pdf); [rcscomponents TDA7050T](https://www.rcscomponents.kiev.ua/datasheets/TDA7050T.pdf).
   *Datasheet; Primary; none; partly read (features, characteristics headings, figure list).*
   Provides: a fixed 26 dB gain; operation from 1.6 V; output power vs supply in BTL and stereo at 10 % THD (Figs 2–3); noise output voltage at RS = 5 kΩ; application diagrams. **Rating 4.**
5. **Samsung KA2209 "Dual Low Voltage Power Amplifier"** in the **1986 Samsung Linear IC Data Book** (674-page OCR scan).
   URL: [bitsavers mirror (decromancer)](https://archive.decromancer.ca/bitsavers.org/components/samsung/_Databooks/1986_Samsung_Linear_IC_Data_Book.pdf).
   *Data book; Primary; none; read (KA2209 pp. 78–80).* Figures as in table 2.5. **Rating 3.**
6. **Nisshinbo NJM2113 datasheet, Ver. 2026-07-29** (6 pp.).
   URL: [nisshinbo-microdevices.co.jp](https://www.nisshinbo-microdevices.co.jp/en/pdf/datasheet/NJM2113_E.pdf).
   *Datasheet; Primary; S-006's part (a newer issue); not read beyond the headings.* A later revision of the register's 2014 sheet. Check whether its typical characteristics differ before citing either.

---

### 4. Searched without result, and dead ends

- **The RR-DR60's capsule RJM0016:** no document or cross-reference found (an exact-phrase search returned nothing relevant).
- **The RR-DR60's speaker part number:** not searched beyond S-001/S-002's "2.8 cm 8 Ω". No Panasonic 28 mm EAS sheet with a curve was found. The EAS-3P series pages are 36 mm and larger, with distributor-only data.
- **Japanese IC-recorder LSIs of 1997–98** (Sanyo, Sharp, Toshiba, NEC, Mitsubishi, Rohm): no press release or datasheet found, in English or Japanese queries. Sanyo's LC823xxx IC-recorder processors are 2011 and MP3. Mitsubishi M65831 is an echo/delay processor, not a recorder.
- **Toshiba TC8830F:** no datasheet; only a MAME header and distributor listings (which say ADM, not ADPCM).
- **Matsushita Technical Journal / National Technical Report** articles on an IC recorder or the MN19xx DSP: none found online. The NDL record shows the journal ran 1998–2008. A library request is the remaining route.
- **New MN19xx documents:** none beyond those in research-sources §4.
- **ROHM/LAPIS official codec PDFs:** `fscdn.rohm.com/lapis/en/products/databook/datasheet/ic/{speech,codec,communication,telecom}/FEDL7704-01.pdf` and `…/speech/FEDL7717-01.pdf` all return 404. Only the ML22xx speech-playback sheet resolves at that path. Use the mirrors.
- **NJM2070:** the vendor paths `…/en/pdf/datasheet/NJM2070_E.pdf` and `…/ja/pdf/datasheet/NJM2070_J.pdf` return 404 (the part is discontinued); `njr.com/semicon/PDF/…` does not connect.
- **TDA1308:** `nxp.com/docs/en/data-sheet/TDA1308.pdf` and `…/TDA1308_TDA1308A.pdf` return 404; alldatasheet returns 403. Not verified.
- **Blocked (403) this session:** datasheetbank `preview/` pages (the `pdf-view/` PDFs worked with curl and a browser user agent), datasheetq `pdf-html/`, dtsheet `doc/`, alldatasheet `html-pdf/`, manualmachine. **Access tip:** `datasheetbank.com/pdf-view/<PART>-OKI.pdf` and `datasheetq.com/pdf-view/<PART>-<MAKER>.pdf` returned real PDFs to `curl -L -A "Mozilla/5.0 …"`, while WebFetch got 403 on the same hosts.
- **Mouser and LCSC datasheet links** (`mouser.com/datasheet/2/334/…`, `datasheet.lcsc.com/…`) served HTML interstitials to curl.
- **IEEE 910491** ("A 64-min single-chip voice recorder/player using embedded 4-b/cell flash memory"): the abstract page rendered empty to the fetch tool. Not verified.
- **Electronicsdatasheets.com** download links returned HTML, not PDFs.
- **Shared scratch directory:** other research agents write to `scratchpad/dl/`. This survey's downloads are in `scratchpad/dlp/`, and its helper scripts are in `scratchpad/tools/`.

---

## Part C. Panasonic / National tape voice recorders


Researcher: agent survey, 2026-11-04. Scope: tape-based Panasonic / National voice recorders of the 1980s–2000s and the ICs inside them. It extends `docs/hardware/research/research-sources.md`; that file's section 13 lists no tape recorder, so nothing here repeats it. Access tips reused: the Internet Archive `advancedsearch.php … output=json` route (the "Sites that blocked" table) found the service CD-ROMs below.

Rating scale (owner's refinement): **implementability first**. 5 = the document alone lets an engineer model a stage (schematic with part values, IC data with gains/thresholds, measured mechanism figures, adjustment targets); 4 = most of a stage, one key datum missing; 3 = good schematic or good figures, not both; 2 = figures only; 1 = pointer or listing only.

### 1. Summary

1. **The best documentation by far is Panasonic's own digital service CD-ROMs on the Internet Archive** (TSNSystem "Technical Service Navigation", 2000–2002). They hold born-digital service manuals: clean 300–600 dpi TIF schematics and, for the 2001–2002 models, **vector PDF schematics whose part values are extractable text**. Found and opened: RN-305 / RN-405 microcassette (2000), RQ-L30 (2001) and RQ-L31 (2002) mini-cassette voice recorders.
2. **Two IC datasheets make whole stages implementable**: the Sanyo **LA4168M** (EN5660, 1997; the single-chip pre-amp / record amp / ALC / VOX / BTL power amp / motor governor used in the RN-305, RN-405, RN-502, RQ-L30, RQ-L31) and the Panasonic **AN7086S** (pre/power amp with ALC and VAS, used in the RQ-L305 and RQ-L340), plus the **AN6612** motor governor (with a motor's constants in its application circuit). All three were read in full from rendered pages.
3. **Implementable from the documents found** (schematic with values + IC datasheet): the **record/playback electronics of the RN-305/RN-405** (schematic + LA4168M), **of the RQ-L31 and RQ-L30** (vector schematic + LA4168M + adjustment table with targets + W&F 0.12 % WRMS + record/play response 250–6300 Hz), i.e. mic amp gain, ALC width/onset, VOX threshold and hysteresis, playback EQ amp gain, power amp gain and clip, governor reference and regulation, DC bias with magnet erase.
4. **Only describable** (figures or listings, no readable schematic in hand): RN-109, RN-Z36, RN-102, RN-106D, RN-105D, RN-502 (ManualsLib text layers only), RQ-2102, RQ-330, RQ-355, RQ-L309, RQ-L319, RQ-L305, RQ-L340, the RR-830/RR-930 transcribers. The ManualsLib PDFs exist but were not downloaded (viewer text only).
5. **No document found gives S/N with a weighting, or frequency response with tolerances at each microcassette speed.** Microcassette figures are sparse: 540–3400 Hz at 2.4 cm/s (RN-102, RN-502), 200–6000 Hz (RN-Z36), flutter 0.58 % at 1.2 cm/s (RN-106D).
6. **Most promising components**: (a) **RQ-L31** (2002): the most complete single source (vector schematic, parts list with chip-value codes, W&F 0.12 % WRMS, two speeds 4.8 / 1.6 cm/s, R/P response 250–6300 Hz, tape-speed and azimuth procedures with test-tape frequencies and a 3000 ± 50 Hz target); (b) **RN-305 / RN-405** microcassette (2000): the 2.4 / 1.2 cm/s two-speed mechanism with the same LA4168M and a clean schematic; (c) **LA4168M** as a reusable "voice-recorder IC" stage (ALC 40–67 dB width, VOX start −85 dBm with 6 dB hysteresis, governor K = 50); (d) **AN7086S + AN6612** as the Panasonic-silicon alternative (VAS 14 µVrms, ALC width 1.5 %, governor ±10 rpm/V with motor constants); (e) **RQ-331** (1978): a discrete-transistor mini-cassette recorder with a schematic, a distinct older circuit.
7. Distinct mechanisms worth noting: **DC bias with permanent-magnet erase** on most voice models checked (RQ-L30/L31/L305/L309/L319, RQ-2102; inferred for the RN-305 from its schematic's erase-head symbol), while the RQ-L340's text layer reads "AC bias, magnet erase" (an OCR reading, unverified against its PDF); **electronic governor motors** throughout; **"x3" long-play by a second speed preset** (RQ-L31: VR3 "tape speed x 3", 1.6 cm/s); continuously variable **TAPE SPEED** control (RQ-L340, RQ-L309-class); **fast playback** via the LA4168M's speed-up pin (RN-305/405 "VAS/FAST PLAY").
8. Mechanism figures available: pinch-roller pressure (140 ± 20 g RN-106D; 160 ± 40 g RQ-L340/RQ-330), take-up torques, W&F (0.12 % RQ-L30/L31; 0.25 % RQ-330, RQ-2102; < 0.3 % RQ-L309/L319; < 0.5 % RQ-355; 0.58 % RN-106D at 1.2 cm/s). The W&F method is stated only as "WRMS" and once "JIS … use test tape" (RQ-330).
9. Not found: any Panasonic microcassette service manual with S/N, any head inductance/gap data, any motor datasheet other than the AN6612's example constants, any teardown of an RN-series unit.
10. Side finding outside this scope: the June 2000 TSN CD also holds **RR-QR240E / RR-QR80E IC-recorder supplements** (replacement-part changes only; they point to the RR-QR240 service manual, order no. AD9909225C5). Not useful for modeling, but the order number is new to the register.

### 2. Model table

Years: from the © line where shown, otherwise **inferred from the order number's year-month digits** (e.g. AD0004… = 2000-04; the rule holds for every model where both are visible: RN-305 AD0004…/© 2000, RQ-L319 TD9801…/1998, RQ-L31 TD0202…/© 2002). "SM" service manual, "OM" owner's manual, "DS" IC datasheet.

| Model | Year | Format, speeds | Stated response / W&F / S/N | ALC / VOX | Speed control | Amp / ICs, speaker | Documentation (opened) | Rating |
|---|---|---|---|---|---|---|---|---|
| **RQ-L31E** | 2002 | mini-cassette, 4.8 / 1.6 cm/s, DC bias, magnet erase | 180–7000 Hz (Normal); **R/P 250–6300 Hz**; **W&F 0.12 % WRMS**; S/N not stated | LA4168M ALC and VOX (VAS knob) | governor; VR2 normal, VR3 "x 3"; 3000 ± 50 Hz target | LA4168M, 450 mW, 3.6 cm 4 Ω | SM (TSN 2002 CD): vector schematic PDF, PCB PDF, parts list with values, adjustment table | **5** |
| **RQ-L30** | 2001 | mini-cassette, 4.8 cm/s, DC bias | 180–7000 Hz; **W&F 0.12 % WRMS** | LA4168M ALC, VAS switch | governor, VR2 | LA4168M, 450 mW, 3.6 cm 4 Ω | SM (TSN 2001 CD): vector schematic PDF, spec, parts | **4** (no adjustment table seen) |
| **RN-305 / RN-405** | 2000 | microcassette, 2.4 / 1.2 cm/s | none stated (spec page has no response line) | LA4168M ALC; VAS (VR101 volume/VAS level) | governor; VR301 2.4 cm/s trim; S5 speed switch (R322/R323/R324); FAST PLAY | LA4168M, 3.6 cm 4 Ω | SM (TSN 2000 CD): clean TIF schematic with all values and DC voltages; simplified (no adjustment table) | **4** |
| RN-502 / RN-202 | 1994 | microcassette, 2.4 / 1.2 cm/s | 540–3400 Hz at 2.4 cm/s | VAS level control | — | LA4168M; 300 mW (SM), 360 mW (OM); 3.6 cm 4 Ω | SM 10 pp (ManualsLib text; elektrotanya PDF listed); OM RQT2452-G | 3 (schematic exists, not in hand) |
| RN-102 | 1990 | microcassette, 2.4 / 1.2 | 540–3400 Hz at 2.4 cm/s | — | — | AN7085NSE2, AN6612SE2; 300 mW | SM AD9010286C8, 11 pp, schematic (ManualsLib text) | 2 |
| RN-106D | 1989 | microcassette, 2.4 / 1.2 | **flutter 0.58 % at 1.2 cm/s** | VAS | — | AN7085NSE2, motor control (AN6612, OCR "ABS612SE2"); 300 mW | SM AD8904086C1, 9 pp; mech specs | 2 |
| RN-105D | 1989 | microcassette, 2.4 / 1.2 | — | — | — | 300 mW, 4.5 cm 10 Ω | SM AD8904079C1, 9 pp (page 1 seen) | 1 |
| RN-109 | 1984 | microcassette, SP 2.4 / LP 1.2 | — | VAS (section) | electrical governor motor | 3.6 cm | SM GAD84035403C1, 10 pp | 2 |
| RN-190 | 1986 | microcassette | — | — | — | — | SM supplement GAD8609447A2 (refers to GAD8606582C1) | 1 |
| RN-Z36 | 1985 | microcassette, SP 2.4 / LP 1.2, one AAA | **200–6000 Hz** | — | governor | 15 mW recorder + 300 mW speaker unit | SM GAD8507518C8, 12 pp, incl. "Measurement and Adjustment" p. 7 | 2 |
| RQ-L340 | 1993 | mini-cassette 4.8 cm/s, AC bias | — | AN7086 VAS; TONE/SLE | **variable TAPE SPEED control** | AN7086SE1, AN6612SE2; 400 mW; 4.5 cm 8 Ω | SM TD93010444C2, 9 pp (text only); OM on IA | 3 (with the two DS) |
| RQ-L305 | ~1990s | mini-cassette 4.8 cm/s, DC bias | — | AN7086 | governor | AN7086ST, AN6612ST, 2SB1132; 400 mW | SM 3 pp (ManualsLib) | 3 (with the two DS) |
| RQ-L309 | 1996 | mini-cassette 4.8 cm/s, DC bias | W&F < 0.3 % WRMS | — | governor | 400 mW, 4.5 cm 8 Ω | SM TD96050555C2, 6 pp | 2 |
| RQ-L319 | 1998 | mini-cassette 4.8 cm/s, DC bias | 180–7000 Hz; W&F < 0.3 % WRMS | — | governor; "tape speed VR center" | 400 mW | SM TD98010600C2, 9 pp | 2 |
| RQ-330 | 1987 | mini-cassette 4.8 cm/s | 180–7000 Hz; **W&F < 0.25 % WRMS (JIS, test tape)** | — | governor | 4 cm 8 Ω | SM TD87070264C2, 6 pp, schematic | 2 |
| RQ-355 | 1983 | mini-cassette 4.8 cm/s | 200–8000 Hz; W&F < 0.5 % (RMS) | voice-activated | **tape speed control** | 500 mW | SM ARD83080273C8-05, 9 pp | 2 |
| RQ-331 | 1978 | mini-cassette (6 V) | — | — | governor; motor speed adj. VR | discrete: 2SC2021 transistors; 2.5" speaker | SM ARD-7803014C on Internet Archive (300 ppi scan, schematic) | 3 |
| RQ-2102 | 1990 | standard cassette 4.8 cm/s, DC bias, 6 V | 180–8000 Hz; W&F 0.25 % WRMS | — | — | 550 mW, 8 cm 8 Ω | SM MD900301, 11 pp (ManualsLib text; elektrotanya PDF 4.2 MB listed) | 2 |
| RR-830 / RR-930 | — | transcribers (cassette / microcassette) | — | — | variable speed | — | OM only (ManualsLib); SM offered for sale only | 1 |

### 3. Source entries

#### A. Primary: Panasonic service CD-ROMs (Internet Archive)

1. **TSNSystem Technical Service Navigation, June 2000 Vol. 1 [CDR00061A0X]** (Panasonic/National/Technics consumer service CD).
   URL: [archive.org/details/tsnsystem-technical-service-navigation-june-2000-vol.-1-cdr00061a0x](https://archive.org/details/tsnsystem-technical-service-navigation-june-2000-vol.-1-cdr00061a0x).
   *Manufacturer service CD-ROM (ISO 9660, 420 MB); Primary; read (the two microcassette manuals extracted and their schematics viewed).*
   Contents used: `/PANADOC/SVCDOC/1AV/14AUDIO/149O_AD/RN305P/` and `/RN405P/`: **RN-305 service manual, order no. AD0004095C1** (© 2000, "MZ20 Mechanism Series", area P) and **RN-405, AD0004096C1**. Each holds an SGML text file (spec, battery life 5.53 h record / 3.27 h play EIAJ for the RN-305) and one schematic TIF (`305SC1.TIF` 2244 × 1827; `405SCE.TIF` 4252 × 3999). The schematics are clean drawings with **every part value and DC voltages in record and playback**: IC1 LA4168M-TE-L "pre/power amp/motor control"; mic input R1/R3 1.5 kΩ, C200 10 µF supply filter, Q4 2SC4555 mic muting; R/P head with C102 6800 pF; record feed R4 6.8 kΩ; EQ feedback network R104 33 kΩ / C104 1000 pF / C105 1 µF; volume/VAS level VR101 10 kΩ(B) with R2 5.6 kΩ, R105 2.2 kΩ, C8 0.01 µF, C111 0.015 µF, R108 5.6 kΩ, R19 3 kΩ, R39 5.6 kΩ; ALC pin network R201 470 kΩ ∥ C203 33 µF; governor VR301 5 kΩ(B) (2.4 cm/s trim), R320 43 kΩ, R321 10 kΩ, R303 330 Ω, R330 1.2 kΩ, sense R5/R6 1 Ω; speed switch S5 with R322 3 kΩ, R323 1.5 kΩ (1.8 kΩ on the RN-405), R324 56 Ω; motor FF/REW R304 4.7 Ω; speaker 3.6 cm 4 Ω; battery currents no signal 121 mA, playback (VR max) 330 mA, record 118 mA (RN-305). The RN-405 adds an external mic jack, an LED (D201), and an auto-stop / tape-end detector with a photo-interrupter Z1 and Q303–Q307. The erase head is drawn without a drive connection (a permanent-magnet erase head, inferred from the drawing; the spec text does not say).
   Also on this CD: `RRQR240E`, `RRQR80E` (IC-recorder supplements, parts changes only; RR-QR240 SM order no. AD9909225C5).
   Access: the ISO is a direct download (`…/TSNSystem%20Technical%20Service%20Navigation%20June%202000%20Vol.%201%20%5BCDR00061A0X%5D.iso`); macOS `bsdtar` lists nothing in it; **pycdlib** (`uv run --with pycdlib`) walks and extracts it. TIFs convert with `sips -s format png`. No adjustment table in these "simplified" manuals.
   **Rating 4** for the electronics (implementable with the LA4168M datasheet); the mechanism has no figures (no W&F, no response).

2. **TSNSystem Technical Service Navigation, Mar. 2002 Vol. 1 [CDR02031A0X]**.
   URL: [archive.org/details/tsnsystem-technical-service-navigation-mar.-2002-vol.-1-cdr02031a0x](https://archive.org/details/tsnsystem-technical-service-navigation-mar.-2002-vol.-1-cdr02031a0x).
   *Manufacturer service CD-ROM (714 MB ISO); Primary; read (the RQ-L31E manual extracted; spec, adjustment and parts list read; schematic text layer read).*
   Contents used: `/PANADOC/SVCDOC/1AV/14AUDIO/143HP_ST/RQL31E/`: **RQ-L31E service manual, order no. TD02020749C2**, © 2002, "TN9X Mechanism Series". Full manual: `SCHE_ALL.PDF` (2-page **vector** schematic; part values, IC pin names and DC voltages are extractable text), `PCB_ALL.PDF`, `SPEC.TIF`, `MEASE.TIF` (adjustment), `NOTES.TIF`, disassembly, deck and cabinet drawings, and the SGML parts list with Panasonic chip-part codes (values decodable, e.g. ERJ3GEYJ474V = 470 kΩ; C22 33 µF and R26 470 kΩ at the ALC pin as on the RN-305).
   Specifications (SPEC.TIF, read from the image): tape speed **4.8 cm/s / 1.6 cm/s**; **wow & flutter 0.12 % (WRMS)**; frequency range **180–7000 Hz (Normal)**; **Rec/Playback 250–6300 Hz**; recording system **DC bias, magnet erase**; electric governor motor; power output 450 mW (RMS max); mic 0.25 mV (150–600 Ω); speaker 3.6 cm 4 Ω; monitor system "Fix".
   Adjustment (MEASE.TIF): preconditions volume max, 3 V DC, tape-speed VR at centre, heads/capstan/pinch roller clean; **head azimuth** with test tape QZZCFM (6.3 kHz, −10 dB) for maximum output at the 8 Ω earphone jack; **tape speed** with QZZCWAT (3 kHz, −10 dB), VR2 (normal) and VR3 ("x 3"), **specification 3000 ± 50 Hz** (± 1.7 %); load and measurement hook-up in Fig. 3 (R = 8 Ω).
   Parts: IC1 LA4168M-TE-L; Q1/Q2 2SK1958; thermistors R32/R36 ERTJ1VT102J and R38 in the governor path (temperature compensation, inferred from the part type); motor ass'y 190908306, flywheel 190905306, main belt 19090812 (no motor data).
   **Rating 5**: the most implementable voice-recorder document found (schematic with values, IC known and documented, measured W&F, R/P response, speed target and test-tape levels). Missing: S/N, head data, response at 1.6 cm/s.

3. **TSNSystem Technical Service Navigation, May 2001 Vol. 1 [CDR01051A0X]**.
   URL: [archive.org/details/tsnsystem-technical-service-navigation-may-2001-vol.-1-cdr01051a0x](https://archive.org/details/tsnsystem-technical-service-navigation-may-2001-vol.-1-cdr01051a0x).
   *Manufacturer service CD-ROM (570 MB ISO; the item description lists no models); Primary; read (RQ-L30 extracted).*
   Contents used: `/panadoc/svcdoc/1av/14audio/142r_cas/rql30pc1/`: **RQ-L30 service manual (Canada), order no. TD01040708C1**, © 2001, TN9X mechanism. Vector schematic `30sche.pdf` (text layer: LA4168M pin names, R/C values, DC voltages, VAS switch SW2, Q1 2SD1048, VR2), spec TIF (4.8 cm/s; **W&F 0.12 % (WRMS)**; 180–7000 Hz (Normal); DC bias, magnet erase; 450 mW), parts list, notes. No adjustment table in this edition (it is a short form).
   **Rating 4.**

4. **TSNSystem Technical Service Navigation, Apr. 2004 Vol. 1 [CDR04041A0X]**.
   URL: [archive.org/details/tsnsystem-technical-service-navigation-apr.-2004-vol.-1-cdr04041a0x](https://archive.org/details/tsnsystem-technical-service-navigation-apr.-2004-vol.-1-cdr04041a0x).
   *Service CD-ROM; Primary; listing read only (ISO not downloaded).* Lists stereo mini-cassette players (RQ-SX57, RQ-SX89V, RQ-SX95F) and a radio-cassette supplement; no voice recorder. **Rating 1** for this scope.

5. **Panasonic RQ-331 Service Manual, order no. ARD-7803014C** ("RQ-332 mechanism series").
   URL: [archive.org/details/panasonic-rq-331-service-manual](https://archive.org/details/panasonic-rq-331-service-manual).
   *Service manual scan (300 ppi, 11 MB PDF); Primary; partly read (OCR text; the schematic image not viewed).*
   Contents: catalog specs (6 V, four AA; electrical governor motor; push-button one-touch recording with auto-stop; MIC 200–600 Ω, REMOTE, MONITOR 8 Ω; 2½" speaker), parts (record/playback/erase head WY036AZ, built-in mic WM034X, motor speed adjustment VR), schematic of a **discrete-transistor** recorder (2SC2021 transistors; VR1 100 kΩ(A), C18 0.0082 µF, C19 0.001 µF visible in OCR). Year 1978 inferred from the order number.
   Access: the `_djvu.txt` answered HTTP 500 twice, then 200 on a later try; the metadata API worked throughout. **Rating 3**: a full schematic of an older, IC-free design, but no measured figures seen.

#### B. Primary: IC datasheets

6. **Sanyo LA4168M, "Record/Playback System for Microcassette and Compact Cassette Recorders", ordering number EN5660** (catalog July 1997, 6 pp).
   URL: [cieri.net LA4168M (Sanyo).pdf](https://www.cieri.net/Documenti/Data%20Sheets/Audio%20signal%20and%20audio%20processors%20ICs/LA4168M%20(Sanyo).pdf).
   *IC datasheet; Primary; read (pages 1–2 rendered and read in full; block diagram, test circuit and sample application circuit pages present, not transcribed).*
   Provides (VCC 3.0 V, 0 dBm = 0.775 V): EQ amp open-loop 85 dB (C_NF 100 µF), closed-loop 52 dB, Vo max 0.75 V, input noise 1.0 µV (2.2 kΩ, DIN audio); mic amp 45 dB (ALC off), input resistance 32 kΩ, noise 2.0 µV; mic + rec 65.5 dB, Vo max 1.0 V (THD 5 %), output noise 3.5 mV; power amp BTL 24 dB / single 23 dB, **350 mW BTL at 10 % THD** (4 Ω), output noise 20 µV; **ALC width 40 min / 67 typ dB** (from ALC onset to 5 % THD), ALC output 0.43 V at −40 dBm, **ALC start input −74 dBm typ (−79 … −69)**; **VOX operation start −85 dBm typ (−90 … −80) at volume (10 kΩ) max, hysteresis 6 dB typ (3 … 9)**; auto-stop 0.7 V; governor Vref 1.25 V, **current division ratio K = 50**, reference voltage/supply 0.2 %/V, K/supply 0.3 %/V, speed-up CONT pin 0.41 V; functions incl. "a circuit that increases the speed of the motor during playback" and a VOX that switches the governor (motor) on and off, with the threshold set by the volume control.
   Access: the cieri.net PDF renders with pypdfium2; text extraction gives the tables out of order (render to read). Mirrors: datasheetq, alldatasheet, jiepei (not opened).
   **Rating 5** for the electronic stages (ALC, VOX, gains, clip, governor) when paired with schematic 1, 2 or 3. ALC attack/release time constants are set by external parts (the schematics give them) and are not stated as times.

7. **Panasonic AN7086S, "Recording/Playback Pre-/Power Amplifier IC for 3V Microcassette"** (5 pp; PDF created 1997-12-16).
   URL: [cieri.net AN7086S (Panasonic).pdf](https://www.cieri.net/Documenti/Data%20Sheets/Audio%20signal%20and%20audio%20processors%20ICs/AN7086S%20(Panasonic).pdf).
   *IC datasheet; Primary; read (page 2 rendered and read; pages 1, 3–5 by text extraction).*
   Provides (VCC 3 V, 1 kHz): pre-amp open-circuit gain 74 dB, THD 0.1 %, Vo max 0.6 Vrms, input noise 1 µVrms (1 kΩ, DIN audio); recording amp 72.5 dB (69.5–75.5), THD 1.3 %, Vo max 1.08 Vrms (5 % THD), output noise −46 dBV; power amp 41 dB, **350 mW** (10 % THD, 8 Ω), output noise −74 dBV; **VAS operation input 14 µVrms typ (8–18)** at the mic input; **ALC effective voltage −4.5 dBV** (−60 dBV in), **effective width 1.5 % typ** (−30 dBV in); ripple rejection −70 / −40 dBV; supply 1.8–4.5 V; block diagram gains 55 / 20 / 24 / 14 / 14 dB; characteristic curves incl. the ALC input/output curve (−80 … 0 dBV) and an application circuit with the head.
   **Rating 4**: complete IC data; needs a host schematic (RQ-L305 / RQ-L340 PDFs not yet downloaded).

8. **Panasonic AN6612 / AN6612S "Motor Control Circuits"** (2 pp; PDF 2005/2011 re-issue).
   URL: [static.chipdip.ru DOC000302302.pdf](https://static.chipdip.ru/lib/302/DOC000302302.pdf).
   *IC datasheet; Primary; read (both pages rendered).*
   Provides: "electronic governor circuits … for … a small tape recorder"; VCC 1.8–8 V; reference voltage 1.32 V typ (1.24–1.40); starting current ≥ 250 mA; **speed vs supply ≤ 10 rpm/V** (1.8–4 V, 72 mA); time drift 0.1 % (15 s–10 min); temperature −0.035 %/°C; current-limit 0.7 V; application circuit (0.5 Ω and 10 Ω sense, 910 Ω, VR 3 kΩ B, 10 kΩ, a stepped VR 500 Ω B with 1 kΩ for speed steps, 2SA881A, 0.47 µF, 2.2 µF) with **motor constants Ra 4.9 Ω, Ke 0.4 mV/rpm, Kt 29 g·cm/A**. Used in RN-102, RN-106D, RQ-L305, RQ-L340.
   Access: manualmachine.com answered 403. **Rating 4** for a governor (drift) model; gives no flutter spectrum.

#### C. Service manuals seen as text layers (ManualsLib viewer; PDFs not downloaded)

Common access note: ManualsLib page text is an OCR layer, often garbled; the schematic images were not viewed. "Download this manual" needs a browser session.

9. **RN-109 SM, GAD84035403C1** (RN-006 mechanism series, US/Canada), 10 pp. [manualslib 4192671](https://www.manualslib.com/manual/4192671/Panasonic-Rn-109.html). *SM; Primary; partly read.* SP 2.4 / LP 1.2 cm/s; "Electrical governor motor"; mic 0.25 mV; FF/REW ≈ 120 s (RT-60MC); 3.6 cm speaker; sections incl. "Voice Activated Recording". **Rating 2.** A supplement (GAD85015403S1) is mentioned on [manualslib 3380692](https://www.manualslib.com/manual/3380692/Panasonic-Rn-109.html) (search result only, not opened).
10. **RN-502 SM** (MZ20 mechanism; order no. OCR-garbled "AD9406…"), 10 pp. [manualslib 4366556](https://www.manualslib.com/manual/4366556/Panasonic-Rn-502.html). *SM; Primary; partly read.* p. 5 schematic, p. 7 measurement and adjustment; **frequency range 540–3400 Hz at 2.4 cm/s**; 300 mW; 3.6 cm 4 Ω; ICs LA4168M-TE-L; transistors 2SB1218, 2SC4555, 2SD1819, DTC144TU (the RN-305's set: same platform). Also [elektrotanya RN-502](https://elektrotanya.com/panasonic_rn-502.pdf/download.html) (PDF, 1.9 MB, 10 pp; opened listing only). **Rating 3.**
11. **RN-102 SM, AD9010286C8** (RN-105 mechanism, MZ20), 11 pp. [manualslib 4338568](https://www.manualslib.com/manual/4338568/Panasonic-Rn-102.html). *SM; Primary; partly read.* 540–3400 Hz at 2.4 cm/s; 300 mW; ICs AN7085NSE2, AN6612SE2, UN5115. **Rating 2** (the AN7085 has no datasheet found).
12. **RN-106D SM, AD8904086C1**, 9 pp. [manualslib 4259472](https://www.manualslib.com/manual/4259472/Panasonic-Rn-106d.html). *SM; Primary; partly read.* Mechanical: playback torque 4.5–8.5 g·cm (2.4 cm/s), FF 4.5–10 g·cm, REW > 30 g·cm, **pinch roller 140 ± 20 g**, **flutter 0.58 % (1.2 cm/s)**; AN7085NSE2. **Rating 2.**
13. **RN-105D SM, AD8904079C1**, 9 pp. [manualslib 3338905](https://www.manualslib.com/manual/3338905/Panasonic-Rn-105d.html). *SM; Primary; page 1 only.* Speeds 1.2 / 2.4; 300 mW. **Rating 1.**
14. **RN-190 SM supplement, GAD8609447A2** (refers to GAD8606582C1), 9 pp. [manualslib 3338902](https://www.manualslib.com/manual/3338902/Panasonic-Rn-190.html). *Supplement; Primary; partly read.* Differences only. **Rating 1.**
15. **RN-Z36 SM, GAD8507518C8**, 12 pp. [manualslib 3387103](https://www.manualslib.com/manual/3387103/Panasonic-Rn-Z36.html). *SM; Primary; partly read.* Schematic p. 6, **Measurement and Adjustment p. 7** (not seen); **200–6000 Hz**; SP 2.4 / LP 1.2; one AAA (1.5 V); 15 mW; separate 300 mW speaker unit. **Rating 2** (would rise if the PDF has the adjustment table).
16. **RN-405 SM, AD0004096C1**, 3 pp. [manualslib 4561454](https://www.manualslib.com/manual/4561454/Panasonic-Rn-405.html). Same document as entry 1's RN-405 (the TSN copy has the schematic). **Rating 1** here; use entry 1.
17. **RQ-L340 SM, TD93010444C2**, 9 pp. [manualslib 4372698](https://www.manualslib.com/manual/4372698/Panasonic-Rq-L340.html). *SM; Primary; partly read.* AC bias, magnet erase; 4.8 cm/s; 400 mW; pinch roller 160 ± 40 g, play take-up 36 ± 14 g·cm, FF/REW 110 ± 50 g·cm; **AN7086SE1 (pre-EQ amp) and AN6612SE2 (motor driver)**; variable TAPE SPEED control, VAS, TONE/SLE. OM on [archive.org/details/panasonic_RQ-L340](https://archive.org/details/panasonic_RQ-L340) (verified: operating instructions, multilingual). **Rating 3** (4 once the schematic page is read, with entries 7–8).
18. **RQ-L305 SM**, 3 pp, order no. not shown. [manualslib 3989932](https://www.manualslib.com/manual/3989932/Panasonic-Rq-L305.html). *SM; Primary; partly read.* DC bias; 400 mW; **AN7086ST, AN6612ST**, 2SB1132. **Rating 3** with entries 7–8.
19. **RQ-L309 SM, TD96050555C2** (1996), 6 pp. [manualslib 4366221](https://www.manualslib.com/manual/4366221/Panasonic-Rq-L309.html). *SM; Primary; partly read.* W&F < 0.3 % WRMS; DC bias, magnet erase; 400 mW; pinch roller 160 g. **Rating 2.**
20. **RQ-L319 SM, TD98010600C2** (1998), 9 pp. [manualslib 4366373](https://www.manualslib.com/manual/4366373/Panasonic-Rq-L319.html). *SM; Primary; partly read.* 180–7000 Hz (Normal); W&F < 0.3 % WRMS; DC bias; pinch 160 g; torques 60–160 g·cm; adjustment preconditions as RQ-L31. **Rating 2.**
21. **RQ-330 SM, TD87070264C2** (1987), 6 pp. [manualslib 3999492](https://www.manualslib.com/manual/3999492/Panasonic-Rq-330.html). *SM; Primary; partly read.* 180–7000 Hz; **W&F (JIS) < 0.25 % W RMS, "use test tape"** (the only stated method); pinch 160 ± 40 g; torques. Schematic p. 4 (garbled in text). **Rating 2.**
22. **RQ-355 SM, ARD83080273C8-05** (1983; "voice-activated mini cassette recorder with tape speed control", RQ-352 mechanism), 9 pp. [manualslib 3333583](https://manualslib.com/manual/3333583/Panasonic-National-Rq-355.html). *SM; Primary; partly read.* 200–8000 Hz; W&F < 0.5 (RMS); 500 mW; governor. **Rating 2.**
23. **RQ-2102 SM, MD900301** ("New mechanism series (SG-20)"), 11 pp. [manualslib 4344420](https://www.manualslib.com/manual/4344420/Panasonic-Rq-2102.html); PDF listed at [elektrotanya](https://elektrotanya.com/panasonic_rq-2102_cassette_recorder.pdf/download.html) (4.2 MB, 11 pp; listing opened). *SM; Primary; partly read.* Standard cassette, 4.8 cm/s, **DC bias**, 180–8000 Hz (partly garbled), **W&F 0.25 % WRMS**, 550 mW, 8 cm 8 Ω, mic 0.32 mV, play torque 55 g·cm. Alignment page not in the text. **Rating 2** (3 once downloaded).

#### D. Owner's manuals and listings

24. **RN-202/502 operating instructions, RQT2452-G** (F0494…), 5 pp. [manualslib 4350636](https://www.manualslib.com/manual/4350636/Panasonic-Rn-202.html). *OM; Primary; partly read.* Controls incl. FAST PLAY, TONE, VOLUME/LEVEL; "360 mW (max.)". **Rating 1.**
25. **RN-505 operating instructions, RQT5406-P**, 2 pp. [manualslib 1195017](https://www.manualslib.com/manual/1195017/Panasonic-Rn-505.html). *OM; Primary; partly read.* "VAS/FAST PLAY" switch; "The recording level is automatically adjusted". **Rating 1.**
26. **RN-405 operating instructions** (covers RN-305, RN-202), 7 pp. [manualslib 4007056](https://www.manualslib.com/manual/4007056/Panasonic-Rn-405.html); also IA [manuallib-id-2654594](https://archive.org/details/manuallib-id-2654594) (metadata verified). **Rating 1.**
27. **RN-502 user manual** on IA, [Panasonic_RN-502_Microcassette_Recorder_User_Manual](https://archive.org/details/Panasonic_RN-502_Microcassette_Recorder_User_Manual) (verified; its OCR text contains a Japanese-labelled section: 標準 / 2倍 speed modes). **Rating 1.**
28. **RN-190 operating instructions** on IA, [panasonic-rn-190-operating-instructions](https://archive.org/details/panasonic-rn-190-operating-instructions) (metadata verified, not read). **Rating 1.**
29. **RR-930 operating instructions**, 12 pp. [manualslib 3422455](https://www.manualslib.com/manual/3422455/Panasonic-Rr-930.html). *OM; read the landing page only;* no figures. RR-830 OM at [manualslib 777035](https://www.manualslib.com/manual/777035/Panasonic-Rr-830.html) (search result, not opened). **Rating 1.**
30. **Radiomuseum, RN-202** (1994; 68 × 124 × 24 mm, 130 g; no schematic). [radiomuseum.org](https://radiomuseum.org/r/panasonic_microcassette_recorder_rn_2.html). *Collector database; Secondary; read.* **Rating 1.** RN-006 and RN-001 pages exist ([RN-006](https://www.radiomuseum.org/r/panasonic_microcassette_recorder_rn_006.html), [RN-001](https://radiomuseum.org/r/panasonic_micro_cassette_recorder_rn_001.html); search results, not opened).
31. **elektrotanya listings** (opened as listings only): [RN-105 SM, 8 pp, 4.1 MB](https://elektrotanya.com/panasonic_rn-105_sm.pdf/download.html); [RN-3050, RAR 209.7 KB, no preview](https://elektrotanya.com/panasonic_rn-3050.rar/download.html) (its size matches the TSN SGML package of entry 1; inference). Search results not opened: RN-305 RAR, RN-405 RAR, RN-4053P RAR. Downloads need a tick box; 10 files/day. **Rating 1** each as listings.

### 4. Searched without result, and dead ends

- **S/N with a weighting** for any Panasonic microcassette or mini-cassette voice recorder: not found in any spec page read (TSN or ManualsLib text).
- **Frequency response at 1.2 cm/s / 1.6 cm/s**: not stated anywhere read; only 2.4 cm/s (540–3400 Hz) and normal speed.
- **AN7085 datasheet** (RN-102, RN-106D): only an ic-phoenix listing with auto-generated text; no Panasonic PDF found.
- **Head data** (inductance, gap, track width) and **motor datasheets**: none; only the AN6612's example motor constants and part numbers.
- **RN-series teardowns / repair photos**: none; stereo2go threads cover Sony M-1000 and an RN-Z500 forum complaint that no high-resolution board photos exist.
- **RR-830 / RR-930 transcriber SMs**: only offered for sale (hifishark listing via search); OMs on ManualsLib.
- **hifiengine**: no Panasonic voice-recorder entries returned.
- **Japanese search** (ナショナル マイクロカセット RN サービスマニュアル 回路図): returned the same ManualsLib/radiomuseum pages; no Japanese-language manual.
- **manualmachine.com** (AN6612): HTTP 403. **WebFetch on PDFs**: returns "binary" summaries; save the PDF and render locally instead (pypdfium2 + pillow).
- **archive.org `_djvu.txt`** for the RQ-331: HTTP 500 twice, later 200 (transient).
- **macOS `bsdtar` / `hdiutil`** cannot list the TSN ISOs; **pycdlib** can. The 2001 ISO uses lowercase paths, the 2000/2002 ones uppercase.
- Not tried within the budget: ManualsLib PDF downloads (RN-502, RN-Z36, RQ-L340, RQ-L305, RQ-2102 schematics and adjustment tables), elektrotanya downloads, the Apr 2004 TSN ISO, further TSN volumes not on the Internet Archive.

#### Local copies (scratchpad, untrusted downloads)

`dl/tsnx/{RN305P,RN405P,RQL30,RQL31E,RRQR240E,RRQR80E}/` (extracted manuals), `dl/la4168/la4168m.pdf`, `dl/an7086/an7086s.pdf`, `dl/an6612/an6612.pdf`, renders `305SC1.png`, `405SCE.png`, `rql31_SPEC.png`, `rql31_MEASE.png`, `rql30_spec.png`, `r_la4168_p1-6.png`, `r_an7086_p2.png`, `r_an6612_p1-2.png` under `/private/tmp/claude-501/-Users-chance-Code-DR60Recorder/72c83af8-2c0c-4705-8e6e-53427e6d134d/scratchpad/`. The TSN ISOs (`dl/tsn*/tsn.iso`, 1.7 GB in total) can be deleted.
