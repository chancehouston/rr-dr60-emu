# Microphone-path design tool (spec 012)

`design_mic_chain.py` derives every constant of signal-chain stages 1 – 2 (the capsule's hiss and the microphone amplifier). It starts from the circuit as read from the service manual's schematic sheet 3.1 (S-002) and writes the constants to `crates/rr_dr60/src/stages/mic_coeffs.rs` as exact `f64` bit patterns.

Every figure it prints is a **design constant of the model** (A-046 – A-053 in `docs/hardware/assumptions.md`), not a measurement of a real RR-DR60. The Rust pins the figures this script prints. The Rust never pins its own output.

## What it prints

### The circuit

A small-signal hybrid-π nodal model of the transimpedance amplifier. From it the script prints:
- **Corners:** the three first-order corners, fitted jointly to the transfer from the capsule's current to the codec input amplifier's output. Nominal 818 / 77 / 12 939 Hz.
- **Cross-checks on the input corner:** the −3 dB point at the amplifier's output (818.6 Hz), the closed form, and its upper bound for any transistors (996 Hz).
- **Grid:** the corners over C1 ±10 %, the transistor ranks, the Early voltage and Q3's current.
- **DC consistency:** a consistency check of the operating points.
- **Level chain:** the full-scale SPL for capsules of −35 / −40 / −44 dBV/Pa.

### Per host rate

- **Fit:** the constrained fit of the three sections, with its worst error against the prototype (at most 0.1 dB) and its constraints. The fit's constraints are what the bound after stage 2 relies on.
- **Section figures:** each section's −3 dB point, the 1 kHz group delay, and the start phase that puts a host sample on the clipper-input peak.
- **Clip limits** in clipper units.
- **Recovery** after a clipped passage.
- **Harmonics:** H2 and H3 of a tone 6 dB above the first onset, against a near-continuous reference.

### The clipper

The limits from the DC voltages (L₋ ≈ +7.15 dBFS where Q1 saturates, L₊ ≈ +16.37 dBFS where Q2 saturates), the knee (k = 0.85), the onsets and the bound L₊ + L₋.

### The hiss

- **Anchor:** 34 dB SPL(A) capsule self-noise, full scale 90 dB SPL; −61.96 dBFS in the 300–3000 Hz band.
- **Shape and range:** the third-octave shape, the credible range, and the source's σ at each host rate.
- **Voice activation:** the margins at its five levels.

### The generator

The ziggurat tables (128 layers), the retry cap and the keying test vectors that the core test pins.

## Usage

From the repository root. `uv` installs `numpy` and `scipy` from the script's header.

```sh
uv run tools/mic-chain/design_mic_chain.py            # print every figure
uv run tools/mic-chain/design_mic_chain.py --check    # verify every bound and constraint; exit 1 on a failure
uv run tools/mic-chain/design_mic_chain.py --write    # regenerate crates/rr_dr60/src/stages/mic_coeffs.rs
uv run tools/mic-chain/design_mic_chain.py --spice    # the ngspice cross-check
```

`--write` must leave `mic_coeffs.rs` byte-identical when nothing changed. Changing the file is a model change: the golden file is re-blessed and the CHANGELOG notes it.

## The ngspice cross-check

`--spice` runs `rr_dr60_mic.cir`, the same circuit with generic transistor models, in ngspice (`brew install ngspice`). It then fits the three corners to the simulated response at the codec input amplifier's output.

- **It is a gate before the golden bless** (spec 012 FR-005): the input corner must agree with the nodal model within ±10 %.
- **As run on 2026-10-16** with ngspice 47:
  - the simulated operating point is 1.71 / 1.08 / 2.60 V (S-002 measured 1.7 / 1.1 / 2.6 V);
  - the corners are 824.2 / 76.8 / 12 939 Hz, +0.7 % on the input corner.
