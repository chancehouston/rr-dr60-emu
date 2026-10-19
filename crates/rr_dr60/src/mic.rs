//! The microphone path's public constants (signal-chain stages 1 – 2; spec 012
//! contracts/rust-api.md).
//!
//! Every figure behind stages 1 – 2 is a **design constant modeled on the circuit** (S-002 sheet
//! 3.1), not a measurement of a real unit: A-046 (the input high-pass), A-047 (the output
//! coupling and the roll-off), A-048 (the hiss level), A-049 (the generator), A-051 (the clip
//! limits). The generated tables behind them are `stages/mic_coeffs.rs`
//! (`tools/mic-chain/design_mic_chain.py`).
//!
//! The engine's input level reference is unchanged by these stages: 0 dBFS at 1 kHz still
//! reaches the codec's full scale. That corresponds to **about 90 dB SPL at the microphone**
//! (80 – 100 dB SPL, low confidence; A-029), a figure documented here and in the user guide,
//! not a setting: a host scales its own microphone input.

/// The valid range of [`MicSettings::hiss_offset_db`](crate::MicSettings::hiss_offset_db) in
/// dB, inclusive (spec 012 FR-003; A-048): −18 to +12 dB around the default hiss level. For
/// user interface sliders.
pub const HISS_OFFSET_RANGE_DB: (f32, f32) = (-18.0, 12.0); // A-048 (012 FR-003)
