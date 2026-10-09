# Contract: Rust public API changes (`rr_dr60` 0.1.0 → 0.2.0)

This is a delta against [001's Rust API contract](../../001-pipeline-skeleton/contracts/rust-api.md). Anything not listed is unchanged. It is a SemVer surface: the dropped `Eq`/`Hash` derives make this a breaking change, so the 0.x minor version goes up. Every new public item gets rustdoc that cites its FR and A-IDs.

```rust
/// Where the pipeline output is taken (FR-007 of 001; FR-003 of 002).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Tap {
    /// NEW. After signal-chain stage 3 (AGC). Stages 4 and 10 are not run.
    AfterAgc,
    /// After stage 4 (record band-limit). Stage 10 is not run.
    AfterRecord,
    /// After stage 10 (playback band-limit). Default (A-002).
    #[default]
    AfterPlayback,
}

/// NEW. Record-path automatic gain control, signal-chain stage 3 (spec 002).
/// Modeled on an assumed AGC: every default is a low-confidence assumption (A-017 – A-020).
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct AgcSettings {
    /// On (true, default, A-020: the device's AGC is assumed always active) or bypassed.
    pub enabled: bool,
    /// Target level in dBFS (sine, AES17). Default −10.0 (A-017). Range −30.0 ..= 0.0.
    pub target_dbfs: f32,
    /// Maximum gain in dB. Default 40.0 (A-017). Range 0.0 ..= 60.0.
    pub max_gain_db: f32,
    /// Maximum attenuation in dB (positive number). Default 20.0 (A-017). Range 0.0 ..= 40.0.
    pub max_attenuation_db: f32,
    /// Attack time in ms (settling within 2/27 of the excursion). Default 10.0 (A-018).
    /// Range 1.0 ..= 100.0.
    pub attack_ms: f32,
    /// Release time in ms. Default 1000.0 (A-018). Range 50.0 ..= 10000.0.
    pub release_ms: f32,
}

impl AgcSettings {
    /// The assumed device values (A-017, A-018, A-020).
    pub const DEVICE: Self;
}
impl Default for AgcSettings { /* DEVICE */ }

/// CHANGED: gains `agc`; no longer `Eq` or `Hash` (floats).
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Settings {
    pub host_rate_hz: u32,
    pub record_stage_enabled: bool,
    pub playback_stage_enabled: bool,
    pub tap: Tap,
    pub seed: u64,
    /// NEW. Stage 3 settings. Default `AgcSettings::DEVICE`.
    pub agc: AgcSettings,
}

impl Settings {
    pub const fn new(host_rate_hz: u32) -> Self;
    /// NEW (added in T031). The same check, in the same order, as `Pipeline::new`:
    /// host rate first, then the AGC fields. Never allocates.
    pub fn validate(&self) -> Result<(), Error>;
}

/// NEW. Names a setting in `Error::InvalidSetting`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Setting {
    AgcTargetDbfs,
    AgcMaxGainDb,
    AgcMaxAttenuationDb,
    AgcAttackMs,
    AgcReleaseMs,
}
impl core::fmt::Display for Setting { /* "agc.attack_ms (valid range 1 to 100 ms)" */ }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    UnsupportedHostRate { requested: u32 },
    LengthMismatch { input: usize, output: usize },
    /// NEW. A setting is outside its valid range, or NaN/±Inf (FR-011).
    /// Returned by `Pipeline::new` and `reconfigure`. On `reconfigure`, the pipeline is unchanged.
    InvalidSetting { setting: Setting },
}
```

## Behavior

- `Pipeline::new`, `reconfigure`: also validate `settings.agc`, even when it is bypassed. Host-rate errors are reported first.
- `Settings::validate`: runs that check without building a pipeline; real-time safe. The C API's `rr_dr60_settings_validate` is built on it.
- `Pipeline::latency_samples`: unaffected by the AGC (FR-010). With `Tap::AfterAgc` the value is the boundary delay only.
- `Pipeline::reset`: also restores the AGC to maximum gain with an empty detector (FR-013).
- `Pipeline::process*`: still real-time safe and block-partition invariant with the AGC on (FR-013).
- **Output range**: output can exceed ±1.0 by up to `max_gain_db` (+40 dB with the defaults) right after creation, a reset or a long silence. The rustdoc on `Pipeline` and `AgcSettings` states this and advises hosts to limit or clip before converting to integer formats (spec Edge Cases). Outputs are always finite: values beyond ±`f32::MAX` saturate (R-07).
- **Test hooks**: none new. The SC-008 mutation test builds pipelines with altered public settings through the harness's `Make` closure (research.md R-12).
