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

    /// Reads a record; `nil` when the library refuses it. A `pipeline` adds the host-rate flag.
    public static func read(_ record: [UInt8], pipeline: OpaquePointer? = nil) -> RrDr60RecordReadout? {
        var r = RrDr60RecordReadout()
        r.struct_size = UInt32(MemoryLayout<RrDr60RecordReadout>.size)
        let status = record.withUnsafeBufferPointer { bytes in
            rr_dr60_processing_record_read(pipeline, bytes.baseAddress, UInt(record.count), &r)
        }
        return status == Int32(RR_DR60_STATUS_OK.rawValue) ? r : nil
    }

    /// The record's change rows (the control's manifest index, the stream word in `reserved`).
    public static func changes(of record: [UInt8], count: UInt64) -> [RrDr60ChangeEntry] {
        var rows = [RrDr60ChangeEntry](repeating: RrDr60ChangeEntry(), count: Int(count))
        let got = record.withUnsafeBufferPointer { bytes in
            rows.withUnsafeMutableBufferPointer { out in
                Int(rr_dr60_record_changes(bytes.baseAddress, UInt(record.count), 0, out.baseAddress, UInt(out.count)))
            }
        }
        return Array(rows.prefix(got))
    }

    /// Re-renders `raw` (24-bit samples) from `record` and returns the stored form's digest, or
    /// `nil` when a step is refused. The caller compares it with the read-out's
    /// `recording_digest`.
    public static func reRender(record: [UInt8], raw: [Int32], block: Int = 4096) -> UInt64? {
        guard let r = read(record), r.origin == RR_DR60_ORIGIN_ENGINE else { return nil }
        var settings = r.settings
        var handle: OpaquePointer?
        guard rr_dr60_create_at_counter(&settings, r.recording_counter, &handle) == Int32(RR_DR60_STATUS_OK.rawValue),
            let p = handle
        else { return nil }
        defer { rr_dr60_destroy(p) }
        // The fill by import: a throwaway 6 kHz engine with every record stage off and the
        // same codec setting records zeros, exports, this engine imports.
        var status = RrDr60MemoryStatus()
        guard rr_dr60_memory_status(p, &status) == Int32(RR_DR60_STATUS_OK.rawValue) else { return nil }
        if status.remaining_samples > r.remaining_samples_at_start {
            let missing = status.remaining_samples - r.remaining_samples_at_start
            guard fill(p, samples: missing, codec: settings.speech_codec_enabled) else { return nil }
        }
        let entries = changes(of: record, count: r.change_count)
        var next = 0
        func schedule() -> Bool {
            while next < entries.count {
                var pending: UInt32 = 0
                guard rr_dr60_pending_changes(p, UInt32(RR_DR60_STREAM_RECORD.rawValue), &pending) == Int32(RR_DR60_STATUS_OK.rawValue)
                else { return false }
                if pending >= 64 { return true }
                let e = entries[next]
                guard rr_dr60_schedule_change(p, UInt32(RR_DR60_STREAM_RECORD.rawValue), e.position, e.control, e.after, e.tag)
                        == Int32(RR_DR60_STATUS_OK.rawValue)
                else { return false }
                next += 1
            }
            return true
        }
        guard schedule(), rr_dr60_start_recording(p) == Int32(RR_DR60_STATUS_OK.rawValue) else { return nil }
        let x = samplesFromI24(raw)
        let fed = min(Int(r.stop.stop_input_position), x.count)
        var pos = 0
        var stopped = false
        while pos < fed {
            let n = min(block, fed - pos)
            var info = RrDr60RecordInfo()
            let st = x.withUnsafeBufferPointer { buf in
                rr_dr60_record(p, buf.baseAddress! + pos, UInt(n), &info, nil, 0)
            }
            guard st == Int32(RR_DR60_STATUS_OK.rawValue) else { return nil }
            pos += n
            guard schedule() else { return nil }
            if info.stopped { stopped = true; break }
        }
        if !stopped {
            var outcome = RrDr60StopOutcome()
            guard rr_dr60_stop_recording(p, &outcome, nil, 0) == Int32(RR_DR60_STATUS_OK.rawValue) else { return nil }
        }
        guard rr_dr60_memory_status(p, &status) == Int32(RR_DR60_STATUS_OK.rawValue) else { return nil }
        var digest: UInt64 = 0
        guard rr_dr60_recording_digest(p, status.count, &digest) == Int32(RR_DR60_STATUS_OK.rawValue) else { return nil }
        return digest
    }

    /// Imports a filler of `samples` device samples of zeros (whole frames with the codec on).
    static func fill(_ p: OpaquePointer, samples: UInt64, codec: Bool) -> Bool {
        var s = rr_dr60_settings_default(6000)
        s.record_stage_enabled = false
        s.adc_clamp_enabled = false
        s.agc_enabled = false
        s.vas_enabled = false
        s.mic_capsule_enabled = false
        s.mic_amp_enabled = false
        s.speech_codec_enabled = codec
        s.memory_capacity_s = UInt32(samples / 6000) + 1
        var handle: OpaquePointer?
        guard rr_dr60_create(&s, &handle) == Int32(RR_DR60_STATUS_OK.rawValue), let q = handle else { return false }
        defer { rr_dr60_destroy(q) }
        let zeros = [Float](repeating: 0, count: Int(samples))
        var info = RrDr60RecordInfo()
        var outcome = RrDr60StopOutcome()
        guard rr_dr60_start_recording(q) == Int32(RR_DR60_STATUS_OK.rawValue),
            zeros.withUnsafeBufferPointer({ rr_dr60_record(q, $0.baseAddress, UInt(zeros.count), &info, nil, 0) })
                == Int32(RR_DR60_STATUS_OK.rawValue),
            rr_dr60_stop_recording(q, &outcome, nil, 0) == Int32(RR_DR60_STATUS_OK.rawValue), outcome.kept,
            outcome.length_samples == samples
        else { return false }
        var len: UInt = 0
        guard rr_dr60_export_len(q, outcome.number, &len) == Int32(RR_DR60_STATUS_OK.rawValue) else { return false }
        var bytes = [UInt8](repeating: 0, count: Int(len))
        var written: UInt = 0
        guard bytes.withUnsafeMutableBufferPointer({ rr_dr60_export(q, outcome.number, $0.baseAddress, len, &written) })
                == Int32(RR_DR60_STATUS_OK.rawValue)
        else { return false }
        var number: UInt32 = 0
        return bytes.withUnsafeBufferPointer { rr_dr60_import(p, $0.baseAddress, len, &number) }
            == Int32(RR_DR60_STATUS_OK.rawValue)
    }
}
