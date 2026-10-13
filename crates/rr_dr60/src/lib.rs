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
//! **playback path** (stage 10) plays a recording back toward a speaker or earphone
//! [`Route`] (a label in this version). The engine is idle, recording or playing, never two at
//! once ([`State`], [`EngineState`]); each recording and each playback starts from the path's
//! reset state (A-037); recording stops by itself at the exact sample at which the memory
//! fills and keeps the partial recording (A-039). Record-then-play is bit-identical to the
//! one-pass processing, which stays for hosts that only want the sound and is refused while a
//! path is active. A recording leaves and enters the memory as an opaque, versioned byte
//! sequence ([`Pipeline::export`], [`Pipeline::import`]): version 1 is the record path's
//! 32-bit device-rate output, to become encoded frames when the codec is modeled (A-004).
//!
//! Stages 4 and 10 are modeled on the OKI MSM7702 voice-band codec at the device's internal
//! rate of 6 kHz (A-026, verified from the service manual, S-002). They follow the codec
//! family's datasheet templates scaled to that clock (A-027, medium confidence): the record
//! stage is a band-pass (about 225 to 2550 Hz) and the playback stage a low-pass that passes
//! DC. The limiter is modeled on the circuit, not measured against a unit: its dead time,
//! release and knee carry low confidence. None of these values has yet been measured against a
//! real unit. See `docs/hardware/assumptions.md`.
//!
//! The crate describes signal processing only: gain control, band-limiting,
//! voice-activated pausing, latency and level. It makes no claims about what recorded audio contains.
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
    AgcSettings, DEVICE_RATE_HZ, MemorySettings, Route, SUPPORTED_HOST_RATES, Settings, Tap,
    VasMode, VasSettings,
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

    /// The settling length of a biquad cascade: the first index after which |h| stays below
    /// `rel` × its own peak (spec 006 research R-06). Allocates; test use only.
    pub fn stage_settling(sos: &[[f64; 5]], rel: f64) -> usize {
        crate::paths::settling_len(sos, rel)
    }
}

/// Crate version, e.g. `"0.1.0"`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
