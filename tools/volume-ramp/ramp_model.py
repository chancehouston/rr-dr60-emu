# /// script
# requires-python = ">=3.11"
# dependencies = ["numpy"]
# ///
"""Reference model of spec 011's volume transition (A-045): prints every design figure the
core and harness tests pin.

The transition moves the knob position p_s -> p_t over samples k = 1 ... N (N = 0.02 x rate,
FR-002) along the smoothstep s(u) = 3u^2 - 2u^3, the gain at each sample being the loaded law
(A-044) at that position, the last sample the 0.10 scalar at p_t (FR-001). The click energy
(FR-004) is the energy in a Hann window of round(0.5 x rate) samples centred on the
transition's midpoint (on the step for the reference), between 20 and 2750 Hz and more than
250 Hz from 1 kHz, for a 1 kHz cosine peaking at the change's first sample; the reduction is
10 log10(E_step / E_transition). The model applies the gain to the cosine directly: at the tap
after stage 11 the coupling corners precede the gain, so the tone's amplitude and phase change
both runs alike and the ratio is unchanged (research R-02).

Usage: uv run tools/volume-ramp/ramp_model.py [--check]
       uv run tools/volume-ramp/ramp_model.py --t1090 MS [--from P] [--to P]

`--t1090 MS` is A-045's knob-flick comparison: for a measured 10-90 % amplitude time it finds,
per rate and route, the smoothstep length whose gain sequence has that 10-90 % time (between
positions P, default FR-004's 0.1 -> 1), and prints that length, the click level in FR-004's
window and band relative to the steady tone at the louder end (the figure a capture reads as the
band energy during the motion in excess of a steady segment's, over the loud segment's total), and
the reduction against the instantaneous step. The default output and `--check` are unchanged.
"""

import sys

import numpy as np

RATES = [6000, 8000, 16000, 44100, 48000, 88200, 96000]
LENGTH_S = 0.02  # A-045
# A-044's constants (crates/rr_dr60/src/stages/output_coeffs.rs).
R_FEEDBACK, R_INPUT, R_POT, R_EAR_SERIES, Z_EAR = 68e3, 10e3, 50e3, 10e3, 6400.0


def g_spk(p):
    return p * (2.0 * R_FEEDBACK) / (R_INPUT + R_POT * p * (1.0 - p))


def g_ear(p):
    z = R_INPUT + R_EAR_SERIES + Z_EAR
    return p * z / (z + R_POT * p * (1.0 - p))


LAWS = {"speaker": g_spk, "earphone": g_ear}


def smoothstep(u):
    return u * u * (3.0 - 2.0 * u)


def linear(u):
    return u


def n_of(rate):
    assert rate % 50 == 0, rate  # 20 ms is a whole number of samples at every supported rate
    return rate * 20 // 1000  # FR-002, integer arithmetic as the core


def gains(law, shape, p_s, p_t, n):
    """The transition's gains at k = 1 ... n as the core computes them: the law at the knob's
    position, held within the two ends and against the previous sample (research R-03's range
    and running hold), the last one the target scalar exactly."""
    g_s, g_t = law(p_s), law(p_t)
    up = g_t > g_s
    out = np.empty(n)
    prev = g_s
    for k in range(1, n + 1):
        if k == n:
            g = g_t
        else:
            g = law(p_s + (p_t - p_s) * shape(k * (1.0 / n)))
            g = min(max(g, prev), g_t) if up else max(min(g, prev), g_t)
        out[k - 1] = g
        prev = g
    return out


def raw_excursions(law, shape, p_s, p_t, n):
    """The unbounded formula in f64: (samples outside the two ends' range, reversals)."""
    k = np.arange(1, n + 1, dtype=np.float64)
    g = law(p_s + (p_t - p_s) * shape(k / n))
    lo, hi = sorted((law(p_s), law(p_t)))
    d = np.diff(np.concatenate(([law(p_s)], g))) * (1.0 if p_t > p_s else -1.0)
    return int(((g < lo) | (g > hi)).sum()), int((d < 0).sum())


def band_energy(y, rate):
    w = np.hanning(len(y))
    f = np.fft.rfftfreq(len(y), 1.0 / rate)
    m = (f >= 20.0) & (f <= 2750.0) & (np.abs(f - 1000.0) > 250.0)
    return float((np.abs(np.fft.rfft(y * w)) ** 2)[m].sum())


def click_reduction(rate, law, shape, p_s, p_t, n=None, level=False):
    """FR-004's reduction (dB) for a transition of n samples (default the engine's N); with
    `level`, also the click level (dB) relative to the steady tone at the louder end in the same
    window (a transition to position 0 ends in silence)."""
    n = n_of(rate) if n is None else n
    win = round(0.5 * rate)
    pad = win  # a settled tone on both sides
    total = 2 * pad + n
    n0 = pad  # index of transition sample 1
    t = (np.arange(total) - n0) / rate
    x = np.cos(2 * np.pi * 1000.0 * t)  # a positive peak at the change's first sample

    step = np.full(total, law(p_s))
    step[n0:] = law(p_t)
    ramp = step.copy()
    ramp[n0 : n0 + n] = gains(law, shape, p_s, p_t, n)

    def windowed(y, centre):
        a = int(round(centre - win / 2))
        return y[a : a + win]

    e_step = band_energy(windowed(x * step, n0), rate)
    e_ramp = band_energy(windowed(x * ramp, n0 + (n - 1) / 2), rate)
    reduction = 10 * np.log10(e_step / e_ramp)
    if not level:
        return reduction
    steady = windowed(x * max(law(p_s), law(p_t)), n0) * np.hanning(win)
    return reduction, 10 * np.log10(e_ramp / float((np.abs(np.fft.rfft(steady)) ** 2).sum()))


def t1090(law, shape, p_s, p_t, n, rate):
    """The 10-90 % amplitude time (s) of an n-sample transition: from the first sample whose
    gain is 10 % of the way from the old gain to the new to the first that is 90 % of the way."""
    g = gains(law, shape, p_s, p_t, n)
    a = (g - law(p_s)) / (law(p_t) - law(p_s))
    return (int(np.argmax(a >= 0.9)) - int(np.argmax(a >= 0.1))) / rate


def n_for_t1090(law, shape, p_s, p_t, rate, target_s):
    """The transition length (samples) whose 10-90 % time is nearest target_s: the smallest n
    reaching it by bisection (the time grows with n), or the one before it if that is nearer."""
    lo, hi = 2, 2
    while t1090(law, shape, p_s, p_t, hi, rate) < target_s:
        lo, hi = hi, hi * 2
    while hi - lo > 1:
        mid = (lo + hi) // 2
        lo, hi = (mid, hi) if t1090(law, shape, p_s, p_t, mid, rate) < target_s else (lo, mid)
    err = lambda n: abs(t1090(law, shape, p_s, p_t, n, rate) - target_s)
    return min((lo, hi), key=err)


def flick(t1090_ms, p_s, p_t):
    """A-045's knob-flick comparison (see the module docstring)."""
    assert 0.0 <= p_s <= 1.0 and 0.0 <= p_t <= 1.0 and p_s != p_t, (p_s, p_t)
    print(f"A-045 knob-flick comparison: a smoothstep with a {t1090_ms:g} ms 10-90 % amplitude time, {p_s:g} -> {p_t:g}\n")
    print("The engine's own 20 ms transition, for reference (10-90 % time in ms):")
    for route, law in LAWS.items():
        own = [t1090(law, smoothstep, p_s, p_t, n_of(r), r) * 1e3 for r in RATES]
        print(f"  {route:>9}: " + ", ".join(f"{t:.2f}" for t in own))
        # Round trip: the length found for the engine's own 10-90 % time has that time (lengths
        # a sample or two apart can share it, the time being counted in whole samples).
        for r, t in zip(RATES, own):
            n = n_for_t1090(law, smoothstep, p_s, p_t, r, t / 1e3)
            assert t1090(law, smoothstep, p_s, p_t, n, r) * 1e3 == t and abs(n - n_of(r)) <= 0.001 * r, (r, n)
    print(f"\n{'rate':>6} {'route':>9} {'N':>7} {'length ms':>9} {'10-90 ms':>9} {'click dB':>9} {'vs step':>8}")
    for rate in RATES:
        for route, law in LAWS.items():
            n = n_for_t1090(law, smoothstep, p_s, p_t, rate, t1090_ms / 1e3)
            red, lvl = click_reduction(rate, law, smoothstep, p_s, p_t, n=n, level=True)
            t = t1090(law, smoothstep, p_s, p_t, n, rate) * 1e3
            print(f"{rate:>6} {route:>9} {n:>7} {n / rate * 1e3:9.2f} {t:9.3f} {lvl:9.2f} {red:8.2f}")
    if max(n_for_t1090(law, smoothstep, p_s, p_t, r, t1090_ms / 1e3) / r for law in LAWS.values() for r in RATES) > 0.5:
        print("\nnote: the movement is longer than FR-004's 0.5 s window, so the window holds only part of it")
    print("\nclick dB: the energy in FR-004's window (Hann, round(0.5 x rate), centred on the movement's")
    print("midpoint) and band (20-2750 Hz, more than 250 Hz from 1 kHz), relative to the whole energy of")
    print("the steady tone at the louder end in the same window; vs step: FR-004's reduction against the")
    print("instantaneous step. A capture's figure is its motion window's band energy less a steady")
    print("segment's (the codec's floor), over the loud steady segment's whole energy.")


def flag(name, default):
    if name not in sys.argv:
        return default
    return float(sys.argv[sys.argv.index(name) + 1])


def drag(rate, law, shape):
    """Ten updates G = 16 round(rate / 960) samples apart (16.0 - 17.0 ms, on 16-sample block
    boundaries) from 0.1 to 1 (US1 AS8, the golden's `ramp_drag`); each update restarts a
    transition from the knob's current position. Returns the reduction against the stepped
    drag and whether the gain is monotonic."""
    n = n_of(rate)
    gap = 16 * round(rate / 960.0)
    win = round(0.5 * rate)
    targets = [0.1 + 0.09 * j for j in range(1, 11)]
    pad = win
    total = 2 * pad + 10 * gap + n
    n0 = pad
    t = (np.arange(total) - n0) / rate
    x = np.cos(2 * np.pi * 1000.0 * t)
    knob = np.full(total, 0.1)
    stepped = np.full(total, 0.1)
    p_cur = 0.1
    for j, p_t in enumerate(targets):
        a = n0 + j * gap
        b = a + gap if j < 9 else total
        stepped[a:] = p_t
        p_s = p_cur
        k = np.arange(1, b - a + 1, dtype=np.float64)
        u = np.minimum(k / n, 1.0)
        seg = p_s + (p_t - p_s) * shape(u)
        knob[a:b] = seg
        p_cur = seg[-1]
    g = law(knob)
    gs = law(stepped)
    mono = bool(np.all(np.diff(g) >= 0))
    centre = n0 + (9 * gap + n) / 2

    def windowed(y):
        a = int(round(centre - win / 2))
        return y[a : a + win]

    red = 10 * np.log10(band_energy(windowed(x * gs), rate) / band_energy(windowed(x * g), rate))
    return red, mono


def main():
    if "--t1090" in sys.argv:
        flick(flag("--t1090", None), flag("--from", 0.1), flag("--to", 1.0))
        return
    check = "--check" in sys.argv
    ok = True
    print("Spec 011 volume transition (A-045): smoothstep in knob position, 20 ms\n")
    print("N (transition samples) per rate:", {r: n_of(r) for r in RATES})

    print("\nFR-004 click reduction (dB) against the instantaneous step, 0.1 <-> 1:")
    print(f"{'rate':>6} {'route':>9} {'up':>7} {'down':>7} {'up_lin':>7}")
    for rate in RATES:
        for route, law in LAWS.items():
            up = click_reduction(rate, law, smoothstep, 0.1, 1.0)
            down = click_reduction(rate, law, smoothstep, 1.0, 0.1)
            lin = click_reduction(rate, law, linear, 0.1, 1.0)
            print(f"{rate:>6} {route:>9} {up:7.2f} {down:7.2f} {lin:7.2f}")
            ok &= min(up, down) >= 30.0  # FR-004's floor
    print("\nUS1 AS8 drag (ten updates 16 round(rate/960) samples apart, 0.1 -> 1), reduction against the stepped drag (dB):")
    for rate in RATES:
        row = []
        for route, law in LAWS.items():
            red, mono = drag(rate, law, smoothstep)
            row.append(f"{route} {red:6.2f} {'monotonic' if mono else 'NOT MONOTONIC'}")
            ok &= red >= 30.0 and mono
        print(f"{rate:>6} " + " | ".join(row))

    print("\nFR-003 in f64: the unbounded formula on a grid of 21 x 21 positions, 7 rates, 2 routes:")
    grid = np.linspace(0.0, 1.0, 21)
    pairs = outside = reversals = 0
    for rate in RATES:
        n = n_of(rate)
        for law in LAWS.values():
            for p_s in grid:
                for p_t in grid:
                    if p_s == p_t:
                        continue
                    o, r = raw_excursions(law, smoothstep, p_s, p_t, n)
                    pairs += o > 0
                    outside += o
                    reversals += r
    print(f"  pairs with a sample outside the ends' range: {pairs} of {7 * 2 * 420} ({outside} samples)")
    print(f"  reversals: {reversals} (the range bound alone keeps the sequence monotonic)")
    ok &= reversals == 0
    print("\nSC-003 retarget (0.1 -> 1, then -> 0.4 after floor(N/2) samples): largest per-sample")
    print("increment of the second transition vs the larger of the two transitions' own largest (speaker law):")
    for rate in RATES:
        n = n_of(rate)
        half = (n // 2) // 16 * 16
        g1 = gains(g_spk, smoothstep, 0.1, 1.0, n)
        k = np.arange(1, half + 1, dtype=np.float64)
        knob_at = 0.1 + 0.9 * smoothstep(half * (1.0 / n))
        g2 = gains(g_spk, smoothstep, knob_at, 0.4, n)
        seq = np.concatenate(([g_spk(0.1)], g1[:half], g2))
        inc_retarget = np.abs(np.diff(seq[half:])).max()
        own = max(np.abs(np.diff(np.concatenate(([g_spk(0.1)], g1)))).max(),
                  np.abs(np.diff(np.concatenate(([g_spk(knob_at)], g2)))).max())
        print(f"  {rate:>6}: at the retarget {inc_retarget:.6g}, own largest {own:.6g}")
        ok &= inc_retarget <= own
    print("\nR-03 running hold: retargets to within 1e-12 of the knob (N = 1920, 600 cases each):")
    rng = np.random.default_rng(11)
    for scale in (1e-10, 1e-11, 1e-12):
        rev_raw = 0
        for _ in range(600):
            pt = float(np.float32(rng.uniform(0.05, 0.95)))  # a host position (f32)
            ps = pt + rng.uniform(-1, 1) * scale  # the moving knob, an f64 near it
            if pt == ps:
                continue
            k = np.arange(1, 1921, dtype=np.float64)
            g = g_spk(ps + (pt - ps) * smoothstep(k * (1.0 / 1920)))
            g = np.clip(g, *sorted((g_spk(ps), g_spk(pt))))
            d = np.diff(np.concatenate(([g_spk(ps)], g))) * (1.0 if pt > ps else -1.0)
            rev_raw += bool((d < 0).any())
            gh = gains(g_spk, smoothstep, ps, pt, 1920)
            dh = np.diff(np.concatenate(([g_spk(ps)], gh))) * (1.0 if pt > ps else -1.0)
            ok &= not (dh < 0).any()
        print(f"  |to - from| ~ {scale:g}: range hold alone reverses in {rev_raw} of 600; with the running hold 0")
    print("\nN_OUT / N (the output chain's share of the drain exceeds a transition, FR-005):")
    n_out = [346, 461, 922, 2539, 2764, 5078, 5527]
    ratios = [o / n_of(r) for o, r in zip(n_out, RATES)]
    print("  " + ", ".join(f"{r}: {q:.4f}" for r, q in zip(RATES, ratios)))
    ok &= all(100 * o > 287 * n_of(r) for o, r in zip(n_out, RATES))
    if check:
        print("\ncheck:", "PASS" if ok else "FAIL")
        sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
