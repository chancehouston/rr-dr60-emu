# Data Model: Playback Speed (spec 013)

## The control (Pipeline)

| Field | Type | Default | Rules |
|---|---|---|---|
| `speed_level` | `u8` | 3 (`speed::DEFAULT_LEVEL`) | 1 … 5; set by `set_playback_speed` (refused otherwise, unchanged); read by `playback_speed`; kept by `reset` and `reconfigure`; copied into the stage as the requested level at the start of every `play()` call (FR-001, FR-007) |
| `speed` | `SpeedStage` | built at `new` / `reconfigure` | the player's speed control; reset at every `start_playback` |

## SpeedStage (stages/speed.rs)

| Field | Type | Meaning |
|---|---|---|
| `banks` | `[Box<[f64]>; 4]` | levels 1, 2, 4, 5: 5 phases × 57 taps, phase ph at `[ph·57 .. ph·57 + 57]`, each phase summing to 1 (research R-03) |
| `ring` | `Box<[f32]>` (128) | the stored samples pulled, by index mod 128 |
| `pulled` | `u64` | stored samples pulled from the source so far |
| `end` | `Option<u64>` | K, once the source returned `None` |
| `p5` | `u64` | the place of the next output, in fifths of a stored sample |
| `active` | `u8` (k = 3 … 7) | the level computing the outputs |
| `requested` | `u8` | the host's level as of the current `play()` call |
| `latched` | `bool` | the place reached K: no level change applies |
| `last_p5` | `Option<u64>` | the place of the last output computed (the report's place) |
| `own` | `u64` | `Own` outputs so far (K′) |
| `hooks` | `SpeedMutation` (test only) | R-12's mutants |

### State machine (per output)

```text
before computing an output:
  if !latched and requested != active:
      if requested == 3 (k = 5):  apply when p5 % 5 == 0 (deferred; ≤ 4 outputs)
      else:                       apply now (a pending switch into 3 is dropped)
compute at place p5 = 5i + ph:
  k == 5:   y = f64::from(x[i])                       (pull x[i] only)
  else:     y = Σ_{m=0}^{56} bank[k][ph][m] · x[i + 28 − m]   (pull through x[i + 28])
  x[j] = 0 for j < 0 or j ≥ K
classify:
  p5 < 5K:                         Own(y),  own += 1
  else if the kernel reaches K − 1: Tail(y)   (level ≠ 3 only; latched = true from the first)
  else:                            Silence  (+0.0)
then: last_p5 = p5; p5 += active (when Own or Tail)
```

"The kernel reaches K − 1": the lowest stored index read, i − 28 for ph = 0 and i − 27 for ph > 0, is at most K − 1. At level 3 the first output at place K is `Silence`. `latched` is set when the place first reaches 5K (any level).

## Transport changes (transport.rs)

| Item | 0.12 | 0.13 |
|---|---|---|
| `Cursor::total` | ⌈K · m ÷ l⌉ + drain at `start_playback` | `u64::MAX` at `start_playback`; ⌈K′ · m ÷ l⌉ + drain when the stage first returns a non-`Own` output (R-05) |
| device sample pushed | `memory` / decoder sample, or 0.0 past the end | the stage's `Own` / `Tail` value, or +0.0 (`Silence`) |
| `PlayInfo::frame_index` | the last decoded frame | min(⌊place ÷ 120⌋, F − 1) with the codec on; 0 bypassed (equal at level 3) |
| `PlayInfo::place` | — | min(⌊last_p5 ÷ 5⌋, K); 0 before any output |
| `start_playback` | resets the player and the decoder | also resets the stage (place 0, the ring cleared, `active` = the host's level, not latched) |
| `playback_length_samples(number, level)` | — | ⌈⌈5K ÷ k⌉ · m ÷ l⌉ + drain |

## Configurations (harness, configs.rs)

| Name | Settings | Use |
|---|---|---|
| `speed_only` | 0.13 defaults with the speech codec, stage 10 and the output chain bypassed (`Tap::RouteOutput`), stages 1 – 5 bypassed for recorded stimuli; the samples memory | FR-004 at 6 kHz (the playback output is the device stream); the impulse; the tail; the continuity oracle |
| `speed_default` | 0.13's default (= 0.12's) | US1 AS5, the golden's default entries, the timing |
| `speed_default_no_codec` | the default with the codec bypassed | the golden, `speed_cost` |

## Stimuli

| Name | Form | Notes |
|---|---|---|
| `stored_tone(f, n)` | imported device samples (the samples form), amplitude 0.5 | FR-004 |
| `stored_impulse(j, n)` | imported, 1.0 at j | FR-008 |
| `stored_end_loud(n)` | imported tone ending on a full-scale sample | the tail |
| `voiced_m20`, `tone_1k_m20` | host-rate stimuli, recorded | the golden, US1 AS5 |

## Report rows (speed_matrix)

Per rate × level: `own_length` (host samples), `expected` ⌈⌈5K ÷ k⌉ · m ÷ l⌉, `fit_db` (at r × f), `residual_db`, `spurious_db` (worst over the tone set), `impulse_peak`, `tail` (device samples), the level-change step (the largest sample difference at a change against both constant runs, reported).
