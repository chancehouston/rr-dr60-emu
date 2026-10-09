# Contract: Golden reference file (`golden-v1.json`)

**Location**: `crates/rr_dr60_harness/golden/golden-v1.json` | **Requirement**: FR-021, SC-003, SC-008

## Schema

```json
{
  "format": "rr_dr60-golden",
  "version": 1,
  "library_version": "0.1.0",
  "entries": [
    {
      "stimulus": "tone_1k_m20",
      "config": "default",
      "host_rate_hz": 48000,
      "n_samples": 24000,
      "sha256": "9f2c…(64 hex)",
      "head": ["00000000", "3a1b2c4d", "…16 items…"],
      "rms_dbfs": -20.03
    }
  ]
}
```

- `entries` is sorted by (`stimulus`, `config`, `host_rate_hz`) so diffs stay stable.
- `sha256`: SHA-256 over the concatenated little-endian bytes of each output `f32`'s bit pattern (`f32::to_bits().to_le_bytes()`).
- `head`: lowercase 8-digit hex of `to_bits()` for output samples 0–15.
- `rms_dbfs`: diagnostic only, rounded to 0.01 dB. It is never used for pass/fail.

## Stimuli (generated in code, bit-reproducible)

| Name | Definition | Duration |
|---|---|---|
| `impulse` | 1.0 at n = 0, otherwise 0 | 0.25 s |
| `tone_1k_m20` | 0.1·sin(2π·1000·n/fs), using `detmath::sin` with an f64 phase increment | 0.5 s |
| `sweep_log` | Exponential sine sweep, 20 Hz → 0.45·fs, amplitude 0.25 | 1.0 s |
| `noise_seed_0d60` | PCG32 (seed 0x0D60, stream 0) → uniform in [−0.5, 0.5) | 0.5 s |

Configurations: `default`, `record_only`, `playback_only`, `bypass_all`. Host rates: all 6. That gives 96 entries. The stimuli are processed in one block. Block-partition equivalence is tested separately (`determinism`).

## Compare and bless

- **Compare** (the default `cargo test`): every entry must match `sha256` exactly. On a mismatch, the test fails and reports the key, the first differing `head` index (if any), and the old and new `rms_dbfs`. Missing or extra entries also fail the test.
- **Bless**: `RR_DR60_BLESS=1 cargo test -p rr_dr60_harness --test golden` rewrites the file. The PR must include a CHANGELOG entry explaining why the output changed (constitution III). The PR template has a checkbox for this.
- **Platform rule**: there is one file for all targets (FR-014). Per-platform overrides are not supported by design.
