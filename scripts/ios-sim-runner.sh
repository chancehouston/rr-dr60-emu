#!/usr/bin/env bash
# Cargo runner for aarch64-apple-ios-sim (tasks.md T062): runs a test binary inside an iOS
# simulator. Configured in .cargo/config.toml, so `cargo test --target aarch64-apple-ios-sim`
# just works on a Mac with Xcode and an iOS simulator runtime installed.
#
# Uses RR_DR60_SIM_UDID if set; otherwise the first available iPhone simulator. Boots it if
# needed (an already-booted device is fine) and passes the binary's exit code through.
# The simulator shares the host filesystem, so paths compiled into the test binary (such as
# the golden file via CARGO_MANIFEST_DIR) resolve normally.
set -euo pipefail

udid="${RR_DR60_SIM_UDID:-}"
if [[ -z "$udid" ]]; then
  udid="$(xcrun simctl list devices available | sed -n 's/^ *iPhone[^(]*(\([0-9A-F-]\{36\}\)).*/\1/p' | head -1)"
fi
if [[ -z "$udid" ]]; then
  echo "ios-sim-runner: no available iPhone simulator; install an iOS runtime with" >&2
  echo "  xcodebuild -downloadPlatform iOS" >&2
  exit 1
fi

xcrun simctl boot "$udid" 2>/dev/null || true # "already booted" is fine
xcrun simctl bootstatus "$udid" -b >/dev/null

exec xcrun simctl spawn "$udid" "$@"
