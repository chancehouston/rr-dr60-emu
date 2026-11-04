// Spec 028 D6 from Swift: the one-pass map's rows equal the Rust example's
// (`cargo run -q -p rr_dr60_harness --example onepass_map_rows`, the fixture), the empty report
// list, the refusals as statuses.
import Foundation
import RRDR60
import RRDR60C
import Testing

@Test func onePassMapRowsMatchTheRustExample() throws {
    let url = try #require(Bundle.module.url(forResource: "onepass-map-rows", withExtension: "txt", subdirectory: "Fixtures"))
    let want = try String(contentsOf: url, encoding: .utf8)
    let taps: [UInt32] = [
        RR_DR60_TAP_AFTER_PLAYBACK.rawValue, RR_DR60_TAP_ROUTE_OUTPUT.rawValue, RR_DR60_TAP_AFTER_VAS.rawValue,
    ]
    let ev = [
        RrDr60VasEvent(output_position: 20000, input_length: 3000),
        RrDr60VasEvent(output_position: 80000, input_length: 1500),
    ]
    let pos: [UInt64] = [0, 1, 5000, 21000, 21500, 22000, 22500, 25000, 80000, 82000, 83000, 200000]
    var got = ""
    for rate: UInt32 in [6000, 48000] {
        for t in taps {
            for c in [false, true] {
                var s = rr_dr60_settings_default(rate)
                s.tap = t
                s.speech_codec_enabled = c
                var h: OpaquePointer?
                #expect(rr_dr60_create(&s, &h) == Int32(RR_DR60_STATUS_OK.rawValue))
                let p = try #require(h)
                defer { rr_dr60_destroy(p) }
                for (i, e) in ev.enumerated() {
                    got += "heard \(rate) \(t) \(c ? 1 : 0) \(i) \(try OnePassMap.spliceOutputPosition(p, event: e))\n"
                }
                for o in pos {
                    let a = try OnePassMap.inputAtOutput(p, position: o, events: ev)
                    got += "map \(rate) \(t) \(c ? 1 : 0) \(o) \(a.centre) \(a.first) \(a.last) \(a.clamped)\n"
                }
            }
        }
    }
    #expect(got == want)
}

@Test func onePassMapEmptyReportsAndRefusals() throws {
    var s = rr_dr60_settings_default(48000)
    var h: OpaquePointer?
    #expect(rr_dr60_create(&s, &h) == Int32(RR_DR60_STATUS_OK.rawValue))
    let p = try #require(h)
    defer { rr_dr60_destroy(p) }
    let a = try OnePassMap.inputAtOutput(p, position: 100_000, events: [])
    #expect(a.first <= a.centre && a.centre <= a.last && a.struct_size == 32)
    var lat: UInt32 = 0
    #expect(rr_dr60_latency_samples(p, &lat) == Int32(RR_DR60_STATUS_OK.rawValue))
    #expect(a.centre == 100_000 - UInt64(lat))
    let heard = try OnePassMap.spliceOutputPosition(p, event: RrDr60VasEvent(output_position: 10, input_length: 1))
    #expect(heard > 10)
}
