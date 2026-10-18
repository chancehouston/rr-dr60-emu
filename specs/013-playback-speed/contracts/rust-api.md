# Rust API contract: Playback Speed (spec 013)

All additions; nothing removed or renamed. Version 0.13.0.

## `Pipeline`

```rust
/// Sets the playback speed level (spec 013 FR-001; A-054): 1 (slowest) … 5 (fastest), 3 normal.
/// Levels play at 3/5, 4/5, 1, 6/5 and 7/5 of normal speed with the pitch moving with the
/// speed (the modeled default, mechanism (b)). Live: settable at any time; from the first
/// device-rate sample a play call computes after it was set, the playback continues at the
/// new level from the place it reached (a change into level 3 waits until the place is a whole
/// stored sample, at most 4 device samples). Applies to `play` only: the record path and the
/// one-pass processing ignore it. Kept by `reset` and `reconfigure`. Real-time safe.
///
/// # Errors
/// `Error::InvalidSpeedLevel { level }` for 0 or above 5; the level is unchanged.
pub fn set_playback_speed(&mut self, level: u8) -> Result<(), Error>;

/// The playback speed level: the last valid setting (default 3).
pub fn playback_speed(&self) -> u8;

/// The host samples `play` produces for recording `number` played at a constant `level`:
/// ⌈(⌈5K ÷ k⌉ + T) · m ÷ l⌉ (the recording's own part, K stored device samples, k the level's
/// numerator, and the speed control's tail of T ≤ 46 device samples, 0 at level 3) plus
/// `drain_samples()`. Real-time safe.
///
/// # Errors
/// `Error::InvalidSpeedLevel` (checked first), then `Error::NoSuchRecording`.
pub fn playback_length_samples(&self, number: u32, level: u8) -> Result<u64, Error>;
```

`start_playback` additionally starts the speed control at the recording's start; `play` runs it. `drain_samples`, `playback_latency_samples` and `recording_length` are unchanged at every level.

## `PlayInfo`

```rust
#[non_exhaustive]
pub struct PlayInfo {
    pub produced: usize,
    pub finished: bool,
    pub frames: usize,
    /// Since 0.13: min(⌊place ÷ 120⌋, F − 1) with the codec on (0.12's value at level 3), 0
    /// bypassed.
    pub frame_index: u64,
    /// Since 0.13 (FR-009): the place in the recording, in whole stored device samples, of the
    /// last device sample computed in this call, at most the recording's length; 0 before
    /// any. It leads the audible output by `playback_latency_samples`, like `frame_index`.
    pub place: u64,
}
```

## `rr_dr60::speed` (new public module)

```rust
pub const LEVEL_COUNT: u8 = 5;
pub const DEFAULT_LEVEL: u8 = 3;                 // S-001: factory set to 3
pub const RATIO_DENOMINATOR: u32 = 5;
pub const RATIO_NUMERATORS: [u32; 5] = [3, 4, 5, 6, 7]; // A-054: 0.6 … 1.4
pub const LOOKAHEAD_SAMPLES: u32 = 28;           // stored samples read ahead (research R-03)
pub const MAX_TAIL_SAMPLES: u32 = 46;            // device samples played into the drain (R-04)
/// The speed ratio of `level` (1 … 5), or `None`.
pub fn ratio(level: u8) -> Option<f64>;
```

## `Error`

```rust
/// Spec 013 FR-001: a playback speed level other than 1 … 5.
InvalidSpeedLevel { level: u8 },
```

`Display`: "invalid playback speed level {level}: expected 1 to 5".

## Test hooks (`__test_hooks`, feature `__test-hooks`; not public API)

- `speed_place_fifths(p: &Pipeline) -> u64`: the place of the next output, in fifths.
- `SpeedMutation { ratio_k7: Option<u32>, no_tail: bool, snap_into_level_3: bool, lookahead_delta: i32, latch_after_switch: bool, no_drain_zero_stop: bool }` with `Pipeline::__with_speed_mutation` (research R-12's six mutants).
- `speed_bank(level: u8) -> Option<Vec<f64>>`: a level's bank as designed at construction (5 × 57, phase-major), for the independent reference (research R-08) and the pins.
- `speed_tail_len(k: u32, stored: u64) -> u64`: T(K, k), for the length checks.
