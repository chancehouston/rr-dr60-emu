//! Named pipeline configurations and host rates for the measurement matrix
//! (data-model.md › Named configurations; tasks.md T045).

use rr_dr60::{SUPPORTED_HOST_RATES, Settings, Tap};
use rr_dr60_ffi::{RrDr60Settings, RrDr60Tap, rr_dr60_settings_default};

/// The five spec 001 configurations, in 001 data-model.md order. Each one now means "the spec
/// 001 configuration with the AGC bypassed" (spec 002 FR-018), so every 001 check and
/// `golden-v1.json` stay unchanged.
pub const CONFIGS: [&str; 5] = [
    "default",
    "record_only",
    "playback_only",
    "tap_after_record",
    "bypass_all",
];

/// The spec 002 AGC configurations (002 data-model.md › Named harness configurations).
pub const AGC_CONFIGS: [&str; 4] = [
    "agc_only",
    "agc_isolated_after_playback",
    "agc_tap_stages_on",
    "default_agc",
];

/// Settings for a named configuration at `host_rate_hz`.
///
/// The spec 001 names ([`CONFIGS`]) have the AGC **bypassed**: `"default"` is the spec 001
/// default, not the 0.2 default. The real 0.2 default (AGC on) is `"default_agc"`.
///
/// # Panics
///
/// On an unknown configuration name.
pub fn settings(name: &str, host_rate_hz: u32) -> Settings {
    let mut s = Settings::new(host_rate_hz);
    if CONFIGS.contains(&name) {
        s.agc.enabled = false; // spec 002 FR-018
    }
    match name {
        "default" => {}
        "record_only" => s.playback_stage_enabled = false,
        "playback_only" => s.record_stage_enabled = false,
        "tap_after_record" => s.tap = Tap::AfterRecord,
        "bypass_all" => {
            s.record_stage_enabled = false;
            s.playback_stage_enabled = false;
        }
        // Spec 002: the AGC-isolated configuration (stages 4 and 10 bypassed).
        "agc_only" => {
            s.record_stage_enabled = false;
            s.playback_stage_enabled = false;
            s.tap = Tap::AfterAgc;
        }
        "agc_isolated_after_playback" => {
            s.record_stage_enabled = false;
            s.playback_stage_enabled = false;
        }
        // US2 AS2: the tap after the AGC ignores the stage settings.
        "agc_tap_stages_on" => s.tap = Tap::AfterAgc,
        "default_agc" => {}
        other => panic!("unknown configuration {other:?}"),
    }
    s
}

/// The C API settings equivalent to `s`, field for field. Taps are mapped exhaustively, so a
/// new tap is never silently tested as another (spec 002 research.md R-10).
///
/// # Panics
///
/// On a tap value this harness does not know yet.
pub fn c_settings(s: &Settings) -> RrDr60Settings {
    let tap = match s.tap {
        Tap::AfterRecord => RrDr60Tap::AfterRecord,
        Tap::AfterPlayback => RrDr60Tap::AfterPlayback,
        Tap::AfterAgc => RrDr60Tap::AfterAgc,
        other => panic!("configs::c_settings: unmapped tap {other:?}"),
    };
    RrDr60Settings {
        record_stage_enabled: s.record_stage_enabled,
        playback_stage_enabled: s.playback_stage_enabled,
        tap: tap as u32,
        seed: s.seed,
        agc_enabled: s.agc.enabled,
        agc_target_dbfs: s.agc.target_dbfs,
        agc_max_gain_db: s.agc.max_gain_db,
        agc_max_attenuation_db: s.agc.max_attenuation_db,
        agc_attack_ms: s.agc.attack_ms,
        agc_release_ms: s.agc.release_ms,
        ..rr_dr60_settings_default(s.host_rate_hz)
    }
}

/// The spec 001 default in the C API: `rr_dr60_settings_default` with the AGC off
/// (spec 002 FR-018).
pub fn c_default_001(host_rate_hz: u32) -> RrDr60Settings {
    RrDr60Settings {
        agc_enabled: false,
        ..rr_dr60_settings_default(host_rate_hz)
    }
}

/// All six supported host rates (FR-002).
pub fn all_rates() -> [u32; 6] {
    SUPPORTED_HOST_RATES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configs_match_data_model_table() {
        let row = |n| {
            let s = settings(n, 48_000);
            (
                s.agc.enabled,
                s.record_stage_enabled,
                s.playback_stage_enabled,
                s.tap,
            )
        };
        // Spec 001 names: AGC bypassed (spec 002 FR-018).
        assert_eq!(row("default"), (false, true, true, Tap::AfterPlayback));
        assert_eq!(row("record_only"), (false, true, false, Tap::AfterPlayback));
        assert_eq!(
            row("playback_only"),
            (false, false, true, Tap::AfterPlayback)
        );
        assert_eq!(
            row("tap_after_record"),
            (false, true, true, Tap::AfterRecord)
        );
        assert_eq!(row("bypass_all"), (false, false, false, Tap::AfterPlayback));
        // Spec 002 names (002 data-model.md).
        assert_eq!(row("agc_only"), (true, false, false, Tap::AfterAgc));
        assert_eq!(
            row("agc_isolated_after_playback"),
            (true, false, false, Tap::AfterPlayback)
        );
        assert_eq!(row("agc_tap_stages_on"), (true, true, true, Tap::AfterAgc));
        assert_eq!(row("default_agc"), (true, true, true, Tap::AfterPlayback));
        assert_eq!(settings("default_agc", 48_000), Settings::new(48_000));
        for name in CONFIGS.iter().chain(&AGC_CONFIGS) {
            let s = settings(name, 44_100);
            let c = c_settings(&s);
            assert_eq!(c.agc_enabled, s.agc.enabled, "{name}");
            assert_eq!(c.record_stage_enabled, s.record_stage_enabled, "{name}");
        }
        assert_eq!(
            c_settings(&settings("agc_only", 8000)).tap,
            RrDr60Tap::AfterAgc as u32
        );
        assert!(!c_default_001(48_000).agc_enabled);
        assert_eq!(all_rates(), [8000, 16000, 44100, 48000, 88200, 96000]);
    }

    #[test]
    #[should_panic(expected = "unknown configuration")]
    fn unknown_config_panics() {
        settings("nope", 48_000);
    }
}
