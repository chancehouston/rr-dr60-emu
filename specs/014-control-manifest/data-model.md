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

Reads: `controls() -> &'static [ControlEntry]`, `control(identifier) -> Option<&ControlEntry>`. The step (1 for integers, 0 for continuous) is derived by the renderers, not stored. All static; no allocation. `CONTROL_COUNT` carries an `// FR-001` tag.

Form rules, const-asserted in the core (and the const functions also called from a test, as `llvm-cov` does not see const evaluation): identifier ASCII `[a-z0-9_.]`, ≤ 32 chars, unique; name ≤ 40 chars; description one sentence, ≤ 160 chars, ends with ".", no digits; IDs ascending; a choice's default is one of its options' values; `min ≤ default ≤ max`. Harness only: the denylist (007 FR-011) on names and descriptions. Content rule (2026-10-29, post-implementation review): a description states the effect in the default engine configuration (every other stage on, the speech coder on).

## The 29 entries

Ranges and defaults are the engine's (R-03 pins them); IDs are the tags on each default's line after the comment work of R-06. Names and descriptions passed the product string review and the hardware review of 2026-10-29.

| # | Identifier | Name | Kind | Timing | Paths | Stage | Value | Unit | IDs | Description |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | `route` | Speaker or earphone | device | live | P, O | 11 | choice speaker=0, earphone=1; default 0 |  | S-001, A-032, A-042 | Sends playback to the modeled speaker or to the earphone output, each with its own response; their relative loudness is left to the host. |
| 2 | `volume` | Volume | device | live | P, O | 11 | continuous 0 – 1; default `VOLUME_REFERENCE` |  | S-002, A-043, A-044, A-045 | Sets the playback level; turned up, loud passages clip in the speaker amplifier. |
| 3 | `playback_speed` | Playback speed | device | live | P | 9 | integer 1 – 5; default 3 | level | S-001, S-012, A-054 | Plays recordings slower or faster than normal; the model lets the pitch follow the speed. |
| 4 | `host_rate_hz` | Host sample rate | host parameter | host-fixed | R, P, O | 0 | choice `hz_6000` … `hz_96000` (value = Hz); no default | Hz | A-026 | Tells the engine the sample rate of the host's audio; the engine converts to and from the device's own lower rate. |
| 5 | `record_stage_enabled` | Codec input filter | emulator option | idle | R, O | 4 | switch; on |  | A-027 | Switches the codec's input band-pass filter off, passing the lows and the top edge it removes; the device's sample rate still limits the top. |
| 6 | `playback_stage_enabled` | Codec output filter | emulator option | idle | P, O | 10 | switch; on |  | A-027 | Switches the codec's playback low-pass filter off, removing its roll-off at the top of the band. |
| 7 | `adc_clamp_enabled` | Converter clipping | emulator option | idle | R, O | 4 | switch; on |  | A-030 | Switches the converter's full-scale clipping off, so loud peaks pass above full scale instead of flattening at it. |
| 8 | `tap` | Listen point | emulator option | idle | O | 0 | choice of eleven taps in C-value order (`after_record` = 0 … `after_mic_amp` = 10); default `route_output` = 5 |  |  | Chooses which stage's output the one-pass engine returns, to hear the chain up to that point; recording and playback ignore it. |
| 9 | `seed` | Hiss seed | host parameter | idle | R, O | 0 | integer 0 – 2⁶⁴−1; default 0 |  | A-049 | Picks a different but repeatable pattern of microphone hiss, with the same level and tone. |
| 10 | `agc.enabled` | Limiter (AGC) | emulator option | idle | R, O | 3 | switch; on |  | A-020 | Switches the limiter off, so loud sounds reach the converter's clipping instead of being held just below it. |
| 11 | `agc.knee_dbfs` | Limiter threshold | model internal | idle | R, O | 3 | continuous −40 – 3; default −1 | dBFS | A-028 | Sets the level above which the limiter holds sound down; lowered, ordinary speech is limited and pumps. |
| 12 | `agc.attack_ms` | Limiter attack | model internal | idle | R, O | 3 | continuous 1 – 50; default 3 | ms | A-034 | Sets how long a loud onset passes at full level before the limiter acts; smaller overloads pass for longer. |
| 13 | `agc.release_ms` | Limiter release | model internal | idle | R, O | 3 | continuous 200 – 10000; default 1370 | ms | A-035 | Sets how long the level stays down after a loud sound. |
| 14 | `vas.enabled` | Voice-activated pause (VAS) | emulator option | idle | R, O | 5 | switch; on |  | S-001, A-008 | Switches voice-activated pausing off, so quiet stretches stay in the recording. |
| 15 | `vas.mode` | Pause handling | emulator option | idle | R, O | 5 | choice drop=0, mute=1; default 0 |  | S-001, A-008 | Chooses whether paused stretches are left out of the recording or kept as digital silence. |
| 16 | `vas.sensitivity` | Microphone sensitivity | device | idle | R, O | 5 | integer 1 – 5; default 3 | level | S-001, A-021, A-022, A-033 | Sets how quiet the input must be before recording pauses; the model applies the level to the pause threshold only. |
| 17 | `vas.threshold_dbfs` | Pause threshold | model internal | idle | R, O | 5 | continuous −60 – 0; default −40 | dBFS | A-022 | Sets the pause threshold at the factory sensitivity level; the other levels step up or down from it. |
| 18 | `vas.hang_ms` | Pause delay | model internal | idle | R, O | 5 | continuous 50 – 10000; default 1000 | ms | A-023 | Sets how long the input must stay quiet before recording pauses. |
| 19 | `vas.onset_ms` | Resume delay | model internal | idle | R, O | 5 | continuous 0 – 200; default 20 | ms | A-024 | Sets how much of the first sound after a pause is lost before recording resumes. |
| 20 | `memory.capacity_s` | Memory length | emulator option | idle, memory-dependent | R, P | 8 | integer 1 – 86400; default 3600 | s | S-001, A-038 | Sets the total recording time the memory holds; when it is used up, recording stops. |
| 21 | `memory.max_recordings` | Maximum recordings | emulator option | idle, memory-dependent | R, P | 8 | integer 1 – 9999; default 99 |  | S-001 | Sets how many recordings the memory holds before it refuses a new one. |
| 22 | `speech_codec_enabled` | Speech coder (CELP) | emulator option | idle, memory-dependent | R, P, O | 7 | switch; on |  | A-004, A-040, A-041 | Switches the speech coder off, removing its compression artifacts from recordings. |
| 23 | `output.volume_amp_enabled` | Volume and power amplifier | emulator option | idle | P, O | 11 | switch; on |  | A-031 | Switches the volume control and speaker amplifier off: unity gain and no clipping, and the volume control has no effect. |
| 24 | `output.speaker_enabled` | Speaker | emulator option | idle | P, O | 12 | switch; on |  | A-010 | Switches the small speaker's modeled tone off on the speaker route, removing its midrange resonance. |
| 25 | `output.earphone_enabled` | Earphone output | emulator option | idle | P, O | 13 | switch; on |  | A-032 | Switches the earphone output's modeled tone off on the earphone route, removing its low-frequency cut. |
| 26 | `output.clamp_enabled` | Output clamp | emulator option | idle | P, O | 0 | switch; off |  | A-042 | Bounds the returned output to full scale, flattening the peaks the modeled speaker and earphone paths produce above it. |
| 27 | `mic.capsule_enabled` | Microphone hiss | emulator option | idle | R, O | 1 | switch; on |  | A-048, A-052 | Switches the microphone capsule's hiss off, removing the steady background hiss from quiet passages. |
| 28 | `mic.amp_enabled` | Microphone amplifier | emulator option | idle | R, O | 2 | switch; on |  | A-046, A-047, A-051 | Switches the microphone amplifier off, removing its thin low end and its clipping on loud input. |
| 29 | `mic.hiss_offset_db` | Hiss level | unit condition | idle | R, O | 1 | continuous −18 – 12; default 0 | dB | A-048 | Raises or lowers the microphone hiss, as capsules differ between units; lowering it also helps when the host's microphone adds its own floor. |

Paths: R record, P playback, O one-pass. Order (FR-001): live controls 1 – 3, then `Settings` declaration order (`host_rate_hz` first, sub-structures in place). The table's order of rows 4 – 29 is checked against the struct by the core test, not by this document.

Notes:
- Order (FR-001) is checked by the harness reading the `pub <field>:` order of `Settings` and its sub-structures from `settings.rs` (the compiler does not check pattern order).
- Row 8: `AFTER_STAGE_10` is an alias and not listed. Row 4's option identifiers are `hz_6000`, `hz_8000`, `hz_16000`, `hz_44100`, `hz_48000`, `hz_88200`, `hz_96000`.
- Row 22's stage is 7; stage 9's note also names it (FR-009).
- Kind rules (R-10): a device control cites S-001 or S-002; a unit condition or model internal cites at least one A-###.
