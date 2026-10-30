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
#     (at most 10 % as corroboration of the in-build figure, not asserted: stage 2 changes the
#     level the later stages see; the test asserts the limiter stays uncharged).
#
#   HEAD_TEST = playback: spec 013's playback at the default speed level against the baseline
#     (013 FR-010: budget 3 %); `tests/playback_timing.rs` is copied into the baseline worktree.
#
#   HEAD_TEST = changes: spec 015's sample-timed changes against the baseline (015 FR-010: the
#     player and the one-pass with the schedule empty, 3 %; RR_DR60_CHANGES_COST_CEILING on the
#     runner; one change per block reported).
#   HEAD_TEST = record: spec 017's live record-side values against the baseline (017 FR-009: the
#     record path and the one-pass with the record schedule empty, 3 %;
#     RR_DR60_RECORD_COST_CEILING on the runner; one knee change per block and the worst block
#     reported).
#   HEAD_TEST = meters: spec 016's per-block meters against the baseline (016 FR-013: the default
#     front within 5 %, every stage off within the same absolute nanoseconds; the ceiling from
#     RR_DR60_METERS_COST_CEILING, 0.12 on the CI runner); asserted, exits 1 when over.
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

if [ "$head_test" = "playback" ]; then
  # Spec 013 FR-010 (research R-07): the playback at the default speed level against the
  # baseline. The test is written against 0.12's public API, so it is copied into the baseline
  # worktree; each build's `playback_timing` binary runs 45 times, strictly alternating, and the
  # fastest of each is compared (budget 3 % on the development machine, 5 % on the CI runner).
  cp "$root/crates/rr_dr60_harness/tests/playback_timing.rs" "$work/base/crates/rr_dr60_harness/tests/"
  pbin_of() {
    (cd "$1" && cargo test -q -p rr_dr60_harness --release --test playback_timing --no-run --message-format=json 2>/dev/null) |
      python3 -c 'import json,sys
for l in sys.stdin:
    try: m = json.loads(l)
    except ValueError: continue
    if m.get("reason") == "compiler-artifact" and m.get("target", {}).get("name") == "playback_timing" and m.get("executable"):
        print(m["executable"])' | tail -1
  }
  echo "building the baseline ($baseline) and the working tree (playback) ..." >&2
  base_bin="$(pbin_of "$work/base")"
  head_bin="$(pbin_of "$root")"
  [ -x "$base_bin" ] && [ -x "$head_bin" ] || { echo "ab-timing: a build failed" >&2; exit 1; }
  : >"$work/base.txt"
  : >"$work/head.txt"
  for i in $(seq 1 45); do
    echo "round $i of 45" >&2
    "$base_bin" --ignored --exact playback_cost --nocapture --test-threads=1 2>/dev/null | grep -o 'playback, .*' >>"$work/base.txt"
    "$head_bin" --ignored --exact playback_cost --nocapture --test-threads=1 2>/dev/null | grep -o 'playback, .*' >>"$work/head.txt"
  done
  python3 - "$work/base.txt" "$work/head.txt" "$baseline" <<'PY'
import re, sys
def best(path):
    v = [float(m.group(1)) for l in open(path) for m in [re.search(r"blocks: ([\d.]+) ns per host sample", l)] if m]
    return min(v)
b, h, rev = best(sys.argv[1]), best(sys.argv[2]), sys.argv[3]
print(f"A/B playback at the default speed level vs {rev}, Speaker route, 48 kHz, 4096 blocks, codec bypassed: "
      f"{rev} {b:.2f} ns, working tree {h:.2f} ns per host sample, {h - b:+.2f} ns ({100 * (h / b - 1):+.2f} %) "
      f"(013 FR-010: budget 3 %, CI ceiling 5 %; fastest of 45 alternating runs each)")
PY
  exit 0
fi

if [ "$head_test" = "meters" ]; then
  # Spec 016 FR-013 (research R-09): the meters' cost. `tests/meters_timing.rs` is written
  # against 0.14's public API and copied into the baseline worktree; each build prints the
  # fastest-of-45 figure of the default front (stages 1 – 2 on, the codec bypassed) and of every
  # stage off. Asserted: the front within the ceiling (5 % by default, RR_DR60_METERS_COST_CEILING
  # on the CI runner: 0.12) and every stage off within the same absolute nanoseconds; exits 1
  # when over.
  cp "$root/crates/rr_dr60_harness/tests/meters_timing.rs" "$work/base/crates/rr_dr60_harness/tests/"
  mbin_of() {
    (cd "$1" && cargo test -q -p rr_dr60_harness --release --test meters_timing --no-run --message-format=json 2>/dev/null) |
      python3 -c 'import json,sys
for l in sys.stdin:
    try: m = json.loads(l)
    except ValueError: continue
    if m.get("reason") == "compiler-artifact" and m.get("target", {}).get("name") == "meters_timing" and m.get("executable"):
        print(m["executable"])' | tail -1
  }
  echo "building the baseline ($baseline) and the working tree (meters) ..." >&2
  base_bin="$(mbin_of "$work/base")"
  head_bin="$(mbin_of "$root")"
  [ -x "$base_bin" ] && [ -x "$head_bin" ] || { echo "ab-timing: a build failed" >&2; exit 1; }
  : >"$work/base.txt"
  : >"$work/head.txt"
  # Each binary already takes the fastest of 45 runs per figure; the two builds alternate three
  # times and the fastest of those is compared (run-order sensitivity, 012 R-09).
  for i in 1 2 3; do
    echo "round $i of 3" >&2
    "$base_bin" --ignored --exact meters_cost --nocapture --test-threads=1 2>/dev/null | grep -oE '(default_front_ns|every_stage_off_ns)=[0-9.]+' >>"$work/base.txt"
    "$head_bin" --ignored --exact meters_cost --nocapture --test-threads=1 2>/dev/null | grep -oE '(default_front_ns|every_stage_off_ns)=[0-9.]+' >>"$work/head.txt"
  done
  python3 - "$work/base.txt" "$work/head.txt" "$baseline" "${RR_DR60_METERS_COST_CEILING:-0.05}" <<'PY'
import re, sys
def best(path):
    out = {}
    for line in open(path):
        m = re.search(r"(\w+)=([\d.]+)", line.strip())
        if m:
            out[m.group(1)] = min(out.get(m.group(1), float("inf")), float(m.group(2)))
    return out
b, h, rev, ceiling = best(sys.argv[1]), best(sys.argv[2]), sys.argv[3], float(sys.argv[4])
bf, hf = b["default_front_ns"], h["default_front_ns"]
bo, ho = b["every_stage_off_ns"], h["every_stage_off_ns"]
budget_ns = ceiling * bf
front_ok = hf / bf - 1 <= ceiling
off_ok = ho - bo <= budget_ns
print(f"A/B meters vs {rev}, 48 kHz, 4096 blocks, speaker route, codec bypassed (016 FR-013; fastest of 45 runs × 3 alternations):")
print(f"  default front (stages 1 – 2 on): {rev} {bf:.2f} ns, working tree {hf:.2f} ns per host sample, {hf - bf:+.2f} ns ({100 * (hf / bf - 1):+.2f} %; ceiling {100 * ceiling:.0f} %) {'PASS' if front_ok else 'FAIL'}")
print(f"  every stage off: {rev} {bo:.2f} ns, working tree {ho:.2f} ns per host sample, {ho - bo:+.2f} ns (budget {budget_ns:.2f} ns, the same absolute nanoseconds) {'PASS' if off_ok else 'FAIL'}")
sys.exit(0 if front_ok and off_ok else 1)
PY
  exit $?
fi

if [ "$head_test" = "record" ]; then
  # Spec 017 FR-009 (research R-11): the record-side values' idle cost. `tests/record_timing.rs`
  # is written against 0.16's public API and copied into the baseline worktree; each build prints
  # the fastest-of-45 figure of the record path and of the one-pass (the default front, the codec
  # bypassed, the record schedule empty). Asserted: both within the ceiling (3 % by default,
  # RR_DR60_RECORD_COST_CEILING on the CI runner: 0.05); exits 1 when over. Then the head's
  # `record_pending_timing` (one knee change per block; the worst block) is run alone and reported.
  cp "$root/crates/rr_dr60_harness/tests/record_timing.rs" "$work/base/crates/rr_dr60_harness/tests/"
  rbin_of() {
    (cd "$1" && cargo test -q -p rr_dr60_harness --release --test "$2" --no-run --message-format=json 2>/dev/null) |
      python3 -c 'import json,sys
name = sys.argv[1]
for l in sys.stdin:
    try: m = json.loads(l)
    except ValueError: continue
    if m.get("reason") == "compiler-artifact" and m.get("target", {}).get("name") == name and m.get("executable"):
        print(m["executable"])' "$2" | tail -1
  }
  echo "building the baseline ($baseline) and the working tree (record) ..." >&2
  base_bin="$(rbin_of "$work/base" record_timing)"
  head_bin="$(rbin_of "$root" record_timing)"
  pending_bin="$(rbin_of "$root" record_pending_timing)"
  [ -x "$base_bin" ] && [ -x "$head_bin" ] && [ -x "$pending_bin" ] || { echo "ab-timing: a build failed" >&2; exit 1; }
  : >"$work/base.txt"
  : >"$work/head.txt"
  for i in 1 2 3; do
    echo "round $i of 3" >&2
    "$base_bin" --ignored --exact record_cost --nocapture --test-threads=1 2>/dev/null | grep -oE '(record_ns|one_pass_ns)=[0-9.]+' >>"$work/base.txt"
    "$head_bin" --ignored --exact record_cost --nocapture --test-threads=1 2>/dev/null | grep -oE '(record_ns|one_pass_ns)=[0-9.]+' >>"$work/head.txt"
  done
  pending="$("$pending_bin" --ignored --exact record_pending_cost --nocapture --test-threads=1 2>/dev/null | grep -oE '(record_pending_ns|worst_block_us)=[0-9.]+' | tr '\n' ' ')"
  python3 - "$work/base.txt" "$work/head.txt" "$baseline" "${RR_DR60_RECORD_COST_CEILING:-0.03}" "$pending" <<'PY'
import re, sys
def best(path):
    out = {}
    for line in open(path):
        m = re.search(r"(\w+)=([\d.]+)", line.strip())
        if m:
            out[m.group(1)] = min(out.get(m.group(1), float("inf")), float(m.group(2)))
    return out
b, h, rev, ceiling, pending = best(sys.argv[1]), best(sys.argv[2]), sys.argv[3], float(sys.argv[4]), sys.argv[5]
br, hr = b["record_ns"], h["record_ns"]
bo, ho = b["one_pass_ns"], h["one_pass_ns"]
r_ok = hr / br - 1 <= ceiling
o_ok = ho / bo - 1 <= ceiling
pend = dict(re.findall(r"(\w+)=([\d.]+)", pending))
pn = float(pend.get("record_pending_ns", "nan"))
wb = float(pend.get("worst_block_us", "nan"))
print(f"A/B record vs {rev}, 48 kHz, 4096 blocks, the default front, codec bypassed, the record schedule empty (017 FR-009; fastest of 45 runs × 3 alternations):")
print(f"  record path: {rev} {br:.2f} ns, working tree {hr:.2f} ns per host sample, {hr - br:+.2f} ns ({100 * (hr / br - 1):+.2f} %; ceiling {100 * ceiling:.0f} %) {'PASS' if r_ok else 'FAIL'}")
print(f"  one-pass: {rev} {bo:.2f} ns, working tree {ho:.2f} ns per host sample, {ho - bo:+.2f} ns ({100 * (ho / bo - 1):+.2f} %; ceiling {100 * ceiling:.0f} %) {'PASS' if o_ok else 'FAIL'}")
print(f"  record path with one knee change per block (reported): {pn:.2f} ns per host sample, {pn - hr:+.2f} ns ({100 * (pn / hr - 1):+.2f} %) against the empty schedule; worst 4096-sample block {wb:.1f} µs of 85333")
sys.exit(0 if r_ok and o_ok else 1)
PY
  exit $?
fi

if [ "$head_test" = "changes" ]; then
  # Spec 015 FR-010 (research R-09): the sample-timed changes' idle cost. `tests/changes_timing.rs`
  # is written against 0.15's public API and copied into the baseline worktree; each build prints
  # the fastest-of-45 figure of the player and of the one-pass (the default front, the codec
  # bypassed, the schedule empty). Asserted: both within the ceiling (3 % by default,
  # RR_DR60_CHANGES_COST_CEILING on the CI runner: 0.05); exits 1 when over. Then the head's
  # `changes_pending_timing` (one change per block) is run alone and reported.
  cp "$root/crates/rr_dr60_harness/tests/changes_timing.rs" "$work/base/crates/rr_dr60_harness/tests/"
  cbin_of() {
    (cd "$1" && cargo test -q -p rr_dr60_harness --release --test "$2" --no-run --message-format=json 2>/dev/null) |
      python3 -c 'import json,sys
name = sys.argv[1]
for l in sys.stdin:
    try: m = json.loads(l)
    except ValueError: continue
    if m.get("reason") == "compiler-artifact" and m.get("target", {}).get("name") == name and m.get("executable"):
        print(m["executable"])' "$2" | tail -1
  }
  echo "building the baseline ($baseline) and the working tree (changes) ..." >&2
  base_bin="$(cbin_of "$work/base" changes_timing)"
  head_bin="$(cbin_of "$root" changes_timing)"
  pending_bin="$(cbin_of "$root" changes_pending_timing)"
  [ -x "$base_bin" ] && [ -x "$head_bin" ] && [ -x "$pending_bin" ] || { echo "ab-timing: a build failed" >&2; exit 1; }
  : >"$work/base.txt"
  : >"$work/head.txt"
  for i in 1 2 3; do
    echo "round $i of 3" >&2
    "$base_bin" --ignored --exact changes_cost --nocapture --test-threads=1 2>/dev/null | grep -oE '(playback_ns|one_pass_ns)=[0-9.]+' >>"$work/base.txt"
    "$head_bin" --ignored --exact changes_cost --nocapture --test-threads=1 2>/dev/null | grep -oE '(playback_ns|one_pass_ns)=[0-9.]+' >>"$work/head.txt"
  done
  pending="$("$pending_bin" --ignored --exact changes_pending_cost --nocapture --test-threads=1 2>/dev/null | grep -oE 'playback_pending_ns=[0-9.]+' | tail -1)"
  python3 - "$work/base.txt" "$work/head.txt" "$baseline" "${RR_DR60_CHANGES_COST_CEILING:-0.03}" "$pending" <<'PY'
import re, sys
def best(path):
    out = {}
    for line in open(path):
        m = re.search(r"(\w+)=([\d.]+)", line.strip())
        if m:
            out[m.group(1)] = min(out.get(m.group(1), float("inf")), float(m.group(2)))
    return out
b, h, rev, ceiling, pending = best(sys.argv[1]), best(sys.argv[2]), sys.argv[3], float(sys.argv[4]), sys.argv[5]
bp, hp = b["playback_ns"], h["playback_ns"]
bo, ho = b["one_pass_ns"], h["one_pass_ns"]
p_ok = hp / bp - 1 <= ceiling
o_ok = ho / bo - 1 <= ceiling
pend = float(re.search(r"=([\d.]+)", pending).group(1)) if pending else float("nan")
print(f"A/B changes vs {rev}, 48 kHz, 4096 blocks, speaker route, codec bypassed, the schedule empty (015 FR-010; fastest of 45 runs × 3 alternations):")
print(f"  playback: {rev} {bp:.2f} ns, working tree {hp:.2f} ns per host sample, {hp - bp:+.2f} ns ({100 * (hp / bp - 1):+.2f} %; ceiling {100 * ceiling:.0f} %) {'PASS' if p_ok else 'FAIL'}")
print(f"  one-pass: {rev} {bo:.2f} ns, working tree {ho:.2f} ns per host sample, {ho - bo:+.2f} ns ({100 * (ho / bo - 1):+.2f} %; ceiling {100 * ceiling:.0f} %) {'PASS' if o_ok else 'FAIL'}")
print(f"  playback with one change per block (reported): {pend:.2f} ns per host sample, {pend - hp:+.2f} ns ({100 * (pend / hp - 1):+.2f} %) against the empty schedule")
sys.exit(0 if p_ok and o_ok else 1)
PY
  exit $?
fi

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
budget = "012 FR-011: budget 3 %" if test == "output_cost" else "012 FR-011: ≤ 10 % corroboration (not asserted)"
for route in ("Speaker", "Earphone"):
    b, h = base[route], head[route]
    print(f"A/B {test} vs {rev}'s output_cost, {route} route, 48 kHz, 4096 blocks, constant volume: "
          f"{rev} {b:.1f} ns, working tree {h:.1f} ns per host sample, {h - b:+.2f} ns "
          f"({100 * (h / b - 1):+.2f} %) ({budget}; fastest of nine alternating runs each)")
EOF
