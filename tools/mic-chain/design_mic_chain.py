# /// script
# requires-python = ">=3.10"
# dependencies = ["numpy>=1.24", "scipy>=1.10"]
# ///
"""Design tool for the rr-dr60-emu microphone path (signal-chain stages 1 and 2; spec 012).

Every figure this script prints is a **design constant of the model**: the circuit as read from
the service manual's schematic sheet 3.1 (S-002), reduced to three first-order sections, a
memoryless clipper and a deterministic Gaussian hiss. Nothing here is a measurement of a real
RR-DR60 (constitution Principle II). The register IDs:

- A-046: the input high-pass. The capsule is a current source; its signal current divides
  between R2 (1 kOhm, plus Q11's emitter resistance) and C1 (0.15 uF) into the amplifier's
  virtual ground (Q1's base, held by R3's feedback at a high loop gain).
- A-047: the output coupling (C4 in series with C5 into R7 + R10 = 23 kOhm, one first-order
  corner while the limiter is idle) and the codec input amplifier's roll-off (C10 across R15).
- A-053: those two sections lumped ahead of the limiter's attenuator.
- A-051: the clip limits from the DC voltages (negative engine samples limited first, where Q1
  saturates; positive where Q2 saturates), spec 009's parabolic knee at k = 0.85.
- A-048: the hiss anchor (34 dB SPL(A) capsule self-noise, full scale 90 dB SPL at 1 kHz).
- A-049: the generator (xoshiro256++, mix64 keying, a 128-layer ziggurat with capped retries).
- A-050: the amplifier's and R2's added floor.

The circuit model (research R-01). A small-signal hybrid-pi nodal network:

    M   the capsule node: the capsule's AC current in; R2 + r_e(Q11) to AC ground
    B   Q1's base: C1 from M; R3 (100 kOhm) from O
    E   the tail: Q1's and Q3's emitters; R5 (10 kOhm) to ground
    C3  Q3's collector: R6 (100 kOhm) to the rail; Q2's base
    O   the output: Q1's and Q2's collectors; the load C4ser C5 + 23 kOhm into the codec input
        amplifier's virtual ground, whose output GSX is -O * (R15 || C10) / Zload

Q3's base sits on the codec's SG reference (AC ground); Q2 is a PNP with its emitter on the
rail. Operating points from the schematic's DC voltages: the tail 1.1 V / 10 kOhm = 110 uA,
Q3 ~6 uA ((3.2 - 2.6) V through R6), Q1 = Q2 ~104 uA.

Usage (from the repository root):
  uv run tools/mic-chain/design_mic_chain.py            # print every design figure
  uv run tools/mic-chain/design_mic_chain.py --check    # verify every bound and constraint
  uv run tools/mic-chain/design_mic_chain.py --write    # regenerate crates/rr_dr60/src/stages/mic_coeffs.rs
  uv run tools/mic-chain/design_mic_chain.py --spice    # the ngspice cross-check (a gate before the bless)
"""

import math
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from scipy.optimize import least_squares

RATES = [6000, 8000, 16000, 44100, 48000, 88200, 96000]
VT = 0.02585  # thermal voltage at 27 C

# --- S-002 sheet 3.1 values (the parts list: C1 ECUV1C154KB +-10 %, C4/C5 Y5V +80/-20 %) ---
R2, RQ11 = 1.0e3, 65.0
C1 = 0.15e-6
R3, R5, R6 = 100e3, 10e3, 100e3
C4, C5 = 0.1e-6, 1.0e-6
R7, R10 = 22e3, 1.0e3
R15, C10 = 82e3, 150e-12
# DC voltages (sheet 3.1, record mode)
V_RAIL, V_O, V_E, V_C3, V_SG = 3.2, 1.7, 1.1, 2.6, 1.6
I_TAIL = V_E / R5
I3_NOM = (V_RAIL - V_C3) / R6
I1_NOM = I_TAIL - I3_NOM
HFE_NOM, VA_NOM = 250.0, 50.0
GSX_FS_PEAK = 0.7  # A-030: codec full scale, +3 dBm0, 0.7 V peak at GSX

# --- A-048: the hiss anchor ---
SELF_NOISE_DBA = 34.0
FULL_SCALE_SPL = 90.0
# --- A-051: the clip limits' saturation voltages (nominal, range) ---
VCESAT_Q1 = (0.15, 0.05, 0.20)  # Q1: output falls to V_E + Vce_sat
VECSAT_Q2 = (0.20, 0.10, 0.25)  # Q2: output rises to V_RAIL - Vec_sat
KNEE_K = 0.85  # spec 009's knee (A-031), justified for this stage in A-051
# --- A-022: the voice-activation thresholds at levels 1..5 (dBFS, sine level) ---
VAS_THRESHOLDS = [-34.0, -37.0, -40.0, -43.0, -46.0]
HISS_OFFSET_RANGE = (-18.0, 12.0)
# --- A-049: generator constants ---
MIC_STREAM = 0x6D69635F63617073  # "mic_caps"
MIC_COUNTER_SALT = 0x7265635F636E7472  # "rec_cntr"
RETRY_CAP = 8
ZIG_LAYERS = 128
ZIG_R = 3.442619855899  # Marsaglia & Tsang (2000), 128 layers
ZIG_V = 9.91256303526217e-3
M64 = (1 << 64) - 1

CENTRES = [20, 25, 31.5, 40, 50, 63, 80, 100, 125, 160, 200, 250, 315, 400, 500, 630, 800, 1000,
           1250, 1600, 2000, 2500, 3150, 4000, 5000, 6300, 8000, 10000, 12500, 16000, 20000]
SHAPE_CENTRES = [315, 400, 500, 630, 800, 1000, 1250, 1600, 2000, 2500]

FIGURES = []  # (name, requirement, value-string, ok) for --check and the generated header


def fig(name, req, value, ok=True):
    FIGURES.append((name, req, value, bool(ok)))


# =====================================================================================
# Part 1: the circuit (R-01)
# =====================================================================================

def nodal(f, hfe=HFE_NOM, va=VA_NOM, i3=I3_NOM, c1=C1, load=True):
    """Node voltages [M, B, E, C3, O] and GSX for 1 A of capsule current, at frequencies f."""
    f = np.atleast_1d(np.asarray(f, float))
    w = 2 * np.pi * f
    i1 = I_TAIL - i3
    gm1, gm3, gm2 = i1 / VT, i3 / VT, i1 / VT
    rpi1, rpi3, rpi2 = hfe / gm1, hfe / gm3, hfe / gm2
    ro1, ro3, ro2 = va / i1, va / i3, va / i1
    cs = 1 / (1 / C4 + 1 / C5)
    n = len(f)
    Y = np.zeros((n, 5, 5), complex)

    def g(a, b, y):
        if a is not None:
            Y[:, a, a] += y
        if b is not None:
            Y[:, b, b] += y
        if a is not None and b is not None:
            Y[:, a, b] -= y
            Y[:, b, a] -= y

    def vccs(fr, to, cp, cn, gm):
        for node, sgn in ((fr, 1), (to, -1)):
            if node is None:
                continue
            if cp is not None:
                Y[:, node, cp] += sgn * gm
            if cn is not None:
                Y[:, node, cn] -= sgn * gm

    M, B, E, C3, O = range(5)
    g(M, None, 1 / (R2 + RQ11))
    g(M, B, 1j * w * c1)
    g(O, B, 1 / R3)
    g(B, E, 1 / rpi1); vccs(O, E, B, E, gm1); g(O, E, 1 / ro1)          # Q1
    g(None, E, 1 / rpi3); vccs(C3, E, None, E, gm3); g(C3, E, 1 / ro3)  # Q3, base on SG
    g(E, None, 1 / R5); g(C3, None, 1 / R6)
    g(C3, None, 1 / rpi2); vccs(O, None, C3, None, gm2); g(O, None, 1 / ro2)  # Q2 PNP
    zload = R7 + R10 + 1 / (1j * w * cs)
    if load:
        g(O, None, 1 / zload)
    I = np.zeros((n, 5), complex)
    I[:, M] = 1.0
    v = np.linalg.solve(Y, I[..., None])[..., 0]
    zf = R15 / (1 + 1j * w * R15 * C10)
    gsx = -v[:, O] * zf / zload
    return v, gsx


def three_pole(f, k, fa, fb, fc):
    s = 1j * np.asarray(f, float)
    return k * (s / fa) / (1 + s / fa) * (s / fb) / (1 + s / fb) / (1 + s / fc)


def fit_three_pole(**kw):
    """The joint fit of three first-order corners to |GSX / I_capsule| (R-01)."""
    f = np.logspace(0, np.log10(2e5), 600)
    _, h = nodal(f, **kw)
    target = 20 * np.log10(np.abs(h))

    def res(p):
        k, fa, fb, fc = np.exp(p)
        return 20 * np.log10(np.abs(three_pole(f, k, fa, fb, fc))) - target

    p0 = np.log([abs(h[-200]), 800.0, 77.0, 13000.0])
    sol = least_squares(res, p0, xtol=1e-14, ftol=1e-14, gtol=1e-14)
    k, fa, fb, fc = np.exp(sol.x)
    return dict(k=k, fa=fa, fb=fb, fc=fc, resid=float(np.max(np.abs(res(sol.x)))))


def minus3db_below_plateau(**kw):
    """The -3 dB point of the loaded transimpedance at O below its high-frequency value (a
    cross-check): at O only the input corner shapes the response (the coupling and the roll-off
    come after O), so this reads the input corner directly, without the fit."""
    f = np.logspace(1, 5, 40000)
    v, _ = nodal(f, **kw)
    vo = np.abs(v[:, 4])
    plateau = np.abs(nodal([1e7], **kw)[0][0, 4])
    return float(f[np.argmax(vo >= plateau / math.sqrt(2))]), plateau


def circuit():
    print("=" * 100)
    print("Part 1: the circuit model (S-002 sheet 3.1; research R-01)")
    print("=" * 100)
    print(f"operating points: tail {I_TAIL*1e6:.1f} uA, Q3 {I3_NOM*1e6:.1f} uA, Q1 = Q2 {I1_NOM*1e6:.1f} uA")
    nom = fit_three_pole()
    fig("input corner (nominal)", "665 – 985 Hz (A-046)", f"{nom['fa']:.1f} Hz", 665 <= nom["fa"] <= 985)
    fig("output coupling (nominal)", "69 – 80 Hz (A-047)", f"{nom['fb']:.1f} Hz", 69 <= nom["fb"] <= 80)
    fig("roll-off (nominal)", "12 – 14 kHz (A-047)", f"{nom['fc']:.0f} Hz", 12000 <= nom["fc"] <= 14000)
    fig("three-pole fit residual", "< 0.001 dB", f"{nom['resid']:.2e} dB", nom["resid"] < 1e-3)
    _, h1k = nodal([1000.0])
    _, h300 = nodal([300.0])
    _, h2550 = nodal([2550.0])
    r300 = 20 * np.log10(abs(h300[0]) / abs(h1k[0]))
    r2550 = 20 * np.log10(abs(h2550[0]) / abs(h1k[0]))
    fig("300 Hz re 1 kHz", "−8 … −6 dB", f"{r300:+.2f} dB", -8 <= r300 <= -6)
    fig("2550 Hz re 1 kHz", "+1 … +2.5 dB", f"{r2550:+.2f} dB", 1 <= r2550 <= 2.5)
    zt = abs(h1k[0])
    fig("transimpedance to GSX at 1 kHz", "report", f"{zt/1e3:.1f} kΩ")
    f3, plat = minus3db_below_plateau()
    fig("−3 dB point below the plateau at O (cross-check)", "within 2 % of the fit", f"{f3:.1f} Hz",
        abs(f3 / nom["fa"] - 1) < 0.02)
    v, _ = nodal([1000.0], load=False)
    # Z_B: the impedance looking into B, from the current through C1 and the voltage at B
    vv, _ = nodal([1000.0])
    i_c1 = (vv[0, 0] - vv[0, 1]) * 2j * math.pi * 1000 * C1
    zb = abs(vv[0, 1] / i_c1)
    fig("impedance into the virtual ground at 1 kHz", "report (loop gain = R3 / Z_B)", f"{zb:.0f} Ω (loop gain ≈ {R3/zb:.0f})")
    closed = 1 / (2 * math.pi * C1 * (R2 + RQ11 + zb))
    fig("closed form 1/(2π·C1·(R2′ + Z_B))", "within 5 % of the fit", f"{closed:.1f} Hz", abs(closed / nom["fa"] - 1) < 0.05)
    upper = 1 / (2 * math.pi * C1 * (R2 + RQ11))
    upper_lo = 1 / (2 * math.pi * 0.9 * C1 * (R2 + RQ11))
    fig("upper bound 1/(2π·C1·R2′)", "report (any transistors)", f"{upper:.0f} Hz ({upper_lo:.0f} Hz at C1 −10 %)")
    # the grid
    # the grid: the 2SD1819A ranks Q 160–260, R 210–340, S 290–460 (the marking "ASTX" probably
    # S; sources.md), the Early voltage, Q3's current and C1's tolerance
    fa_all, fa_rs, fb = [], [], []
    for hfe in (150.0, 210.0, 290.0, 460.0):
        for va in (20.0, 50.0, 100.0):
            for i3 in (5e-6, 6e-6, 7e-6):
                for c1 in (0.9 * C1, C1, 1.1 * C1):
                    r = fit_three_pole(hfe=hfe, va=va, i3=i3, c1=c1)
                    fa_all.append(r["fa"]); fb.append(r["fb"])
                    if hfe >= 210.0:
                        fa_rs.append(r["fa"])
    fig("input corner over the grid, ranks R/S (hFE 210–460; C1 ±10 %, VA 20–100 V, I3 5–7 µA)", "within 665 – 985 Hz (A-046)",
        f"{min(fa_rs):.0f} – {max(fa_rs):.0f} Hz", 660 <= min(fa_rs) and max(fa_rs) <= 990)
    fig("input corner over the grid, every rank (hFE 150–460)", "report (A-046's Q-rank worst case)",
        f"{min(fa_all):.0f} – {max(fa_all):.0f} Hz")
    fig("output coupling over the grid", "report", f"{min(fb):.1f} – {max(fb):.1f} Hz")
    # DC consistency (not independent evidence: the currents come from the same voltages)
    dvbe = VT * math.log(I1_NOM / I3_NOM)
    veb2 = VT * math.log(I1_NOM / 1e-14)
    fig("ΔVbe(Q1, Q3) = VT·ln(I1/I3)", "consistent with the meter (0.1 ± 0.1 V)", f"{dvbe*1e3:.0f} mV", 0 <= dvbe <= 0.2)
    fig("Q2's Veb at I1 (Is 1e-14)", "consistent with 3.2 − 2.6 V (± 0.1)", f"{veb2:.3f} V", 0.5 <= veb2 <= 0.7)
    # the level chain closes (R-01, plan review Q10)
    i_fs = GSX_FS_PEAK / math.sqrt(2) / zt
    fig("full scale in capsule current (rms)", "report", f"{i_fs*1e6:.2f} µA")
    for s_dbv in (-35.0, -40.0, -44.0):
        s_i = 10 ** (s_dbv / 20) / 2200.0
        spl = 94 + 20 * math.log10(i_fs / s_i)
        fig(f"full scale for a {s_dbv:.0f} dBV/Pa capsule (at 2.2 kΩ)", "80 – 100 dB SPL (A-029)", f"{spl:.1f} dB SPL", 80 <= spl <= 100)
    return nom


# =====================================================================================
# Part 2: the realization per host rate (R-02, R-03)
# =====================================================================================

def ana(f, nom):
    return three_pole(f, 1.0, nom["fa"], nom["fb"], nom["fc"])


def sect(c, f, fs):
    """A first-order section [b0, b1, a1]: H = (b0 + b1 z^-1) / (1 + a1 z^-1)."""
    z = np.exp(-1j * 2 * np.pi * np.asarray(f, float) / fs)
    return (c[0] + c[1] * z) / (1 + c[2] * z)


def sections(p1, p2, c, p3):
    hp_in = [1.0, -1.0, -p1]
    g2 = (1 + p2) / 2
    hp_out = [g2, -g2, -p2]
    lp = [c * (1 - p3), (1 - c) * (1 - p3), -p3]
    return hp_in, hp_out, lp


def cascade(secs, f, fs):
    h = np.ones(len(np.atleast_1d(f)), complex)
    for s in secs:
        h = h * sect(s, f, fs)
    return h


def fit_rate(fs, nom):
    """The per-rate constrained minimax fit (iterated least squares, power 2 -> 16; R-02)."""
    fmax = min(20000.0, 0.45 * fs)
    cs = np.array([c for c in CENTRES if c <= fmax], float)
    an = 20 * np.log10(np.abs(ana(cs, nom) / ana([1000.0], nom)[0]))
    cs = cs[an > -40]
    fd = np.logspace(np.log10(cs[0]), np.log10(cs[-1]), 400)
    ad = 20 * np.log10(np.abs(ana(fd, nom) / ana([1000.0], nom)[0]))

    def resid(p, f, a):
        secs = sections(*p)
        return 20 * np.log10(np.abs(cascade(secs, f, fs) / cascade(secs, [1000.0], fs)[0])) - a

    k1 = math.tan(math.pi * nom["fa"] / fs)
    k2 = math.tan(math.pi * nom["fb"] / fs)
    x = np.array([(1 - k1) / (1 + k1), (1 - k2) / (1 + k2), 0.8, 0.1])
    lo, hi = [0, 0, 0, 0], [0.99999999, 0.99999999, 1.0, 0.99]
    for pw in (2, 4, 8, 16):
        x = least_squares(lambda p: np.sign(resid(p, fd, ad)) * np.abs(resid(p, fd, ad)) ** (pw / 2),
                          x, bounds=(lo, hi), xtol=1e-15, ftol=1e-15, gtol=1e-15).x
    hp_in, hp_out, lp = sections(*x)
    # fold the 1 kHz normalization into HP_in's numerator
    g = 1.0 / abs(cascade([hp_in, hp_out, lp], [1000.0], fs)[0])
    hp_in = [hp_in[0] * g, hp_in[1] * g, hp_in[2]]
    an_c = 20 * np.log10(np.abs(ana(cs, nom) / ana([1000.0], nom)[0]))
    re_c = 20 * np.log10(np.abs(cascade([hp_in, hp_out, lp], cs, fs)))
    err_c = re_c - an_c
    err_d = resid(x, fd, ad)
    return dict(hp_in=hp_in, hp_out=hp_out, lp=lp, centres=cs, err_c=err_c,
                worst=float(np.max(np.abs(err_d))), worst_c=float(np.max(np.abs(err_c))))


def impulse(secs, n):
    x = np.zeros(n); x[0] = 1.0
    for c in secs:
        y = np.zeros(n); s = 0.0
        for i in range(n):
            y[i] = c[0] * x[i] + s
            s = c[1] * x[i] - c[2] * y[i]
        x = y
    return x


def knee(u, k):
    """Spec 009's parabolic knee (stages/output.rs::knee): linear to k, C1 joints, ceiling 1."""
    u = np.asarray(u, float)
    y = np.where(u <= k, u, 1.0 - (2.0 - k - u) ** 2 / (4.0 * (1.0 - k)))
    return np.where(u >= 2.0 - k, 1.0, y)


def clip(v, lp_, ln_, k):
    v = np.asarray(v, float)
    pos = lp_ * knee(v / lp_, k)
    neg = -ln_ * knee(-v / ln_, k)
    lin = (v >= -k * ln_) & (v <= k * lp_)
    return np.where(lin, v, np.where(v > 0, pos, neg))


def run_chain(x, r, lp_, ln_, k, clipper=True):
    y = x.astype(float)
    for c in (r["hp_in"],):
        y = filt(c, y)
    if clipper:
        y = clip(y, lp_, ln_, k)
    y = filt(r["hp_out"], y)
    return filt(r["lp"], y)


def filt(c, x):
    from scipy.signal import lfilter
    return lfilter([c[0], c[1]], [1.0, c[2]], x)


def onset_ratio(k):
    """The input peak (re L) at which the output peak is 0.1 dB below the linear peak."""
    target = 10 ** (-0.1 / 20)
    lo, hi = k, 2 - k
    for _ in range(200):
        mid = (lo + hi) / 2
        if float(knee(mid, k)) / mid > target:
            lo = mid
        else:
            hi = mid
    return (lo + hi) / 2


def realization(nom):
    print("=" * 100)
    print("Part 2: the realization per host rate (R-02, R-03)")
    print("=" * 100)
    # the clip limits in volts at O and the full-scale voltage at O at 1 kHz
    v, gsx = nodal([1000.0])
    gsx_per_vo = abs(gsx[0] / v[0, 4])
    v_fs_o = GSX_FS_PEAK / gsx_per_vo
    fig("full scale at the amplifier's output (1 kHz)", "≈ 0.196 V peak (A-029)", f"{v_fs_o:.4f} V", 0.18 <= v_fs_o <= 0.21)
    sw_neg = V_O - (V_E + VCESAT_Q1[0])
    sw_pos = (V_RAIL - VECSAT_Q2[0]) - V_O
    sw_neg_r = (V_O - (V_E + VCESAT_Q1[2]), V_O - (V_E + VCESAT_Q1[1]))
    sw_pos_r = ((V_RAIL - VECSAT_Q2[2]) - V_O, (V_RAIL - VECSAT_Q2[1]) - V_O)
    l_neg_e = sw_neg / v_fs_o  # engine-input units at 1 kHz
    l_pos_e = sw_pos / v_fs_o
    db = lambda a: 20 * math.log10(a)
    fig("limit L− (negative samples, Q1 saturates), engine input at 1 kHz", "+5 … +9 dBFS (A-051)",
        f"{db(l_neg_e):+.2f} dBFS (swing {sw_neg:.2f} V; range {db(sw_neg_r[0]/v_fs_o):+.1f} … {db(sw_neg_r[1]/v_fs_o):+.1f})",
        5 <= db(l_neg_e) <= 9)
    fig("limit L+ (positive samples, Q2 saturates), engine input at 1 kHz", "+16 … +17.5 dBFS (A-051)",
        f"{db(l_pos_e):+.2f} dBFS (swing {sw_pos:.2f} V; range {db(sw_pos_r[0]/v_fs_o):+.1f} … {db(sw_pos_r[1]/v_fs_o):+.1f})",
        16 <= db(l_pos_e) <= 17.5)
    a_on = onset_ratio(KNEE_K)
    fig("knee k", "0.85 (A-051)", f"{KNEE_K}")
    fig("onset re its limit (0.1 dB compression)", "−1 … −0.3 dB", f"{db(a_on):+.3f} dB", -1 <= db(a_on) <= -0.3)
    fig("output reaches the limit at (re the limit)", "report", f"{db(2 - KNEE_K):+.3f} dB")
    fig("bound after stage 2: L+ + L− (engine units)", "report", f"{l_pos_e + l_neg_e:.4f} ({db(l_pos_e + l_neg_e):+.2f} dBFS)")
    per_rate = {}
    for fs in RATES:
        r = fit_rate(fs, nom)
        hp_in, hp_out, lp = r["hp_in"], r["hp_out"], r["lp"]
        tail = abs(cascade([hp_out, lp], [1000.0], fs)[0])  # |HP_out·LP| at 1 kHz
        # clipper units: engine 1.0 at 1 kHz arrives at the clipper as 1 / tail
        l_pos, l_neg = l_pos_e / tail, l_neg_e / tail
        r.update(tail=tail, l_pos=l_pos, l_neg=l_neg)
        ok_fit = r["worst"] <= 0.1
        fig(f"{fs} prototype match (dense grid; centres ≥ −40 dB, ≤ min(20 kHz, 0.45·fs))", "≤ 0.1 dB (SC-002)",
            f"{r['worst']:.4f} dB (centres {r['worst_c']:.4f})", ok_fit)
        # constraints and the bound's premises
        hp_ok = hp_in[1] == -hp_in[0] and 0 <= -hp_in[2] < 1
        bil = abs(hp_out[0] - (1 - hp_out[2]) / 2) < 1e-15 and hp_out[1] == -hp_out[0] and 0 <= -hp_out[2] < 1
        lpc = lp[0] >= 0 and lp[1] >= 0 and 0 <= -lp[2] < 1 and abs((lp[0] + lp[1]) / (1 + lp[2]) - 1) < 1e-12
        h_lp = impulse([lp], 4000)
        h_out = impulse([hp_out], 4000)
        lpp = -h_out.copy(); lpp[0] = 1 - h_out[0]
        prem = h_lp.min() >= 0 and lpp.min() >= 0 and abs(h_lp.sum() - 1) < 1e-9 and abs(lpp.sum() - 1) < 1e-6
        fig(f"{fs} constraints (HP_in zero at DC; HP_out bilinear; LP b ≥ 0, unity DC)", "all hold", f"{hp_ok and bil and lpc}", hp_ok and bil and lpc)
        fig(f"{fs} the bound's premises (1 − HP_out and LP non-negative, each summing to 1)", "hold", f"{prem}", prem)
        # each section's -3 dB point and the 1 kHz group delay
        f = np.logspace(0, np.log10(0.499 * fs), 20000)
        m_in = np.abs(sect(hp_in, f, fs)); m_in /= m_in[-1]
        m_out = np.abs(sect(hp_out, f, fs)); m_out /= m_out[-1]
        m_lp = np.abs(sect(lp, f, fs))
        f_in = f[np.argmax(m_in >= 1 / math.sqrt(2))]
        f_out = f[np.argmax(m_out >= 1 / math.sqrt(2))]
        below = np.where(m_lp < 1 / math.sqrt(2))[0]
        f_lp = f[below[0]] if len(below) else float("nan")
        r.update(f_in=f_in, f_out=f_out, f_lp=f_lp)
        ph = np.unwrap(np.angle(cascade([hp_in, hp_out, lp], [999.5, 1000.5], fs)))
        gd = -(ph[1] - ph[0]) / (2 * math.pi * 1.0)
        r["gd_ms"] = gd * 1e3
        fig(f"{fs} section −3 dB points (HP_in / HP_out / LP)", "report",
            f"{f_in:.1f} / {f_out:.1f} / {('above Nyquist' if math.isnan(f_lp) else f'{f_lp:.0f}')} Hz")
        fig(f"{fs} stage 2 group delay at 1 kHz", "report (not added to the latency, R-05)", f"{gd*1e3:.3f} ms")
        # the start phase that puts a host sample on the clipper-input peak at 1 kHz
        phi = float(np.angle(sect(hp_in, [1000.0], fs)[0]))
        r["start_phase"] = -phi
        fig(f"{fs} HP_in phase at 1 kHz / start phase", "report", f"{math.degrees(phi):+.2f}° / {math.degrees(-phi):+.2f}°")
        fig(f"{fs} clip limits in clipper units (L− / L+)", "report", f"{l_neg:.6f} / {l_pos:.6f}")
        # settle: +13 dBFS for 50 ms, then -20 dBFS, against the clipper-off twin
        n1 = int(0.05 * fs); n2 = int(0.1 * fs)
        t = np.arange(n1 + n2) / fs
        amp = np.where(np.arange(n1 + n2) < n1, 10 ** (13 / 20), 10 ** (-20 / 20))
        x = amp * np.cos(2 * math.pi * 1000 * t + r["start_phase"])
        y_c = run_chain(x, r, l_pos, l_neg, KNEE_K, True)
        y_l = run_chain(x, r, l_pos, l_neg, KNEE_K, False)
        e = np.abs(y_c - y_l)
        tol = 10 ** (-20 / 20) * (10 ** (0.1 / 20) - 1)
        late = np.where(e[n1:] > tol)[0]
        settle_ms = (late[-1] + 1) / fs * 1e3 if len(late) else 0.0
        r["settle_ms"] = settle_ms
        fig(f"{fs} recovery within ±0.1 dB after +13 dBFS for 50 ms (US2 AS4)", "≤ 20 ms", f"{settle_ms:.2f} ms", settle_ms <= 20)
        # harmonics of a tone 6 dB above the first (negative) onset: discrete vs continuous
        a = l_neg_e * a_on * 10 ** (6 / 20)
        h_d = harmonics(fs, r, a, l_pos, l_neg)
        r["h2_db"], r["h3_db"] = h_d
        per_rate[fs] = r
    hc = harmonics_continuous(nom, l_pos_e, l_neg_e, a_on)
    for fs in RATES:
        r = per_rate[fs]
        fig(f"{fs} H2 / H3 re the fundamental, 6 dB above the first onset (discrete)", "report (US2 AS3 pins these ±0.5 dB)",
            f"{r['h2_db']:.2f} / {r['h3_db']:.2f} dB (alias {r['h2_db']-hc[0]:+.2f} / {r['h3_db']-hc[1]:+.2f} dB vs continuous {hc[0]:.2f} / {hc[1]:.2f})")
    return per_rate, dict(l_pos_e=l_pos_e, l_neg_e=l_neg_e, a_on=a_on, v_fs_o=v_fs_o)


def harmonics(fs, r, a, l_pos, l_neg):
    periods = 100
    n = int(round(fs * periods / 1000))
    warm = int(fs * 0.5)
    t = np.arange(warm + n) / fs
    x = a * np.cos(2 * math.pi * 1000 * t + r["start_phase"])
    y = run_chain(x, r, l_pos, l_neg, KNEE_K, True)[warm:]
    spec = np.fft.rfft(y)
    f1 = abs(spec[periods])
    out = []
    for hnum in (2, 3):
        k = periods * hnum
        out.append(20 * math.log10(abs(spec[k]) / f1) if k < len(spec) else float("nan"))
    return out


def harmonics_continuous(nom, l_pos_e, l_neg_e, a_on):
    """A near-continuous reference: the analog prototype's sections bilinear at 1.536 MHz."""
    fs = 1536000
    def bil_hp(fc):
        kk = math.tan(math.pi * fc / fs); b0 = 1 / (1 + kk); return [b0, -b0, (kk - 1) / (kk + 1)]
    def bil_lp(fc):
        kk = math.tan(math.pi * fc / fs); b0 = kk / (1 + kk); return [b0, b0, (kk - 1) / (kk + 1)]
    hp_in, hp_out, lp = bil_hp(nom["fa"]), bil_hp(nom["fb"]), bil_lp(nom["fc"])
    g = 1 / abs(cascade([hp_in, hp_out, lp], [1000.0], fs)[0])
    hp_in = [hp_in[0] * g, hp_in[1] * g, hp_in[2]]
    tail = abs(cascade([hp_out, lp], [1000.0], fs)[0])
    r = dict(hp_in=hp_in, hp_out=hp_out, lp=lp, start_phase=-float(np.angle(sect(hp_in, [1000.0], fs)[0])))
    return harmonics(fs, r, l_neg_e * a_on * 10 ** (6 / 20), l_pos_e / tail, l_neg_e / tail)


# =====================================================================================
# Part 3: the hiss and the generator (R-04, R-07)
# =====================================================================================

def a_weight(f):
    f = np.asarray(f, float)
    f2 = f * f
    ra = (12194.0 ** 2 * f2 * f2) / ((f2 + 20.6 ** 2) * np.sqrt((f2 + 107.7 ** 2) * (f2 + 737.9 ** 2)) * (f2 + 12194.0 ** 2))
    return ra / ((12194.0 ** 2 * 1e12) / ((1e6 + 20.6 ** 2) * np.sqrt((1e6 + 107.7 ** 2) * (1e6 + 737.9 ** 2)) * (1e6 + 12194.0 ** 2)))


def integrate(fn, lo, hi, n=40001):
    f = np.linspace(lo, hi, n)
    return np.trapezoid(fn(f), f) if hasattr(np, "trapezoid") else np.trapz(fn(f), f)


def hiss(nom, per_rate):
    print("=" * 100)
    print("Part 3: the hiss and the generator (R-04, R-07)")
    print("=" * 100)
    h1 = abs(ana([1000.0], nom)[0])
    hn = lambda f: np.abs(ana(f, nom) / h1) ** 2
    p_a = integrate(lambda f: a_weight(f) ** 2, 20, 20000)
    p_band = integrate(hn, 300, 3000)
    off = 10 * math.log10(p_band / p_a)
    level = SELF_NOISE_DBA - FULL_SCALE_SPL + off
    fig("A-weighted to 300–3000 Hz band offset through stage 2", "≈ −6 dB", f"{off:+.2f} dB", -7 <= off <= -5)
    fig("hiss in the 300–3000 Hz band at stage 2's output (AES17)", "−62 ± 0.5 dBFS (FR-003)", f"{level:.2f} dBFS", abs(level + 62) <= 0.5)
    # third octaves (analog prototype, re AES17 full scale)
    n0 = 10 ** (level / 10) / p_band  # PSD (dBFS-power per Hz) with the band at `level`
    def toct(fc):
        return 10 * math.log10(n0 * integrate(hn, fc * 2 ** (-1 / 6), fc * 2 ** (1 / 6)))
    shape = [(fc, toct(fc)) for fc in SHAPE_CENTRES]
    fig("third-octave levels 315–2500 Hz (dBFS)", "report (US1 AS2 pins these ±1 dB)",
        ", ".join(f"{fc}: {l:.2f}" for fc, l in shape))
    t1k = dict(shape)[1000]
    fig("1 kHz third octave re the band level", "report", f"{t1k - level:+.2f} dB")
    # the amplifier's and R2's added floor (A-050; plan review Q10)
    _, gsx = nodal([1000.0])
    amp_floor = -73.0
    fig("amplifier and R2 floor (A-050, from the reviewer's noise model)", "report", f"≈ {amp_floor:.0f} dBFS (adds {10*math.log10(1+10**((amp_floor-level)/10)):.2f} dB at the anchor)")
    lo_c = (SELF_NOISE_DBA - 4) - (FULL_SCALE_SPL + 4) + off
    hi_c = (SELF_NOISE_DBA + 4) - (FULL_SCALE_SPL - 4) + off
    fig("credible range (S/N 58–62 dB held, full scale 86–94 dB SPL)", "≈ −70 … −54 dBFS (A-048)", f"{lo_c:.1f} … {hi_c:.1f} dBFS")
    # sigma per rate through the realized response
    for fs in RATES:
        r = per_rate[fs]
        secs = [r["hp_in"], r["hp_out"], r["lp"]]
        top = min(3000.0, fs / 2)
        pb = integrate(lambda f: np.abs(cascade(secs, f, fs)) ** 2, 300, top)
        target_rms = 10 ** (level / 20) / math.sqrt(2)
        sigma = target_rms / math.sqrt(pb / (fs / 2))
        r["sigma"] = sigma
        fig(f"{fs} hiss source σ (white at the host rate)", "report", f"{sigma:.6e}")
    # the voice activation (R-07): the hiss at the VAS input approximated by its 225–2550 Hz level
    p_vas = integrate(hn, 225, 2550)
    l_vas = 10 * math.log10(n0 * p_vas)
    fig("hiss RMS at the voice-activation input (≈ 225–2550 Hz)", "report", f"{l_vas:.2f} dBFS")
    for lvl, thr in enumerate(VAS_THRESHOLDS, start=1):
        m = thr - l_vas
        fig(f"level {lvl} RMS margin ({thr:.0f} dBFS)", "≥ 10 dB at level 5 (SC-010)" if lvl == 5 else "report",
            f"{m:.1f} dB (estimated stop offset ≈ {m - 6.5:+.1f} dB)", m >= 10 if lvl == 5 else True)
    return level


def ziggurat():
    """Doornik's ZIGNOR layout (128 layers): X[0..=128], RATIO[i] = X[i+1] / X[i]."""
    f = lambda x: math.exp(-0.5 * x * x)
    x = [0.0] * (ZIG_LAYERS + 1)
    x[0] = ZIG_V / f(ZIG_R)
    x[1] = ZIG_R
    for i in range(2, ZIG_LAYERS):
        x[i] = math.sqrt(-2.0 * math.log(ZIG_V / x[i - 1] + f(x[i - 1])))
    x[ZIG_LAYERS] = 0.0
    ratio = [x[i + 1] / x[i] for i in range(ZIG_LAYERS)]
    ok = all(x[i] > x[i + 1] for i in range(1, ZIG_LAYERS)) and x[ZIG_LAYERS - 1] < 0.5
    fig("ziggurat tables (128 layers)", "X decreasing; X[127] small", f"X[0] {x[0]:.6f}, X[1] {x[1]:.6f}, X[127] {x[127]:.6f}", ok)
    p_cap = (0.013) ** RETRY_CAP  # the per-attempt rejection probability is about 1.3 %
    fig("retry cap", f"{RETRY_CAP} attempts per loop (A-049)", f"P(cap) ≈ {p_cap:.1e} per sample", p_cap < 1e-12)
    return x, ratio


def mix64(z):
    z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9 & M64
    z = (z ^ (z >> 27)) * 0x94D049BB133111EB & M64
    return z ^ (z >> 31)


def key(seed, counter):
    z = mix64(seed ^ MIC_STREAM) ^ mix64(counter ^ MIC_COUNTER_SALT)
    s = []
    for _ in range(4):
        z = (z + 0x9E3779B97F4A7C15) & M64
        s.append(mix64(z))
    return s


def xoshiro_next(s):
    rotl = lambda v, k: ((v << k) | (v >> (64 - k))) & M64
    result = (rotl((s[0] + s[3]) & M64, 23) + s[0]) & M64
    t = (s[1] << 17) & M64
    s[2] ^= s[0]; s[3] ^= s[1]; s[1] ^= s[2]; s[0] ^= s[3]; s[2] ^= t; s[3] = rotl(s[3], 45)
    return result


def keying():
    vecs = {}
    for sd, ct in ((0, 0), (1, 0), (0, 1)):
        st = key(sd, ct)
        first = list(st)
        outs = [xoshiro_next(st) for _ in range(3)]
        vecs[(sd, ct)] = (first, outs)
        fig(f"key({sd}, {ct}) state", "pinned by the core test", " ".join(f"{v:#018x}" for v in first), all(first))
        fig(f"key({sd}, {ct}) first three outputs", "pinned by the core test", " ".join(f"{v:#018x}" for v in outs))
    return vecs


# =====================================================================================
# --spice, --write, main
# =====================================================================================

SPICE = Path(__file__).with_name("rr_dr60_mic.cir")


def spice(nom):
    print("=" * 100)
    print("ngspice cross-check (R-01; a gate before the golden bless)")
    print("=" * 100)
    exe = shutil.which("ngspice")
    if exe is None:
        print("ngspice not found: install it (brew install ngspice) to run the gate")
        return False
    with tempfile.TemporaryDirectory() as d:
        out = Path(d) / "gsx.txt"
        net = SPICE.read_text().replace("@OUT@", str(out))
        cir = Path(d) / "mic.cir"
        cir.write_text(net)
        run = subprocess.run([exe, "-b", str(cir)], capture_output=True, text=True, timeout=120)
        if not out.exists():
            print(run.stdout[-2000:], run.stderr[-2000:])
            return False
        data = np.loadtxt(out)
    f, re_, im_ = data[:, 0], data[:, 1], data[:, 2]
    h = np.abs(re_ + 1j * im_)
    target = 20 * np.log10(h)

    def res(p):
        k, fa, fb, fc = np.exp(p)
        return 20 * np.log10(np.abs(three_pole(f, k, fa, fb, fc))) - target

    sol = least_squares(res, np.log([h[len(h) // 2], 800.0, 77.0, 13000.0]))
    k, fa, fb, fc = np.exp(sol.x)
    for line in run.stdout.splitlines():
        if line.strip().startswith(("v(o)", "v(e)", "v(c3)", "v(b)")):
            print("  op:", line.strip())
    dev = fa / nom["fa"] - 1
    print(f"  ngspice corners: {fa:.1f} / {fb:.1f} / {fc:.0f} Hz (fit residual {np.max(np.abs(res(sol.x))):.3f} dB)")
    print(f"  nodal corners:   {nom['fa']:.1f} / {nom['fb']:.1f} / {nom['fc']:.0f} Hz")
    print(f"  input corner difference: {dev*100:+.1f} % (gate: within ±10 %)")
    return abs(dev) <= 0.10


def hexbits(x):
    return "0x" + struct.pack(">d", float(x)).hex()


def write(per_rate, clipinfo, xz, ratio):
    lines = [
        "//! Microphone-path coefficients and design constants (signal-chain stages 1 – 2; spec 012).",
        "//!",
        "//! GENERATED by `tools/mic-chain/design_mic_chain.py --write`; do not edit by hand.",
        "//! Regenerate with: `uv run tools/mic-chain/design_mic_chain.py --write`.",
        "//!",
        "//! Every figure is a design constant modeled on the circuit (S-002 sheet 3.1): A-046 (the input",
        "//! high-pass), A-047 (the output coupling and the roll-off), A-053 (both lumped ahead of the",
        "//! limiter), A-051 (the clip limits and the knee), A-048 (the hiss level), A-049 (the generator).",
        "//! Nothing here is a measurement of a real unit.",
        "//!",
        "//! Sections are `[b0, b1, a1]` for `H(z) = (b0 + b1 z⁻¹) / (1 + a1 z⁻¹)`, a per-rate constrained",
        "//! fit to the circuit's three-pole prototype (research R-02), indexed by `SUPPORTED_HOST_RATES`",
        "//! order; exact bit patterns (001 R-04).",
        "//!",
        "//! Figures the script printed (the Rust pins these, never its own output):",
    ]
    w1 = max(len(n) for n, _, _, _ in FIGURES)
    w2 = max(len(r) for _, r, _, _ in FIGURES)
    for n, r, v, _ in FIGURES:
        lines.append(f"//!   {n.ljust(w1)}   {r.ljust(w2)}   {v}")
    lines += ["", "#![allow(clippy::unreadable_literal, clippy::excessive_precision)]", ""]

    def table(name, doc, key_):
        lines.append(f"/// {doc}")
        lines.append(f"pub(crate) const {name}: [[f64; 3]; 7] = [")
        for fs in RATES:
            row = per_rate[fs][key_]
            # rustfmt's layout for a row longer than the line limit (so --write and `cargo fmt` agree)
            lines.append("    [")
            for v in row:
                lines.append(f"        f64::from_bits({hexbits(v)}),")
            lines.append(f"    ], // {fs} Hz")
        lines.append("];")
        lines.append("")

    table("HP_IN", "Stage 2's input high-pass with the 1 kHz normalization in its numerator (A-046).", "hp_in")
    table("HP_OUT", "Stage 2's output coupling, bilinear form (A-047, A-053).", "hp_out")
    table("LP", "Stage 2's roll-off: b0, b1 ≥ 0, unity DC gain (A-047, A-053).", "lp")

    def per(name, doc, key_):
        lines.append(f"/// {doc}")
        lines.append(f"pub(crate) const {name}: [f64; 7] = [")
        for fs in RATES:
            v = per_rate[fs][key_]
            lines.append(f"    f64::from_bits({hexbits(v)}), // {fs} Hz: {v:.9g}")
        lines.append("];")
        lines.append("")

    per("HISS_SIGMA", "The hiss source's RMS per host rate at offset 0 (A-048).", "sigma")
    per("CLIP_POS", "The clipper's positive limit L+ in clipper units per host rate (A-051).", "l_pos")
    per("CLIP_NEG", "The clipper's negative limit L− in clipper units per host rate (A-051).", "l_neg")
    per("START_PHASE", "The 1 kHz start phase that puts a host sample on the clipper-input peak (radians; harness).", "start_phase")
    lines.append("/// The knee start (A-051; spec 009's knee).")
    lines.append(f"pub(crate) const KNEE_K: f64 = f64::from_bits({hexbits(KNEE_K)}); // {KNEE_K}")
    lines.append("")
    lines.append("/// The ziggurat's layer abscissae, Doornik's ZIGNOR layout, 128 layers (A-049).")
    lines.append(f"pub(crate) const ZIG_X: [f64; {ZIG_LAYERS + 1}] = [")
    for v in xz:
        lines.append(f"    f64::from_bits({hexbits(v)}),")
    lines.append("];")
    lines.append("")
    lines.append("/// The ziggurat's rectangle ratios X[i + 1] / X[i] (A-049).")
    lines.append(f"pub(crate) const ZIG_RATIO: [f64; {ZIG_LAYERS}] = [")
    for v in ratio:
        lines.append(f"    f64::from_bits({hexbits(v)}),")
    lines.append("];")
    lines.append("")
    lines.append("/// The ziggurat's tail start R (A-049).")
    lines.append(f"pub(crate) const ZIG_TAIL_R: f64 = f64::from_bits({hexbits(ZIG_R)}); // {ZIG_R}")
    lines.append("")
    lines.append("/// The capsule's stream constant (A-049).")
    lines.append(f"pub(crate) const MIC_STREAM: u64 = {MIC_STREAM:#018x};")
    lines.append("/// The recording counter's salt (A-049).")
    lines.append(f"pub(crate) const MIC_COUNTER_SALT: u64 = {MIC_COUNTER_SALT:#018x};")
    lines.append("/// Attempts per rejection loop before the last candidate is returned (A-049).")
    lines.append(f"pub(crate) const RETRY_CAP: u32 = {RETRY_CAP};")
    path = Path(__file__).resolve().parents[2] / "crates/rr_dr60/src/stages/mic_coeffs.rs"
    path.write_text("\n".join(lines) + "\n")
    print(f"wrote {path}")


def main():
    args = set(sys.argv[1:])
    nom = circuit()
    per_rate, clipinfo = realization(nom)
    hiss(nom, per_rate)
    xz, ratio = ziggurat()
    keying()
    print("=" * 100)
    print("Figures")
    print("=" * 100)
    w1 = max(len(n) for n, _, _, _ in FIGURES)
    w2 = max(len(r) for _, r, _, _ in FIGURES)
    for n, r, v, ok in FIGURES:
        print(f"{'  ' if ok else '✗ '}{n.ljust(w1)}   {r.ljust(w2)}   {v}")
    if "--spice" in args:
        if not spice(nom):
            print("spice gate: FAIL")
            sys.exit(1)
        print("spice gate: PASS")
    if "--write" in args:
        write(per_rate, clipinfo, xz, ratio)
    if "--check" in args:
        bad = [n for n, _, _, ok in FIGURES if not ok]
        if bad:
            print("check: FAIL"); [print("  ", b) for b in bad]; sys.exit(1)
        print(f"check: PASS ({len(FIGURES)} figures)")


if __name__ == "__main__":
    main()
