// Spec 022 (the pre-1.0 C ABI pass): every struct the library fills or reads starts with its
// size. Swift's zero-initializer (`RrDr60BlockInfo()`) leaves `struct_size` 0, which the library
// refuses by design; `.sized` is the constructor to use.
import RRDR60C

/// A C struct whose first field is its caller-set size (the compatible prefix-fill rule).
public protocol RRDR60Sized {
    var struct_size: UInt32 { get set }
    init()
}

extension RRDR60Sized {
    /// The zeroed struct with `struct_size` set to its size, ready for a library call.
    public static var sized: Self {
        var s = Self()
        s.struct_size = UInt32(MemoryLayout<Self>.size)
        return s
    }
}

// The fifteen structs the library fills and the two inputs it reads. `RrDr60Settings` is not
// among them: a zeroed settings struct is refused; it comes from `rr_dr60_settings_default`.
extension RrDr60BlockInfo: RRDR60Sized {}
extension RrDr60RecordInfo: RRDR60Sized {}
extension RrDr60StopOutcome: RRDR60Sized {}
extension RrDr60PlayInfo: RRDR60Sized {}
extension RrDr60EngineState: RRDR60Sized {}
extension RrDr60MemoryStatus: RRDR60Sized {}
extension RrDr60SpeechCodecInfo: RRDR60Sized {}
extension RrDr60ManifestHeader: RRDR60Sized {}
extension RrDr60StageEntry: RRDR60Sized {}
extension RrDr60Control: RRDR60Sized {}
extension RrDr60ControlOption: RRDR60Sized {}
extension RrDr60Meters: RRDR60Sized {}
extension RrDr60RecordReadout: RRDR60Sized {}
extension RrDr60InputAlignment: RRDR60Sized {}
extension RrDr60PlaceState: RRDR60Sized {}
extension RrDr60RunData: RRDR60Sized {}
extension RrDr60StopFields: RRDR60Sized {}
// Spec 025: the control reading, filled and read by the reads and writes by control index.
extension RrDr60ControlReading: RRDR60Sized {}
