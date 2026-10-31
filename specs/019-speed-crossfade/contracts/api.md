# Contracts: Speed-Change Crossfade (019)

## Rust (`rr_dr60`)

```rust
pub mod speed {
    /// The crossfade's length in device samples at a level change (spec 019 FR-002; an engine convention).
    pub const CROSSFADE_SAMPLES: u32 = 120;
    /// The same in milliseconds (20), beside `output::VOLUME_TRANSITION_MS`.
    pub const CROSSFADE_MS: u32 = 20;
}
```

No other public change: `set_playback_speed`, `playback_speed`, `schedule_change` (the speed on the playback stream), `play_with_changes`, `playback_length_samples`, `PlayInfo` and the change entries keep their signatures and meanings (the entries and the reading carry target levels, chained, as spec 015 logs them at the request).

### Behavior

- A level change lands as today (the plain setter's first computed device sample; `changes::speed_landing(P, rate)` for a scheduled one; a change into level 3 at the next whole place) and produces 119 blend samples y_old + w_n·(y_new − y_old) and a 120th that is the new level's value itself, then the new level's constant-level computation.
- A request during a blend waits for the blend's end; the latest wins; every request is logged at its position.
- The recording's length (`playback_length_samples`), the latch, the tail and the drain are unchanged for a constant level and for the new branch; a schedule whose requests are under 20 ms apart plays longer than in 0.18 by the waits (at most 120 device samples per waited request).

## C (`include/rr_dr60.h`)

One define added, pinned to the core constant (product M1; the precedent `RR_DR60_VOLUME_TRANSITION_MS`):

```c
#define RR_DR60_SPEED_CROSSFADE_MS 20   /* spec 019: a level change blends over this many milliseconds (120 device samples); an engine convention */
```

No function or struct changes. The smoke's speed case re-pins `RR_DR60_SMOKE_SPEED_HASH` (the sample count `RR_DR60_SMOKE_SPEED_SAMPLES` is unchanged: one change, no wait).

## Control manifest and stage manifest

The control manifest is unchanged (canonical text byte-identical). Stage 9's `controls` and `coverage` notes name the blend at model version 0.19.0; the stage manifest's canonical text, the README table and the guide blocks are re-blessed once.

## Test hooks (`__test_hooks`, feature-gated)

`SpeedMutation { fade_step, fade_linear, fade_len_delta, fade_no_wait, … }`; `crossfade_weights() -> Vec<f64>` (the engine's table, for the reference's check); `speed_device_outputs(pipeline) -> Vec<f64>` (the speed stage's device outputs of the last play call, for the blend identity at the other host rates).

## Harness

`analysis::click_energy_in(y, rate, centre, window_s, exclusions_hz)`; `speed_checks::reference_output_at` with the wait and the fade; `crossfade_checks::{blend_identity, splice, click_table, voiced_click, wait_cases, end_cut, partitions, lengths_unchanged}`.
