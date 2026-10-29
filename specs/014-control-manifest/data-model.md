# Data Model: Control Manifest (014)

## Types (Rust, `rr_dr60::controls`)

| Type | Fields / values |
|---|---|
| `ControlEntry` | `identifier: &str`, `name: &str`, `kind: ControlKind`, `timing: ControlTiming`, `paths: ControlPaths`, `stage: u8` (0 = none), `value: ControlValue`, `unit: &str`, `sources: &[&str]`, `assumptions: &[&str]`, `description: &str` |
| `ControlKind` | `DeviceControl = 0`, `UnitCondition = 1`, `ModelInternal = 2`, `EmulatorOption = 3`, `HostParameter = 4` |
| `ControlTiming` | `Live = 0`, `Idle = 1`, `IdleMemoryDependent = 2`, `HostFixed = 3` |
| `ControlPaths` | bit set: `RECORD = 1`, `PLAYBACK = 2`, `ONE_PASS = 4` |
| `ControlValue` | `Switch { default }`, `Integer { min, max, default }` (u64), `Continuous { min, max, default }` (f32), `Choice { options, default: Option<u32> }` |
| `ControlOption` | `identifier: &str`, `value: u32` (the C enumeration value, or hertz) |

Reads: `controls() -> &'static [ControlEntry]`, `control(identifier) -> Option<&ControlEntry>`, `ControlValue::step() -> u64/f32`. All `const`/static; no allocation.

Form rules (const-asserted in the core, re-checked in the harness): identifier ASCII `[a-z0-9_.]`, ≤ 32 chars, unique; name ≤ 40 chars; description one sentence, ≤ 160 chars, ends with ".", no digits, denylist (007 FR-011); IDs ascending; a choice's default is one of its options' values; `min ≤ default ≤ max`.

## The 29 entries

Ranges and defaults are the engine's (R-03 pins them); IDs follow R-06 after the comment corrections. Descriptions are drafts for the string review.

| # | Identifier | Kind | Timing | Paths | Stage | Value | Unit | IDs | Description |
|---|---|---|---|---|---|---|---|---|---|
| 1 | `route` | device | live | P, O | 11 | choice speaker=0, earphone=1; default 0 | | S-001 (comment added, R-06) | Sends playback to the modeled speaker or to the earphone output, each with its own tone and level. |
| 2 | `volume` | device | live | P, O | 11 | continuous 0 – 1; default `VOLUME_REFERENCE` | | A-043 | Sets the playback level; turned up, loud passages clip in the speaker amplifier. |
| 3 | `playback_speed` | device | live | P | 9 | integer 1 – 5; default 3 | level | S-001 | Plays recordings slower or faster than normal, with the pitch moving with the speed. |
| 4 | `host_rate_hz` | host parameter | host-fixed | R, P, O | 0 | choice 6000, 8000, 16000, 44100, 48000, 88200, 96000; no default | Hz | A-026 | The sample rate the host's audio runs at; the engine converts to and from the device rate. |
| 5 | `record_stage_enabled` | emulator option | idle | R, O | 4 | switch; on | | A-027 | Switches the codec's input band-pass filter off, opening the recorded band. |
| 6 | `playback_stage_enabled` | emulator option | idle | P, O | 10 | switch; on | | A-027 | Switches the codec's reconstruction filter off on playback. |
| 7 | `adc_clamp_enabled` | emulator option | idle | R, O | 4 | switch; on | | A-030 | Switches the converter's full-scale clipping off, so loud onsets pass beyond full scale. |
| 8 | `tap` | emulator option | idle | O | 0 | choice of eleven taps (C values 0 – 10); default route output = 5 | | | Chooses which stage's output the one-pass engine returns; recording and playback ignore it. |
| 9 | `seed` | host parameter | idle | R, O | 0 | integer 0 – 2⁶⁴−1; default 0 | | | Picks a different, reproducible realization of the microphone hiss. |
| 10 | `agc.enabled` | emulator option | idle | R, O | 3 | switch; on | | A-020 | Switches the limiter off, so loud sounds clip at the converter instead of being held down. |
| 11 | `agc.knee_dbfs` | model internal | idle | R, O | 3 | continuous −40 – 3; default −1 | dBFS | A-028 | Sets the level above which the limiter holds sound down; lowered, ordinary speech is limited and pumps. |
| 12 | `agc.attack_ms` | model internal | idle | R, O | 3 | continuous 1 – 50; default 3 | ms | A-034 | Sets how long a loud onset passes before the limiter acts. |
| 13 | `agc.release_ms` | model internal | idle | R, O | 3 | continuous 200 – 10000; default 1370 | ms | A-035 | Sets how long the level stays down after a loud sound. |
| 14 | `vas.enabled` | emulator option | idle | R, O | 5 | switch; on | | S-001, A-008 | Switches voice-activated pausing off, so quiet stretches stay in the recording. |
| 15 | `vas.mode` | emulator option | idle | R, O | 5 | choice drop=0, mute=1; default 0 | | S-001, A-008 | Chooses whether paused stretches are left out of the recording or kept as digital silence. |
| 16 | `vas.sensitivity` | device | idle | R, O | 5 | integer 1 – 5; default 3 | level | S-001, A-021 | Sets how quiet the input must be before recording pauses. |
| 17 | `vas.threshold_dbfs` | model internal | idle | R, O | 5 | continuous −60 – 0; default −40 | dBFS | A-022 | Sets the pause threshold directly, in finer steps than the sensitivity levels. |
| 18 | `vas.hang_ms` | model internal | idle | R, O | 5 | continuous 50 – 10000; default 1000 | ms | A-023 | Sets how long the input must stay quiet before recording pauses. |
| 19 | `vas.onset_ms` | model internal | idle | R, O | 5 | continuous 0 – 200; default 20 | ms | A-024 | Sets how much of the first sound after a pause is lost before recording resumes. |
| 20 | `memory.capacity_s` | emulator option | idle, memory-dependent | R, P | 8 | integer 1 – 86400; default 3600 | s | S-001 | Sets the total recording time the memory holds. |
| 21 | `memory.max_recordings` | emulator option | idle, memory-dependent | R, P | 8 | integer 1 – 9999; default 99 | | S-001 | Sets how many recordings the memory holds. |
| 22 | `speech_codec_enabled` | emulator option | idle, memory-dependent | R, P, O | 7 | switch; on | | A-004 | Switches the speech coder off, removing its compression artifacts from recordings. |
| 23 | `output.volume_amp_enabled` | emulator option | idle | P, O | 11 | switch; on | | A-031 | Switches the volume control and power amplifier off, with no gain or clipping. |
| 24 | `output.speaker_enabled` | emulator option | idle | P, O | 12 | switch; on | | A-010 | Switches the speaker's modeled response off on the speaker route. |
| 25 | `output.earphone_enabled` | emulator option | idle | P, O | 13 | switch; on | | A-032 | Switches the earphone output's modeled response off on the earphone route. |
| 26 | `output.clamp_enabled` | emulator option | idle | P, O | 0 | switch; off | | | Bounds the output to full scale, flattening peaks the device would let through. |
| 27 | `mic.capsule_enabled` | emulator option | idle | R, O | 1 | switch; on | | A-048 | Switches the microphone capsule's hiss off. |
| 28 | `mic.amp_enabled` | emulator option | idle | R, O | 2 | switch; on | | A-046, A-047, A-051 | Switches the microphone amplifier off, removing its thin low end and its clipping on loud input. |
| 29 | `mic.hiss_offset_db` | unit condition | idle | R, O | 1 | continuous −18 – 12; default 0 | dB | A-048 | Raises or lowers the microphone hiss, as capsules differ from unit to unit. |

Paths: R record, P playback, O one-pass. Order (FR-001): live controls 1 – 3, then `Settings` declaration order (`host_rate_hz` first, sub-structures in place). The table's order of rows 4 – 29 is checked against the struct by the core test, not by this document.

Notes:
- Rows 1 – 3: IDs from the default's comment (R-06): `VOLUME_REFERENCE` cites A-043; `speed::DEFAULT_LEVEL`'s doc line cites S-001; `Route`'s `#[default]` variant gains an S-001 tag. The speed ratios (A-054) are the levels' meaning, not the default's basis, and stay in the stage 9 manifest entry.
- Row 8: the eleven tap options are listed by identifier (`after_record` … `after_mic_amp`) with their C values; `AFTER_STAGE_10` is an alias and not listed.
- Row 22's stage is 7; stage 9's note also names it (FR-009).
