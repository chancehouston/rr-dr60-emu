// Spec 022 (the pre-1.0 C ABI pass) from Swift: the ABI check passes for this build, `.sized`
// sets every struct's size, a zero-initialized report is refused, a mismatch names its reason,
// and the ABI rows built through the C API equal the Rust example's (the committed fixture,
// which the harness's `us2_abi` test pins to `abi_checks::rows_at(48000)`).
import Foundation
@testable import RRDR60
import RRDR60C
import Testing

private func isOK(_ status: Int32) -> Bool { status == Int32(RR_DR60_STATUS_OK.rawValue) }

@Test func checkABIPasses() throws {
    try RRDR60.checkABI()
    #expect(RRDR60.abiOK)
    #expect(rr_dr60_abi_version() == UInt32(RR_DR60_ABI_VERSION))
    #expect(RR_DR60_ABI_VERSION == 1 && RR_DR60_MAX_STRUCT_SIZE == 4096)
    #expect(RR_DR60_VAS_EVENT_SIZE == 16 && RR_DR60_CHANGE_ENTRY_SIZE == 40)
    #expect(MemoryLayout<RrDr60VasEvent>.size == 16 && MemoryLayout<RrDr60ChangeEntry>.size == 40)
}

private func sizedMatches<T: RRDR60Sized>(_: T.Type) -> Bool {
    T.sized.struct_size == UInt32(MemoryLayout<T>.size) && MemoryLayout<T>.size == MemoryLayout<T>.stride
}

@Test func sizedSetsMemoryLayoutSize() {
    #expect(sizedMatches(RrDr60BlockInfo.self) && MemoryLayout<RrDr60BlockInfo>.size == 40)
    #expect(sizedMatches(RrDr60RecordInfo.self) && MemoryLayout<RrDr60RecordInfo>.size == 48)
    #expect(sizedMatches(RrDr60StopOutcome.self) && MemoryLayout<RrDr60StopOutcome>.size == 88)
    #expect(sizedMatches(RrDr60PlayInfo.self) && MemoryLayout<RrDr60PlayInfo>.size == 48)
    #expect(sizedMatches(RrDr60EngineState.self) && MemoryLayout<RrDr60EngineState>.size == 20)
    #expect(sizedMatches(RrDr60MemoryStatus.self) && MemoryLayout<RrDr60MemoryStatus>.size == 48)
    #expect(sizedMatches(RrDr60SpeechCodecInfo.self) && MemoryLayout<RrDr60SpeechCodecInfo>.size == 28)
    #expect(sizedMatches(RrDr60ManifestHeader.self) && MemoryLayout<RrDr60ManifestHeader>.size == 72)
    #expect(sizedMatches(RrDr60StageEntry.self) && MemoryLayout<RrDr60StageEntry>.size == 80)
    #expect(sizedMatches(RrDr60Control.self) && MemoryLayout<RrDr60Control>.size == 120)
    #expect(sizedMatches(RrDr60ControlOption.self) && MemoryLayout<RrDr60ControlOption>.size == 24)
    #expect(sizedMatches(RrDr60Meters.self) && MemoryLayout<RrDr60Meters>.size == 64)
    #expect(sizedMatches(RrDr60RecordReadout.self) && MemoryLayout<RrDr60RecordReadout>.size == 376)
    #expect(sizedMatches(RrDr60InputAlignment.self) && MemoryLayout<RrDr60InputAlignment>.size == 32)
    #expect(sizedMatches(RrDr60PlaceState.self) && MemoryLayout<RrDr60PlaceState>.size == 32)
    #expect(sizedMatches(RrDr60RunData.self) && MemoryLayout<RrDr60RunData>.size == 80)
    #expect(sizedMatches(RrDr60StopFields.self) && MemoryLayout<RrDr60StopFields>.size == 40)
    #expect(MemoryLayout<RrDr60Settings>.size == 120)
}

@Test func zeroInitializedReportIsRefused() throws {
    var settings = rr_dr60_settings_default(48000)
    var pipeline: OpaquePointer?
    #expect(isOK(rr_dr60_create(&settings, &pipeline)))
    let p = try #require(pipeline)
    defer { rr_dr60_destroy(p) }
    let x = [Float](repeating: 0.1, count: 64)
    var y = [Float](repeating: 0, count: 64)
    var zero = RrDr60BlockInfo()
    let refused = x.withUnsafeBufferPointer { xb in
        y.withUnsafeMutableBufferPointer { yb in rr_dr60_process(p, xb.baseAddress, yb.baseAddress, 64, &zero) }
    }
    #expect(refused == Int32(RR_DR60_STATUS_INVALID_ARGUMENT.rawValue))
    #expect(zero.struct_size == 0 && zero.produced == 0)
    var sized = RrDr60BlockInfo.sized
    let ok = x.withUnsafeBufferPointer { xb in
        y.withUnsafeMutableBufferPointer { yb in rr_dr60_process(p, xb.baseAddress, yb.baseAddress, 64, &sized) }
    }
    #expect(isOK(ok))
    #expect(sized.struct_size == 40)
    // The settings struct is validated whole (D13).
    var big = rr_dr60_settings_default(48000)
    big.struct_size = 121
    var q: OpaquePointer?
    #expect(rr_dr60_create(&big, &q) == Int32(RR_DR60_STATUS_INVALID_ARGUMENT.rawValue))
    #expect(q == nil)
}

@Test func mismatchReasonIsNamed() throws {
    let ok = (UInt32(1), UInt(8), UInt(120), UInt(16), UInt(40))
    #expect(RRDR60.mismatchReason(version: ok.0, pointerSize: ok.1, settingsSize: ok.2, vasEventSize: ok.3, changeEntrySize: ok.4) == nil)
    #expect(RRDR60.mismatchReason(version: 2, pointerSize: 8, settingsSize: 120, vasEventSize: 16, changeEntrySize: 40)
        == .version(library: 1, header: 2))
    #expect(RRDR60.mismatchReason(version: 1, pointerSize: 4, settingsSize: 120, vasEventSize: 16, changeEntrySize: 40) == .pointerSize)
    #expect(RRDR60.mismatchReason(version: 1, pointerSize: 8, settingsSize: 121, vasEventSize: 16, changeEntrySize: 40) == .settingsSize)
    #expect(RRDR60.mismatchReason(version: 1, pointerSize: 8, settingsSize: 119, vasEventSize: 16, changeEntrySize: 40) == .settingsSize)
    #expect(RRDR60.mismatchReason(version: 1, pointerSize: 8, settingsSize: 120, vasEventSize: 24, changeEntrySize: 40) == .vasEventSize)
    #expect(RRDR60.mismatchReason(version: 1, pointerSize: 8, settingsSize: 120, vasEventSize: 16, changeEntrySize: 48) == .changeEntrySize)
    // A library refusal with every header comparison holding names the status (coach 9, B2).
    RRDR60.abiOverride = Int32(RR_DR60_STATUS_ABI_MISMATCH.rawValue)
    defer { RRDR60.abiOverride = nil }
    #expect(!RRDR60.abiOK)
    #expect(throws: RRDR60Error.abiMismatch(reason: .libraryRefused(status: Int32(RR_DR60_STATUS_ABI_MISMATCH.rawValue)))) {
        try RRDR60.checkABI()
    }
    // The package's entry points consult the check before any library call (coach 2).
    #expect(throws: RRDR60Error.self) { try ProcessingRecord.read([1, 2, 3]) }
    #expect(throws: RRDR60Error.self) { try ProcessingRecord.changes(of: [1, 2, 3], count: 0) }
    #expect(throws: RRDR60Error.self) { try ProcessingRecord.reRender(record: [1, 2, 3], raw: []) }
    RRDR60.abiOverride = nil
    let unreadable = try ProcessingRecord.read([1, 2, 3])
    #expect(unreadable == nil, "an unreadable record is nil, not an error")
    // The C check itself, each argument wrong alone.
    #expect(isOK(rr_dr60_abi_check(1, 8, 120, 16, 40)))
    for bad in [(2, 8, 120, 16, 40), (1, 4, 120, 16, 40), (1, 8, 121, 16, 40), (1, 8, 119, 16, 40), (1, 8, 120, 24, 40), (1, 8, 120, 16, 48)] {
        #expect(rr_dr60_abi_check(UInt32(bad.0), UInt(bad.1), UInt(bad.2), UInt(bad.3), UInt(bad.4)) == Int32(RR_DR60_STATUS_ABI_MISMATCH.rawValue))
    }
}

/// The ABI rows through the C API from Swift, in the smoke's and the Rust example's format.
private func abiRows() throws -> String {
    var o = "sizes\n"
    let sizes: [(String, Int)] = [
        ("RrDr60BlockInfo", MemoryLayout<RrDr60BlockInfo>.size), ("RrDr60RecordInfo", MemoryLayout<RrDr60RecordInfo>.size),
        ("RrDr60StopOutcome", MemoryLayout<RrDr60StopOutcome>.size), ("RrDr60PlayInfo", MemoryLayout<RrDr60PlayInfo>.size),
        ("RrDr60EngineState", MemoryLayout<RrDr60EngineState>.size), ("RrDr60MemoryStatus", MemoryLayout<RrDr60MemoryStatus>.size),
        ("RrDr60SpeechCodecInfo", MemoryLayout<RrDr60SpeechCodecInfo>.size), ("RrDr60ManifestHeader", MemoryLayout<RrDr60ManifestHeader>.size),
        ("RrDr60StageEntry", MemoryLayout<RrDr60StageEntry>.size), ("RrDr60Control", MemoryLayout<RrDr60Control>.size),
        ("RrDr60ControlOption", MemoryLayout<RrDr60ControlOption>.size), ("RrDr60Meters", MemoryLayout<RrDr60Meters>.size),
        ("RrDr60RecordReadout", MemoryLayout<RrDr60RecordReadout>.size), ("RrDr60InputAlignment", MemoryLayout<RrDr60InputAlignment>.size),
        ("RrDr60PlaceState", MemoryLayout<RrDr60PlaceState>.size), ("RrDr60VasEvent", MemoryLayout<RrDr60VasEvent>.size),
        ("RrDr60ChangeEntry", MemoryLayout<RrDr60ChangeEntry>.size), ("RrDr60StopFields", MemoryLayout<RrDr60StopFields>.size),
        ("RrDr60RunData", MemoryLayout<RrDr60RunData>.size), ("RrDr60Settings", MemoryLayout<RrDr60Settings>.size),
    ]
    for (name, size) in sizes { o += "\(name) \(size)\n" }
    o += "defines \(RR_DR60_ABI_VERSION) \(RR_DR60_MAX_STRUCT_SIZE) \(RR_DR60_VAS_EVENT_SIZE) \(RR_DR60_CHANGE_ENTRY_SIZE)\n"
    let n = 4096
    let x: [Float] = (0 ..< n).map { i in i % 48 < 24 ? 0.125 : -0.125 }
    var y = [Float](repeating: 0, count: n)
    var ev = [RrDr60VasEvent](repeating: RrDr60VasEvent(), count: 64)
    var s = rr_dr60_settings_default(48000)
    s.speech_codec_enabled = false
    var handle: OpaquePointer?
    #expect(isOK(rr_dr60_create(&s, &handle)))
    let p = try #require(handle)
    defer { rr_dr60_destroy(p) }
    func hex(_ f: Float) -> String { String(format: "%08x", f.bitPattern) }
    var bi = RrDr60BlockInfo.sized
    #expect(isOK(x.withUnsafeBufferPointer { xb in y.withUnsafeMutableBufferPointer { yb in
        rr_dr60_process_with_events(p, xb.baseAddress, yb.baseAddress, UInt(n), &bi, &ev, 64) } }))
    o += "block_info \(bi.struct_size) \(bi.produced) \(bi.events) \(bi.paused ? 1 : 0) \(bi.frames)\n"
    var m = RrDr60Meters.sized
    #expect(isOK(rr_dr60_meters(p, &m)))
    o += "meters \(m.struct_size) \(hex(m.input_peak)) \(hex(m.limiter_input_peak)) \(hex(m.gain_reduction_db)) \(hex(m.hold_charge)) \(hex(m.record_filter_peak)) \(m.adc_clamp_count) \(m.vas_paused) \(m.hang_remaining_samples) \(hex(m.reconstruction_peak)) \(hex(m.route_peak)) \(m.output_clamp_count) \(m.amp_clip_count) \(hex(m.battery_ceiling))\n"
    #expect(isOK(rr_dr60_start_recording(p)))
    var ri = RrDr60RecordInfo.sized
    #expect(isOK(x.withUnsafeBufferPointer { xb in rr_dr60_record(p, xb.baseAddress, UInt(n), &ri, &ev, 64) }))
    o += "record_info \(ri.struct_size) \(ri.stored) \(ri.events) \(ri.paused ? 1 : 0) \(ri.stopped ? 1 : 0) \(ri.stop_reason) \(ri.frames) \(ri.frames_total)\n"
    var so = RrDr60StopOutcome.sized
    #expect(isOK(rr_dr60_stop_recording(p, &so, &ev, 64)))
    o += "stop_outcome \(so.struct_size) \(so.flush_stored) \(so.flush_events) \(so.flush_paused ? 1 : 0) \(so.flush_stopped ? 1 : 0) \(so.flush_stop_reason) \(so.flush_frames) \(so.flush_frames_total) \(so.kept ? 1 : 0) \(so.number) \(so.length_samples) \(so.silence_fed_samples) \(so.stop_input_position) \(so.padding_start_sample)\n"
    #expect(isOK(rr_dr60_start_playback(p, 1)))
    var pi = RrDr60PlayInfo.sized
    #expect(isOK(y.withUnsafeMutableBufferPointer { yb in rr_dr60_play(p, yb.baseAddress, UInt(n), &pi) }))
    o += "play_info \(pi.struct_size) \(pi.produced) \(pi.finished ? 1 : 0) \(pi.frames) \(pi.frame_index) \(pi.place)\n"
    var st = RrDr60EngineState.sized
    #expect(isOK(rr_dr60_get_state(p, &st)))
    o += "engine_state \(st.struct_size) \(st.state) \(st.route) \(st.current_recording) \(st.speaker_muted ? 1 : 0)\n"
    var ms = RrDr60MemoryStatus.sized
    #expect(isOK(rr_dr60_memory_status(p, &ms)))
    o += "memory_status \(ms.struct_size) \(ms.capacity_samples) \(ms.used_samples) \(ms.remaining_samples) \(ms.max_recordings) \(ms.count) \(ms.slots_remaining)\n"
    _ = rr_dr60_stop_playback(p)
    var ci = RrDr60SpeechCodecInfo.sized
    #expect(isOK(rr_dr60_speech_codec_info(&ci)))
    o += "speech_codec_info \(ci.struct_size) \(ci.frame_samples) \(ci.frame_ms) \(ci.bits_per_frame) \(ci.delay_samples) \(ci.model_version.0).\(ci.model_version.1).\(ci.model_version.2)\n"
    var h = RrDr60ManifestHeader.sized
    #expect(isOK(rr_dr60_manifest_header(&h)))
    o += "manifest_header \(h.struct_size) \(String(cString: h.library_version)) \(h.device_rate_hz) \(h.stage_count) \(h.count_not_modeled) \(h.count_assumed) \(h.count_sourced) \(h.count_checked)\n"
    var e = RrDr60StageEntry.sized
    #expect(isOK(rr_dr60_manifest_stage(3, &e)))
    o += "stage_entry \(e.struct_size) \(e.number) \(String(cString: e.identifier)) \(e.status) \(e.path) \(e.confidence) \(e.source_count) \(e.assumption_count)\n"
    var c = RrDr60Control.sized
    #expect(isOK(rr_dr60_control(0, &c)))
    o += "control \(c.struct_size) 0 \(String(cString: c.identifier)) \(c.kind) \(c.timing) \(c.paths) \(c.stage) \(c.value_type) \(c.int_min) \(c.int_max) \(c.int_default) \(hex(c.float_min)) \(hex(c.float_max)) \(hex(c.float_default)) \(c.option_count) \(c.choice_default) \(c.has_default) \(c.source_count) \(c.assumption_count)\n"
    var choice: UInt32 = 0
    for i in 0 ..< rr_dr60_control_count() {
        var k = RrDr60Control.sized
        #expect(isOK(rr_dr60_control(i, &k)))
        if k.value_type == UInt32(RR_DR60_CONTROL_VALUE_TYPE_CHOICE.rawValue) { choice = i; break }
    }
    var op = RrDr60ControlOption.sized
    #expect(isOK(rr_dr60_control_option(choice, 0, &op)))
    o += "control_option \(op.struct_size) \(choice) 0 \(String(cString: op.identifier)) \(op.value)\n"
    return o
}

@Test func abiRowsMatchTheRustExample() throws {
    let url = try #require(Bundle.module.url(forResource: "abi-rows-48000", withExtension: "txt", subdirectory: "Fixtures"))
    let want = try String(contentsOf: url, encoding: .utf8)
    let got = try abiRows()
    #expect(got == want)
}
