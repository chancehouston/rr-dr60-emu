# Data Model: An Insertable Component Slot and the Band Filter (spec 024)

## InsertionPoint (`rr_dr60::components::InsertionPoint`, constant data)

| Field | Value (the one entry) |
|---|---|
| `identifier` | `before_limiter` (C value 0, `RR_DR60_INSERTION_POINT_BEFORE_LIMITER`) |
| `name` | "Before the limiter" |
| `paths` | record (the recorder and the one-pass's record side) |
| `placement` | "Between stage 2, the microphone amplifier, and stage 3, the limiter, at the host rate; an addition of the emulator, the original recorder has no place for one." |
| `control_index` | 31 (the slot's control in the control manifest) |

## Component (`rr_dr60::components::Component`, constant data)

| Field | `band_filter` (C value 1; `none` is 0) |
|---|---|
| `value`, `identifier`, `name` | 1, `band_filter`, "Band filter" (indexed 0 in `components()`; none, 0, has no entry) |
| `description` | "One second-order filter, low-pass, high-pass, band-pass or notch, at a chosen frequency and Q, then a gain of −24 to +24 dB; an addition of the emulator, not a part of the original recorder." |
| `parameter_offset` | 0 (its base in the global store) |
| `latency_samples` | 0 (FR-013) |
| `paths` | record |
| `parameters` | the four below, in this order (the store's indices 0 – 3) |

| # | Identifier | Value | Range | Unit | Default | `scale` (the transition's, a slider's hint) | `control_index` |
|---|---|---|---|---|---|---|---|
| 0 | `band_filter.type` | Choice `low_pass` 0, `high_pass` 1, `band_pass` 2, `notch` 3 | 0 – 3 | — | 0 | linear (a 20 ms blend of two mixes, R-03) | 32 |
| 1 | `band_filter.frequency_hz` | Continuous | 20 – 20 000 (applied ≤ 0.45 × rate) | Hz | 1000 | log (of the applied frequency) | 33 |
| 2 | `band_filter.q` | Continuous | 0.5 – 10 | — | 0.7071 | log | 34 |
| 3 | `band_filter.gain_db` | Continuous | −24 – +24 | dB | 0 | decibel | 35 |

`COMPONENT_PARAMETER_BUDGET` = 16: all components' parameters together, each at its `parameter_offset`, never reused; a core test asserts offsets + counts ≤ 16 with no overlap (plan R4: the per-slot reading is the owner's question at the merge).

## ComponentStore (in `Pipeline`)

`component: u32` (0 none, 1 band filter), `values: [f32; 16]` (global indices: the band filter 0 – 3) and `tags: [u64; 16]` (the tag of each value's last landing, for the cross-stream mirror). Created at the defaults; kept by `set_component`, `reset`, `reconfigure`; written by `set_component_parameter` (idle, no entry) and by the call epilogue (a landing); copied into the slot block at `start_recording`; seeds both record paths' `RecordCtl` and inserts at construction, `reconfigure` and `start_recording` (`adopt`) with no entry.

## Insert (in each `RecordPath`, `stages/insert.rs`)

| Field | Meaning |
|---|---|
| `component` | none or band filter (mirrors the store; set by `set_component`) |
| `section` | `BandSection { s1, s2 }` (f64; flushed below 1e-30) and the cached `a1, a2, a3, k, gain`; `mix: [f64; 3]` a constant triple on `(x, k·v1, v2)` |
| `ramps[3]` | per continuous parameter: `{from, to, n, active}` in the parameter's scale (the frequency's endpoints clamped to the ceiling) |
| `blend` | the type blend: `{old_mix (a constant triple), new_type, k, dir}`; a type landing during a blend freezes the current mix and restarts at 0; back to the origin reverses (R-03) |
| `n_transition` | N = 20 ms of host samples (spec 011's count) |
| `ceiling_hz` | 0.45 × the host rate |

`RecordPath.run_insert: bool` sits in the padding after `run_limiter`; `insert` after `ctl` (no existing offset moves). `RecordPath::reset` calls `Insert::reset` (state zeroed, ramps and blend at their targets).

## Record control state (`changes::RecordCtl`, widened)

Slots 0 – 7 the record-side values (spec 017), 8 + i the global store index i (the band filter at 8 – 11); masks `u32`; `pending_value`, `pending_tag`, `landed_tag` `[_; 24]`; `last_component: [f32; 16]`; `RECORD_LOG` = 64 + 8 + 16 = 88. `new(settings, &store)`, `adopt(settings, &store)`, `begin_call(settings, &store, &tags24)`. `LIVE_COMPONENT` = `[32, 33, 34, 35]` (the control manifest's indices of the band filter's parameters).

## SlotBlock (`memory::SlotBlock`, per recording)

Gains `component: u32` and `store: [f32; 16]` (the store at `start_recording`).

## RecordReadout and RecordComponent

The Rust `RecordReadout` gains `component` and `store: [f32; 16]`. C keeps `RrDr60RecordReadout` unchanged (D9; plan R3) and adds `RrDr60RecordComponent { uint32_t struct_size; uint32_t component; uint32_t parameter_count; uint32_t reserved; float store[16]; }` (80 bytes), filled by `rr_dr60_processing_record_component`; `parameter_count` 0 means none written (a version-1 record).

## State: the slot

```
none ──set_component(band_filter), idle──► band_filter (state cleared, run_insert)
band_filter ──set_component(none), idle──► none (state cleared)
idle: set_component_parameter(index, v) ─► the store and both paths' last values; no entry
recording or playing: set_component ─► ComponentWhileActive (43), nothing changes
a parameter landing at P: ramp (or blend for the type) for N samples; a second landing restarts from the current value
reset / reconfigure / start_recording / set_component: ramps end at their targets and the state is cleared (the one-pass keeps it across calls)
```
