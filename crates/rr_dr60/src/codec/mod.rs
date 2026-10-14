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
//!
//! The encoder (research R-04), per frame: the LPC analysis on the 240-sample window, LPC → LSP,
//! the companded LSP quantizer, interpolation to the two subframes; the perceptual weighting
//! filter `A(z/0.9)/A(z/0.5)` on the input from the **unquantized** predictor; the open-loop
//! lag on the weighted speech; per subframe the target (the weighted speech minus the
//! zero-input response of the weighted synthesis filter from the local decoder's state), the
//! closed-loop lag with its half-sample refinement, the fixed codebook, the sequential gain
//! quantization, the excitation and the local synthesis. The decoder (R-05) mirrors the
//! synthesis and adds the postfilter.

#![deny(clippy::arithmetic_side_effects)]

pub(crate) mod acelp;
pub(crate) mod bits;
pub(crate) mod fixed;
pub(crate) mod gains;
pub(crate) mod lpc;
pub(crate) mod lsp;
pub(crate) mod pitch;
pub(crate) mod postfilter;
pub(crate) mod tables;

use self::bits::FrameParams;
use self::fixed::{f32_to_q15, q15_slice_to_f32, round_shr, sat16, sub};
use self::gains::{GainPredictor, Gains, Terms};
use self::lpc::{FLAT, Lpc};
use self::lsp::Lsp;
use self::pitch::{ClosedLoop, Excitation, HALF_MAX, WSP_LEN};
use self::postfilter::Postfilter;
use self::tables::{GAMMA_W1_POW, GAMMA_W2_POW, PAST_WEIGHTED, WINDOW_SAMPLES};

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

/// Whether `frame` uses a code point the encoder never produces: subframe 1's lag code 127
/// (spec 008 FR-010; contracts/codec-design.md). The decoder reads it as the longest lag, so it
/// would decode, but import refuses such a frame as damaged (contracts/recording-form.md).
pub(crate) fn uses_unused_code(frame: &Frame) -> bool {
    bits::unpack(frame).lag1 == pitch::UNUSED_LAG_CODE
}

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

/// Samples per frame and subframe as `usize`.
const FRAME: usize = tables::FRAME_SAMPLES;
const SUBFRAME: usize = tables::SUBFRAME_SAMPLES;
/// Samples before the frame in the analysis window.
const PAST_INPUT: usize = WINDOW_SAMPLES - FRAME - tables::LOOKAHEAD_SAMPLES; // 60
/// The frame's place in the analysis window.
const FRAME_START: usize = PAST_INPUT;
const FRAME_END: usize = PAST_INPUT + FRAME;
/// A unit impulse in Q12: the impulse response comes out in the predictor's format.
const IMPULSE_Q12: i32 = fixed::Q12_ONE as i32; // the predictor's Q12 format (S-009 §3.2.2)
/// The adaptive gain's ceiling before quantization: 1.4 in Q14 (the table's top entry).
const ADAPT_GAIN_MAX_Q14: i64 = 22938; // design constant (A-041; tools/celp-design: ADAPT_GAIN[7])

/// The predictor order as `usize`.
const ORDER: usize = tables::LPC_ORDER;

/// The two memories of the weighting filter `A(z/γ₁) / A(z/γ₂)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct WeightingMemory {
    /// The moving-average part: the last eight inputs.
    num: [i32; ORDER],
    /// The all-pole part: the last eight outputs.
    den: [i32; ORDER],
}

impl WeightingMemory {
    /// `x` through the weighting filter with these memories, updated.
    fn run(&mut self, w1: &Lpc, w2: &Lpc, x: &[i32], out: &mut [i32]) {
        let mut tmp = [0i32; FRAME];
        let tmp = &mut tmp[..x.len()];
        lpc::fir(w1, x, &mut self.num, tmp);
        lpc::iir(w2, tmp, &mut self.den, out);
    }
}

/// The speech encoder: signal-chain stage 7 (spec 008 research R-04; data-model.md › Codec
/// state). Allocation-free; `Clone + Debug`; reached by hosts through the engine.
#[derive(Clone, Debug)]
pub(crate) struct SpeechEncoder {
    /// The analysis window: 60 past samples, the frame, 60 look-ahead (Q15).
    buf: [i32; WINDOW_SAMPLES],
    /// Where the next pushed sample goes.
    fill: usize,
    /// Weighted speech: 240 past samples and the current frame.
    wsp: [i32; WSP_LEN],
    /// The weighting filter on the input speech.
    w_in: WeightingMemory,
    /// The weighting filter on the local decoder's output (for the zero-input response).
    w_syn: WeightingMemory,
    /// The local synthesis filter's memory.
    syn_mem: [i32; ORDER],
    exc: Excitation,
    pred: GainPredictor,
    /// The previous frame's predictor (the Levinson fallback).
    prev_a: Lpc,
    /// The previous frame's unquantized LSPs (the root-search fallback).
    prev_lsp: Lsp,
    /// The previous frame's decoded LSPs (the quantizer's prediction and the interpolation).
    prev_q: Lsp,
}

impl Default for SpeechEncoder {
    fn default() -> Self {
        Self {
            buf: [0; WINDOW_SAMPLES],
            fill: PAST_INPUT,
            wsp: [0; WSP_LEN],
            w_in: WeightingMemory::default(),
            w_syn: WeightingMemory::default(),
            syn_mem: [0; ORDER],
            exc: Excitation::default(),
            pred: GainPredictor::default(),
            prev_a: FLAT,
            prev_lsp: lsp::mean(),
            prev_q: lsp::mean(),
        }
    }
}

impl SpeechEncoder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Back to the reset state (FR-018): every memory cleared, the LSP predictors at the mean
    /// vector, the gain predictor at its mean.
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    /// Pushes one post-VAS device sample. Returns the frame it completes: frame *k* on the
    /// push of sample 120(k+1) + 59 (zero-based), i.e. the 180th push, then every 120
    /// (FR-004, FR-007; `LOOKAHEAD_SAMPLES` is defined by this count).
    #[inline]
    pub(crate) fn push(&mut self, x: f32) -> Option<Frame> {
        self.buf[self.fill] = f32_to_q15(x);
        self.fill = self.fill.wrapping_add(1);
        if self.fill < WINDOW_SAMPLES {
            return None;
        }
        let frame = self.encode_frame();
        // The frame's last 60 samples become the past, the look-ahead the next frame's start.
        self.buf.copy_within(FRAME.., 0);
        self.fill = FRAME;
        Some(frame)
    }

    /// One frame from the full analysis window.
    fn encode_frame(&mut self) -> Frame {
        // 1. LPC analysis (R-04): autocorrelation, Levinson with its fallback.
        let r = lpc::autocorrelation(&self.buf);
        let (a, _) = lpc::levinson(&r, &self.prev_a);
        self.prev_a = a;
        // 2. LSPs, their quantization and the two subframe predictors.
        let (lsp, _) = lsp::lpc_to_lsp(&a, &self.prev_lsp);
        self.prev_lsp = lsp;
        let (lsp_idx, q) = lsp::quantize(&lsp, &self.prev_q);
        let a_sub = [
            lsp::lsp_to_lpc(&lsp::interpolate(&self.prev_q, &q)),
            lsp::lsp_to_lpc(&q),
        ];
        self.prev_q = q;
        // 3. The weighting filter from the unquantized predictor (S-008 §2.8) and the weighted
        //    speech of the frame.
        let w1 = lpc::weighted(&a, &GAMMA_W1_POW);
        let w2 = lpc::weighted(&a, &GAMMA_W2_POW);
        self.wsp.copy_within(FRAME.., 0);
        let mut sw = [0i32; FRAME];
        let (w_in, buf) = (&mut self.w_in, &self.buf);
        w_in.run(&w1, &w2, &buf[FRAME_START..FRAME_END], &mut sw);
        self.wsp[PAST_WEIGHTED..].copy_from_slice(&sw);
        // 4. The open-loop lag on the weighted speech (S-009 §3.4).
        let open_loop = pitch::open_loop_lag(&self.wsp);
        // 5. The two subframes.
        let mut params = FrameParams {
            lsp: lsp_idx,
            ..FrameParams::default()
        };
        let mut lag1_half = HALF_MAX;
        for (sf, a_q) in a_sub.iter().enumerate() {
            let start = sf.wrapping_mul(SUBFRAME);
            let mut sw_sf = [0i32; SUBFRAME];
            sw_sf.copy_from_slice(&sw[start..start.wrapping_add(SUBFRAME)]);
            let h = impulse_response(a_q, &w1, &w2);
            let zir = self.zero_input_response(a_q, &w1, &w2);
            let mut target = [0i32; SUBFRAME];
            for ((t, &s), &z) in target.iter_mut().zip(&sw_sf).zip(&zir) {
                *t = sub(s, z);
            }
            // The adaptive codebook (contracts/codec-design.md › Searches).
            let cl = if sf == 0 {
                let (lo, hi) = (pitch::HALF_MIN, HALF_MAX);
                self.lag_search(open_loop, lo, hi, true, &target, &h)
            } else {
                let (lo, hi) = pitch::delta_window(lag1_half);
                let center = (lag1_half.wrapping_add(1).wrapping_shr(1)) as usize;
                self.lag_search(center, lo, hi, false, &target, &h)
            };
            if sf == 0 {
                lag1_half = cl.half;
                params.lag1 = pitch::half_to_lag_code(cl.half);
            } else {
                params.lag_delta2 = pitch::delta_encode(lag1_half, cl.half);
            }
            // The unquantized adaptive gain, clipped to 0 … 1.4 (Q14), and the codebook target.
            let g_p = fixed::div_q(cl.corr.max(0), cl.energy, 14).clamp(0, ADAPT_GAIN_MAX_Q14);
            let mut target2 = [0i32; SUBFRAME];
            for ((t2, &t), &y) in target2.iter_mut().zip(&target).zip(&cl.y) {
                *t2 = sat16(
                    i64::from(t).saturating_sub(round_shr(g_p.saturating_mul(i64::from(y)), 14)),
                );
            }
            // The fixed codebook (S-008 §2.16).
            let mut d = [0i64; SUBFRAME];
            acelp::backward_filter(&target2, &h, &mut d);
            let mut phi = [[0i64; SUBFRAME]; SUBFRAME];
            acelp::correlation_matrix(&h, &mut phi);
            let code = acelp::search(&d, &phi);
            let mut c = [0i32; SUBFRAME];
            acelp::build_vector(code, &mut c);
            let mut z = [0i32; SUBFRAME];
            acelp::filter_pulses(&c, &h, &mut z);
            // The gains (sequential, 8 + 8 evaluations).
            let terms = Terms::new(&target, &cl.y, &z);
            let g = gains::quantize(&terms, self.pred.predicted_q8());
            // The excitation, the local synthesis and the memories.
            let u = excitation(&cl.v, &c, &g);
            self.exc.push_subframe(&u);
            let mut synth = [0i32; SUBFRAME];
            lpc::iir(a_q, &u, &mut self.syn_mem, &mut synth);
            let mut discard = [0i32; SUBFRAME];
            self.w_syn.run(&w1, &w2, &synth, &mut discard);
            self.pred.update(gains::correction_q8(g.fixed_index));
            if sf == 0 {
                params.codebook1 = code;
                params.adaptive_gain1 = g.adaptive_index;
                params.fixed_gain1 = g.fixed_index;
            } else {
                params.codebook2 = code;
                params.adaptive_gain2 = g.adaptive_index;
                params.fixed_gain2 = g.fixed_index;
            }
        }
        bits::pack(&params)
    }

    /// The zero-input response of the weighted synthesis filter from the local decoder's
    /// memories (copies: the memories update only with the real excitation).
    fn zero_input_response(&self, a_q: &Lpc, w1: &Lpc, w2: &Lpc) -> [i32; SUBFRAME] {
        let mut syn_mem = self.syn_mem;
        let mut s = [0i32; SUBFRAME];
        lpc::iir(a_q, &[0i32; SUBFRAME], &mut syn_mem, &mut s);
        let mut w = self.w_syn;
        let mut out = [0i32; SUBFRAME];
        w.run(w1, w2, &s, &mut out);
        out
    }

    /// The closed-loop lag: the integer lags within ±3 of `center` inside `[lo, hi]`, then
    /// the half-sample neighbours of the winner (`sf1`: only those subframe 1's code carries).
    fn lag_search(
        &self,
        center: usize,
        lo: u32,
        hi: u32,
        sf1: bool,
        target: &[i32; SUBFRAME],
        h: &[i32; SUBFRAME],
    ) -> ClosedLoop {
        let mut ints = [0u32; 7];
        let n = pitch::integer_candidates(center, lo, hi, &mut ints);
        let integer = pitch::closed_loop(&self.exc, target, h, &ints[..n]);
        let half_int = integer.as_ref().map_or(lo, |cl| cl.half);
        let mut fr = [0u32; 3];
        let m = pitch::fraction_candidates(half_int, lo, hi, sf1, &mut fr);
        match pitch::closed_loop(&self.exc, target, h, &fr[..m]) {
            Some(cl) => cl,
            // coverage: unreachable by construction (Constitution III): the window always
            // holds the winner itself; kept as a total function.
            None => pitch::closed_loop(&self.exc, target, h, &[lo]).unwrap_or(ClosedLoop {
                half: lo,
                v: [0; SUBFRAME],
                y: [0; SUBFRAME],
                corr: 0,
                energy: 0,
            }),
        }
    }
}

/// The impulse response of the weighted synthesis filter `W(z) / Â(z)` over one subframe, in
/// the predictor's Q12 (a unit impulse of 4096).
fn impulse_response(a_q: &Lpc, w1: &Lpc, w2: &Lpc) -> [i32; SUBFRAME] {
    let mut delta = [0i32; SUBFRAME];
    delta[0] = IMPULSE_Q12;
    let mut s = [0i32; SUBFRAME];
    lpc::iir(a_q, &delta, &mut [0i32; ORDER], &mut s);
    let mut w = WeightingMemory::default();
    let mut h = [0i32; SUBFRAME];
    w.run(w1, w2, &s, &mut h);
    h
}

/// The excitation `u = ĝ_a v + ĝ_c c` (Q15, saturated): the adaptive vector scaled by the Q14
/// gain, the codebook pulses by the amplitude.
fn excitation(v: &[i32; SUBFRAME], c: &[i32; SUBFRAME], g: &Gains) -> [i32; SUBFRAME] {
    let mut u = [0i32; SUBFRAME];
    for ((un, &vn), &cn) in u.iter_mut().zip(v).zip(c) {
        let adaptive = round_shr(i64::from(g.g_a_q14).saturating_mul(i64::from(vn)), 14);
        let fixed = i64::from(g.g_c).saturating_mul(i64::from(cn));
        *un = sat16(adaptive.saturating_add(fixed));
    }
    u
}

/// The speech decoder: signal-chain stage 9 (spec 008 research R-05). Allocation-free;
/// adds no delay: a frame's 120 samples are produced when it is decoded.
#[derive(Clone, Debug)]
pub(crate) struct SpeechDecoder {
    prev_q: Lsp,
    syn_mem: [i32; ORDER],
    exc: Excitation,
    pred: GainPredictor,
    post: Postfilter,
}

impl Default for SpeechDecoder {
    fn default() -> Self {
        Self {
            prev_q: lsp::mean(),
            syn_mem: [0; ORDER],
            exc: Excitation::default(),
            pred: GainPredictor::default(),
            post: Postfilter::default(),
        }
    }
}

impl SpeechDecoder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Back to the reset state (FR-018).
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    /// Decodes one frame into 120 device samples. Every bit pattern decodes to finite output
    /// within ±1.0 (FR-010): the LSPs are re-ordered and spaced, lag code 127 (never produced)
    /// reads as the longest lag, the delta is clamped, every codebook code is legal, the gains
    /// are table lookups and the arithmetic saturates.
    pub(crate) fn decode(&mut self, frame: &Frame, out: &mut [f32; FRAME]) {
        let p = bits::unpack(frame);
        let q = lsp::decode(&p.lsp, &self.prev_q);
        let a_sub = [
            lsp::lsp_to_lpc(&lsp::interpolate(&self.prev_q, &q)),
            lsp::lsp_to_lpc(&q),
        ];
        self.prev_q = q;
        let lag1_half = pitch::lag_code_to_half(p.lag1).unwrap_or(HALF_MAX);
        let lags = [lag1_half, pitch::delta_decode(lag1_half, p.lag_delta2)];
        let codes = [p.codebook1, p.codebook2];
        let adaptive = [p.adaptive_gain1, p.adaptive_gain2];
        let fixed = [p.fixed_gain1, p.fixed_gain2];
        let mut pcm = [0i32; FRAME];
        for sf in 0..2 {
            let a_q = &a_sub[sf];
            let mut v = [0i32; SUBFRAME];
            self.exc.adaptive_vector(lags[sf], &mut v);
            let mut c = [0i32; SUBFRAME];
            acelp::build_vector(codes[sf], &mut c);
            let g = gains::gains_for(self.pred.predicted_q8(), adaptive[sf], fixed[sf]);
            let u = excitation(&v, &c, &g);
            self.exc.push_subframe(&u);
            let mut synth = [0i32; SUBFRAME];
            lpc::iir(a_q, &u, &mut self.syn_mem, &mut synth);
            let start = sf.wrapping_mul(SUBFRAME);
            let mut post = [0i32; SUBFRAME];
            self.post.process(a_q, &synth, &mut post);
            pcm[start..start.wrapping_add(SUBFRAME)].copy_from_slice(&post);
            self.pred.update(gains::correction_q8(g.fixed_index));
        }
        q15_slice_to_f32(&pcm, out);
    }
}

/// 008 T010 and T014 (spec 008 FR-003, FR-004, FR-006, FR-007, FR-010, FR-018; research R-04,
/// R-05): the constants, the push count, frame causality, silence, every bit pattern, reset,
/// the decoder's zero delay and a voiced round trip.
#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, clippy::disallowed_methods)] // test arithmetic
mod tests {
    extern crate std;
    use super::*;
    use std::vec::Vec;

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

    /// A seeded PCG32 for the random-frame test (the harness's generator, written here so the
    /// core's tests stay self-contained).
    struct Pcg32 {
        state: u64,
        inc: u64,
    }

    impl Pcg32 {
        fn new(seed: u64, stream: u64) -> Self {
            let mut p = Self {
                state: 0,
                inc: (stream << 1) | 1,
            };
            p.next_u32();
            p.state = p.state.wrapping_add(seed);
            p.next_u32();
            p
        }

        fn next_u32(&mut self) -> u32 {
            let old = self.state;
            self.state = old
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(self.inc);
            let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
            let rot = (old >> 59) as u32;
            xorshifted.rotate_right(rot)
        }
    }

    /// A synthetic voiced signal at the device rate: a pulse train whose period glides
    /// 100 → 150 Hz through two resonators (500 and 1500 Hz), scaled to `level_dbfs` RMS
    /// (AES17: a full-scale sine is 0 dBFS).
    fn voiced(samples: usize, level_dbfs: f64) -> Vec<f32> {
        let fs = 6000.0;
        let mut phase = 0.0f64;
        let mut res = [(0.0f64, 0.0f64); 2];
        let mut out = Vec::with_capacity(samples);
        for n in 0..samples {
            let u = n as f64 / samples as f64;
            let f0 = 100.0 + 50.0 * u;
            phase += f0 / fs;
            let excitation = if phase >= 1.0 {
                phase -= 1.0;
                1.0
            } else {
                0.0
            };
            let mut y = 0.0;
            for (r, (fc, bw)) in res.iter_mut().zip([(500.0, 100.0), (1500.0, 150.0)]) {
                let rad = (-std::f64::consts::PI * bw / fs).exp();
                let c1 = 2.0 * rad * (2.0 * std::f64::consts::PI * fc / fs).cos();
                let c2 = -rad * rad;
                let v = excitation + c1 * r.0 + c2 * r.1;
                r.1 = r.0;
                r.0 = v;
                y += v;
            }
            out.push(y);
        }
        let rms = (out.iter().map(|v| v * v).sum::<f64>() / samples as f64).sqrt();
        let want = 10f64.powf(level_dbfs / 20.0) / 2f64.sqrt();
        out.iter().map(|&v| (v / rms * want) as f32).collect()
    }

    fn rms_dbfs(x: &[f32]) -> f64 {
        let ms = x.iter().map(|&v| f64::from(v) * f64::from(v)).sum::<f64>() / x.len() as f64;
        10.0 * (ms * 2.0).max(1e-30).log10()
    }

    /// Encodes `x`, returning the frames and the push index at which each was emitted.
    fn encode_all(enc: &mut SpeechEncoder, x: &[f32]) -> (Vec<Frame>, Vec<usize>) {
        let (mut frames, mut at) = (Vec::new(), Vec::new());
        for (i, &v) in x.iter().enumerate() {
            if let Some(f) = enc.push(v) {
                frames.push(f);
                at.push(i);
            }
        }
        (frames, at)
    }

    fn decode_all(dec: &mut SpeechDecoder, frames: &[Frame]) -> Vec<f32> {
        let mut out = Vec::with_capacity(frames.len() * FRAME);
        for f in frames {
            let mut buf = [0f32; FRAME];
            dec.decode(f, &mut buf);
            out.extend_from_slice(&buf);
        }
        out
    }

    /// FR-004, FR-007 (R-04): the first frame on push 179 (zero-based), then every 120 pushes;
    /// this count defines `LOOKAHEAD_SAMPLES`.
    #[test]
    fn frame_emitted_on_push_179_then_every_120() {
        let x = voiced(1000, -20.0);
        let (frames, at) = encode_all(&mut SpeechEncoder::new(), &x);
        assert_eq!(at, [179, 299, 419, 539, 659, 779, 899]);
        assert_eq!(frames.len(), 7);
        assert_eq!(179, (FRAME + tables::LOOKAHEAD_SAMPLES) - 1);
        assert_eq!(at[1] - at[0], FRAME);
    }

    /// FR-007: two streams identical through sample 120(k+1) + 59 give identical frames 0 … k,
    /// whatever follows.
    #[test]
    fn frame_depends_on_nothing_after_its_lookahead() {
        let a = voiced(1200, -20.0);
        for k in [0usize, 3, 7] {
            let cut = 120 * (k + 1) + 60;
            let mut b = a.clone();
            for (i, v) in b.iter_mut().enumerate().skip(cut) {
                *v = if i % 2 == 0 { 0.9 } else { -0.9 };
            }
            let (fa, _) = encode_all(&mut SpeechEncoder::new(), &a);
            let (fb, _) = encode_all(&mut SpeechEncoder::new(), &b);
            assert_eq!(fa[..=k], fb[..=k], "k = {k}");
            assert_ne!(
                fa[k + 1..],
                fb[k + 1..],
                "k = {k}: the change must show later"
            );
        }
    }

    /// FR-006: ten frames of zeros encode and decode without a panic to a level below
    /// −60 dBFS after the first frame; the figure is printed.
    #[test]
    fn all_zero_input_is_defined_and_quiet() {
        let x = std::vec![0.0f32; 180 + 120 * 9];
        let (frames, _) = encode_all(&mut SpeechEncoder::new(), &x);
        assert_eq!(frames.len(), 10);
        let y = decode_all(&mut SpeechDecoder::new(), &frames);
        assert!(y.iter().all(|v| v.is_finite() && v.abs() <= 1.0));
        let after_first = rms_dbfs(&y[FRAME..]);
        std::println!("silence floor: {after_first:.1} dBFS after frame 1 (FR-006 bound −60)");
        assert!(after_first < -60.0, "{after_first} dBFS");
    }

    /// FR-010: 10 000 seeded random frames, the all-zero and the all-one frame, through a
    /// fresh decoder and one with state: finite output within ±1.0 every time.
    #[test]
    fn every_bit_pattern_decodes() {
        let mut rng = Pcg32::new(0x0D60, 8);
        let mut fresh = SpeechDecoder::new();
        let mut stateful = SpeechDecoder::new();
        let warm = voiced(600, -20.0);
        let (warm_frames, _) = encode_all(&mut SpeechEncoder::new(), &warm);
        let _ = decode_all(&mut stateful, &warm_frames);
        let mut frames: Vec<Frame> = std::vec![[0u8; 10], [0xffu8; 10]];
        for _ in 0..10_000 {
            let mut f = [0u8; 10];
            for b in &mut f {
                *b = (rng.next_u32() >> 24) as u8;
            }
            frames.push(f);
        }
        let mut out = [0f32; FRAME];
        for (i, f) in frames.iter().enumerate() {
            let mut d = SpeechDecoder::new();
            d.decode(f, &mut out);
            assert!(
                out.iter().all(|v| v.is_finite() && v.abs() <= 1.0),
                "frame {i} fresh"
            );
            fresh.decode(f, &mut out);
            assert!(
                out.iter().all(|v| v.is_finite() && v.abs() <= 1.0),
                "frame {i}"
            );
            stateful.decode(f, &mut out);
            assert!(
                out.iter().all(|v| v.is_finite() && v.abs() <= 1.0),
                "frame {i} warm"
            );
        }
    }

    /// FR-018: after a voiced burst and a reset the encoder's silence frames equal a fresh
    /// encoder's; the decoder likewise.
    #[test]
    fn reset_clears_state() {
        let burst = voiced(1800, -12.0);
        let silence = std::vec![0.0f32; 1800];
        let mut enc = SpeechEncoder::new();
        let _ = encode_all(&mut enc, &burst);
        enc.reset();
        let (after_reset, _) = encode_all(&mut enc, &silence);
        let (fresh, _) = encode_all(&mut SpeechEncoder::new(), &silence);
        assert_eq!(after_reset, fresh);
        let (burst_frames, _) = encode_all(&mut SpeechEncoder::new(), &burst);
        let mut dec = SpeechDecoder::new();
        let _ = decode_all(&mut dec, &burst_frames);
        dec.reset();
        assert_eq!(
            decode_all(&mut dec, &fresh),
            decode_all(&mut SpeechDecoder::new(), &fresh)
        );
        // Without the reset the states differ.
        let mut enc2 = SpeechEncoder::new();
        let _ = encode_all(&mut enc2, &burst);
        let (no_reset, _) = encode_all(&mut enc2, &silence);
        assert_ne!(no_reset, fresh);
    }

    /// FR-004 (R-05): the decoder adds no delay. A burst that starts at sample 0 shows in the
    /// first subframe of the first decoded frame (its energy is well above the silence floor),
    /// and a frame's 120 samples all come from one `decode` call.
    #[test]
    fn decoder_adds_no_delay() {
        let mut x = voiced(600, -12.0);
        x.truncate(180);
        let (frames, _) = encode_all(&mut SpeechEncoder::new(), &x);
        assert_eq!(frames.len(), 1);
        let mut out = [0f32; FRAME];
        SpeechDecoder::new().decode(&frames[0], &mut out);
        let first = rms_dbfs(&out[..SUBFRAME]);
        assert!(first > -40.0, "first subframe {first} dBFS");
        assert!(out.iter().any(|&v| v != 0.0));
    }

    /// A-029 (contracts/codec-design.md › Fixed gain, on the encoder itself): two seconds of
    /// −10 dBFS seeded white noise encode with every fixed-gain correction index strictly
    /// inside 0 … 7 once the predictor has converged (after the tenth frame): the top of the
    /// gain range is not reached.
    #[test]
    fn minus_10_dbfs_noise_is_not_gain_limited_in_the_encoder() {
        let mut rng = Pcg32::new(0x0D60, 11);
        let amp = 10f64.powf(-10.0 / 20.0) / 2f64.sqrt() * 3f64.sqrt(); // uniform noise at −10 dBFS RMS
        let x: Vec<f32> = (0..12_180)
            .map(|_| ((f64::from(rng.next_u32() >> 8) / 16_777_216.0 - 0.5) * 2.0 * amp) as f32)
            .collect();
        let (frames, _) = encode_all(&mut SpeechEncoder::new(), &x);
        assert_eq!(frames.len(), 101);
        let mut top = 0;
        for (k, f) in frames.iter().enumerate().skip(10) {
            let p = bits::unpack(f);
            for idx in [p.fixed_gain1, p.fixed_gain2] {
                assert!((1..=6).contains(&idx), "frame {k}: correction index {idx}");
                top = top.max(idx);
            }
        }
        std::println!("−10 dBFS noise: highest fixed-gain correction index {top} of 7");
    }

    /// FR-005 (a core-side smoke figure; the harness measures properly): a 2 s voiced burst at
    /// −20 dBFS keeps its RMS within 1.5 dB through encoder and decoder, measured after the
    /// 180-sample delay.
    #[test]
    fn encode_decode_voiced_roundtrip_level() {
        let x = voiced(12_000 + 180, -20.0);
        let (frames, _) = encode_all(&mut SpeechEncoder::new(), &x);
        assert_eq!(frames.len(), 101);
        let y = decode_all(&mut SpeechDecoder::new(), &frames);
        // Decoded sample n corresponds to input sample n (the frames start at input 0).
        let n = 12_000;
        for f in (0..n / FRAME).step_by(10) {
            std::println!(
                "frame {f:>3}: in {:>7.2} dBFS out {:>7.2} dBFS",
                rms_dbfs(&x[f * FRAME..(f + 1) * FRAME]),
                rms_dbfs(&y[f * FRAME..(f + 1) * FRAME])
            );
        }
        let (lx, ly) = (rms_dbfs(&x[600..n]), rms_dbfs(&y[600..n]));
        std::println!("voiced round trip: in {lx:.2} dBFS, out {ly:.2} dBFS");
        assert!((lx - ly).abs() <= 1.5, "in {lx} dBFS, out {ly} dBFS");
        // Rough waveform agreement: the decoded signal correlates with the input.
        let dot: f64 = x[600..n]
            .iter()
            .zip(&y[600..n])
            .map(|(&a, &b)| f64::from(a) * f64::from(b))
            .sum();
        let ex: f64 = x[600..n].iter().map(|&a| f64::from(a) * f64::from(a)).sum();
        let ey: f64 = y[600..n].iter().map(|&b| f64::from(b) * f64::from(b)).sum();
        let corr = dot / (ex * ey).sqrt();
        std::println!("voiced round trip: waveform correlation {corr:.3}");
        assert!(corr > 0.5, "correlation {corr}");
    }
}
