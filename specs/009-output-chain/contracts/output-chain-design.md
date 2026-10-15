# Contract: Output-chain design constants (feature 009; research R-02 – R-04, R-08)

The design script `tools/output-chain/design_output_chain.py` takes the circuit figures below as inputs, derives every constant, verifies every FR-005 – FR-008 bound analytically (`--check`, non-zero exit on failure) and writes `crates/rr_dr60/src/stages/output_coeffs.rs` as exact `f64` bit patterns (`--write`; byte-identical on regeneration). A core unit test pins the script's printed figures; the Rust never pins its own output.

## Inputs (the circuit as read from S-002 sheet 3.1 at plan time; S-006; A-031, A-032, A-030)

| Input | Value | Note |
|---|---|---|
| Rated output power | 55 mW into 16 Ω at 10 % THD at V+ = 3 V | S-006; S-002's 110 mW into 8 Ω agrees (0.938 V RMS, 1.33 V peak at 3.0 V); IC9 runs from the battery rail, VDD read **2.8 V** |
| Clip ceiling `V_CLIP` | **1.15 V peak differential nominal** at the 2.8 V rail the schematic marks (S-002) (each pin's swing is bounded by V+ − 1.1 V above and 0.21 V below about a quiescent level of ≈ V+/2 − 0.3 V, so the hard ceiling is ≈ V+ − 1.6 V = 1.2 V and the 10 % THD point ≈ 1.15 V); range **1.0 – 1.4 V** with the cells' state (3.1 → 2.4 V over a pair of AAA cells) and the quiescent spread; 1.33 V is the datasheet's 3.0 V figure | A-031, medium; one constant, re-derived before the golden is blessed if S-003's AOUT level or a capture moves it |
| Gain resistors | Rf = R18 68 kΩ (pin 5 → pin 4); Ri = R16 10 kΩ (series input) | `AMP_GAIN = 2 Rf / Ri = 13.6` (22.67 dB) **at position 1**; the volume control's wiper resistance `R_W(p) = 50 kΩ · p (1 − p)` adds to Ri, so the **loaded law** is `g_spk(p) = p · 2 Rf / (Ri + R_W(p))` in volts per volt of codec output (A-044: 9.6 dB at mid travel against the ideal 16.7); R19 220 kΩ + C16 10 pF is the microcontroller's beep injection, not the gain network |
| Codec full scale | 1.0 V peak at the codec output | A-030 receive +3 dBm0 |
| Volume-control input coupling | C14 0.1 µF into VR1 50 kΩ | `F_VOL = 1 / (2π · 0.1 µF · 50 kΩ)` = 31.8 Hz, both routes |
| Amplifier input coupling | C8 0.1 µF into R16 10 kΩ (the inverting input is a virtual ground) | `F_AMP` = 159.2 Hz, **kept fixed**: the wiper resistance would lower it to ≈ 70 Hz at mid travel, under 1 dB at 300 Hz (A-044 registers the simplification) |
| Follower transistor | Q14 is a 2SD1819A**S** (the sheet's rank suffix): hFE **370 nominal, 290 – 460** at a few mA (second-source datasheets, a lead for the sources register) | A-032 |
| Earphone input coupling | C8 + C60 in series (0.05 µF) into R16 + R65 (20 kΩ) + the follower's loaded input impedance (R64 33 kΩ ∥ hFE × the emitter load, which differs below and above the output corner) | `F_EAR_IN` **120 Hz nominal, 65 – 130 Hz** (115 – 126 Hz over the rank above the output corner; 67 Hz below it) |
| Earphone output coupling | C61 10 µF into the 16 Ω load plus the follower's output resistance (re ≈ 6 Ω + the ≈ 12 kΩ source impedance ÷ hFE ≈ 40 Ω) | `F_EAR_OUT` **280 Hz nominal, 240 – 320 Hz** over the rank |
| Earphone branch loading of the volume control | the branch's input impedance `Z_B` ≈ 24 kΩ (low-frequency value, R64 ∥ hFE × 470 Ω) in series with R16 + R65 | the earphone route's loaded law `g_ear(p) = p · (20 kΩ + Z_B) / (20 kΩ + Z_B + R_W(p))` (unity at position 1; −8.2 dB at mid travel against the ideal −6.0; A-044, low) |
| Earphone drive into 16 Ω | the load's share of the follower's swing after the input divider: about −15 … −24 dB below the codec output at position 1 (hFE-dependent) | a loudness constant outside the model (A-042) |
| Earphone cut-off (not modeled) | the follower is class A with ≈ 4 mA of quiescent current, so into 16 Ω it can only sink ≈ 50 – 75 mV of negative swing: a limiter-held recording is half-wave clipped above roughly 0.4 – 0.8 of the travel | registered in A-032 at low confidence and in stage 13's "not modeled here" note; the first real-unit capture (hardware review of the plan, P2) |
| Speaker template | f0 750 Hz, Q 1.8; shelf zero 780 Hz, pole 1310 Hz (+4.50 dB, so the rise is complete to within 0.65 dB by 2.5 kHz) | A-010 ranges: f0 600 – 900, Q 1.2 – 2.5, shelf +3 … +6 dB |
| Knee start `k` | 0.85 | one parameter; the bracket check below picks it |

## Derived constants

| Constant | Formula | Value |
|---|---|---|
| `VOLUME_REFERENCE` (P_REF) | the position at which `g_spk(p) × 1.0 V = V_CLIP` under the loaded law (solved by the script; 0.165 with the 1.33 V datasheet ceiling) | **0.1335** (−17.5 dB of travel) |
| `K_SPK(p)` | the speaker route's per-block scalar `g_spk(p) / V_CLIP` (unity at P_REF within 1e-7 of the `f32` default; 11.8 = 21.5 dB at position 1 in the route's scale, where the ceiling is 1.0) | per block |
| `K_EAR(p)` | the earphone route's per-block scalar `g_ear(p)` (unity at position 1) | per block |
| Earphone level at P_REF | `20 log10 g_ear(P_REF)` | −18.6 dB (the spec's "about −18.5 dB ± 1.5") |
| Knee end | `2 − k` | 1.15 |

## The knee (R-04)

```
u = |x|                                   (x = K_SPK · p · hpf_amp(v); the ceiling is 1.0)
u ≤ k         : y = u
k < u < 2 − k : y = 1 − (2 − k − u)² / (4 (1 − k))
u ≥ 2 − k     : y = 1
output = sign(x) · y
```

C¹ at both joints (slope 1 at `k`, 0 at `2 − k`). Verified figures (1 kHz sine, 48 kHz, whole cycles), pinned by a unit test:

| Level re ceiling | Peak out | THD |
|---|---|---|
| −6 dB | 0.501 (−6.00 dB) | 0.0 % (≤ 1 %) |
| 0 dB (at the ceiling) | 0.9625 (−0.33 dB; ≤ 1 dB compression) | 1.5 % |
| +1 dB | 0.9987 | 4.5 % |
| +2 dB | 1.0000 | 8.9 % |
| +2.27 dB | | **10 %** (within +1 … +3) |
| +3 dB | 1.0000 | 13.0 % |
| +6 dB | 1.0000 (≥ −0.1 dB) | 23.0 % (15 – 30 %) |

## Sections per host rate (R-03)

Bilinear transform with the corner (or f0) prewarped at each rate `fs ∈ {6000, 8000, 16000, 44100, 48000, 88200, 96000}`:

- First-order high-pass at `fc`: `H(z) = g (1 − z⁻¹) / (1 − a z⁻¹)`, `a = (1 − t) / (1 + t)`, `t = tan(π fc / fs)`, `g = (1 + a) / 2` (unity at Nyquist). Used for `F_VOL`, `F_AMP`, `F_EAR_IN`, `F_EAR_OUT`.
- Speaker resonance: second-order high-pass biquad from `H(s) = s² / (s² + (ω0/Q) s + ω0²)`, prewarped at f0.
- Speaker shelf: `H(s) = (1 + s/ωz) / (1 + s/ωp)` (zero 780 Hz, pole 1310 Hz) as a first-order section with `b0, b1, a1`, **not prewarped** (prewarping the shelf worsens the top of the band at 6 kHz).
- Peak normalization: the product resonance × shelf is evaluated on a 1 Hz grid from 20 Hz to `fs/2` at each rate and the biquad's numerator scaled so the peak gain is exactly 1.0 at that rate; the script prints the peak frequency (≈ 840 Hz), the 1 kHz reading (≈ −0.66 dB), the 2.5 kHz and 3 kHz readings (≈ −2.74 and −2.77 dB: the shelf's remaining rise above 2.5 kHz is ≤ 0.7 dB) and the 300 Hz reading (≈ −22 dB). Every figure in this file is the plan session's arithmetic and is **provisional until the script prints it**; the Rust pins the script's figures.

## Figures the script prints and the Rust pins

| Figure | Bound (spec) | Plan value |
|---|---|---|
| −3 dB point of each first-order corner at each rate, computed from its coefficients | within ±10 % of the registered figure (a core unit test; not fitted from a measured response: the corners are too close for a fit, P8) | exact by prewarping |
| Earphone level at 1 kHz, position 1 | −0.5 … 0 dB | −0.37 dB (32 + 120 + 280 Hz corners; −0.33 dB from the 280 Hz corner alone) |
| Amplifier passband above 5 × F_AMP (800 Hz) | flat within 0.2 dB | −0.17 dB at 800 Hz, −0.11 dB at 1 kHz |
| Speaker template peak gain | exactly 1.0 (never above) | 1.0 by normalization |
| Speaker template, the Rust against the design's discrete realization | ±0.5 dB at every rate (harness) | identical coefficients |
| Speaker template, the discrete realization against the analog prototype, 100 Hz – 2.5 kHz | ±1.2 dB at 6 and 8 kHz (the bilinear warp near the band's top), ±0.5 dB at 16 kHz and above (script `--check`, reported) | 1.0 / 0.56 / 0.14 dB at 6 / 8 / 16 kHz |
| Speaker template 1 kHz group delay | ≤ 0.5 ms | 0.28 ms |
| Earphone 1 kHz group delay | ≤ 0.5 ms | ≈ 0.07 ms (32 + 120 + 280 Hz) |
| `N_OUT` (the 32 Hz corner's response below −100 dB of its peak, spec 006 FR-008's rule for N₁₀) | | 57.6 ms: 346 / 461 / 922 / 2 539 / 2 764 / 5 078 / 5 527 |
| Volume-0 decay at the route output to −120 dBFS | ≤ 100 ms (amended; a loose bound) | 20 – 30 ms (the 32 Hz corner sits before the multiply, so only the 159 / 120 / 280 Hz sections ring) |
| Knee bracket | the table above | k = 0.85 |

## Emitted file

`stages/output_coeffs.rs`: a header comment with every figure above; `pub(crate) const VOLUME_REFERENCE: f64`, `V_CLIP_V: f64`, `AMP_GAIN: f64`, `R_INPUT_OHM`, `R_POT_OHM`, `Z_EAR_OHM` (the loaded-law constants), `KNEE_START: f64`; per-rate arrays `VOL_HPF: [[f64; 3]; 7]`, `AMP_HPF: [[f64; 3]; 7]`, `EAR_IN: [[f64; 3]; 7]`, `EAR_OUT: [[f64; 3]; 7]`, `GAIN_LAW_SAMPLES: [[f64; 3]; 6]` (`[position, g_spk, g_ear]` at positions 1, 0.5, 0.25, P_REF, 0.1 and 0.01 for the pinned test; SC-004), `SPK_RES: [[f64; 5]; 7]`, `SPK_SHELF: [[f64; 3]; 7]`, `N_OUT: [u32; 7]` (the −100 dB settle), `GROUP_DELAY_1K_SPK_MS: [f64; 7]`, `GROUP_DELAY_1K_EAR_MS: [f64; 7]`, indexed by `SUPPORTED_HOST_RATES` order; every `f64` as `f64::from_bits(0x…)`.
