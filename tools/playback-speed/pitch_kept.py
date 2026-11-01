"""Spec 023: the pitch-kept playback speed option's design (research R-02 – R-06, R-09, R-12).

The time-scaler exactly as the plan states it, lifted from the plan review's simulation
(hardware B1 – B8): the place in fifths; a hop grid anchored at `origin`; the nominal analysis
position a = floor(p5 / 5) at a hop's first output; the search over s = 0, -1, +1, …, -Δ, +Δ with
a strict comparison, c(s) = Σ_{n<Lc} x[a + s + n] · x[prev_a + prev_s + Ss + n], the sums in the
Rust's four-partial-sum order; x() = 0 outside [0, K); the window w[0 … 59] a periodic Hann and
w[60 + n] = 1 − w[n] exactly; the exact-continuation shortcut; lazy pulls. Every figure printed
here is a pin for the Rust (`crates/rr_dr60/src/stages/speed.rs`) and the harness
(`crates/rr_dr60_harness/src/pitch_checks.rs`); `--write` regenerates `speed_window.rs`.
An engine convention, not a device behavior (A-054).
"""
from __future__ import annotations

import struct
import sys
from pathlib import Path

import numpy as np

FS = 6000.0  # A-026
NUM = {1: 3, 2: 4, 3: 5, 4: 6, 5: 7}  # A-054 (013 FR-002)
N, SS, DELTA, LC = 120, 60, 40, 60  # engineering targets (023 R-02, R-03; Δ < Ss: R-06's proof)
ORDER = [0] + [v for d in range(1, DELTA + 1) for v in (-d, d)]
FADE_N = 120  # 019 FR-002
OFFGRID_HZ = 1234.5
AMP = 0.5  # −6 dBFS


def window() -> np.ndarray:
    """w[0 … 59] = 0.5 − 0.5·cos(2πn/120); w[60 + n] = 1 − w[n] exactly (hardware B1)."""
    w = np.zeros(N)
    w[:SS] = 0.5 - 0.5 * np.cos(2 * np.pi * np.arange(SS) / N)
    w[SS:] = 1.0 - w[:SS]
    return w


W = window()


def fade_weights(n: int = FADE_N) -> np.ndarray:
    u = np.arange(1, n + 1) / n
    return u * u * (3.0 - 2.0 * u)


def fnv1a64_i32(values) -> int:
    h = 0xCBF29CE484222325
    for v in values:
        for b in struct.pack("<i", int(v)):
            h ^= b
            h = (h * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def dot4(a: np.ndarray, b: np.ndarray) -> float:
    """The Rust `dot` order: four partial sums over m ≡ q (mod 4), ascending, then (s0+s1)+(s2+s3)."""
    s = [0.0, 0.0, 0.0, 0.0]
    for m in range(len(a)):
        s[m & 3] += float(a[m]) * float(b[m])
    return (s[0] + s[1]) + (s[2] + s[3])


class Scaler:
    """The time-scaler on a stored stream `x` (f64 values of the f32 stored samples)."""

    def __init__(self, x, level: int, mutant: str | None = None, origin_out: int = 0):
        self.x = np.asarray(x, float)
        self.K = len(self.x)
        self.w = W.copy()
        self.delta = DELTA
        self.mutant = mutant
        if mutant == "window_sum_off":  # a symmetric Hann: the sum is not 1
            self.w = 0.5 - 0.5 * np.cos(2 * np.pi * np.arange(N) / (N - 1))
        if mutant == "delta_narrow":
            self.delta = 20
        self.order = [0] + [v for d in range(1, self.delta + 1) for v in (-d, d)]
        if mutant == "tie_break_outward_off":
            self.order = list(range(-self.delta, self.delta + 1))
        self.k = NUM[level]
        self.p5 = 0
        self.o = 0
        self.origin = None
        self.shifts: list[int] = []
        self.pending: list[tuple[int, int]] = []  # (output index, level)
        self.pulled = 0
        self.span = 0

    def xs(self, j: int) -> float:
        return float(self.x[j]) if 0 <= j < self.K else 0.0

    def seg(self, start: int, n: int) -> np.ndarray:
        idx = start + np.arange(n)
        ok = (idx >= 0) & (idx < self.K)
        return np.where(ok, self.x[np.clip(idx, 0, max(self.K - 1, 0))], 0.0) if self.K else np.zeros(n)

    def pull(self, j: int):
        self.pulled = max(self.pulled, min(j + 1, self.K))

    def read(self, j: int):
        self.span = max(self.span, self.pulled - j)

    def search(self, a: int, cont_start: int) -> int:
        if self.mutant == "no_search":
            return 0
        cont = self.seg(cont_start, LC)
        if self.mutant == "match_nominal":
            cont = self.seg(a - SS + SS, LC)  # the previous frame's *nominal* continuation: x[a …]
        best, bs = -np.inf, 0
        for s in self.order:
            c = dot4(self.seg(a + s, LC), cont)
            if c > best:
                best, bs = c, s
        return bs

    def anchor(self):
        a0 = self.p5 // 5
        self.origin = self.o
        self.prev_a, self.prev_s = a0 - SS, 0
        self.cur_a, self.cur_s = a0, 0
        if self.mutant == "onset_fade":
            self.prev_a = -10**9  # the previous half reads zeros
        self.shifts.append(0)

    def next(self) -> tuple[float, int]:
        while self.pending and self.o >= self.pending[0][0]:
            self.k = NUM[self.pending.pop(0)[1]]
        if self.origin is None:
            self.anchor()
        n = self.o - self.origin
        if n % SS == 0 and n > 0:
            self.prev_a, self.prev_s = self.cur_a, self.cur_s
            if self.mutant == "restart_overlap":
                self.prev_a, self.prev_s = (self.p5 // 5) - SS, 0
            self.cur_a = self.p5 // 5
            self.pull(self.cur_a + self.delta + LC - 1)
            self.cur_s = self.search(self.cur_a, self.prev_a + self.prev_s + SS)
            self.shifts.append(self.cur_s)
        n %= SS
        j1 = self.prev_a + self.prev_s + SS + n
        j2 = self.cur_a + self.cur_s + n
        self.pull(max(j1, j2))
        self.read(min(j1, j2))
        if self.prev_a + self.prev_s + SS == self.cur_a + self.cur_s and self.mutant != "no_shortcut":
            y = self.xs(j2)  # the exact continuation (hardware B1 b)
        else:
            y = self.xs(j1) * self.w[SS + n] + self.xs(j2) * self.w[n]
        p5 = self.p5
        self.p5 += self.k
        self.o += 1
        return y, p5


def tail_c(own: int, origin: int = 0) -> int:
    """T_c(K, k) = 120 − ((own − origin) mod 60), or 60 when the remainder is 0 (R-06)."""
    d = (own - origin) % SS
    return 2 * SS - d if d else SS


def own_len(K: int, k: int) -> int:
    return -(-5 * K // k)


def run(x, level: int, outputs: int | None = None, **kw) -> tuple[np.ndarray, int, Scaler]:
    sc = Scaler(x, level, **kw)
    K = len(x)
    ys = []
    own = 0
    total = None if outputs is not None else own_len(K, NUM[level]) + tail_c(own_len(K, NUM[level]))
    while True:
        if outputs is not None and sc.o >= outputs:
            break
        if total is not None and sc.o >= total + 2 * N:
            break
        y, p5 = sc.next()
        ys.append(y)
        if p5 < 5 * K:
            own += 1
    return np.array(ys), own, sc


# ---- stimuli (the harness builds the same; the square is exactly representable) ----

def tone(f: float, secs: float = 2.0, amp: float = AMP) -> np.ndarray:
    return amp * np.sin(2 * np.pi * f * np.arange(int(secs * FS)) / FS)


def two_tone(secs: float = 2.0) -> np.ndarray:
    n = np.arange(int(secs * FS))
    return AMP / 2 * (np.sin(2 * np.pi * 701.0 * n / FS) + np.sin(2 * np.pi * 1901.0 * n / FS))


def square17(secs: float = 2.0) -> np.ndarray:
    """±0.125, 8 up and 9 down (period 17: off the hop grid, exactly representable)."""
    n = np.arange(int(secs * FS))
    return np.where(n % 17 < 8, 0.125, -0.125)


def voiced(seconds: float = 2.0, seed: int = 0x0D60, f0_fn=None) -> tuple[np.ndarray, np.ndarray]:
    rng = np.random.default_rng(seed)
    n = int(FS * seconds)
    out = np.zeros(n)
    phase, env = 0.0, 0.0
    st = np.zeros((3, 2))
    f0s = np.zeros(n)
    for i in range(n):
        u = i / n
        if f0_fn is None:
            tri_ = 2 * u if u < 0.5 else 2 - 2 * u
            f0 = 90.0 * np.exp(tri_ * np.log(220 / 90))
        else:
            f0 = f0_fn(u)
        f0s[i] = f0
        phase += f0 / FS
        exc = 0.0
        if phase >= 1:
            phase -= 1
            env = 1.0
        exc += env
        env *= np.exp(-1 / (FS * 0.0008))
        exc += (rng.random() - 0.5) * 2 * 0.025
        centres = [300 + 450 * u, 1100 + 800 * u, 2300 + 300 * u]
        bws = [80, 110, 150]
        y = 0.0
        for j in range(3):
            rr = np.exp(-np.pi * bws[j] / FS)
            th = 2 * np.pi * centres[j] / FS
            a1, a2 = 2 * rr * np.cos(th), -rr * rr
            v = exc + a1 * st[j, 0] + a2 * st[j, 1]
            st[j, 1], st[j, 0] = st[j, 0], v
            y += v * (1 - rr)
        out[i] = y
    out *= 0.1 / np.sqrt(np.mean(out**2))
    return out, f0s


def click_train(secs: float = 2.0, spacing_s: float = 0.1, amp: float = 0.5) -> np.ndarray:
    x = np.zeros(int(secs * FS))
    x[int(0.05 * FS)::int(spacing_s * FS)] = amp
    return x


# ---- readers (the harness implements the same rules) ----

def sine_fit(y: np.ndarray, f: float):
    n = np.arange(len(y))
    A = np.stack([np.cos(2 * np.pi * f * n / FS), np.sin(2 * np.pi * f * n / FS)], 1)
    c, *_ = np.linalg.lstsq(A, y, rcond=None)
    return float(np.hypot(*c)), y - A @ c


def best_freq(y: np.ndarray, f0: float, span: float = 0.03, steps: int = 601) -> float:
    fs_ = f0 * (1 + np.linspace(-span, span, steps))
    errs = [float(np.dot(sine_fit(y, f)[1], sine_fit(y, f)[1])) for f in fs_]
    return float(fs_[int(np.argmin(errs))])


def spurious_db(res: np.ndarray, amp: float, lo: float = 20.0, hi: float = 3000.0) -> float:
    n = len(res)
    w = np.kaiser(n, 20.0)
    spec = np.abs(np.fft.rfft(res * w, 4 * n)) / (w.sum() / 2)
    f = np.fft.rfftfreq(4 * n, 1 / FS)
    m = (f >= lo) & (f <= hi)
    return 20 * np.log10(spec[m].max() / amp)


def worst_window_spurious(y: np.ndarray, f: float, window: int, step: int) -> tuple[float, float]:
    """The spurious reader (R-09): a sine fit per window of `window` samples stepping by `step`
    over `y` (the own part with its first and last frame excluded), the worst window's residual
    peak in 20 – 3000 Hz re the fitted amplitude, and the fitted level range. A window of 100 ms
    reads the splice products; a longer window adds the search's bounded phase wander (a
    frequency modulation under 0.05 %), which is reported separately."""
    worst, lo_a, hi_a = -999.0, 1e9, -1e9
    for s0 in range(0, len(y) - window + 1, step):
        seg = y[s0:s0 + window]
        fb = best_freq(seg, f, span=0.01, steps=101)
        a, res = sine_fit(seg, fb)
        worst = max(worst, spurious_db(res, a))
        lo_a, hi_a = min(lo_a, a), max(hi_a, a)
    return worst, (lo_a, hi_a)


def pitch_track(y: np.ndarray, expect=None, fmin: float = 70.0, fmax: float = 260.0,
                frame: int = 240, hop: int = 60) -> np.ndarray:
    """(centre, f0, normalized peak): a normalized autocorrelation over 40 ms windows every
    10 ms, the lag window ±20 % of the expected f0 when given, parabolic interpolation."""
    rows = []
    for s in range(0, len(y) - frame - int(FS / fmin) - 2, hop):
        if expect is not None:
            e = expect(s + frame / 2)
            lmin, lmax = int(FS / (e * 1.25)), int(FS / (e * 0.8))
        else:
            lmin, lmax = int(FS / fmax), int(FS / fmin)
        seg = y[s:s + frame]
        ac = np.array([np.dot(seg, y[s + l:s + l + frame]) for l in range(lmax + 2)])
        ac = ac / (ac[0] + 1e-12)
        l = lmin + int(np.argmax(ac[lmin:lmax + 1]))
        a, b, c = ac[l - 1], ac[l], ac[l + 1]
        dl = 0.5 * (a - c) / (a - 2 * b + c) if (a - 2 * b + c) != 0 else 0.0
        rows.append((s + frame / 2, FS / (l + dl), ac[l]))
    return np.array(rows)


def click_in(y: np.ndarray, centre: float, excl, length: int = 240) -> float:
    """019's reader: the energy of a 40 ms Hann window in 20 – 2750 Hz, each excluded tone ± 250 Hz."""
    start = int(round(centre - length / 2))
    n = np.arange(length)
    w = 0.5 - 0.5 * np.cos(2 * np.pi * n / (length - 1))
    idx = start + n
    v = np.where((idx >= 0) & (idx < len(y)), y[np.clip(idx, 0, len(y) - 1)], 0.0) * w
    spec = np.abs(np.fft.rfft(v)) ** 2
    f = np.arange(len(spec)) * FS / length
    m = (f >= 20) & (f <= 2750)
    for e in excl:
        e = e if e <= FS / 2 else FS - e
        m &= np.abs(f - e) > 250
    return float(spec[m].sum())


def click_count(y: np.ndarray, amp: float, sep: int = 18, thr: float = 0.2) -> int:
    """Peaks above thr × amp at least `sep` samples (3 ms) apart (coach 7)."""
    idx = np.flatnonzero(np.abs(y) >= thr * amp)
    count, last = 0, -10**9
    for i in idx:
        if i - last >= sep:
            count += 1
            last = i
    return count


# ---- the report ----

def level_changed_run(x, a: int, b: int, change_out: int, mutant=None):
    sc = Scaler(x, a, mutant=mutant)
    sc.pending = [(change_out, b)]
    ys = []
    for _ in range(change_out + 2000):
        ys.append(sc.next()[0])
    return np.array(ys)


def held_run(x, a: int, outputs: int):
    return run(x, a, outputs=outputs)[0]


def report(bound, write: bool = False) -> None:
    db = lambda r: 20 * np.log10(max(r, 1e-300))
    print("pitch-kept (spec 023): WSOLA on the decoded 6 kHz stream; N 120, Ss 60, Δ 40, Lc 60,"
          " the window w[0..60] periodic Hann, w[60+n] = 1 − w[n] exactly; outward strict search;"
          " an engine convention (A-054)")
    sums = W[:SS] + W[SS:]
    print(f"window: every w[n] + w[n+60] == 1.0 exactly: {bool(np.all(sums == 1.0))}; w[30] bits"
          f" 0x{struct.unpack('<Q', struct.pack('<d', W[30]))[0]:016x}; w[0] = {W[0]}, w[60] = {W[60]}")
    bound(bool(np.all(sums == 1.0)), "window: a sum is not exactly 1.0")

    print("tail T_c(K, k) = 120 − ((own − origin) mod 60), or 60 (R-06; a grid bound, Δ < Ss):")
    for K in (1, 59, 119, 600, 6000):
        cells = []
        for lv in (1, 2, 4, 5):
            own = own_len(K, NUM[lv])
            cells.append(f"L{lv} own {own} tail {tail_c(own)}")
        print(f"  K = {K}: " + "; ".join(cells))
    # the content never passes own + T_c: a random check with the search allowed past K
    rng = np.random.default_rng(23)
    exceed = 0
    for _ in range(60):
        K = int(rng.integers(1, 700))
        lv = int(rng.choice([1, 2, 4, 5]))
        x = rng.standard_normal(K) * 0.3
        ys, own, _ = run(x, lv)
        last_nz = int(np.flatnonzero(np.abs(ys) > 0)[-1]) + 1 if np.any(ys != 0) else 0
        if last_nz > own + tail_c(own):
            exceed += 1
    print(f"  content past own + T_c in 60 random (K, level, noise) runs: {exceed}")
    bound(exceed == 0, "tail: content past own + T_c")

    print("tones (6 kHz, speed-only; 2 s stored; the own part with its first and last frame excluded):")
    print("  the fundamental by a sine fit over the whole span; the spurious the worst 100 ms window's residual"
          " peak (20 – 3000 Hz); the 1 s-window figure reported (it adds the search's bounded phase wander)")
    print("  stimulus      level   fundamental err %   level dB (100 ms windows)   spurious 100 ms   1 s")
    tone_fig = {}
    for name, x, f in (("1000 Hz", tone(1000.0), 1000.0), (f"{OFFGRID_HZ} Hz", tone(OFFGRID_HZ), OFFGRID_HZ),
                       ("2345 Hz", tone(2345.0), 2345.0)):
        for lv in (1, 2, 4, 5):
            ys, own, _ = run(x, lv)
            mid = ys[N:own - N]
            fb = best_freq(mid, f)
            err = 100 * (fb / f - 1)
            sp, (lo_a, hi_a) = worst_window_spurious(mid, f, 600, 300)
            sp1, _ = worst_window_spurious(mid, f, 6000, 3000)
            lvl_lo, lvl_hi = db(lo_a / AMP), db(hi_a / AMP)
            tone_fig[(name, lv)] = (err, lvl_lo, lvl_hi, sp, sp1)
            print(f"  {name:12s}  {lv}      {err:+8.3f}           {lvl_lo:+6.2f} .. {lvl_hi:+6.2f}"
                  f"           {sp:7.1f}     {sp1:7.1f}")
            if name.startswith(str(OFFGRID_HZ)):
                bound(abs(err) <= 1.0, f"{name} L{lv}: fundamental {err:+.3f} % beyond 1 %")
                bound(max(abs(lvl_lo), abs(lvl_hi)) <= 0.5, f"{name} L{lv}: level beyond 0.5 dB")
                bound(sp <= -28.0, f"{name} L{lv}: spurious {sp:.1f} dB above −28")
    x2 = two_tone()
    for lv in (1, 2, 4, 5):
        ys, own, _ = run(x2, lv)
        mid = ys[N:own - N]
        a1, r1 = sine_fit(mid, best_freq(mid, 701.0))
        a2, r2 = sine_fit(r1, best_freq(r1, 1901.0))
        print(f"  701+1901 Hz   {lv}      levels {db(a1 / (AMP / 2)):+.2f} / {db(a2 / (AMP / 2)):+.2f} dB"
              f"   residual {spurious_db(r2, AMP / 2):.1f} dB (reported)")

    print("no_search mutant on the off-grid tone (its oracle):")
    for lv in (1, 2, 4, 5):
        ys, own, _ = run(tone(OFFGRID_HZ), lv, mutant="no_search")
        mid = ys[N:own - N]
        sp, (lo_a, hi_a) = worst_window_spurious(mid, OFFGRID_HZ, 600, 300)
        print(f"  L{lv}: level {db(lo_a / AMP):+.1f} .. {db(hi_a / AMP):+.1f} dB, spurious (100 ms) {sp:.1f} dB")

    print("vowel (steady 130 Hz; the reader: 40 ms windows every 10 ms, lag ±20 %, parabolic):")
    xv, f0v = voiced(2.0, f0_fn=lambda u: 130.0)
    tr0 = pitch_track(xv, expect=lambda t: 130.0)
    e0 = np.abs(100 * (tr0[:, 1] / 130.0 - 1))
    print(f"  the reader on the stored vowel: |err| p50 {np.percentile(e0, 50):.2f} %, p95 {np.percentile(e0, 95):.2f} %,"
          f" max {e0.max():.2f} %")
    for lv in (1, 2, 4, 5):
        ys, own, _ = run(xv, lv)
        tr = pitch_track(ys[:own], expect=lambda t: 130.0)
        voiced_rows = tr[tr[:, 2] >= 0.5]
        e = np.abs(100 * (voiced_rows[:, 1] / 130.0 - 1))
        lvl = db(np.sqrt(np.mean(ys[N:own - N] ** 2)) / np.sqrt(np.mean(xv ** 2)))
        print(f"  L{lv}: |err| p50 {np.percentile(e, 50):.2f} %, p95 {np.percentile(e, 95):.2f} %, max {e.max():.2f} %;"
              f" level {lvl:+.2f} dB")
        bound(np.percentile(e, 50) <= 1.0, f"vowel L{lv}: median pitch error {np.percentile(e, 50):.2f} % > 1")

    print("glide (90 → 220 → 90 Hz, 2 s; the output's window at t against the stored track at r·t):")
    xg, f0g = voiced(2.0)
    trg0 = pitch_track(xg, expect=lambda t: f0g[min(int(t), len(f0g) - 1)])
    eg0 = np.abs(100 * (trg0[:, 1] / f0g[np.minimum(trg0[:, 0].astype(int), len(f0g) - 1)] - 1))
    print(f"  the reader on the stored glide: p50 {np.percentile(eg0, 50):.2f} %, p95 {np.percentile(eg0, 95):.2f} %,"
          f" max {eg0.max():.2f} %")
    for lv in (1, 2, 4, 5):
        r = NUM[lv] / 5
        ys, own, _ = run(xg, lv)
        exp = lambda t, r=r: f0g[min(int(t * r), len(f0g) - 1)]
        tr = pitch_track(ys[:own], expect=exp)
        rows = tr[tr[:, 2] >= 0.5]
        ref = np.array([exp(t) for t in rows[:, 0]])
        e = np.abs(100 * (rows[:, 1] / ref - 1))
        print(f"  L{lv}: p50 {np.percentile(e, 50):.2f} %, p95 {np.percentile(e, 95):.2f} %, max {e.max():.2f} %")
        bound(np.percentile(e, 50) <= 1.0, f"glide L{lv}: median {np.percentile(e, 50):.2f} % > 1")
        bound(np.percentile(e, 95) <= 2.5, f"glide L{lv}: p95 {np.percentile(e, 95):.2f} % > 2.5")

    print("click train (one per 100 ms, 0.5; the counter: ≥ 0.2 × the click, ≥ 3 ms apart; reported):")
    xc = click_train()
    nin = click_count(xc, 0.5)
    for lv in (1, 2, 4, 5):
        ys, own, _ = run(xc, lv)
        print(f"  L{lv}: {click_count(ys[:own], 0.5)} of {nin}")

    print("level change in pitch-kept mode (R-04): reading A (40 ms at the first hop boundary after the"
          " change, the tone excluded, on the residual against the held run) vs B (200 ms later), dB re the tone:")
    print("  pair    stimulus      A       B    A−B   restart_overlap A   into3_swap n/a")
    for a, b in ((1, 5), (5, 1), (2, 4), (4, 2), (1, 2), (5, 4)):
        for name, x, f in ((f"{OFFGRID_HZ} Hz", tone(OFFGRID_HZ, 3.0), OFFGRID_HZ), ("1000 Hz", tone(1000.0, 3.0), 1000.0)):
            change = 1800 + 17  # mid-hop
            held = held_run(x, a, change + 2000)
            ys = level_changed_run(x, a, b, change)
            res = ys - held[:len(ys)]
            first_hop = change + (SS - (change % SS)) % SS
            A = db(np.sqrt(click_in(res, first_hop, [f])) / AMP)
            B = db(np.sqrt(click_in(res, first_hop + 1200, [f])) / AMP)
            ym = level_changed_run(x, a, b, change, mutant="restart_overlap")
            Am = db(np.sqrt(click_in(ym - held[:len(ym)], first_hop, [f])) / AMP)
            print(f"  {a}->{b}   {name:12s} {A:6.1f} {B:6.1f} {A - B:+5.1f}        {Am:6.1f}")
            if name.startswith(str(OFFGRID_HZ)):
                bound(A <= B + 1.0, f"level change {a}->{b} on {name}: A {A:.1f} > B {B:.1f} + 1")

    print("ring span (lazy pulls; the stationary pitch-kept run): max over the tone runs")
    worst = 0
    for lv in (1, 2, 4, 5):
        _, _, sc = run(tone(OFFGRID_HZ), lv)
        worst = max(worst, sc.span)
    print(f"  {worst} (the mode-blend worst case 224 by the plan review's sweep; RING 256)")

    print("shift digests (FNV-1a 64 over the per-frame shifts as i32 LE; the Rust reference pins these"
          " on the exactly representable square; the tones' and the vowel's are the harness reference's):")
    for name, x in (("square17", square17()),):
        for lv in (1, 2, 4, 5):
            # Exactly the stage's outputs (own + T_c): the shifts the Rust stage records.
            o = own_len(len(x), NUM[lv])
            _, _, sc = run(x, lv, outputs=o + tail_c(o))
            print(f"  {name} L{lv}: {len(sc.shifts)} frames, digest 0x{fnv1a64_i32(sc.shifts):016x},"
                  f" first 12 {sc.shifts[:12]}")

    if write:
        out = Path(__file__).resolve().parents[2] / "crates" / "rr_dr60" / "src" / "stages" / "speed_window.rs"
        lines = [
            "//! The pitch-kept time-scaler's window (spec 023 research R-02; signal-chain stage 9).",
            "//!",
            "//! GENERATED by `tools/playback-speed/speed_design.py --write`; do not edit by hand.",
            "//! Regenerate with: `uv run tools/playback-speed/speed_design.py --write`.",
            "//!",
            "//! `WINDOW[n]` for n = 0 … 59 is the periodic Hann 0.5 − 0.5·cos(2πn/120) and `WINDOW[60 + n]`",
            "//! is exactly 1 − WINDOW[n], so every overlapping pair sums to 1.0 bit for bit (the plan",
            "//! review's B1). Engineering targets: N = 120, Ss = 60 (an engine convention, A-054).",
            "",
            "/// The window, 120 entries, exact bit patterns.",
            "pub(crate) const WINDOW: [f64; 120] = [",
        ]
        for v in W:
            bits = struct.unpack("<Q", struct.pack("<d", float(v)))[0]
            lines.append(f"    f64::from_bits(0x{bits:016x}), // {float(v)!r}")
        lines.append("];")
        lines.append("")
        out.write_text("\n".join(lines) + "\n")
        print(f"wrote {out}")
