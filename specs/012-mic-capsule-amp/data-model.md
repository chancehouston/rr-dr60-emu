# Data Model: Microphone Capsule and Amplifier (stages 1–2)

**Spec**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md)

## Settings (Rust `Settings::mic`, C fields at 112–119)

| Field | Type | Default (`MicSettings::DEVICE`) | Bypassed (`MicSettings::BYPASSED`) | Validation |
|---|---|---|---|---|
| `capsule_enabled` | bool | true | false | none |
| `amp_enabled` | bool | true | false | none |
| `hiss_offset_db` | f32 | 0.0 | 0.0 | finite, −18.0 ≤ x ≤ +12.0, else `Error::InvalidSetting` (C: invalid-setting status, field `mic_hiss_offset_db`) |

`Settings::new(rate)` uses `MicSettings::DEVICE`. The offset applies only when `capsule_enabled` is on. A setting is applied by `reconfigure` (not a live control).

## Stage objects (core, `stages/mic.rs`)

### `MicCapsule` (stage 1)

| Field | Meaning |
|---|---|
| `state: [u64; 4]` | xoshiro256++ state, keyed by `key(seed, counter)` |
| `seed: u64`, `counter: u64` | the key, kept for `rekey` and `reset` |
| `gain: f64` | σ_rate · 10^(offset/20), computed at construction with `detmath::exp` (σ_rate from `mic_coeffs`) |

**Behaviour.**
- `next() -> f64` draws one ziggurat sample. The fast path is integer and table work; the tail and wedge paths use `detmath::{exp, ln}`.
- `process(x) = x + gain · next()`.
- `rekey(counter)` rebuilds the state and keeps nothing else.

### `MicAmp` (stage 2)

| Field | Meaning |
|---|---|
| `hp_in: FirstOrder` | the input high-pass (A-046), bilinear, prewarped |
| `g_n: f64` | the normalization to 0 dB at 1 kHz |
| `inv_lp, inv_ln, lp, ln: f64` | the clipper's limits L₊, L₋ and their reciprocals (A-051) |
| `k: f64` | the knee start (A-051), via 009's `knee` |
| `hp_out: FirstOrder` | the output coupling (A-047), bilinear, prewarped |
| `lp_13k: FirstOrder` | the roll-off (A-047), magnitude-matched |

**Behaviour.** `process(x) = lp_13k(hp_out(clip(g_n · hp_in(x))))`, where `clip` is branchless per polarity. `reset` puts the three states at 0.

### `MicFront` (in `RecordPath`)

`{ capsule: Option<MicCapsule>, amp: Option<MicAmp>, limiter: Limiter, run_limiter }`. `None` is decided at construction from the switches.

`front(x) = limiter(amp(capsule(sanitize_in(x))))`, each step skipped when off. `front_only(upto)` stops after stage 1 or stage 2 for the taps.

## Stream keying

`key(seed, counter)`:
1. `z = seed ^ MIC_STREAM`, then `z ^= counter.wrapping_mul(GOLDEN_GAMMA)`.
2. Four SplitMix64 outputs from `z` fill the state.

`MIC_STREAM` and the SplitMix64 constants are fixed and registered in A-049.

| Event | One-pass record path | Split engine (`Transport`) |
|---|---|---|
| construction | key(seed, 0) | counter = 0 |
| `reset` | rekey(0), sections at rest | counter = 0 |
| `reconfigure` | rebuilt: key(seed, 0) | rebuilt: counter = 0 |
| `start_recording` | n/a | the recorder's path reset, rekey(counter), then counter += 1 |
| `stop_recording` flush | n/a | zeros through the front (the hiss continues) |
| zero-length call | nothing advances | nothing advances |

So recording k (from 0) after a construction, reset or reconfigure carries stream k, and recording 0 equals the one-pass.

## Taps

| Tap | C value | Signal | Latency |
|---|---|---|---|
| `AfterMicCapsule` | 9 | input + hiss (unshaped, at the host rate) | 0 |
| `AfterMicAmp` | 10 | after stage 2 | 0 |
| `AfterAgc` (existing, 2) | 2 | after stages 1–2 and the limiter | 0 (unchanged) |

At the two new taps the one-pass runs no conversion, no voice activation and no chain (as `AfterAgc`), and reports no events.

## Harness configurations (`configs.rs`)

| Name | Base | Changes |
|---|---|---|
| `settings_011` | `Settings::new` | `mic = BYPASSED` (the 0.11 default) |
| `settings_008` | `settings_011` | as before (output bypassed, tap after stage 10) |
| `default_output`, `default_earphone`, `default_loud` | `settings_011` | as before |
| `mic_only` | `bypass_all` | mic `DEVICE`, tap `AfterMicAmp` |
| `mic_only_quiet` | `mic_only` | `capsule_enabled = false` |
| `mic_capsule_only` | `bypass_all` | capsule on, amp off, tap `AfterMicCapsule` |
| `default_mic` | `Settings::new` | `vas.enabled = false` (the 0.12 default with the voice activation bypassed) |

## Golden entry (`golden-mic-v1.json`)

`{ config, stimulus, host_rate_hz, seed, counter, hiss_offset_db, out_len, out_hash }`, with 84 entries: 3 stimuli × 4 configurations × 7 rates (contracts/golden-format.md).

## Measurement rows (harness report)

The rows reuse the existing `MeasurementResult`:
- the prototype deviation per third-octave;
- the hiss level and shape;
- the offsets;
- the clipper onsets and limits;
- the bound;
- the harmonics and alias levels;
- the settling time;
- the 4σ count;
- the cross-correlations;
- the voice-activation margin, exceedances and the bisected offsets per level;
- the limiter ratio;
- the decoded hiss;
- the 1 kHz group delay.
