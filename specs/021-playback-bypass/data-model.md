# Data Model: Live Playback-Stage Bypass (spec 021)

## Entities

### Fade (per stage; `stages/output.rs` for 11 – 13, `paths.rs::PlaybackHalf` for 10)

| Field | Type | Meaning | Rules |
|---|---|---|---|
| `k` | u32 | The phase index, 0 … N. 0 = at rest on, N = at rest bypassed. | Landing sample computes k = 1 (or N − 1 for a return from rest). |
| `dir` | i8 | −1 toward on, 0 at rest, +1 toward bypassed. | A landing from rest sets it; a landing during a fade flips it (R-02); clamped at 0 and N. |
| `n` | u32 | N = 0.02 × rate (host) or 120 (device). | Fixed at construction. |
| `inv_n` | f64 | 1 / N. | Fixed at construction. |

The stage's contribution at a sample: at rest on → y (selected); at rest bypassed → x (selected); fading → y + w·(x − y), w = u·u·(3 − 2u), u = k · inv_n (R-01). "y" is the stage's processed output (the stage runs every sample while configured on), "x" its input.

### OutputChain additions

| Field | Type | Meaning |
|---|---|---|
| `fading` | bool | Any of the three fades has `dir != 0` (the hot-path gate, R-08). |
| `fade_amp`, `fade_speaker`, `fade_earphone` | Fade | Stages 11, 12, 13. |

`reset_sections` (a route switch) keeps the fades; `reset` (the engine's) sets each fade to its rest state from `settings()` (R-11: the landed values persist). The clip count is incremented only when `fade_amp.k < n` (D7, per sample at the count).

### PlaybackHalf additions

| Field | Type | Meaning |
|---|---|---|
| `fade` | Fade | Stage 10's (N = 120). |

### StreamState additions (`changes.rs`)

| Field | Type | Meaning |
|---|---|---|
| `last_bypass` | [bool; 3] | The last applied value per stage 11 – 13 (true = bypassed). |
| `landed_bypass` | [Option<bool>; 3] | The call's landings, read by the pipeline for the mirror (R-05). |
| `xpend` | [Option<(bool, u64)>; 3] | The cross-stream pending value and tag per stage; `next_apply` armed at the stream's next sample while any is set. |
| `last_stage10` | bool | The player's stage-10 value (no mirror). |

`Apply` gains `Bypass { stage: u8, on: bool }` (stage 10 … 13; `on` = false means bypassed... **convention**: `Apply::Bypass(stage, bypassed)` with `bypassed: bool` to avoid the double negative).

### Settings reflection

The engine's `settings` copy is written at each landing (`playback_stage_enabled`, `output.volume_amp_enabled`, `output.speaker_enabled`, `output.earphone_enabled` = !bypassed), as spec 017 does for the record-side values; `reset` re-applies them to the fades' rest states; `reconfigure(s)` takes `s`.

### Error

`Error::StageNotConfigured { control: u32 }` → C `RR_DR60_STATUS_STAGE_NOT_CONFIGURED = 36`. Text: "control {n}: its stage does not run on this stream (configured off at construction, or not reached by the one-pass's tap); construct with the stage on and bypass it live".

### Constants

`MAX_CHANGES_PER_CALL` = 142 (2·64 + 3 + 8 + 3). `output::SPEAKER_SCALE_DB` (new, public): 20·log10(CODEC_FULL_SCALE_V / V_CLIP_V), so an app computes stage 11's bypass level change on the speaker route as −(speaker_gain_db(p) − SPEAKER_SCALE_DB) and on the earphone route as −earphone_gain_db(p).

## State transitions (one stage)

```
rest on (k = 0) --landing bypassed--> fading +1 --k = N--> rest bypassed (k = N)
rest bypassed   --landing on-------> fading −1 --k = 0--> rest on
fading ±1       --landing opposite-> fading ∓1 (k kept)      (reverse from phase)
fading ±1       --landing same-----> no change, logged as a no-op entry
any             --route switch-----> unchanged (sections reset, fade kept)
any             --reset------------> rest at settings()'s value
```

## Control manifest

Entries 5, 22, 23, 24: `timing: Live`; everything else unchanged. The canonical text re-blessed once.

## Golden (`golden-bypass-v1.json`)

Entry: `{ config, route, stream, rate, stage(s), schedule: [{position, stage, value, tag}], produced, changes: [...], digest, reversal: bool }`; 140 entries (R-09).
