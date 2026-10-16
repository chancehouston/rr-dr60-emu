# Contract: C API (0.12.0)

## `RrDr60Settings`: 112 → 120 bytes (8-aligned)

| Offset | Field | Type | Default |
|---|---|---|---|
| 0 – 111 | (unchanged, 0.11) | | |
| 112 | `mic_capsule_enabled` | `bool` | true |
| 113 | `mic_amp_enabled` | `bool` | true |
| 114 | `reserved_012` | `uint8_t[2]` | 0 |
| 116 | `mic_hiss_offset_db` | `float` | 0.0 |

Const assertions:
- `size_of == 120`;
- `align_of == 8`;
- `offset_of(mic_capsule_enabled) == 112`;
- `offset_of(mic_amp_enabled) == 113`;
- `offset_of(reserved_012) == 114`;
- `offset_of(mic_hiss_offset_db) == 116`;
- `offset_of(mic_hiss_offset_db) + 4 == size_of`.

`struct_size = 112` (0.11) is refused with the existing invalid-argument status and the struct-size field (spec 010's pattern). A hiss offset that is not finite, or outside −18 … +12, is refused with the invalid-setting status and the new field `RR_DR60_SETTING_FIELD_MIC_HISS_OFFSET_DB = 16` (appended; 5 and 6 are retired). It is checked whether or not the capsule is on.

The fields land **inert in Phase 1**: `rr_dr60_settings_default` returns them on only from Phase 2, when the Rust default flips.

## Taps

`RR_DR60_TAP_AFTER_MIC_CAPSULE = 9` and `RR_DR60_TAP_AFTER_MIC_AMP = 10` are appended. Values 0–8 are unchanged.

## Version

`RR_DR60_VERSION_MINOR 12`.

## Smoke loop (`tests/c/smoke.c`)

- Every pre-012 case sets `mic_capsule_enabled = false` and `mic_amp_enabled = false` after `rr_dr60_settings_default`, so its pinned hash is unchanged.
- New cases:
  - (a) the 0.12 default one-pass at 48 kHz on a ±0.125 square wave plus silence: `RR_DR60_SMOKE_MIC_DEFAULT_HASH`;
  - (b) two consecutive recordings of silence with the hiss on, whose hashes differ (stream per recording), with a `reset` between them that does not rewind the counter: `RR_DR60_SMOKE_MIC_REC0_HASH` and `RR_DR60_SMOKE_MIC_REC1_HASH`;
  - (c) `struct_size = 112` refused;
  - (d) an offset of 13.0 refused, and a NaN offset refused.

  These hashes are pinned in `smoke_reference.rs` and checked by `ffi_parity`.

## Parity (`ffi_parity`)

For each of the five mic configurations at the seven rates, the C output equals the Rust output, using an exactly representable stimulus (spec 006's lesson).
