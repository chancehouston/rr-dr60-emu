# Contract: the exported recording form, version 1 (feature 006; FR-014, research R-04)

A recording leaves and enters the memory as an opaque byte sequence. Hosts store or send it unchanged; they never parse it. The engine documents it so that a later engine can import it and so that reviewers can check the claims.

## Layout (little-endian)

| Offset | Size | Field | Value |
|---|---|---|---|
| 0 | 8 | magic | ASCII `RRDR60RC` |
| 8 | 2 | form version | 1 |
| 10 | 2 | flags | 0 |
| 12 | 4 | device rate, Hz | 6000 (A-026) |
| 16 | 2 | library version major | exporter's |
| 18 | 2 | library version minor | exporter's |
| 20 | 2 | library version patch | exporter's |
| 22 | 2 | reserved | 0 |
| 24 | 8 | sample count K | ≥ 1 |
| 32 | 4·K | samples | IEEE 754 binary32 bit patterns, the engine's device-rate stream after the VAS (narrowed, FR-009) |
| 32 + 4·K | 4 | CRC-32 | IEEE 802.3 (reflected polynomial 0xEDB88320, init 0xFFFFFFFF, final XOR 0xFFFFFFFF) over bytes 0 … 32 + 4·K − 1 |

Total length `36 + 4·K`. `export_len(n)` returns it; `export` writes exactly it.

## Validation on import (in this order; the memory is untouched until all pass)

| Check | Refusal |
|---|---|
| length < 8, or magic ≠ `RRDR60RC` | `UnknownForm` |
| version ≠ 1, or flags ≠ 0 | `UnsupportedFormVersion { version }` |
| length < 36 (checked before K is read) | `Damaged` |
| (length − 36) not a multiple of 4; K ≠ (length − 36) / 4 (compared as 64-bit, never computing 36 + 4·K); device rate ≠ 6000; K = 0; CRC mismatch; any sample NaN, ±Inf or subnormal | `Damaged` |
| K > remaining capacity, or no recording slot | `DoesNotFit` |

"Within the record path's output range" (FR-014) means finite and non-subnormal: with every stage bypassed the record path passes the sanitized input, so no magnitude bound is honest. The library version fields are informational and never cause a refusal.

## Guarantees

- `import(export(n))` in any engine of this version, at any host rate, yields a recording of the same length whose playback output is bit-identical (US3).
- The same recording exports to the same bytes every time.
- Any truncation and any single-byte change is refused (the length and CRC checks); the harness's corruption set proves it (research R-08).
- A later form version (encoded frames after A-004) changes only the version field and what follows the header; this version refuses it as `UnsupportedFormVersion`, and the later engine imports version 1 or says why not (US3 scenario 5).
