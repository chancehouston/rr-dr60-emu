# Data Model: Limiter-Type AGC, ADC Clamp, VAS Threshold (feature 005)

This revises the data models of [spec 002](../002-agc/data-model.md) (the AGC settings and stage are replaced) and [spec 003](../003-vas/data-model.md) (one default changes). Spec 001 and 004 entities are unchanged except for one new setting.

## Limiter (signal-chain stage 3; replaces the spec 002 AGC stage)

Runs at the **host rate** on the input before the rate conversion (research R-01). State: one value.

| Field | Type | Meaning |
|---|---|---|
| `vc` | f64, volts | hold-capacitor voltage; 0.0 when discharged; ≥ 0; flushed to 0.0 below `V_FLUSH` (R-03) |

Derived at construction from the settings (R-05) and the constants below:

| Parameter | From | Meaning |
|---|---|---|
| `vbe6` | `knee_dbfs` | detector threshold voltage at Q6's base: `KDIV · V_FS · 10^(knee/20)` |
| `beta6` | `attack_ms` | detector current gain: `300 · 2.37 / attack_ms` |
| `k_dis` | `release_ms` | discharge scale on both discharge terms: `1300 / release_ms` |
| `v_floor` | constants | `vc` below which the shunt's effect is < 1e-9: gain exactly 1.0, no multiply |
| `v_limit` | constants | `vc` giving 40 dB of attenuation (the limit, engineering target) |

### Circuit constants (R-02)

| Constant | Value | Trace |
|---|---|---|
| `C6` | 1 µF | S-002 |
| `R11` | 10 MΩ | S-002 (R11 · C6 = 10 s, the arming memory) |
| `R12` | 390 Ω | S-002 (caps the detector's peak charge current) |
| `R13`, `R17` | 100 kΩ, 330 kΩ | S-002; `RTH` = 76.7 kΩ, `KDIV` = 0.767 |
| `R7`, `R10`, `R_IN` | 22 kΩ, 1 kΩ, 23 kΩ | S-002 (the attenuator divider and the codec amp's input resistance) |
| `V_REF` | 1.6 V | S-002 (AIN+ reference) |
| `V_FS` | 0.7 V peak at GSX | A-030 (0 dBFS = +3 dBm0) |
| `VT` | 26 mV | physics (room temperature) |
| `IS` | 1e-14 A | transistor estimate (A-028) |
| `BETA_R` | 5 | Q4 reverse current gain, estimate (A-028) |
| `BETA5` | 300 | Q5 current gain, estimate (A-028; adds the follower's base current to the discharge) |
| `V_RAIL` | 1.2 V | codec input-amp swing limit at GSX, estimate (A-028; detector view only) |
| `POLARITY` | +1 (positive input samples sensed) | A-036 |
| `V_FLUSH` | 1e-6 V | engineering target (R-03) |

### Per-sample transitions (R-02, R-03)

1. `a = gain(vc)` (exactly 1.0 below `v_floor`); `y = a · x`.
2. `gsx = clamp(y · V_FS, ±V_RAIL)`; `over = KDIV · POLARITY · gsx − vbe6`.
3. Charge if `over > 0`: `ic = min(beta6 · over / RTH, (V_REF − vc) / R12)`.
4. Discharge: `idis = k_dis · (vc / R11 + ib4(vc) / BETA5)`.
5. `vc ← max(0, vc + (ic − idis) · dt / C6)`; if charged, `vc ← min(vc, vc_target(|x|))` (the cap); `vc ← min(vc, v_limit)`; if `vc < V_FLUSH`, `vc ← 0`.

Reset and creation: `vc = 0`. The state after a full release (`vc` flushed) equals the fresh state.

## ADC clamp (new stage, after the record stage; R-06)

| Field | Type | Meaning |
|---|---|---|
| `enabled` | bool | from `Settings::adc_clamp_enabled`; default true |

`process(d) = d.clamp(−1.0, 1.0)` when enabled; `d` unchanged when bypassed. No state. Trace: A-030 (full scale), S-005 (full-scale code).

## Settings (changed; contracts/rust-api.md, contracts/c-api.md)

| Setting | Default | Range | Trace | Was (spec 002) |
|---|---|---|---|---|
| `agc.enabled` | true | — | A-020 | same |
| `agc.knee_dbfs` | +1.0 | −40.0 … +6.0 | A-028 | `target_dbfs` −10.0 (−30 … 0) |
| `agc.attack_ms` (dead time at 10 dB over the knee) | 3.0 | 0.5 … 50.0 | A-034 | attack time 10.0 (1 … 100), settling definition |
| `agc.release_ms` (to −1 dB from 10 dB of attenuation) | 1300.0 | 200.0 … 10 000.0 | A-035 | release time 1000.0 (50 … 10 000), settling definition |
| `adc_clamp_enabled` | true | — | A-030 | (new) |
| `vas.threshold_dbfs` | −40.0 | −60.0 … 0.0 (unchanged) | A-022 revised | −18.0 |
| removed: `agc.max_gain_db`, `agc.max_attenuation_db` | — | — | — | 40.0 (0 … 60), 20.0 (0 … 40) |

`Setting` variants: `AgcKneeDbfs` (replaces `AgcTargetDbfs`), `AgcAttackMs`, `AgcReleaseMs` (meanings changed); `AgcMaxGainDb` and `AgcMaxAttenuationDb` removed. C field-enum numbers: 4 = knee (was target), 5 and 6 retired, 7 and 8 unchanged.

## Pipeline order and taps (changed; R-01)

```
host input → [limiter] → (tap AfterAgc: host-rate output, latency 0)
           → host→device conversion → [record stage] → [ADC clamp] → (tap AfterRecord)
           → [VAS] → (tap AfterVas) → [playback stage] → device→host conversion → (tap AfterPlayback)
```

Latency: `AfterAgc` 0; every other tap as spec 004 (unchanged with the limiter on or off).

## Harness stimuli (new; R-08)

| Name | Definition | Used by |
|---|---|---|
| Static-curve set | 1 kHz at the aligned phase, −60 … +20 dBFS in 2 dB steps and 1 dB steps from −4 to +6, 5 s each | FR-004 |
| Knee bisection | 1 kHz 5 s tones, bisection of the −1 dB gain point to 0.1 dB between −4 and +8 dBFS | FR-004 |
| Attack step | −20 dBFS 0.1 s then +10 dBFS (or +2 dBFS) 0.4 s, fresh pipeline | FR-005 |
| Armed step | +10 dBFS 0.2 s, −20 dBFS 2.0 s, +10 dBFS 0.2 s | FR-005 |
| Release | +11 dBFS 1 s (or +18 dBFS), then −20 dBFS 3 s | FR-006 |
| Asymmetric | `sin(ωt) + 0.5 cos(2ωt)` at 1 kHz, scaled so the larger peak is +4 dBFS; and its negation | FR-007 |
| Bandwidth | 1 kHz at −20 dBFS plus 6 kHz at +6 dBFS, 3 s, at host rates > 12 kHz | FR-007 |
| Clamp | +10 dBFS 1 kHz 1 s through `record_only`; the attack step through `default` | FR-008 |
| Speech-like pattern | three groups of four 300 ms 1 kHz bursts at −30 dBFS peak, 150 ms apart, 5 s gaps, over a band-limited noise floor (225–2550 Hz) at −60 or −45 dBFS RMS; 0.5 s lead, 1 s tail | FR-012 (i), (ii) |
| Loud-after-floor | 30 s of the −60 dBFS floor, then +6 dBFS 1 s, then 1 s floor | FR-012 (iii) |

## Golden stimuli and configurations (changed; contracts/golden-format.md)

AGC: `agc_onset`, `agc_rearm`, `agc_staircase` × `agc_only`, `default_agc` × 7 rates = 42. VAS: `vas_burst_gap`, `vas_noise_gaps`, `vas_short_bursts` (bursts at −30 dBFS) × `default_vas`, `vas_mute`, `vas_only` × 7 = 63. Spec 001: unchanged (96 → 112 entries were 004's; still 112, byte-identical).

## Changed figures (the CHANGELOG table, FR-019)

| Item | Was (0.4) | Now (0.5) |
|---|---|---|
| AGC model | compressor: target −10 dBFS, 10:1 slope, +40 dB max gain, 20 dB max attenuation (A-017) | limiter: unity below the knee, knee +1 dBFS, ceiling at the knee, ratio unlimited, 40 dB limit (A-028) |
| Attack | 10 ms settling (A-018) | dead time ≈ 3 ms at 10 dB over the knee, ≤ 0.5 ms when armed (A-034) |
| Release | 1.0 s settling, exponential in dB (A-018) | ≈ 1.3 s to −1 dB from 10 dB, fast then slow (A-035); arming memory ≈ 10 s |
| Detector | analytic envelope (Hilbert) + peak hold, device band, starts at +40 dB (A-019) | half-wave on positive samples, host-rate bandwidth, starts at unity (A-019 revised, A-036) |
| Silence / pauses | gain rises to +40 dB (noise rise) | gain stays at unity |
| Content above the device band | no effect on the gain | reduces the gain (FR-007) |
| Placement | device rate, after the decimator | host rate, before the decimator |
| Tap "after AGC" | device-rate output through the converters, boundary latency | host-rate output, latency 0 |
| ADC clamp | none | ±1.0 after stage 4, own setting, default on (A-030) |
| VAS threshold at level 3 | −18 dBFS (A-022) | −40 dBFS (A-022 revised); steps 3 dB unchanged |
| VAS burst-gap stimuli (harness, golden) | 1 kHz at −8 dBFS | 1 kHz at −30 dBFS |
| Input-referred VAS threshold (003 R-03) | ≈ −58 dBFS via the compressor's curve | −40 dBFS (identity below the knee) |
| Settings (Rust) | `target_dbfs`, `max_gain_db`, `max_attenuation_db`, `attack_ms`, `release_ms` | `knee_dbfs`, `attack_ms`, `release_ms`; `adc_clamp_enabled` |
| C struct | 72 bytes | 80 bytes; `agc_knee_dbfs` at 36; `reserved_agc_1/2` at 40/44; `adc_clamp_enabled` at 72 |
| Golden entries (001 / AGC / VAS) | 112 / 42 / 63 | 112 (unchanged) / 42 / 63 (re-blessed) |
| Version | 0.4.0 | 0.5.0 |

## Assumption status changes (FR-019)

| ID | Change |
|---|---|
| A-007 | verified in circuit (S-002); behavior in A-028 |
| A-017, A-018 | superseded by A-028 / A-034 / A-035 (spec 005) |
| A-019 | revised: half-wave, host-rate bandwidth in the emulator, starts at unity, no envelope filter |
| A-022 | revised: −40 dBFS at level 3 (rationale from A-029; 6 dB alternative recorded) |
| A-028 | adopted and reinterpreted (knee +1 dBFS ± 2 dB; R12·C6 current limit; R11·C6 arming; rail estimate; temperature; 40 dB limit; 13 kHz / 7 Hz omissions) |
| A-029 | refined: the mic amp clips 7–16 dB above the knee; the SPL anchor |
| A-034 (new) | attack dead time |
| A-035 (new) | release |
| A-036 (new) | sensed polarity |
