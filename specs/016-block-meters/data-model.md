# Data model: Per-Block Meters (016)

## The read-out (`rr_dr60::Meters`)

Public, `Copy`, `#[non_exhaustive]`, fields in this order (spec FR-002 – FR-003; the stage 4 names after the product review M3). "Point" is where the figure is read (research R-02); "Gate" is the construction-time condition under which the figure is read at all (R-03). Rest is the value of a fresh engine and of a meter whose point is not on the call's path (spec FR-004, D7).

| Field | Type | Group | Point | Gate | Unit | Rest | Spec |
|---|---|---|---|---|---|---|---|
| `input_peak` | f32 | record | the sanitized input (`sanitize_in`) | always on the record path | linear full-scale, ≥ 0 | 0 | FR-002 (a) |
| `limiter_input_peak` | f32 | record | stage 2's output (`MicFront::process`; the input with both stages off) | not at `Tap::AfterMicCapsule` | linear full-scale, ≥ 0 (above 1.0 possible) | 0 | (b) |
| `gain_reduction_db` | f32 | record | stage 3: `−20·log10(a(vc_max))`, 0 below the floor | the limiter runs | dB, ≥ 0 | 0 | (c) |
| `hold_charge` | f32 | record | stage 3: `vc / v_limit` at the end of the call | the limiter runs | 0 – 1 | 0 | (d) |
| `record_filter_peak` | f32 | record | stage 4's output before `ClampStage` | the half runs (not at a front tap) | linear full-scale, ≥ 0 (above 1.0 when the clamp counts) | 0 | (e) |
| `adc_clamp_count` | u32 | record | `ClampStage` output ≠ input | the half runs | device samples | 0 | (f) |
| `vas_paused` | bool | record | `VasStage::is_paused()` | the gate runs (`run_vas`) | — | false | (g) |
| `hang_remaining_samples` | u64 | record | `hang − silent` while recording, 0 while paused | the gate runs; `hang` otherwise | device samples | the configured hang | (h) |
| `reconstruction_peak` | f32 | playback | the interpolator's (or the identity's pending) output before `OutputChain::process` | `meter_playback`: the player, or the one-pass at a tap at or after stage 10 | linear full-scale, ≥ 0 | 0 | FR-003 (i) |
| `route_peak` | f32 | playback | `OutputChain::inner`'s result before `bound` | `meter_route`: the chain's tap is the route output (also with every chain stage off) | linear full-scale, ≥ 0 (speaker route up to about 2.3) | 0 | (j) |
| `output_clamp_count` | u32 | playback | `bound(r) ≠ r` | the clamp gate (`clamp_enabled && tap == RouteOutput`) | host samples | 0 | (k) |

`Meters::default()` is the all-zero value (the C zero-struct's twin); the engine's rest carries the configured hang: `Pipeline::rest_meters()` returns it and `new`, `reset`, `reconfigure` store it. The read-out describes the last **non-empty** processing call (research R-04); `Pipeline::meters()` copies it.

### Invariants

- Every f32 is finite; the peaks and the charge are ≥ 0; the charge ≤ 1.0 (`vc ≤ v_limit` by `Limiter::process`).
- `gain_reduction_db > 0` ⇒ the limiter ran in the call with `vc_max ≥ v_floor`; `hold_charge > 0` ⇒ the limiter ran since the last reset (state).
- `adc_clamp_count ≤` the device samples the call produced; `output_clamp_count ≤` the host samples it output.
- `vas_paused` ⇒ `hang_remaining_samples == 0`; the gate not running ⇒ `!vas_paused && hang_remaining_samples == hang`.
- `output.clamp_enabled == false` ⇒ `output_clamp_count == 0`; `!meter_route` ⇒ `route_peak == 0 && output_clamp_count == 0`; `!meter_playback` ⇒ the playback group is rest.
- A peak at a host-rate point equals `narrow_out(max |x|)` of the samples at that point, and at a tapped point `max |output at the tap|` with the following clamp off (spec FR-005).
- Partition identities (R-10): over any partition into blocks, the maximum of each peak's rows equals the one-block row's, the counts sum to it, the final state, hang and charge equal it.

## The accumulators (crate-private)

- `RecordPath`: `input_peak: f64`, `limiter_input_peak: f64`, `vc_max: f64`.
- `RecordHalf`: `filter_peak: f64`, `clamp_count: u32`.
- `PlaybackPath`: `reconstruction_peak: f64`, flag `meter_playback: bool`.
- `OutputChain`: `route_peak: f64`, `clamp_count: u32`, flag `meter_route: bool`.

`finalize` on each path narrows, converts and clears; `Pipeline::finalize_meters(record: bool, playback: bool)` writes the groups into the read-out and leaves the other group at rest (D2). Clearing happens at finalization, so a zero-length call (no finalization) leaves both the read-out and the accumulators untouched.

## State transitions of the read-out

| Event | Read-out |
|---|---|
| `Pipeline::new`, `reset`, `reconfigure` | `rest_meters()` |
| one-pass `process*` with `out.len() > 0` | both groups from the call (meters whose gate is off read rest) |
| `record` with input fed | record group; playback group rest |
| `stop_recording` with a flush | the flush's record group; playback rest |
| `stop_recording` with the flush skipped, zero-length calls, `start_*`, `stop_playback`, `erase*` | unchanged |
| `play` with `produced > 0` (including the drain) | playback group; record group rest |

## The C twin (`RrDr60Meters`)

`repr(C)`, 56 bytes, 8-aligned; offsets asserted at compile time (contracts/api.md). `struct_size` is caller-set; `rr_dr60_meters` fills the 56 bytes and zeroes `[56, struct_size)`.

## Test hooks (`__test-hooks`)

`stage4_trace`, `limiter_v_limit`, and `MeterMutation { filter_peak_after_clamp: bool, gain_from_state_after_update: bool, hang_off_by_one: bool, skip_clear: bool }` (`Default` = the committed behavior) with `Pipeline::__with_meter_mutation(settings, &MeterMutation)` (research R-12).

## The golden entry extension (`MetersGolden`)

Optional field `meters` of `GoldenEntry` (absent in every earlier file): `block_len: u32` (1024 for the one-pass rows), `blocks: Vec<MetersRow>` (the one-pass read-out per block), `record_blocks: Vec<MetersRow>` (`split_checks::RECORD_BLOCK`), `flush: Option<MetersRow>`, `play_blocks: Vec<MetersRow>` (`PLAY_BLOCK`), `drain_blocks: Vec<MetersRow>` (every drain call), `digest: Option<String>` (SHA-256 over the rows' bytes in place of the rows for a stimulus over 0.5 s; none at v1). A `MetersRow` is the eleven figures, f32s as 8-digit hex bit patterns, integers decimal, the state 0/1. The entry's `sha256` stays the one-pass output's; the record entries carry `seed` and `counter`.

Stimuli (0.5 s): `voiced_m20_half`, `vas_burst`, `limiter_step`, `clamp_square`. Configurations: `default` (0.14), `default_output_clamp`, `limiter_clamp_011` (`settings_011` with the limiter and the ADC clamp on), `output_only_amp` (the chain-only configuration), `meters_short_hang` (hang 50 ms, onset 20 ms, drop, stages 1 – 2 off), `meters_short_hang_mute`. Bless-time assertions: the tap and count identities on every entry; at least one paused row and one resume event in each short-hang entry.

## The harness reference gate (`meters_checks::ReferenceGate`)

Written from spec 003's data model (`specs/003-vas/data-model.md` › Per-sample processing), not from `vas.rs`: fields `threshold: f64`, `hang: u64`, `onset: u64`, `w: u64`, `state`, `silent`, `run`, `since_sound`, `dropped: u64`; `fn push(&mut self, d: f64)`; `fn paused(&self) -> bool`; `fn hang_remaining(&self) -> u64`; `fn dropped(&self) -> u64` (the cumulative drop count D for the event corroboration of R-08).

## The timing test (`meters_timing.rs`)

Release-only; written against the 0.14 public API so the A/B script can copy it into the baseline worktree. Prints `default_front_ns=<f>` (`configs::settings("default_output", 48_000)` + `MicSettings::DEVICE`, codec bypassed, speaker route) and `every_stage_off_ns=<f>` (`bypass_route_output`), each the fastest of 45 runs over 30 s of −20 dB noise in 4096-sample blocks, asserting `limiter_state == 0.0` after the default-front run.
