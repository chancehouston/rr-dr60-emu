//! Named pipeline configurations and host rates for the measurement matrix
//! (data-model.md › Named configurations; tasks.md T045).

use rr_dr60::{SUPPORTED_HOST_RATES, Settings, Tap, VasMode};
use rr_dr60_ffi::{RrDr60Settings, RrDr60Tap, RrDr60VasMode, rr_dr60_settings_default};

/// The five spec 001 configurations, in 001 data-model.md order. Each one now means "the spec
/// 001 configuration with the AGC and the VAS bypassed" (spec 002 FR-018, spec 003 FR-020), so
/// every 001 check and `golden-v1.json` stay unchanged.
pub const CONFIGS: [&str; 5] = [
    "default",
    "record_only",
    "playback_only",
    "tap_after_record",
    "bypass_all",
];

/// The spec 002 AGC configurations (002 data-model.md › Named harness configurations). Each one
/// now means "the spec 002 configuration with the VAS bypassed" (spec 003 FR-020), so every 002
/// check and `golden-agc-v1.json` stay unchanged.
pub const AGC_CONFIGS: [&str; 4] = [
    "agc_only",
    "agc_isolated_after_playback",
    "agc_tap_stages_on",
    "default_agc",
];

/// The spec 003 VAS configurations (003 contracts/golden-format.md › Configurations). These are
/// Foundational: US1 and US2 tests use them with plain assertions (plan › Story boundaries).
pub const VAS_CONFIGS: [&str; 3] = ["default_vas", "vas_mute", "vas_only"];

/// Settings for a named configuration at `host_rate_hz`.
///
/// The spec 001 names ([`CONFIGS`]) have the AGC and the VAS **bypassed**: `"default"` is the
/// spec 001 default. The spec 002 names ([`AGC_CONFIGS`]) have the VAS bypassed:
/// `"default_agc"` is the 0.2 default. The real 0.3 default (AGC and VAS on) is
/// `"default_vas"`.
///
/// # Panics
///
/// On an unknown configuration name.
pub fn settings(name: &str, host_rate_hz: u32) -> Settings {
    let mut s = Settings::new(host_rate_hz);
    if CONFIGS.contains(&name) {
        s.agc.enabled = false; // spec 002 FR-018
    }
    if CONFIGS.contains(&name) || AGC_CONFIGS.contains(&name) {
        s.vas.enabled = false; // spec 003 FR-020
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
        // Spec 003: the VAS-isolated configuration (AGC, stages 4 and 10 bypassed).
        "vas_only" | "vas_mute" => {
            s.agc.enabled = false;
            s.record_stage_enabled = false;
            s.playback_stage_enabled = false;
            s.tap = Tap::AfterVas;
            if name == "vas_mute" {
                s.vas.mode = VasMode::Mute;
            }
        }
        "default_vas" => {}
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
        Tap::AfterVas => RrDr60Tap::AfterVas,
        other => panic!("configs::c_settings: unmapped tap {other:?}"),
    };
    let vas_mode = match s.vas.mode {
        VasMode::Drop => RrDr60VasMode::Drop,
        VasMode::Mute => RrDr60VasMode::Mute,
        other => panic!("configs::c_settings: unmapped VAS mode {other:?}"),
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
        vas_enabled: s.vas.enabled,
        vas_mode: vas_mode as u32,
        vas_sensitivity: u32::from(s.vas.sensitivity),
        vas_threshold_dbfs: s.vas.threshold_dbfs,
        vas_hang_ms: s.vas.hang_ms,
        vas_onset_ms: s.vas.onset_ms,
        ..rr_dr60_settings_default(s.host_rate_hz)
    }
}

/// The spec 001 default in the C API: `rr_dr60_settings_default` with the AGC and the VAS off
/// (spec 002 FR-018, spec 003 FR-020).
pub fn c_default_001(host_rate_hz: u32) -> RrDr60Settings {
    RrDr60Settings {
        agc_enabled: false,
        vas_enabled: false,
        ..rr_dr60_settings_default(host_rate_hz)
    }
}

/// The spec 002 default in the C API: `rr_dr60_settings_default` with the VAS off
/// (spec 003 FR-020).
pub fn c_default_002(host_rate_hz: u32) -> RrDr60Settings {
    RrDr60Settings {
        vas_enabled: false,
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
        // Spec 001 and 002 names: VAS bypassed (spec 003 FR-020).
        for name in CONFIGS.iter().chain(&AGC_CONFIGS) {
            assert!(!settings(name, 48_000).vas.enabled, "{name}");
        }
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
        // Spec 003 names (003 contracts/golden-format.md).
        assert_eq!(row("vas_only"), (false, false, false, Tap::AfterVas));
        assert_eq!(row("vas_mute"), (false, false, false, Tap::AfterVas));
        assert_eq!(row("default_vas"), (true, true, true, Tap::AfterPlayback));
        assert_eq!(settings("vas_only", 48_000).vas.mode, VasMode::Drop);
        assert_eq!(settings("vas_mute", 48_000).vas.mode, VasMode::Mute);
        assert!(settings("vas_only", 48_000).vas.enabled);
        assert_eq!(settings("default_vas", 48_000), Settings::new(48_000));
        for name in CONFIGS.iter().chain(&AGC_CONFIGS).chain(&VAS_CONFIGS) {
            let s = settings(name, 44_100);
            let c = c_settings(&s);
            assert_eq!(c.agc_enabled, s.agc.enabled, "{name}");
            assert_eq!(c.vas_enabled, s.vas.enabled, "{name}");
            assert_eq!(c.record_stage_enabled, s.record_stage_enabled, "{name}");
        }
        assert_eq!(
            c_settings(&settings("agc_only", 8000)).tap,
            RrDr60Tap::AfterAgc as u32
        );
        assert_eq!(
            c_settings(&settings("vas_mute", 8000)).vas_mode,
            RrDr60VasMode::Mute as u32
        );
        assert_eq!(
            c_settings(&settings("vas_only", 8000)).tap,
            RrDr60Tap::AfterVas as u32
        );
        assert!(!c_default_001(48_000).agc_enabled);
        assert!(!c_default_001(48_000).vas_enabled);
        assert!(c_default_002(48_000).agc_enabled);
        assert!(!c_default_002(48_000).vas_enabled);
        assert_eq!(all_rates(), [8000, 16000, 44100, 48000, 88200, 96000]);
    }

    #[test]
    #[should_panic(expected = "unknown configuration")]
    fn unknown_config_panics() {
        settings("nope", 48_000);
    }
}
