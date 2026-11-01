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

/// The playback speed **mode** (spec 023 D1): how the five levels change the speed. The default
/// moves the pitch with the speed (A-054's mechanism (b), a resampling); the pitch-kept mode
/// time-scales the decoded stream at the 6 kHz device rate (a waveform-similarity overlap-add
/// with frames of 20 ms), **an engine convention, not a device behavior**: the device's speed
/// method is undocumented (A-054). Level 3 is the identity in both. A live control of the
/// playback stream (the control manifest's `playback_speed_mode`, index 30), set with
/// [`Pipeline::set_playback_speed_mode`](crate::Pipeline::set_playback_speed_mode).
#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SpeedMode {
    /// The pitch moves with the speed (the modeled default; 0 in C).
    #[default]
    PitchWithSpeed = 0, // S-001, S-012, A-054 (023 D1: the modeled default)
    /// The pitch is kept while the speed changes (1 in C).
    PitchKept = 1,
}

impl SpeedMode {
    /// The mode for a C value (0 or 1), or `None`.
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::PitchWithSpeed),
            1 => Some(Self::PitchKept),
            _ => None,
        }
    }
}

/// The number of speed modes (spec 023 D1).
pub const MODE_COUNT: u32 = 2; // 023 D1

/// The time-scaler's analysis frame in device samples (spec 023 D2, research R-02): 20 ms.
pub const FRAME_SAMPLES: u32 = 120; // engineering target (023 R-02); an engine convention (A-054)
/// The time-scaler's synthesis hop in device samples: half a frame, 50 % overlap (spec 023 D2).
pub const HOP_SAMPLES: u32 = 60; // engineering target (023 R-02)
/// The time-scaler's search window, ± stored samples (spec 023 D2, research R-03): 6.67 ms,
/// coherent for fundamentals of 75 Hz and above; below the hop, which makes the tail provable
/// (research R-06).
pub const SEARCH_SAMPLES: u32 = 40; // engineering target (023 R-03, R-06)
/// The samples the similarity search correlates over (spec 023 D2): the overlap region.
pub const CORRELATION_SAMPLES: u32 = 60; // engineering target (023 R-03)
/// The most device samples the pitch-kept tail adds after a recording's own part, before the
/// drain (spec 023 D4, research R-06: 120 − ((own − origin) mod 60), or 60).
pub const MAX_PITCH_KEPT_TAIL_SAMPLES: u32 = 120; // 023 R-06 (a grid bound; Δ < Ss)
/// The most device samples the audio heard in pitch-kept mode can sit from the place the
/// progress reports (spec 023 D5: Δ + 59·|1 − r| at levels 1 and 5; 52 at 2 and 4).
pub const MAX_PITCH_KEPT_OFFSET_SAMPLES: u32 = 64; // 023 R-05 (the plan review's coach 3)

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
