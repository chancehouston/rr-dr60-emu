// The Swift port of the re-render recipe reproduces a recording's digest through the C API
// (spec 020 US1 AS2 on the simulator): a ±0.125 square with a gap, a knee and a hiss change,
// then a schedule of more than 64 entries (the refill loop), then the feed-point rule against
// the C helper.
import RRDR60
import RRDR60C
import Testing

private func isOK(_ status: Int32) -> Bool { status == Int32(RR_DR60_STATUS_OK.rawValue) }

/// The control manifest's indices of `agc.knee_dbfs` and `mic.hiss_offset_db` (spec 014: the
/// indices are stable; an app resolves them by identifier at start-up).
private let kneeControl: UInt32 = 10
private let hissControl: UInt32 = 28

/// Records `raw` with `schedule` (position, control, value) on a default 48 kHz engine with the
/// codec on, collects the run, writes the record; returns (record, digest).
private func recordAndWrite(raw: [Int32], schedule: [(UInt64, UInt32, Double)]) throws -> ([UInt8], UInt64) {
    var settings = rr_dr60_settings_default(48000)
    settings.vas_hang_ms = 100
    var handle: OpaquePointer?
    #expect(isOK(rr_dr60_create(&settings, &handle)))
    let p = try #require(handle)
    defer { rr_dr60_destroy(p) }
    var next = 0
    func fill() {
        while next < schedule.count {
            var pending: UInt32 = 0
            #expect(isOK(rr_dr60_pending_changes(p, UInt32(RR_DR60_STREAM_RECORD.rawValue), &pending)))
            if pending >= 64 { return }
            let (pos, control, value) = schedule[next]
            #expect(isOK(rr_dr60_schedule_change(p, UInt32(RR_DR60_STREAM_RECORD.rawValue), pos, control, value, UInt64(next + 1))))
            next += 1
        }
    }
    fill()
    #expect(isOK(rr_dr60_start_recording(p)))
    let x = ProcessingRecord.samplesFromI24(raw)
    var events: [RrDr60VasEvent] = []
    var changes: [RrDr60ChangeEntry] = []
    var ev = [RrDr60VasEvent](repeating: RrDr60VasEvent(), count: 64)
    var ch = [RrDr60ChangeEntry](repeating: RrDr60ChangeEntry(), count: Int(RR_DR60_MAX_CHANGES_PER_CALL))
    var pos = 0
    while pos < x.count {
        let n = min(1024, x.count - pos)
        var info = RrDr60RecordInfo()
        var count: UInt = 0
        var total: UInt64 = 0
        let st = x.withUnsafeBufferPointer { buf in
            rr_dr60_record_with_changes(p, buf.baseAddress! + pos, UInt(n), &info, &ev, 64, &ch, UInt(ch.count), &count, &total)
        }
        #expect(isOK(st))
        events.append(contentsOf: ev.prefix(Int(info.events)))
        changes.append(contentsOf: ch.prefix(Int(count)))
        pos += n
        fill()
    }
    var outcome = RrDr60StopOutcome()
    var count: UInt = 0
    var total: UInt64 = 0
    #expect(isOK(rr_dr60_stop_recording_with_changes(p, &outcome, &ev, 64, &ch, UInt(ch.count), &count, &total)))
    #expect(outcome.kept)
    events.append(contentsOf: ev.prefix(Int(outcome.flush.events)))
    changes.append(contentsOf: ch.prefix(Int(count)))
    var run = RrDr60RunData()
    run.struct_size = UInt32(MemoryLayout<RrDr60RunData>.size)
    run.stop.struct_size = UInt32(MemoryLayout<RrDr60StopFields>.size)
    run.stop.how = UInt32(RR_DR60_STOP_CALL)
    run.stop.stop_input_position = outcome.stop_input_position
    run.stop.padding_start_sample = outcome.padding_start_sample
    run.stop.input_samples_total = total
    run.stop.silence_fed_samples = outcome.silence_fed_samples
    var len: UInt = 0
    var record: [UInt8] = []
    try events.withUnsafeBufferPointer { e in
        try changes.withUnsafeBufferPointer { c in
            run.events = e.baseAddress
            run.event_count = UInt(e.count)
            run.changes = c.baseAddress
            run.change_count = UInt(c.count)
            #expect(isOK(rr_dr60_processing_record_len(p, outcome.number, &run, &len)))
            record = [UInt8](repeating: 0, count: Int(len))
            var written: UInt = 0
            let st = record.withUnsafeMutableBufferPointer {
                rr_dr60_processing_record_write(p, outcome.number, &run, $0.baseAddress, len, &written)
            }
            #expect(isOK(st))
            #expect(written == len)
            _ = try #require(record.count > 0)
        }
    }
    var digest: UInt64 = 0
    #expect(isOK(rr_dr60_recording_digest(p, outcome.number, &digest)))
    return (record, digest)
}

/// A ±0.125 square (1 kHz, exactly representable) for 0.5 s, 0.4 s of silence, 0.3 s of square,
/// as 24-bit codes.
private func squareWithGap() -> [Int32] {
    let rate = 48000
    return (0 ..< rate * 12 / 10).map { i in
        let on = i < rate / 2 || i >= rate * 9 / 10
        return on ? (i % 48 < 24 ? 1_048_576 : -1_048_576) : 0
    }
}

@Test func digestRoundTrip() throws {
    let raw = squareWithGap()
    let (record, digest) = try recordAndWrite(raw: raw, schedule: [(10000, kneeControl, -30), (30000, hissControl, 3)])
    let r = try #require(ProcessingRecord.read(record))
    #expect(r.recording_digest == digest)
    #expect(r.flags == 0)
    #expect(r.change_count == 2)
    #expect(ProcessingRecord.reRender(record: record, raw: raw) == digest)
    // One sample moved: the digest differs.
    var other = raw
    other[12000] += 400_000
    #expect(ProcessingRecord.reRender(record: record, raw: other) != digest)
}

@Test func scheduleRefillPast64() throws {
    let raw = squareWithGap()
    let schedule: [(UInt64, UInt32, Double)] = (0 ..< 80).map { i in (UInt64(2000 + 500 * i), hissControl, Double(i % 5)) }
    let (record, digest) = try recordAndWrite(raw: raw, schedule: schedule)
    let r = try #require(ProcessingRecord.read(record))
    #expect(r.change_count == 80)
    #expect(ProcessingRecord.reRender(record: record, raw: raw) == digest)
}

@Test func samplesFromI24MatchesTheRule() {
    let codes: [Int32] = [-8_388_608, -1, 0, 1, 8_388_607, 1_048_576]
    let got = ProcessingRecord.samplesFromI24(codes)
    for (c, v) in zip(codes, got) {
        #expect(v == Float(c) / 8_388_608)
    }
}
