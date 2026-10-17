# Service manual (S-002) power and clock circuitry: a component-level reading for stages 14 (clock) and 15 (power)

Source for everything marked **[read]**: Panasonic RR-DR60 Service Manual, Order No. AD9810180A3 (© 1998 Matsushita), the 14 page images of the Internet Archive item [`manual_RRDR60_SM_PANASONIC_EN`](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN) (file `RRDR60_SM_PANASONIC_EN_jp2.zip`, 600 dpi, read at full resolution with crops on 2026-10-17; viewer: [ManualsLib](https://www.manualslib.com/manual/2950781/Panasonic-Rr-Dr60.html)). Page references are the scan image index (0–13): image 1 is the specifications, images 3–6 the parts list (printed pages 4–7), image 8 is schematic sheet 3.1 (analog and power), image 9 sheet 3.2 (DSP IC2), image 10 sheet 3.3 (microcontroller IC4, flash IC7, oscillator IC3, IC5, IC6), image 11 the "operation tone" addendum, image 12 the schematic notes, images 7 and 13 the PCB (sides B and A).

Conventions: **[read]** = printed on the sheet (value, designator, voltage, connection). **[traced]** = a connection followed across a crossing, a bracket bus or a sheet edge; I checked it, but it is my reading of the line work. **[inferred]** = my engineering inference, not printed. The sheet note (image 12) says: "DC voltage measurements are taken with electronics voltmeter … No mark ... Recording, ( ) .... Playback" [read]. A value with no parentheses therefore gives the recording-mode reading only; it is not clear whether it also means "same in playback".

Read-only constraint respected: nothing from the manual is copied into the repository except short extracts.

## Q1. Power tree: input, IC8 boost converter, rails and what they feed, Q20, Q9, Q11, IC6, back-up cell, battery sensing, measured DC voltages

### Takeaway
The 2× AAA input feeds four things directly from the battery terminal, before the series diode D5: Q9 (the NJM2113 supply), the R85/R86 divider (IC8 CE and the back-up switch's sense), and R40 into a microcontroller A/D input (battery sensing). IC8 XC6372D331PR is the **external-transistor (EXT) type**. It drives Q8 2SD1119 through R20 1 kΩ ∥ C19 3300 pF, with L1, the Schottky D2 and 2 × 150 µF output capacitors. Its CE pin is held at half the battery voltage, so it runs whenever batteries are fitted; no microcontroller pin switches it. From its 3.3 V output, the microcontroller switches three loads:
- Q20 makes the 3.2 V analog rail.
- Q9, on the same control line as Q20, powers the power amp.
- Q10 makes a switched 3.3 V "H" rail for the flash, the 8 MHz oscillator IC3 and the buffer IC5.

A rechargeable-looking lithium back-up cell is charged from the 3.3 V rail through D6 and R81 180 Ω. A latch (Q16–Q18) switches the cell onto the 3.3 V rail when the main battery is removed.

### Cited Findings
**Input and IC8 (sheet 3.1, image 8)** [read]
- Battery J1 (+) / J2 (−), "BATTERIES,3V (TWO R03/LR03, AAA,UM-4)". In series: D5 MA735TX (anode on J1), then C7 0.1 µF and C9 "4V10" (4 V 10 µF) to ground, then L1, then the switch node, which is Q8's collector, marked "3.3V". Then D2 MA735TX (anode on the switch node), then the 3.3 V output node — [S-002 sheet 3.1](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- IC8 is labelled "XC6372D331PR / DC-DC CONVERTER". Pins: 1 NC; 2 V OUT "3.3V", on the output node after D2; 3 CE "1.4V"; 4 EXT "0V"; 5 Vss — [S-002 sheet 3.1](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- EXT drives Q8's base (marked 0V) through R20 1K in parallel with C19 3300P. Q8 is labelled "2SD1119RTX / SWITCHING" with its emitter grounded — [S-002 sheet 3.1](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- On the 3.3 V output: C24 "4V150" and C81 "4V150" (parts list: C24 RCSX0GC157RE 4 V 150 µF; C81 RCSX0GC157RE 4 V 150 µF), labelled "<3.3V>" — [S-002 sheet 3.1 and parts list p.4–5](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)

**IC8 CE wiring** [read/traced]
- CE (pin 3) connects through a long line to the node of R85 270K (from the battery line, before D5) and R86 270K (to ground), with C69 0.1 µF to ground. The same node drives Q18's gate, marked "1.4V". So CE ≈ Vbat/2. No microcontroller pin reaches CE — [S-002 sheet 3.1](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)

**Marking decode (secondary)**
- "XC6372": Torex PWM/PFM-switchable step-up DC/DC family; oscillation frequency options 50/100/180 kHz (±15 %). A search summary of the Torex datasheet gives the frequency digit as 0 = 50 kHz, 1 = 100 kHz, 2 = 180 kHz. I could not open the datasheet itself (HTTP 403 from torexsemi.com and mouser.com) — [Torex XC6372 product page](https://product.torexsemi.com/en/series/xc6372)
- The sheet itself shows that the D-type package has the pins NC / VOUT / CE / EXT / Vss (5 pins; the outline on image 12 is a 5-lead SOT-89) [read]. "33" matches the 3.3 V marked on the output [read].

**Rails and loads** [read/traced]
- **3.3 V main rail** (IC8 output, always on with batteries fitted). It feeds:
  - DSP IC2 VDD (pins 18, 26, 50, 67, 88, 97 at 3.3 V; decoupling C25, C27, C29, C32 0.1 µF).
  - Microcontroller IC4 VDD (pin 8) and VREF+ (pin 24, 3.3 V).
  - IC6 IN.
  - The REC LED D1 anode (cathode through R1 1.5K to IC4).
  - R93 10 Ω → Q20 emitter / DSP AVDD.
  - Q10 emitter, and the back-up circuit.

  — [S-002 sheets 3.1–3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **Q20 "2SB1218ASTX / POWER SUPPLY"** (PNP): emitter 3.3V (fed from the 3.3 V rail through R93 10 Ω), collector 3.2V (the "<3.2V>" analog rail, C17 "4V47" = 4 V 47 µF), base 2.7V through R87 10K. [read]
  - The 3.2 V rail feeds codec IC1 VDD (pin 4) and its NC pins 5–6, all marked 3.2V. [read]
  - It also feeds the mic amp (Q1–Q3, R6), the AGC (Q5 collector via R8 100 Ω), the mic-bias filter Q11 and the earphone buffer Q14 (R64 33K to its base, collector 3.2V). [read]
- **Q20's base resistor R87 10K** runs to the bus letter D on sheet 3.1. D carries IC4 pin 61 "P72", which reads **0.2V**. The same net also feeds R26 3.3K → **Q9 base** [traced: R87's line runs straight down to the junction with R26 and on to letter D]. So **one microcontroller port (P72) switches both the 3.2 V analog rail and the power-amp supply** — [S-002 sheets 3.1 and 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **Q9 "2SB1295-6-TB / POWER SUPPLY"** (PNP): emitter "3V" on the raw battery line (taken at J1, before D5), collector "2.8V" = NJM2113 pin 6 VDD with C20 "4V22", base "1.8V" through R26 3.3K from P72 — [S-002 sheet 3.1](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **Q10 "2SB1295-6-TB / POWER SUPPLY"** (PNP): emitter 3.3V on the 3.3 V rail, collector 3.3V → thick line "H", base 2.6V via R27 4.7K from the collector of **Q13 "2SD1819ASTX / POWER SUPPLY CONTROL"** (collector 2.8V, base 0V). Q13's base has R63 330K from the 3.3 V rail and goes to bus letter C → IC4 pin 62 "P73", **0V** — [S-002 sheets 3.1 and 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **"H" rail destinations** [traced on sheet 3.3]: H runs to C40 "4V22" + C41 0.1 µF, then to flash IC7 VDD (pin 1). From there it follows the flash's tie-high bus left and down to **IC5 TC7W126 Vcc (pin 8)** and up to **IC3 TC7WU04 Vcc (pin 8)**, which has C63 0.1 µF. So the 8 MHz oscillator, the clock buffer and the flash all sit on the Q10-switched rail. The sheet labels this line "<3.3V>" in several places — [S-002 sheet 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **Q11 "2SD1819ASTX / MIC AMP"** (NPN), the mic-bias filter: collector 3.2V on the analog rail; base 3.2V at the junction of R4 39K (from the 3.2 V rail) and C2 "4V22" (to ground); emitter **2.7V** → R2 1K → microphone. [read] This is an emitter-follower ripple filter, a capacitance multiplier [inferred from topology] — [S-002 sheet 3.1](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)

**IC6 reset detector**
- **IC6 "XC61AN2002MR / RESET"** (parts list: XC61AN2002MR): IN (pin 1) on the 3.3 V rail, marked 3.3V; OUT (pin 2) marked 3.3V, with C49 0.1 µF to ground, → **IC4 pin 32 "/RST"** (3.3V); GND pin 3. No pull-up resistor is drawn on OUT. [read/traced] — [S-002 sheet 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- Marking decode by analogy with the current Torex XC61C naming: XC61C**N2002MR** is sold as an "N-Channel Open Drain Output, 2.0Vdetect … SOT-23" part. So "N" = N-channel open drain, "20" = 2.0 V detect, "2" = ±2 %, "MR" = SOT-23 on tape. I found no XC61A datasheet to confirm that the older A-series uses the same code — [Farnell XC61CN2002MR listing](https://uk.farnell.com/torex/xc61cn2002mr/volt-detector-nch-2-0v-sot-23/dp/3605486)

**Back-up cell and switch** (sheet 3.1, image 8) [read]
- "BACKUP BATTERY 3V": (+) runs to Q16's emitter (marked 3.3V) and through **R81 180 Ω and D6 MA729TX** to the 3.3 V rail node at Q10's emitter. D6's cathode faces the cell, its anode faces the 3.3 V rail [read: symbol orientation].
- **Q16 2SB1295-6-TB** (PNP): emitter on the cell; collector (3.3V) on the 3.3 V rail; base (3.1V) through R83 10K to Q17's drain (3V, with C77 1 µF to ground), and R82 390K from base to emitter.
- **Q17 2SK1958T1** (N-MOSFET, "SWITCHING (BACK UP)"): gate 0V, pulled up by R84 390K from the 3.3 V rail and shorted to ground by **Q18 2SK1958T1** (drain 0V). Q18's gate (1.4V) is the R85/R86/C69 battery divider.
- **Q17's source runs to bus letter B → IC4 pin 63 "P74", 0V** [traced].

  — [S-002 sheet 3.1](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- Specification (image 1): "Memory Back-up; DC3V (Built-in Lithium battery) About 3 minutes" [read] — [S-002 specifications](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- Operation-tone addendum (image 11): "The tone may be turned off if the unit is left without battery power for a time." [read] — [S-002 addendum](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- PCB side A (image 13): "BACKUP BATTERY 3V" hangs on flying leads, RED (+) and BLUE (−), at location 4A. **The parts list has no entry for the cell**; it lists only BATT.TERMINAL(+) RJC40015, BATT.TERMINAL(−) RJC80017 and BATT.TERMINAL RJC94013-2 [read] — [S-002 PCB and parts list](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)

**Battery-voltage sensing** [read/traced]
- R40 100K runs from the raw battery line (sheet 3.2, a thick "positive voltage line" from sheet 3.1's battery line) to bus letter D → **IC4 pin 19 "AN3", 2.8V**, with **C37 0.01 µF** to ground at the pin. There is **no divider**: the A/D reference is VREF+ = 3.3 V (pin 24) and VREF− = 0 V (pin 15) — [S-002 sheets 3.2, 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- The cover photo's LCD shows a battery-level icon [read, image 0] — [S-002 cover](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)

**Measured DC voltages (recording; playback in parentheses where printed)** [read]
| Point | Value |
|---|---|
| IC8 VOUT / CE / EXT | 3.3 V / 1.4 V / 0 V |
| Q8 collector / base | 3.3 V / 0 V |
| Q20 emitter / base / collector | 3.3 V / 2.7 V / 3.2 V |
| Codec VDD (pins 4–6) | 3.2 V |
| Q11 collector / base / emitter | 3.2 V / 3.2 V / 2.7 V |
| Q9 emitter / base / collector | 3 V / 1.8 V / 2.8 V |
| NJM2113 pin 6 VDD / pin 1 CD | 2.8 V / 2.1 V |
| Q10 emitter / base / collector | 3.3 V / 2.6 V / 3.3 V |
| Q13 collector / base | 2.8 V / 0 V |
| Q16 emitter / base / collector | 3.3 V / 3.1 V / 3.3 V |
| Q17 gate / drain / source | 0 V / 3 V / 0 V |
| Q18 gate / drain | 1.4 V / 0 V |
| IC4 P72 (pin 61) | 0.2 V |
| IC4 P73 (pin 62) | 0 V |
| IC4 P74 (pin 63) | 0 V |
| IC4 P71 (pin 60, REC LED) | 0.2 V (3.3 V) |
| IC4 P70 (pin 59, amp CD) | 3.3 V |
| IC4 P77 (pin 66, earphone detect) | 3.1 V |
| IC4 AN3 (battery) | 2.8 V |
| IC4 /RST | 3.3 V |
| IC6 IN / OUT | 3.3 V / 3.3 V |
| DSP VDD / AVDD | 3.3 V / 3.3 V |
| DSP VCO IN / PC OUT | 0.8 V / 0.8 V |
| DSP OSCF0 | 1.6 V |
| IC3 pins 1, 2, 6, 7, 8 | 1.6, 1.6, 1.7, 1.7, 3.3 V |
| IC4 OSC1 | 1.6 V |
| IC4 XI / XO | 1.3 V / 1.7 V |
| Codec PDN | 3.3 V |

— [S-002 sheets 3.1–3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- Battery life (specifications): Panasonic alkaline, "Playback; About 8 hours", "Recording; About 10 hours" [read] — [S-002 specifications](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)

### Inferences
- [inferred] IC8 is not switched by the firmware. Its CE follows the batteries: about 1.4–1.5 V with fresh cells, 0 V with them removed. So the boost converter, and the switching-converter noise it brings, runs in stop, record and playback alike. The firmware power control sits downstream, on Q20/Q9 (P72), Q10 (P73) and the codec PDN (P11).
- [inferred] The 3.2 V analog rail is the 3.3 V converter output minus R93 (10 Ω) and Q20's saturation drop. It is **not** a separately regulated rail, so converter ripple reaches the analog section attenuated only by R93 with the DSP AVDD capacitors, then C17 (see Q4).
- [inferred] Q9 places the NJM2113 on the unregulated battery (≈ 2.8 V at the pin), so the speaker amplifier's clip level tracks the cells. This matches the existing S-006 note.
- [inferred] D6 and R81 form a charge path from the 3.3 V rail into the cell. The cell is therefore a **rechargeable** lithium type, consistent with "Built-in Lithium battery … About 3 minutes". With the cell at 2.5 V the charge current is at most ≈ (3.3 − 0.3 − 2.5)/180 ≈ 2.8 mA, falling to near zero as the cell approaches about 3.0 V. The cell's chemistry and capacity are not printed.
- [inferred] Back-up action. With batteries in, Q18 is on, so Q17 and Q16 are off, and the cell only charges. With batteries removed:
  1. R86 pulls Q18's gate low.
  2. Q17's gate rises through R84 from the decaying 3.3 V rail.
  3. Q17 conducts if IC4 holds P74 low.
  4. Q17 pulls Q16's base down through R83.
  5. Q16 connects the cell to the 3.3 V rail.

  This keeps IC4 (clock and settings such as the beep on/off) alive for about 3 minutes. P74 lets the firmware cut the back-up path, for example to stop a deep discharge. Recordings are in flash (non-volatile), so the back-up is not needed for them.
- [inferred] AN3 sees the raw battery voltage directly. Above ≈ 3.3 V (fresh cells under light load) it saturates against VREF+, so the firmware's battery gauge only resolves below about 3.3 V.
- [inferred] Average battery current ≈ 100 mA (recording) to ≈ 120 mA (playback), from "about 10 h / 8 h" with AAA alkaline cells of roughly 1 Ah at that drain. This is a rough order of magnitude, not printed.

### Gaps
- **Inconsistent printed values around Q10/Q13.** Q13's base and P73 read 0 V (so Q13 is off), yet Q10's base reads 2.6 V and its collector (the H rail) 3.3 V in recording. The flash and IC3 must be powered while recording. Either a printed value is wrong, or the H rail behaves in a way I could not trace. This cannot be resolved from the sheet.
- The playback values for P72 (Q20/Q9), P70 (amp CD) and P73 (Q10) are printed without parentheses, so whether the 3.2 V rail and the amplifier supply stay on in playback, and what they do in **stop**, is not stated. A P70 of 3.3 V in both modes would put the speaker amplifier in standby during playback, which cannot be right for speaker playback; so the "no parentheses" convention probably means "recording only".
- XC6372 electrical behavior (PFM/PWM switching threshold, the CE thresholds, soft-start) was not read; the Torex datasheet was not reachable. Another researcher covers this.
- The XC61A-series datasheet was not found; the 2.0 V decode rests on the XC61C naming analogy.
- L1's inductance is not printed: the parts list gives only "RL09U028T-T COIL".
- The back-up cell's part number, chemistry and capacity are not in this supplement (no parts-list entry).

## Q2. Sequencing: what enables the analog rail, codec PDN, power amp and DSP; every RC on those paths and its time constant

### Takeaway
Four microcontroller (IC4) ports do the sequencing:
- **P72** (pin 61) switches the 3.2 V analog rail (Q20) and the power-amp supply (Q9) together.
- **P11** (pin 34) drives the codec PDN and the DSP /ACFL input.
- **P10** (pin 33) drives the DSP /RST.
- **P70** (pin 59) drives the NJM2113 CD through R91 56 kΩ.

The only long time constant on any power-up path is the mic-bias filter: R4 39 kΩ × C2 22 µF ≈ **0.86 s**. Everything else is milliseconds or shorter.

### Cited Findings
- **P72 → Q20/Q9:**
  - P72 (pin 61, 0.2V) → bus D (sheet 3.1) → R87 10K → Q20 base, and R26 3.3K → Q9 base. No capacitor sits on this net on sheet 3.1. [read/traced]
  - C17 4V47 (ECST0GX476R, 4 V 47 µF) sits on Q20's collector (the 3.2 V rail), and C20 4V22 on Q9's collector. [read]

  — [S-002 sheets 3.1, 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **Codec PDN and DSP /ACFL:** IC4 pin 34 "P11" (3.3V, output arrow) → bus J → sheet 3.2 net "A". [read/traced] Net A carries:
  - **R29 1M to ground and C30 1000P to ground**, where both tops join a ground symbol. [read]
  - DSP pin 29 "/ACFL" (3.3V; the arrow points into the DSP, so it is an input). [read]
  - The codec PDN, pin 8 (3.3V), on sheet 3.1. [read]

  — [S-002 sheets 3.2, 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **DSP /RST:** IC4 pin 33 "P10" (3.3V, output) → bus I → DSP pin 30 "/RST" (3.3V, input). **R28 1M** runs from the 3.3 V rail to /RST, and **C31 1000P** is drawn between /RST and the 3.3 V line [read] — [S-002 sheets 3.2, 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **IC4 reset:** IC6 OUT with C49 0.1 µF → IC4 /RST (pin 32) [read] — [S-002 sheet 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **NJM2113 CD (pin 1, 2.1V):** driven only through **R91 56K** from bus F → **IC4 pin 59 "P70", 3.3V**. No capacitor is drawn on CD. R87 10K crosses the CD line without a junction dot: R87 is Q20's base resistor, not part of the CD network [read/traced] — [S-002 sheets 3.1, 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **DSP ↔ IC4 control lines (sheet 3.2 top bracket "H G F T P")** [read/traced]
  - DSP GO4 (81, 3.3V) / GO3 (80, 3.3V) / GO2 (79, 0V) are outputs to IC4 PA7 (23) / PA6 (22) / PA5 (21).
  - IC4 pin 50 "CHG CLK" (3.3V) → T → DSP pin 78 "/PMUTE" (3.3V).
  - IC4 pin 45 "P32" (0V) → P → DSP pin 77 "/ACDWN" (0V). A waveform "T≒25.6ms" (0/3.3 V square) points at /ACDWN.

  — [S-002 sheets 3.2, 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **DSP PLL sleep:** pin 27 "PLLSLEEP" has **no wire drawn**. DSP pin 28 "/FRSYNC" (2.7V, output) loops to pin 37 "GI0" (2.7V) with R25 1M to ground; a "T≒27ms" negative-pulse waveform points at it [read] — [S-002 sheet 3.2](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- Other analog-side capacitors charged when the 3.2 V rail comes up [read]:
  - C2 22 µF, the mic-bias filter: R4 39K.
  - C50 0.1 µF on the codec SGC (pin 1, 1.5V).
  - C13 "16V1" (ECST1CY105RR, 16 V 1 µF) on the NJM2113 BYPASS (pin 3, 1V).
  - C61 "4V10" (10 µF) in the earphone path; R64 33K biases Q14 from the 3.2 V rail.
  - The mic-amp coupling capacitors C1 0.15 µF, C4 0.1 µF, C5 1 µF and C6 1 µF (the AGC, R11 10 MΩ).

  — [S-002 sheet 3.1](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)

### Inferences
Computed time constants [inferred, from the printed values]:
| Path | Components | τ | Note |
|---|---|---|---|
| Mic-bias filter (Q11 base) | R4 39 kΩ × C2 22 µF | **0.858 s** (corner 0.186 Hz) | The mic bias ramps over about 4 s (5τ) whenever the 3.2 V rail comes up. Through R2 1 kΩ, the capsule and C1 into the ≈ 818 Hz amplifier high-pass (A-046), such a slow ramp is attenuated by a very large factor; it is a candidate cause of a record-start transient only if the rail is off in stop. |
| DSP AVDD / Q20 emitter filter | R93 10 Ω × (C28 1 µF + C21 0.1 µF + C22 10 µF = 11.1 µF) | 111 µs (corner 1.43 kHz) | Also limits the in-rush into C17 47 µF |
| IC8 CE / Q18 gate | (R85 ∥ R86 = 135 kΩ) × C69 0.1 µF | 13.5 ms | At battery insertion only |
| Back-up switch, Q16 turn-on | R83 10 kΩ × C77 1 µF | 10 ms | |
| Back-up switch, Q16 turn-off | (R82 390 kΩ + R83 10 kΩ) × C77 1 µF | ≈ 0.4 s | |
| Codec PDN / DSP /ACFL | R29 1 MΩ × C30 1000 pF | 1 ms | Matters only while the IC4 port is high-impedance (e.g. during IC4 reset); a driven edge is sub-µs |
| DSP /RST | R28 1 MΩ × C31 1000 pF | 1 ms | Same caveat |
| Battery sense AN3 | R40 100 kΩ × C37 0.01 µF | 1 ms | |
| IC8 EXT speed-up | R20 1 kΩ × C19 3300 pF | 3.3 µs | |

- [inferred] Q20's base has no capacitor. With ≈ 0.26 mA base drive (2.6 V / 10 kΩ) and a typical small-signal hFE, Q20 can source tens of mA, so the 3.2 V rail (C17 47 µF plus the rest) reaches its level within a few ms of P72 going low. The rail edge is fast compared with the mic-bias τ.
- [inferred] Because P72 switches the codec supply (Q20) and the amplifier supply (Q9) together, any rail-switching transient at the start of playback arrives together with the NJM2113's own start-up (C13 1 µF bypass, C20 22 µF supply, and CD released by P70). The NJM2113's internal bias resistance, and so the BYPASS time constant, is not on the sheet.
- [inferred] The codec power-up sequence the firmware can enforce:
  1. P72 low (rail on).
  2. P11 high (codec PDN released; DSP told via /ACFL).
  3. P10 high (DSP /RST released).
  4. P70 low (amp enabled), playback only.

  The order and delays live in firmware and are not printed.
- [inferred] A record-start "thump" from the power circuitry would have to come from:
  - the mic bias settling (τ 0.86 s, heavily high-passed);
  - the codec's own analog power-up after PDN;
  - the AGC capacitor C6 1 µF and the coupling capacitors charging (C5 1 µF into R10 1 kΩ, among others).

  None of these is quantifiable from the sheet without knowing whether the 3.2 V rail is off in stop.

### Gaps
- No printed stop-mode voltages, so whether P72, P11 and P70 change between stop and record is not stated.
- The MSM7702 PDN-to-ready time and the NJM2113 CD/BYPASS start-up time are in their datasheets (other researchers), not in S-002.
- The meaning of DSP "/ACFL", "/ACDWN", "/PMUTE", "PLLSLEEP" and IC4 "CHG CLK" is not stated anywhere in this supplement. The names suggest "A/C (codec) fail/flag", "A/C power down", "playback mute" and "clock change", but these are guesses [inferred, low confidence].

## Q3. Clock: X1 and IC3, the DSP PLL and loop filter, other clock destinations, X2, waveform annotations, DSP core frequency

### Takeaway
X1 is a 3-terminal 8 MHz **ceramic resonator with built-in capacitors** (part RSXY8M00M07T, "CERAMIC OSC"). It oscillates with the first inverter of IC3 TC7WU04 (R37 1 MΩ bias, R69 680 Ω series). A second IC3 inverter buffers the result, and the buffered 8 MHz feeds **three** places: DSP OSCF0, **IC4 OSC1**, and IC5's input 2A. IC3 sits on the Q10-switched "H" rail. The DSP's PLL loop filter is **R23 560 Ω in series and R22 56 Ω + C23 0.047 µF in shunt**. The printed waveforms are "T≒2us" (BCLOCK), "T≒170us" (RXSYNC2 → codec XSYNC and RSYNC) and "F=8MHz 3.2V P-P" (OSCF0). There is no annotation of the DSP core frequency.

### Cited Findings
**Oscillator (sheet 3.3, image 10)** [read]
- X1 "(8MHz)" is drawn as a dashed box with pins 1 and 3 and a resonator plus two internal capacitors to pin 2 (ground), i.e. built-in load capacitors.
- Parts list: X1 RSXY8M00M07T "CERAMIC OSC". [read]

  — [S-002 sheet 3.3 and parts list p.7](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- IC3 "TC7WU04FUT2L / TRIPLE INVERTER" [read]:
  - Inverter A: input pin 1 (1.6V) from X1 pin 1; output pin 7 (1.7V). R37 1M runs from pin 7 to X1 pin 1 (bias), and **R69 680** from pin 7 to X1 pin 3.
  - The waveform "F=8MHz, 3V P-P" (sine-like) points at the pin 7 / R69 node.
  - Inverter B: input pin 6 (1.7V) from the pin 7 node; output **pin 2 (1.6V)**.
  - Inverter C: input pin 3 grounded; output pin 5 unconnected.
  - VCC pin 8 3.3V with C63 0.1 µF.

  — [S-002 sheet 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **Buffered 8 MHz destinations** [traced]: IC3 pin 2 → down a vertical track →
  - IC4 pin 10 "OSC1" (1.6V); IC4 pin 9 "OSC2" is unconnected.
  - IC5 TC7W126 pin 5 (2A, 1.6V).
  - The sheet-3.3 left-edge line, then across sheet 3.2's bottom to **DSP pin 24 "OSCF0"** (1.6V), where the waveform reads "F=8MHz, 3.2V P-P". **DSP pin 25 "OSCF1" has no wire**.

  — [S-002 sheets 3.2, 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **IC5 "TC7W126FUT2L / BUFFER AMP"** [read/traced]:
  - Buffer 1: 1A pin 2 (3.3V), 1G pin 1 (3.3V) from IC4 pin 36 "P13" (3.3V); output pin 6.
  - Buffer 2: 2A pin 5 = the 8 MHz; 2G pin 7 (**0V**, so disabled while recording) from IC4 pin 37 "P14" (0V); output pin 3.
  - Outputs 3 and 6 are tied together → flash IC7 pin 24 "SCLK", with R90 47K to the flash's H-rail bus.
  - Vcc pin 8 is on the H rail; C64 0.1 µF.

  — [S-002 sheet 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **DSP PLL (sheet 3.2, image 9)** [read]:
  - Pin 20 "VCO IN" (0.8V) and pin 21 "PC OUT" (0.8V), joined by **R23 560**. Parts list: R23 is MCR03PZHJ561, the only non-ERJ resistor in the list.
  - From the VCO IN node, **R22 56 in series with C23 0.047** to the Vss node (pins 13, 22 AVss, 23 Vss).
  - Pin 19 "AVDD" (3.3V) has C28 1 µF, C21 0.1 µF and C22 "4V10" (10 µF) to ground, on a line from sheet 3.1's R93/Q20-emitter node.

  — [S-002 sheet 3.2, parts list p.6](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **Waveform annotations, exact print** [read]:
  - Sheet 3.2:
    - "F=8MHz" "3.2V P-P" → OSCF0.
    - "T≒2us." (0V/3.3V square) → pin 48 "BCLOCK" (1.7V), which goes via bus D to codec pin 13 BCLK (1.7V).
    - "T≒170us." (narrow positive pulse, 0V/3.3V) → pin 90 "RXSYNC2" (0V). This pin is an output that runs to sheet 3.1 and the **codec XSYNC (pin 12, 0V) and RSYNC (pin 9, 0V)** [traced].
    - "T≒180us." → pin 38 "GI1" (1.3V); GI1 is looped from pin 99 "/BPSYNC" (1.3V), with R24 1M to ground [traced].
    - "T≒27ms." → GI0/FRSYNC.
    - "T≒25.6ms." → /ACDWN.
    - "T≒27.4ms." (bursts) → the P0–P7 host port.
  - Sheet 3.3:
    - "F=32.7kHz 1.3V P-P" at XI and "F=32.7kHz 3.4V P-P" at XO.
    - "T≒52.4ms." → SBI0/SBT0/SBO1/SBI1.
    - "T≒55ms." → P30/P31.
    - LCD drive levels 3.3/2.2/1.1/0 V.
  - Sheet 3.3 IC3 node: "F=8MHz, 3V P-P".

  — [S-002 sheets 3.2, 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **X2:** "X2 (32.7kHz)" between IC4 pin 12 "XI" (1.3V) and pin 13 "XO" (1.7V); **C38 22P** on XI and **C39 12P** on XO to ground. Parts list: X2 RSXC32K7L04T "XTAL,32K"; C38 ECUV1H220KCV 50 V 22 pF; C39 ECUV1H120KCV 50 V 12 pF [read] — [S-002 sheet 3.3, parts list p.4, p.7](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- No annotation of the DSP core or PLL output frequency appears anywhere in the supplement [read: absence] — [S-002](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)

### Inferences
- [inferred] **One 8 MHz source clocks both processors**: the DSP takes an external clock on OSCF0, and IC4's high-speed clock is the same signal on OSC1. The 32.768 kHz crystal is IC4's sub-clock (for timekeeping and standby). The codec's BCLK and sync both come from the DSP, so the codec's sample rate is locked to X1's tolerance. This supports A-011's "per-unit deviation follows the resonator" and adds that IC4 shares the clock.
- [inferred] IC3 and IC5 are on the Q10/H rail. So in standby IC4 can switch off the 8 MHz oscillator (by cutting H through P73/Q13/Q10) and run from the 32.768 kHz crystal. On wake, X1 must restart: a ceramic resonator typically starts in under a millisecond to a few milliseconds (not printed).
- [inferred] Arithmetic on the printed values:
  - "T≒2us" = 500 kHz = 8 MHz ÷ 16 if exact.
  - "T≒170us" ≈ 5.88 kHz, against the specified 6 kHz (166.7 µs). The "≒" marks the scope readings as approximate (about 2 %), so they do not show an off-nominal rate.
  - 8 MHz ÷ 6 kHz = 1333.3 is not an integer, so an exact 6000 Hz frame needs the PLL, e.g. ×3 to 24 MHz then ÷ 4000; and 500 kHz ÷ 6 kHz = 83.3 BCLKs per frame is not an integer either.

  Either the frame rate is not exactly 6000 Hz (e.g. 8 MHz ÷ 1334 = 5997 Hz, −0.05 %), or BCLK is not exactly 500 kHz. The sheet cannot settle this; a capture of a real unit can (A-011's verification). The "T≒180us" loop on /BPSYNC → GI1 suggests a second frame-rate timing signal internal to the DSP, but 170 vs 180 µs is within the reading error of a scope annotation.
- [inferred] Loop filter: a passive lag-lead with τ1 = (560 + 56) Ω × 47 nF = 29.0 µs (pole ≈ 5.5 kHz) and τ2 = 56 Ω × 47 nF = 2.63 µs (zero ≈ 60.5 kHz). The phase-detector reference frequency and the VCO gain are unknown, so the loop bandwidth cannot be computed. PLL jitter at audio rates is therefore not modelable from S-002; it is most likely negligible at 6 kHz sampling with an 8-bit µ-law codec.
- [inferred] IC5 is a 2:1 selector for the flash serial clock. Buffer 1's input (pin 2) is probably IC4's serial clock: the SBT0/SBT1 pins are tied together and routed toward IC5, but I did not trace them to completion. Buffer 2 can put the 8 MHz itself on the flash SCLK, disabled in the recording-mode reading.

### Gaps
- X1's frequency tolerance and its manufacturer: RSXY8M00M07T is a Panasonic service code; the vendor part is not given.
- No DSP core-clock, PLL-multiplier or MCLK annotation. The MN1931712BB has no public datasheet found (S-002 is the only source).
- Which signal feeds IC5 1A was not traced to completion.
- The exact relation between 8 MHz and the codec frame rate (integer or not) is not printed.

## Q4. Decoupling and filtering on the analog rails, ferrites/inductors, ground notes, switching-noise mitigation

### Takeaway
There is **no ferrite or inductive filter** between the boost converter and the analog section. L100–L102 are 0 Ω chip jumpers, and L1 is the boost coil. The mitigation is purely RC and active:
- R93 10 Ω with 11.1 µF at the DSP AVDD / Q20 emitter node;
- C17 47 µF on the switched 3.2 V rail;
- the Q11 capacitance multiplier (R4 39 kΩ / C2 22 µF) for the microphone bias;
- 300 µF of electrolytic on the converter output.

The supplement has no ground-layout notes.

### Cited Findings
- Parts list: "L100-02 ERJ3GEY0R00V CHIP JUMPER" (3 pcs) and "L1 RL09U028T-T COIL". The schematic marks L100, L101, L102 "0 ohm" in the earphone jack path [read] — [S-002 parts list p.5, sheet 3.1](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- Converter output: C24 + C81, each 4 V 150 µF (RCSX0GC157RE). Converter input: C9 4 V 10 µF (RCST0GA106RE) + C7 0.1 µF [read] — [S-002 parts list p.4–5](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- Analog-side decoupling [read]:
  - R93 10 Ω (ERJ3GEYJ100V) feeds Q20's emitter and the DSP AVDD line (C28 1 µF ECUV1C105ZFN, C21 0.1 µF, C22 4 V 10 µF ECST0GY106RR).
  - C17 4 V 47 µF (ECST0GX476R) on the 3.2 V rail.
  - C50 0.1 µF on the codec SGC.
  - C2 4 V 22 µF (ECST0GY226RR) at Q11's base, R4 39 kΩ.
  - C20 4 V 22 µF (ECST0GY226RR) on the NJM2113 VDD; C13 16 V 1 µF on BYPASS.

  — [S-002 sheets 3.1–3.2, parts list p.4](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- Digital decoupling, all 0.1 µF ECUZNC104ZFV [read]: C25, C27, C29, C32 at the DSP; C63 at IC3; C64 at IC5; C41 with C40 4 V 22 µF on the H rail; C49 at IC6; C65 on IC4's LCD ladder; C44 0.22 µF at the flash Vpp side — [S-002 sheets 3.2–3.3, parts list](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- Parts-list preamble: "special parts which have purposes of fire-retardant (resistors), high-quality sound (capacitors), low-noise (resistors), etc. are used" — a general note that does not name parts [read] — [S-002 p.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- PCB (images 7, 13): side A carries the analog/codec block (IC1, Q1–Q6, C2, C17) in the upper right, the microcontroller IC4 in the middle and the DSP IC2 at the bottom. The boost converter (IC8, Q8) sits on side B at location 6C, with L1 at 5D on side A. Board code "1974C-3 (REP2714B-M)". No ground-plane or star-ground note is printed [read] — [S-002 PCB](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)

### Inferences
- [inferred] Ripple attenuation from the converter to the analog rail. R93 with 11.1 µF is a first-order low-pass at ≈ 1.4 kHz, so at a 100 kHz switching frequency (if the "1" digit means 100 kHz) the attenuation is ≈ 37 dB before Q20, and C17 47 µF on the far side helps further.
  - In **PFM** mode at light load, the XC6372 family switches in bursts at lower, load-dependent rates. Those components can fall into or near the audio band, where R93's filter attenuates them less (e.g. ≈ 6 dB at 2.5 kHz).
  - The codec's own supply rejection and the Q11 multiplier on the mic bias are the remaining defences.
  - This makes converter-burst tones a plausible, unverified contributor to the noise floor (a stage 15 candidate; another researcher covers the XC6372 PFM behavior).
- [inferred] The microphone bias is the most heavily filtered supply on the board. A capacitance multiplier with a 0.19 Hz corner means converter ripple on the bias is attenuated by roughly the ratio of the ripple frequency to 0.19 Hz, limited in practice by Q11's collector-base capacitance and the Early effect. Supply-borne noise at the capsule should be well below the capsule's own hiss (A-048).

### Gaps
- XC6372D331PR's actual switching frequency and its PFM burst behavior at this load: datasheet not reachable here.
- Ground layout and copper pour: no notes, and the PCB images show tracks only at a resolution that does not reveal the ground strategy reliably.
- L1's value is not printed.

## Q5. Other items relevant to noise reduction, playback speed control (S6) and beep/tone injection

### Takeaway
- **S6 SPEED CONTROL** goes to the **microcontroller IC4 (pin 41, IRQ3)**, not the DSP, so any speed change is a firmware command from IC4 to the DSP.
- The **beep** comes from **IC4 pin 35 (P12)** through R19 220 kΩ / C16 10 pF into the NJM2113 inverting input. It bypasses the codec and DSP entirely. IC4's pin 31 "BUZZER" is unconnected.
- No DSP pin is named for noise reduction. The DSP's /DTMFCS is tied high, and LMUTE, PSACK, PLLSLEEP and GO5–GO10 are unconnected.

### Cited Findings
- Switch wiring (sheet 3.2) [read/traced]:
  - The commons of S5 (HOLD), S7 (MODE SELECT) and S6 (SPEED CONTROL) go to bus letters C, O and N, each with a 1M pull-up (R34, R35, R36) to the 3.3 V rail.
  - S6 "ON" grounds its line; S5 "ON" grounds through R98 1K; S7 "CLOCK/MIC SET" grounds its line.
  - N → **IC4 pin 41 "IRQ3" (2.8V)**; O → IC4 pin 42 "IRQ4" (2.8V); C → IC4 pin 18 "KEY2" (2.8V).

  — [S-002 sheets 3.2, 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- Schematic notes (image 12): "S6 : Speed control(SPEED CONTROL) switch to "ON" position." [read] — [S-002 notes](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- Key ladders [read]:
  - S3 dial (UP/DOWN/SET) and S11 (10 SEC REVIEW/BEEP) through R32 120K and R41 180K, with R43 100K pull-up, → IC4 pin 16 "AN0" (3.3V idle). S3's UP and DOWN also go to IRQ1 and IRQ2 (pins 39, 40).
  - S2 ERASE, S4 REC START/STOP and S9 MODE through R31 390K, R33 120K and R42 180K, with R44 100K pull-up, → IC4 pin 17 "AN1" (3.3V).

  — [S-002 sheets 3.2, 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- Beep [read/traced]:
  - R19 220K + C16 10P from NJM2113 pin 4 to bus G. On sheet 3.3, G connects to **IC4 pin 35 "P12" (0V in recording)**.
  - IC4 pin 31 "BUZZER" has **no wire drawn**. (Pins 27 and 30, SBT0/SBT1, join a separate line that ends at pin 30 and does not reach G.)
  - The operation-tone addendum: "a tone sounds each time the unit is operated", "The tone is set not to sound when the unit is shipped."

  — [S-002 sheet 3.3 and image 11](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- DSP pins with no audio-processing control visible [read]:
  - MODE0–3 (31–34) grounded.
  - LMUTE (35) and PSACK (36) unconnected.
  - MEM MODE, EX CLK, EXSH CLK, ADPCM IN and HWYSYNC (43–47) grounded; ADPCM OUT (49) unconnected.
  - /DTMFCS (97) tied to the 3.3 V VDD bus; HWY CLK (98) joined to Vss (100) at a ground symbol.
  - GO5–GO10 (82–87) unconnected.
  - The host interface is P0–P7 (59–66) ↔ IC4 D0–D7 (67–74), with POBF/PIBE/PRE/PWE/PCS handshakes.

  — [S-002 sheet 3.2](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- REC lamp: D1 LNJ210C6RRA anode on the 3.3 V rail, cathode through R1 1.5K to IC4 pin 60 "P71": "0.2V (3.3V)", i.e. lit while recording, off in playback [read] — [S-002 sheets 3.1, 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)

### Inferences
- [inferred] Playback speed control is firmware. IC4 reads S6 and must tell the DSP, either over the P0–P7 host port or over a dedicated line. The candidate is IC4 pin 50 "CHG CLK" → DSP "/PMUTE" (3.3 V while recording), but the name pairing is ambiguous. There is no hardware clock switching for speed: the 8 MHz source is fixed and the codec's clocks come from the DSP.
- [inferred] The beep is injected after the codec, at the power amplifier's inverting input, scaled by R18/R19 (68 kΩ/220 kΩ ≈ −10 dB relative to the amplifier's output reference). It cannot be recorded electrically, only acoustically if the speaker plays it during recording. It is off by default. The earphone branch taps the same pin-4 node, so the beep presumably also reaches the earphone.
- [inferred] Nothing in the hardware implements noise reduction. If the "noise reduction" claim (S-004) is real, it is DSP firmware, consistent with the evidence-pass conclusion in CLAUDE.md.

### Gaps
- The firmware behavior of S6 (speed range, pitch handling) cannot be learned from the schematic.
- The original service manual for the RR-DR60P-S (Order No. AD9807107C1, cited on the cover as "Please file and use this manual together with…") may hold the block diagram, IC pin descriptions and troubleshooting text. This supplement "refer[s]" all other items to it. Not found.

## Q6. Parts list: part numbers for IC8, IC6, X1, X2, L parts, battery parts, Q9, Q20, Q11 and related

### Takeaway
All requested semiconductors and timing parts are listed. The lithium back-up cell is **not** in this supplement's parts list.

### Cited Findings (parts list, images 3–6, printed pages 4–7) [read]
| Ref. | Part No. | Description |
|---|---|---|
| IC8 | XC6372D331PR | IC |
| IC6 | XC61AN2002MR | IC |
| IC3 | TC7WU04FUT2L | IC |
| IC5 | TC7W126FUT2L | IC |
| IC4 | MN101C16ACA3 | IC |
| IC2 | MN1931712BB | IC |
| X1 | RSXY8M00M07T | CERAMIC OSC |
| X2 | RSXC32K7L04T | XTAL,32K |
| L1 | RL09U028T-T | COIL |
| L100–102 | ERJ3GEY0R00V | CHIP JUMPER (3) |
| Q8 | 2SD1119RTX | TRANSISTOR |
| Q9, Q10 | 2SB1295-6-TB | TRANSISTOR (2) |
| Q11 | 2SD1819ASTX | TRANSISTOR |
| Q13, Q14 | 2SD1819ASTX | TRANSISTOR (2) |
| Q16 | 2SB1295-6-TB | TRANSISTOR |
| Q17, Q18 | 2SK1958T1 | TRANSISTOR (2) |
| Q20 | 2SB1218ASTX | TRANSISTOR |
| D2, D5 | MA735TX | DIODE |
| D4 | MA728TX | DIODE |
| D6 | MA729TX | DIODE |
| D1 | LNJ210C6RRA | LED |
| C24, C81 | RCSX0GC157RE | 4V 150U |
| C9 | RCST0GA106RE | 4V 10 |
| C17 | ECST0GX476R | 4V 47U |
| C2, C20, C40 | ECST0GY226RR | 4V 22U |
| C22 | ECST0GY106RR | 4V 10U |
| C23 | ECUVNC473KBV | 16V 0.047U |
| C19 | ECUV1H332KBV | 50V 3300P |
| C30, C31 | ECUV1H102KBV | 50V 1000P |
| C37 | ECUV1E103KBV | 25V 0.01U |
| C38 / C39 | ECUV1H220KCV / ECUV1H120KCV | 50V 22P / 50V 12P |
| C69 | ECUV1C104KBV | 16V 0.1U |
| C77 | ECUVNJ105KBV | 63V 1U |
| R23 | MCR03PZHJ561 | 1/16W 560 |
| R22 | ERJ3GEYJ560V | 1/16W 56 |
| R69 | ERJ3GEYJ681V | 1/16W 680 |
| R93 | ERJ3GEYJ100V | 1/16W 10 |
| R85, R86 | ERJ3GEYJ274V | 1/16W 270K |
| R81 | ERJ3GEYJ181V | 1/16W 180 |
| Battery parts | RJC40015 BATT.TERMINAL(+); RJC80017 BATT.TERMINAL(−); RJC94013-2 BATT.TERMINAL; RKK0115-S BATT.COVER | — |

— [S-002 parts list](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- The IC package drawings on image 12 group "XC61AN2002MR" (a 3-lead outline) and "XC6372D331PR" (a 5-lead outline, pins 1–5), and "MA735TX" and "MA728TX/MA729TX" as diodes with cathode marks [read] — [S-002 notes](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)

### Inferences
- [inferred] R23 is the only resistor in the list from a different series (MCR03, Rohm-style code; "J" = ±5 %, the same tolerance as the ERJ parts). It may be a second-source substitution rather than a precision part; nothing indicates a special tolerance.
- [inferred] MA735 and MA729 are Panasonic Schottky barrier diodes, from the Schottky-style symbols on the sheet and the series names. That fits a boost rectifier, a reverse-protection diode and a low-drop charge path.

### Gaps
- No part number for the lithium back-up cell, nor for its leads or holder.
- L1's inductance and current rating are not given (service code only).

## Q7. Where the existing S-002 summary (sources.md) and the stage rows look wrong or incomplete

### Takeaway
Four statements in the S-002 row misidentify which microcontroller pin a sheet-3.1 net reaches, because sheet 3.3 carries **two** bracket buses that both use the letters A–H. Sheet 3.1's bus (F G A E H D C B) lands on IC4's **right-hand** pins P70–P77 and the G/H lines, not on the left-hand AN0/PA4–PA7 pins. The printed voltages confirm this mapping. Two other statements about R29/C30 and R87 are also wrong.

### Cited Findings
- **Earphone plug detect:** the S-002 row says "R67 390 kΩ pulls it to 3.3 V → net "A" → IC4 pin 16 AN0". On the sheet, sheet 3.1's A reaches **IC4 pin 66 "P77" (3.1V)**. IC4's AN0 (pin 16) is the S3/S11 key ladder from sheet 3.2's own net A (R32/R41/R43) [read/traced] — [S-002 sheets 3.1–3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **Power-amp shutdown:** the S-002 row says "the shutdown pin 1 reads 2.1 V (standby) in record mode through R87 10 kΩ, driven via R91 56 kΩ from net "F" → IC4 pin 21 (PA5)". On the sheet:
  - R87 10 kΩ is **Q20's base resistor** and only crosses the CD line, with no junction dot.
  - CD is fed through R91 56 kΩ alone, from sheet 3.1's F → **IC4 pin 59 "P70" (3.3V)**.
  - IC4 PA5 (pin 21, 0V) is fed by the DSP's GO2.

  [read/traced] — [S-002 sheets 3.1, 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **Beep line:** the S-002 row says "R19 220 kΩ + C16 10 pF run from pin 4 to net "G" → IC4 pin 22 (PA6)". On the sheet, sheet 3.1's G reaches **IC4 pin 35 "P12"**. PA6 (pin 22) is fed by DSP GO3, and IC4's "BUZZER" pin 31 is unconnected [read/traced] — [S-002 sheets 3.1, 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **Codec PDN:** the S-002 row says "PDN (8) driven from net A, which the DSP's /ACFL line feeds through R29/C30 (a microcontroller link to that net is unverified)". On the sheet:
  - The net is driven by **IC4 pin 34 "P11"** (via J). The link is verified.
  - R29 1 MΩ and C30 1000 pF are both **shunts to ground**, not a series RC.
  - "/ACFL" is a DSP **input** (the arrow points into pin 29).

  [read/traced] — [S-002 sheets 3.2, 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **Clock tree:** the S-002 row says "X1 8 MHz ceramic resonator (DSP OSCF0/1 through IC3)". Only OSCF0 is wired, and OSCF1 is open. The buffered 8 MHz also clocks **IC4 (OSC1)** and reaches IC5. A-011's statement "the 32 kHz crystal clocks the microcontroller only" should read: the 32 kHz crystal is the microcontroller's sub-clock, and its main clock is the shared 8 MHz [read/traced] — [S-002 sheets 3.2, 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)
- **A-013 / signal-chain row 15:** "2× AAA → 3.3 V DC-DC converter; switched 3.2 V analog rail" is correct but incomplete. The power amplifier runs from the raw battery via Q9, switched by the same port as Q20 (P72). The flash, the 8 MHz oscillator and IC5 sit on a third switched rail (Q10, P73). The DC-DC converter is enabled by the battery itself (CE = Vbat/2), not by firmware [read/traced] — [S-002 sheets 3.1, 3.3](https://archive.org/details/manual_RRDR60_SM_PANASONIC_EN)

### Inferences
- [inferred] **Why the letters were confused.** Sheet 3.3 has a left-hand bracket A–H at IC4 pins 16–23. It carries sheet 3.2's key ladders (A AN0, B AN1), the S5 HOLD line (C KEY2), the battery sense (D AN3) and the DSP flags (E PIBE → PA4; F/G/H GO2/GO3/GO4 → PA5/PA6/PA7). Sheet 3.3 also has a right-hand bracket A–H at pins 59–66 plus G/H at the bottom right, which carries sheet 3.1's power-control bus. The recording-mode voltages agree with the right-hand mapping:
  - B 0 V = Q17 source 0 V.
  - C 0 V = Q13 base 0 V.
  - D 0.2 V = the Q20/Q9 bases pulled low, i.e. on.
  - E 0.2 V (3.3 V) = the REC LED on while recording.
  - F 3.3 V = amplifier standby, CD 2.1 V.

  The left-hand mapping would contradict several of these (PA5 0 V vs CD 2.1 V standby; KEY2 2.8 V vs Q13 base 0 V).
- [inferred] Repository records that cite the earlier mappings (the S-002 row, any A-0xx row or spec text naming "IC4 pin 21 PA5", "pin 22 PA6" or "pin 16 AN0" for the amp CD, the beep or the plug detect) should be corrected through the usual register process. None of these change an audio model, only the provenance text.

### Gaps
- The bus-letter mapping is a reading of line work across three sheets. Each mapping is consistent with the printed voltages, but a continuity check on a real board would confirm it (listed for the "Hardware evidence" template).
