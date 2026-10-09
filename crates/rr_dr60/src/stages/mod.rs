//! Modeled signal-chain stages (docs/hardware/signal-chain.md).
#![cfg_attr(not(test), allow(dead_code))] // Wired into the pipeline in tasks.md T035.

mod biquad;
pub(crate) mod voiceband;
mod voiceband_coeffs;
