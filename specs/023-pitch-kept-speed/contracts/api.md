# API contract: Pitch-Kept Playback Speed (spec 023, version 0.23.0)

Additive. The default mode is bit-identical to 0.22. **No struct changes; one published constant moves** (`MAX_CHANGES_PER_CALL` 142 → 143).

## Rust (`rr_dr60`)

- `speed::SpeedMode { PitchWithSpeed = 0, PitchKept = 1 }`, `speed::MODE_COUNT` (2).
- `Pipeline::set_playback_speed_mode(mode: SpeedMode)`, `Pipeline::playback_speed_mode() -> SpeedMode` (kept by `reset`/`reconfigure`; playback only; a change during playback lands at the next produced sample as a plain change, logged with tag 0).
- `schedule_change(Stream::Playback, position, 30, 0.0 | 1.0, tag)` (another value `InvalidControlValue`; the one-pass or record stream `ControlNotOnStream`).
- `Pipeline::playback_length_samples_in_mode(number, level, mode) -> Result<u64, Error>` (exact in both modes; `playback_length_samples` keeps the default mode's meaning).
- `Error::InvalidSpeedMode { mode: u32 }`.
- `changes::MAX_CHANGES_PER_CALL` 143; `controls::CONTROL_COUNT` 31; `rr_dr60::controls()[30]`.
- `__test_hooks`: `PitchMutation`, `pitch_shift_trace()`.

## C (`include/rr_dr60.h`)

- `RR_DR60_SPEED_MODE_PITCH_WITH_SPEED` 0, `RR_DR60_SPEED_MODE_PITCH_KEPT` 1.
- `RrDr60Status rr_dr60_set_playback_speed_mode(RrDr60Pipeline *, uint32_t mode)` (`RR_DR60_STATUS_INVALID_SPEED_MODE` 38 for another value), `rr_dr60_get_playback_speed_mode(RrDr60Pipeline *, uint32_t *out_mode)`.
- `rr_dr60_playback_length_samples_in_mode(RrDr60Pipeline *, uint32_t number, uint8_t level, uint32_t mode, uint64_t *out_total)`.
- `RR_DR60_MAX_CHANGES_PER_CALL` 143 (a 0.22 caller's buffer of 142 takes the short-buffer path).
- The promise: `scripts/abi-promise.sh 12a3c45` (0.22's smoke against 0.23's library) green locally and in CI.

## Conventions stated with the control

"An engine convention, not a device behavior: the device's speed method is undocumented (A-054). Pitch-kept mode stretches or compresses the decoded stream by repeating or dropping short stretches of the waveform at matching points (frames of about 20 ms with a matched overlap), so the pitch stays within 1 % of the recording's own while the length changes by the level's ratio; it adds artifacts of its own (a doubled transient at the slow levels, attenuated transients at the fast levels, a flutter on unvoiced sounds); the content sits within about 7 ms of the nominal place. Neither mode adds anything to a recording or removes anything from it beyond those artifacts."
