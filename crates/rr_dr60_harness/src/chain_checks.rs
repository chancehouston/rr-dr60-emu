//! Spec 005 chain-level checks: the record chain still records speech (FR-012 (i)–(iii);
//! research R-07; data-model › Speech-like pattern, Loud-after-floor; tasks.md T029).
//!
//! Everything runs through the default pipeline (`default_vas`: limiter, record stage, clamp,
//! VAS and playback stage on) and is measured by **output length and events** only (spec 003
//! research R-11). Bursts are 1 kHz at the device-grid aligned phase (spec 004 FR-012), the
//! rule for stimuli observed at the default output. Levels, timings and the ±1 ms figure are
//! engineering targets (005 FR-017); the threshold traces to A-022 (revised), its rationale to
//! A-029.

use rr_dr60::Pipeline;

use crate::agc_checks;
use crate::checks::{Make, MeasurementResult, Tolerance};
use crate::vas_checks::{self, Run, hang_s, onset_s};
use crate::{configs, stimulus};

const CONFIG: &str = "default_vas";
const T_CHAIN: &[&str] = &["A-022", "A-029", "engineering target"];

/// Burst level: the level-3 threshold (−40 dBFS, A-022 revised) + 10 dB (005 FR-012).
pub const BURST_DBFS: f64 = -30.0; // 005 FR-012; A-022 (revised)
/// The speech-like pattern (data-model.md): three groups of four 300 ms bursts 150 ms apart,
/// 5 s gaps, a 0.5 s lead and a 1 s tail (engineering targets, 005 FR-012).
const GROUPS: usize = 3; // engineering target (005 FR-012)
const BURSTS_PER_GROUP: usize = 4; // engineering target (005 FR-012)
const BURST_S: f64 = 0.3; // engineering target (005 FR-012)
const BETWEEN_S: f64 = 0.15; // engineering target (005 FR-012)
const GAP_S: f64 = 5.0; // engineering target (005 FR-012)
const LEAD_S: f64 = 0.5; // engineering target (005 FR-012): under the hang time
const TAIL_S: f64 = 1.0; // engineering target (005 FR-012): not above the hang time
/// Scenario (iii)'s floor before the loud burst: 30 s (10 s under coverage instrumentation).
const FLOOR_S: f64 = if cfg!(coverage) { 10.0 } else { 30.0 }; // engineering target (005 FR-012)
/// Scenario (iii)'s burst: +6 dBFS, 5 dB over the knee (engineering target, 005 FR-012).
const LOUD_DBFS: f64 = 6.0; // engineering target (005 FR-012)

fn samples(secs: f64, fs: f64) -> usize {
    (secs * fs + 0.5) as usize
}

/// The device-grid aligned phase for 1 kHz at `rate` (spec 004 FR-012, research R-08).
fn aligned(rate: u32) -> f64 {
    let bypass = Pipeline::new(configs::settings("bypass_all", rate))
        .expect("supported rate")
        .latency_samples();
    stimulus::aligned_phase(1000.0, rate, bypass)
}

/// The speech-like pattern: `GROUPS` groups of `BURSTS_PER_GROUP` 1 kHz bursts at `burst_dbfs`
/// (peak), `BURST_S` long and `BETWEEN_S` apart, the groups separated by `GAP_S`, with a
/// `LEAD_S` lead and a `TAIL_S` tail, over band-limited noise (225–2550 Hz, A-027) at
/// `floor_dbfs_rms` throughout (the harness's AES17-style level, as spec 003 used it). Returns
/// the samples and each group's `[start, end)` range in input samples.
pub fn speech_like_pattern(
    rate: u32,
    burst_dbfs: f64,
    floor_dbfs_rms: f64,
    seed: u64,
) -> (Vec<f32>, Vec<(usize, usize)>) {
    let fs = f64::from(rate);
    let phase = aligned(rate);
    let mut x: Vec<f32> = vec![0.0; samples(LEAD_S, fs)];
    let mut groups = Vec::with_capacity(GROUPS);
    for g in 0..GROUPS {
        let start = x.len();
        for b in 0..BURSTS_PER_GROUP {
            x.extend(stimulus::tone_at_phase(
                1000.0,
                burst_dbfs,
                phase,
                fs,
                samples(BURST_S, fs),
            ));
            if b + 1 < BURSTS_PER_GROUP {
                x.extend(std::iter::repeat_n(0.0f32, samples(BETWEEN_S, fs)));
            }
        }
        groups.push((start, x.len()));
        let rest = if g + 1 < GROUPS { GAP_S } else { TAIL_S };
        x.extend(std::iter::repeat_n(0.0f32, samples(rest, fs)));
    }
    let noise = stimulus::bandlimited_noise(seed, x.len(), fs, floor_dbfs_rms);
    (x.iter().zip(&noise).map(|(b, n)| b + n).collect(), groups)
}

/// Rows for a run of the speech-like pattern that is expected to pause in every gap: the
/// splice count (one per gap), each splice's removed length against gap − hang + onset (003
/// FR-008, FR-009: the hang time is kept, the onset of the next group is lost) within ±1 ms,
/// and each resume's input-mapped position against the next group's start + onset, within the
/// chain latency + 1 ms (a splice inside a group would be at least 150 ms off).
fn paused_gap_rows(
    rate: u32,
    label: &str,
    r: &Run,
    groups: &[(usize, usize)],
    settings: rr_dr60::Settings,
) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let gaps = GROUPS - 1;
    let hang = hang_s(&settings.vas);
    let onset = onset_s(&settings.vas);
    let lat_ms = f64::from(Pipeline::new(settings).unwrap().latency_samples()) / fs * 1e3;
    let mut out = vec![MeasurementResult::new(
        "005/FR-012",
        "splices (one per 5 s gap; none inside a group)",
        T_CHAIN,
        rate,
        CONFIG,
        label.to_string(),
        r.events.len() as f64,
        "events",
        Tolerance::Exact(gaps as f64), // 005 FR-012 (i): one splice per gap
    )];
    let mut removed_so_far = 0u64;
    for (k, e) in r.events.iter().enumerate() {
        removed_so_far += e.input_length;
        let want_removed = (GAP_S - hang + onset) * fs;
        out.push(MeasurementResult::new(
            "005/FR-012",
            "removed length of a gap - (gap - hang + onset)",
            T_CHAIN,
            rate,
            CONFIG,
            format!("{label}: gap {}", k + 1),
            (e.input_length as f64 - want_removed) / fs * 1e3,
            "ms",
            Tolerance::Range(-1.0, 1.0), // engineering target (005 FR-012): ±1 ms
        ));
        if let Some(&(start, _)) = groups.get(k + 1) {
            // Output positions count output samples; adding what was removed before (this
            // splice included) maps the resume back to input time.
            let resume_in = e.output_position as f64 + removed_so_far as f64;
            let want = start as f64 + onset * fs;
            out.push(MeasurementResult::new(
                "005/FR-012",
                "resume position - (next group start + onset)",
                T_CHAIN,
                rate,
                CONFIG,
                format!("{label}: splice {}", k + 1),
                (resume_in - want) / fs * 1e3,
                "ms",
                // engineering target (005 FR-012): within the chain latency plus 1 ms (the
                // positions carry the conversion delay, 003 R-06); a splice inside a group
                // would be at least 150 ms off.
                Tolerance::Range(-(lat_ms + 1.0), lat_ms + 1.0),
            ));
        }
    }
    out
}

/// 005 FR-012 (i), US2 AS1: the speech-like pattern at −30 dBFS over a −60 dBFS floor at
/// level 3: no splice inside a burst group, each 5 s gap cut to the hang time.
pub fn check_fr012_i(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let settings = configs::settings(CONFIG, rate);
    let (x, groups) = speech_like_pattern(rate, BURST_DBFS, -60.0, 0x0D60); // engineering target (005 FR-012)
    let r = vas_checks::run(make, settings, &x);
    paused_gap_rows(
        rate,
        "bursts -30 dBFS over a -60 dBFS floor, level 3",
        &r,
        &groups,
        settings,
    )
}

/// 005 FR-012 (ii), US2 AS2: over a −45 dBFS floor (peaks about 8.5 dB above RMS, 003 R-03;
/// above the level-3 threshold often enough never to pause for a hang time, below the level-1
/// threshold always, research R-15) there is no pause at level 3, and at level 1 every gap is
/// paused with the (i) figures. If the floor crosses the level-1 threshold the rows fail; that
/// is an R-15 finding to raise, not slack to add.
pub fn check_fr012_ii(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let (x, groups) = speech_like_pattern(rate, BURST_DBFS, -45.0, 0x0D60); // engineering target (005 FR-012)
    let level3 = configs::settings(CONFIG, rate);
    let r3 = vas_checks::run(make, level3, &x);
    let mut out = vec![
        MeasurementResult::new(
            "005/FR-012",
            "splices at level 3",
            T_CHAIN,
            rate,
            CONFIG,
            "bursts -30 dBFS over a -45 dBFS floor, level 3",
            r3.events.len() as f64,
            "events",
            Tolerance::Exact(0.0), // 005 FR-012 (ii): no pause at level 3
        ),
        MeasurementResult::new(
            "005/FR-012",
            "input samples removed at level 3",
            T_CHAIN,
            rate,
            CONFIG,
            "bursts -30 dBFS over a -45 dBFS floor, level 3",
            (x.len() - r3.y.len()) as f64,
            "samples",
            Tolerance::Exact(0.0), // 005 FR-012 (ii): the output keeps the input's length
        ),
    ];
    let mut level1 = level3;
    level1.vas.sensitivity = 1; // S-001, A-021: the least sensitive level (−34 dBFS)
    let r1 = vas_checks::run(make, level1, &x);
    out.extend(paused_gap_rows(
        rate,
        "bursts -30 dBFS over a -45 dBFS floor, level 1",
        &r1,
        &groups,
        level1,
    ));
    out
}

/// 005 FR-012 (iii), US2 AS3: `FLOOR_S` of a −60 dBFS floor, then a +6 dBFS 1 s burst at the
/// aligned phase, then 1 s of floor: exactly one splice, whose removed length is
/// floor − hang + onset (recording resumes after the onset time; by output length), the burst
/// limited to the ceiling after the dead time (its steady gain at the host-rate tap, 5 dB
/// over the knee), and, at the 6 kHz identity rate, clamped to exactly 1.0 at the record-path
/// tap (reported at the other rates, where the up-converter interpolates).
pub fn check_fr012_iii(rate: u32, make: Make<'_>) -> Vec<MeasurementResult> {
    let fs = f64::from(rate);
    let settings = configs::settings(CONFIG, rate);
    let (n_floor, n_burst, n_tail) = (samples(FLOOR_S, fs), samples(1.0, fs), samples(TAIL_S, fs));
    let mut x: Vec<f32> = vec![0.0; n_floor];
    x.extend(stimulus::tone_at_phase(
        1000.0,
        LOUD_DBFS,
        aligned(rate),
        fs,
        n_burst,
    ));
    x.extend(std::iter::repeat_n(0.0f32, n_tail));
    let noise = stimulus::bandlimited_noise(0x0D60, x.len(), fs, -60.0); // engineering target (005 FR-012)
    let x: Vec<f32> = x.iter().zip(&noise).map(|(b, n)| b + n).collect();
    let r = vas_checks::run(make, settings, &x);
    let hang = hang_s(&settings.vas);
    let onset = onset_s(&settings.vas);
    let stim = format!("{FLOOR_S} s of a -60 dBFS floor, +6 dBFS 1 s, 1 s floor");
    let removed = r.removed() as f64;
    // A stream that starts in silence pauses H device samples after the *stream* start, which
    // is the host-to-device converter's delay before the corresponding input time (the
    // converter's empty history counts as silence), while the resume is referenced to the burst
    // through that same delay. The removed length therefore carries one conversion delay,
    // half the bypass latency (001 R-10: two equal converters), 0 at 6 kHz. Scenario (i) does
    // not see it because its first pause follows sound. Measured 4.75–5.2 ms at the converting
    // rates (005 T029 finding); a model property of the pipeline's latency, not slack.
    let d_in = f64::from(
        Pipeline::new(configs::settings("bypass_all", rate))
            .unwrap()
            .latency_samples(),
    ) / 2.0;
    let mut out = vec![
        MeasurementResult::new(
            "005/FR-012",
            "splices after a loud burst following a long floor",
            T_CHAIN,
            rate,
            CONFIG,
            stim.clone(),
            r.events.len() as f64,
            "events",
            Tolerance::Exact(1.0), // 005 FR-012 (iii): exactly one splice
        ),
        MeasurementResult::new(
            "005/FR-012",
            "removed length - (floor - hang + onset + conversion delay)",
            T_CHAIN,
            rate,
            CONFIG,
            format!("{stim}; conversion delay {:.3} ms", d_in / fs * 1e3),
            (removed - (FLOOR_S - hang + onset) * fs - d_in) / fs * 1e3,
            "ms",
            Tolerance::Range(-1.0, 1.0), // engineering target (005 FR-012): ±1 ms
        ),
        MeasurementResult::new(
            "005/FR-012",
            "output length - (input - removed)",
            T_CHAIN,
            rate,
            CONFIG,
            stim.clone(),
            r.y.len() as f64 - (x.len() as f64 - removed),
            "samples",
            Tolerance::Exact(0.0), // 003 FR-004: one output per kept input
        ),
    ];
    // The burst is limited: its steady gain at the host-rate tap is the ceiling margin.
    let trace = agc_checks::gain_trace(make, rate, "agc_only", &x, stimulus::amplitude(LOUD_DBFS));
    let t_burst = n_floor as f64 / fs;
    out.push(MeasurementResult::new(
        "005/FR-012",
        "steady gain during the burst (limited to the ceiling)",
        &["A-028", "engineering target"],
        rate,
        "agc_only",
        stim.clone(),
        agc_checks::steady_db(&trace, fs, t_burst + 0.9, t_burst + 1.0),
        "dB",
        Tolerance::Range(-5.5, -4.5), // A-028: ceiling +1 dBFS against +6 dBFS (engineering target, 005 FR-012)
    ));
    // The clamp: at the record-path tap the burst's peak is exactly 1.0 (6 kHz), else reported.
    let path = configs::settings("record_path_agc", rate);
    let lat = Pipeline::new(path).unwrap().latency_samples() as usize;
    let y = {
        let mut p = make(path);
        let mut y = vec![0.0f32; x.len()];
        let produced = p.process(&x, &mut y).expect("equal lengths").produced;
        y.truncate(produced);
        y
    };
    let seg = &y[(n_floor + lat).min(y.len())..(n_floor + lat + n_burst).min(y.len())];
    let peak = f64::from(seg.iter().fold(0.0f32, |m, v| m.max(v.abs())));
    out.push(MeasurementResult::new(
        "005/FR-012",
        if rate == rr_dr60::DEVICE_RATE_HZ {
            "burst peak at the record-path tap (clamped)"
        } else {
            "burst peak at the record-path tap (interpolated; reported)"
        },
        &["A-030", "A-028"],
        rate,
        "record_path_agc",
        stim,
        peak,
        "FS",
        if rate == rr_dr60::DEVICE_RATE_HZ {
            Tolerance::Exact(1.0) // A-030: full scale
        } else {
            Tolerance::Report
        },
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speech_like_pattern_layout() {
        let rate = 6000;
        let (x, groups) = speech_like_pattern(rate, BURST_DBFS, -60.0, 0x0D60);
        let group_len = 4 * 1800 + 3 * 900;
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0], (3000, 3000 + group_len));
        assert_eq!(groups[1].0, groups[0].1 + 30_000);
        assert_eq!(groups[2].0, groups[1].1 + 30_000);
        assert_eq!(x.len(), groups[2].1 + 6000);
        let peak = x[groups[0].0..groups[0].1]
            .iter()
            .fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(
            (f64::from(peak) - stimulus::amplitude(-30.0)).abs() < 0.002,
            "{peak}"
        );
        assert_eq!(x, speech_like_pattern(rate, BURST_DBFS, -60.0, 0x0D60).0);
    }
}
