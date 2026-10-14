//! Spec 008 FR-022: the seven-rate codec matrix (SC-001 – SC-009; task T029), every
//! `codec_checks` check of US1 and US2 in one `-- --nocapture` report; the release-only cases
//! (the 60-minute recording through the codec and the 7 × 7 cross-rate grid, `--ignored`); the
//! calibration reader (research R-09; `--ignored`: needs the decoded references that
//! `tools/celp-calibrate/calibrate.py` writes); and the stimulus digests that pin the
//! calibration's inputs (T008).
//!
//! `cargo test -p rr_dr60_harness --test codec_matrix -- --nocapture`

use rr_dr60_harness::checks::{MeasurementResult, assert_all};
use rr_dr60_harness::stimulus::{unvoiced_voiced, voiced};
use rr_dr60_harness::{codec_checks, configs, report};
use sha2::{Digest, Sha256};

type Check = (&'static str, fn(u32) -> Vec<MeasurementResult>);

/// Every per-rate check (research R-09; US1 and US2).
const CHECKS: &[Check] = &[
    ("bit_rate", codec_checks::bit_rate),
    ("delay_lag", codec_checks::delay_lag),
    ("one_pass_equivalence", codec_checks::one_pass_equivalence),
    (
        "after_vas_tap_unchanged",
        codec_checks::after_vas_tap_unchanged,
    ),
    ("vas_events_unchanged", codec_checks::vas_events_unchanged),
    ("latency_bound", codec_checks::latency_bound),
    ("frame_reports", codec_checks::frame_reports),
    ("bypassed_equals_006", codec_checks::bypassed_equals_006),
    ("footprint", codec_checks::footprint),
    ("capacity_frames", codec_checks::capacity_frames),
    ("mid_frame_stop", codec_checks::mid_frame_stop),
    ("round_trip", codec_checks::round_trip),
    ("corruption_set", codec_checks::corruption_set),
    (
        "vas_splices_inside_frames",
        codec_checks::vas_splices_inside_frames,
    ),
];

/// The full matrix: every check at the seven rates, the three-level transparency and noise
/// checks, and the causality probe; one report, every row within tolerance.
#[test]
fn codec_matrix() {
    let mut results = Vec::new();
    let mut counts = Vec::new();
    for (name, check) in CHECKS {
        let before = results.len();
        for rate in configs::all_rates() {
            results.extend(check(rate));
        }
        counts.push((*name, results.len() - before));
    }
    for (name, check) in [
        (
            "transparency",
            codec_checks::transparency as fn(u32, f64) -> Vec<MeasurementResult>,
        ),
        ("noise_floor", codec_checks::noise_floor),
    ] {
        let before = results.len();
        for rate in configs::all_rates() {
            for level in codec_checks::LEVELS_DBFS {
                results.extend(check(rate, level));
            }
        }
        counts.push((name, results.len() - before));
    }
    let before = results.len();
    results.extend(codec_checks::frame_causality());
    counts.push(("frame_causality (6 kHz)", results.len() - before));
    report::print(&results);
    for (name, n) in &counts {
        println!("{name:>32}: {n} rows");
    }
    println!("codec matrix: {} rows at seven rates", results.len());
    assert_all(&results);
}

/// The 60-minute recording through the codec at the device's own capacity (S-001), 6 kHz;
/// release only.
#[test]
#[ignore = "60 minutes of audio through the codec; run with --release -- --ignored"]
fn sixty_minutes_6k() {
    let results = codec_checks::sixty_minutes();
    report::print(&results);
    assert_all(&results);
}

/// The 7 × 7 cross-rate grid with a 5 s voiced recording (FR-010); release only.
#[test]
#[ignore = "49 five-second recordings through the codec; run with --release -- --ignored"]
fn cross_rate_grid() {
    let mut results = Vec::new();
    for record_rate in configs::all_rates() {
        for play_rate in configs::all_rates() {
            results.extend(codec_checks::cross_rate(record_rate, play_rate));
        }
    }
    report::print(&results);
    println!("cross-rate grid: {} rows", results.len());
    assert_all(&results);
}

/// The calibration stimuli at 8 kHz, as `examples/codec_stimuli.rs` writes them (stem, voiced?,
/// level dBFS, SHA-256 of the raw s16le bytes). A stimulus change moves a digest and
/// invalidates `specs/008-celp-codec/calibration.md` visibly (coach review Q5).
const STIMULUS_DIGESTS: [(&str, bool, f64, &str); 6] = [
    (
        "voiced_m10",
        true,
        -10.0,
        "aa1412f569ee1785e34612acbce3e404a89975670605b7455d34bdca650512cd",
    ),
    (
        "voiced_m20",
        true,
        -20.0,
        "6ea93374468266ce22f1eae4ac4872a49284292599ca9d3b12316861147727d7",
    ),
    (
        "voiced_m40",
        true,
        -40.0,
        "f123678dfaa5701caf24106ce042e41ef91132d13f1f84022927238536c465f6",
    ),
    (
        "unvoiced_voiced_m10",
        false,
        -10.0,
        "6bb76c9be4bedceb8f287eda966e60f4c30692664a1ef347d0976dd644d7a122",
    ),
    (
        "unvoiced_voiced_m20",
        false,
        -20.0,
        "bc2cfa6e15e6a55c20bf6cd5b16dcc3d909e0fffcca86c6440c8250e1628b6a0",
    ),
    (
        "unvoiced_voiced_m40",
        false,
        -40.0,
        "a701f3469d9947f8d6917bac16eebc13438355777c932de1b112789c4502b3c0",
    ),
];

fn raw_s16le(x: &[f32]) -> Vec<u8> {
    x.iter()
        .flat_map(|&v| {
            ((f64::from(v) * 32768.0).round().clamp(-32768.0, 32767.0) as i16).to_le_bytes()
        })
        .collect()
}

/// T008: the six 8 kHz calibration stimuli have the recorded digests.
#[test]
fn stimulus_digests() {
    for (stem, kind, level, want) in STIMULUS_DIGESTS {
        let x = if kind {
            voiced(8000.0, 10.0, level, 0x0D60)
        } else {
            unvoiced_voiced(8000.0, 10.0, level, 0x0D60)
        };
        let got: String = Sha256::digest(raw_s16le(&x))
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(
            got, want,
            "{stem}: the calibration stimulus changed; re-run the calibration"
        );
    }
}

/// Research R-09 (T009): reads the stimuli and the decoded G.723.1 references under
/// `tools/celp-calibrate/out/` when present and prints the same metrics the codec checks use.
/// `--ignored`: the references come from the user-run calibration tool, never from CI.
#[test]
#[ignore = "needs tools/celp-calibrate/out/*.g723.raw from the calibration tool"]
fn calibration() {
    use rr_dr60_harness::analysis::{
        level_track, lpc_lsd, rms_dbfs, segmental_snr, third_octave_levels, xcorr_lag,
    };
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/celp-calibrate/out");
    let read = |p: &std::path::Path| -> Option<Vec<f32>> {
        let bytes = std::fs::read(p).ok()?;
        Some(
            bytes
                .chunks_exact(2)
                .map(|c| f32::from(i16::from_le_bytes([c[0], c[1]])) / 32768.0)
                .collect(),
        )
    };
    let fs = 8000.0;
    println!("G.723.1 5.3 kbit/s reference figures (8 kHz; the same metrics as codec_checks):");
    println!(
        "{:<22} {:>7} {:>8} {:>7} {:>7} {:>7}  third-octave deviations (dB)",
        "stimulus", "lag", "rms dB", "lsd", "segsnr", "track"
    );
    let mut any = false;
    for (stem, _, _, _) in STIMULUS_DIGESTS {
        let (Some(x), Some(y)) = (
            read(&dir.join(format!("{stem}.raw"))),
            read(&dir.join(format!("{stem}.g723.raw"))),
        ) else {
            println!("{stem:<22} (missing; run tools/celp-calibrate/calibrate.py)");
            continue;
        };
        any = true;
        let lag = xcorr_lag(&x, &y, 800).max(0) as usize;
        let n = x.len().min(y.len() - lag);
        let (xs, ys) = (&x[..n], &y[lag..lag + n]);
        let bx = third_octave_levels(xs, fs);
        let by = third_octave_levels(ys, fs);
        let devs: Vec<String> = bx
            .iter()
            .zip(&by)
            .map(|(a, b)| format!("{:+.1}", b - a))
            .collect();
        println!(
            "{stem:<22} {lag:>7} {:>8.2} {:>7.2} {:>7.2} {:>7.2}  {}",
            rms_dbfs(ys) - rms_dbfs(xs),
            lpc_lsd(xs, ys, fs),
            segmental_snr(&x, &y, fs, lag),
            level_track(&x, &y, fs, lag),
            devs.join(" ")
        );
    }
    assert!(any, "no calibration files found under {}", dir.display());
}
