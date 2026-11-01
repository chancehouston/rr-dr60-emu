// Spec 022 (the pre-1.0 C ABI pass): the start-up check of the header against the library. The
// library ships inside the app, so a mismatch is a build error of the app (a stale XCFramework,
// a mixed build), never a user condition: stop in a debug build; in a release build show that
// the app's build is broken and must be updated, and do not create an engine.
import RRDR60C

/// Which of the five comparisons failed.
public enum ABIMismatch: Error, Equatable, Sendable {
    /// The header's `RR_DR60_ABI_VERSION` is not the library's.
    case version(library: UInt32, header: UInt32)
    /// The pointer width is not 8 (the C ABI is 64-bit only).
    case pointerSize
    /// `sizeof(RrDr60Settings)` differs (the settings struct is validated whole, D13).
    case settingsSize
    /// `sizeof(RrDr60VasEvent)` is not 16 (frozen).
    case vasEventSize
    /// `sizeof(RrDr60ChangeEntry)` is not 40 (frozen).
    case changeEntrySize
    /// The library refused the header's tuple while every comparison this package can make
    /// holds (the library's own sizes are not exposed, so a settings or frozen-size difference
    /// against the library reports here; by D13 a settings growth moves the version first).
    case libraryRefused(status: Int32)
}

/// The package's errors.
public enum RRDR60Error: Error, Equatable, Sendable {
    /// `rr_dr60_abi_check` refused this header against this library.
    case abiMismatch(reason: ABIMismatch)
    /// A library call refused with this `RR_DR60_STATUS_*` value (spec 025: the reads and
    /// writes by control index).
    case status(Int32)
}

public enum RRDR60 {
    /// The header's values, as this package was compiled.
    static let headerVersion = UInt32(RR_DR60_ABI_VERSION)
    static let pointerSize = UInt(MemoryLayout<UnsafeRawPointer>.size)
    static let settingsSize = UInt(MemoryLayout<RrDr60Settings>.size)
    static let vasEventSize = UInt(MemoryLayout<RrDr60VasEvent>.size)
    static let changeEntrySize = UInt(MemoryLayout<RrDr60ChangeEntry>.size)

    /// The library's answer, computed once (a `static let` runs once, thread-safely) before the
    /// package's first library call.
    static let abiStatus: Int32 = rr_dr60_abi_check(headerVersion, pointerSize, settingsSize, vasEventSize, changeEntrySize)

    /// Test-only: a status that replaces the library's answer within one task (the package's
    /// tests exercise the mismatch path with `$abiOverride.withValue`; task-local, so tests running
    /// in parallel never see it; never set by an app).
    @TaskLocal static var abiOverride: Int32?

    /// Whether the header matches the library.
    public static var abiOK: Bool { (abiOverride ?? abiStatus) == Int32(RR_DR60_STATUS_OK.rawValue) }

    /// Checks the header against the library; call it once at launch. The package's own entry
    /// points run it before their first library call.
    public static func checkABI() throws {
        if !abiOK {
            throw RRDR60Error.abiMismatch(
                reason: mismatchReason(
                    version: headerVersion, pointerSize: pointerSize, settingsSize: settingsSize,
                    vasEventSize: vasEventSize, changeEntrySize: changeEntrySize)
                    ?? .libraryRefused(status: abiOverride ?? abiStatus))
        }
    }

    /// The first failing comparison of the five, in the check's order, or `nil` when all hold
    /// (the C check returns one status; the reason is derived here). The three size reasons are
    /// reachable only with caller-supplied values: from `checkABI()` the header's values are
    /// compared with themselves, so a library refusal with an equal version is
    /// `.libraryRefused`.
    public static func mismatchReason(version: UInt32, pointerSize: UInt, settingsSize: UInt, vasEventSize: UInt,
                               changeEntrySize: UInt) -> ABIMismatch? {
        let library = rr_dr60_abi_version()
        if version != library { return .version(library: library, header: version) }
        if pointerSize != 8 { return .pointerSize }
        if settingsSize != UInt(MemoryLayout<RrDr60Settings>.size) { return .settingsSize }
        if vasEventSize != UInt(RR_DR60_VAS_EVENT_SIZE) { return .vasEventSize }
        if changeEntrySize != UInt(RR_DR60_CHANGE_ENTRY_SIZE) { return .changeEntrySize }
        return nil
    }
}
