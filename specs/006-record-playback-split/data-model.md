# Data Model: Record and Playback Paths with a Modeled Memory (feature 006)

Extends the data models of specs 001 (pipeline, settings, latency), 003 (block reports, events) and 005 (pipeline order). Stage internals are unchanged.

## Paths (research R-01)

```
host input → [limiter] → host→device conversion → [stage 4] → [clamp] → [VAS] → narrow to f32 ──┐
                                                    RecordHalf ─────────────────────────────────┘
                                                                                                 │
   one-pass: ──────────────────────────────────────────────────────────────────────────────────── ├─→ PlaybackHalf: widen → [stage 10] → device→host conversion → host output
   split:    recorder ──→ Memory (store, f32 device samples) ──→ player ─────────────────────────┘
```

| Type | Fields | Notes |
|---|---|---|
| `RecordHalf` | `record: RecordStage`, `run_record`, `clamp: ClampStage`, `vas: VasStage`, `run_vas`, `mute` | `process(f64) -> RecordOut`; `Keep(f32)`, `Resume(f32)`, `Drop`; the `f32` is `narrow_device(d)` (R-02) |
| `PlaybackHalf` | `playback: PlaybackStage`, `run_playback` | `process(f32) -> f64` |
| `RecordPath` | `limiter`, `run_limiter`, `down: Option<PolyphaseDown>`, `half: RecordHalf`, `counters: EventCounters` (owned here only) | `push(f32) -> Option<f32>` (the kept narrowed sample after the counters recorded the outcome), `take_event() -> Option<VasEvent>`; the recorder's counters use `l = m = 1` (device positions); the one-pass's `RecordPath` uses the rate plan (host positions, spec 003) |
| `PlaybackPath` | `half: PlaybackHalf`, `up: Option<PolyphaseUp>`, `pending: Option<f32>` (identity rate) | `push_device(f32)`, `try_next_host() -> Option<f32>` |
| `OnePass` | `rec: RecordPath`, `play: PlaybackPath`, `tap_after_agc` | the existing stream; its halves honour the tap; no counters of its own |

`narrow_device(d: f64) -> f32 = narrow_out(d)` (round to nearest, subnormals → 0.0, saturate at ±`f32::MAX`). Applied for every tap but `AfterAgc`.

## Memory (research R-03)

| Field | Type | Meaning |
|---|---|---|
| `store` | `Vec<f32>` | device-rate samples of all recordings, in order, the in-progress recording last; capacity reserved once (`try_reserve_exact`), never grows |
| `spans` | `Vec<Span { start: usize, len: usize }>` | one per recording in number order; capacity `max_recordings + 1` |
| `capacity_samples` | `usize` | `capacity_s × 6000` (A-026, A-038) |
| `max_recordings` | `usize` | S-001: 99 by default |

Derived: `used = store.len()`, `remaining = capacity_samples − used`, `count = spans.len()` (minus one while recording), `slots_remaining = max_recordings − count`, `footprint_bytes = capacity_samples × 4 + (max_recordings + 1) × 16`.

Operations: `open()` (push an empty span; refuses when `remaining == 0` → `MemoryFull`, or `count == max` → `NoRecordingSlots`), `append(f32) -> bool` (false exactly when the store is full after the push; the caller stops), `close(fed_any) -> StopResult` (`Kept { number, length }` or `DiscardedEmpty`, which pops the empty span), `erase(n)`, `erase_all()`, `import(&[f32])` (after validation; `DoesNotFit` when `len > remaining` or no slot), `span(n)`, `status() -> MemoryStatus`. Invariants: spans are contiguous and ordered; `Σ len == store.len()`; `store.capacity() == capacity_samples` at all times (asserted in debug and by `footprint()`).

`Clone`: manual, reserving the same capacity. `reconfigure`: idle only; keeps the data when it fits (same capacity → same `Vec`; different → a new reservation and a copy), else `StoredExceedsCapacity`.

## Transport (research R-05)

| Entity | Values / fields |
|---|---|
| `State` | `Idle`, `Recording`, `Playing` |
| `Route` | `Speaker` (default), `Earphone`; `#[non_exhaustive]`; exclusive (S-001); identical output in this version |
| `EngineState` | `state: State`, `route: Route`, `current: Option<u32>` (the number being recorded, `count + 1`, or played), `speaker_muted: bool` (`state == Recording`; S-002) |
| `RecordInfo` | `stored: usize` (device samples appended by this call), `events: usize`, `paused: bool`, `stopped: Option<StopReason>` |
| `StopReason` | `MemoryFull`; `#[non_exhaustive]` |
| `StopOutcome` | `flush: RecordInfo` (the stop flush's report; all zeros when skipped), `result: StopResult` |
| `StopResult` | `Kept { number: u32, length: u64 }`, `DiscardedEmpty` |
| `PlayInfo` | `produced: usize` (host samples written), `finished: bool` (the drain ended in this call; the state is idle) |
| `MemoryStatus` | `capacity_samples: u64`, `used_samples: u64`, `remaining_samples: u64`, `max_recordings: u32`, `count: u32`, `slots_remaining: u32` |
| Player cursor | `span index`, `offset` within the span, `drained: usize` (host samples emitted since the span ended), `fed_zero: bool` |
| Recorder flags | `fed_any: bool` (any host sample pushed since start, an empty block does not count; decides the flush and the empty case); an open mute-mode region is closed by a synthesized event at stop |

### Transitions

| From | Call | To | Refusals (in check order) |
|---|---|---|---|
| Idle | `start_recording` | Recording | `MemoryFull`, `NoRecordingSlots` |
| Recording | `start_recording` / `start_playback` / one-pass / `erase*` / `import` / `reconfigure` | Recording | `RecordingInProgress` |
| Playing | `start_recording` / `start_playback` / one-pass / `reconfigure` | Playing | `PlaybackInProgress` |
| Recording | `record(input, events)` | Recording, or Idle when the memory fills (`stopped = MemoryFull`; the span is closed, `Kept`; a following `stop_recording` is `NotRecording`) | — |
| Recording | `stop_recording` | Idle | — (flush unless `!fed_any`; `Kept` or `DiscardedEmpty`) |
| Idle | `start_playback(n)` | Playing | `NoSuchRecording { n }` (`n == 0 || n > count`) |
| Playing | `play(output)` | Playing, or Idle when the drain completes (`finished`) | — |
| Playing | `stop_playback` | Idle | — |
| Playing | `erase(n == current)` / `erase_all` | Idle (playback stopped first), then erased | — |
| Playing | `erase(n ≠ current)` | Playing (current renumbered if `n < current`) | — |
| any | `record` when not Recording | unchanged | `NotRecording` |
| any | `play` / `stop_playback` when not Playing | unchanged | `NotPlaying` |
| any | `set_route`, `route`, `state`, `memory_status`, `recording_length`, `export_len`, `export`, latencies, `drain_samples`, `footprint_bytes` | unchanged | `NoSuchRecording` for a bad number; `BufferTooSmall { needed }` for `export` |
| any | `reset` | Idle | — (a recording in progress is discarded without flush: span popped, store truncated to the previous recordings; a playback stops without drain; paths reset; memory contents and route kept) |

## Settings (contracts/rust-api.md, contracts/c-api.md)

| Setting | Default | Range | Trace |
|---|---|---|---|
| `memory.capacity_s` | 3600 | 1 ..= 86 400 | S-001 (60 min); range engineering target (006 FR-010) |
| `memory.max_recordings` | 99 | 1 ..= 9 999 | S-001 (99 items); range engineering target |

`MemorySettings::DEVICE = { 3600, 99 }`. `Setting::MemoryCapacityS`, `Setting::MemoryMaxRecordings`. Everything else unchanged; the tap setting does not affect the split paths (clarify Q5). The route is **not** a setting.

## Errors (research R-05)

`Error` gains `RecordingInProgress`, `PlaybackInProgress`, `NotRecording`, `NotPlaying`, `MemoryFull`, `NoRecordingSlots`, `NoSuchRecording { number: u32 }`, `DoesNotFit`, `UnknownForm`, `UnsupportedFormVersion { version: u16 }`, `Damaged`, `StoredExceedsCapacity`, `AllocationFailed`, `BufferTooSmall { needed: usize }`. C statuses 6–19 in that order (contracts/c-api.md).

## Per-rate figures (research R-06; default configuration; pinned by unit tests)

| Host rate | l/m | D | record latency | playback latency | one-pass | playback length for K stored | drain (host samples) | drain (ms) |
|---|---|---|---|---|---|---|---|---|
| 6 000 | 1/1 | 0 | 1 | 0 | 1 | K | 121 | 20.2 |
| 8 000 | 3/4 | 40 | 41 | 40 | 81 | ⌈4K/3⌉ | 242 | 30.2 |
| 16 000 | 3/8 | 80 | 81 | 81 | 162 | ⌈8K/3⌉ | 483 | 30.2 |
| 44 100 | 20/147 | 219 | 223 | 221 | 444 | ⌈147K/20⌉ | 1 328 | 30.1 |
| 48 000 | 1/8 | 238 | 242 | 241 | 483 | 8K | 1 444 | 30.1 |
| 88 200 | 10/147 | 437 | 445 | 442 | 887 | ⌈147K/10⌉ | 2 653 | 30.1 |
| 96 000 | 1/16 | 476 | 485 | 481 | 966 | 16K | 2 888 | 30.1 |

Constants: `N10 = 121` device samples (stage 10's impulse response below 10⁻⁵ of its peak from there on; engineering target: −100 dB); `drain = ⌈N10·m/l⌉ + 2D` (0 + 2D with stage 10 bypassed); τ_rec = 0.5387, τ_play = 0.3133 device samples at 1 kHz (from the committed coefficients); stop flush = record latency host samples of silence.

## Exported byte form (research R-04; contracts/recording-form.md)

Header 32 bytes (magic `RRDR60RC`, version 1, flags 0, device rate 6000, library version, reserved, sample count K) + 4·K bytes of little-endian `f32` + CRC-32. `export_len = 36 + 4K`.

## Harness additions (research R-08)

| Name | Definition | Used by |
|---|---|---|
| Record block / play block sizes | 4 801 host samples in, 997 out (engineering targets, so boundaries fall everywhere) | golden and live equivalence |
| Live stimulus set | the golden stimuli; the 005 speech-like pattern over a −60 dBFS floor; 30 s of floor then a +6 dBFS burst | FR-007 |
| Capacity scenario | capacity 10 s, 3 recordings; 25 s of −20 dBFS 1 kHz (VAS off), then with the VAS on in drop and mute mode over a pattern that pauses 20 s | US2 |
| Cross-rate tone | 1 kHz −20 dBFS 5 s, VAS off, record at R₁, play at R₂, fit over the last second | US1 AS5, matrix 7 × 7 |
| Drain stimulus | 1 kHz 0 dBFS 1 s, VAS off, stopped on a positive peak (`peak_phase`) | FR-008 |
| Corruption set | truncations (every 4 096 B and the last byte), 64 single-byte flips, foreign bytes with valid magic, version 2, flags 1, NaN sample with valid CRC, subnormal sample, K = 0, oversized | US3 |
| Mutants | drop the first stored sample before playback; flip one CRC bit before import | SC-002, SC-004 teeth |
