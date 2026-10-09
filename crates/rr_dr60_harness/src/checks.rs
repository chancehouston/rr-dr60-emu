//! Measurement results and one check function per spec requirement (tasks.md T022, T048).
//!
//! Every result carries the requirement it verifies and its trace (A-/S- IDs, or
//! "engineering target"), as FR-020 requires.

use std::fmt;

/// The pass condition for a measured value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tolerance {
    /// `lo ≤ measured ≤ hi`.
    Range(f64, f64),
    /// `measured ≥ min`.
    AtLeast(f64),
    /// `measured ≤ max`.
    AtMost(f64),
    /// `measured == value`.
    Exact(f64),
}

impl Tolerance {
    /// Whether `measured` satisfies this tolerance. NaN never does.
    pub fn contains(self, measured: f64) -> bool {
        match self {
            Tolerance::Range(lo, hi) => (lo..=hi).contains(&measured),
            Tolerance::AtLeast(min) => measured >= min,
            Tolerance::AtMost(max) => measured <= max,
            Tolerance::Exact(value) => measured == value,
        }
    }
}

impl fmt::Display for Tolerance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Tolerance::Range(lo, hi) => write!(f, "{lo}..={hi}"),
            Tolerance::AtLeast(min) => write!(f, ">= {min}"),
            Tolerance::AtMost(max) => write!(f, "<= {max}"),
            Tolerance::Exact(value) => write!(f, "== {value}"),
        }
    }
}

/// One harness check (data-model.md › Measurement result).
#[derive(Clone, Debug, PartialEq)]
pub struct MeasurementResult {
    /// Spec requirement verified, e.g. `"FR-010"`.
    pub requirement: &'static str,
    /// What was measured, e.g. `"upper -3 dB point"`.
    pub property: &'static str,
    /// Trace IDs, e.g. `["A-002", "A-014"]`, or `["engineering target"]`.
    pub trace: &'static [&'static str],
    /// Host rate of the run, in Hz.
    pub host_rate_hz: u32,
    /// Named configuration, e.g. `"record_only"`.
    pub config: &'static str,
    /// Stimulus description, e.g. `"tone 3400 Hz -20 dBFS"`.
    pub stimulus: String,
    /// Measured value.
    pub measured: f64,
    /// Unit of `measured` and of the tolerance, e.g. `"Hz"`.
    pub unit: &'static str,
    /// Pass condition.
    pub tolerance: Tolerance,
    /// Whether `measured` satisfies `tolerance`.
    pub passed: bool,
}

impl MeasurementResult {
    /// Builds a result and evaluates `passed` from the tolerance.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        requirement: &'static str,
        property: &'static str,
        trace: &'static [&'static str],
        host_rate_hz: u32,
        config: &'static str,
        stimulus: impl Into<String>,
        measured: f64,
        unit: &'static str,
        tolerance: Tolerance,
    ) -> Self {
        Self {
            requirement,
            property,
            trace,
            host_rate_hz,
            config,
            stimulus: stimulus.into(),
            measured,
            unit,
            tolerance,
            passed: tolerance.contains(measured),
        }
    }
}

impl fmt::Display for MeasurementResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} [{}] {} Hz {} ({}): measured {:.4} {}, tolerance {} {} — {}",
            self.requirement,
            self.property,
            self.trace.join(","),
            self.host_rate_hz,
            self.config,
            self.stimulus,
            self.measured,
            self.unit,
            self.tolerance,
            self.unit,
            if self.passed { "PASS" } else { "FAIL" }
        )
    }
}

/// Panics with a table of every failed result if any check failed.
pub fn assert_all(results: &[MeasurementResult]) {
    let failures: Vec<String> = results
        .iter()
        .filter(|r| !r.passed)
        .map(ToString::to_string)
        .collect();
    assert!(
        failures.is_empty(),
        "{} of {} checks failed:\n  {}",
        failures.len(),
        results.len(),
        failures.join("\n  ")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(measured: f64, tolerance: Tolerance) -> MeasurementResult {
        MeasurementResult::new(
            "FR-010",
            "upper -3 dB point",
            &["A-002", "A-014"],
            48_000,
            "record_only",
            "sweep",
            measured,
            "Hz",
            tolerance,
        )
    }

    #[test]
    fn tolerance_contains() {
        assert!(Tolerance::Range(3350.0, 3450.0).contains(3406.0));
        assert!(!Tolerance::Range(3350.0, 3450.0).contains(3300.0));
        assert!(
            Tolerance::AtLeast(20.0).contains(20.0) && !Tolerance::AtLeast(20.0).contains(19.9)
        );
        assert!(Tolerance::AtMost(2.0).contains(1.9) && !Tolerance::AtMost(2.0).contains(2.1));
        assert!(Tolerance::Exact(0.0).contains(0.0) && !Tolerance::Exact(0.0).contains(1e-300));
        assert!(!Tolerance::AtLeast(0.0).contains(f64::NAN));
    }

    #[test]
    fn result_evaluates_passed_and_formats() {
        let ok = result(3406.0, Tolerance::Range(3350.0, 3450.0));
        assert!(ok.passed);
        let text = ok.to_string();
        for part in [
            "FR-010",
            "A-002,A-014",
            "48000",
            "record_only",
            "3406",
            "3350..=3450",
            "PASS",
        ] {
            assert!(text.contains(part), "{text} lacks {part}");
        }
        assert!(
            result(1.0, Tolerance::AtLeast(2.0))
                .to_string()
                .contains("FAIL")
        );
        assert!(Tolerance::AtMost(2.0).to_string().contains("<= 2"));
        assert!(Tolerance::Exact(1.0).to_string().contains("== 1"));
        assert!(Tolerance::AtLeast(1.0).to_string().contains(">= 1"));
    }

    #[test]
    fn assert_all_passes_when_all_pass() {
        assert_all(&[result(3400.0, Tolerance::Range(3350.0, 3450.0))]);
        assert_all(&[]);
    }

    #[test]
    #[should_panic(expected = "1 of 2 checks failed")]
    fn assert_all_panics_on_failure() {
        assert_all(&[
            result(3400.0, Tolerance::AtMost(3450.0)),
            result(3500.0, Tolerance::AtMost(3450.0)),
        ]);
    }
}
