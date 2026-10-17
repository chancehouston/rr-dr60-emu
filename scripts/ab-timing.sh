#!/usr/bin/env bash
# The cost A/B of two builds (spec 011 FR-009, research R-08; spec 012 FR-011, research R-09).
#
# Builds the release `timing` test binary of a baseline revision (a temporary git worktree) and
# of the working tree, then runs the baseline's `output_cost` (spec 009: the codec-bypassed
# one-pass at 48 kHz, 4096-sample blocks, a constant volume) and the working tree's HEAD_TEST
# alternately nine times each and prints, per route, the fastest figure of each build and their
# ratio. Two modes:
#   HEAD_TEST = output_cost (default): the same configuration in both builds. In the working tree
#     `default_output` is built on 0.11's default (stages 1 – 2 bypassed), so this measures what
#     the bypassed code costs: spec 012 FR-011's budget is 3 %.
#   HEAD_TEST = output_cost_mic: the working tree with stages 1 – 2 on against the baseline
#     (reported only: stage 2 changes the level the later stages see; the test asserts the
#     limiter stays uncharged).
#
# Usage: scripts/ab-timing.sh [baseline-rev [head-test]]
#   (default baseline c9f9994, version 0.11, the spec 012 branch point; default head test
#   output_cost)
# Run solo: the timing tests measure wall-clock cost.
set -euo pipefail

baseline="${1:-c9f9994}"
head_test="${2:-output_cost}"
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

run() { # the output-cost lines of test $2 in binary $1
  "$1" --ignored --exact "$2" --nocapture --test-threads=1 2>/dev/null | grep -o 'output chain, .*'
}

: >"$work/base.txt"
: >"$work/head.txt"
for i in $(seq 1 "$rounds"); do
  echo "round $i of $rounds" >&2
  run "$base_bin" output_cost >>"$work/base.txt"
  run "$head_bin" "$head_test" >>"$work/head.txt"
done

python3 - "$work/base.txt" "$work/head.txt" "$baseline" "$head_test" <<'EOF'
import re, sys
def best(path):
    out = {}
    for line in open(path):
        m = re.search(r"output chain, (\w+) route, 48 kHz, 4096 blocks: ([\d.]+) ns per host sample with the chain", line)
        if m:
            out[m.group(1)] = min(out.get(m.group(1), float("inf")), float(m.group(2)))
    return out
base, head, rev, test = best(sys.argv[1]), best(sys.argv[2]), sys.argv[3], sys.argv[4]
budget = "012 FR-011: budget 3 %" if test == "output_cost" else "012 FR-011: reported"
for route in ("Speaker", "Earphone"):
    b, h = base[route], head[route]
    print(f"A/B {test} vs {rev}'s output_cost, {route} route, 48 kHz, 4096 blocks, constant volume: "
          f"{rev} {b:.1f} ns, working tree {h:.1f} ns per host sample, {h - b:+.2f} ns "
          f"({100 * (h / b - 1):+.2f} %) ({budget}; fastest of nine alternating runs each)")
EOF
