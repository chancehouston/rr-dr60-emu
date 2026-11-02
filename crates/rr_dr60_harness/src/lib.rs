//! Measurement harness for `rr_dr60` (spec FR-019 – FR-022).
//!
//! It generates stimuli in code (tones, sweeps, impulses, silence, DC, seeded
//! noise), measures the pipeline's response, checks it against the spec's
//! tolerances, and compares output against golden reference hashes. Every
//! check reports the requirement and the A-/S- IDs it verifies.

pub mod abi_checks;
pub mod agc_checks;
pub mod analysis;
pub mod battery_checks;
pub mod bypass_checks;
pub mod chain_checks;
pub mod changes_checks;
pub mod checks;
pub mod codec_checks;
pub mod configs;
pub mod controls_checks;
pub mod crossfade_checks;
pub mod golden;
pub mod manifest_checks;
pub mod meters_checks;
pub mod mic_checks;
pub mod output_checks;
pub mod output_clamp_checks;
pub mod pitch_checks;
pub mod record_checks;
pub mod record_contract_checks;
pub mod report;
pub mod settings_read_checks;
pub mod smoke_reference;
pub mod speed_checks;
pub mod split_checks;
pub mod stimulus;
pub mod vas_checks;
pub mod volume_ramp_checks;
