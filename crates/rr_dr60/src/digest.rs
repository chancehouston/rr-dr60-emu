//! The 64-bit FNV-1a digest (spec 020 D3, FR-003; research R-03): the tie between a
//! processing record and the stored form it describes, and the record's own checksum.
//!
//! Each step is a bijection on the 64-bit state (an XOR with a byte, then a multiplication by
//! an odd constant), so two inputs of the same length that differ in one byte never collide;
//! unrelated inputs coincide with probability 2⁻⁶⁴; a deliberate collision is easy to make.
//! It is a consistency check, not a signature.

/// The FNV-1a 64 offset basis.
pub const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
/// The FNV-1a 64 prime.
pub const PRIME: u64 = 0x0000_0100_0000_01b3;

/// A streaming FNV-1a 64 state (the two-pass digest of a recording in memory, research R-03).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fnv1a64(u64);

impl Default for Fnv1a64 {
    fn default() -> Self {
        Self::new()
    }
}

impl Fnv1a64 {
    /// The state before any byte.
    #[must_use]
    pub const fn new() -> Self {
        Self(OFFSET_BASIS)
    }

    /// Feeds `bytes`.
    pub fn update(&mut self, bytes: &[u8]) {
        let mut h = self.0;
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(PRIME);
        }
        self.0 = h;
    }

    /// The digest so far.
    #[must_use]
    pub const fn finish(self) -> u64 {
        self.0
    }
}

/// The FNV-1a 64 digest of `bytes` (spec 020 FR-003). Allocates nothing; linear in the length.
#[must_use]
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h = Fnv1a64::new();
    h.update(bytes);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The published vectors (research R-03).
    #[test]
    fn published_vectors() {
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
    }

    /// Streaming in pieces equals one pass.
    #[test]
    fn streaming_equals_one_pass() {
        let bytes: [u8; 37] = core::array::from_fn(|i| (i * 7 + 3) as u8);
        let mut h = Fnv1a64::new();
        h.update(&bytes[..10]);
        h.update(&bytes[10..]);
        assert_eq!(h.finish(), fnv1a64(&bytes));
    }

    /// A one-byte difference on equal lengths always differs (the bijection).
    #[test]
    fn one_byte_difference_always_differs() {
        let base: [u8; 16] = core::array::from_fn(|i| i as u8);
        let h0 = fnv1a64(&base);
        for at in [0usize, 7, 15] {
            for v in 0..=255u8 {
                if v == base[at] {
                    continue;
                }
                let mut b = base;
                b[at] = v;
                assert_ne!(fnv1a64(&b), h0, "at {at} value {v}");
            }
        }
    }
}
