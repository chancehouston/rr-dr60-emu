# API contract: Pitch-Kept Playback Speed (spec 023, version 0.23.0)

Additive. The default mode is bit-identical to 0.22. **No struct changes; no published constant moves** (`MAX_CHANGES_PER_CALL` stays 142: the one-pass attains it and the mode is not on that stream; the player attains 69).

## Rust (`rr_dr60`)

- `speed::SpeedMode { PitchWithSpeed = 0, PitchKept = 1 }`, `speed::MODE_COUNT` (2).
- `Pipeline::set_playback_speed_mode(mode: SpeedMode)`, `Pipeline::playback_speed_mode() -> SpeedMode` (kept by `reset`/`reconfigure`; playback only; a change during playback lands at the next produced sample as a plain change, logged with tag 0).
- `schedule_change(Stream::Playback, position, 30, 0.0 | 1.0, tag)` (a whole number other than 0 and 1 `InvalidSpeedMode`, anything else `InvalidControlValue`; the one-pass or record stream `ControlNotOnStream`).
- `Pipeline::playback_length_samples_in_mode(number, level, mode) -> Result<u64, Error>` (exact in both modes; `playback_length_samples` keeps **the default mode's** meaning: its rustdoc and the header say so, and the guide tells an app to run progress on `place` and end on `finished`, never on a precomputed total; whether the old helper reads the current mode is a 0.9x candidate).
- `timeline::place_at_in_mode(index, host_rate_hz, start_level, start_mode, unit_count, entries)` beside the unchanged `place_at` (which keeps the default-mode reading).
- `Error::InvalidSpeedMode { mode: u32 }`.
- `controls::CONTROL_COUNT` 31; `rr_dr60::controls()[30]`.
- `__test_hooks`: `PitchMutation`, `pitch_shift_trace()`.

## C (`include/rr_dr60.h`)

- `RR_DR60_SPEED_MODE_PITCH_WITH_SPEED` 0, `RR_DR60_SPEED_MODE_PITCH_KEPT` 1.
- `RrDr60Status rr_dr60_set_playback_speed_mode(RrDr60Pipeline *, uint32_t mode)` (`RR_DR60_STATUS_INVALID_SPEED_MODE` 38 for another value), `rr_dr60_get_playback_speed_mode(RrDr60Pipeline *, uint32_t *out_mode)`.
- `rr_dr60_playback_length_samples_in_mode(RrDr60Pipeline *, uint32_t number, uint8_t level, uint32_t mode, uint64_t *out_total)`; `rr_dr60_place_at_in_mode(index, host_rate_hz, start_level, start_mode, unit_count, entries, entry_count, out)`.
- The schedule value is a `double` exactly 0.0 or 1.0 on control index 30, playback stream only; the setter takes a `uint32_t` (0.5 is `INVALID_CONTROL_VALUE`, 2 is `INVALID_SPEED_MODE`).
- The promise: `scripts/abi-promise.sh 12a3c45` (0.22's smoke sources against 0.22's header, linked to the current library): every `--x` mode's output byte-identical to the current smoke's, the default run exit 0 with its OK line (its CHECKs grew in 0.23); green locally and in CI.
- The audible onset for an app's highlight: a level change in pitch-kept mode P + ≤ 60 device samples + `playback_latency_samples()`; a mode change (and an into-3 change in pitch-kept mode) P + the 120-sample blend + the latency; a request during either blend lands at the blend's end. The engine keeps the mode across playbacks like the level; the app persists it across launches and its "device as modeled" preset turns it off. A shared listening is reproduced by replaying the playback change log (control 30 included).

## Conventions stated with the control

"An engine convention, not a device behavior: the device's speed method is undocumented (A-054). Pitch-kept mode lengthens or shortens the decoded stream by repeating or dropping short pieces of the waveform at matching points (frames of about 20 ms with a matched overlap), so the length changes by the level's ratio while the pitch stays put: on the harness's tones and a steady vowel within 1 % of the stored one. It has artifacts of its own: a repeated transient at the slow levels, attenuated transients at the fast levels when they fall closer than a frame, a flutter on unvoiced sounds (reported, not bounded). The audio heard at any output lies within about 11 ms (64 device samples) either side of the place the progress reports. Neither mode changes the stored recording; both read it as it is."
