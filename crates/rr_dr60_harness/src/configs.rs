//! Named pipeline configurations and host rates for the measurement matrix
//! (data-model.md › Named configurations; tasks.md T045; spec 005 data-model.md › Named harness
//! configurations).

use rr_dr60::{SUPPORTED_HOST_RATES, Settings, Tap, VasMode};
use rr_dr60_ffi::{RrDr60Settings, RrDr60Tap, RrDr60VasMode, rr_dr60_settings_default};

/// The five spec 001 configurations, in 001 data-model.md order. Each one means "the spec 001
/// configuration with the limiter and the VAS bypassed" (spec 002 FR-018, spec 003 FR-020) and,
/// since spec 005, with the ADC clamp **on** except in `bypass_all` (spec 005 research R-06: no
/// 001 stimulus reaches full scale, so every 001 check and `golden-v1.json` stay unchanged).
pub const CONFIGS: [&str; 5] = [
    "default",
    "record_only",
    "playback_only",
    "tap_after_record",
    "bypass_all",
];

/// The spec 005 AGC configurations (005 data-model.md › Named harness configurations): the
/// limiter alone at the host-rate "after AGC" tap, and the 0.5 default with the VAS off. The
/// spec 002 names `agc_isolated_after_playback` and `agc_tap_stages_on` are retired with the
/// reference-ratio method (005 research R-08).
pub const AGC_CONFIGS: [&str; 2] = ["agc_only", "default_agc"];

/// The spec 003 VAS configurations (003 contracts/golden-format.md › Configurations). These are
/// Foundational: US1 and US2 tests use them with plain assertions (plan › Story boundaries).
pub const VAS_CONFIGS: [&str; 3] = ["default_vas", "vas_mute", "vas_only"];

/// Settings for a named configuration at `host_rate_hz` (spec 005 data-model.md › Named
/// harness configurations).
///
/// The spec 001 names ([`CONFIGS`]) have the limiter and the VAS **bypassed** and the clamp on
/// (off in `bypass_all`): `"default"` is the spec 001 default. `"record_only_clamp_off"` is
/// `record_only` with the clamp off (spec 005 FR-008). The AGC names ([`AGC_CONFIGS`]) have
/// the VAS bypassed; `"agc_only"` is the limiter alone at the host-rate tap, `"default_agc"`
/// the 0.5 default with the VAS off, and `"record_path_agc"` the latter tapped after the record
/// stage and the clamp (FR-007, FR-008). The VAS-isolated names have the limiter, both stages
/// and the clamp off. The real 0.5 default (everything on) is `"default_vas"`.
///
/// # Panics
///
/// On an unknown configuration name.
pub fn settings(name: &str, host_rate_hz: u32) -> Settings {
    let mut s = Settings::new(host_rate_hz);
    let no_agc = |s: &mut Settings| s.agc.enabled = false; // spec 002 FR-018
    let no_vas = |s: &mut Settings| s.vas.enabled = false; // spec 003 FR-020
    let no_stages = |s: &mut Settings| {
        s.record_stage_enabled = false;
        s.playback_stage_enabled = false;
    };
    match name {
        "default" => {
            no_agc(&mut s);
            no_vas(&mut s);
        }
        "record_only" => {
            no_agc(&mut s);
            no_vas(&mut s);
            s.playback_stage_enabled = false;
        }
        "playback_only" => {
            no_agc(&mut s);
            no_vas(&mut s);
            s.record_stage_enabled = false;
        }
        "tap_after_record" => {
            no_agc(&mut s);
            no_vas(&mut s);
            s.tap = Tap::AfterRecord;
        }
        "bypass_all" => {
            no_agc(&mut s);
            no_vas(&mut s);
            no_stages(&mut s);
            s.adc_clamp_enabled = false; // spec 005 research R-06
        }
        // Spec 005 FR-008: the un-clamped record stage.
        "record_only_clamp_off" => {
            no_agc(&mut s);
            no_vas(&mut s);
            s.playback_stage_enabled = false;
            s.adc_clamp_enabled = false;
        }
        // Spec 005: the AGC-isolated configuration is the limiter alone at the host-rate tap.
        "agc_only" => {
            no_vas(&mut s);
            no_stages(&mut s);
            s.adc_clamp_enabled = false;
            s.tap = Tap::AfterAgc;
        }
        "default_agc" => no_vas(&mut s),
        // Spec 005 FR-007, FR-008: the record path with the limiter on, observed at the device
        // rate: limiter, record stage and clamp on, VAS off, tap after the record stage.
        "record_path_agc" => {
            no_vas(&mut s);
            s.tap = Tap::AfterRecord;
        }
        // Spec 003: the VAS-isolated configuration (limiter, stages and clamp bypassed).
        "vas_only" | "vas_mute" => {
            no_agc(&mut s);
            no_stages(&mut s);
            s.adc_clamp_enabled = false;
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

/// The C API settings equivalent to `s`, field for field (the two reserved AGC slots are
/// written as 0). Taps are mapped exhaustively, so a new tap is never silently tested as
/// another (spec 002 research.md R-10).
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
        agc_knee_dbfs: s.agc.knee_dbfs,
        reserved_agc_1: 0.0,
        reserved_agc_2: 0.0,
        agc_attack_ms: s.agc.attack_ms,
        agc_release_ms: s.agc.release_ms,
        vas_enabled: s.vas.enabled,
        vas_mode: vas_mode as u32,
        vas_sensitivity: u32::from(s.vas.sensitivity),
        vas_threshold_dbfs: s.vas.threshold_dbfs,
        vas_hang_ms: s.vas.hang_ms,
        vas_onset_ms: s.vas.onset_ms,
        adc_clamp_enabled: s.adc_clamp_enabled,
        ..rr_dr60_settings_default(s.host_rate_hz)
    }
}

/// The spec 001 default in the C API: `rr_dr60_settings_default` with the limiter and the VAS
/// off (spec 002 FR-018, spec 003 FR-020; the clamp stays on, spec 005 research R-06).
pub fn c_default_001(host_rate_hz: u32) -> RrDr60Settings {
    RrDr60Settings {
        agc_enabled: false,
        vas_enabled: false,
        ..rr_dr60_settings_default(host_rate_hz)
    }
}

/// The spec 005 AGC default in the C API (`default_agc`): `rr_dr60_settings_default` with the
/// VAS off (spec 003 FR-020).
pub fn c_default_005(host_rate_hz: u32) -> RrDr60Settings {
    RrDr60Settings {
        vas_enabled: false,
        ..rr_dr60_settings_default(host_rate_hz)
    }
}

/// All seven supported host rates (spec 004 FR-002; 6 kHz is the identity rate).
pub fn all_rates() -> [u32; 7] {
    SUPPORTED_HOST_RATES
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec 005 data-model.md › Named harness configurations: (limiter, record stage, ADC clamp,
    /// VAS, playback stage, tap) per name.
    #[test]
    fn configs_match_data_model_table() {
        let row = |n| {
            let s = settings(n, 48_000);
            (
                s.agc.enabled,
                s.record_stage_enabled,
                s.adc_clamp_enabled,
                s.vas.enabled,
                s.playback_stage_enabled,
                s.tap,
            )
        };
        let (on, off) = (true, false);
        // Spec 001 names: limiter and VAS bypassed, the clamp on except in bypass_all (R-06).
        assert_eq!(row("default"), (off, on, on, off, on, Tap::AfterPlayback));
        assert_eq!(
            row("record_only"),
            (off, on, on, off, off, Tap::AfterPlayback)
        );
        assert_eq!(
            row("playback_only"),
            (off, off, on, off, on, Tap::AfterPlayback)
        );
        assert_eq!(
            row("tap_after_record"),
            (off, on, on, off, on, Tap::AfterRecord)
        );
        assert_eq!(
            row("bypass_all"),
            (off, off, off, off, off, Tap::AfterPlayback)
        );
        assert_eq!(
            row("record_only_clamp_off"),
            (off, on, off, off, off, Tap::AfterPlayback)
        );
        // Spec 005 AGC names.
        assert_eq!(row("agc_only"), (on, off, off, off, off, Tap::AfterAgc));
        assert_eq!(
            row("default_agc"),
            (on, on, on, off, on, Tap::AfterPlayback)
        );
        assert_eq!(
            row("record_path_agc"),
            (on, on, on, off, on, Tap::AfterRecord)
        );
        // Spec 003 names: the VAS-isolated configuration has the clamp off too.
        assert_eq!(row("vas_only"), (off, off, off, on, off, Tap::AfterVas));
        assert_eq!(row("vas_mute"), (off, off, off, on, off, Tap::AfterVas));
        assert_eq!(row("default_vas"), (on, on, on, on, on, Tap::AfterPlayback));
        assert_eq!(settings("vas_only", 48_000).vas.mode, VasMode::Drop);
        assert_eq!(settings("vas_mute", 48_000).vas.mode, VasMode::Mute);
        assert_eq!(settings("default_vas", 48_000), Settings::new(48_000));
        assert_eq!(
            rr_dr60::Pipeline::new(settings("agc_only", 48_000))
                .unwrap()
                .latency_samples(),
            0,
            "the AGC-isolated configuration is the host-rate tap (005 FR-001)"
        );
        for name in CONFIGS
            .iter()
            .chain(&AGC_CONFIGS)
            .chain(&VAS_CONFIGS)
            .chain(&["record_only_clamp_off"])
        {
            let s = settings(name, 44_100);
            let c = c_settings(&s);
            assert_eq!(c.agc_enabled, s.agc.enabled, "{name}");
            assert_eq!(c.vas_enabled, s.vas.enabled, "{name}");
            assert_eq!(c.record_stage_enabled, s.record_stage_enabled, "{name}");
            assert_eq!(c.adc_clamp_enabled, s.adc_clamp_enabled, "{name}");
            assert_eq!(c.agc_knee_dbfs, s.agc.knee_dbfs, "{name}");
            assert_eq!((c.reserved_agc_1, c.reserved_agc_2), (0.0, 0.0), "{name}");
        }
        assert_eq!(
            c_settings(&settings("agc_only", 6000)).tap,
            RrDr60Tap::AfterAgc as u32
        );
        assert_eq!(
            c_settings(&settings("vas_mute", 6000)).vas_mode,
            RrDr60VasMode::Mute as u32
        );
        assert_eq!(
            c_settings(&settings("vas_only", 6000)).tap,
            RrDr60Tap::AfterVas as u32
        );
        assert!(!c_default_001(48_000).agc_enabled);
        assert!(!c_default_001(48_000).vas_enabled);
        assert!(c_default_001(48_000).adc_clamp_enabled);
        assert!(c_default_005(48_000).agc_enabled);
        assert!(!c_default_005(48_000).vas_enabled);
        assert_eq!(c_default_005(48_000).agc_knee_dbfs, 1.0); // A-028
        // Spec 004 FR-002 (data-model.md › Host rates and rate plans): seven rates, 6 kHz first
        // and the identity (bypass_all reports latency 0 and passes a block unchanged).
        assert_eq!(
            all_rates(),
            [6000, 8000, 16000, 44100, 48000, 88200, 96000] // 004 FR-002
        );
        assert_eq!(all_rates()[0], rr_dr60::DEVICE_RATE_HZ); // A-026
        for rate in all_rates() {
            let p = rr_dr60::Pipeline::new(settings("bypass_all", rate)).unwrap();
            let identity = rate == rr_dr60::DEVICE_RATE_HZ;
            assert_eq!(p.latency_samples() == 0, identity, "{rate} Hz");
        }
        // Beyond full scale too: bypass_all has the clamp off (005 R-06).
        let x: Vec<f32> = (0..600).map(|i| i as f32 * 0.005 - 1.5).collect();
        let mut p = rr_dr60::Pipeline::new(settings("bypass_all", 6000)).unwrap();
        let mut y = vec![0.0f32; x.len()];
        assert_eq!(p.process(&x, &mut y).unwrap().produced, x.len());
        assert_eq!(x, y, "6 kHz bypass_all is the identity (004 FR-002)");
    }

    #[test]
    #[should_panic(expected = "unknown configuration")]
    fn unknown_config_panics() {
        settings("nope", 48_000);
    }
}
