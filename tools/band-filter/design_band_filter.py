# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Reference design of spec 024's band filter (research R-02, R-07): prints the figures the core
and harness tests pin, and writes the harness's reference table.

The section is the topology-preserving state-variable form: g = tan(pi f / r), k = 1 / Q. Its
digital transfer functions are the bilinear, prewarped prototypes, written here in closed form
(independent of the engine's state update):

    D(z)   = (1 + g k + g^2) + (2 g^2 - 2) z^-1 + (1 - g k + g^2) z^-2
    low    = g^2 (1 + z^-1)^2 / D
    band   = k g (1 - z^-2) / D            (0 dB peak, spec D4)
    high   = (1 - z^-1)^2 / D
    notch  = ((1 + g^2) + (2 g^2 - 2) z^-1 + (1 + g^2) z^-2) / D

each followed by the gain 10^(dB / 20). The parameter values are the single-precision values the
engine's store holds (research R-06); the frequency is the applied one, min(f, 0.45 r) (D4). The
oracle frequencies are the exact third-octave centres 1000 * 2^(n / 3) from 20 Hz to 0.45 r plus
the applied corner. The impulse length follows the pole radius (hardware plan review B5):
a2 = |z_p|^2 = (1 - g k + g^2) / (1 + g k + g^2), L = ceil(2 ln(1e8) / (-ln a2)) + ceil(0.1 r).

Usage: uv run tools/band-filter/design_band_filter.py [--check | --write]
"""

import cmath
import math
import struct
import sys
from pathlib import Path

RATES = [6000, 8000, 16000, 44100, 48000, 88200, 96000]
CEILING = 0.45  # engineering target (024 D4)
TYPES = ["low_pass", "high_pass", "band_pass", "notch"]
# The parameter sets (research R-08): each type at the defaults, then the low-pass at three sets.
SETS = [
    (0, 1000.0, 0.7071, 0.0),
    (1, 1000.0, 0.7071, 0.0),
    (2, 1000.0, 0.7071, 0.0),
    (3, 1000.0, 0.7071, 0.0),
    (0, 300.0, 0.5, 0.0),
    (0, 1000.0, 3.0, 0.0),
    (0, 3000.0, 10.0, 0.0),
]
OUT = Path(__file__).resolve().parents[2] / "crates/rr_dr60_harness/src/band_filter_ref.rs"


def f32(x: float) -> float:
    return struct.unpack("<f", struct.pack("<f", x))[0]


def bits(x: float) -> int:
    return struct.unpack("<Q", struct.pack("<d", x))[0]


def params(freq, q, gain_db, rate):
    f = min(f32(freq), CEILING * rate)
    q = f32(q)
    gain = 10.0 ** (f32(gain_db) / 20.0)
    g = math.tan(math.pi * f / rate)
    k = 1.0 / q
    return f, g, k, gain


def response(ftype, g, k, gain, w):
    z1 = cmath.exp(-1j * w)
    z2 = z1 * z1
    den = (1 + g * k + g * g) + (2 * g * g - 2) * z1 + (1 - g * k + g * g) * z2
    if ftype == 0:
        num = g * g * (1 + z1) ** 2
    elif ftype == 1:
        num = (1 - z1) ** 2
    elif ftype == 2:
        num = k * g * (1 - z2)
    else:
        num = (1 + g * g) + (2 * g * g - 2) * z1 + (1 + g * g) * z2
    return gain * num / den


def impulse_length(g, k, rate):
    a2 = (1 - g * k + g * g) / (1 + g * k + g * g)
    return math.ceil(2 * math.log(1e8) / (-math.log(a2))) + math.ceil(0.1 * rate)


def oracle_freqs(f_applied, rate):
    top = CEILING * rate
    fs = []
    n = -17
    while True:
        f = 1000.0 * 2.0 ** (n / 3.0)
        if f > top:
            break
        if f >= 20.0:
            fs.append(f)
        n += 1
    fs.append(f_applied)
    return fs


def cases():
    out = []
    for s, (ftype, freq, q, gain_db) in enumerate(SETS):
        for rate in RATES:
            f, g, k, gain = params(freq, q, gain_db, rate)
            dc = response(ftype, g, k, gain, 0.0)
            nyq = response(ftype, g, k, gain, math.pi)
            rows = []
            for fr in oracle_freqs(f, rate):
                h = response(ftype, g, k, gain, 2 * math.pi * fr / rate)
                rows.append((fr, h))
            out.append(dict(set=s, ftype=ftype, rate=rate, freq=f32(freq), q=f32(q),
                            gain_db=f32(gain_db), f_applied=f, g=g, k=k,
                            length=impulse_length(g, k, rate), dc=dc, nyq=nyq, rows=rows))
    return out


def cross_rate_table():
    # Hardware plan review B16: a low-pass at Q 0.7071 read at 2500 Hz, by corner and host rate.
    lines = []
    for corner in (1000.0, 1500.0, 2000.0, 2500.0):
        row = []
        for rate in RATES:
            f, g, k, gain = params(corner, 0.7071, 0.0, rate)
            h = response(0, g, k, gain, 2 * math.pi * 2500.0 / rate)
            row.append(20 * math.log10(abs(h)))
        lines.append((corner, row))
    return lines


def print_figures(cs):
    print("Spec 024 band filter: the reference design (research R-02, R-07)")
    print(f"rates {RATES}; ceiling {CEILING} x rate")
    for c in cs:
        print(f"set {c['set']} {TYPES[c['ftype']]:9s} rate {c['rate']:5d} f {c['f_applied']:9.3f} "
              f"Q {c['q']:.4f} L {c['length']:7d} |DC| {abs(c['dc']):.3e} |Nyq| {abs(c['nyq']):.3e} "
              f"rows {len(c['rows'])}")
    print("cross-rate table: low-pass Q 0.7071 read at 2500 Hz (dB), corners by host rate")
    print("corner " + " ".join(f"{r:>8d}" for r in RATES))
    for corner, row in cross_rate_table():
        print(f"{corner:6.0f} " + " ".join(f"{v:8.2f}" for v in row))


def check(cs):
    ok = True
    for c in cs:
        t = c["ftype"]
        zero_dc = t in (1, 2)
        zero_nyq = t in (0, 2)
        if zero_dc and abs(c["dc"]) != 0.0:
            print(f"FAIL DC zero {c}")
            ok = False
        if zero_nyq and abs(c["nyq"]) > 1e-15:
            print(f"FAIL Nyquist zero set {c['set']} rate {c['rate']}: {abs(c['nyq'])}")
            ok = False
        if not zero_dc and abs(abs(c["dc"]) - 1.0) > 1e-10:
            print(f"FAIL DC unity set {c['set']} rate {c['rate']}")
            ok = False
        if not zero_nyq and abs(abs(c["nyq"]) - 1.0) > 1e-10:
            print(f"FAIL Nyquist unity set {c['set']} rate {c['rate']}")
            ok = False
        if c["length"] > 400_000:
            print(f"FAIL impulse length {c['length']}")
            ok = False
    print("check:", "ok" if ok else "FAILED")
    return ok


def render(cs):
    out = [
        "//! The band filter's reference response (spec 024 research R-07). GENERATED by",
        "//! `uv run tools/band-filter/design_band_filter.py --write`; do not edit. The closed-form",
        "//! bilinear prototypes at the store's single-precision parameter values; the frequencies",
        "//! are engineering targets of the oracle (024 D4), not device figures.",
        "",
        "/// One parameter set at one host rate.",
        "#[derive(Clone, Copy, Debug)]",
        "pub struct RefCase {",
        "    /// The set's index (research R-08: 0 – 3 the four types at the defaults, 4 – 6 the low-pass).",
        "    pub set: u8,",
        "    /// The type (0 low-pass, 1 high-pass, 2 band-pass, 3 notch).",
        "    pub ftype: u8,",
        "    /// The host rate.",
        "    pub rate: u32,",
        "    /// The requested frequency as the store holds it.",
        "    pub freq: f32,",
        "    /// The Q as the store holds it.",
        "    pub q: f32,",
        "    /// The gain in dB as the store holds it.",
        "    pub gain_db: f32,",
        "    /// The applied frequency (min(f, 0.45 r)).",
        "    pub f_applied: f64,",
        "    /// The impulse length that reaches a −160 dB tail (the pole-radius rule).",
        "    pub length: usize,",
        "    /// The response at DC (real; the imaginary part is zero).",
        "    pub dc: f64,",
        "    /// The response at Nyquist (real; the imaginary part is zero).",
        "    pub nyquist: f64,",
        "    /// The first row of [`REF_ROWS`] this case owns.",
        "    pub first: usize,",
        "    /// The number of rows this case owns: `REF_ROWS[first .. first + count]`.",
        "    pub count: usize,",
        "}",
        "",
        "/// One oracle frequency: (frequency, real, imaginary) of the response.",
        "pub type RefRow = (f64, f64, f64);",
        "",
    ]
    case_lines = ["/// Every case, by set then rate.", "#[rustfmt::skip]", "pub const REF_CASES: &[RefCase] = &["]
    row_lines = ["/// The oracle rows of every case, in case order.", "#[rustfmt::skip]", "pub const REF_ROWS: &[RefRow] = &["]
    first = 0
    for c in cs:
        case_lines.append(
            f"    RefCase {{ set: {c['set']}, ftype: {c['ftype']}, rate: {c['rate']}, "
            f"freq: f32::from_bits(0x{struct.unpack('<I', struct.pack('<f', c['freq']))[0]:08x}), "
            f"q: f32::from_bits(0x{struct.unpack('<I', struct.pack('<f', c['q']))[0]:08x}), "
            f"gain_db: f32::from_bits(0x{struct.unpack('<I', struct.pack('<f', c['gain_db']))[0]:08x}), "
            f"f_applied: f64::from_bits(0x{bits(c['f_applied']):016x}), length: {c['length']}, "
            f"dc: f64::from_bits(0x{bits(c['dc'].real):016x}), "
            f"nyquist: f64::from_bits(0x{bits(c['nyq'].real):016x}), first: {first}, count: {len(c['rows'])} }},")
        for fr, h in c["rows"]:
            row_lines.append(
                f"    (f64::from_bits(0x{bits(fr):016x}), f64::from_bits(0x{bits(h.real):016x}), "
                f"f64::from_bits(0x{bits(h.imag):016x})),")
        first += len(c["rows"])
    case_lines.append("];")
    row_lines.append("];")
    return "\n".join(out + case_lines + [""] + row_lines) + "\n"


def main():
    cs = cases()
    if "--write" in sys.argv:
        OUT.write_text(render(cs))
        print(f"wrote {OUT}")
        return
    if "--check" in sys.argv:
        sys.exit(0 if check(cs) else 1)
    print_figures(cs)


if __name__ == "__main__":
    main()
