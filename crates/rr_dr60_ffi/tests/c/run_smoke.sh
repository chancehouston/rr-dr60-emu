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
# Spec 007 SC-001: the C program's manifest print equals the Rust example's, byte for byte.
./target/smoke --manifest > target/smoke-manifest.txt
cargo run -q -p rr_dr60_harness --example manifest > target/example-manifest.txt
diff target/smoke-manifest.txt target/example-manifest.txt
echo "smoke --manifest matches the Rust example"
# Spec 014 SC-004: the C program's control-manifest print equals the Rust example's, byte for
# byte (f32 values in the shortest round-trip form on both sides).
./target/smoke --controls > target/smoke-controls.txt
cargo run -q -p rr_dr60_harness --example control_manifest > target/example-controls.txt
diff target/smoke-controls.txt target/example-controls.txt
echo "smoke --controls matches the Rust example"
# Spec 016 SC-004: the C program's meter rows equal the Rust example's, byte for byte (floats as
# bit patterns on both sides).
./target/smoke --meters > target/smoke-meters.txt
cargo run -q -p rr_dr60_harness --example meters_rows -- --stimulus clamp_square --config meters_default_clamp --rate 48000 > target/example-meters.txt
diff target/smoke-meters.txt target/example-meters.txt
echo "smoke --meters matches the Rust example"
