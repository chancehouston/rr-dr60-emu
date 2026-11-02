// Spec 025 (D10; research R-09) from Swift: every default read by index equals the manifest's,
// the identifier lookup, the one-hot round trip through the wrappers, the refusals as statuses.
@testable import RRDR60
import RRDR60C
import Testing

private func engine(_ rate: UInt32 = 48000) -> OpaquePointer {
    var s = rr_dr60_settings_default(rate)
    var h: OpaquePointer?
    precondition(rr_dr60_create(&s, &h) == Int32(RR_DR60_STATUS_OK.rawValue))
    return h!
}

@Test func readingIsSized() {
    #expect(RrDr60ControlReading.sized.struct_size == 24 && MemoryLayout<RrDr60ControlReading>.size == 24)
}

@Test func defaultsByIndex() throws {
    let p = engine()
    defer { rr_dr60_destroy(p) }
    for i in 0..<rr_dr60_control_count() {
        let read = try RRDR60.controlValue(p, index: i)
        if let d = ControlReading(defaultOf: i) {
            #expect(read == d, "index \(i)")
        } else {
            #expect(read == .choice(48000), "the host rate")
        }
    }
    #expect(ControlReading(defaultOf: 99) == nil)
}

@Test func identifierLookup() {
    #expect(ControlIndex(identifier: "volume")?.rawValue == 1)
    #expect(ControlIndex(identifier: "playback_speed_mode")?.rawValue == 30)
    #expect(ControlIndex(identifier: "no.such") == nil)
    #expect(RRDR60.controlIndex(forField: UInt32(RR_DR60_SETTING_FIELD_VAS_HANG_MS.rawValue))
        == ControlIndex(identifier: "vas.hang_ms"))
    #expect(RRDR60.controlIndex(forField: 0) == nil)
}

@Test func oneHotRoundTrip() throws {
    var s = rr_dr60_settings_default(48000)
    var settingsCount = 0
    for i in 0..<rr_dr60_control_count() {
        guard let current = try? s.value(index: i) else { continue }
        settingsCount += 1
        let new: ControlReading
        switch current {
        case .switch(let b): new = .switch(!b)
        case .integer: new = .integer(UInt64(i) + 2)
        case .continuous: new = .continuous(Float(i) + 0.5)
        case .choice(let c): new = .choice(i == 3 ? (c == 6000 ? 8000 : 6000) : (c == 0 ? 1 : 0))
        }
        try s.setValue(index: i, new)
        #expect(try s.value(index: i) == new, "index \(i)")
    }
    #expect(settingsCount == 26)
    // Named fields (coach review 7): a swapped mapping would pass the round trip alone.
    let d = rr_dr60_settings_default(48000)
    #expect(s.agc_attack_ms == 11.5 && s.agc_release_ms == 12.5 && d.agc_release_ms != 12.5)
    #expect(s.vas_sensitivity == 17 && s.memory_capacity_s == 21 && s.seed == 10)
    #expect(s.output_speaker_enabled == !d.output_speaker_enabled && s.output_earphone_enabled == !d.output_earphone_enabled)
    #expect(s.mic_hiss_offset_db == 28.5 && s.tap != d.tap)
}

@Test func refusalsAsStatuses() throws {
    let p = engine()
    defer { rr_dr60_destroy(p) }
    #expect(throws: RRDR60Error.status(Int32(RR_DR60_STATUS_NOT_A_PLAIN_LIVE_CONTROL.rawValue))) {
        try RRDR60.setControlValue(p, index: 10, .continuous(-3))
    }
    #expect(throws: RRDR60Error.status(Int32(RR_DR60_STATUS_INVALID_VOLUME.rawValue))) {
        try RRDR60.setControlValue(p, index: 1, .continuous(1.5))
    }
    var s = rr_dr60_settings_default(48000)
    #expect(throws: RRDR60Error.status(Int32(RR_DR60_STATUS_NOT_A_SETTING.rawValue))) {
        try s.setValue(index: 1, .continuous(0.5))
    }
    try RRDR60.setControlValue(p, index: 1, .continuous(0.5))
    #expect(try RRDR60.controlValue(p, index: 1) == .continuous(0.5))
    let applied = try RRDR60.appliedSettings(p)
    #expect(applied.host_rate_hz == 48000 && applied.agc_knee_dbfs == -1)
}
