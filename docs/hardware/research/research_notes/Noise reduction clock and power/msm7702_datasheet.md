# OKI MSM7702-01/02/03 datasheet (E2U0018-28-81, Aug 1998): a full, page-cited extraction for the codec, noise, clock and power models

Researched 2026-10-17. All 17 pages were read **from rendered page images**, one per page. The images are headless-Chrome renders of the mirror's own HTML conversion of the PDF, which has a positioned text layer plus a per-page vector SVG (see Q1). An earlier pass extracted the text only. Every figure below was read off the image and cross-checked against the positioned text layer. Citation `p. N` means `https://pdf.jiepei.com/msm7702-9605035.html?page=N`.

**Symbol-font key (important).** The conversion renders the PDF's Symbol-font glyphs as Latin letters. The mapping was confirmed from the HTML class (`stl_21`) that marks them: `m` → µ, `W` → Ω, `¥` → ×, `Æ` → →, `£` → ≤. So the page images literally print "±200 mA", "0.1 mF", "1 ms" (jitter), "2.0 mA" (leakage) and "kW". These are **±200 µA, 0.1 µF, 1 µs, 2.0 µA and kΩ**. The supply currents (IDD1–3) are in a normal font and really are **mA**. They are also consistent with the mW figures on p. 1.

## Q1. Access: which URL form worked, and what "page images" exist

### Takeaway
The lowercase page URL `https://pdf.jiepei.com/msm7702-9605035.html?page=N` (N = 1…17) serves every page. No high-resolution scan exists on the mirror, only 240 × 310 px thumbnails. Readable page images were made by rendering the mirror's own HTML+SVG conversion of each page, and all 17 pages are fully legible that way.

### Cited Findings
- The page HTML loads with HTTP 200 for N = 1…17. — [p. 1](https://pdf.jiepei.com/msm7702-9605035.html?page=1) … [p. 17](https://pdf.jiepei.com/msm7702-9605035.html?page=17)
- The embedded page "images" are thumbnails only: `https://pdffile.jiepei.com/pdf1/p00077/img/page/MSM7702_403306_files/MSM7702_403306_N.jpg` returns a 240 × 310 px JPEG for N = 1…17 (N = 18 gives 404), too small to read.
  - curl on this machine cannot verify that host's certificate chain (openssl s_client verifies it). The images were fetched with certificate checks off, as untrusted data.
  - Plain `http://` returns 404.
  — [p. 1](https://pdf.jiepei.com/msm7702-9605035.html?page=1)
- Each page's body is positioned HTML text over a vector drawing: `https://pdffile.jiepei.com/pdf1/p00077/html/MSM7702_403306/MSM7702_403306_files/img_NN.svg` (NN = 01…17), with `style.css` in the same folder. **These answer 404 without a `Referer` header** and 200 with `Referer: https://pdf.jiepei.com/msm7702-9605035.html?page=1`.
  - The earlier "garbled text" came from reading this text layer out of position.
  - Rendering the HTML with the SVG gives clean page images. Local renders are in the session scratchpad (not committed).
  — [p. 9](https://pdf.jiepei.com/msm7702-9605035.html?page=9)
- The document is E2U0018-28-81, "This version: Aug. 1998, Previous version: Nov. 1996", titled "Single Rail CODEC". — [p. 1](https://pdf.jiepei.com/msm7702-9605035.html?page=1)
- Page map:
  - p. 1: description and features.
  - p. 2: block diagram.
  - p. 3: pin configuration.
  - pp. 4–7: pin and functional descriptions.
  - p. 8: absolute maximum ratings and recommended operating conditions.
  - p. 9: DC and analog interface characteristics.
  - pp. 10–12: AC characteristics.
  - p. 13: timing diagram.
  - p. 14: application circuit.
  - p. 15: design recommendations.
  - pp. 16–17: package dimensions only.
  — [pp. 1–17](https://pdf.jiepei.com/msm7702-9605035.html?page=1)

### Inferences
- The values are typeset text from the original PDF, not OCR, so the digits are reliable. Only the Symbol-font unit glyphs need the key above.
- To reproduce the renders: fetch `?page=N`, cut the `<div class="stl_02">` block, point the `<object>` at a local copy of `img_NN.svg` fetched with a Referer, inline `style.css`, then run `chrome --headless --screenshot --force-device-scale-factor=2 --window-size=760,1080`.

### Gaps
- No high-resolution raster scan of the original was found. The renders use substitute fonts, so the layout is approximate but the numbers are exact.

## Q2. Clocking: XSYNC / RSYNC / BCLK, sync range, jitter, PLL, internal clock

### Takeaway
The MSM7702 states the same clocking rules as the MSM7717 proxy:
- A PLL is triggered by XSYNC.
- Both syncs must be "synchronized in phase with BCLK".
- The AC characteristics are guaranteed only at 8 kHz ± 50 ppm. Operation is allowed over 8 kHz ± 2 kHz (recommended Fs 6.0 / 8.0 / 10.0 kHz) "but the electrical characteristics … are not guaranteed".
- BCLK takes 13 listed rates. Allowable jitter is 1 µs.

The sheet does **not** give a PLL lock time or the internal switched-capacitor / oversampling clock rate. It also does not say that the sync period must be a whole number of BCLK cycles.

### Cited Findings
- Features: "Built-in PLL eliminates a master clock". Serial data rate 64/128/256/512/1024/2048 kHz and 96/192/384/768/1536/1544/200 kHz. "The device uses the same transmission clocks as those used in the MSM7508B and MSM7509B." — [p. 1](https://pdf.jiepei.com/msm7702-9605035.html?page=1)
- Block diagram:
  - The PLL is fed by XSYNC and BCLK and drives TCONT (transmit control).
  - RTIM (receive timing) is fed by RSYNC and BCLK and drives RCONT.
  - AUTO ZERO is internal, between the A/D and the 8th-order BPF.
  - The "VR GEN" and "SG GEN" blocks are separate. "PWD Logic" is driven by PDN.
  — [p. 2](https://pdf.jiepei.com/msm7702-9605035.html?page=2)
- XSYNC: "triggers the PLL and synchronizes all timing signals of the transmit section … must be synchronized in phase with BCLK. The frequency should be 8 kHz ±50 ppm to guarantee the AC characteristics which are mainly the frequency characteristics of the transmit section … can operate in the range of 8 kHz ±2 kHz, but the electrical characteristics in this specification are not guaranteed." — [p. 5](https://pdf.jiepei.com/msm7702-9605035.html?page=5)
- RSYNC: "Signals in the receive section are synchronized by this synchronizing signal", with the same in-phase-with-BCLK, ±50 ppm and ±2 kHz wording for the receive section. It selects the eight bits from PCMIN. — [p. 5](https://pdf.jiepei.com/msm7702-9605035.html?page=5)
- BCLK: "The frequency, equal to the data rate, is 64, 96, 128, 192, 256, 384, 512, 768, 1024, 1536, 1544, 2048, or 200 kHz." — [p. 5](https://pdf.jiepei.com/msm7702-9605035.html?page=5)
- Power save by clock: holding BCLK (or XSYNC) at a static "1" or "0" "drives both transmit and receive circuits to the power saving state". — [p. 5](https://pdf.jiepei.com/msm7702-9605035.html?page=5)
- Recommended operating conditions:
  - Clock frequency Fc (BCLK): the same 13 values.
  - **Sync pulse frequency Fs (XSYNC, RSYNC): min 6.0, typ 8.0, max 10.0 kHz.**
  - Clock duty ratio 40/50/60 %.
  - Digital rise and fall times ≤ 50 ns.
  - Sync setting times tXS, tSX, tRS, tSR ≥ 100 ns.
  - **Sync pulse width tWS: min 1 BCLK, max 100 µs.**
  - PCMIN set-up and hold ≥ 100 ns.
  - **Allowable jitter width (XSYNC, RSYNC, BCLK): max 1 µs.**
  — [p. 8](https://pdf.jiepei.com/msm7702-9605035.html?page=8)
- Timing diagram:
  - PCMOUT shifts MSD first on the BCLK rising edge after XSYNC. The receive side latches 8 bits after RSYNC.
  - "When tXS ≤ 1/2·Fc, the Delay of the MSD bit is defined as tXD1. When tSX ≤ 1/2·Fc … tSD."
  - Digital output delays tSD, tXD1–3 are 20–200 ns at CL = 100 pF.
  — [p. 13](https://pdf.jiepei.com/msm7702-9605035.html?page=13), [p. 12](https://pdf.jiepei.com/msm7702-9605035.html?page=12)
- In the application circuit, XSYNC and RSYNC are tied together and labelled "8 kHz SYNC signal input". — [p. 14](https://pdf.jiepei.com/msm7702-9605035.html?page=14)
- The absolute delay (A to A) is ≤ 0.60 ms at BCLK = 64 kHz, the only clock-dependent AC row. — [p. 11](https://pdf.jiepei.com/msm7702-9605035.html?page=11)

### Inferences
- At the RR-DR60's 6 kHz sync (A-026) the device sits on the **minimum of the recommended Fs range** (p. 8). Every AC figure on pp. 10–12 is therefore "not guaranteed" at 6 kHz (p. 5), so A-027's ×0.75 scaling stays an inference. The datasheet neither confirms nor contradicts it.
  - Since "the PLL … synchronizes all timing signals" (p. 5), a PLL-derived filter clock that scales with the sync is the natural reading.
  - Scaled points: 60 → 45 Hz, 300 → 225, 1020 → 765, 2020 → 1515, 3000 → 2250, 3400 → 2550. Discrimination 4.6–72 kHz → 3.45–54 kHz. Spurious band 4.6–100 kHz → 3.45–75 kHz.
  - If the 0.60 ms absolute delay is clock-proportional, it becomes ≤ 0.80 ms at 6 kHz.
- The BCLK list is given as data rates for an 8 kHz frame: 64 kHz = 8 bits × 8 kHz, and 200 kHz = 25 × 8 kHz.
  - The sheet requires sync and BCLK to be "in phase" but never states an integer ratio.
  - At a 6 kHz sync, 64 kHz BCLK would give 10.67 BCLK per frame, and 96 kHz gives exactly 16. The ratio the RR-DR60 actually uses must come from the service manual (S-002), not from this sheet.
- The 1 µs jitter limit is 0.6 % of a 166.7 µs frame at 6 kHz.

### Gaps
- **The internal switched-capacitor or oversampling clock rate is not stated anywhere** (pp. 1–17). So the frequencies where supply ripple or clock spurs alias cannot be derived from this sheet.
- **PLL lock time, or the settling after sync start, is not stated.**
- The sheet has no statement about operation at exactly 6.0 kHz beyond the range limits.
- The sync-to-BCLK integer requirement is not stated explicitly.
- Leads: the MSM7508B/7509B datasheets (same "transmission clocks", p. 1) and the MSM7578/7579 (pin-compatible).

## Q3. Levels, full scale and noise

### Takeaway
The MSM7702 confirms the proxy's level chain exactly:
- Transmit 0 dBm0 = 0.35 Vrms at GSX (0.338–0.362).
- GSX output ±0.7 V, analog input ≤ 1.4 Vpp, so full scale is ±0.7 V = +3 dBm0.
- Receive 0 dBm0 = 0.50 Vrms (0.483–0.518). AOUT swings ±1.0 V about VDD/2 into ≥ 1.2 kΩ.

**Idle channel noise differs from the proxy.** Transmit is **−70.5 dBm0p typ** (proxy −72.5), receive **−78 typ** (proxy −76.5). The maxima are the same, −68 and −74.

### Cited Findings
- Transmit analog interface (VDD 2.7–3.8 V, −30…+85 °C):
  - Input resistance RINX (AIN+, AIN−) ≥ 10 MΩ.
  - Output load RLGX (GSX to SG) ≥ 20 kΩ. CLGX ≤ 30 pF.
  - **Output amplitude VOGX −0.7 … +0.7 V.**
  - Offset VOSGX ±20 mV (gain = 1).
  — [p. 9](https://pdf.jiepei.com/msm7702-9605035.html?page=9)
- DC: analog input resistance RIN (AIN+, AIN−) 10 MΩ typ. Input capacitance 5 pF typ. — [p. 9](https://pdf.jiepei.com/msm7702-9605035.html?page=9)
- Recommended:
  - **Analog input voltage VAIN ≤ 1.4 Vpp ("Connect AIN− and GSX")**.
  - Analog input allowable DC offset ±100 mV at gain 1, ±10 mV at gain 10.
  — [p. 8](https://pdf.jiepei.com/msm7702-9605035.html?page=8)
- Receive analog interface:
  - RLAO (AOUT to SG) ≥ 1.2 kΩ. CLAO ≤ 50 pF.
  - **Output amplitude VOAO −1.0 … +1.0 V.**
  - Offset VOSAO ±100 mV.
  — [p. 9](https://pdf.jiepei.com/msm7702-9605035.html?page=9)
- AOUT: "The output signal has a maximum amplitude of 2.0 V[PP] above and below the signal ground voltage (VDD/2). The output load resistance is a minimum of 1.2 kΩ."
  - The PP subscript overlaps the text in the conversion. With p. 9 and p. 14 this reads as 2.0 Vpp = ±1.0 V.
  — [p. 4](https://pdf.jiepei.com/msm7702-9605035.html?page=4)
- Application note: "The analog output signal has a maximum amplitude of ±1.0 V above and below the offset voltage level of VDD/2." — [p. 14](https://pdf.jiepei.com/msm7702-9605035.html?page=14)
- "The analog output signal can directly drive a piezoelectric type handset receiver". "Analog output can directly drive a load equivalent to 1.2 kΩ". — [p. 1](https://pdf.jiepei.com/msm7702-9605035.html?page=1)
- Absolute level (1020 Hz, 0 dBm0, VDD 3.0 V, 25 °C):
  - **AV T 0.338 / 0.35 / 0.362 Vrms.**
  - **AV R 0.483 / 0.50 / 0.518 Vrms.**
  - Deviation over 2.7–3.8 V and −30…85 °C: ±0.2 dB (T and R).
  — [p. 11](https://pdf.jiepei.com/msm7702-9605035.html?page=11)
- Idle channel noise (psophometric):
  - **Nidle T (AIN = SG): typ −70.5, max −68 dBm0p.**
  - **Nidle R: typ −78, max −74 dBm0p**, with input code µ-law all "1" (A-law "11010101").
  — [p. 11](https://pdf.jiepei.com/msm7702-9605035.html?page=11)
- PCM coding (p. 6), µ-law (-02):
  - +full scale `1000 0000`, +0 `1111 1111`, −0 `0111 1111`, −full scale `0000 0000`.
  - The -03 (A-law) inverts the even bits.
  - "compatible with the ITU-T recommendation on coding law and output coding format".
  — [p. 6](https://pdf.jiepei.com/msm7702-9605035.html?page=6)

### Inferences
- **Full scale.** ±0.7 V at GSX (p. 9) and 1.4 Vpp at the input with gain 1 (p. 8) against 0.35 Vrms = 0 dBm0 (p. 11) give a peak of 0.495 V at 0 dBm0.
  - So the full scale of 0.7 V peak is +3.0 dBm0 (20·log(0.7/0.495) = 3.01 dB).
  - This confirms A-030's mapping "0 dBFS = +3 dBm0 = 0.7 V peak" from the MSM7702 itself, with no proxy needed.
- **Receive.** 0.5 Vrms at 0 dBm0 gives 0.707 V peak, so +3 dBm0 is 0.999 V peak, matching VOAO ±1.0 V. A-030's "+3 dBm0 = 1.0 V peak" is confirmed.
  - The MSM7702's receive output pin is **AOUT, not VFRO** (the MSM7717's name). Only the label differs.
  - Its minimum load is 1.2 kΩ, and it is said to drive a piezo handset receiver directly.
- **Idle noise against the emulator's 0 dBFS (sine at +3 dBm0):**
  - Transmit noise is **−73.5 dB typ / −71 dB max** below the full-scale sine (psophometric). A-030 says "≈ 75 dB".
  - Receive is −81 dB typ / −77 dB max.
  - The transmit figure is measured with AIN = SG, so it includes the input op-amp and the A/D but not the external SG-referenced circuitry.
- 0 dBm0 at AIN with gain 1 is 0.35 Vrms. The VAIN limit of 1.4 Vpp is exactly the codec's full scale, so the input is not spec'd beyond full scale.

### Gaps
- The datasheet does not say whether the A/D clips hard at full scale or saturates in the GSX op-amp. It gives no overload-recovery figure.

## Q4. Frequency response, distortion, gain tracking, group delay, discrimination, crosstalk, IMD

### Takeaway
The limits on the transmit and receive templates match the proxy, but several **typical** values differ:
- Transmit loss at 3400 Hz: **0.5 dB typ** (proxy 0.4).
- Receive loss at 3400 Hz: **0.47 dB typ** (proxy 0.56).
- Receive at 3000 Hz is limited to **+0.20 dB max** (proxy ≤ 0.25), with a −0.15 dB minimum stated.

The distortion figures nearly match: transmit S/D at −30 dBm0 is 37 dB (proxy 38) and at −40 dBm0 is 29.5 (proxy 30). The group delay rows are identical. New rows absent from the proxy summary are IMD, crosstalk and the receive S/D values.

### Cited Findings
All rows at 0 dBm0, loss relative to 1020 Hz (min / typ / max):
- **Transmit frequency response:**
  - 60 Hz: ≥ 20, typ 26 dB.
  - 300 Hz: −0.15 / **+0.1** / +0.20.
  - 1020 Hz: reference.
  - 2020 Hz: −0.15 / **−0.04** / +0.20.
  - 3000 Hz: −0.15 / **+0.13** / +0.20.
  - **3400 Hz: 0 / 0.5 / 0.80 dB.**
  — [p. 10](https://pdf.jiepei.com/msm7702-9605035.html?page=10)
- **Receive frequency response:**
  - 300 Hz: −0.15 / **−0.04** / +0.20.
  - 1020 Hz: reference.
  - 2020 Hz: −0.15 / **+0.02** / +0.20.
  - 3000 Hz: −0.15 / **+0.10** / +0.20.
  - **3400 Hz: 0.0 / 0.47 / 0.80 dB.**
  - No receive row below 300 Hz.
  — [p. 10](https://pdf.jiepei.com/msm7702-9605035.html?page=10)
- **Transmit S/D at 1020 Hz (psophometric; µ-law upper row, A-law lower)**, min / typ:
  - +3 dBm0: 35 / 43.
  - 0: 35 / 41.
  - −30: 35 / **37**.
  - −40: 28 / **29.5** (A-law 29).
  - −45: 23 / **25** (A-law 24).
  — [p. 10](https://pdf.jiepei.com/msm7702-9605035.html?page=10)
- **Receive S/D**, min / typ:
  - +3: 36 / 43.
  - 0: 36 / 41.
  - −30: 36 / 40.
  - −40: 30 / 33.5 (A-law 29 / 32).
  - −45: 25 / 30 (A-law 24 / 27).
  — [p. 10](https://pdf.jiepei.com/msm7702-9605035.html?page=10)
- **Gain tracking (1020 Hz, re −10 dBm0):**
  - Transmit, min / typ / max:
    - +3 dBm0: −0.3 / 0 / +0.3.
    - −40: −0.3 / +0.1 / +0.3.
    - −50: −0.5 / −0.03 / +0.6.
    - −55: −1.2 / 0 / +1.2.
  - Receive, min / typ / max:
    - +3: −0.3 / 0.0 / +0.3.
    - −40: −0.3 / +0.11 / +0.3.
    - −50: −0.6 / +0.22 / +0.6.
    - −55: −1.2 / +0.15 / +1.2.
  — [p. 10](https://pdf.jiepei.com/msm7702-9605035.html?page=10)
- **Absolute delay** Td (A to A, BCLK = 64 kHz): max 0.60 ms. — [p. 11](https://pdf.jiepei.com/msm7702-9605035.html?page=11)
- **Group delay distortion** (*4: relative to the minimum), typ / max ms:
  - Transmit:
    - 500 Hz: 0.19 / 0.75.
    - 600 Hz: 0.11 / 0.35.
    - 1000 Hz: 0.02 / 0.125.
    - 2600 Hz: 0.05 / 0.125.
    - 2800 Hz: 0.07 / 0.75.
  - Receive:
    - 500 Hz: 0.00 / 0.75.
    - 600 Hz: 0.00 / 0.35.
    - 1000 Hz: 0.00 / 0.125.
    - 2600 Hz: 0.09 / 0.125.
    - 2800 Hz: 0.12 / 0.75.
  — [p. 11](https://pdf.jiepei.com/msm7702-9605035.html?page=11)
- **Crosstalk attenuation** (1020 Hz, 0 dBm0): transmit → receive min 75, typ 85 dB. Receive → transmit min 70, typ 80 dB. — [p. 11](https://pdf.jiepei.com/msm7702-9605035.html?page=11)
- **Discrimination:** for 4.6–72 kHz input at 0 dBm0, measured in 0–4000 Hz: min 30, typ 32 dB. — [p. 12](https://pdf.jiepei.com/msm7702-9605035.html?page=12)
- **Out-of-band spurious:** for a 300–3400 Hz input at 0 dBm0, measured in 4.6–100 kHz: typ −37.5, max −35 dBm0. — [p. 12](https://pdf.jiepei.com/msm7702-9605035.html?page=12)
- **Intermodulation distortion:** fa = 470 Hz and fb = 320 Hz at −4 dBm0, product 2fa − fb. Typ −52, max −35 dBm0. — [p. 12](https://pdf.jiepei.com/msm7702-9605035.html?page=12)
- Description: "voice signals ranging from 300 to 3400 Hz". Block diagram: transmit is AIN op-amp → **RC LPF → 8th BPF** → AD (auto zero) → TCONT; receive is RCONT → DA → **5th LPF** → resistor → AOUT inverting amp referenced to SG. — [p. 1](https://pdf.jiepei.com/msm7702-9605035.html?page=1), [p. 2](https://pdf.jiepei.com/msm7702-9605035.html?page=2)

### Inferences
- **The record template (A-027, stage 4) now has a primary source** instead of the proxy:
  - The limits are identical: 300 Hz −0.15…+0.20, 3000 Hz ≤ +0.20, 3400 Hz 0…0.8, 60 Hz ≥ 20.
  - The 2550 Hz design goal in `tools/filter-design/design_voiceband.py` (`edge_typ_db=0.4`, line 91) would become **0.5 dB**. The CLAUDE.md status says the shipped record stage reads 0.31 dB at 2550 Hz, so it moves 0.19 dB further from the typical, still inside the 0…0.8 dB limit.
- **The playback template (stage 10):**
  - The goal `edge_typ_db=0.56` (line 103) would become **0.47 dB**. The shipped 0.48 dB is then within 0.01 dB of the MSM7702 typical.
  - The passband upper limit is **+0.20 dB**, not +0.25. A-027 says "−0.15 … +0.25 dB from DC to 2250 Hz".
  - The −0.15 dB lower limit that A-027 "carried from the transmit row (engineering target)" is **stated directly** for receive on p. 10.
  - The receive table has no row below 300 Hz, so the "passes DC" reading stays unconstrained by the MSM7702 sheet.
- The S/D rows confirm µ-law companding behavior. At 6 kHz they are "not guaranteed" (p. 5).
- The group-delay rows are identical to S-005, so A-027's scaled GDD limits are unchanged: 1.0 / 0.47 / 0.17 / 1.0 ms after ÷ 0.75.
- Crosstalk of ≥ 70 dB is irrelevant for a recorder that records and plays at different times. It would matter only if both paths ran at once (e.g. monitoring while recording).

### Gaps
- There is no frequency-response row between 3400 Hz and 4000 Hz (2550–3000 Hz at 6 kHz). A-027's note that "no source constrains" this region stands.
- There is no full-scale (+3 dBm0) clipping or overload characteristic.

## Q5. Power: supply, currents, PSRR, SG reference, start-up, power-down behavior, input op-amp

### Takeaway
- Supply: 2.7–3.8 V (typ 3.0 V), "Voltage must be fixed".
- IDD: operating 5 / 9 mA, power-down 0.01 / 0.05 mA, power-save 1.2 / 3.0 mA.
- **PSRR is a single typical figure: 30 dB (transmit and receive), 0 to 50 kHz, 50 mVpp ripple, measured under idle channel.** There is no curve versus frequency and no minimum.
- SG is VDD/2, with ±200 µA drive and a 0.1 µF capacitor on SGC. Its level is "undefined during power saving or power down mode".
- **No SG source impedance, SG noise, SG settling time, start-up time, PLL lock time, power-on mute or post-PDN muting is stated anywhere.**

### Cited Findings
- VDD: "Power supply for +2.7 V to +3.8 V. (Typically 3.0 V)". — [p. 5](https://pdf.jiepei.com/msm7702-9605035.html?page=5)
- Recommended VDD 2.7 / 3.0 / 3.8 V, "Voltage must be fixed". Absolute maximum 0 to 7 V. Analog and digital inputs −0.3 to VDD + 0.3 V. — [p. 8](https://pdf.jiepei.com/msm7702-9605035.html?page=8)
- Power consumption at VDD = 3 V: operating 15 mW typ, power save 3.6 mW typ, power down 0.05 mW typ. — [p. 1](https://pdf.jiepei.com/msm7702-9605035.html?page=1)
- Supply current:
  - **IDD1 operating, no signal: typ 5, max 9 mA.**
  - **IDD2 power-down (PDN = 0): 0.01 / 0.05 mA.**
  - **IDD3 power-save (PDN = 1, XSYNC → OFF): 1.2 / 3.0 mA.**
  — [p. 9](https://pdf.jiepei.com/msm7702-9605035.html?page=9)
- Leakage: IIH ≤ 2.0 µA, IIL ≤ 0.5 µA, PCMOUT ≤ 10 µA (Symbol µ). — [p. 9](https://pdf.jiepei.com/msm7702-9605035.html?page=9)
- **Power Supply Noise Rejection Ratio:** PSR T and PSR R, frequency "0 to 50 kHz", level "50 mVPP", condition *5 "The measurement under idle channel noise". **Typ 30 dB**, no min, no max. — [p. 12](https://pdf.jiepei.com/msm7702-9605035.html?page=12)
- SG: "Signal ground voltage output. The output voltage is 1/2 of the power supply voltage. The output drive current capability is ±200 µA. This pin provides the SG level for CODEC peripherals. This output voltage level is undefined during power saving or power down mode." — [p. 7](https://pdf.jiepei.com/msm7702-9605035.html?page=7)
- SGC: "Used to generate the signal ground voltage level by connecting a bypass capacitor. Connect a 0.1 µF capacitor with excellent high frequency characteristics between the AG pin and the SGC pin." — [p. 7](https://pdf.jiepei.com/msm7702-9605035.html?page=7)
- GSX: "During power saving and power down modes, the GSX output is at AG voltage." — [p. 4](https://pdf.jiepei.com/msm7702-9605035.html?page=4)
- AOUT: "During power saving or power down mode, the output of AOUT is at the voltage level of the signal ground." — [p. 4](https://pdf.jiepei.com/msm7702-9605035.html?page=4)
- PDN: "A logic "0" level drives both transmit and receive circuits to a power down state." Application circuit: "1" = Operation, "0" = Power down. — [p. 6](https://pdf.jiepei.com/msm7702-9605035.html?page=6), [p. 14](https://pdf.jiepei.com/msm7702-9605035.html?page=14)
- PCMOUT is high-impedance during power saving or power down. — [p. 6](https://pdf.jiepei.com/msm7702-9605035.html?page=6)
- Application circuit:
  - +3 V reaches VDD through "0 to 10 Ω", decoupled by 10 µF (electrolytic) and 1 µF to DG.
  - 0.1 µF from SGC to AG.
  - AG tied to DG and 0 V.
  - AIN+ tied to SG (inverting input type).
  - 1 kΩ pull-up on PCMOUT.
  - The capacitor units are printed "mF" in the conversion; µF by the Symbol key.
  — [p. 14](https://pdf.jiepei.com/msm7702-9605035.html?page=14)
- Input op-amp configurations (p. 4):
  - **Inverting:** R2 > 20 kΩ, C1 > 1/(2·3.14·30·R1), Gain = R2/R1 ≤ 10.
  - **Non-inverting:** R3, R4 > 20 kΩ, R5 > 50 kΩ, C2 > 1/(2·3.14·30·R5), Gain = 1 + R4/R3 ≤ 10.
  - "When not using AIN− and AIN+, connect AIN− to GSX and AIN+ to SG."
  — [p. 4](https://pdf.jiepei.com/msm7702-9605035.html?page=4)
- Design recommendations (p. 15):
  - Use HF bypass capacitors close to the pins.
  - Join AG and DG as close as possible.
  - No IC sockets.
  - EM shielding near power-supply transformers.
  - "Keep the voltage on the VDD pin not lower than −0.3 V even instantaneously to avoid latch-up phenomenon when turning the power on."
  - "Use a low noise (particularly, low level type of high frequency spike noise or pulse noise) power supply to avoid erroneous operation and the degradation of the characteristics".
  — [p. 15](https://pdf.jiepei.com/msm7702-9605035.html?page=15)
- "Built-in reference voltage supply". The block diagram has separate "SG GEN" (SGC, SG pins) and "VR GEN" blocks and a "PWD Logic" block. — [p. 1](https://pdf.jiepei.com/msm7702-9605035.html?page=1), [p. 2](https://pdf.jiepei.com/msm7702-9605035.html?page=2)

### Inferences
- **The PSRR figure, if read as output-referred** (an assumption about OKI's definition; the sheet does not define it):
  - 50 mVpp of ripple is about 17.7 mVrms if sinusoidal. At 30 dB rejection that is about 0.56 mVrms.
  - Transmit: about **−56 dBm0, or about −59 dB below the emulator's full-scale sine**.
  - Receive (0.5 Vrms reference): about −59 dBm0, or −62 dB re full scale.
  - Only the typical is given, the measurement band (0–50 kHz) is lumped, and how the 0–50 kHz band maps through the 6 kHz clock is not stated.
  - So any supply-ripple model built on it is an assumption, not a datasheet figure.
- The sheet's emphasis on "high frequency spike noise or pulse noise" (p. 15) implies that switching-regulator or digital spikes alias into the band. The sheet does not quantify this.
- **SG settling, only an order-of-magnitude bound.** The SG generator is buffered (SG out ±200 µA) with an internal divider filtered by the external 0.1 µF on SGC.
  - The divider impedance is not given. With a hypothetical 50–500 kΩ divider, τ = 5–50 ms, a few τ = 15–250 ms.
  - This is a **plausibility range**, not a datasheet value. It belongs in the register as an assumption if it is used to model a record-start thump.
- "Undefined" SG (p. 7) versus "AOUT at the voltage level of the signal ground" (p. 4) during power-down are mutually inconsistent as written.
  - Read together, AOUT tracks whatever SG does. So on wake-up AOUT should ramp with SG rather than step, and the external coupling capacitor sees the SG transition.
  - GSX is stated as "at AG voltage" (AG = analog ground pin), which may be a wording slip for SG. Treat it as uncertain.
- **The input op-amp:**
  - Max gain is 10, feedback resistance ≥ 20 kΩ (the GSX load), input resistance ≥ 10 MΩ, GSX swing ±0.7 V, offset ±20 mV.
  - The recommended input coupling corners are ≤ 30 Hz.
  - **Open-loop gain, bandwidth, slew rate and noise density are not given.**
- The SG pin's ±200 µA drive is the only datasheet figure that bounds what the AIN+ / SG reference can source. It is a current capability, not a source impedance (sources.md asks for the latter for A-034).

### Gaps
- PSRR versus frequency: **not given**, only one typ figure over 0–50 kHz.
- SG output voltage tolerance, SG noise, SG output impedance and SG/VR start-up or settling time: **not given**.
- PLL lock or wake-up time after PDN → 1 or after the sync starts, and any power-on reset, "mute during start-up" or "output muted for N frames": **not given anywhere in pp. 1–17**.
- Op-amp open-loop gain and noise: **not given**.
- Leads for these: OKI application notes for the MSM75xx/77xx family, and the MSM7508B/7509B, MSM7578/7579 and MSM7716/7717 datasheets. The MSM7717 (S-005) was read in full by the project and its summary lists no such row. Any of these figures needs a measurement on a real unit (e.g. scope AOUT and SG at PDN ↑).

## Q6. Pin descriptions (PDN, ALAW, TCONT / auto-zero, SG, unused pins)

### Takeaway
- PDN is active-low power-down. ALAW exists only on the -01 and is internally pulled down (µ-law if open). TCONT and AUTO ZERO are **internal blocks, not pins**.
- The 20-pin SSOP has four NC pins (5, 6, 15, 16) and pin 17 is (ALAW). On the -02 that pin carries no function per the footnote.
- Power save is entered by stopping XSYNC or BCLK (PDN = 1). Power down is entered by PDN = 0.

### Cited Findings
- 20-pin SSOP:
  - 1 SGC, 2 SG, 3 AOUT, 4 VDD, 5 NC, 6 NC, 7 DG, 8 PDN, 9 RSYNC, 10 PCMIN.
  - 11 PCMOUT, 12 XSYNC, 13 BCLK, 14 AG, 15 NC, 16 NC, 17 (ALAW)*, 18 GSX, 19 AIN−, 20 AIN+.
- 24-pin SOP:
  - 1 SGC, 3 SG, 5 AOUT, 6 VDD, 7 DG, 10 PDN, 11 RSYNC, 12 PCMIN, 13 PCMOUT, 14 XSYNC, 16 BCLK, 18 AG, 19 (ALAW)*, 21 GSX, 23 AIN−, 24 AIN+.
  - NC on 2, 4, 8, 9, 15, 17, 20, 22.
- "* The ALAW pin is only applied to the MSM7702-01GS-K/MSM7702-01MS-K."
— [p. 3](https://pdf.jiepei.com/msm7702-9605035.html?page=3)
- ALAW: "Provides only for the MSM7702-01GS-K/7702-01MS-K … µ-law when this pin is at a logic "0" … A-law when … "1" … operates in the µ-law if the pin is left open, since this pin is internally pulled down." — [p. 7](https://pdf.jiepei.com/msm7702-9605035.html?page=7)
- Product codes: -01 µ/A-law pin selectable, **-02 µ-law**, -03 A-law. Packages SOP24 (…GS-K) and SSOP20 (…MS-K). — [p. 1](https://pdf.jiepei.com/msm7702-9605035.html?page=1)
- DG: separate from analog ground and "must be connected to the AG pin on the printed circuit board". AG: "Analog signal ground." — [p. 6](https://pdf.jiepei.com/msm7702-9605035.html?page=6), [p. 4](https://pdf.jiepei.com/msm7702-9605035.html?page=4)
- PCMIN: data shifted on the falling BCLK edge, latched after eight bits, MSD at the rising edge of RSYNC. PCMOUT: open drain, needs a pull-up (≥ 0.5 kΩ, ≤ 100 pF; p. 8). — [p. 5](https://pdf.jiepei.com/msm7702-9605035.html?page=5), [p. 6](https://pdf.jiepei.com/msm7702-9605035.html?page=6)

### Inferences
- S-002 names the codec MSM7702-02 with ALAW open. Either way the part is µ-law: by part number (-02), or by the -01's internal pull-down if the part were a -01. **A-003 is doubly confirmed.**
- Which pins the RR-DR60 leaves open, beyond ALAW, must be read from S-002's schematic. The datasheet only defines the NC pins.

### Gaps
- The function of pin 17 on the -02 / -03 is not stated beyond the footnote. It is presumably unconnected internally.

## Q7. Row-by-row comparison: MSM7717 proxy (S-005) → MSM7702 (S-003) → effect on the registers

Proxy values are as summarized in docs/hardware/sources.md S-005 and the assumption rows (read 2026-10-17). The MSM7702 values are from the pages cited.

| Quantity | MSM7717 proxy (S-005) | MSM7702 (page) | Same? | Register row affected |
|---|---|---|---|---|
| Sync range / guarantee | 8 kHz ± 50 ppm guaranteed; 6.0 / 8.0 / 10 kHz | identical wording; 6.0 / 8.0 / 10.0 kHz (p. 5, p. 8) | same | A-027 basis (now primary source), A-026 context |
| BCLK list | 64 … 2048 kHz | 13 values incl. 200 and 1544 kHz (p. 5, p. 8) | same | — |
| Allowable jitter | 1000 ns | 1 µs (p. 8) | same | — |
| Sync pulse width | (not in S-005 summary) | 1 BCLK … 100 µs (p. 8) | new | — |
| Transmit 0 dBm0 at GSX | 0.35 Vrms (0.338–0.362) | 0.35 Vrms (0.338–0.362) (p. 11) | same | A-030 (now primary) |
| GSX full scale | ±0.7 V (+3 dBm0) | ±0.7 V (p. 9); VAIN ≤ 1.4 Vpp (p. 8) | same | A-030, A-028 knee anchor, A-029 level anchor |
| Receive 0 dBm0 | 0.5 Vrms at VFRO | 0.50 Vrms (0.483–0.518) at AOUT (p. 11) | same (pin name differs) | A-030; A-043 volume reference unaffected |
| Receive full-scale swing | +3 dBm0 = 2.0 Vpp | ±1.0 V about VDD/2, load ≥ 1.2 kΩ, ≤ 50 pF (p. 4, 9, 14) | same | A-030; A-031 / A-032 output loads (1.2 kΩ min) |
| Tx loss 60 Hz | ≥ 20 (26 typ) | ≥ 20, 26 typ (p. 10) | same | A-027 (45 Hz point) |
| Tx loss 300 Hz | +0.07 typ (−0.15 … +0.2) | **+0.1 typ** (−0.15 … +0.20) | limits same, typ differs | A-027 (225 Hz) — none needed |
| Tx loss 2020 Hz | −0.01 | **−0.04** (−0.15 … +0.20) | typ differs | A-027 (1515 Hz) — none needed |
| Tx loss 3000 Hz | +0.15 (≤ 0.2) | **+0.13** (−0.15 … +0.20) | typ differs | A-027 (2250 Hz) — none needed |
| Tx loss 3400 Hz | 0.4 typ (0 … 0.8) | **0.5 typ** (0 … 0.80) | **typ differs** | **A-027 record edge; `design_voiceband.py` `edge_typ_db=0.4` → 0.5** |
| Rx loss 300 Hz | −0.03 | **−0.04** (−0.15 … +0.20) | typ differs | A-027 (225 Hz) |
| Rx loss 2020 Hz | — | +0.02 (−0.15 … +0.20) | new | A-027 |
| Rx loss 3000 Hz | +0.15 (≤ 0.25) | **+0.10 (−0.15 … +0.20)** | **limits and typ differ** | **A-027 playback passband: "+0.25" → +0.20; the −0.15 lower bound becomes sourced, not "carried"** |
| Rx loss 3400 Hz | 0.56 typ (≤ 0.8) | **0.47 typ** (0.0 … 0.80) | **typ differs** | **A-027 playback edge; `edge_typ_db=0.56` → 0.47 (shipped 0.48 then matches)** |
| Discrimination | 4.6–72 kHz ≥ 30 (32) | same (p. 12) | same | A-027 anti-alias (3.45 kHz at 6 kHz) |
| Out-of-band spurious | −37.5 typ | −37.5 typ / −35 max, 4.6–100 kHz (p. 12) | same | A-027 |
| Tx S/D +3 / 0 dBm0 | 43 / 41 | 43 / 41 (min 35) (p. 10) | same | A-030, A-003 |
| Tx S/D −30 dBm0 | 38 | **37** (min 35) | differs (1 dB) | A-030 (not quoted there) |
| Tx S/D −40 dBm0 | 30 | **29.5** (min 28) | differs (0.5 dB) | **A-030 "30 dB at −40" → 29.5** |
| Tx S/D −45 dBm0 | 25 | 25 (min 23) | same | A-030 |
| Rx S/D | (not in summary) | 43 / 41 / 40 / 33.5 / 30 at +3 / 0 / −30 / −40 / −45 (p. 10) | new | A-030 (playback-side µ-law) |
| Gain tracking | ±0.3 dB, +3 … −40 dBm0 | ±0.3 (+3 … −40); Tx −0.5/+0.6, Rx ±0.6 at −50; ±1.2 at −55 (p. 10) | same + more rows | A-003 / A-030 |
| **Idle noise Tx** | **−72.5 typ (−68 max)** | **−70.5 typ (−68 max) dBm0p**, AIN = SG (p. 11) | **typ differs by 2 dB** | **A-030 ("≈ 75 dB below FS" → ≈ 73.5 dB typ, 71 dB worst); A-003 (cites −72.5); A-050 context** |
| **Idle noise Rx** | **−76.5 typ (−74 max)** | **−78 typ (−74 max) dBm0p** (p. 11) | **typ differs by 1.5 dB** | sources S-005 text; any playback noise floor |
| Absolute delay | ≤ 0.6 ms at BCLK 64 kHz | ≤ 0.60 ms (p. 11) | same | (codec latency context) |
| Group delay distortion | Tx 0.19/0.11/0.02/0.05/0.07; Rx 0/0/0/0.09/0.12; max 0.75/0.35/0.125/0.125/0.75 | identical (p. 11) | same | A-027 GDD limits unchanged |
| IMD | — | −52 typ / −35 max dBm0 (2fa − fb, 470/320 Hz, −4 dBm0) (p. 12) | new | candidate codec-distortion check |
| Crosstalk | — | T→R 75/85, R→T 70/80 dB (p. 11) | new | none (paths not simultaneous) |
| PSRR | — | **30 dB typ, 0–50 kHz, 50 mVpp, idle (p. 12)** | new | power-supply-effects research (stages 14–15) |
| Supply / currents | (15 mW operating per S-003 p. 1) | 2.7–3.8 V; IDD 5/9 mA, PDN 0.01/0.05 mA, save 1.2/3.0 mA (p. 9) | new | stage 15 power research |
| SG | VDD/2 | VDD/2, ±200 µA drive, 0.1 µF on SGC, undefined in power-save/down (p. 7) | same + more | **A-050 (no SG noise figure exists in S-003)**; A-034 (only a drive-current figure, no source impedance) |
| Coding format | ITU-T, µ-law FS `1000 0000` | identical (p. 6); -02 = µ-law, ALAW pulled down (p. 1, p. 7) | same | A-003 |

### Takeaway
The MSM7702 sheet confirms the proxy's level chain, limits, group delay and clocking rules. Four typical values differ in ways the registers quote:
- Tx 3400 Hz: 0.5 vs 0.4.
- Rx 3400 Hz: 0.47 vs 0.56.
- Rx 3000 Hz max: +0.20 vs +0.25.
- Tx idle noise: −70.5 vs −72.5 dBm0p.

A-030's S/D at −40 dBm0 moves by 0.5 dB, to 29.5.

### Cited Findings
- All MSM7702 values are as cited in Q2–Q6, pages [8](https://pdf.jiepei.com/msm7702-9605035.html?page=8), [9](https://pdf.jiepei.com/msm7702-9605035.html?page=9), [10](https://pdf.jiepei.com/msm7702-9605035.html?page=10), [11](https://pdf.jiepei.com/msm7702-9605035.html?page=11) and [12](https://pdf.jiepei.com/msm7702-9605035.html?page=12).
- The proxy values are from the S-005 row of docs/hardware/sources.md, which summarizes the [MSM7717 datasheet](http://igorkov.org/revo/datasheets/M7717.pdf).

### Inferences
- **S-003** can be marked as read in full (pp. 1–17, 2026-10-17, via the jiepei HTML render). S-005 drops from "proxy until obtained" to a corroborating sibling.
- **A-027:** the basis can cite S-003 directly.
  - Change the playback passband upper bound from +0.25 to +0.20 dB.
  - Note the −0.15 dB receive lower bound as sourced.
  - Update the edge typicals to 0.5 (record) and 0.47 (playback).
  - Whether to re-run the design script against the new typicals is a CHANGELOG-recorded choice (a filter change would re-bless goldens). The shipped stages already meet the 0 … 0.8 dB limits, so this is not a correctness issue.
- **A-030:**
  - The levels are confirmed: 0.35 Vrms, ±0.7 V, 0.5 Vrms, ±1.0 V.
  - Change the idle noise to −70.5 dBm0p typ (−68 max), i.e. ≈ 73.5 dB below full scale typ.
  - Change S/D at −40 dBm0 to 29.5 dB.
  - The confidence can rise from "Medium (proxy datasheet)" for the 8 kHz values. At 6 kHz they stay "not guaranteed" (p. 5).
- **A-003:** update the cited noise figure to −70.5 dBm0p. The µ-law verification is reinforced by the ALAW pull-down.
- **A-050:** S-003 contains **no SG reference-noise figure**, so the lead "S-003's unread pages 4–17" is exhausted.
  - The transmit idle noise (−70.5 dBm0p with AIN = SG) bounds the codec's own input-referred noise.
  - At gain 1 and 0 dBm0 = 0.35 Vrms, −70.5 dBm0p is about 104 µVrms psophometric at GSX. This is far larger than the 1.2 µV that A-050 says would equal the whole hiss.
  - So the datasheet cannot exclude a dominant SG contribution. A capture with the capsule disconnected remains the only check.
- **sources.md's open item** ("AIN+ reference output's source impedance … A-034") is answered only partly: SG can drive ±200 µA. No impedance is given.

### Gaps
- None of the following is in the MSM7702 datasheet:
  - the internal filter clock rate;
  - the PLL lock time;
  - the SG settling or start-up time;
  - a start-up mute;
  - SG noise;
  - op-amp open-loop gain;
  - PSRR versus frequency.
- These remain open for the record-start-transient and power-supply models. They need either a family application note or a measurement.
