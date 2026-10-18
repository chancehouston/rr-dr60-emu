//! Spec 013 edge cases at a constant level (tasks.md T016): very short recordings, the codec
//! bypassed, DC and silence, refused levels, a level set while idle or recording, and the length
//! helper's refusals.

use rr_dr60::{Error, Pipeline, speed};
use rr_dr60_harness::checks::standard;
use rr_dr60_harness::configs::{self, stored_recording};
use rr_dr60_harness::speed_checks::{self as sc, LEVELS, play, reference_output, stored_stream};
use rr_dr60_harness::stimulus;

/// FR-005 (Edge Cases: very short recordings): a recording of one stored sample, and one
/// shorter than the look-ahead, play every stored sample at every level, bit for bit as the
/// reference computes them.
#[test]
fn very_short_recordings() {
    for len in [1usize, 5, 27] {
        let x: Vec<f32> = (0..len).map(|n| 0.25 + 0.01 * n as f32).collect();
        for level in 1..=speed::LEVEL_COUNT {
            let (mut p, n) = stored_recording(&standard, configs::settings("speed_only", 6000), &x);
            let stream = stored_stream(&mut p, n);
            let played = play(&mut p, n, 120, &[(0, level)]);
            let (want, _) = reference_output(&stream, 120, &[(0, level)]);
            assert_eq!(
                sc::mismatches(&played.out, &want),
                0,
                "len {len} level {level}"
            );
            assert_eq!(played.finished, 1);
            assert_eq!(
                played.out.len() as u64,
                p.playback_length_samples(n, level).unwrap(),
                "len {len} level {level}"
            );
        }
    }
}

/// FR-003 (Edge Cases: DC and silence): silence plays as exact zeros at every level; a stored DC
/// level passes within 1e-6 (one f32 rounding of the unit-DC phases) once the kernel is inside
/// the recording.
#[test]
fn dc_and_silence() {
    for level in LEVELS {
        let (mut p, n) = stored_recording(
            &standard,
            configs::settings("speed_only", 6000),
            &stimulus::silence(2000),
        );
        let y = play(&mut p, n, 120, &[(0, level)]).out;
        assert!(y.iter().all(|&v| v == 0.0), "level {level}: silence");
        let (mut p, n) = stored_recording(
            &standard,
            configs::settings("speed_only", 6000),
            &stimulus::dc(0.25, 2000),
        );
        let y = play(&mut p, n, 120, &[(0, level)]).out;
        for &v in &y[100..1000] {
            assert!((v - 0.25).abs() < 1e-6, "level {level}: DC {v}"); // 013 FR-003
        }
    }
}

/// FR-001 (Edge Cases: refused levels): 0, 6 and 255 are refused with the level unchanged, by
/// the setter and by the length helper (which checks the level before the number).
#[test]
fn refused_levels() {
    let mut p = Pipeline::new(configs::settings("speed_only", 6000)).unwrap();
    p.set_playback_speed(2).unwrap();
    for bad in [0u8, 6, 255] {
        assert_eq!(
            p.set_playback_speed(bad),
            Err(Error::InvalidSpeedLevel { level: bad })
        );
        assert_eq!(p.playback_speed(), 2);
        assert_eq!(
            p.playback_length_samples(99, bad),
            Err(Error::InvalidSpeedLevel { level: bad })
        );
    }
    assert_eq!(
        p.playback_length_samples(99, 4),
        Err(Error::NoSuchRecording { number: 99 })
    );
}

/// FR-001 (US2 AS4): a level set while idle or while recording applies from the next
/// playback's first sample; it is kept by reset and reconfigure.
#[test]
fn level_set_while_idle_or_recording() {
    let cfg = configs::settings("speed_only", 6000);
    let x = stimulus::stored_tone(700.0, 0.5, 3000);
    let (mut p, n) = stored_recording(&standard, cfg, &x);
    let stream = stored_stream(&mut p, n);
    p.set_playback_speed(4).unwrap(); // idle
    let played = play(&mut p, n, 120, &[]);
    let (want, _) = reference_output(&stream, 120, &[(0, 4)]);
    assert_eq!(sc::mismatches(&played.out, &want), 0);
    // While recording a second item.
    p.start_recording().unwrap();
    p.set_playback_speed(1).unwrap();
    let _ = p.record(&x, &mut []).unwrap();
    let _ = p.stop_recording(&mut []).unwrap();
    assert_eq!(p.playback_speed(), 1);
    let played = play(&mut p, n, 120, &[]);
    let (want, _) = reference_output(&stream, 120, &[(0, 1)]);
    assert_eq!(sc::mismatches(&played.out, &want), 0);
    // Reset and reconfigure keep it.
    p.reset();
    assert_eq!(p.playback_speed(), 1);
    p.reconfigure(cfg).unwrap();
    assert_eq!(p.playback_speed(), 1);
}

/// FR-009 (Edge Cases: codec bypassed): the frame index stays 0 and the place progresses to K
/// at every level.
#[test]
fn codec_bypassed_reports() {
    let x = stimulus::stored_voiced(0.5);
    for level in 1..=speed::LEVEL_COUNT {
        let (mut p, n) = stored_recording(&standard, configs::settings("speed_only", 6000), &x);
        let k = p.recording_length(n).unwrap();
        let played = play(&mut p, n, 120, &[(0, level)]);
        assert!(
            played.frame_indices.iter().all(|&f| f == 0),
            "level {level}"
        );
        assert!(
            played.places.windows(2).all(|w| w[1] >= w[0]),
            "level {level}"
        );
        assert_eq!(*played.places.last().unwrap(), k, "level {level}");
    }
}
