//! Spec 012 Edge Cases, one test each (tasks.md T039), in the stages-only configurations unless
//! the case is about the whole engine. Report lines print with `--nocapture`.

use rr_dr60::{Pipeline, Settings, StopResult, Tap};
use rr_dr60_harness::mic_checks as mic;
use rr_dr60_harness::{analysis, configs, stimulus};

fn run(s: Settings, x: &[f32]) -> Vec<f32> {
    let mut p = Pipeline::new(s).unwrap();
    let mut y = vec![0.0f32; x.len()];
    let n = p.process(x, &mut y).unwrap().produced;
    y.truncate(n);
    y
}

/// Silence carries the hiss (at its level); with stage 1 off it is exact zeros.
#[test]
fn silence_gives_the_hiss_and_zeros_with_stage_1_off() {
    for &rate in configs::measurement_rates() {
        let x = stimulus::silence(rate as usize);
        let hiss = run(mic::mic_only(rate, 0.0), &x);
        let fs = f64::from(rate);
        let level = analysis::band_level_aes17(&hiss, fs, 300.0, 3000.0);
        assert!(
            (level - mic::HISS_BAND_DBFS).abs() < 1.0,
            "{rate} Hz: {level} dBFS" // 012 FR-003 (a 1 s read; the 10 s figure is SC-001's)
        );
        let quiet = run(configs::settings("mic_only_quiet", rate), &x);
        assert!(quiet.iter().all(|v| v.to_bits() == 0), "{rate} Hz");
    }
}

/// A DC step is blocked: after stage 2 it decays to nothing (the input high-pass and the output
/// coupling both have their zero at DC; A-046, A-047).
#[test]
fn dc_is_blocked() {
    for &rate in configs::measurement_rates() {
        let y = run(
            configs::settings("mic_only_quiet", rate),
            &stimulus::dc(0.5, 2 * rate as usize),
        );
        let tail = &y[y.len() - rate as usize / 10..];
        let peak = tail.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        println!("{rate} Hz: a 0.5 DC step after 1.9 s reads {peak:e}");
        assert!(peak < 1e-9, "{rate} Hz: {peak}"); // 012 FR-005: DC does not pass
    }
}

/// +40 dBFS is bounded by L+ + L- and stays finite, with the hiss on.
#[test]
fn plus_40_dbfs_is_bounded() {
    for &rate in configs::measurement_rates() {
        let x = stimulus::tone_at_phase(1000.0, 40.0, 0.0, f64::from(rate), rate as usize / 5);
        let y = run(mic::mic_only(rate, 0.0), &x);
        let peak = y.iter().fold(0.0f64, |m, v| m.max(f64::from(*v).abs()));
        assert!(y.iter().all(|v| v.is_finite()));
        // The hiss adds at most a few hundredths through the sections: well inside the bound.
        assert!(peak <= mic::stage2_bound(rate), "{rate} Hz: {peak}"); // 012 FR-006
    }
}

/// Non-finite and subnormal input samples become 0.0 (spec 001's sanitizer) and carry the hiss
/// alone: the output equals the run on zeros, bit for bit.
#[test]
fn nan_and_subnormal_input_carry_the_hiss_alone() {
    for &rate in configs::measurement_rates() {
        let n = rate as usize / 10;
        let bad: Vec<f32> = (0..n)
            .map(|i| match i % 4 {
                0 => f32::NAN,
                1 => f32::INFINITY,
                2 => f32::NEG_INFINITY,
                _ => f32::from_bits(1), // the smallest subnormal
            })
            .collect();
        let zeros = stimulus::silence(n);
        for s in [
            mic::mic_only(rate, 0.0),
            configs::settings("mic_capsule_only", rate),
        ] {
            assert_eq!(run(s, &bad), run(s, &zeros), "{rate} Hz");
        }
    }
}

/// Loud input then silence with the hiss off reaches exact zeros (the sections' states flush
/// below 1e-30), and no output sample is an `f32` subnormal.
#[test]
fn loud_then_silence_reaches_exact_zeros() {
    for &rate in configs::measurement_rates() {
        let fs = f64::from(rate);
        let mut x = stimulus::tone_at_phase(1000.0, 20.0, 0.0, fs, rate as usize / 10);
        x.extend(stimulus::silence(20 * rate as usize));
        let y = run(configs::settings("mic_only_quiet", rate), &x);
        assert!(y.iter().all(|v| !v.is_subnormal()), "{rate} Hz");
        let last_nonzero = y.iter().rposition(|v| *v != 0.0).unwrap_or(0);
        println!(
            "{rate} Hz: the last non-zero sample at {:.2} s",
            last_nonzero as f64 / fs
        );
        assert!(last_nonzero < y.len() - 1, "{rate} Hz: never reached zero");
        assert_eq!(*y.last().unwrap(), 0.0);
    }
}

/// The sensitivity setting acts after the limiter (A-022, A-033): it leaves stages 1 – 2 and the
/// limiter unchanged.
#[test]
fn sensitivity_leaves_stages_1_2_unchanged() {
    for &rate in configs::measurement_rates() {
        let x = stimulus::voiced(f64::from(rate), 0.3, -10.0, 7);
        for tap in [Tap::AfterMicAmp, Tap::AfterAgc] {
            let at = |level: u8| {
                let mut s = Settings::new(rate);
                s.tap = tap;
                s.vas.sensitivity = level;
                run(s, &x)
            };
            let base = at(3);
            for level in [1, 2, 4, 5] {
                assert_eq!(at(level), base, "{rate} Hz {tap:?} level {level}");
            }
        }
    }
}

/// The offset +12 dB (the range's end) is accepted; the voice activation's behaviour at it is
/// reported (US3 AS6: level 5 stops pausing near +9 dB).
#[test]
fn offset_plus_12_is_accepted_and_reported() {
    let rate = 48_000;
    let x = stimulus::silence(20 * rate as usize);
    for level in 1..=5u8 {
        let s = mic::vas_default(rate, level, 12.0);
        let r = rr_dr60_harness::vas_checks::run(&rr_dr60_harness::checks::standard, s, &x);
        println!(
            "offset +12 dB, level {level}: kept {:.2} s of 20 s, {} resumes, paused at the end: {}",
            r.y.len() as f64 / f64::from(rate),
            r.events.len(),
            r.info.paused
        );
    }
}

/// A reset between recordings gives new hiss: the counter has advanced (FR-009).
#[test]
fn reset_between_recordings_gives_new_hiss() {
    for &rate in configs::measurement_rates() {
        let s = configs::memory_capacity(configs::settings("default_mic", rate), 10, 4);
        let mut p = Pipeline::new(s).unwrap();
        let x = stimulus::silence(rate as usize / 2);
        let take = |p: &mut Pipeline| {
            p.start_recording().unwrap();
            let _ = p.record(&x, &mut []).unwrap();
            let StopResult::Kept { number, .. } = p.stop_recording(&mut []).unwrap().result else {
                panic!("kept")
            };
            rr_dr60_harness::split_checks::play_to_end(p, number, 997)
        };
        let a = take(&mut p);
        p.reset();
        let b = take(&mut p);
        assert_eq!(a.len(), b.len(), "{rate} Hz");
        assert_ne!(a, b, "{rate} Hz: the second recording carries stream 1");
    }
}

/// With the limiter bypassed, a loud tone passes the clipper (limited near +17.1 dBFS) and reaches
/// the codec's full-scale clamp (A-030): at the 6 kHz identity rate the tap after the record
/// stage holds samples at exactly ±1.0 and none beyond. The hiss reaches the same tap: silence
/// there is not silent (coach review C18).
#[test]
fn limiter_bypassed_reaches_the_clamp() {
    let rate = 6000;
    let mut s = Settings::new(rate);
    s.agc.enabled = false;
    s.vas.enabled = false;
    s.tap = Tap::AfterRecord;
    let x = stimulus::tone_at_phase(1000.0, 20.0, 0.0, 6000.0, 6000);
    let y = run(s, &x);
    let peak = y.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let at_rail = y.iter().filter(|v| v.abs() == 1.0).count();
    println!("limiter bypassed, +20 dBFS at 6 kHz: peak {peak}, {at_rail} samples at the rail");
    assert_eq!(peak, 1.0); // A-030: the clamp
    assert!(at_rail > 0);
    let quiet = run(s, &stimulus::silence(6000));
    assert!(
        quiet.iter().any(|&v| v != 0.0),
        "the hiss reaches the record tap with the limiter bypassed (FR-003)"
    );
}
