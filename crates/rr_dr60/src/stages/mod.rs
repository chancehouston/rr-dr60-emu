//! Modeled signal-chain stages (docs/hardware/signal-chain.md).
//!
//! Stage 3, the limiter, runs at the host rate before the rate conversion (spec 005 research
//! R-01); the clamp, the VAS and the two voice-band stages run at the 6 kHz device rate; the
//! output chain (stages 11 – 13, spec 009) runs at the host rate after the rate conversion.

pub(crate) mod biquad;
pub(crate) mod clamp;
pub(crate) mod first_order;
pub(crate) mod limiter;
#[allow(dead_code)] // spec 012: the generated microphone-path tables; read from Phase 2 (T025, T026)
pub(crate) mod mic_coeffs;
pub(crate) mod output;
#[allow(dead_code)] // the generated table carries the script's pinned figures, read by tests
pub(crate) mod output_coeffs;
pub(crate) mod vas;
pub(crate) mod voiceband;
pub(crate) mod voiceband_coeffs;
