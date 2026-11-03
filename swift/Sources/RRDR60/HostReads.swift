// Spec 026 D9: the meter manifest and the stage manifest's words from Swift. The strings are
// the library's (static for the life of the process); a host shows them rather than keeping
// copies.
import RRDR60C

private let ok = Int32(RR_DR60_STATUS_OK.rawValue)

/// The meter read-out described: one entry per `RrDr60Meters` field, in its declaration order.
public enum MeterManifest {
    /// One field's description.
    public struct Field: Equatable, Sendable {
        /// The field's index in `RrDr60Meters`'s declaration order.
        public let index: UInt32
        /// The field's name.
        public let identifier: String
        /// Its unit.
        public let unit: String
        /// One neutral sentence.
        public let sentence: String
        /// 0 a maximum over the call, 1 a count, 2 a state at the call's end.
        public let kind: UInt32
        /// 0 the record side (input), 1 the playback side (output).
        public let side: UInt32
    }

    /// The call a read-out comes from (`RR_DR60_METER_CALL_*`).
    public enum Call: UInt32, Sendable {
        case onePass = 0
        case record = 1
        case play = 2
    }

    /// Every field, in order.
    public static var fields: [Field] {
        (0..<rr_dr60_meter_field_count()).compactMap { i in
            var f = RrDr60MeterField.sized
            guard rr_dr60_meter_field(i, &f) == ok else { return nil }
            return Field(
                index: f.index, identifier: String(cString: f.identifier), unit: String(cString: f.unit),
                sentence: String(cString: f.sentence), kind: f.kind, side: f.side)
        }
    }

    /// Whether `call` reaches field `index` at the engine's settings; throws the library's status.
    public static func reached(_ pipeline: OpaquePointer, field index: UInt32, call: Call) throws -> Bool {
        var out = false
        let st = rr_dr60_meter_reached(pipeline, index, call.rawValue, &out)
        guard st == ok else { throw RRDR60Error.status(st) }
        return out
    }

    /// Field `index`'s lag in host samples for `call`, or `nil` when the call does not reach it;
    /// throws the library's status.
    public static func lagSamples(_ pipeline: OpaquePointer, field index: UInt32, call: Call) throws -> UInt64? {
        var out: UInt64 = 0
        let st = rr_dr60_meter_lag_samples(pipeline, index, call.rawValue, &out)
        guard st == ok else { throw RRDR60Error.status(st) }
        return out == RR_DR60_METER_NOT_REACHED ? nil : out
    }
}

/// The stage manifest's words: a status, path or confidence as the canonical text's token or the
/// display label.
public enum ManifestWord {
    /// The word's kind (`RR_DR60_MANIFEST_WORD_*`).
    public enum Kind: UInt32, Sendable {
        case status = 0
        case path = 1
        case confidence = 2
    }

    /// The canonical text's token, or `nil` for a value the library does not publish.
    public static func token(_ kind: Kind, _ value: UInt32) -> String? {
        word(kind, value, UInt32(RR_DR60_MANIFEST_WORD_TOKEN))
    }

    /// The display label (lowercase), or `nil` for a value the library does not publish.
    public static func label(_ kind: Kind, _ value: UInt32) -> String? {
        word(kind, value, UInt32(RR_DR60_MANIFEST_WORD_LABEL))
    }

    private static func word(_ kind: Kind, _ value: UInt32, _ form: UInt32) -> String? {
        var out: UnsafePointer<CChar>?
        guard rr_dr60_manifest_word(kind.rawValue, value, form, &out) == ok, let s = out else { return nil }
        return String(cString: s)
    }
}
