//! Modeled signal-chain stages (docs/hardware/signal-chain.md).

// Used by the AGC stage (spec 002, tasks.md T022); until then only the coefficients exist.
#[allow(dead_code)]
pub(crate) mod agc_hilbert_coeffs;
mod biquad;
pub(crate) mod voiceband;
pub(crate) mod voiceband_coeffs;
