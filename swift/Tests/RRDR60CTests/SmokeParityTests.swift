// The Swift path through the C API reproduces the Rust path bit for bit: the C smoke test's
// 0.12 default one-pass case (crates/rr_dr60_ffi/tests/c/smoke.c, spec 012 T048), pinned by
// the harness's smoke_reference.rs.
import RRDR60C
import Testing

/// FNV-1a 64 over the little-endian bytes of the floats (the harness's smoke_reference::fnv1a64).
private func fnv1a64(_ samples: [Float]) -> UInt64 {
    var hash: UInt64 = 0xCBF2_9CE4_8422_2325
    for sample in samples {
        withUnsafeBytes(of: sample.bitPattern.littleEndian) { bytes in
            for byte in bytes {
                hash ^= UInt64(byte)
                hash = hash &* 0x0000_0100_0000_01B3
            }
        }
    }
    return hash
}

/// The C functions return the status as an `Int32` (the header's C11 typedef), while Swift imports
/// the `RR_DR60_STATUS_*` constants with the enum's own type; compare through the raw value.
private func isOK(_ status: Int32) -> Bool {
    status == Int32(RR_DR60_STATUS_OK.rawValue)
}

@Test func versionMatchesTheHeader() {
    let version = String(cString: rr_dr60_version_string())
    #expect(version == "\(RR_DR60_VERSION_MAJOR).\(RR_DR60_VERSION_MINOR).\(RR_DR60_VERSION_PATCH)")
}

@Test func defaultOnePassMatchesTheRustPath() throws {
    let rate = 48000
    var settings = rr_dr60_settings_default(UInt32(rate))
    #expect(settings.mic_capsule_enabled && settings.mic_amp_enabled)

    // A ±0.125 square wave (1 kHz, exactly representable) for 1 s, then 0.5 s of silence.
    let input: [Float] = (0 ..< (rate + rate / 2)).map { i in
        i < rate ? (i % 48 < 24 ? 0.125 : -0.125) : 0
    }
    var pipeline: OpaquePointer?
    #expect(isOK(rr_dr60_create(&settings, &pipeline)))
    let handle = try #require(pipeline)
    defer { rr_dr60_destroy(handle) }

    let block = 480
    var output = [Float](repeating: 0, count: block)
    var all: [Float] = []
    for start in stride(from: 0, to: input.count, by: block) {
        var info = RrDr60BlockInfo()
        info.struct_size = UInt32(MemoryLayout<RrDr60BlockInfo>.size) // the bare C module: sized by hand (spec 022)
        let status = input.withUnsafeBufferPointer { inBuf in
            output.withUnsafeMutableBufferPointer { outBuf in
                rr_dr60_process(handle, inBuf.baseAddress! + start, outBuf.baseAddress!, UInt(block), &info)
            }
        }
        #expect(isOK(status))
        all.append(contentsOf: output[0 ..< Int(info.produced)])
    }
    #expect(all.count == 72000)
    #expect(fnv1a64(all) == 0x533E_9117_7F8C_C1D6)
}
