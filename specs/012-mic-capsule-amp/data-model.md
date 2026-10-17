# Data Model: Microphone Capsule and Amplifier (stages 1–2)

> **As built (2026-10-16, after the post-implementation reviews):** this file records the design as planned. Where it differs from the implementation, the spec's Clarifications › Post-implementation reviews, research R-03 › As built, R-04 and R-09 › As measured hold: the clipper has one limit and one knee per polarity (`Clipper { l_pos, l_neg, k_pos, k_neg, … }`, L₋ ≈ +9.15 dBFS, k₋ = 0.65, L₊ ≈ +17.10 dBFS, k₊ = 0.80); the keying is nested, `mix64(mix64(seed ^ MIC_STREAM) ^ counter ^ MIC_COUNTER_SALT)`; `MicFront` holds the capsule and the amplifier; `reset` keys the one-pass to the counter's current value (FR-009), never back to 0; the registers run A-046 – A-053; the golden file holds 147 entries with the earlier files' `n_samples` and `sha256` fields; the in-build cost protocol is 45 strictly alternating runs.

**Spec**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md)

## Settings (Rust `Settings::mic`, C fields at 112–119)

| Field | Type | Default (`MicSettings::DEVICE`) | Bypassed (`MicSettings::BYPASSED`) | Validation |
|---|---|---|---|---|
| `capsule_enabled` | bool | true | false | none |
| `amp_enabled` | bool | true | false | none |
| `hiss_offset_db` | f32 | 0.0 | 0.0 | finite, −18.0 ≤ x ≤ +12.0, checked whether or not the capsule is on, else `Error::InvalidSetting(Setting::MicHissOffsetDb)` (C: invalid-setting status, field `RR_DR60_SETTING_FIELD_MIC_HISS_OFFSET_DB` = 16) |

`Settings::new(rate)` uses `MicSettings::DEVICE`. The offset applies only when `capsule_enabled` is on. A setting is applied by `reconfigure` (not a live control).

## Stage objects (core, `stages/mic.rs`)

### `MicCapsule` (stage 1)

| Field | Meaning |
|---|---|
| `state: [u64; 4]` | xoshiro256++ state, keyed by `key(seed, counter)` |
| `seed: u64`, `counter: u64` | the key, kept for `rekey` and `reset` |
| `gain: f64` | σ_rate · 10^(offset/20), computed at construction with `detmath::exp` (σ_rate from `mic_coeffs`) |

**Behaviour.**
- `next() -> f64` draws one ziggurat sample:
  - the layer index and sign come from bits 0–7, the mantissa from bits 11–63;
  - the fast path is integer and table work; the tail and wedge paths use `detmath::{exp, ln}`, with uniforms `(r >> 11) + 1`;
  - each rejection loop is capped at 8 attempts, returning the last candidate.
- `process(x) = x + gain · next()`.
- `rekey(counter)` rebuilds the state and keeps nothing else.

### `MicAmp` (stage 2)

| Field | Meaning |
|---|---|
| `hp_in: FirstOrder` | the input high-pass (A-046) with the 1 kHz normalization folded into its numerator; per-rate fitted coefficients (zero at DC, pole in [0, 1)) |
| `l_pos, l_neg, inv_pos, inv_neg, k_pos, k_neg: f64` | the clipper's limits L₊, L₋, their reciprocals and the onset thresholds k·L (A-051) |
| `k: f64` | the knee start, 0.85 (A-051), via 009's `knee` |
| `hp_out: FirstOrder` | the output coupling (A-047): bilinear form, gain (1 + p)/2, per-rate fitted pole |
| `lp: FirstOrder` | the roll-off (A-047): b0, b1 ≥ 0, pole in [0, 1), unity DC gain, per-rate fitted |

**Behaviour.** `process(x) = lp(hp_out(clip(hp_in(x))))`.
- `clip(v)` is `v` when −k·L₋ ≤ v ≤ k·L₊ (exact); otherwise it is the knee per polarity, as branchless selects.
- `reset` puts the three states at 0.
- Test hooks expose the clipper's input and output per sample, a clipper-off twin and the mutants.

### `MicFront` (in `RecordPath`)

`{ capsule: Option<MicCapsule>, amp: Option<MicAmp>, limiter: Limiter, run_limiter }`. `None` is decided at construction from the switches.

`front(x) = limiter(amp(capsule(sanitize_in(x))))`, each step skipped when off. `front_only(upto)` stops after stage 1 or stage 2 for the taps.

## Stream keying

`key(seed, counter)`:
1. `z = mix64(seed ^ MIC_STREAM) ^ mix64(counter ^ MIC_COUNTER_SALT)`, where `mix64` is SplitMix64's finalizer.
2. Four SplitMix64 outputs from `z` fill the xoshiro256++ state.

`MIC_STREAM`, `MIC_COUNTER_SALT` and the SplitMix64 constants are fixed, printed by the design script and registered in A-049.

**The recording counter** never rewinds. Its transitions are research R-05's table:
- construction: 0;
- `reset` and an accepted `reconfigure`: unchanged;
- a successful `start_recording`: rekey the recorder to (seed, counter), then counter += 1;
- a refused start, a refused reconfigure, erase, import, playback and one-pass calls: unchanged;
- an empty recording discarded at the stop has consumed its stream.

The one-pass capsule is keyed to (seed, counter) at construction, `reset` and `reconfigure`. So recording k since construction carries stream k, and the first recording after any of those events equals the one-pass.

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
| `mic_capsule_only` | `bypass_all` | capsule on at offset −12 dB, amp off, tap `AfterMicCapsule` |
| `default_mic` | `Settings::new` | `vas.enabled = false` (the 0.12 default with the voice activation bypassed) |
| `default_mic_quiet` | `default_mic` | `mic.capsule_enabled = false` |

## Golden entry (`golden-mic-v1.json`)

`{ config, stimulus, host_rate_hz, seed, counter, hiss_offset_db, out_len, out_hash }`, with 147 entries: 4 stimuli × 5 configurations × 7 rates, plus 7 second-recording entries (contracts/golden-format.md).

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
