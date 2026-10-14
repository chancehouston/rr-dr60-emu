# /// script
# requires-python = ">=3.10"
# ///
"""Calibration of the speech codec's tolerances against a documented reference coder (spec 008
research R-09; clarify Q5). Plan time only: never in CI, never at build or run time.

The reference is **ITU-T G.723.1 at 5.3 kbit/s** (S-008): an ACELP coder of the same family
and of the same bit density as the model (0.667 bit per sample). ffmpeg's G.723.1 *encoder*
refuses the 5.3 kbit/s mode (`g723_1enc.c`: "Use bitrate 6300 instead of 5300"; checked in the
source on 2026-10-12), so this tool uses the **ITU-T reference implementation**, the free
electronic attachment to Recommendation G.723.1 (05/2006), which the user downloads from
https://www.itu.int/rec/T-REC-G.723.1 and unpacks under `tools/celp-calibrate/ref/` (git-ignored;
nothing of it enters the repository). The tool builds its `lbccodec` with the system C compiler,
runs it at 5.3 kbit/s (`-r53`) on every `*.raw` stimulus (16-bit little-endian, 8000 Hz, written
by `cargo run -p rr_dr60_harness --example codec_stimuli -- --rate 8000 --out <dir>`), writes
`<name>.g723.raw` beside each, and cross-checks the decode with ffmpeg's G.723.1 *decoder* when
`ffmpeg` is on PATH. The harness then measures reference and model with the same metric code:
`cargo test -p rr_dr60_harness --release --test codec_matrix calibration -- --ignored --nocapture`.

Usage: uv run tools/celp-calibrate/calibrate.py <stimulus dir> [--ref tools/celp-calibrate/ref]
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path

HINT = (
    "Download the G.723.1 reference software (the free electronic attachment to ITU-T G.723.1\n"
    "(05/2006), 'Software' on https://www.itu.int/rec/T-REC-G.723.1), unzip it under\n"
    "tools/celp-calibrate/ref/ (any sub-folder; the tool looks for lbccodec.c or coder.c), and\n"
    "make sure a C compiler (cc) is on PATH. Without it the committed calibration.md stands and\n"
    "the spec's floors (±1.5 dB, ±3 dB, 4 dB) remain the minimum the model must meet."
)


def find_sources(ref: Path) -> Path | None:
    """The fixed-point sources (main body + Annex A, 'Release 2'; the files are upper-case in
    the ITU archive: LBCCODEC.C, CODER.C, …)."""
    hits = [p for p in ref.rglob("*") if p.is_file() and p.name.lower() == "lbccodec.c"]
    return hits[0].parent if hits else None


def build(src: Path, out: Path) -> Path:
    exe = out / "lbccodec"
    cc = shutil.which("cc") or shutil.which("gcc") or shutil.which("clang")
    if cc is None:
        sys.exit("error: no C compiler on PATH\n" + HINT)
    c_files = sorted(p for p in src.iterdir() if p.suffix.lower() == ".c")
    # The archive's files are upper-case (`.C` reads as C++ to clang: force C), the code is 1990s
    # C (gnu89), and TYPEDEF.H knows only old compilers: the `__alpha` branch gives the standard
    # 16/32-bit `short` and `int` types (the `__sun` branch makes Word32 a 64-bit `long` here, and the coder
    # then hangs).
    cmd = [cc, "-O2", "-w", "-x", "c", "-std=gnu89", "-D__alpha", "-o", str(exe)] + [str(p) for p in c_files] + ["-lm"]
    print("building:", " ".join(cmd[:5]), f"... ({len(c_files)} files)")
    subprocess.run(cmd, check=True)
    return exe


def run_codec(exe: Path, raw: Path) -> Path:
    """Encode then decode `raw` at 5.3 kbit/s. The reference `lbccodec` takes
    `-r53 <input.pcm> <output.pcm>` in one pass (encoder + decoder) for its 'both' mode; the
    exact flags differ between archive versions, so try the documented forms."""
    out = raw.with_suffix(".g723.raw")
    bits = raw.with_suffix(".g723.bit")
    # LBCCODEC.C: `-r53` selects 5.3 kbit/s, `-b` runs coder and decoder back to back ("Both",
    # 16-bit PCM in and out), `-c` writes the bitstream, `-d` decodes one; the VAD/CNG of Annex A
    # stays off (its default). The bitstream is also written so ffmpeg's decoder can cross-check.
    both = subprocess.run([str(exe), "-r53", "-b", str(raw), str(out)], capture_output=True, text=True)
    enc = subprocess.run([str(exe), "-r53", "-c", str(raw), str(bits)], capture_output=True, text=True)
    if both.returncode == 0 and out.exists() and out.stat().st_size > 0:
        return out
    dec = subprocess.run([str(exe), "-r53", "-d", str(bits), str(out)], capture_output=True, text=True)
    if enc.returncode == 0 and dec.returncode == 0 and out.exists():
        return out
    sys.exit(f"error: lbccodec failed on {raw.name}:\n{both.stderr}\n{enc.stderr}\n{dec.stderr}")


def ffmpeg_crosscheck(bits: Path, out: Path) -> str:
    ff = shutil.which("ffmpeg")
    if ff is None or not bits.exists():
        return "ffmpeg not on PATH or no bitstream file: decode cross-check skipped"
    alt = out.with_suffix(".ffdec.raw")
    r = subprocess.run([ff, "-y", "-loglevel", "error", "-f", "g723_1", "-i", str(bits), "-f", "s16le", "-ar", "8000", str(alt)],
                       capture_output=True, text=True)
    if r.returncode != 0:
        return f"ffmpeg decode failed: {r.stderr.strip()}"
    a, b = out.read_bytes(), alt.read_bytes()
    n = min(len(a), len(b))
    diff = sum(1 for i in range(0, n, 2) if a[i:i + 2] != b[i:i + 2])
    return f"ffmpeg decoder cross-check: {diff} of {n // 2} samples differ"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("stimuli", type=Path, help="directory of *.raw stimuli (s16le, 8000 Hz)")
    ap.add_argument("--ref", type=Path, default=Path(__file__).parent / "ref")
    args = ap.parse_args()
    src = find_sources(args.ref)
    if src is None:
        print(f"error: no G.723.1 reference sources under {args.ref}", file=sys.stderr)
        print(HINT, file=sys.stderr)
        return 2
    exe = build(src, args.ref)
    raws = sorted(p for p in args.stimuli.glob("*.raw") if ".g723" not in p.name and ".ffdec" not in p.name)
    if not raws:
        print(f"error: no *.raw stimuli in {args.stimuli}", file=sys.stderr)
        return 2
    print(f"{'stimulus':<28} {'bytes in':>10} {'bytes out':>10}  note")
    for raw in raws:
        out = run_codec(exe, raw)
        note = ffmpeg_crosscheck(raw.with_suffix(".g723.bit"), out)
        print(f"{raw.name:<28} {raw.stat().st_size:>10} {out.stat().st_size:>10}  {note}")
    print("done; now run: cargo test -p rr_dr60_harness --release --test codec_matrix calibration -- --ignored --nocapture")
    return 0


if __name__ == "__main__":
    sys.exit(main())
