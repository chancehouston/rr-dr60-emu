# RRDR60 Swift package

The rr_dr60 C API (`crates/rr_dr60_ffi/include/rr_dr60.h`) as a Swift package, for iOS and macOS apps. The C ABI is 64-bit only (every iOS device the app can target; on Android `arm64-v8a` and `x86_64`).

1. Build the binary (needs Xcode and the Rust targets in `rust-toolchain.toml` plus `aarch64-apple-darwin`):

   ```
   scripts/build-xcframework.sh
   ```

   It writes `swift/RRDR60C.xcframework` (not committed). Rebuild it after any change to the library.

2. Add the package to an app as a local package (this `swift/` directory) and `import RRDR60` for the helpers (`.sized`, `RRDR60.checkABI()`, the processing record's re-render). `import RRDR60C` alone is the bare C API (`rr_dr60_settings_default`, `rr_dr60_create`, `rr_dr60_process` and the rest, with the same real-time and ownership rules the header documents), where every report must be sized by hand: `var info = RrDr60BlockInfo(); info.struct_size = UInt32(MemoryLayout<RrDr60BlockInfo>.size)`.

3. Test it (the Swift path must reproduce the Rust path bit for bit):

   ```
   cd swift && swift test
   xcodebuild test -scheme RRDR60 -destination 'platform=iOS Simulator,name=iPhone 17,OS=26.0'
   ```

**Every struct the library fills starts with its size** (since 0.22, the pre-1.0 C ABI pass): set `struct_size` to `sizeof` before the call. Swift's zero-initializer (`RrDr60BlockInfo()`) leaves it 0, which the library refuses with `RR_DR60_STATUS_INVALID_ARGUMENT` at run time, by design; use `RrDr60BlockInfo.sized` (the `RRDR60Sized` protocol, on every struct the library fills or reads except `RrDr60Settings`, which comes from `rr_dr60_settings_default`).

**Call `try RRDR60.checkABI()` once at launch.** It checks the header this package was compiled against the library it links (the ABI version, the pointer width, the settings struct's size and the two frozen structs' sizes). The library ships inside the app, so a mismatch is a build error of the app (a stale XCFramework, a mixed build), never a user condition: stop in a debug build; in a release build show that the app's build is broken and must be updated, and do not create an engine. The error names the failing comparison (`RRDR60Error.abiMismatch(reason:)`). The package's own entry points (`ProcessingRecord`) run the check before their first library call and throw `RRDR60Error.abiMismatch` on a mismatch, so a broken build is never mistaken for an unreadable record (which is `nil`).

Status codes: the functions return `Int32`, while Swift imports the `RR_DR60_STATUS_*` constants with the enum's own type, so compare with `status == Int32(RR_DR60_STATUS_OK.rawValue)`.

The Swift-native surface is the helpers above and the processing record's re-render in the `RRDR60` target; a typed engine wrapper (ownership, errors, buffers) would be its own feature.
