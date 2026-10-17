# Power and enable sequencing of the RR-DR60 in record, playback and stop, and the start-of-item transients it implies (stage 15)

Scope: this note resolves (or bounds) the open items left by the first service-manual reading ([service_manual_power_clock.md](service_manual_power_clock.md)) and the power researcher's notes ([power_supply_effects.md](power_supply_effects.md)). It then turns the sequencing into a quantified estimate of the record-start and playback-start transients, and checks whether the speaker or earphone carries anything while recording.

**Sources.**
- Primary: Panasonic RR-DR60 Service Manual, Order No. AD9810180A3, © 1998 Matsushita ([Internet Archive `manual_RRDR60_SM_PANASONIC_EN`](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN), S-002). Its 600 dpi page images were re-read on 2026-10-17 from the first researcher's download (`RRDR60_SM_PANASONIC_EN_jp2.zip`, already in the session scratchpad, so nothing was downloaded again), with new crops of sheet 3.1 (image 8), sheet 3.2 (image 9), sheet 3.3 (image 10) and the notes page (image 12).
- MSM7702 pin descriptions: the parallel clock researcher's page captures of the OKI MSM7702-01/02/03 datasheet ([pdf.jiepei.com, pages 4–9](https://pdf.jiepei.com/msm7702-9605035.html?page=4)). No `msm7702_datasheet.md` exists in this folder yet; the MSM7702 pages were read directly instead of the MSM7717 proxy.

**Tags.**
- **[read]**: printed on the sheet or in the cited document.
- **[traced]**: a line followed across crossings or sheets. I checked it, but it is my reading of the line work.
- **[inferred]**: my engineering inference, not printed.

The manual is quoted only in short extracts.

Calculations were run with a scratch script (not in the repository; reproduced in prose here). It used numpy at 192 kHz for the analog part, then a block average to 6 kHz, and the emulator's own record band-pass coefficients `RECORD_SOS` ([crates/rr_dr60/src/stages/voiceband_coeffs.rs](../../../../../crates/rr_dr60/src/stages/voiceband_coeffs.rs)), so the in-band figures are on the same footing as the engine.

## Q1. Q10 / Q13 and the switched 3.3 V "H" rail: is Q13 a PNP, is the logic inverted, or is H fed another way?

### Takeaway
- **Q13 is an ordinary NPN** (2SD1819A, drawn with its emitter grounded) and **Q10 a PNP** (2SB1295). There are no built-in-resistor (UN/XN-type) parts here.
- H is not fed any other way: it is Q10's collector only.
- The **H rail must be on while recording**. It powers the 8 MHz oscillator IC3, the clock buffer IC5 and the flash, and the sheet prints the running 8 MHz waveform and 3.3 V on every H load for the recording state.
- So the printed **P73 = 0 V / Q13 base = 0 V / Q13 collector = 2.8 V cannot be recording-state values**: they describe Q13 *off*.
- They are self-consistent with Q13 off and H off, read with a 10 MΩ meter. That points to values taken in another state (an idle stop) or misprinted, not to an inverted logic.
- The circuit's own default supports **P73 released (high-Z) or high = H on**: R63 330 kΩ pulls Q13's base up, so H comes on whenever the microcontroller's port floats (at reset). That is what a microcontroller that boots on the 8 MHz clock from IC3 needs.

### Cited Findings
- **Q13 and Q10, sheet 3.1** [read]:
  - Q13 "2SD1819ASTX / POWER SUPPLY CONTROL", NPN symbol, emitter to ground, collector 2.8V, base 0V.
  - R27 4.7K runs from Q13's collector to Q10's base, which reads 2.6V.
  - Q10 "2SB1295-6-TB", PNP symbol: emitter 3.3V on the 3.3 V rail, collector 3.3V driving the thick line "H".
  - R63 330K runs from the 3.3 V rail to Q13's base node, which continues to bus letter C. Q10 has **no base–emitter resistor**.

  — [S-002 sheet 3.1](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **P73 reads 0 V** [read]: IC4 pin 62 "P73" is printed "0V" on sheet 3.3, on the line to bracket letter C. P74 (pin 63, letter B) also reads "0V", so the reading does not depend on which of the two letters reaches Q13 — [S-002 sheet 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **H rail loads, sheet 3.3** [traced]:
  - H enters at the bottom right and runs to C41 0.1 µF / C40 4 V 22 µF, then down to the flash IC7 (SDTB-16A) VDD.
  - It continues as a thick "<3.3V>" bus, arrow pointing left, along the bottom to IC5 TC7W126 Vcc (pin 8, 3.3V).
  - From there it runs up the left edge, crossing the main 3.3 V line without a junction dot, to IC3 TC7WU04 Vcc (pin 8, 3.3V, C63 0.1 µF).

  This confirms the first researcher's trace — [S-002 sheet 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **The 8 MHz clock is running in the measured state** [read]:
  - "F=8MHz, 3V P-P" at IC3 and "F=8MHz, 3.2V P-P" at the DSP's OSCF0.
  - IC3 pins 1/2/6/7 read 1.6/1.6/1.7/1.7 V (mid-rail, i.e. oscillating).
  - The buffered 8 MHz also clocks the microcontroller IC4 (OSC1, pin 10, 1.6 V) and the DSP. Without H, neither processor has its main clock.

  — [S-002 sheets 3.2, 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **Transistor types**: the SeCoS 2SB1218A datasheet lists it as a PNP "complementary to 2SD1819A"; a Panasonic 2SD1819A listing gives NPN, 50 V, 100 mA, 150 mW, SMini3 — [SeCoS 2SB1218A (alldatasheet)](https://www.alldatasheet.net/datasheet-pdf/pdf/419422/SECOS/2SB1218A.html); [Panasonic 2SD1819ASL (GlobalSpec)](https://datasheets.globalspec.com/ds/panasonic/2sd1819asl/cfe57e9b-914e-41e5-9d69-f6d3dfb8a2ca)
  - The sheet's package table groups 2SB1218ASTX / 2SB1295-6-TB / 2SD1819ASTX / 2SD1328STX under one three-lead outline with B, C, E pins, i.e. plain bipolar transistors [read] — [S-002 notes page](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
  - I found no datasheet for the 2SB1295. Its PNP polarity rests on the sheet symbol [read] and the "2SB" prefix.
- **Microcontroller reset behavior (same family, MN101C51F manual)**:
  - "the starting mode is NORMAL mode that high oscillation is the base clock";
  - ports are tri-state, and "At reset, the input mode is selected and pull-up resistors are disabled (high impedance output)";
  - some port bits have "n-channel open-drain" options.

  The RR-DR60's MN101C16ACA3 manual was not found; the family behavior is a proxy — [Panasonic MN101C51F LSI manual (bitsavers)](https://bitsavers.mirrorservice.org/components/panasonic/panaXseries/21451-010E_MN101C51F.pdf)

### Inferences
- [inferred] **Q10 on in recording.** Q10's base at 2.6 V with its emitter at 3.3 V is a forward-biased junction (Vbe ≈ 0.7 V). Through R27 that needs ≈ (3.3 − 0.7 − 0.1)/4.7 kΩ ≈ 0.53 mA of base current, which only a saturated Q13 can sink. Together with the running 8 MHz and the 3.3 V printed at IC3, IC5 and the flash, the H rail is certainly up in recording.
- [inferred] **The printed Q13 values describe Q13 off**, and they are self-consistent for that state:
  - With Q13 off, Q10's base node floats (no base–emitter resistor), and a 10 MΩ voltmeter on Q13's collector draws ≈ 0.28 µA through Q10's emitter–base junction.
  - At that current a small PNP's Vbe ≈ 0.44–0.50 V (Is = 10⁻¹⁴…10⁻¹⁵ A, VT = 25.7 mV), so the node reads 3.3 − 0.44…0.50 ≈ **2.80–2.86 V**: the printed "2.8V".
  - The base reads 0 V (P73 low).
  - But **Q10's own base (2.6 V) and collector (3.3 V) describe Q10 on**. The five values around Q10/Q13 therefore mix two states. No single state of this circuit gives all five.
- [inferred] **Polarity of the control.**
  - P73 held low gives Q13 off, so H is off.
  - P73 high-Z (or open-drain off) lets R63 supply ≈ (3.3 − 0.65)/330 kΩ ≈ 8.0 µA of base current. That saturates Q13 for any hFE ≥ ≈ 66; the 2SD1819A ranks are 160–460 (A-034's aggregator figure). So H is on.
  - P73 has no series base resistor. A push-pull "high" would be clamped at about 0.7 V by Q13's base–emitter junction, so P73 is most likely used as **low = off, released (input or open-drain) = on**.
- [inferred] **Boot default.** At reset the MN101C ports go high-Z, so R63 turns H on, the 8 MHz oscillator starts, and IC4 can start "in NORMAL mode" on that clock. A design in which H defaulted off would deadlock the boot. This strongly supports "H on whenever the firmware is not deliberately holding P73 low".
- [inferred] **What P73-low is for.** It is most plausibly an idle standby: IC4 drops to its 32.768 kHz sub-clock (the X2 crystal, SLOW mode), then cuts H, stopping the 8 MHz oscillator, the DSP's clock and the flash. That is the only way a unit with a continuously running clock display and no power switch keeps its standby drain in the microamp range. In that state, P73 = 0 V, Q13 base 0 V and Q13 collector ≈ 2.8 V (meter-loaded) are exactly the printed values. **Best reading:** the printed P73/Q13 values were taken with the unit idle (or are a transcription slip); they are not the recording state. Confidence medium-high.
- [inferred] **Same pattern elsewhere on the sheet** (the readings mix states):
  - Q14 (earphone follower) is printed base 2.5 V but emitter 3.1 V, impossible for a conducting NPN follower.
  - Emitter 3.1 V (= P77's plug-detect reading) is the no-plug state, in which R66's return is lifted (A-032). Base 2.5 V matches the plugged-in, conducting state at hFE ≈ 200: V_E = (3.2 − 0.65)/(1 + 33 kΩ/(470 Ω · 200)) ≈ 1.9 V, so V_B ≈ 2.5 V.
  - Q9: base 1.8 V against an emitter at 3 V would need a 1.2 V Vbe.

  The printed DC values in S-002 are therefore approximate and occasionally taken in mixed conditions. Treat any single value that contradicts the topology as suspect.

### Gaps
- No datasheet found for the 2SB1295 (PNP from the symbol), nor the MN101C16 manual (the reset and port behavior is from the MN101C51F family manual).
- Which IC4 port mode P73 uses (push-pull, open-drain or input toggling) is firmware, not printed.
- A continuity and voltmeter check on a real unit (P73 and H in record, in play, and in idle stop after some seconds) would settle this in minutes. It belongs in the "Hardware evidence" template.

## Q2. How the manual prints record vs playback voltages, and whether the analog rail (P72) and the speaker amplifier (P70) are on in playback

### Takeaway
- The notes page defines **"No mark ... Recording, ( ) .... Playback"**.
- In the whole schematic only **17 points carry a parenthesized playback value**: the DSP's host port P0–P7, IC4's D0–D7 (the same bus, "0.5V(0.3V)") and the REC LED port P71 ("0.2V (3.3V)"). **Sheet 3.1 (all the analog and power circuitry) has none.**
- So an unmarked value is a recording reading, and the playback state of the analog section is simply undocumented. It is not documented as "the same".
- From function:
  - In speaker playback **P72 must be low**: the 3.2 V rail powers the codec's DAC and the same port powers the amplifier via Q9.
  - **P70 must be low** (amplifier enabled); a CD above 2.0 V mutes the amplifier by 70 dB.
  - With an earphone inserted, P70 is held high ("Sound will not be heard from the speaker").

### Cited Findings
- **Legend**, notes page: "DC voltage measurements are taken with electronics voltmeter … No mark ... Recording, ( ) .... Playback" [read] — [S-002 notes page](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **Where parentheses appear** [read]:
  - IC4 pins 67–74 (D7–D0): "0.5V(0.3V)".
  - DSP pins 59–66 (P0–P7): "0.5V (0.3V)".
  - IC4 pin 60 P71: "0.2V (3.3V)".
  - IC4 pin 61 P72: "0.2V"; pin 59 P70: "3.3V"; pin 62 P73: "0V"; pin 63 P74: "0V"; pin 66 P77: "3.1V", all without parentheses.
  - Nothing on sheet 3.1 carries parentheses (codec, mic amp, AGC, Q20, Q9, NJM2113, Q14).

  — [S-002 sheets 3.1–3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **P72's net** [traced]: R87 10K (Q20's base) and R26 3.3K (Q9's base) share one vertical net with a junction dot at R26, continuing down to bus letter D, which reaches P72. The crop confirms the first researcher's trace — [S-002 sheet 3.1](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **Amplifier CD** [read]: NJM2113 pin 1 reads "2.1V", fed through R91 56K from bus F, which reaches P70 (3.3V). NJM2113 pin 2 (FC2) has no capacitor; pin 3 (FC1) has C13 16 V 1 µF, reading "1V"; pin 6 VDD reads 2.8 V — [S-002 sheet 3.1](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **NJM2113 datasheet**: "active mode during the CD terminal is Low level (<0.8V) and it is stand-by mode during the CD terminal is High level (>2.0V)"; CD input voltage H 2.0 V min, L 0.8 V max; CD input resistance R_CD 50 / 75 / 175 kΩ (at V_CD = 16 V); mute attenuation 70 dB at pin 1 = 2.0 V; power-down current 72 µA typ — [JRC NJM2113 datasheet](https://www.endrich.com/Datenbl%C3%A4tter/Aktive%20Komponenten/NJM2113_E.pdf)
- **MC34119 (pin-equivalent)**: "Input impedance at Pin 1 is a nominal 90 kΩ"; "If Pin 1 is open, that is equivalent to a Logic '0'"; when disabled "the voltages at FC1 and FC2 do not change … The outputs … change to a high impedance condition" — [Motorola MC34119 datasheet](https://rigpix.com/components/mc34119.pdf)
- **Owner's manual (earphone)**: "Sound will not be heard from the speaker"; troubleshooting "No sound from the speaker — Is earphone (not included) connected?" — [RR-DR60 Operating Instructions (RQT4606-1P), Internet Archive](https://archive.org/details/manuallib-id-2654684); [second scan](https://archive.org/details/Panasonic_RR-DR60_Microcassette_Recorder_User_Manual)

### Inferences
- [inferred] **The convention is "recording; playback added only where the author measured it".** If an unmarked value meant "same in both", P70 = 3.3 V would put CD at 2.1 V (> 2.0 V) in playback and mute the speaker by ≥ 70 dB. That contradicts the device's speaker playback, so the convention must be the narrower one.
- [inferred] **The R91 / CD divider is consistent with P70 high and nothing else on the pin.** 3.3 V · R_CD/(R_CD + 56 kΩ) = 2.1 V gives R_CD ≈ 98 kΩ, within the NJM2113's 50–175 kΩ and next to the MC34119's 90 kΩ nominal. Over the datasheet range CD would sit at 1.56–2.50 V. At the low end (R_CD = 50 kΩ) a unit's amplifier would be in the undefined 0.8–2.0 V band during recording. The printed 2.1 V gives only ≈ 0.1 V of margin over V_IH (a unit-variation risk; see Q8).
- [inferred] **Playback state by function** (confidence high):

  | Port / point | Record [read] | Playback, speaker [inferred] | Playback, earphone [inferred] |
  |---|---|---|---|
  | P72 → Q20 3.2 V rail + Q9 amplifier supply | 0.2 V (on) | low (on): the codec DAC needs the rail | low (on): the codec and Q14 need the rail |
  | P11 → codec PDN | 3.3 V (codec on) | high (on) | high (on) |
  | P70 → amplifier CD via R91 | 3.3 V → CD 2.1 V (standby) | low (enabled) | high (standby; the speaker is muted) |
  | P71 → REC LED | 0.2 V (lit) | 3.3 V (off) [read] | 3.3 V [read] |
  | P73 → Q13/Q10 H rail | printed 0 V (see Q1; must be on) | on | on |

### Gaps
- No printed playback voltages for any analog node. In particular, the amplifier's quiescent output, FC1 and the CD voltage in playback are inferred, not read.
- The NJM2113 CD input resistance at 3 V (the datasheet gives it at 16 V; the MC34119 shows a curve, Figure 20, not digitized here).

## Q3. Is the 3.2 V analog rail (mic amp, limiter, codec) switched on only at record start, or kept on in stop?

### Takeaway
No document states the stop-mode state. The circuit's defaults, the existence of the switch, and the standby budget point the same way:
- **At reset, and almost certainly in idle stop, the 3.2 V analog rail and the amplifier supply are off** (P72 high or high-Z).
- The codec is held in power-down (R29 1 MΩ pulls PDN low when its port floats).
- The 8 MHz/flash rail H is on at reset, and probably cut only in deep idle.

What cannot be settled is **when** the firmware turns the rail on:
- (a) at the REC/PLAY press, so each item begins with a rail power-up; or
- (b) at the first key or dial activity (a "wake" state), so the rail may already be up and settled when recording starts, and may stay up for some seconds after STOP.

This is the deciding question for whether a record-start transient exists at all (Q6). It needs a capture, or a voltmeter on Q20's collector.

### Cited Findings
- **Q20** "2SB1218ASTX / POWER SUPPLY" (PNP) [read]: emitter 3.3V, base 2.7V via R87 10K only (no base–emitter resistor, no base capacitor), collector 3.2V with C17 4 V 47 µF. **Q9** "2SB1295-6-TB" (PNP): emitter on the battery line "3V", collector 2.8V with C20 4 V 22 µF, base 1.8V via R26 3.3K — [S-002 sheet 3.1](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **R93 10 Ω** feeds both Q20's emitter and the DSP's AVDD node: C28 1 µF + C21 0.1 µF + C22 10 µF, pin 19 "AVDD" 3.3V [read/traced] — [S-002 sheets 3.1, 3.2](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **The 3.2 V rail feeds** [read/traced]:
  - codec VDD (pin 4, 3.2V);
  - the mic amplifier (Q1–Q3, R6 100K) and the mic-bias capacitance multiplier Q11 (R4 39K, C2 4 V 22 µF);
  - the limiter (Q5 collector through R8 100 Ω);
  - the earphone follower Q14 (collector 3.2V; R64 33K bias).

  — [S-002 sheet 3.1](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **The codec's reference is the record chain's reference** [read/traced]:
  - The MSM7702's SG output (pin 2, 1.6V) runs to Q6's emitter (1.6V), then to the AIN+ node (pin 20, 1.6V).
  - That node is also Q3's base in the mic amplifier (1.6V) and is loaded by R89 150K to ground.
  - SGC (pin 1, 1.5V) carries C50 0.1 µF.

  — [S-002 sheet 3.1](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **MSM7702 datasheet**:
  - PDN: "A logic '0' level drives both transmit and receive circuits to a power down state";
  - SG: "The output voltage is 1/2 of the power supply voltage … This output voltage level is undefined during power saving or power down mode";
  - SGC: "Connect a 0.1 µF capacitor … between the AG pin and the SGC pin";
  - AOUT: "During power saving or power down mode, the output of AOUT is at the voltage level of the signal ground";
  - GSX: "During power saving and power down modes, the GSX output is at AG voltage";
  - supply current 5 mA typ (9 max) operating, 0.01 mA power-down, 1.2 mA power-save (XSYNC off).

  — [MSM7702 datasheet p. 4](https://pdf.jiepei.com/msm7702-9605035.html?page=4), [p. 6](https://pdf.jiepei.com/msm7702-9605035.html?page=6), [p. 7](https://pdf.jiepei.com/msm7702-9605035.html?page=7), [p. 9](https://pdf.jiepei.com/msm7702-9605035.html?page=9)
- **Codec PDN**: IC4 pin 34 "P11" drives net J, which carries the codec's PDN (pin 8, 3.3V) and the DSP's /ACFL. R29 1M and C30 1000P run to ground from that net [read/traced, the first researcher's reading confirmed on the sheet-3.2 thumbnail] — [S-002 sheets 3.2, 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **Owner's manual**:
  - "The battery indicator is not shown while the recorder is stopped";
  - the stop display is called "the normal (off) display";
  - batteries must be replaced "within about three minutes" (the back-up cell).

  — [RR-DR60 Operating Instructions](https://archive.org/details/manuallib-id-2654684)
- **No standby battery-life figure** is published; only "Playback About 8 hours / Recording About 10 hours" [read] — [S-002 specifications](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)

### Inferences
- [inferred] **Reset/default state from the pull resistors:**
  - P72 floats, and Q20 and Q9 have no base pull-down, so they get only leakage: the analog rail and the amplifier supply are **off**.
  - P11 floats and R29 pulls PDN to 0, so the **codec is in power-down**.
  - P73 floats and R63 turns Q13 and Q10 on, so **H and the 8 MHz are on**.
  - P70 floats, and CD open equals enabled, but the amplifier has no supply.

  The designer made "analog off, codec down, clock on" the safe power-on state.
- [inferred] **The switch exists to be opened.** The codec has its own PDN (10 µA) and the amplifier its own CD (72 µA). Q20/Q9 add value only by also removing what PDN and CD cannot:
  - the mic amplifier: Q1 ≈ 104 µA plus Q3 ≈ 6 µA (A-029);
  - the capsule's drain current (≈ 0.1–0.5 mA);
  - Q11's R4 bleed (82 µA);
  - Q14's bias, R89 (≈ 11 µA);
  - the amplifier's standby current and C20/C13 leakage.

  Roughly 0.5–1 mA in all. On a pair of ≈ 1 Ah AAA cells, 1 mA alone is ≈ 6 weeks of shelf life before the converter's losses, so a recorder with an always-on clock display would cut it in stop. Confidence medium-high that the rail is **off in idle stop**.
- [inferred] **Possible intermediate "wake" state.** Many 1990s IC recorders keep the processors awake for a few seconds after a key press (the RR-DR60's stop-after-record display cycles through date, time and length before "normal display"). Whether P72 goes low only with REC/PLAY or already in that wake period is not documented. Both readings are compatible with every printed value.
- [inferred] **Rail switch-on dynamics.** Q20's base drive is ≈ (3.3 − 0.7 − 0.2)/10 kΩ ≈ 0.24–0.26 mA. At hFE 160 / 300 / 460 (the 2SB1218A's complementary ranks) Q20 can source ≈ 42 / 78 / 120 mA, charging C17 47 µF (+ ≈ 1 µF elsewhere) to 3.2 V in **≈ 3.7 / 2.0 / 1.3 ms**.
  - That in-rush flows through R93, so **the DSP's AVDD (its PLL supply) dips by ≈ 0.4 / 0.8 / 1.2 V for those 1–4 ms**; the 11.1 µF on AVDD covers only the first ≈ 0.1 ms.
  - A firmware that switches the rail while the DSP's PLL is running would disturb the PLL. That is one more reason to expect the rail to be switched before the DSP starts streaming, not mid-frame.
- [inferred] **Mode table** (the best current reading; ★ = printed):

  | Rail / enable | Idle stop | Record | Playback (speaker) | Playback (earphone) |
  |---|---|---|---|---|
  | IC8 3.3 V boost (CE = Vbat/2) | on | on ★ | on | on |
  | H (Q10: 8 MHz osc, IC5, flash) | probably off (P73 low) | on (from the 8 MHz ★, Q10 base ★) | on | on |
  | 3.2 V analog (Q20, P72) | off (default; inferred) | on ★ | on | on |
  | Amplifier supply (Q9, P72) | off | on ★ (2.8 V) | on | on |
  | Amplifier CD (P70) | (unpowered) | standby ★ (2.1 V) | enabled (CD < 0.8 V) | standby |
  | Codec PDN (P11) | power-down (R29 default) | on ★ (3.3 V) | on | on |
  | Earphone follower Q14 | unpowered | powered; conducts only with a plug in | n/a (amplifier drives the speaker) | conducts |

### Gaps
- The stop and idle states are undocumented. The deciding measurements are Q20's collector and P72 with a voltmeter, 1 s and 10 s after STOP, and while the dial is being turned in stop. Equivalently, an acoustic capture of the first second of several items (A-037's plan).
- The firmware ordering of P72, P11, P10 (DSP /RST) and the coder start, and any blanking delay: firmware, unrecoverable from the schematic.
- The original full service manual (Q5) might contain a mode table or an operation description. Not found.

## Q4. The NJM2113 CD pin: 2.1 V (standby) in record; what in playback, and is the amplifier powered in stop?

### Takeaway
- **Record:** the amplifier is **powered but disabled**:
  - V+ = 2.8 V through Q9, because P72 switches Q9 together with the analog rail;
  - CD = 2.1 V (standby, outputs high impedance, ≥ 70 dB mute);
  - FC1 (C13) is charged (1 V printed), so its bias is up.
- **Speaker playback:** CD must be low (enabled).
- **Earphone playback:** CD stays high.
- **Stop:** the amplifier is most likely **unpowered** (Q9 off with the analog rail), not merely disabled.

The amplifier is powered in record only as a side effect of sharing P72.

### Cited Findings
- NJM2113: pin 1 "2.1V", pin 3 (C13 1 µF) "1V", pin 4 "0V", pin 6 VDD "2.8V", pin 8 "0V"; pin 2 has no external capacitor [read] — [S-002 sheet 3.1](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- NJM2113: R+IN (pin 3) 100/150/220 kΩ, RREF (pin 2) 18/25/40 kΩ; output DC offset ±30 mV max (R_f = 75 kΩ); note 3 "C1 and C2 make slow power rise up to the NJM2113 regardless the external power supply condition" — [JRC NJM2113 datasheet](https://www.endrich.com/Datenbl%C3%A4tter/Aktive%20Komponenten/NJM2113_E.pdf)
- MC34119:
  - "C1 and C2 also affect the turn-on time of the circuit at power-up, since the two capacitors must charge up through the internal 50 k and 125 kΩ resistors … The turn-on time is ≈60% longer for VCC = 3.0 V" (Figure 1: turn-on time vs C1, C2 at 6 V);
  - "The turn-off time of the audio output, from the application of the CD signal, is <2.0 µs, and turn on-time is 12 ms–15 ms. Both times are independent of C1, C2, and VCC";
  - output DC level 1.0/1.15/1.25 V at VCC = 3.0 V.

  — [Motorola MC34119 datasheet](https://rigpix.com/components/mc34119.pdf)

### Inferences
- [inferred] **In record**, FC1 = 1 V on C13 shows the amplifier's internal bias is established while it is disabled. So if the user goes from a recording **directly** to playback without the rail dropping, the amplifier only needs the 12–15 ms CD turn-on.
- [inferred] **From an idle stop** (Q9 off), the amplifier must first charge C13 through ≈ 50–220 kΩ. τ = 1 µF × (100–220 kΩ) = 0.10–0.22 s at the datasheet's 6 V; ≈ 60 % longer at 3 V gives **≈ 0.16–0.35 s**. Sound cannot reach the speaker before then, so a firmware that respects it adds ≈ 0.2–0.4 s of start latency to playback from stop.
- [inferred] **Q9 off in stop is consistent**: with P72 driven to 3.3 V, Q9's base sits above its emitter (≈ 3.0 V battery) and above its collector. Both of a PNP's junctions are reverse-biased, so the amplifier supply is fully off; there is no back-feed path through R26.

### Gaps
- MC34119 Figure 1 (turn-on time for C1 = 1 µF, C2 = 0) was not digitized. The 0.16–0.35 s range is the RC estimate.
- The printed Q9 values are mutually inconsistent (Vbe 1.2 V), so the 2.8 V VDD figure is itself approximate.

## Q5. The original full service manual (Order No. AD9807107C1)

### Takeaway
**Not found.** The only RR-DR60 service document online is the 14-page supplement AD9810180A3 (S-002), mirrored on the Internet Archive and ManualsLib. Panasonic's archived service-manual CD-ROMs ("TSN Technical Service Navigation") on the Internet Archive start in June 2000. Their IC-recorder entries are later RR-QR models, so the 1998 original is unlikely to be on them.

### Cited Findings
- A web search for "AD9807107C1" returned no matching document — [web search 2026-10-17, no hit]
- ManualsLib lists one RR-DR60 service manual of 14 pages (the supplement) and the operating instructions — [ManualsLib RR-DR60 product page](https://www.manualslib.com/products/Panasonic-Rr-Dr60-619491.html)
- Internet Archive full-text search for "AD9807107C1": 0 results. An "RRDR60" search finds:
  - the S-002 item;
  - two scans of the operating instructions (`manuallib-id-2654684`, RQT4606-1P; and `Panasonic_RR-DR60_Microcassette_Recorder_User_Manual`).

  — [Internet Archive search API](https://archive.org/advancedsearch.php?q=AD9807107C1&fl%5B%5D=identifier&rows=20&output=json); [manuallib-id-2654684](https://archive.org/details/manuallib-id-2654684)
- TSN June 2000 Vol. 1 (CDR00061A0X) lists "IC Recorder: RR-QR240 (Simplified) … RR-QR80 … (Supplement)", with no RR-DR60. Other TSN volumes on the Archive are 2001–2004 — [TSN June 2000 Vol. 1, Internet Archive](https://archive.org/details/tsnsystem-technical-service-navigation-june-2000-vol.-1-cdr00061a0x)

### Inferences
- [inferred] A Panasonic "original" manual of that era would typically add a block diagram, IC pin-function tables (for the MN101C16 and MN1931712), disassembly and adjustment notes. It might add a mode table, though the supplement's own legend suggests the same record/playback-only voltage convention. Even if found, it may not settle the stop-mode question.

### Gaps
- Not checked: paid service-manual sellers, Japanese-language sources (Panasonic Japan model numbers), and Panasonic's own service portal (requires a dealer login).

## Q6. Record-start transient estimate (stage 15)

### Takeaway
- **If the rail and the codec are already up and settled** when recording starts (the "wake" reading of Q3): **no power transient**. The start effect then is A-037's question instead: whether the limiter's C6 still holds charge from a recent loud item.
- **If the 3.2 V rail and the codec's PDN are switched at REC**, the transient has three parts:

  1. **The microphone bias fade-in** (Q11, R4 39 kΩ × C2 22 µF, τ = 0.858 s). This one is certain if the rail is switched.
     - **Nothing from the capsule for ≈ 0.2–0.25 s**: its hiss is also absent, leaving only the amplifier's own floor of about −73 dBFS (A-050).
     - **Then reduced sensitivity until ≈ 0.5–0.8 s.**
     - The bias edge itself is inaudible: −35 to −47 dBFS at the codec input amplifier, below −70 dBFS in band.
  2. **A codec reference (SG) step through the 76 Hz coupling.** Size depends on how far the coupling capacitors are from their final charge and on the SG rise time (unknown, bounded 0.5–200 ms).
     - At GSX: from **−46 dBFS (benign case) up to +14 dBFS (worst case, clipped by the codec input amplifier)**.
     - In band, after the codec's band-pass: **−61 to +4 dBFS**.
     - It decays below −62 dBFS in band within **≈ 10–26 ms**.
  3. **The limiter.** Only in the worst cases (a ΔV of order 1.6 V, the SG rising in ≤ 2 ms) is the knee exceeded for longer than the dead time. Then C6 charges and **the first ≈ 1.4 s of the recording is ducked** (A-035's release).

- **Firmware can blank part 2 by discarding the first one or two frames** (≈ 20–40 ms at the project's 20 ms frame, A-040), or by releasing PDN ≥ ≈ 50–100 ms before the coder starts.
- Firmware cannot blank part 1 without losing up to ≈ 0.8 s of audio, and cannot undo a limiter charge.

### Cited Findings
- **Mic amplifier and codec input topology** [read]:
  - The capsule is fed from Q11's emitter (2.7V) through R2 1K.
  - C1 0.15 µF couples it into Q1's base (1.7V); R3 100K is the feedback from the output node (1.7V; Q1/Q2 collectors).
  - Q3's base is the reference (1.6V, the SG/AIN+ node).
  - The output goes through C4 0.1 µF, R7 22K, C5 1 µF and R10 1K into the codec's AIN− (pin 19, 1.6V).
  - R15 82K ∥ C10 150P runs from AIN− to GSX (pin 18, 1.6V).
  - Q11: base 3.2V at R4 39K / C2 22 µF, collector 3.2V.

  — [S-002 sheet 3.1](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **Limiter** [read]: Q6's emitter on the SG line, R12 390 Ω, C6 1 µF, R11 10M, Q5/Q4 shunt at the R7 node — [S-002 sheet 3.1](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **MSM7702**:
  - SG = VDD/2, "undefined during power saving or power down";
  - the SGC bypass is recommended at 0.1 µF (the RR-DR60's C50 is 0.1 µF [read]);
  - GSX at AG voltage in power-down.

  No SG start-up time is given on pages 4–9 — [MSM7702 datasheet p. 4, 7](https://pdf.jiepei.com/msm7702-9605035.html?page=7)
- **Project figures used**:
  - full scale 0.7 V peak at GSX = 0.196 V at the mic-amp output (A-029, A-030);
  - output coupling 76–77 Hz = (C4 in series with C5) into R7 + R10 = 23 kΩ, τ = 0.0909 µF × 23 kΩ = 2.09 ms (A-047);
  - gain R15/(R7 + R10) = 3.57;
  - limiter knee −1 dBFS, dead time ≈ 3 ms at 10 dB over and ≈ 20–25 ms at 2 dB over, release ≈ 1.4 s, half-wave detector (A-028, A-034, A-035, A-036).

  — [assumptions.md](../../../assumptions.md)
- **Earlier estimate** (power researcher): bias-ramp residue ≈ −65 dBFS; SG-step residue ≈ −9 dBFS for a 50 ms SG ramp; capsule under-biased for ≈ 0.56 s — [power_supply_effects.md §6](power_supply_effects.md)

### Inferences
**(a) The mic bias fade-in** [inferred; the time constants from printed values]:
- Q11's base follows 3.2 V·(1 − e^(−t/0.858 s)). Q11 conducts once its base passes ≈ 0.55–0.6 V, at **t ≈ 0.16–0.18 s**. The capsule (bias = Q11's emitter ≈ base − 0.6 V, less I_D·R2 ≈ 0.3 V) then has, by the computed timeline:

  | t (s) | 0.25 | 0.40 | 0.50 | 0.60 | 0.80 | 1.0 | 2.0 | 4.0 |
  |---|---|---|---|---|---|---|---|---|
  | Q11 base (V) | 0.81 | 1.19 | 1.41 | 1.61 | 1.94 | 2.20 | 2.89 | 3.17 |
  | capsule V_DS (V) | ≈ 0 | 0.29 | 0.51 | 0.71 | 1.04 | 1.30 | 1.99 | 2.27 |

- An electret's JFET is saturated for V_DS above about its pinch-off magnitude (assumed 0.3–0.8 V). Below that, in the triode region, its transconductance and so the capsule's sensitivity scale roughly with V_DS.
- **Shape:** silence (no capsule hiss) until ≈ 0.2–0.25 s, then a fade-in reaching full sensitivity by **≈ 0.5–0.8 s**. Assumption: sensitivity ∝ min(1, V_DS/|V_P|), |V_P| 0.3–0.8 V; roughly −5 dB at 0.4 s for |V_P| = 0.5 V.
- The bias edge itself (the C1 current C1·dV/dt ≈ 0.15 µF × 3 V/s ≈ 0.45 µA into R3) gives ≈ 40–44 mV at the mic-amp output. Its onset is softened by Q11's turn-on: modeled as a softplus of 20–80 mV.
  - Through the 76 Hz coupling it reads **−35 to −47 dBFS at GSX**, near t ≈ 0.16 s.
  - After the emulator's record band-pass it is **below −72 dBFS**. Inaudible; no limiter action.
  - The power researcher's −65 dBFS was for the slow tail; the onset is larger at GSX but has no in-band content.

**(b) The codec-reference step** [inferred; ΔV_eff and τ_SG are bounded, not known].
- SG drives AIN+ and Q3's base. So the mic amplifier's whole operating point (output ≈ SG + 0.1 V) and the codec input amplifier's virtual ground ride on SG; to first order they cancel as common mode.
- What reaches GSX is the change in the voltage across the coupling capacitors C4/C5 (an effective step ΔV_eff), high-passed with τ 2.09 ms and multiplied by 3.57, with a rise time of order the SG ramp τ_SG.
- Bounds:
  - **ΔV_eff ≈ 0.1 V** if everything tracks SG (only the amplifier's 0.1 V offset establishes);
  - **≈ 0.4 V** if the mic amplifier sits near a Vbe (≈ 0.5 V) before SG rises (Q3 off, Q2 off);
  - **up to ≈ 1.6 V** if the coupling capacitors charged against a powered-down codec (GSX at ground through R15) before PDN released.
- τ_SG: C50 0.1 µF times an internal SG source resistance that is not published, so 0.5–200 ms is a bound, not a value.
- Computed (GSX in dBFS re 0.7 V; "in band" after `RECORD_SOS` at 6 kHz; settle times from the start of the SG ramp):

  | ΔV_eff | τ_SG | GSX peak | above the −1 dBFS knee | in-band peak | in band < −40 dBFS after | < −62 dBFS after |
  |---|---|---|---|---|---|---|
  | 0.1 V | 0.5 ms | −9.8 dBFS | 0 | −15.0 dBFS | 13 ms | 19 ms |
  | 0.1 V | 10 ms | −23.1 | 0 | −35.8 | 5 ms | 12 ms |
  | 0.1 V | 200 ms | −45.9 | 0 | −61.4 | 0 | 1 ms |
  | 0.4 V | 0.5 ms | +2.3 | 1.9 ms | −3.0 | 14 ms | 23 ms |
  | 0.4 V | 2 ms | −2.3 | 0 | −11.4 | 15 ms | 20 ms |
  | 0.4 V | 10 ms | −11.0 | 0 | −23.7 | 11 ms | 17 ms |
  | 0.4 V | 50 ms | −22.6 | 0 | −37.4 | 5 ms | 12 ms |
  | 1.6 V | 0.5 ms | +14.3 (clipped near +6.6 by the input amplifier's rails) | 5.1 ms | +4.2 | 19 ms | 25 ms |
  | 1.6 V | 2 ms | +9.7 | 6.9 ms | +0.5 | 17 ms | 26 ms |
  | 1.6 V | 10 ms | +1.0 | 6.6 ms | −11.7 | 12 ms | 26 ms |
  | 1.6 V | 50 ms | −10.6 | 0 | −25.4 | 11 ms | 16 ms |
  | 1.6 V | 200 ms | −21.8 | 0 | −37.3 | 5 ms | 12 ms |

- **Shape:** a single low-frequency "thump", one polarity then a smaller overshoot, lasting ≈ 10–25 ms in band. Its sign is unknown (it depends on which node leads).
- **Amplitude:** benign (−35 to −61 dBFS in band) for slow references or well-tracked bias; a full-scale click for a fast reference with badly pre-charged coupling capacitors.

**(c) The limiter** [inferred]:
- Exceeding the knee alone is not enough: the excursion must outlast the dead time on the detector's polarity (A-034, A-036).
- In the table, the knee is exceeded for 5–7 ms only in the ΔV_eff ≈ 1.6 V cases with τ_SG ≤ 10 ms.
  - At +9.7 to +14 dBFS (≈ 11–15 dB over the knee) the dead time is ≈ 3 ms, so **C6 would charge**, and the recording would start ≈ 10 dB ducked, recovering over ≈ 1.4 s (A-035).
  - At +1 dBFS (2 dB over) the dead time is 20–25 ms, longer than the 6.6 ms excursion, so no trigger.
  - The 0.4 V / 0.5 ms case (1.9 ms over, 3.3 dB over the knee) is also below its dead time.
- Each of these triggers only if the excursion has the detector's polarity, so roughly half of the trigger-capable cases.
- With the rail off in stop, C6 starts empty (it discharges through R11 10 MΩ and Q5's junction, τ ≈ 10 s; A-037).

**(d) What else rides on the rail edge** [inferred]:
- The 1–4 ms in-rush dips the DSP's AVDD by 0.4–1.2 V (Q3). This is harmless if the coder has not started.
- If the operation tone is enabled (off by default; S-002 addendum), a beep through the speaker at the REC press could be picked up acoustically. That is not a power transient.

**(e) Firmware blanking** [inferred]:
- The in-band SG thump has decayed below the capsule hiss (−62 dBFS) within ≈ 26 ms in every case. Discarding the first **2 frames** (≈ 40 ms at the project's assumed 20 ms framing, A-040) or delaying the coder ≥ ≈ 50 ms after PDN would remove it.
- The bias fade-in (0.5–0.8 s) is too long to blank without an obvious REC latency.
- A designer would more likely power the analog rail early, on the first key or dial activity, or simply accept a short fade-in.
- The service manual gives no hint; the owner's manual describes no REC delay.

**(f) For the emulator** (a proposal; not a decision):
- A stage-15 "record-start" option could apply a gain envelope g(t): 0 until t₀ ≈ 0.2 s, rising to 1 by ≈ 0.6–0.8 s, applied to stage 1–2's output (capsule signal and hiss together; the amplifier's own floor stays).
- Optionally add a single thump: an exponential-rise step through the 2.09 ms high-pass ×3.57, injected at the limiter's input, with ΔV_eff and τ_SG as parameters, default in the benign region (e.g. ΔV_eff 0.1 V, τ_SG 10 ms → −23 dBFS at GSX, −36 dBFS in band).
- Both off by default until A-037's capture shows them.

### Gaps
- The MSM7702's SG start-up time and its source impedance (pages 10–17 were not checked for a power-up figure; pages 4–9 have none).
- The electret's pinch-off and I_D (no capsule part number in S-002), so the fade-in's exact curve.
- The firmware order and timing of P72, P11, the DSP reset and the coder start.
- The initial charge on C4/C5 in the off state, which sets ΔV_eff: when the rail is off, every node bleeds to 0 V, but the order in which the mic amplifier and the codec come up is not printed.
- A capture settles all of it:
  - record silence (capsule covered, or a quiet room) at least 30 s after the last activity, and inspect the first 1 s of several items for the onset of hiss, any thump, and the level of a steady tone started before REC;
  - then repeat with REC pressed immediately after STOP.

## Q7. Playback-start transient estimate

### Takeaway
- **From an idle stop** (rail, amplifier supply and codec off), playback start needs three things to settle:
  1. the 3.2 V rail (1–4 ms);
  2. the codec's SG/AOUT reference (0 → 1.6 V with the unknown τ_SG);
  3. the amplifier's internal bias (C13, ≈ 0.16–0.35 s).
- **If the firmware enables the amplifier (CD low) before AOUT settles**, the AOUT step through C14/VR1 (32 Hz) and C8/R16 (159 Hz) reaches the speaker as a click:
  - **−3 to −20 dB re the route's full scale** for τ_SG 0.5–10 ms, **−34 to −46 dB** for 50–200 ms;
  - it scales with the volume position like the signal.
- The CD turn-on itself (12–15 ms, "independent of C1, C2 and VCC") hides the first part. A click released together with the SG ramp is ≈ **−29 to −34 dB re full scale** after 15 ms for τ_SG ≤ 10 ms.
- The amplifier's own power-up adds at most a ≤ 30 mV sub-audio differential offset (≤ −32 dB re the 1.15 V clip ceiling), and the small speaker barely reproduces it.
- **On the earphone route** (amplifier disabled, the follower Q14 coupled straight through C61), a rail power-up with the plug inserted gives a **volume-independent ≈ 0.1 V, few-ms thump in the earphone**, of the order of the route's full-volume level.

### Cited Findings
- Playback path [read]: codec AOUT (pin 3, 1.6V) → C14 0.1 µF → VR1 50K(B) → wiper → C8 0.1 µF → R16 10K → NJM2113 pin 4, with R18 68K feedback. The node also feeds C60 0.1 µF → R65 10K → Q14's base (2.5V) → C61 4 V 10 µF → jack; R66 470 Ω is the emitter's return through the jack's plug-detect contact — [S-002 sheet 3.1](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN); [A-031, A-032](../../../assumptions.md)
- MSM7702 AOUT: "maximum amplitude of 2.0 VPP above and below the signal ground voltage (VDD/2)", and at the SG level in power-down. MSM7702 SG: undefined in power-down — [MSM7702 datasheet p. 4](https://pdf.jiepei.com/msm7702-9605035.html?page=4), [p. 7](https://pdf.jiepei.com/msm7702-9605035.html?page=7)
- MC34119 CD turn-on 12–15 ms; outputs high impedance when disabled; output DC offset ±30 mV (NJM2113) — [MC34119](https://rigpix.com/components/mc34119.pdf); [NJM2113](https://www.endrich.com/Datenbl%C3%A4tter/Aktive%20Komponenten/NJM2113_E.pdf)
- Project figures: receive full scale 1.0 V peak at AOUT (A-030); the stage-11 corners 32 Hz and 159 Hz; clip ceiling 1.15 V; the earphone load's swing is −15 … −24 dB below the codec output at full volume, with the follower's output resistance ≈ 40–130 Ω (A-031, A-032) — [assumptions.md](../../../assumptions.md)

### Inferences
**AOUT step at the speaker** [inferred]:
- Computed for AOUT rising 0 → 1.6 V (1.6 × full scale) with τ_SG, through τ = 5 ms (C14·VR1) and then τ = 1 ms (C8·R16):

  | τ_SG | peak (dB re full scale) | worst value after 15 ms | below −60 dB after |
  |---|---|---|---|
  | 0.5 ms | −2.7 | −33.1 | 31 ms |
  | 2 ms | −9.4 | −30.0 | 33 ms |
  | 5 ms | −15.4 | −29.2 | 39 ms |
  | 10 ms | −20.5 | −34.1 | 52 ms |
  | 50 ms | −33.6 | −54.4 | 64 ms |
  | 200 ms | −45.5 | −69.9 | 11 ms |

- **Shape:** a single low-frequency pulse whose main energy is below ≈ 150 Hz. Through the speaker template (resonance 750 Hz, A-010) it is further attenuated: a soft "tock" rather than a sharp click.
- It needs the amplifier enabled while AOUT moves. Firmware that waits ≈ 60 ms after PDN before releasing CD removes it entirely. The C13 charge (0.16–0.35 s) already forces such a wait from an idle stop, unless the firmware ignores it.

**Amplifier power-up** [inferred]:
- From Q9 off, FC1 ramps the outputs in common mode over ≈ 0.16–0.35 s. The BTL speaker sees only the mismatch: at most the ±30 mV output-offset spec, appearing as the bias comes up.
- 30 mV / 1.15 V = −32 dB re the clip ceiling, over a few hundred ms: effectively inaudible on a 2.8 cm speaker.
- If CD is released after the bias settles, the 12–15 ms turn-on brings in the same ≤ 30 mV offset: a ≤ −32 dB, ≈ 12 ms step, a faint tick at most.

**Earphone rail power-up** [inferred; low confidence]:
- With the plug in, Q14's emitter rises to ≈ 1.8–2.1 V as its base charges through R64 33 kΩ into C60 (τ ≈ 3.3 ms) and the rail comes up.
- The current through C61 into the 16 Ω earphone ≈ C61·dV/dt ≈ 10 µF × 2 V / 3.3 ms ≈ 6 mA, so ≈ 0.1 V across the earphone, decaying with τ ≈ 3 ms. The C61 path's own τ is 10 µF × (16 + 40…130 Ω) ≈ 0.6–1.5 ms, short enough for the quasi-static estimate.
- The earphone's full-volume swing is ≈ 0.06–0.18 V peak (−15 … −24 dB re the 1.0 V codec full scale, A-032). So the thump is **of the order of the earphone route's full-volume level (≈ −5 … +4 dB), independent of the volume knob**.
- It happens whenever the analog rail powers up with an earphone inserted: playback from idle stop, and also a recording start if the rail is switched at REC.
- A plug inserted while the rail is up gives its own click (C61 and the detect contact), outside stage 15.

### Gaps
- τ_SG (as in Q6) and whether AOUT rises with SG or is held by the receive section in power-down: the datasheet says AOUT is "at the voltage level of the signal ground" in power-down, but SG itself is "undefined".
- The firmware order of PDN and CD, and the playback start latency (measurable by stopwatch-free means: a capture of a recorded click's playback delay from the PLAY press).
- The earphone figures depend on Q14's bias network, whose printed values are mixed-state (Q1). A capture into a 16 Ω load would settle it.

## Q8. Does the speaker or earphone carry any signal while recording?

### Takeaway
- **Speaker: no, by design.** During recording the NJM2113 is powered but held in standby (CD 2.1 V > 2.0 V): outputs high-impedance, ≥ 70 dB mute.
  - The one caveat is the thin margin: 0.1 V over V_IH. A unit whose CD input resistance sits at the datasheet minimum would put CD at ≈ 1.6 V, inside the undefined band; low confidence that this matters in practice.
- **Earphone: the hardware path exists.**
  - The follower Q14 is on the powered 3.2 V rail, and the codec's receive section is active in record: PDN is common to both directions, PCMIN is toggling with a 0.6 V average, and AOUT sits at 1.6 V.
  - With a plug inserted, Q14 conducts and would pass whatever the DSP writes to the codec's receive input, times the volume.
  - Whether the firmware sends a monitor signal, silence or nothing is unknown. No user documentation mentions monitoring while recording.
  - With no plug, Q14 is cut off (its emitter is pulled to 3.1 V via R66 by the plug-detect pull-up), so nothing reaches the jack.

### Cited Findings
- NJM2113 pin 1 "2.1V" and VDD "2.8V" in recording [read]; CD > 2.0 V = standby with 70 dB mute; R_CD 50–175 kΩ — [S-002 sheet 3.1](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN); [NJM2113](https://www.endrich.com/Datenbl%C3%A4tter/Aktive%20Komponenten/NJM2113_E.pdf)
- Codec in recording [read]: PDN (pin 8) 3.3V; PCM IN (pin 10) "0.6V"; R SYNC (pin 9) / X SYNC (pin 12) 0V average with the "T≒170us" pulse annotation; BCLK (pin 13) 1.7V; AOUT (pin 3) 1.6V — [S-002 sheets 3.1, 3.2](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- MSM7702 PDN acts on "both transmit and receive circuits" — [MSM7702 datasheet p. 6](https://pdf.jiepei.com/msm7702-9605035.html?page=6)
- Q14 [read]: collector 3.2V (rail), base 2.5V, emitter 3.1V; R66 470 Ω from the emitter to jack pin 1 (the plug-detect contact, which also reaches IC4 P77 at 3.1V) — [S-002 sheets 3.1, 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN); A-032 records that "R66 470 Ω is the emitter's DC return to the jack's plug-detect contact, which the plug grounds" — [assumptions.md](../../../assumptions.md)
- Owner's manual: no record-monitoring feature is described. The earphone section concerns listening; troubleshooting lists no record-monitor item — [RR-DR60 Operating Instructions](https://archive.org/details/manuallib-id-2654684)
- Beep: IC4 P12 → R19 220K / C16 10P → the amplifier's input node (shared with the earphone branch); off at shipment — [S-002 sheet 3.1 and addendum](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)

### Inferences
- [inferred] **PCMIN at a 0.6 V average** (≈ 18 % of 3.3 V) shows the DSP is clocking data into the receive section during recording. With 8 data bits at the ≈ 2 µs BCLK in a ≈ 170 µs frame (≈ 9 % of the frame), that is consistent with idle or low-level codes in about two slots, or a busy line with mostly-zero bits. It shows activity, not content.
- [inferred] **Earphone during recording**: the follower's input is the amplifier's input node, which is not a virtual ground while the amplifier is in standby. So the receive output reaches the earphone at the volume setting. If the firmware wrote the input back to the receive path (a monitor), the earphone would carry it, delayed by at least the coder's look-ahead if it were the decoded signal. If it writes idle codes, the earphone carries the codec's idle-channel noise (≈ −76 dBm0p receive, A-030) times the volume, i.e. effectively silence.
- [inferred] **No acoustic feedback path through the speaker** while recording, short of the CD-margin caveat. That matters for the emulator: no speaker-to-mic feedback model is needed in stage 15.
- [inferred] **The beep shares the node**: an enabled operation tone at a key press during recording would reach a plugged-in earphone, but not the speaker (in standby), and not the recording (it bypasses the codec's transmit path).

### Gaps
- What the DSP writes to PCMIN during recording. Settled by recording with an earphone plugged in and listening, or by a capture at the jack into 16 Ω.
- The CD input resistance of real NJM2113 parts at 3 V, i.e. how many units, if any, sit below V_IH in recording.
