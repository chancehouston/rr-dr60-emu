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
    n = round(LENGTH_S * rate)
    assert n == LENGTH_S * rate, rate  # an integer at every supported rate (FR-002)
    return n


def gains(law, shape, p_s, p_t, n):
    """The transition's gains at k = 1 ... n, held within the two ends and monotonic
    (research R-01's running bound), the last one the target scalar exactly."""
    k = np.arange(1, n + 1, dtype=np.float64)
    p = p_s + (p_t - p_s) * shape(k / n)
    g = law(p)
    lo, hi = sorted((law(p_s), law(p_t)))
    g = np.clip(g, lo, hi)
    g[-1] = law(p_t)
    return g


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


def click_reduction(rate, law, shape, p_s, p_t):
    n = n_of(rate)
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
    return 10 * np.log10(e_step / e_ramp)


def drag(rate, law, shape):
    """Ten updates 1/60 s apart from 0.1 to 1 (US1 AS8); each update restarts a transition from
    the knob's current position. Returns the reduction against the stepped drag, the largest
    per-sample increment, and whether the gain is monotonic."""
    n = n_of(rate)
    gap = round(rate / 60.0)
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
    print("\nUS1 AS8 drag (ten 1/60 s updates 0.1 -> 1), reduction against the stepped drag (dB):")
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
    print("\nN_OUT / N (the drain completes a transition, FR-005):")
    n_out = [346, 461, 922, 2539, 2764, 5078, 5527]
    ratios = [o / n_of(r) for o, r in zip(n_out, RATES)]
    print("  " + ", ".join(f"{r}: {q:.4f}" for r, q in zip(RATES, ratios)))
    ok &= min(ratios) > 2.87
    if check:
        print("\ncheck:", "PASS" if ok else "FAIL")
        sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
