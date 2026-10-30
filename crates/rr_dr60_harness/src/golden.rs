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
//!
//! Spec 008 (the speech codec; specs/008-celp-codec/contracts/golden-format.md, research R-09):
//! every pre-008 configuration runs with the codec bypassed, so the three files above are
//! byte-identical (FR-021). The codec has its own file, `golden-codec-v1.json`: 4 stimuli × 3
//! configurations × 7 rates, each entry the one-pass output as before plus the record-then-play
//! recording's frame count and frame-byte digest ([`generate_codec`]); blessed once with
//! `RR_DR60_BLESS=codec cargo test -p rr_dr60_harness --test golden_codec`.

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

/// Spec 008 codec golden stimuli, in sorted order (specs/008-celp-codec/contracts/golden-format.md):
/// the two speech-like stimuli at −20 dBFS (2 s each, seed 0x0D60) and two of spec 001's.
pub const CODEC_STIMULI: [&str; 4] = [
    "noise_seed_0d60",
    "tone_1k_m20",
    "unvoiced_voiced_m20",
    "voiced_m20",
];

/// Spec 008 codec golden configurations: `configs::CODEC_CONFIGS` in the golden file's sorted
/// order (a unit test asserts the two are the same set): the codec alone at the decoder tap,
/// the codec with the VAS on (drop mode), and the real 0.8 default (every stage on).
pub const CODEC_CONFIGS: [&str; 3] = ["codec_only", "codec_only_vas", "default_codec"];

/// Spec 009 output-chain golden stimuli, in sorted order (specs/009-output-chain/contracts/
/// golden-format.md): the log sweep 20 Hz – 3 kHz at −20 dBFS, a 1 kHz tone at 0 dBFS (in the
/// `default_*` configurations the limiter and the clamp hold it at or below full scale; at
/// `default_loud`'s position it reaches the clipper about 6 dB over the ceiling), a 1 kHz tone
/// at −10 dBFS, and the voiced stimulus at −20 dBFS; 2 s each.
pub const OUTPUT_STIMULI: [&str; 4] = ["log_sweep_m20", "tone_1k_0", "tone_1k_m10", "voiced_m20"];

/// Spec 009 output-chain golden configurations in the file's sorted order (the five of
/// `configs::OUTPUT_CONFIGS` the golden file holds: `output_only_amp` is a measurement
/// configuration, not a golden one). Each carries the route and the volume position the generator sets before the
/// stimulus is processed (`output_route_volume`).
pub const GOLDEN_OUTPUT_CONFIGS: [&str; 5] = [
    "default_earphone",
    "default_loud",
    "default_output",
    "output_only_earphone",
    "output_only_speaker",
];

/// The route and the volume position of a spec 009 golden configuration
/// (contracts/golden-format.md): the speaker route at the reference position, the earphone
/// route at 1.0, and `default_loud` at the "6 dB over" position (the clipper engaged by the
/// volume, not by an input the limiter and the clamp would hold at full scale).
pub fn output_route_volume(config: &str) -> (rr_dr60::Route, f32) {
    use rr_dr60::Route;
    use rr_dr60::output::{VOLUME_6DB_OVER, VOLUME_REFERENCE};
    match config {
        "output_only_speaker" | "default_output" => (Route::Speaker, VOLUME_REFERENCE),
        "output_only_earphone" | "default_earphone" => (Route::Earphone, 1.0),
        "default_loud" => (Route::Speaker, VOLUME_6DB_OVER),
        other => panic!("golden::output_route_volume: not an output configuration {other:?}"),
    }
}

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
    /// Spec 008 (contracts/golden-format.md): the frames of the record-then-play recording of
    /// the same stimulus. Present in codec-file entries only; absent (and not serialized) in
    /// the three earlier files, which therefore read unchanged (FR-021).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frames: Option<u64>,
    /// Spec 008: SHA-256 of the recording's concatenated 10-byte frames (hex). Present with
    /// `frames`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frames_sha256: Option<String>,
    /// Spec 009 (contracts/golden-format.md): the route set before the stimulus was processed,
    /// `"speaker"` or `"earphone"`. Present in output-file entries only; absent (and not
    /// serialized) in the four earlier files, which therefore read unchanged (FR-016).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    /// Spec 009: the volume position set before the stimulus was processed. Present with
    /// `route`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<f32>,
    /// Spec 010 (contracts/golden-format.md): the count of output samples exactly equal to
    /// +1.0 or −1.0, bounded or already there (a ±1.0 square at the identity rate counts every
    /// sample). Present in
    /// output-clamp-file entries only; absent (and not serialized) in the five earlier files,
    /// which therefore read unchanged (FR-002).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n_at_bound: Option<u32>,
    /// Spec 011 (contracts/golden-format.md): the scenario's volume changes as
    /// `[sample, position]`, each at a 16-sample block boundary (the input sample, equal to the
    /// output sample: every block produced its full length, asserted). Present in
    /// volume-ramp-file entries only; absent (and not serialized) in the six earlier files,
    /// which therefore read unchanged (FR-006).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes: Option<Vec<(u64, f32)>>,
    /// Spec 012 (contracts/golden-format.md): the engine seed. Present with `counter` and
    /// `hiss_offset_db` in microphone-file entries only; absent (and not serialized) in the
    /// seven earlier files, which therefore read unchanged (FR-014).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    /// Spec 012: the recording counter the output's hiss stream is keyed to (0: the one-pass
    /// after construction; 1: the engine's second recording, FR-009).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counter: Option<u64>,
    /// Spec 012: the hiss offset, dB (FR-007).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hiss_offset_db: Option<f32>,
    /// Spec 013 (contracts/golden-format.md): the playback speed level set before
    /// `start_playback` (a schedule's first). Present in speed-file entries only; absent (and not
    /// serialized) in the eight earlier files, which therefore read unchanged (FR-006).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed_level: Option<u8>,
    /// Spec 013: a schedule's `[call index, level]`, each set before that call (calls of rate ÷
    /// 50 host samples); absent for a constant level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed_changes: Option<Vec<(u64, u8)>>,
    /// Spec 013: the last `PlayInfo::place`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<u64>,
    /// Spec 013: host samples produced before the drain (the recording's own part and the speed
    /// control's tail).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub own_length: Option<u64>,
    /// Spec 016 (contracts/api.md › Golden format): the meter read-out per block. Present in
    /// meters-file entries only; absent (and not serialized) in the nine earlier files, which
    /// therefore read unchanged (FR-012).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meters: Option<MetersGolden>,
}

/// Spec 016: the read-out rows of one entry (data-model.md › MetersGolden), each row the
/// eleven figures as `meters_checks::row` prints them (f32s as hex bit patterns).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetersGolden {
    /// The one-pass block length (host samples).
    pub block_len: u32,
    /// The one-pass read-out after each block.
    pub blocks: Vec<String>,
    /// The read-out after each `record` call (`RECORD_BLOCK` samples).
    pub record_blocks: Vec<String>,
    /// The read-out after the stop (the flush's).
    pub flush: Option<String>,
    /// The read-out after each `play` call before the drain (`PLAY_BLOCK` samples).
    pub play_blocks: Vec<String>,
    /// The read-out after each `play` call of the drain.
    pub drain_blocks: Vec<String>,
    /// A SHA-256 over the rows in place of the rows (none at v1: every stimulus is 0.5 s).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
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
        // Spec 008 (contracts/golden-format.md): the harness's synthetic speech-like stimuli at
        // −20 dBFS, 2 s, seed 0x0D60 (engineering target, 008 R-09; `us1_codec` measures the
        // 6 s versions).
        "voiced_m20" => stimulus::voiced(f, 2.0, -20.0, 0x0D60),
        // Spec 009 (contracts/golden-format.md): the output-chain stimuli, 2 s each.
        "log_sweep_m20" => stimulus::log_sweep(20.0, 3000.0, 0.1, f, secs(2.0)),
        "tone_1k_m10" => stimulus::tone(1000.0, 0.316_227_77, f, secs(2.0)),
        "tone_1k_0" => stimulus::tone(1000.0, 1.0, f, secs(2.0)),
        "unvoiced_voiced_m20" => stimulus::unvoiced_voiced(f, 2.0, -20.0, 0x0D60),
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
        frames: None,
        frames_sha256: None,
        route: None,
        volume: None,
        n_at_bound: None,
        changes: None,
        seed: None,
        counter: None,
        hiss_offset_db: None,
        speed_level: None,
        speed_changes: None,
        place: None,
        own_length: None,
        meters: None,
    }
}

/// Hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
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

/// Generates every spec 008 codec entry (4 stimuli × 3 configurations × 7 rates;
/// contracts/golden-format.md): the one-pass output in one block (with the VAS events for
/// `codec_only_vas`), then the record-then-play recording of the same stimulus (spec 006's
/// block sizes) for its frame count and frame-byte digest. The recording's playback is
/// checked against the one-pass output under FR-009's rule (the stop's silence appended to
/// the one-pass input, the output skipped by the codec delay), so a golden run that passes
/// also proves the two paths agree.
///
/// # Panics
///
/// If record-then-play and the one-pass disagree (FR-009).
pub fn generate_codec(make: Make<'_>) -> GoldenFile {
    use crate::split_checks::{self, PLAY_BLOCK, RECORD_BLOCK};
    let mut entries = Vec::new();
    for &s in &CODEC_STIMULI {
        for &c in &CODEC_CONFIGS {
            for rate in configs::all_rates() {
                let x = stimulus(s, rate);
                let settings = configs::settings(c, rate);
                let mut p = make(settings);
                let mut y = vec![0.0; x.len()];
                let mut events = vec![rr_dr60::VasEvent::default(); p.max_events(x.len())];
                let info = p
                    .process_with_events(&x, &mut y, &mut events)
                    .expect("equal lengths");
                y.truncate(info.produced);
                events.truncate(info.events);
                let mut e = if c == "codec_only_vas" {
                    entry_vas(s, c, rate, &y, &events, info.paused)
                } else {
                    entry(s, c, rate, &y)
                };

                // Record-then-play of the same stimulus: the frames and their digest.
                let mut p = make(settings);
                let run = split_checks::record_then_play(&mut p, &x, RECORD_BLOCK, PLAY_BLOCK);
                let frames = rr_dr60::__test_hooks::stored_frames(&p, run.number);
                let frame_bytes = rr_dr60::speech_codec::FRAME_BYTES as usize;
                assert_eq!(frames.len() % frame_bytes, 0);
                e.frames = Some((frames.len() / frame_bytes) as u64);
                e.frames_sha256 = Some(sha256_hex(frames));

                // FR-009: the playback equals the one-pass output of the input followed by
                // the record latency's silence, the stop's silence, the delay line's 120
                // device samples and one device sample of slack (008 research R-06; the rule
                // of `codec_checks::one_pass_equivalence`).
                let d = (rr_dr60::speech_codec::DELAY_SAMPLES * rate / 6000) as usize;
                let per_device = split_checks::host_per_device(rate);
                let pre_encoder = p.record_latency_samples() as usize - d;
                // FR-009: the delay less the look-ahead (one frame) drains the last frame.
                let drain = (rr_dr60::speech_codec::DELAY_SAMPLES
                    - rr_dr60::speech_codec::LOOKAHEAD_SAMPLES)
                    as usize;
                let tail =
                    pre_encoder + (run.stop.silence_fed_samples as usize + drain + 1) * per_device;
                let mut padded = x.clone();
                padded.extend(core::iter::repeat_n(0.0f32, tail));
                let mut q = make(settings);
                let mut want = vec![0.0; padded.len()];
                let produced = q
                    .process(&padded, &mut want)
                    .expect("equal lengths")
                    .produced;
                want.truncate(produced);
                let got = &run.pre_drain;
                assert!(
                    want.len() >= d + got.len()
                        && got
                            .iter()
                            .zip(&want[d..d + got.len()])
                            .all(|(a, b)| a.to_bits() == b.to_bits()),
                    "{s}/{c}/{rate} Hz: record-then-play differs from the one-pass (008 FR-009)"
                );
                entries.push(e);
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

/// Spec 009 FR-016 (contracts/golden-format.md; research R-12): 4 stimuli × 5 configurations ×
/// 7 rates = 140 entries, each with the route and the volume position set before the stimulus
/// is processed and the one-pass output's SHA; for the three `default_*` configurations the
/// generator also asserts that record-then-play of the same stimulus at the same route and
/// position equals the one-pass output under spec 008 FR-009's rule (the equivalence is
/// asserted, not stored).
pub fn generate_output(make: Make<'_>) -> GoldenFile {
    use crate::split_checks::{self, PLAY_BLOCK, RECORD_BLOCK};
    let mut entries = Vec::new();
    for &s in &OUTPUT_STIMULI {
        for &c in &GOLDEN_OUTPUT_CONFIGS {
            let (route, volume) = output_route_volume(c);
            for rate in configs::all_rates() {
                let x = stimulus(s, rate);
                let settings = configs::settings(c, rate);
                let one_pass = |x: &[f32]| {
                    let mut p = make(settings);
                    p.set_route(route);
                    p.set_volume(volume).expect("a valid position");
                    let mut y = vec![0.0; x.len()];
                    let produced = p.process(x, &mut y).expect("equal lengths").produced;
                    y.truncate(produced);
                    y
                };
                let y = one_pass(&x);
                let mut e = entry(s, c, rate, &y);
                e.route = Some(
                    match route {
                        rr_dr60::Route::Earphone => "earphone",
                        _ => "speaker",
                    }
                    .into(),
                );
                e.volume = Some(volume);
                if c.starts_with("default_") {
                    let mut p = make(settings);
                    p.set_route(route);
                    p.set_volume(volume).expect("a valid position");
                    let run = split_checks::record_then_play(&mut p, &x, RECORD_BLOCK, PLAY_BLOCK);
                    let d = (rr_dr60::speech_codec::DELAY_SAMPLES * rate / 6000) as usize;
                    let per_device = split_checks::host_per_device(rate);
                    let pre_encoder = p.record_latency_samples() as usize - d;
                    let drain = (rr_dr60::speech_codec::DELAY_SAMPLES
                        - rr_dr60::speech_codec::LOOKAHEAD_SAMPLES)
                        as usize;
                    let tail = pre_encoder
                        + (run.stop.silence_fed_samples as usize + drain + 1) * per_device;
                    let mut padded = x.clone();
                    padded.extend(core::iter::repeat_n(0.0f32, tail));
                    let want = one_pass(&padded);
                    let got = &run.pre_drain;
                    assert!(
                        want.len() >= d + got.len()
                            && got
                                .iter()
                                .zip(&want[d..d + got.len()])
                                .all(|(a, b)| a.to_bits() == b.to_bits()),
                        "{s}/{c}/{rate} Hz: record-then-play differs from the one-pass with the chain on (009 FR-010)"
                    );
                }
                entries.push(e);
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
            // Spec 008: the recording's frame count and frame-byte digest are compared too.
            Some(got) if (got.frames, &got.frames_sha256) != (want.frames, &want.frames_sha256) => {
                problems.push(format!(
                    "{}/{}/{} Hz: frames differ: {:?} -> {:?} frames, digest {:?} -> {:?}",
                    want.stimulus,
                    want.config,
                    want.host_rate_hz,
                    want.frames,
                    got.frames,
                    want.frames_sha256,
                    got.frames_sha256
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

/// The committed spec 008 codec golden file, embedded at compile time like [`committed`].
///
/// # Panics
///
/// If the embedded file is not valid golden JSON.
pub fn committed_codec() -> GoldenFile {
    serde_json::from_str(include_str!("../golden/golden-codec-v1.json"))
        .expect("valid embedded codec golden file")
}

/// Spec 010 output-clamp golden stimuli, in sorted order (specs/010-output-clamp/contracts/
/// golden-format.md): the 1 kHz sine of amplitude 1.5 (the primary stimulus), the full-scale
/// square (a half-period of round(rate ÷ 2000) samples) and the 1 kHz tone at 0 dBFS; 2 s each,
/// made by `output_clamp_checks::stimulus_by_name`.
pub const OUTPUT_CLAMP_STIMULI: [&str; 3] = ["sine_1k_1p5", "square_1k_fs", "tone_1k_0"];

/// Spec 010 output-clamp golden configurations in the file's sorted order
/// (`configs::OUTPUT_CLAMP_CONFIGS`; the route and position from
/// `configs::output_clamp_route_volume`).
pub const GOLDEN_OUTPUT_CLAMP_CONFIGS: [&str; 6] = [
    "default_earphone_clamp",
    "default_output_clamp",
    "default_output_ref_clamp",
    "output_clamp_only",
    "output_only_earphone_clamp",
    "output_only_speaker_clamp",
];

/// Spec 010 FR-010 (contracts/golden-format.md): the output-clamp golden file, 3 stimuli × 6
/// configurations × 7 rates = 126 entries with the route, the position, the one-pass output's
/// SHA and `n_at_bound`. With `bless` the generator also asserts, for every entry, the identity
/// against the option-off twin (`on == clamp(off)` sample for sample), zero twin samples beyond
/// the bound for `default_output_ref_clamp` on the two tones, and record-then-play equal to the
/// one-pass for the three `default_*` configurations (spec 008 FR-009's rule); a compare run
/// does one one-pass per entry.
pub fn generate_output_clamp(make: Make<'_>, bless: bool) -> GoldenFile {
    use crate::output_clamp_checks::{self, Expect};
    use crate::split_checks::{self, PLAY_BLOCK, RECORD_BLOCK};
    let mut entries = Vec::new();
    for &s in &OUTPUT_CLAMP_STIMULI {
        for &c in &GOLDEN_OUTPUT_CLAMP_CONFIGS {
            let (route, volume) = configs::output_clamp_route_volume(c);
            for rate in configs::all_rates() {
                let x = output_clamp_checks::stimulus_by_name(s, rate, 2.0);
                let settings = configs::settings(c, rate);
                let one_pass = |x: &[f32]| {
                    let mut p = make(settings);
                    p.set_route(route);
                    p.set_volume(volume).expect("a valid position");
                    let mut y = vec![0.0; x.len()];
                    let produced = p.process(x, &mut y).expect("equal lengths").produced;
                    y.truncate(produced);
                    y
                };
                let y = one_pass(&x);
                let mut e = entry(s, c, rate, &y);
                e.route = Some(
                    match route {
                        rr_dr60::Route::Earphone => "earphone",
                        _ => "speaker",
                    }
                    .into(),
                );
                e.volume = Some(volume);
                e.n_at_bound = Some(y.iter().filter(|v| v.abs() == 1.0).count() as u32);
                if bless {
                    let expect = if c == "default_output_ref_clamp" && s != "square_1k_fs" {
                        Expect::Within
                    } else {
                        Expect::Report
                    };
                    let rows = output_clamp_checks::identity(make, rate, c, s, 2.0, expect);
                    crate::checks::assert_all(&rows);
                    if c.starts_with("default_") {
                        let mut p = make(settings);
                        p.set_route(route);
                        p.set_volume(volume).expect("a valid position");
                        let run =
                            split_checks::record_then_play(&mut p, &x, RECORD_BLOCK, PLAY_BLOCK);
                        let d = (rr_dr60::speech_codec::DELAY_SAMPLES * rate / 6000) as usize;
                        let per_device = split_checks::host_per_device(rate);
                        let pre_encoder = p.record_latency_samples() as usize - d;
                        let drain = (rr_dr60::speech_codec::DELAY_SAMPLES
                            - rr_dr60::speech_codec::LOOKAHEAD_SAMPLES)
                            as usize;
                        let tail = pre_encoder
                            + (run.stop.silence_fed_samples as usize + drain + 1) * per_device;
                        let mut padded = x.clone();
                        padded.extend(core::iter::repeat_n(0.0f32, tail));
                        let want = one_pass(&padded);
                        let got = &run.pre_drain;
                        assert!(
                            want.len() >= d + got.len()
                                && got
                                    .iter()
                                    .zip(&want[d..d + got.len()])
                                    .all(|(a, b)| a.to_bits() == b.to_bits()),
                            "{s}/{c}/{rate} Hz: record-then-play differs from the one-pass with the option on (010 FR-006)"
                        );
                    }
                }
                entries.push(e);
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

/// Spec 011 volume-ramp golden scenarios in sorted order (contracts/golden-format.md).
pub const VOLUME_RAMP_SCENARIOS: [&str; 5] = [
    "ramp_down",
    "ramp_drag",
    "ramp_retarget",
    "ramp_to_zero",
    "ramp_up",
];

/// Spec 011 volume-ramp golden configurations in sorted order (contracts/golden-format.md).
pub const GOLDEN_VOLUME_RAMP_CONFIGS: [&str; 4] = [
    "default_earphone",
    "default_output",
    "stage11_only_earphone",
    "stage11_only_speaker",
];

/// A scenario's starting position and changes at a host rate (contracts/golden-format.md:
/// c = ⌊rate ÷ 2⌋₁₆, N = 0.02 × rate, G = 16 · round(rate ÷ 960)).
pub fn volume_ramp_scenario(scenario: &str, rate: u32) -> (f32, Vec<(usize, f32)>) {
    use crate::volume_ramp_checks::{drag_gap, drag_positions, n_of};
    let c = rate as usize / 2 / 16 * 16;
    match scenario {
        "ramp_up" => (0.1, vec![(c, 1.0)]),
        "ramp_down" => (1.0, vec![(c, 0.1)]),
        "ramp_to_zero" => (1.0, vec![(c, 0.0)]),
        "ramp_retarget" => (0.1, vec![(c, 1.0), (c + n_of(rate) / 2 / 16 * 16, 0.4)]),
        "ramp_drag" => (
            0.1,
            drag_positions()
                .iter()
                .enumerate()
                .map(|(j, &p)| (c + j * drag_gap(rate), p))
                .collect(),
        ),
        other => panic!("unknown scenario {other:?}"),
    }
}

/// Spec 011 FR-012 (contracts/golden-format.md): the volume-ramp golden file, 5 scenarios × 4
/// configurations × 7 rates = 140 entries, 1 s each through the one-pass in 16-sample blocks
/// (every block producing its full length, asserted), with the route, the starting position
/// and the changes. With `bless` the generator also asserts, for the two stage-11-only
/// configurations, the splice identity: bit-identical to a run held at the starting position
/// before the first change and to a run held at the last position from the last change's
/// transition sample N (research R-05 (a)).
pub fn generate_volume_ramp(make: Make<'_>, bless: bool) -> GoldenFile {
    use crate::volume_ramp_checks::{Blocks, n_of, run_with_changes, same};
    let mut entries = Vec::new();
    for &s in &VOLUME_RAMP_SCENARIOS {
        for &c in &GOLDEN_VOLUME_RAMP_CONFIGS {
            let route = match c {
                "default_earphone" | "stage11_only_earphone" => rr_dr60::Route::Earphone,
                _ => rr_dr60::Route::Speaker,
            };
            for rate in configs::all_rates() {
                let fs = f64::from(rate);
                let settings = configs::settings(c, rate);
                let x = match c {
                    "stage11_only_speaker" => {
                        stimulus::tone(1000.0, stimulus::amplitude(-30.0), fs, rate as usize)
                    }
                    "stage11_only_earphone" => stimulus::tone(1000.0, 1.0, fs, rate as usize),
                    _ => stimulus::voiced(fs, 1.0, -20.0, 0x0D60),
                };
                let (start, changes) = volume_ramp_scenario(s, rate);
                let y = run_with_changes(
                    make,
                    settings,
                    route,
                    start,
                    &changes,
                    &x,
                    Blocks::Fixed(16),
                );
                if bless && c.starts_with("stage11_only") {
                    let held = |p: f32| {
                        run_with_changes(make, settings, route, p, &[], &x, Blocks::Fixed(16))
                    };
                    let (first, _) = changes[0];
                    let (last, p_last) = *changes.last().unwrap();
                    let from_n = last + n_of(rate) - 1;
                    assert!(
                        same(&y[..first], &held(start)[..first])
                            && same(&y[from_n..], &held(p_last)[from_n..]),
                        "{s}/{c}/{rate} Hz: the splice identity (011 R-05)"
                    );
                }
                let mut e = entry(s, c, rate, &y);
                e.route = Some(
                    match route {
                        rr_dr60::Route::Earphone => "earphone",
                        _ => "speaker",
                    }
                    .into(),
                );
                e.volume = Some(start);
                e.changes = Some(changes.iter().map(|&(at, p)| (at as u64, p)).collect());
                entries.push(e);
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

/// A spec 013 speed-only run: its configuration name, schedule, after-the-end change and call
/// size.
type SpeedRun = (String, Vec<(usize, u8)>, Option<u8>, usize);

/// Spec 013 (contracts/golden-format.md): the levels other than 3 the speed file plays.
pub const SPEED_LEVELS: [u8; 4] = [1, 2, 4, 5];

/// Spec 013: the two schedules as `[call index, level]` (calls of rate ÷ 50 host samples):
/// A into and out of level 3 and between two non-3 levels; B between non-3 levels, then a
/// change to level 3 after the place reached the end (appended at run time).
pub fn speed_schedule(name: &str) -> Vec<(usize, u8)> {
    match name {
        "a" => vec![(0, 3), (20, 1), (45, 3), (65, 5), (90, 4)],
        "b" => vec![(0, 2), (30, 5), (55, 1)],
        // Played in calls of 121 (speed-only set only): call 1 starts at place 121·3/5, not a
        // whole stored sample, so the switch into level 3 is deferred (FR-007).
        "c" => vec![(0, 1), (1, 3), (3, 5), (4, 3)],
        other => panic!("unknown schedule {other:?}"),
    }
}

/// A spec 013 playback: every sample to `finished` in calls of `call`, the schedule applied
/// before its calls; with `after_end`, a change to `after_end` before the first call after the
/// place reached the recording's length `k`. Returns the output, the schedule as applied, the
/// last place, the frames decoded and the first call that reported the place at K.
fn speed_play(
    p: &mut rr_dr60::Pipeline,
    call: usize,
    schedule: &[(usize, u8)],
    after_end: Option<u8>,
    k: u64,
) -> (Vec<f32>, Vec<(u64, u8)>, u64, u64, usize) {
    let mut applied: Vec<(u64, u8)> = schedule.iter().map(|&(c, l)| (c as u64, l)).collect();
    if let Some(&(_, l)) = schedule.iter().find(|(c, _)| *c == 0) {
        p.set_playback_speed(l).unwrap();
    }
    p.start_playback(1).unwrap();
    let mut buf = vec![0.0f32; call];
    let (mut out, mut frames, mut place, mut pending) = (Vec::new(), 0u64, 0u64, after_end);
    let mut reached = usize::MAX; // the first call that reported the place at K
    for c in 0.. {
        for &(at, l) in schedule {
            if at == c && c > 0 {
                p.set_playback_speed(l).unwrap();
            }
        }
        if place >= k {
            if let Some(l) = pending.take() {
                p.set_playback_speed(l).unwrap();
                applied.push((c as u64, l));
            }
        }
        let info = p.play(&mut buf).unwrap();
        out.extend_from_slice(&buf[..info.produced]);
        frames += info.frames as u64;
        place = info.place;
        if place >= k && reached == usize::MAX {
            reached = c;
        }
        if info.finished {
            break;
        }
    }
    (out, applied, place, frames, reached)
}

/// Spec 013 FR-013 (contracts/golden-format.md): the playback-speed file, 196 entries. With
/// `bless`, asserts the lengths against the length helper, schedule B's after-the-end change as
/// having no effect, and the speed-only entries against the independent reference.
pub fn generate_speed(make: Make<'_>, bless: bool) -> GoldenFile {
    use crate::speed_checks::{mismatches, reference_output, stored_stream};
    let mut entries = Vec::new();
    let mut push = |stim: &str,
                    config: String,
                    rate: u32,
                    y: &[f32],
                    level: u8,
                    changes: Option<Vec<(u64, u8)>>,
                    place: u64,
                    own: u64,
                    frames: Option<u64>,
                    seed: Option<u64>| {
        let mut e = entry(stim, &config, rate, y);
        e.speed_level = Some(level);
        e.speed_changes = changes;
        e.place = Some(place);
        e.own_length = Some(own);
        e.frames = frames;
        if seed.is_some() {
            e.seed = seed;
            e.counter = Some(0); // 012 FR-009: the engine's first recording
        }
        entries.push(e);
    };
    for rate in configs::all_rates() {
        let call = (rate / 50) as usize;
        for stim in ["tone_1k_m20", "voiced_m20"] {
            let x = stimulus(stim, rate);
            for (config, codec) in [("speed_default", true), ("speed_default_no_codec", false)] {
                let settings = configs::settings(config, rate);
                let mut p = make(settings);
                p.start_recording().unwrap();
                let _ = p.record(&x, &mut []).unwrap();
                let _ = p.stop_recording(&mut []).unwrap();
                let k = p.recording_length(1).unwrap();
                let drain = u64::from(p.drain_samples());
                let routes: &[(rr_dr60::Route, &str)] = if codec {
                    &[
                        (rr_dr60::Route::Speaker, "speaker"),
                        (rr_dr60::Route::Earphone, "earphone"),
                    ]
                } else {
                    &[(rr_dr60::Route::Speaker, "speaker")]
                };
                for &(route, rname) in routes {
                    p.set_route(route);
                    for level in SPEED_LEVELS {
                        let (y, _, place, frames, _) =
                            speed_play(&mut p, call, &[(0, level)], None, k);
                        let own = y.len() as u64 - drain;
                        if bless {
                            let helper = p.playback_length_samples(1, level).unwrap();
                            assert_eq!(
                                y.len() as u64,
                                helper,
                                "{stim} {config} {rate} level {level}: the helper (013 FR-009)"
                            );
                            assert_eq!(
                                place, k,
                                "{stim} {config} {rate} level {level}: the place (013 FR-009)"
                            );
                        }
                        let name = if codec {
                            format!("speed_default_{rname}_l{level}")
                        } else {
                            format!("speed_no_codec_speaker_l{level}")
                        };
                        push(
                            stim,
                            name,
                            rate,
                            &y,
                            level,
                            None,
                            place,
                            own,
                            codec.then_some(frames),
                            Some(settings.seed),
                        );
                    }
                }
                if codec && stim == "voiced_m20" {
                    p.set_route(rr_dr60::Route::Speaker);
                    for sched in ["a", "b"] {
                        let schedule = speed_schedule(sched);
                        let after = (sched == "b").then_some(3u8);
                        let (y, applied, place, frames, reached) =
                            speed_play(&mut p, call, &schedule, after, k);
                        if bless {
                            // Every scheduled change lands before the place reaches K (review B1).
                            assert!(
                                schedule.iter().all(|&(c, _)| c < reached),
                                "{rate} {sched}: a change after the end"
                            );
                        }
                        if bless && after.is_some() {
                            let (z, _, _, _, _) = speed_play(&mut p, call, &schedule, None, k);
                            assert!(
                                mismatches(&y, &z) == 0,
                                "{rate}: schedule B's change after the end changed the output (013 FR-007)"
                            );
                        }
                        let own = y.len() as u64 - drain;
                        push(
                            stim,
                            format!("speed_sched_{sched}"),
                            rate,
                            &y,
                            schedule[0].1,
                            Some(applied),
                            place,
                            own,
                            Some(frames),
                            Some(settings.seed),
                        );
                    }
                }
            }
        }
    }
    // (d) The speed-only configuration at 6 kHz on imported stimuli.
    let only = configs::settings("speed_only", 6000);
    for (stim, x) in [
        ("stored_tone_1k", stimulus::stored_tone(1000.0, 0.5, 12_000)),
        ("stored_voiced", stimulus::stored_voiced(2.0)),
    ] {
        let (mut p, n) = configs::stored_recording(make, only, &x);
        assert_eq!(n, 1);
        let k = p.recording_length(1).unwrap();
        let stream = stored_stream(&mut p, 1);
        let mut runs: Vec<SpeedRun> = SPEED_LEVELS
            .iter()
            .map(|&l| (format!("speed_only_l{l}"), vec![(0, l)], None, 120))
            .collect();
        runs.push(("speed_only_sched_a".into(), speed_schedule("a"), None, 120));
        runs.push((
            "speed_only_sched_b".into(),
            speed_schedule("b"),
            Some(3),
            120,
        ));
        runs.push(("speed_only_sched_c".into(), speed_schedule("c"), None, 121));
        for (name, schedule, after, call) in runs {
            let (y, applied, place, _, reached) = speed_play(&mut p, call, &schedule, after, k);
            if bless {
                assert!(
                    schedule.iter().all(|&(c, _)| c < reached),
                    "{stim} {name}: a change after the end (review B1)"
                );
                let sched: Vec<(usize, u8)> =
                    applied.iter().map(|&(c, l)| (c as usize, l)).collect();
                let (want, _) = reference_output(&stream, call, &sched);
                assert_eq!(
                    mismatches(&y, &want),
                    0,
                    "{stim} {name}: the reference (013 R-08)"
                );
            }
            let changes = (schedule.len() > 1).then_some(applied);
            push(
                stim,
                name,
                6000,
                &y,
                schedule[0].1,
                changes,
                place,
                y.len() as u64,
                None,
                None,
            );
        }
    }
    entries.sort_by(|a, b| {
        (a.stimulus.as_str(), a.config.as_str(), a.host_rate_hz).cmp(&(
            b.stimulus.as_str(),
            b.config.as_str(),
            b.host_rate_hz,
        ))
    });
    GoldenFile {
        format: "rr_dr60-golden".into(),
        version: 1,
        library_version: rr_dr60::VERSION.into(),
        entries,
    }
}

/// The committed spec 013 playback-speed golden file.
///
/// # Panics
///
/// If the file is missing or unreadable.
pub fn committed_speed() -> GoldenFile {
    load(&path_speed()).expect("golden-speed-v1.json")
}

/// The path of `golden/golden-speed-v1.json`.
pub fn path_speed() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("golden/golden-speed-v1.json")
}

/// Spec 012 microphone golden stimuli in sorted order (contracts/golden-format.md › Stimuli).
pub const MIC_STIMULI: [&str; 4] = ["loud_tone", "silence_1s", "very_loud_tone", "voiced_1s"];

/// Spec 012 microphone golden configurations in sorted order (data-model.md › Harness
/// configurations); the second-recording family `default_mic_rec1` follows `default_mic_quiet`.
pub const GOLDEN_MIC_CONFIGS: [&str; 5] = [
    "default_mic",
    "default_mic_quiet",
    "mic_capsule_only",
    "mic_only",
    "mic_only_quiet",
];

/// A spec 012 microphone golden stimulus at host rate `rate` (contracts/golden-format.md): the
/// tones are 1 kHz for 0.25 s starting on the script's clipper-input peak phase
/// ([`crate::mic_checks::START_PHASE`]).
///
/// # Panics
///
/// On an unknown stimulus name.
pub fn mic_stimulus(name: &str, rate: u32) -> Vec<f32> {
    let fs = f64::from(rate);
    let quarter = rate as usize / 4;
    match name {
        "silence_1s" => stimulus::silence(rate as usize),
        "voiced_1s" => stimulus::voiced(fs, 1.0, -20.0, 7),
        "loud_tone" => crate::mic_checks::peak_tone(rate, 12.0, quarter),
        "very_loud_tone" => crate::mic_checks::peak_tone(rate, 24.0, quarter),
        other => panic!("unknown stimulus {other:?}"),
    }
}

/// The one-pass output of `x` through `settings`, one block.
fn one_pass(make: Make<'_>, settings: Settings, x: &[f32]) -> Vec<f32> {
    let mut p = make(settings);
    let mut y = vec![0.0; x.len()];
    let produced = p.process(x, &mut y).expect("equal lengths").produced;
    y.truncate(produced);
    y
}

/// [`entry`] plus the spec 012 fields.
fn entry_mic(
    s: &str,
    c: &str,
    rate: u32,
    settings: &Settings,
    counter: u64,
    y: &[f32],
) -> GoldenEntry {
    let mut e = entry(s, c, rate, y);
    e.seed = Some(settings.seed);
    e.counter = Some(counter);
    e.hiss_offset_db = Some(settings.mic.hiss_offset_db);
    e
}

/// Spec 012 FR-014 (contracts/golden-format.md): the microphone golden file, 4 stimuli × 5
/// configurations × 7 rates one-pass entries (one block each, keyed to counter 0) plus the
/// `default_mic_rec1` family, `silence_1s` recorded as the engine's second recording (at
/// counter 1) and played to the end, at the seven rates: 147 entries with the seed, the counter and the
/// hiss offset, sorted by configuration, stimulus and rate. With `bless` the generator also
/// asserts that every `mic_only_quiet` entry on silence is exact zeros, that every `mic_only`
/// entry on silence reads the script's hiss band level within ±0.5 dB, that every `default_mic`
/// entry equals a record-then-play of the same input under spec 008's coordinates (FR-009's
/// rule), and that every `default_mic_rec1` playback differs from the matching counter-0 entry.
pub fn generate_mic(make: Make<'_>, bless: bool) -> GoldenFile {
    use crate::mic_checks::{HISS_BAND_DBFS, HISS_TOL_DB};
    use crate::split_checks::{self, PLAY_BLOCK, RECORD_BLOCK};
    use rr_dr60::speech_codec;
    let mut entries = Vec::new();
    for &c in &GOLDEN_MIC_CONFIGS {
        for &s in &MIC_STIMULI {
            for rate in configs::all_rates() {
                let settings = configs::settings(c, rate);
                let x = mic_stimulus(s, rate);
                let y = one_pass(make, settings, &x);
                if bless && s == "silence_1s" && c == "mic_only_quiet" {
                    assert!(
                        y.iter().all(|v| v.to_bits() == 0),
                        "{s}/{c}/{rate} Hz: the hiss off on silence is exact zeros (012 FR-007)"
                    );
                }
                if bless && s == "silence_1s" && c == "mic_only" {
                    let got = crate::analysis::band_level_aes17(&y, f64::from(rate), 300.0, 3000.0);
                    assert!(
                        (got - HISS_BAND_DBFS).abs() <= HISS_TOL_DB, // SC-001
                        "{s}/{c}/{rate} Hz: hiss band {got} dBFS re {HISS_BAND_DBFS} (012 SC-001)"
                    );
                }
                if bless && c == "default_mic" {
                    // Spec 008 FR-009's coordinates (mic_checks::equivalence): the one-pass of
                    // the input, the pre-encoder latency and the stop's frames of silence,
                    // skipped by the codec delay; its prefix is this entry (causality).
                    let mut p = make(configs::memory_capacity(settings, 60, 9));
                    let run = split_checks::record_then_play(&mut p, &x, RECORD_BLOCK, PLAY_BLOCK);
                    let d = (speech_codec::DELAY_SAMPLES * rate / 6000) as usize;
                    let per_device = split_checks::host_per_device(rate);
                    let pre_encoder = p.record_latency_samples() as usize - d;
                    let drain =
                        (speech_codec::DELAY_SAMPLES - speech_codec::LOOKAHEAD_SAMPLES) as usize;
                    let tail = pre_encoder
                        + (run.stop.silence_fed_samples as usize + drain + 1) * per_device;
                    let mut padded = x.clone();
                    padded.extend(core::iter::repeat_n(0.0f32, tail));
                    let want = one_pass(make, settings, &padded);
                    let got = &run.pre_drain;
                    let bits_eq = |a: &[f32], b: &[f32]| {
                        a.len() == b.len()
                            && a.iter().zip(b).all(|(u, v)| u.to_bits() == v.to_bits())
                    };
                    assert!(
                        want.len() >= d + got.len()
                            && bits_eq(got, &want[d..d + got.len()])
                            && bits_eq(&y, &want[..y.len()]),
                        "{s}/{c}/{rate} Hz: the first recording differs from the one-pass (012 SC-008)"
                    );
                }
                entries.push(entry_mic(s, c, rate, &settings, 0, &y));
            }
        }
        if c == "default_mic_quiet" {
            for rate in configs::all_rates() {
                entries.push(mic_rec1(make, rate, bless, &entries));
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

/// One `default_mic_rec1` entry: a 64-sample recording of silence (counter 0), then
/// `silence_1s` recorded (counter 1) and played to the end, drain included. With `bless`,
/// asserts that the playback before the drain differs from the matching counter-0
/// `default_mic` entry (in `earlier`) skipped by the codec delay (FR-009).
fn mic_rec1(make: Make<'_>, rate: u32, bless: bool, earlier: &[GoldenEntry]) -> GoldenEntry {
    use crate::split_checks::{self, PLAY_BLOCK, RECORD_BLOCK};
    let settings = configs::memory_capacity(configs::settings("default_mic", rate), 60, 9);
    let x = mic_stimulus("silence_1s", rate);
    let mut p = make(settings);
    p.start_recording().expect("room for a recording");
    let _ = p
        .record(&stimulus::silence(64), &mut [])
        .expect("recording");
    let _ = p.stop_recording(&mut []).expect("recording");
    let run = split_checks::record_then_play(&mut p, &x, RECORD_BLOCK, PLAY_BLOCK);
    assert_eq!(run.number, 2, "the second recording");
    if bless {
        let counter0 = one_pass(make, settings, &x);
        let d = (rr_dr60::speech_codec::DELAY_SAMPLES * rate / 6000) as usize;
        let want = earlier
            .iter()
            .find(|e| {
                e.config == "default_mic" && e.stimulus == "silence_1s" && e.host_rate_hz == rate
            })
            .expect("the counter-0 entry precedes the family");
        assert_eq!(
            entry("silence_1s", "default_mic", rate, &counter0).sha256,
            want.sha256
        );
        let n = run.pre_drain.len().min(counter0.len().saturating_sub(d));
        assert!(
            run.pre_drain[..n]
                .iter()
                .zip(&counter0[d..d + n])
                .any(|(a, b)| a.to_bits() != b.to_bits()),
            "default_mic_rec1/{rate} Hz: the second recording's hiss equals counter 0's (012 FR-009)"
        );
    }
    let mut y = run.pre_drain;
    y.extend_from_slice(&run.drain);
    entry_mic("silence_1s", "default_mic_rec1", rate, &settings, 1, &y)
}

/// The committed spec 012 microphone golden file.
///
/// # Panics
///
/// If the embedded file is not valid JSON.
pub fn committed_mic() -> GoldenFile {
    serde_json::from_str(include_str!("../golden/golden-mic-v1.json"))
        .expect("valid embedded microphone golden file")
}

/// Path of the spec 012 microphone golden file. Bless it only with
/// `RR_DR60_BLESS=mic cargo test -p rr_dr60_harness --test golden_mic`, with a CHANGELOG entry
/// (012 FR-014).
pub fn path_mic() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("golden/golden-mic-v1.json")
}

/// Spec 016 meters golden stimuli (data-model.md › MetersGolden), 0.5 s each, sorted.
pub const METERS_STIMULI: [&str; 4] = [
    "clamp_square",
    "limiter_step",
    "vas_burst",
    "voiced_m20_half",
];

/// Spec 016 meters golden configurations, sorted.
pub const GOLDEN_METERS_CONFIGS: [&str; 6] = [
    "limiter_clamp_011",
    "meters_default",
    "meters_default_clamp",
    "meters_short_hang",
    "meters_short_hang_mute",
    "output_only_amp",
];

/// A spec 016 stimulus at `rate` (research R-10).
///
/// # Panics
///
/// On an unknown name.
pub fn meters_stimulus(name: &str, rate: u32) -> Vec<f32> {
    use crate::meters_checks as mc;
    let f = f64::from(rate);
    match name {
        "voiced_m20_half" => stimulus::voiced(f, 0.5, -20.0, 0x0D60),
        "vas_burst" => mc::vas_burst(rate),
        "limiter_step" => stimulus::tone_at_phase(
            1000.0,
            9.0, // 10 dB over the −1 dBFS knee (A-028)
            stimulus::peak_phase(1000.0, rate),
            f,
            (0.5 * f) as usize,
        ),
        "clamp_square" => mc::clamp_square(rate, 0.5),
        other => panic!("unknown stimulus {other:?}"),
    }
}

/// Spec 016 FR-012 (contracts/api.md › Golden format): 4 stimuli × 6 configurations × 7 rates,
/// each entry with the one-pass output's digest (the audio) and the read-out rows of the
/// one-pass, the record calls, the flush, the play calls and the drain. With `bless` the tap
/// and count identities are asserted on every route-output entry (the limiter-input,
/// reconstruction and route peaks against clamp-off twins at the taps; the output-clamp count
/// against the clamp-off twin), and each short-hang entry must hold a paused row and an event.
pub fn generate_meters(make: Make<'_>, bless: bool) -> GoldenFile {
    use crate::meters_checks::{self as mc, ONE_PASS_BLOCK, peak_of, row};
    use crate::split_checks::{self, PLAY_BLOCK, RECORD_BLOCK};
    use rr_dr60::{Tap, VasEvent};
    let mut entries = Vec::new();
    for &s in &METERS_STIMULI {
        for &c in &GOLDEN_METERS_CONFIGS {
            for rate in configs::all_rates() {
                let settings = configs::settings(c, rate);
                let x = meters_stimulus(s, rate);
                let mut p = make(settings);
                let blocks = mc::one_pass_blocks(&mut p, &x, ONE_PASS_BLOCK);
                let y: Vec<f32> = blocks
                    .iter()
                    .flat_map(|b| b.output.iter().copied())
                    .collect();
                let mut e = entry(s, c, rate, &y);
                e.seed = Some(settings.seed);
                e.counter = Some(0);
                if bless {
                    let tag = format!("{s}/{c}/{rate} Hz");
                    let twin = |tap: Tap, clamp: bool| {
                        let mut t = settings;
                        t.tap = tap;
                        t.output.clamp_enabled = clamp;
                        let mut q = make(t);
                        mc::one_pass_blocks(&mut q, &x, ONE_PASS_BLOCK)
                    };
                    if settings.tap == Tap::RouteOutput {
                        let amp = twin(Tap::AfterMicAmp, false);
                        let play = twin(Tap::AfterPlayback, false);
                        let route = twin(Tap::RouteOutput, false);
                        for (i, b) in blocks.iter().enumerate() {
                            assert_eq!(
                                b.meters.limiter_input_peak.to_bits(),
                                peak_of(&amp[i].output).to_bits(),
                                "{tag} block {i}: the limiter-input peak vs the AfterMicAmp tap"
                            );
                            assert_eq!(
                                b.meters.reconstruction_peak.to_bits(),
                                peak_of(&play[i].output).to_bits(),
                                "{tag} block {i}: the reconstruction peak vs the AfterPlayback tap"
                            );
                            assert_eq!(
                                b.meters.route_peak.to_bits(),
                                peak_of(&route[i].output).to_bits(),
                                "{tag} block {i}: the route peak vs the clamp-off RouteOutput tap"
                            );
                            let want = mc::clamp_diff_count(&b.output, &route[i].output);
                            assert_eq!(
                                b.meters.output_clamp_count, want,
                                "{tag} block {i}: the output-clamp count vs the clamp-off twin"
                            );
                        }
                    }
                    if c.starts_with("meters_short_hang") && s == "vas_burst" {
                        assert!(
                            blocks.iter().any(|b| b.meters.vas_paused),
                            "{tag}: no paused row (016 research R-10)"
                        );
                        assert!(
                            blocks.iter().any(|b| !b.events.is_empty()),
                            "{tag}: no event (016 research R-10)"
                        );
                    }
                }
                // Record-then-play in the harness's block sizes.
                let mut q = make(settings);
                q.start_recording().expect("idle");
                let mut ev = vec![VasEvent::default(); q.max_record_events(RECORD_BLOCK)];
                let mut record_blocks = Vec::new();
                for chunk in x.chunks(RECORD_BLOCK) {
                    let info = q.record(chunk, &mut ev).expect("recording");
                    assert!(info.stopped.is_none(), "memory full");
                    record_blocks.push(row(&q.meters()));
                }
                let before_stop = q.meters();
                let _ = q.stop_recording(&mut ev).expect("recording");
                // The flush's row exactly when the engine finalized one (FR-001): the read-out moved.
                let flush = (q.meters() != before_stop).then(|| row(&q.meters()));
                let number = q.memory_status().count;
                let own =
                    split_checks::playback_len(q.recording_length(number).expect("kept"), rate);
                q.start_playback(number).expect("a recording");
                let (mut play_blocks, mut drain_blocks) = (Vec::new(), Vec::new());
                let mut produced = 0u64;
                loop {
                    let mut buf = vec![0.0f32; PLAY_BLOCK];
                    let info = q.play(&mut buf).expect("playing");
                    produced += info.produced as u64;
                    if produced > own {
                        drain_blocks.push(row(&q.meters()));
                    } else {
                        play_blocks.push(row(&q.meters()));
                    }
                    if info.finished {
                        break;
                    }
                }
                e.meters = Some(MetersGolden {
                    block_len: ONE_PASS_BLOCK as u32,
                    blocks: blocks.iter().map(|b| row(&b.meters)).collect(),
                    record_blocks,
                    flush,
                    play_blocks,
                    drain_blocks,
                    digest: None,
                });
                entries.push(e);
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

/// The committed spec 016 meters golden file.
///
/// # Panics
///
/// If the embedded file is not valid golden JSON.
pub fn committed_meters() -> GoldenFile {
    serde_json::from_str(include_str!("../golden/golden-meters-v1.json"))
        .expect("valid embedded meters golden file")
}

/// Path of the spec 016 meters golden file. Bless it only with
/// `RR_DR60_BLESS=meters cargo test -p rr_dr60_harness --test golden_meters`, with a CHANGELOG
/// entry.
pub fn path_meters() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("golden/golden-meters-v1.json")
}

/// The committed spec 011 volume-ramp golden file.
///
/// # Panics
///
/// If the embedded file is not valid JSON.
pub fn committed_volume_ramp() -> GoldenFile {
    serde_json::from_str(include_str!("../golden/golden-volume-ramp-v1.json"))
        .expect("valid embedded volume-ramp golden file")
}

/// Path of the spec 011 volume-ramp golden file. Bless it only with
/// `RR_DR60_BLESS=volume_ramp cargo test -p rr_dr60_harness --test golden_volume_ramp`, with a
/// CHANGELOG entry (011 FR-012).
pub fn path_volume_ramp() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("golden/golden-volume-ramp-v1.json")
}

/// The committed spec 010 output-clamp golden file.
///
/// # Panics
///
/// If the embedded file is not valid JSON.
pub fn committed_output_clamp() -> GoldenFile {
    serde_json::from_str(include_str!("../golden/golden-output-clamp-v1.json"))
        .expect("valid embedded output-clamp golden file")
}

/// Path of the spec 010 output-clamp golden file. Bless it only with
/// `RR_DR60_BLESS=output_clamp cargo test -p rr_dr60_harness --test golden_output_clamp`, with a
/// CHANGELOG entry (010 FR-010).
pub fn path_output_clamp() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("golden/golden-output-clamp-v1.json")
}

/// The committed spec 009 output-chain golden file.
///
/// # Panics
///
/// If the embedded file is not valid JSON.
pub fn committed_output() -> GoldenFile {
    serde_json::from_str(include_str!("../golden/golden-output-v1.json"))
        .expect("valid embedded output golden file")
}

/// Path of the spec 009 output-chain golden file. Bless it only with
/// `RR_DR60_BLESS=output cargo test -p rr_dr60_harness --test golden_output`, with a CHANGELOG
/// entry (009 FR-016: a later change to the chain's output is a model change).
pub fn path_output() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("golden/golden-output-v1.json")
}

/// Path of the spec 008 codec golden file. Bless it only with
/// `RR_DR60_BLESS=codec cargo test -p rr_dr60_harness --test golden_codec`, with a CHANGELOG
/// entry (008 FR-021: a later change to the frame bytes is a model change).
pub fn path_codec() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("golden/golden-codec-v1.json")
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
        // Spec 008: 2 s each (contracts/golden-format.md).
        assert_eq!(stimulus("voiced_m20", 6000).len(), 12_000);
        assert_eq!(stimulus("unvoiced_voiced_m20", 6000).len(), 12_000);
        assert_eq!(stimulus("voiced_m20", 48_000).len(), 96_000);
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
    fn codec_entries_compare_frames_and_round_trip() {
        // Spec 008 (contracts/golden-format.md): the frame count and digest are compared, and
        // the older files serialize without the two fields (FR-021).
        let mut a = entry("voiced_m20", "codec_only", 8000, &[1.0, 0.0]);
        a.frames = Some(100);
        a.frames_sha256 = Some(sha256_hex(&[0u8; 10]));
        let mut b = a.clone();
        b.frames_sha256 = Some(sha256_hex(&[1u8; 10]));
        let file = |entries: Vec<GoldenEntry>| GoldenFile {
            format: "rr_dr60-golden".into(),
            version: 1,
            library_version: "0".into(),
            entries,
        };
        assert!(compare(&file(vec![a.clone()]), &file(vec![a.clone()])).is_ok());
        let err = compare(&file(vec![a.clone()]), &file(vec![b])).unwrap_err();
        assert!(err.contains("frames differ"), "{err}");
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains("\"frames\":100"));
        assert_eq!(serde_json::from_str::<GoldenEntry>(&json).unwrap(), a);
        let old = entry("impulse", "default", 8000, &[1.0]);
        assert!(!serde_json::to_string(&old).unwrap().contains("frames"));
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(CODEC_STIMULI.len() * CODEC_CONFIGS.len() * 7, 84);
        // The golden file's sorted order of `configs::CODEC_CONFIGS` (the same set).
        let mut sorted = configs::CODEC_CONFIGS;
        sorted.sort_unstable();
        assert_eq!(sorted, CODEC_CONFIGS);
        assert!(CODEC_STIMULI.windows(2).all(|w| w[0] < w[1]));
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
