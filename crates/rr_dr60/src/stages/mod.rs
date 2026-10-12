//! Modeled signal-chain stages (docs/hardware/signal-chain.md).
//!
//! Stage 3, the limiter, runs at the host rate before the rate conversion (spec 005 research
//! R-01); the clamp, the VAS and the two voice-band stages run at the 6 kHz device rate.

pub(crate) mod biquad;
pub(crate) mod clamp;
pub(crate) mod limiter;
pub(crate) mod vas;
pub(crate) mod voiceband;
pub(crate) mod voiceband_coeffs;
