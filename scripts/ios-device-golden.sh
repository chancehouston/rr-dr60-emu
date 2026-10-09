#!/usr/bin/env bash
# Release gate (tasks.md T064; FR-014, SC-003): runs the golden-file and determinism tests on
# a physical iOS device, using cargo-dinghy.
#
# Prerequisites:
#   - Xcode, with an Apple development signing identity and a provisioning profile that
#     covers the connected device (cargo-dinghy signs the test bundle with it).
#   - An iPhone or iPad connected by USB or on the same network, unlocked, with this Mac
#     trusted and Developer Mode on.
#   - rustup target add aarch64-apple-ios
#
# Usage: scripts/ios-device-golden.sh [device-id]   (default: the first device dinghy lists)
# Record the PASS line (device model and iOS version) in the release checklist.
set -euo pipefail
cd "$(dirname "$0")/.."

if ! cargo dinghy --version >/dev/null 2>&1; then
  echo "Installing cargo-dinghy..."
  cargo install cargo-dinghy --locked
fi

device="${1:-}"
if [[ -z "$device" ]]; then
  echo "Devices seen by dinghy:"
  cargo dinghy all-devices
  device="$(cargo dinghy all-devices 2>/dev/null | sed -n 's/.*IosDevice.*id: *\([0-9A-Fa-f-]*\).*/\1/p' | head -1)"
fi
if [[ -z "$device" ]]; then
  echo "ios-device-golden: no physical iOS device found" >&2
  exit 1
fi

model="$(xcrun devicectl device info details --device "$device" 2>/dev/null | sed -n 's/.*marketingName: *//p' | head -1)"
os="$(xcrun devicectl device info details --device "$device" 2>/dev/null | sed -n 's/.*osVersionNumber: *//p' | head -1)"

# The golden file is embedded in the test binary, so nothing needs copying to the device.
if cargo dinghy -d "$device" test -p rr_dr60_harness --test golden --test golden_agc --test determinism; then
  echo "ios-device-golden: PASS on ${model:-$device} (iOS ${os:-unknown}), rr_dr60 $(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1), $(git rev-parse --short HEAD)"
else
  echo "ios-device-golden: FAIL on ${model:-$device} (iOS ${os:-unknown})" >&2
  exit 1
fi
