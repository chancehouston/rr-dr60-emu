//! The playback speed control's public constants (spec 013 FR-001, FR-002; A-054).
//!
//! The RR-DR60 plays a recording at one of five speed levels, factory set to 3, normal (S-001).
//! The emulator models the levels as speed ratios of 3/5, 4/5, 1, 6/5 and 7/5 (the speed reading
//! of the "Slow/Fast Play (+/-40%)" figure, S-012; linear spacing assumed) with the pitch moving
//! with the speed: a resampling of the decoded stream at the 6 kHz device rate between the speech
//! decoder and the codec's reconstruction filter (A-054's mechanism (b), the labelled default).
//! **Modeled on the documents, not measured on a unit**; the device's own mechanism is open
//! (capture P1 in the hardware-evidence template). Level 3 passes the stored stream unaltered,
//! so a playback at level 3 is bit-identical to version 0.12's. Set the level with
//! [`Pipeline::set_playback_speed`](crate::Pipeline::set_playback_speed).

/// The number of speed levels (S-001: five playback speeds).
pub const LEVEL_COUNT: u8 = 5;

/// The default level: normal speed (S-001: level 3 at purchase).
pub const DEFAULT_LEVEL: u8 = 3; // S-001, S-012, A-054

/// The denominator of every speed ratio: the ratios are exact fifths (spec 013 FR-002).
pub const RATIO_DENOMINATOR: u32 = 5;

/// The numerators of the speed ratios at levels 1 … 5: 3/5, 4/5, 5/5, 6/5, 7/5 (A-054, low
/// confidence; the duration reading, 5/7 … 5/3, is the registered alternative).
pub const RATIO_NUMERATORS: [u32; 5] = [3, 4, 5, 6, 7]; // A-054 (013 FR-002)

/// Stored device samples the speed control reads ahead of its place at levels other than 3, so
/// its response is centred on the place and it adds no latency (spec 013 research R-03).
pub const LOOKAHEAD_SAMPLES: u32 = 28; // engineering target (013 R-03: 281-tap prototype)

/// The most device samples the speed control's tail adds after a recording's own part, before
/// the drain (level 1; 0 at level 3; spec 013 FR-005, research R-04).
pub const MAX_TAIL_SAMPLES: u32 = 46; // 013 R-04 (speed_design.py)

/// The crossfade at a level change, in device samples (spec 019 FR-002): from the device sample
/// a change lands on, the old level fades out at its own place and ratio while the new level
/// fades in from the common place, over this many outputs (20 ms; an **engine convention**,
/// A-045's length restated, A-054: the device's behavior at a change is unknown).
pub const CROSSFADE_SAMPLES: u32 = 120; // 019 FR-002: an engine convention (A-045's length)

/// The crossfade's length in milliseconds (`CROSSFADE_SAMPLES` ÷ 6, at the device rate A-026),
/// beside [`output::VOLUME_TRANSITION_MS`](crate::output::VOLUME_TRANSITION_MS).
pub const CROSSFADE_MS: u32 = 20; // 019 FR-002

/// The speed ratio of `level` (1 … 5) as a number, for display only; `None` for any other
/// level. The engine counts places in exact fifths and never uses this value.
pub fn ratio(level: u8) -> Option<f64> {
    let i = usize::from(level).checked_sub(1)?;
    RATIO_NUMERATORS
        .get(i)
        .map(|&n| f64::from(n) / f64::from(RATIO_DENOMINATOR))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 013 T006 (FR-002): the ratios and their bounds.
    #[test]
    fn ratios() {
        assert_eq!(ratio(0), None);
        assert_eq!(ratio(6), None);
        assert_eq!(ratio(255), None);
        assert_eq!(ratio(3), Some(1.0));
        assert_eq!(ratio(1), Some(0.6));
        assert_eq!(ratio(5), Some(1.4));
        assert_eq!(usize::from(LEVEL_COUNT), RATIO_NUMERATORS.len());
        assert_eq!(
            RATIO_NUMERATORS[usize::from(DEFAULT_LEVEL - 1)],
            RATIO_DENOMINATOR
        );
    }
}
