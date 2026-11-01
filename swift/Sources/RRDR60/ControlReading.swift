// Spec 025 (D10; research R-09): every control by its manifest index from Swift. A typed
// reading, the manifest default as a reading, the index of an identifier, and throwing wrappers
// around the six C calls. Presets and sessions store identifiers, never indices: an index is the
// manifest's order in this build, an identifier is stable.
import RRDR60C

private let ok = Int32(RR_DR60_STATUS_OK.rawValue)

/// A control's value in its manifest value type.
public enum ControlReading: Equatable, Sendable {
    /// A switch: off or on.
    case `switch`(Bool)
    /// An integer, exact over the whole `UInt64` range (the seed spans it).
    case integer(UInt64)
    /// A continuous value, single precision as the engine stores it.
    case continuous(Float)
    /// A choice: the engine value of one of the control's options.
    case choice(UInt32)

    /// The reading of a C reading; `nil` for an unknown value type or a switch other than 0 or 1.
    public init?(_ c: RrDr60ControlReading) {
        switch c.value_type {
        case 0:
            guard c.int_value <= 1 else { return nil }
            self = .switch(c.int_value == 1)
        case 1: self = .integer(c.int_value)
        case 2: self = .continuous(c.float_value)
        case 3:
            guard let v = UInt32(exactly: c.int_value) else { return nil }
            self = .choice(v)
        default: return nil
        }
    }

    /// The C reading, sized.
    public var c: RrDr60ControlReading {
        var r = RrDr60ControlReading.sized
        switch self {
        case .switch(let b):
            r.value_type = 0
            r.int_value = b ? 1 : 0
        case .integer(let n):
            r.value_type = 1
            r.int_value = n
        case .continuous(let v):
            r.value_type = 2
            r.float_value = v
        case .choice(let v):
            r.value_type = 3
            r.int_value = UInt64(v)
        }
        return r
    }

    /// The manifest default of control `index` as a reading; `nil` for the host rate (it has no
    /// default) and for an index past the manifest.
    public init?(defaultOf index: UInt32) {
        var e = RrDr60Control.sized
        guard rr_dr60_control(index, &e) == ok else { return nil }
        switch e.value_type {
        case 0: self = .switch(e.int_default != 0)
        case 1: self = .integer(e.int_default)
        case 2: self = .continuous(e.float_default)
        case 3:
            guard e.has_default != 0 else { return nil }
            self = .choice(e.choice_default)
        default: return nil
        }
    }
}

/// A control manifest index, looked up by identifier (a loop over the manifest).
public struct ControlIndex: RawRepresentable, Hashable, Sendable {
    public let rawValue: UInt32

    public init(rawValue: UInt32) { self.rawValue = rawValue }

    /// The index of the control with `identifier`, or `nil`.
    public init?(identifier: String) {
        let count = rr_dr60_control_count()
        for i in 0..<count {
            var e = RrDr60Control.sized
            guard rr_dr60_control(i, &e) == ok, let id = e.identifier else { continue }
            if String(cString: id) == identifier {
                self.rawValue = i
                return
            }
        }
        return nil
    }
}

extension RRDR60 {
    private static func check(_ status: Int32) throws {
        if status != ok { throw RRDR60Error.status(status) }
    }

    /// The current value of control `index` on an engine (`rr_dr60_control_value`).
    public static func controlValue(_ pipeline: OpaquePointer, index: UInt32) throws -> ControlReading {
        try checkABI()
        var r = RrDr60ControlReading.sized
        try check(rr_dr60_control_value(pipeline, index, &r))
        guard let v = ControlReading(r) else { throw RRDR60Error.status(Int32(RR_DR60_STATUS_INTERNAL_ERROR.rawValue)) }
        return v
    }

    /// Sets one of the five live controls by index through its setter (`rr_dr60_set_control_value`).
    public static func setControlValue(_ pipeline: OpaquePointer, index: UInt32, _ value: ControlReading) throws {
        try checkABI()
        var r = value.c
        try check(rr_dr60_set_control_value(pipeline, index, &r))
    }

    /// The engine's applied settings (`rr_dr60_get_settings`; the size set here).
    public static func appliedSettings(_ pipeline: OpaquePointer) throws -> RrDr60Settings {
        try checkABI()
        var s = rr_dr60_settings_default(48000)
        s.struct_size = UInt32(MemoryLayout<RrDr60Settings>.size)
        try check(rr_dr60_get_settings(pipeline, &s))
        return s
    }

    /// The control index of a field `rr_dr60_settings_validate` named, or `nil`.
    public static func controlIndex(forField field: UInt32) -> ControlIndex? {
        var i: UInt32 = 0
        guard rr_dr60_setting_field_control(field, &i) == ok else { return nil }
        return ControlIndex(rawValue: i)
    }
}

extension RrDr60Settings {
    /// The value of setting `index` (`rr_dr60_settings_control_value`).
    public func value(index: UInt32) throws -> ControlReading {
        try RRDR60.checkABI()
        var copy = self
        var r = RrDr60ControlReading.sized
        let status = rr_dr60_settings_control_value(&copy, index, &r)
        if status != ok { throw RRDR60Error.status(status) }
        guard let v = ControlReading(r) else { throw RRDR60Error.status(Int32(RR_DR60_STATUS_INTERNAL_ERROR.rawValue)) }
        return v
    }

    /// Writes setting `index` (`rr_dr60_set_settings_control_value`; representability only, the
    /// ranges are `rr_dr60_settings_validate`'s).
    public mutating func setValue(index: UInt32, _ value: ControlReading) throws {
        try RRDR60.checkABI()
        var r = value.c
        let status = rr_dr60_set_settings_control_value(&self, index, &r)
        if status != ok { throw RRDR60Error.status(status) }
    }
}
