//! Spec 005 edge cases (tasks.md T019; spec › Edge Cases; research R-03, R-14).
//!
//! Every limiter observation is at the host-rate "after AGC" tap of the AGC-isolated
//! configuration (`agc_only`), with the exact per-sample gain.

use rr_dr60::{__test_hooks, Pipeline, Settings};
use rr_dr60_harness::agc_checks;
use rr_dr60_harness::analysis::db;
use rr_dr60_harness::checks::{Tolerance, standard};
use rr_dr60_harness::stimulus::{self, Pcg32};
use rr_dr60_harness::{configs, report};

fn run(settings: Settings, x: &[f32]) -> Vec<f32> {
    let mut y = vec![0.0; x.len()];
    let produced = Pipeline::new(settings)
        .unwrap()
        .process(x, &mut y)
        .unwrap()
        .produced;
    y.truncate(produced);
    y
}

/// A 1 kHz tone at the host-grid peak phase (005 R-08).
fn tone(level_dbfs: f64, secs: f64, rate: u32) -> Vec<f32> {
    let fs = f64::from(rate);
    stimulus::tone_at_phase(
        1000.0,
        level_dbfs,
        stimulus::peak_phase(1000.0, rate),
        fs,
        (secs * fs) as usize,
    )
}

/// The onset stimulus: −20 dBFS 0.1 s, then +11 dBFS 0.4 s (data-model.md › Attack step).
fn onset(rate: u32) -> Vec<f32> {
    stimulus::step_at_phase(
        1000.0,
        &[-20.0, 11.0],
        &[0.1, 0.4],
        stimulus::peak_phase(1000.0, rate),
        f64::from(rate),
    )
}

/// Spec Edge Cases › Input that never reaches the knee: 60 s of a −6 dBFS tone at 6 kHz is
/// returned bit for bit, with no drift (FR-003).
#[test]
fn never_reaches_the_knee_for_60_s_is_bit_exact() {
    let x = tone(-6.0, 60.0, 6000);
    let y = run(configs::settings("agc_only", 6000), &x);
    let differing = x
        .iter()
        .zip(&y)
        .filter(|(a, b)| a.to_bits() != b.to_bits())
        .count();
    assert_eq!(differing, 0, "{differing} differing samples in 60 s");
}

/// Spec Edge Cases › Input within 1.5 dB of the knee: the FR-004 row for a tone 0.5 dB above
/// the measured knee is a report row, not a pass/fail row. The level is taken relative to the
/// bisected knee rather than fixed at +1.5 dBFS, so the test asserts the band rule itself and
/// not where the knee happens to read.
#[test]
fn tone_within_the_knee_band_is_reported_not_failed() {
    let rate = 48_000;
    let knee = agc_checks::knee_bisection(rate, &standard, 0.0);
    let rows = agc_checks::static_curve_rows(rate, &standard, &[knee + 0.5], knee);
    report::print(&rows);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].tolerance, Tolerance::Report, "{}", rows[0]);
    assert!(rows[0].passed);
}

/// Spec Edge Cases › Input far above the knee: +20 dBFS is held at the ceiling, the default
/// knee (within the FR-004 tolerance).
#[test]
fn plus_20_dbfs_is_held_at_the_ceiling() {
    let knee = f64::from(rr_dr60::AgcSettings::DEVICE.knee_dbfs); // A-028
    for rate in [6000, 48_000] {
        let x = tone(20.0, 5.0, rate);
        let y = run(configs::settings("agc_only", rate), &x);
        let tail = &y[y.len() - rate as usize..];
        let peak = db(f64::from(tail.iter().fold(0.0f32, |m, v| m.max(v.abs()))));
        eprintln!("{rate} Hz: +20 dBFS in -> peak {peak:+.3} dBFS (ceiling {knee:+.1})");
        assert!((peak - knee).abs() <= 0.5, "{rate} Hz: peak {peak} dBFS"); // 005 FR-004; A-028
    }
}

/// Spec Edge Cases › Input far above the knee: +40 dBFS, beyond the device's own range, stays
/// finite with the gain at or above the 40 dB limit (engineering target).
#[test]
fn plus_40_dbfs_stays_stable() {
    let rate = 48_000;
    let x = tone(40.0, 2.0, rate);
    let trace = agc_checks::gain_trace(&standard, rate, "agc_only", &x, stimulus::amplitude(40.0));
    let y = run(configs::settings("agc_only", rate), &x);
    assert!(y.iter().all(|v| v.is_finite()));
    let gmin = trace
        .iter()
        .copied()
        .filter(|g| !g.is_nan())
        .fold(f64::INFINITY, f64::min);
    eprintln!("+40 dBFS: minimum gain {gmin:.3} dB");
    assert!(gmin >= -40.0 - 1e-3, "gain {gmin} dB"); // engineering target (005 FR-004)
}

/// Spec Edge Cases › DC offset: a −30 dBFS offset added to the static-curve tones moves the
/// bisected knee by under 0.3 dB. The detector's 7 Hz DC block is not modeled (R-14).
#[test]
fn dc_offset_shifts_the_knee_under_0_3_db() {
    let rate = 48_000;
    let plain = agc_checks::knee_bisection(rate, &standard, 0.0);
    let offset = agc_checks::knee_bisection(rate, &standard, stimulus::amplitude(-30.0));
    eprintln!("knee without offset {plain:+.3} dBFS, with a -30 dBFS DC offset {offset:+.3} dBFS");
    // engineering target (005 Edge Cases): under 0.3 dB
    assert!(
        (offset - plain).abs() < 0.3,
        "knee shift {} dB",
        offset - plain
    );
}

/// Spec Edge Cases › DC offset above the knee: a +2 dBFS DC level alone keeps the limiter
/// engaged (steady gain below 0 dB; reported).
#[test]
fn dc_offset_above_the_knee_keeps_the_limiter_engaged() {
    let rate = 48_000;
    let level = stimulus::amplitude(2.0) as f32; // +2 dBFS, 3 dB above the −1 dBFS knee (A-028)
    let x = stimulus::dc(level, 3 * rate as usize);
    let y = run(configs::settings("agc_only", rate), &x);
    let tail = &y[y.len() - 4800..];
    let mean = tail.iter().map(|&v| f64::from(v)).sum::<f64>() / tail.len() as f64;
    let gain = db(mean / f64::from(level));
    eprintln!("+2 dBFS DC alone: steady gain {gain:.3} dB (report)");
    assert!(gain < 0.0, "the limiter did not engage on DC: {gain} dB");
    assert!(gain > -3.0, "implausible DC gain {gain} dB");
}

/// Spec Edge Cases › Non-finite input: NaN, ±Inf and subnormal samples in a +11 dBFS tone act
/// as 0.0 before the detector sees them: the output equals the same tone with 0.0 in their
/// place.
#[test]
fn non_finite_input_is_zeroed_before_the_detector() {
    let rate = 48_000;
    let clean = tone(11.0, 0.5, rate);
    let (mut bad, mut zeroed) = (clean.clone(), clean);
    for (i, v) in [
        (4_000, f32::NAN),
        (9_000, f32::INFINITY),
        (15_000, f32::NEG_INFINITY),
        (15_001, f32::from_bits(1)),
    ] {
        bad[i] = v;
        zeroed[i] = 0.0;
    }
    let s = configs::settings("agc_only", rate);
    let y = run(s, &bad);
    assert!(y.iter().all(|v| v.is_finite()));
    assert_eq!(y, run(s, &zeroed));
}

/// Spec Edge Cases › Reset or reconfigure mid-attack: the control returns to the discharged
/// state and the next onset has the full dead time, bit for bit as a fresh pipeline.
#[test]
fn reset_mid_attack_restores_the_full_dead_time() {
    for how in ["reset", "reconfigure"] {
        let rate = 48_000;
        let s = configs::settings("agc_only", rate);
        let x = onset(rate);
        let fresh = run(s, &x);
        let mut p = Pipeline::new(s).unwrap();
        // 1 ms into the +11 dBFS segment: mid dead time (3 ms).
        let cut = (0.1 * f64::from(rate)) as usize + 48;
        let mut partial = x[..cut].to_vec();
        let _ = p.process_in_place(&mut partial);
        assert!(__test_hooks::limiter_state(&p) > 0.0, "{how}: not charging");
        match how {
            "reset" => p.reset(),
            _ => p.reconfigure(s).unwrap(),
        }
        assert_eq!(__test_hooks::limiter_state(&p), 0.0, "{how}");
        let mut y = vec![0.0; x.len()];
        let produced = p.process(&x, &mut y).unwrap().produced;
        y.truncate(produced);
        assert_eq!(y, fresh, "{how}");
    }
}

/// Spec Edge Cases › Reconfigure while armed: 2 s after a loud burst the control is still
/// charged; `reconfigure` discharges it and the next onset again has the full dead time.
#[test]
fn reconfigure_discharges() {
    let rate = 6000;
    let s = configs::settings("agc_only", rate);
    let mut p = Pipeline::new(s).unwrap();
    let mut armed = stimulus::step_at_phase(
        1000.0,
        &[11.0, -20.0],
        &[0.2, 2.0],
        stimulus::peak_phase(1000.0, rate),
        f64::from(rate),
    );
    let _ = p.process_in_place(&mut armed);
    let state = __test_hooks::limiter_state(&p);
    assert!(state > 0.5, "not armed: {state} V"); // A-028: armed between about 0.5 and 1.1 V
    p.reconfigure(s).unwrap();
    assert_eq!(__test_hooks::limiter_state(&p), 0.0);
    let x = onset(rate);
    let mut y = vec![0.0; x.len()];
    let produced = p.process(&x, &mut y).unwrap().produced;
    y.truncate(produced);
    assert_eq!(y, run(s, &x));
}

/// Spec Edge Cases › Block boundaries (FR-014): at 44.1 kHz the onset stimulus in 50 random
/// partitions gives identical output and identical limiter state.
#[test]
fn block_partitions_give_identical_output_and_state() {
    let rate = 44_100;
    let s = configs::settings("agc_only", rate);
    let x = onset(rate);
    let mut whole = Pipeline::new(s).unwrap();
    let mut want = vec![0.0; x.len()];
    let _ = whole.process(&x, &mut want).unwrap();
    let want_state = __test_hooks::limiter_state(&whole);
    let mut rng = Pcg32::new(0x0D60, 7);
    for i in 0..50 {
        let mut p = Pipeline::new(s).unwrap();
        let mut out = Vec::with_capacity(x.len());
        let mut pos = 0;
        while pos < x.len() {
            let n = (rng.below(4097) as usize).min(x.len() - pos);
            let mut y = vec![0.0; n];
            let produced = p.process(&x[pos..pos + n], &mut y).unwrap().produced;
            out.extend_from_slice(&y[..produced]);
            pos += n;
        }
        assert_eq!(out, want, "partition {i}");
        assert_eq!(
            __test_hooks::limiter_state(&p).to_bits(),
            want_state.to_bits(),
            "partition {i}: state"
        );
    }
}
