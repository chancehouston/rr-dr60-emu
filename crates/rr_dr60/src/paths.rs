//! The record and playback halves and paths shared by the one-pass engine and the split
//! engine (spec 006 research R-01, R-02, R-06; data-model.md › Paths).
//!
//! Spec 006 T003 creates this file with the plan's derived per-rate figures pinned below;
//! T014 fills in the types.
#![allow(dead_code)]

/// The plan's derived figures (006 research R-06; data-model.md › Per-rate figures), pinned
/// here so the T013 tests can recompute each from the committed code and fail if one moved.
#[cfg(test)]
pub(crate) mod figures {
    // derived 2026-10-11 from rate.rs and voiceband_coeffs.rs (006 research R-06); the tests
    // below recompute each

    /// The seven host rates, in `SUPPORTED_HOST_RATES` order (spec 004 FR-002).
    pub(crate) const RATES: [u32; 7] = [6000, 8000, 16_000, 44_100, 48_000, 88_200, 96_000];
    /// One converter's linear-phase delay D, in host samples (`RatePlan::delay_host_samples`).
    pub(crate) const D: [u32; 7] = [0, 40, 80, 219, 238, 437, 476];
    /// Record-path latency, host samples: round(D + τ_rec · host / 6000) (006 FR-016).
    pub(crate) const RECORD_LATENCY: [u32; 7] = [1, 41, 81, 223, 242, 445, 485];
    /// Playback-path latency, host samples: round(D + τ_play · host / 6000) (006 FR-016).
    pub(crate) const PLAYBACK_LATENCY: [u32; 7] = [0, 40, 81, 221, 241, 442, 481];
    /// The one-pass latency table (001 R-10; the committed `latency` test prints it).
    pub(crate) const ONE_PASS_LATENCY: [u32; 7] = [1, 81, 162, 444, 483, 887, 966];
    /// Playback drain, host samples: ⌈N₁₀ · m / l⌉ + 2D (006 FR-008).
    pub(crate) const DRAIN: [u32; 7] = [121, 242, 483, 1328, 1444, 2653, 2888];
    /// Stage 10's settling length at −100 dB relative to its own impulse-response peak, in
    /// device samples (006 FR-008; engineering target: the −100 dB criterion).
    pub(crate) const N10: usize = 121;
    /// Group delay at 1 kHz of stage 4, in device samples (`voiceband_coeffs.rs`).
    pub(crate) const TAU_REC: f64 = 0.5387;
    /// Group delay at 1 kHz of stage 10, in device samples (`voiceband_coeffs.rs`).
    pub(crate) const TAU_PLAY: f64 = 0.3133;
    /// ‖h₁₀‖₁: the L1 norm of stage 10's impulse response (006 research R-02, review P2).
    pub(crate) const H10_L1: f64 = 2.65;
    /// The largest interpolator branch's L1 norm at 48 kHz (006 research R-02, review P2).
    pub(crate) const MAX_INTERPOLATOR_BRANCH_L1: f64 = 2.33;
}
