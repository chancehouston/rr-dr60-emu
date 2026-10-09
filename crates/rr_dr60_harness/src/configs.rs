//! Named pipeline configurations and host rates for the measurement matrix
//! (data-model.md › Named configurations; tasks.md T045).

use rr_dr60::{SUPPORTED_HOST_RATES, Settings, Tap};

/// The five named configurations, in data-model.md order.
pub const CONFIGS: [&str; 5] = [
    "default",
    "record_only",
    "playback_only",
    "tap_after_record",
    "bypass_all",
];

/// Settings for a named configuration at `host_rate_hz`.
///
/// # Panics
///
/// On an unknown configuration name.
pub fn settings(name: &str, host_rate_hz: u32) -> Settings {
    let mut s = Settings::new(host_rate_hz);
    match name {
        "default" => {}
        "record_only" => s.playback_stage_enabled = false,
        "playback_only" => s.record_stage_enabled = false,
        "tap_after_record" => s.tap = Tap::AfterRecord,
        "bypass_all" => {
            s.record_stage_enabled = false;
            s.playback_stage_enabled = false;
        }
        other => panic!("unknown configuration {other:?}"),
    }
    s
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
            (s.record_stage_enabled, s.playback_stage_enabled, s.tap)
        };
        assert_eq!(row("default"), (true, true, Tap::AfterPlayback));
        assert_eq!(row("record_only"), (true, false, Tap::AfterPlayback));
        assert_eq!(row("playback_only"), (false, true, Tap::AfterPlayback));
        assert_eq!(row("tap_after_record"), (true, true, Tap::AfterRecord));
        assert_eq!(row("bypass_all"), (false, false, Tap::AfterPlayback));
        assert_eq!(all_rates(), [8000, 16000, 44100, 48000, 88200, 96000]);
    }

    #[test]
    #[should_panic(expected = "unknown configuration")]
    fn unknown_config_panics() {
        settings("nope", 48_000);
    }
}
