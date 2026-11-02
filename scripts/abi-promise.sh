#!/usr/bin/env bash
# Spec 023 US2 AS6 (FR-008; research R-10): the 0.22 ABI promise exercised with an oracle.
#
# Compiles revision REV's C smoke program against REV's header and links it against the
# **current** static library, then runs it: the default run must exit 0 (its CHECKs pass: the
# ABI check with REV's sizes, every status and define it knows), or, since 0.92 (spec 024 plan
# R8), exit 1 having completed with its summary line and with exactly the tolerated failures
# below; and each `--x` mode's output must be byte-identical to the current smoke program's
# (the data printed is the library's).
# A caller compiled against REV's header keeps working with every later library of the same ABI
# version (spec 022 D4's promise; the first exercise is 0.23, which appends a control, functions,
# two defines and a status).
#
# Usage: scripts/abi-promise.sh [rev]   (default 12a3c45, version 0.22's merge)
set -euo pipefail
rev="${1:-12a3c45}"
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/include"
# A shallow checkout (CI) may not hold the revision: fetch it alone (a fetch needs the full
# object id, so CI passes the 40-character form; locally the short form resolves).
git cat-file -e "$rev^{commit}" 2>/dev/null || git fetch --depth=1 origin "$rev"
git show "$rev:crates/rr_dr60_ffi/include/rr_dr60.h" > "$work/include/rr_dr60.h"
git show "$rev:crates/rr_dr60_ffi/tests/c/smoke.c" > "$work/smoke.c"
cargo build -p rr_dr60_ffi --release
native_libs="$(cargo rustc --color never -p rr_dr60_ffi --release --lib --crate-type staticlib -- --print native-static-libs 2>&1 \
  | sed -n 's/.*native-static-libs: //p' | tail -1)"
cc -std=c11 -Wall -Wextra -Werror -O2 -I "$work/include" \
  "$work/smoke.c" target/release/librr_dr60_ffi.a $native_libs -lm -o "$work/smoke-old"
cc -std=c11 -Wall -Wextra -Werror -O2 -I crates/rr_dr60_ffi/include \
  crates/rr_dr60_ffi/tests/c/smoke.c target/release/librr_dr60_ffi.a $native_libs -lm -o "$work/smoke-new"
echo "abi-promise: $rev's smoke (against $rev's header) linked to the current library: the default run"
# Spec 024 (plan R8): appending an enum value is compatible by design, so a CHECK of REV's that
# asserts an unassigned value is refused may fail once a later library assigns it. Exactly
# these are tolerated, each named: REV's "tap = 11" (Tap::AfterInsert, appended in 0.92) and
# its knock-on "struct_size = 4" (the create before it succeeded, so the sentinel handle moved).
# Every other failure fails the promise.
set +e
"$work/smoke-old" > "$work/default.txt" 2>&1
status=$?
set -e
cat "$work/default.txt"
check_default() {
  # $1: the default run's output file, $2: its exit status. 0 when acceptable.
  [ "$2" -eq 0 ] && return 0
  [ "$2" -eq 1 ] || return 1
  local summary fails n listed
  summary="$(grep -E '^smoke: [0-9]+ failure\(s\)$' "$1" | tail -1)"
  [ -n "$summary" ] || return 1   # the summary prints only at the end: the run completed
  n="$(printf '%s' "$summary" | sed -E 's/^smoke: ([0-9]+) failure.*/\1/')"
  fails="$(grep '^smoke: FAIL ' "$1" | sed 's/^smoke: FAIL [^ ]*: //' || true)"
  listed="$(printf '%s\n' "$fails" | grep -c -x -e 'tap = 11' -e 'struct_size = 4' || true)"
  printf '%s\n' "$fails" | grep -q -x 'tap = 11' || return 1
  [ "$n" -eq "$listed" ] || return 1
  [ "$(printf '%s\n' "$fails" | grep -c . )" -eq "$listed" ] || return 1
  return 0
}
if [ "${ABI_PROMISE_SELF_TEST:-0}" = "1" ]; then
  # The filter's own check (coach post-implementation 1): canned outputs.
  t="$work/t.txt"
  printf 'smoke: FAIL x.c:1: tap = 11\nsmoke: FAIL x.c:2: struct_size = 4\nsmoke: 2 failure(s)\n' > "$t"; check_default "$t" 1 || { echo "self-test: the tolerated pair was refused" >&2; exit 1; }
  printf 'smoke: FAIL x.c:1: tap = 11\nsmoke: FAIL x.c:3: other\nsmoke: 2 failure(s)\n' > "$t"; ! check_default "$t" 1 || { echo "self-test: an extra failure passed" >&2; exit 1; }
  printf 'smoke: FAIL x.c:1: tap = 11\n' > "$t"; ! check_default "$t" 1 || { echo "self-test: a missing summary passed" >&2; exit 1; }
  printf 'smoke: FAIL x.c:1: tap = 11\nsmoke: 1 failure(s)\n' > "$t"; ! check_default "$t" 139 || { echo "self-test: a crash passed" >&2; exit 1; }
  echo "abi-promise: the filter's self-test passed"
fi
if ! check_default "$work/default.txt" "$status"; then
  echo "abi-promise: $rev's default run failed (status $status)" >&2
  exit 1
fi
if [ "$status" -ne 0 ]; then
  echo "abi-promise: $rev's default run: only the appended tap's checks differ (tap = 11, struct_size = 4; plan R8)"
fi
# Every mode REV's smoke knows: byte-identical to the current smoke's output.
for mode in --manifest --controls --meters --changes --record-changes --battery --bypass --record --abi; do
  "$work/smoke-old" "$mode" > "$work/old.txt"
  "$work/smoke-new" "$mode" > "$work/new.txt"
  if ! diff "$work/old.txt" "$work/new.txt" > "$work/diff.txt"; then
    echo "abi-promise: $mode differs between $rev's smoke and the current one:" >&2
    head -20 "$work/diff.txt" >&2
    exit 1
  fi
  echo "abi-promise: $mode byte-identical ($(wc -l < "$work/old.txt" | tr -d ' ') lines)"
done
echo "abi-promise: OK ($rev's smoke against the current library: exit 0, every mode byte-identical)"
