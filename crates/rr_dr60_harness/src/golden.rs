//! Golden-file hashing, comparison and blessing (contracts/golden-format.md; FR-021; T055).
//!
//! Every stimulus here is bit-reproducible (detmath and integer PCG32 only), so a single
//! golden file serves every platform (FR-014). This module stays under the clippy R-04 ban.

use std::fmt::Write as _;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::checks::Make;
use crate::{configs, stimulus};

/// Golden stimuli, in sorted order (contracts/golden-format.md › Stimuli).
pub const STIMULI: [&str; 4] = ["impulse", "noise_seed_0d60", "sweep_log", "tone_1k_m20"];

/// Golden configurations, in sorted order.
pub const CONFIGS: [&str; 4] = ["bypass_all", "default", "playback_only", "record_only"];

/// Spec 002 AGC golden stimuli, in sorted order (specs/002-agc/contracts/golden-format.md).
pub const AGC_STIMULI: [&str; 3] = ["agc_noise_burst", "agc_start_m10", "agc_step"];

/// Spec 002 AGC golden configurations, in sorted order.
pub const AGC_CONFIGS: [&str; 2] = ["agc_only", "default_agc"];

/// The golden file (format `rr_dr60-golden`, version 1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GoldenFile {
    /// Always `"rr_dr60-golden"`.
    pub format: String,
    /// Always 1.
    pub version: u32,
    /// `rr_dr60` version that produced the file.
    pub library_version: String,
    /// Entries sorted by (stimulus, config, host_rate_hz).
    pub entries: Vec<GoldenEntry>,
}

/// One stimulus × configuration × rate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GoldenEntry {
    /// Stimulus name.
    pub stimulus: String,
    /// Configuration name.
    pub config: String,
    /// Host rate in Hz.
    pub host_rate_hz: u32,
    /// Number of output samples.
    pub n_samples: u32,
    /// SHA-256 of the output's little-endian f32 bit patterns.
    pub sha256: String,
    /// First 16 output samples as lowercase 8-digit hex bit patterns (diagnostic).
    pub head: Vec<String>,
    /// RMS level in dBFS, rounded to 0.01 (diagnostic only; never compared).
    pub rms_dbfs: f64,
}

/// A golden stimulus at host rate `fs` (contracts/golden-format.md).
///
/// # Panics
///
/// On an unknown stimulus name.
pub fn stimulus(name: &str, fs: u32) -> Vec<f32> {
    let f = f64::from(fs);
    let secs = |s: f64| (s * f) as usize;
    match name {
        "impulse" => stimulus::impulse(secs(0.25)),
        "tone_1k_m20" => stimulus::tone(1000.0, 0.1, f, secs(0.5)),
        "sweep_log" => stimulus::log_sweep(20.0, 0.45 * f, 0.25, f, secs(1.0)),
        "noise_seed_0d60" => stimulus::noise(0x0D60, secs(0.5)),
        // Spec 002 (contracts/golden-format.md).
        "agc_noise_burst" => crate::agc_checks::noise_burst_stimulus(f),
        "agc_start_m10" => stimulus::step(1000.0, &[-10.0], &[0.5], f),
        "agc_step" => stimulus::step(1000.0, &[-40.0, -10.0, -40.0], &[3.0, 1.0, 4.0], f),
        other => panic!("unknown stimulus {other:?}"),
    }
}

/// Hash, head and RMS of an output.
pub fn entry(stimulus: &str, config: &str, host_rate_hz: u32, output: &[f32]) -> GoldenEntry {
    let mut hasher = Sha256::new();
    for v in output {
        hasher.update(v.to_bits().to_le_bytes());
    }
    let sha256 = hasher.finalize().iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    });
    let head = output
        .iter()
        .take(16)
        .map(|v| format!("{:08x}", v.to_bits()))
        .collect();
    let mean_square = output
        .iter()
        .map(|&v| f64::from(v) * f64::from(v))
        .sum::<f64>()
        / output.len().max(1) as f64;
    let rms_dbfs = if mean_square > 0.0 {
        (10.0 * rr_dr60_detmath::ln(mean_square) / rr_dr60_detmath::ln(10.0) * 100.0).round()
            / 100.0
    } else {
        -999.0
    };
    GoldenEntry {
        stimulus: stimulus.into(),
        config: config.into(),
        host_rate_hz,
        n_samples: output.len() as u32,
        sha256,
        head,
        rms_dbfs,
    }
}

/// Generates every spec 001 entry (4 stimuli × 4 configurations × 6 rates), each stimulus
/// processed in one block. The configurations have the AGC bypassed (spec 002 FR-018).
pub fn generate(make: Make<'_>) -> GoldenFile {
    generate_set(&STIMULI, &CONFIGS, make)
}

/// Generates every spec 002 AGC entry (3 stimuli × 2 configurations × 6 rates).
pub fn generate_agc(make: Make<'_>) -> GoldenFile {
    generate_set(&AGC_STIMULI, &AGC_CONFIGS, make)
}

fn generate_set(stimuli: &[&str], config_names: &[&str], make: Make<'_>) -> GoldenFile {
    let mut entries = Vec::new();
    for &s in stimuli {
        for &c in config_names {
            for rate in configs::all_rates() {
                let x = stimulus(s, rate);
                let mut p = make(configs::settings(c, rate));
                let mut y = vec![0.0; x.len()];
                p.process(&x, &mut y).expect("equal lengths");
                entries.push(entry(s, c, rate, &y));
            }
        }
    }
    GoldenFile {
        format: "rr_dr60-golden".into(),
        version: 1,
        library_version: rr_dr60::VERSION.into(),
        entries,
    }
}

/// Compares `actual` with `expected` by SHA-256. Lists every differing, missing or extra
/// entry with diagnostics.
///
/// # Errors
///
/// A report of every difference, if there are any.
pub fn compare(expected: &GoldenFile, actual: &GoldenFile) -> Result<(), String> {
    let key = |e: &GoldenEntry| (e.stimulus.clone(), e.config.clone(), e.host_rate_hz);
    let mut problems = Vec::new();
    for want in &expected.entries {
        match actual.entries.iter().find(|a| key(a) == key(want)) {
            None => problems.push(format!("missing entry {:?}", key(want))),
            Some(got) if got.sha256 != want.sha256 => {
                let first_diff = want.head.iter().zip(&got.head).position(|(a, b)| a != b);
                problems.push(format!(
                    "{}/{}/{} Hz differs: first differing head index {:?}, rms {} -> {} dBFS",
                    want.stimulus,
                    want.config,
                    want.host_rate_hz,
                    first_diff,
                    want.rms_dbfs,
                    got.rms_dbfs
                ));
            }
            Some(_) => {}
        }
    }
    for got in &actual.entries {
        if !expected.entries.iter().any(|e| key(e) == key(got)) {
            problems.push(format!("extra entry {:?}", key(got)));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

/// The committed golden file, embedded at compile time, so comparisons need no filesystem
/// access. That matters on a physical iOS device, where the source tree doesn't exist
/// (tasks.md T064). After blessing, rebuild to pick up the new file.
///
/// # Panics
///
/// If the embedded file is not valid golden JSON.
pub fn committed() -> GoldenFile {
    serde_json::from_str(include_str!("../golden/golden-v1.json"))
        .expect("valid embedded golden file")
}

/// Path of the committed golden file.
pub fn path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("golden/golden-v1.json")
}

/// The committed spec 002 AGC golden file, embedded at compile time like [`committed`].
///
/// # Panics
///
/// If the embedded file is not valid golden JSON.
pub fn committed_agc() -> GoldenFile {
    serde_json::from_str(include_str!("../golden/golden-agc-v1.json"))
        .expect("valid embedded AGC golden file")
}

/// Path of the committed spec 002 AGC golden file. Bless it only with
/// `RR_DR60_BLESS=agc cargo test -p rr_dr60_harness --test golden_agc`, with a CHANGELOG entry.
pub fn path_agc() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("golden/golden-agc-v1.json")
}

/// Loads a golden file.
///
/// # Errors
///
/// I/O or JSON errors, as text.
pub fn load(path: &std::path::Path) -> Result<GoldenFile, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Writes a golden file (pretty JSON, trailing newline). Only for deliberate re-blessing
/// (`RR_DR60_BLESS=1`), which needs a CHANGELOG entry (Constitution III).
///
/// # Errors
///
/// I/O errors, as text.
pub fn bless(path: &std::path::Path, file: &GoldenFile) -> Result<(), String> {
    let text = serde_json::to_string_pretty(file).map_err(|e| e.to_string())? + "\n";
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_head_format() {
        let e = entry("impulse", "default", 8000, &[1.0, 0.0, -0.5, 0.25]);
        assert_eq!(e.head, ["3f800000", "00000000", "bf000000", "3e800000"]);
        assert_eq!(e.n_samples, 4);
        // SHA-256 of bytes 00 00 80 3f | 00 00 00 00 | 00 00 00 bf | 00 00 80 3e.
        let bytes: Vec<u8> = [1.0f32, 0.0, -0.5, 0.25]
            .iter()
            .flat_map(|v| v.to_bits().to_le_bytes())
            .collect();
        let want = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(e.sha256, want);
        assert_eq!(e.sha256.len(), 64);
        assert_eq!(e.rms_dbfs, -4.84); // 10·log10(0.328125)
        assert_eq!(entry("x", "y", 8000, &[0.0]).rms_dbfs, -999.0);
    }

    #[test]
    fn compare_reports_differences() {
        let a = entry("impulse", "default", 8000, &[1.0, 0.0]);
        let b = entry("impulse", "default", 8000, &[1.0, 0.5]);
        let c = entry("impulse", "default", 16_000, &[1.0]);
        let file = |entries: Vec<GoldenEntry>| GoldenFile {
            format: "rr_dr60-golden".into(),
            version: 1,
            library_version: "0".into(),
            entries,
        };
        assert!(compare(&file(vec![a.clone()]), &file(vec![a.clone()])).is_ok());
        let err = compare(&file(vec![a.clone()]), &file(vec![b])).unwrap_err();
        assert!(
            err.contains("impulse/default/8000 Hz differs") && err.contains("Some(1)"),
            "{err}"
        );
        let err = compare(&file(vec![a.clone()]), &file(vec![c])).unwrap_err();
        assert!(
            err.contains("missing entry") && err.contains("extra entry"),
            "{err}"
        );
    }

    #[test]
    fn stimuli_have_contract_lengths() {
        assert_eq!(stimulus("impulse", 48_000).len(), 12_000);
        assert_eq!(stimulus("tone_1k_m20", 48_000).len(), 24_000);
        assert_eq!(stimulus("sweep_log", 44_100).len(), 44_100);
        assert_eq!(stimulus("noise_seed_0d60", 8000).len(), 4000);
        assert_eq!(stimulus("agc_step", 8000).len(), 64_000);
        assert_eq!(stimulus("agc_start_m10", 16_000).len(), 8000);
        assert_eq!(stimulus("agc_noise_burst", 8000).len(), 80_000);
    }

    #[test]
    #[should_panic(expected = "unknown stimulus")]
    fn unknown_stimulus_panics() {
        stimulus("nope", 8000);
    }

    #[test]
    fn bless_and_load_round_trip() {
        let dir = std::env::temp_dir().join(format!("rr_dr60_golden_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("g.json");
        let f = GoldenFile {
            format: "rr_dr60-golden".into(),
            version: 1,
            library_version: "0".into(),
            entries: vec![entry("impulse", "default", 8000, &[1.0])],
        };
        bless(&p, &f).unwrap();
        assert_eq!(load(&p).unwrap(), f);
        assert!(load(&dir.join("missing.json")).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
