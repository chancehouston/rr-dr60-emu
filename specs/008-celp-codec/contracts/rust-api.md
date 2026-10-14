# Contract: Rust API additions (feature 008; research R-08)

All items are in `rr_dr60`; every public item has rustdoc citing its FR and S-/A-IDs. Names carry "speech", "encoder", "decoder" or "frame" (the spec's terminology rule).

## Settings and taps

| Item | Contract |
|---|---|
| `Settings::speech_codec_enabled: bool` | default `true` (A-004 adopted: the device has no bypass); bypasses stages 7 and 9 together; also selects the memory's encoding (`Encoding::Frames` when on, `Samples` when off); a bool, so no `Setting` variant |
| `Tap::AfterDecoder` | the speech decoder's output before stage 10 (one-pass only; the paths ignore the tap); with the codec bypassed it equals `AfterVas` |
| `Encoding` (`pub enum { Frames, Samples }`) | `unit_bytes()`, `unit_samples()`; `Pipeline::encoding()` |

## Constants and info (`rr_dr60::speech_codec`)

`DEVICE_RATE_HZ = 6000`, `FRAME_SAMPLES: u32 = 120`, `FRAME_MS: u32 = 20`, `SUBFRAME_SAMPLES = 60`, `BITS_PER_FRAME = 80`, `FRAME_BYTES = 10`, `LOOKAHEAD_SAMPLES = 60`, `DELAY_SAMPLES = 180`, `LPC_ORDER = 8`, `MIN_LAG = 15`, `MAX_LAG = 120`, `FRAMES_PER_SECOND = 50`, `MODEL_VERSION: [u16; 3] = [0, 8, 0]`; `pub fn info() -> SpeechCodecInfo` (`repr(C)`, `Copy`, 24 bytes: `frame_samples, frame_ms, bits_per_frame, delay_samples: u32; model_version: [u16; 3]; reserved: u16`). The encoder and decoder types stay `pub(crate)`.

## Reports

| Type | New fields |
|---|---|
| `BlockInfo` | `frames: usize` (one-pass frames encoded in the block) |
| `RecordInfo` | `frames: usize`, `frames_total: u64` |
| `StopOutcome` | `silence_fed_samples: u64` (post-VAS device samples of silence produced after the last represented input sample: the padding to the frame boundary plus the look-ahead; 0 with the codec bypassed) |
| `PlayInfo` | `frames: usize`, `frame_index: u64` |

All report structs stay `Copy`; `VasEvent` unchanged.

## Pipeline behaviour

| Call | Change |
|---|---|
| `process*` (one-pass) | with the codec on and the tap at or after `AfterDecoder`, the device stream is delayed by exactly 180 kept samples and passes through encoder and decoder; `latency_samples()` includes the delay; VAS event positions stay post-VAS stream indices |
| `start_recording` / `record` | frames appended per 120 kept samples (+ 60 look-ahead); `RecordInfo.frames*`; "memory full" at a frame boundary (the recording closes on the push that completes the frame filling the memory; the 60 look-ahead samples kept after it are not represented, so a VAS event in that block may name a position at most 60 device samples, the look-ahead, past the stored length; not clamped) |
| `stop_recording` | with no kept sample: discards as 006 does; otherwise feeds host silence for the pre-encoder latency, then to the frame boundary and the look-ahead, with the VAS gate held open in drop mode (a forced resume records its splice event); `StopOutcome.silence_fed_samples`; the length is a whole number of frames; bounded work (≤ flush + 179 device samples' worth); byte-identical to 006 when the codec is bypassed |
| `play` | decodes frame by frame; `PlayInfo.frames`, `frame_index` (held at the last frame during the drain); the drain unchanged |
| `record_latency_samples` | includes the codec delay; `playback_latency_samples` unchanged; the sum rule holds |
| `export_len` / `export` / `import` | form version 2 (contracts/recording-form.md) |
| `reconfigure` | a change of `speech_codec_enabled` with recordings present → `Error::StoredExceedsCapacity` (documented as "the stored recordings do not fit the new configuration"); with an empty memory the store is re-reserved; every other reconfigure (a tap change among them) follows 006's rules unchanged |
| `recording_length`, `memory_status` | unchanged types; figures are multiples of 120 device samples with the codec on |
| `reset` | also resets the one-pass codec pair |

## Errors

`Error::EncodingDiffers` ("the recording's encoding differs from the engine's configuration"), `Error::UnsupportedCodecModelVersion { major, minor, patch }`. `Error::StoredExceedsCapacity`'s documentation broadens; `UnsupportedFormVersion { version: 1 }` is what a 0.6/0.7 export now gets.

## Test hooks (`__test-hooks` feature only)

`Memory::__with_unit_samples(7)` to exercise the capacity rounding rule; `OnePass::__codec_fifo_fill()` to assert the 180-sample delay; `codec::__search_trace` (counts of criterion evaluations per frame) for the cost bound. `Tap` is `#[non_exhaustive]`, so `AfterDecoder` breaks no exhaustive match outside the crate.
