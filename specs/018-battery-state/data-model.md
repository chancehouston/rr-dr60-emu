# Data model: Battery State (018)

## The control

| Field | Type | Range / default | Where | Reset by |
|---|---|---|---|---|
| `battery_volts` | f32 | 2.2 … 3.0 inclusive; default 3.0 | `Pipeline` (an engine control beside the route, the volume and the speed) | never by `reset` / `reconfigure` (kept, like the route) |
| `last_battery` / `pending_battery` / `landed_battery` | f32 / f32 / Option<f32> | — | `StreamState` (spec 015), one per stream | with the stream |

Validation: finite and within the range, else `Error::InvalidBatteryVolts { volts }` (C `INVALID_BATTERY_VOLTS` 30); the schedule call narrows the double to f32 (`+ 0.0`) and validates the same way.

## The factor

- `b = ceiling_factor(volts)` = `((f64::from(volts) − 1.8) / 1.2) as f32`, exactly 1.0 at 3.0 (pinned); 0.333 at 2.2 V; the chain's `batt` is the widened f32. `set_battery` is idempotent against its target volts.
- `OutputChain` state: `batt`, `batt_from`, `batt_to` (f64), `batt_k`, `batt_n` (u32; `batt_n` = the volume's N), `batt_inv_n`; the derived thresholds `knee_lo = k·b`, `knee_hi = (2 − k)·b`, `knee_den = 4·(1 − k)·b` recomputed when `batt` moves.
- Transition: as spec 011's `ramp_step` on b (sample 1 = the landing sample, sample N exactly `batt_to`); at once when the chain is fresh, stage 11 bypassed, the earphone route active, or at a route change (`reset_sections` ends a transition and recomputes the thresholds).

## The clipper

`knee_b(x, k, b)` = `|x| ≤ k·b → x`; `|x| ≥ (2 − k)·b → ±b`; else `±(b − d² / (4·(1 − k)·b))`, d = (2 − k)·b − |x|. Bit-identical to `knee(x, k)` at b = 1.0.

## The meters (spec 016's `Meters`, appended)

| Field | Type | Meaning | Rest |
|---|---|---|---|
| `amp_clip_count` | u32 | speaker-route host samples of the call the knee or the ceiling changed (`|x| > k·b` at the knee's input); written at the taps that run the knee | 0 |
| `battery_ceiling` | f32 | the factor in force at the call's end | `ceiling_factor(battery_volts)` (1.0 at the default; `Meters::default()` carries 0.0) |

`RrDr60Meters` 56 → 64 bytes (`amp_clip_count` at 56, `battery_ceiling` at 60); the size rule: below 56 refused, 56 … 63 filled to the size, 64 and above filled with the tail zeroed (spec 016 amended 2026-10-30). `meters_checks::row` unchanged (eleven columns); `battery_row` adds the two.

## Public data (`rr_dr60::battery`)

`ceiling_factor(volts) -> f32`, `ceiling_db(volts) -> f32` (both total over finite inputs), `ALKALINE_TRAJECTORY: [(f32, f32); 7]` = [(0, 2.96), (1, 2.76), (2, 2.66), (4, 2.52), (6, 2.42), (8, 2.32), (10, 2.22)] (hours of recording under S-001's load; row 0 is a loaded reading, not the default; A-056, S-017), `NIMH_PAIR_VOLTS` = 2.45, `BATTERY_VOLTS_RANGE`, `DEFAULT_BATTERY_VOLTS`; generated into `output_coeffs.rs`, re-exported by `battery.rs`.

## The manifest

Control `battery_volts` at index 29 (appended; UnitCondition, Live, stage 15, V, S-002/S-006/S-017, A-056); stage 15 `Assumed`, confidence low, model version 0.18.0, controls note `set_battery_volts, battery_volts; Meters`, coverage note "the 3.3 V converter and its rails; the cells' sag under load; supply noise; the low-battery indicator and cut-off"; stage 11's coverage note "the 159 Hz corner's knob shift; the clip level's spread; the battery's sag; wiper noise; knob speed; the beeps". `CONTROL_COUNT` 30.

## Golden

`golden-battery-v1.json`: 259 entries (6 law points × 2 routes × 3 stimuli × 7 rates = 252, plus 7 scheduled-change entries with their logs); `BatteryGolden { volts, route, battery_row, log, max_per_call }` on the entry.
