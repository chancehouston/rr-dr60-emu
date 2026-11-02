# Data Model: An Insertable Component Slot and the Band Filter (spec 024)

## InsertionPoint (`rr_dr60::components::InsertionPoint`, constant data)

| Field | Value (the one entry) |
|---|---|
| `identifier` | `before_limiter` (C value 0, `RR_DR60_INSERTION_POINT_BEFORE_LIMITER`) |
| `name` | "Before the limiter" |
| `paths` | record (the recorder and the one-pass's record side) |
| `placement` | "Between stage 2, the microphone amplifier, and stage 3, the limiter, at the host rate; an addition of the app, the original recorder has no place for one." |

## Component (`rr_dr60::components::Component`, constant data)

| Field | `band_filter` (C value 1; `none` is 0) |
|---|---|
| `identifier`, `name` | `band_filter`, "Band filter" |
| `description` | "One second-order filter, low-pass, high-pass, band-pass or notch, at a chosen frequency and Q, then a gain of −24 to +24 dB; an addition of the app, not a part of the original recorder." |
| `latency_samples` | 0 (FR-013) |
| `paths` | record |
| `parameters` | the four below, in this order (the store's indices 0 – 3) |

| # | Identifier | Value | Range | Unit | Default | Scale of the transition |
|---|---|---|---|---|---|---|
| 0 | `band_filter.type` | Choice `low_pass` 0, `high_pass` 1, `band_pass` 2, `notch` 3 | 0 – 3 | — | 0 | a 20 ms blend of two mixes (R-03) |
| 1 | `band_filter.frequency_hz` | Continuous | 20 – 20 000 (applied ≤ 0.45 × rate) | Hz | 1000 | log-frequency |
| 2 | `band_filter.q` | Continuous | 0.5 – 10 | — | 0.7071 | log-Q |
| 3 | `band_filter.gain_db` | Continuous | −24 – +24 | dB | 0 | dB |

`COMPONENT_PARAMETER_BUDGET` = 16: the components' parameters together; a core test asserts the sum ≤ 16.

## ComponentStore (in `Pipeline`)

`component: u32` (0 none, 1 band filter) and `values: [f32; 16]` (the parameters by index; slots past a component's count unused). Created at the defaults; kept by `set_component`, `reset`, `reconfigure`; the cross-stream carrier (a landing on one record path is written here in the call's epilogue and becomes pending for the other path at its next call); copied into the slot block at `start_recording`.

## Insert (in each `RecordPath`, `stages/insert.rs`)

| Field | Meaning |
|---|---|
| `component` | none or band filter (mirrors the store; set by `set_component`) |
| `section` | `BandSection { s1, s2 }` (f64) and the cached `a1, a2, a3, k, gain`, `mix` |
| `ramps[3]` | per continuous parameter: `{from, to, n, active}` in the parameter's scale |
| `blend` | the type blend: `{old_mix, k, dir}` (spec 021's fade arithmetic over N) |
| `n_transition` | N = 20 ms of host samples (spec 011's count) |
| `ceiling_hz` | 0.45 × the host rate |

`RecordPath.run_insert: bool` sits beside `run_limiter` (the hot line).

## Record control state (`changes::RecordCtl`, widened)

Slots 0 – 7 the record-side values (spec 017), 8 – 23 the component parameter budget (the band filter at 8 – 11); masks `u32`; `pending_value`, `pending_tag`, `landed_tag` `[_; 24]`; `RECORD_LOG` = 64 + 8 + 16. `LIVE_COMPONENT` = `[32, 33, 34, 35]` (the control manifest's indices of the band filter's parameters).

## SlotBlock (`memory::SlotBlock`, per recording)

Gains `component: u32` and `parameters: [f32; 16]` (the store at `start_recording`).

## RecordReadout

Gains `component: u32` and `component_parameters: [f32; 16]`; the C tail `insert_before_limiter`, `reserved_024`, `component_parameters[16]` (376 → 448 bytes; plan N1).

## State: the slot

```
none ──set_component(band_filter), idle──► band_filter (state cleared, run_insert)
band_filter ──set_component(none), idle──► none (state cleared)
recording or playing: set_component ─► ComponentWhileActive (43), nothing changes
a parameter landing at P: ramp (or blend for the type) for N samples; a second landing restarts from the current value
reset / reconfigure / start_recording: ramps end at their targets; reset and reconfigure clear the state
```
