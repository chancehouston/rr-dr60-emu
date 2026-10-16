# Data Model: Volume Ramp (feature 011)

## The volume control (unchanged surface)

| Item | Meaning | Change |
|---|---|---|
| `Pipeline::volume` (`f32`, 0 … 1) | the **volume position**: the host's last valid setting (spec 009 FR-003); what `volume()` / `rr_dr60_get_volume` return | none |
| `Pipeline::set_volume` / `rr_dr60_set_volume` | validate and store; refused positions leave it unchanged | rustdoc and the C comment describe the transition |
| `output::VOLUME_TRANSITION_MS: u32 = 20` | the transition's length (A-045) | **new** public constant; `RR_DR60_VOLUME_TRANSITION_MS` in the header |
| `output::volume_transition_samples(host_rate_hz) -> Option<u32>` | N for a supported rate (120, 160, 320, 882, 960, 1764, 1920); `None` otherwise | **new** (Rust only) |

## `OutputChain` state (new fields)

| Field | Type | Meaning |
|---|---|---|
| `knob` | `f64` | the **knob position** the active route's scalar was last computed from; equals `to` outside a transition |
| `from`, `to` | `f64` | the transition's start (`knob` when it began) and target (the new volume position, widened from `f32`) |
| `target` | `f32` | the last position `set_controls` received (the compare that decides "a change") |
| `ramp_k`, `ramp_n` | `u32` | the transition sample counter (0 … N) and N for this rate (`ramp_k == ramp_n` outside a transition) |
| `inv_n` | `f64` | 1 / N, computed once at construction |
| `up` | `bool` | the transition's direction (the active route's target scalar above its start), which picks the running hold's side (R-03) |
| `g_to_spk`, `g_to_ear` | `f64` | the target's block scalars, computed as 0.10 does (`k_spk(p)·gain_scale`, `k_ear(p)`), written at sample N |
| `fresh` | `bool` | true after `new()` and `reset()` until the first processed sample (R-04) |
| `volume_step`, `volume_shape_linear` | `bool` | test hooks only (R-07); false in production |
| `volume_ramp_delta` | `i64` | test hook only: added to N (the length mutants); 0 in production |

The existing `k_spk` and `k_ear` stay the per-sample scalars the multiply reads.

## State transitions

```text
              new() / reset() / route change
                         │
                         ▼
   ┌──────────────── Fresh ────────────────┐
   │ set_controls(p): take_at_once(p):     │
   │ scalars = law(p), knob = to = p, k = N│
   └──────────── first processed sample ───┘
                         │
                         ▼
                      Steady  ◄──────────────────────────────┐
                         │ set_controls(p), p ≠ target,      │
                         │ active && run_amp && !volume_step │
                         ▼                                   │
            Ramping(k = 0, from = knob, to = p)              │
                         │ each processed sample: k += 1     │
                         │ k < N: scalar = hold(law(knob(k)))│
                         │ k = N: scalars = g_to, knob = to ─┘
                         │
                         │ set_controls(p'), p' ≠ target: Ramping(k = 0, from = knob, to = p')
                         │ set_controls(target): continues
                         │ route change / reset / reconfigure / new playback: → Fresh
```

`set_controls` with `!active || !run_amp || volume_step` takes the position at once from any state through `take_at_once` (the scalars have no effect, or the hook restores 0.10's step); `__mutate` also goes through `take_at_once`; `reset()` sets `fresh` and `ramp_k = ramp_n`.

## The per-sample transition step

For transition sample k (1 … N) on the active route, with `law` = `k_spk(·)·gain_scale` (speaker) or `k_ear(·)` (earphone):

```text
u    = k · inv_n                      (non-decreasing in k)
s    = u · u · (3 − 2u)               (smoothstep; `u` with the linear-shape hook)
knob = from + (to − from) · s
g    = law(knob), held within [g_prev, g_to] (upward) or [g_to, g_prev] (downward): the range and running hold (R-03)
k = N: g = g_to (exactly 0.10's scalar), knob = to
```

## Configurations and stimuli (harness)

| Name | Settings | Route | Read at |
|---|---|---|---|
| `stage11_only_speaker` | as built: `output_only_amp` (every earlier stage, the codec and the ADC clamp off; stage 11 on) | speaker | `Tap::AfterVolumeAmp` |
| `stage11_only_earphone` | the same | earphone | `Tap::AfterVolumeAmp` |
| `output_only_speaker`, `output_only_earphone` | spec 009's | each | `Tap::RouteOutput` (US1 AS7, reported) |
| `default_output`, `default_earphone` | spec 009's | each | `Tap::RouteOutput` (US2, golden) |

| Stimulus | Content |
|---|---|
| `tone_1k_ramp` | a 1 kHz cosine at the route's level (earphone 0 dBFS, speaker −30 dBFS), its peak placed on the change's output sample through the path latency (R-05); 0.5 s settled before the window |
| `drag_0p1_to_1` | ten positions 0.1 + 0.09 j at G = 16 · round(rate ÷ 960)-sample intervals (16.0 – 17.0 ms), 16-sample blocks, the same as the golden's `ramp_drag` |
| the earlier stimuli | unchanged |

## Reported rows (the harness)

`volume_ramp_checks.rs` returns `MeasurementResult` rows (the established form): FR-001 / SC-001 (the splice identity, the first differing sample 1 and the last differing sample N − 1, the gain sequence via the core test's summary, the ratio reader), US1 AS4 (the ceiling on a full-scale tone), FR-002 (N per rate, partition identity), FR-003 / SC-003 (range, retarget increments), FR-004 / SC-002 (the click reduction ≥ 30 dB and within ±0.25 dB of R-02's table), FR-005 (start-at-once cases, drain completion), FR-007 (one-pass vs record-then-play from the change), FR-008 (exact zeros from N, decay), US1 AS7 (route output, reported), US1 AS8 (drag).
