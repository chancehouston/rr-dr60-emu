//! The speech codec: signal-chain stages 7 (encoder) and 9 (decoder); spec 008.
//!
//! The service manual (S-002) sources the coder's class (CELP), bit rate (4 kbit/s) and
//! sampling rate (6 kHz, A-026); its algorithm is unknown, and this module is a **class
//! match**, not the device's coder (A-004, adopted by spec 008): an algebraic CELP coder of the
//! G.729 / G.723.1 family (S-009, S-008; secondary sources for the class) at 20 ms frames of
//! 80 bits, adapted to 6 kHz with every constant re-derived (A-040: frame length and
//! look-ahead; A-041: structure, bit allocation and post-processing; research R-02). Every
//! figure here is a design constant printed by `tools/celp-design/design_celp.py` and pinned
//! by a test; none is a measurement of a real unit.
//!
//! Arithmetic is fixed-point integer only (research R-03): Q15 samples and coefficients,
//! `i64` accumulators, saturating operators written in [`fixed`]; no floating point beyond the
//! `f32` ↔ Q15 conversions at the boundary, no `rr_dr60_detmath`, ties to the lowest index.
//! Every operation has explicit overflow semantics so debug and release builds agree.
//!
//! The encoder emits one 10-byte frame per 120 post-VAS samples once it holds the 60-sample
//! look-ahead (push 120(k+1) + 59, zero-based), so the codec's delay is 180 device samples
//! (FR-004); the decoder adds none. The record path stores the frames (spec 008 FR-010); the
//! one-pass processing runs encoder, decoder and an exact 180-sample delay line (research
//! R-06); playback decodes frame by frame.

#![deny(clippy::arithmetic_side_effects)]

// T010 (spec 008): the tables and constants land first; the encoder and decoder (T017 – T019)
// use them. Until then the generated tables are unused.
#[allow(dead_code)]
pub(crate) mod tables;

#[allow(dead_code)] // 008 T018: used by the encoder and decoder (T019)
pub(crate) mod acelp;
#[allow(dead_code)] // 008 T018: used by the encoder and decoder (T019)
pub(crate) mod bits;
#[allow(dead_code)] // 008 T017: used by the modules T018 – T019 add
pub(crate) mod fixed;
#[allow(dead_code)] // 008 T018: used by the encoder and decoder (T019)
pub(crate) mod gains;
#[allow(dead_code)] // 008 T018: used by the encoder and decoder (T019)
pub(crate) mod lpc;
#[allow(dead_code)] // 008 T018: used by the encoder and decoder (T019)
pub(crate) mod lsp;
#[allow(dead_code)] // 008 T018: used by the encoder and decoder (T019)
pub(crate) mod pitch;
#[allow(dead_code)] // 008 T018: used by the encoder and decoder (T019)
pub(crate) mod postfilter;

/// The device rate the codec runs at, in Hz (A-026).
pub const DEVICE_RATE_HZ: u32 = tables::DEVICE_RATE_HZ;
/// Frame length in device samples: 120 (20 ms). A design constant (A-040), not the device's.
pub const FRAME_SAMPLES: u32 = tables::FRAME_SAMPLES as u32;
/// Frame length in milliseconds: 20 (A-040).
pub const FRAME_MS: u32 = FRAME_SAMPLES * 1000 / DEVICE_RATE_HZ;
/// Subframe length in device samples: 60 (10 ms; two per frame).
pub const SUBFRAME_SAMPLES: u32 = tables::SUBFRAME_SAMPLES as u32;
/// Bits per frame: 80, so that 50 frames/s × 80 = 4000 bit/s exactly (S-002; spec 008 FR-003).
pub const BITS_PER_FRAME: u32 = tables::BITS_PER_FRAME as u32;
/// Bytes per frame: 10.
pub const FRAME_BYTES: u32 = tables::FRAME_BYTES as u32;
/// Look-ahead in device samples: 60 (10 ms). Defined by the encoder's push count: frame *k*
/// is emitted on the push of post-VAS sample 120(k+1) + 59 (A-040).
pub const LOOKAHEAD_SAMPLES: u32 = tables::LOOKAHEAD_SAMPLES as u32;
/// The codec's algorithmic delay in device samples: 180 (30 ms), frame plus look-ahead,
/// attributed to the record path and the one-pass processing (the decoder adds none); a
/// multiple of 20, so a whole number of host samples at every supported rate (FR-004).
pub const DELAY_SAMPLES: u32 = tables::DELAY_SAMPLES as u32;
/// Linear-prediction order: 8 (A-041: three formants fit the 225–2550 Hz band).
pub const LPC_ORDER: u32 = tables::LPC_ORDER as u32;
/// Shortest pitch lag in device samples (400 Hz; A-041, S-009's range kept in hertz).
pub const MIN_LAG: u32 = tables::MIN_LAG as u32;
/// Longest pitch lag in device samples (50 Hz; A-041, S-009's range kept in hertz).
pub const MAX_LAG: u32 = tables::MAX_LAG as u32;
/// Frames per second of device time: 50.
pub const FRAMES_PER_SECOND: u32 = DEVICE_RATE_HZ / FRAME_SAMPLES;
/// The library version that last changed the coder's model (spec 008 FR-010, FR-011): the
/// exported byte form carries it so an importer knows which decoder the frames need. Before
/// 1.0 a dropped model version is refused on import; from 1.0 every released one keeps its
/// decoder (A-004 adopted as a class match; A-040, A-041).
pub const MODEL_VERSION: [u16; 3] = [0, 8, 0];

/// One encoded frame: 80 bits in 10 bytes, most significant bit of byte 0 first
/// (contracts/codec-design.md › Bit order).
pub type Frame = [u8; FRAME_BYTES as usize];

/// The codec's constants as one plain struct, for hosts and for the C API
/// (`rr_dr60_speech_codec_info`; spec 008 FR-003, FR-008).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpeechCodecInfo {
    /// [`FRAME_SAMPLES`].
    pub frame_samples: u32,
    /// [`FRAME_MS`].
    pub frame_ms: u32,
    /// [`BITS_PER_FRAME`].
    pub bits_per_frame: u32,
    /// [`DELAY_SAMPLES`].
    pub delay_samples: u32,
    /// [`MODEL_VERSION`].
    pub model_version: [u16; 3],
    /// Reserved; 0.
    pub reserved: u16,
}

/// The codec's constants (spec 008 FR-003): frame length, bits per frame, delay and the model
/// version. Constant data; allocates nothing; real-time safe.
pub const fn info() -> SpeechCodecInfo {
    SpeechCodecInfo {
        frame_samples: FRAME_SAMPLES,
        frame_ms: FRAME_MS,
        bits_per_frame: BITS_PER_FRAME,
        delay_samples: DELAY_SAMPLES,
        model_version: MODEL_VERSION,
        reserved: 0,
    }
}

// Spec 008 FR-003, FR-004 and contracts/codec-design.md: the arithmetic the design rests on.
const _: () = assert!(BITS_PER_FRAME * FRAMES_PER_SECOND == 4000); // S-002: 4 kbit/s exactly
const _: () = assert!(DELAY_SAMPLES % 20 == 0); // a whole number of host samples at 44.1 kHz
const _: () = assert!(FRAME_SAMPLES % 12 == 0 && BITS_PER_FRAME == 2 * FRAME_SAMPLES / 3);
const _: () = assert!(DELAY_SAMPLES == FRAME_SAMPLES + LOOKAHEAD_SAMPLES && DELAY_SAMPLES <= 300); // 008 FR-004 (engineering target 300)
const _: () = assert!(size_of::<SpeechCodecInfo>() == 24); // 008 contracts/c-api.md (A-004)
const _: () = {
    let mut sum = 0usize;
    let mut i = 0;
    while i < tables::ALLOCATION.len() {
        sum += tables::ALLOCATION[i] as usize;
        i += 1;
    }
    assert!(sum == tables::BITS_PER_FRAME);
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec 008 FR-003, FR-004 (T010): the constants and the info struct agree with the design
    /// tool (`tools/celp-design/design_celp.py`).
    #[test]
    fn constants_match_the_design() {
        assert_eq!((FRAME_SAMPLES, FRAME_MS, SUBFRAME_SAMPLES), (120, 20, 60)); // A-040
        assert_eq!(
            (BITS_PER_FRAME, FRAME_BYTES, FRAMES_PER_SECOND),
            (80, 10, 50)
        ); // FR-003
        assert_eq!((LOOKAHEAD_SAMPLES, DELAY_SAMPLES), (60, 180)); // A-040; FR-004
        assert_eq!((LPC_ORDER, MIN_LAG, MAX_LAG), (8, 15, 120));
        let i = info();
        assert_eq!(i.frame_samples, 120);
        assert_eq!(i.delay_samples, 180);
        assert_eq!(i.model_version, [0, 8, 0]);
        assert_eq!(i.reserved, 0);
        assert_eq!(
            tables::ALLOCATION
                .iter()
                .map(|&b| u32::from(b))
                .sum::<u32>(),
            80
        );
        assert_eq!(tables::LAG_CODE_TO_HALF.len(), 127); // code 127 unused
        assert_eq!(tables::LAG_CODE_TO_HALF[0], 30); // lag 15
        assert_eq!(tables::LAG_CODE_TO_HALF[41], 71); // lag 35.5
        assert_eq!(tables::LAG_CODE_TO_HALF[42], 72); // lag 36
        assert_eq!(tables::LAG_CODE_TO_HALF[126], 240); // lag 120
        assert_eq!(tables::LPC_WINDOW.len(), 240);
        assert_eq!(tables::LPC_WINDOW[199], 32767); // the window's peak
    }
}
