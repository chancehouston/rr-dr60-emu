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
    /// Spec 015: the schedule and the change log of a scheduled run. Present in changes-file
    /// entries only; absent (and not serialized) in the ten earlier files (FR-009).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timed_changes: Option<ChangesGolden>,
    /// Spec 017: the schedule, the change log and the final input position of a record-side
    /// scheduled run. Present in record-changes-file entries only; absent (and not serialized)
    /// in the eleven earlier files (FR-008).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_changes: Option<RecordChangesGolden>,
    /// Spec 018 (research R-09): the battery value, the route, the last call's two battery
    /// meters and the change log of a run. Present in battery-file entries only; absent (and not
    /// serialized) in the twelve earlier files (FR-005).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub battery: Option<BatteryGolden>,
    /// Spec 020: the processing record of the entry's recording. Present in the record file's
    /// entries only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_contract: Option<RecordContractGolden>,
    /// Spec 021 (research R-09): the live bypass schedule, the log's switch entries, the
    /// produced count and whether the endpoint identities are bit-exact for the entry. Present
    /// in bypass-file entries only; absent (and not serialized) in the earlier files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bypass: Option<BypassGolden>,
    /// Spec 023 (research R-12): the pitch-kept run: the start mode, the schedule, the level and
    /// mode entries, the produced count and the per-frame shift digest. Present in
    /// pitch-kept-file entries only; absent (and not serialized) in the earlier files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pitch: Option<PitchGolden>,
}

/// Spec 023 (research R-12): the pitch-kept part of a golden entry.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PitchGolden {
    /// The playback speed mode set before `start_playback` (0 pitch with speed, 1 pitch kept).
    pub mode: u32,
    /// The playback speed level set before `start_playback`.
    pub level: u8,
    /// The schedule: (position in host output samples, control, value, tag); empty for a
    /// constant run.
    pub schedule: Vec<(u64, u32, f64, u64)>,
    /// The level and mode entries of the log: (position, control, before, after, tag, stream).
    pub log: Vec<(u64, u32, f64, f64, u64, u32)>,
    /// The produced count.
    pub produced: u64,
    /// The time-scaler's frames (the shift trace's length; 0 when no grid ran).
    pub frames: u64,
    /// FNV-1a 64 over the shifts as i32 LE (hex).
    pub shifts_fnv1a64: String,
}

/// Spec 020: the record part of a golden entry (plan review, coach 11): the record's text with
/// the `library_version` and `stage.N.model_version` values replaced by `<current>` (those
/// lines are asserted against the library and the manifest at compare time, so the golden moves
/// only when the record's data moves) and its own checksum line dropped (it covers the raw
/// versions).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordContractGolden {
    /// The normalized record, one string per line.
    pub lines: Vec<String>,
    /// The stored form's digest as the record names it.
    pub digest: String,
    /// The event and change rows.
    pub event_count: u64,
    /// The change rows.
    pub change_count: u64,
}

/// Spec 018: the battery part of a golden entry (research R-09).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BatteryGolden {
    /// The battery value set before the stimulus (none for the twin without the setter).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volts: Option<f32>,
    /// `speaker` or `earphone`.
    pub route: String,
    /// `meters_checks::battery_row` of the last call: the amplifier clip count and the ceiling
    /// factor's bits.
    pub battery_row: String,
    /// Every battery change entry the calls reported, in order: `position control before after
    /// tag` (the doubles as hex bit patterns).
    pub log: Vec<String>,
    /// The largest `changes` of one call.
    pub max_per_call: u32,
}

/// Spec 017: the record-side part of a golden entry (research R-12).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordChangesGolden {
    /// `record` or `one_pass`.
    pub stream: String,
    /// The changes scheduled, in submission order: `position control value tag`.
    pub schedule: Vec<String>,
    /// Every entry the calls reported, in order: `position control before after tag` (the
    /// doubles as hex bit patterns).
    pub log: Vec<String>,
    /// The largest `changes` of one call.
    pub max_per_call: u32,
    /// The record stream's final input position (the stop's report); none on the one-pass.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_position: Option<u64>,
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

/// Spec 015 (research R-10): the schedule and the log of one scheduled run. A schedule row is
/// `position control value tag` (the plain setters as `plain control value`), a log row
/// `position control before after tag`, the doubles in the shortest round-trip form.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChangesGolden {
    /// `playback` or `one_pass`.
    pub stream: String,
    /// The block partition the entry's output was produced at.
    pub partition: String,
    /// The changes scheduled or set, in submission order.
    pub schedule: Vec<String>,
    /// Every entry the calls reported, in order.
    pub log: Vec<String>,
    /// The largest `changes` of one call.
    pub max_per_call: u32,
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
        timed_changes: None,
        record_changes: None,
        battery: None,
        record_contract: None,
        bypass: None,
        pitch: None,
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
    if bless {
        speed_re_bless_gate();
    }
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
                            // Every scheduled change lands before the place reaches K (review B1)
                            // and its 20 ms blend completes before it (spec 019 research R-07:
                            // a call is 20 ms, so the next call is still before K).
                            assert!(
                                schedule.iter().all(|&(c, _)| c + 1 < reached),
                                "{rate} {sched}: a change or its blend after the end"
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
                // The blend of 120 outputs is one call of 120 (or 121): the next call before K.
                assert!(
                    schedule.iter().all(|&(c, _)| c + 1 < reached),
                    "{stim} {name}: a change or its blend after the end (review B1; 019 R-07)"
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
/// The SHA-256 of `golden-speed-v1.json` as version 0.18 committed it (spec 013's bless): the
/// 0.18 gate runs while the committed file is still that one and skips itself afterwards.
const GOLDEN_SPEED_0_18_SHA256: &str =
    "d36b0a75625e67b11cb513d6b80be29b8827e3f60ee737c084eebdbc6e28dce2";
/// The SHA-256 of `golden-changes-v1.json` as version 0.18 committed it (spec 015's bless).
const GOLDEN_CHANGES_0_18_SHA256: &str =
    "e6f1565c425a068b5a51ad39d07d15cad994847318407a5e2e9f8566c94b88de";

/// Whether the committed golden `name` is still version 0.18's (its SHA-256 equals `sha`).
fn committed_is_0_18(name: &str, sha: &str) -> bool {
    let bytes = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("golden")
            .join(name),
    )
    .expect("the committed golden");
    let hex: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    hex == sha
}

/// Spec 019 research R-07, the re-bless gate (hardware B2): with `fade_step` on (no fade, the
/// request applied at once: version 0.18's switch) the generator regenerates the committed
/// `golden-speed-v1.json` bit for bit, entry by entry, before the file is re-blessed for the
/// crossfade; afterwards every entry without a schedule must still equal the committed one
/// (the constant-level identity, FR-003). The 0.18 check runs only while the committed file is
/// still 0.18's (its SHA-256) and skips itself on a later re-bless.
pub fn speed_re_bless_gate() {
    let committed = committed_speed();
    if committed_is_0_18("golden-speed-v1.json", GOLDEN_SPEED_0_18_SHA256) {
        let stepped = generate_speed(&crate::crossfade_checks::stepped, false);
        assert_eq!(stepped.entries.len(), committed.entries.len());
        for (c, a) in committed.entries.iter().zip(&stepped.entries) {
            assert_eq!(
                (&c.stimulus, &c.config, c.host_rate_hz, &c.sha256),
                (&a.stimulus, &a.config, a.host_rate_hz, &a.sha256),
                "the 0.18 gate: the stepped build does not regenerate the committed entry"
            );
        }
    }
    let actual = generate_speed(&crate::checks::standard, false);
    for (c, a) in committed.entries.iter().zip(&actual.entries) {
        if c.speed_changes.is_none() {
            assert_eq!(
                c, a,
                "{}/{}/{} Hz: a constant-level entry moved (019 FR-003)",
                c.stimulus, c.config, c.host_rate_hz
            );
        }
    }
}

/// The committed spec 013 speed golden file (re-blessed once by spec 019).
///
/// # Panics
///
/// If the embedded file is not valid golden JSON.
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

/// Spec 015 (research R-10): the scenarios of `golden-changes-v1.json`, each a name and the
/// stream it runs on; the changes are built per rate by [`changes_scenario`].
pub const CHANGES_SCENARIOS: [(&str, &str); 13] = [
    ("volume_inside", "one_pass"),
    ("route_inside", "one_pass"),
    ("two_at_one", "one_pass"),
    ("passed_position", "one_pass"),
    ("scheduled_no_op", "one_pass"),
    ("plain_setters", "one_pass"),
    ("cross_stream_setter", "one_pass"),
    ("cross_stream_scheduled", "one_pass"),
    ("volume_inside", "playback"),
    ("route_inside", "playback"),
    ("speed_inside", "playback"),
    ("drain_and_post_latch", "playback"),
    ("sixty_four_plus_three", "playback"),
];

/// The spec 015 golden stimulus: 1 s of the voiced stimulus at `rate` (seed 0x015).
pub fn changes_stimulus(rate: u32) -> Vec<f32> {
    stimulus::voiced(f64::from(rate), 1.0, -20.0, 0x015)
}

/// One scheduled run of a scenario at `partition` (research R-10): the output, the schedule
/// rows and the log. `make` builds the engine; `config` names the configuration.
///
/// # Panics
///
/// On an unknown scenario or a refused call.
pub fn changes_scenario(
    make: Make<'_>,
    scenario: &str,
    stream: &str,
    config: &str,
    rate: u32,
    partition: crate::changes_checks::Partition,
) -> (Vec<f32>, ChangesGolden) {
    use crate::changes_checks::{self as cc, Change, Source};
    use rr_dr60::{Route, Stream};
    let x = changes_stimulus(rate);
    let r = u64::from(rate);
    let settings = configs::settings(config, rate);
    let mut p = make(settings);
    let mut rows = Vec::new();
    let row = |c: &Change| format!("{} {} {:?} {}", c.position, c.control, c.value, c.tag);
    let (st, source_is_playback) = if stream == "playback" {
        (Stream::Playback, true)
    } else {
        (Stream::OnePass, false)
    };
    let number = if source_is_playback {
        p.start_recording().expect("idle");
        let _ = p.record(&x, &mut []).expect("record");
        let _ = p.stop_recording(&mut []).expect("stop");
        1
    } else {
        0
    };
    let mut changes: Vec<Change> = Vec::new();
    match scenario {
        "volume_inside" => changes.push(Change::volume(st, r / 3 + 7, 0.9, 1)),
        "route_inside" => changes.push(Change::route(st, r / 3 + 7, Route::Earphone, 1)),
        "speed_inside" => {
            changes.push(Change::speed(r / 4 + 7, 5, 1));
            changes.push(Change::speed(r / 2 + 7, 1, 2));
            changes.push(Change::speed(r / 2 + 9, 3, 3)); // a deferred switch into 3, kept
        }
        "two_at_one" => {
            changes.push(Change::volume(st, r / 3 + 7, 0.3, 1));
            changes.push(Change::volume(st, r / 3 + 7, 0.8, 2));
        }
        "passed_position" => {
            // 3000 samples first, then a change at 5 (passed) and one at the position.
            let _ = cc::run(&mut p, Source::OnePass(&x[..3000]), partition);
            let pos = p.stream_position(Stream::OnePass).expect("stream");
            changes.push(Change::volume(st, 5, 0.9, 1));
            changes.push(Change::route(st, pos, Route::Earphone, 2));
            let run = cc::run_scheduled(&mut p, Source::OnePass(&x[3000..]), &changes, partition);
            rows.extend(changes.iter().map(row));
            return (
                run.out,
                ChangesGolden {
                    stream: stream.into(),
                    partition: format!("{partition:?}"),
                    schedule: rows,
                    log: run.log.iter().map(log_row).collect(),
                    max_per_call: run.max_per_call as u32,
                },
            );
        }
        "scheduled_no_op" => {
            changes.push(Change::volume(
                st,
                r / 3,
                rr_dr60::output::VOLUME_REFERENCE,
                1,
            ));
            changes.push(Change::route(st, r / 3 + 1, Route::Speaker, 2));
        }
        "plain_setters" => {
            p.set_volume(0.7).expect("valid");
            p.set_route(Route::Earphone);
            rows.push("plain 1 0.7 0".into());
            rows.push("plain 0 1 0".into());
        }
        "cross_stream_setter" | "cross_stream_scheduled" => {
            // A playback with a volume moved during it, then the one-pass run.
            p.start_recording().expect("idle");
            let _ = p.record(&x[..x.len() / 2], &mut []).expect("record");
            let _ = p.stop_recording(&mut []).expect("stop");
            let _ = cc::run(&mut p, Source::OnePass(&x[..1024]), partition);
            p.start_playback(1).expect("start");
            let mut buf = vec![0.0f32; 1024];
            let _ = p.play(&mut buf).expect("play");
            if scenario == "cross_stream_setter" {
                p.set_volume(0.6).expect("valid");
                rows.push("plain 1 0.6 0 (during the playback)".into());
            } else {
                let c = Change::volume(Stream::Playback, 1100, 0.6, 21);
                cc::schedule_all(&mut p, &[c]);
                rows.push(format!("{} (on the playback stream)", row(&c)));
            }
            let _ = p.play(&mut buf).expect("play");
            p.stop_playback().expect("stop");
            let run = cc::run(&mut p, Source::OnePass(&x[1024..]), partition);
            return (
                run.out,
                ChangesGolden {
                    stream: stream.into(),
                    partition: format!("{partition:?}"),
                    schedule: rows,
                    log: run.log.iter().map(log_row).collect(),
                    max_per_call: run.max_per_call as u32,
                },
            );
        }
        "drain_and_post_latch" => {
            let total = p.playback_length_samples(number, 3).expect("length");
            let drain = u64::from(p.drain_samples());
            changes.push(Change::speed(10, 5, 1)); // level 5: the own part shortens
            let own5 = p.playback_length_samples(number, 5).expect("length") - drain;
            changes.push(Change::speed(own5.saturating_sub(5), 2, 2)); // inside the tail, after the latch
            changes.push(Change::volume(st, total - drain / 2, 0.9, 3)); // in the drain (level 3's clock is longer; still inside)
        }
        "sixty_four_plus_three" => {
            for i in 0..64u64 {
                changes.push(Change::volume(
                    st,
                    5000 + i * 7,
                    0.2 + 0.01 * (i % 50) as f32,
                    i + 1,
                ));
            }
            p.set_route(Route::Earphone);
            p.set_volume(0.9).expect("valid");
            p.set_playback_speed(2).expect("valid");
            rows.push("plain 0 1 0".into());
            rows.push("plain 1 0.9 0".into());
            rows.push("plain 2 2 0".into());
        }
        other => panic!("unknown changes scenario {other}"),
    }
    rows.extend(changes.iter().map(row));
    let source = if source_is_playback {
        Source::Playback(number)
    } else {
        Source::OnePass(&x)
    };
    let run = cc::run_scheduled(&mut p, source, &changes, partition);
    (
        run.out,
        ChangesGolden {
            stream: stream.into(),
            partition: format!("{partition:?}"),
            schedule: rows,
            log: run.log.iter().map(log_row).collect(),
            max_per_call: run.max_per_call as u32,
        },
    )
}

fn log_row(e: &rr_dr60::ChangeEntry) -> String {
    format!(
        "{} {} {:?} {:?} {}",
        e.position, e.control, e.before, e.after, e.tag
    )
}

/// Spec 015 FR-009 (research R-10): the changes golden: every scenario at the seven rates in
/// `default_output` (and `speed_inside` also with the speech codec on), each entry the output's
/// digest and the full log at 64-sample blocks. At bless time the partition identity (the
/// irregular partition gives the same output and log) and the block-size-1 setter oracle are
/// asserted on every entry whose schedule the oracle can replay (every scenario but the
/// cross-stream and passed-position ones, which schedule after a prefix run).
pub fn generate_changes(make: Make<'_>, bless: bool) -> GoldenFile {
    use crate::changes_checks::{self as cc, Partition, Source};
    if bless {
        changes_re_bless_gate();
    }
    let mut entries = Vec::new();
    for &(scenario, stream) in &CHANGES_SCENARIOS {
        let configs_for: &[&str] = if scenario == "speed_inside" {
            &["default_output", "default_codec"]
        } else {
            &["default_output"]
        };
        for &config in configs_for {
            for rate in configs::all_rates() {
                let (y, g) =
                    changes_scenario(make, scenario, stream, config, rate, Partition::Fixed(64));
                let name = format!("{scenario}/{stream}");
                let mut e = entry(&name, config, rate, &y);
                if bless {
                    let tag = format!("{name}/{config}/{rate} Hz");
                    let (y2, g2) = changes_scenario(
                        make,
                        scenario,
                        stream,
                        config,
                        rate,
                        Partition::Irregular,
                    );
                    assert!(cc::same(&y, &y2), "{tag}: the partition identity (output)");
                    assert_eq!(g.log, g2.log, "{tag}: the partition identity (log)");
                    if !scenario.starts_with("cross_stream") && scenario != "passed_position" {
                        // The setter oracle replays the schedule rows (the plain rows first).
                        let x = changes_stimulus(rate);
                        let settings = configs::settings(config, rate);
                        let mut q = make(settings);
                        let (source, _) = if stream == "playback" {
                            q.start_recording().expect("idle");
                            let _ = q.record(&x, &mut []).expect("record");
                            let _ = q.stop_recording(&mut []).expect("stop");
                            (Source::Playback(1), ())
                        } else {
                            (Source::OnePass(&x), ())
                        };
                        let mut changes = Vec::new();
                        for row in &g.schedule {
                            let parts: Vec<&str> = row.split(' ').collect();
                            if parts[0] == "plain" {
                                let control: u32 = parts[1].parse().expect("control");
                                let value: f64 = parts[2].parse().expect("value");
                                cc::Change {
                                    stream: if stream == "playback" {
                                        rr_dr60::Stream::Playback
                                    } else {
                                        rr_dr60::Stream::OnePass
                                    },
                                    position: 0,
                                    control,
                                    value,
                                    tag: 0,
                                }
                                .apply_plain(&mut q);
                            } else {
                                changes.push(cc::Change {
                                    stream: if stream == "playback" {
                                        rr_dr60::Stream::Playback
                                    } else {
                                        rr_dr60::Stream::OnePass
                                    },
                                    position: parts[0].parse().expect("position"),
                                    control: parts[1].parse().expect("control"),
                                    value: parts[2].parse().expect("value"),
                                    tag: parts[3].parse().expect("tag"),
                                });
                            }
                        }
                        let want = cc::setter_oracle(&mut q, source, &changes);
                        assert!(
                            cc::same(&y, &want),
                            "{tag}: the block-size-1 setter oracle at {:?}",
                            cc::first_diff(&y, &want)
                        );
                    }
                }
                e.timed_changes = Some(g);
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

/// Spec 019 research R-07 (hardware B2, B3): before `golden-changes-v1.json` is re-blessed for
/// the crossfade, the stepped build regenerates the committed entries with a speed change bit
/// for bit (while the committed file is still 0.18's, by its SHA-256), every other entry equals the
/// committed one, and the `speed_inside` entries' produced counts moved by the wait (longer: the
/// request into level 3 lands at the fade's end instead of a few outputs after the request).
pub fn changes_re_bless_gate() {
    let committed = committed_changes();
    let actual = generate_changes(&crate::checks::standard, false);
    assert_eq!(committed.entries.len(), actual.entries.len());
    let run_gate = committed_is_0_18("golden-changes-v1.json", GOLDEN_CHANGES_0_18_SHA256);
    // An entry carries a speed change when a schedule row names the speed control
    // (`position control value tag`; `speed_inside` and the drain / post-latch scenarios).
    let speed = crate::changes_checks::speed_control().to_string();
    let has_speed = |e: &GoldenEntry| {
        e.timed_changes.as_ref().is_some_and(|g| {
            g.schedule
                .iter()
                .any(|row| row.split(' ').nth(1) == Some(speed.as_str()))
        })
    };
    if run_gate {
        let stepped = generate_changes(&crate::crossfade_checks::stepped, false);
        for (c, a) in committed.entries.iter().zip(&stepped.entries) {
            if has_speed(c) {
                assert_eq!(
                    (&c.sha256, c.n_samples),
                    (&a.sha256, a.n_samples),
                    "the 0.18 gate: {}/{}/{} Hz",
                    c.stimulus,
                    c.config,
                    c.host_rate_hz
                );
            }
        }
    }
    for (c, a) in committed.entries.iter().zip(&actual.entries) {
        if has_speed(c) {
            if c.stimulus.starts_with("speed_inside") {
                // Two requests one device sample apart: the second waits (hardware B3).
                assert!(
                    a.n_samples >= c.n_samples,
                    "{}/{}/{} Hz: the wait shortened the playback (019 R-07)",
                    c.stimulus,
                    c.config,
                    c.host_rate_hz
                );
            } else {
                assert_eq!(
                    c.n_samples, a.n_samples,
                    "{}/{}/{} Hz: a lone speed change moved the length (019 FR-006)",
                    c.stimulus, c.config, c.host_rate_hz
                );
            }
        } else {
            assert_eq!(
                c, a,
                "{}/{}/{} Hz: an entry without a speed change moved (019 FR-003)",
                c.stimulus, c.config, c.host_rate_hz
            );
        }
    }
}

/// The committed spec 015 changes golden file.
///
/// # Panics
///
/// If the embedded file is not valid golden JSON.
pub fn committed_changes() -> GoldenFile {
    serde_json::from_str(include_str!("../golden/golden-changes-v1.json"))
        .expect("valid embedded changes golden file")
}

/// Path of the spec 015 changes golden file. Bless it only with
/// `RR_DR60_BLESS=changes cargo test -p rr_dr60_harness --test golden_changes`, with a CHANGELOG
/// entry.
pub fn path_changes() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("golden/golden-changes-v1.json")
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

// ---- Spec 017: the record-changes golden (research R-12) --------------------------------------

/// The spec 017 golden scenarios: `(scenario, stream)`. Per record-side value a change at
/// input 0 (`zero`) and one inside a block (`inside`) on both streams, in drop and mute mode;
/// then the hang shortening on a burst train, the stop case (a change inside the flush) and the
/// cross-stream landing (a value landed on the recorder reaching the one-pass).
pub const RECORD_CHANGES_VALUES: [&str; 8] = [
    "vas.sensitivity",
    "vas.threshold_dbfs",
    "vas.hang_ms",
    "vas.onset_ms",
    "agc.knee_dbfs",
    "agc.attack_ms",
    "agc.release_ms",
    "mic.hiss_offset_db",
];

/// The spec 017 golden configurations.
pub const RECORD_CHANGES_CONFIGS: [&str; 3] = [
    "record_default",
    "record_default_mute",
    "record_default_codec",
];

/// The spec 017 golden stimulus: 0.5 s of the voiced stimulus at `rate` (seed 0x017), or the
/// burst train of the hang scenario (60 ms on, 140 ms off, 1.4 s).
pub fn record_changes_stimulus(scenario: &str, rate: u32) -> Vec<f32> {
    let f = f64::from(rate);
    if scenario == "hang_shortening" {
        let segments: Vec<(bool, f64)> =
            (0..7).flat_map(|_| [(true, 0.06), (false, 0.14)]).collect();
        stimulus::burst_gap(1000.0, -20.0, &segments, f)
    } else {
        stimulus::voiced(f, 0.5, -20.0, 0x017)
    }
}

/// The representative value of each record-side control in the golden (one per identifier).
pub fn record_changes_value(identifier: &str) -> crate::record_checks::Value {
    use crate::record_checks::Value;
    match identifier {
        "vas.sensitivity" => Value::Sensitivity(5),
        "vas.threshold_dbfs" => Value::Threshold(-50.0),
        "vas.hang_ms" => Value::Hang(50.0),
        "vas.onset_ms" => Value::Onset(5.0),
        "agc.knee_dbfs" => Value::Knee(-5.0),
        "agc.attack_ms" => Value::Attack(10.0),
        "agc.release_ms" => Value::Release(400.0),
        "mic.hiss_offset_db" => Value::Hiss(6.0),
        other => panic!("unknown record-side value {other:?}"),
    }
}

fn record_log_rows(log: &[rr_dr60::ChangeEntry]) -> Vec<String> {
    log.iter()
        .map(|e| {
            format!(
                "{} {} {:016x} {:016x} {}",
                e.position,
                e.control,
                e.before.to_bits(),
                e.after.to_bits(),
                e.tag
            )
        })
        .collect()
}

fn record_schedule_rows(changes: &[crate::changes_checks::Change]) -> Vec<String> {
    changes
        .iter()
        .map(|c| format!("{} {} {:?} {}", c.position, c.control, c.value, c.tag))
        .collect()
}

/// One scheduled run of a spec 017 scenario (research R-12): the "output" (the recorder's
/// stored samples, or its frame bytes as the digest with the codec on; the one-pass's output)
/// and the rows. `make` builds the engine.
///
/// # Panics
///
/// On an unknown scenario or a refused call.
pub fn record_changes_scenario(
    make: Make<'_>,
    scenario: &str,
    value: &str,
    stream: &str,
    config: &str,
    rate: u32,
    partition: crate::changes_checks::Partition,
) -> (Vec<f32>, Vec<u8>, RecordChangesGolden) {
    use crate::changes_checks as cc;
    use crate::record_checks as rc;
    use rr_dr60::Stream;
    let x = record_changes_stimulus(scenario, rate);
    let settings = configs::settings(config, rate);
    let v = record_changes_value(value);
    let p0 = u64::from(rate / 3) + 7;
    let st = if stream == "record" {
        Stream::Record
    } else {
        Stream::OnePass
    };
    let mut p = make(settings);
    let changes: Vec<cc::Change> = match scenario {
        "zero" => vec![rc::change(st, 0, v, 1)],
        "inside" => vec![rc::change(st, p0, v, 1)],
        "hang_shortening" => vec![rc::change(st, p0, rc::Value::Hang(50.0), 1)],
        "stop_case" => {
            // A change inside the flush (the recorder only): half-way through the record latency.
            let latency = u64::from(p.record_latency_samples());
            vec![
                rc::change(Stream::Record, 100, rc::Value::Knee(-5.0), 1),
                rc::change(
                    Stream::Record,
                    x.len() as u64 + latency / 2,
                    rc::Value::Release(400.0),
                    2,
                ),
            ]
        }
        "cross_stream" => {
            // A value landed on the recorder, then the one-pass's first call lands it too.
            let r = rc::record_run(
                &mut p,
                &x,
                partition,
                &[rc::change(Stream::Record, 0, v, 7)],
            );
            assert_eq!(r.log.len(), 1);
            let run = rc::one_pass_run(&mut p, &x, partition, &[]);
            return (
                run.out,
                Vec::new(),
                RecordChangesGolden {
                    stream: stream.into(),
                    schedule: vec![format!(
                        "0 {} {:?} 7 (on the record stream)",
                        v.control(),
                        v.as_f64()
                    )],
                    log: record_log_rows(&run.log),
                    max_per_call: run.max_per_call as u32,
                    final_position: None,
                },
            );
        }
        other => panic!("unknown scenario {other:?}"),
    };
    if st == Stream::Record {
        let r = rc::record_run(&mut p, &x, partition, &changes);
        (
            r.samples.clone(),
            r.frames.clone(),
            RecordChangesGolden {
                stream: stream.into(),
                schedule: record_schedule_rows(&changes),
                log: record_log_rows(&r.log),
                max_per_call: r.max_per_call as u32,
                final_position: Some(r.stop.input_samples_total),
            },
        )
    } else {
        let run = rc::one_pass_run(&mut p, &x, partition, &changes);
        (
            run.out,
            Vec::new(),
            RecordChangesGolden {
                stream: stream.into(),
                schedule: record_schedule_rows(&changes),
                log: record_log_rows(&run.log),
                max_per_call: run.max_per_call as u32,
                final_position: None,
            },
        )
    }
}

/// Spec 017 FR-008 (research R-12): per value, the `zero` and `inside` scenarios on both
/// streams in the three configurations, then the hang shortening (both streams, drop and
/// mute), the stop case (the recorder) and the cross-stream landing (the one-pass), at seven
/// rates; each entry the output's digest (the stored samples, or the frame bytes' SHA with the
/// codec on) and the full log. With `bless` the input-0 identity (a `zero` entry against the
/// constant configuration) and the partition identity (an `inside` entry at the irregular
/// partition) are asserted on every entry they apply to.
pub fn generate_record_changes(make: Make<'_>, bless: bool) -> GoldenFile {
    use crate::changes_checks::{self as cc, Partition};
    use crate::record_checks as rc;
    use rr_dr60::Stream;
    let mut entries = Vec::new();
    let mut push = |scenario: &str, value: &str, stream: &str, config: &str, rate: u32| {
        let (y, frames, g) = record_changes_scenario(
            make,
            scenario,
            value,
            stream,
            config,
            rate,
            Partition::Fixed(64),
        );
        let name = format!("{scenario}/{value}/{stream}");
        let mut e = entry(&name, config, rate, &y);
        if !frames.is_empty() {
            let digest = Sha256::digest(&frames);
            e.frames = Some(frames.len() as u64 / 10);
            e.frames_sha256 = Some(digest.iter().fold(String::new(), |mut s, b| {
                let _ = write!(s, "{b:02x}");
                s
            }));
        }
        e.seed = Some(configs::settings(config, rate).seed);
        e.counter = Some(0);
        if bless {
            let tag = format!("{name}/{config}/{rate} Hz");
            let (y2, frames2, g2) = record_changes_scenario(
                make,
                scenario,
                value,
                stream,
                config,
                rate,
                Partition::Irregular,
            );
            assert!(cc::same(&y, &y2), "{tag}: the partition identity (output)");
            assert_eq!(frames, frames2, "{tag}: the partition identity (frames)");
            assert_eq!(g.log, g2.log, "{tag}: the partition identity (log)");
            if scenario == "zero" {
                let settings = configs::settings(config, rate);
                let constant = rc::constant_settings(&settings, record_changes_value(value));
                let x = record_changes_stimulus(scenario, rate);
                if stream == "record" {
                    let r = rc::record_run(&mut make(constant), &x, Partition::Fixed(64), &[]);
                    assert!(
                        cc::same(&y, &r.samples),
                        "{tag}: the input-0 identity (samples)"
                    );
                    assert_eq!(frames, r.frames, "{tag}: the input-0 identity (frames)");
                } else {
                    let r = rc::one_pass_run(&mut make(constant), &x, Partition::Fixed(64), &[]);
                    assert!(
                        cc::same(&y, &r.out),
                        "{tag}: the input-0 identity (one-pass)"
                    );
                }
            }
            if scenario == "hang_shortening" {
                assert!(g.log.len() == 1, "{tag}: one landing");
            }
            if scenario == "cross_stream" {
                assert!(
                    g.log.len() == 1 && g.log[0].ends_with(" 7"),
                    "{tag}: the landing's tag"
                );
            }
            if scenario == "stop_case" {
                assert_eq!(g.log.len(), 2, "{tag}: the entry inside the flush");
            }
            let _ = Stream::Record;
        }
        e.record_changes = Some(g);
        entries.push(e);
    };
    for &value in &RECORD_CHANGES_VALUES {
        for &scenario in &["zero", "inside"] {
            for &stream in &["record", "one_pass"] {
                for &config in &RECORD_CHANGES_CONFIGS {
                    for rate in configs::all_rates() {
                        push(scenario, value, stream, config, rate);
                    }
                }
            }
        }
    }
    for &stream in &["record", "one_pass"] {
        for &config in &["record_default", "record_default_mute"] {
            for rate in configs::all_rates() {
                push("hang_shortening", "vas.hang_ms", stream, config, rate);
            }
        }
    }
    for rate in configs::all_rates() {
        push(
            "stop_case",
            "agc.knee_dbfs",
            "record",
            "record_default",
            rate,
        );
        push(
            "cross_stream",
            "agc.knee_dbfs",
            "one_pass",
            "record_default",
            rate,
        );
    }
    GoldenFile {
        format: "rr_dr60-golden".into(),
        version: 1,
        library_version: rr_dr60::VERSION.into(),
        entries,
    }
}

/// The committed spec 017 record-changes golden file.
///
/// # Panics
///
/// If the embedded file is not valid golden JSON.
pub fn committed_record_changes() -> GoldenFile {
    serde_json::from_str(include_str!("../golden/golden-record-changes-v1.json"))
        .expect("valid embedded record-changes golden file")
}

/// Path of the spec 017 record-changes golden file. Bless it only with
/// `RR_DR60_BLESS=record_changes cargo test -p rr_dr60_harness --test golden_record_changes`,
/// with a CHANGELOG entry.
pub fn path_record_changes() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("golden/golden-record-changes-v1.json")
}

/// Spec 018 (research R-09): the three battery stimuli at `rate`: the clipped tone
/// (`battery_checks::clipped_tone`, 1.5 s at 0 dBFS, run at `VOLUME_6DB_OVER`), the −20 dBFS
/// voiced stimulus (2 s, run at the reference position) and a full-scale 1 kHz square (1 s, run
/// at position 1).
pub fn battery_stimulus(name: &str, rate: u32) -> Vec<f32> {
    let f = f64::from(rate);
    match name {
        "clipped_tone" => crate::battery_checks::clipped_tone(rate),
        "voiced_m20" => stimulus::voiced(f, 2.0, -20.0, 0x0D60),
        "square_fs" => stimulus::square(1000.0, 1.0, f, rate as usize),
        other => panic!("unknown battery stimulus {other}"),
    }
}

/// The three battery stimuli and the volume position each runs at.
pub const BATTERY_STIMULI: [(&str, f32); 3] = [
    ("clipped_tone", rr_dr60::output::VOLUME_6DB_OVER),
    ("voiced_m20", rr_dr60::output::VOLUME_REFERENCE),
    ("square_fs", 1.0),
];

/// The battery configurations: the default output on the speaker and on the earphone route.
pub const BATTERY_CONFIGS: [&str; 2] = ["battery_default", "battery_default_earphone"];

fn battery_route(config: &str) -> rr_dr60::Route {
    if config == "battery_default_earphone" {
        rr_dr60::Route::Earphone
    } else {
        rr_dr60::Route::Speaker
    }
}

/// One battery run: the engine at `config` and `rate` on its route at `volume`, the battery set
/// to `volts` (none: the twin without the setter), `changes` scheduled, the stimulus run in
/// 1024-sample blocks through the log-taking variant. Returns the output and the battery part.
fn battery_run(
    make: Make<'_>,
    config: &str,
    rate: u32,
    stim: &str,
    volume: f32,
    volts: Option<f32>,
    changes: &[crate::changes_checks::Change],
) -> (Vec<f32>, BatteryGolden) {
    use crate::changes_checks::{self as cc, Partition, Source};
    let x = battery_stimulus(stim, rate);
    let route = battery_route(config);
    let mut p = make(configs::settings(config, rate));
    p.set_route(route);
    p.set_volume(volume).expect("a valid position");
    if let Some(v) = volts {
        p.set_battery_volts(v).expect("a valid battery value");
    }
    let r = cc::run_scheduled(&mut p, Source::OnePass(&x), changes, Partition::Fixed(1024));
    let log = crate::battery_checks::battery_entries(&r.log)
        .iter()
        .map(|e| {
            format!(
                "{} {} {:016x} {:016x} {}",
                e.position,
                e.control,
                e.before.to_bits(),
                e.after.to_bits(),
                e.tag
            )
        })
        .collect();
    let g = BatteryGolden {
        volts,
        route: if route == rr_dr60::Route::Earphone {
            "earphone".into()
        } else {
            "speaker".into()
        },
        battery_row: crate::meters_checks::battery_row(&p.meters()),
        log,
        max_per_call: r.max_per_call as u32,
    };
    (r.out, g)
}

/// Spec 018 FR-005, FR-010 (research R-09): the battery golden: the five law points and the
/// twin without the setter × the two routes × the three stimuli × the seven rates (252
/// entries), plus one scheduled change 3.0 → 2.4 V at `battery_checks::change_at` on the
/// clipped tone per rate (7 entries): 259. At bless time the default entries' digests equal
/// the twin's, the below-knee (voiced) entries equal the default's at every value, every
/// earphone-route entry equals its twin, and a scheduled no-op to 3.0 V equals the plain entry
/// with a one-entry log.
pub fn generate_battery(make: Make<'_>, bless: bool) -> GoldenFile {
    use crate::changes_checks::{self as cc, Change};
    let mut entries = Vec::new();
    let points: [Option<f32>; 6] = [None, Some(3.0), Some(2.8), Some(2.6), Some(2.4), Some(2.2)];
    for config in BATTERY_CONFIGS {
        for (stim, volume) in BATTERY_STIMULI {
            for rate in configs::all_rates() {
                let mut twin: Option<GoldenEntry> = None;
                for volts in points {
                    let (y, g) = battery_run(make, config, rate, stim, volume, volts, &[]);
                    let name = match volts {
                        None => format!("{stim}@default"),
                        Some(v) => format!("{stim}@{v}V"),
                    };
                    let mut e = entry(&name, config, rate, &y);
                    e.battery = Some(g);
                    if bless {
                        let tag = format!("{name}/{config}/{rate} Hz");
                        if let Some(t) = &twin {
                            let identity = volts == Some(3.0)
                                || stim == "voiced_m20"
                                || config == "battery_default_earphone";
                            if identity {
                                assert_eq!(e.sha256, t.sha256, "{tag}: equals the twin");
                            } else {
                                assert_ne!(e.sha256, t.sha256, "{tag}: the ceiling moved");
                            }
                        } else {
                            twin = Some(e.clone());
                        }
                    }
                    entries.push(e);
                }
                if bless && config == "battery_default" && stim == "clipped_tone" {
                    // The no-op: a scheduled change to the default equals the plain run with a
                    // one-entry log.
                    let (y, g) = battery_run(
                        make,
                        config,
                        rate,
                        stim,
                        volume,
                        None,
                        &[Change::battery(rr_dr60::Stream::OnePass, 500, 3.0, 7)],
                    );
                    let plain = entry(stim, config, rate, &y);
                    let t = twin.as_ref().expect("the twin");
                    assert_eq!(plain.sha256, t.sha256, "{rate} Hz: the no-op twin's output");
                    assert_eq!(g.log.len(), 1, "{rate} Hz: the no-op is logged once");
                    let _ = cc::same;
                }
            }
        }
    }
    for rate in configs::all_rates() {
        let c = crate::battery_checks::change_at(rate);
        let (y, g) = battery_run(
            make,
            "battery_default",
            rate,
            "clipped_tone",
            rr_dr60::output::VOLUME_6DB_OVER,
            None,
            &[Change::battery(rr_dr60::Stream::OnePass, c, 2.4, 1)],
        );
        let mut e = entry(
            &format!("clipped_tone@3.0->2.4V@{c}"),
            "battery_default",
            rate,
            &y,
        );
        assert_eq!(g.log.len(), 1, "{rate} Hz: the scheduled change landed");
        e.battery = Some(g);
        entries.push(e);
    }
    GoldenFile {
        format: "rr_dr60-golden".into(),
        version: 1,
        library_version: rr_dr60::VERSION.into(),
        entries,
    }
}

/// The committed spec 018 battery golden file.
///
/// # Panics
///
/// If the embedded file is not valid golden JSON.
pub fn committed_battery() -> GoldenFile {
    serde_json::from_str(include_str!("../golden/golden-battery-v1.json"))
        .expect("valid embedded battery golden file")
}

// ---- Spec 021: the live bypass golden (research R-09) -----------------------------------------

/// Spec 021: the bypass part of a golden entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BypassGolden {
    /// The stage(s) switched.
    pub stages: Vec<u8>,
    /// The schedule: (position, control, value, tag).
    pub schedule: Vec<(u64, u32, f64, u64)>,
    /// The stream: `playback` or `one_pass`.
    pub stream: String,
    /// The route.
    pub route: String,
    /// The switch entries of the log: (position, control, before, after, tag, stream word).
    pub log: Vec<(u64, u32, f64, f64, u64, u32)>,
    /// The produced count.
    pub produced: u64,
    /// The endpoint identities are bit-exact for this entry (asserted at bless time); the
    /// others carry the convergence bound.
    pub exact: bool,
    /// A reversal schedule (the second change inside the blend).
    pub reversal: bool,
}

/// One golden scenario per cell: `bypass_return` (a bypass at P and a return at Q) or
/// `reversal` (a bypass at P reversed at P + N/2).
fn bypass_scenarios() -> [&'static str; 2] {
    ["bypass_return", "reversal"]
}

/// Spec 021 (research R-09): 24 entries per rate (eleven cells' two scenarios and two two-stage
/// entries), the output's digest with the schedule and the switch entries. At bless time the
/// exact entries' endpoint identities (the all-on run before P, the configured-off twin from
/// P + N − 1 to Q, the all-on run after Q + N − 1) and every entry's partition identity (block
/// size 1 against 4096) are asserted; the converging entries' difference from the twin after
/// the settle is asserted ≤ −100 dBFS.
pub fn generate_bypass(make: Make<'_>, bless: bool) -> GoldenFile {
    use crate::bypass_checks as bp;
    use crate::changes_checks::{self as cc, Partition};
    let mut entries = Vec::new();
    for rate in configs::all_rates() {
        let stored = bp::stored_tone(1000.0, 1.5);
        let host = bp::host_tone(1000.0, 1.5, rate);
        let mut cells = bp::cells();
        cells.retain(|c| {
            // The matrix's 14 cells less the four whose twin is the same engine twice (the
            // one-pass stage-11/12/13 stage-only cells duplicate the player's identities).
            !(c.stream == rr_dr60::Stream::OnePass && c.config != bp::DEFAULT_CHAIN)
        });
        for cell in &cells {
            let (p, q) = bp::positions(cell.stage, rate);
            let volume = if cell.stage == 11 {
                bp::neutral_position(cell.route)
            } else {
                rr_dr60::output::VOLUME_REFERENCE
            };
            let n = bp::n_fade(cell.stage, rate) as u64;
            let n_host = if cell.stage == 10 {
                n * u64::from(rate) / 6000
            } else {
                n
            };
            for scenario in bypass_scenarios() {
                let sched = match scenario {
                    "bypass_return" => {
                        vec![bp::change(cell, p, false, 1), bp::change(cell, q, true, 2)]
                    }
                    _ => vec![
                        bp::change(cell, p, false, 1),
                        bp::change(cell, p + n_host / 2, true, 2),
                    ],
                };
                let s = bp::settings(cell.config, rate);
                let run = bp::run_cell(
                    make,
                    s,
                    cell,
                    &stored,
                    &host,
                    volume,
                    &sched,
                    Partition::Fixed(4096),
                );
                let name = format!("{scenario}/stage{}/{:?}", cell.stage, cell.route);
                let config = if cell.stream == rr_dr60::Stream::OnePass {
                    format!("{}@one_pass", cell.config)
                } else {
                    cell.config.to_string()
                };
                let mut e = entry(&name, &config, rate, &run.out);
                let log: Vec<(u64, u32, f64, f64, u64, u32)> =
                    cc::entries_of(&run.log, cell.control)
                        .iter()
                        .map(|c| (c.position, c.control, c.before, c.after, c.tag, c.stream))
                        .collect();
                e.bypass = Some(BypassGolden {
                    stages: vec![cell.stage],
                    schedule: sched
                        .iter()
                        .map(|c| (c.position, c.control, c.value, c.tag))
                        .collect(),
                    stream: if cell.stream == rr_dr60::Stream::OnePass {
                        "one_pass".into()
                    } else {
                        "playback".into()
                    },
                    route: format!("{:?}", cell.route).to_lowercase(),
                    log,
                    produced: run.out.len() as u64,
                    exact: cell.exact,
                    reversal: scenario == "reversal",
                });
                if bless {
                    let tag = format!("{name}/{config}/{rate} Hz");
                    // The partition identity.
                    let one = bp::run_cell(
                        make,
                        s,
                        cell,
                        &stored,
                        &host,
                        volume,
                        &sched,
                        Partition::Fixed(1),
                    );
                    assert!(
                        cc::same(&one.out, &run.out) && one.log == run.log,
                        "{tag}: block size 1"
                    );
                    if scenario == "bypass_return" {
                        let all_on = bp::run_cell(
                            make,
                            s,
                            cell,
                            &stored,
                            &host,
                            volume,
                            &[],
                            Partition::Fixed(4096),
                        );
                        let twin = bp::run_cell(
                            make,
                            bp::twin_settings(cell, rate),
                            cell,
                            &stored,
                            &host,
                            volume,
                            &[],
                            Partition::Fixed(4096),
                        );
                        let (p, q) = (p as usize, q as usize);
                        assert!(cc::same(&run.out[..p], &all_on.out[..p]), "{tag}: before P");
                        if cell.exact {
                            // Stage 10's blend reaches the host output through the rate
                            // conversion: its identity holds from the bound P + N_host − 1 + 2D
                            // (research R-03); the chain stages' from P + N − 1.
                            let two_d = if cell.stage == 10 {
                                2 * rr_dr60::__test_hooks::resampler_delay_host_samples(rate)
                                    .unwrap_or(0) as usize
                            } else {
                                0
                            };
                            let n_h = n_host as usize + two_d;
                            assert!(
                                cc::same(&run.out[p + n_h - 1..q], &twin.out[p + n_h - 1..q]),
                                "{tag}: the twin"
                            );
                            let end = run.out.len().min(all_on.out.len());
                            assert!(
                                cc::same(&run.out[q + n_h - 1..end], &all_on.out[q + n_h - 1..end]),
                                "{tag}: the return"
                            );
                        } else {
                            let settle = rr_dr60::output::settle_samples(rate).unwrap() as usize;
                            let two_d = if cell.stage == 10 {
                                2 * rr_dr60::__test_hooks::resampler_delay_host_samples(rate)
                                    .unwrap_or(0) as usize
                            } else {
                                0
                            };
                            let from = p + n_host as usize - 1 + settle + two_d;
                            let worst = (from..q)
                                .map(|i| (f64::from(run.out[i]) - f64::from(twin.out[i])).abs())
                                .fold(0.0, f64::max);
                            assert!(worst <= 1e-5, "{tag}: converges within -100 dBFS ({worst})");
                        }
                    }
                }
                entries.push(e);
            }
        }
        // Two two-stage entries: 11 + 12 on the speaker route, 11 + 13 on the earphone route,
        // at one position with one tag (the whole-output-path recipe).
        for (route, other) in [
            (rr_dr60::Route::Speaker, 23u32),
            (rr_dr60::Route::Earphone, 24u32),
        ] {
            let cell = bp::Cell {
                stage: 11,
                control: bp::CONTROL_STAGE_11,
                config: bp::DEFAULT_CHAIN,
                route,
                stream: rr_dr60::Stream::Playback,
                exact: false,
            };
            let p = u64::from(rate) / 2;
            let sched = vec![
                rr_dr60_change(p, bp::CONTROL_STAGE_11, 0.0, 9),
                rr_dr60_change(p, other, 0.0, 9),
            ];
            let s = bp::settings(cell.config, rate);
            let run = bp::run_cell(
                make,
                s,
                &cell,
                &stored,
                &host,
                bp::neutral_position(route),
                &sched,
                Partition::Fixed(4096),
            );
            let name = format!(
                "whole_path/11+{}/{:?}",
                if other == 23 { 12 } else { 13 },
                route
            );
            let mut e = entry(&name, bp::DEFAULT_CHAIN, rate, &run.out);
            let log: Vec<(u64, u32, f64, f64, u64, u32)> = run
                .log
                .iter()
                .filter(|c| c.control == bp::CONTROL_STAGE_11 || c.control == other)
                .map(|c| (c.position, c.control, c.before, c.after, c.tag, c.stream))
                .collect();
            e.bypass = Some(BypassGolden {
                stages: vec![11, if other == 23 { 12 } else { 13 }],
                schedule: sched
                    .iter()
                    .map(|c| (c.position, c.control, c.value, c.tag))
                    .collect(),
                stream: "playback".into(),
                route: format!("{route:?}").to_lowercase(),
                log,
                produced: run.out.len() as u64,
                exact: false,
                reversal: false,
            });
            if bless {
                let one = bp::run_cell(
                    make,
                    s,
                    &cell,
                    &stored,
                    &host,
                    bp::neutral_position(route),
                    &sched,
                    Partition::Fixed(1),
                );
                assert!(
                    cc::same(&one.out, &run.out),
                    "{name}/{rate} Hz: block size 1"
                );
            }
            entries.push(e);
        }
    }
    GoldenFile {
        format: "rr_dr60-golden".into(),
        version: 1,
        library_version: rr_dr60::VERSION.into(),
        entries,
    }
}

fn rr_dr60_change(
    position: u64,
    control: u32,
    value: f64,
    tag: u64,
) -> crate::changes_checks::Change {
    crate::changes_checks::Change {
        stream: rr_dr60::Stream::Playback,
        position,
        control,
        value,
        tag,
    }
}

/// The committed spec 021 bypass golden file.
///
/// # Panics
///
/// If the embedded file is not valid golden JSON.
pub fn committed_bypass() -> GoldenFile {
    serde_json::from_str(include_str!("../golden/golden-bypass-v1.json"))
        .expect("valid embedded bypass golden file")
}

/// The path of the spec 021 bypass golden file.
pub fn path_bypass() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("golden/golden-bypass-v1.json")
}

// ---- Spec 020: the processing-record golden (plan review, coach 11) ------------------------

/// Normalizes a record's text for the golden: the version values replaced, the checksum line
/// dropped. Returns the lines and the raw `(library_version, stage lines)` for the compare-time
/// assertions.
pub fn normalize_record(text: &str) -> (Vec<String>, String, Vec<String>) {
    let mut lines = Vec::new();
    let mut library = String::new();
    let mut stages = Vec::new();
    for l in text.lines() {
        if let Some(v) = l.strip_prefix("library_version = ") {
            library = v.to_string();
            lines.push("library_version = <current>".to_string());
        } else if l.starts_with("stage.") {
            stages.push(l.to_string());
            let name = l.split(" = ").next().unwrap_or(l);
            lines.push(format!("{name} = <current>"));
        } else if l.starts_with("record_checksum = ") {
            lines.push("record_checksum = <dropped>".to_string());
        } else if l.starts_with("recording_digest = ") {
            // Spec 021 (restated at implementation): the stored form's header carries the
            // library version (contracts/recording-form.md), so the digest moves at every
            // version bump; its consistency with the export is asserted by spec 020's story
            // tests, not pinned here.
            lines.push("recording_digest = <version-dependent>".to_string());
        } else {
            lines.push(l.to_string());
        }
    }
    (lines, library, stages)
}

/// The spec 020 golden (research R-02; plan review coach 11): one entry per rate, the default
/// engine with the codec bypassed (the stored samples are the entry's output), a knee change and
/// the matrix stimulus's gaps, the record normalized. At bless time the raw version lines are
/// asserted against the library and the manifest; at compare time too, by the test.
pub fn generate_record_contract(make: Make<'_>) -> GoldenFile {
    use crate::record_contract_checks::{self as rc, Cell};
    let mut entries = Vec::new();
    for rate in configs::all_rates() {
        let cell = Cell {
            codec: false,
            changes: true,
            mute: false,
            self_stop: false,
        };
        let raw = rc::raw_stimulus(rate);
        let (p, run) = rc::record_run(make, rate, cell, &raw);
        let stored = rr_dr60::__test_hooks::stored_samples(&p, run.number).to_vec();
        let text = String::from_utf8(run.record.clone()).expect("ASCII");
        let (lines, library, stages) = normalize_record(&text);
        assert_eq!(library, rr_dr60::VERSION, "{rate} Hz: the library line");
        assert_eq!(stages, manifest_stage_lines(), "{rate} Hz: the stage lines");
        let r = rc::readout(&run.record);
        let mut e = entry("matrix_voice", "record_contract", rate, &stored);
        e.record_contract = Some(RecordContractGolden {
            lines,
            digest: "<version-dependent>".into(), // 021: the header carries the library version
            // (`run.digest` is asserted against the export by spec 020's story tests)
            event_count: r.event_count,
            change_count: r.change_count,
        });
        entries.push(e);
    }
    GoldenFile {
        format: "rr_dr60-golden".into(),
        version: 1,
        library_version: rr_dr60::VERSION.into(),
        entries,
    }
}

/// The `stage.N.model_version = …` lines the manifest implies, in stage order (the record's).
pub fn manifest_stage_lines() -> Vec<String> {
    rr_dr60::manifest()
        .stages
        .iter()
        .filter(|e| !e.model_version.is_empty())
        .map(|e| format!("stage.{}.model_version = {}", e.number, e.model_version))
        .collect()
}

/// The committed spec 020 golden file (embedded at compile time).
///
/// # Panics
///
/// If the embedded JSON is not a golden file.
pub fn committed_record_contract() -> GoldenFile {
    serde_json::from_str(include_str!("../golden/golden-record-v1.json"))
        .expect("valid embedded record golden file")
}

/// Path of the spec 020 record golden file. Bless it only with
/// `RR_DR60_BLESS=record cargo test -p rr_dr60_harness --test golden_record`, with a CHANGELOG
/// entry.
pub fn path_record_contract() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("golden/golden-record-v1.json")
}

/// Path of the spec 018 battery golden file. Bless it only with
/// `RR_DR60_BLESS=battery cargo test -p rr_dr60_harness --test golden_battery`, with a CHANGELOG
/// entry.
pub fn path_battery() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("golden/golden-battery-v1.json")
}

// ---- Spec 023: the pitch-kept golden (research R-12) -------------------------------------------

/// The stimuli of the pitch-kept golden, in sorted order: stored 6 kHz streams imported into
/// the target engine (2 s each: the schedules' last change must land before the end at the
/// fast levels).
pub const PITCH_STIMULI: [&str; 4] = ["glide_2s", "tone_1k_2s", "tone_offgrid_2s", "vowel_2s"];

fn pitch_stimulus(name: &str) -> Vec<f32> {
    use crate::pitch_checks as pc;
    match name {
        "glide_2s" => pc::glide(2.0),
        "tone_1k_2s" => pc::tone(1000.0, 2.0),
        "tone_offgrid_2s" => pc::tone(pc::OFFGRID_HZ, 2.0),
        "vowel_2s" => pc::vowel(2.0),
        other => panic!("unknown pitch stimulus {other:?}"),
    }
}

/// A pitch golden run: the output, the log, the shifts and the schedule as listed.
type PitchRun = (
    Vec<f32>,
    Vec<rr_dr60::ChangeEntry>,
    Vec<i32>,
    Vec<(u64, u32, f64, u64)>,
);
/// A run to make: (name, start level, start mode, requests).
type PitchRunSpec = (String, u8, u32, Vec<(usize, u32, f64)>);

/// The schedules (requests at call indices of `rate ÷ 50` host samples): the level changes in
/// pitch-kept mode, the mode changes at level 1, and both.
pub fn pitch_schedule(name: &str) -> (u8, u32, Vec<(usize, u32, f64)>) {
    match name {
        "levels" => (
            1,
            1,
            vec![(20, 2, 5.0), (45, 2, 2.0), (70, 2, 3.0), (90, 2, 4.0)],
        ),
        "modes" => (1, 0, vec![(20, 30, 1.0), (50, 30, 0.0), (80, 30, 1.0)]),
        "both" => (
            1,
            1,
            vec![
                (20, 2, 4.0),
                (20, 30, 0.0),
                (45, 30, 1.0),
                (60, 2, 3.0),
                (80, 2, 2.0),
            ],
        ),
        other => panic!("unknown pitch schedule {other:?}"),
    }
}

/// Spec 023 D8 (research R-12): the pitch-kept file, 420 entries: at the seven rates, the four
/// stimuli in `speed_default` (the codec on) and `speed_default_no_codec`, each at the four
/// pitch-kept levels and under the three schedules (392), plus the speed-only set at 6 kHz (28).
/// With `bless`, asserts each level's default-mode twin bit-identical to the untouched engine,
/// every constant length equal to the mode-aware helper, every scheduled change landing before
/// the end, and the speed-only entries bit-exact against the independent reference with the
/// same shift sequence.
pub fn generate_pitch(make: Make<'_>, bless: bool) -> GoldenFile {
    use crate::changes_checks::{self as cc, Change, Partition, Source};
    use crate::pitch_checks::{self as pc, Req};
    use crate::speed_checks as sc;
    use rr_dr60::speed::SpeedMode;
    let mut entries = Vec::new();
    let mode_of = |m: u32| SpeedMode::from_u32(m).unwrap();
    let digest = |shifts: &[i32]| {
        let mut h = rr_dr60::digest::Fnv1a64::new();
        for s in shifts {
            h.update(&s.to_le_bytes());
        }
        format!("{:016x}", h.finish())
    };
    let mut push = |stim: &str,
                    config: String,
                    rate: u32,
                    y: &[f32],
                    level: u8,
                    mode: u32,
                    schedule: Vec<(u64, u32, f64, u64)>,
                    log: &[rr_dr60::ChangeEntry],
                    shifts: &[i32]| {
        let mut e = entry(stim, &config, rate, y);
        let log: Vec<(u64, u32, f64, f64, u64, u32)> = log
            .iter()
            .filter(|c| c.control == 2 || c.control == 30)
            .map(|c| (c.position, c.control, c.before, c.after, c.tag, c.stream))
            .collect();
        e.pitch = Some(PitchGolden {
            mode,
            level,
            schedule,
            log,
            produced: y.len() as u64,
            frames: shifts.len() as u64,
            shifts_fnv1a64: digest(shifts),
        });
        entries.push(e);
    };
    let run_sched = |p: &mut rr_dr60::Pipeline,
                     call: usize,
                     level: u8,
                     mode: u32,
                     sched: &[(usize, u32, f64)]|
     -> PitchRun {
        p.set_playback_speed(level).unwrap();
        p.set_playback_speed_mode(mode_of(mode));
        let changes: Vec<Change> = sched
            .iter()
            .enumerate()
            .map(|(i, &(c, control, v))| Change {
                stream: rr_dr60::Stream::Playback,
                position: (c * call) as u64,
                control,
                value: v,
                tag: i as u64 + 1,
            })
            .collect();
        let run = cc::run_scheduled(p, Source::Playback(1), &changes, Partition::Fixed(call));
        let shifts = rr_dr60::__test_hooks::pitch_shift_trace(p);
        let listed = changes
            .iter()
            .map(|c| (c.position, c.control, c.value, c.tag))
            .collect();
        (run.out, run.log, shifts, listed)
    };
    for rate in configs::all_rates() {
        let call = (rate / 50) as usize;
        for stim in PITCH_STIMULI {
            let x = pitch_stimulus(stim);
            for config in ["speed_default", "speed_default_no_codec"] {
                let settings = configs::settings(config, rate);
                let (mut p, n) = configs::stored_recording(make, settings, &x);
                assert_eq!(n, 1);
                p.set_route(rr_dr60::Route::Speaker);
                let drain = u64::from(p.drain_samples());
                for level in pc::LEVELS {
                    let (y, log, shifts, _) = run_sched(&mut p, call, level, 1, &[]);
                    if bless {
                        let tag = format!("{stim} {config} {rate} level {level}");
                        let helper = p
                            .playback_length_samples_in_mode(1, level, SpeedMode::PitchKept)
                            .unwrap();
                        assert_eq!(y.len() as u64, helper, "{tag}: the helper (023 FR-005)");
                        // The default-mode twin equals the untouched engine's playback.
                        let (d, _, _, _) = run_sched(&mut p, call, level, 0, &[]);
                        let plain = sc::play(&mut p, 1, call, &[(0, level)]);
                        assert_eq!(sc::mismatches(&d, &plain.out), 0, "{tag}: the default twin");
                    }
                    push(
                        stim,
                        format!("{config}_kept_l{level}"),
                        rate,
                        &y,
                        level,
                        1,
                        vec![],
                        &log,
                        &shifts,
                    );
                }
                for sched in ["levels", "modes", "both"] {
                    let (level, mode, requests) = pitch_schedule(sched);
                    let (y, log, shifts, listed) = run_sched(&mut p, call, level, mode, &requests);
                    if bless {
                        let end = y.len() as u64 - drain;
                        assert!(
                            listed
                                .iter()
                                .all(|&(pos, ..)| pos + 2 * u64::from(rate) / 50 < end),
                            "{stim} {config} {rate} {sched}: a change or its blend after the end"
                        );
                        assert_eq!(
                            cc::entries_of(&log, 2).len() + cc::entries_of(&log, 30).len(),
                            requests.len(),
                            "{stim} {config} {rate} {sched}: every change logged"
                        );
                    }
                    push(
                        stim,
                        format!("{config}_sched_{sched}"),
                        rate,
                        &y,
                        level,
                        mode,
                        listed,
                        &log,
                        &shifts,
                    );
                }
            }
        }
    }
    // The speed-only set at 6 kHz, asserted against the reference at bless time.
    let only = configs::settings("speed_only", 6000);
    for stim in PITCH_STIMULI {
        let x = pitch_stimulus(stim);
        let (mut p, n) = configs::stored_recording(make, only, &x);
        assert_eq!(n, 1);
        let runs: Vec<PitchRunSpec> = pc::LEVELS
            .iter()
            .map(|&l| (format!("speed_only_kept_l{l}"), l, 1, vec![]))
            .chain(["levels", "modes", "both"].iter().map(|s| {
                let (level, mode, req) = pitch_schedule(s);
                (format!("speed_only_sched_{s}"), level, mode, req)
            }))
            .collect();
        for (name, level, mode, requests) in runs {
            let (y, log, shifts, listed) = run_sched(&mut p, 120, level, mode, &requests);
            if bless {
                let dev: Vec<(usize, Req)> = requests
                    .iter()
                    .map(|&(c, control, v)| {
                        (
                            c * 120,
                            if control == 2 {
                                Req::Level(v as u8)
                            } else {
                                Req::Mode(mode_of(v as u32))
                            },
                        )
                    })
                    .collect();
                let (mism, eq, a, b) =
                    pc::against_reference(make, "speed_only", &x, level, mode_of(mode), &dev);
                assert_eq!(mism, 0, "{stim} {name}: against the reference (023 R-09)");
                assert!(eq, "{stim} {name}: the shifts ({a} vs {b})");
            }
            push(stim, name, 6000, &y, level, mode, listed, &log, &shifts);
        }
    }
    entries.sort_by(|a, b| {
        (&a.stimulus, &a.config, a.host_rate_hz).cmp(&(&b.stimulus, &b.config, b.host_rate_hz))
    });
    GoldenFile {
        format: "rr_dr60-golden".into(),
        version: 1,
        library_version: env!("CARGO_PKG_VERSION").into(),
        entries,
    }
}

/// The committed spec 023 pitch-kept golden file, embedded at compile time.
///
/// # Panics
///
/// If the embedded file is not valid JSON.
pub fn committed_pitch() -> GoldenFile {
    serde_json::from_str(include_str!("../golden/golden-pitch-kept-v1.json"))
        .expect("valid embedded pitch-kept golden file")
}

/// The path of the spec 023 pitch-kept golden file.
pub fn path_pitch() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("golden/golden-pitch-kept-v1.json")
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
