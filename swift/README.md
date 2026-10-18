# RRDR60 Swift package

The rr_dr60 C API (`crates/rr_dr60_ffi/include/rr_dr60.h`) as a Swift package, for iOS and macOS apps.

1. Build the binary (needs Xcode and the Rust targets in `rust-toolchain.toml` plus `aarch64-apple-darwin`):

   ```
   scripts/build-xcframework.sh
   ```

   It writes `swift/RRDR60C.xcframework` (not committed). Rebuild it after any change to the library.

2. Add the package to an app as a local package (this `swift/` directory) and `import RRDR60C`. The module is the C API itself: `rr_dr60_settings_default`, `rr_dr60_create`, `rr_dr60_process` and the rest, with the same real-time and ownership rules the header documents.

3. Test it (the Swift path must reproduce the Rust path bit for bit):

   ```
   cd swift && swift test
   xcodebuild test -scheme RRDR60 -destination 'platform=iOS Simulator,name=iPhone 17,OS=26.0'
   ```

Status codes: the functions return `Int32`, while Swift imports the `RR_DR60_STATUS_*` constants with the enum's own type, so compare with `status == Int32(RR_DR60_STATUS_OK.rawValue)`.

There is no Swift-native API yet; a wrapper (ownership, errors, buffers) would be its own feature.
