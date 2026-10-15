# /// script
# requires-python = ">=3.10"
# dependencies = ["numpy>=1.24", "scipy>=1.10"]
# ///
"""Design tool for the rr-dr60-emu output chain (signal-chain stages 11, 12 and 13; spec 009).

Every figure this script prints is a **design constant of the model**: the circuit as read from the
service manual's schematic sheet 3.1 (S-002) and the amplifier datasheet (S-006), reduced to
first- and second-order sections, a gain law and a clipper. Nothing here is a measurement of a
real RR-DR60 (constitution Principle II; spec 009 FR-018). The register IDs:

- A-031 (adopted by spec 009): the 50 kΩ linear volume control, the NJM2113 amplifier's gain
  2·Rf/Ri = 2·68/10 (22.7 dB at full travel), its clip ceiling ≈ 1.15 V peak differential at the
  2.8 V battery rail the schematic marks (1.0 – 1.4 V registered), the 32 Hz (C14 into VR1) and 159 Hz
  (C8 into R16) input coupling corners, the symmetric knee as a design choice bracketed by the
  datasheet's distortion figures;
- A-044: the position is the control's wiper fraction and the gains follow the loaded law
  (the wiper resistance 50 kΩ·p(1−p) in series with the amplifier's input resistor, and
  dividing against the earphone branch's input impedance); the 159 Hz corner's shift with the
  wiper is not modeled;
- A-043: the default position is the reference position where codec full scale (1.0 V peak,
  A-030) meets the clip ceiling under the loaded law;
- A-042: each route's output is scaled to its own full swing (speaker: the ceiling; earphone:
  the volume control's full swing);
- A-032 (adopted): the earphone follower's two coupling corners, 120 Hz nominal (65 – 130) at
  its input and 280 Hz nominal (240 – 320) at its output into 16 Ω (the 2SD1819A S rank);
- A-010 (revised): the speaker is a **template** of a small enclosed driver: a second-order
  roll-off resonant at 750 Hz with Q 1.8 and a first-order +4.5 dB shelf (zero 780 Hz, pole
  1310 Hz), peak-normalized to 1.0 at each host rate; low confidence; never a measured driver.

Realization (research R-03, R-04): bilinear sections with the corner prewarped at each of the
seven host rates (the shelf not prewarped), emitted as exact `f64` bit patterns so the Rust
build never depends on this script's floating-point environment; a parabolic knee
`y = 1 − (2 − k − u)² / (4 (1 − k))` for `k < u < 2 − k`, `k = 0.85`.

Usage (from the repository root):
  uv run tools/output-chain/design_output_chain.py            # print every design figure
  uv run tools/output-chain/design_output_chain.py --check    # verify every FR-005 – FR-008 bound
  uv run tools/output-chain/design_output_chain.py --write    # regenerate crates/rr_dr60/src/stages/output_coeffs.rs
"""

from __future__ import annotations

import argparse
import math
import struct
import sys
from pathlib import Path

import numpy as np
from scipy import optimize, signal

OUTPUT = Path("crates/rr_dr60/src/stages/output_coeffs.rs")
RATES = [6000, 8000, 16000, 44100, 48000, 88200, 96000]  # SUPPORTED_HOST_RATES order (004 FR-002)

# --- The circuit (S-002 sheet 3.1, read at plan time; S-006) ---------------------------------
V_CLIP = 1.15  # A-031: clip ceiling, V peak differential, at the 2.8 V rail the schematic marks (S-002)
V_CLIP_RANGE = (1.0, 1.4)  # A-031: over the cells' state and the quiescent spread
V_CLIP_DATASHEET = 1.33  # S-006: 55 mW into 16 Ω at 10 % THD at 3.0 V
CODEC_FULL_SCALE_V = 1.0  # A-030: receive +3 dBm0, 1.0 V peak at the codec output
R_F = 68e3  # R18, the feedback resistor (pin 5 → pin 4)
R_IN = 10e3  # R16, the series input resistor into the inverting input
R_POT = 50e3  # VR1, linear ("B") taper
R_EAR = 10e3  # R65, the earphone branch's series resistor
Z_EAR = 6.4e3  # A-044: the follower's in-band input impedance, R64 33 kΩ ∥ hFE·(470 Ω ∥ 16 Ω + re) at hFE 370 (5.2–7.6 kΩ over the rank; the value F_EAR_IN is derived from; the 24 kΩ low-frequency value applies below the 280 Hz output corner only; hardware review T043 B1), low
F_VOL = 1.0 / (2 * math.pi * 0.1e-6 * R_POT)  # C14 0.1 µF into VR1: 31.8 Hz, both routes
F_AMP = 1.0 / (2 * math.pi * 0.1e-6 * R_IN)  # C8 0.1 µF into R16 at the virtual ground: 159 Hz
F_EAR_IN, F_EAR_IN_RANGE = 120.0, (65.0, 130.0)  # A-032: input coupling (C8 + C60 in series)
F_EAR_OUT, F_EAR_OUT_RANGE = 280.0, (240.0, 320.0)  # A-032: C61 10 µF into 16 Ω + ≈ 40 Ω
SPK_F0, SPK_Q = 750.0, 1.8  # A-010 (revised): the template's resonance
SPK_SHELF_ZERO, SPK_SHELF_POLE = 780.0, 1310.0  # A-010: the +4.5 dB shelf
KNEE_START = 0.85  # A-031 (low): the knee's shape, bracketed by S-006's distortion figures
LAW_POSITIONS = [1.0, 0.5, 0.25, None, 0.1, 0.01]  # None = the reference position (SC-004)
KNEE_LEVELS_DB = [-6.0, 0.0, 1.0, 2.0, 3.0, 6.0]  # re the ceiling (FR-006)

AMP_GAIN = 2 * R_F / R_IN  # 13.6, 22.67 dB at position 1 (S-006 formula, S-002 values)


# --- The loaded volume law (A-044) ---------------------------------------------------------
def r_wiper(p: float) -> float:
    return R_POT * p * (1.0 - p)


def g_spk(p: float) -> float:
    """Speaker route: volts of amplifier output per volt of codec output at position p."""
    return p * 2 * R_F / (R_IN + r_wiper(p))


def g_ear(p: float) -> float:
    """Earphone route: the volume control's output per volt of codec output (unity at p = 1)."""
    z = R_IN + R_EAR + Z_EAR
    return p * z / (z + r_wiper(p))


def f32_round(x: float) -> float:
    return struct.unpack(">f", struct.pack(">f", x))[0]


def solve_position(target_v: float) -> float:
    """The position at which g_spk(p) · CODEC_FULL_SCALE_V == target_v."""
    return optimize.brentq(lambda p: g_spk(p) * CODEC_FULL_SCALE_V - target_v, 1e-9, 1.0, xtol=1e-15)


P_REF = f32_round(solve_position(V_CLIP))  # A-043, rounded to the f32 the Rust default carries
P_6DB_OVER = f32_round(solve_position(2.0 * V_CLIP))  # the clip checks and `default_loud`


def db(x: float) -> float:
    return 20.0 * math.log10(x)


# --- The knee (R-04) ------------------------------------------------------------------------
def knee(x: np.ndarray, k: float = KNEE_START) -> np.ndarray:
    u = np.abs(x)
    y = np.where(u <= k, u, np.where(u >= 2 - k, 1.0, 1 - (2 - k - u) ** 2 / (4 * (1 - k))))
    return np.sign(x) * y


def knee_figures(level_db: float, fs: int = 48000, f0: int = 1000, cycles: int = 100) -> tuple[float, float]:
    """Peak and THD (fraction) of a 1 kHz sine `level_db` re the ceiling through the knee."""
    n = fs // f0 * cycles  # whole cycles: f0 divides fs
    t = np.arange(n) / fs
    y = knee(10 ** (level_db / 20) * np.sin(2 * np.pi * f0 * t))
    spec = np.abs(np.fft.rfft(y)) / n
    bins = [cycles * h for h in range(1, fs // (2 * f0) + 1)]
    fund = spec[bins[0]]
    harm = math.sqrt(sum(spec[b] ** 2 for b in bins[1:]))
    return float(np.max(np.abs(y))), harm / fund


def knee_10pct_db() -> float:
    return optimize.brentq(lambda l: knee_figures(l)[1] - 0.10, 0.0, 6.0, xtol=1e-6)


# --- Sections (R-03) -----------------------------------------------------------------------
def first_order_hp(fc: float, fs: int) -> np.ndarray:
    """[b0, b1, a1] of H(z) = g (1 − z⁻¹) / (1 − a z⁻¹), prewarped, unity at Nyquist."""
    t = math.tan(math.pi * fc / fs)
    a = (1 - t) / (1 + t)
    g = (1 + a) / 2
    return np.array([g, -g, -a])


def shelf(fs: int) -> np.ndarray:
    """[b0, b1, a1] of (1 + s/ωz) / (1 + s/ωp) by the bilinear transform, not prewarped."""
    wz, wp = 2 * math.pi * SPK_SHELF_ZERO, 2 * math.pi * SPK_SHELF_POLE
    b, a = signal.bilinear([1 / wz, 1.0], [1 / wp, 1.0], fs)
    b, a = b / a[0], a / a[0]
    return np.array([b[0], b[1], a[1]])


def resonance(fs: int) -> np.ndarray:
    """[b0, b1, b2, a1, a2] of s² / (s² + (ω0/Q) s + ω0²), prewarped at f0 (before normalization)."""
    w0 = 2 * fs * math.tan(math.pi * SPK_F0 / fs)  # prewarped so the bilinear lands f0 exactly
    b, a = signal.bilinear([1.0, 0.0, 0.0], [1.0, w0 / SPK_Q, w0 * w0], fs)
    b, a = b / a[0], a / a[0]
    return np.array([b[0], b[1], b[2], a[1], a[2]])


def response(sections: list[np.ndarray], f: np.ndarray, fs: int) -> np.ndarray:
    """Complex response of a cascade at frequencies `f` (Hz); sections are [b0,b1,a1] or [b0..a2]."""
    w = 2 * np.pi * f / fs
    h = np.ones_like(f, dtype=complex)
    for s in sections:
        if len(s) == 3:
            b, a = [s[0], s[1]], [1.0, s[2]]
        else:
            b, a = [s[0], s[1], s[2]], [1.0, s[3], s[4]]
        h *= signal.freqz(b, a, worN=w)[1]
    return h


def analog_template(f: np.ndarray) -> np.ndarray:
    s = 2j * np.pi * f
    w0, wz, wp = 2 * math.pi * SPK_F0, 2 * math.pi * SPK_SHELF_ZERO, 2 * math.pi * SPK_SHELF_POLE
    return (s * s / (s * s + (w0 / SPK_Q) * s + w0 * w0)) * ((1 + s / wz) / (1 + s / wp))


def group_delay_ms(sections: list[np.ndarray], f: float, fs: int) -> float:
    w = np.array([2 * math.pi * f / fs])
    total = 0.0
    for s in sections:
        b, a = ([s[0], s[1]], [1.0, s[2]]) if len(s) == 3 else ([s[0], s[1], s[2]], [1.0, s[3], s[4]])
        total += float(signal.group_delay((b, a), w=w)[1][0])
    return total / fs * 1e3


def corner_from_coeffs(sec: np.ndarray, fs: int) -> float:
    """The −3 dB point of a first-order high-pass from its pole alone (the Rust's test rule)."""
    a = -sec[2]
    return fs * math.atan((1 - a) / (1 + a)) / math.pi


def decay_samples(sec: np.ndarray, fs: int, ratio: float) -> int:
    """Samples for the section's slowest pole to decay by `ratio` (its state's envelope)."""
    if len(sec) == 3:
        pmax = abs(sec[2])
    else:
        pmax = float(np.max(np.abs(np.roots([1.0, sec[3], sec[4]]))))
    return math.ceil(math.log(1 / ratio) / -math.log(pmax))


class Design:
    def __init__(self, fs: int) -> None:
        self.fs = fs
        self.vol = first_order_hp(F_VOL, fs)
        self.amp = first_order_hp(F_AMP, fs)
        self.ear_in = first_order_hp(F_EAR_IN, fs)
        self.ear_out = first_order_hp(F_EAR_OUT, fs)
        self.shelf = shelf(fs)
        res = resonance(fs)
        self.grid = np.arange(20.0, fs / 2 + 0.5, 1.0)  # 1 Hz grid, 20 Hz … Nyquist
        mag = np.abs(response([res, self.shelf], self.grid, fs))
        self.peak_hz = float(self.grid[int(np.argmax(mag))])
        res[:3] /= np.max(mag)  # peak gain exactly 1.0 at this rate (FR-005)
        self.res = res
        self.spk = [self.res, self.shelf]

    def spk_db(self, f: float) -> float:
        return db(abs(response(self.spk, np.array([f]), self.fs)[0]))

    def spk_peak(self) -> float:
        return float(np.max(np.abs(response(self.spk, self.grid, self.fs))))

    def discrete_vs_analog_db(self) -> float:
        f = self.grid[(self.grid >= 100.0) & (self.grid <= 2500.0)]
        d = 20 * np.log10(np.abs(response(self.spk, f, self.fs)))
        a = 20 * np.log10(np.abs(analog_template(f)))
        a -= np.max(20 * np.log10(np.abs(analog_template(self.grid))))
        return float(np.max(np.abs(d - a)))

    def ear_db(self, f: float) -> float:
        return db(abs(response([self.vol, self.ear_in, self.ear_out], np.array([f]), self.fs)[0]))

    def amp_db(self, f: float) -> float:
        return db(abs(response([self.vol, self.amp], np.array([f]), self.fs)[0]))

    def amp_flatness_db(self, lo: float = 800.0, hi: float = 2500.0) -> float:
        f = self.grid[(self.grid >= lo) & (self.grid <= hi)]
        d = 20 * np.log10(np.abs(response([self.vol, self.amp], f, self.fs)))
        return float(np.max(d) - np.min(d)) if len(f) else 0.0

    def ear_flatness_db(self, lo: float = 1000.0, hi: float = 2500.0) -> float:
        f = self.grid[(self.grid >= lo) & (self.grid <= hi)]
        d = 20 * np.log10(np.abs(response([self.vol, self.ear_in, self.ear_out], f, self.fs)))
        return float(np.max(d) - np.min(d)) if len(f) else 0.0

    def gd_spk_ms(self) -> float:
        return group_delay_ms([self.vol, self.amp, self.res, self.shelf], 1000.0, self.fs)

    def gd_ear_ms(self) -> float:
        return group_delay_ms([self.vol, self.ear_in, self.ear_out], 1000.0, self.fs)

    def n_out(self) -> int:
        # The drain's settle: the 32 Hz section's state decays by 100 dB (spec 006 FR-008's
        # −100 dB criterion, applied to the pole's envelope: after a sustained signal the state
        # is of the signal's order, so the impulse response's n ≥ 1 samples understate it).
        return decay_samples(self.vol, self.fs, 1e-5)

    def spk_l1_norm(self, n: int = 1 << 16) -> float:
        """The template's impulse-response L1 norm (hardware review T043, N4)."""
        x = np.zeros(n)
        x[0] = 1.0
        h = signal.lfilter([self.res[0], self.res[1], self.res[2]], [1.0, self.res[3], self.res[4]], x)
        h = signal.lfilter([self.shelf[0], self.shelf[1]], [1.0, self.shelf[2]], h)
        return float(np.sum(np.abs(h)))

    def volume_zero_decay_ms(self) -> tuple[float, float]:
        spk = max(decay_samples(s, self.fs, 1e-6) for s in [self.amp, self.res, self.shelf])
        ear = max(decay_samples(s, self.fs, 1e-6) for s in [self.ear_in, self.ear_out])
        return spk / self.fs * 1e3, ear / self.fs * 1e3


# --- Report and checks ----------------------------------------------------------------------
def law_rows() -> list[tuple[float, float, float]]:
    return [(P_REF if p is None else p, g_spk(P_REF if p is None else p), g_ear(P_REF if p is None else p)) for p in LAW_POSITIONS]


def report(designs: list[Design]) -> list[tuple[str, str, str, bool]]:
    rows: list[tuple[str, str, str, bool]] = []
    add = lambda name, bound, value, ok: rows.append((name, bound, value, ok))
    print("Output chain design (spec 009; S-002 sheet 3.1, S-006; A-010, A-031, A-032, A-042 – A-044)")
    print(f"  V_CLIP {V_CLIP:.3f} V peak (range {V_CLIP_RANGE[0]} – {V_CLIP_RANGE[1]}; datasheet 3.0 V figure {V_CLIP_DATASHEET})")
    print(f"  AMP_GAIN 2·Rf/Ri = {AMP_GAIN:.3f} ({db(AMP_GAIN):.2f} dB) at position 1; codec full scale {CODEC_FULL_SCALE_V} V")
    print(f"  corners: F_VOL {F_VOL:.2f} Hz, F_AMP {F_AMP:.2f} Hz, F_EAR_IN {F_EAR_IN} Hz {F_EAR_IN_RANGE}, F_EAR_OUT {F_EAR_OUT} Hz {F_EAR_OUT_RANGE}")
    print(f"  speaker template: f0 {SPK_F0} Hz, Q {SPK_Q}, shelf zero {SPK_SHELF_ZERO} Hz, pole {SPK_SHELF_POLE} Hz ({db(SPK_SHELF_POLE / SPK_SHELF_ZERO):.2f} dB)")
    print(f"  knee start k = {KNEE_START}")
    print()
    print(f"  VOLUME_REFERENCE (P_REF) = {P_REF!r} ({db(P_REF):.2f} dB of travel); f64 bits {hexbits(P_REF)}")
    k_ref = g_spk(P_REF) * CODEC_FULL_SCALE_V / V_CLIP
    print(f"  K_SPK(P_REF) = {k_ref:.12f} (unity within {abs(k_ref - 1):.2e})")
    add("K_SPK(P_REF) unity after f32 rounding", "|K − 1| ≤ 1e-7", f"{abs(k_ref - 1):.2e}", abs(k_ref - 1) <= 1e-7)
    print(f"  K_SPK(1) = {g_spk(1.0) / V_CLIP:.4f} ({db(g_spk(1.0) / V_CLIP):.2f} dB in the route's scale; {db(g_spk(1.0)):.2f} dB in volts)")
    print(f"  earphone level at P_REF = {db(g_ear(P_REF)):.2f} dB (re full swing at position 1)")
    print(f"  VOLUME_6DB_OVER = {P_6DB_OVER!r} (full-scale material 6 dB over the ceiling)")
    print()
    print("  loaded law (A-044): position  g_spk [V/V]  (dB)      g_ear  (dB)")
    for p, gs, ge in law_rows():
        print(f"    {p:<10.6g} {gs:10.5f} {db(gs):8.2f}  {ge:8.5f} {db(ge):8.2f}")
    add("speaker gain at position 1", "22.67 dB (2·68/10)", f"{db(g_spk(1.0)):.2f} dB", abs(db(g_spk(1.0)) - 22.67) < 0.01)
    add("earphone level at P_REF", "−20 … −17 dB (spec: ≈ −18.5 ± 1.5)", f"{db(g_ear(P_REF)):.2f} dB", -20.0 <= db(g_ear(P_REF)) <= -17.0)
    print()
    print("  knee (1 kHz sine at 48 kHz, whole cycles): level re ceiling  peak      THD")
    table = []
    for l in KNEE_LEVELS_DB:
        peak, thd = knee_figures(l)
        table.append((l, peak, thd * 100))
        print(f"    {l:+5.1f} dB  {peak:.6f} ({db(peak):+.3f} dB)  {thd * 100:6.2f} %")
    k10 = knee_10pct_db()
    print(f"    10 % THD at {k10:+.3f} dB over the ceiling")
    t = dict((l, (p, d)) for l, p, d in table)
    add("THD at −6 dB", "≤ 1 %", f"{t[-6.0][1]:.2f} %", t[-6.0][1] <= 1.0)
    add("compression at the ceiling", "≤ 1 dB", f"{-db(t[0.0][0]):.3f} dB", -db(t[0.0][0]) <= 1.0)
    add("10 % THD crossing", "+1 … +3 dB over", f"{k10:+.3f} dB", 1.0 <= k10 <= 3.0)
    add("THD at +6 dB", "15 – 30 %", f"{t[6.0][1]:.2f} %", 15.0 <= t[6.0][1] <= 30.0)
    add("peak at +6 dB", "≥ −0.1 dB", f"{db(t[6.0][0]):+.3f} dB", db(t[6.0][0]) >= -0.1)
    add("peak never above 1.0", "≤ 1.0", f"{max(p for _, p, _ in table):.6f}", max(p for _, p, _ in table) <= 1.0)
    # C¹ joints: slope 1 at k, 0 at 2 − k.
    e = 1e-6
    s1 = (knee(np.array([KNEE_START + e])) - knee(np.array([KNEE_START - e])))[0] / (2 * e)
    s2 = (knee(np.array([2 - KNEE_START + e])) - knee(np.array([2 - KNEE_START - e])))[0] / (2 * e)
    add("knee slope at k", "1 ± 1e-4", f"{s1:.6f}", abs(s1 - 1) < 1e-4)
    add("knee slope at 2 − k", "0 ± 1e-4", f"{s2:.6f}", abs(s2) < 1e-4)
    print()
    print("  per rate:   corner −3 dB points from the coefficients [Hz]          template")
    print("    fs      vol     amp     ear_in  ear_out   peak Hz  1k dB   2.5k dB  dev dB  gd_spk ms  gd_ear ms  ear 1k dB  amp flat  N_OUT   v0 spk/ear ms")
    for d in designs:
        fs = d.fs
        c = [corner_from_coeffs(s, fs) for s in (d.vol, d.amp, d.ear_in, d.ear_out)]
        dev = d.discrete_vs_analog_db()
        v0s, v0e = d.volume_zero_decay_ms()
        print(f"    {fs:<6} {c[0]:7.2f} {c[1]:7.2f} {c[2]:7.2f} {c[3]:8.2f}  {d.peak_hz:7.0f} {d.spk_db(1000):7.2f} {d.spk_db(2500):8.2f} {dev:7.2f} {d.gd_spk_ms():9.3f} {d.gd_ear_ms():10.3f} {d.ear_db(1000):9.2f} {d.amp_flatness_db():8.3f} {d.n_out():6d}   {v0s:5.1f}/{v0e:5.1f}  L1 {d.spk_l1_norm():.3f}")
        add(f"{fs} speaker template impulse-response L1 norm (the worst-case sample peak for a bounded input; A-042's overshoot)", "report", f"{d.spk_l1_norm():.3f}", True)
        for name, want, got in zip(("vol", "amp", "ear_in", "ear_out"), (F_VOL, F_AMP, F_EAR_IN, F_EAR_OUT), c):
            add(f"{fs} {name} corner", f"{want:.1f} Hz ± 10 %", f"{got:.2f} Hz", abs(got - want) <= 0.1 * want)
        add(f"{fs} template peak gain", "1.0 (± 1e-9)", f"{d.spk_peak():.12f}", abs(d.spk_peak() - 1.0) <= 1e-9)
        lim = 1.2 if fs <= 8000 else 0.5
        add(f"{fs} template discrete vs analog 100 Hz – 2.5 kHz", f"≤ {lim} dB", f"{dev:.3f} dB", dev <= lim)
        add(f"{fs} speaker route group delay at 1 kHz", "≤ 0.5 ms", f"{d.gd_spk_ms():.3f} ms", d.gd_spk_ms() <= 0.5)
        add(f"{fs} earphone route group delay at 1 kHz", "≤ 0.5 ms", f"{d.gd_ear_ms():.3f} ms", d.gd_ear_ms() <= 0.5)
        add(f"{fs} earphone level at 1 kHz, position 1", "−0.5 … 0 dB", f"{d.ear_db(1000):.3f} dB", -0.5 <= d.ear_db(1000) <= 0.0)
        add(f"{fs} earphone passband 1 – 2.5 kHz", "flat within 0.4 dB", f"{d.ear_flatness_db():.3f} dB", d.ear_flatness_db() <= 0.4)  # 0.33 dB at the nominals: the 280 Hz corner alone is 0.28; FR-007 amended 0.3 → 0.4 at implementation
        add(f"{fs} amplifier passband 800 Hz – 2.5 kHz", "flat within 0.2 dB", f"{d.amp_flatness_db():.3f} dB", d.amp_flatness_db() <= 0.2)
        add(f"{fs} volume-0 decay to −120 dB", "≤ 100 ms", f"{max(v0s, v0e):.1f} ms", max(v0s, v0e) <= 100.0)
        # FR-005: "no further feature above 2.5 kHz and the shelf's remaining rise there at most
        # 0.7 dB": the largest rise over 2.5 kHz anywhere up to 0.45·fs (coach review T044, 1).
        rise = max(d.spk_db(f) - d.spk_db(2500) for f in np.arange(2500.0, 0.45 * fs, 5.0)) if 0.45 * fs > 2505 else 0.0
        add(f"{fs} shelf rise remaining above 2.5 kHz (to 0.45 fs)", "≤ 0.7 dB", f"{rise:.3f} dB", rise <= 0.7)
    print()
    return rows


def hexbits(x: float) -> str:
    return "0x" + struct.pack(">d", float(x)).hex()


def const_f64(name: str, x: float, doc: str) -> list[str]:
    return [f"/// {doc}", f"pub(crate) const {name}: f64 = f64::from_bits({hexbits(x)}); // {x:.17g}"]


def const_rows(name: str, rows: list[list[float]], width: int, doc: str, notes: list[str] | None = None) -> list[str]:
    lines = [f"/// {doc}", "#[rustfmt::skip]", f"pub(crate) const {name}: [[f64; {width}]; {len(rows)}] = ["]
    for i, row in enumerate(rows):
        note = f" // {notes[i]}" if notes else ""
        lines.append(f"    [{', '.join(f'f64::from_bits({hexbits(v)})' for v in row)}],{note}")
    lines.append("];")
    return lines


def const_vec(name: str, vals: list[float], doc: str) -> list[str]:
    lines = [f"/// {doc}", "#[rustfmt::skip]", f"pub(crate) const {name}: [f64; {len(vals)}] = ["]
    for v, fs in zip(vals, RATES):
        lines.append(f"    f64::from_bits({hexbits(v)}), // {fs} Hz: {v:.6g}")
    lines.append("];")
    return lines


def render_file(designs: list[Design], rows) -> str:
    k10 = knee_10pct_db()
    table = [[l, *knee_figures(l)] for l in KNEE_LEVELS_DB]
    lines = [
        "//! Output-chain coefficients and design constants (signal-chain stages 11 – 13; spec 009).",
        "//!",
        "//! GENERATED by `tools/output-chain/design_output_chain.py --write`; do not edit by hand.",
        "//! Regenerate with: `uv run tools/output-chain/design_output_chain.py --write`.",
        "//!",
        "//! Every figure is a design constant modeled on the circuit (S-002 sheet 3.1) and the",
        "//! amplifier datasheet (S-006): A-031 (the volume control, the gain, the ceiling, the two",
        "//! coupling corners, the knee), A-044 (the loaded volume law), A-043 (the reference position),",
        "//! A-042 (the per-route output scale), A-032 (the earphone follower's two corners) and A-010",
        "//! (the speaker **template**, low confidence). Nothing here is a measurement of a real unit.",
        "//!",
        "//! Sections are bilinear with the corner prewarped at each host rate (the shelf not prewarped),",
        "//! `[b0, b1, a1]` for `H(z) = (b0 + b1 z⁻¹) / (1 + a1 z⁻¹)` and `[b0, b1, b2, a1, a2]` for the",
        "//! resonance biquad, indexed by `SUPPORTED_HOST_RATES` order; exact bit patterns (001 R-04).",
        "//!",
        "//! Figures the script printed (the Rust pins these, never its own output):",
    ]
    lines.extend(render_table(rows))
    lines.append("")
    lines += const_f64("VOLUME_REFERENCE", P_REF, "A-043: the reference position (the `f32` default's value), where codec full scale meets the ceiling under the loaded law.")
    lines += const_f64("VOLUME_6DB_OVER", P_6DB_OVER, "The position at which full-scale material sits 6 dB over the ceiling (the clip checks; the `default_loud` golden configuration).")
    lines += const_f64("V_CLIP_V", V_CLIP, "A-031: the clip ceiling, V peak differential at the 2.8 V rail the schematic marks (S-002) (1.0 – 1.4 V registered).")
    lines += const_f64("CODEC_FULL_SCALE_V", CODEC_FULL_SCALE_V, "A-030: codec full scale at its analog output, V peak.")
    lines += const_f64("AMP_GAIN", AMP_GAIN, "A-031: 2·Rf/Ri = 2·68/10 at position 1 (S-006's formula, S-002's values).")
    lines += const_f64("R_FEEDBACK_OHM", R_F, "S-002: R18, the feedback resistor.")
    lines += const_f64("R_INPUT_OHM", R_IN, "S-002: R16, the amplifier's series input resistor.")
    lines += const_f64("R_POT_OHM", R_POT, "S-002: VR1, the 50 kΩ linear volume control.")
    lines += const_f64("R_EAR_SERIES_OHM", R_EAR, "S-002: R65, the earphone branch's series resistor.")
    lines += const_f64("Z_EAR_OHM", Z_EAR, "A-044 (low): the earphone branch's low-frequency input impedance that loads the control.")
    lines += const_f64("KNEE_START", KNEE_START, "A-031 (low): the knee starts at this fraction of the ceiling; `y = 1 − (2 − k − u)² / (4 (1 − k))` up to `2 − k`.")
    lines += const_f64("KNEE_10PCT_DB", k10, "The level re the ceiling at which the knee reaches 10 % THD (1 kHz sine at 48 kHz).")
    lines += const_f64("F_VOL_HZ", F_VOL, "A-031: the volume control's input coupling corner (C14 into VR1), both routes.")
    lines += const_f64("F_AMP_HZ", F_AMP, "A-031: the amplifier's input coupling corner (C8 into R16), speaker route.")
    lines += const_f64("F_EAR_IN_HZ", F_EAR_IN, "A-032: the earphone follower's input coupling corner, nominal (65 – 130 Hz registered).")
    lines += const_f64("F_EAR_OUT_HZ", F_EAR_OUT, "A-032: the earphone follower's output coupling corner into 16 Ω, nominal (240 – 320 Hz registered).")
    lines += const_f64("SPK_F0_HZ", SPK_F0, "A-010 (revised): the speaker template's resonance.")
    lines += const_f64("SPK_Q", SPK_Q, "A-010 (revised): the resonance's Q.")
    lines += const_f64("SPK_SHELF_ZERO_HZ", SPK_SHELF_ZERO, "A-010 (revised): the rising shelf's zero.")
    lines += const_f64("SPK_SHELF_POLE_HZ", SPK_SHELF_POLE, "A-010 (revised): the rising shelf's pole (+4.5 dB).")
    lines.append("")
    lines += const_rows("KNEE_TABLE", [[l, p, thd * 100] for l, p, thd in table], 3,
                        "The knee's verified figures: `[level_db re ceiling, peak, thd_percent]` for a 1 kHz sine at 48 kHz over whole cycles.")
    lines += const_rows("GAIN_LAW_SAMPLES", [list(r) for r in law_rows()], 3,
                        "A-044: the loaded law at six positions: `[position, g_spk (V/V), g_ear]` (SC-004).")
    lines.append("")
    for name, attr, doc in (
        ("VOL_HPF", "vol", "A-031: the 32 Hz volume-control corner, per rate."),
        ("AMP_HPF", "amp", "A-031: the 159 Hz amplifier input corner, per rate."),
        ("EAR_IN", "ear_in", "A-032: the earphone input corner (120 Hz nominal), per rate."),
        ("EAR_OUT", "ear_out", "A-032: the earphone output corner (280 Hz nominal), per rate."),
        ("SPK_SHELF", "shelf", "A-010: the speaker template's shelf section, per rate (not prewarped)."),
    ):
        lines += const_rows(name, [list(getattr(d, attr)) for d in designs], 3, doc, [f"{d.fs} Hz" for d in designs])
    lines += const_rows("SPK_RES", [list(d.res) for d in designs], 5,
                        "A-010: the speaker template's resonance biquad, per rate, numerator scaled so the template's peak gain is exactly 1.0 at that rate.",
                        [f"{d.fs} Hz" for d in designs])
    lines.append("")
    lines += const_rows("CORNER_HZ", [[corner_from_coeffs(s, d.fs) for s in (d.vol, d.amp, d.ear_in, d.ear_out)] for d in designs], 4,
                        "The −3 dB point of each first-order section recomputed from its pole, per rate: `[vol, amp, ear_in, ear_out]` (FR-006, FR-007: within ±10 % of the registered figure).",
                        [f"{d.fs} Hz" for d in designs])
    lines += const_vec("SPK_PEAK_HZ", [d.peak_hz for d in designs], "The speaker template's peak frequency on a 1 Hz grid, per rate.")
    lines += const_vec("SPK_1K_DB", [d.spk_db(1000) for d in designs], "The speaker template's 1 kHz reading re its peak, per rate (FR-005: stated, negative).")
    lines += const_vec("SPK_2K5_DB", [d.spk_db(2500) for d in designs], "The speaker template's 2.5 kHz reading re its peak, per rate.")
    lines += const_vec("EAR_1K_DB", [d.ear_db(1000) for d in designs], "The earphone route's 1 kHz level at position 1 (the three corners' residual), per rate (FR-007: −0.5 … 0 dB).")
    lines += const_vec("GROUP_DELAY_1K_SPK_MS", [d.gd_spk_ms() for d in designs], "The speaker route's group delay at 1 kHz, ms, per rate (FR-008: ≤ 0.5 ms).")
    lines += const_vec("GROUP_DELAY_1K_EAR_MS", [d.gd_ear_ms() for d in designs], "The earphone route's group delay at 1 kHz, ms, per rate (FR-008: ≤ 0.5 ms).")
    lines += ["/// The drain's settle in host samples, per rate: the 32 Hz section's state decays by 100 dB",
              "/// (spec 006 FR-008's criterion on the pole's envelope; research R-08).",
              "#[rustfmt::skip]",
              f"pub(crate) const N_OUT: [u32; {len(designs)}] = [{', '.join(str(d.n_out()) for d in designs)}];"]
    lines += ["/// The host rates the tables are indexed by (`SUPPORTED_HOST_RATES` order, 004 FR-002).",
              f"pub(crate) const RATES: [u32; {len(RATES)}] = [{', '.join(str(r) for r in RATES)}];"]
    return "\n".join(lines).rstrip("\n") + "\n"


def render_table(rows) -> list[str]:
    width = max(len(r[0]) for r in rows)
    return [f"//!   {name:<{width}}  {bound:<36} {value}" for name, bound, value, _ in rows]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--check", action="store_true", help="verify every bound; exit non-zero on failure")
    ap.add_argument("--write", action="store_true", help="regenerate the Rust coefficient file")
    ap.add_argument("--output", type=Path, default=OUTPUT, help=argparse.SUPPRESS)
    args = ap.parse_args()

    designs = [Design(fs) for fs in RATES]
    rows = report(designs)
    failed = [r for r in rows if not r[3]]
    print(f"  checks: {len(rows)} bounds, {len(rows) - len(failed)} passed, {len(failed)} failed")
    for name, bound, value, _ in failed:
        print(f"    FAIL {name}: {value} (bound {bound})")
    if args.write:
        args.output.write_text(render_file(designs, rows), encoding="utf-8")
        print(f"  wrote {args.output}")
    return 1 if (args.check or args.write) and failed else 0


if __name__ == "__main__":
    sys.exit(main())
