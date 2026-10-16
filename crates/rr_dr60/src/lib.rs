//! # rr_dr60
//!
//! A portable software emulator of the audio signal chain of the Panasonic
//! RR-DR60 digital voice recorder (mid-1990s IC recorder).
//!
//! This version models the record path as the limiter (stage 3), the host-to-device rate
//! conversion, the record band-limit (stage 4), the ADC's full-scale saturation, the VAS
//! (stage 5), and the playback path as the playback band-limit (stage 10) and the
//! device-to-host conversion:
//!
//! - **Stage 3**, the record-path automatic gain control, is a **limiter**: below
//!   its knee (about the codec's full scale) it is the identity, above it the
//!   output is held at a ceiling after a dead time of a few milliseconds, and it
//!   releases over about 1.4 s. Quiet input is not raised and nothing happens in
//!   pauses. It is modeled on the circuit in the service manual's schematic
//!   (S-002; A-028, A-034 – A-036) and is on by default; see [`AgcSettings`].
//! - **Stage 4**, the record-side anti-alias filter and ADC, followed by the
//!   ADC's **full-scale clamp** (A-030; see [`Settings::adc_clamp_enabled`]).
//! - **Stage 5**, the Voice Activated System (VAS). Recording pauses while no
//!   sound is detected, so long pauses shrink to the hang time, the start of a
//!   sound after a pause is lost, and the remaining audio is joined at abrupt
//!   splices. It is modeled on the owner's manual and assumed values (A-008,
//!   A-021 – A-025) and is on by default; see [`VasSettings`]. And
//! - **Stage 10**, the playback-side DAC and reconstruction filter.
//!
//! Since spec 006 the engine also has the device's structure: the **record path** (stages 3, 4
//! with the clamp, and 5) writes into a **memory** of 60 minutes and 99 recordings (S-001;
//! [`MemorySettings`], counted in stored device time, A-038; the store is reserved when the
//! pipeline is created, about 86 MB at the defaults, virtual until written), and the
//! **playback path** (stage 10, then the output chain since spec 009) plays a recording back
//! toward a speaker or earphone [`Route`]. The engine is idle, recording or playing, never two at
//! once ([`State`], [`EngineState`]); each recording and each playback starts from the path's
//! reset state (A-037); recording stops by itself at the exact sample at which the memory
//! fills and keeps the partial recording (A-039). Record-then-play is bit-identical to the
//! one-pass processing, which stays for hosts that only want the sound and is refused while a
//! path is active. A recording leaves and enters the memory as an opaque, versioned byte
//! sequence ([`Pipeline::export`], [`Pipeline::import`]).
//!
//! Since spec 008 the **speech codec** (stages 7 and 9) sits between them, on by default
//! ([`Settings::speech_codec_enabled`]): the record path encodes the post-VAS stream into
//! 4 kbit/s frames (20 ms, 80 bits, 10 bytes each; [`speech_codec`]) and stores the frames
//! (about 1.8 MB reserved at the default capacity), the playback path decodes them, and the
//! one-pass processing runs both with an exact delay line, so it stays bit-identical to
//! record-then-play. The codec adds 180 device samples (30 ms: one frame plus the look-ahead)
//! to the record path's latency and none to the playback path's; a recording is a whole number
//! of frames, so the stop feeds silence to complete the last one and reports how much
//! ([`StopOutcome::silence_fed_samples`]). The exported form is **version 2**: it declares the
//! encoding (frames, or device-rate samples with the codec bypassed, [`Encoding`]) and the
//! codec model version that wrote the frames; a memory holds one encoding at a time, so
//! changing the codec control needs an empty memory and an import of the other encoding is
//! refused ([`Error::EncodingDiffers`]). The coder is **modeled on a 4 kbit/s CELP-class
//! coder of the period** (an algebraic CELP of the G.729 / G.723.1 family adapted to the
//! 6 kHz clock; S-008 – S-011 for the class), not on the device's own coder, whose algorithm
//! is unidentified (A-004 adopted; A-040, A-041, low confidence); its frames and output are
//! bit-identical on every target because the codec is fixed-point integer arithmetic. The
//! codec's output can be tapped before stage 10 ([`Tap::AfterDecoder`]); a bypassed engine is
//! not a device configuration.
//!
//! Since spec 009 the playback path and the one-pass processing end at the **output chain**
//! (stages 11 – 13), on by default ([`OutputSettings`]) and running at the host rate after the
//! resampler: the volume control (the **volume position**, [`Pipeline::set_volume`], the
//! control's wiper fraction with the circuit's loaded law, A-044; default
//! [`output::VOLUME_REFERENCE`], the reference position where a full-scale codec output just
//! reaches the amplifier's clip ceiling, A-043) with its 32 Hz coupling corner; on the speaker
//! route the amplifier's 159 Hz corner, its gain over the 1.15 V ceiling and a symmetric knee
//! (A-031), then **a template of a small speaker** (a 750 Hz resonance and a rising shelf, peak
//! gain 1.0; A-010, low confidence, its own bypass); on the earphone route the follower's two
//! corners near 120 and 280 Hz with no clipper (A-032). The [`Route`] selects the branch and
//! the **output scale** (A-042): a sample of 1.0 is the clip ceiling on the speaker route and
//! the volume control's full swing on the earphone route, so a full-scale recording reads
//! about 0 dBFS on the speaker route at the default position and about −18.6 dB on the
//! earphone route, and positions above the default clip full-scale material. The corners sit
//! below the codec's 225 Hz record band edge, so they matter little on a recording the device
//! made. The chain adds no reported latency (its filters are minimum-phase; the 1 kHz group
//! delay, about 0.3 ms on the speaker route, is not counted) and the playback drain grows by
//! its settle ([`output::settle_samples`]). A steady tone below the knee never exceeds full
//! scale on the speaker route, but the template's resonance rings on clipped or transient
//! material (about 1.9 at 48 kHz) and the earphone route passes the playback filter's
//! reconstruction overshoot: hosts writing integer buffers leave headroom or clamp. At the
//! three low host rates a clipped tone's harmonics alias within the host band, so the clipped
//! speaker route there is band-limited by the host rate. The default tap is the route's output
//! ([`Tap::RouteOutput`]); [`Tap::AfterPlayback`] is version 0.8's output, before the chain. A
//! bypassed output chain is a combination the device cannot produce. Every figure is modeled on
//! the circuit (S-002) and the datasheet (S-006), not measured against a unit.
//!
//! Since spec 010 the engine has an **output clamp** ([`OutputSettings::clamp_enabled`]), a
//! host option that is **off by default**: when on, every sample at the route-output tap above
//! +1.0 is replaced by +1.0 and every sample below −1.0 by −1.0 (the route's full scale,
//! A-042), on both routes, after any output-chain bypass; the play calls always return the
//! route output, so they are always bounded with the option on, while the per-stage taps and
//! every earlier tap are never bounded. It is **an emulator convenience, not a device
//! behavior**, for hosts that write integer buffers (+1.0 maps to 32767 in a 16-bit
//! conversion); on material above the bound it replaces the excess with flat tops, which adds
//! distortion of its own that the device does not have. See the README's output section for
//! the figures and the headroom alternative.
//!
//! Stages 4 and 10 are modeled on the OKI MSM7702 voice-band codec at the device's internal
//! rate of 6 kHz (A-026, verified from the service manual, S-002). They follow the codec
//! family's datasheet templates scaled to that clock (A-027, medium confidence): the record
//! stage is a band-pass (about 225 to 2550 Hz) and the playback stage a low-pass that passes
//! DC. The limiter is modeled on the circuit, not measured against a unit: its dead time,
//! release and knee carry low confidence. The speech codec is a class match, not the device's
//! coder. None of these values has yet been measured against a real unit. See
//! `docs/hardware/assumptions.md`.
//!
//! The crate describes signal processing only: gain control, band-limiting,
//! voice-activated pausing, speech coding, latency and level. It makes no claims about what
//! recorded audio contains.
//!
//! **What the engine says about itself** (spec 007): the [`manifest()`] is constant data
//! describing the fifteen signal-chain stages, one [`StageEntry`] per stage under a
//! [`Header`], with each stage's status (not modeled, assumed, sourced, checked against a real
//! unit), its source and assumption IDs, the project's confidence, what the engine leaves out,
//! which controls touch it and the version that last changed its model. An app generates its
//! "about this model" view from it; a harness test keeps it consistent with the hardware
//! registers. Reading it allocates nothing:
//!
//! ```
//! let m = rr_dr60::manifest();
//! println!("{} stages, {} checked against a real unit", m.header.stage_count, m.header.counts.checked);
//! for s in m.stages() {
//!     println!("{} {}: {:?} ({:?}) {}", s.number, s.name, s.status, s.confidence, s.description);
//! }
//! ```
//!
//! **Level scale:** 0 dBFS (a sine of peak amplitude 1.0) is the codec's full scale, about
//! 0.196 V peak at the microphone amplifier's output (A-030); speech at a conversational
//! distance is assumed roughly 10–35 dB below it (A-029, low confidence). The limiter holds
//! sustained loud input about 1 dB below full scale, so the clamp acts only on the dead time
//! of a loud onset; with the clamp bypassed the output can exceed ±1.0 there. The output is
//! always finite.
//!
//! **Output length:** with the VAS on in drop mode (the default), pauses are
//! removed, so a block can produce fewer output samples than it consumed, or
//! none. Every processing call returns a [`BlockInfo`]; only
//! `output[..produced]` is output. [`VasMode::Mute`] keeps the length instead.
//!
//! ## Guarantees
//!
//! - **Deterministic.** The same input, settings and seed give bit-identical
//!   output on every supported platform and for any block size (FR-014),
//!   including the VAS events.
//! - **Real-time safe processing.** No allocation, locks or I/O per block
//!   (FR-015), in the one-pass calls and in the record and playback calls, the
//!   start and stop calls, the route and the reads (spec 006 FR-017). `erase`
//!   and `erase_all` move stored samples (bounded by the store's size) and
//!   `export` and `import` run a checksum over a recording, so they are not.
//!   The crate is `no_std` and `forbid(unsafe_code)`, so clock
//!   access, OS randomness, locks and I/O are unavailable to it by
//!   construction (FR-016).
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

/// The speech codec's constants and info (signal-chain stages 7 and 9; spec 008).
///
/// The encoder and decoder themselves are internal: hosts reach them through the engine's
/// record, playback and one-pass calls and the `speech_codec_enabled` setting.
pub mod speech_codec {
    pub use crate::codec::{
        BITS_PER_FRAME, DELAY_SAMPLES, DEVICE_RATE_HZ, FRAME_BYTES, FRAME_MS, FRAME_SAMPLES,
        FRAMES_PER_SECOND, Frame, LOOKAHEAD_SAMPLES, LPC_ORDER, MAX_LAG, MIN_LAG, MODEL_VERSION,
        SUBFRAME_SAMPLES, SpeechCodecInfo, info,
    };
}

mod codec;
mod error;
mod form;
pub mod manifest;
mod memory;
pub mod output;
mod paths;
mod pipeline;
mod rate;
mod resample;
mod sanitize;
mod settings;
mod stages;
#[cfg(test)]
mod test_util;
mod transport;
mod validate;

pub use error::{Error, Setting};
pub use manifest::{
    Confidence, Header, Manifest, StageEntry, StagePath, StageStatus, StatusCounts, manifest,
};
pub use memory::{Encoding, MemoryStatus};
pub use pipeline::{BlockInfo, Pipeline, VasEvent};
pub use settings::{
    AgcSettings, DEVICE_RATE_HZ, MemorySettings, OutputSettings, Route, SUPPORTED_HOST_RATES,
    Settings, Tap, VasMode, VasSettings,
};
pub use transport::{
    EngineState, PlayInfo, RecordInfo, State, StopOutcome, StopReason, StopResult,
};

/// Test-only hooks for the measurement harness. Not part of the public API; enabled only by
/// the `__test-hooks` feature.
#[cfg(feature = "__test-hooks")]
#[doc(hidden)]
pub mod __test_hooks {
    /// The committed record-stage coefficients (stage 4), `[b0, b1, b2, a1, a2]` per section,
    /// so the harness can compare measured responses with the analytic one (spec 004 R-06).
    pub fn record_sos() -> [[f64; 5]; 5] {
        crate::stages::voiceband_coeffs::RECORD_SOS
    }

    /// The committed playback-stage coefficients (stage 10; spec 004 R-06).
    pub fn playback_sos() -> [[f64; 5]; 3] {
        crate::stages::voiceband_coeffs::PLAYBACK_SOS
    }

    /// The limiter's hold-capacitor voltage, in volts: 0.0 when discharged (spec 005 R-03;
    /// contracts/rust-api.md › Test hooks).
    pub fn limiter_state(pipeline: &crate::Pipeline) -> f64 {
        pipeline.limiter_state()
    }

    /// The actual `Vec` capacities of the recording store and the span table, so the SC-007
    /// footprint check reads the reservation and not the engine's arithmetic (spec 006 plan
    /// review P6; contracts/rust-api.md › Test hooks).
    pub fn memory_capacities(pipeline: &crate::Pipeline) -> (usize, usize) {
        pipeline.memory_capacities()
    }

    /// Stage 10's settling length in device samples, the constant the playback drain uses
    /// (spec 006 FR-008; research R-06; contracts/rust-api.md › Test hooks).
    pub fn n10() -> usize {
        crate::paths::N10
    }

    /// The one-pass speech codec's delay line: the decoded samples waiting to be emitted, or
    /// `None` with the codec bypassed (spec 008 research R-06; contracts/rust-api.md › Test
    /// hooks: `OnePass::__codec_fifo_fill`).
    pub fn codec_fifo_fill(pipeline: &crate::Pipeline) -> Option<usize> {
        pipeline.codec_fifo_fill()
    }

    /// The stored frame bytes of recording `number`, 10 per frame (spec 008 SC-008: the
    /// determinism and mutation tests compare and alter them before the byte form of US2
    /// exists); empty for a samples memory or a bad number.
    pub fn stored_frames(pipeline: &crate::Pipeline, number: u32) -> &[u8] {
        pipeline.stored_frames(number)
    }

    /// Flips one bit of recording `number`'s stored frames (spec 008 SC-008: a one-bit change
    /// must change the playback). Returns whether a byte was there to flip.
    pub fn flip_stored_frame_bit(
        pipeline: &mut crate::Pipeline,
        number: u32,
        byte: usize,
        bit: u8,
    ) -> bool {
        pipeline.flip_stored_frame_bit(number, byte, bit)
    }

    /// The settling length of a biquad cascade: the first index after which |h| stays below
    /// `rel` × its own peak (spec 006 research R-06). Allocates; test use only.
    pub fn stage_settling(sos: &[[f64; 5]], rel: f64) -> usize {
        crate::paths::settling_len(sos, rel)
    }

    /// A deliberate alteration of the output chain for the spec 009 SC-006 mutation tests
    /// (`Pipeline::__with_output_mutation`; tasks.md T017). `Default` is the committed chain.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct OutputMutation {
        /// The speaker template's numerator scaled by this (1.0: as committed; 1.02 lifts the
        /// peak gain above 1.0).
        pub speaker_numerator_scale: f64,
        /// The knee's start as a fraction of the ceiling (the committed 0.85; 0.5 softens it).
        pub knee_start: f64,
        /// Added to the chain's settle `N_OUT` (0: as committed).
        pub n_out_delta: i64,
        /// The speaker route's per-block scalar scaled by this (1.0: as committed; 1.1 drives
        /// full-scale material 0.8 dB into the knee at the reference position).
        pub gain_scale: f64,
        /// The output clamp's bound (spec 010 research R-06; 1.0: as committed, A-042; 0.5 or
        /// 1 + 2⁻²³ fail the identity check).
        pub clamp_bound: f64,
        /// The output clamp applied to positive samples only (false: as committed; true fails
        /// the identity check on the negative half-cycles).
        pub clamp_positive_only: bool,
        /// Version 0.10's instantaneous volume step (spec 011 research R-07; false: as
        /// committed, the 20 ms transition of A-045; true: every position change is taken at
        /// once at the next block, the reference of the route-output comparison and of the
        /// cost measurement).
        pub volume_step: bool,
        /// The volume transition's knob moved linearly in time instead of along the smoothstep
        /// (false: as committed; true is the mutant that fails spec 011 FR-004 on the speaker
        /// law, about 19 dB against the 30 dB floor).
        pub volume_shape_linear: bool,
        /// Added to the transition's length N in host samples (0: as committed; ±1 are the
        /// length mutants that fail the splice identity at sample N or N − 1).
        pub volume_ramp_delta: i64,
    }

    impl Default for OutputMutation {
        fn default() -> Self {
            Self {
                speaker_numerator_scale: 1.0,
                knee_start: crate::stages::output_coeffs::KNEE_START,
                n_out_delta: 0,
                gain_scale: 1.0,
                clamp_bound: 1.0,
                clamp_positive_only: false,
                volume_step: false,
                volume_shape_linear: false,
                volume_ramp_delta: 0,
            }
        }
    }
}

/// Crate version, e.g. `"0.1.0"`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
