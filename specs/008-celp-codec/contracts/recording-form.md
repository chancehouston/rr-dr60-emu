# Contract: the exported recording form, version 2 (feature 008; FR-010 – FR-012; research R-07)

Supersedes [spec 006's version 1](../../006-record-playback-split/contracts/recording-form.md). Hosts store or send the bytes unchanged and never parse them.

## Layout (little-endian)

| Offset | Size | Field | Value |
|---|---|---|---|
| 0 | 8 | magic | ASCII `RRDR60RC` |
| 8 | 2 | form version | **2** |
| 10 | 2 | flags | 0 |
| 12 | 4 | device rate, Hz | 6000 (A-026) |
| 16 | 6 | library version major, minor, patch | exporter's (informational) |
| 22 | 2 | **encoding** | 1 = frames (the speech codec on), 2 = device-rate samples (bypassed) |
| 24 | 6 | **codec model version** major, minor, patch | `speech_codec::MODEL_VERSION` for frames; 0, 0, 0 for samples |
| 30 | 2 | reserved | 0 |
| 32 | 2 | frame samples | 120 for frames; 0 for samples |
| 34 | 2 | bits per frame | 80 for frames; 0 for samples |
| 36 | 4 | reserved | 0 |
| 40 | 8 | unit count K | ≥ 1 (frames or samples) |
| 48 | K × 10 or K × 4 | units | frames: the 10-byte frames in order; samples: IEEE 754 binary32 bit patterns |
| 48 + data | 4 | CRC-32 | IEEE 802.3 (reflected 0xEDB88320, init and final XOR 0xFFFFFFFF) over bytes 0 … 48 + data − 1 |

Total length 52 + K × unit bytes. `export_len(n)` returns it; `export` writes exactly it.

## Validation on import (in this order; the memory is untouched until all pass)

| Check | Refusal |
|---|---|
| length < 8, or magic ≠ `RRDR60RC` | `UnknownForm` |
| length < 12 | `Damaged` |
| version ≠ 2, or flags ≠ 0 (version 1 included: the never-shipped sample form of 0.6/0.7) | `UnsupportedFormVersion { version }` |
| length < 52 | `Damaged` |
| encoding ∉ {1, 2}; reserved ≠ 0; device rate ≠ 6000; K = 0; (frames) frame samples ≠ 120 or bits ≠ 80; (samples) model version ≠ 0; data length ≠ K × unit bytes (compared as 64-bit) | `Damaged` |
| (frames) codec model version not decodable by this release (≠ `MODEL_VERSION` before 1.0; not in the kept set from 1.0) | `UnsupportedCodecModelVersion { major, minor, patch }` |
| CRC mismatch | `Damaged` |
| (samples) any sample NaN, ±Inf or subnormal; (frames) any frame whose subframe-1 lag code is 127 | `Damaged` |
| encoding ≠ the engine's configured encoding | `EncodingDiffers` |
| K > remaining capacity, or no recording slot | `DoesNotFit` |

`EncodingDiffers` is checked before `DoesNotFit` so a host learns the real reason. The library version fields never cause a refusal.

## Guarantees

- `import(export(n))` in any engine of this version configured with the same encoding, at any host rate, yields a recording of the same length whose playback is bit-identical (US2 scenario 4).
- The same recording exports to the same bytes every time; the frames are the stored bytes verbatim.
- Any truncation and any single-byte change is refused (length and CRC); the harness's corruption set proves it, including a version 1 sequence, a wrong model version, the other encoding, and a frame with lag code 127.
- **Frozen decoders** (FR-011, clarify): from the **1.0 release** on, every codec model version that a released library wrote keeps its decoder in the library (`codec/v<major>_<minor>/`), and import dispatches on the field; a 1.x engine decodes every 1.0+ form bit-identically to the release that wrote it. **Before 1.0**, a model or form version may be dropped with a CHANGELOG entry; a sequence that needs a dropped decoder is refused as `UnsupportedCodecModelVersion`. This release decodes `MODEL_VERSION = 0.8.0` only.
- A later form version changes only the version field and what follows the header; this version refuses it as `UnsupportedFormVersion`.
- **Playback by frame** (FR-017): the frames are self-delimiting fixed-size units and the playback cursor counts frames, so a later feature can start playback at frame *n*, or play frames at another rate or time-scaled, without a form change; the decoder's state converges over a few frames from a cold start, so output identity with a from-start playback is not promised.
