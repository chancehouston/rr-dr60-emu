# Contract: the codec design constants and the 80-bit frame (feature 008; research R-02)

Every figure here is a **design constant** of the model, printed by `tools/celp-design/design_celp.py` and pinned by a core unit test. None is a measurement of, or a claim about, the RR-DR60's own coder (A-004, A-040, A-041). The model is "modeled on a 4 kbit/s CELP-class coder of the period" (S-008 – S-011 as secondary sources for the class). Revised after the plan reviews (hardware P1, P3, P4, P6, P7; coach Q6).

## Rates and sizes

| Constant | Value |
|---|---|
| Device rate | 6000 Hz (A-026) |
| Frame | 120 samples, 20 ms; 50 frames per second |
| Subframe | 60 samples, 10 ms; two per frame |
| Bits per frame | 80 (10 bytes); 80 × 50 = 4000 bit/s exactly (FR-003; frame payload, A-040) |
| Look-ahead | **60 samples, 10 ms** (G.723.1's 7.5 ms class; chosen so that the delay below is a multiple of 20) |
| Algorithmic delay `D` | **180 samples, 30 ms** = frame + look-ahead; a multiple of 20, 10 and 3, so it is a whole number of host samples at all seven rates (the one-pass equivalence of FR-009 needs this: the interpolator's phase cycle is 20 device samples at 44.1 kHz, 10 at 88.2, 3 at 8 and 16) |
| LPC order | 8 |

The encoder emits frame *k* on the push of post-VAS sample 120(k+1) + 59 (zero-based); `LOOKAHEAD_SAMPLES` is defined by that push count (unit test), not by a sensitivity probe.

## Analysis

- LPC window: **240 samples** (60 before the frame, the 120 of the frame, 60 look-ahead), G.729's asymmetric window verbatim (S-009 §3.2.1): w[n] = 0.54 − 0.46·cos(2πn/399) for n = 0..199, w[n] = cos(2π(n − 200)/159) for n = 200..239. The encoder keeps 60 past samples for it and 240 past weighted-speech samples for the open-loop pitch at lag 120 over a 120-sample frame.
- Autocorrelation r[0..8]; white-noise correction r[0] × 1.0001; r[0] floored at 1 (Q15 units) so all-zero input is defined; lag window w_lag[k] = exp(−½(2π·60·k/6000)²) (a 60 Hz bandwidth expansion, **in hertz**, FR-001).
- Levinson–Durbin in Q31 accumulators; a non-minimum-phase result (|reflection coefficient| ≥ 1 at any order) keeps the previous frame's coefficients (unit-tested with a crafted autocorrelation).
- LPC → LSP by the Chebyshev-polynomial root search (S-009 §3.2.3): 60 grid points on [0, π], 4 bisection steps; fewer than 8 roots → the previous frame's LSPs (unit-tested with a crafted polynomial).
- LSP interpolation: subframe 1 uses (previous + current)/2 of the quantized LSPs, subframe 2 the current.
- The weighting and postfilter factors are **kept as published** (dimensionless; they describe formant bandwidths, which do not scale with the sampling rate), while the lag window is conserved in hertz; FR-001 says so (plan reviews, P6).

## Quantizers (analytic tables, with one borrowed set of coefficients)

- **LSP**: 22 bits as predicted scalar differences with **companded steps**. Mean vector m_i = i·π/9, i = 1..8; prediction p_i = m_i + 0.6·(q̂_i(prev) − m_i); residual d_i = ω_i − p_i quantized with b_i = (3, 3, 3, 3, 3, 3, 2, 2) bits to the companded levels ±s_i·(2^|k| − 1), k = 0 … 2^(b_i − 1) − 1 (3 bits: 0, ±s, ±3s, ±7s, −15s; 2 bits: 0, ±s, −3s), with s_i printed by the tool from the analytic LSP range so that the largest level covers a full-range formant jump in one frame; the decoded LSPs are re-ordered ascending with a **minimum spacing of 0.04 rad** (≈ 38 Hz; G.729 uses 0.0391 rad) and clamped into (0.04, π − 0.04). The tool prints an analytic spectral-distortion estimate for the table. (The alternative, log-scale quantization of the eight LSP gaps, is the tool's second mode and the first lever if the LSD margin fails, R-13.)
- **Pitch lag**: lags 15 – 120 in **half-sample resolution over the whole range** in the decoder (the interpolator exists); the 7-bit subframe-1 code is half-sample for 15 ≤ lag < 36 (codes 0..41 ↦ 15, 15.5, …, 35.5) and integer for 36 – 120 (codes 42..126); **code 127 unused** (never produced; refused on import). Subframe 2: a 5-bit two's-complement delta in half-samples (−16..+15) from subframe 1's lag, clamped into 15 – 120. Lags shorter than the subframe (< 60) use the periodic extension of the past excitation (S-009 §3.7.1) in search and decoder.
- **Fixed codebook** (S-008 §2.16): four pulses, pulse k ∈ 0..3 on track k with positions 2k + 8m (m = 0..7, 3 bits) plus a grid bit g ∈ {0, 1} adding g to every position; a sign bit per pulse; a pulse whose position ≥ 60 (tracks 2 and 3 with m = 7) is not placed. 17 bits per subframe; every code decodes.
- **Adaptive gain**: 3 bits ↦ {0.0, 0.2, 0.4, 0.6, 0.8, 1.0, 1.2, 1.4} (Q14).
- **Fixed gain**: a 3-bit correction index i ∈ 0..7 giving −12 … +16 dB in 4 dB steps about a predicted excitation energy; the predictor is a 4th-order moving average of the past quantized corrections with **G.729's coefficients (0.68, 0.58, 0.34, 0.19), borrowed from S-009 §3.9, not designed here**; its mean energy and clamp are **derived by the tool from the project's level anchor** (A-029: speech at −10 … −40 dBFS; the device's coder sees µ-law at these levels) so that a −10 dBFS noise-like frame converges strictly inside the correction range (index 1 … 6) with ≥ 6 dB of headroom at the top (mean 60 dB, clamp ±22 dB in the tool's 16-bit energy units; the tool prints required against available energy at both ends); the −40 dBFS strongly voiced end (≈ 30 dB prediction gain) is reachable with the lowest correction held and is **reported** (the tool's margin arithmetic assumed the clamp and the −12 dB step together, which the moving average cannot sustain, and reads −N dBFS as amplitude, 3 dB above AES17; hardware review T053). A unit test asserts the −10 dBFS end on the predictor model and on the encoder (every correction index strictly inside 0 … 7 after convergence); the harness's −40 dBFS transparency rows measure the bottom end on the encoder (within 0.4 dB RMS). The silence floor follows from the lowest step (expected near −77 dBFS; FR-006's bound is −60).

## Searches (encoder; fixed trip counts)

- Weighting filter W(z) = A(z/0.9)/A(z/0.5) per subframe (S-008 §2.8, factors as published).
- Open-loop pitch per frame on the weighted speech (240 past samples kept): the best normalized correlation in each of the ranges 15 – 29, 30 – 59, 60 – 120, with the shorter-range candidate preferred when within 0.85 of the longer (S-009 §3.4 rule adapted).
- Closed-loop lag per subframe: integer lags within ±3 of the open-loop estimate (subframe 1) or of subframe 1's lag (subframe 2, within the delta range), half-sample refinement by a 12-tap windowed-sinc interpolator; the adaptive gain is the clipped (0 – 1.4) normalized correlation, then quantized.
- Fixed codebook: for each grid, eight passes with pulse 0 at each of its positions and pulses 1 – 3 chosen greedily by the maximum of (correlation² / energy) given the earlier pulses; the best of 16 candidates wins; **ties resolve to the lowest code**. A deliberately simple search; its figure is reported, not optimized.
- Gains: the adaptive index minimizing the weighted error with the fixed gain at its predicted value, then the fixed index minimizing the weighted error given the adaptive gain (sequential; 8 + 8 evaluations).

## Decoder

- Excitation u = ĝ_a·v(lag) + ĝ_c·c(code); synthesis 1/Â(z) per subframe; past-excitation update.
- Postfilter: H(z) = Â(z/0.65)/Â(z/0.75) (factors as published, S-008 §3.6); tilt compensation 1 − μz⁻¹ in the form of S-008's reference code (`Spf`): k₁ = R(1)/R(0) of the synthesized speech entering the postfilter, smoothed p = 0.75·p + 0.25·k₁, and μ = 0.5·p (the reference's `PreCoef` is −0.25·2; the tool's 0.25 is the smoother's weight and half the coefficient; *implementation note, T019/T021: the first wording, "μ = 0.25·k₁ of the postfilter numerator", gave the opposite sign and a third of the strength, which the US1 transparency check caught as a 2 – 3 dB loss above 1.2 kHz; to record in the spec at T055 and for the hardware review*); gain scaling to the subframe energy before the postfilter with a one-pole smoother (0.9). No pitch postfilter in this model version (A-041; G.723.1's shapes its rendering of noise and is a candidate for a later model version).
- Output Q15 → f32 by ÷ 32768; the decoder adds no delay.

## Arithmetic (research R-03)

Q15 samples and coefficients in `i32`, saturated to ±32767 on store; Q31 accumulators in `i64`; `mult(a, b) = (a·b + 2^14) >> 15` saturated; `div(a, b)` for 0 ≤ a < b by 16 restoring steps; `norm` by leading-zero count; rounding to nearest, ties away from zero. **Every arithmetic operation in `codec/` is an explicit `saturating_*`, `wrapping_*` or `checked_*` method or a range-proven primitive** (clippy `arithmetic_side_effects` is denied for the module), so debug and release builds agree; one frame-byte digest is checked in the release profile (the release timing job). Input conversion f32 → Q15 without `std`: `let s = x * 32768.0; let r = if s >= 0.0 { s + 0.5 } else { s - 0.5 }; (r as i32).clamp(-32768, 32767)` (the `as` cast truncates toward zero; `f32::round` is not in `core` at MSRV 1.85). Every operator has a unit test for its saturating branch.

## Bit order in the frame

Byte 0 holds bits 79..72 (MSB first). Fields in order: LSP 1..8 (22), lag 1 (7), codebook 1 (positions k = 0..3 × 3 bits, signs k = 0..3, grid; 17), adaptive gain 1 (3), fixed gain 1 (3), lag delta 2 (5), codebook 2 (17), adaptive gain 2 (3), fixed gain 2 (3) = 80. The packer and the unpacker are driven by one `ALLOCATION: [(Field, u8); 15]` table; a unit test checks the sum is 80 and a round trip of every field at its extreme values; a second test decodes 10 000 seeded random frames, the all-zero and the all-one frame through a fresh decoder and a decoder with state, asserting finite output with |y| ≤ 1.0 (FR-010).
