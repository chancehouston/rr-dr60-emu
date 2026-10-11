//! Spec 005 limiter checks: one function per requirement (research R-08; tasks.md T021–T024,
//! T033).
//!
//! Every limiter check runs the AGC-isolated configuration (`agc_only`: the "after AGC" tap at
//! the host rate, everything else off) and measures the **exact per-sample gain** `y[n] / x[n]`
//! at every host rate: no reference run and no boundary latency (the spec 002 reference-ratio
//! method and its Hilbert detector are retired, 005 R-08). Results report `005/FR-0xx` with
//! their trace IDs; tolerances that are not device properties are engineering targets (spec 005
//! FR-017, research R-13).
//!
//! **Phase 2 state (005 T016):** the check bodies are stubs that return no results until
//! T021–T024 and T033 write them; the matrix and the `Case` type are final.

use rr_dr60::AgcSettings;

use crate::checks::{Make, MeasurementResult};

/// A named AGC setting for the matrix (spec 005 FR-011).
#[derive(Clone, Copy, Debug)]
pub struct Case {
    /// Label for reports, e.g. `"knee -10 dBFS"`.
    pub label: &'static str,
    /// The AGC settings.
    pub agc: AgcSettings,
}

impl Case {
    /// Whether these are the device defaults (default tolerances apply).
    pub fn is_default(&self) -> bool {
        self.agc == AgcSettings::DEVICE
    }
}

/// The FR-011 settings matrix (005 research R-08): the defaults, then one setting moved at a
/// time with the others at default: knee −10 and +3 dBFS, attack 1, 10 and 50 ms, release 200,
/// 4000 and 10 000 ms.
pub fn matrix() -> Vec<Case> {
    let with = |label, f: fn(&mut AgcSettings)| {
        let mut agc = AgcSettings::DEVICE;
        f(&mut agc);
        Case { label, agc }
    };
    vec![
        with("defaults", |_| {}),
        with("knee -10 dBFS", |a| a.knee_dbfs = -10.0),
        with("knee +3 dBFS", |a| a.knee_dbfs = 3.0),
        with("attack 1 ms", |a| a.attack_ms = 1.0),
        with("attack 10 ms", |a| a.attack_ms = 10.0),
        with("attack 50 ms", |a| a.attack_ms = 50.0),
        with("release 200 ms", |a| a.release_ms = 200.0),
        with("release 4000 ms", |a| a.release_ms = 4000.0),
        with("release 10000 ms", |a| a.release_ms = 10_000.0),
    ]
}

/// 005 FR-003: bit-exact passthrough below the knee, and bit-exact bypass.
pub fn check_fr003_passthrough(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let _ = (rate, make);
    Vec::new() // stub; 005 T022
}

/// 005 FR-004: the static curve, the ceiling and the knee bisection.
pub fn check_fr004_static(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let _ = (rate, make);
    Vec::new() // stub; 005 T022
}

/// 005 FR-005: the dead time from the discharged state, the 2 dB-over step, the armed state.
pub fn check_fr005_attack(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let _ = (rate, make);
    Vec::new() // stub; 005 T023
}

/// 005 FR-006: the release trajectory.
pub fn check_fr006_release(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let _ = (rate, make);
    Vec::new() // stub; 005 T023
}

/// 005 FR-007: the sensed polarity.
pub fn check_fr007_polarity(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let _ = (rate, make);
    Vec::new() // stub; 005 T023
}

/// 005 FR-007: content above the device band drives the limiter (rates ≥ 16 kHz).
pub fn check_fr007_bandwidth(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let _ = (rate, make);
    Vec::new() // stub; 005 T023
}

/// 005 FR-008: the ADC clamp and the clipped onset.
pub fn check_fr008_clamp(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let _ = (rate, make);
    Vec::new() // stub; 005 T024
}

/// 005 FR-009: THD+N reports at the default output.
pub fn check_fr009_thdn(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let _ = (rate, make);
    Vec::new() // stub; 005 T024
}

/// 005 FR-001: the "after AGC" tap has latency 0; every other tap is unchanged by the limiter.
pub fn check_fr001_latency(rate: u32) -> Vec<MeasurementResult> {
    let _ = rate;
    Vec::new() // stub; 005 T024
}

/// 005 FR-011: the settings matrix (one non-default setting per case).
pub fn check_fr011_settings(rate: u32, case: &Case, make: Make<'_>) -> Vec<MeasurementResult> {
    let _ = (rate, case, make);
    Vec::new() // stub; 005 T033
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_has_the_fr011_cases() {
        let m = matrix();
        assert_eq!(m.len(), 9);
        assert!(m[0].is_default() && m[1..].iter().all(|c| !c.is_default()));
        // Every case is valid.
        for c in &m {
            let mut s = rr_dr60::Settings::new(48_000);
            s.agc = c.agc;
            assert_eq!(s.validate(), Ok(()), "{}", c.label);
        }
    }
}
