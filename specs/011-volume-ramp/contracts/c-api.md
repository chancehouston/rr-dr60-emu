# Contract: C API (feature 011; research R-07, R-09)

| Item | Contract |
|---|---|
| `RrDr60Settings` | **Unchanged**: 112 bytes, alignment 8, every offset as in 0.10 (FR-011); the existing layout assertions unchanged. |
| `RR_DR60_VERSION_MINOR` | 11 (`RR_DR60_VERSION_MAJOR` 0, `RR_DR60_VERSION_PATCH` 0). |
| `RR_DR60_VOLUME_TRANSITION_MS` | New define, `20` (A-045): the length of a volume transition in milliseconds. From the literal Rust const, so cbindgen emits it. |
| `rr_dr60_set_volume` | Unchanged signature and statuses; the comment describes the 20 ms transition, the start-at-once rules and the read-back rule (as the Rust contract). |
| `rr_dr60_get_volume` | Unchanged; returns the last valid setting. |
| statuses | none new. |

## Smoke loop (`crates/rr_dr60_ffi/tests/c/smoke.c`)

- The existing route cases (positions set before each `start_playback`) keep their pinned hashes: each playback starts fresh (research R-04, R-06).
- One new case: the recorded square played on the speaker route with the position changed mid-playback (0.1 at the start, 1.0 before block 10, 0.4 before block 11), hashed and compared with `RR_DR60_SMOKE_SPEAKER_RAMP_HASH` (pinned by `smoke_reference.rs`).
- `RR_DR60_VOLUME_TRANSITION_MS == 20` checked.

## Parity (`ffi_parity.rs`)

A mid-playback change case at the seven rates (0.1 → 1 → 0.4 at fixed blocks, both routes): the C output equals the Rust output bit for bit.

The header is regenerated with cbindgen 0.29.4 and the drift check is clean.
