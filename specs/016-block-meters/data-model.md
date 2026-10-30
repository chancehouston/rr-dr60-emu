# Data model: Per-Block Meters (016)

## The read-out (`rr_dr60::Meters`)

Public, `Copy`, `#[non_exhaustive]`, fields in this order (spec FR-002 – FR-003). "Point" is where the figure is read (research R-02). Rest is the value of a fresh engine and of a meter whose code did not run in the call (spec FR-004, D7).

| Field | Type | Group | Point | Unit | Rest | Spec |
|---|---|---|---|---|---|---|
| `input_peak` | f32 | record | the sanitized input (`sanitize_in`) | full-scale, ≥ 0 | 0 | FR-002 (a) |
| `limiter_input_peak` | f32 | record | stage 2's output (`MicFront::process`; the input with both stages off); rest at `Tap::AfterMicCapsule` | full-scale, ≥ 0 | 0 | (b) |
| `gain_reduction_db` | f32 | record | stage 3: `−20·log10(a(vc_max))`, 0 below the floor | dB, ≥ 0 | 0 | (c) |
| `hold_charge` | f32 | record | stage 3: `vc / v_limit` at the end of the call | 0 – 1 | 0 | (d) |
| `filter_peak` | f32 | record | stage 4's output before `ClampStage` | full-scale, ≥ 0 | 0 | (e) |
| `converter_clamp_count` | u32 | record | `ClampStage` output ≠ input | device samples | 0 | (f) |
| `vas_paused` | bool | record | `VasStage::is_paused()` when the gate runs | — | false | (g) |
| `hang_remaining_samples` | u64 | record | `hang − silent` while recording, 0 while paused, `hang` when the gate does not run | device samples | the configured hang | (h) |
| `reconstruction_peak` | f32 | playback | the interpolator's (or the identity's pending) output before `OutputChain::process` | full-scale, ≥ 0 | 0 | FR-003 (i) |
| `route_peak` | f32 | playback | `OutputChain::inner` result when the chain's tap is the route output, before `bound` | full-scale, ≥ 0 | 0 | (j) |
| `output_clamp_count` | u32 | playback | `bound(r) ≠ r` when the clamp gate is on | host samples | 0 | (k) |

`Meters::default()` is the rest state with `hang_remaining_samples = 0`; the engine's rest (`new`, `reset`, `reconfigure`) carries the configured hang. The read-out describes the last **non-empty** processing call (research R-04); `Pipeline::meters(&self) -> Meters` copies it.

### Invariants

- Every f32 is finite and, for the peaks and the charge, ≥ 0; the charge ≤ 1.0 (`vc ≤ v_limit` by `Limiter::process`).
- `gain_reduction_db > 0` ⇒ the limiter ran in the call with `vc_max ≥ v_floor`; `hold_charge > 0` ⇒ the limiter ran at some time since the last reset (state).
- `converter_clamp_count ≤` the device samples the call produced; `output_clamp_count ≤` the host samples it output.
- `vas_paused` ⇒ `hang_remaining_samples == 0`; the gate not running ⇒ `!vas_paused && hang_remaining_samples == hang`.
- With `output.clamp_enabled == false`: `output_clamp_count == 0`. With the chain at a tap other than the route output: `route_peak == 0 && output_clamp_count == 0`.
- A peak at a host-rate point equals `narrow_out(max |x|)` of the samples at that point; the figure at a tapped point equals `max |output at the tap|` with the following clamp off (spec FR-005).

## The accumulators (crate-private)

`MeterAcc` fields live in the paths (research R-01):

- `RecordPath`: `input_peak: f64`, `limiter_input_peak: f64`, `vc_max: f64` (the largest limiter entry voltage; 0 when the limiter did not run), and in `RecordHalf`: `filter_peak: f64`, `clamp_count: u32`.
- `PlaybackPath`: `reconstruction_peak: f64`; in `OutputChain`: `route_peak: f64`, `clamp_count: u32`.

`finalize(&mut self) -> MeterGroup` on each path narrows, converts and clears; `Pipeline::finalize_meters(record: bool, playback: bool)` writes the groups into the read-out, leaving the other group at rest (D2: a record call leaves the playback group rest, a play call the record group; the one-pass writes both). Clearing happens at finalization, so a zero-length call (no finalization) leaves both the read-out and the accumulators untouched.

## State transitions of the read-out

| Event | Read-out |
|---|---|
| `Pipeline::new`, `reset`, `reconfigure` | rest (with the configured hang) |
| one-pass `process*` with `out.len() > 0` | both groups from the call (groups whose code did not run read rest) |
| `record` with input fed | record group; playback group rest |
| `stop_recording` with a flush | the flush's record group; playback rest |
| `stop_recording` with the flush skipped, zero-length calls, `start_*`, `stop_playback`, `erase*` | unchanged |
| `play` with `produced > 0` (including the drain) | playback group; record group rest |

## The C twin (`RrDr60Meters`)

`repr(C)`, 56 bytes, 8-aligned; offsets asserted at compile time (contracts/api.md). `struct_size` is caller-set; `rr_dr60_meters` fills the 56 bytes and zeroes `[56, struct_size)`.

## The golden entry extension (`MetersGolden`)

Optional field `meters` of `GoldenEntry` (absent in every earlier file): `blocks: Vec<MetersRow>` (the one-pass or record read-out per block; each row the eleven figures, f32s as 8-digit hex bit patterns, integers decimal, the state 0/1), `flush: Option<MetersRow>`, `play_blocks: Vec<MetersRow>`, `drain_last: Option<MetersRow>`, and `digest: Option<String>` (SHA-256 over the rows' bytes when the rows are omitted for a stimulus over 0.5 s; none at v1). The entry's `sha256` stays the one-pass output's.

## The harness reference gate (`meters_checks::ReferenceGate`)

Fields `threshold: f64`, `hang: u64`, `onset: u64`, `w: u64`, `state`, `silent`, `run`, `since_sound`; `fn push(&mut self, d: f64)`; `fn paused(&self) -> bool`; `fn hang_remaining(&self) -> u64`. Written from spec 003's data model and `vas.rs`'s field comments, not from its code (the harness keeps its own copy, as `smoke_reference` does for the one-pass).
