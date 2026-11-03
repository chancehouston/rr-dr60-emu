// The processing record's re-render recipe over the C API (spec 020 D6, D12; research R-06):
// an engine built at the record's host rate, settings, seed and counter, its memory filled by
// import to the record's remaining capacity, the record's entries scheduled on the record stream
// (refilled past 64 pending), the kept microphone audio fed through the feed-point rule up to
// the stop position, the same stop, the stored form's digest. Parsing stays in the core so iOS
// and Android never diverge.
import RRDR60C

public enum ProcessingRecord {
    /// The engine's input for 24-bit integer samples (x ÷ 2²³; the C helper).
    public static func samplesFromI24(_ raw: [Int32]) -> [Float] {
        var out = [Float](repeating: 0, count: raw.count)
        raw.withUnsafeBufferPointer { src in
            out.withUnsafeMutableBufferPointer { dst in
                rr_dr60_samples_from_i24(src.baseAddress, dst.baseAddress, UInt(raw.count))
            }
        }
        return out
    }

    /// Reads a record; `nil` when the library refuses it; throws `RRDR60Error.abiMismatch` when
    /// the header does not match the library (spec 022: a build error of the app, told apart
    /// from an unreadable record). A `pipeline` adds the host-rate flag.
    public static func read(_ record: [UInt8], pipeline: OpaquePointer? = nil) throws -> RrDr60RecordReadout? {
        try RRDR60.checkABI()
        var r = RrDr60RecordReadout.sized
        let status = record.withUnsafeBufferPointer { bytes in
            rr_dr60_processing_record_read(pipeline, bytes.baseAddress, UInt(record.count), &r)
        }
        return status == Int32(RR_DR60_STATUS_OK.rawValue) ? r : nil
    }

    /// The record's slot (spec 024 D7): the component and the store at the recording's start;
    /// `nil` when the library refuses the record; throws on a header mismatch.
    public static func component(of record: [UInt8]) throws -> RrDr60RecordComponent? {
        try RRDR60.checkABI()
        var c = RrDr60RecordComponent.sized
        let status = record.withUnsafeBufferPointer { bytes in
            rr_dr60_processing_record_component(bytes.baseAddress, UInt(record.count), &c)
        }
        return status == Int32(RR_DR60_STATUS_OK.rawValue) ? c : nil
    }

    /// The record's change rows (the control's manifest index, the stream word in `stream`);
    /// throws `RRDR60Error.abiMismatch` when the header does not match the library.
    public static func changes(of record: [UInt8], count: UInt64) throws -> [RrDr60ChangeEntry] {
        try RRDR60.checkABI()
        var rows = [RrDr60ChangeEntry](repeating: RrDr60ChangeEntry(), count: Int(count))
        let got = record.withUnsafeBufferPointer { bytes in
            rows.withUnsafeMutableBufferPointer { out in
                Int(rr_dr60_record_changes(bytes.baseAddress, UInt(record.count), 0, out.baseAddress, UInt(out.count)))
            }
        }
        return Array(rows.prefix(got))
    }

    /// Re-renders `raw` (24-bit samples) from `record` through the engine's session (spec 027)
    /// and returns the rebuilt item's digest, or `nil` when a step is refused; throws
    /// `RRDR60Error.abiMismatch` on a header mismatch and `CancellationError` when the task is
    /// cancelled between chunks. The caller compares it with the read-out's `recording_digest`.
    public static func reRender(record: [UInt8], raw: [Int32], block: Int = 4096) throws -> UInt64? {
        try RRDR60.checkABI()
        guard let session = try? ReRenderSession(record: record) else { return nil }
        var pos = 0
        while pos < raw.count {
            try Task.checkCancellation()
            let n = min(block, raw.count - pos)
            guard let fed = try? session.feed(i24: raw[pos ..< pos + n]) else { return nil }
            pos += n
            if fed.reachedStop || fed.selfStopped { break }
        }
        guard let result = try? session.finish() else { return nil }
        return result.outcome.digest
    }
}

/// A re-render session over the C API (spec 027 D8): the plan read from a record and editable
/// before the first feed, the streaming feed, progress, the meters and the result. Not
/// `Sendable`: use it from one task. The handle is destroyed with the object.
public final class ReRenderSession {
    /// The verdict of a re-render; it describes processing only and does not vouch for what the
    /// input contains.
    public enum Verdict: UInt32, Sendable { case identical = 0, differs = 1, rebuilt = 2 }
    /// Why the session ended as it did.
    public enum Reason: UInt32, Sendable { case kept = 0, discardedEmpty = 1 }
    /// The record's flags (`RR_DR60_RECORD_FLAG_*`).
    public struct Flags: OptionSet, Sendable {
        public let rawValue: UInt32
        public init(rawValue: UInt32) { self.rawValue = rawValue }
    }
    /// What `finish` returns.
    public struct Outcome: Sendable {
        public let reason: Reason
        public let verdict: Verdict
        public let edited: Bool
        public let matchesOriginal: Bool
        public let digest: UInt64?
        public let flags: Flags
        public let remainingAtStart: UInt64
        public let rowsLanded: UInt64
    }
    /// The outcome with the rebuilt item's bytes, copied eagerly (nil when nothing was kept or
    /// the run was not collected).
    public struct Result: Sendable {
        public let outcome: Outcome
        public let storedForm: [UInt8]?
        public let record: [UInt8]?
    }
    /// What one feed did.
    public struct Fed: Sendable {
        public let taken: UInt64
        public let reachedStop: Bool
        public let selfStopped: Bool
    }

    private var plan: OpaquePointer?
    private var session: OpaquePointer?

    /// Reads `record` into a plan (spec 020's reader); throws the library's status.
    public init(record: [UInt8]) throws {
        try RRDR60.checkABI()
        var p: OpaquePointer?
        let st = record.withUnsafeBufferPointer { rr_dr60_rerender_plan_from_record($0.baseAddress, UInt($0.count), &p) }
        try Self.check(st)
        plan = p
    }

    deinit {
        if let s = session { rr_dr60_rerender_destroy(s) }
        if let p = plan { rr_dr60_rerender_plan_destroy(p) }
    }

    private static func check(_ status: Int32) throws {
        if status != Int32(RR_DR60_STATUS_OK.rawValue) { throw RRDR60Error.status(status) }
    }

    private func editPlan() throws -> OpaquePointer {
        guard session == nil, let p = plan else { throw RRDR60Error.status(Int32(RR_DR60_STATUS_RERENDER_STATE.rawValue)) }
        return p
    }

    /// A settings control's value in the plan, by identifier.
    public func value(of identifier: String) throws -> ControlReading {
        guard let i = ControlIndex(identifier: identifier) else { throw RRDR60Error.status(Int32(RR_DR60_STATUS_NO_SUCH_CONTROL.rawValue)) }
        var r = RrDr60ControlReading.sized
        try Self.check(rr_dr60_rerender_plan_control_value(try editPlan(), i.rawValue, &r))
        guard let v = ControlReading(r) else { throw RRDR60Error.status(Int32(RR_DR60_STATUS_INTERNAL_ERROR.rawValue)) }
        return v
    }

    /// Sets a settings control in the plan, by identifier (validated as `reconfigure` does).
    public func setValue(of identifier: String, _ value: ControlReading) throws {
        guard let i = ControlIndex(identifier: identifier) else { throw RRDR60Error.status(Int32(RR_DR60_STATUS_NO_SUCH_CONTROL.rawValue)) }
        var c = value.c
        try Self.check(rr_dr60_rerender_plan_set_control_value(try editPlan(), i.rawValue, &c))
    }

    /// Store value `index` of the plan.
    public func parameter(_ index: UInt32) throws -> Double {
        var v = 0.0
        try Self.check(rr_dr60_rerender_plan_parameter(try editPlan(), index, &v))
        return v
    }

    /// Writes store value `index` in the plan.
    public func setParameter(_ index: UInt32, _ value: Double) throws {
        try Self.check(rr_dr60_rerender_plan_set_parameter(try editPlan(), index, value))
    }

    /// Whether the plan's values differ from the record's.
    public func edited() throws -> Bool {
        var b = false
        try Self.check(rr_dr60_rerender_plan_edited(try editPlan(), &b))
        return b
    }

    /// Collects the run (the default) or not (a digest-only session).
    public func setCollectRun(_ collect: Bool) throws {
        try Self.check(rr_dr60_rerender_plan_set_collect_run(try editPlan(), collect))
    }

    private func started() throws -> OpaquePointer {
        if let s = session { return s }
        guard let p = plan else { throw RRDR60Error.status(Int32(RR_DR60_STATUS_RERENDER_STATE.rawValue)) }
        var s: OpaquePointer?
        try Self.check(rr_dr60_rerender_new(p, &s))
        plan = nil // consumed on success
        session = s
        return s!
    }

    /// Feeds 24-bit codes (the first feed builds the session).
    public func feed(i24 codes: ArraySlice<Int32>) throws -> Fed {
        let s = try started()
        var f = RrDr60ReRenderFed.sized
        let st = codes.withUnsafeBufferPointer { rr_dr60_rerender_feed_i24(s, $0.baseAddress, UInt($0.count), &f) }
        try Self.check(st)
        return Fed(taken: f.taken, reachedStop: f.reached_stop, selfStopped: f.self_stopped)
    }

    /// Feeds engine samples (identical only for samples equal to `samplesFromI24` of the codes).
    public func feed(f32 samples: ArraySlice<Float>) throws -> Fed {
        let s = try started()
        var f = RrDr60ReRenderFed.sized
        let st = samples.withUnsafeBufferPointer { rr_dr60_rerender_feed_f32(s, $0.baseAddress, UInt($0.count), &f) }
        try Self.check(st)
        return Fed(taken: f.taken, reachedStop: f.reached_stop, selfStopped: f.self_stopped)
    }

    /// The input position and the record's stop position, in host samples.
    public var progress: (input: UInt64, stop: UInt64) {
        guard let s = try? started() else { return (0, 0) }
        var a: UInt64 = 0
        var b: UInt64 = 0
        _ = rr_dr60_rerender_progress(s, &a, &b)
        return (a, b)
    }

    /// The engine's meter read-out of its last call.
    public func meters() throws -> RrDr60Meters {
        var m = RrDr60Meters.sized
        try Self.check(rr_dr60_rerender_meters(try started(), &m))
        return m
    }

    /// Stops and reads the rebuilt item, copying its bytes.
    public func finish() throws -> Result {
        let s = try started()
        var o = RrDr60ReRenderOutcome.sized
        try Self.check(rr_dr60_rerender_finish(s, &o))
        let outcome = Outcome(
            reason: Reason(rawValue: o.reason) ?? .kept, verdict: Verdict(rawValue: o.verdict) ?? .differs,
            edited: o.edited, matchesOriginal: o.matches_original, digest: o.has_digest ? o.digest : nil,
            flags: Flags(rawValue: o.flags), remainingAtStart: o.remaining_at_start, rowsLanded: o.rows_landed)
        func bytes(_ len: (OpaquePointer, UnsafeMutablePointer<UInt>) -> Int32,
                   _ write: (OpaquePointer, UnsafeMutablePointer<UInt8>?, UInt, UnsafeMutablePointer<UInt>) -> Int32) -> [UInt8]? {
            var n: UInt = 0
            guard len(s, &n) == Int32(RR_DR60_STATUS_OK.rawValue), n > 0 else { return nil }
            var b = [UInt8](repeating: 0, count: Int(n))
            var w: UInt = 0
            let st = b.withUnsafeMutableBufferPointer { write(s, $0.baseAddress, n, &w) }
            return st == Int32(RR_DR60_STATUS_OK.rawValue) ? b : nil
        }
        return Result(
            outcome: outcome,
            storedForm: bytes(rr_dr60_rerender_export_len, rr_dr60_rerender_export),
            record: bytes(rr_dr60_rerender_record_len, rr_dr60_rerender_record_write))
    }

    /// The engine holding the rebuilt item as its last recording (to play it); consumes the
    /// session. The caller destroys the returned handle with `rr_dr60_destroy`.
    public func intoPipeline() throws -> OpaquePointer {
        let s = try started()
        var p: OpaquePointer?
        try Self.check(rr_dr60_rerender_into_pipeline(s, &p))
        session = nil
        return p!
    }
}
