# Contract: the codec design constants and the 80-bit frame (feature 008; research R-02)

Every figure here is a **design constant** of the model, printed by `tools/celp-design/design_celp.py` and pinned by a core unit test. None is a measurement of, or a claim about, the RR-DR60's own coder (A-004, A-040, A-041). The model is "modeled on a 4 kbit/s CELP-class coder of the period" (S-008 – S-011 as secondary sources for the class).

## Rates and sizes

| Constant | Value |
|---|---|
| Device rate | 6000 Hz (A-026) |
| Frame | 120 samples, 20 ms; 50 frames per second |
| Subframe | 60 samples, 10 ms; two per frame |
| Bits per frame | 80 (10 bytes); 80 × 50 = 4000 bit/s exactly (FR-003; frame payload, A-040) |
| Look-ahead | 30 samples, 5 ms |
| Algorithmic delay | 150 samples, 25 ms (attributed to the record path and the one-pass; the decoder adds none) |
| LPC order | 8 |

## Analysis

- LPC window: 180 samples (30 before the frame, the 120 of the frame, 30 look-ahead), asymmetric: w[n] = 0.54 − 0.46·cos(2πn/299) for n = 0..149, w[n] = cos(2π(n − 150)/120) for n = 150..179 (the G.729 shape, S-009, scaled).
- Autocorrelation r[0..8]; white-noise correction r[0] × 1.0001; r[0] floored at 1 (Q15 units) so all-zero input is defined; lag window w_lag[k] = exp(−½(2π·60·k/6000)²) (60 Hz bandwidth expansion).
- Levinson–Durbin in Q31 accumulators; a non-minimum-phase result (|reflection coefficient| ≥ 1) keeps the previous frame's coefficients.
- LPC → LSP by the Chebyshev-polynomial root search (S-009 §3.2.3): 60 grid points on [0, π], 4 bisection steps; fewer than 8 roots → the previous frame's LSPs.
- LSP interpolation: subframe 1 uses (previous + current)/2 of the quantized LSPs, subframe 2 the current.

## Quantizers (all analytic; designed, not trained)

- **LSP**: mean vector m_i = i·π/9, i = 1..8; prediction p_i = m_i + 0.6·(q̂_i(prev) − m_i); residual d_i = ω_i − p_i quantized uniformly with step s_i and bits b_i = (3, 3, 3, 3, 3, 3, 2, 2): the step covers ±(range_i)/2 where range_i is the tool-printed span of the residual over the analytic LSP range; indices 0..2^b − 1; decoded LSPs re-ordered ascending with a minimum spacing of 0.02 rad (clamped into (0.02, π − 0.02)).
- **Pitch lag**: lags 15 – 120; half-sample resolution for 15 ≤ lag < 36 (codes 0..41 ↦ 15, 15.5, …, 35.5), integer for 36 – 120 (codes 42..126); **code 127 unused** (never produced; refused on import). Subframe 2: a 5-bit two's-complement delta in half-samples (−16..+15) from subframe 1's lag, clamped into 15 – 120 after decoding.
- **Fixed codebook** (S-008 §2.16): four pulses, pulse k ∈ 0..3 on track k with positions 2k + 8m (m = 0..7, 3 bits) plus a grid bit g ∈ {0, 1} adding g to every position; sign bit per pulse (+1 / −1); a pulse whose position ≥ 60 (tracks 2 and 3 with m = 7) is not placed. 17 bits per subframe; every code decodes.
- **Adaptive gain**: 3 bits ↦ {0.0, 0.2, 0.4, 0.6, 0.8, 1.0, 1.2, 1.4} (Q14).
- **Fixed gain**: g_c = 10^((E_pred + 30 + 4·(i − 3)) / 20) / sqrt(E_c) in log form: a 3-bit correction index i ∈ 0..7 giving −12 … +16 dB about the predicted energy; E_pred is a 4th-order moving average of the past quantized corrections with coefficients (0.68, 0.58, 0.34, 0.19) (S-009 §3.9 structure), clamped to ±14 dB; E_c is the energy of the fixed codebook vector.

## Searches (encoder; fixed trip counts)

- Weighting filter W(z) = A(z/0.9)/A(z/0.5) per subframe (S-008 §2.8).
- Open-loop pitch per frame on the weighted speech: the best normalized correlation in each of the ranges 15 – 29, 30 – 59, 60 – 120, with the shorter-range candidate preferred when within 0.85 of the longer (S-009 §3.4 rule adapted).
- Closed-loop lag per subframe: integer lags within ±3 of the open-loop estimate (subframe 1) or of subframe 1's lag (subframe 2, within the delta range), half-sample refinement for lags < 36 by a 12-tap windowed-sinc interpolator; the adaptive gain is the clipped (0 – 1.4) normalized correlation, then quantized.
- Fixed codebook: for each grid, eight passes with pulse 0 at each of its positions and pulses 1 – 3 chosen greedily by the maximum of (correlation² / energy) given the earlier pulses; the best of 16 candidates wins; **ties resolve to the lowest code**.
- Gains: the adaptive index minimizing the weighted error with the fixed gain at its predicted value, then the fixed index minimizing the weighted error given the adaptive gain (sequential; 8 + 8 evaluations).

## Decoder

- Excitation u = ĝ_a·v(lag) + ĝ_c·c(code); synthesis 1/Â(z) per subframe; past-excitation update.
- Postfilter: H(z) = Â(z/0.65)/Â(z/0.75); tilt compensation 1 − μz⁻¹ with μ = 0.25·k₁ (k₁ the first reflection coefficient of the postfilter numerator); gain scaling to the subframe energy before the postfilter with a one-pole smoother (0.9). No pitch postfilter (A-041).
- Output Q15 → f32 by ÷ 32768.

## Arithmetic (research R-03)

Q15 samples and coefficients in `i32` with saturation to ±32767 on store; Q31 accumulators in `i64`; `mult(a, b) = (a·b + 2^14) >> 15` saturated; `div(a, b)` for 0 ≤ a < b by 16 restoring steps; `norm` by leading-zero count; rounding to nearest, ties away from zero. Every operator has a unit test for its saturating branch.

## Bit order in the frame

Byte 0 holds bits 79..72 (MSB first). Fields in order: LSP 1..8 (22), lag 1 (7), codebook 1 (positions k = 0..3 × 3 bits, signs k = 0..3, grid; 17), adaptive gain 1 (3), fixed gain 1 (3), lag delta 2 (5), codebook 2 (17), adaptive gain 2 (3), fixed gain 2 (3) = 80. The packer and the unpacker are driven by one `ALLOCATION: [(Field, u8); 15]` table; a unit test checks the sum is 80 and a round trip of every field at its extreme values.
