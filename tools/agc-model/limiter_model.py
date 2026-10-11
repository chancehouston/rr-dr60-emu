#!/usr/bin/env python3
"""Reference model of the RR-DR60 record-path AGC (signal-chain stage 3; spec 005).

Continuous-time-ish simulation (192 kHz Euler) of the limiter in the service manual's schematic
(S-002, sheet 3.1): Q6 half-wave peak detector on the codec input-amp output (GSX), R12 390 ohm
into the hold capacitor C6 1 uF, R11 10 Mohm discharge, Q5 emitter follower driving the shunt
transistor Q4 at the node after R7 22 kohm, R10 1 kohm into the codec's inverting input amp
(gain 82/23). Component values are S-002's; transistor parameters are estimates (A-028, A-034,
A-035, A-036). It prints the figures the spec cites: the static curve and the knee, the attack
dead time versus overdrive and detector gain, the armed dead time, the release trajectory, and
the arming decay. It also runs the emulator's per-sample discretization (research R-03) at the
host rates, to show it reproduces the reference without over-attenuation.

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
# --- estimates (A-028: knee; A-034: dead time; A-035: release) ---------------------------------
VT = 0.026           # thermal voltage, V
IS = 1e-14           # transistor saturation current (Vbe = VT ln(I/IS))
VBE6 = 0.6           # Q6 turn-on, V (knee)
BETA6 = 300.0        # Q6 current gain (150-600 bounds the dead time)
BETA5 = 300.0        # Q5 current gain (its base current adds to the discharge)
BETA_R = 5.0         # Q4 reverse beta (saturated shunt conductance)
V_FS = 0.7           # GSX full scale, peak (A-030: +3 dBm0 = 0 dBFS)
V_RAIL = 1.2         # codec input-amp swing limit at GSX, V (estimate)
V_REF = 1.6          # AIN+ reference, V (S-002)


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


def simulate(levels_db, durations_s, fs=192_000.0, vc0=0.0, beta6=BETA6, beta5=BETA5, freq=1000.0, polarity=-1.0):
    """Returns (t, gain_db, vc) samples for a 1 kHz tone stepping through `levels_db` (dBFS peak)."""
    dt = 1.0 / fs
    vc = vc0
    out = []
    n = 0
    w = 2.0 * math.pi * freq
    for lvl, dur in zip(levels_db, durations_s):
        amp = V_FS * 10 ** (lvl / 20.0)
        for _ in range(int(dur * fs)):
            t = n * dt
            n += 1
            x = amp * math.sin(w * t + math.pi / 2.0)
            a = gain_of_vc(vc)
            gsx = max(-V_RAIL, min(V_RAIL, x * a))        # codec amp rail (sidechain view)
            over = KDIV * (polarity * gsx) - VBE6          # Q6 conducts on negative GSX swings
            ic6 = 0.0
            if over > 0.0:
                ic6 = min(beta6 * over / RTH, (V_REF - vc) / R12 if vc < V_REF else 0.0)
            idis = vc / R11 + ib4_of_vc(vc) / beta5
            vc = max(0.0, vc + (ic6 - idis) * dt / C6)
            out.append((t, 20.0 * math.log10(max(a, 1e-9)), vc))
    return out


def at(o, t):
    return next(g for tt, g, _ in o if tt >= t)


def dead_time_ms(o, t0):
    d = next((tt for tt, g, _ in o if tt >= t0 and g < -1.0), None)
    return None if d is None else (d - t0) * 1e3


def main():
    print("# RR-DR60 limiter reference model (spec 005). S-002 values; transistor parameters estimated.\n")
    print("## Static curve (1 kHz, 1 s tones, last 0.1 s; gain re unity, output peak in dBFS)")
    knee = None
    for db in (-20, -3, -1, 0, 1, 2, 3, 4, 6, 8, 12, 18, 20):
        o = simulate([db], [1.0])
        g = sum(gg for tt, gg, _ in o if tt >= 0.9) / sum(1 for tt, _, _ in o if tt >= 0.9)
        print(f"  in {db:+3d} dBFS -> gain {g:7.2f} dB, out peak {db + g:+6.2f} dBFS, Vc {o[-1][2]:.3f} V")
    lo, hi = -2.0, 6.0
    for _ in range(14):
        mid = 0.5 * (lo + hi)
        o = simulate([mid], [1.0])
        g = sum(gg for tt, gg, _ in o if tt >= 0.9) / sum(1 for tt, _, _ in o if tt >= 0.9)
        lo, hi = (lo, mid) if g < -1.0 else (mid, hi)
    print(f"  knee (steady gain = -1 dB): {0.5 * (lo + hi):+.2f} dBFS\n")

    print("## Attack: dead time after 20 s below the knee (modeled as Vc = 0), step from -20 dBFS")
    for b6 in (150.0, 300.0, 600.0):
        for step in (2.0, 5.0, 10.0, 18.0):
            o = simulate([-20.0, step, -20.0], [0.05, 1.0, 0.1], beta6=b6)
            dt_ms = dead_time_ms(o, 0.05)
            gmin = min(g for tt, g, _ in o if 0.05 <= tt < 1.05)
            gst = sum(g for tt, g, _ in o if 1.0 <= tt < 1.05) / sum(1 for tt, _, _ in o if 1.0 <= tt < 1.05)
            print(f"  beta6 {b6:3.0f}, +{step:4.1f} dBFS: dead time {dt_ms:7.2f} ms, min gain {gmin:6.2f} dB, steady {gst:6.2f} dB")
    print("  armed (Vc0 = 1.0 V, as 1-10 s after a loud sound), +10 dBFS:",
          f"dead time {dead_time_ms(simulate([-20.0, 10.0], [0.05, 0.2], vc0=1.0), 0.05):.2f} ms\n")

    print("## Release: 1 s burst, then -20 dBFS; gain re unity at t after the burst")
    for burst in (11.0, 18.0):
        o = simulate([burst, -20.0], [1.0, 3.0])
        rel = next((tt - 1.0 for tt, g, _ in o if tt >= 1.0 and g > -1.0), None)
        print(f"  burst {burst:+.0f} dBFS (attenuation {-at(o, 0.999):.1f} dB): to -1 dB in {rel:.2f} s;",
              " ".join(f"{t:.2f}s {at(o, 1.0 + t):6.2f} dB" for t in (0.05, 0.1, 0.2, 0.3, 0.5, 0.75, 1.0, 1.5)))
    print(f"  arming decay R11*C6 = {R11 * C6:.0f} s: Vc 1.1 -> 0.5 V takes {R11 * C6 * math.log(1.1 / 0.5):.1f} s\n")

    print("## Polarity (A-036): asymmetric stimulus sin + 0.5 cos(2w), peaks +1.125 / -1.5 (2.5 dB)")
    for sign in (1.0, -1.0):
        amp = 10 ** (4.0 / 20.0) / 1.5 * V_FS  # the larger peak at +4 dBFS
        dt = 1.0 / 192_000.0
        vc = 0.0
        gains = []
        for n in range(int(2.0 * 192_000)):
            t = n * dt
            x = sign * amp * (math.sin(2 * math.pi * 1000 * t) + 0.5 * math.cos(4 * math.pi * 1000 * t))
            a = gain_of_vc(vc)
            gsx = max(-V_RAIL, min(V_RAIL, x * a))
            over = KDIV * (-gsx) - VBE6
            ic6 = min(BETA6 * over / RTH, (V_REF - vc) / R12) if over > 0 else 0.0
            vc = max(0.0, vc + (ic6 - vc / R11 - ib4_of_vc(vc) / BETA5) * dt / C6)
            if t >= 1.9:
                gains.append(20 * math.log10(a))
        print(f"  sign {sign:+.0f}: steady gain {sum(gains) / len(gains):6.2f} dB")
    print()


if __name__ == "__main__":
    main()
