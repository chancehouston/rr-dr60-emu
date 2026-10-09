//! Spec 002 User Story 2: tune, bypass, or tap the AGC (tasks.md T027).

use rr_dr60::{Error, Pipeline, SUPPORTED_HOST_RATES, Setting, Settings};
use rr_dr60_harness::analysis::{db, gain_trajectory_exact, single_bin};
use rr_dr60_harness::{configs, golden, stimulus};

fn process(settings: Settings, x: &[f32]) -> Vec<f32> {
    let mut y = vec![0.0; x.len()];
    Pipeline::new(settings).unwrap().process(x, &mut y).unwrap();
    y
}

/// US2 AS1 (002 FR-002, FR-018): with the AGC bypassed, every spec 001 golden entry is
/// reproduced bit for bit.
#[test]
fn as1_bypassed_agc_reproduces_spec_001_golden_file() {
    let actual = golden::generate(&|s| {
        assert!(!s.agc.enabled, "spec 001 configurations must have the AGC bypassed");
        Pipeline::new(s).unwrap()
    });
    golden::compare(&golden::committed(), &actual).unwrap();
}

/// US2 AS2 and 002 FR-003: the tap after the AGC ignores the band-limit stage settings, and
/// equals the AGC-isolated configuration with the tap after playback.
#[test]
fn as2_tap_after_agc_ignores_stage_settings() {
    for rate in SUPPORTED_HOST_RATES {
        let fs = f64::from(rate);
        let x = stimulus::step(1000.0, &[-40.0, -10.0, -40.0], &[0.2, 0.2, 0.3], fs);
        let isolated = process(configs::settings("agc_only", rate), &x);
        let stages_on = process(configs::settings("agc_tap_stages_on", rate), &x);
        let after_playback = process(configs::settings("agc_isolated_after_playback", rate), &x);
        assert_eq!(stages_on, isolated, "{rate} Hz: stage settings changed AfterAgc output");
        assert_eq!(after_playback, isolated, "{rate} Hz");
    }
}

/// US2 AS3 (002 FR-012): a 3 s release measures 3 s ± 20 % (exact gain at 8 kHz).
#[test]
fn as3_release_three_seconds() {
    let fs = 8000.0;
    let mut s = configs::settings("agc_only", 8000);
    s.agc.release_ms = 3000.0;
    // 002 step stimulus: 0.5 s low, ≥ 20 × attack high, ≥ 1.5 × release + 0.5 s low.
    let x = stimulus::step(1000.0, &[-40.0, -10.0, -40.0], &[0.5, 0.3, 5.0], fs);
    let y = process(s, &x);
    let traj = gain_trajectory_exact(&x, &y, stimulus::amplitude(-40.0));
    let s2 = (0.8 * fs) as usize;
    let down: Vec<(usize, f64)> = traj
        .iter()
        .filter(|&&(n, _)| n >= s2)
        .map(|&(n, g)| (n - s2, g))
        .collect();
    let start = traj.iter().rfind(|&&(n, _)| n < s2).unwrap().1;
    let fin = down[down.len() - 1].1;
    let band = (fin - start).abs() * 2.0 / 27.0;
    let release_s = down
        .iter()
        .rfind(|(_, g)| (g - fin).abs() > band)
        .map_or(0, |&(n, _)| n + 1) as f64
        / fs;
    eprintln!("US2 AS3: release {release_s:.3} s (want 3 ± 0.6)");
    assert!((release_s - 3.0).abs() <= 0.6, "release {release_s:.3} s");
}

/// US2 AS4 (002 FR-012, A-017): target −20 dBFS with a −30 dBFS tone → −21 dBFS ± 1 dB.
#[test]
fn as4_target_minus_20() {
    let rate = 48_000;
    let fs = f64::from(rate);
    let mut s = configs::settings("agc_only", rate);
    s.agc.target_dbfs = -20.0;
    let y = process(s, &stimulus::step(1000.0, &[-30.0], &[1.0], fs));
    let len = (0.25 * fs) as usize;
    let got = db(single_bin(&y, 1000.0, fs, y.len() - len, len).norm());
    eprintln!("US2 AS4: −30 dBFS in, target −20 → {got:.3} dBFS (want −21 ± 1)");
    assert!((got + 21.0).abs() <= 1.0, "{got:.3} dBFS");
}

/// US2 AS5 (002 FR-011): an out-of-range value of any AGC field is rejected by `new` and by
/// `reconfigure`, naming the field; a failed reconfigure leaves the pipeline unchanged.
#[test]
fn as5_invalid_settings_are_named_and_rejected() {
    let cases: [(Setting, fn(&mut Settings)); 5] = [
        (Setting::AgcTargetDbfs, |s| s.agc.target_dbfs = 0.5),
        (Setting::AgcMaxGainDb, |s| s.agc.max_gain_db = 61.0),
        (Setting::AgcMaxAttenuationDb, |s| s.agc.max_attenuation_db = -1.0),
        (Setting::AgcAttackMs, |s| s.agc.attack_ms = 0.0),
        (Setting::AgcReleaseMs, |s| s.agc.release_ms = 10_001.0),
    ];
    let x = stimulus::step(1000.0, &[-30.0], &[0.2], 48_000.0);
    for (setting, break_it) in cases {
        let mut bad = Settings::new(48_000);
        break_it(&mut bad);
        let want = Error::InvalidSetting { setting };
        assert_eq!(Pipeline::new(bad).err(), Some(want));
        assert!(want.to_string().contains("agc."), "{want}");

        let mut p = Pipeline::new(Settings::new(48_000)).unwrap();
        let mut warm = x.clone();
        p.process_in_place(&mut warm);
        let mut control = p.clone();
        assert_eq!(p.reconfigure(bad), Err(want));
        assert_eq!(p.settings(), control.settings());
        let (mut a, mut b) = (x.clone(), x.clone());
        p.process_in_place(&mut a);
        control.process_in_place(&mut b);
        assert_eq!(a, b, "{setting:?}: failed reconfigure changed the pipeline");
    }
}
