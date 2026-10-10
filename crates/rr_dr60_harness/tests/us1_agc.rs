//! Spec 002 User Story 1: hear the RR-DR60's auto-gain character (tasks.md T021).
//!
//! Standalone: uses only the pipeline and the Phase 2 analysis helpers, not the US3 checks
//! (Principle VII). AGC measurements use the AGC-isolated configuration `agc_only`.

use rr_dr60::{Pipeline, SUPPORTED_HOST_RATES, Settings, Tap};
use rr_dr60_harness::analysis::{db, gain_trajectory_exact, single_bin};
use rr_dr60_harness::{configs, stimulus};

const FS: u32 = 48_000;

fn process(settings: Settings, x: &[f32]) -> Vec<f32> {
    let mut y = vec![0.0; x.len()];
    let produced = Pipeline::new(settings)
        .unwrap()
        .process(x, &mut y)
        .unwrap()
        .produced;
    y.truncate(produced);
    y
}

/// Steady 1 kHz output level in dBFS (AES17 peak amplitude), over the last 0.25 s.
fn steady_level(settings: Settings, input_dbfs: f64) -> f64 {
    let fs = f64::from(settings.host_rate_hz);
    let x = stimulus::step(1000.0, &[input_dbfs], &[1.0], fs);
    let y = process(settings, &x);
    let len = (0.25 * fs) as usize;
    db(single_bin(&y, 1000.0, fs, y.len() - len, len).norm())
}

/// US1 AS1 (002 FR-004, A-017): −40 / −10 / 0 dBFS in → −13 / −10 / −9 dBFS out, ±1 dB.
#[test]
fn as1_regulation() {
    for (input, want) in [(-40.0, -13.0), (-10.0, -10.0), (0.0, -9.0)] {
        let got = steady_level(configs::settings("agc_only", FS), input);
        eprintln!("US1 AS1: {input} dBFS in → {got:.3} dBFS out (want {want} ± 1)");
        assert!(
            (got - want).abs() <= 1.0,
            "{input} dBFS → {got:.3} dBFS, want {want} ± 1"
        );
    }
}

/// US1 AS2 (002 FR-005, A-017): −60 dBFS is below the regulated range: +40 dB → −20 dBFS.
#[test]
fn as2_maximum_gain() {
    let got = steady_level(configs::settings("agc_only", FS), -60.0);
    eprintln!("US1 AS2: −60 dBFS in → {got:.3} dBFS out (want −20 ± 1)");
    assert!(
        (got + 20.0).abs() <= 1.0,
        "−60 dBFS → {got:.3} dBFS, want −20 ± 1"
    );
}

/// US1 AS3 (002 FR-006, A-018): attack 10 ms ± 2 ms and release 1.0 s ± 0.2 s at 48 kHz.
///
/// Gain trajectory = output ÷ a reference run with the AGC bypassed, sample by sample, where
/// the reference is at least 0.1 × the tone amplitude (research.md R-11, as corrected in
/// T026). Both runs share the rate conversion, so the ratio is the AGC's gain at every host
/// sample, already latency-aligned. An FFT envelope reads ≈1.6 ms long here at every rate,
/// because the 1 kHz tone is too slow a carrier for a 3.7 ms time constant.
#[test]
fn as3_attack_and_release() {
    let fs = f64::from(FS);
    let ms = |t: f64| (t * fs / 1000.0) as usize;
    let segments = [(-40.0, 0.5), (-10.0, 0.3), (-40.0, 2.0)];
    let x = stimulus::step(1000.0, &segments.map(|s| s.0), &segments.map(|s| s.1), fs);
    let on = configs::settings("agc_only", FS);
    let mut off = on;
    off.agc.enabled = false;
    let (y, y_ref) = (process(on, &x), process(off, &x));
    let lat = Pipeline::new(on).unwrap().latency_samples() as usize;
    let (s1, s2) = (ms(500.0) + lat, ms(800.0) + lat);
    // Gate on the low level so both segments are measured.
    let traj = gain_trajectory_exact(&y_ref, &y, stimulus::amplitude(-40.0));
    let segment = |from: usize, to: usize| -> Vec<(usize, f64)> {
        traj.iter()
            .filter(|&&(n, _)| n >= from && n < to)
            .map(|&(n, g)| (n - from, g))
            .collect()
    };
    let mean = |t: &[(usize, f64)]| t.iter().map(|p| p.1).sum::<f64>() / t.len() as f64;

    let before = mean(&segment(s1 - ms(50.0), s1 - ms(10.0)));
    let up = segment(s1, s2 - ms(10.0));
    let fin_up = mean(&segment(s2 - ms(50.0), s2 - ms(10.0)));
    let attack_ms = settle(&up, fin_up, before - fin_up) as f64 / fs * 1000.0;
    eprintln!("US1 AS3: attack {attack_ms:.2} ms (want 10 ± 2)");
    assert!((attack_ms - 10.0).abs() <= 2.0, "attack {attack_ms:.2} ms");

    let down = segment(s2, y.len() - ms(50.0));
    let fin_down = mean(&segment(y.len() - ms(150.0), y.len() - ms(50.0)));
    let release_s = settle(&down, fin_down, fin_down - fin_up) as f64 / fs;
    eprintln!("US1 AS3: release {release_s:.4} s (want 1.0 ± 0.2)");
    assert!((release_s - 1.0).abs() <= 0.2, "release {release_s:.4} s");
}

/// Samples from the start of the trajectory to just after the last point outside the
/// settling band (2/27 of the excursion, spec 002 Overview).
fn settle(traj: &[(usize, f64)], final_db: f64, excursion: f64) -> usize {
    let band = excursion.abs() * 2.0 / 27.0;
    traj.iter()
        .rev()
        .find(|(_, v)| (v - final_db).abs() > band)
        .map_or(0, |&(n, _)| n + 1)
}

/// US1 AS4 (002 FR-007): digital silence in gives exact zeros out, isolated and by default.
#[test]
fn as4_silence() {
    for config in ["agc_only", "default_agc"] {
        let y = process(
            configs::settings(config, FS),
            &stimulus::silence(FS as usize),
        );
        assert!(y.iter().all(|&v| v.to_bits() == 0), "{config}");
    }
}

/// US1 AS5: the real default pipeline (AGC and both band-limit stages on) gives −13 dBFS ±
/// 1.2 dB for a −40 dBFS 1 kHz tone (AGC ±1 dB plus the stages' ±0.2 dB, A-015).
#[test]
fn as5_default_pipeline() {
    let got = steady_level(Settings::new(FS), -40.0);
    eprintln!("US1 AS5: default pipeline, −40 dBFS in → {got:.3} dBFS out (want −13 ± 1.2)");
    assert!((got + 13.0).abs() <= 1.2, "default pipeline: {got:.3} dBFS");
}

/// 002 FR-010: the AGC adds no latency, for every tap at every rate.
#[test]
fn fr010_no_added_latency() {
    for rate in SUPPORTED_HOST_RATES {
        for tap in [Tap::AfterAgc, Tap::AfterRecord, Tap::AfterPlayback] {
            let mut on = Settings::new(rate);
            on.tap = tap;
            let mut off = on;
            off.agc.enabled = false;
            assert_eq!(
                Pipeline::new(on).unwrap().latency_samples(),
                Pipeline::new(off).unwrap().latency_samples(),
                "{rate} Hz, {tap:?}"
            );
        }
    }
}
