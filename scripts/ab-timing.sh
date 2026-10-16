#!/usr/bin/env bash
# Spec 011 FR-009 (research R-08): the idle-cost A/B of two builds.
#
# Builds the release `timing` test binary of a baseline revision (a temporary git worktree) and
# of the working tree, then runs spec 009's `output_cost` (the codec-bypassed one-pass at 48 kHz,
# 4096-sample blocks, a constant volume) alternately nine times each and prints, per route, the
# fastest figure of each build and their ratio. The working tree's figure carries the volume
# transition's idle code (one branch and one store per sample); the budget is 3 %.
#
# Usage: scripts/ab-timing.sh [baseline-rev]   (default bd6d827, version 0.10)
# Run solo: the timing tests measure wall-clock cost.
set -euo pipefail

baseline="${1:-bd6d827}"
rounds=9
root="$(git rev-parse --show-toplevel)"
work="$(mktemp -d)"
trap 'git -C "$root" worktree remove --force "$work/base" >/dev/null 2>&1 || true; rm -rf "$work"' EXIT

git -C "$root" worktree add --detach "$work/base" "$baseline" >/dev/null

bin_of() { # the release timing test binary of the tree at $1
  (cd "$1" && cargo test -q -p rr_dr60_harness --release --test timing --no-run --message-format=json 2>/dev/null) |
    python3 -c 'import json,sys
for l in sys.stdin:
    try: m = json.loads(l)
    except ValueError: continue
    if m.get("reason") == "compiler-artifact" and m.get("target", {}).get("name") == "timing" and m.get("executable"):
        print(m["executable"])' | tail -1
}

echo "building the baseline ($baseline) and the working tree ..." >&2
base_bin="$(bin_of "$work/base")"
head_bin="$(bin_of "$root")"
[ -x "$base_bin" ] && [ -x "$head_bin" ] || { echo "ab-timing: a build failed" >&2; exit 1; }

run() { # the output_cost lines of one run of binary $1
  "$1" --ignored --exact output_cost --nocapture --test-threads=1 2>/dev/null | grep -o 'output chain, .*'
}

: >"$work/base.txt"
: >"$work/head.txt"
for i in $(seq 1 "$rounds"); do
  echo "round $i of $rounds" >&2
  run "$base_bin" >>"$work/base.txt"
  run "$head_bin" >>"$work/head.txt"
done

python3 - "$work/base.txt" "$work/head.txt" "$baseline" <<'EOF'
import re, sys
def best(path):
    out = {}
    for line in open(path):
        m = re.search(r"output chain, (\w+) route, 48 kHz, 4096 blocks: ([\d.]+) ns per host sample with the chain", line)
        if m:
            out[m.group(1)] = min(out.get(m.group(1), float("inf")), float(m.group(2)))
    return out
base, head, rev = best(sys.argv[1]), best(sys.argv[2]), sys.argv[3]
for route in ("Speaker", "Earphone"):
    b, h = base[route], head[route]
    print(f"idle A/B, {route} route, 48 kHz, 4096 blocks, constant volume: {rev} {b:.1f} ns, "
          f"working tree {h:.1f} ns per host sample, {h - b:+.2} ns ({100 * (h / b - 1):+.2} %) "
          f"(011 FR-009: budget 3 %; fastest of nine alternating runs each)")
EOF
