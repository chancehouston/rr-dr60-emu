# Data model: Battery State (018)

## The control

| Field | Type | Range / default | Where | Reset by |
|---|---|---|---|---|
| `battery_volts` | f32 | 2.2 … 3.0 inclusive; default 3.0 | `Pipeline` (an engine control beside the route, the volume and the speed) | never by `reset` / `reconfigure` (kept, like the route) |
| `last_battery` / `pending_battery` / `landed_battery` | f32 / f32 / Option<f32> | — | `StreamState` (spec 015), one per stream | with the stream |

Validation: finite and within the range, else `Error::InvalidBatteryVolts { volts }` (C `INVALID_BATTERY_VOLTS` 30); the schedule call narrows the double to f32 (`+ 0.0`) and validates the same way.

## The factor

- `b = ceiling_factor(volts)`: the form exactly 1.0 at 3.0 (`(volts − 3.0) / 1.2 + 1.0`, pinned; within 1 ULP of `(volts − 1.8) / 1.2`); 0.333 at 2.2 V.
- `OutputChain` state: `batt`, `batt_from`, `batt_to` (f64), `batt_k`, `batt_n` (u32; `batt_n` = the volume's N), `batt_inv_n`; the derived thresholds `knee_lo = k·b`, `knee_hi = (2 − k)·b`, `knee_den = 4·(1 − k)·b` recomputed when `batt` moves.
- Transition: as spec 011's `ramp_step` on b (sample 1 = the landing sample, sample N exactly `batt_to`); at once when the chain is fresh, stage 11 bypassed, the earphone route active, or at a route change.

## The clipper

`knee_b(x, k, b)` = `|x| ≤ k·b → x`; `|x| ≥ (2 − k)·b → ±b`; else `±(b − d² / (4·(1 − k)·b))`, d = (2 − k)·b − |x|. Bit-identical to `knee(x, k)` at b = 1.0.

## The meters (spec 016's `Meters`, appended)

| Field | Type | Meaning | Rest |
|---|---|---|---|
| `amp_clip_count` | u32 | speaker-route host samples of the call whose knee input is at or above `k·b` | 0 |
| `battery_ceiling` | f32 | the factor in force at the call's end | 1.0 |

`RrDr60Meters` 56 → 64 bytes (`amp_clip_count` at 56, `battery_ceiling` at 60); spec 016's size rule.

## Public data (`rr_dr60::battery`)

`ceiling_factor(volts) -> f32`, `ceiling_db(volts) -> f32`, `ALKALINE_TRAJECTORY: [(f32, f32); 7]` = [(0, 2.96), (1, 2.76), (2, 2.66), (4, 2.52), (6, 2.42), (8, 2.32), (10, 2.22)] (A-056, S-017), `BATTERY_VOLTS_RANGE`, `DEFAULT_BATTERY_VOLTS`.

## The manifest

Control `battery_volts` (UnitCondition, Live, stage 15, V, S-002/S-006/S-017, A-056); stage 15 `Assumed`, confidence low, model version 0.18.0, its controls note naming the control, the setter, the getter and `Meters`; stage 11's coverage note "the clip level's transistor spread; the battery's sag under load". `CONTROL_COUNT` 30.

## Golden

`golden-battery-v1.json`: 252 entries (6 law points × 2 routes × 3 stimuli × 7 rates) plus 7 scheduled-change entries with their logs; `BatteryGolden { volts, route, log, max_per_call }` on the entry.
