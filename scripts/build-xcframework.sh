#!/usr/bin/env bash
# Builds RRDR60C.xcframework from the C API crate (rr_dr60_ffi) for the Swift package in swift/.
#
# Slices: iOS device (arm64), iOS simulator (arm64 + x86_64) and macOS (arm64, so `swift test`
# runs on an Apple-silicon Mac). Each slice is the release static library; the headers are
# include/rr_dr60.h plus a module map that names the C module RRDR60C.
#
# Prerequisites: Xcode, and the Rust targets from rust-toolchain.toml plus aarch64-apple-darwin
# (the host target on an Apple-silicon Mac).
#
# Usage: scripts/build-xcframework.sh
# Output: swift/RRDR60C.xcframework (ignored by git; rebuild after any change to the library).
set -euo pipefail
cd "$(dirname "$0")/.."

# The oldest OS versions the library is built for (the Swift package declares the same).
export IPHONEOS_DEPLOYMENT_TARGET=16.0
export MACOSX_DEPLOYMENT_TARGET=13.0

targets=(aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios aarch64-apple-darwin)
for t in "${targets[@]}"; do
  cargo build -q --release -p rr_dr60_ffi --target "$t"
done

work="target/xcframework"
rm -rf "$work"
mkdir -p "$work/headers" "$work/ios-simulator"
cp crates/rr_dr60_ffi/include/rr_dr60.h "$work/headers/"
cat > "$work/headers/module.modulemap" <<'EOF'
module RRDR60C {
    header "rr_dr60.h"
    export *
}
EOF

lib=librr_dr60_ffi.a
lipo -create \
  "target/aarch64-apple-ios-sim/release/$lib" \
  "target/x86_64-apple-ios/release/$lib" \
  -output "$work/ios-simulator/$lib"

out="swift/RRDR60C.xcframework"
rm -rf "$out"
xcodebuild -create-xcframework \
  -library "target/aarch64-apple-ios/release/$lib" -headers "$work/headers" \
  -library "$work/ios-simulator/$lib" -headers "$work/headers" \
  -library "target/aarch64-apple-darwin/release/$lib" -headers "$work/headers" \
  -output "$out" >/dev/null
echo "build-xcframework: $out (rr_dr60 $(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1), $(git rev-parse --short HEAD))"
