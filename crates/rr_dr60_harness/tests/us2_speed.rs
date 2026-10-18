//! Spec 013 US2: changing the speed while a recording plays (tasks.md T017, T019; research
//! R-08 (c)).
//!
//! Every schedule's whole output, at 6 kHz in `speed_only` and `speed_only_codec`, must equal the
//! independent reference bit for bit, with its places (FR-005, FR-007, FR-009). Call sizes that
//! are not multiples of 5 (121, 123) put changes at every phase of the place; one-sample calls
//! put a change inside the deferred switch into level 3 and near the recording's end.

use rr_dr60::{Pipeline, Route, speed};
use rr_dr60_harness::checks::standard;
use rr_dr60_harness::configs::{self, stored_recording};
use rr_dr60_harness::speed_checks::{self as sc, LEVELS, play, reference_output, stored_stream};
use rr_dr60_harness::stimulus;

/// Plays `schedule` in `config` at 6 kHz on `x` and compares with the reference.
fn check(config: &str, x: &[f32], call: usize, schedule: &[(usize, u8)], what: &str) {
    let (mut p, n) = stored_recording(&standard, configs::settings(config, 6000), x);
    let stream = stored_stream(&mut p, n);
    let played = play(&mut p, n, call, schedule);
    let (want, places) = reference_output(&stream, call, schedule);
    assert_eq!(
        sc::mismatches(&played.out, &want),
        0,
        "{config} {what}: call {call}, schedule {schedule:?}: {} vs {} samples",
        played.out.len(),
        want.len()
    );
    assert_eq!(played.places, places, "{config} {what}: places");
    assert_eq!(played.finished, 1, "{config} {what}");
    assert!(
        played.places.windows(2).all(|w| w[1] >= w[0]),
        "{config} {what}: the place decreased"
    );
    if config == "speed_only_codec" {
        // FR-009 (post-implementation review B3): with the codec on, the frame index follows the
        // place at every level, never decreases, and every frame is decoded exactly once.
        let k = p.recording_length(n).unwrap();
        let frames = k / 120;
        for (place, fi) in played.places.iter().zip(&played.frame_indices) {
            assert_eq!(
                *fi,
                (place / 120).min(frames - 1),
                "{config} {what}: frame index"
            );
        }
        assert!(played.frame_indices.windows(2).all(|w| w[1] >= w[0]));
        assert_eq!(played.frames, frames, "{config} {what}: frames decoded");
    }
}

const CONFIGS: [&str; 2] = ["speed_only", "speed_only_codec"];

/// US2 AS1 (FR-007): every ordered pair (a, b ≠ 3) at two change points and three call sizes.
#[test]
fn every_ordered_pair() {
    let x = stimulus::stored_voiced(1.0);
    for config in CONFIGS {
        for a in 1..=speed::LEVEL_COUNT {
            for b in LEVELS {
                if a == b {
                    continue;
                }
                for call in [120usize, 121, 123] {
                    for c in [3usize, 7] {
                        check(config, &x, call, &[(0, a), (c, b)], "pair");
                    }
                }
            }
        }
    }
}

/// US2 AS2 (FR-007): into level 3 from each level at every phase (calls of 121 and 123 put the
/// change at places p5 ≢ 0 mod 5); the old ratio holds for at most 4 outputs.
#[test]
fn into_level_3_at_every_phase() {
    let x = stimulus::stored_voiced(1.0);
    for config in CONFIGS {
        for a in LEVELS {
            for call in [121usize, 123, 124, 122] {
                for c in [1usize, 2, 3, 4] {
                    check(config, &x, call, &[(0, a), (c, 3)], "into 3");
                }
            }
        }
    }
}

/// FR-007 (the clarification): a change to another level during the wait for a whole place
/// applies at once and drops the pending switch; one-sample calls put it inside the wait.
#[test]
fn change_during_the_deferred_switch() {
    let x = stimulus::stored_voiced(0.5);
    for config in CONFIGS {
        // Level 1 (k = 3): output 21 sits at p5 = 63 (phase 3), so the switch into 3 waits.
        check(config, &x, 1, &[(0, 1), (21, 3), (22, 5)], "wait then 5");
        check(config, &x, 1, &[(0, 1), (21, 3), (23, 2)], "wait then 2");
        // Level 5 (k = 7): output 11 at p5 = 77 (phase 2).
        check(config, &x, 1, &[(0, 5), (11, 3), (12, 4)], "wait then 4");
    }
}

/// FR-005, FR-007 (Q3): changes in the last outputs before the place reaches K, and after it
/// (latched: no effect), with one-sample calls.
#[test]
fn changes_near_and_after_the_end() {
    let x = stimulus::stored_end_loud(301);
    for config in ["speed_only"] {
        for a in LEVELS {
            let k = u64::from(sc::numerator(a));
            let own = (5 * 301u64).div_ceil(k) as usize;
            for back in 1..=5usize {
                let at = own - back;
                check(config, &x, 1, &[(0, a), (at, 3)], "into 3 near the end");
                let b = if a == 4 { 5 } else { 4 };
                check(config, &x, 1, &[(0, a), (at, b)], "near the end");
            }
            // After the place reached K: the latch.
            for after in 0..3usize {
                check(config, &x, 1, &[(0, a), (own + after, 3)], "after the end");
                check(config, &x, 1, &[(0, a), (own + after, 1)], "after the end");
            }
        }
    }
}

/// US2 AS3 (FR-010's rapid changes): a new level before every call through 3 → 1 → 5 → 2 →
/// 4 → 3 …, with calls of 120 and 121.
#[test]
fn rapid_changes() {
    let x = stimulus::stored_voiced(1.0);
    let cycle = [3u8, 1, 5, 2, 4];
    for config in CONFIGS {
        for call in [120usize, 121] {
            let schedule: Vec<(usize, u8)> = (0..200).map(|c| (c, cycle[c % 5])).collect();
            check(config, &x, call, &schedule, "rapid");
        }
    }
}

/// US2 AS6 (FR-001, FR-007): stop and restart start from rest; reset and reconfigure keep the
/// level; erasing the recording being played stops the playback and keeps the level.
#[test]
fn whole_engine_controls() {
    let cfg = configs::settings("speed_only", 6000);
    let x = stimulus::stored_voiced(1.0);
    let (mut p, n) = stored_recording(&standard, cfg, &x);
    p.set_playback_speed(2).unwrap();
    let fresh = play(&mut p, n, 120, &[]).out;
    // A playback interrupted after a level change, then restarted, equals a fresh one.
    p.start_playback(n).unwrap();
    let mut buf = vec![0.0f32; 120];
    let _ = p.play(&mut buf).unwrap();
    p.set_playback_speed(5).unwrap();
    let _ = p.play(&mut buf).unwrap();
    p.stop_playback().unwrap();
    p.set_playback_speed(2).unwrap();
    let again = play(&mut p, n, 120, &[]).out;
    assert_eq!(sc::mismatches(&fresh, &again), 0, "restart from rest");
    // Reset and reconfigure keep the level.
    p.set_playback_speed(4).unwrap();
    p.reset();
    assert_eq!(p.playback_speed(), 4);
    p.reconfigure(cfg).unwrap();
    assert_eq!(p.playback_speed(), 4);
    // Erasing the recording being played stops the playback; the level is kept.
    p.start_playback(n).unwrap();
    let _ = p.play(&mut buf).unwrap();
    p.erase(n).unwrap();
    assert!(p.play(&mut buf).is_err(), "the playback stopped");
    assert_eq!(p.playback_speed(), 4);
}

/// US2 AS7 (FR-008): at every level the one-pass output, the record path's stored bytes and the
/// export are those of level 3, and the playback latency and the drain are the same.
#[test]
fn scope_and_latency() {
    for &rate in configs::measurement_rates() {
        let x = stimulus::voiced(f64::from(rate), 0.5, -20.0, 7);
        let reference = {
            let mut p = Pipeline::new(configs::settings("speed_default", rate)).unwrap();
            run_engine(&mut p, &x)
        };
        for level in 1..=speed::LEVEL_COUNT {
            let mut p = Pipeline::new(configs::settings("speed_default", rate)).unwrap();
            p.set_playback_speed(level).unwrap();
            let got = run_engine(&mut p, &x);
            assert_eq!(got.0, reference.0, "{rate} level {level}: one-pass");
            assert_eq!(got.1, reference.1, "{rate} level {level}: export");
            assert_eq!(got.2, reference.2, "{rate} level {level}: latency, drain");
        }
    }
}

/// The one-pass output bits, the export of a recording of `x`, and the playback latency and
/// drain, from one engine.
fn run_engine(p: &mut Pipeline, x: &[f32]) -> (Vec<u32>, Vec<u8>, (u32, u32)) {
    let mut y = x.to_vec();
    let mut one_pass = Vec::new();
    for chunk in y.chunks_mut(480) {
        let info = p.process_in_place(chunk);
        one_pass.extend(chunk[..info.produced].iter().map(|v| v.to_bits()));
    }
    p.reset();
    p.start_recording().unwrap();
    let _ = p.record(x, &mut []).unwrap();
    let _ = p.stop_recording(&mut []).unwrap();
    let mut bytes = vec![0u8; p.export_len(1).unwrap()];
    let _ = p.export(1, &mut bytes).unwrap();
    (
        one_pass,
        bytes,
        (p.playback_latency_samples(), p.drain_samples()),
    )
}

/// US2 AS6 (FR-007): the route, the volume and the output clamp act independently of the level:
/// a playback at level 4 on the earphone route with a volume change mid-playback and the clamp
/// on equals the same playback with the level set before the start (no level change involved),
/// and a level change mid-playback leaves the route and the volume as set.
#[test]
fn independent_of_route_volume_and_clamp() {
    let mut cfg = configs::settings("speed_default", 48_000);
    cfg.output.clamp_enabled = true;
    let x = stimulus::stored_voiced(1.0);
    let run = |late_level: bool| {
        let (mut p, n) = stored_recording(&standard, cfg, &x);
        p.set_route(Route::Earphone);
        if !late_level {
            p.set_playback_speed(4).unwrap();
        }
        p.start_playback(n).unwrap();
        if late_level {
            p.set_playback_speed(4).unwrap(); // before the first play call: from its first sample
        }
        let mut buf = vec![0.0f32; 960];
        let mut out = Vec::new();
        let mut c = 0;
        loop {
            if c == 5 {
                p.set_volume(0.5).unwrap();
            }
            let info = p.play(&mut buf).unwrap();
            out.extend_from_slice(&buf[..info.produced]);
            c += 1;
            if info.finished {
                break;
            }
        }
        assert_eq!(p.route(), Route::Earphone);
        assert_eq!(p.volume(), 0.5);
        out
    };
    let a = run(false);
    let b = run(true);
    assert_eq!(sc::mismatches(&a, &b), 0);
    assert!(
        a.iter().all(|v| v.abs() <= 1.0),
        "the clamp bounds the route output"
    );
}

/// FR-007 (the deferred switch, read in fifths; tasks.md T040): at 6 kHz in `speed_only`, after
/// 121 outputs at level 1 the next place is 363 fifths (not a whole stored sample); a change to
/// level 3 keeps the old step of 3 fifths until the place is whole (366, 369, 372, 375), then
/// steps by 5 (380, 385).
#[test]
fn deferred_switch_in_fifths() {
    use rr_dr60::__test_hooks::speed_place_fifths;
    let x = stimulus::stored_voiced(0.5);
    let (mut p, n) = stored_recording(&standard, configs::settings("speed_only", 6000), &x);
    p.set_playback_speed(1).unwrap();
    p.start_playback(n).unwrap();
    assert_eq!(speed_place_fifths(&p), 0);
    let mut buf = vec![0.0f32; 121];
    let _ = p.play(&mut buf).unwrap();
    assert_eq!(speed_place_fifths(&p), 363);
    p.set_playback_speed(3).unwrap();
    let mut one = [0.0f32; 1];
    let places: Vec<u64> = (0..6)
        .map(|_| {
            let _ = p.play(&mut one).unwrap();
            speed_place_fifths(&p)
        })
        .collect();
    assert_eq!(places, [366, 369, 372, 375, 380, 385]);
}
