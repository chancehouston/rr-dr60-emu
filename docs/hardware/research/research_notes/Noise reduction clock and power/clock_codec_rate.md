# The RR-DR60's clock: codec rate, accuracy and the effects of clock error

Researcher notes, 2026-10-17. Scope: the clock only. The playback SPEED CONTROL mechanism and the schematic re-read are covered by other researchers. Tags used below: **Fact** (sourced, with URL), **Calc** (my arithmetic, shown), **Inference** (reasoning, not evidence).

Project context read first: `docs/hardware/sources.md` (S-002, S-003, S-005), `docs/hardware/assumptions.md` (A-011, A-026, A-027, A-040), `docs/hardware/field-measurement.md`. Downloads are under `/private/tmp/claude-501/-Users-chance-Code-DR60Recorder/baebfed1-e821-4f45-bb42-e63af34626c2/scratchpad/clockdl/` (untrusted data). The scripts are in `.../scratchpad/clockscripts/` (`rates.py` reproduces every Calc table below).

**Headline: the MSM7702's own datasheet pages 4–17 were found and read.** The mirror `https://pdf.jiepei.com/msm7702-9605035.html?page=N` (N = 1…17) serves the text of all 17 pages of E2U0018-28-81. The lowercase host/path matters: `mpdf.jiepei.com/MSM7702-…` (the URL in sources.md) returns empty pages or a certificate error, and the uppercase `pdf.jiepei.com/MSM7702-…` answers 301. This closes the S-003 gap that sources.md lists as open.

---

## 1. Clocking constraints of the OKI MSM7702 codec family

### Takeaway
The MSM7702's own pages confirm what S-005 (MSM7717) said. The internal PLL is triggered by XSYNC and "synchronizes all timing signals" (BCLK only shifts data). XSYNC and RSYNC must be "synchronized in phase with BCLK". The sync is guaranteed at 8 kHz ± 50 ppm and allowed from 6.0 to 10.0 kHz with no guaranteed characteristics. BCLK is listed as 13 discrete rates, and 500 kHz is not among them. Allowable jitter on XSYNC, RSYNC and BCLK is 1 µs. Nothing in the datasheet mentions 6 kHz operation except the 6.0 kHz minimum of the sync range. The RR-DR60's 6 kHz therefore sits exactly at the lower corner of the allowed range.

### Cited findings
- **XSYNC (MSM7702 p. 5, verbatim):** "This synchronizing signal triggers the PLL and synchronizes all timing signals of the transmit section. This synchronizing signal must be synchronized in phase with BCLK. The frequency should be 8 kHz ±50 ppm to guarantee the AC characteristics which are mainly the frequency characteristics of the transmit section. However, if the frequency characteristic of an applied system is not specified exactly, this device can operate in the range of 8 kHz ±2 kHz, but the electrical characteristics in this specification are not guaranteed." — [MSM7702 datasheet p. 5 (jiepei mirror)](https://pdf.jiepei.com/msm7702-9605035.html?page=5)
- **RSYNC (p. 5):** the same wording for the receive section ("must be synchronized in phase with the BCLK", 8 kHz ± 50 ppm guaranteed, ± 2 kHz operable, not guaranteed). Holding XSYNC or BCLK at a static "1" or "0" puts both sections into power-saving. — [MSM7702 p. 5](https://pdf.jiepei.com/msm7702-9605035.html?page=5)
- **BCLK (p. 5, verbatim):** "Shift clock signal input for the PCMIN and PCMOUT signal. The frequency, equal to the data rate, is 64, 96, 128, 192, 256, 384, 512, 768, 1024, 1536, 1544, 2048, or 200 kHz." — [MSM7702 p. 5](https://pdf.jiepei.com/msm7702-9605035.html?page=5)
- **Block diagram (p. 2):** the PLL block is fed from XSYNC and BCLK; RSYNC goes to a separate receive-timing block (RTIM). Signal path: RC LPF → 8th-order BPF → A/D (auto-zero) → TCONT; D/A → 5th-order LPF → AOUT. — [MSM7702 p. 2](https://pdf.jiepei.com/msm7702-9605035.html?page=2)
- **Recommended operating conditions (p. 8; table text garbled in extraction, values read from the row order):** BCLK clock frequency FC: the same 13-value list. Sync pulse frequency FS (XSYNC, RSYNC): **min 6.0, typ 8.0, max 10.0 kHz**. BCLK duty ratio 40/50/60 %. Digital rise and fall times ≤ 50 ns. Sync setting times tXS, tSX, tRS, tSR ≥ 100 ns relative to BCLK edges. Sync pulse width tWS: min 1 BCLK, max apparently 100 µs. PCMIN set-up and hold ≥ 100 ns. **"Allowable Jitter Width — XSYNC, RSYNC, BCLK — 1 µs"** (the unit is printed in the Symbol font as "m s"). Operating temperature −30 … +85 °C. — [MSM7702 p. 8](https://pdf.jiepei.com/msm7702-9605035.html?page=8). The sibling MSM7717 states the same jitter limit as "1000 ns", which confirms the reading. — [MSM7717 datasheet](http://igorkov.org/revo/datasheets/M7717.pdf), p. 9
- The MSM7702 "uses the same transmission clocks as those used in the MSM7508B and MSM7509B" and is "pin-for-pin compatible with the MSM7578 and MSM7579". — [MSM7702 p. 1](https://pdf.jiepei.com/msm7702-9605035.html?page=1)
- The MSM7578/7579 datasheet carries the same XSYNC/RSYNC/BCLK wording and BCLK list. Its sync range was extracted as 6.0 / 8.0 / **9.0** kHz max; that may be an extraction error, since the MSM7702 and MSM7717 both say 10.0. — [MSM7579 p. 5](https://pdf.jiepei.com/msm7579-9756125.html?page=5), [p. 8](https://pdf.jiepei.com/msm7579-9756125.html?page=8)
- The application circuit labels XSYNC/RSYNC "8 kHz SYNC signal input". — [MSM7702 p. 14](https://pdf.jiepei.com/msm7702-9605035.html?page=14)
- The AC characteristics are all specified at the nominal sync (absolute delay "≤ 0.60 ms at BCLK = 64 kHz", group-delay rows at 500/600/1000/2600/2800 Hz). No row is given at any other sync frequency. — [MSM7702 p. 11](https://pdf.jiepei.com/msm7702-9605035.html?page=11)
- **Byproduct for other register rows (outside this topic; extraction garbled, check against page images before adopting):**
  - p. 4: AOUT "maximum amplitude of 2.0 VPP above and below the signal ground voltage (VDD/2)", load ≥ 1.2 kΩ.
  - p. 9: receive output amplitude VOAO −1.0 … +1.0 V; GSX output −0.7 … +0.7 V; AIN input resistance ≥ 10 MΩ. The ±1.0 V row is consistent with A-030's 1.0 V peak.
  - p. 11: absolute level 0.338/0.35/0.362 Vrms (transmit) and 0.483/0.50/0.518 Vrms (receive). Idle noise: transmit −70.5 typ / −68 max dBm0p, receive −78 typ / −74 max (the MSM7717 reads −72.5 and −76.5).
  - p. 10: 3400 Hz loss typ about 0.5 dB transmit and 0.47 dB receive (MSM7717: 0.4 and 0.56).
  - p. 12: discrimination 4.6–72 kHz ≥ 30 dB (32 typ); out-of-band spurious −37.5 typ / −35 max dBm0.
  - Sources: [p. 4](https://pdf.jiepei.com/msm7702-9605035.html?page=4), [p. 9](https://pdf.jiepei.com/msm7702-9605035.html?page=9), [p. 10](https://pdf.jiepei.com/msm7702-9605035.html?page=10), [p. 11](https://pdf.jiepei.com/msm7702-9605035.html?page=11), [p. 12](https://pdf.jiepei.com/msm7702-9605035.html?page=12)
- S-002 (as summarized in the project's sources.md): BCLK (pin 13) comes from the DSP, waveform marked T ≈ 2 µs. XSYNC (12) and RSYNC (9) come from the DSP, and the related sync waveform is marked T ≈ 170 µs. The DSP has a PLL (VCO IN / PC OUT pins) and is clocked by X1, an 8 MHz ceramic resonator, through IC3 (TC7WU04). — project file `docs/hardware/sources.md` row S-002, from [the service manual on the Internet Archive](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)

### Inferences
- **The PLL locks to XSYNC; the filters scale with the sync.** The datasheet says XSYNC "triggers the PLL and synchronizes all timing signals", and the part "eliminates a master clock". The switched-capacitor filters are therefore clocked from a multiple of XSYNC, and their corner frequencies scale in proportion to the sync rate. A-027 already assumes this. The datasheet does not say it in so many words; it follows from the switched-capacitor architecture and the absence of any other clock input. In the RR-DR60, RSYNC and XSYNC come from the same DSP, so both directions share one timebase.
- **BCLK must be an integer multiple of the sync, in practice.** "Synchronized in phase with BCLK", the ≥ 100 ns set-up and hold times against BCLK edges and the pulse width ≥ 1 BCLK mean that sync edges fall on BCLK edges. The sync period is therefore an integer number of BCLK cycles (N ≥ 9 for 8 data bits), at least on average.
- **The BCLK list.** Every listed rate is an integer multiple of 8 kHz: 8, 12, 16, 24, 25, 32, 48, 64, 96, 128, 192, 193 and 256 × 8 kHz (Calc). The list therefore looks like the set of data rates OKI characterised at 8 kHz, not an independent constraint, since the PLL runs from XSYNC. If the ratio rather than the absolute rate were the true constraint, the allowed values at 6 kHz would be 48, 72, 96, 144, 150, 192, 288, **384**, **576**, 768 … kHz, and the value nearest the "≈ 2 µs" annotation would be 576 kHz (1.74 µs) or 384 kHz (2.60 µs). A BCLK of 500 kHz, which is what 8 MHz/16 gives and what T ≈ 2 µs suggests, is in neither list. Either the codec tolerates an unlisted BCLK (the most likely reading, since BCLK only shifts bits) or the annotation is a rounded 512 kHz. This cannot be settled from the datasheet.
- **The 6 kHz corner.** At 6 kHz the device runs 25 % below the guaranteed 8 kHz, so the ± 50 ppm guarantee and the resonator's ± 0.5 % scatter are both irrelevant to whether the specifications apply: none are guaranteed. Panasonic chose an operating point that OKI permits but does not characterise. The datasheet gives no evidence on PLL lock margin at 6.0 kHz. A clock about 1 % below 6 kHz (for example 5952 Hz, see §2) would be just outside the stated minimum. That is not necessarily a failure, but it is a reason a designer would keep fs at or above 6.0 kHz.

### Gaps
- No OKI statement on the internal PLL multiplication ratio, the switched-capacitor clock rate, PLL lock range or lock time, or behaviour below 6.0 kHz.
- Whether an unlisted BCLK (500 kHz) is acceptable is not stated.
- The p. 8 and p. 10 tables are garbled in text extraction. The page images (the mirror has a "下载PDF" (download PDF) link) should be checked before the byproduct figures above are adopted into A-030, A-027 or S-003.

---

## 2. The plausible exact sample rate (8 MHz root, DSP PLL)

### Takeaway
No integer division of 8 MHz gives exactly 6000 Hz: 8 MHz ÷ 6000 = 1333.33. Exactly 6000 Hz needs a root that is a multiple of 3 MHz × 4 = 12 MHz (12, 24, 48 MHz…), which means a PLL multiplying the 8 MHz by 3 or 6 (or by 1.5). The annotations fit a BCLK of 8 MHz/16 = 500.000 kHz (T = 2.000 µs). With that BCLK the only integer sync rates near 6 kHz are **6024.1 Hz (N = 83, 166 µs)**, **5952.4 Hz (N = 84, 168 µs)** and **5882.4 Hz (N = 85, 170.0 µs)**. The literal 170 µs annotation matches N = 85, which is −1.96 % from 6 kHz and below the codec's 6.0 kHz minimum. A hand reading of a scope graticule cannot tell 166, 168 and 170 µs apart. This is all inference: the device's true rate could be 6000 Hz exactly (via a ×3 PLL), 6024 Hz, 5952 Hz or another value. Whatever it is, it is common to all units (a design offset), and the resonator tolerance adds per-unit scatter on top.

### Cited findings
- Specification: "Sampling frequency; 6 kHz", "Data transfer rate; 4 kbit/s". Annotations: BCLK T ≈ 2 µs, sync T ≈ 170 µs. X1 is an 8 MHz ceramic resonator; the DSP has a PLL (VCO IN / PC OUT); X2 32 kHz clocks the microcontroller only. — project `docs/hardware/sources.md` S-002 row ([service manual](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN))
- The codec's sync range minimum is 6.0 kHz. — [MSM7702 p. 8](https://pdf.jiepei.com/msm7702-9605035.html?page=8)

### Calculations (Calc; `rates.py`)
Candidates with BCLK = root/k in 380–650 kHz and fs = BCLK/N in 5.85–6.15 kHz (selected rows; the full table is in the script output):

| Root | ÷k | BCLK (kHz) | T_BCLK (µs) | N | fs (Hz) | T_sync (µs) | vs 6000 Hz |
|---|---|---|---|---|---|---|---|
| 8 MHz | 16 | 500.000 | 2.000 | 83 | 6024.096 | 166.0 | +0.402 % |
| 8 MHz | 16 | 500.000 | 2.000 | 84 | 5952.381 | 168.0 | −0.794 % |
| 8 MHz | 16 | 500.000 | 2.000 | 85 | 5882.353 | **170.0** | −1.961 % |
| 8 MHz | 15 | 533.333 | 1.875 | 89 | 5992.509 | 166.9 | −0.125 % |
| 8 MHz | 18 | 444.444 | 2.250 | 74 | 6006.006 | 166.5 | +0.100 % |
| 8 MHz | 14 | 571.429 | 1.750 | 95 | 6015.038 | 166.2 | +0.251 % |
| 24 MHz (PLL ×3) | 50 | 480.000 | 2.083 | 80 | **6000.000** | 166.7 | exact |
| 24 MHz (PLL ×3) | 40 | 600.000 | 1.667 | 100 | **6000.000** | 166.7 | exact |
| 24 MHz (PLL ×3) | 47 | 510.638 | 1.958 | 85 | 6007.509 | 166.5 | +0.125 % |
| 512 kHz (listed BCLK) | — | 512.000 | 1.953 | 85 | 6023.53 | 166.0 | +0.39 % |
| 512 kHz (listed BCLK) | — | 512.000 | 1.953 | 86 | 5953.49 | 168.0 | −0.78 % |

- Direct division of 8 MHz near 6 kHz: 8 MHz/1333 = 6001.500 Hz (+0.025 %), but 1333 = 31 × 43, so no BCLK in the 380–650 kHz range divides it evenly. 8 MHz/1334 = 5997.0 Hz; 8 MHz/1328 = 6024.1 Hz (= 16 × 83); 8 MHz/1344 = 5952.4 Hz (= 16 × 84).
- Exact 6000 Hz from integer division requires root/6000 to be an integer: true for 12, 24 and 48 MHz, false for 8, 16, 32 and 40 MHz.
- **The frame structure does not constrain fs.** A 20 ms frame of 80 bits is 120 samples at exactly 6000 Hz (A-040). At 6024.1 Hz, 120 samples last 19.92 ms and the bit rate becomes 4016 bit/s; at 5952.4 Hz, 20.16 ms and 3968 bit/s. A coder defined in samples per frame works at any fs, and "4 kbit/s" in the specification is a nominal figure, like "6 kHz".
- The sync pulse train at BCLK = 500 kHz with 8 data bits: the data occupy 16 µs of each 166–170 µs frame.

### Inferences
- **The most self-consistent reading of the annotations alone** is BCLK = 8 MHz/16 = 500 kHz from a simple divider chain, with fs = 500 kHz/N. N = 83 (6024 Hz) is the only such choice that is ≥ 6.0 kHz (the codec minimum) and within 0.5 % of the specification's "6 kHz". On that reading the specification's 6 kHz is a rounding of 6.024 kHz. Panasonic specification tables commonly round, but I found no source stating the device's exact rate.
- **The alternative** is a DSP PLL that multiplies 8 MHz by 3 (24 MHz) or 6 (48 MHz) as the core clock, from which an exact 6000 Hz and a 480 or 600 kHz BCLK follow. T_BCLK would then be 2.08 µs or 1.67 µs, both "≈ 2 µs" on a scope. The DSP's PLL pins show that it multiplies something, but the multiplier is unknown. A CELP coder at 4 kbit/s is heavy for a 1990s DSP, which makes a PLL-multiplied core clock of tens of MHz plausible.
- **A fractional sync** (an 83/83/84 BCLK pattern averaging 6000 Hz from 500 kHz) would have 0.67 BCLK = 1.33 µs of peak-to-peak deterministic jitter, beyond the codec's 1 µs allowance (Calc: the phase error relative to the ideal grid is −0.333, −0.667, 0 BCLK). A designer reading the datasheet would avoid it, which favours an integer N.
- **For the emulator:** the device rate's design offset (say +0.40 % or −0.79 %) and the per-unit resonator error are indistinguishable in effect (see §4), and both cancel in same-clock record-and-playback pitch. Only band-edge positions, memory minutes and sample-counted durations would move by a fraction of a percent. The emulator's exact 6000 Hz (A-026) stays a reasonable nominal; register the possible design offset as a note on A-011.

### Gaps
- Nothing found on the MN1931712BB DSP (no datasheet or catalogue entry). Its PLL multiplier, serial-port clock dividers and the way it generates the sync are unknown.
- The schematic's "≈ 2 µs" and "≈ 170 µs" annotations are rounded scope readings, and the reading resolution is not stated. A direct measurement (frequency counter on IC1 pin 12 or 13) would settle the question in minutes on any working unit.

---

## 3. Ceramic-resonator accuracy (initial, temperature, aging, circuit) and PLL jitter

### Takeaway
Murata and Kyocera 8 MHz ceramic resonators are specified at an **initial ±0.5 %**, a **temperature stability of ±0.2 … ±0.5 % over −20 … +80 °C**, and **aging of ±0.1 … ±0.3 % over 10 years**. Load capacitance changes of ±10 % shift the frequency by about ±0.05 %, and supply voltage by ≤ ±0.05 %. Murata also warns that its frequency is set in its standard measuring circuit and "may shift" in a different IC circuit. A-011's ±0.5 % is the initial tolerance only. A worst-case arithmetic sum for a 25–30-year-old unit at room temperature is about ±0.8 … ±1.1 %; a root-sum-square estimate is about ±0.6 %. Typical units should sit well inside these limits (they are maxima), but I found no distribution data. Jitter is not a concern (§6).

### Cited findings
- Murata CERALOCK general text: "Temperature stability is ±0.3 to ±0.5% against initial values within -20 to +80 °C. The initial frequency precision is ±0.5% for standard products. The frequency of the standard CERALOCK® is adjusted by the standard measuring circuit, but the oscillation frequency may shift when used in the actual IC circuit." — [Murata CERALOCK Application Manual P17E (2018)](https://mm.digikey.com/Volume0/opasdata/d220001/medias/docus/7176/490_p17e.ashx.pdf), §3.1
- Murata current specifications: CSTLS□G53/56 (3.40–10.00 MHz, lead type): initial ±0.5 %, temperature (−20 … +80 °C) ±0.2 %, aging ±0.2 %. CSTCE□G (8.00–13.99 MHz, chip): ±0.5 %, ±0.2 %, ±0.1 %. Standard circuit: TC4069UBP or TC74HCU04 unbuffered inverter, Rf 1 MΩ, **Rd 680 Ω** (CSTLS□G56). — [Murata P17E](https://mm.digikey.com/Volume0/opasdata/d220001/medias/docus/7176/490_p17e.ashx.pdf), pp. 13–14
- Murata on circuit sensitivity: "The stability versus temperature change is ±0.1 to 0.5% within a range of -20 to +80 °C, although it varies slightly depending on the ceramic material. Influence of load capacitance (CL1, CL2) on the oscillation frequency is relatively high … It varies approximately ±0.05% for a capacitance deviation of ±10%. The stability versus supply voltage is normally within ±0.05% in the working voltage range, although it varies with the characteristics of the IC." — [Murata P17E](https://mm.digikey.com/Volume0/opasdata/d220001/medias/docus/7176/490_p17e.ashx.pdf), §5.1 (measured example: TC74HCU04 with CSTCR4M00G55)
- Murata's comparison table: ceramic resonator initial tolerance ±0.5 %, long-term stability "Excellent"; crystal ±0.001 %; LC/CR ±2.0 % "Fair". The manual says a clock precision need of about ±2–3 % can usually use the standard type. — [Murata P17E](https://mm.digikey.com/Volume0/opasdata/d220001/medias/docus/7176/490_p17e.ashx.pdf), pp. 2 and 12
- Older Murata catalogue P16E-13 (2002), "Aging (10 years)" column. CSTLS_G 2.00–10.00 MHz: initial ±0.5 %, temperature ±0.2 % (−20 … 80 °C), **aging (10 years) ±0.2 %**. CSTCE_G 8.00–12.50 MHz: ±0.5 %, ±0.2 %, **±0.1 %**. CSTCC_G 2–4 MHz: ±0.3 % aging. (The extraction renders "±" as "–".) — [Murata CERALOCK catalogue P16E-13](https://media.elv.com/file/03522_keramikschwinger_data.pdf)
- Kyocera KBR (discontinued) MSB series, 3.52–8.00 MHz: tolerance ±0.5 %, temperature stability ±0.3 % (−20 … 80 °C); MSE ±0.5 % / ±0.5 %; MKC/MKD/MKE (to 8.00 MHz) ±0.5 % / ±0.5 %. No aging figure is given. — [Kyocera KBR datasheet](https://ele.kyocera.com/sites/default/files/assets/static/discontinued/kbr_2003_e.pdf)
- RR-DR60 oscillator: X1 8 MHz ceramic resonator on the DSP's OSCF0/1 through IC3 TC7WU04 (an unbuffered inverter). sources.md notes "R69 680 Ω is the 8 MHz crystal's series resistor". — project `docs/hardware/sources.md` S-002

### Calculations
- Worst-case arithmetic sum (old unit, room temperature): initial ±0.5 + 10-year aging ±0.1 … ±0.2 (extended to about 30 years, see the Gaps entry: ±0.2 … ±0.3) + circuit and load ±0.05 … ±0.1 → **±0.75 … ±0.9 %**. Adding the full temperature band (±0.2 … ±0.5 %) gives **±0.95 … ±1.4 %** over −20 … +80 °C.
- Root-sum-square of 0.5, 0.25 (aging), 0.07 (circuit) and 0.2 (temperature) ≈ **±0.6 %**.
- Temperature coefficient bound: ±0.2 % spread over the 55 K from 25 to 80 °C, assumed linear, is ≤ 36 ppm/K. For ±0.5 % it is ≤ 91 ppm/K.

### Inferences
- **The RR-DR60 oscillator is close to Murata's standard measuring circuit**: an unbuffered inverter (TC7WU04, the same family as the TC74HCU04 in Murata's standard circuit) and a 680 Ω series resistor (the Rd of CSTLS□G56). So the "actual IC circuit" offset is probably small, of the order of the ±0.05 % load effect. Whether X1 is a two-terminal part with external capacitors (CSA type) or a three-terminal part with built-in capacitors (CST type) decides how much capacitor tolerance and aging add. The schematic researcher should check the part number and any C1/C2.
- **Supply:** the DSP and its oscillator are most likely on the regulated 3.3 V rail (XC6372). Battery sag should then move the clock by ≪ 0.05 %. This is unverified.
- **A-011 status:** "±0.5 %" is right as the initial tolerance and a reasonable one-sigma-like figure for a population. As a bound for 25–30-year-old units across temperatures, about ±1 % is the defensible worst case. On top of that sits the possible common design offset of §2 (+0.40 % or −0.79 % if fs = 500 kHz/83 or /84).

### Gaps
- No datasheet found for the 1990s parts (CSA8.00MG, CSA8.00MTZ, CST8.00MTW, TDK FCR8.0M). The CSA□MG catalogue rows were not readable in the copy found. The P16E-13 and P17E rows are for later series of the same technology.
- No source found for aging beyond 10 years (whether it follows a log-time law, how much the second and third decades add) or for the population distribution within the ±0.5 % limit. The 30-year extrapolation above is my estimate.

---

## 4. What a clock error does to the recorded and played-back sound

### Takeaway
On one unit, recording at fs(1+ε) and playing back at the same fs(1+ε) gives **no pitch or speed error**: a tone at f is stored as f/fs(1+ε) cycles per sample and comes back at exactly f. What does scale with the clock is everything defined by the codec's PLL or by sample counts:
- the codec filter band edges (225 → 225(1+ε) Hz, 2550 → 2550(1+ε) Hz)
- the anti-alias and image positions (fs − f)
- the CELP frame duration (20 ms/(1+ε))
- any VAS or DSP time constant counted in samples or frames
- the memory capacity in minutes (60/(1+ε))

The analog stages do not scale: the mic amplifier's corners, the AGC's RC timings, the output-chain corners and the speaker. A pitch error appears only if the clock **changes between recording and playback**: temperature drift, which is typically 0.05–0.2 % (≤ about 3.5 cents, below the 5–6-cent pitch JND), or the SPEED CONTROL setting (out of scope here).

### Cited findings
- The codec's timing is derived from XSYNC by its PLL. — [MSM7702 p. 5](https://pdf.jiepei.com/msm7702-9605035.html?page=5)
- Resonator temperature stability: ±0.2 % (CSTLS/CSTCE) to ±0.3 … ±0.5 % (general) over −20 … +80 °C; supply ≤ ±0.05 %. — [Murata P17E](https://mm.digikey.com/Volume0/opasdata/d220001/medias/docus/7176/490_p17e.ashx.pdf); Kyocera MSB ±0.3 %. — [Kyocera KBR](https://ele.kyocera.com/sites/default/files/assets/static/discontinued/kbr_2003_e.pdf)
- Pitch discrimination: "humans can distinguish a difference in pitch of about 5–6 cents" (secondary, citing a thesis). — [Wikipedia: Cent (music)](https://en.wikipedia.org/wiki/Cent_(music)). Adult frequency-discrimination thresholds for a 1000 Hz standard are about 0.5 % (citing Olsho 1984; Sinnott & Aslin 1985). — [Buss, Taylor & Leibold 2014, JSLHR 57(5)](https://pmc.ncbi.nlm.nih.gov/articles/PMC4192033). The JND is "about 1/30 of the critical bandwidth". — [CCRMA demonstration page](https://ccrma.stanford.edu/~malcolm/correlograms/text/33%20Frequency%20Difference%20Limen%20Or%20Jnd.html). The primary paper usually cited, Wier, Jesteadt & Green (1977), JASA 61, 178–184, was not read; its 1 kHz values are not quoted here. Trained listeners in laboratory conditions reach lower thresholds than the 0.5 % figure, but I did not find a citable number.
- Memory: 16 Mbit, "60 minutes / 99 items" (S-002); A-004 puts the payload capacity at 69.9 min. — project `docs/hardware/sources.md`, `assumptions.md`

### Calculations (`rates.py`)
| Ratio error | Cents | 1 kHz shift | 2550 Hz edge shift | 60 min capacity shift |
|---|---|---|---|---|
| 0.05 % | 0.87 | 0.5 Hz | 1.3 Hz | 1.8 s |
| 0.10 % | 1.73 | 1.0 Hz | 2.6 Hz | 3.6 s |
| 0.20 % | 3.46 | 2.0 Hz | 5.1 Hz | 7.2 s |
| 0.30 % | 5.19 | 3.0 Hz | 7.7 Hz | 10.8 s |
| 0.50 % | 8.63 | 5.0 Hz | 12.8 Hz | 18.0 s |
| 0.80 % | 13.8 | 8.0 Hz | 20.4 Hz | 28.8 s |
| 1.00 % | 17.2 | 10.0 Hz | 25.5 Hz | 36.0 s |

- **Cold recording, warm playback** (ΔT = 25 K, for example 0 °C outdoors to 25 °C indoors): at ≤ 36 ppm/K (±0.2 % parts), ≤ 0.09 % ≈ 1.6 cents; at ≤ 91 ppm/K (±0.5 % parts), ≤ 0.23 % ≈ 3.9 cents. **Extreme case** (−20 °C recording, +60 °C playback, ΔT = 80 K): up to 0.3 % (±0.2 % parts) to about 0.7 % (±0.5 % parts), 5–12 cents. These bounds come from spec limits, assuming a linear coefficient; real curves are usually flatter near room temperature.
- The direction of the shift: if the clock is slower at recording than at playback, the playback is sharp by the ratio fs_play/fs_rec.

### Inferences
- **Inaudible in normal use.** Same-unit playback has no pitch error at all from ε. Temperature drift between recording and playback gives sub-JND shifts (≲ 4 cents) in everyday conditions, reaching about the JND only across extreme temperature swings. On speech with no reference tone the shift is even less noticeable. The emulator's per-unit deviation (A-011) is therefore not a pitch control for the default path.
- **What ε does change, if modelled.** In the emulator, a uniform clock error equals a device rate of 6000(1+ε) Hz in the host-rate resamplers, with every device-domain stage unchanged (the filter templates are defined at normalized frequency). The band edges in Hz, the image positions, the frame and VAS durations and the memory minutes then all scale automatically, the analog host-rate stages (mic, limiter, output chain) stay put, and record-then-play pitch remains identity. A record/playback clock *mismatch* (temperature) would be a separate, explicit ratio between the record path's and the playback path's device rates.
- The analog-versus-digital split matters only at the ±0.5–1 % level: the A-046 mic high-pass (818 Hz, ±18 % uncertain) and the AGC timings are already far less certain than the clock.
- The specification's "60 minutes" would change by tens of seconds across units (±18 s at ±0.5 %), and the design offset of §2 could add a fixed −24 s (6024 Hz) or +48 s (5952 Hz).

### Gaps
- Whether any DSP firmware timing (VAS hang time, the 10 SEC REVIEW skip length) is counted in samples or frames (which scales with ε) or by the microcontroller's 32 kHz crystal (which does not). The microcontroller has a clock function (S7 "CLOCK-MIC SET"), so some times are crystal-based.
- No measured temperature curve for the actual resonator.

---

## 5. How a clock error could be observed from outside; the field-measurement claim

### Takeaway
**The claim in `docs/hardware/field-measurement.md` is incorrect as written.** It says a 1 kHz tone's frequency shift against the phone's crystal "gives the device clock error directly". Through record-then-playback on the same clock, the shift is fs_play/fs_rec − 1, which is about zero for any steady unit whatever its absolute error. The tone measures the record-to-playback clock *change* (temperature) and the SPEED CONTROL setting, not the clock. Methods that do give the absolute clock from outside:
1. the DAC image: the playback output contains an image of each in-band component at fs − f, so f_tone + f_image = fs
2. the alias of an out-of-band record tone (fs − f_in on playback)
3. the time to fill the memory, against a stopwatch
4. a radio receiver tuned to the 8 MHz oscillator or the 500 kHz BCLK harmonics
5. a frequency counter on the codec's XSYNC or BCLK pins

The band edge's position moves only 12.8 Hz at 0.5 %, so it confirms 6 kHz versus 8 kHz but cannot read ε precisely.

### Cited findings
- The field note's table row: "Clock accuracy and playback speed | A-011, S-001 speed control | A 1 kHz tone | A frequency shift against the phone's crystal (parts per million) gives the device clock error directly. The SPEED CONTROL switch must be off…" — project file `docs/hardware/field-measurement.md`
- The codec's receive section: out-of-band spurious (the images of in-band signals) −37.5 dBm0 typ, −35 max, over 4.6 kHz–100 kHz at the 8 kHz rating (scaled by 0.75 at 6 kHz: about 3.45–75 kHz); discrimination ≥ 30 dB (32 typ). — [MSM7702 p. 12](https://pdf.jiepei.com/msm7702-9605035.html?page=12)
- The earphone path is an emitter follower from the power amplifier's input node, with high-pass corners only (A-032; 120/280 Hz). — project `docs/hardware/assumptions.md`, `sources.md`

### Calculations
- **Same-clock loop.** The phone emits f₀ = 1000 Hz on its own crystal (±20 ppm class). The device samples at fs′ = fs(1+ε), stores the digital frequency f₀/fs′ cycles per sample, and plays it back at fs′ → f₀. The shift is zero, independent of ε. With a temperature change between the two passes, the output is f₀·fs′_play/fs′_rec.
- **Image method.** For a decoded component at f (whatever the CELP coder renders), the codec's output image sits at fs′ − f. At fs′ = 6024.1 Hz and f = 1000 Hz the image is at 5024.1 Hz; at ε = +0.5 % of 6000 it moves by 30 Hz. In a 10 s capture an FFT resolves 0.1 Hz, so fs′ is resolved to about 17 ppm (0.1/6000), far below the resonator scatter. The phone's own ±20 ppm clock error moves the reading by only 20 ppm. The image is ≤ −35 dBm0 for a 0 dBm0 tone (datasheet bound; typical −37.5) and passes the earphone follower, which has no low-pass, so it is well above a phone's noise floor on a cable return. On the speaker route the small speaker's response near 5 kHz attenuates it further (unknown).
- **Alias method.** A 3500 Hz record tone (above the 3.45 kHz scaled stopband start, ≥ 30 dB down) aliases to fs′ − 3500 ≈ 2524 Hz and plays back there, so the playback frequency equals fs′ − 3500 directly. Each Hz of the alias is 1/6000 = 167 ppm. Caveat: a −30 dB tone near 2.5 kHz must survive the 4 kbit/s coder; the image method does not have this problem.
- **Band edge.** At 6 kHz the stopband falls about 20 dB between roughly 2550 and 3000 Hz (≈ 0.044 dB/Hz; from G.712-style templates scaled by 0.75, my estimate). A 0.5 % error (12.8 Hz) changes the level at a fixed frequency on the slope by only about 0.6 dB. That is comparable to the ±0.4 dB template tolerance (A-027) and to the CELP LPC envelope's smoothing, so it gives ε to about ±0.5–1 % at best.
- **Memory fill.** With the VAS off, the time to "FULL" = (frames of capacity × 120)/fs′. A 60–70-minute fill timed to ±1 s resolves ε to about 280 ppm, but only if the capacity in frames is known exactly (A-040's framing overhead is unknown). The **ratio** of two units' fill times gives their relative clock error without that knowledge.

### Inferences
- **Fix for the field-measurement note** (another feature's spec; this file does not edit it): the 1 kHz tone row should read "record-to-playback clock ratio and the SPEED CONTROL effect". The absolute-clock figure should come from the image (or alias) method, which uses the same capture: add a spectrum read at 3–6 kHz of the cable return. Its confidence stays high for the image method on the earphone return.
- **Radio pickup.** An 8 MHz ceramic oscillator radiates weakly. A shortwave receiver or SDR in SSB/CW mode held against the case could read the carrier to about 1 Hz (0.1 ppm). The BCLK's harmonics (500 kHz × n: 1.0, 1.5 MHz…) fall in the AM broadcast band. Whether either is strong enough to receive is untested.
- **Direct probe** (opening the unit): a counter on IC1 pin 12 (XSYNC) or pin 13 (BCLK) gives fs and the BCLK/N relation exactly, and also settles §2.
- **Displayed time.** If the LCD's recording counter comes from the microcontroller's 32 kHz crystal (±20 ppm class), the counter at FULL compared with a stopwatch reads the crystal, not the DSP clock. If the counter counts frames, a stopwatch comparison over a long recording reads ε directly. Which one applies is unknown.

### Gaps
- No real-unit capture of the image level at the earphone jack. The −35 dBm0 bound is the datasheet's at 8 kHz, untested at 6 kHz.
- Whether the speed control alters the sync rate (which would make the same image method also measure the speed setting) belongs to the speed-control researcher.

---

## 6. Jitter, phase noise and clock-tree spurs in the audio band

### Takeaway
Jitter is negligible against the µ-law's 36–43 dB signal-to-distortion and the 4 kbit/s coder: even 100 ns RMS on the sampling instants limits a full-scale 2.5 kHz tone to 56 dB SNR, and the codec's own PLL filters input jitter within the 1 µs allowance. Clocks derived from the same 8 MHz by integer division are coherent with fs, so their leakage aliases to DC, not to an audible tone. Incoherent sources can alias into the band in principle, at the 6 kHz rate:
- the microcontroller's 32.768 kHz crystal: 2768 Hz and 464 Hz at 6000 Hz, or 2647 Hz and 729 Hz at 6024 Hz
- the XC6372 converter's 50/100/180 kHz (±15 %) oscillator or its light-load PFM bursts

They are attenuated by the codec's ≥ 30 dB stopband and the analog layout. Their audibility is unknown without a capture.

### Cited findings
- Allowable jitter on XSYNC, RSYNC and BCLK: 1 µs. — [MSM7702 p. 8](https://pdf.jiepei.com/msm7702-9605035.html?page=8); 1000 ns — [MSM7717 p. 9](http://igorkov.org/revo/datasheets/M7717.pdf)
- µ-law signal-to-distortion: about 41–43 dB at 0/+3 dBm0 (MSM7702 p. 10 extraction; MSM7717 43/41 dB). — [MSM7702 p. 10](https://pdf.jiepei.com/msm7702-9605035.html?page=10), [MSM7717](http://igorkov.org/revo/datasheets/M7717.pdf)
- Transmit discrimination 4.6–72 kHz ≥ 30 dB (at 8 kHz). — [MSM7702 p. 12](https://pdf.jiepei.com/msm7702-9605035.html?page=12)
- Torex XC6371/XC6372 step-up converters: "Oscillation frequency is also selectable from 50kHz, 100kHz, and 180kHz (accuracy: ±15%)", PWM and PWM/PFM-switchable versions (from an aggregator search snippet; not read in full). — [alldatasheet XC6372 page](https://html.alldatasheet.com/html-pdf/178047/TOREX/XC6372E702DR/155/1/XC6372E702DR.html)
- The MSM7702's design notes recommend "a low noise (particularly, low level type of high frequency spike noise or pulse noise) power supply to avoid erroneous operation and the degradation of the characteristics", and electromagnetic shielding near power-supply transformers. — [MSM7702 p. 15](https://pdf.jiepei.com/msm7702-9605035.html?page=15)

### Calculations
- **Jitter-limited SNR** for a full-scale sine: SNR = −20 log₁₀(2π f σ). At f = 2.5 kHz: σ = 1 ns → 96 dB; 10 ns → 76 dB; 100 ns → 56 dB; 1 µs → 36 dB.
- **Deterministic pattern** (if a fractional 83/83/84 sync were used, §2): peak phase deviation ±0.67 µs at a rate of fs/3 ≈ 2 kHz. For a 1 kHz tone, β = 2π × 1000 × 0.67 µs ≈ 0.0042, giving sidebands at about −54 dB before any PLL filtering.
- **Coherent leakage.** If fs = 8 MHz/1328, then 8 MHz = 1328·fs and 500 kHz = 83·fs, so either aliases to exactly 0 Hz.
- **32.768 kHz aliases.** 32768 − 5 × 6000 = 2768 Hz and 65536 − 11 × 6000 = −464 → 464 Hz. At fs = 6024.1: 2647.5 Hz and 729 Hz. If the microcontroller's clock leaked into the analog input before the codec, the first alias would be at the band edge and the second in the band.

### Inferences
- The codec's internal PLL is the actual sampling clock, and it tracks XSYNC with a loop bandwidth that should be small relative to fs. DSP-edge jitter of ns order is therefore further filtered. The 8 MHz ceramic resonator's phase noise (worse than a crystal's) matters only through the jitter it produces at audio-rate offsets, which is in the ps–ns range for such oscillators (no source found; order-of-magnitude judgement).
- Clock spurs, if present, would most likely come from **incoherent** sources (the microcontroller's 32 kHz domain, its LCD multiplexing, the DC-DC converter's PFM bursts at load-dependent audio-band rates) coupling through the supply or ground into the mic preamplifier or the codec input. They would not come from the DSP clock tree. The codec's power-supply noise rejection and the 3.2 V analog-rail switching (Q20) belong to the power researcher. The emulator models none of these, and nothing in the register suggests it should until a capture shows a tone.
- On playback, the BCLK, 8 MHz and converter harmonics are far above anything the NJM2113, the earphone follower or the speaker reproduces; there is no sampling after the DAC to alias them down.

### Gaps
- No capture of the RR-DR60's idle noise spectrum to look for discrete tones at 464/729 Hz, 2.6–2.8 kHz or the converter's PFM rate.
- XC6372 suffix, mode (PWM or PFM) and frequency in the RR-DR60 are unknown (parts list S-002; the power researcher).
- No phase-noise data for 8 MHz CERALOCK oscillators.
