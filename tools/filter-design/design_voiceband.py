# /// script
# requires-python = ">=3.10"
# dependencies = ["numpy", "scipy"]
# ///
"""Design and verify the RR-DR60 codec band-limit stages (signal-chain stages 4 and 10).

Spec 004 (research R-04, R-05, R-06), at the 6 kHz device rate (A-026, S-002). The two stages
are different filters, each modeled on the OKI codec family's datasheet template (S-005) with
every frequency scaled by 0.75 to the measured clock (A-027), designed to the datasheet's
typical values with its limits as tolerances:

  * record (stage 4, transmit): Butterworth high-pass, order 3, -3 dB at 112.5 Hz, plus a
    Chebyshev-II (inverse Chebyshev) low-pass, order 5, 32 dB stopband, stopband edge tuned so
    the loss at 2550 Hz lands near the typical 0.4 dB;
  * playback (stage 10, receive): the Chebyshev-II low-pass alone (no low-frequency rejection:
    the receive filter passes DC), tuned near the typical 0.56 dB at 2550 Hz.
  * gain normalized to exactly 0 dB at 765 Hz (0.75 x the datasheet's 1020 Hz reference; A-015);
    minimum-phase by construction (A-016).

The stopband attenuation (32 dB, S-005's typical discrimination), the filter family and order,
the edge-search margins and the DC floor are engineering targets (004 FR-015, R-14); the loss and
group-delay limits are S-005 scaled (A-027); the 40 dB DC attenuation is an engineering target
(ITU-T G.712, S-007, states no DC requirement).

The script verifies every FR-005, FR-006 and FR-008 bound analytically and exits non-zero on any
failure. The emitted Rust file stores each coefficient as an exact bit pattern, so the Rust build
never depends on this script's floating-point environment (001 R-04).

Usage (from the repository root):
  uv run tools/filter-design/design_voiceband.py            # design, verify, write coefficients
  uv run tools/filter-design/design_voiceband.py --check    # design and verify only
  uv run tools/filter-design/design_voiceband.py --family ellip --check   # comparison table only
  uv run tools/filter-design/design_voiceband.py --shift-record-hp-hz 100 --emit-fixture PATH
  uv run tools/filter-design/design_voiceband.py --shift-record-lp-hz -100 --emit-fixture PATH
  uv run tools/filter-design/design_voiceband.py --shift-playback-lp-hz -100 --emit-fixture PATH
      # SC-007 fixtures: one edge shifted 100 Hz toward the band
"""

from __future__ import annotations

import argparse
import struct
import sys
from dataclasses import dataclass
from pathlib import Path

import numpy as np
from scipy import signal

FS = 6000.0  # A-026: device rate (S-002)
REF_HZ = 765.0  # 0.75 x 1020 Hz, the S-005 reference frequency (A-027)
HP_ORDER, HP_FC = 3, 112.5  # R-04: 0.75 x 150 Hz; order fixed by the S-005 group-delay rows
LP_ORDER, LP_RS = 5, 32.0  # R-04/R-05: Chebyshev-II, 32 dB = S-005 typical discrimination
BAND_LO, BAND_HI, EDGE = 225.0, 2250.0, 2550.0  # 0.75 x (300, 3000, 3400) Hz, A-027
GDD_BAND = (225.0, 2550.0)  # group-delay distortion is relative to the minimum over this band
REPORT_HZ = (2700.0, 2900.0)  # not constrained by any source: report only (004 FR-005, FR-008)
# Edge-search rule (R-04; engineering targets, 004 FR-015):
TYP_TOL_DB = 0.1  # land within this of the typical 2550 Hz loss
GDD_1950_MAX = 0.17 - 0.015  # keep 0.015 ms of design margin under the 0.17 ms limit
EDGE_LOSS_MIN_DB = 0.02  # the 0..0.8 dB tolerance must not fail on a rounding sign
DC_CLAMP_DB = 200.0  # zeros at z = 1 give infinite attenuation; clamp for the table

REPO_ROOT = Path(__file__).resolve().parents[2]
OUTPUT = REPO_ROOT / "crates/rr_dr60/src/stages/voiceband_coeffs.rs"


@dataclass(frozen=True)
class Template:
    """One stage's limits (S-005 scaled by 0.75, A-027) and design goals (typical values)."""

    name: str
    const: str
    sections: int
    has_highpass: bool
    loss_lo_db: float  # lower bound of the in-band loss (negative = gain allowed)
    loss_hi_db: float
    band_lo_hz: float  # where the in-band loss limits start (225 Hz, or DC for the receive side)
    edge_typ_db: float  # typical loss at 2550 Hz (design goal)
    gdd_limits_ms: dict  # frequency -> max group-delay distortion
    gdd_typ_ms: dict  # frequency -> typical (design goal, reported)


RECORD = Template(
    name="record (stage 4, S-005 transmit)",
    const="RECORD_SOS",
    sections=5,
    has_highpass=True,
    loss_lo_db=-0.15,
    loss_hi_db=0.2,
    band_lo_hz=BAND_LO,
    # A-027 (amended 2026-10-17): the MSM7702's own sheet (S-003) gives 0.5 dB typ; the shipped
    # filters are kept at the S-005 proxy value (0.306 dB realized, inside S-003's 0 ... 0.8 dB).
    edge_typ_db=0.4,
    gdd_limits_ms={375: 1.0, 450: 0.47, 750: 0.17, 1000: 0.17, 1500: 0.17, 1950: 0.17, 2100: 1.0},
    gdd_typ_ms={375: 0.25, 450: 0.15, 750: 0.03, 1000: 0.03, 1500: 0.03, 1950: 0.067, 2100: 0.093},
)
PLAYBACK = Template(
    name="playback (stage 10, S-005 receive)",
    const="PLAYBACK_SOS",
    sections=3,
    has_highpass=False,
    loss_lo_db=-0.15,  # carried from the transmit row (004 FR-015); S-003 prints it for receive too
    # A-027 (amended 2026-10-17): S-003 gives +0.20 dB and 0.47 dB typ; the shipped filters are
    # kept at the S-005 proxy values (realized max +0.001 dB, 0.478 dB at 2550 Hz).
    loss_hi_db=0.25,
    band_lo_hz=0.0,
    edge_typ_db=0.56,
    gdd_limits_ms={375: 0.17, 450: 0.17, 750: 0.17, 1000: 0.17, 1500: 0.17, 1950: 0.17, 2100: 1.0},
    gdd_typ_ms={375: 0.0, 450: 0.0, 750: 0.0, 1000: 0.0, 1500: 0.0, 1950: 0.12, 2100: 0.16},
)


def lowpass(family: str, edge_hz: float) -> np.ndarray:
    if family == "cheby2":
        return signal.cheby2(LP_ORDER, LP_RS, edge_hz, "lowpass", fs=FS, output="sos")
    if family == "ellip":  # comparison only (R-04 alternatives): 0.1 dB ripple, 35 dB stopband
        return signal.ellip(LP_ORDER, 0.1, 35.0, edge_hz, "lowpass", fs=FS, output="sos")
    raise ValueError(family)


def assemble(t: Template, family: str, lp_edge_hz: float, hp_fc_hz: float) -> np.ndarray:
    parts = []
    if t.has_highpass:
        parts.append(signal.butter(HP_ORDER, hp_fc_hz, "highpass", fs=FS, output="sos"))
    parts.append(lowpass(family, lp_edge_hz))
    sos = np.vstack(parts)
    _, href = signal.sosfreqz(sos, worN=[REF_HZ], fs=FS)
    sos[0, :3] /= abs(href[0])  # A-015: unity gain at the reference frequency
    assert sos.shape == (t.sections, 6) and np.allclose(sos[:, 3], 1.0)
    return sos


def loss_db(sos: np.ndarray, freqs) -> np.ndarray:
    _, h = signal.sosfreqz(sos, worN=np.atleast_1d(np.asarray(freqs, dtype=float)), fs=FS)
    return -20.0 * np.log10(np.maximum(np.abs(h), 1e-300))


def group_delay_ms(sos: np.ndarray, f: float, df: float = 0.5) -> float:
    _, h = signal.sosfreqz(sos, worN=[f - df, f + df], fs=FS)
    dphi = np.unwrap(np.angle(h))
    return -(dphi[1] - dphi[0]) / (2.0 * np.pi * 2.0 * df) * 1000.0


def gdd_table(sos: np.ndarray, freqs) -> dict:
    band = np.arange(GDD_BAND[0], GDD_BAND[1] + 0.1, 2.5)
    gmin = min(group_delay_ms(sos, f) for f in band)
    return {f: group_delay_ms(sos, f) - gmin for f in freqs}


def search_edge(t: Template, family: str, hp_fc_hz: float) -> tuple[float, str]:
    """R-04 edge-search rule: the highest stopband edge whose 2550 Hz loss is within TYP_TOL_DB
    of the typical, with GDD(1950) <= GDD_1950_MAX and loss(2550) >= EDGE_LOSS_MIN_DB. If no edge
    satisfies all three, the GDD margin wins: the highest edge with GDD(1950) within the margin
    whose loss stays >= EDGE_LOSS_MIN_DB, and the shortfall is reported."""
    candidates = np.arange(2600.0, 2900.0 + 0.5, 1.0)
    best, fallback = None, None
    for edge in candidates:
        sos = assemble(t, family, edge, hp_fc_hz)
        e_loss = float(loss_db(sos, EDGE)[0])
        gdd1950 = gdd_table(sos, [1950.0])[1950.0]
        if e_loss < EDGE_LOSS_MIN_DB or gdd1950 > GDD_1950_MAX:
            continue
        if fallback is None or abs(e_loss - t.edge_typ_db) < abs(fallback[1] - t.edge_typ_db):
            fallback = (edge, e_loss)
        if abs(e_loss - t.edge_typ_db) <= TYP_TOL_DB:
            best = (edge, e_loss)  # candidates ascend, so the last hit is the highest edge
    if best is not None:
        return best[0], f"edge {best[0]:.0f} Hz: loss at {EDGE:.0f} Hz {best[1]:.3f} dB (typ {t.edge_typ_db})"
    if fallback is None:
        raise SystemExit(f"{t.name}: no stopband edge satisfies the GDD margin and the positive-loss floor")
    return fallback[0], (
        f"edge {fallback[0]:.0f} Hz: loss at {EDGE:.0f} Hz {fallback[1]:.3f} dB; the typical "
        f"{t.edge_typ_db} dB is out of reach under the GDD margin (shortfall {t.edge_typ_db - fallback[1]:+.3f} dB)"
    )


def verify(t: Template, sos: np.ndarray) -> list[tuple[str, str, str, bool]]:
    rows = []
    gref = float(-loss_db(sos, REF_HZ)[0])
    rows.append((f"gain at {REF_HZ:.0f} Hz", "0 dB ± 0.1", f"{gref:+.4f} dB", abs(gref) <= 0.1))

    lo = max(t.band_lo_hz, 0.5)
    f = np.linspace(lo, BAND_HI, int(BAND_HI - lo) + 1)
    inband = loss_db(sos, f)
    if not t.has_highpass:
        inband = np.append(inband, loss_db(sos, 0.0))  # DC belongs to the receive band
    lo_name = "DC" if not t.has_highpass else f"{t.band_lo_hz:.0f} Hz"
    rows.append((f"loss {lo_name}–{BAND_HI:.0f} Hz (min)", f"≥ {t.loss_lo_db} dB", f"{inband.min():+.3f} dB", inband.min() >= t.loss_lo_db))
    rows.append((f"loss {lo_name}–{BAND_HI:.0f} Hz (max)", f"≤ +{t.loss_hi_db} dB", f"{inband.max():+.3f} dB", inband.max() <= t.loss_hi_db))

    e = float(loss_db(sos, EDGE)[0])
    rows.append((f"loss at {EDGE:.0f} Hz", f"0 … 0.8 dB (typ {t.edge_typ_db})", f"{e:.3f} dB", 0.0 <= e <= 0.8))

    if t.has_highpass:
        a45 = float(loss_db(sos, 45.0)[0])
        a20 = float(loss_db(sos, 20.0)[0])
        adc = float(min(loss_db(sos, 0.0)[0], DC_CLAMP_DB))
        rows.append(("attenuation at 45 Hz", "≥ 20 dB (typ 26)", f"{a45:.1f} dB", a45 >= 20.0))
        rows.append(("attenuation at 20 Hz", "≥ 20 dB", f"{a20:.1f} dB", a20 >= 20.0))
        rows.append(("attenuation at DC", "≥ 40 dB (eng. target)", f"{adc:.0f} dB", adc >= 40.0))
    else:
        for fx in (0.0, 20.0, 45.0):
            v = float(loss_db(sos, fx)[0])
            rows.append((f"loss at {fx:.0f} Hz (passes DC)", f"{t.loss_lo_db} … +{t.loss_hi_db} dB", f"{v:+.3f} dB", t.loss_lo_db <= v <= t.loss_hi_db))

    for fx in REPORT_HZ:
        rows.append((f"loss at {fx:.0f} Hz", "report only", f"{float(loss_db(sos, fx)[0]):.1f} dB", True))

    gdd = gdd_table(sos, list(t.gdd_limits_ms))
    for fx, lim in t.gdd_limits_ms.items():
        rows.append((f"GDD at {fx} Hz", f"≤ {lim} ms (typ {t.gdd_typ_ms[fx]})", f"{gdd[fx]:.3f} ms", gdd[fx] <= lim))

    gd1k = group_delay_ms(sos, 1000.0)
    gd_edge = group_delay_ms(sos, EDGE)
    rows.append(("group delay at 1 kHz", "≤ 2 ms (A-016)", f"{gd1k:.3f} ms", gd1k <= 2.0))
    if t.has_highpass:
        gd_lo = group_delay_ms(sos, BAND_LO)
        rows.append((f"group delay at {BAND_LO:.0f} Hz > 1 kHz", "yes (A-016)", f"{gd_lo:.3f} ms", gd_lo > gd1k))
    rows.append((f"group delay at {EDGE:.0f} Hz > 1 kHz", "yes (A-016)", f"{gd_edge:.3f} ms", gd_edge > gd1k))
    rows.append((f"group delay at {EDGE:.0f} Hz", "≤ 2 ms (eng. target)", f"{gd_edge:.3f} ms", gd_edge <= 2.0))

    z, p, _ = signal.sos2zpk(sos)
    pmax, zmax = float(np.max(np.abs(p))), float(np.max(np.abs(z)))
    rows.append(("max |pole| (stable)", "< 1", f"{pmax:.6f}", pmax < 1.0))
    rows.append(("max |zero| (min-phase)", "≤ 1 (+1e-6)", f"{zmax:.9f}", zmax <= 1.0 + 1e-6))
    return rows


def hexbits(x: float) -> str:
    return "0x" + struct.pack(">d", float(x)).hex()


def render_const(t: Template, sos: np.ndarray, const_name: str, visibility: str, edge_note: str) -> list[str]:
    kinds = (["high-pass"] * (t.sections - 3) + ["low-pass"] * 3) if t.has_highpass else ["low-pass"] * 3
    lines = [f"/// {t.name}: {edge_note}.", "#[rustfmt::skip]",
             f"{visibility}const {const_name}: [[f64; 5]; {t.sections}] = ["]
    for i, row in enumerate(sos):
        coeffs = [row[0], row[1], row[2], row[4], row[5]]
        lines.append(f"    // section {i}: {kinds[i]}")
        lines.append("    [")
        for n, c in zip(["b0", "b1", "b2", "a1", "a2"], coeffs):
            lines.append(f"        f64::from_bits({hexbits(c)}), // {n} = {c:.17e}")
        lines.append("    ],")
    lines.append("];")
    return lines


def render_table(rows) -> list[str]:
    width = max(len(r[0]) for r in rows)
    return [f"//!   {prop:<{width}}  {target:<28} {measured}" for prop, target, measured, _ in rows]


def render_file(designs, family: str, shifts: dict, fixture: bool) -> str:
    cmd = "uv run tools/filter-design/design_voiceband.py"
    for k, v in shifts.items():
        if v:
            cmd += f" --{k} {v:g}"
    if fixture:
        cmd += " --emit-fixture <path>"
    title = ("Test fixture: codec band-limit stages with one edge shifted (SC-007)." if fixture
             else "Codec band-limit stage coefficients (signal-chain stages 4 and 10; spec 004).")
    lines = [
        f"//! {title}",
        "//!",
        "//! GENERATED by tools/filter-design/design_voiceband.py — do not edit.",
        f"//! Regenerate with: `{cmd}`",
        "//!",
        f"//! Design (spec 004 research R-04, R-05), at the {FS:g} Hz device rate (A-026, S-002):",
        f"//! - record (stage 4): Butterworth high-pass, order {HP_ORDER}, -3 dB at {HP_FC + shifts.get('shift-record-hp-hz', 0):g} Hz,",
        f"//!   plus a {family} low-pass, order {LP_ORDER}, {LP_RS:g} dB stopband (S-005 typical discrimination)",
        f"//! - playback (stage 10): the {family} low-pass alone (receive filter: passes DC)",
        f"//! - gain normalized to 0 dB at {REF_HZ:g} Hz (0.75 x 1020 Hz, A-015); minimum-phase (A-016)",
        "//! - templates: S-005 scaled by 0.75 (A-027); family, order, stopband and the edge-search",
        "//!   margins are engineering targets (004 FR-015); 40 dB at DC is an engineering target",
        "//!   (ITU-T G.712, S-007, states no DC requirement)",
        "//!",
    ]
    for t, sos, note, rows in designs:
        lines.append(f"//! {t.name}: {note}")
        lines.extend(render_table(rows))
        lines.append("//!")
    lines += ["//! Each section is `[b0, b1, b2, a1, a2]` with `a0 = 1`, stored as exact bit patterns",
              "//! so the coefficients are identical on every platform (001 R-04).", ""]
    for t, sos, note, _ in designs:
        name = f"SHIFTED_{t.const}" if fixture else t.const
        vis = "pub " if fixture else "pub(crate) "
        lines.extend(render_const(t, sos, name, vis, note))
        lines.append("")
    return "\n".join(lines).rstrip("\n") + "\n"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--check", action="store_true", help="design and verify only; write nothing")
    ap.add_argument("--family", choices=["cheby2", "ellip"], default="cheby2", help="low-pass family (cheby2 is the committed design)")
    ap.add_argument("--shift-record-hp-hz", type=float, default=0.0, help="shift the record high-pass corner (fixtures only)")
    ap.add_argument("--shift-record-lp-hz", type=float, default=0.0, help="shift the record low-pass stopband edge (fixtures only; negative = toward the band)")
    ap.add_argument("--shift-playback-lp-hz", type=float, default=0.0, help="shift the playback low-pass stopband edge (fixtures only)")
    ap.add_argument("--emit-fixture", type=Path, help="write a test fixture instead of the coefficient file")
    ap.add_argument("--output", type=Path, default=OUTPUT, help=argparse.SUPPRESS)
    args = ap.parse_args()

    shifts = {"shift-record-hp-hz": args.shift_record_hp_hz, "shift-record-lp-hz": args.shift_record_lp_hz,
              "shift-playback-lp-hz": args.shift_playback_lp_hz}
    shifted = any(shifts.values())
    if shifted and not (args.emit_fixture or args.check):
        ap.error("edge shifts are only allowed with --emit-fixture or --check")
    if args.family != "cheby2" and not args.check:
        ap.error("--family ellip is for comparison tables only (--check)")

    designs = []
    any_failed = False
    for t in (RECORD, PLAYBACK):
        hp_fc = HP_FC + args.shift_record_hp_hz
        edge, note = search_edge(t, args.family, HP_FC)  # the search runs on the unshifted design
        shift = args.shift_record_lp_hz if t.has_highpass else args.shift_playback_lp_hz
        sos = assemble(t, args.family, edge + shift, hp_fc if t.has_highpass else HP_FC)
        if shift or (t.has_highpass and args.shift_record_hp_hz):
            note += f"; FIXTURE: lp edge {edge + shift:.0f} Hz, hp {hp_fc:g} Hz"
        rows = verify(t, sos)
        width = max(len(r[0]) for r in rows)
        print(f"\n{t.name} — {args.family} — {note}")
        for prop, target, measured, ok in rows:
            print(f"  {prop:<{width}}  {target:<30} {measured:<14} {'PASS' if ok else 'FAIL'}")
        failed = [r for r in rows if not r[3]]
        any_failed |= bool(failed)
        designs.append((t, sos, note, rows))

    if args.emit_fixture:
        args.emit_fixture.write_text(render_file(designs, args.family, shifts, fixture=True))
        print(f"\nwrote fixture {args.emit_fixture}")
        return 0  # a shifted design is expected to fail its template
    if any_failed:
        print("\nFAILED: a template bound is not met; nothing written.", file=sys.stderr)
        return 1
    if not args.check:
        args.output.write_text(render_file(designs, args.family, shifts, fixture=False))
        print(f"\nwrote {args.output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
