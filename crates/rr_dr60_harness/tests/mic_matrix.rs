//! Spec 012 seven-rate report of the microphone path (tasks.md T032, T036): User Story 1's rows
//! at every host rate (the band's deviations from the prototype, the realized −3 dB points, the
//! 1 kHz group delay, the hiss's level and shape at the offsets, the streams' cross-correlations)
//! and User Story 2's (the clipper's static curve, onsets, limits, bound, harmonics and
//! recovery), each asserted. Report: `cargo test -p rr_dr60_harness --test mic_matrix -- --nocapture`.
//! Release-only: `cargo test -p rr_dr60_harness --release --test mic_matrix -- --ignored
//! --nocapture` (the 60-minute hiss at 48 kHz, US1 AS5).
//!
//! The band, the hiss and the clipper are per-rate figures and cheap, so they run at all seven
//! rates (`configs::all_rates`); the streams process 1.4 M samples per rate and their figures do
//! not depend on the rate (the draws are the same, only σ scales them), so they run at the
//! measurement rates (`configs::measurement_rates`).

use rr_dr60::Pipeline;
use rr_dr60_harness::analysis;
use rr_dr60_harness::checks::{MeasurementResult, Tolerance, assert_all, standard};
use rr_dr60_harness::mic_checks as mic;
use rr_dr60_harness::{configs, report};

fn finish(results: Vec<MeasurementResult>) {
    report::print(&results);
    assert!(!results.is_empty(), "no measurements");
    assert_all(&results);
}

/// US1 AS1 (SC-002) at the seven rates: the band, the group delay and the −3 dB points.
#[test]
fn band_at_seven_rates() {
    finish(
        configs::all_rates()
            .iter()
            .flat_map(|&r| mic::band(&standard, r))
            .collect(),
    );
}

/// US1 AS2, AS3 (SC-001) at the seven rates: the hiss at offsets 0, −18, −6 and +12 dB.
#[test]
fn hiss_at_seven_rates() {
    finish(
        configs::all_rates()
            .iter()
            .flat_map(|&r| {
                [0.0, -18.0, -6.0, 12.0]
                    .iter()
                    .flat_map(move |&o| mic::hiss_level(&standard, r, o))
                    .collect::<Vec<_>>()
            })
            .collect(),
    );
}

/// US2 AS1 – AS4 (SC-003) at the seven rates: the static curve, the engine-input onsets and
/// limits, the bound, the harmonics against the discrete model (with the alias levels) and the
/// recovery.
#[test]
fn clipper_at_seven_rates() {
    finish(
        configs::all_rates()
            .iter()
            .flat_map(|&r| {
                let mut rows = mic::static_curve(&standard, r);
                rows.extend(mic::engine_onsets(&standard, r));
                rows.extend(mic::bound(&standard, r));
                rows.extend(mic::harmonics(&standard, r));
                rows.extend(mic::recovery(&standard, r));
                rows
            })
            .collect(),
    );
}

/// US1 AS4 (SC-005): the streams' cross-correlations.
#[test]
fn streams_report() {
    finish(
        configs::measurement_rates()
            .iter()
            .flat_map(|&r| mic::streams(&standard, r))
            .collect(),
    );
}

/// US1 AS5 (release-only): every 10 s window of a 60-minute hiss at 48 kHz (`mic_only`) is
/// within ±0.5 dB of the figure in the 300 – 3000 Hz band.
#[test]
#[ignore = "release-only: 60 minutes at 48 kHz (cargo test --release --test mic_matrix -- --ignored)"]
fn sixty_minute_hiss_is_stable_at_48k() {
    let rate = 48_000u32;
    let fs = f64::from(rate);
    let window = 10 * rate as usize; // US1 AS5: 10 s windows
    let windows = 360; // 60 minutes
    let mut p: Pipeline = standard(mic::mic_only(rate, 0.0));
    let zeros = vec![0.0f32; window];
    let mut y = vec![0.0f32; window];
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    let mut rows = Vec::new();
    for w in 0..windows {
        assert_eq!(p.process(&zeros, &mut y).unwrap().produced, window);
        let dev = analysis::band_level_aes17(&y, fs, 300.0, 3000.0) - mic::HISS_BAND_DBFS;
        lo = lo.min(dev);
        hi = hi.max(dev);
        if !(-mic::HISS_TOL_DB..=mic::HISS_TOL_DB).contains(&dev) {
            rows.push((w, dev));
        }
    }
    println!(
        "60-minute hiss at 48 kHz: 10 s windows within {lo:+.3} … {hi:+.3} dB of {} dBFS",
        mic::HISS_BAND_DBFS
    );
    finish(vec![
        MeasurementResult::new(
            "SC-001",
            "60-minute hiss, lowest 10 s window re the figure",
            mic::T_HISS,
            rate,
            "mic_only",
            "silence 60 min",
            lo,
            "dB",
            Tolerance::AtLeast(-mic::HISS_TOL_DB), // US1 AS5
        ),
        MeasurementResult::new(
            "SC-001",
            "60-minute hiss, highest 10 s window re the figure",
            mic::T_HISS,
            rate,
            "mic_only",
            "silence 60 min",
            hi,
            "dB",
            Tolerance::AtMost(mic::HISS_TOL_DB), // US1 AS5
        ),
    ]);
    assert!(rows.is_empty(), "windows outside ±0.5 dB: {rows:?}");
}
