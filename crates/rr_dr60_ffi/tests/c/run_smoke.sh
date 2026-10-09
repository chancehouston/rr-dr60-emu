#!/usr/bin/env bash
# Builds the rr_dr60_ffi static library and runs the C smoke test against it (tasks.md T038).
# Usage (from anywhere): crates/rr_dr60_ffi/tests/c/run_smoke.sh
set -euo pipefail
root="$(cd "$(dirname "$0")/../../../.." && pwd)"
cd "$root"
cargo build -p rr_dr60_ffi --release
# Native libraries the Rust staticlib needs on this platform (e.g. -lSystem, -lpthread -ldl -lm).
native_libs="$(cargo rustc --color never -p rr_dr60_ffi --release --lib --crate-type staticlib -- --print native-static-libs 2>&1 \
  | sed -n 's/.*native-static-libs: //p' | tail -1)"
cc -std=c11 -Wall -Wextra -Werror -O2 -I crates/rr_dr60_ffi/include \
  crates/rr_dr60_ffi/tests/c/smoke.c target/release/librr_dr60_ffi.a $native_libs -lm -o target/smoke
./target/smoke
