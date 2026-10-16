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
| `g_lo`, `g_hi` | `f64` | the active route's scalar range for the transition (R-03) |
| `g_to_spk`, `g_to_ear` | `f64` | the target's block scalars, computed as 0.10 does (`k_spk(p)·gain_scale`, `k_ear(p)`), written at sample N |
| `fresh` | `bool` | true after `new()` and `reset()` until the first processed sample (R-04) |
| `volume_step`, `shape_linear` | `bool` | test hooks only (R-07); false in production |

The existing `k_spk` and `k_ear` stay the per-sample scalars the multiply reads.

## State transitions

```text
              new() / reset() / route change
                         │
                         ▼
   ┌──────────────── Fresh ────────────────┐
   │ set_controls(p): scalars = law(p),    │
   │ knob = to = p   (at once, 0.10)       │
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

`set_controls` with `!active || !run_amp || volume_step` takes the position at once from any state (the scalars have no effect, or the hook restores 0.10's step).

## The per-sample transition step

For transition sample k (1 … N) on the active route, with `law` = `k_spk(·)·gain_scale` (speaker) or `k_ear(·)` (earphone):

```text
u    = k · inv_n                      (non-decreasing in k)
s    = u · u · (3 − 2u)               (smoothstep; `u` with the linear-shape hook)
knob = from + (to − from) · s
g    = law(knob), held within [g_lo, g_hi]
k = N: g = g_to (exactly 0.10's scalar), knob = to
```

## Configurations and stimuli (harness)

| Name | Settings | Route | Read at |
|---|---|---|---|
| `stage11_only_speaker` | `bypass_all` + `output.volume_amp_enabled = true` | speaker | `Tap::AfterVolumeAmp` |
| `stage11_only_earphone` | the same | earphone | `Tap::AfterVolumeAmp` |
| `output_only_speaker`, `output_only_earphone` | spec 009's | each | `Tap::RouteOutput` (US1 AS7, reported) |
| `default_output`, `default_earphone` | spec 009's | each | `Tap::RouteOutput` (US2, golden) |

| Stimulus | Content |
|---|---|
| `tone_1k_ramp` | a 1 kHz cosine at the route's level (earphone 0 dBFS, speaker −30 dBFS), its peak placed on the change's output sample through the path latency (R-05); 0.5 s settled before the window |
| `drag_0p1_to_1` | ten positions 0.1 + 0.09 j at round(rate ÷ 60)-sample intervals, block-aligned |
| the earlier stimuli | unchanged |

## Reported rows (the harness)

`volume_ramp_checks.rs` returns `MeasurementResult` rows (the established form): FR-001 / SC-001 (the splice identity, the gain sequence via the core test's summary, the ratio reader), FR-002 (N per rate, partition identity), FR-003 / SC-003 (range, retarget increments), FR-004 / SC-002 (the click reduction ≥ 30 dB and within ±1 dB of R-02's table), FR-005 (start-at-once cases, drain completion), FR-007 (one-pass vs record-then-play from the change), FR-008 (exact zeros from N, decay), US1 AS7 (route output, reported), US1 AS8 (drag).
