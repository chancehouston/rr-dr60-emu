//! The frame packer and unpacker (spec 008 FR-003, FR-010; contracts/codec-design.md › Bit
//! order): 80 bits in 10 bytes, most significant bit of byte 0 first, the fields in the order
//! and widths of `tables::ALLOCATION` (LSP 1 … 8, lag 1, codebook 1, adaptive gain 1, fixed gain
//! 1, lag delta 2, codebook 2, adaptive gain 2, fixed gain 2). One table drives both directions.

use super::tables::{ALLOCATION, BITS_PER_FRAME, FRAME_BYTES, LPC_ORDER};

/// One encoded frame: 80 bits in 10 bytes (`speech_codec::Frame`).
pub(crate) type Frame = [u8; FRAME_BYTES];

/// The parameters of one frame, each within its field's width.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct FrameParams {
    /// The eight LSP indices (3, 3, 3, 3, 3, 3, 2, 2 bits).
    pub(crate) lsp: [u8; LPC_ORDER],
    /// Subframe 1's lag code (7 bits; 127 unused).
    pub(crate) lag1: u8,
    /// Subframe 1's codebook code (17 bits).
    pub(crate) codebook1: u32,
    /// Subframe 1's adaptive-gain index (3 bits).
    pub(crate) adaptive_gain1: u8,
    /// Subframe 1's fixed-gain correction index (3 bits).
    pub(crate) fixed_gain1: u8,
    /// Subframe 2's lag delta code (5 bits).
    pub(crate) lag_delta2: u8,
    /// Subframe 2's codebook code (17 bits).
    pub(crate) codebook2: u32,
    /// Subframe 2's adaptive-gain index (3 bits).
    pub(crate) adaptive_gain2: u8,
    /// Subframe 2's fixed-gain correction index (3 bits).
    pub(crate) fixed_gain2: u8,
}

/// The number of fields.
const FIELDS: usize = ALLOCATION.len();

impl FrameParams {
    /// The fields in frame order.
    fn fields(&self) -> [u32; FIELDS] {
        let mut f = [0u32; FIELDS];
        for (o, &v) in f.iter_mut().zip(&self.lsp) {
            *o = u32::from(v);
        }
        f[8] = u32::from(self.lag1);
        f[9] = self.codebook1;
        f[10] = u32::from(self.adaptive_gain1);
        f[11] = u32::from(self.fixed_gain1);
        f[12] = u32::from(self.lag_delta2);
        f[13] = self.codebook2;
        f[14] = u32::from(self.adaptive_gain2);
        f[15] = u32::from(self.fixed_gain2);
        f
    }

    fn from_fields(f: &[u32; FIELDS]) -> Self {
        let mut p = Self::default();
        for (o, &v) in p.lsp.iter_mut().zip(&f[..LPC_ORDER]) {
            // range-proven: masked to the field's width (≤ 3 bits) by the unpacker.
            *o = v as u8;
        }
        p.lag1 = f[8] as u8;
        p.codebook1 = f[9];
        p.adaptive_gain1 = f[10] as u8;
        p.fixed_gain1 = f[11] as u8;
        p.lag_delta2 = f[12] as u8;
        p.codebook2 = f[13];
        p.adaptive_gain2 = f[14] as u8;
        p.fixed_gain2 = f[15] as u8;
        p
    }
}

/// The mask of a field of `bits` (1 … 31) bits.
fn mask(bits: u8) -> u32 {
    1u32.wrapping_shl(u32::from(bits)).wrapping_sub(1)
}

/// Packs the parameters (each masked to its width) into a frame, MSB first.
pub(crate) fn pack(p: &FrameParams) -> Frame {
    let mut frame = [0u8; FRAME_BYTES];
    let mut bit = 0usize; // the next bit to write, 0 = MSB of byte 0
    for (&value, &bits) in p.fields().iter().zip(ALLOCATION.iter()) {
        let value = value & mask(bits);
        for i in (0..bits).rev() {
            let b = value.wrapping_shr(u32::from(i)) & 1;
            if b == 1 {
                let byte = bit.wrapping_shr(3);
                let shift = 7u32.wrapping_sub((bit & 7) as u32);
                frame[byte] |= 1u8.wrapping_shl(shift);
            }
            bit = bit.wrapping_add(1);
        }
    }
    debug_assert_eq!(bit, BITS_PER_FRAME);
    frame
}

/// Unpacks a frame into its parameters.
pub(crate) fn unpack(frame: &Frame) -> FrameParams {
    let mut fields = [0u32; FIELDS];
    let mut bit = 0usize;
    for (f, &bits) in fields.iter_mut().zip(ALLOCATION.iter()) {
        let mut value = 0u32;
        for _ in 0..bits {
            let byte = bit.wrapping_shr(3);
            let shift = 7u32.wrapping_sub((bit & 7) as u32);
            let b = u32::from(frame[byte].wrapping_shr(shift) & 1);
            value = value.wrapping_shl(1) | b;
            bit = bit.wrapping_add(1);
        }
        *f = value;
    }
    FrameParams::from_fields(&fields)
}

/// 008 T013 (spec 008 FR-003, FR-010; contracts/codec-design.md › Bit order): the allocation
/// sums to 80, every field round-trips at 0 and at its maximum, byte 0's MSB is the first LSP
/// bit, and the fields sit at the documented offsets.
#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, clippy::disallowed_methods)] // test arithmetic
mod tests {
    use super::*;

    #[test]
    fn allocation_sums_to_80() {
        assert_eq!(ALLOCATION.iter().map(|&b| u32::from(b)).sum::<u32>(), 80);
        assert_eq!(
            ALLOCATION,
            [3, 3, 3, 3, 3, 3, 2, 2, 7, 17, 3, 3, 5, 17, 3, 3]
        );
        assert_eq!(FRAME_BYTES * 8, BITS_PER_FRAME);
    }

    #[test]
    fn every_field_round_trips_at_zero_and_maximum() {
        let zero = FrameParams::default();
        assert_eq!(pack(&zero), [0u8; 10]);
        assert_eq!(unpack(&[0u8; 10]), zero);
        let max = FrameParams {
            lsp: [7, 7, 7, 7, 7, 7, 3, 3],
            lag1: 127,
            codebook1: (1 << 17) - 1,
            adaptive_gain1: 7,
            fixed_gain1: 7,
            lag_delta2: 31,
            codebook2: (1 << 17) - 1,
            adaptive_gain2: 7,
            fixed_gain2: 7,
        };
        assert_eq!(pack(&max), [0xffu8; 10]);
        assert_eq!(unpack(&[0xffu8; 10]), max);
        // Each field alone at its maximum, the rest zero.
        let fields = max.fields();
        for i in 0..FIELDS {
            let mut one = [0u32; FIELDS];
            one[i] = fields[i];
            let p = FrameParams::from_fields(&one);
            assert_eq!(unpack(&pack(&p)), p, "field {i}");
            let ones = pack(&p).iter().map(|b| b.count_ones()).sum::<u32>();
            assert_eq!(
                ones,
                u32::from(ALLOCATION[i]),
                "field {i} occupies its width"
            );
        }
        // An over-wide value is masked.
        let mut wide = zero;
        wide.lsp[7] = 0xff;
        wide.lag1 = 0xff;
        let p = unpack(&pack(&wide));
        assert_eq!((p.lsp[7], p.lag1), (3, 127));
    }

    #[test]
    fn byte_0_msb_is_the_first_lsp_bit_and_offsets_follow_the_allocation() {
        let mut p = FrameParams::default();
        p.lsp[0] = 4; // 100b
        assert_eq!(pack(&p)[0], 0x80);
        p = FrameParams::default();
        p.lsp[1] = 1;
        assert_eq!(pack(&p)[0], 0x04, "LSP 2 occupies bits 6 … 4 of byte 0");
        // The lag code starts at bit 22: byte 2, bit position 5 (0x20) for its MSB.
        p = FrameParams::default();
        p.lag1 = 64;
        assert_eq!(pack(&p)[2], 0x02);
        // The last field (fixed gain 2) ends at bit 79: byte 9's LSB.
        p = FrameParams::default();
        p.fixed_gain2 = 1;
        assert_eq!(pack(&p)[9], 0x01);
        // A scattered pattern round-trips.
        let scattered = FrameParams {
            lsp: [1, 2, 3, 4, 5, 6, 1, 2],
            lag1: 42,
            codebook1: 0x15a5a,
            adaptive_gain1: 5,
            fixed_gain1: 2,
            lag_delta2: 17,
            codebook2: 0x0a5a5,
            adaptive_gain2: 3,
            fixed_gain2: 6,
        };
        assert_eq!(unpack(&pack(&scattered)), scattered);
    }
}
