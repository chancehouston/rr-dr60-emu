//! # rr_dr60
//!
//! A portable software emulator of the audio signal chain of the Panasonic
//! RR-DR60 digital voice recorder (mid-1990s IC recorder).
//!
//! This version models two stages of that chain:
//!
//! - **Stage 4**, the record-side anti-alias filter and ADC, and
//! - **Stage 10**, the playback-side DAC and reconstruction filter.
//!
//! Both are modeled on the OKI MSM7702 voice-band codec. They are *assumed* to
//! follow a telephone-style 300–3400 Hz band-pass (A-002, A-014) at the
//! device's internal rate of 8 kHz (A-001). These values have not yet been
//! measured against a real unit. See `docs/hardware/assumptions.md`.
//!
//! The crate describes signal processing only: band-limiting, latency and
//! level. It makes no claims about what recorded audio contains.
//!
//! ## Guarantees
//!
//! - **Deterministic.** The same input, settings and seed give bit-identical
//!   output on every supported platform and for any block size (FR-014).
//! - **Real-time safe processing.** No allocation, locks or I/O per block
//!   (FR-015). The crate is `no_std` and `forbid(unsafe_code)`, so clock
//!   access, OS randomness, locks and I/O are unavailable to it by
//!   construction (FR-016).
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod error;
mod pipeline;
mod rate;
mod resample;
mod sanitize;
mod settings;
mod stages;
#[cfg(test)]
mod test_util;
mod validate;

pub use error::{Error, Setting};
pub use pipeline::Pipeline;
pub use settings::{AgcSettings, DEVICE_RATE_HZ, SUPPORTED_HOST_RATES, Settings, Tap};

/// Test-only hooks for the measurement harness. Not part of the public API; enabled only by
/// the `__test-hooks` feature.
#[cfg(feature = "__test-hooks")]
#[doc(hidden)]
pub mod __test_hooks {
    /// The committed voice-band stage coefficients, `[b0, b1, b2, a1, a2]` per section, so the
    /// harness can compare measured responses with the analytic one (tasks.md T040, T048).
    pub fn voiceband_sos() -> [[f64; 5]; 6] {
        crate::stages::voiceband_coeffs::VOICEBAND_SOS
    }
}

/// Crate version, e.g. `"0.1.0"`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
