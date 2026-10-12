//! The exported recording form, version 1 (spec 006 FR-014; research R-04;
//! contracts/recording-form.md): a 32-byte little-endian header, the device-rate `f32` samples
//! and a trailing CRC-32. Hosts store or send the bytes unchanged and never parse them.
//!
//! This version stores the record path's device-rate output after the VAS (narrowed, FR-009);
//! it will become encoded frames when the codec is modeled (A-004), with the form version
//! telling the two apart. `no_std`, no allocation.

use crate::error::Error;
use crate::settings::DEVICE_RATE_HZ;

/// The magic at offset 0.
pub(crate) const MAGIC: [u8; 8] = *b"RRDR60RC";
/// The form version this engine writes and accepts.
pub(crate) const FORM_VERSION: u16 = 1; // contracts/recording-form.md (FR-014)
/// The header: magic, version, flags, device rate, library version, reserved, K.
const HEADER_LEN: usize = 32; // contracts/recording-form.md (FR-014)
/// Header plus the trailing CRC-32.
const OVERHEAD: usize = HEADER_LEN + 4; // contracts/recording-form.md (FR-014)
/// The IEEE 802.3 polynomial, reflected (research R-04).
const CRC_POLY: u32 = 0xEDB8_8320; // contracts/recording-form.md (FR-014)

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

/// The exported length of a recording of `k` samples: 36 + 4·k.
pub(crate) fn export_len(k: usize) -> usize {
    OVERHEAD + 4 * k
}

/// Writes the form of `samples` into `out`: the header, the samples little-endian, the CRC.
/// Returns the bytes written (`export_len`). Allocates nothing; the CRC pass is bounded by
/// the recording's size (not real-time safe, FR-017).
///
/// # Errors
///
/// [`Error::BufferTooSmall`] with the needed length when `out` is shorter; `out` is untouched.
pub(crate) fn export(samples: &[f32], out: &mut [u8]) -> Result<usize, Error> {
    let needed = export_len(samples.len());
    if out.len() < needed {
        return Err(Error::BufferTooSmall { needed });
    }
    let out = &mut out[..needed];
    out[0..8].copy_from_slice(&MAGIC);
    out[8..10].copy_from_slice(&FORM_VERSION.to_le_bytes());
    out[10..12].copy_from_slice(&0u16.to_le_bytes());
    out[12..16].copy_from_slice(&DEVICE_RATE_HZ.to_le_bytes()); // A-026
    out[16..18].copy_from_slice(&LIB_VERSION[0].to_le_bytes());
    out[18..20].copy_from_slice(&LIB_VERSION[1].to_le_bytes());
    out[20..22].copy_from_slice(&LIB_VERSION[2].to_le_bytes());
    out[22..24].copy_from_slice(&0u16.to_le_bytes());
    out[24..32].copy_from_slice(&(samples.len() as u64).to_le_bytes());
    for (slot, s) in out[HEADER_LEN..needed - 4].chunks_exact_mut(4).zip(samples) {
        slot.copy_from_slice(&s.to_le_bytes());
    }
    let crc = crc32(&out[..needed - 4]);
    out[needed - 4..].copy_from_slice(&crc.to_le_bytes());
    Ok(needed)
}

/// Checks `bytes` in the contract's order and returns the sample bytes (4·K, K ≥ 1) when
/// every check passes. Never reads past the slice and never computes 36 + 4·K (plan review
/// P3). The library version fields are not checked.
///
/// # Errors
///
/// [`Error::UnknownForm`] (shorter than the magic, or another magic);
/// [`Error::UnsupportedFormVersion`] (a version other than 1, or non-zero flags);
/// [`Error::Damaged`] (truncated inside the header, a length that is not 36 + 4·K for the
/// stated K, another device rate, K = 0, a CRC mismatch, or a sample that is NaN, infinite or
/// subnormal).
pub(crate) fn validate(bytes: &[u8]) -> Result<&[u8], Error> {
    if bytes.len() < MAGIC.len() || bytes[..8] != MAGIC {
        return Err(Error::UnknownForm);
    }
    if bytes.len() < 12 {
        return Err(Error::Damaged); // truncated inside the version field
    }
    let version = u16::from_le_bytes([bytes[8], bytes[9]]);
    let flags = u16::from_le_bytes([bytes[10], bytes[11]]);
    if version != FORM_VERSION || flags != 0 {
        return Err(Error::UnsupportedFormVersion { version });
    }
    if bytes.len() < OVERHEAD {
        return Err(Error::Damaged); // the length check precedes reading K
    }
    let body = bytes.len() - OVERHEAD;
    if body % 4 != 0 {
        return Err(Error::Damaged);
    }
    let k = u64::from_le_bytes(bytes[24..32].try_into().expect("8 bytes"));
    if k != (body / 4) as u64 || k == 0 {
        return Err(Error::Damaged);
    }
    let rate = u32::from_le_bytes(bytes[12..16].try_into().expect("4 bytes"));
    if rate != DEVICE_RATE_HZ {
        return Err(Error::Damaged); // A-026
    }
    let (content, trailer) = bytes.split_at(bytes.len() - 4);
    let stated = u32::from_le_bytes(trailer.try_into().expect("4 bytes"));
    if crc32(content) != stated {
        return Err(Error::Damaged);
    }
    let samples = &content[HEADER_LEN..];
    // Finite and non-subnormal: the record path's output range (contracts/recording-form.md).
    if !decode(samples).all(|v| v == 0.0 || v.is_normal()) {
        return Err(Error::Damaged);
    }
    Ok(samples)
}

/// The samples of validated sample bytes, in order, bit-exact.
pub(crate) fn decode(sample_bytes: &[u8]) -> impl Iterator<Item = f32> + '_ {
    sample_bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
}

/// 006 T032 (spec 006 FR-014; contracts/recording-form.md; plan review P3).
#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::error::Error;
    use alloc::vec;
    use alloc::vec::Vec;

    /// The form of `samples`, through `export` into an exactly sized buffer.
    fn exported(samples: &[f32]) -> Vec<u8> {
        let mut out = vec![0u8; export_len(samples.len())];
        let n = export(samples, &mut out).unwrap();
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

    /// The IEEE 802.3 check value, and the empty message.
    #[test]
    fn crc32_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    /// The layout of contracts/recording-form.md: magic at 0, version 1 at 8, flags 0 at 10,
    /// 6000 at 12, the library version at 16/18/20, reserved 0 at 22, K at 24, samples at 32,
    /// the CRC last; total 36 + 4K.
    #[test]
    fn header_layout() {
        let samples = [0.25f32, -0.5, 1.0];
        let b = exported(&samples);
        assert_eq!(b.len(), 36 + 4 * 3);
        assert_eq!(export_len(3), 48);
        assert_eq!(&b[0..8], b"RRDR60RC");
        assert_eq!(u16::from_le_bytes([b[8], b[9]]), 1);
        assert_eq!(u16::from_le_bytes([b[10], b[11]]), 0);
        assert_eq!(u32::from_le_bytes([b[12], b[13], b[14], b[15]]), 6000);
        let mut parts = crate::VERSION.split('.').map(|p| p.parse::<u16>().unwrap());
        let (major, minor, patch) = (
            parts.next().unwrap(),
            parts.next().unwrap(),
            parts.next().unwrap(),
        );
        assert_eq!(u16::from_le_bytes([b[16], b[17]]), major);
        assert_eq!(u16::from_le_bytes([b[18], b[19]]), minor);
        assert_eq!(u16::from_le_bytes([b[20], b[21]]), patch);
        assert_eq!(u16::from_le_bytes([b[22], b[23]]), 0);
        assert_eq!(u64::from_le_bytes(b[24..32].try_into().unwrap()), 3);
        for (i, s) in samples.iter().enumerate() {
            assert_eq!(&b[32 + 4 * i..36 + 4 * i], &s.to_le_bytes());
        }
        let crc = u32::from_le_bytes(b[44..48].try_into().unwrap());
        assert_eq!(crc, crc32(&b[..44]));
        // The same samples export to the same bytes every time (US3 scenario 1).
        assert_eq!(exported(&samples), b);
    }

    /// Every bit pattern that the record path can produce survives: −0.0, the largest and the
    /// smallest normal magnitudes, and ordinary values.
    #[test]
    fn round_trip_bit_exact() {
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
        let b = exported(&samples);
        let back = decoded(validate(&b).unwrap());
        assert_eq!(back.len(), samples.len());
        for (a, want) in back.iter().zip(&samples) {
            assert_eq!(a.to_bits(), want.to_bits());
        }
        assert_eq!(validate(&b).unwrap().len(), 4 * samples.len());
    }

    /// The refusals in the contract's order, each with a minimal input; no panic and no
    /// overflow on the short and the huge sequences.
    #[test]
    fn refusals_in_order() {
        let good = exported(&[0.5f32, -0.25]);
        // Unknown form.
        assert_eq!(validate(&[]), Err(Error::UnknownForm));
        assert_eq!(validate(&good[..7]), Err(Error::UnknownForm));
        let mut wrong_magic = good.clone();
        wrong_magic[0] = b'X';
        assert_eq!(validate(&wrong_magic), Err(Error::UnknownForm));
        // Unsupported version or flags.
        let mut v2 = good.clone();
        v2[8] = 2;
        assert_eq!(
            validate(&v2),
            Err(Error::UnsupportedFormVersion { version: 2 })
        );
        let mut flags = good.clone();
        flags[10] = 1;
        assert_eq!(
            validate(&flags),
            Err(Error::UnsupportedFormVersion { version: 1 })
        );
        // Truncated inside the version field: damaged, not unknown.
        assert_eq!(validate(&good[..10]), Err(Error::Damaged));
        // A 20-byte sequence with a valid magic and version: the length check precedes K.
        assert_eq!(validate(&good[..20]), Err(Error::Damaged));
        // Length not a multiple of 4 past the overhead.
        assert_eq!(validate(&good[..good.len() - 1]), Err(Error::Damaged));
        assert_eq!(validate(&good[..good.len() - 3]), Err(Error::Damaged));
        // K disagrees with the length (truncated by a whole sample, CRC then also wrong).
        assert_eq!(validate(&good[..good.len() - 4]), Err(Error::Damaged));
        let mut k_off = good.clone();
        k_off[24] = 1;
        assert_eq!(validate(&with_crc(k_off)), Err(Error::Damaged));
        // K = 2^62 with a short body: no overflow.
        let mut huge = good.clone();
        huge[24..32].copy_from_slice(&(1u64 << 62).to_le_bytes());
        assert_eq!(validate(&with_crc(huge)), Err(Error::Damaged));
        // Device rate 8000.
        let mut rate = good.clone();
        rate[12..16].copy_from_slice(&8000u32.to_le_bytes());
        assert_eq!(validate(&with_crc(rate)), Err(Error::Damaged));
        // K = 0: a header and a CRC only.
        let mut empty = good[..32].to_vec();
        empty[24..32].copy_from_slice(&0u64.to_le_bytes());
        empty.extend_from_slice(&[0, 0, 0, 0]);
        assert_eq!(validate(&with_crc(empty)), Err(Error::Damaged));
        // CRC flipped.
        let mut crc = good.clone();
        let n = crc.len();
        crc[n - 1] ^= 0x01;
        assert_eq!(validate(&crc), Err(Error::Damaged));
        // A payload byte flipped (the CRC catches it).
        let mut payload = good.clone();
        payload[33] ^= 0x80;
        assert_eq!(validate(&payload), Err(Error::Damaged));
        // Out-of-range samples with a recomputed CRC.
        for bad in [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            1.0e-40,
            -1.0e-40,
        ] {
            let mut b = good.clone();
            b[36..40].copy_from_slice(&bad.to_le_bytes());
            assert_eq!(validate(&with_crc(b)), Err(Error::Damaged), "{bad}");
        }
        // The library version fields never refuse.
        let mut lib = good.clone();
        lib[16..22].copy_from_slice(&[9, 9, 9, 9, 9, 9]);
        assert!(validate(&with_crc(lib)).is_ok());
        assert!(validate(&good).is_ok());
    }

    /// `export` into a buffer one byte short is `BufferTooSmall { needed }` with the buffer
    /// untouched; a longer buffer takes exactly `needed` bytes.
    #[test]
    fn export_into_short_buffer() {
        let samples = [0.5f32; 7];
        let needed = export_len(7);
        let mut short = vec![0xAAu8; needed - 1];
        assert_eq!(
            export(&samples, &mut short),
            Err(Error::BufferTooSmall { needed })
        );
        assert!(short.iter().all(|&b| b == 0xAA));
        let mut long = vec![0xAAu8; needed + 5];
        assert_eq!(export(&samples, &mut long), Ok(needed));
        assert_eq!(&long[..needed], &exported(&samples)[..]);
        assert!(long[needed..].iter().all(|&b| b == 0xAA));
        assert_eq!(
            export(&[], &mut long),
            Ok(export_len(0)),
            "K = 0 exports; import refuses it"
        );
    }
}
