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
//!   (FR-015). The crate is `no_std` and `forbid(unsafe_code)`, so clock
//!   access, OS randomness, locks and I/O are unavailable to it by
//!   construction (FR-016).
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod error;
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
pub use memory::MemoryStatus;
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
