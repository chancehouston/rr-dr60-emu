#!/usr/bin/env bash
# Release gate (tasks.md T064; FR-014, SC-003): runs the golden-file (001, AGC, VAS, codec, output and
# output clamp; spec 010 added the last two, 009 having omitted golden_output here) and
# determinism tests on a physical iOS device, using cargo-dinghy.
#
# Prerequisites:
#   - Xcode, with an Apple development signing identity and a provisioning profile that
#     covers the connected device (cargo-dinghy signs the test bundle with it). Dinghy wants
#     a profile whose name ends in "Dinghy", and it does not embed the profile in the test
#     bundle, so the profile must also be installed on the device: build and install a
#     throwaway app with a bundle identifier ending in ".Dinghy" once with Xcode (automatic
#     signing; a free Personal Team's profile lasts 7 days, then repeat).
#   - An iPhone or iPad connected by USB or on the same network, unlocked, with this Mac
#     trusted and Developer Mode on.
#   - iOS 17 and later: pymobiledevice3 (`uv tool install pymobiledevice3`). Dinghy starts its
#     device tunnel with sudo, so every run asks for this Mac's password in the terminal; run
#     the script from your own terminal (a codesign keychain prompt: "Always Allow").
#   - rustup target add aarch64-apple-ios
#
# Usage: scripts/ios-device-golden.sh [device-id]   (default: the first connected physical
# device that devicectl lists)
# Record the PASS line (device model and iOS version) in the release checklist.
set -euo pipefail
cd "$(dirname "$0")/.."
# cargo-dinghy 0.8.6 passes an option pymobiledevice3 11 no longer has; see the shim.
export PATH="$PWD/scripts/dinghy-shim:$PATH"

if ! cargo dinghy --version >/dev/null 2>&1; then
  echo "Installing cargo-dinghy..."
  cargo install cargo-dinghy --locked
fi

device="${1:-}"
if [[ -z "$device" ]]; then
  echo "Devices seen by dinghy:"
  cargo dinghy all-devices
  device="$(xcrun devicectl list devices 2>/dev/null | awk '/physical/ && !/unavailable/ && /iPhone|iPad/ { for (i = 1; i <= NF; i++) if ($i == "(UDID)") { print $(i - 1); exit } }')"
fi
if [[ -z "$device" ]]; then
  echo "ios-device-golden: no physical iOS device found" >&2
  exit 1
fi

model="$(xcrun devicectl device info details --device "$device" 2>/dev/null | sed -n -E 's/.*(marketingName|Marketing Name): *//p' | head -1)"
os="$(xcrun devicectl device info details --device "$device" 2>/dev/null | sed -n -E 's/.*(osVersionNumber|OS Version): *//p' | head -1)"

# The golden file is embedded in the test binary, so nothing needs copying to the device.
if cargo dinghy -d "$device" test -p rr_dr60_harness --test golden --test golden_agc --test golden_vas --test golden_codec --test golden_output --test golden_output_clamp --test golden_volume_ramp --test golden_mic --test golden_speed --test determinism; then
  echo "ios-device-golden: PASS on ${model:-$device} (iOS ${os:-unknown}), rr_dr60 $(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1), $(git rev-parse --short HEAD)"
else
  echo "ios-device-golden: FAIL on ${model:-$device} (iOS ${os:-unknown})" >&2
  exit 1
fi
