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
    /// The offset's valid range in dB (FR-003).
    pub const HISS_OFFSET_RANGE_DB: (f32, f32) = (-18.0, 12.0);
    /// The documented sound level at the microphone that reaches the codec's full scale at
    /// 1 kHz (A-029, 80–100 dB SPL, low confidence). Documentation: the engine applies no SPL.
    pub const FULL_SCALE_SPL_DB: f32 = 90.0;
}
```

## Changed behaviour

- **`Settings::new`:** `mic: MicSettings::DEVICE`. The default output changes. A DC offset in the input no longer reaches the limiter.
- **`Settings::seed`:** its docs now say it seeds the capsule's hiss (spec 012), with each recording's stream keyed by a recording counter.
- **`Pipeline::start_recording`:** keys the hiss to the recording counter and advances the counter. `reset` and `reconfigure` set the counter to 0.
- **`Tap::AfterAgc`:** its docs say it includes stages 1–2 when they run.
- **`Error::InvalidSetting`:** now also covers `hiss_offset_db` (non-finite or out of range).

## Test hooks (`__test_hooks`, not public API)

- **`MicMutation`:**
  - `no_input_hp`: the input high-pass skipped;
  - `symmetric_clip`: L₊ = L₋;
  - `shared_stream`: counter ignored;
  - `hiss_db_delta: f32`: the hiss level moved.
- **`Pipeline::__with_mic_mutation`.**
- **`MicAmp` clipper probe:** `__clip_only(x)` returns the clipper's output for a sample, for US2's limit checks.
