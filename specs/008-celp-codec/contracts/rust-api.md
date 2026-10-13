# Contract: Rust API additions (feature 008; research R-08)

All items are in `rr_dr60`; every public item has rustdoc citing its FR and S-/A-IDs. Names carry "speech", "encoder", "decoder" or "frame" (the spec's terminology rule).

## Settings and taps

| Item | Contract |
|---|---|
| `Settings::speech_codec_enabled: bool` | default `true` (A-004 adopted: the device has no bypass); bypasses stages 7 and 9 together; also selects the memory's encoding (`Encoding::Frames` when on, `Samples` when off); a bool, so no `Setting` variant |
| `Tap::AfterDecoder` | the speech decoder's output before stage 10 (one-pass only; the paths ignore the tap); with the codec bypassed it equals `AfterVas` |
| `Encoding` (`pub enum { Frames, Samples }`) | `unit_bytes()`, `unit_samples()`; `Pipeline::encoding()` |

## Constants and info (`rr_dr60::speech_codec`)

`FRAME_SAMPLES: u32 = 120`, `FRAME_MS: u32 = 20`, `SUBFRAME_SAMPLES = 60`, `BITS_PER_FRAME = 80`, `FRAME_BYTES = 10`, `LOOKAHEAD_SAMPLES = 30`, `DELAY_SAMPLES = 150`, `LPC_ORDER = 8`, `MIN_LAG = 15`, `MAX_LAG = 120`, `FRAMES_PER_SECOND = 50`, `MODEL_VERSION: [u16; 3] = [0, 8, 0]`; `pub fn info() -> SpeechCodecInfo` (`repr(C)`, `Copy`, 24 bytes: `frame_samples, frame_ms, bits_per_frame, delay_samples: u32; model_version: [u16; 3]; reserved: u16`). The encoder and decoder types stay `pub(crate)`.

## Reports

| Type | New fields |
|---|---|
| `BlockInfo` | `frames: usize` (one-pass frames encoded in the block) |
| `RecordInfo` | `frames: usize`, `frames_total: u64` |
| `StopOutcome` | `silence_fed_samples: u64` (post-VAS device samples of silence fed by the stop: the pre-encoder flush and the padding to the frame boundary) |
| `PlayInfo` | `frames: usize`, `frame_index: u64` |

All report structs stay `Copy`; `VasEvent` unchanged.

## Pipeline behaviour

| Call | Change |
|---|---|
| `process*` (one-pass) | with the codec on and the tap at or after `AfterDecoder`, the device stream is delayed by exactly 150 kept samples and passes through encoder and decoder; `latency_samples()` includes the delay |
| `start_recording` / `record` | frames appended per 120 kept samples (+ 30 look-ahead); `RecordInfo.frames*`; "memory full" at a frame boundary |
| `stop_recording` | feeds host silence for the pre-encoder latency, then to the frame boundary with the VAS held open; `StopOutcome.silence_fed_samples`; the length is a whole number of frames |
| `play` | decodes frame by frame; `PlayInfo.frames`, `frame_index`; the drain unchanged |
| `record_latency_samples` | includes the codec delay; `playback_latency_samples` unchanged; the sum rule holds |
| `export_len` / `export` / `import` | form version 2 (contracts/recording-form.md) |
| `reconfigure` | a change of `speech_codec_enabled` with recordings present → `Error::StoredExceedsCapacity` (documented as "the stored recordings do not fit the new configuration"); with an empty memory the store is re-reserved |
| `recording_length`, `memory_status` | unchanged types; figures are multiples of 120 device samples with the codec on |
| `reset` | also resets the one-pass codec pair |

## Errors

`Error::EncodingDiffers` ("the recording's encoding differs from the engine's configuration"), `Error::UnsupportedCodecModelVersion { major, minor, patch }`. `Error::StoredExceedsCapacity`'s documentation broadens; `UnsupportedFormVersion { version: 1 }` is what a 0.6/0.7 export now gets.

## Test hooks (`__test-hooks` feature only)

`Memory::__with_unit_samples(7)` to exercise the capacity rounding rule; `OnePass::__codec_fifo_fill()` to assert the 150-sample delay; `codec::__search_trace` (counts of criterion evaluations per frame) for the cost bound.
