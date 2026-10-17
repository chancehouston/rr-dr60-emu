# Research: Microphone Capsule and Amplifier (stages 1–2)

**Spec**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md) | **Date**: 2026-10-16 (plan reviews applied the same day)

## Sources of the figures

Figures marked *plan-time* come from scratch computations made while planning:
- the hardware reviewer's `scratchpad/calc/{hiss,vas}.py` and `scratchpad/review012/{nodal2,full,realize,fit4,hiss2,costconf}.py`;
- the planner's `scratchpad/mic/nodal.py`.

The design script `tools/mic-chain/design_mic_chain.py` (Phase 1) recomputes and prints each of them. The tests pin to its print, never to these notes (spec Assumptions).

## R-01: The circuit model and the corners (FR-005; spec D1, plan review Q5, Q13)

### The model

The design script models S-002 sheet 3.1's microphone amplifier as a small-signal **hybrid-π nodal network**:

- **Capsule node M.** The capsule is a current source into M. R2 (1 kΩ) plus Q11's emitter resistance (≈ 65 Ω) go to AC ground.
- **Input path.** C1 (0.15 µF, ±10 %, ECUV1C154KB) runs from M to Q1's base B. R3 (100 kΩ) feeds back from the output O to B.
- **Differential pair.**
  - Q1: base B, tail E, collector O.
  - Q3: base at the codec's AC-grounded SG reference, collector C3.
  - R5 (10 kΩ) from E.
- **Second stage.** R6 (100 kΩ) from C3 to the rail. Q2 (PNP): base C3, emitter at the rail, collector O.
- **Load.** The output coupling's series loop into the codec input amplifier's virtual ground: C4 in series with C5, both +80/−20 % Y5V, then R7 + R10 = 23 kΩ. The amplifier itself is R15 (82 kΩ) with C10 (150 pF) to GSX.

### Operating points

They come from the schematic's DC voltages:
- tail current: 1.1 V / 10 kΩ = 110 µA;
- Q3 carries ≈ 6 µA, from (3.2 − 2.6) V through R6 (plus Q2's ≈ 0.4 µA base current);
- Q1 and Q2 carry ≈ 104 µA.

### What the script computes

The script solves the network from the capsule's current to **GSX** (the codec input amplifier's output, the limiter detector's and the ADC's view). It reports:
- the three first-order corners as the poles of a joint fit to that transfer (the modeled quantity is exactly three first-order sections, residual < 0.001 dB);
- the −3 dB point below the analytic plateau (f → ∞), as a cross-check;
- the response at 300 Hz and 2550 Hz relative to 1 kHz;
- the transimpedance at 1 kHz.

These are given for the nominal parts and over a grid of C1 ±10 %, hFE 150–460 (the 2SD1819A Q/R/S ranks; the "ASTX" marking is probably S), the Early voltage 20–100 V, and Q3's current 5–7 µA.

### Plan-time results (`full.py`, `nodal.py`)

| Case | Input corner | Output coupling | Roll-off | 300 Hz re 1 kHz | 2550 Hz re 1 kHz | Transimpedance to GSX at 1 kHz |
|---|---|---|---|---|---|---|
| nominal (hFE 250, VA 50 V, I3 6 µA, C1 0.15 µF) | **818.4 Hz** | **76.8 Hz** | **12 939 Hz** | −7.26 dB | +1.68 dB | 227 kΩ |
| hFE 150, VA 20 V, I3 5 µA | 701.6 Hz | 77.3 Hz | 12 939 Hz | −6.60 dB | +1.30 dB | 206 kΩ |
| hFE 400, VA 100 V, I3 7 µA | 878.3 Hz | 76.6 Hz | 12 939 Hz | −7.55 dB | +1.88 dB | 236 kΩ |
| the grid, ranks R/S (hFE 210–460) with C1 ±10 % (as printed 2026-10-16) | 666–982 Hz (637 Hz at the Q rank's worst case) | 76.6–77.3 Hz | | | | |

### What the model shows (plan review Q5)

- **The loop gain is high.** The impedance into B is about 135–420 Ω, so the loop gain is about 240–740.
- **The plateau at about 83 % of R3** is the current division R2′/(R2′ + Z_B) into the capsule's bias resistor, not a low loop gain.
- **The corner is bounded above** by 1/(2π·C1·R2′) ≈ 996 Hz (1.1 kHz at C1 −10 %), whatever the transistors.
- **Below-clip distortion** is negligible.
- **The level chain closes** (Q10). Full scale (0.495 V rms at GSX ÷ 227 kV/A) is about 2.18 µA rms of capsule current, which is about 82 / 88 / 92 dB SPL for capsules of −35 / −40 / −44 dBV/Pa (at 2.2 kΩ). That supports 90 dB SPL (A-029 replaced: "does not close" is superseded).

**Registered range** (A-046): nominal 818 Hz, **665–985 Hz**, bounded above by about 1.0 kHz. Confidence: high that the corner exists (topology and parts list), medium in the value.

### Cross-checks

- **`--spice`** writes a netlist and runs **ngspice 47** (installed 2026-10-16, Homebrew) with generic 2SD1819A/2SB1218A models. It prints the AC response at GSX and the corners.
  - It is a **gate before the golden bless** (spec plan review). A disagreement above 10 % in the input corner holds the bless.
  - The netlist and the models' parameters are committed with the script.
- **The DC check** is an Ebers–Moll solve with free saturation currents. It reproduces 1.7 / 1.1 / 2.6 V by construction, so it is a **consistency** check, not independent evidence: ΔVbe(Q1, Q3) = VT·ln(I1/I3) ≈ 74 mV against the meter's 0.1 ± 0.1 V, and Q2's Veb at 104 µA.
- **The reviewer's closed form** 1/(2π·C1·(R2′ + Z_B)) is printed as a sanity check.
- **As run (2026-10-16, T052; ngspice-47):** the operating point reads v(o) 1.712 V, v(e) 1.076 V, v(c3) 2.605 V, v(b) 1.672 V. The fitted corners are **824.2 / 76.8 / 12939 Hz** (fit residual 0.000 dB) against the nodal model's 818.4 / 76.8 / 12939 Hz. The input corner differs by **+0.7 %**, inside the ±10 % gate, so the gate passes and the bless may proceed. The same run now also sweeps the clip levels (R-03 › As built): the model within 0.45 dB (negative) and 0.29 dB (positive) of ngspice at every level from 0 to +30 dBFS, gate ±1 dB.

**Alternatives considered:**
- A SPICE-only design: the build tool would depend on an external binary.
- An unmodeled corner: rejected by the user.
- A grid of hFE alone: it omits VA and I3, which matter as much.

## R-02: Stage 2's realization and the bound (FR-005, FR-006; plan review Q1, P14)

### The sections

At the host rate, in f64, in the device's order:

1. **HP_in**, the input high-pass: a first-order section with its zero at DC and its pole in [0, 1). The **1 kHz normalization is folded into its numerator**, so no extra multiply is needed. It sits before the clipper, so the clip limits are in engine units.
2. **The clipper** (R-03).
3. **HP_out**, the output coupling: the bilinear form (gain (1 + p)/2, zero at DC), so 1 − HP_out = LP′ is a low-pass with a non-negative impulse response summing to 1.
4. **LP**, the roll-off: b0 ≥ 0, b1 ≥ 0, pole p in [0, 1), DC gain exactly 1.

### The fit

The script fits **the three sections jointly per host rate** by constrained minimax (iterated least squares at power 2 → 16) against the analog prototype (R-01's three corners, normalized at 1 kHz):
- **Where:** at the third-octave centres from 20 Hz to the lower of 20 kHz and 0.45·fs, wherever the prototype is above −40 dB.
- **Achieved** (`fit4.py`, reproduced 2026-10-16): worst error **0.002 / 0.015 / 0.060 / 0.036 / 0.075 / 0.005 / 0.004 dB** at 6 / 8 / 16 / 44.1 / 48 / 88.2 / 96 kHz, with every constraint satisfied.
- **The obvious realization fails.** Bilinear high-passes plus a Nyquist-matched low-pass err by 0.65 / 0.34 / 0.25 / 0.76 / 0.76 dB at 6 / 8 / 16 / 44.1 / 48 kHz. A three-point matched low-pass's pole goes negative at 6 kHz.
- **Output.** The script emits each rate's coefficients as exact `f64` bit patterns in `stages/mic_coeffs.rs`.
- **`--check`** asserts the constraints, the ±0.1 dB match, and that each realized impulse response satisfies the bound's premises.

**Fallback** if the Rust impulse/DTFT reader adds error near the thin 48 kHz margin (0.025 dB): the tolerance above 4 kHz becomes ±0.25 dB, recorded as a deviation (spec US1 AS1). Everything above 3 kHz is removed by the decimator.

### The bound after stage 2

- The clipper's output lies in [−L₋, L₊].
- HP_out = 1 − LP′, with LP′ non-negative and summing to 1, so HP_out's output lies in [−(L₊ + L₋), L₊ + L₋].
- LP is non-negative with unity DC gain, so it keeps that interval.
- The bound is therefore **L₊ + L₋** (6.68 + 2.24 = 8.92, **+19.0 dBFS**), up to a few ULP. The test asserts it in f32, rounded up.

### Settling

- The output coupling's time constant is about **2.1 ms** (77 Hz).
- The oracle is the identity e = stage2(x) − stage2_linear(x), with a clipper-off twin hook. After the last clipped sample, e is exactly HP_out's and LP's decaying tail.
- The script prints the time to return within ±0.1 dB for the stated stimulus: a 1 kHz tone at +13 dBFS for 50 ms, then −20 dBFS. Expect about 4–10 ms.

**The output coupling's corner** (spec D5) is one corner, not two:
- with the limiter idle, C4, R7, the shunt node, C5 and R10 form one series loop: about 77 Hz;
- C4 and C5 are Y5V parts, +80/−20 %, so units vary over about 40–95 Hz (registered, not modeled);
- under deep limiting the corner moves to C5·R10 = 159 Hz (not modeled).

**Alternatives considered:**
- Bilinear prewarped sections: they fail SC-002, as shown above.
- Biquads: more operations, and they would need a new bound proof.
- An L1-norm bound: looser, and unnecessary given the 1 − LP′ identity.

## R-03: The clipper (FR-006; plan review Q4, P5)

### The form

A memoryless asymmetric soft clipper, **exactly linear below the onset**:

- y = x when −k·L₋ ≤ x ≤ k·L₊;
- otherwise y = L₊·knee(x·invL₊, k) for x > 0, or −L₋·knee(−x·invL₋, k) for x < 0;

as branchless selects, reusing spec 009's parabolic knee (`stages/output.rs::knee`: C¹ at both joints, a hard ceiling at 1). The select keeps the linear region bit-exact, so US2 AS4's "equals the linear path" holds without an x·invL·L round trip.

### The limits

S-002's DC voltages give the **limits**, with 0 dBFS = 0.196 V peak at the amplifier's output (A-029, A-030):

| Engine samples | Mechanism | Swing | Limit (plan-time) |
|---|---|---|---|
| Negative (limited first) | the output (1.7 V) falls until Q1 saturates near 1.15–1.3 V, with its emitter at 1.1 V | 0.4–0.55 V | **L₋ ≈ +7 dBFS** (+5 … +9) |
| Positive | Q2 saturates (3.2 V − 0.1…0.2 V − 1.7 V) | 1.3–1.4 V | **L₊ ≈ +16.5 dBFS** (+16 … +17.5) |

### As built: refitted to the circuit simulation (T054 B1, 2026-10-16)

The post-implementation review ran the committed ngspice netlist as a 1 kHz transient at rising levels and read the output's swing from its operating point (1.712 V). The onsets agreed with the hard-rail model within about 0.7 dB, but the limits did not. Node B stays a virtual ground to beyond +20 dBFS: as the output falls, **Q1's base–collector junction conducts across R3**, a diode-feedback limit that keeps creeping up (0.516 V at +10 dBFS, 0.577 V at +20, 0.595 V at +30), with no hard rail. The positive side is Q2 reaching the rail (1.39 V at +20, 1.46 V at +30). The hard-rail estimates (0.45 / 1.30 V) were 2.4 / 1.0 dB low.

| Polarity | Swing (V) | Knee | Limit (engine input, 1 kHz) | 0.1 dB onset | Worst error against ngspice, 0 … +30 dBFS |
|---|---|---|---|---|---|
| Negative (limited first) | **0.566** (0.45 … 0.62) | **k₋ = 0.65** | **+9.15 dBFS** (+7.2 … +9.9) | +6.77 dBFS (ngspice +7.02) | 0.45 dB (the hard-rail model 2.43) |
| Positive | **1.414** (1.30 … 1.48) | **k₊ = 0.80** | **+17.10 dBFS** (+16.4 … +17.5) | +16.09 dBFS (ngspice +16.34) | 0.29 dB (the hard-rail model 1.02) |

- **The fit:** spec 009's knee, the minimax over the swing and k with the onset held within 0.3 dB of ngspice's, at each polarity. The ranges run from the hard-rail estimates to ngspice with junction resistances (r_b 100 Ω, r_c 20 Ω) and a reverse beta of 10.
- **The gate:** `design_mic_chain.py --spice` reruns the sweep (19 levels, 0 … +30 dBFS) and fails if the model is more than 1 dB from ngspice at any level of either polarity; it also reprints the refit. As run on 2026-10-16: 0.45 / 0.29 dB, the refit equal to the committed constants.
- **Beyond +30 dBFS** the negative side keeps creeping (logarithmically), so the model's hard limit reads a few tenths of a dB low there. That is documented, not modeled.
- **Consequences:** the L₊ + L₋ bound grows to about +20 dBFS (the limiter's 40 dB range still covers it); the knee is now one per polarity (`KNEE_NEG`, `KNEE_POS`), the static onsets are asserted in clipper units at ±0.065 dB (B4: the rising tone's level step per cycle), and a knee mutant (±0.05) must fail them.

### The knee

- *(Plan-time; as built, one knee per polarity, k₋ = 0.65 and k₊ = 0.80: see "As built" above.)* **k = 0.85**, spec 009's value. It is justified (A-051) by Q1's saturation curvature: Q1 starts at Vcb ≈ 0, so its base-collector junction conducts from about +0.4 V, at a high loop gain.
- **The onset** (0.1 dB compression) sits about **0.63 dB below L**, and the output reaches L about 1.21 dB above it. For k = 0.8 or 0.9 these are −1.01 / +1.58 dB and −0.30 / +0.83 dB.
- The script prints k, both onsets and both limits, **in clipper units and referred to the engine input at 1 kHz**. The latter includes |HP_out·LP| at 1 kHz, about −0.04 dB.

### Measurement

- **The static curve** is asserted in clipper units through a probe exposing the engine's actual (clip_in, clip_out) per sample. That is exact, needs no phase, and doesn't reimplement HP_in in the harness.
- **The engine-input 1 kHz figures** use a tone started on the phase the script prints. The phase compensates HP_in's ≈ +39° at 1 kHz, so a sample lands on the clipper-input peak. Without it, 6 kHz reads 0.59 dB low (the 004 R-08 and 009 lessons).

**Polarity.** Negative engine samples are the polarity the limiter does not sense (A-036). A-036 gains the sentence.

### Harmonics

H2 and H3 of a tone 6 dB above the first onset are asserted within ±0.5 dB of the script's **discrete** model at each rate. The alias level (the discrete figure minus the continuous one) is reported. *(As built, B3: at 6 kHz H3 = 3 kHz falls on Nyquist, where the DFT of a real signal reads N·|a·cos φ| rather than N·a/2, so that bin is not an amplitude: its row compares the same estimator at the same phase, and its alias is not reported. The real alias figures are at most about 2 dB: 6 kHz H2 +1.96 dB, 8 kHz H2 −0.46 / H3 +0.30 dB after the refit.)* The C¹ knee's harmonics fall as about 1/n³, so the aliases are small. Oversampling or ADAA was rejected: about 2–4× the clipper's cost for an effect only very loud input triggers.

## R-04: The hiss (FR-003; spec Q2, D3, D9, D10; plan review Q2, Q3, Q7, Q8, Q10)

### The anchor (registered; printed by the script)

- **Capsule self-noise:** 34 dB SPL(A) (32–36 for small electrets of the period, S/N 58–62 dB at 94 dB SPL).
- **Full scale:** 90 dB SPL at 1 kHz (R-01's closing chain; A-029).
- **A-weighted to band:** −5.96 dB, from the A-weighted level to the 300–3000 Hz band through the full-network band.
- **Result:** **−62.0 dBFS** (RMS, AES17) in the 300–3000 Hz band at stage 2's output.
- **The 1 kHz third-octave** sits 11.3 dB below the band level.
- **The amplifier and R2 add a floor near −73 dBFS** (Q3 at 6 µA: e_n ≈ 7 nV/√Hz at a noise gain of about 95; R2's thermal noise about −78 dBFS). That adds about +0.3 dB at the anchor, neglected in the level (inside its ±0.5 dB; corrected wording, B2: "folded in" was not literally true).
- **Credible range:** about **−70 … −54 dBFS**, with the capsule's S/N held at 58–62 dB (A-048). *(Restated 2026-10-16, B2: that range varied the self-noise and the full scale by ±4 dB each, independently. Through the closed chain (2.181 µA rms of capsule current at full scale into 2.2 kΩ; capsules of −35 … −44 dBV/Pa at S/N 58 … 62 dB) it is **−65.6 … −52.6 dBFS**, a −40 dBV/Pa capsule at S/N 60 gives −59.6, and the −62 default corresponds to a capsule of about −42.4 dBV/Pa. The default is kept, the user's choice.)*
- **Unknown:** the codec's SG reference noise reaches the output with the same gain. About 1.2 µV rms in band (22 nV/√Hz) would equal the whole hiss (A-050; S-003 pages 4–17 are the lead).

### The source

- The source is white at the host rate. The script integrates the band level through the **realized** stage-2 response at each rate and solves for the source's RMS σ_r per rate, emitted as exact `f64` bits.
- The level gain is σ_r·10^(offset/20), computed at construction with `detmath::exp`. `exp(0.0)` is exactly 1.0, so offset 0 multiplies by σ_r exactly.

### The generator (no platform math)

- **Uniform source:** `xoshiro256++`, period 2²⁵⁶ − 1.
- **Keying** (plan review Q8): `z = mix64(seed ^ MIC_STREAM) ^ mix64(counter ^ MIC_COUNTER_SALT)`, where mix64 is SplitMix64's finalizer. Four SplitMix64 outputs from z fill the state.
  - Two independent bijections remove the XOR/add collision families. *(Wrong, B5: the same bijection XORed twice is symmetric, key(s, c) = key(c ⊕ K, s ⊕ K) with K = MIC_STREAM ⊕ MIC_COUNTER_SALT, and seeds K ⊕ n at counter n all share one stream. **As built (2026-10-16): `z = mix64(mix64(seed ^ MIC_STREAM) ^ counter ^ MIC_COUNTER_SALT)`**, a bijection of the seed at a fixed counter and of the counter at a fixed seed, with no structured cross-collisions; the script's key vectors were re-pinned.)*
  - SplitMix64 never yields an all-zero state, so seed 0 does not degenerate.
  - Both constants are registered (A-049).
- **Gaussian:** a ziggurat (Marsaglia & Tsang 2000, 128 layers), with tables emitted by the script as `f64` bits.
  - **Bit use:** the layer index and sign come from bits 0–7, and the 53-bit mantissa from bits 11–63 (no overlap; Doornik 2005).
  - **Tail:** the uniforms are `(r >> 11) + 1`, so `ln` never sees 0. The tail and the wedge use `detmath::{exp, ln}`.
  - **Bounded work** (constitution IV): each rejection loop is capped at **8 attempts**. At the cap the last candidate is returned, deterministically. The probability of reaching the cap is about 1e-16 per sample (registered in A-049). *(Restated, B6: that counted only the main loop, 0.0122⁸ ≈ 5e-16. The tail loop dominates: about 5.7e-4 tail entries per sample × 0.0689⁸ ≈ **2.9e-13 per sample**, about 1e-4 cap events per hour at 96 kHz; the capped value is still a tail value. The script prints both.)* A hook forces the cap path in a unit test.
- **Advance count.** The source advances a variable but deterministic number of times per sample. The hiss is a function of the host samples processed, so partition independence holds.

### Streams (spec FR-009, the user's plan-review choice)

- The **recording counter** lives in the split engine (`Transport`). It is 0 at construction and **never rewinds**.
- `start_recording`, after `memory.open()` succeeds, rekeys the recorder's capsule to (seed, counter) and then increments the counter.
- The one-pass record path's capsule is keyed to the counter's **current** value (the next recording's) at construction, `reset` and `reconfigure`.
- So the first recording after any of these equals the one-pass, and no two recordings of one engine share a stream.

### Tests

| Test | Where | Assertion |
|---|---|---|
| Tail counts | core, release-only, 10⁸ draws (about 0.2 s) | the 4σ count within ±5 % of its expectation (≈ 6 334); the 5σ count within ±40 % (≈ 57) |
| Cross-correlation | harness, the tap after stage 1 (white), 480 000 samples per rate | below 5/√N at lag 0 and below 6/√N over lags ±64, for seeds 0 / 1 and counters 0 / 1 |
| Level stability | release-only, a 60-minute run at 48 kHz | every 10 s window within ±0.5 dB |
| Seeds | all | the seeds are fixed and never re-picked to make a test pass |

### Alternatives considered

- **Box–Muller:** about 15–25 ns per sample.
- **Irwin–Hall(12):** its tails are light.
- **The harness's PCG32:** fine for stimuli, but keeping the core's generator separate keeps the harness free to change.
- **A counter that rewinds at reset:** rejected by the user, because recordings kept in memory would share hiss.

## R-05: Placement, taps, latency and the counter's states (FR-001, FR-007 – FR-009)

### Placement

- **Where the stages run.** Stages 1–2 run inside `RecordPath::limited` (renamed `front`), between `sanitize_in` and the limiter, so the one-pass, the recorder, the stop's flush and the frame completion all pass through them.
- **The stop.** It calls `recorder.push` with zero-valued host input (`transport.rs` `feed_zeros`), so the hiss continues through it. `StopOutcome::silence_fed_samples`'s rustdoc is amended: "zero-valued input samples; with stage 1 on, the recording carries hiss there".
- **Bypass.** A bypassed stage is a skipped branch decided at construction, so it is bit-identical to 0.11 by construction.

### Taps

- `Tap::AfterMicCapsule` and `Tap::AfterMicAmp` are host-rate taps without conversion, voice activation or chain, with latency 0. The one-pass's `limiter_only` path is generalized to `front_only(upto)`.
- The recorder ignores taps (it is built with `honour_tap = false`); the tap docs say so, as the existing ones do.
- `Tap::AfterAgc` now includes stages 1–2 when they run. Its latency stays 0.

### Latency

Not reported (spec 009 R-08's precedent). The hardware reviewer withdrew the objection: the group delay is frequency-dependent (0.1 ms at 1 kHz, 0.3 ms at 300 Hz, about 2 ms near 77 Hz), so no single integer is right. The 1 kHz group delay is printed per rate.

### The counter's states

The transport (spec plan review, coach P11 / hardware Q2):

| Event | Counter | One-pass capsule |
|---|---|---|
| construction | 0 | keyed (seed, 0) |
| `reset` (idle or mid-recording) | unchanged | rekeyed (seed, counter) |
| `reconfigure` accepted | unchanged (carried into the rebuilt engine) | keyed (seed, counter) |
| `reconfigure` refused | unchanged | unchanged |
| `start_recording` succeeds | recorder rekeyed (seed, counter); then counter += 1 | unchanged |
| `start_recording` refused (any error) | unchanged | unchanged |
| empty recording discarded at stop | already advanced (the stream is consumed) | unchanged |
| memory-full self-stop | unchanged | unchanged |
| erase, import, playback, one-pass calls | unchanged | the one-pass stream continues |
| zero-length call | nothing advances | nothing advances |

Each row gets a unit test in `transport.rs`.

## R-06: The 0.11-default configuration and the rebase (FR-010; coach C7, P1, P2)

### Harness

`configs::settings_011(rate)` is `Settings::new` with `mic = MicSettings::BYPASSED`. The existing base configurations are rebased on it:
- `settings_008`, and therefore `settings_007` and every named configuration;
- `"default_output" | "default_earphone" | "default_loud"`;
- the output-clamp and volume-ramp twins (through those names).

Direct `Settings::new` uses in `output_checks.rs`, `smoke_reference.rs` and `tests/output_edge_cases.rs` move to `settings_011`.

### Core and FFI (coach P2)

About 70 `Settings::new(` uses in `crates/rr_dr60/src` (pipeline.rs 34, settings.rs 16, paths.rs 8, transport.rs 5, validate.rs 4, output.rs 2, memory.rs, manifest.rs), 5 in `rr_dr60_ffi/src/lib.rs`, and the doctests. They are audited at the default flip (Phase 2):
- a `test_util::settings_011` helper in the core;
- each use that checks pre-012 behaviour (for example pipeline.rs's "the first 180 outputs == 0.0") is rebased;
- each use that checks the default is kept and restated.

### C (coach P1)

The C fields land **inert in Phase 1**: the structure grows to 120 bytes, `to_settings` maps the fields, and `smoke.c`'s earlier cases set both switches off. The C loop and `ffi_parity` are green before the default flips in Phase 2. **The bit-identity proof (the seven SHA guards, the C loop, `ffi_parity`) runs at the flip in Phase 2**, not in Phase 1, where the default is still bypassed.

### Default-engine figures that move

They are found by grepping the README, `assumptions.md` and CLAUDE.md for "default" figures (Phase 4b), then restating each or labelling it "0.11 default". Known so far:
- A-042's 1.96–2.06 peaks;
- the README's earphone 1.04–1.07;
- spec 011's route-output click figures;
- spec 008's "silence −78 dBFS and below";
- spec 009's "decoder output on silence is quiet".

## R-07: The voice activation and the hiss (spec US3 AS6; D4; plan review P15)

### Plan-time simulation

The reviewer's `vas.py`: the real state machine (hang 6000, onset 120, gap 32 device samples) on Gaussian hiss through an approximation of stage 4.

| Hiss (300–3000 Hz, AES17) | Level 5 (−46 dBFS) | Level 4 (−43) | Level 3 (−40) |
|---|---|---|---|
| −62 (default) | pauses after the 1 s hang, never resumes | same | same |
| −55 | pauses | pauses | pauses |
| −52 | **never pauses** | pauses | pauses |
| −50 | never pauses | pauses after 48 s | pauses |
| −48 | never pauses | **never pauses** | pauses after 1.8 s |

The gate pauses reliably when threshold − hiss RMS ≥ about 9 dB, and fails below about 6–7 dB. At the default, the RMS margin to level 5 is about 16 dB.

### The harness check

- **Where:** `Tap::AfterVas`, which does not run the codec; the events do not depend on it (coach P15).
- **The run:** a seeded 60 s silent run at each level and host rate, with `measurement_rates()` under coverage. It asserts from the events that the recording keeps exactly the hang time, pauses once, and never resumes. The RMS margin and the exceedance count are printed.
- **The offset bisection:** the offset at which levels 4 and 5 stop pausing is found by bisection. Under `cfg(coverage)` it runs at 6 kHz only (the voice-activation input's hiss level is the same at every rate by construction); in the normal suite it runs at all rates.
- **As run (2026-10-16, after the post-implementation review's keying change, B5):** in 10 s silent runs, level 5 stops pausing from **+8.8 … +9.5 dB** by host rate; level 4 pauses beyond +12 dB at five rates and stops from **+11.9 dB** at 44.1 and 96 kHz (before the keying change it paused beyond +12 dB at every rate). Near these offsets a gate can still pause late in a longer run (the table above: 48 s at −50 dBFS), so the 10 s bisection is a lower estimate of the stop offset. The 600 s runs at offset 0: margins 16.8 dB at level 5, hiss peaks −50.6 … −51.4 dBFS, the hang time kept at every level and rate.
- **Release-only:** the 600 s runs.

**A-022** is re-derived in Phase 4b:
- the hiss's peaks (about −51 dBFS over 600 s) replace "capsule self-noise peaks around −57 to −62 dBFS";
- room-noise placements move by the input high-pass's tilt.

## R-08: The limiter and the speech codec with the hiss (spec US3 AS7, AS8; plan review Q12)

### The limiter (AS8)

- **Setup.** Twin engines with the same seed, one with the limiter bypassed, are read at `Tap::AfterAgc`. A +9 dBFS 1 kHz burst, starting on the host-grid peak, is followed by 3 s of silence.
- **Assertions over the silence:**
  - (a) the on/off ratio is at most 1 and non-decreasing (spec 011's reader, 4 f32 ULP);
  - (b) *(restated at implementation, 2026-10-16)* the limiter's state (`__test_hooks::limiter_state`, every sample) over the release equals, within 1e-6 relative, that of a **reconstruction**: an engine with stage 1 off, fed the first engine's own stage-1 output (read at `Tap::AfterMicCapsule`, the same seed) during the burst and exact zeros during the silence. As measured: the state releases from about 1.07 V to 0.78 V and the two agree within 7e-10 (the reconstruction's `f32` input rounding).

  The original (b), "the ratio trajectory is identical for seeds 0 and 1 within 4 f32 ULP", cannot hold: each seed's hiss moves the burst's peaks by about 1e-3 relative, so the charge at the burst's end differs and the seeds' trajectories differ by about 700 – 5 800 ULP (reported). The reconstruction isolates the claim: if the hiss in the silence charged the detector, the hiss-on state would fall behind. The release figures themselves remain spec 005's.

### The codec (AS7)

- **Setup.** The default configuration with the voice activation bypassed records and plays 10 s of silence.
- **Measurement.** The decoded hiss's level relative to the input hiss is computed at 8 kHz after resampling both sides (spec 008's lesson) and reported per rate. It is expected a few dB low.
- **Placement.** The hiss sits about 20 dB above the fixed-gain quantizer's floor (≈ −82 dBFS), so it is coded, not zeroed.
- **Default.** With the voice activation on, the recording's length (hang plus stop feed) is reported.

## R-09: Cost (FR-011; the user's plan-review choice; coach P8, P9; hardware Q6)

### Measurement conditions

48 kHz, codec bypassed, speaker route, release build, timing tests one at a time. In one build each side is the fastest of 45 strictly alternating runs; across builds, the fastest of nine alternating rounds (`scripts/ab-timing.sh`). *(The in-build protocol was the fastest of nine alternating runs until the implementation; see "As measured".)*

| Measurement | Test | Stimulus | Budget | Where |
|---|---|---|---|---|
| **In-build, on against bypassed** | `timing::mic_cost` | −20 dBFS noise; full-scale noise and the hiss offsets as a reported spread | **≤ 10 %** | asserted on the development machine; on CI under a **25 %** safety ceiling (the x86 runner is several times slower on branchless selects, spec 010) |
| **Cross-build, bypassed against 0.11** | `scripts/ab-timing.sh` (`output_cost` in both builds; in the branch its configuration is rebased on `settings_011`) | `output_cost`'s fixed stimulus | **≤ 3 %** | asserted on the development machine; once on CI through the `ab-timing` label |
| **Cross-build, on against 0.11** | `scripts/ab-timing.sh c9f9994 output_cost_mic` (a new optional head-test argument) | `output_cost`'s fixed stimulus | **≤ 10 % as corroboration** (amended 2026-10-16; checked by hand, not asserted) | `output_cost_mic` asserts `limiter_state() == 0.0` at the end, so a charged limiter can never pass as mic cost |

Why the cross-build "on" figure is only reported: stage 2 changes the stimulus's level (RMS 0.240, peak 0.784, only 1.1 dB below the limiter's knee) and therefore the cost of the limiter and the speaker knee.

### Scripts and workflow

- `scripts/ab-timing.sh`'s default baseline becomes **`c9f9994`** (the 0.11.0 merge on main, the 012 branch point).
- `.github/workflows/ab-timing.yml` gains a second step for `output_cost_mic` and the new baseline. A pull_request event uses the PR's copy of the workflow.
- The A/B script only prints, so a task records its figures against the budgets.

### Estimate and fallback

- **Estimate:** ziggurat plus xoshiro ≈ 2 ns, three first-order sections ≈ 3 ns, the clipper ≈ 1–2 ns (the normalization is free, folded into HP_in). That is **about 6–7 ns on about 73 ns, +8–10 %**.
- **Fallback** if the in-build ratio exceeds 10 %: merge HP_out and LP into one biquad. That voids the 1 − LP′ argument, so the script must re-prove the bound for the merged section (the L1 norm × (L₊ + L₋), and FR-006 amended) before the change. The change goes to review first.

### As measured (T042, 2026-10-16, development machine, solo release runs)

| Measurement | Figure | Budget |
|---|---|---|
| Cross-build, bypassed against `c9f9994` (`scripts/ab-timing.sh`) | speaker 73.7 → 74.0 ns, **+0.41 %**; earphone 65.2 → 65.7 ns, **+0.77 %** | ≤ 3 % ✓ |
| Cross-build, on against `c9f9994` (`scripts/ab-timing.sh c9f9994 output_cost_mic`) | speaker 73.8 → 78.1 ns, **+4.30 ns (+5.83 %)**; earphone 65.3 → 68.7 ns, +3.40 ns (+5.21 %); the limiter uncharged at the end of every run | ≤ 10 % as corroboration ✓ |
| In-build `mic_cost`, 45 strictly alternating runs, −20 dBFS noise | 65.9 → 69.4 ns, **+3.53 ns (+5.36 %)**; a second run +3.93 ns (+5.92 %); full-scale noise +5.6 – 5.7 %; offsets −18 / +12 dB equal within 0.1 ns; split: stage 1 +2.1 – 2.8 ns, stage 2 +1.4 – 1.9 ns | ≤ 10 % ✓ (CI ceiling 25 %) |

**Why the in-build protocol changed.** With the specified fastest of nine alternating runs, `mic_cost` read **+10.3 – 10.9 ns (+15 – 17 %)** in four tries, and nine rounds of five runs per side read +11.9 – 12.3 ns (+18 %), while 45 strictly alternating runs read +3.9 – 4.2 ns and the independent cross-build A/B +4.3 ns. In the high mode stage 2 alone read +8 – 11 ns. Probes ruled out the arithmetic:
- each first-order section costs 2.7 – 3.1 ns in isolation, the same for all three;
- removing the clipper, the state flush, or the amplifier's inlining changed nothing;
- with one section at a time in the engine, the output coupling alone read +8 ns and the roll-off alone +0, although their outputs are almost the same signal.

So the high readings depend on how the timed runs are ordered, not on the code. The cause was not found (a scheduling or memory-layout effect is suspected, not shown). R-09's fallback (one biquad for the output coupling and the roll-off) was therefore not taken: the roll-off already costs nothing. The user chose the robust protocol, with the 10 % budget kept and the cross-build figure held to it as corroboration.

**Estimate against measurement.** Plan time: 6 – 7 ns (+8 – 10 %). Measured: about 4 ns (+5 – 6 %).

### As measured (T053, 2026-10-16, development machine, one solo release run: `cargo test -p rr_dr60_harness --release --test timing -- --ignored --nocapture --test-threads=1 mic_cost output_cost_mic codec_cost_per_sample_and_worst_block`)

| Measurement | Figure | Budget |
|---|---|---|
| `mic_cost`, −20 dBFS noise | 67.7 → 71.3 ns, **+3.54 ns (+5.22 %)**; full-scale noise +5.24 %; offsets −18 / +12 dB 71.3 / 71.3 ns; split: stage 1 alone +1.92 ns, stage 2 alone +1.17 ns | ≤ 10 % ✓ |
| `output_cost_mic` (stages 1 – 2 on) | speaker 65.2 → 78.4 ns, +20.4 %; earphone 65.2 → 68.9 ns, +5.7 % | 009 SC-007 ≤ 25 % ✓ |
| Spec 008's worst 64-sample block at 48 kHz (`default_codec`) | stages bypassed **195.2 µs** (median 3.8 µs); stages 1 – 2 on **144.5 µs** (median 4.0 µs) | 1333.3 µs ✓ (margin 7× / 9×) |
| Spec 008's per-sample figures | 0.896 / 1.361 µs per device sample at 6 / 48 kHz; record + play 1.015× | 8 µs ✓ |

The worst block is one run's maximum and is set by scheduling jitter, not by the code: the case with stages 1 – 2 on read lower than the bypassed one. Its medians (3.8 against 4.0 µs) are the cost signal. The worst-block case with stages 1 – 2 on was added to `codec_cost_per_sample_and_worst_block` for this task.

After the post-implementation review's fixes (the clipper's per-polarity knee, the nested keying), one more solo run: `mic_cost` 67.9 → 71.3 ns, **+3.43 ns (+5.05 %)**; `output_cost_mic` speaker +20.4 %, earphone +5.9 %. The per-polarity knee select costs nothing the runs resolve.

## R-10: Interfaces (FR-012; plan review P13, U4)

### Rust

- `Settings::mic: MicSettings { capsule_enabled, amp_enabled, hiss_offset_db }`, with `MicSettings::DEVICE` (on, on, 0.0) and `MicSettings::BYPASSED`. `#[non_exhaustive]`.
- **Validation:** `hiss_offset_db` must be finite and within −18 … +12 dB, **checked whether or not the capsule is on** (as the AGC fields are), else `Error::InvalidSetting(Setting::MicHissOffsetDb)`. `Setting::MicHissOffsetDb` is appended, with its `range()`.
- **Taps:** `Tap::AfterMicCapsule`, `Tap::AfterMicAmp`.
- **Constants:** `mic::HISS_OFFSET_RANGE_DB` only, for UI sliders. **No public full-scale SPL constant** (coach P16 Q5): the 90 dB SPL figure lives in the rustdoc, the README and A-029.

### C

- `RrDr60Settings` grows 112 → **120** bytes (8-aligned):
  - `mic_capsule_enabled` at 112 and `mic_amp_enabled` at 113;
  - `reserved_012 [u8; 2]` at 114;
  - `mic_hiss_offset_db: f32` at 116.
- **Const assertions** on the size, the alignment and **every new offset** (112, 113, 114, 116), plus offset 116 + 4 == size.
- `struct_size = 112` is refused with the existing invalid-argument status and the struct-size field (spec 010's pattern). `RrDr60SettingField::MicHissOffsetDb = 16` is appended (5 and 6 are retired).
- Taps `RR_DR60_TAP_AFTER_MIC_CAPSULE = 9` and `RR_DR60_TAP_AFTER_MIC_AMP = 10`. `RR_DR60_VERSION_MINOR` is 12.
- The hardware reviewer noted that the three fields would fit in 0.11's reserved tail without a C break. Growing was kept for consistency with the earlier features (the project breaks before 1.0; U4 at its default).

### Stored form and controls

- **Stored form:** unchanged (version 2).
- **Live control:** the offset is a setting applied by `reconfigure`. A live control belongs to the deferred controls discussion.

## R-11: Golden file, mutants and records (FR-013, FR-014; plan review P12, Q11, Q15)

### Golden file: `golden-mic-v1.json`

- **Stimuli:**
  - `silence_1s`;
  - `voiced_1s` (`stimulus::voiced(fs, 1.0, −20.0, 7)`);
  - `loud_tone` (1 kHz, +12 dBFS, 0.25 s);
  - `very_loud_tone` (1 kHz, +24 dBFS, 0.25 s, reaching both limits).

  The tones start on the phase the script prints.
- **Configurations:**
  - `mic_only` (stages 1–2, tap after stage 2);
  - `mic_only_quiet` (hiss off);
  - `mic_capsule_only` (tap after stage 1, at offset −12 dB, which pins the `detmath::exp` gain);
  - `default_mic` (the 0.12 default, voice activation bypassed);
  - `default_mic_quiet` (the same, capsule off).
- **Second recording:** a `default_mic_rec1` family of `silence_1s` at counter 1, recorded then played.
- **Size:** 4 × 5 × 7 + 7 = **147 entries**.
- **Bless:** once, after the post-implementation reviews. At bless time it asserts:
  - the quiet entries on silence are exact zeros;
  - the hiss band level is within ±0.5 dB of the script;
  - `default_mic` at the first recording equals a record-then-play.

### Mutants (five)

1. `no_input_hp`;
2. `symmetric_clip`;
3. `swapped_polarity`, the likeliest real sign bug;
4. `shared_stream` (the counter ignored);
5. `hiss_db_delta` = 3 dB.

Each must fail its row.

### Manifest

- Stages 1 and 2 become `assumed`, model version 0.12.0, with sources, assumptions, not-modeled notes (≤ 120 characters) and the Rust API items.
- **One re-bless** of the canonical text and the README table in Phase 1, after the wording review and the denylist run. **One review-driven re-bless** is allowed in Phase 6 (as in specs 008 and 010).

### Registers

**New assumptions:**

| ID | Content | Confidence |
|---|---|---|
| A-046 | the input high-pass | high that it exists; medium in the value |
| A-047 | the output coupling with C4/C5's tolerance, and the 13 kHz roll-off | |
| A-048 | the hiss anchor and shape, with its credible range | |
| A-049 | the generator, keying, retry cap and streams | an engine convention |
| A-050 | the amplifier's added noise, and the unknown SG reference noise with its 22 nV/√Hz equivalence figure | |
| A-051 | the clip limits, the knee and its justification, polarity | |
| A-052 | the capsule's flat response | |
| A-053 | the lumping of A-047's sections ahead of the limiter | |

**Amended:**
- A-005;
- A-006: the topology, with the loop gain high;
- A-019, A-028: the detector no longer sees the host's DC;
- A-022: re-derived;
- A-029: the corners, the closed level chain, the self-noise figure; never "low loop gain";
- A-036: the first-clipping polarity.

**Other records:**
- signal-chain rows 1–2;
- `sources.md`'s S-002 summary (R6; R1 is the record lamp's resistor);
- a candidate secondary source for the 2SD1819A ranks;
- the hardware-evidence issue template (the reviewers' captures);
- the seed's rustdoc and its C comment.

## R-12: Risks

- **The input corner is wrong.** A capture showing a flat low end becomes a coefficient change and a re-bless (A-046 carries the capture protocol).
- **The cost budget:** see R-09's fallback.
- **The voice-activation coupling.** A capture that raises the hiss can turn levels 4–5 into "never pause". This is documented.
- **The thin 48 kHz fit margin:** see R-02's fallback.
- **The ziggurat tables and constants** are exact bit patterns. Regenerating them is a model change and a re-bless.
- **Coverage time.** The voice-activation runs are at `AfterVas` without the codec; the bisection runs at one rate under coverage; the 600 s and 60-minute runs are release-only. ci.yml's release `--ignored` step gains `--test mic_matrix`.
