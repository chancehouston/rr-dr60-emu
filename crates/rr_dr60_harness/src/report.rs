//! Human-readable report of measurement results (spec FR-020; tasks.md T046).

use crate::checks::MeasurementResult;

/// Formats results as an aligned table: requirement, property, trace, rate, config,
/// measured, tolerance, PASS/FAIL (quickstart.md §2).
pub fn format(results: &[MeasurementResult]) -> String {
    let rows: Vec<[String; 8]> = results
        .iter()
        .map(|r| {
            [
                r.requirement.to_string(),
                r.property.to_string(),
                format!("[{}]", r.trace.join(",")),
                r.host_rate_hz.to_string(),
                r.config.to_string(),
                format!("{:.3} {}", r.measured, r.unit),
                format!("{} {}", r.tolerance, r.unit),
                if r.passed { "PASS" } else { "FAIL" }.to_string(),
            ]
        })
        .collect();
    let mut widths = [0usize; 8];
    for row in &rows {
        for (w, cell) in widths.iter_mut().zip(row) {
            *w = (*w).max(cell.chars().count());
        }
    }
    let mut out = String::new();
    for row in &rows {
        let line: Vec<String> = row
            .iter()
            .zip(widths)
            .map(|(c, w)| format!("{c:<w$}"))
            .collect();
        out.push_str(line.join("  ").trim_end());
        out.push('\n');
    }
    let failed = results.iter().filter(|r| !r.passed).count();
    out.push_str(&format!(
        "{} checks, {} passed, {} failed\n",
        results.len(),
        results.len() - failed,
        failed
    ));
    out
}

/// Prints the table to stdout (visible with `--nocapture`, and always on failure).
pub fn print(results: &[MeasurementResult]) {
    print!("{}", format(results));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::Tolerance;

    #[test]
    fn table_is_aligned_and_counts_failures() {
        let results = [
            MeasurementResult::new(
                "FR-010",
                "upper -3 dB point",
                &["A-002", "A-014"],
                48_000,
                "record_only",
                "impulse",
                3406.2,
                "Hz",
                Tolerance::Range(3350.0, 3450.0),
            ),
            MeasurementResult::new(
                "FR-012",
                "latency vs measured",
                &["engineering target"],
                8000,
                "default",
                "tone",
                2.5,
                "samples",
                Tolerance::AtMost(1.0),
            ),
        ];
        let text = format(&results);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(
            lines[0].contains("FR-010")
                && lines[0].contains("[A-002,A-014]")
                && lines[0].ends_with("PASS")
        );
        assert!(lines[1].contains("[engineering target]") && lines[1].ends_with("FAIL"));
        assert_eq!(
            lines[0].find("48000"),
            lines[1].find("8000"),
            "rate column misaligned"
        );
        assert_eq!(lines[2], "2 checks, 1 passed, 1 failed");
        print(&results);
    }
}
