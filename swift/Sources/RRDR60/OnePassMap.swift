// Spec 028 D6: the one-pass output map from Swift. The result is the library's own sized
// `RrDr60InputAlignment`, the type the record-path map fills, so a host handles one alignment
// shape on both paths.
import RRDR60C

private let ok = Int32(RR_DR60_STATUS_OK.rawValue)

/// The input behind a one-pass output position, and where a pause report is heard.
public enum OnePassMap {
    /// The input behind one-pass output `position`, with spec 020's alignment window. `events`
    /// is every pause report the one-pass gave since the stream's last start, in report order;
    /// the engine supplies the latency, the speech coder's delay and the gate's mode. Throws the
    /// library's status.
    public static func inputAtOutput(
        _ pipeline: OpaquePointer, position: UInt64, events: [RrDr60VasEvent]
    ) throws -> RrDr60InputAlignment {
        var out = RrDr60InputAlignment.sized
        let st = events.withUnsafeBufferPointer { b in
            rr_dr60_one_pass_input_at_output(pipeline, position, b.baseAddress, UInt(b.count), &out)
        }
        guard st == ok else { throw RRDR60Error.status(st) }
        return out
    }

    /// Where a one-pass pause report is heard on the output, in host samples: the report's
    /// position plus the speech coder's delay when it runs plus the playback half's latency.
    /// Throws the library's status.
    public static func spliceOutputPosition(_ pipeline: OpaquePointer, event: RrDr60VasEvent) throws -> UInt64 {
        var out: UInt64 = 0
        var e = event
        let st = rr_dr60_one_pass_splice_output_position(pipeline, &e, &out)
        guard st == ok else { throw RRDR60Error.status(st) }
        return out
    }
}
