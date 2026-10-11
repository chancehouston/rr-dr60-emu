#!/usr/bin/env python3
"""Reference model of the RR-DR60 record-path AGC (signal-chain stage 3; spec 005).

Two simulations of the limiter in the service manual's schematic (S-002, sheet 3.1): Q6
half-wave peak detector on the codec input-amp output (GSX), R12 390 ohm into the hold capacitor
C6 1 uF, R11 10 Mohm discharge, Q5 emitter follower driving the shunt transistor Q4 at the node
after R7 22 kohm, R10 1 kohm into the codec's inverting input amp (gain 82/23). Component values
are S-002's; transistor parameters are estimates (A-028, A-034, A-035, A-036).

1. The **reference**: a 192 kHz Euler integration of the continuous loop (`simulate`).
2. The **emulator's discretization** (research R-03; `emulate`): the same loop stepped at a host
   rate with the analytic charge cap, the exact-unity floor and the flush-to-zero rule, run at the
   seven supported host rates to show it reproduces the reference without over-attenuation.

Both use the emulator's conventions: the sensed polarity is positive input samples (A-036) and a
tone starts at phase 0 (reference) or at the host-grid peak phase (emulator; research R-08). The
default knee is -1 dBFS (A-028, after the post-implementation review of 2026-10-11); the attack
mapping is referenced to the +1 dBFS knee at which T_REF was first measured (R-05). Every level in
the default-settings sections is relative to the default knee (10 dB over = +9 dBFS).
The script prints the figures the spec and research cite: the static curve and knee, the dead
time versus overdrive and host rate (including the knee-aware attack mapping, research R-05),
the armed dead time, the release trajectory versus the release setting, the unity floor and the
flush time, the polarity difference, the bandwidth-test expectation and the clipped-onset duty.

Usage:  uv run tools/agc-model/limiter_model.py          # or: python3 tools/agc-model/limiter_model.py
Pure Python, no dependencies. Numbers are design references, not measurements of a unit.
"""
import math

# --- S-002 component values -------------------------------------------------------------------
C6 = 1e-6            # hold capacitor, F
R11 = 10e6           # discharge, ohm (R11 * C6 = 10 s: the arming memory)
R12 = 390.0          # detector collector resistor, ohm (limits the peak charge current)
R13, R17 = 100e3, 330e3   # Q6 base divider from GSX and the 1.6 V reference
RTH = R13 * R17 / (R13 + R17)   # Thevenin source resistance at Q6's base, 76.7 kohm
KDIV = R17 / (R13 + R17)         # fraction of the GSX swing reaching Q6's base, 0.767
R7, R10, R_IN = 22e3, 1e3, 23e3  # series resistor, node-to-codec resistor, codec amp input (R10 + 22k)
# --- estimates (A-028: knee, rail; A-034: dead time; A-035: release; A-036: polarity) -----------
VT = 0.026           # thermal voltage, V
IS = 1e-14           # transistor saturation current (Vbe = VT ln(I/IS))
BETA6_REF = 300.0    # Q6 current gain (150-600 bounds the dead time)
BETA5 = 300.0        # Q5 current gain (its base current adds to the discharge)
BETA_R = 5.0         # Q4 reverse beta (saturated shunt conductance)
V_FS = 0.7           # GSX full scale, peak (A-030: +3 dBm0 = 0 dBFS)
V_RAIL = 1.2         # codec input-amp swing limit at GSX, V (estimate; the detector's view only)
V_REF = 1.6          # AIN+ reference, V (S-002)
POLARITY = 1.0       # positive input samples are sensed (A-036)
# --- settings defaults (spec 005 FR-010) --------------------------------------------------------
KNEE_DBFS = -1.0     # default knee (A-028): VBE6 = KDIV * V_FS * 10^(knee/20) = 0.478 V. Moved from +1.0 after
                     # the post-implementation review (2026-10-11): a hard 0.6 V threshold is the attack-phase
                     # Vbe; at the detector's steady-state current the ceiling sits about 2 dB lower (see the
                     # "Self-consistent ceiling" section), just below the codec's full scale.
KNEE_REF_DBFS = 1.0  # the knee the attack mapping is referenced to (R-05; T_REF is measured here; VBE6 0.602 V)
ATTACK_MS = 3.0      # dead time at 10 dB over the knee (A-034)
RELEASE_MS = 1370.0  # time to -1 dB from 10 dB of attenuation (A-035); k_dis = RELEASE_MS / release_ms
V_FLUSH = 1e-6       # vc below this is set to 0.0 (engineering target, R-03)
MAX_ATTEN_DB = 40.0  # attenuation limit (engineering target)
HOST_RATES = (6000, 8000, 16000, 44100, 48000, 88200, 96000)


def vbe6_of_knee(knee_dbfs):
    return KDIV * V_FS * 10 ** (knee_dbfs / 20.0)


def ib4_of_vc(vc):
    """Q4 base current for a capacitor voltage: two stacked Vbe's (Q5 then Q4), I = IS exp(vc / 2VT)."""
    return IS * math.exp(vc / (2.0 * VT))


def gain_of_ib4(ib4):
    """Linear gain of the R7 / (Q4 shunt || R10 + codec amp) divider relative to its unloaded value."""
    g = (BETA_R + 1.0) * ib4 / VT        # saturated-BJT conductance at Vce ~ 0
    r = 1.0 / g if g > 0 else 1e12
    zn = r * R10 / (r + R10)            # Q4 in parallel with R10 (the codec input path)
    return (zn / (R7 + zn)) / (R10 / R_IN)


def gain_of_vc(vc):
    return gain_of_ib4(ib4_of_vc(vc))


def vc_of_gain(a):
    """Closed-form inverse of gain_of_vc (research R-03); +inf when a >= 1."""
    q = a * (R10 / R_IN)
    if q >= R10 / R_IN:
        return math.inf
    zn = q * R7 / (1.0 - q)
    r = zn * R10 / (R10 - zn)
    ib4 = VT / ((BETA_R + 1.0) * r)
    return 2.0 * VT * math.log(ib4 / IS)


def vc_target(ax, vbe6):
    """The capacitor voltage at which a sample of amplitude |x| (FS units) no longer exceeds the threshold."""
    if ax <= 0.0:
        return math.inf
    a_t = (vbe6 / KDIV) / (ax * V_FS)
    return math.inf if a_t >= 1.0 else vc_of_gain(a_t)


def v_floor():
    """vc at which the shunt's deviation from unity reaches 2^-24 (research R-03; P4): below it gain is exactly 1.0."""
    lo, hi = 0.0, 1.0
    for _ in range(60):
        mid = 0.5 * (lo + hi)
        lo, hi = (mid, hi) if 1.0 - gain_of_vc(mid) < 2.0 ** -24 else (lo, mid)
    return 0.5 * (lo + hi)


def detector_current_index(knee_dbfs, n=360):
    """Mean over one cycle of the detector's overdrive for a 1 kHz sine at knee + 10 dB (research R-05):
    I(knee) = mean_theta max(0, KDIV * clamp(V_FS * A * sin theta, +-V_RAIL) - VBE6(knee)), A = 10^((knee+10)/20)."""
    vbe6 = vbe6_of_knee(knee_dbfs)
    amp = V_FS * 10 ** ((knee_dbfs + 10.0) / 20.0)
    s = 0.0
    for k in range(n):
        gsx = max(-V_RAIL, min(V_RAIL, amp * math.sin(2.0 * math.pi * (k + 0.5) / n)))
        s += max(0.0, KDIV * gsx - vbe6)
    return s / n


def beta6_of_settings(knee_dbfs, attack_ms, t_ref_ms):
    """Knee-aware attack mapping (R-05): BETA6 = BETA6_REF * (t_ref / attack) * I(reference knee) / I(knee),
    where t_ref is the reference dead time at BETA6_REF and the reference knee (+1 dBFS, KNEE_REF_DBFS)."""
    return BETA6_REF * (t_ref_ms / attack_ms) * detector_current_index(KNEE_REF_DBFS) / detector_current_index(knee_dbfs)


def self_consistent_ceiling_dbfs(i_s, i_avg):
    """The GSX peak (dBFS) at which an *exponential* Q6 (Is = i_s) conducts `i_avg` on average over a cycle:
    mean over the sensed half-cycle of Is * exp(KDIV * Vp * sin(theta) / VT) = i_avg. The model uses a hard 0.6 V
    threshold instead (the Vbe at the attack's mA charge peaks); this is the steady-state check behind the
    post-implementation review's finding B1 (2026-10-11)."""
    def i_mean(vp):
        n = 3600
        return sum(i_s * math.exp(KDIV * vp * math.sin(math.pi * (k + 0.5) / n) / VT) for k in range(n)) / (2 * n)
    lo, hi = 0.1, 2.0
    for _ in range(50):
        mid = 0.5 * (lo + hi)
        lo, hi = (mid, hi) if i_mean(mid) < i_avg else (lo, mid)
    return 20 * math.log10(0.5 * (lo + hi) / V_FS)


def peak_phase(freq, fs):
    """Host-grid phase rule (R-08): a host sample lands on the positive peak."""
    return (math.pi / 2.0) % (2.0 * math.pi * freq / fs)


def loop(samples, fs, vbe6, beta6, k_dis=1.0, vc0=0.0, discretized=False, vfloor=0.0, v_limit=math.inf):
    """Run the loop over `samples` (FS units). Returns (gain_db[], vc[]) per sample.
    discretized=True applies the emulator's rules: exact unity below vfloor, the charge cap, the limit, the flush."""
    dt = 1.0 / fs
    vc = vc0
    gains, vcs = [], []
    for x in samples:
        a = 1.0 if (discretized and vc < vfloor) else gain_of_vc(vc)
        y = a * x
        gsx = max(-V_RAIL, min(V_RAIL, y * V_FS))
        over = KDIV * POLARITY * gsx - vbe6
        ic = 0.0
        if over > 0.0:
            ic = min(beta6 * over / RTH, (V_REF - vc) / R12 if vc < V_REF else 0.0)
        idis = k_dis * (vc / R11 + ib4_of_vc(vc) / BETA5)
        vc = max(0.0, vc + (ic - idis) * dt / C6)
        if discretized:
            if ic > 0.0:
                vc = min(vc, vc_target(abs(x), vbe6))
            vc = min(vc, v_limit)
            if vc < V_FLUSH:
                vc = 0.0
        gains.append(20.0 * math.log10(max(a, 1e-9)))
        vcs.append(vc)
    return gains, vcs


def tone(levels_db, durations_s, fs, freq=1000.0, phase=0.0):
    """A 1 kHz tone stepping through `levels_db` (dBFS peak), continuous phase across segments."""
    out = []
    n = 0
    w = 2.0 * math.pi * freq / fs
    for lvl, dur in zip(levels_db, durations_s):
        amp = 10 ** (lvl / 20.0)
        for _ in range(int(round(dur * fs))):
            out.append(amp * math.sin(w * n + phase))
            n += 1
    return out


def simulate(levels_db, durations_s, fs=192_000.0, **kw):
    """Reference run (continuous loop at 192 kHz, phase 0)."""
    kw.setdefault("vbe6", vbe6_of_knee(KNEE_DBFS))
    kw.setdefault("beta6", BETA6_REF)
    return loop(tone(levels_db, durations_s, fs), fs, **kw)


def emulate(levels_db, durations_s, fs, **kw):
    """Emulator run (discretized loop at a host rate, host-grid peak phase)."""
    kw.setdefault("vbe6", vbe6_of_knee(KNEE_DBFS))
    kw.setdefault("beta6", BETA6_REF)
    kw.setdefault("vfloor", V_FLOOR)
    kw.setdefault("v_limit", vc_of_gain(10 ** (-MAX_ATTEN_DB / 20.0)))
    return loop(tone(levels_db, durations_s, fs, phase=peak_phase(1000.0, fs)), fs, discretized=True, **kw)


def mean(xs):
    return sum(xs) / len(xs)


def steady_db(gains, fs, t_from, t_to):
    return mean(gains[int(t_from * fs):int(t_to * fs)])


def dead_time_ms(gains, fs, t0, steady):
    """First sample after t0 whose attenuation exceeds half the steady attenuation (spec FR-005, plan review P1)."""
    half = 0.5 * steady
    for n in range(int(t0 * fs), len(gains)):
        if gains[n] < half:
            return (n / fs - t0) * 1e3
    return None


def ramp_ms(gains, fs, t0, steady):
    """From the dead-time point to within 1 dB of the steady value."""
    half = 0.5 * steady
    n_dead = next(n for n in range(int(t0 * fs), len(gains)) if gains[n] < half)
    n_done = next(n for n in range(n_dead, len(gains)) if gains[n] < steady + 1.0)
    return (n_done - n_dead) / fs * 1e3


def release_s(gains, fs, t0, to_db=-1.0):
    n = next((n for n in range(int(t0 * fs), len(gains)) if gains[n] > to_db), None)
    return None if n is None else n / fs - t0


V_FLOOR = v_floor()


def main():
    print("# RR-DR60 limiter reference model (spec 005). S-002 values; transistor parameters estimated.")
    print(f"# conventions: sensed polarity +1 (A-036), phase 0; knee {KNEE_DBFS:+.1f} dBFS -> VBE6 {vbe6_of_knee(KNEE_DBFS):.3f} V\n")

    # ---- static curve -------------------------------------------------------------------------
    print("## Static curve (1 kHz, 1 s tones, last 0.1 s; gain re unity, output peak in dBFS)")
    for db in (-20, -3, -1, 0, 1, 2, 3, 4, 6, 8, 12, 18, 20):
        g, vc = simulate([db], [1.0])
        gs = steady_db(g, 192_000, 0.9, 1.0)
        print(f"  in {db:+3d} dBFS -> gain {gs:7.2f} dB, out peak {db + gs:+6.2f} dBFS, Vc {vc[-1]:.3f} V")
    lo, hi = -2.0, 6.0
    for _ in range(14):
        mid = 0.5 * (lo + hi)
        g, _ = simulate([mid], [1.0])
        lo, hi = (lo, mid) if steady_db(g, 192_000, 0.9, 1.0) < -1.0 else (mid, hi)
    print(f"  knee (steady gain = -1 dB): {0.5 * (lo + hi):+.2f} dBFS")
    ceiling = 20 * math.log10(vbe6_of_knee(KNEE_DBFS) / KDIV / V_FS)
    c_fs = 10 ** (ceiling / 20)
    clip = f"{100 * (1 - 2 / math.pi * math.asin(1 / c_fs)):.0f} % of a steady sine's samples at +-1.0" if c_fs > 1 else "below full scale: no steady clipping"
    print(f"  ceiling (detector onset) {ceiling:+.2f} dBFS = {c_fs:.3f} FS (ADC clamp: {clip})\n")

    # ---- self-consistent ceiling (post-implementation review B1) -------------------------------
    print("## Self-consistent ceiling: an exponential Q6 holding the steady-state discharge current (review B1, 2026-10-11)")
    vc_10 = vc_of_gain(10 ** (-10.0 / 20.0))
    i_avg = vc_10 / R11 + ib4_of_vc(vc_10) / BETA5
    print(f"  discharge at 10 dB of attenuation (Vc {vc_10:.3f} V): {i_avg * 1e6:.3f} uA average, which the detector must replace")
    for i_s in (1e-15, 1e-14, 1e-13):
        print(f"  Is = {i_s:.0e}: ceiling {self_consistent_ceiling_dbfs(i_s, i_avg):+.2f} dBFS (hard-threshold model: {ceiling:+.2f})")
    print("  (the hard 0.6 V threshold is the Vbe at the attack's mA charge peaks; the default knee follows the Is = 1e-14 row)\n")

    # ---- discretized static curve (review B5): the figures the Rust unit test pins -------------
    print("## Static curve (emulator discretization, R-03): output peak over the last 0.1 s of a 1 s tone, 6 kHz / 48 kHz")
    for db in (KNEE_DBFS + 3, KNEE_DBFS + 4, KNEE_DBFS + 7, KNEE_DBFS + 13, 20.0):
        peaks = []
        for fs in (6000, 48000):
            g, _ = emulate([db], [1.0], fs, beta6=beta6_of_settings(KNEE_DBFS, ATTACK_MS, 2.167))
            xs = tone([db], [1.0], fs, phase=peak_phase(1000.0, fs))
            ys = [x * 10 ** (gg / 20) for x, gg in zip(xs, g)]
            peaks.append(20 * math.log10(max(abs(y) for y in ys[int(0.9 * fs):])))
        print(f"  in {db:+5.1f} dBFS -> out peak {peaks[0]:+.3f} dBFS at 6 kHz, {peaks[1]:+.3f} at 48 kHz (ceiling {ceiling:+.2f}; the excess is the one-sample gain lag, no look-ahead)")
    print()

    # ---- attack: reference ---------------------------------------------------------------------
    print(f"## Attack (reference, 192 kHz, at the mapping's reference knee {KNEE_REF_DBFS:+.0f} dBFS): dead time to half the steady attenuation; step from -20 dBFS after Vc = 0")
    t_ref = None
    vbe_ref = vbe6_of_knee(KNEE_REF_DBFS)
    for b6 in (150.0, 300.0, 600.0):
        row = []
        for step in (3.0, 6.0, 11.0, 19.0):   # 2, 5, 10, 18 dB over the +1 dBFS reference knee
            g, _ = simulate([-20.0, step, -20.0], [0.05, 1.0, 0.1], beta6=b6, vbe6=vbe_ref)
            st = steady_db(g, 192_000, 1.0, 1.05)
            dt_ms = dead_time_ms(g, 192_000, 0.05, st)
            gmin = min(g[int(0.05 * 192_000):int(1.05 * 192_000)])
            row.append(f"{step:+.0f} dBFS: dead {dt_ms:6.2f} ms, ramp {ramp_ms(g, 192_000, 0.05, st):.2f} ms, steady {st:6.2f} dB, min {gmin:6.2f}")
            if b6 == BETA6_REF and step == 11.0:
                t_ref = dt_ms
        print(f"  beta6 {b6:3.0f}: " + " | ".join(row))
    g, _ = simulate([-20.0, 11.0], [0.05, 0.2], vc0=1.0, vbe6=vbe_ref)
    print(f"  armed (Vc0 = 1.0 V, as 1-3 s after a loud sound), +11 dBFS: dead time {dead_time_ms(g, 192_000, 0.05, steady_db(g, 192_000, 0.2, 0.25)):.3f} ms")
    print(f"  T_REF (beta6 {BETA6_REF:.0f}, knee {KNEE_REF_DBFS:+.0f} dBFS, +10 dB over): {t_ref:.3f} ms -> default attack {ATTACK_MS} ms at the default knee {KNEE_DBFS:+.0f} dBFS maps to beta6 {beta6_of_settings(KNEE_DBFS, ATTACK_MS, t_ref):.1f}\n")

    # ---- attack: emulator at the host rates ---------------------------------------------------
    b6_default = beta6_of_settings(KNEE_DBFS, ATTACK_MS, t_ref)
    K = KNEE_DBFS
    print(f"## Attack (emulator discretization, R-03) at the default settings (knee {K:+.0f} dBFS, beta6 {b6_default:.1f}); step {K - 21:+.0f} -> {K + 10:+.0f} dBFS (10 dB over)")
    print(f"   V_FLOOR {V_FLOOR:.4f} V (1 - gain = 2^-24), V_FLUSH {V_FLUSH:g} V, limit {MAX_ATTEN_DB:.0f} dB at Vc {vc_of_gain(10 ** (-MAX_ATTEN_DB / 20)):.3f} V")
    for fs in (192_000,) + HOST_RATES:
        run = (lambda *a, **k: simulate(*a, fs=192_000, beta6=b6_default)) if fs == 192_000 else (lambda *a, **k: emulate(*a, fs, beta6=b6_default))
        g, _ = run([K - 21.0, K + 10.0, K - 21.0], [0.05, 1.0, 0.1])
        st = steady_db(g, fs, 1.0, 1.05)
        gmin = min(g[int(0.05 * fs):int(1.05 * fs)])
        g2, _ = run([K - 21.0, K + 2.0], [0.05, 1.5])
        st2 = steady_db(g2, fs, 1.5, 1.55)
        g3, _ = run([K + 10.0, K - 21.0, K + 10.0], [0.2, 2.0, 0.2]) if fs != 192_000 else simulate([K + 10.0, K - 21.0, K + 10.0], [0.2, 2.0, 0.2], beta6=b6_default)
        st3 = steady_db(g3, fs, 2.35, 2.4)
        tag = "reference" if fs == 192_000 else "emulator "
        print(f"  {tag} {fs:6d} Hz: +10 dB-over dead {dead_time_ms(g, fs, 0.05, st):5.2f} ms, ramp {ramp_ms(g, fs, 0.05, st):.2f} ms, "
              f"over-attenuation {gmin - st:+.2f} dB | +2 dB-over dead {dead_time_ms(g2, fs, 0.05, st2):6.2f} ms (steady {st2:.2f} dB) | "
              f"armed (2 s after a 0.2 s burst) dead {dead_time_ms(g3, fs, 2.2, st3):.3f} ms")
    print()

    # ---- armed dead time versus the gap (review B2): the arming continuum -------------------------
    print("## Armed dead time versus the quiet gap (emulator, 48 kHz, 0.2 s burst at knee + 10 dB, then knee - 21 dB for the gap, then the burst)")
    fs = 48000
    row = []
    for gap in (1.0, 2.0, 3.0, 5.0, 10.0, 20.0):
        g, vc = emulate([K + 10.0, K - 21.0, K + 10.0], [0.2, gap, 0.2], fs, beta6=b6_default)
        t_on = 0.2 + gap
        st = steady_db(g, fs, t_on + 0.15, t_on + 0.2)
        row.append(f"{gap:4.0f} s: {dead_time_ms(g, fs, t_on, st):.2f} ms (Vc {vc[int(t_on * fs) - 1]:.3f} V)")
    g, _ = emulate([K - 21.0, K + 10.0], [0.05, 0.2], fs, beta6=b6_default)
    print("  " + " | ".join(row) + f" | fresh: {dead_time_ms(g, fs, 0.05, steady_db(g, fs, 0.2, 0.25)):.2f} ms")
    print("  (R11*C6 = 10 s is the arming decay's time constant, not the duration of the fast-attack state)\n")

    # ---- the golden onset stimulus (the harness's US3 AS3 pin) -------------------------------------
    g, _ = emulate([-20.0, 11.0, -20.0], [0.1, 0.4, 2.0], fs, beta6=b6_default)
    st = steady_db(g, fs, 0.45, 0.5)
    print(f"## Golden `agc_onset` stimulus at 48 kHz (-20 / +11 / -20 dBFS for 0.1 / 0.4 / 2.0 s; +11 dBFS is {11 - K:.0f} dB over the knee)")
    print(f"  dead time {dead_time_ms(g, fs, 0.1, st):.3f} ms, steady {st:.2f} dB, release to -1 dB {release_s(g, fs, 0.5):.4f} s\n")

    # ---- attack setting and knee-aware mapping ------------------------------------------------
    print("## Attack setting (dead time at knee + 10 dB, emulator at 6 kHz / 48 kHz) and the knee-aware mapping (R-05)")
    for knee in (KNEE_DBFS, KNEE_REF_DBFS, -10.0, -25.0, -40.0, 3.0):
        vbe6 = vbe6_of_knee(knee)
        row = []
        for att in (0.5, 1.0, 3.0, 10.0, 50.0):
            b6 = beta6_of_settings(knee, att, t_ref)
            vals = []
            for fs in (6000, 48000):
                g, _ = emulate([knee - 21.0, knee + 10.0], [0.05, 1.0], fs, vbe6=vbe6, beta6=b6)
                st = steady_db(g, fs, 1.0, 1.05)
                vals.append(dead_time_ms(g, fs, 0.05, st))
            row.append(f"{att:4.1f} -> {vals[0]:5.2f}/{vals[1]:5.2f}")
        print(f"  knee {knee:+5.1f} dBFS (I = {detector_current_index(knee):.4f} V): " + ", ".join(row) + " ms")
    print("  (beta6 is clamped to a sane range in the implementation; the +3 dBFS knee's I is bounded by the rail)\n")

    # ---- release -------------------------------------------------------------------------------
    print(f"## Release (reference, knee {K:+.0f} dBFS): 1 s burst, then knee - 21 dB; gain re unity at t after the burst")
    for burst in (K + 10.0, K + 17.0):
        g, _ = simulate([burst, K - 21.0], [1.0, 3.0], beta6=b6_default)
        att = -steady_db(g, 192_000, 0.95, 1.0)
        print(f"  burst {burst:+.0f} dBFS (attenuation {att:.1f} dB): to -1 dB in {release_s(g, 192_000, 1.0):.2f} s; "
              + " ".join(f"{t:.2f}s {g[int((1.0 + t) * 192_000)]:6.2f} dB" for t in (0.05, 0.1, 0.2, 0.3, 0.5, 0.75, 1.0, 1.5)))
    print(f"## Release setting: k_dis = RELEASE_MS / release_ms (emulator at 48 kHz, {K + 10:+.0f} dBFS 1 s burst)")
    for rel in (200.0, 1370.0, 4000.0, 10000.0):
        g, _ = emulate([K + 10.0, K - 21.0], [1.0, 12.0], 48000, beta6=b6_default, k_dis=RELEASE_MS / rel)
        print(f"  release {rel:7.0f} ms -> {release_s(g, 48000, 1.0):.3f} s to -1 dB (ratio {release_s(g, 48000, 1.0) * 1e3 / rel:.3f})")
    g, vc = emulate([K + 10.0, K - 21.0], [1.0, 20.0], 6000, beta6=b6_default)
    n_unity = next(n for n in range(6000, len(g)) if vc[n] < V_FLOOR)
    tau = R11 * C6
    print(f"  arming decay R11*C6 = {tau:.0f} s: Vc 1.1 -> 0.5 V takes {tau * math.log(1.1 / 0.5):.1f} s; "
          f"gain exactly 1.0 (Vc < V_FLOOR) {n_unity / 6000 - 1.0:.1f} s after the burst; "
          f"flush (Vc < V_FLUSH) about {tau * math.log(V_FLOOR / V_FLUSH):.0f} s later\n")

    # ---- polarity ------------------------------------------------------------------------------
    print(f"## Polarity (A-036): sin + 0.5 cos(2w) has peaks +0.75 / -1.5 (6.0 dB apart); larger peak scaled to knee + 3 dB ({K + 3:+.0f} dBFS), smaller {K - 3:+.0f} dBFS")
    fs = 192_000
    for sign in (1.0, -1.0):
        amp = 10 ** ((K + 3.0) / 20.0) / 1.5
        xs = [sign * amp * (math.sin(2 * math.pi * 1000 * n / fs) + 0.5 * math.cos(4 * math.pi * 1000 * n / fs)) for n in range(2 * fs)]
        g, _ = loop(xs, fs, vbe6_of_knee(KNEE_DBFS), b6_default)
        print(f"  sign {sign:+.0f} (positive peak {'+0.75' if sign > 0 else '+1.5'} x scale): steady gain {steady_db(g, fs, 1.9, 2.0):6.2f} dB")
    print()

    # ---- bandwidth test expectation (R-08, P11) ---------------------------------------------------
    print(f"## Bandwidth test (FR-007): 6 kHz at knee + 5 dB ({K + 5:+.0f} dBFS) plus a 1 kHz probe at -20 dBFS, 48 kHz host")
    fs = 48000
    xs = [10 ** ((K + 5.0) / 20) * math.sin(2 * math.pi * 6000 * n / fs) + 0.1 * math.sin(2 * math.pi * 1000 * n / fs) for n in range(3 * fs)]
    peak = 20 * math.log10(max(abs(x) for x in xs))
    g, _ = loop(xs, fs, vbe6_of_knee(KNEE_DBFS), b6_default, discretized=True, vfloor=V_FLOOR)
    print(f"  sum peak {peak:+.2f} dBFS vs ceiling {ceiling:+.2f} dBFS -> expected probe drop {peak - ceiling:.2f} dB; "
          f"simulated steady gain {steady_db(g, fs, 2.9, 3.0):.2f} dB\n")

    # ---- clipped onset duty (FR-008, Q4) ---------------------------------------------------------
    print(f"## Clipped onset (FR-008): fraction of samples at |y| >= 1.0 for a tone 10 dB over the knee ({K + 10:+.0f} dBFS) after the limiter, before the record filter")
    fs = 48000
    xs = tone([-20.0, K + 10.0], [0.05, 0.1], fs, phase=peak_phase(1000.0, fs))
    g, _ = loop(xs, fs, vbe6_of_knee(KNEE_DBFS), b6_default, discretized=True, vfloor=V_FLOOR)
    ys = [x * 10 ** (gg / 20) for x, gg in zip(xs, g)]
    n0 = int(0.05 * fs)
    first = ys[n0:n0 + int(0.002 * fs)]
    later = ys[n0 + int(0.02 * fs):n0 + int(0.1 * fs)]
    print(f"  first 2 ms: {100 * sum(abs(y) >= 1.0 for y in first) / len(first):.0f} % (unity gain: a {10 ** ((K + 10) / 20):.2f} FS sine has "
          f"{100 * (1 - 2 / math.pi * math.asin(1 / 10 ** ((K + 10) / 20))):.0f} % of its samples beyond 1.0); after 20 ms: "
          f"{100 * sum(abs(y) >= 1.0 for y in later) / len(later):.0f} %")


if __name__ == "__main__":
    main()
