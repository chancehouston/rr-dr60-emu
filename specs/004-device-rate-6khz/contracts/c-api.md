# Contract: C API (delta vs spec 003)

**No ABI change.** `RrDr60Settings` keeps its 72-byte 0.3 layout and every field; no function is added, removed or re-typed; all status and field enums are unchanged. The header is regenerated for documentation text only.

| Item | Change |
|---|---|
| `RrDr60Settings.host_rate_hz` doc | "8000, 16000, 44100, 48000, 88200 or 96000" → "6000, 8000, 16000, 44100, 48000, 88200 or 96000" |
| `rr_dr60_create` / `rr_dr60_settings_validate` | accept 6000; any other rate still returns `RR_DR60_STATUS_UNSUPPORTED_HOST_RATE` |
| `rr_dr60_latency_samples` | new values at every rate (research R-03); still a whole number of host samples, ≤ 20 ms with defaults |
| `rr_dr60_version_string` | `"0.4.0"` |
| Output | changes at every rate (new device rate, boundary and stages); bit-identical to the Rust API at every rate including 6000 (001 FR-023, spec FR-017) |

## Smoke test additions (`tests/c/smoke.c`)

- Create a pipeline at 6000 Hz with every stage bypassed, process a short tone, check the output equals the input sample for sample (identity) and `latency == 0`.
- Create at 7000 Hz: `RR_DR60_STATUS_UNSUPPORTED_HOST_RATE`, pipeline pointer NULL.
- The existing 48 kHz checks keep their meaning; the "1 kHz within 0.2 dB" check moves its reference to 765 Hz or keeps 1 kHz with the new combined loss (both inside the template; the task picks 1 kHz for continuity and widens nothing).
- The latency check (`<= FS / 50`, i.e. 20 ms) is unchanged.

## Rules (unchanged from 003)

1. Panics never cross the boundary.
2. Buffers are written only on success (except `out_info`, spec 003).
3. `struct_size` is read first.
4. Settings only grow at the end; this feature adds nothing.
