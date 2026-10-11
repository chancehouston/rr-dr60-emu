# Contract: Rust API changes (feature 005)

Breaking changes (0.4 → 0.5; nothing released). Everything not listed is unchanged.

## `rr_dr60::AgcSettings` (replaced)

```text
#[non_exhaustive] pub struct AgcSettings {
    pub enabled: bool,      // default true (A-020)
    pub knee_dbfs: f32,     // default 1.0 (A-028); range -40.0 ..= 3.0 (above about +4.7 the detector cannot conduct, R-05)
    pub attack_ms: f32,     // dead time at 10 dB over the knee; default 3.0 (A-034); range 1.0 ..= 50.0 (the circuit floors it near 1 ms, R-05)
    pub release_ms: f32,    // time to within 1 dB of unity from 10 dB of attenuation; default 1370.0 (A-035); range 200.0 ..= 10_000.0
}
impl AgcSettings { pub const DEVICE: Self; }   // the values above
```

Removed: `target_dbfs`, `max_gain_db`, `max_attenuation_db`. `attack_ms` and `release_ms` keep their names with new meanings, defaults and ranges.

## `rr_dr60::Settings` (one field added)

`pub adc_clamp_enabled: bool` (default `true`; A-030). `Settings::new(host)` sets it. `#[non_exhaustive]` as before.

## `rr_dr60::Setting` (variants changed)

`AgcKneeDbfs` (new; replaces `AgcTargetDbfs`); `AgcMaxGainDb` and `AgcMaxAttenuationDb` removed; `AgcAttackMs`, `AgcReleaseMs` unchanged in name. `Error::InvalidSetting { setting }` and `Settings::validate` as spec 002.

## `rr_dr60::VasSettings::DEVICE`

`threshold_dbfs` = −40.0 (A-022 revised). Range unchanged.

## `rr_dr60::Tap::AfterAgc` (meaning changed)

The limiter's output at the **host rate**, sample for sample; the rate converters are not run; `latency_samples()` = 0; `max_events(n)` = 0; `BlockInfo { produced: n, events: 0, paused: false }`.

## `rr_dr60::Pipeline` behavior contract

- With `agc.enabled = false` the limiter is skipped bit-exact. With it on, input whose peaks of the sensed polarity stay 1.5 dB below the knee passes bit-exact (FR-003).
- `latency_samples()` is unchanged for every tap but `AfterAgc` (spec 004 values).
- `reset()` discharges the limiter (`vc = 0`). A pipeline whose limiter has fully released is bit-identical in state to a fresh one; "fully released" means the capacitor has flushed to 0.0, about 140 s of sub-knee input after a loud burst (the gain is exactly 1.0 again after about 18 s; R-03). Tests assert `limiter_state() == 0.0` or feed > 140 s.
- The clamp applies after the record stage whenever `adc_clamp_enabled`; the `AfterRecord`, `AfterVas` and `AfterPlayback` outputs see it; `AfterAgc` does not.

## Crate docs

`lib.rs`: the record path is "the limiter (stage 3), the host-to-device conversion, the record band-limit (stage 4), the ADC's full-scale saturation, the VAS (stage 5) …"; the level anchor (0 dBFS = the codec's full scale, about 0.196 V peak at the mic-amp output; speech at 30 cm 25–35 dB below); "modeled on the circuit, not measured against a unit".

## Test hooks (`__test-hooks` feature; unpublished)

- `__test_hooks::limiter_state(&Pipeline) -> f64` (the capacitor voltage) for unit and harness tests.
- `Pipeline::__with_limiter_mutation(settings, knee_shift_db: f64, release_scale: f64)` for the SC-007 mutants (or settings-level mutants if the spec tolerances make the hook unnecessary; plan choice at implementation).
- `record_sos()` / `playback_sos()` / `__with_stage_sos` unchanged (spec 004).

## Version

`rr_dr60::VERSION` = "0.5.0" (workspace).
