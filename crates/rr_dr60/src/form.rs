//! The exported recording form, version 2 (spec 008 FR-010 – FR-012; research R-07;
//! contracts/recording-form.md): a 48-byte little-endian header that declares the recording's
//! **encoding** (speech-codec frames, or device-rate `f32` samples when the codec was
//! bypassed) and, for frames, the codec's **model version**; then the units; then a trailing
//! CRC-32. Hosts store or send the bytes unchanged and never parse them.
//!
//! Version 1 (spec 006's device-rate samples with a 32-byte header) never shipped; this
//! release refuses it with [`Error::UnsupportedFormVersion`] `{ version: 1 }` (spec 008
//! FR-011), as it refuses any later version.
//!
//! **Frozen decoders** (spec 008 FR-011): from the 1.0 release on, every codec model version a
//! released library wrote keeps its decoder in the library and the importer dispatches on the
//! header's model version field, so a 1.x engine decodes every 1.0+ form bit-identically to the
//! release that wrote it. Before 1.0 a model version may be dropped with a CHANGELOG entry; a
//! sequence that needs a dropped decoder is refused as [`Error::UnsupportedCodecModelVersion`].
//! This release decodes [`crate::codec::MODEL_VERSION`] only.
//!
//! `no_std`, no allocation.

use crate::codec;
use crate::error::Error;
use crate::memory::Encoding;
use crate::settings::DEVICE_RATE_HZ;

/// The magic at offset 0.
pub(crate) const MAGIC: [u8; 8] = *b"RRDR60RC";
/// The form version this engine writes and accepts.
pub(crate) const FORM_VERSION: u16 = 2; // contracts/recording-form.md (008 FR-010)
/// The header: magic, version, flags, device rate, library version, encoding, codec model
/// version, reserved, frame samples, bits per frame, reserved, K.
const HEADER_LEN: usize = 48; // contracts/recording-form.md (008 FR-010)
/// Header plus the trailing CRC-32.
const OVERHEAD: usize = HEADER_LEN + 4; // contracts/recording-form.md (008 FR-010)
/// The header fields' offsets (contracts/recording-form.md › Layout; 008 FR-010).
const OFF_VERSION: usize = 8; // contracts/recording-form.md (008 FR-010)
const OFF_FLAGS: usize = 10; // contracts/recording-form.md (008 FR-010)
const OFF_RATE: usize = 12; // contracts/recording-form.md (008 FR-010)
const OFF_LIB_VERSION: usize = 16; // contracts/recording-form.md (008 FR-010)
const OFF_ENCODING: usize = 22; // contracts/recording-form.md (008 FR-010)
const OFF_MODEL_VERSION: usize = 24; // contracts/recording-form.md (008 FR-010)
const OFF_RESERVED_1: usize = 30; // contracts/recording-form.md (008 FR-010)
const OFF_FRAME_SAMPLES: usize = 32; // contracts/recording-form.md (008 FR-010)
const OFF_BITS: usize = 34; // contracts/recording-form.md (008 FR-010)
const OFF_RESERVED_2: usize = 36; // contracts/recording-form.md (008 FR-010)
const OFF_COUNT: usize = 40; // contracts/recording-form.md (008 FR-010)
/// The IEEE 802.3 polynomial, reflected (research R-04).
const CRC_POLY: u32 = 0xEDB8_8320; // contracts/recording-form.md (006 FR-014)

/// The exporting library's version, for the informational header fields.
const LIB_VERSION: [u16; 3] = [
    parse_u16(env!("CARGO_PKG_VERSION_MAJOR")),
    parse_u16(env!("CARGO_PKG_VERSION_MINOR")),
    parse_u16(env!("CARGO_PKG_VERSION_PATCH")),
];

/// A decimal string to `u16` at compile time (the version parts).
const fn parse_u16(s: &str) -> u16 {
    let b = s.as_bytes();
    let mut v: u16 = 0;
    let mut i = 0;
    while i < b.len() {
        v = v * 10 + (b[i] - b'0') as u16;
        i += 1;
    }
    v
}

/// The byte-wise CRC-32 table, built at compile time (research R-04).
static CRC_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ CRC_POLY
            } else {
                c >> 1
            };
            k += 1;
        }
        table[n] = c;
        n += 1;
    }
    table
};

/// CRC-32 (IEEE 802.3): reflected polynomial 0xEDB88320, initial 0xFFFFFFFF, final XOR.
pub(crate) fn crc32(bytes: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in bytes {
        c = CRC_TABLE[((c ^ u32::from(b)) & 0xFF) as usize] ^ (c >> 8);
    }
    !c
}

/// A recording's stored units, as the memory holds them (spec 008 FR-010, FR-012).
#[derive(Clone, Copy, Debug)]
pub(crate) enum Payload<'a> {
    /// Speech-codec frames, 10 bytes each, verbatim.
    Frames(&'a [u8]),
    /// Device-rate samples (the codec bypassed).
    Samples(&'a [f32]),
}

impl Payload<'_> {
    /// The encoding the payload is in.
    pub(crate) fn encoding(&self) -> Encoding {
        match self {
            Payload::Frames(_) => Encoding::Frames,
            Payload::Samples(_) => Encoding::Samples,
        }
    }

    /// The unit count K.
    pub(crate) fn units(&self) -> usize {
        match self {
            Payload::Frames(b) => b.len() / Encoding::Frames.unit_bytes(),
            Payload::Samples(s) => s.len(),
        }
    }
}

/// A validated sequence: its encoding and its data bytes, `K × unit bytes` long.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Validated<'a> {
    /// The encoding the header declares.
    pub(crate) encoding: Encoding,
    /// The units' bytes: the frames verbatim, or the samples' little-endian bit patterns
    /// ([`decode`] reads them).
    pub(crate) units: &'a [u8],
}

/// The exported length of a recording of `k` units in `encoding`: 52 + 10·k for frames,
/// 52 + 4·k for samples.
pub(crate) fn export_len(encoding: Encoding, k: usize) -> usize {
    OVERHEAD + encoding.unit_bytes() * k
}

/// Writes the form of `payload` into `out`: the header, the units little-endian, the CRC.
/// Returns the bytes written (`export_len`). Allocates nothing; the CRC pass is bounded by
/// the recording's size (not real-time safe, spec 006 FR-017). The same payload exports to
/// the same bytes every time.
///
/// # Errors
///
/// [`Error::BufferTooSmall`] with the needed length when `out` is shorter; `out` is untouched.
pub(crate) fn export(payload: Payload<'_>, out: &mut [u8]) -> Result<usize, Error> {
    let encoding = payload.encoding();
    let k = payload.units();
    let needed = export_len(encoding, k);
    if out.len() < needed {
        return Err(Error::BufferTooSmall { needed });
    }
    let out = &mut out[..needed];
    out[..MAGIC.len()].copy_from_slice(&MAGIC);
    out[OFF_VERSION..OFF_FLAGS].copy_from_slice(&FORM_VERSION.to_le_bytes());
    out[OFF_FLAGS..OFF_RATE].copy_from_slice(&0u16.to_le_bytes());
    out[OFF_RATE..OFF_LIB_VERSION].copy_from_slice(&DEVICE_RATE_HZ.to_le_bytes()); // A-026
    for (i, part) in LIB_VERSION.iter().enumerate() {
        let at = OFF_LIB_VERSION + 2 * i;
        out[at..at + 2].copy_from_slice(&part.to_le_bytes());
    }
    out[OFF_ENCODING..OFF_MODEL_VERSION].copy_from_slice(&encoding.form_code().to_le_bytes());
    // The codec's model version for frames; 0, 0, 0 for samples (contracts/recording-form.md).
    let (model, frame_samples, bits): ([u16; 3], u16, u16) = match encoding {
        Encoding::Frames => (
            codec::MODEL_VERSION,
            codec::FRAME_SAMPLES as u16,
            codec::BITS_PER_FRAME as u16,
        ),
        Encoding::Samples => ([0; 3], 0, 0),
    };
    for (i, part) in model.iter().enumerate() {
        let at = OFF_MODEL_VERSION + 2 * i;
        out[at..at + 2].copy_from_slice(&part.to_le_bytes());
    }
    out[OFF_RESERVED_1..OFF_FRAME_SAMPLES].copy_from_slice(&0u16.to_le_bytes());
    out[OFF_FRAME_SAMPLES..OFF_BITS].copy_from_slice(&frame_samples.to_le_bytes());
    out[OFF_BITS..OFF_RESERVED_2].copy_from_slice(&bits.to_le_bytes());
    out[OFF_RESERVED_2..OFF_COUNT].copy_from_slice(&0u32.to_le_bytes());
    out[OFF_COUNT..HEADER_LEN].copy_from_slice(&(k as u64).to_le_bytes());
    match payload {
        Payload::Frames(frames) => out[HEADER_LEN..needed - 4].copy_from_slice(frames),
        Payload::Samples(samples) => {
            for (slot, s) in out[HEADER_LEN..needed - 4].chunks_exact_mut(4).zip(samples) {
                slot.copy_from_slice(&s.to_le_bytes());
            }
        }
    }
    let crc = crc32(&out[..needed - 4]);
    out[needed - 4..].copy_from_slice(&crc.to_le_bytes());
    Ok(needed)
}

fn read_u16(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn read_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().expect("4 bytes"))
}

/// Checks `bytes` in the contract's order (contracts/recording-form.md › Validation) and
/// returns the encoding and the unit bytes (K ≥ 1) when every check passes. Never reads past
/// the slice and never computes 52 + K × unit bytes in the slice's width (spec 006 plan review
/// P3: K is compared against the body as 64-bit). The library version fields are not checked.
/// The caller then checks the encoding against the engine's ([`Error::EncodingDiffers`])
/// before the fit ([`Error::DoesNotFit`]).
///
/// # Errors
///
/// [`Error::UnknownForm`] (shorter than the magic, or another magic);
/// [`Error::UnsupportedFormVersion`] (a version other than 2, version 1 included, or non-zero
/// flags); [`Error::Damaged`] (truncated inside the header, an encoding other than frames or
/// samples, a non-zero reserved field, another device rate, K = 0, frame samples or bits per
/// frame other than the codec's for frames or a model version, frame samples or bits for
/// samples, a length that is
/// not 52 + K × unit bytes, a CRC mismatch, a sample that is NaN, infinite or subnormal, or a
/// frame that uses a code point the encoder never produces);
/// [`Error::UnsupportedCodecModelVersion`] (frames of a model version this release does not
/// decode; checked before the CRC so a host learns the real reason).
pub(crate) fn validate(bytes: &[u8]) -> Result<Validated<'_>, Error> {
    if bytes.len() < MAGIC.len() || bytes[..MAGIC.len()] != MAGIC {
        return Err(Error::UnknownForm);
    }
    if bytes.len() < OFF_RATE {
        return Err(Error::Damaged); // truncated inside the version or flags field
    }
    let version = read_u16(bytes, OFF_VERSION);
    let flags = read_u16(bytes, OFF_FLAGS);
    if version != FORM_VERSION || flags != 0 {
        return Err(Error::UnsupportedFormVersion { version });
    }
    if bytes.len() < OVERHEAD {
        return Err(Error::Damaged); // the length check precedes reading the rest
    }
    let encoding = Encoding::from_form_code(read_u16(bytes, OFF_ENCODING)).ok_or(Error::Damaged)?;
    if read_u16(bytes, OFF_RESERVED_1) != 0 || read_u32(bytes, OFF_RESERVED_2) != 0 {
        return Err(Error::Damaged);
    }
    if read_u32(bytes, OFF_RATE) != DEVICE_RATE_HZ {
        return Err(Error::Damaged); // A-026
    }
    let k = u64::from_le_bytes(bytes[OFF_COUNT..HEADER_LEN].try_into().expect("8 bytes"));
    if k == 0 {
        return Err(Error::Damaged);
    }
    let model = [
        read_u16(bytes, OFF_MODEL_VERSION),
        read_u16(bytes, OFF_MODEL_VERSION + 2),
        read_u16(bytes, OFF_MODEL_VERSION + 4),
    ];
    let frame_samples = read_u16(bytes, OFF_FRAME_SAMPLES);
    let bits = read_u16(bytes, OFF_BITS);
    match encoding {
        Encoding::Frames => {
            if u32::from(frame_samples) != codec::FRAME_SAMPLES
                || u32::from(bits) != codec::BITS_PER_FRAME
            {
                return Err(Error::Damaged);
            }
        }
        Encoding::Samples => {
            if model != [0; 3] || frame_samples != 0 || bits != 0 {
                return Err(Error::Damaged);
            }
        }
    }
    let body = (bytes.len() - OVERHEAD) as u64;
    let unit_bytes = encoding.unit_bytes() as u64;
    if k.checked_mul(unit_bytes) != Some(body) {
        return Err(Error::Damaged);
    }
    if encoding == Encoding::Frames && model != codec::MODEL_VERSION {
        return Err(Error::UnsupportedCodecModelVersion {
            major: model[0],
            minor: model[1],
            patch: model[2],
        });
    }
    let (content, trailer) = bytes.split_at(bytes.len() - 4);
    let stated = u32::from_le_bytes(trailer.try_into().expect("4 bytes"));
    if crc32(content) != stated {
        return Err(Error::Damaged);
    }
    let units = &content[HEADER_LEN..];
    match encoding {
        // Finite and non-subnormal: the record path's output range (contracts/recording-form.md).
        Encoding::Samples => {
            if !decode(units).all(|v| v == 0.0 || v.is_normal()) {
                return Err(Error::Damaged);
            }
        }
        // No frame uses the lag code the encoder never produces (spec 008 FR-010).
        Encoding::Frames => {
            let unused = units
                .chunks_exact(codec::FRAME_BYTES as usize)
                .any(|f| codec::uses_unused_code(f.try_into().expect("10 bytes")));
            if unused {
                return Err(Error::Damaged);
            }
        }
    }
    Ok(Validated { encoding, units })
}

/// The samples of validated sample bytes, in order, bit-exact.
pub(crate) fn decode(sample_bytes: &[u8]) -> impl ExactSizeIterator<Item = f32> + '_ {
    sample_bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
}

/// 006 T032 and 008 T024 (spec 008 FR-010, FR-011; contracts/recording-form.md; spec 006 plan
/// review P3).
#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::error::Error;
    use alloc::vec;
    use alloc::vec::Vec;

    /// The form of `payload`, through `export` into an exactly sized buffer.
    fn exported(payload: Payload<'_>) -> Vec<u8> {
        let mut out = vec![0u8; export_len(payload.encoding(), payload.units())];
        let n = export(payload, &mut out).unwrap();
        assert_eq!(n, out.len());
        out
    }

    /// `bytes` with its trailing CRC recomputed over the rest.
    fn with_crc(mut bytes: Vec<u8>) -> Vec<u8> {
        let n = bytes.len();
        let c = crc32(&bytes[..n - 4]);
        bytes[n - 4..].copy_from_slice(&c.to_le_bytes());
        bytes
    }

    fn decoded(sample_bytes: &[u8]) -> Vec<f32> {
        decode(sample_bytes).collect()
    }

    /// Three legal frames: all zeros (lag code 0), one with every field at a legal maximum
    /// (lag code 126 at bits 22 … 28), and one arbitrary pattern.
    const FRAMES: [u8; 30] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
        0xFF, 0xFF, 0xFF, 0xF7, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, //
        0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x0F, 0x1E,
    ];
    /// A frame whose subframe-1 lag code is 127 (bits 22 … 28 all set).
    const LAG_127: [u8; 10] = [0, 0, 0x03, 0xF8, 0, 0, 0, 0, 0, 0];

    fn good_frames() -> Vec<u8> {
        exported(Payload::Frames(&FRAMES))
    }

    fn good_samples() -> Vec<u8> {
        exported(Payload::Samples(&[0.5f32, -0.25]))
    }

    /// The IEEE 802.3 check value, and the empty message.
    #[test]
    fn crc32_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    fn lib_version() -> [u16; 3] {
        let mut parts = crate::VERSION.split('.').map(|p| p.parse::<u16>().unwrap());
        [
            parts.next().unwrap(),
            parts.next().unwrap(),
            parts.next().unwrap(),
        ]
    }

    /// The layout of contracts/recording-form.md for both encodings: magic at 0, version 2 at
    /// 8, flags 0 at 10, 6000 at 12, the library version at 16/18/20, the encoding at 22, the
    /// codec model version at 24/26/28 (0, 0, 0 for samples), reserved 0 at 30, frame samples
    /// at 32 and bits per frame at 34 (120 / 80 for frames, 0 / 0 for samples), reserved 0 at
    /// 36, K at 40, the units at 48, the CRC last.
    #[test]
    fn header_is_48_bytes_version_2() {
        let lib = lib_version();
        let b = good_frames();
        assert_eq!(b.len(), 52 + 10 * 3);
        assert_eq!(&b[0..8], b"RRDR60RC");
        assert_eq!(read_u16(&b, 8), 2);
        assert_eq!(read_u16(&b, 10), 0);
        assert_eq!(read_u32(&b, 12), 6000);
        assert_eq!([read_u16(&b, 16), read_u16(&b, 18), read_u16(&b, 20)], lib);
        assert_eq!(read_u16(&b, 22), 1, "frames");
        assert_eq!(
            [read_u16(&b, 24), read_u16(&b, 26), read_u16(&b, 28)],
            codec::MODEL_VERSION
        );
        assert_eq!(codec::MODEL_VERSION, [0, 8, 0]);
        assert_eq!(read_u16(&b, 30), 0);
        assert_eq!(read_u16(&b, 32), 120);
        assert_eq!(read_u16(&b, 34), 80);
        assert_eq!(read_u32(&b, 36), 0);
        assert_eq!(u64::from_le_bytes(b[40..48].try_into().unwrap()), 3);
        assert_eq!(&b[48..78], &FRAMES[..], "the frames verbatim");
        assert_eq!(read_u32(&b, 78), crc32(&b[..78]));
        // The same frames export to the same bytes every time.
        assert_eq!(good_frames(), b);

        let samples = [0.25f32, -0.5, 1.0];
        let b = exported(Payload::Samples(&samples));
        assert_eq!(b.len(), 52 + 4 * 3);
        assert_eq!(read_u16(&b, 8), 2);
        assert_eq!(read_u16(&b, 22), 2, "samples");
        assert_eq!(
            [read_u16(&b, 24), read_u16(&b, 26), read_u16(&b, 28)],
            [0, 0, 0]
        );
        assert_eq!((read_u16(&b, 32), read_u16(&b, 34)), (0, 0));
        assert_eq!(u64::from_le_bytes(b[40..48].try_into().unwrap()), 3);
        for (i, s) in samples.iter().enumerate() {
            assert_eq!(&b[48 + 4 * i..52 + 4 * i], &s.to_le_bytes());
        }
        assert_eq!(read_u32(&b, 60), crc32(&b[..60]));
        assert_eq!(exported(Payload::Samples(&samples)), b);
    }

    /// `export_len`: 52 + 10K for frames and 52 + 4K for samples.
    #[test]
    fn export_len_frames_and_samples() {
        assert_eq!(export_len(Encoding::Frames, 0), 52);
        assert_eq!(export_len(Encoding::Frames, 1), 62);
        assert_eq!(export_len(Encoding::Frames, 180_000), 52 + 1_800_000);
        assert_eq!(export_len(Encoding::Samples, 1), 56);
        assert_eq!(export_len(Encoding::Samples, 21_600_000), 52 + 86_400_000);
        assert_eq!(Payload::Frames(&FRAMES).units(), 3);
        assert_eq!(Payload::Samples(&[0.0; 7]).units(), 7);
    }

    /// Frames come back verbatim; every sample bit pattern the record path can produce survives:
    /// −0.0, the largest and the smallest normal magnitudes, and ordinary values.
    #[test]
    fn round_trips_bit_exact() {
        let b = good_frames();
        let v = validate(&b).unwrap();
        assert_eq!(v.encoding, Encoding::Frames);
        assert_eq!(v.units, &FRAMES[..]);

        let samples = [
            0.0f32,
            -0.0,
            0.1,
            -0.1,
            f32::MAX,
            -f32::MAX,
            f32::MIN_POSITIVE,
            -f32::MIN_POSITIVE,
            1.0,
            -1.0,
        ];
        let b = exported(Payload::Samples(&samples));
        let v = validate(&b).unwrap();
        assert_eq!(v.encoding, Encoding::Samples);
        assert_eq!(v.units.len(), 4 * samples.len());
        let back = decoded(v.units);
        assert_eq!(back.len(), samples.len());
        for (a, want) in back.iter().zip(&samples) {
            assert_eq!(a.to_bits(), want.to_bits());
        }
    }

    /// A version 1 sequence (spec 006's 32-byte header) carrying `samples`, as 0.6 and 0.7
    /// would have written it.
    fn version_1(samples: &[f32]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&MAGIC);
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(&6000u32.to_le_bytes());
        for part in lib_version() {
            b.extend_from_slice(&part.to_le_bytes());
        }
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(&(samples.len() as u64).to_le_bytes());
        for s in samples {
            b.extend_from_slice(&s.to_le_bytes());
        }
        let crc = crc32(&b);
        b.extend_from_slice(&crc.to_le_bytes());
        assert_eq!(b.len(), 36 + 4 * samples.len());
        b
    }

    /// The refusals in the contract's order, each with a minimal input; no panic and no
    /// overflow on the short and the huge sequences.
    #[test]
    fn refusals_in_order() {
        let good = good_frames();
        let good_s = good_samples();
        // Unknown form.
        assert_eq!(validate(&[]).unwrap_err(), Error::UnknownForm);
        assert_eq!(validate(&good[..7]).unwrap_err(), Error::UnknownForm);
        let mut wrong_magic = good.clone();
        wrong_magic[0] = b'X';
        assert_eq!(validate(&wrong_magic).unwrap_err(), Error::UnknownForm);
        // Truncated inside the version field: damaged, not unknown.
        assert_eq!(validate(&good[..10]).unwrap_err(), Error::Damaged);
        // Version 1 (spec 006's never-shipped form), version 3, non-zero flags.
        assert_eq!(
            validate(&version_1(&[0.5, -0.25])).unwrap_err(),
            Error::UnsupportedFormVersion { version: 1 }
        );
        let mut v3 = good.clone();
        v3[8] = 3;
        assert_eq!(
            validate(&v3).unwrap_err(),
            Error::UnsupportedFormVersion { version: 3 }
        );
        let mut flags = good.clone();
        flags[10] = 1;
        assert_eq!(
            validate(&flags).unwrap_err(),
            Error::UnsupportedFormVersion { version: 2 }
        );
        // A valid magic and version but shorter than the header and CRC: the length check
        // precedes reading the rest (every length from 12 to 51).
        for at in 12..52 {
            assert_eq!(validate(&good[..at]).unwrap_err(), Error::Damaged, "{at}");
        }
        // Encoding 3 (and 0).
        for code in [0u16, 3] {
            let mut enc = good.clone();
            enc[22..24].copy_from_slice(&code.to_le_bytes());
            assert_eq!(validate(&with_crc(enc)).unwrap_err(), Error::Damaged);
        }
        // Non-zero reserved fields.
        let mut r1 = good.clone();
        r1[31] = 1;
        assert_eq!(validate(&with_crc(r1)).unwrap_err(), Error::Damaged);
        let mut r2 = good.clone();
        r2[38] = 1;
        assert_eq!(validate(&with_crc(r2)).unwrap_err(), Error::Damaged);
        // Device rate 8000.
        let mut rate = good.clone();
        rate[12..16].copy_from_slice(&8000u32.to_le_bytes());
        assert_eq!(validate(&with_crc(rate)).unwrap_err(), Error::Damaged);
        // K = 0: a header and a CRC only.
        let mut empty = good[..48].to_vec();
        empty[40..48].copy_from_slice(&0u64.to_le_bytes());
        empty.extend_from_slice(&[0, 0, 0, 0]);
        assert_eq!(validate(&with_crc(empty)).unwrap_err(), Error::Damaged);
        // Frame samples 119, bits 79.
        let mut fs = good.clone();
        fs[32..34].copy_from_slice(&119u16.to_le_bytes());
        assert_eq!(validate(&with_crc(fs)).unwrap_err(), Error::Damaged);
        let mut bits = good.clone();
        bits[34..36].copy_from_slice(&79u16.to_le_bytes());
        assert_eq!(validate(&with_crc(bits)).unwrap_err(), Error::Damaged);
        // A samples sequence with a model version, with frame samples, with bits.
        let mut sm = good_s.clone();
        sm[26] = 8;
        assert_eq!(validate(&with_crc(sm)).unwrap_err(), Error::Damaged);
        let mut sfs = good_s.clone();
        sfs[32..34].copy_from_slice(&120u16.to_le_bytes());
        assert_eq!(validate(&with_crc(sfs)).unwrap_err(), Error::Damaged);
        let mut sbits = good_s.clone();
        sbits[34..36].copy_from_slice(&80u16.to_le_bytes());
        assert_eq!(validate(&with_crc(sbits)).unwrap_err(), Error::Damaged);
        // Length arithmetic: truncated by one byte, by part of a frame, by a whole frame
        // (K then disagrees), K off by one with a valid CRC, K = 2^62 (no overflow).
        assert_eq!(
            validate(&good[..good.len() - 1]).unwrap_err(),
            Error::Damaged
        );
        assert_eq!(
            validate(&good[..good.len() - 7]).unwrap_err(),
            Error::Damaged
        );
        assert_eq!(
            validate(&good[..good.len() - 10]).unwrap_err(),
            Error::Damaged
        );
        let mut k_off = good.clone();
        k_off[40] = 2;
        assert_eq!(validate(&with_crc(k_off)).unwrap_err(), Error::Damaged);
        let mut huge = good.clone();
        huge[40..48].copy_from_slice(&(1u64 << 62).to_le_bytes());
        assert_eq!(validate(&with_crc(huge)).unwrap_err(), Error::Damaged);
        // A codec model version this release does not decode: 0.7.0 (checked before the CRC,
        // so the stale CRC does not hide the reason).
        let mut model = good.clone();
        model[24..30].copy_from_slice(&[0, 0, 7, 0, 0, 0]);
        assert_eq!(
            validate(&model).unwrap_err(),
            Error::UnsupportedCodecModelVersion {
                major: 0,
                minor: 7,
                patch: 0
            }
        );
        assert_eq!(
            validate(&with_crc(model)).unwrap_err(),
            Error::UnsupportedCodecModelVersion {
                major: 0,
                minor: 7,
                patch: 0
            }
        );
        // CRC flipped; a payload byte flipped (the CRC catches it) in both encodings.
        let mut crc = good.clone();
        let n = crc.len();
        crc[n - 1] ^= 0x01;
        assert_eq!(validate(&crc).unwrap_err(), Error::Damaged);
        let mut payload = good.clone();
        payload[49] ^= 0x80;
        assert_eq!(validate(&payload).unwrap_err(), Error::Damaged);
        let mut payload_s = good_s.clone();
        payload_s[49] ^= 0x80;
        assert_eq!(validate(&payload_s).unwrap_err(), Error::Damaged);
        // Out-of-range samples with a recomputed CRC.
        for bad in [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            1.0e-40,
            -1.0e-40,
        ] {
            let mut b = good_s.clone();
            b[52..56].copy_from_slice(&bad.to_le_bytes());
            assert_eq!(validate(&with_crc(b)).unwrap_err(), Error::Damaged, "{bad}");
        }
        // A frame with lag code 127, in any position, with a recomputed CRC.
        assert!(codec::uses_unused_code(&LAG_127));
        for at in 0..3 {
            let mut b = good.clone();
            b[48 + 10 * at..58 + 10 * at].copy_from_slice(&LAG_127);
            assert_eq!(validate(&with_crc(b)).unwrap_err(), Error::Damaged, "{at}");
        }
        // The library version fields never refuse.
        let mut lib = good.clone();
        lib[16..22].copy_from_slice(&[9, 9, 9, 9, 9, 9]);
        assert!(validate(&with_crc(lib)).is_ok());
        assert!(validate(&good).is_ok());
        assert!(validate(&good_s).is_ok());
    }

    /// `export` into a buffer one byte short is `BufferTooSmall { needed }` with the buffer
    /// untouched; a longer buffer takes exactly `needed` bytes; both encodings.
    #[test]
    fn export_into_short_buffer() {
        let samples = [0.5f32; 7];
        let needed = export_len(Encoding::Samples, 7);
        let mut short = vec![0xAAu8; needed - 1];
        assert_eq!(
            export(Payload::Samples(&samples), &mut short),
            Err(Error::BufferTooSmall { needed })
        );
        assert!(short.iter().all(|&b| b == 0xAA));
        let mut long = vec![0xAAu8; needed + 5];
        assert_eq!(export(Payload::Samples(&samples), &mut long), Ok(needed));
        assert_eq!(&long[..needed], &exported(Payload::Samples(&samples))[..]);
        assert!(long[needed..].iter().all(|&b| b == 0xAA));
        let needed_f = export_len(Encoding::Frames, 3);
        let mut short_f = vec![0xAAu8; needed_f - 1];
        assert_eq!(
            export(Payload::Frames(&FRAMES), &mut short_f),
            Err(Error::BufferTooSmall { needed: needed_f })
        );
        assert!(short_f.iter().all(|&b| b == 0xAA));
        assert_eq!(
            export(Payload::Frames(&[]), &mut long),
            Ok(export_len(Encoding::Frames, 0)),
            "K = 0 exports; import refuses it"
        );
    }
}
