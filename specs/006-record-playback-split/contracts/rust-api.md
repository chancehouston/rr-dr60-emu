# Contract: Rust API changes (feature 006)

Breaking changes (0.5 → 0.6; nothing released): the device-rate output is narrowed to `f32` after the VAS (FR-009; every golden file re-blessed once), `Settings` gains `memory`, `Error` gains variants. Everything not listed is unchanged, including every one-pass call's signature and meaning.

## `rr_dr60::Settings` (one field added)

```text
pub memory: MemorySettings            // default MemorySettings::DEVICE
#[non_exhaustive] pub struct MemorySettings {
    pub capacity_s: u32,              // default 3600 (S-001: 60 minutes); range 1 ..= 86_400
    pub max_recordings: u32,          // default 99 (S-001); range 1 ..= 9_999
}
impl MemorySettings { pub const DEVICE: Self; }
```

`Setting::MemoryCapacityS`, `Setting::MemoryMaxRecordings` (validation as spec 002 FR-011: finite, in range, named). Capacity above the device's is allowed and documented as not the device's.

## New types

```text
#[non_exhaustive] pub enum State { Idle, Recording, Playing }
#[non_exhaustive] pub enum Route { #[default] Speaker, Earphone }
#[non_exhaustive] pub struct EngineState { pub state: State, pub route: Route, pub current: Option<u32>, pub speaker_muted: bool }
#[non_exhaustive] pub enum StopReason { MemoryFull }
#[non_exhaustive] pub struct RecordInfo { pub stored: usize, pub events: usize, pub paused: bool, pub stopped: Option<StopReason> }
#[non_exhaustive] pub enum StopResult { Kept { number: u32, length: u64 }, DiscardedEmpty }
#[non_exhaustive] pub struct StopOutcome { pub flush: RecordInfo, pub result: StopResult }
#[non_exhaustive] pub struct PlayInfo { pub produced: usize, pub finished: bool }
#[non_exhaustive] pub struct MemoryStatus { pub capacity_samples: u64, pub used_samples: u64, pub remaining_samples: u64, pub max_recordings: u32, pub count: u32, pub slots_remaining: u32 }
```

All `Clone, Copy, Debug, PartialEq, Eq`; the infos `Default` and `#[must_use]`.

## `rr_dr60::Pipeline` (new methods)

| Method | Real-time safe | Errors |
|---|---|---|
| `start_recording(&mut self) -> Result<(), Error>` | yes | `RecordingInProgress`, `PlaybackInProgress`, `MemoryFull`, `NoRecordingSlots` |
| `record(&mut self, input: &[f32], events: &mut [VasEvent]) -> Result<RecordInfo, Error>` | yes | `NotRecording` |
| `max_record_events(&self, frames: usize) -> usize` | yes | — |
| `stop_recording(&mut self, events: &mut [VasEvent]) -> Result<StopOutcome, Error>` | yes | `NotRecording` |
| `start_playback(&mut self, number: u32) -> Result<(), Error>` | yes | `RecordingInProgress`, `PlaybackInProgress`, `NoSuchRecording { number }` |
| `play(&mut self, output: &mut [f32]) -> Result<PlayInfo, Error>` | yes | `NotPlaying` |
| `stop_playback(&mut self) -> Result<(), Error>` | yes | `NotPlaying` |
| `set_route(&mut self, route: Route)` / `route(&self) -> Route` | yes | — |
| `state(&self) -> EngineState` | yes | — |
| `memory_status(&self) -> MemoryStatus` | yes | — |
| `recording_length(&self, number: u32) -> Result<u64, Error>` | yes | `NoSuchRecording` |
| `erase(&mut self, number: u32) -> Result<(), Error>` | no (bounded memmove, no allocation) | `RecordingInProgress`, `NoSuchRecording` |
| `erase_all(&mut self) -> Result<(), Error>` | no | `RecordingInProgress` |
| `export_len(&self, number: u32) -> Result<usize, Error>` | yes | `NoSuchRecording` |
| `export(&self, number: u32, out: &mut [u8]) -> Result<usize, Error>` | no (CRC over the recording) | `NoSuchRecording`, `BufferTooSmall { needed }` |
| `import(&mut self, bytes: &[u8]) -> Result<u32, Error>` (the new number) | no | `RecordingInProgress`, `UnknownForm`, `UnsupportedFormVersion { version }`, `Damaged`, `DoesNotFit` |
| `record_latency_samples(&self) -> u32`, `playback_latency_samples(&self) -> u32`, `drain_samples(&self) -> u32` | yes | — (host samples) |
| `footprint_bytes(&self) -> usize` | yes | — |

Changed: `reconfigure` is refused while recording or playing and with `StoredExceedsCapacity` when the stored recordings would not fit; otherwise it keeps the memory's contents and the route. `reset` is allowed in any state: a recording in progress is discarded without flush (its span popped, the store truncated to the previous recordings), a playback stops without drain, the paths reset, the memory's contents and the route are kept. `new` returns `AllocationFailed` when the store cannot be reserved. The one-pass calls return `RecordingInProgress` / `PlaybackInProgress` while a path is active and are otherwise unchanged (except FR-009's narrowing).

## Behaviour contract

- **Equivalence** (FR-007): for the same settings, seed and host rate, the first ⌈K·m/l⌉ samples `play` produces for a recording of K stored samples are bit-identical to the prefix of the one-pass output of the same input followed by `record_latency_samples() + ⌈m/l⌉` zeros; `play` produces exactly ⌈K·m/l⌉ samples before the drain and `drain_samples()` more, then `finished`.
- **Start state** (FR-004): `start_recording` and `start_playback` reset their path (limiter discharged, filters and converters cleared, VAS unpaused); the one-pass stream's state is untouched.
- **Stop** (FR-005): pushes `record_latency_samples()` zeros through the recorder (skipped when no input was fed; then `DiscardedEmpty`).
- **Events** (FR-006): `record` and `stop_recording` report VAS events with positions and lengths in the recording's device samples from its start.
- **Memory** (FR-010 – FR-013): time counted in stored device samples; mute mode's silence is stored and counts; recording stops at the exact sample the capacity is reached (`stopped = Some(MemoryFull)`, the span kept); numbering 1 ..= count, renumbered on erase.
- **Route** (FR-015): identical output on both routes in this version; readable; survives reset and reconfigure.
- **Narrowing** (FR-009): `narrow_device` on every kept post-VAS sample in the one-pass and the recorder; the measured delta is in the CHANGELOG.

## Test hooks (`__test-hooks` feature; unpublished)

- `Pipeline::__with_device_narrowing(settings, enabled: bool)`: the pre-0.6 un-narrowed one-pass, for `narrowing_delta` (R-02).
- `__test_hooks::n10() -> usize` (121) and `__test_hooks::stage_settling(sos, rel) -> usize` for the pinning test.
- Existing hooks unchanged.

## Version

`rr_dr60::VERSION` = "0.6.0" (workspace).
