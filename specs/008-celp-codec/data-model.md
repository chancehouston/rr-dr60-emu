# Data Model: Speech Codec (feature 008)

Extends the data models of specs 006 (paths, memory, transport, byte form) and 003/005 (block reports). Stage internals of stages 3, 4, 5 and 10 are unchanged.

## The chain with the codec (research R-06)

```
host input → [limiter] → host→device → [stage 4] → [clamp] → [VAS] → f32 ─→ [7 SpeechEncoder] ─→ frames (10 B / 120 samples)
                                                                                   one-pass: ─→ FIFO(180) ─→ [9 SpeechDecoder] ─┐
                                                                                   split:    ─→ Memory (Frames) ─→ [9] ─────────┤
                                                                                                                                 ├─→ [stage 10] → device→host → out
Taps: AfterAgc (host rate) · AfterRecord (after 4 + clamp) · AfterVas (= before the codec) · AfterDecoder (NEW) · AfterPlayback (default)
Codec bypassed: f32 ─→ Memory (Samples) ─→ [stage 10], exactly spec 006.
```

## Codec constants (research R-02; `rr_dr60::speech_codec`)

| Name | Value | Unit |
|---|---|---|
| `FRAME_SAMPLES` | 120 | device samples |
| `FRAME_MS` | 20 | ms |
| `SUBFRAME_SAMPLES` | 60 | device samples (2 per frame) |
| `BITS_PER_FRAME` | 80 | bits |
| `FRAME_BYTES` | 10 | bytes |
| `LOOKAHEAD_SAMPLES` | 60 | device samples (defined by the push count: a frame is emitted on the push of sample 120(k+1) + 59) |
| `DELAY_SAMPLES` | 180 | device samples (`FRAME_SAMPLES + LOOKAHEAD_SAMPLES`; a multiple of 20, so a whole number of host samples at every rate) |
| `LPC_ORDER` | 8 | |
| `MIN_LAG`, `MAX_LAG` | 15, 120 | device samples |
| `FRAMES_PER_SECOND` | 50 | |
| `MODEL_VERSION` | (0, 8, 0) | the library version that last changed the coder |

`SpeechCodecInfo { frame_samples: u32, frame_ms: u32, bits_per_frame: u32, delay_samples: u32, model_version: [u16; 3], reserved: u16 }` (`repr(C)`, 24 bytes) mirrors them for C.

## Frame (research R-02; contracts/codec-design.md)

`Frame = [u8; 10]`, 80 bits, most significant bit of byte 0 first:

| Field | Bits | Range / coding |
|---|---|---|
| LSP indices 1 – 8 | 3, 3, 3, 3, 3, 3, 2, 2 = 22 | predicted scalar differences, companded levels |
| Subframe 1 lag code | 7 | 0 – 126 (127 unused: refused on import) |
| Subframe 1 codebook | 17 | positions 4 × 3, signs 4, grid 1 |
| Subframe 1 gains | 3 + 3 | adaptive index, fixed-gain correction index |
| Subframe 2 lag delta | 5 | −16 … +15 half-samples, clamped |
| Subframe 2 codebook | 17 | |
| Subframe 2 gains | 3 + 3 | |

Every value of every field except lag code 127 decodes (FR-010).

## Codec state (research R-04, R-05)

| Type | Fields | Reset |
|---|---|---|
| `SpeechEncoder` | input buffer `[i16; 240]` (60 past + 120 + 60 look-ahead) and fill count; past weighted speech `[i16; 240]` (the open-loop search at lag 120 over a 120-sample frame); previous quantized LSPs `[i16; 8]`; weighted-speech and synthesis filter memories; past excitation `[i16; 120 + 60 + 12]` (the adaptive codebook with the interpolator's margin; lags < 60 use the periodic extension); gain predictor state `[i16; 4]`; open-loop lag | all zero, LSPs to the mean vector, predictor to 0 (its mean energy is a constant) |
| `SpeechDecoder` | previous LSPs; synthesis filter memory `[i16; 8]`; past excitation; gain predictor; postfilter memories (short-term, tilt, gain smoother) | as above |
| `OnePassCodec` | `enc`, `dec`, `fifo: Fifo<f32, 180>` (a fixed ring: a pure 180-sample delay line), `decoded: [f32; 120]` with a read index | cleared; the FIFO's first 180 pops yield 0.0 |

`SpeechEncoder::push(sample: f32) -> Option<Frame>` returns `Some` on the push that completes the look-ahead of a frame (push 120(k+1) + 59, zero-based; every 120 pushes after the first 180). `SpeechDecoder::decode(&Frame, &mut [f32; 120])`. Both are `Clone + Debug`, allocation-free, and live in the core's `codec` module; the public re-exports are the constants and `SpeechCodecInfo` only (the encoder and decoder types are `pub(crate)`; hosts reach them through the engine).

## Memory (research R-07)

| Type | Fields | Notes |
|---|---|---|
| `Encoding` (public) | `Frames`, `Samples` | `unit_bytes()` 10 / 4, `unit_samples()` 120 / 1 |
| `Memory` | `store: Vec<u8>`, `encoding`, `spans: Vec<Span>` (units), `capacity_units`, `max_recordings`, `in_progress` | reservation `capacity_units × unit_bytes`; `capacity_units = capacity_s × 6000 / unit_samples` rounded down (exact for 120) |
| `MemoryStatus` | unchanged fields, sample-denominated (`units × unit_samples`) | `capacity_samples` is a multiple of 120 with the codec on |
| `Span` | `start`, `len` in units | `recording_length` = `len × unit_samples` |

Calls: `append_frame(&Frame) -> bool`, `append_sample(f32) -> bool` (one per encoding; the other panics in debug, is unreachable by construction), `units(span) -> &[u8]`, `import_units(encoding, bytes)`, `fits(settings, encoding)`, `footprint_bytes` (store capacity + spans × 16).

## Record and playback reports (research R-06)

| Type | New fields | Meaning |
|---|---|---|
| `BlockInfo` (one-pass) | `frames: usize` | frames encoded in the block (0 when bypassed) |
| `RecordInfo` | `frames: usize`, `frames_total: u64` | in the block; in the recording so far |
| `StopOutcome` | `silence_fed_samples: u64` | post-VAS device samples of silence produced after the last represented input sample, look-ahead included (= 120·F − n' + 60); 0 when the codec is bypassed |
| `PlayInfo` | `frames: usize`, `frame_index: u64` | frames decoded in the block; the index (0-based) of the frame being played at the block's end, held at the last frame during the drain |

`Cursor` (playback) counts `frame: u64` and `offset_in_frame: usize`; `total` host samples = ⌈120·F·m/l⌉ + drain.

## Settings, taps, errors (research R-08)

| Item | Change |
|---|---|
| `Settings` | `speech_codec_enabled: bool`, default `true` |
| `Tap` | `AfterDecoder` (between `AfterVas` and `AfterPlayback` in meaning; appended in the enum) |
| `Error` | `EncodingDiffers`; `UnsupportedCodecModelVersion { major: u16, minor: u16, patch: u16 }` |
| `Pipeline` | `encoding() -> Encoding`; `speech_codec_info() -> SpeechCodecInfo` (free function too) |

## Stored form, version 2 (research R-07; contracts/recording-form.md)

Header 48 bytes: magic, version 2, flags 0, device rate 6000, library version, encoding (1 frames / 2 samples), codec model version (3 × u16; 0,0,0 for samples), frame samples (120; 0 for samples), bits per frame (80; 0), reserved, unit count K; then K × 10 (or 4) bytes; then CRC-32. `export_len = 52 + K × unit_bytes`.

## Per-rate figures (research R-06; pinned by `latency` tests)

| Host rate | One-pass (006, measured 2026-10-12) | Codec delay in host samples (180 × rate / 6000, an integer at every rate) | One-pass (008) | One-pass ms | Bound: 50 ms + 006 figure |
|---|---|---|---|---|---|
| 6000 | 1 (0.17 ms) | 180 | 181 | 30.2 | 50.2 |
| 8000 | 81 (10.12 ms) | 240 | 321 | 40.1 | 60.1 |
| 16000 | 162 (10.12 ms) | 480 | 642 | 40.1 | 60.1 |
| 44100 | 444 (10.07 ms) | 1323 (exact: 180 × 147 / 20) | 1767 | 40.07 | 60.07 |
| 48000 | 483 (10.06 ms); record 242 + playback 241 | 1440 | 1923; record 1682, playback 241 | 40.06 | 60.06 |
| 88200 | 887 (10.06 ms) | 2646 | 3533 | 40.06 | 60.06 |
| 96000 | 966 (10.06 ms) | 2880 | 3846 | 40.06 | 60.06 |

The record path's latency grows by the same host-sample figure at each rate; the playback path's is unchanged. The design tool prints the full table (the 006 figures come from the committed code); the sum rule (record + playback = one-pass within one host sample) is pinned at all seven rates.

## Capacity figures (research R-07)

| Setting | Codec on | Codec bypassed |
|---|---|---|
| Default 3600 s | 180 000 frames = 1 800 000 B | 21 600 000 samples = 86 400 000 B |
| Span table | 100 × 16 B | same |
| Minimum 1 s | 50 frames | 6000 samples |
| Maximum 86 400 s | 4 320 000 frames = 43.2 MB | 2.07 GB (as 006) |
