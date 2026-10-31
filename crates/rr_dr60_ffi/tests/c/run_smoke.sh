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
# Spec 015 SC-003: the C program's scheduled run (entries and output digests) equals the Rust
# example's, byte for byte.
./target/smoke --changes > target/smoke-changes.txt
cargo run -q -p rr_dr60_harness --example changes_rows > target/example-changes.txt
diff target/smoke-changes.txt target/example-changes.txt
echo "smoke --changes matches the Rust example"
# Spec 017 SC-003: the C program's scheduled recording and one-pass with record-side changes
# (entries, the final position and the digests) equal the Rust example's, byte for byte.
./target/smoke --record-changes > target/smoke-record-changes.txt
cargo run -q -p rr_dr60_harness --example record_changes_rows > target/example-record-changes.txt
diff target/smoke-record-changes.txt target/example-record-changes.txt
echo "smoke --record-changes matches the Rust example"
# Spec 018 SC-004: the C program's battery rows (entries, the two battery meters per block, the
# digests, the law and the trajectory as bit patterns) equal the Rust example's, byte for byte.
./target/smoke --battery > target/smoke-battery.txt
cargo run -q -p rr_dr60_harness --example battery_rows > target/example-battery.txt
diff target/smoke-battery.txt target/example-battery.txt
echo "smoke --battery matches the Rust example"
# Spec 021 FR-011: the C program's bypass rows (the entries per block, the output digest, the
# level function's four cells) are byte-identical to the Rust example.
./target/smoke --bypass > target/smoke-bypass.txt
cargo run -q -p rr_dr60_harness --example bypass_rows > target/example-bypass.txt
diff target/smoke-bypass.txt target/example-bypass.txt
echo "smoke --bypass matches the Rust example"
# Spec 020 (contracts/api.md): the C program's processing-record rows (the record's text, the
# read-out, the rewrite, the digest check, the three maps, the feed-point rule) against the Rust
# example `record_rows`.
./target/smoke --record > target/smoke-record.txt
cargo run -q -p rr_dr60_harness --example record_rows > target/example-record.txt
diff target/smoke-record.txt target/example-record.txt
echo "smoke --record matches the Rust example"
# Spec 022 SC-001: the C program's ABI rows (the size table, the defines and one row per struct
# the library fills, with the read-back size) equal the Rust example's, byte for byte.
./target/smoke --abi > target/smoke-abi.txt
cargo run -q -p rr_dr60_harness --example abi_rows > target/example-abi.txt
diff target/smoke-abi.txt target/example-abi.txt
echo "smoke --abi matches the Rust example"
