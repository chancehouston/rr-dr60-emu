# /// script
# requires-python = ">=3.11"
# dependencies = ["numpy"]
# ///
"""Reference design of spec 013's playback speed control (A-054, mechanism (b)): prints every
design figure the core and harness tests pin.

The speed control resamples the stored 6 kHz device stream at the device rate. Level k/5
(k = 3, 4, 5, 6, 7) plays at speed r = k/5: device output sample n sits at place p_n = n·k/5
stored samples (kept exactly as an integer count of fifths, research R-02). The value at a place
p = i + ph/5 (i whole, ph = 0 … 4) is

    y = sum_{m=0}^{2D} c_k[ph][m] · x[i + D − m],     x[j] = 0 outside 0 … K−1,

where c_k[ph][m] = h_k[ph + 5m] is phase ph of a Kaiser-windowed sinc prototype h_k of 10·D + 1
taps at the 30 kHz prototype rate (5 × 6 kHz), centred on tap 5·D, and each phase is normalized
to sum to exactly 1 (`resample::design::polyphase_split`). The response is centred on the place,
so the speed control adds no latency (FR-003, FR-008). Level 3 (k = 5) passes x[i] unaltered.

Per level the prototype's band edges, in the stored stream's frequencies (research R-03):
  r < 1:  passband 2250 Hz, stopband 3100 Hz (the image 6000 − f of a stored 2900 Hz tone);
  r > 1:  passband 2250 / r, stopband 3000 / r (the device Nyquist frequency after the move),
the cutoff halfway between, 70 dB of design attenuation (FR-004's 60 dB with margin).

Usage: uv run tools/playback-speed/speed_design.py [--check]
"""

import re
import sys
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[2]
FS = 6000.0  # A-026: the device rate
PHASES = 5  # fifths of a stored sample (research R-02)
PROTO_RATE = FS * PHASES  # 30 kHz
D = 28  # half-length in stored samples: the look-ahead (research R-03)
TAPS = 10 * D + 1  # 281
PER_PHASE = 2 * D + 1  # 57
ATTEN_DB = 70.0  # design attenuation (FR-004: 60 dB, with margin)
LEVELS = (3, 4, 5, 6, 7)  # k: r = k / 5 (FR-002)
FLOOR_DB = -60.0  # FR-004 (b), (c)
PASS_DB = 0.1  # FR-004 (a)


def kaiser_beta(a: float) -> float:
    """`resample::design::kaiser_beta` (> 50 dB form)."""
    return 0.1102 * (a - 8.7)


def edges(k: int) -> tuple[float, float]:
    r = k / 5
    return (2250.0, 3100.0) if k < 5 else (2250.0 / r, 3000.0 / r)


def prototype(k: int) -> np.ndarray:
    """`design::kaiser_lowpass(TAPS, fc / PROTO_RATE, kaiser_beta(ATTEN_DB))`."""
    fp, fs = edges(k)
    cutoff = (fp + fs) / 2 / PROTO_RATE
    centre = (TAPS - 1) / 2
    t = np.arange(TAPS) - centre
    x = 2 * cutoff * t
    sinc = np.where(t == 0, 1.0, np.sin(np.pi * x) / np.where(t == 0, 1.0, np.pi * x))
    beta = kaiser_beta(ATTEN_DB)
    w = np.i0(beta * np.sqrt(1 - (t / centre) ** 2)) / np.i0(beta)
    return 2 * cutoff * sinc * w


def bank(k: int) -> np.ndarray:
    """`design::polyphase_split(h, 5)`: phase p is h[p], h[p + 5], …, each summing to 1."""
    h = prototype(k)
    out = np.zeros((PHASES, PER_PHASE))
    for p in range(PHASES):
        b = h[p::PHASES]
        out[p, : len(b)] = b
        out[p] /= out[p].sum()
    return out


BANKS = {k: bank(k) for k in LEVELS if k != 5}


def play(x: np.ndarray, k: int, with_tail: bool = False) -> np.ndarray:
    """The device outputs of a constant-level playback of stored samples x: the recording's own
    part (places before K), and with `with_tail` the tail (places whose kernel still reaches a
    stored sample, research R-04)."""
    K = len(x)
    n_own = -(-5 * K // k)  # ceil(5K / k) = ceil(K / r)
    n = n_own + (tail_len(k) if with_tail else 0)
    p5 = np.arange(n) * k
    i, ph = p5 // 5, p5 % 5
    if k == 5:
        return np.where(i < K, x[np.minimum(i, K - 1)], 0.0)
    idx = i[:, None] + D - np.arange(PER_PHASE)[None, :]
    xs = np.where((idx >= 0) & (idx < K), x[np.clip(idx, 0, K - 1)], 0.0)
    return np.sum(BANKS[k][ph] * xs, axis=1)


def tail_len(k: int) -> int:
    """Device outputs after the recording's own part whose kernel still reaches the last stored
    sample K − 1, worst case over K mod k: the place p = i + ph/5 reaches x[i + D − 2D] for
    ph = 0 and x[i − D + 1] for ph > 0 (phase ph's last tap is zero-padded)."""
    if k == 5:
        return 0
    worst = 0
    for K in range(1000, 1000 + 5 * k):
        n_own = -(-5 * K // k)
        n = n_own
        while True:
            i, ph = divmod(n * k, 5)
            lowest = i - D if ph == 0 else i - D + 1
            if lowest > K - 1:
                break
            n += 1
        worst = max(worst, n - n_own)
    return worst


def fit(y: np.ndarray, f: float):
    t = np.arange(len(y))
    a = np.stack([np.cos(2 * np.pi * f / FS * t), np.sin(2 * np.pi * f / FS * t)], 1)
    c, *_ = np.linalg.lstsq(a, y, rcond=None)
    return float(np.hypot(*c)), y - a @ c


def peak_db(res: np.ndarray, lo: float = 20.0, hi: float = 3000.0) -> float:
    """The largest spectral component of `res` between lo and hi Hz, dB re a unit sine (a
    Kaiser-windowed FFT, β 20, normalized to the sine's peak)."""
    w = np.kaiser(len(res), 20)
    spec = np.abs(np.fft.rfft(res * w)) / w.sum() * 2
    f = np.arange(len(spec)) * FS / len(res)
    m = (f > lo) & (f < hi)
    return float(20 * np.log10(spec[m].max() + 1e-300))


def response_db(k: int, f: float) -> float:
    """The speed control's level response to a stored tone at f (fit at r × f), dB."""
    x = np.sin(2 * np.pi * f / FS * np.arange(9000))
    y = play(x, k)[900:-900]
    a, _ = fit(y, k / 5 * f)
    return 20 * np.log10(a + 1e-300)


def stage10_db(f: float) -> float:
    """Stage 10's response at f from the committed PLAYBACK_SOS (`voiceband_coeffs.rs`)."""
    src = (REPO / "crates/rr_dr60/src/stages/voiceband_coeffs.rs").read_text()
    block = src[src.index("PLAYBACK_SOS") :]
    block = block[: block.index("];\n")]
    vals = [np.frombuffer(bytes.fromhex(h)[::-1], dtype="<f8")[0]
            for h in re.findall(r"from_bits\(0x([0-9a-f]{16})\)", block)]
    z = np.exp(-1j * 2 * np.pi * f / FS)
    hz = 1.0
    for b0, b1, b2, a1, a2 in np.array(vals).reshape(-1, 5):
        hz *= (b0 + b1 * z + b2 * z * z) / (1 + a1 * z + a2 * z * z)
    return float(20 * np.log10(abs(hz)))


def main() -> int:
    check = "--check" in sys.argv
    failures = []

    def bound(ok: bool, what: str):
        if not ok:
            failures.append(what)

    print(f"prototype: {TAPS} taps at {PROTO_RATE:.0f} Hz, {PER_PHASE} per phase, look-ahead D = {D}"
          f" stored samples ({1000 * D / FS:.2f} ms), Kaiser beta {kaiser_beta(ATTEN_DB):.4f}"
          f" ({ATTEN_DB:.0f} dB)")
    print(f"cost: {PER_PHASE} multiply-adds per device output sample at levels 1, 2, 4, 5"
          f" ({PER_PHASE / 8:.1f} per host sample at 48 kHz at level-3-equivalent density)")
    print()
    print("level  k  r    pass Hz  stop Hz  cutoff Hz  passband dev dB  worst spurious dB (f)   tail")
    figures = {}
    for k in (3, 4, 6, 7):
        r = k / 5
        fp, fs = edges(k)
        f_top = min(2250.0, 2250.0 / r)
        dev = 0.0
        res_worst = -999.0
        for f in np.arange(100.0, f_top + 1e-9, 50.0) + 3.0:
            x = np.sin(2 * np.pi * f / FS * np.arange(9000))
            y = play(x, k)[900:-900]
            a, res = fit(y, r * f)
            dev = max(dev, abs(20 * np.log10(a)))
            res_worst = max(res_worst, peak_db(res))
        spur, spur_f = -999.0, 0.0
        for f in np.arange(100.0, 2901.0, 25.0) + 3.0:
            x = np.sin(2 * np.pi * f / FS * np.arange(9000))
            y = play(x, k)[900:-900]
            if r * f < 2990.0:
                _, res = fit(y, r * f)
            else:
                res = y  # FR-004 (c): the moved tone is above Nyquist; nothing may remain
            s = peak_db(res)
            if s > spur:
                spur, spur_f = s, f
        tl = tail_len(k)
        figures[k] = (dev, spur, res_worst, tl)
        print(f"  {LEVELS.index(k) + 1}    {k}  {r:.1f}  {fp:7.1f}  {fs:7.1f}  {(fp + fs) / 2:9.1f}"
              f"  {dev:15.4f}  {spur:8.1f} ({spur_f:6.0f} Hz)     {tl}")
        bound(dev <= PASS_DB, f"level k={k}: passband deviation {dev:.4f} dB > {PASS_DB}")
        bound(res_worst <= FLOOR_DB, f"level k={k}: passband residual {res_worst:.1f} dB > {FLOOR_DB}")
        bound(spur <= FLOOR_DB, f"level k={k}: spurious {spur:.1f} dB > {FLOOR_DB}")
        bound(tl <= 50, f"level k={k}: tail {tl} > 50 device samples")
    print()

    # FR-008: an impulse at stored sample j peaks at device output round(j / r), symmetric.
    print("impulse (FR-008): stored sample j = 600 at each level")
    for k in (3, 4, 6, 7):
        r = k / 5
        x = np.zeros(1200)
        x[600] = 1.0
        y = play(x, k, with_tail=True)
        n_pk = int(np.argmax(np.abs(y)))
        want = round(600 / r)
        # symmetric about the peak's place when the peak sits on a whole place (5 · 600 / k)
        sym = "exact place" if (5 * 600) % k == 0 else "between outputs"
        print(f"  k={k}: peak at output {n_pk} (round(j/r) = {want}), {sym}")
        bound(n_pk == want, f"impulse k={k}: peak {n_pk} != {want}")
        if (5 * 600) % k == 0:
            w = 40
            seg = y[n_pk - w : n_pk + w + 1]
            asym = float(np.max(np.abs(seg - seg[::-1])))
            print(f"        symmetry error over ±{w} outputs {asym:.2e}")
            bound(asym <= 1e-12, f"impulse k={k}: asymmetric {asym:.2e}")
    print()

    # FR-005: durations; the recording's own part is ceil(K / r) device samples.
    print("duration (FR-005): the recording's own part, ceil(5K / k) device samples")
    for K in (1, 120, 6000, 21_600_000):
        row = ", ".join(f"k={k}: {-(-5 * K // k)}" for k in LEVELS)
        print(f"  K = {K}: {row}")
    print()

    # FR-007: the switch into level 3 waits until the place is whole: at most 4 outputs.
    print("switch into level 3 (FR-007): outputs at the old ratio until the place is whole")
    for k in (3, 4, 6, 7):
        # from any phase ph, the number of k-steps until (ph + n·k) ≡ 0 (mod 5)
        worst = max(next(n for n in range(5) if (ph + n * k) % 5 == 0) for ph in range(5))
        print(f"  k={k}: at most {worst} outputs")
        bound(worst <= 4, f"switch k={k}: {worst} > 4")
    print()

    # A-054's capture P1 figures under (b): a stored 2.3 kHz tone at levels 4 and 5.
    print("A-054 capture P1 under (b): a stored 2300 Hz tone")
    for k in (6, 7):
        r = k / 5
        sp = response_db(k, 2300.0) if r * 2300 < 3000 else float("nan")
        s10 = stage10_db(r * 2300.0) if r * 2300 < 3000 else float("nan")
        if r * 2300 < 3000:
            print(f"  level {LEVELS.index(k) + 1}: plays at {r * 2300:.0f} Hz: speed control {sp:+.1f} dB,"
                  f" stage 10 {s10:+.1f} dB, total {sp + s10:+.1f} dB")
        else:
            x = np.sin(2 * np.pi * 2300.0 / FS * np.arange(9000))
            y = play(x, k)[900:-900]
            print(f"  level {LEVELS.index(k) + 1}: moved to {r * 2300:.0f} Hz (above Nyquist): largest residue"
                  f" {peak_db(y):+.1f} dB")
    print()

    # FR-007: the step a change of level makes (reported): a stored 1 kHz tone, the level
    # changing at output 3000, the output against the two constant-level references.
    print("level change (FR-007, reported): the band each level passes, stored-stream frequencies")
    for k in (3, 4, 6, 7):
        print(f"  k={k}: -0.1 dB edge {min(2250.0, 2250.0 / (k / 5)):.0f} Hz or above,"
              f" -60 dB from {edges(k)[1]:.0f} Hz")

    if check:
        if failures:
            print("\nCHECK FAILED:")
            for f in failures:
                print("  " + f)
            return 1
        print("\ncheck: all bounds hold")
    return 0


if __name__ == "__main__":
    sys.exit(main())
