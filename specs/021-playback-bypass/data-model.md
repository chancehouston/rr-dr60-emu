# Data Model: Live Playback-Stage Bypass (spec 021)

## Entities

### Fade (per stage; `stages/fade.rs`, used by `stages/output.rs` for 11 – 13 and `paths.rs::PlaybackHalf` for 10)

| Field | Type | Meaning | Rules |
|---|---|---|---|
| `k` | u32 | The phase index, 0 … N. 0 = at rest on, N = at rest bypassed. | The landing sample computes k = 1 (a return from rest N − 1); the step clamps to [0, N] first, then tests the ends (R-02). |
| `dir` | i8 | −1 toward on, 0 at rest, +1 toward bypassed. | A landing from rest sets it; a landing during a fade flips it (k kept). |
| `n` | u32 | N = `volume_transition_samples(rate)` (host) or `speed::CROSSFADE_SAMPLES` (device). | Fixed at construction. |
| `inv_n` | f64 | 1 / N. | Fixed at construction; (N/2)·inv_n = 0.5 exactly at the seven rates (R-01). |

The stage's contribution at a sample: k = 0 → y (selected); k = N → x (selected); otherwise y + w·(x − y), w = u·u·(3 − 2u), u = k · inv_n. "y" is the stage's processed output (the stage runs every sample while configured on), "x" its input. `land(bypassed, at_once)`: `at_once` snaps (k = N or 0, dir = 0); otherwise sets or flips `dir`.

### OutputChain additions (`#[repr(C)]`, the hot fields first)

| Field | Type | Meaning |
|---|---|---|
| `bypass_state` | u8 | Bit 0: any fade running; bits 1 – 3: stages 11, 12, 13 at rest bypassed. 0 = 0.20's path (R-08). |
| `fade_amp`, `fade_speaker`, `fade_earphone` | Fade | Stages 11, 12, 13. |

`reaches(stage) -> bool`: the construction switch on and the tap reaching the stage (`RouteOutput`: 11, 12, 13; `AfterVolumeAmp`: 11; `AfterSpeaker`: 11, 12; `AfterEarphone`: 11, 13; the taps before the chain: none) (R-04). `reset_sections` (a route switch) keeps the fades; the engine's `reset` snaps each fade to `settings()`'s value (R-11).

### PlaybackHalf additions

| Field | Type | Meaning |
|---|---|---|
| `fade` | Fade | Stage 10's (N = 120). `land(bypassed, at_once)` from the player's pre-pull apply; `runs()` is the reach predicate. |

### StreamState additions (`changes.rs`)

| Field | Type | Meaning |
|---|---|---|
| `last_bypass` | [bool; 3] | The last applied value per stage 11 – 13 (true = bypassed); the one-pass compares `settings` to it at `begin_call` and lands the differences first (R-05). |
| `landed_bypass` | [Option<(bool, u64)>; 3] | The call's landings with their tags, read by the pipeline into `settings` and `bypass_tags`. |
| `mirror` | [Option<(bool, u64)>; 3] | The one-pass's pending mirror landings for the call (set by `begin_mirror`). |
| `last_stage10` | bool | The player's stage-10 value (no mirror). |

`Apply` gains `Bypass { stage: u8, bypassed: bool, at_once: bool }` (stage 10 … 13; `at_once` = the stream's first produced sample).

### Pipeline additions

`bypass_tags: [u64; 3]` (the tags a mirrored landing carries); the landed switches written into `settings` after every call (`playback_stage_enabled`, `output.volume_amp_enabled`, `output.speaker_enabled`, `output.earphone_enabled` = !bypassed).

### Error

`Error::StageNotConfigured { control: u32 }` → C `RR_DR60_STATUS_STAGE_NOT_CONFIGURED = 36`. `Display`: "control {n} ({identifier}, stage {k}) does not run on this stream: its setting was off at construction or the one-pass's tap stops before it; build the engine with the stage on (and a tap at or after it) and switch it with schedule_change instead".

### Public function

`output::stage11_bypass_level_db(route: Route, position: f32) -> f32` / C `rr_dr60_stage11_bypass_level_db(route, position)`: the level change when stage 11 is bypassed, dB, positive = louder; speaker −(speaker_gain_db(p) + SCALE), SCALE ≈ −1.214 dB private; earphone −earphone_gain_db(p); +∞ at position 0; stated for levels below the knee (R-07).

### Constants

`MAX_CHANGES_PER_CALL` = 142 (2·64 + 3 + 8 + 3; the one-pass attains it, the player at most 68).

## State transitions (one stage)

```
rest on (k = 0)    --landing bypassed----------> fading +1 --k = N--> rest bypassed (k = N)
rest bypassed      --landing on----------------> fading −1 --k = 0--> rest on
fading ±1          --landing opposite----------> fading ∓1 (k kept; reverse from phase)
fading ±1          --landing same (stream's last value)--> unchanged; a no-op entry
rest, stream's first produced sample --landing--> rest at the value at once (no blend; the trigger is produced == 0, never the chain's fresh flag)
any                --route switch--------------> unchanged (the sections reset, the fade kept)
any                --start_playback------------> rest at settings()'s values (the adopt; no entry, no blend)
any                --reset---------------------> rest at settings()'s values (all four fades, the one-pass's stage 10 included)
any                --reconfigure(s)------------> rebuilt: a switch on in s → rest on (k = 0); off → the stage does not run (not a fade state; its changes refused)
```

## Control manifest

Entries 5, 22, 23, 24: `timing: Live`; everything else unchanged. The canonical text re-blessed once.

## Golden (`golden-bypass-v1.json`)

Entry: `{ stages, schedule, stream, route, log, produced, exact, reversal }` beside the output digest; 168 entries (eleven cells × two scenarios + two whole-path entries, × 7 rates); `exact` marks the entries whose endpoint identities are bit-exact (R-10), the others carrying the convergence bound (R-09).
