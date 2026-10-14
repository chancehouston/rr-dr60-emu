# /// script
# requires-python = ">=3.10"
# dependencies = ["numpy>=1.24"]
# ///
"""Design tool for the rr-dr60-emu speech codec (signal-chain stages 7 and 9; spec 008).

Every figure this script prints is a **design constant of the model**, not a measurement of, or
a claim about, the Panasonic RR-DR60's own coder. The service manual (S-002) sources only the
coder's class (CELP), its bit rate (4 kbit/s) and its sampling rate (6 kHz, A-026); the
algorithm is unknown (A-004, adopted by spec 008 as a class match). The model is an algebraic
CELP coder of the ITU-T G.729 / G.723.1 family (S-009, S-008; secondary sources for the class)
at 20 ms frames of 80 bits, adapted to 6 kHz (research R-02; contracts/codec-design.md):

- frame 120 samples, two 60-sample subframes, look-ahead 60, delay 180 (A-040);
- LPC order 8, G.729's asymmetric 240-sample window and 60 Hz lag window (S-009 §3.2.1);
- LSPs quantized as predicted scalar differences with companded levels (22 bits; analytic);
- pitch lags 15 – 120 with half-sample resolution below 36 (7 + 5 bits; S-009 §3.7);
- G.723.1's 17-bit four-pulse codebook per 60-sample subframe (S-008 §2.16);
- two scalar gains per subframe (3 + 3 bits) with a moving-average energy predictor whose four
  coefficients are G.729's (S-009 §3.9; borrowed, not designed here) and whose mean and clamp
  are derived from the project's level anchor (A-029: speech at −10 … −40 dBFS);
- weighting filter A(z/0.9)/A(z/0.5) and postfilter A(z/0.65)/A(z/0.75), factors as published
  (S-008 §2.8, §3.6; dimensionless, so kept; spec FR-001 as amended).

Usage:
  uv run tools/celp-design/design_celp.py            # print every design figure
  uv run tools/celp-design/design_celp.py --check    # the bit budget and the bit order
  uv run tools/celp-design/design_celp.py --write    # regenerate crates/rr_dr60/src/codec/tables.rs
  uv run tools/celp-design/design_celp.py --lsp-mode gaps   # the alternative LSP quantizer (R-13)

The generated `tables.rs` is byte-deterministic: `--write && git diff --exit-code` proves it.
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

import numpy as np

# ---------------------------------------------------------------- rates and sizes (R-02, A-040)
DEVICE_RATE_HZ = 6000  # A-026
FRAME_SAMPLES = 120  # 20 ms; a multiple of 12 (whole bytes) and of 20 (host-sample delay)
SUBFRAME_SAMPLES = 60  # 10 ms; G.723.1's subframe length, so its codebook fits unchanged
LOOKAHEAD_SAMPLES = 60  # 10 ms
DELAY_SAMPLES = FRAME_SAMPLES + LOOKAHEAD_SAMPLES  # 180 = 30 ms
BITS_PER_FRAME = 2 * FRAME_SAMPLES // 3  # 80 (FR-003: 80 × 50 frames/s = 4000 bit/s)
FRAME_BYTES = BITS_PER_FRAME // 8
LPC_ORDER = 8
MIN_LAG, MAX_LAG = 15, 120  # 50 – 400 Hz at 6 kHz (S-009's 50 – 400 Hz kept in hertz)
HALF_SAMPLE_BELOW = 36  # half-sample lag resolution for lags < 36 in subframe 1's code
WINDOW_SAMPLES = 240  # 60 past + 120 frame + 60 look-ahead
PAST_WEIGHTED = 240  # open-loop pitch at lag 120 over a 120-sample frame

HOST_RATES = [6000, 8000, 16000, 44100, 48000, 88200, 96000]
# Spec 006's one-pass latencies (host samples), measured 2026-10-12 by the harness's latency test.
ONE_PASS_006 = {6000: 1, 8000: 81, 16000: 162, 44100: 444, 48000: 483, 88200: 887, 96000: 966}

# ---------------------------------------------------------------- the 80-bit allocation (R-02)
LSP_BITS = (3, 3, 3, 3, 3, 3, 2, 2)  # 22
LAG1_BITS, LAG2_BITS = 7, 5  # 12
PULSE_POS_BITS, PULSES, SIGN_BITS, GRID_BITS = 3, 4, 4, 1  # 17 per subframe (S-008 §2.16)
CODEBOOK_BITS = PULSE_POS_BITS * PULSES + SIGN_BITS + GRID_BITS
ADAPT_GAIN_BITS, FIXED_GAIN_BITS = 3, 3  # 6 per subframe
ALLOCATION = (
    [("lsp%d" % (i + 1), b) for i, b in enumerate(LSP_BITS)]
    + [("lag1", LAG1_BITS), ("codebook1", CODEBOOK_BITS), ("adapt_gain1", ADAPT_GAIN_BITS),
       ("fixed_gain1", FIXED_GAIN_BITS), ("lag2", LAG2_BITS), ("codebook2", CODEBOOK_BITS),
       ("adapt_gain2", ADAPT_GAIN_BITS), ("fixed_gain2", FIXED_GAIN_BITS)]
)

# ---------------------------------------------------------------- analysis constants
LAG_WINDOW_HZ = 60.0  # bandwidth expansion, in hertz (conserved, FR-001)
WHITE_NOISE_CORRECTION = 1.0001  # S-009 §3.2.1
CHEB_GRID_POINTS = 60  # root search grid on [0, π] (S-009 §3.2.3 uses 50 + endpoints)
COS_TABLE_POINTS = 256  # angle → cosine lookup with interpolation (257 entries)
GAMMA_W1, GAMMA_W2 = 0.9, 0.5  # weighting filter (S-008 §2.8)
GAMMA_PN, GAMMA_PD = 0.65, 0.75  # postfilter (S-008 §3.6)
TILT_FACTOR = 0.25
POST_GAIN_SMOOTH = 0.9

# ---------------------------------------------------------------- quantizers
LSP_PRED = 0.6  # first-order prediction toward the mean vector
LSP_MIN_GAP_RAD = 0.04  # ≈ 38 Hz (G.729: 0.0391 rad at 8 kHz = 50 Hz)
ADAPT_GAIN_LEVELS = [0.0, 0.2, 0.4, 0.6, 0.8, 1.0, 1.2, 1.4]
FIXED_GAIN_STEP_DB = 4
FIXED_GAIN_CORR_DB = [-12 + FIXED_GAIN_STEP_DB * i for i in range(8)]  # −12 … +16
PRED_COEF = [0.68, 0.58, 0.34, 0.19]  # G.729 §3.9, borrowed (S-009)
PULSES_PER_SUBFRAME_ENERGY = 4.0  # Σc² of a four-pulse ±1 codeword
LEVEL_ANCHOR_DBFS = (-10.0, -40.0)  # A-029: speech at −10 … −40 dBFS
ANCHOR_MARGIN_DB = 6.0


def q15(x: float) -> int:
    return max(-32768, min(32767, int(round(x * 32768.0))))


def q14(x: float) -> int:
    return max(-32768, min(32767, int(round(x * 16384.0))))


# ---------------------------------------------------------------- tables
def lpc_window() -> np.ndarray:
    """G.729's asymmetric window (S-009 §3.2.1), 240 samples: a half-Hamming rise over 200
    samples, a quarter-cosine fall over 40."""
    n = np.arange(WINDOW_SAMPLES)
    w = np.where(n < 200, 0.54 - 0.46 * np.cos(2 * np.pi * n / 399.0),
                 np.cos(2 * np.pi * (n - 200) / 159.0))
    return w


def lag_window() -> np.ndarray:
    k = np.arange(1, LPC_ORDER + 1)
    return np.exp(-0.5 * (2 * np.pi * LAG_WINDOW_HZ * k / DEVICE_RATE_HZ) ** 2)


def half_sample_interpolator(taps: int = 12) -> np.ndarray:
    """Windowed-sinc taps for the half-sample phase: positions k + 0.5, k = −6 … 5, Hamming
    window over the 12 taps, normalized to unit sum."""
    k = np.arange(-taps // 2, taps // 2) + 0.5
    h = np.sinc(k) * (0.54 + 0.46 * np.cos(np.pi * k / (taps / 2)))  # Hamming over the 12 taps
    return h / h.sum()


def cheb_grid() -> np.ndarray:
    k = np.arange(CHEB_GRID_POINTS + 1)
    return np.cos(np.pi * k / CHEB_GRID_POINTS)


def cos_table() -> np.ndarray:
    k = np.arange(COS_TABLE_POINTS + 1)
    return np.cos(np.pi * k / COS_TABLE_POINTS)


def lsp_mean() -> np.ndarray:
    return np.array([(i + 1) * np.pi / (LPC_ORDER + 1) for i in range(LPC_ORDER)])


def lsp_residual_bound(i: int) -> float:
    """Analytic bound on the prediction residual of LSP i: the LSP's own half-range about the
    mean (one slot of π/9) plus the predictor's share of the previous frame's excursion."""
    half_range = np.pi / (LPC_ORDER + 1)
    return (1.0 + LSP_PRED) * half_range


def lsp_levels(bits: int, step: float) -> list[float]:
    """Companded levels ±step·(2^|j| − 1) for the two's-complement index j (3 bits: −15s, −7s,
    −3s, −s, 0, s, 3s, 7s; 2 bits: −3s, −s, 0, s). The small levels give stationary speech a
    fine step; the large ones carry a formant jump in one or two frames (plan review P4)."""
    lo, hi = -(2 ** (bits - 1)), 2 ** (bits - 1)
    return [math.copysign(step * (2 ** abs(j) - 1), j) if j else 0.0 for j in range(lo, hi)]


def lsp_steps() -> list[float]:
    """The step of each LSP's companded quantizer: the largest positive level (7s for 3 bits,
    s for 2 bits) covers half the analytic residual bound, so a full-range jump converges in
    two frames while the first level stays fine (≈ 38 Hz for 3 bits)."""
    steps = []
    for b in LSP_BITS:
        top = 2 ** (2 ** (b - 1) - 1) - 1  # 7 for 3 bits, 1 for 2 bits
        divisor = 2 * top if b >= 3 else 6  # 2-bit indices: the −3s level covers the bound
        steps.append(lsp_residual_bound(0) / divisor)
    return steps


def lsp_gap_levels(bits: int) -> list[float]:
    """The alternative quantizer (R-13 lever): log-scale levels for the eight LSP gaps."""
    lo, hi = LSP_MIN_GAP_RAD, 2.5 * np.pi / (LPC_ORDER + 1)
    return list(np.geomspace(lo, hi, 2 ** bits))


def lsp_sd_estimate(steps: list[float]) -> float:
    """A rough analytic spectral-distortion proxy: the RMS LSP error in Hz if residuals are
    uniform within the first level (the common case)."""
    errs = [(s / 2) / math.sqrt(3) * DEVICE_RATE_HZ / (2 * np.pi) for s in steps]
    return math.sqrt(sum(e * e for e in errs) / len(errs))


def fixed_gain_design() -> dict:
    """Mean and clamp of the fixed-gain predictor, derived from the level anchor.

    The model's fixed gain is G = MEAN + E_pred + corr(i) in dB (20·log10 of the pulse
    amplitude in Q15 units), corr ∈ FIXED_GAIN_CORR_DB, E_pred the MA prediction clamped to
    ±CLAMP. The excitation RMS needed is the residual RMS of the input; the pulse amplitude is
    that RMS × sqrt(N / Σc²) = RMS × sqrt(60 / 4). Required range: a −10 dBFS noise-like frame
    (prediction gain ≈ 3 dB) at the top, a −40 dBFS strongly voiced frame (prediction gain up to
    30 dB) at the bottom.

    The *sustainable* range (hardware review T053 B1): the moving average holds at most
    Σcoef × the extreme correction (±12·1.79 = −21.5 / +28.6 dB, the clamp binding only at the
    top), so the bottom is MEAN − 21.5 − 12 and the top MEAN + 22 + 16, further bounded by Q15
    pulse saturation at 90.3 dB. "−N dBFS" is a sine of peak amplitude 10^(−N/20) full scale,
    so its RMS is 1/√2 of that (AES17; the first draft read it as an amplitude, 3 dB high,
    assumed the clamp and the −12 dB step stack and claimed 6.1 / 8.9 dB; the true margins
    at mean 60 were 2.5 / 4.2). The rule: ≥ ANCHOR_MARGIN_DB at the bottom; the top is bounded
    by saturation and its margin is reported."""
    full_scale = 32768.0
    pulse_factor = 20 * math.log10(math.sqrt(SUBFRAME_SAMPLES / PULSES_PER_SUBFRAME_ENERGY))
    hi_rms = full_scale * 10 ** (LEVEL_ANCHOR_DBFS[0] / 20) / math.sqrt(2)  # −10 dBFS, AES17 RMS
    lo_rms = full_scale * 10 ** (LEVEL_ANCHOR_DBFS[1] / 20) / math.sqrt(2)  # −40 dBFS, AES17 RMS
    required_hi = 20 * math.log10(hi_rms) - 3.0 + pulse_factor  # noise: ~3 dB prediction gain
    required_lo = 20 * math.log10(lo_rms) - 30.0 + pulse_factor  # voiced: up to 30 dB
    mean_db = 54  # T053 B1: 60 → 54 for ≥ 6 dB at the bottom of the anchor
    clamp_db = 22
    coef_sum = sum(PRED_COEF)  # 1.79
    sat_db = 20 * math.log10(32767.0)  # a single pulse saturates Q15 above this
    bottom = mean_db + max(-clamp_db, FIXED_GAIN_CORR_DB[0] * coef_sum) + FIXED_GAIN_CORR_DB[0]
    top = min(mean_db + min(clamp_db, FIXED_GAIN_CORR_DB[-1] * coef_sum) + FIXED_GAIN_CORR_DB[-1], sat_db)
    available = (bottom, top)
    return dict(mean_db=mean_db, clamp_db=clamp_db, available=available,
                required=(required_lo, required_hi), pulse_factor_db=pulse_factor,
                margin_lo=required_lo - available[0], margin_hi=available[1] - required_hi,
                saturation_db=sat_db,
                silence_floor_dbfs=available[0] - pulse_factor - 20 * math.log10(full_scale))


def pitch_code_map() -> list[int]:
    """Subframe-1 lag codes → lag in half-samples: codes 0..41 ↦ 30..71 (15 – 35.5),
    codes 42..126 ↦ 72, 74, …, 240 (36 – 120); code 127 unused."""
    codes = [30 + c for c in range(42)] + [72 + 2 * (c - 42) for c in range(42, 127)]
    assert len(codes) == 127 and codes[-1] == 2 * MAX_LAG and codes[0] == 2 * MIN_LAG
    return codes


def pulse_tracks() -> list[list[int]]:
    """Track k holds positions 2k + 8m, m = 0..7 (S-008 §2.16); the grid bit adds one to every
    position; a position ≥ 60 is 'not placed'."""
    return [[2 * k + 8 * m for m in range(8)] for k in range(PULSES)]


def latency_table() -> list[tuple[int, int, int, int, float]]:
    rows = []
    for rate in HOST_RATES:
        delay = DELAY_SAMPLES * rate // DEVICE_RATE_HZ
        assert DELAY_SAMPLES * rate % DEVICE_RATE_HZ == 0, rate
        total = ONE_PASS_006[rate] + delay
        rows.append((rate, ONE_PASS_006[rate], delay, total, total / rate * 1000.0))
    return rows


def db_to_log2() -> float:
    return math.log2(10.0) / 20.0  # 0.16610


def pow2_frac_table(points: int = 32) -> np.ndarray:
    return 2.0 ** (np.arange(points + 1) / points)  # 1.0 … 2.0


# ---------------------------------------------------------------- printing
def print_design(lsp_mode: str) -> None:
    print("rr-dr60-emu speech codec: design constants (spec 008 R-02; A-040, A-041; not device figures)")
    print(f"device rate {DEVICE_RATE_HZ} Hz (A-026); frame {FRAME_SAMPLES} samples = {FRAME_SAMPLES * 1000 // DEVICE_RATE_HZ} ms; "
          f"subframe {SUBFRAME_SAMPLES}; look-ahead {LOOKAHEAD_SAMPLES}; delay {DELAY_SAMPLES} = {DELAY_SAMPLES * 1000 / DEVICE_RATE_HZ:.0f} ms; "
          f"bits/frame {BITS_PER_FRAME} ({FRAME_BYTES} bytes); {DEVICE_RATE_HZ // FRAME_SAMPLES} frames/s × {BITS_PER_FRAME} = "
          f"{DEVICE_RATE_HZ // FRAME_SAMPLES * BITS_PER_FRAME} bit/s; LPC order {LPC_ORDER}")
    print(f"delay {DELAY_SAMPLES} mod 20 = {DELAY_SAMPLES % 20} (a whole number of host samples at every rate)")
    print("\nlatency table (host samples): rate, one-pass 006, codec delay, one-pass 008, ms")
    for rate, base, delay, total, ms in latency_table():
        print(f"  {rate:>6} {base:>5} {delay:>5} {total:>5} {ms:7.2f}")
    print("\nbit allocation:")
    for name, b in ALLOCATION:
        print(f"  {name:<12} {b}")
    print(f"  total        {sum(b for _, b in ALLOCATION)}")
    print(f"\nLPC window: {WINDOW_SAMPLES} samples, peak at n=199 (value {lpc_window()[199]:.4f}), w[0]={lpc_window()[0]:.3f}, w[239]={lpc_window()[239]:.4f}")
    print("lag window:", " ".join(f"{v:.5f}" for v in lag_window()), f"(white-noise correction {WHITE_NOISE_CORRECTION})")
    print("half-sample interpolator:", " ".join(f"{v:+.5f}" for v in half_sample_interpolator()))
    codes = pitch_code_map()
    print(f"\npitch: lags {MIN_LAG}–{MAX_LAG}, half-sample below {HALF_SAMPLE_BELOW}; {len(codes)} codes "
          f"(42 half-sample + 85 integer); code 127 unused; delta 5 bits = −16…+15 half-samples")
    print("pulse tracks (grid 0):", pulse_tracks(), "+ grid adds 1; positions ≥ 60 not placed (tracks 2, 3 at m = 7)")
    print("adaptive gain (Q14):", [q14(v) for v in ADAPT_GAIN_LEVELS])
    g = fixed_gain_design()
    print(f"fixed gain: corrections {FIXED_GAIN_CORR_DB} dB; predictor {PRED_COEF} (G.729, borrowed); "
          f"mean {g['mean_db']} dB, clamp ±{g['clamp_db']} dB → available [{g['available'][0]}, {g['available'][1]}] dB of 20log10(pulse amplitude)")
    print(f"  required: −40 dBFS voiced (30 dB prediction gain) {g['required'][0]:.1f} dB, −10 dBFS noise (3 dB) {g['required'][1]:.1f} dB; "
          f"margins {g['margin_lo']:.1f} dB at the bottom (≥ {ANCHOR_MARGIN_DB} required) / {g['margin_hi']:.1f} dB at the top (bounded by Q15 pulse saturation at {g['saturation_db']:.1f} dB; reported)")
    print(f"  silence floor (lowest step, one pulse per 60): about {g['silence_floor_dbfs']:.0f} dBFS (spec bound −60)")
    steps = lsp_steps()
    print(f"\nLSP quantizer ({lsp_mode}): mean vector i·π/9 = {[round(float(v), 4) for v in lsp_mean()]}; prediction {LSP_PRED}; min gap {LSP_MIN_GAP_RAD} rad")
    if lsp_mode == "companded":
        for i, (b, s) in enumerate(zip(LSP_BITS, steps)):
            print(f"  lsp{i + 1}: {b} bits, step {s:.4f} rad ({s * DEVICE_RATE_HZ / (2 * np.pi):.1f} Hz), levels {[round(float(v), 3) for v in lsp_levels(b, s)]}")
        print(f"  analytic RMS LSP error proxy: {lsp_sd_estimate(steps):.1f} Hz")
    else:
        for i, b in enumerate(LSP_BITS):
            print(f"  gap{i + 1}: {b} bits, log levels {[round(float(v), 3) for v in lsp_gap_levels(b)]} rad")
    print(f"\nweighting A(z/{GAMMA_W1})/A(z/{GAMMA_W2}); postfilter A(z/{GAMMA_PN})/A(z/{GAMMA_PD}), tilt {TILT_FACTOR}·k1, gain smoother {POST_GAIN_SMOOTH} (factors as published)")


def check() -> int:
    total = sum(b for _, b in ALLOCATION)
    ok = total == BITS_PER_FRAME
    print(f"bit budget: {total} of {BITS_PER_FRAME} → {'OK' if ok else 'MISMATCH'}")
    print("bit order (MSB of byte 0 first):", " | ".join(f"{n}:{b}" for n, b in ALLOCATION))
    assert FRAME_SAMPLES % 12 == 0 and DELAY_SAMPLES % 20 == 0 and BITS_PER_FRAME * (DEVICE_RATE_HZ // FRAME_SAMPLES) == 4000
    g = fixed_gain_design()
    assert g["margin_lo"] >= ANCHOR_MARGIN_DB and g["margin_hi"] > 0, g  # T053 B1
    return 0 if ok else 1


# ---------------------------------------------------------------- tables.rs
def rs_array(name: str, ty: str, values, per_line: int = 12) -> str:
    vals = [str(int(v)) for v in values]
    lines = [", ".join(vals[i:i + per_line]) for i in range(0, len(vals), per_line)]
    body = ",\n    ".join(lines)
    # `#[rustfmt::skip]`: the tool's layout is the committed one, so `cargo fmt` leaves it alone and
    # `--write && git diff --exit-code` stays meaningful.
    return f"#[rustfmt::skip]\npub(crate) const {name}: [{ty}; {len(vals)}] = [\n    {body},\n];\n"


def tables_rs() -> str:
    g = fixed_gain_design()
    steps = lsp_steps()
    out = []
    out.append("//! GENERATED by `tools/celp-design/design_celp.py --write`; do not edit by hand.\n")
    out.append("//!\n//! Design constants of the speech codec model (spec 008 research R-02; contracts/codec-design.md;\n")
    out.append("//! A-040, A-041). Modeled on the G.729 / G.723.1 family (S-009, S-008; secondary sources for the\n")
    out.append("//! class); none of these figures is a measurement of the RR-DR60's own coder (A-004).\n")
    out.append("//! Q15 unless stated. Regenerate: `uv run tools/celp-design/design_celp.py --write`.\n\n")
    out.append(f"/// A-026: the device rate in Hz.\npub(crate) const DEVICE_RATE_HZ: u32 = {DEVICE_RATE_HZ};\n")
    out.append(f"/// A-040 (design constant): frame length in device samples (20 ms).\npub(crate) const FRAME_SAMPLES: usize = {FRAME_SAMPLES};\n")
    out.append(f"/// Subframe length in device samples (10 ms; G.723.1's subframe, S-008; A-041).\npub(crate) const SUBFRAME_SAMPLES: usize = {SUBFRAME_SAMPLES};\n")
    out.append(f"/// A-040 (design constant): look-ahead in device samples (10 ms).\npub(crate) const LOOKAHEAD_SAMPLES: usize = {LOOKAHEAD_SAMPLES};\n")
    out.append(f"/// Frame plus look-ahead: the codec's algorithmic delay (a multiple of 20, so a whole number of host\n/// samples at every supported rate; spec FR-004).\npub(crate) const DELAY_SAMPLES: usize = {DELAY_SAMPLES};\n")
    out.append(f"/// FR-003: bits per frame (80 × 50 frames/s = 4000 bit/s exactly, S-002).\npub(crate) const BITS_PER_FRAME: usize = {BITS_PER_FRAME};\n")
    out.append(f"/// Bytes per frame (008 FR-003; A-040).\npub(crate) const FRAME_BYTES: usize = {FRAME_BYTES};\n")
    out.append(f"/// LPC order (three formants fit the 225–2550 Hz band; A-041, A-027).\npub(crate) const LPC_ORDER: usize = {LPC_ORDER};\n")
    out.append(f"/// Pitch lag range in device samples (50 – 400 Hz, S-009's range kept in hertz).\npub(crate) const MIN_LAG: usize = {MIN_LAG};\npub(crate) const MAX_LAG: usize = {MAX_LAG};\n")
    out.append(f"/// Lags below this have half-sample resolution in subframe 1's 7-bit code (A-041; S-009 §3.7).\npub(crate) const HALF_SAMPLE_BELOW: usize = {HALF_SAMPLE_BELOW};\n")
    out.append(f"/// The LPC analysis window length (60 past + frame + 60 look-ahead; A-040, A-041; S-009 §3.2.1).\npub(crate) const WINDOW_SAMPLES: usize = {WINDOW_SAMPLES};\n")
    out.append(f"/// Past weighted speech kept for the open-loop pitch search (A-041; S-009 §3.4).\npub(crate) const PAST_WEIGHTED: usize = {PAST_WEIGHTED};\n\n")
    out.append("/// Bits per field in frame order (MSB of byte 0 first); the packer and unpacker read this (A-041; 008 FR-003).\n")
    out.append("pub(crate) const ALLOCATION: [u8; %d] = [%s];\n\n" % (len(ALLOCATION), ", ".join(str(b) for _, b in ALLOCATION)))
    out.append("/// G.729's asymmetric LPC window (S-009 §3.2.1), Q15.\n" + rs_array("LPC_WINDOW", "i16", [q15(v) for v in lpc_window()]))
    out.append("/// 60 Hz lag window for k = 1..8 (bandwidth expansion in hertz; S-009 §3.2.1; 008 FR-001), Q15.\n" + rs_array("LAG_WINDOW", "i16", [q15(v) for v in lag_window()]))
    out.append(f"/// White-noise correction of r(0): 1.0001 as a Q15 multiplier of the excess (S-009 §3.2.1).\npub(crate) const WN_CORR_EXCESS: i32 = {q15(WHITE_NOISE_CORRECTION - 1.0)};\n")
    out.append("/// Half-sample interpolator (12 taps, windowed sinc), Q15 (A-041; S-009 §3.7.1).\n" + rs_array("INTERP_HALF", "i16", [q15(v) for v in half_sample_interpolator()]))
    out.append("/// Chebyshev root-search grid: cos(π k / 60), k = 0..=60, Q15 (S-009 §3.2.3).\n" + rs_array("CHEB_GRID", "i16", [q15(v) for v in cheb_grid()]))
    out.append("/// cos(π k / 256), k = 0..=256, Q15: angle (Q15 of ω/π) → cosine with linear interpolation (A-041).\n" + rs_array("COS_TABLE", "i16", [q15(v) for v in cos_table()]))
    out.append("/// LSP mean vector i·π/9 as Q15 of ω/π (A-041, design constant).\n" + rs_array("LSP_MEAN", "i16", [q15(v / np.pi) for v in lsp_mean()]))
    out.append("/// LSP quantizer bits per index (A-041; 008 FR-003).\n" + rs_array("LSP_BITS", "u8", LSP_BITS))
    out.append("/// LSP companded quantizer steps as Q15 of ω/π (levels ±step·(2^|j| − 1); A-041, design constant).\n" + rs_array("LSP_STEP", "i16", [q15(s / np.pi) for s in steps]))
    out.append(f"/// LSP prediction coefficient toward the mean, Q15 (A-041, design constant).\npub(crate) const LSP_PRED: i32 = {q15(LSP_PRED)};\n")
    out.append(f"/// Minimum LSP spacing (0.04 rad) as Q15 of ω/π (A-041; S-009 uses 0.0391 rad).\npub(crate) const LSP_MIN_GAP: i32 = {q15(LSP_MIN_GAP_RAD / np.pi)};\n")
    out.append("/// Adaptive-codebook gain levels, Q14 (A-041, design constant).\n" + rs_array("ADAPT_GAIN", "i16", [q14(v) for v in ADAPT_GAIN_LEVELS]))
    out.append("/// Fixed-gain corrections in dB per 3-bit index (A-041, design constant).\n" + rs_array("FIXED_GAIN_CORR_DB", "i16", FIXED_GAIN_CORR_DB))
    out.append(f"/// Fixed-gain predictor mean (dB of the pulse amplitude in Q15 units) and clamp (dB), derived from\n/// the level anchor A-029 (−10 … −40 dBFS): ≥ 6 dB of margin at the bottom; the top is bounded by\n/// Q15 pulse saturation (hardware review T053 B1: mean 60 → 54 dB).\npub(crate) const FIXED_GAIN_MEAN_DB: i32 = {g['mean_db']};\npub(crate) const FIXED_GAIN_CLAMP_DB: i32 = {g['clamp_db']};\n")
    out.append("/// Moving-average predictor coefficients, Q15: G.729's (S-009 §3.9), borrowed.\n" + rs_array("PRED_COEF", "i16", [q15(v) for v in PRED_COEF]))
    out.append(f"/// log2(10)/20 in Q15: dB → log2 (A-041).\npub(crate) const DB_TO_LOG2: i32 = {q15(db_to_log2())};\n")
    out.append("/// 2^(k/32), k = 0..=32, as Q14 mantissas (1.0 … 2.0; A-041).\n" + rs_array("POW2_FRAC", "i16", [q14(v) for v in pow2_frac_table()]))
    out.append("/// Weighting filter factors 0.9^i and 0.5^i, i = 0..=8, Q15 (S-008 §2.8, as published).\n" + rs_array("GAMMA_W1_POW", "i16", [q15(GAMMA_W1 ** i) for i in range(LPC_ORDER + 1)]))
    out.append("/// 0.5^i (S-008 §2.8).\n" + rs_array("GAMMA_W2_POW", "i16", [q15(GAMMA_W2 ** i) for i in range(LPC_ORDER + 1)]))
    out.append("/// Postfilter factors 0.65^i and 0.75^i, i = 0..=8, Q15 (S-008 §3.6, as published).\n" + rs_array("GAMMA_PN_POW", "i16", [q15(GAMMA_PN ** i) for i in range(LPC_ORDER + 1)]))
    out.append("/// 0.75^i (S-008 §3.6).\n" + rs_array("GAMMA_PD_POW", "i16", [q15(GAMMA_PD ** i) for i in range(LPC_ORDER + 1)]))
    out.append(f"/// Tilt compensation factor (μ = TILT_FACTOR · k1), Q15 (S-008 §3.6; A-041).\npub(crate) const TILT_FACTOR: i32 = {q15(TILT_FACTOR)};\n")
    out.append(f"/// Postfilter gain smoother, Q15 (S-008 §3.7; A-041).\npub(crate) const POST_GAIN_SMOOTH: i32 = {q15(POST_GAIN_SMOOTH)};\n")
    out.append("/// Subframe-1 lag codes → lag in half-samples (codes 0..=126; 127 unused; A-041; 008 FR-010).\n" + rs_array("LAG_CODE_TO_HALF", "u8", pitch_code_map(), 16))
    out.append("/// Pulse track positions (grid 0): track k, m = 0..7 (S-008 §2.16).\n")
    tracks = pulse_tracks()
    out.append("#[rustfmt::skip]\npub(crate) const PULSE_TRACKS: [[u8; 8]; 4] = [\n" + "".join(f"    [{', '.join(str(p) for p in t)}],\n" for t in tracks) + "];\n")
    return "".join(out)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--check", action="store_true", help="print the bit budget and the bit order")
    ap.add_argument("--write", action="store_true", help="regenerate crates/rr_dr60/src/codec/tables.rs")
    ap.add_argument("--lsp-mode", choices=["companded", "gaps"], default="companded")
    args = ap.parse_args()
    if args.check:
        return check()
    if args.write:
        root = Path(__file__).resolve().parents[2]
        target = root / "crates" / "rr_dr60" / "src" / "codec" / "tables.rs"
        target.write_text(tables_rs(), encoding="utf-8")
        print(f"wrote {target.relative_to(root)}")
        return 0
    print_design(args.lsp_mode)
    return 0


if __name__ == "__main__":
    sys.exit(main())
