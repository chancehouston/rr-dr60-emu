//! Spec 003 User Story 2: tune, bypass, tap, or mute the VAS (tasks.md T026).
//!
//! Acceptance scenarios AS1–AS6 with plain assertions (no `vas_checks`, per plan › Story
//! boundaries). Lengths and timings are measured from `produced` and the events, never from an
//! amplitude envelope (research R-11).

use rr_dr60::{BlockInfo, Error, Pipeline, Setting, Settings, Tap, VasEvent, VasMode};
use rr_dr60_harness::{configs, golden, stimulus};

/// The burst level: the default threshold (−18 dBFS, A-022) + 10 dB (spec 003 definitions).
const BURST_DBFS: f64 = -8.0;

/// Processes `x` in one block; returns the output, its events and the block result.
fn run(settings: Settings, x: &[f32]) -> (Vec<f32>, Vec<VasEvent>, BlockInfo) {
    let mut p = Pipeline::new(settings).unwrap();
    let mut y = vec![0.0f32; x.len()];
    let mut ev = vec![VasEvent::default(); p.max_events(x.len())];
    let info = p.process_with_events(x, &mut y, &mut ev).unwrap();
    y.truncate(info.produced);
    ev.truncate(info.events);
    (y, ev, info)
}

fn burst_gap(segments: &[(bool, f64)], rate: u32) -> Vec<f32> {
    stimulus::burst_gap(1000.0, BURST_DBFS, segments, f64::from(rate))
}

/// US2 AS1 (FR-002, FR-020, SC-003): with the VAS bypassed, every spec 001 and 002 golden entry
/// matches with zero differences, and every block produces exactly as many samples as it
/// consumed.
#[test]
fn as1_bypassed_vas_reproduces_the_001_and_002_golden_files() {
    let make = |s| Pipeline::new(s).unwrap();
    golden::compare(&golden::committed(), &golden::generate(&make)).unwrap();
    golden::compare(&golden::committed_agc(), &golden::generate_agc(&make)).unwrap();

    for rate in configs::all_rates() {
        let x = golden::stimulus("sweep_log", rate);
        for config in configs::CONFIGS.iter().chain(&configs::AGC_CONFIGS) {
            let s = configs::settings(config, rate);
            assert!(!s.vas.enabled, "{config} must bypass the VAS (FR-020)");
            let mut p = Pipeline::new(s).unwrap();
            for block in x.chunks(1000) {
                let mut y = vec![0.0f32; block.len()];
                let info = p.process(block, &mut y).unwrap();
                assert_eq!(info.produced, block.len(), "{rate} Hz {config}");
                assert_eq!(info.events, 0);
                assert!(!info.paused);
            }
        }
    }
}

/// US2 AS2 (FR-003): the tap "after VAS" equals the same pipeline with stage 10 bypassed, sample
/// for sample, with identical events, at every rate.
#[test]
fn as2_tap_after_vas_equals_stage_10_bypassed() {
    for rate in configs::all_rates() {
        let x = burst_gap(&[(true, 0.5), (false, 2.0), (true, 0.5)], rate);
        let mut tapped = Settings::new(rate);
        tapped.tap = Tap::AfterVas;
        let mut bypassed = tapped;
        bypassed.playback_stage_enabled = false;
        let (y_tap, ev_tap, info_tap) = run(tapped, &x);
        let (y_off, ev_off, info_off) = run(bypassed, &x);
        assert_eq!(y_tap, y_off, "{rate} Hz");
        assert_eq!(ev_tap, ev_off, "{rate} Hz");
        assert_eq!(info_tap, info_off, "{rate} Hz");
        assert_eq!(ev_tap.len(), 1, "{rate} Hz: one splice expected");
    }
}

/// The lowest 1 kHz level, to 0.05 dB (engineering target, 003 R-15), that is kept in full for
/// H + 1 s with the given settings, by bisection between `lo` (dropped) and `hi` (kept).
fn lowest_kept_level_dbfs(settings: Settings, mut lo: f64, mut hi: f64) -> f64 {
    let fs = f64::from(settings.host_rate_hz);
    let secs = f64::from(settings.vas.hang_ms) / 1000.0 + 1.0;
    let kept_in_full = |level: f64| {
        let x = stimulus::step(1000.0, &[level], &[secs], fs);
        let (y, ev, _) = run(settings, &x);
        y.len() == x.len() && ev.is_empty()
    };
    assert!(!kept_in_full(lo), "{lo} dBFS must be dropped");
    assert!(kept_in_full(hi), "{hi} dBFS must be kept");
    while hi - lo > 0.05 {
        let mid = 0.5 * (lo + hi);
        if kept_in_full(mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    hi
}

/// US2 AS3 (FR-007, A-022): at sensitivity 1 and 5 the threshold is 6 dB above and 6 dB below
/// the level-3 threshold (−12 and −24 dBFS), each ±1 dB.
#[test]
fn as3_sensitivity_1_and_5_move_the_threshold_by_6_db() {
    for (level, want) in [(1u8, -12.0), (5u8, -24.0)] {
        let mut s = configs::settings("vas_only", 48_000);
        s.vas.sensitivity = level;
        let got = lowest_kept_level_dbfs(s, -40.0, 0.0);
        eprintln!("US2 AS3: sensitivity {level}: threshold {got:.2} dBFS (want {want} ± 1)");
        let tol = 1.0; // ±1 dB, engineering target (003 FR-017)
        assert!((got - want).abs() <= tol, "level {level}: {got} dBFS");
    }
}

/// US2 AS4 (FR-008, FR-013): with a hang time of 3 s, a burst, a 10 s gap and a burst keep
/// 3 s ± 1 ms of the gap, at 8 and 48 kHz. The kept gap is read from the splice's removed length
/// (gap − removed + onset), so the end-of-stream latency doesn't matter (R-11).
#[test]
fn as4_hang_time_3_s_keeps_3_s_of_the_gap() {
    for rate in [8_000u32, 48_000] {
        let fs = f64::from(rate);
        let mut s = configs::settings("vas_only", rate);
        s.vas.hang_ms = 3000.0;
        let x = burst_gap(&[(true, 1.0), (false, 10.0), (true, 1.0)], rate);
        let (_, ev, _) = run(s, &x);
        assert_eq!(ev.len(), 1, "{rate} Hz");
        let onset = f64::from(s.vas.onset_ms) / 1000.0 * fs; // A-024
        let kept_gap = 10.0 * fs - ev[0].input_length as f64 + onset;
        let tol = 0.001 * fs; // ±1 ms, engineering target (003 FR-017)
        eprintln!(
            "US2 AS4: {rate} Hz: kept gap {kept_gap} samples (want {} ± {tol})",
            3.0 * fs
        );
        assert!((kept_gap - 3.0 * fs).abs() <= tol, "{rate} Hz: {kept_gap}");
    }
}

/// US2 AS5 (FR-004, FR-005): mute mode in the VAS-isolated configuration keeps the input's
/// length, reports one muted region whose length equals drop mode's removed length (exact at 8
/// and 48 kHz, ±1 at 44.1 kHz), is exactly 0.0 inside the region (at 8 kHz; at 48 kHz after the
/// first `latency + 1` samples of the region), and equals drop mode's output outside it (8 kHz).
#[test]
fn as5_mute_mode_keeps_the_length_and_reports_the_region() {
    for rate in [8_000u32, 48_000, 44_100] {
        let x = burst_gap(&[(true, 1.0), (false, 5.0), (true, 1.0)], rate);
        let drop = configs::settings("vas_only", rate);
        let mute = configs::settings("vas_mute", rate);
        assert_eq!(mute.vas.mode, VasMode::Mute);
        let (y_drop, ev_drop, _) = run(drop, &x);
        let (y_mute, ev_mute, info) = run(mute, &x);
        assert_eq!(y_mute.len(), x.len(), "{rate} Hz: mute keeps the length");
        assert_eq!(info.produced, x.len());
        assert!(!info.paused);
        assert_eq!(ev_drop.len(), 1, "{rate} Hz");
        assert_eq!(ev_mute.len(), 1, "{rate} Hz");
        let (start, len) = (
            ev_mute[0].output_position as usize,
            ev_mute[0].input_length as usize,
        );
        let removed = ev_drop[0].input_length as i64;
        let tol = if rate == 44_100 { 1 } else { 0 }; // ±1 host sample, engineering target (003 FR-017)
        assert!(
            (len as i64 - removed).abs() <= tol,
            "{rate} Hz: region {len} vs removed {removed}"
        );
        assert!(start + len <= y_mute.len());

        let latency = Pipeline::new(mute).unwrap().latency_samples() as usize;
        let exact_from = if rate == 8_000 { 0 } else { latency + 1 }; // 003 FR-004 allowance
        for (i, v) in y_mute[start + exact_from..start + len].iter().enumerate() {
            assert!(
                v.to_bits() == 0,
                "{rate} Hz: sample {} of the region is {v}",
                exact_from + i
            );
        }
        if rate == 8_000 {
            assert_eq!(start, ev_drop[0].output_position as usize);
            assert_eq!(&y_mute[..start], &y_drop[..start], "before the region");
            assert_eq!(&y_mute[start + len..], &y_drop[start..], "after the region");
        }
    }
}

/// US2 AS6 (FR-012): every out-of-range VAS value is rejected by `new` and by `reconfigure`
/// with an error that names the setting, and a failed `reconfigure` leaves the pipeline as it
/// was.
#[test]
fn as6_out_of_range_settings_are_named_and_change_nothing() {
    type Break = fn(&mut Settings);
    let cases: [(Setting, Break); 9] = [
        (Setting::VasSensitivity, |s| s.vas.sensitivity = 0),
        (Setting::VasSensitivity, |s| s.vas.sensitivity = 6),
        (Setting::VasThresholdDbfs, |s| s.vas.threshold_dbfs = 0.5),
        (Setting::VasThresholdDbfs, |s| s.vas.threshold_dbfs = -60.5),
        (Setting::VasThresholdDbfs, |s| {
            s.vas.threshold_dbfs = f32::NAN
        }),
        (Setting::VasHangMs, |s| s.vas.hang_ms = 49.9),
        (Setting::VasHangMs, |s| s.vas.hang_ms = 10_000.1),
        (Setting::VasOnsetMs, |s| s.vas.onset_ms = -0.1),
        (Setting::VasOnsetMs, |s| s.vas.onset_ms = f32::INFINITY),
    ];
    let x = burst_gap(&[(true, 0.3), (false, 1.5), (true, 0.3)], 48_000);
    for (setting, break_it) in cases {
        let mut bad = configs::settings("vas_only", 48_000);
        break_it(&mut bad);
        assert_eq!(
            Pipeline::new(bad).err(),
            Some(Error::InvalidSetting { setting }),
            "{setting:?}"
        );

        let good = configs::settings("vas_only", 48_000);
        let mut p = Pipeline::new(good).unwrap();
        let mut scratch = vec![0.0f32; 4000];
        let _ = p.process(&x[..4000], &mut scratch).unwrap();
        let reference = p.clone();
        assert_eq!(
            p.reconfigure(bad).err(),
            Some(Error::InvalidSetting { setting }),
            "{setting:?}"
        );
        assert_eq!(p.settings(), reference.settings());
        assert_eq!(p.latency_samples(), reference.latency_samples());
        let (mut a, mut b) = (p, reference);
        let (mut ya, mut yb) = (vec![0.0f32; x.len()], vec![0.0f32; x.len()]);
        let (mut ea, mut eb) = (
            vec![VasEvent::default(); a.max_events(x.len())],
            vec![VasEvent::default(); b.max_events(x.len())],
        );
        let ia = a.process_with_events(&x, &mut ya, &mut ea).unwrap();
        let ib = b.process_with_events(&x, &mut yb, &mut eb).unwrap();
        assert_eq!(ia, ib, "{setting:?}");
        assert_eq!(ya[..ia.produced], yb[..ib.produced], "{setting:?}");
        assert_eq!(ea[..ia.events], eb[..ib.events], "{setting:?}");
    }
}
