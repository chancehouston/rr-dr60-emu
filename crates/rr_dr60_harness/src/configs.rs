//! Named pipeline configurations and host rates for the measurement matrix
//! (data-model.md › Named configurations; tasks.md T045; spec 005 data-model.md › Named harness
//! configurations).

use rr_dr60::{OutputSettings, SUPPORTED_HOST_RATES, Settings, Tap, VasMode};
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

/// The spec 008 speech-codec configurations (008 contracts/golden-format.md): the real 0.8
/// default (everything on, the codec included), the codec alone at the decoder tap, and the
/// codec with the VAS on (drop mode) and everything else off.
pub const CODEC_CONFIGS: [&str; 3] = ["default_codec", "codec_only", "codec_only_vas"];

/// The spec 009 output-chain configurations (009 contracts/golden-format.md; research R-10):
/// the chain alone on each route (every earlier stage bypassed, the route output tap; the route
/// is a control the check sets), the amplifier path alone (stage 11 with the speaker template
/// off, for the clip and corner checks), and the 0.9 default on each route plus the default at
/// the "6 dB over" volume (`default_loud`; the check sets the position from
/// `rr_dr60::output::VOLUME_6DB_OVER`).
pub const OUTPUT_CONFIGS: [&str; 6] = [
    "output_only_speaker",
    "output_only_earphone",
    "output_only_amp",
    "default_output",
    "default_earphone",
    "default_loud",
];

/// The spec 010 output-clamp configurations (010 data-model.md › Harness configurations;
/// contracts/golden-format.md): each is its option-off twin ([`output_clamp_twin`]) with
/// `output.clamp_enabled = true`. `output_clamp_only` is every stage bypassed at the route-output
/// tap (the primary stimulus needs no stage); the four position-1 names clamp the chain's
/// overshoot; `default_output_ref_clamp` is the 0.9 default at the reference position, the
/// non-overshooting case (FR-010).
pub const OUTPUT_CLAMP_CONFIGS: [&str; 6] = [
    "output_clamp_only",
    "output_only_speaker_clamp",
    "output_only_earphone_clamp",
    "default_output_clamp",
    "default_earphone_clamp",
    "default_output_ref_clamp",
];

/// The option-off twin of a spec 010 clamp configuration: the same settings with
/// `clamp_enabled = false` (the harness's identity oracle, 010 FR-005).
pub fn output_clamp_twin(name: &str) -> &'static str {
    match name {
        "output_clamp_only" => "bypass_route_output",
        "output_only_speaker_clamp" => "output_only_speaker",
        "output_only_earphone_clamp" => "output_only_earphone",
        "default_output_clamp" | "default_output_ref_clamp" => "default_output",
        "default_earphone_clamp" => "default_earphone",
        other => panic!("configs::output_clamp_twin: not a clamp configuration {other:?}"),
    }
}

/// The route and the volume position of a spec 010 clamp configuration in the golden file
/// (contracts/golden-format.md): position 1 everywhere except the reference configuration;
/// the position is without effect in `output_clamp_only`.
pub fn output_clamp_route_volume(name: &str) -> (rr_dr60::Route, f32) {
    use rr_dr60::Route;
    match name {
        "output_clamp_only" | "output_only_speaker_clamp" | "default_output_clamp" => {
            (Route::Speaker, 1.0)
        }
        "output_only_earphone_clamp" | "default_earphone_clamp" => (Route::Earphone, 1.0),
        "default_output_ref_clamp" => (Route::Speaker, rr_dr60::output::VOLUME_REFERENCE),
        other => panic!("configs::output_clamp_route_volume: not a clamp configuration {other:?}"),
    }
}

/// The 0.8 default: [`Settings::new`] with the output chain **bypassed** and the tap after stage
/// 10 (spec 009 FR-016). Every pre-009 check, golden file and harness scenario builds on this
/// instead of `Settings::new`, so the earlier specs' checks and the four golden files stay
/// exactly what they were (the pattern of specs 002, 003 and 008, whose stages the earlier
/// checks bypass too).
pub fn settings_008(host_rate_hz: u32) -> Settings {
    let mut s = Settings::new(host_rate_hz);
    s.output = OutputSettings::BYPASSED; // 009 FR-016: the earlier checks run with stages 11–13 bypassed
    s.tap = Tap::AfterPlayback; // 009 FR-011: 0.8's default output
    s
}

/// The 0.7 default: [`settings_008`] with the speech codec **bypassed** too (spec 008 FR-021).
/// Every pre-008 check, golden file and harness scenario uses this instead of `Settings::new`,
/// so the earlier specs' checks and the three golden files stay exactly what they were (the
/// pattern of specs 002 and 003, whose stages the earlier checks bypass too).
pub fn settings_007(host_rate_hz: u32) -> Settings {
    let mut s = settings_008(host_rate_hz);
    s.speech_codec_enabled = false; // 008 FR-021
    s
}

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
    // Spec 008 FR-021: every pre-008 configuration runs with the speech codec bypassed; the
    // codec names below turn it back on.
    let mut s = settings_007(host_rate_hz);
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
        // Spec 008 (contracts/golden-format.md): the codec configurations (the output chain
        // bypassed, 009 FR-016).
        "default_codec" => s.speech_codec_enabled = true,
        "codec_only" | "codec_only_vas" => {
            no_agc(&mut s);
            no_stages(&mut s);
            s.adc_clamp_enabled = false;
            s.speech_codec_enabled = true;
            s.tap = Tap::AfterDecoder;
            if name == "codec_only" {
                no_vas(&mut s);
            }
        }
        // Spec 009 (contracts/golden-format.md; research R-10): the output-chain configurations
        // are the only ones with any of stages 11–13 on.
        "output_only_speaker" | "output_only_earphone" | "output_only_amp" => {
            no_agc(&mut s);
            no_vas(&mut s);
            no_stages(&mut s);
            s.adc_clamp_enabled = false;
            s.speech_codec_enabled = false;
            s.tap = Tap::RouteOutput;
            s.output.volume_amp_enabled = true; // stage 11 (A-031)
            s.output.speaker_enabled = name == "output_only_speaker"; // stage 12 (A-010)
            s.output.earphone_enabled = name == "output_only_earphone"; // stage 13 (A-032)
        }
        "default_output" | "default_earphone" | "default_loud" => s = Settings::new(host_rate_hz),
        // Spec 010: `bypass_all` at the route-output tap (the twin of `output_clamp_only`).
        "bypass_route_output" => {
            s = settings("bypass_all", host_rate_hz);
            s.tap = Tap::RouteOutput;
        }
        // Spec 010 (data-model.md › Harness configurations): the clamp configurations are their
        // twins with the output clamp on; nothing else differs.
        name if OUTPUT_CLAMP_CONFIGS.contains(&name) => {
            s = settings(output_clamp_twin(name), host_rate_hz);
            s.output.clamp_enabled = true; // 010 FR-001
        }
        other => panic!("unknown configuration {other:?}"),
    }
    s
}

/// `settings` with the memory's capacity set to `secs` seconds and `max` recordings (spec 006
/// FR-010): the small capacities the US2 scenarios record to the limit with (research R-11:
/// never the default 60 minutes in a debug test).
pub fn memory_capacity(mut settings: Settings, secs: u32, max: u32) -> Settings {
    settings.memory.capacity_s = secs;
    settings.memory.max_recordings = max;
    settings
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
        Tap::AfterDecoder => RrDr60Tap::AfterDecoder,
        Tap::RouteOutput => RrDr60Tap::RouteOutput,
        Tap::AfterVolumeAmp => RrDr60Tap::AfterVolumeAmp,
        Tap::AfterSpeaker => RrDr60Tap::AfterSpeaker,
        Tap::AfterEarphone => RrDr60Tap::AfterEarphone,
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
        memory_capacity_s: s.memory.capacity_s,
        memory_max_recordings: s.memory.max_recordings,
        reserved_008: 0,
        speech_codec_enabled: s.speech_codec_enabled,
        output_volume_amp_enabled: s.output.volume_amp_enabled, // 009 FR-014
        output_speaker_enabled: s.output.speaker_enabled,
        output_earphone_enabled: s.output.earphone_enabled,
        ..rr_dr60_settings_default(s.host_rate_hz)
    }
}

/// The 0.8 default in the C API: `rr_dr60_settings_default` with the output chain off and the
/// tap after stage 10 (spec 009 FR-016; the C twin of [`settings_008`]).
pub fn c_default_008(host_rate_hz: u32) -> RrDr60Settings {
    RrDr60Settings {
        output_volume_amp_enabled: false, // 009 FR-016
        output_speaker_enabled: false,
        output_earphone_enabled: false,
        tap: RrDr60Tap::AfterPlayback as u32, // 009 FR-011: 0.8's output
        ..rr_dr60_settings_default(host_rate_hz)
    }
}

/// The spec 001 default in the C API: `rr_dr60_settings_default` with the limiter and the VAS
/// off (spec 002 FR-018, spec 003 FR-020; the clamp stays on, spec 005 research R-06).
pub fn c_default_001(host_rate_hz: u32) -> RrDr60Settings {
    RrDr60Settings {
        agc_enabled: false,
        vas_enabled: false,
        speech_codec_enabled: false,   // 008 FR-021
        ..c_default_008(host_rate_hz)  // 009 FR-016
    }
}

/// The spec 005 AGC default in the C API (`default_agc`): `rr_dr60_settings_default` with the
/// VAS off (spec 003 FR-020).
pub fn c_default_005(host_rate_hz: u32) -> RrDr60Settings {
    RrDr60Settings {
        vas_enabled: false,
        speech_codec_enabled: false,   // 008 FR-021
        ..c_default_008(host_rate_hz)  // 009 FR-016
    }
}

/// All seven supported host rates (spec 004 FR-002; 6 kHz is the identity rate).
pub fn all_rates() -> [u32; 7] {
    SUPPORTED_HOST_RATES
}

/// The host rates the audio-heavy measurement loops run at: all seven in a normal build; under
/// coverage instrumentation (`cfg(coverage)`, set by cargo-llvm-cov) three, one per conversion
/// class: 6 kHz (the identity rate), 44.1 kHz (a non-integer ratio, 147:20) and 48 kHz (an
/// integer ratio, 8:1).
///
/// Instrumentation slows the fixed-point codec and the metrics about thirty times (a 22 s story
/// test took 722 s; PR #20's coverage job took 97 min, the two codec binaries 39 of them).
/// Coverage counts lines, and no line depends on the host rate beyond those three classes; the
/// per-rate figures (latencies, lengths) stay checked at all seven rates by the cheap loops that
/// keep `all_rates`, and every plain `cargo test` run measures all seven. This is a measurement
/// budget, not a statement about the other four rates.
pub fn measurement_rates() -> &'static [u32] {
    if cfg!(coverage) {
        &[6000, 44_100, 48_000]
    } else {
        &SUPPORTED_HOST_RATES
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `measurement_rates` is a subset of the seven, in order, and always holds the identity
    /// rate, a non-integer ratio and an integer ratio; outside coverage it is the seven.
    #[test]
    fn measurement_rates_are_a_subset_with_the_three_classes() {
        let m = measurement_rates();
        let all = all_rates();
        assert!(m.iter().all(|r| all.contains(r)));
        assert!(m.windows(2).all(|w| w[0] < w[1]));
        for required in [6000, 44_100, 48_000] {
            assert!(m.contains(&required), "{required} Hz missing");
        }
        if !cfg!(coverage) {
            assert_eq!(m, &all[..]);
        }
    }

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
        assert_eq!(settings("default_vas", 48_000), settings_007(48_000)); // 008 FR-021
        // Spec 008: the codec names; the 0.8 default is `default_codec` (the output chain
        // bypassed since 009, FR-016).
        assert_eq!(settings("default_codec", 48_000), settings_008(48_000));
        assert_eq!(
            settings("default_codec", 48_000).output,
            OutputSettings::BYPASSED
        );
        assert!(settings("default_codec", 48_000).speech_codec_enabled);
        assert_eq!(
            row("codec_only"),
            (off, off, off, off, off, Tap::AfterDecoder)
        );
        assert_eq!(
            row("codec_only_vas"),
            (off, off, off, on, off, Tap::AfterDecoder)
        );
        assert!(settings("codec_only", 48_000).speech_codec_enabled);
        for name in CONFIGS.iter().chain(&AGC_CONFIGS).chain(&VAS_CONFIGS) {
            assert!(
                !settings(name, 6000).speech_codec_enabled,
                "{name}: codec bypassed (008 FR-021)"
            );
        }
        // Spec 009 FR-016: every pre-009 name runs with stages 11–13 bypassed at 0.8's tap or
        // its own earlier tap; only the output names turn the chain on.
        for name in CONFIGS
            .iter()
            .chain(&AGC_CONFIGS)
            .chain(&VAS_CONFIGS)
            .chain(&CODEC_CONFIGS)
            .chain(&["record_only_clamp_off", "record_path_agc"])
        {
            let s = settings(name, 6000);
            assert_eq!(s.output, OutputSettings::BYPASSED, "{name} (009 FR-016)");
            assert_ne!(
                s.tap,
                Tap::RouteOutput,
                "{name}: no pre-009 name uses the route tap"
            );
            let c = c_settings(&s);
            assert!(
                !c.output_volume_amp_enabled
                    && !c.output_speaker_enabled
                    && !c.output_earphone_enabled,
                "{name}"
            );
        }
        let out = |n| {
            let s = settings(n, 48_000);
            (
                s.output.volume_amp_enabled,
                s.output.speaker_enabled,
                s.output.earphone_enabled,
                s.tap,
            )
        };
        // Spec 010 T004: every pre-010 name keeps the output clamp off; every clamp name sets
        // it and differs from its twin in nothing else.
        for name in CONFIGS
            .iter()
            .chain(AGC_CONFIGS.iter())
            .chain(VAS_CONFIGS.iter())
            .chain(CODEC_CONFIGS.iter())
            .chain(OUTPUT_CONFIGS.iter())
            .chain(["bypass_route_output"].iter())
        {
            assert!(!settings(name, 48_000).output.clamp_enabled, "{name}");
        }
        for name in OUTPUT_CLAMP_CONFIGS {
            let c = settings(name, 48_000);
            let mut twin = settings(output_clamp_twin(name), 48_000);
            assert!(c.output.clamp_enabled, "{name}");
            twin.output.clamp_enabled = true;
            assert_eq!(c, twin, "{name}");
        }
        assert_eq!(
            settings("bypass_route_output", 48_000).tap,
            Tap::RouteOutput
        );
        assert_eq!(
            output_clamp_route_volume("default_output_ref_clamp").1,
            rr_dr60::output::VOLUME_REFERENCE
        );
        assert_eq!(out("output_only_speaker"), (on, on, off, Tap::RouteOutput));
        assert_eq!(out("output_only_earphone"), (on, off, on, Tap::RouteOutput));
        assert_eq!(out("output_only_amp"), (on, off, off, Tap::RouteOutput));
        assert_eq!(
            row("output_only_speaker"),
            (off, off, off, off, off, Tap::RouteOutput)
        );
        assert!(!settings("output_only_earphone", 48_000).speech_codec_enabled);
        for name in ["default_output", "default_earphone", "default_loud"] {
            assert_eq!(settings(name, 48_000), Settings::new(48_000), "{name}");
        }
        assert_eq!(
            c_settings(&settings("output_only_earphone", 6000)).tap,
            RrDr60Tap::RouteOutput as u32
        );
        assert!(c_settings(&settings("default_output", 6000)).output_speaker_enabled);
        assert!(!c_default_008(48_000).output_volume_amp_enabled);
        assert_eq!(c_default_008(48_000).tap, RrDr60Tap::AfterPlayback as u32);
        assert!(!c_default_001(48_000).output_earphone_enabled);
        assert!(!c_default_005(48_000).output_speaker_enabled);
        assert_eq!(
            c_settings(&settings("codec_only", 6000)).tap,
            RrDr60Tap::AfterDecoder as u32
        );
        assert!(!c_default_001(48_000).speech_codec_enabled);
        assert!(!c_default_005(48_000).speech_codec_enabled);
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
            assert_eq!(c.memory_capacity_s, s.memory.capacity_s, "{name}");
            assert_eq!(c.memory_max_recordings, s.memory.max_recordings, "{name}");
            assert_eq!(c.speech_codec_enabled, s.speech_codec_enabled, "{name}");
        }
        // Spec 006 FR-010: the small-capacity helper.
        let small = memory_capacity(settings("default", 48_000), 10, 3);
        assert_eq!(
            (small.memory.capacity_s, small.memory.max_recordings),
            (10, 3)
        );
        assert_eq!(c_settings(&small).memory_capacity_s, 10);
        assert_eq!(c_settings(&small).memory_max_recordings, 3);
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
        assert_eq!(c_default_005(48_000).agc_knee_dbfs, -1.0); // A-028
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
