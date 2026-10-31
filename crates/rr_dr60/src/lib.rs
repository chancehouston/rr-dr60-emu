//! # rr_dr60
//!
//! A portable software emulator of the audio signal chain of the Panasonic
//! RR-DR60 digital voice recorder (a late-1990s IC recorder; its earliest dated document is
//! from 1998).
//!
//! This version models the record path as the microphone path (stages 1 – 2), the limiter
//! (stage 3), the host-to-device rate conversion, the record band-limit (stage 4), the ADC's
//! full-scale saturation, the VAS (stage 5), and the playback path as the playback band-limit
//! (stage 10) and the device-to-host conversion (the later stages are described below):
//!
//! - **Stages 1 – 2**, the microphone capsule and amplifier, since spec 012: the capsule's hiss
//!   and the amplifier's band and clipping ahead of the limiter, on by default; see
//!   [`MicSettings`] and the paragraph below.
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
//! - **Stage 10**, the playback-side DAC and reconstruction filter. Ahead of it, since spec 013,
//!   the player's **playback speed control** (part of stage 9's playback firmware): five levels
//!   at 3/5 … 7/5 of normal speed with the pitch moving with the speed, a live control
//!   ([`Pipeline::set_playback_speed`]; A-054, modeled on the documents, not measured), level 3
//!   the unchanged output; see [`speed`].
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
//! the circuit (S-002) and the datasheet (S-006), not measured against a unit. Since spec 011 a
//! volume change is a **20 ms transition** of the knob ([`output::VOLUME_TRANSITION_MS`];
//! A-045): the wiper's continuity is the device's, the 20 ms and the shape are an engine
//! convention; the position is taken at once after construction, `reset`, `reconfigure`, a
//! playback start and a route change.
//!
//! Since spec 012 the record path starts at the microphone: **stages 1 – 2** run at the host
//! rate ahead of the limiter, on by default ([`MicSettings`]). Stage 1 is the capsule's
//! **hiss**, a deterministic Gaussian noise at about −62 dBFS in the 300 – 3000 Hz band after
//! stage 2 (A-048), with its own stream for each recording from [`Settings::seed`] and a
//! recording counter that never rewinds (A-049). Stage 2 is the microphone amplifier: an input
//! high-pass near 818 Hz (A-046), an asymmetric soft clipper that limits negative samples
//! first and softly, toward +9 dBFS, and positive samples near +17 dBFS (A-051), and an output
//! coupling near 77 Hz and a
//! roll-off near 12.9 kHz (A-047, lumped ahead of the limiter, A-053), normalized to unity gain
//! at 1 kHz, so 0 dBFS at 1 kHz still reaches the codec's full scale (about 90 dB SPL at the
//! microphone, A-029). The default output therefore changes from 0.11: the low end is thinner,
//! silence carries the hiss, and a DC offset in the input no longer reaches the limiter. The
//! taps [`Tap::AfterMicCapsule`] and [`Tap::AfterMicAmp`] read the stages alone, with latency 0;
//! [`MicSettings::BYPASSED`] gives the 0.11 engine. The figures are modeled on the circuit
//! (S-002) with typical part values, not measured against a unit.
//!
//! Since spec 010 the engine has an **output clamp** ([`OutputSettings::clamp_enabled`]), a
//! host option that is **off by default**: when on, every sample at the route-output tap above
//! +1.0 is replaced by +1.0 and every sample below −1.0 by −1.0 (the route's full scale,
//! A-042), on both routes, after any output-chain bypass; the play calls always return the
//! route output, so they are always bounded with the option on, while the per-stage taps and
//! every earlier tap are never bounded. It is **an emulator convenience, not a device
//! behavior**, for hosts that write integer buffers (+1.0 maps to 32767 in a 16-bit
//! conversion); on material above the bound it replaces the excess with flat tops, which adds
//! distortion of its own that the device does not have. See the user guide's output section for
//! the figures and the headroom alternative.
//!
//! Stages 4 and 10 are modeled on the OKI MSM7702 voice-band codec at the device's internal
//! rate of 6 kHz (A-026, verified from the service manual, S-002). They follow the codec
//! family's datasheet templates scaled to that clock (A-027, medium confidence): the record
//! stage is a band-pass (about 225 to 2550 Hz) and the playback stage a low-pass that passes
//! DC. The microphone path and the limiter are modeled on the circuit, not measured against a
//! unit: the input corner, the hiss level, the clip limits and the limiter's dead time,
//! release and knee carry low or medium confidence. The speech codec is a class match, not the
//! device's coder. None of these values has yet been measured against a real unit. See
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
//! **The controls the engine accepts** (spec 014): [`controls()`] describes every adjustable
//! setting and live control, one [`ControlEntry`] each, with its kind, timing, paths, stage,
//! range, default, unit, IDs and one sentence on what it does to the sound; [`control()`] looks
//! one up by identifier. Tests bind it to the engine's validation and defaults. Reading it
//! allocates nothing:
//!
//! ```
//! let speed = rr_dr60::control("playback_speed").unwrap();
//! println!("{}: {}", speed.name, speed.description);
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

pub mod battery;
pub mod changes;
mod codec;
pub mod controls;
pub mod digest;
mod error;
mod form;
pub mod input;
pub mod manifest;
mod memory;
pub mod mic;
pub mod output;
mod paths;
mod pipeline;
mod rate;
pub mod record;
mod resample;
mod sanitize;
mod settings;
pub mod speed;
mod stages;
#[cfg(test)]
mod test_util;
pub mod timeline;
mod transport;
mod validate;

pub use changes::{ChangeEntry, Clock, STREAM_ONE_PASS, STREAM_PLAYBACK, STREAM_RECORD, Stream};
pub use controls::{
    ControlEntry, ControlKind, ControlOption, ControlPaths, ControlTiming, ControlValue, control,
    controls,
};
pub use error::{Error, Setting};
pub use manifest::{
    Confidence, Header, Manifest, StageEntry, StagePath, StageStatus, StatusCounts, manifest,
};
pub use memory::{Encoding, MemoryStatus};
pub use pipeline::{BlockInfo, Meters, Pipeline, VasEvent};
pub use settings::{
    AgcSettings, DEVICE_RATE_HZ, MemorySettings, MicSettings, OutputSettings, Route,
    SUPPORTED_HOST_RATES, Settings, Tap, VasMode, VasSettings,
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

    /// Deliberate defects of the meter read-out for the spec 016 mutation tests (research
    /// R-12); `Default` is the committed behavior. Each is caught by a named oracle: the
    /// clamp-on cells of AS3 (a peak read after the clamp), the 0.01 dB onset rows of AS2 (the
    /// gain from the state after the update), the reference gate's countdown of AS5 (off by
    /// one), the second-block rows of any stimulus (a missing clear).
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct MeterMutation {
        /// The record-filter peak read after the converter clamp.
        pub filter_peak_after_clamp: bool,
        /// The gain reduction from the hold after the call's last update, not the deepest
        /// applied gain.
        pub gain_from_state_after_update: bool,
        /// The hang remaining one device sample short.
        pub hang_off_by_one: bool,
        /// The input peak's accumulator not cleared at finalization.
        pub skip_clear: bool,
    }

    pub use crate::changes::{ChangeMutation, RecordMutation};

    /// Test-only (spec 015 research R-13): the published `speed_landing` or, with the
    /// `speed_landing_ceiling` mutant, the defective ⌊P · 6000 ÷ rate⌋.
    pub fn speed_landing_mutated(position: u64, host_rate_hz: u32, m: &ChangeMutation) -> u64 {
        if m.speed_landing_ceiling {
            return position * 6000 / u64::from(host_rate_hz);
        }
        crate::changes::speed_landing(position, host_rate_hz).expect("a supported rate")
    }

    /// Test-only (spec 017 research R-14): the published `record_landing` or, with the
    /// `landing_floor` mutant, the defective ⌊P · 6000 ÷ rate⌋.
    pub fn record_landing_mutated(position: u64, host_rate_hz: u32, m: &RecordMutation) -> u64 {
        if m.landing_floor {
            return position * 6000 / u64::from(host_rate_hz);
        }
        crate::changes::record_landing(position, host_rate_hz).expect("a supported rate")
    }

    /// Spec 018 (research R-07): the battery's ceiling factor in force in the one-pass's chain
    /// (`Stream::OnePass`) or the player's (`Stream::Playback`); `None` for the record stream.
    pub fn battery_factor(pipeline: &crate::Pipeline, stream: crate::Stream) -> Option<f64> {
        match stream {
            crate::Stream::OnePass => Some(pipeline.one_pass.play.battery_factor()),
            crate::Stream::Playback => Some(pipeline.player.battery_factor()),
            crate::Stream::Record => None,
        }
    }

    /// The limiter's hold-capacitor voltage, in volts: 0.0 when discharged (spec 005 R-03;
    /// contracts/rust-api.md › Test hooks).
    pub fn limiter_state(pipeline: &crate::Pipeline) -> f64 {
        pipeline.limiter_state()
    }

    /// Stage 4's output before the converter clamp for host sample `x` through the one-pass's
    /// record path (the front, the conversion, the band-limit), when a device sample is due;
    /// nothing after it runs (spec 016 research R-06).
    pub fn stage4_trace(pipeline: &mut crate::Pipeline, x: f32) -> Option<f64> {
        pipeline.one_pass.rec.stage4_trace(x)
    }

    /// The limiter's capacitor-voltage limit in volts, the charge meter's denominator (spec
    /// 016 research R-07).
    pub fn limiter_v_limit(pipeline: &crate::Pipeline) -> f64 {
        pipeline.one_pass.rec.limiter_v_limit()
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

    /// The stored device samples of recording `number` (spec 017 research R-10: the
    /// pre-landing identity in the kept-sample index); empty for a frames memory.
    pub fn stored_samples(pipeline: &crate::Pipeline, number: u32) -> &[f32] {
        pipeline.stored_samples(number)
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
        /// Spec 018 (research R-07): the battery factor's transition moved linearly in time
        /// instead of along the smoothstep (false: as committed).
        pub battery_shape_linear: bool,
        /// Spec 018: added to the battery transition's length N (0: as committed).
        pub battery_ramp_delta: i64,
        /// Spec 018: the knee's output scaled by the factor with its thresholds fixed, the
        /// rejected alternative of D3 (false: as committed; true breaks the below-knee
        /// identity). Scaling the input and the output both is bit-identical to the committed
        /// form after the f32 narrowing, so that is no mutant (restated at implementation).
        pub battery_scale_sample: bool,
        /// Spec 018: a route change does not take the battery factor at once (false: as
        /// committed; true leaves a transition running across the switch).
        pub battery_no_at_once_on_route: bool,
        /// Spec 018: the amplifier-clip count counts samples at or above the knee's start
        /// instead of the samples the knee changed (false: as committed).
        pub battery_count_at_or_above: bool,
    }

    /// A deliberate alteration of the microphone path (stages 1 – 2) for the spec 012 mutation
    /// tests and the clipper-off twin (`Pipeline::__with_mic_mutation`; tasks.md T033).
    /// `Default` is the committed path.
    #[derive(Clone, Copy, Debug, PartialEq, Default)]
    pub struct MicMutation {
        /// The input high-pass skipped (an identity in its place, without the 1 kHz
        /// normalization).
        pub no_input_hp: bool,
        /// A symmetric clipper: L₊ = L₋ (both polarities limited at the negative limit).
        pub symmetric_clip: bool,
        /// The limits exchanged between the polarities.
        pub swapped_polarity: bool,
        /// The recording counter ignored: every stream keyed to counter 0.
        pub shared_stream: bool,
        /// The hiss level moved by this many dB (0: as committed).
        pub hiss_db_delta: f32,
        /// The clipper removed: the linear twin of US2 AS4's oracle.
        pub clipper_off: bool,
        /// The ziggurat's cap path taken on every draw (A-049).
        pub force_retry_cap: bool,
        /// Added to both clipper knees k₊ and k₋ (0: as committed; ±0.05 is the knee mutant,
        /// hardware review B4).
        pub knee_delta: f32,
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
                battery_shape_linear: false,
                battery_ramp_delta: 0,
                battery_scale_sample: false,
                battery_no_at_once_on_route: false,
                battery_count_at_or_above: false,
            }
        }
    }

    /// A deliberate alteration of the playback speed control for the spec 013 mutation tests
    /// (`Pipeline::__with_speed_mutation`; research R-12). `Default` is the committed stage.
    #[derive(Clone, Copy, Debug, PartialEq, Default)]
    pub struct SpeedMutation {
        /// Level 5's place step in fifths replaced by this (`None`: as committed, 7).
        pub ratio_k7: Option<u32>,
        /// The tail dropped: silence right after the recording's own part.
        pub no_tail: bool,
        /// A change into level 3 snaps the place forward instead of waiting for a whole place.
        pub snap_into_level_3: bool,
        /// Added to the look-ahead D (0: as committed, 28; −1 moves the response off the place).
        pub lookahead_delta: i32,
        /// The latch tested after a pending change instead of before it.
        pub latch_after_switch: bool,
        /// Level 3 reads 28 stored samples ahead like the other levels (the play reports' frame
        /// counts then differ from 0.12's; spec 013 FR-006).
        pub lookahead_at_level_3: bool,
        /// Spec 019 (research R-05): no crossfade is created and a request applies at once:
        /// version 0.18's switch, bit for bit (the click reader's switch reference and the
        /// re-bless gate; false: as committed).
        pub fade_step: bool,
        /// Spec 019: a linear weight (u) in place of the smoothstep (false: as committed).
        pub fade_linear: bool,
        /// Spec 019: added to the crossfade's length N (0: as committed, 120).
        pub fade_len_delta: i32,
        /// Spec 019: a request during a crossfade applies at once, the running fade's new branch
        /// becoming the old branch (D4's rejected alternative; false: as committed, the wait).
        pub fade_no_wait: bool,
    }

    /// Spec 019 (research R-02): the crossfade's weight table as the stage holds it at `level`
    /// (120 entries from the pinned expression).
    pub fn crossfade_weights(level: u8) -> alloc::vec::Vec<f64> {
        crate::stages::speed::SpeedStage::new(level)
            .weights()
            .to_vec()
    }

    /// Spec 019 (research R-06): the speed control's device outputs of the last `play` call
    /// (own, tail and the drain's zeros), for the blend identity at the host rates whose output
    /// passes stage 10 and the resampler.
    pub fn speed_device_outputs(pipeline: &crate::Pipeline) -> alloc::vec::Vec<f64> {
        pipeline.speed_trace.clone()
    }

    /// Spec 020 (research R-04): the speed control's per-output place state of the last `play`
    /// call (the own part, the tail and the drain's silence, where the state no longer moves),
    /// the first 2048 outputs: map (c)'s pin.
    pub fn speed_place_trace(
        pipeline: &crate::Pipeline,
    ) -> alloc::vec::Vec<crate::timeline::PlaceState> {
        pipeline.speed_place_trace.clone()
    }

    /// The speed control's bank for `level` as designed at construction: 5 phases × 57 taps,
    /// phase-major (spec 013 research R-03, R-08); `None` for level 3 (the identity) or an
    /// invalid level.
    pub fn speed_bank(level: u8) -> Option<alloc::vec::Vec<f64>> {
        if !(1..=5).contains(&level) || level == 3 {
            return None;
        }
        Some(crate::stages::speed::design_bank(crate::stages::speed::numerator(level)).into_vec())
    }

    /// T(K, k): the speed control's tail after a constant-level recording of `stored` device
    /// samples at numerator `k` (spec 013 research R-04).
    pub fn speed_tail_len(k: u32, stored: u64) -> u64 {
        crate::stages::speed::tail_len(stored, k)
    }

    /// The speed control's place of the next output, in fifths of a stored sample.
    pub fn speed_place_fifths(pipeline: &crate::Pipeline) -> u64 {
        pipeline.speed.place_fifths()
    }
}

/// Crate version, e.g. `"0.1.0"`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
