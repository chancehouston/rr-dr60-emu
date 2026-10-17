# Power supply of the Panasonic RR-DR60 and its audible / measurable effects

Scope: component behavior and quantification of the RR-DR60 supply. 2× AAA → Torex XC6372 step-up to 3.3 V, a switched 3.2 V analog rail (Q20), the NJM2113 speaker amplifier on the raw battery line through the PNP Q9 (2SB1295), and the Torex XC61AN reset detector. All of it is checked against how the supply could shape the recorded or played-back sound.

**Topology assumed here** (from the project's reading of S-002; another researcher is confirming it on the schematic):
- XC6372 output = 3.3 V digital/codec rail.
- Q20 = a saturated switch from 3.3 V to the 3.2 V analog rail (mic amp, codec analog, earphone follower bias).
- Mic bias = Q11 emitter follower whose base sits on R4 39 kΩ / C2 22 µF (a "capacitance multiplier"), giving ≈ 2.7 V.
- NJM2113 V+ = battery through Q9, with C20 22 µF at pin 6. The service sheet marks 2.8 V there in record mode.
- NJM2113 pin 3 (FC1/"+VIN") = C13 1 µF. Pin 2 (FC2/"VREF") capacitor not known to me.
- XC61AN monitors an unknown node (the battery line or the 3.3 V rail).

Project context read first: [sources.md](../../../sources.md) S-001, S-002, S-006; [assumptions.md](../../../assumptions.md) A-013, A-029, A-031, A-037, A-048, A-050; [signal-chain.md](../../../signal-chain.md) rows 11 and 15.

Local copies of the downloaded documents (untrusted) are in `/private/tmp/claude-501/-Users-chance-Code-DR60Recorder/baebfed1-e821-4f45-bb42-e63af34626c2/scratchpad/power_dl/`. The calculation script is `/private/tmp/claude-501/-Users-chance-Code-DR60Recorder/baebfed1-e821-4f45-bb42-e63af34626c2/scratchpad/scripts/calc.py`, and every number below marked "calc" comes from it.

---

## 1. XC6372 step-up converter: part decode, switching modes, frequency, ripple, efficiency, start/hold voltage

### Takeaway
"XC6372D331PR" decodes as:
- **PWM/PFM auto-switching** step-up with **standby (CE) capability**.
- An **external switching transistor** (EXT pin, so there is a discrete switch transistor and inductor on the board).
- **3.3 V** output (±2.5 %).
- **100 kHz** oscillator (±15 %: 85–115 kHz).
- SOT-89-5 package, embossed tape.

In record and playback, the estimated 3.3 V load (≈ 45–60 mA) is well above the PFM/PWM boundary (≈ 4–24 mA for L = 100 µH, calc), so the converter should run **fixed-frequency PWM at 85–115 kHz**, above the audio band. PFM pulse skipping, whose rates fall in the audio band (0.4–25 kHz at 0.1–1 mA loads, calc), is expected only at light load (stop/standby), when nothing is recorded or played.

### Cited Findings
- XC6371 = PWM-controlled. XC6372 = "switches from PWM to PFM control during light loads when automatically switching is selected". Further datasheet figures:
  - built-in 1.4 Ω switch on the built-in types;
  - output 2.0–7.0 V, ±2.5 %;
  - oscillator selectable 50/100/180 kHz, ±15 %;
  - soft-start internally set ("protection against in-rush currents … prevents voltage overshoot");
  - operation start voltage range 0.9–10 V.
  — [Torex XC6371/XC6372 datasheet ETR0402_005](https://uploadcdn.oneyac.com/attachments/files/05/FC/XC6371A501PR-G.pdf)
- Ordering code (older full-family datasheet, six types). ① type:
  - A = 3-pin, built-in switching transistor;
  - B = 3-pin, external switching transistor;
  - C = standby capability, built-in transistor;
  - **D = standby capability, external switching transistor**;
  - E/F = separated VDD and VOUT (built-in / external).

  Remaining fields:
  - ②③ = output voltage (e.g. 3.5 V → 35);
  - ④ = oscillator: **0 = 50 kHz, 1 = 100 kHz, 2 = 180 kHz**;
  - ⑤ package: A–B → SOT-89-3, **C–F → SOT-89-5 ("P")**;
  - ⑥ R = embossed tape, right orientation.

  The pin table gives the C/D types CE on pin 3, with "Built-in tr. type units use the LX pin. External tr. type units use the EXT pin. The CE pin is only used with the XC6371C and XC6371D." — [Torex XC6371 older datasheet (searchdatasheet mirror of torex.co.jp)](http://pdf.searchdatasheet.com/b30b978d/torex.co.jp/XC6371B350PL.html)
- The current datasheet lists only the A and C types: "④ Oscillation Frequency 0 50kHz / 1 100kHz / 2 180kHz"; "PR = SOT-89 (A type) / SOT-89-5 (C type)". The D type is discontinued; Torex's obsolescence list includes XC6371D/XC6372D (per search-result snippet). — [Torex XC6371/XC6372 datasheet ETR0402_005](https://uploadcdn.oneyac.com/attachments/files/05/FC/XC6371A501PR-G.pdf); [Torex EOL list (snippet)](https://www.torexsemi.com/file/en/products/discontinued/-2016/EOL-E0520.pdf)
- Electrical characteristics (XC6371/72A501, VOUT = 5.0 V, FOSC = 100 kHz, 25 °C):

  | Parameter | Figure |
  |---|---|
  | Oscillation frequency | 85 / 100 / 115 kHz |
  | Max duty ratio | 80 / 87 / 92 % |
  | **PFM duty ratio** (XC6372) | **10 / 17 / 25 %** |
  | Operation start voltage, external components, IOUT = 1 mA | ≤ 0.90 V |
  | Oscillation start voltage | ≤ 0.80 V |
  | No-load input current | 12.8 µA typ, 25.7 max |
  | Supply current while continuously oscillating | 80.2 µA typ |
  | Lx on-resistance | 1.4 Ω typ, 2.4 max |
  | Efficiency | 85 % typ |
  | **Slow-start time** | **4.0 / 10.0 / 20.0 ms** |
  | Standby current (C type) | ≤ 0.5 µA |
  | CE high / low | ≥ 0.75 V / ≤ 0.20 V |

  — [Torex XC6371/XC6372 datasheet ETR0402_005](https://uploadcdn.oneyac.com/attachments/files/05/FC/XC6371A501PR-G.pdf)
- Typical curves read from the rendered datasheet (L = 100 µH Sumida CR54, CL = 47 µF tantalum, built-in-transistor A types):
  - **Ripple, XC6372A301PR (3.0 V out):** ≈ 5–8 mVp-p from 0.1 mA to a few mA. ≈ 10 mVp-p at 10 mA. Rising to ≈ 20–40 mVp-p at 30–100 mA, depending on VIN (VIN = 2.7 V reaches ≈ 20 mV only near 100 mA, lower VIN rises sooner). The 0–100 mVp-p axis tops out near 60 mV at the highest currents.
  - **Efficiency, XC6372A301PR:** ≈ 75–80 % at 0.1 mA (the PFM benefit over the XC6371's ≈ 55–60 % at 0.1 mA). ≈ 80–88 % from 1 to ≈ 50 mA at VIN 1.2–2.7 V, highest (≈ 88 %) at VIN = 2.7 V.
  - **Hold voltage** (XC6371A501): output held down to VIN ≈ 0.4 V at 1 mA, ≈ 0.88 V at 10 mA, ≈ 1.2 V at 30 mA. Start voltage ≈ 0.7–0.9 V at light load.
  - **Load transient** 1 → 30 mA at VIN = 3 V: a dip/overshoot of order 0.1 V lasting a few ms.

  — [Torex XC6371/XC6372 datasheet ETR0402_005, pp. 8–12](https://uploadcdn.oneyac.com/attachments/files/05/FC/XC6371A501PR-G.pdf)
- The datasheet's reference circuit uses an MA735/MA2Q735 Schottky (Matsushita), a 100 µH coil and a 47 µF tantalum. The RR-DR60's actual L, C and switch transistor are not known to this researcher. — [Torex XC6371/XC6372 datasheet ETR0402_005](https://uploadcdn.oneyac.com/attachments/files/05/FC/XC6371A501PR-G.pdf)
- General PFM behavior: in PFM, pulses are skipped as the load drops, and the resulting sub-harmonic shifts with load. It can land in the audible band. Vendors add circuits to keep it above ≈ 20 kHz (TI measured ≈ 53 kHz at no load with such a scheme). — [TI, "Make a Boost Converter Quieter" (search summary)](https://www.ti.com/lit/pdf/sszt873); [TI SLVA890 (search summary)](https://www.ti.com/lit/an/slva890/slva890.pdf); burst-mode frequency becomes load-dependent and can fall into the noise band — [Embedded Computing Design, boost converters at light load (search summary)](https://embeddedcomputing.com/technology/analog-and-power/analog-semicundoctors-sensors/boost-converter-design-for-sensitive-loads-at-light-load)

### Inferences
- **The board has an external switch transistor and inductor** (D type). The 1.4 Ω Lx figures and the A-type curves are therefore only indicative. Peak currents can be higher than the built-in type allows.
- **PFM pulse energy and the PFM/PWM boundary** (calc; DCM boost, PFM on-time t_on = 0.17 × 10 µs = 1.7 µs, Vout = 3.3 V, Schottky Vd ≈ 0.3 V):
  - I_pk = Vin·t_on/L
  - E_L = ½·L·I_pk²
  - energy delivered per pulse E_out = E_L·(Vout+Vd)/(Vout+Vd−Vin)
  - pulse rate = Vout·I_load/E_out
  - PFM can carry at most 100 kHz·E_out/Vout

  | L | Vin (pair) | I_pk | E_out | Max PFM load | Pulse rate at 0.1 / 1 / 5 mA |
  |---|---|---|---|---|---|
  | 100 µH | 2.0 V | 34 mA | 130 nJ | 3.9 mA | 2.5 / 25 / (PWM) kHz |
  | 100 µH | 2.6 V | 44 mA | 352 nJ | 10.7 mA | 0.94 / 9.4 / 47 kHz |
  | 100 µH | 3.0 V | 51 mA | 780 nJ | 23.6 mA | 0.42 / 4.2 / 21 kHz |
  | 47 µH | 2.6 V | 94 mA | 748 nJ | 22.7 mA | 0.44 / 4.4 / 22 kHz |
  | 47 µH | 3.0 V | 109 mA | 1660 nJ | 50 mA | 0.20 / 2.0 / 9.9 kHz |

  So with fresh cells (Vin close to Vout) and a smaller inductor, the PFM region extends to tens of mA. **With L ≈ 47 µH and fresh cells, the record-mode load (≈ 50 mA, section 3) would sit near the PFM/PWM boundary.** That is the one scenario where audio-band pulse-skipping sub-harmonics could exist while recording. With L = 100 µH the record/playback load is 2–5× above the boundary (PWM, 85–115 kHz).
- Pulses in Torex PFM occur on oscillator clock edges (fixed-duty pulses, skipped cycles), so PFM ripple spectra are sub-harmonics 100 kHz/N, dithered by the load. This is inferred from the "PFM duty ratio measured on the Lx waveform" spec.
- **PWM duty and inductor ripple** (CCM, calc):
  - D = 1 − Vin/(Vout+Vd): 0.17 at 3.0 V, 0.28 at 2.6 V, 0.44 at 2.0 V.
  - ΔI_L = Vin·D/(L·f) with 100 µH: ≈ 50–90 mAp-p. That is comparable to the ≈ 70–90 mA average input current, so the converter is near the CCM/DCM boundary in record and playback.
- **Output ripple estimate per PFM pulse** (calc), with 47 µF and no ESR: ΔV = Q/C ≈ 0.8–4.6 mV. The tantalum ESR term (≈ I_pk × ESR; ≈ 34–51 mV spikes at 1 Ω) dominates, consistent in order with the datasheet's 5–10 mVp-p light-load curves for a lower-ESR cap. In PWM at 50 mA the datasheet curves give ≈ 15–30 mVp-p at 100 kHz.
- **No buck capability**: a boost converter cannot regulate below its input. Lithium AAA pairs (≈ 3.4–3.6 V open circuit) would leave the "3.3 V" rail unregulated at ≈ Vin − Vd. That is not relevant to alkaline (pair ≤ 3.2 V), which always needs boost.
- The converter holds 3.3 V down to a pair voltage of roughly 1.0–1.5 V at 50 mA (extrapolating the hold-voltage curve). **The end-of-life cutoff is therefore set by the reset detector / firmware, not by converter dropout** (see sections 2–3).

### Gaps
- The actual inductor value, output capacitor (type and ESR), switch transistor and Schottky on the RR-DR60 board (another researcher is reading the parts list).
- The XC6372's exact PFM↔PWM switching criterion (Torex does not publish a current threshold; the boundary above is derived from duty and energy).
- No published ripple or efficiency curves exist for the D (external-transistor) type at 3.3 V. Those are discontinued-part data not found online.
- Whether the converter's CE pin is driven by the microcontroller (standby with only the memory back-up), or the converter runs always.

---

## 2. XC61AN reset detector: marking decode, detect voltage, hysteresis, shutdown battery voltage

### Takeaway
The schematic label "XC61AN" identifies only the series (XC61A) and, by Torex's XC61 naming, the **N-channel open-drain output** variant. **The detect voltage is in the omitted digits** (a full part number would read like XC61AN**27**0**2**MR = 2.7 V), so it cannot be decoded from "XC61AN" alone. It needs the full part number from the parts list or the package marking. The owner's manual confirms that **the firmware declares the batteries exhausted early "to preserve the contents of the memory"**. Shutdown therefore happens at a pair voltage well above the converter's dropout, estimated ≈ 2.1–2.2 V under load from the stated 10 h recording life (section 3).

### Cited Findings
- Torex's XC61 listings: "XC61A … Low Voltage Detectors (VDF = 0.8 V)" (a 6-page datasheet); the XC61C covers "Low Voltage Detectors (VDF = 0.8 V～1.5 V) Standard Voltage Detectors (VDF 1.6 V～6.0 V)". — [alldatasheet XC61 listing (search summary)](https://www.alldatasheet.com/view.jsp?Searchword=XC61&sField=2); [alldatasheet XC61C](https://www.alldatasheet.net/datasheet-pdf/pdf/2238706/TOREX/XC61CN4502MR-G.html)
- XC61C: "N-ch open drain or CMOS" output; detect voltages 0.8–6.0 V in 0.1 V steps; ≈ 0.7 µA standby at 1.5 V. Part numbers follow XC61C**N**{VV}{x}{2}MR, e.g. XC61CN2702MR-G (a 2.7 V detector) and XC61CN2502MR (2.5 V). — [LCSC XC61CN2702MR-G](https://www.lcsc.com/product-detail/C112139.html); [LCSC XC61CN2502MR](https://www.lcsc.com/product-detail/C112138.html); [Mouser XC61x overview](https://www.mouser.co.id/new/torex-semiconductor/torex-xc61x-voltage-detectors)
- Owner's manual (S-001, OCR):
  - "The battery indicator keeps track of the life left in the battery. Replace the batteries when ['Lo' symbol, OCR'd as "UOr"] is displayed."
  - "The battery indicator is not shown while the recorder is stopped."
  - "**This unit indicates that the batteries have run down when there is actually still some life left in them. This is to preserve the contents of the memory.**"
  - "Insert fresh batteries within about three minutes of removing the old ones. If the unit is left without batteries for longer than this, all settings revert to the conditions as at time of purchase … Recordings remain unaffected."
  - Troubleshooting: "['Lo' symbol, OCR'd "U01"] appears on the display — Have the batteries run down?"

  — [RR-DR60 Operating Instructions, Internet Archive OCR](https://archive.org/stream/Panasonic_RR-DR60_Microcassette_Recorder_User_Manual/Panasonic_RR-DR60_Microcassette_Recorder_User_Manual_djvu.txt)
- The battery icon is shown only while operating, so it reads a loaded voltage. The microcontroller has analog inputs (IC4 pin 16 AN0 is the earphone plug detect per S-002). — [sources.md S-002](../../../sources.md)

### Inferences
- Decode by analogy with the XC61C numbering: XC61 **A** (series) **N** (N-ch open drain) then {detect voltage, 2 digits}{delay/option digit}{accuracy digit}{package M = SOT-23}{tape R}. "XC61AN" printed on a schematic is the generic series plus output type. The XC61A sheet being described as a "low-voltage detector" family suggests detect voltages from 0.8 V upward. Medium-low confidence: no XC61A datasheet text was obtained.
- **Two plausible roles:**
  - **Monitoring the 3.3 V rail.** A detect voltage around 2.7–3.0 V would reset the system only when the converter collapses, which happens at a very low battery (section 1: pair ≈ 1.0–1.5 V at 50 mA). The "early" battery-dead decision would then come from the microcontroller's own battery reading (an ADC or comparator), not the XC61AN.
  - **Monitoring the raw battery line.** A detect voltage around 1.8–2.2 V would directly set the shutdown point.

  Either way the manual's statement places the firmware cutoff above the electrical limit.
- The memory back-up ("built-in lithium memory back-up", S-002) together with the "about three minutes" settings retention means a supercap/lithium cell keeps the clock and settings alive. Flash recordings are non-volatile.
- **Audible consequence:** none while operating normally. The emulator-relevant effect is the *end point* of the battery trajectory: the unit stops (or refuses to record) before the speaker clip ceiling sinks below ≈ 0.4 V (section 4).

### Gaps
- The full XC61A part number and its detect voltage (parts list or package marking; the marking code on a SOT-23 would need the XC61A marking table).
- XC61A hysteresis: no XC61A datasheet text was found, and I found no reliable source for the figure (do not use the XC61C value without verification).
- Which node the XC61AN watches, and the threshold the firmware uses for "Lo" and for the stop. Measurable on a real unit with a bench supply ramped down while recording.

---

## 3. AAA alkaline and NiMH discharge, internal resistance vs state of charge and temperature, and the RR-DR60's currents over a session

### Takeaway
The manual's **≈ 10 h recording / ≈ 8 h playback on Panasonic alkaline AAA at 25 °C**, set against Energizer E92 constant-power data, implies about **≈ 200 mW from the battery while recording (≈ 80 mA at a 2.5 V pair) and ≈ 230 mW in playback (≈ 92 mA)**. That is a 3.3 V load of ≈ 48–59 mA at 80–85 % efficiency. Over a 10 h session each cell sags from ≈ 1.45 V to ≈ 1.1 V under load: the pair goes **≈ 2.95 V → 2.2 V**. Effective internal resistance grows from ≈ 0.15–0.3 Ω per fresh cell to ≈ 0.5–0.7 Ω at mid-life and more near the end. Cold adds ≈ 0.4 Ω per cell at 0 °C and ≈ 1 Ω at −20 °C at 250 mA (graph-derived). NiMH sits flat near 1.2 V per cell (pair ≈ 2.4–2.5 V) with ≈ 30–35 mΩ AC impedance.

### Cited Findings
- RR-DR60 specs:
  - "DC 3 V (two R03/LR03, AAA, UM-4 batteries)";
  - "110 mW (max.)";
  - Battery life "[When used at 25 °C (77 °F) temperature and on a flat and stable surface] … Panasonic alkaline batteries: Playback About 8 hours; Recording About 10 hours … The battery life may be less depending on the operating conditions";
  - "Long life alkaline batteries are recommended. If rechargeable batteries are to be used, rechargeable batteries manufactured by Panasonic are recommended";
  - care notes: "Do not apply heat to batteries, or internal short circuit may occur", "If this unit is not to be used for a long period of time, remove the batteries", "Remove spent batteries immediately."

  — [RR-DR60 Operating Instructions, Internet Archive OCR](https://archive.org/stream/Panasonic_RR-DR60_Microcassette_Recorder_User_Manual/Panasonic_RR-DR60_Microcassette_Recorder_User_Manual_djvu.txt)
- Energizer E92 (alkaline AAA): nominal IR **150–300 mΩ (fresh)**; operating temperature −18 to 55 °C. **Capacity to 0.8 V at 21 °C** (bar chart read from the render): ≈ 1150 mAh at 25 mA, ≈ 940 mAh at 100 mA, ≈ 670 mAh at 250 mA, ≈ 440 mAh at 500 mA. — [Energizer E92 datasheet](https://files.batteryjunction.com/frontend/files/energizer-e92bp-24.pdf); same IR on [Energizer EN92](https://files.batteryjunction.com/frontend/files/EN92.pdf)
- E92 discharge curves (read from the render, 21 °C):
  - **100 mW constant power:** ≈ 1.42 V at 0.5 h, 1.33 at 2 h, 1.26 at 4 h, 1.21 at 6 h, 1.16 at 8 h, ≈ 1.11 at 10 h, ≈ 1.05 at 11 h, 0.8 V at ≈ 11.7 h.
  - **250 mW:** to 0.8 V at ≈ 2.9 h.
  - **"Tape-Game-Digital Audio" 100 mA 1 h/day:** 1.25 V at 3 h, ≈ 1.17 at 6 h, ≈ 1.10 at 8 h, ≈ 1.0 at 9.7 h, 0.8 V at ≈ 11 h.
  - **250 mA continuous at 21 / 0 / −20 °C:** ≈ 2.6 / 1.3 / 0.55 h to 0.8 V. Early-life plateau ≈ 1.3 V at 21 °C, ≈ 1.2 V at 0 °C, ≈ 1.05 V at −20 °C.

  — [Energizer E92 datasheet p. 2](https://files.batteryjunction.com/frontend/files/energizer-e92bp-24.pdf)
- Energizer Alkaline Handbook:
  - "The sloping discharge in alkaline batteries is primarily due to the increase in battery internal resistance due to reaction byproducts forming on the electrode surfaces and decrease availability of the fuels (i.e., water)."
  - "Cold temperatures and their subsequent slowing of the chemical kinetics reactions impact the internal resistance (Ri) of the cell. High drain rates in cold environments will cause a large voltage drop due to the higher battery Ri."
  - "No capacity is actually lost due to cold temperatures."
  - Electronic resistance acts "within the first few milliseconds"; ionic (polarization) resistance "starting a few milliseconds or more after" load; "the 1000 Hz impedance value will be less than the total effective resistance".
  - Dual-pulse example: 5 mA background + 505 mA pulse for ≈ 100 ms, 1.485 → 1.378 V, **0.214 Ω**.

  — [Energizer Alkaline Handbook](https://data.energizer.com/pdfs/alkaline_appman.pdf)
- A Wikipedia summary citing Energizer gives a fresh AA (E91) at ≈ 0.9 Ω at −40 °C, ≈ 0.15 Ω at room temperature and ≈ 0.1 Ω at 40 °C (AA, not AAA; secondary). — [Wikipedia, Internal resistance (search summary)](https://en.wikipedia.org/wiki/Internal_resistance)
- Panasonic NiMH AAA (modern):
  - HHR-70AAA: 1.2 V nominal, 730 mAh typ / 700 min, **≈ 30 mΩ internal impedance at 1 kHz, charged**; curves at 0/20/40 °C, capacity at 0.2 It (140 mA).
  - HHR-60AAAH: 550 mAh typ / 500 min, ≈ 35 mΩ.

  — [Panasonic HHR-70AAA datasheet (search summary)](https://cellpacksolutions.co.uk/wp-content/uploads/2016/07/HHR-70AAAB8.pdf); [Panasonic HHR-60AAAH datasheet (search summary)](https://mm.digikey.com/Volume0/opasdata/d220001/medias/docus/7181/HHR-60AAAHB2-Datasheet.pdf)

### Inferences
- **Average power and current** (calc). From the 100 mW curve (≈ 9.8 h to 1.1 V per cell) and the slope of the E92 constant-power service (100 mW → 11.5 h, 250 mW → 2.9 h, so t ∝ P^−1.5):
  - 10 h recording → ≈ 99 mW per cell, **≈ 200 mW total, ≈ 79 mA at a 2.5 V pair**.
  - 8 h playback → ≈ 114 mW per cell, **≈ 230 mW, ≈ 92 mA**.
  - 3.3 V load at η = 0.80–0.85: **≈ 48–51 mA (record), 56–59 mA (playback)**.
  - Cross-check with E92 capacity at 100 mA: 940 mAh / 10 h = 94 mA.

  1998 Panasonic alkaline AAA capacities were probably below the modern E92, which would lower these currents by perhaps 10–25 % (unverified). Playback's extra ≈ 30 mW is consistent with the NJM2113's 2.7 mA quiescent current at ≈ 2.6 V (≈ 7 mW) plus speech-level speaker drive.
- **Implied cutoff:** reaching "about 10 h" on the 100 mW curve requires a cutoff near **1.05–1.1 V per cell under load (pair ≈ 2.1–2.2 V)**. That is consistent with the manual's "still some life left". The converter itself could run to ≈ 0.5–0.75 V per cell.
- **Battery trajectory over a recording session** (pair under ≈ 80 mA load; from the 100 mW curve):

  | t (h) | 0 | 1 | 2 | 4 | 6 | 8 | 10 | 11 |
  |---|---|---|---|---|---|---|---|---|
  | Pair (V) | 2.96 | 2.76 | 2.66 | 2.52 | 2.42 | 2.32 | 2.22 | 2.10 |

  Playback runs the same curve ≈ 20 % faster.
- **Effective internal resistance vs state of charge** (graph-derived, includes polarization and the rest-recovery difference between the intermittent 100 mA test and the continuous 250 mA test; indicative only):
  - R ≈ ΔV/ΔI between the 100 mA and 250 mA curves at equal delivered charge.
  - At 300 mAh: (1.25 − 1.18)/0.15 ≈ **0.47 Ω per cell**.
  - At 600 mAh: (1.17 − 1.06)/0.15 ≈ **0.73 Ω per cell**.
  - Fresh: 0.15–0.3 Ω (datasheet 1 kHz-type figure); 0.21 Ω (handbook pulse example, AA).
  - Expect > 1 Ω per cell in the last 10–20 %.
- **Cold** (graph-derived, 250 mA): plateau drop vs 21 °C ≈ 0.10 V at 0 °C (≈ +0.4 Ω per cell) and ≈ 0.25 V at −20 °C (≈ +1 Ω per cell). At the RR-DR60's ≈ 80–90 mA the voltage penalty is about a third of that: ≈ 0.04 V per cell at 0 °C, ≈ 0.09 V at −20 °C, from the IR term alone.
- **NiMH:** pair ≈ 2.4–2.5 V for most of the discharge (flat), 2 × 30–35 mΩ AC impedance, so dynamic sag is small. Run time on 1990s 400–550 mAh AAA cells would be ≈ 5–7 h recording (550 / 80 ≈ 6.9 h); on modern 730 mAh cells ≈ 9 h. The NiMH knee (≈ 1.0–1.1 V per cell) falls near the inferred firmware cutoff.

### Gaps
- No AAA-specific published curve of internal resistance vs depth of discharge or temperature was found. The figures above are derived from voltage curves.
- Discharge data for 1998-era Panasonic alkaline AAA (the manual's reference cell).
- The RR-DR60's actual stop/standby current (it sets shelf drain and whether PFM ripple is present in stop).
- The real current profile in record and playback (pulsed flash writes, DSP activity per frame), which matters for section 5. It is measurable with a current probe on a unit.

---

## 4. NJM2113 speaker amplifier: clip ceiling vs battery state, supply sag during loud passages

### Takeaway
With the datasheet swing limits (VOH = V+ − 1.1 V, VOL = 0.21 V at 75 mA) around a quiescent output level of ≈ V+/2 − 0.3 V, the BTL peak ceiling is **≈ V+ − 1.6 V**. That is project A-031's formula, giving 1.2 V at the sheet's 2.8 V. Across an alkaline discharge (pair 2.96 → 2.2 V, V+ ≈ pair − 0.2 V), the ceiling falls **≈ 1.16 → 0.42 V (≈ +0.1 → −8.7 dB re the model's 1.15 V)** by 10 h. That is a wider range than A-031's registered "1.0–1.4 V over the cells' state". On top of the slow trend, a loud passage drawing ≈ 95–150 mA from cells of 0.8–3.2 Ω total sags V+ by ≈ 0.08–0.48 V, so the ceiling is also **signal-dependent at syllabic rate**. C20 22 µF only smooths above ≈ 2–9 kHz.

### Cited Findings
- NJM2113 datasheet (V+ = 6 V unless stated):
  - operating 2–16 V;
  - ICC1 2.7 mA typ (4.0 max) at V+ = 3.0 V;
  - power-down 72 µA typ at 3 V;
  - PO1 ≥ 55 mW at V+ = 3.0 V, RL = 16 Ω, THD ≤ 10 %;
  - THD2 0.5 % at V+ ≥ 3 V, 8 Ω, 20 mW, GVD = 12 dB;
  - output DC level VO1 1.00 / 1.18 / 1.25 V at V+ = 3.0 V (2.68 V at 6 V, 5.71 V at 12 V);
  - **VOH = V+ − 1.1 V at IOUT = −75 mA, V+ = 2.0–16 V; VOL = 0.21 V at 75 mA**;
  - output peak current ±250 mA (absolute maximum).

  The operating-current curve shows the IC turning on near V+ ≈ 1.6–2 V. — [JRC NJM2113 datasheet Ver. 2014-03-17](https://www.endrich.com/Datenbl%C3%A4tter/Aktive%20Komponenten/NJM2113_E.pdf)
- In the RR-DR60, the amplifier is fed from the raw battery line through Q9 (2SB1295) with C20 22 µF. The sheet marks VDD 2.8 V in record mode, with the amp in standby (pin 1 at 2.1 V). The project's adopted ceiling is 1.15 V peak differential at 2.8 V (1.0–1.4 V registered). — [sources.md S-002/S-006](../../../sources.md); [assumptions.md A-031](../../../assumptions.md)
- Rated output: "Power output 110 mW (max.)", speaker 8 Ω. — [RR-DR60 Operating Instructions OCR](https://archive.org/stream/Panasonic_RR-DR60_Microcassette_Recorder_User_Manual/Panasonic_RR-DR60_Microcassette_Recorder_User_Manual_djvu.txt)
- Battery IR and its rise with discharge and cold: section 3. — [Energizer Alkaline Handbook](https://data.energizer.com/pdfs/alkaline_appman.pdf); [Energizer E92](https://files.batteryjunction.com/frontend/files/energizer-e92bp-24.pdf)

### Inferences
- **Ceiling formula:**
  - Per output pin, headroom up = VOH − VQ = (V+ − 1.1) − (V+/2 − 0.3) = V+/2 − 0.8.
  - Headroom down = VQ − VOL = V+/2 − 0.51 (larger).
  - BTL peak differential = 2 × (V+/2 − 0.8) = **V+ − 1.6 V** (at 75 mA; into 8 Ω at 1.2 V peak the current is 150 mA, so the real ceiling is lower still).
- **Ceiling vs session time** (calc; V+ = pair − 0.2 V, taking the sheet's 2.8 V at an assumed 3.0 V service-manual bench supply):

  | t (h) | Pair (V) | V+ (V) | Ceiling (V pk diff) | dB re 1.15 V |
  |---|---|---|---|---|
  | 0 | 2.96 | 2.76 | 1.16 | +0.1 |
  | 1 | 2.76 | 2.56 | 0.96 | −1.6 |
  | 2 | 2.66 | 2.46 | 0.86 | −2.5 |
  | 4 | 2.52 | 2.32 | 0.72 | −4.1 |
  | 6 | 2.42 | 2.22 | 0.62 | −5.4 |
  | 8 | 2.32 | 2.12 | 0.52 | −6.9 |
  | 10 | 2.22 | 2.02 | 0.42 | −8.7 |
  | 11 | 2.10 | 1.90 | 0.30 | −11.7 |

  **Discrepancy for the writer:** A-031 states "over a pair of AAA cells (3.1 → 2.4 V) … the ceiling ranges 1.0–1.4 V". By A-031's own V+ − 1.6 formula, 3.1 → 2.4 V gives ≈ 1.5 → 0.8 V without the Q9 drop, or 1.3 → 0.6 V with it. The registered range appears to cover only the first ≈ 1–2 h of a discharge.
- **NiMH:** pair ≈ 2.4–2.5 V gives a ceiling ≈ 0.6–0.7 V for most of the discharge, ≈ 4–5 dB below fresh alkaline from the start, but steadier.
- **Dynamic sag:**
  - A BTL amp draws supply current ≈ |V_diff|/RL. A sine at a 1.2 V peak into 8 Ω draws 150 mA peak, ≈ 95 mA average (2/π).
  - Sag ΔV+ = I × R_total, with R_total = 2 cells + Q9 + contacts (assumed +0.2 Ω):

  | Battery state | R_total | Sag at 95 mA avg | Sag at 150 mA pk |
  |---|---|---|---|
  | Fresh | 0.8 Ω | 76 mV | 120 mV |
  | Mid-life or fresh at 0 °C | 1.6 Ω | 152 mV | 240 mV |
  | Late | 3.2 Ω | 304 mV | 480 mV |

  Since the ceiling moves one-for-one with V+, a loud passage lowers its own clip point by 0.1–0.5 V: ≈ −0.9 dB fresh, ≈ −2.5 dB mid-life, several dB late, where it is self-limiting.
  - **Time scale:** C20 22 µF with R_total forms a low-pass at 1/(2π·R·C) ≈ 9.0 / 4.5 / 2.3 kHz for 0.8 / 1.6 / 3.2 Ω. Waveform-rate current peaks above those frequencies are partly smoothed, but the speech-envelope (syllabic, ≈ 2–20 Hz) component passes fully. Alkaline polarization adds a slower component (ms and longer, per the handbook), so sag deepens over a sustained loud passage.
  - The converter's own input current (≈ 70–90 mA, constant-power, rising as V falls) shares the same cell resistance. That raises the baseline drop by ≈ 60–290 mV over the session.
- **Audible character:** at reference volume (full scale meets the ceiling, A-043), playback on weak cells clips earlier and asymmetrically in time (envelope-modulated clipping: louder syllables lower their own ceiling). This matches the generic "weak batteries → distorted loud playback" pattern (section 7). The record path is unaffected because the codec and mic amp run on the regulated 3.3 / 3.2 V rails.
- The earphone route (Q14 follower on the 3.2 V regulated analog rail, amp shut down) should be essentially immune to battery state.

### Gaps
- Q9 (2SB1295) VCE(sat) at 50–150 mA, and the true series resistance from the cells to pin 6 (contacts, any fuse or inductor). The 0.2 V offset is inferred from the 2.8 V mark at an assumed 3.0 V bench supply.
- The NJM2113's swing limits at 150 mA into 8 Ω (the datasheet specifies 75 mA only) and its quiescent output level vs V+ below 3 V.
- Whether Q9 stays on in stop mode (it appears on in record mode, since VDD is marked 2.8 V there).

---

## 5. Supply ripple / noise coupling into the audio paths

### Takeaway
Four paths matter:
- **(a) Converter PWM ripple (≈ 15–30 mVp-p at 85–115 kHz) into the codec and the analog rail.** The OKI MSM77xx family's power-supply noise rejection is only ≈ 30 dB typ (0–50 kHz). Supply-coupled ripple at ≈ −69 dBFS could alias into the 6 kHz band *only if* the oscillator sits within ±3 kHz of a multiple of the codec's internal sampling clock. That is a unit-dependent, temperature-dependent, faint wandering tone, low confidence.
- **(b) Battery-line ripple and frame-rate load modulation into the NJM2113.** Its PSRR with C1 = 1 µF and C2 = 0 is only ≈ 27–30 dB below 1 kHz, so supply noise reaches the speaker **after the volume control**: a volume-independent floor estimated at −70 … −50 dB re the clip level.
- **(c) The mic bias,** filtered by a 0.19 Hz RC (−49 dB at 50 Hz, −75 dB at 1 kHz, ideal).
- **(d) PFM sub-harmonics** in the audio band, expected only at light load (not while recording), unless the inductor is small and the cells fresh (section 1).

### Cited Findings
- NJM2113 PSRR (V+ = 6 V, ΔV+ = 3 V):
  - PSRR1 ≥ 50 dB DC (C1 = ∞, C2 = 0.01 µF);
  - **PSRR2 12 dB typ at 1 kHz (C1 = 0.1 µF, C2 = 0)**;
  - **PSRR3 52 dB typ at 1 kHz (C1 = 1.0 µF, C2 = 5.0 µF)**;
  - mute attenuation 70 dB.

  Note 2: "C1 and C2 improve power supply rejection ratio. In case of C1 is enough large, C2 is unnecessary." PSRR-vs-frequency curves (read from the render):
  - **C2 = 0, C1 = 1 µF: ≈ 27–28 dB at 20–300 Hz, ≈ 30 dB at 1 kHz, ≈ 40 dB at 5–10 kHz, ≈ 37 dB at 20 kHz.**
  - C2 = 0, C1 = 5 µF: ≈ 40–47 dB.
  - C2 = 1 µF, C1 = 1 µF: ≈ 38 dB at 20 Hz rising to ≈ 50 dB at 2 kHz.
  - C2 = 1 µF, C1 = 0: ≈ 11 dB below 1 kHz with a resonance peak ≈ 35 dB near 3 kHz.

  — [JRC NJM2113 datasheet](https://www.endrich.com/Datenbl%C3%A4tter/Aktive%20Komponenten/NJM2113_E.pdf)
- The NJM2113 is pin- and specification-equivalent to the Motorola MC34119: identical PSRR test conditions and RFC1 100/150/220 kΩ, RFC2 18/25/40 kΩ, matching NJM2113's R+IN and RREF. MC34119 pin descriptions:
  - **"FC1 (pin 3) Analog ground for the amplifiers. A 1.0 µF capacitor at this pin (with a 5.0 µF capacitor at Pin 2) provides (typically) 52 dB of power supply rejection."**
  - "FC2 (pin 2) A capacitor at this pin increases power supply rejection … can be left open if the capacitor at FC1 is sufficient."
  - "C2 is somewhat dominant at low frequencies, while C1 is dominant at high frequencies."

  — [Motorola MC34119 datasheet](https://rigpix.com/components/mc34119.pdf)
- RR-DR60: NJM2113 pin 3 has C13 1 µF (1 V); V+ from the battery via Q9 with C20 22 µF. — [sources.md S-002](../../../sources.md)
- OKI MSM7717 (same family as the RR-DR60's MSM7702; 2.7–3.8 V single rail):
  - **"Power Supply Noise Rejection Ratio PSR T / PSR R: 0 to 50 kHz, 50 mVpp, typ 30 dB" (measured under idle channel noise)**;
  - idle channel noise transmit −72.5 / −70.5 / −68 dBm0p, receive −76.5 dBm0p (psophometric);
  - "SG: Signal ground voltage output. The output voltage is 1/2 of the power supply voltage";
  - "SGC: Used to generate the signal ground voltage level by connecting a bypass capacitor";
  - "This output voltage level is undefined during power-saving or power-down mode";
  - notes on use: "Use a low noise (particularly, low level type of high frequency spike noise or pulse noise) power supply to avoid erroneous operation and the degradation of the characteristics."

  The PSR row was parsed from text extraction of a table; column alignment is medium-confidence. — [OKI MSM7717 datasheet](http://igorkov.org/revo/datasheets/M7717.pdf)
- XC6372 ripple ≈ 15–30 mVp-p at 30–100 mA, oscillator 85–115 kHz (section 1). — [Torex XC6371/XC6372 datasheet](https://uploadcdn.oneyac.com/attachments/files/05/FC/XC6371A501PR-G.pdf)
- Mic bias network Q11 / R4 39 kΩ / C2 22 µF → ≈ 2.7 V; codec full scale +3 dBm0 = 0.7 V peak at GSX (A-030); hiss ≈ −62 dBFS in 300–3000 Hz (A-048); the SG reference noise is unknown (A-050). — [assumptions.md](../../../assumptions.md)

### Inferences
- **(a) Codec aliasing of converter ripple** (calc):
  - 20 mVp-p triangle ≈ 5.8 mV rms; × 30 dB PSR → 0.18 mV rms.
  - Re a full-scale sine of 0.7 V peak (0.495 V rms): **≈ −69 dBFS** equivalent.
  - The final 6 kHz sampling folds 85 / 100 / 115 kHz to 1 / 2 / 1 kHz. Across the ±15 % tolerance the alias can land anywhere in 0–3 kHz.
  - But the codec's switched-capacitor filters run at an internal rate f_int > 6 kHz. Ripple first folds modulo f_int and is removed by the SC low-pass unless it lands within ≈ ±3 kHz of a multiple of f_int. For an illustrative f_int = 48 kHz, only oscillator frequencies of 93–99 kHz alias in-band: ≈ 20 % of the 85–115 kHz spread, if uniformly distributed.
  - When it happens, it is a narrow tone ≈ 17 dB above the hiss's per-10-Hz-bin density (hiss −62 dBFS over 2.7 kHz ≈ −86 dBFS per 10 Hz). It drifts with temperature and battery voltage because the converter's RC oscillator is not locked to the codec PLL.
  - The 4 kbit/s CELP coder would probably not preserve a −69 dBFS tone faithfully (its silence floor is −78 dBFS and below in the project's model).
  - Low confidence throughout: the PSR at 100 kHz is not specified (the spec stops at 50 kHz), and the MSM7702's f_int is unknown.
- **(a′) Signal-path entry:** ripple on the 3.2 V rail entering the mic amp is attenuated by the C10/R15 roll-off (12.9 kHz, first order: ≈ −18 dB at 100 kHz) and by the mic amp's structure. Q2's emitter and R6's top both sit on the rail, so rail ripple is common to Q2's base and emitter (to first order). The ≈ 460 loop gain suppresses what remains at audio frequencies. Expected to be smaller than the codec's own supply coupling (inference).
- **(a″) SG/AIN+ reference:** SG = VDD/2, so 50 % of VDD ripple appears on SG unless SGC filtering removes it. The mic amp's Q3 base is tied to the AIN+ reference (S-002), so reference ripple enters the record path at the amplifier's gain to that node, on top of A-050's unknown SG noise.
- **(b) Speaker path:** the NJM2113's supply is the raw battery line, which carries:
  - (i) the converter's input-current ripple at 100 kHz: ΔI_L ≈ 50–90 mAp-p × (battery R ∥ input-cap reactance). ≈ 2.5 mVp-p with a 47 µF input cap (X ≈ 0.034 Ω at 100 kHz); up to ≈ 44 mVp-p into 0.6 Ω without one. Inaudible as such, beyond the speaker's band and PSRR ≈ 37 dB at 20 kHz.
  - (ii) **load modulation from the digital section**: flash reads, DSP frame processing, the 20 ms-class codec frames → 50 Hz-class repetition and harmonics; amplitude unknown.

  Illustrative calc: a 10–20 mA load step × 0.6–3 Ω → 6–60 mV on the battery line; × PSRR ≈ 27 dB (C1 = 1 µF, C2 = 0, below 300 Hz) → **0.27–2.7 mV at the speaker**, i.e. **−70 to −50 dB re the 1.2 V-peak clip level (0.85 V rms)**. Because the amplifier follows the volume control, this floor **does not scale with the volume position**, so it is most noticeable at low volume and in silences. It would rise as the cells age (higher R). This is the classic "digital buzz" mechanism of battery-powered speaker amps. Medium-low confidence pending a capture.
  - If the RR-DR60 has a capacitor on NJM2113 pin 2 (C2), PSRR rises to ≈ 38–50 dB and this floor drops by ≈ 10–20 dB.
- **(c) Mic bias:**
  - The R4/C2 corner is 1/(2π·0.858 s) = 0.185 Hz.
  - Ideal attenuation −48.6 dB at 50 Hz, −74.6 dB at 1 kHz, −115 dB at 100 kHz.
  - In practice limited by Q11's C_cb and layout to perhaps 40–60 dB at 100 kHz (inference).
  - The electret's own supply rejection (JFET drain as a current source) adds more. So the capsule bias is not a meaningful entry point.
- **(d) PFM in the audio band:** light-load pulse rates of 0.4–25 kHz (section 1). Relevant only in stop or standby, or during a VAS pause if the DSP load drops enough. A VAS pause in drop mode records nothing, so no artifact in the stored audio. Exception: a small-inductor / fresh-cell build near the boundary during recording.
- **Earphone route:** Q14's collector and the base bias (R64 33 kΩ) are on the regulated 3.2 V rail. The follower's PSRR from its base divider is roughly the base node's ripple fraction (no extra filter shown). Rail ripple is mostly 100 kHz PWM, out of band.

### Gaps
- MSM7702's own PSR figure, its internal SC sampling rate and SGC capacitor value (S-003 pages 4–17 unread; the MSM7717 family figure is the proxy).
- The NJM2113 pin 2 capacitor in the RR-DR60 (another researcher, schematic).
- Converter input/output capacitor values and ESR, and the inductor.
- Any measured supply-current profile of the DSP/flash in record and playback. Without it, path (b)'s amplitude is a placeholder range, not a figure.
- No 1990s voice-recorder teardown or application note quantifying converter ripple in recordings was found.

---

## 6. Power-up / record-start / playback-start transients ("thump")

### Takeaway
Time constants:
- converter soft-start **4–20 ms** (10 typ);
- mic bias **τ = R4·C2 = 0.858 s** (90 / 95 / 99 % at 1.98 / 2.57 / 3.95 s);
- mic-amp output coupling **τ ≈ 2.1 ms** (77 Hz);
- NJM2113 analog ground **τ ≈ 0.10–0.22 s** (C13 1 µF × 100–220 kΩ);
- MC34119-class CD un-mute **12–15 ms**;
- playback coupling C14 × VR1 ≈ 5 ms, C8 × R16 ≈ 1 ms.

If the analog rail and bias start at record start, the slow mic-bias ramp alone gives only ≈ −65 dBFS at the codec (benign). But the capsule JFET is under-biased for the first ≈ 0.56 s (reduced or distorted sensitivity), and **the codec's SG/AIN+ reference startup could give a large transient** (order −10 dBFS for a 50 ms ramp; the SGC value is unknown). That transient could also arm the limiter. Whether these occur depends on the rail sequencing (open, A-037) and on firmware blanking, for which no source was found.

### Cited Findings
- XC6371/XC6372 slow-start time 4.0 / 10.0 / 20.0 ms; soft-start "prevents voltage overshoot". — [Torex XC6371/XC6372 datasheet](https://uploadcdn.oneyac.com/attachments/files/05/FC/XC6371A501PR-G.pdf)
- NJM2113 note 3: "C1 and C2 make slow power rise up to the NJM2113 regardless the external power supply condition." R+IN (pin 3) 100/150/220 kΩ, RREF (pin 2) 18/25/40 kΩ. — [JRC NJM2113 datasheet](https://www.endrich.com/Datenbl%C3%A4tter/Aktive%20Komponenten/NJM2113_E.pdf)
- MC34119 (equivalent part):
  - "C1 and C2 also affect the turn-on time of the circuit at power-up, since the two capacitors must charge up through the internal 50 k and 125 kΩ resistors … The turn-on time is ≈ 60 % longer for VCC = 3.0 V";
  - Figure 1 gives turn-on time vs C1, C2 (axis to 360 ms);
  - "The turn-off time of the audio output, from the application of the CD signal, is < 2.0 µs, and turn on-time is 12 ms–15 ms. Both times are independent of C1, C2, and VCC";
  - "When the MC34119 is disabled, the voltages at FC1 and FC2 do not change as they are powered from VCC. The outputs … change to a high impedance condition."

  — [Motorola MC34119 datasheet](https://rigpix.com/components/mc34119.pdf)
- MSM7717: SG = VDD/2, generated with a bypass capacitor on SGC; "undefined during power-saving or power-down mode". — [OKI MSM7717 datasheet](http://igorkov.org/revo/datasheets/M7717.pdf)
- The RR-DR60 record chain:
  - capsule biased via R2 1 kΩ from the filtered 2.7 V rail (Q11, R4 39 kΩ, C2 22 µF);
  - C1 0.15 µF into the transimpedance amp (R3 100 kΩ feedback);
  - C4 0.1 µF / R7 22 kΩ / C5 1 µF output coupling (≈ 77 Hz, A-047);
  - Q3's base held at the codec's AIN+ reference (1.6 V);
  - full scale ≈ 0.196 V peak at the mic-amp output (A-029).

  A-037 records that S-002 lists voltages only for record and playback, and suggests capturing "the first 0.5 s of several items for a rail power-up thump (C2 22 µF via R4 39 kΩ, τ ≈ 0.9 s) or a firmware blanking delay". — [sources.md S-002](../../../sources.md); [assumptions.md A-029, A-037](../../../assumptions.md)
- The NJM2113 is in standby (CD at 2.1 V) during recording, with VDD present (2.8 V marked in record mode). — [sources.md S-002](../../../sources.md)

### Inferences
- **Mic-bias ramp through the record path** (calc; assuming the capsule JFET acts as a constant current once saturated, so the mic node follows the bias):
  - dV/dt(0) = 2.7 V / 0.858 s = 3.15 V/s.
  - Current through C1 into the virtual ground = C1·dV/dt = 0.47 µA.
  - TIA output = R3 × i = **47 mV**, decaying with τ = 0.858 s.
  - The 77 Hz coupling high-pass scales a slow exponential by τ_hp/τ = 2.07 ms / 858 ms → **0.11 mV ≈ −65 dBFS**: a sub-audio drift, negligible against the hiss.
- **Capsule under-bias:** with I_D·R2 ≈ 0.3 V and a JFET needing V_DS ≳ 1 V to be well saturated (assumed), the bias reaches 1.3 V at t = −τ·ln(1 − 1.3/2.7) ≈ **0.56 s**. If the bias rail starts at record start, the first ≈ 0.3–0.6 s could show reduced sensitivity and some distortion on loud input. If the rail stays up while stopped, nothing happens.
- **Rail and reference step:** if Q20 switches the 3.2 V rail on at record start, the mic amp's output moves 0 → 1.7 V and the codec's SG/AIN+ 0 → 1.6 V.
  - A fast edge through the 2.1 ms coupling would be +19 dB over full scale (1.7 V vs 0.196 V). The codec's differential input (AIN− vs SG) cancels part of the common-mode move.
  - With an SG ramp time constant T, the high-passed residue scales ≈ 1.6 V × 2.1 ms / T, e.g. 67 mV ≈ −9 dBFS for T = 50 ms (illustrative; SGC unknown).
  - A transient above the limiter's knee and longer than its dead time would charge C6 (10 s hold, ≈ 1.4 s release, spec 005). The start of the recording would then be ducked as well as thumped. A-037's capture plan would detect both.
- **Converter start** (only if the 3.3 V rail itself is switched): the 4–20 ms soft-start is shorter than the DSP's boot. Any glitch precedes the first codec frame, so it is unlikely to reach a recording.
- **Playback start:**
  - The NJM2113's FC1/FC2 stay charged while it is disabled (MC34119 behavior), so un-muting via CD is a 12–15 ms turn-on and is normally click-free.
  - If the codec leaves power-down at playback start, AOUT's DC (SG ≈ 1.6 V) charges C14 through VR1 (τ ≈ 5 ms at the top) and then C8 through R16 (τ ≈ 1 ms). If the firmware un-mutes before this settles, the step's high-passed residue (× p × 13.6 gain) is a click whose size depends on the volume position (inference).
  - If Q9 also switches V+ on at playback start, the C13 charge (τ 0.10–0.22 s; MC34119 ≈ 60 % longer at 3 V) delays sound by ≈ 0.2–0.4 s; the outputs ramp in common mode, so a BTL pop arises only from mismatch.
- **Firmware masking:** 1990s DSP recorders plausibly blank or mute the first frames after power-up, or start the coder only after the analog rail settles. The RR-DR60's DSP would need ≈ 1–4 s to cover the mic-bias settling, which seems long for a record button. This suggests either the rail stays up in stop mode or the unit accepts a short settling period. Speculative.

### Gaps
- Rail sequencing: whether Q20 / the 3.2 V rail, Q11's bias, Q9 and the codec power-down change at record or play start or at power-on (another researcher; A-037).
- The MSM7702 SGC capacitor value and SG start-up time (S-003 pages 4–17).
- MC34119 Figure 1 values for C1 = 1 µF with no C2 (curve not digitized).
- No source was found on RR-DR60 or comparable-recorder firmware blanking at record start. It is measurable with A-037's capture plan, or by recording silence with the capsule disconnected and inspecting the first second of several items.

---

## 7. User-reported battery artifacts in cheap voice recorders (anecdotal)

### Takeaway
I found no RR-DR60-specific reports and no reliable published account linking weak batteries to specific audible artifacts in 1990s IC recorders. The anecdotes found are generic, from other devices:
- distorted loud playback when the supply cannot deliver the current;
- an unverified claim of "whine when battery is low";
- whine on battery power attributed to grounding or a defect.

They are consistent with section 4's clip-ceiling and sag mechanism, but they are not evidence for this unit.

### Cited Findings
- ISD1820 voice-recorder board (anecdotal): playback badly distorted on a 3 V CR2032, clean on 5 V USB. A forum expert: "The CR2032 battery is too small to produce 1/2W of audio power" and the sound "will be very distorted if you record it and/or play it back louder than the amplifier can produce". — [All About Circuits forum thread](https://forum.allaboutcircuits.com/threads/isd1820-distortion-put-sound-on-device.174133/post-1568401)
- Tascam DR-05X (anecdotal): high-pitched whine while recording on battery; advice pointed to grounding or a defective unit, not battery charge. — [Tascam forums](https://www.tascamforums.com/threads/dr-05x-whine-when-recording.7987)
- MPC500 sampler (anecdotal, unverified by the poster): a user had read that a whine occurs with low battery charge. — [MPC-Forums](https://www.mpc-forums.com/viewtopic.php?p=1598841)
- Sony IC recorder help lists playback noise, buzzing/humming and short battery life as separate symptoms without linking them. — [Sony ICD help guide](https://helpguide.sony.net/icd/t65/v1/en1/contents/contentslist02.html)

### Inferences
- For the RR-DR60, the mechanisms that would make battery state audible are:
  - (1) **speaker-route clipping that worsens over the discharge** (−9 dB ceiling by 10 h) **and modulates with loudness** (sag);
  - (2) a **volume-independent supply-coupled floor** in the speaker that grows with battery resistance;
  - (3) **cold-weather** amplification of both, through the internal-resistance rise.
- The **recorded** audio should be largely immune to battery state, because the record path runs on the regulated 3.3 / 3.2 V rails. The candidate exceptions are the narrow converter-alias tone (unit-dependent) and the start-of-recording transients (sequencing-dependent).
- "Motorboating" (a low-frequency relaxation oscillation through a shared supply impedance) is conceivable on the battery line when the NJM2113's current modulates the converter input at high cell resistance, but no source or calculation here supports it occurring.

### Gaps
- No user reports specific to the RR-DR60's sound on weak batteries were found (EVP-community forums were not searched in depth).
- No measurement data from any 1990s IC recorder on supply-related artifacts. The decisive evidence would be a real unit:
  - a speaker-output capture of a full-scale tone at fixed volume on a bench supply swept 3.2 → 2.0 V with series resistance 0.5–3 Ω;
  - a silent-input recording at several supply voltages, to look for an alias tone;
  - the first second of several items, for start transients.
