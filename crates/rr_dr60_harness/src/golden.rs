//! Golden-file hashing, comparison and blessing (contracts/golden-format.md; FR-021; T055).
//!
//! Every stimulus here is bit-reproducible (detmath and integer PCG32 only), so a single
//! golden file serves every platform (FR-014). This module stays under the clippy R-04 ban.
//!
//! Spec 004 (the 6 kHz device rate; contracts/golden-format.md, research R-11): the generators
//! cover the seven host rates, so the three files hold 112, 42 and 63 entries.
//!
//! Spec 005 (the limiter; contracts/golden-format.md, research R-10): the three AGC stimuli are
//! replaced (`agc_onset`, `agc_rearm`, `agc_staircase`: 1 kHz at the host-grid peak phase,
//! [`stimulus::peak_phase`], so the detector reads the true peaks at every rate), the VAS
//! bursts move from −8 to −30 dBFS with the re-anchored threshold (A-022 revised; still 1 kHz
//! at phase 0, 003 R-08), `agc_only` is the host-rate "after AGC" tap and `default_vas` runs
//! the limiter and the clamp. `golden-v1.json` is unchanged (the 001 configurations have the
//! limiter bypassed and no 001 stimulus reaches full scale, R-06).

use std::fmt::Write as _;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use rr_dr60::Settings;

use crate::checks::Make;
use crate::{configs, stimulus};

/// Golden stimuli, in sorted order (contracts/golden-format.md › Stimuli).
pub const STIMULI: [&str; 4] = ["impulse", "noise_seed_0d60", "sweep_log", "tone_1k_m20"];

/// Golden configurations, in sorted order.
pub const CONFIGS: [&str; 4] = ["bypass_all", "default", "playback_only", "record_only"];

/// Spec 005 AGC golden stimuli, in sorted order (specs/005-limiter-agc/contracts/golden-format.md;
/// research R-10). The 0.4 stimuli `agc_noise_burst`, `agc_start_m10` and `agc_step` measured
/// the spec 002 compressor and are retired.
pub const AGC_STIMULI: [&str; 3] = ["agc_onset", "agc_rearm", "agc_staircase"];

/// Spec 005 AGC golden configurations, in sorted order: the limiter alone at the host-rate
/// "after AGC" tap, and the 0.5 default with the VAS off.
pub const AGC_CONFIGS: [&str; 2] = ["agc_only", "default_agc"];

/// Spec 003 VAS golden stimuli, in sorted order (specs/003-vas/contracts/golden-format.md).
pub const VAS_STIMULI: [&str; 3] = ["vas_burst_gap", "vas_noise_gaps", "vas_short_bursts"];

/// Spec 003 VAS golden configurations, in sorted order.
pub const VAS_CONFIGS: [&str; 3] = ["default_vas", "vas_mute", "vas_only"];

/// One golden entry record-then-play must reproduce (spec 006 SC-002; contracts/golden-format.md
/// › The SC-002 reproduction rule), with the settings the split paths use for it.
#[derive(Clone, Debug, PartialEq)]
pub struct Applicable {
    /// The golden file's name.
    pub file: &'static str,
    /// The stimulus name.
    pub stimulus: &'static str,
    /// The configuration name.
    pub config: &'static str,
    /// The host rate in Hz.
    pub host_rate_hz: u32,
    /// The settings for the split paths: the entry's, with the playback stage bypassed for
    /// the after-VAS-tap entries (the paths ignore the tap, 006 FR-001, FR-002).
    pub settings: Settings,
    /// The committed entry.
    pub entry: GoldenEntry,
}

/// The entries record-then-play reproduces (spec 006 SC-002; research R-09): the 154 at the
/// default tap as they are (all 112 of `golden-v1.json`, the 21 `default_agc`, the 21
/// `default_vas`), the 42 `vas_only` / `vas_mute` entries with `playback_stage_enabled =
/// false`; the 21 `agc_only` entries at the host-rate "after AGC" tap are excluded. 196 of
/// 217; the count is asserted so the check cannot skip entries silently.
///
/// # Panics
///
/// If a committed entry names an unknown stimulus or configuration, or the count is not 196.
pub fn applicable_entries() -> Vec<Applicable> {
    fn name_of(names: &[&'static str], s: &str) -> &'static str {
        names
            .iter()
            .copied()
            .find(|n| *n == s)
            .unwrap_or_else(|| panic!("unknown golden name {s:?}"))
    }
    let files: [(&'static str, GoldenFile, &[&'static str], &[&'static str]); 3] = [
        ("golden-v1.json", committed(), &STIMULI, &CONFIGS),
        (
            "golden-agc-v1.json",
            committed_agc(),
            &AGC_STIMULI,
            &AGC_CONFIGS,
        ),
        (
            "golden-vas-v1.json",
            committed_vas(),
            &VAS_STIMULI,
            &VAS_CONFIGS,
        ),
    ];
    let mut out = Vec::new();
    for (file, golden, stimuli, cfgs) in files {
        for entry in golden.entries {
            let config = name_of(cfgs, &entry.config);
            if config == "agc_only" {
                continue; // the host-rate tap never reaches the device chain
            }
            let stimulus = name_of(stimuli, &entry.stimulus);
            let mut settings = configs::settings(config, entry.host_rate_hz);
            if matches!(config, "vas_only" | "vas_mute") {
                settings.playback_stage_enabled = false; // the paths ignore the tap (006 FR-002)
            }
            out.push(Applicable {
                file,
                stimulus,
                config,
                host_rate_hz: entry.host_rate_hz,
                settings,
                entry,
            });
        }
    }
    assert_eq!(
        out.len(),
        196,
        "SC-002: 196 of 217 golden entries are applicable"
    );
    out
}

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
    /// Spec 003: the VAS events and final state. Present in VAS-file entries only; absent (and
    /// not serialized) in the spec 001 and 002 files, which therefore read unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vas: Option<VasGolden>,
}

/// The VAS part of a golden entry (specs/003-vas/contracts/golden-format.md).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VasGolden {
    /// `[output_position, input_length]` pairs, in order (splices or muted regions).
    pub events: Vec<[u64; 2]>,
    /// Whether the VAS was paused at the end of the stimulus.
    pub paused_at_end: bool,
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
        // Spec 005 (contracts/golden-format.md; research R-10): 1 kHz level steps at the
        // host-grid peak phase (R-08; the coach's MVP review: a phase-0 tone on the 6 kHz grid
        // reads 1.25 dB low). Levels are absolute (engineering target, 005 FR-016): +11 dBFS is
        // 12 dB over the −1 dBFS knee (A-028; the levels were set for the +1 dBFS knee of the
        // first design and kept at the second re-bless).
        // The onset: dead time, ceiling, then the release.
        "agc_onset" => stimulus::step_at_phase(
            1000.0,
            &[-20.0, 11.0, -20.0],
            &[0.1, 0.4, 2.0],
            stimulus::peak_phase(1000.0, fs),
            f,
        ),
        // Arming: a burst, 2 s of quiet, the burst again (the second onset is limited within
        // half a millisecond, FR-005).
        "agc_rearm" => stimulus::step_at_phase(
            1000.0,
            &[11.0, -20.0, 11.0, -20.0],
            &[0.2, 2.0, 0.2, 0.6],
            stimulus::peak_phase(1000.0, fs),
            f,
        ),
        // The knee region and the ceiling (the clipped onset in `default_agc`): −20, 0, +3, +5,
        // +8, +14 dBFS for 0.4 s each. The levels were chosen for a phase-0 tone, where 0 and
        // +2 dBFS would both read below the +1 dBFS onset on the 6 kHz grid (plan review P12);
        // at the peak phase no level is inert at any rate, and the levels are kept.
        "agc_staircase" => stimulus::staircase(
            &[-20.0, 0.0, 3.0, 5.0, 8.0, 14.0],
            0.4,
            f,
            stimulus::peak_phase(1000.0, fs),
        ),
        // Spec 003 (contracts/golden-format.md). Bursts are 1 kHz at −30 dBFS: the default
        // threshold (A-022, −40 dBFS since spec 005) + 10 dB (engineering target, 003 R-15).
        "vas_burst_gap" => stimulus::burst_gap(
            1000.0,
            -30.0,
            &[
                (true, 1.0),
                (false, 0.6),
                (true, 0.5),
                (false, 3.0),
                (true, 1.0),
                (false, 2.5),
            ],
            f,
        ),
        "vas_noise_gaps" => {
            let bursts = stimulus::tone_bursts(1000.0, -30.0, 1.0, 3.0, 2, f);
            let noise = stimulus::bandlimited_noise(0x0D60, bursts.len(), f, -70.0);
            bursts.iter().zip(&noise).map(|(b, n)| b + n).collect()
        }
        "vas_short_bursts" => {
            stimulus::short_bursts(1000.0, -30.0, 2.0, &[10.0, 30.0, 200.0], 2.0, f)
        }
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
        vas: None,
    }
}

/// [`entry`] plus the VAS events and final state (spec 003 contracts/golden-format.md).
pub fn entry_vas(
    stimulus: &str,
    config: &str,
    host_rate_hz: u32,
    output: &[f32],
    events: &[rr_dr60::VasEvent],
    paused_at_end: bool,
) -> GoldenEntry {
    let mut e = entry(stimulus, config, host_rate_hz, output);
    e.vas = Some(VasGolden {
        events: events
            .iter()
            .map(|e| [e.output_position, e.input_length])
            .collect(),
        paused_at_end,
    });
    e
}

/// Generates every spec 001 entry (4 stimuli × 4 configurations × 7 rates), each stimulus
/// processed in one block. The configurations have the limiter bypassed (spec 005 FR-016).
pub fn generate(make: Make<'_>) -> GoldenFile {
    generate_set(&STIMULI, &CONFIGS, make)
}

/// Generates every spec 005 AGC entry (3 stimuli × 2 configurations × 7 rates): `agc_only` is
/// the host-rate "after AGC" tap, so its entries have the stimulus's length and no conversion.
pub fn generate_agc(make: Make<'_>) -> GoldenFile {
    generate_set(&AGC_STIMULI, &AGC_CONFIGS, make)
}

/// Generates every spec 003 VAS entry (3 stimuli × 3 configurations × 7 rates), each stimulus
/// processed in one block with events; `n_samples` and `sha256` cover the produced samples.
pub fn generate_vas(make: Make<'_>) -> GoldenFile {
    let mut entries = Vec::new();
    for &s in &VAS_STIMULI {
        for &c in &VAS_CONFIGS {
            for rate in configs::all_rates() {
                let x = stimulus(s, rate);
                let mut p = make(configs::settings(c, rate));
                let mut y = vec![0.0; x.len()];
                let mut events = vec![rr_dr60::VasEvent::default(); p.max_events(x.len())];
                let info = p
                    .process_with_events(&x, &mut y, &mut events)
                    .expect("equal lengths");
                y.truncate(info.produced);
                events.truncate(info.events);
                entries.push(entry_vas(s, c, rate, &y, &events, info.paused));
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

fn generate_set(stimuli: &[&str], config_names: &[&str], make: Make<'_>) -> GoldenFile {
    let mut entries = Vec::new();
    for &s in stimuli {
        for &c in config_names {
            for rate in configs::all_rates() {
                let x = stimulus(s, rate);
                let mut p = make(configs::settings(c, rate));
                let mut y = vec![0.0; x.len()];
                let produced = p.process(&x, &mut y).expect("equal lengths").produced;
                y.truncate(produced);
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
                    "{}/{}/{} Hz differs: first differing head index {:?}, rms {} -> {} dBFS, {} -> {} samples",
                    want.stimulus,
                    want.config,
                    want.host_rate_hz,
                    first_diff,
                    want.rms_dbfs,
                    got.rms_dbfs,
                    want.n_samples,
                    got.n_samples
                ));
            }
            // Spec 003: the events and the final state are compared too.
            Some(got) if got.vas != want.vas => {
                problems.push(format!(
                    "{}/{}/{} Hz: VAS events or final state differ: {:?} -> {:?}",
                    want.stimulus, want.config, want.host_rate_hz, want.vas, got.vas
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

/// The committed spec 005 AGC golden file, embedded at compile time like [`committed`].
///
/// # Panics
///
/// If the embedded file is not valid golden JSON.
pub fn committed_agc() -> GoldenFile {
    serde_json::from_str(include_str!("../golden/golden-agc-v1.json"))
        .expect("valid embedded AGC golden file")
}

/// Path of the committed spec 005 AGC golden file. Bless it only with
/// `RR_DR60_BLESS=agc cargo test -p rr_dr60_harness --test golden_agc`, with a CHANGELOG entry.
pub fn path_agc() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("golden/golden-agc-v1.json")
}

/// The committed spec 003 VAS golden file, embedded at compile time like [`committed`].
///
/// # Panics
///
/// If the embedded file is not valid golden JSON.
pub fn committed_vas() -> GoldenFile {
    serde_json::from_str(include_str!("../golden/golden-vas-v1.json"))
        .expect("valid embedded VAS golden file")
}

/// Path of the committed spec 003 VAS golden file. Bless it only with
/// `RR_DR60_BLESS=vas cargo test -p rr_dr60_harness --test golden_vas`, with a CHANGELOG entry.
pub fn path_vas() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("golden/golden-vas-v1.json")
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
        // The 6 kHz identity rate (spec 004 FR-016): the same durations in device samples.
        assert_eq!(stimulus("impulse", 6000).len(), 1500);
        assert_eq!(stimulus("tone_1k_m20", 6000).len(), 3000);
        assert_eq!(stimulus("sweep_log", 6000).len(), 6000);
        assert_eq!(stimulus("noise_seed_0d60", 6000).len(), 3000);
        // Spec 005: 2.5 s, 3.0 s and 2.4 s (contracts/golden-format.md).
        assert_eq!(stimulus("agc_onset", 6000).len(), 15_000);
        assert_eq!(stimulus("agc_rearm", 6000).len(), 18_000);
        assert_eq!(stimulus("agc_staircase", 6000).len(), 14_400);
        assert_eq!(stimulus("vas_burst_gap", 6000).len(), 51_600);
        assert_eq!(stimulus("vas_noise_gaps", 6000).len(), 48_000);
        assert_eq!(stimulus("vas_short_bursts", 6000).len(), 49_440);
        assert_eq!(stimulus("impulse", 48_000).len(), 12_000);
        assert_eq!(stimulus("tone_1k_m20", 48_000).len(), 24_000);
        assert_eq!(stimulus("sweep_log", 44_100).len(), 44_100);
        assert_eq!(stimulus("noise_seed_0d60", 8000).len(), 4000);
        assert_eq!(stimulus("agc_onset", 8000).len(), 20_000);
        assert_eq!(stimulus("agc_rearm", 16_000).len(), 48_000);
        assert_eq!(stimulus("agc_staircase", 8000).len(), 19_200);
        // The AGC stimuli start at the host-grid peak phase (005 R-08): the first sample of the
        // staircase is the −20 dBFS peak at 8 kHz (phase 0 … sin(π/6) at 6 kHz).
        let a = stimulus::amplitude(-20.0) as f32;
        assert_eq!(stimulus("agc_staircase", 8000)[0], 0.0);
        assert_eq!(stimulus("agc_staircase", 8000)[2], a);
        assert!((stimulus("agc_staircase", 6000)[1] - a).abs() < 1e-7);
        // The VAS bursts are at −30 dBFS (A-022 revised): the first burst's peak.
        let v = stimulus("vas_burst_gap", 48_000);
        let peak = v[..48_000].iter().fold(0.0f32, |m, &x| m.max(x.abs()));
        assert!(
            (f64::from(peak) - stimulus::amplitude(-30.0)).abs() < 1e-6,
            "{peak}"
        );
        // Spec 003: 8.6 s, 8 s and 8.24 s (contracts/golden-format.md).
        assert_eq!(stimulus("vas_burst_gap", 8000).len(), 68_800);
        assert_eq!(stimulus("vas_noise_gaps", 8000).len(), 64_000);
        assert_eq!(stimulus("vas_short_bursts", 8000).len(), 65_920);
    }

    #[test]
    fn vas_entries_compare_events_and_round_trip() {
        let mut event = rr_dr60::VasEvent::default();
        event.output_position = 10;
        event.input_length = 20;
        let ev = [event];
        let a = entry_vas("vas_burst_gap", "vas_only", 8000, &[1.0, 0.0], &ev, false);
        let mut b = a.clone();
        b.vas.as_mut().unwrap().paused_at_end = true;
        let file = |entries: Vec<GoldenEntry>| GoldenFile {
            format: "rr_dr60-golden".into(),
            version: 1,
            library_version: "0".into(),
            entries,
        };
        assert!(compare(&file(vec![a.clone()]), &file(vec![a.clone()])).is_ok());
        let err = compare(&file(vec![a.clone()]), &file(vec![b])).unwrap_err();
        assert!(err.contains("VAS events or final state differ"), "{err}");
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains("\"events\":[[10,20]]") && json.contains("\"paused_at_end\":false"));
        assert_eq!(serde_json::from_str::<GoldenEntry>(&json).unwrap(), a);
        // Older entries have no `vas` field and serialize without one.
        let old = entry("impulse", "default", 8000, &[1.0]);
        assert!(!serde_json::to_string(&old).unwrap().contains("vas"));
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
