# Data Model: Voice Activated System (VAS)

**Feature**: [spec.md](spec.md) | **Research**: [research.md](research.md)

This extends the data models of [spec 001](../001-pipeline-skeleton/data-model.md) and [spec 002](../002-agc/data-model.md). Only new or changed entities are listed.

## VasSettings (new)

| Field | Type | Default | Valid range (inclusive) | Source |
|---|---|---|---|---|
| `enabled` | bool | `true` | — | S-001, A-008 |
| `mode` | `VasMode` | `Drop` | `Drop`, `Mute` | S-001, A-008 (drop); mute is an emulator option |
| `sensitivity` | u8 | 3 | 1 … 5 | S-001, A-021 |
| `threshold_dbfs` | f32 | −18.0 | −60.0 … 0.0 | A-022 |
| `hang_ms` | f32 | 1000.0 | 50.0 … 10000.0 | A-023 |
| `onset_ms` | f32 | 20.0 | 0.0 … 200.0 | A-024 |

- **Validation**: in `Pipeline::new` and `reconfigure`, after the AGC fields, in struct order. NaN and ±Inf are out of range. The ranges are engineering targets (spec FR-017). Validation happens even when VAS is bypassed.
- **Derived at construction** (R-02, R-04):
  - effective threshold T<sub>s</sub> = `threshold_dbfs` + 3 dB · (3 − `sensitivity`) (A-022);
  - A<sub>thr</sub> = 10^(T<sub>s</sub>/20), computed with detmath;
  - H = round(`hang_ms` · 8) and O = round(`onset_ms` · 8) device samples.
- **Combinations**: any combination of valid values is allowed, including an onset time longer than the hang time.

## VasMode (new)

`Drop` (default): paused audio is removed. `Mute`: paused audio becomes +0.0, so the output keeps the input's length (spec clarification Q2).

## Settings (changed)

- New field `vas: VasSettings`. `Settings::new(rate)` sets `vas = VasSettings::DEVICE`.

## Tap (changed)

| Variant | Runs (each if enabled) | Output length |
|---|---|---|
| `AfterAgc` | boundary, AGC | fixed |
| `AfterRecord` | boundary, AGC, stage 4 | fixed |
| `AfterVas` (new) | boundary, AGC, stage 4, VAS | variable in drop mode |
| `AfterPlayback` (default) | boundary, AGC, stage 4, VAS, stage 10 | variable in drop mode |

## Setting (changed)

New variants, each with a `Display` name and range: `VasSensitivity` (`vas.sensitivity`, 1…5), `VasThresholdDbfs` (`vas.threshold_dbfs`, −60…0 dBFS), `VasHangMs` (`vas.hang_ms`, 50…10000 ms) and `VasOnsetMs` (`vas.onset_ms`, 0…200 ms). `enabled` and `mode` can't be invalid in Rust. In C, an invalid `vas_mode` value is reported as `INVALID_ARGUMENT` with field `VAS_MODE`, as an invalid tap is.

## BlockInfo (new)

Returned by every processing call.

| Field | Type | Meaning |
|---|---|---|
| `produced` | usize | Output samples written to the start of the output buffer. ≤ input length. Equal to it when VAS doesn't run, is bypassed, or is in mute mode. |
| `events` | usize | VAS events this block generated (splices in drop mode, muted regions in mute mode). |
| `paused` | bool | VAS is paused at the end of the block. |

## VasEvent (new)

| Field | Type | Drop mode | Mute mode |
|---|---|---|---|
| `output_position` | u64 | Stream output index of the first sample after the splice | Stream output index of the region's first muted sample |
| `input_length` | u64 | Host input samples removed at this splice (cumulative-floor rule, R-06) | Region length (output samples = input samples) |

Positions count from the start of the stream (creation, reset or reconfigure), so they don't depend on block partition. An event is reported by the block whose output contains its position, which is the block in which recording resumed (R-05, R-06).

## VasStage state (internal)

| Field | Meaning |
|---|---|
| `state` | `Recording` or `Paused` |
| `silent` | Consecutive non-sound samples while recording |
| `run` | Length of the current sound run while paused (0 = none) |
| `since_sound` | Samples since the last sound sample while paused (for the W bridge) |
| `dropped` | Cumulative dropped (or muted) device samples, for removed lengths |
| `pending` | Whether a resume happened that the interpolator has not yet placed in the output |

All of it is fixed-size and inline. `reset()` returns to `Recording` with every counter at 0.

### Per-sample processing (device rate, R-02, R-04)

```text
sound = |x| ≥ A_thr
Recording:
  if sound: silent = 0 else: silent += 1
  if silent ≤ H: KEEP x
  else: state = Paused; run = 0; since_sound = W + 1; DROP/MUTE
Paused:
  if sound:
    if run > 0 and since_sound ≤ W: run += 1 else: run = 1
    since_sound = 0
  else:
    since_sound += 1
    if since_sound > W: run = 0
    elif run > 0: run += 1
  if run ≥ O + 1: state = Recording; silent = 0; KEEP x  (first sample after the splice)
  else: DROP/MUTE
```

## Emission schedule state (internal, R-05)

The interpolator gains two counters: `kept` (K, device samples fed to it) and `emitted` (n, host samples output). After each host input step it emits one sample when K ≥ ⌊n·l/m⌋ + 1. At the 8 kHz identity rate, each kept sample is emitted immediately.

## Golden entry (changed)

VAS golden entries add an optional `vas` object: `{ "events": [[output_position, input_length], …], "paused_at_end": bool }`. Older files don't have it ([contracts/golden-format.md](contracts/golden-format.md)).
