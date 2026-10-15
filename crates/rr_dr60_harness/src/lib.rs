//! Measurement harness for `rr_dr60` (spec FR-019 – FR-022).
//!
//! It generates stimuli in code (tones, sweeps, impulses, silence, DC, seeded
//! noise), measures the pipeline's response, checks it against the spec's
//! tolerances, and compares output against golden reference hashes. Every
//! check reports the requirement and the A-/S- IDs it verifies.

pub mod agc_checks;
pub mod analysis;
pub mod chain_checks;
pub mod checks;
pub mod codec_checks;
pub mod configs;
pub mod golden;
pub mod manifest_checks;
pub mod output_checks;
pub mod output_clamp_checks;
pub mod report;
pub mod smoke_reference;
pub mod split_checks;
pub mod stimulus;
pub mod vas_checks;
