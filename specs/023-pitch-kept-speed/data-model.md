# Data Model: Pitch-Kept Playback Speed (spec 023)

## SpeedMode (`rr_dr60::speed::SpeedMode`)

`PitchWithSpeed = 0` (the default; mechanism (b)) | `PitchKept = 1`. A live control on the playback stream (index 30), kept by `reset`/`reconfigure`; a C `uint32_t` (`RR_DR60_SPEED_MODE_*`); `Error::InvalidSpeedMode { mode }` (C 38) for another value.

## TimeScaler (inside `SpeedStage`, fixed size)

| Field | Meaning |
|---|---|
| `origin: u64` | the device output index the frame grid is anchored at (leaving level 3, a mode change) |
| `n: u32` | the output's index inside the current hop (0 … 59) |
| `cur_a: i64`, `cur_s: i32` | the current frame's nominal analysis position (the place at the hop's first output, a whole stored sample) and its shift (−40 … 40) |
| `prev_a: i64`, `prev_s: i32` | the previous frame's |

Constants: N = 120, Ss = 60, Δ = 40 (< Ss: the tail proof's condition), Lc = 60, the window `w` (120, `speed_window.rs`: w[0 … 59] a periodic Hann, w[60 + n] = 1 − w[n] exactly); two contiguous f64 search buffers (140 + 60) reserved at construction. Output: `y = x[prev_a + prev_s + Ss + n]·w[Ss + n] + x[cur_a + cur_s + n]·w[n]`. At n = 60: `prev ← cur`, `cur_a ← ⌊place⌋`, `cur_s ← argmax_s c(s)` (outward order, strict). When `prev_a + prev_s + 60 == cur_a + cur_s` the output is `x[cur_a + cur_s + n]` itself (bit-exact continuation). Anchor: `prev_a = a₀ − 60`, `prev_s = cur_s = 0`. Pulls lazy: the search through `cur_a + 99`, the synthesis `max(j_prev, j_cur)`; the worst ring span 224 ≤ 256.

## SpeedStage additions

`mode`, `requested_mode`, `scaler: TimeScaler`; `Fade` gains `old_mode` and `old_scaler` (the old branch frozen, continuing its own hops). `next_general` computes `y_new` by `(mode, active)`: level 3 → `x[i]`; pitch kept → the scaler; else the bank. Fades: a mode change (either direction, not at level 3), a level change in the default mode (019), **a change into level 3 in pitch-kept mode (D12)**; a change between levels 1, 2, 4, 5 in pitch-kept mode has none. The wait rule: both `apply_request` and `apply_mode_request` return while a fade runs.

## The length

Own = ⌈5K ÷ k⌉ (013). Pitch-kept tail `T_c = 120 − ((own − origin) mod 60)` when the remainder is non-zero, else 60 (the frame's tail fading through the next hop); in [60, 119] (a grid bound, provable because Δ < Ss). `playback_length_samples_in_mode(number, level, mode)` = ⌈(own + T)·m ÷ l⌉ + drain, T the mode's tail at origin 0. The timeline walker: `place_at_in_mode` beside the unchanged `place_at`.

## The state machine (a playback in pitch-kept mode)

```
start_playback ─► level 3: the identity (no grid)                 ─► leave 3 at d(P): anchor the grid, no wait
hop boundary: prev ← cur; cur_a = place; cur_s = search          ─► a level change: the place's ratio from d(P); the next hop reads it
mode change at d(P): Fade{old branch frozen} + the new branch     ─► 120 outputs blend; requests wait; past the latch dropped + logged
into level 3 at d(P): wait for a whole place, then Fade{frozen scaler → x[i]} (D12)
latch (place ≥ K): final_count = own + tail + T_c(remaining)      ─► Tail outputs, then Silence, then the drain
```

## The control manifest entry 30

`playback_speed_mode` / "Playback speed mode" / ModelInternal / Live / playback / stage 9 / Choice {pitch_with_speed 0, pitch_kept 1; default 0} / "" / S-001, S-012 / A-054 / "Chooses whether slow and fast playback moves the pitch with the speed (the modeled default) or keeps it by time-scaling the decoded stream."
