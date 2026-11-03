// Spec 026 D9 from Swift: the meter manifest and the manifest's words equal the library's, the
// reach and lag reads, the refusals as statuses.
import RRDR60
import RRDR60C
import Testing

@Test func meterFieldsAreTheLibrarys() throws {
    let fields = MeterManifest.fields
    #expect(fields.count == 13)
    #expect(fields.first?.identifier == "input_peak")
    #expect(fields.last?.identifier == "battery_ceiling")
    #expect(fields.allSatisfy { $0.sentence.hasSuffix(".") && !$0.unit.isEmpty })
    #expect(fields.enumerated().allSatisfy { UInt32($0.offset) == $0.element.index })
    #expect(RrDr60MeterField.sized.struct_size == 40)
}

@Test func reachAndLag() throws {
    var s = rr_dr60_settings_default(48000)
    var h: OpaquePointer?
    #expect(rr_dr60_create(&s, &h) == Int32(RR_DR60_STATUS_OK.rawValue))
    let p = try #require(h)
    defer { rr_dr60_destroy(p) }
    #expect(try MeterManifest.reached(p, field: 0, call: .onePass))
    #expect(try MeterManifest.lagSamples(p, field: 0, call: .onePass) == 0)
    #expect(try MeterManifest.lagSamples(p, field: 9, call: .record) == nil)
    #expect(throws: RRDR60Error.status(Int32(RR_DR60_STATUS_NO_SUCH_METER_FIELD.rawValue))) {
        try MeterManifest.reached(p, field: 13, call: .play)
    }
}

@Test func manifestWords() {
    #expect(ManifestWord.token(.status, 0) == "not_modeled")
    #expect(ManifestWord.label(.status, 3) == "checked against a real unit")
    #expect(ManifestWord.label(.path, 2) == "record and playback")
    #expect(ManifestWord.token(.confidence, 0) == "not_applicable")
    #expect(ManifestWord.token(.path, 3) == nil)
}
