# Contract: Rust API (0.12.0)

## New

```rust
/// Signal-chain stages 1–2 (spec 012). Applied by `Pipeline::new` / `reconfigure`.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MicSettings {
    /// Stage 1, the capsule: its self-noise (the hiss). On by default (A-048).
    pub capsule_enabled: bool,
    /// Stage 2, the amplifier: input high-pass, gain, clipper, output coupling, roll-off.
    pub amp_enabled: bool,
    /// The hiss level offset in dB, −18 … +12, default 0 (spec 012 FR-003).
    pub hiss_offset_db: f32,
}
impl MicSettings {
    pub const DEVICE: Self;    // on, on, 0.0
    pub const BYPASSED: Self;  // off, off, 0.0
}

pub struct Settings { /* … */ pub mic: MicSettings, /* … */ }

pub enum Tap { /* … existing … */ AfterMicCapsule, AfterMicAmp }

pub mod mic {
    /// The offset's valid range in dB (FR-003), for UI sliders.
    pub const HISS_OFFSET_RANGE_DB: (f32, f32) = (-18.0, 12.0);
}
// No public full-scale SPL constant (plan review): the 90 dB SPL figure (80–100, low)
// lives in the rustdoc of `Settings::mic`, the README and A-029.

pub enum Setting { /* … existing … */ MicHissOffsetDb }   // appended; range() = −18 … +12
```

## Changed behaviour

- **`Settings::new`:** `mic: MicSettings::DEVICE`. The default output changes. A DC offset in the input no longer reaches the limiter.
- **`Settings::seed`:** its docs now say it seeds the capsule's hiss (spec 012), with each recording's stream keyed by a recording counter.
- **`Pipeline::start_recording`:** after the memory accepts the start, keys the hiss to the recording counter and advances the counter. The counter never rewinds: `reset` and `reconfigure` keep it, and the one-pass uses its current value (the next recording's stream).
- **`StopOutcome::silence_fed_samples`:** its docs say these are zero-valued input samples; with stage 1 on, the recording carries hiss there.
- **The taps' docs:** the recorder ignores the tap, as before.
- **`Tap::AfterAgc`:** its docs say it includes stages 1–2 when they run.
- **`Error::InvalidSetting`:** now also covers `hiss_offset_db` (non-finite or out of range), checked whether or not the capsule is on.

## Test hooks (`__test_hooks`, not public API)

- **`MicMutation`:**
  - `no_input_hp`: the input high-pass skipped;
  - `symmetric_clip`: L₊ = L₋;
  - `swapped_polarity`: the limits exchanged between the polarities;
  - `shared_stream`: the counter ignored;
  - `hiss_db_delta: f32`: the hiss level moved;
  - `clipper_off`: the linear twin for US2 AS4's oracle;
  - `force_retry_cap`: the ziggurat's cap path taken.
- **`Pipeline::__with_mic_mutation`.**
- **The clipper probe:** `Pipeline::__mic_clip_trace(&mut self, input, &mut [(f64, f64)])` processes a block at the tap after stage 2 and writes the engine's own clipper input and output per sample, for US2's static curve. It is not a reimplementation of the input high-pass in the harness.
