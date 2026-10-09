# Data Model: Automatic Gain Control (AGC)

**Feature**: [spec.md](spec.md) | **Research**: [research.md](research.md)

This extends [spec 001's data model](../001-pipeline-skeleton/data-model.md). Only new or changed entities are listed.

## AgcSettings (new)

| Field | Type | Default | Valid range (inclusive) | Source |
|---|---|---|---|---|
| `enabled` | bool | `true` | — | A-020 |
| `target_dbfs` | f32 | −10.0 | −30.0 … 0.0 | A-017 |
| `max_gain_db` | f32 | 40.0 | 0.0 … 60.0 | A-017 |
| `max_attenuation_db` | f32 | 20.0 | 0.0 … 40.0 | A-017 |
| `attack_ms` | f32 | 10.0 | 1.0 … 100.0 | A-018 |
| `release_ms` | f32 | 1000.0 | 50.0 … 10000.0 | A-018 |

- **Validation**: in `Pipeline::new` and `reconfigure`. NaN and ±Inf are out of range. The setting ranges are engineering targets (spec FR-015).
- **When the AGC is bypassed**: the numeric fields are still validated, so a bad value is reported even then. They have no effect on output.
- **Combinations**: any combination of valid values is allowed, including a release shorter than the attack (FR-012).
- **Fixed constants (not settings)**:
  - regulation slope 10:1 (A-017)
  - detector design (A-019, R-02/R-03)
  - starting gain = `max_gain_db` (A-019)

## Settings (changed)

- New field `agc: AgcSettings`. `Settings::new(rate)` sets `agc = AgcSettings::DEVICE`.
- Derives `Clone, Copy, Debug, PartialEq`. `Eq` and `Hash` are removed (R-08).

## Tap (changed)

| Variant | Runs | Notes |
|---|---|---|
| `AfterAgc` (new) | boundary, AGC (if enabled) | Stage 4 and 10 settings are ignored (spec FR-003). |
| `AfterRecord` | boundary, AGC (if enabled), stage 4 (if enabled) | |
| `AfterPlayback` (default) | boundary, AGC, stage 4, stage 10 (each if enabled) | |

## Error (changed)

- New variant `InvalidSetting { setting: Setting }`. `Setting` is a new `#[non_exhaustive]` enum: `AgcTargetDbfs`, `AgcMaxGainDb`, `AgcMaxAttenuationDb`, `AgcAttackMs`, `AgcReleaseMs`. Its `Display` gives the field name and its valid range.
- An unsupported host rate keeps its own variant, `UnsupportedHostRate`.

## AGC stage state (new, internal)

| State | Type | Initial / reset value |
|---|---|---|
| detector history: the last 63 device samples (Hilbert input and peak-hold window) | `[f64; 63]` ring + index | all 0.0, index 0 |
| gain `G` in dB | f64 | `max_gain_db` |
| derived at construction: `α_a`, `α_r`, `T`, `G_max`, `A_max` | f64 | from settings (R-05) |

All state lives inline in the stage, so nothing is allocated (FR-013). Reset restores the initial values without allocating.

## Per-sample processing (device rate, AGC enabled)

1. x = input device sample (already sanitized, 001 R-05).
2. Push x into the history.
3. e² = max(max over k = 0…31 of x[n−k]², x[n−31]² + h[n]², 1e-20), where h is the Hilbert FIR output (R-02).
4. L = (10 / ln 10) · ln(e²).
5. G<sub>t</sub> = clamp(−0.9 · (L − T), −A<sub>max</sub>, +G<sub>max</sub>).
6. G ← G + (G<sub>t</sub> − G) · (α<sub>a</sub> if G<sub>t</sub> < G else α<sub>r</sub>), then G ← flush_state(G) (|G| < 1e-30 → 0.0; R-07).
7. y = x · exp(G · ln 10 / 20).

Constants: ln 10 = `core::f64::consts::LN_10`. 10 / ln 10 and ln 10 / 20 are computed by IEEE division at construction. 0.9 = 1 − 1/10 is the A-017 slope. α<sub>a</sub> and α<sub>r</sub> come from `detmath::exp` at construction (R-05, R-06). Detector constants are engineering targets (R-15).

**Stage gating in `DeviceChain`**: `run_agc = agc.enabled`, `run_record = record_stage_enabled && tap != AfterAgc`, `run_playback = playback_stage_enabled && tap == AfterPlayback`.

## Named harness configurations (changed)

| Name | AGC | Stage 4 | Stage 10 | Tap | Used by |
|---|---|---|---|---|---|
| `default`, `record_only`, `playback_only`, `tap_after_record`, `bypass_all` | **bypassed** | as in 001 | as in 001 | as in 001 | all spec 001 checks and `golden-v1.json` (FR-018). The names are kept. A doc comment says each means "the 001 configuration with the AGC bypassed". |
| `agc_only` (AGC-isolated) | on | bypassed | bypassed | `AfterAgc` | AGC checks, `golden-agc-v1.json` |
| `agc_isolated_after_playback` | on | bypassed | bypassed | `AfterPlayback` | FR-003 (must equal `agc_only`) |
| `agc_tap_stages_on` | on | **on** | **on** | `AfterAgc` | US2 AS2: stage settings have no effect (must equal `agc_only`) |
| `default_agc` (the real default) | on | on | on | `AfterPlayback` | US1 AS5, `golden-agc-v1.json` |

## MeasurementResult, GoldenFile (unchanged format)

Same structure as 001. New results cite FR-0xx of spec 002 (written `002/FR-004` in reports to avoid clashing with 001's IDs), plus A-017 to A-020. The AGC golden file uses the same format, version 1 (see [contracts/golden-format.md](contracts/golden-format.md)).
