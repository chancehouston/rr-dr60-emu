#!/usr/bin/env bash
# Spec 023 US2 AS6 (FR-008; research R-10): the 0.22 ABI promise exercised with an oracle.
#
# Compiles revision REV's C smoke program against REV's header and links it against the
# **current** static library, then runs it: the default run must exit 0 (its CHECKs pass: the
# ABI check with REV's sizes, every status and define it knows), and each `--x` mode's output
# must be byte-identical to the current smoke program's (the data printed is the library's).
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
# A shallow checkout (CI) may not hold the revision: fetch it alone.
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
"$work/smoke-old"
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
