//! Modeled signal-chain stages (docs/hardware/signal-chain.md).
//!
//! Stage 3, the limiter, runs at the host rate before the rate conversion (spec 005 research
//! R-01); the clamp, the VAS and the two voice-band stages run at the 6 kHz device rate; the
//! output chain (stages 11 – 13, spec 009) runs at the host rate after the rate conversion.

pub(crate) mod biquad;
pub(crate) mod clamp;
#[allow(dead_code)] // 009 T012 (US1) uses it; Phase 1 lands the section alone
pub(crate) mod first_order;
pub(crate) mod limiter;
#[allow(dead_code)] // 009 T012 (US1) uses the tables; Phase 1 generates and pins them
pub(crate) mod output_coeffs;
pub(crate) mod vas;
pub(crate) mod voiceband;
pub(crate) mod voiceband_coeffs;
