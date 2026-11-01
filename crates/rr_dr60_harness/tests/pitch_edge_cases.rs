//! Spec 023 edge cases (tasks.md T014): one test per spec edge case, nine in all.

use rr_dr60::speed::SpeedMode;
use rr_dr60::{Error, Pipeline, Settings, Stream};
use rr_dr60_harness::changes_checks::{self as cc, Change, Partition, Source};
use rr_dr60_harness::checks::standard;
use rr_dr60_harness::pitch_checks::{self as pc, Req};

const MODE: u32 = 30;

/// A mode change at the playback's first output switches at once (no blend), and at level 3 at
/// once with the entry logged.
#[test]
fn first_output_and_level_3_switch_at_once() {
    let x = pc::vowel(0.5);
    // Position 0 at level 1: the output equals the constant pitch-kept run bit for bit.
    let mut p = pc::engine(&standard, "speed_only", 6000, &x);
    p.set_playback_speed(1).unwrap();
    let with = cc::run_scheduled(
        &mut p,
        Source::Playback(1),
        &[Change::mode(0, SpeedMode::PitchKept, 1)],
        Partition::Fixed(64),
    );
    let mut q = pc::engine(&standard, "speed_only", 6000, &x);
    q.set_playback_speed(1).unwrap();
    q.set_playback_speed_mode(SpeedMode::PitchKept);
    let constant = cc::run(&mut q, Source::Playback(1), Partition::Fixed(64));
    assert!(cc::same(&with.out, &constant.out));
    assert_eq!(cc::entries_of(&with.log, MODE).len(), 1);
    // Level 3: at once, no blend, the entry logged.
    let mut p = pc::engine(&standard, "speed_only", 6000, &x);
    let with = cc::run_scheduled(
        &mut p,
        Source::Playback(1),
        &[Change::mode(700, SpeedMode::PitchKept, 2)],
        Partition::Fixed(64),
    );
    let mut q = pc::engine(&standard, "speed_only", 6000, &x);
    let plain = cc::run(&mut q, Source::Playback(1), Partition::Fixed(64));
    assert!(cc::same(&with.out, &plain.out));
    let e = cc::entries_of(&with.log, MODE);
    assert_eq!((e.len(), e[0].position, e[0].tag), (1, 700, 2));
}

/// A mode change and a level change at the same position: the mode blend runs and the level
/// applies to the place at d(P); the reference agrees bit for bit.
#[test]
fn same_position_mode_and_level() {
    let x = pc::vowel(1.0);
    for (level, mode, to_level, to_mode) in [
        (1u8, SpeedMode::PitchWithSpeed, 5u8, SpeedMode::PitchKept),
        (5, SpeedMode::PitchKept, 2, SpeedMode::PitchWithSpeed),
    ] {
        let sched = [
            (1300usize, Req::Level(to_level)),
            (1300, Req::Mode(to_mode)),
        ];
        let (mism, eq, _, _) =
            pc::against_reference(&standard, "speed_only", &x, level, mode, &sched);
        assert_eq!(mism, 0, "{level:?} {mode:?}");
        assert!(eq);
    }
}

/// A request during a blend waits (the latest wins): a mode request during a mode blend, a
/// level request during a mode blend, a mode request during a level crossfade in the default
/// mode; each bit-exact against the reference, which carries the one wait rule.
#[test]
fn requests_during_a_blend_wait() {
    let x = pc::vowel(1.0);
    let s = 1300usize;
    let cases: [pc::Case; 3] = [
        (
            "mode during mode",
            1,
            SpeedMode::PitchWithSpeed,
            vec![
                (s, Req::Mode(SpeedMode::PitchKept)),
                (s + 30, Req::Mode(SpeedMode::PitchWithSpeed)),
                (s + 60, Req::Mode(SpeedMode::PitchKept)),
            ],
        ),
        (
            "level during mode",
            4,
            SpeedMode::PitchKept,
            vec![
                (s, Req::Mode(SpeedMode::PitchWithSpeed)),
                (s + 30, Req::Level(1)),
                (s + 90, Req::Level(2)),
            ],
        ),
        (
            "mode during level crossfade",
            2,
            SpeedMode::PitchWithSpeed,
            vec![
                (s, Req::Level(5)),
                (s + 30, Req::Mode(SpeedMode::PitchKept)),
            ],
        ),
    ];
    for (name, level, mode, sched) in &cases {
        let (mism, eq, _, _) =
            pc::against_reference(&standard, "speed_only", &x, *level, *mode, sched);
        assert_eq!(mism, 0, "{name}");
        assert!(eq, "{name}");
    }
}

/// The recording's end during a blend: the old mechanism's remainder past the end is cut by the
/// drain (the playback finishes, the final place is K); a request past the latch is dropped
/// and logged.
#[test]
fn end_during_a_blend_and_past_the_latch() {
    let x = pc::vowel(0.5);
    let k = x.len() as u64;
    for &rate in [6000u32, 48_000].iter() {
        let own = (5 * k).div_ceil(3);
        let pos_dev = own - 40; // a blend starting 40 outputs before the own part's end
        let pos = pos_dev * u64::from(rate) / 6000;
        let mut p = pc::engine(&standard, "speed_only", rate, &x);
        p.set_playback_speed(1).unwrap();
        p.set_playback_speed_mode(SpeedMode::PitchKept);
        // The second request lands inside the blend (it waits) and before the end: both logged.
        let sched = [
            Change::mode(pos, SpeedMode::PitchWithSpeed, 1),
            Change::mode(pos + 30, SpeedMode::PitchKept, 2),
        ];
        let run = cc::run_scheduled(&mut p, Source::Playback(1), &sched, Partition::Fixed(256));
        assert_eq!(
            cc::entries_of(&run.log, MODE).len(),
            2,
            "{rate}: both logged"
        );
        let drain = p.drain_samples() as usize;
        assert!(
            run.out[run.out.len() - drain / 2..]
                .iter()
                .all(|v| v.abs() < 1e-3),
            "{rate}: the drain's tail"
        );
        assert_eq!(p.state().state, rr_dr60::State::Idle);
    }
}

/// Very short recordings: the own part is ⌈5K ÷ k⌉ and the tail T_c at K = 1, 59, 119 and 600
/// (own 2 / 99 / 199 / 1000 at level 1; tails 118 / 81 / 101 / 80), and the playback ends on
/// the helper at every rate.
#[test]
fn very_short_recordings() {
    for (k, own_l1, tail_l1) in [
        (1usize, 2u64, 118u64),
        (59, 99, 81),
        (119, 199, 101),
        (600, 1000, 80),
    ] {
        let x: Vec<f32> = pc::vowel(0.5)[..k].to_vec();
        for &rate in [6000u32, 48_000].iter() {
            for level in pc::LEVELS {
                let mut p = pc::engine(&standard, "speed_only", rate, &x);
                let kk = p.recording_length(1).unwrap();
                let want = p
                    .playback_length_samples_in_mode(1, level, SpeedMode::PitchKept)
                    .unwrap();
                let played = pc::play_modes(
                    &mut p,
                    64,
                    &[(0, Req::Level(level)), (0, Req::Mode(SpeedMode::PitchKept))],
                );
                assert_eq!(played.out.len() as u64, want, "K={k} L{level} {rate}");
                let own =
                    (5 * kk).div_ceil(u64::from(rr_dr60_harness::speed_checks::numerator(level)));
                if level == 1 && kk == k as u64 {
                    assert_eq!(own, own_l1, "K={k}: own");
                    assert_eq!(pc::tail_c(own), tail_l1, "K={k}: tail");
                }
                if rate == 6000 {
                    let drain = u64::from(p.drain_samples());
                    assert_eq!(
                        played.out.len() as u64 - drain,
                        own + pc::tail_c(own),
                        "K={k} L{level}: own + T_c"
                    );
                }
            }
        }
    }
    // The stated figures for K exactly (the stop's flush adds nothing at 6 kHz in bypass_all: the
    // recording length equals K).
    for (k, own_l1, tail_l1) in [
        (1u64, 2u64, 118u64),
        (59, 99, 81),
        (119, 199, 101),
        (600, 1000, 80),
    ] {
        assert_eq!((5 * k).div_ceil(3), own_l1);
        assert_eq!(pc::tail_c(own_l1), tail_l1);
    }
}

/// Level 3 in pitch-kept mode is the exact bypass: no frame grid runs (no shift recorded); leaving
/// level 3 anchors the grid at that output; returning waits for a whole place and blends.
#[test]
fn level_3_is_the_exact_bypass() {
    let x = pc::vowel(0.5);
    let mut p = pc::engine(&standard, "speed_only", 6000, &x);
    let played = pc::play_modes(
        &mut p,
        64,
        &[(0, Req::Level(3)), (0, Req::Mode(SpeedMode::PitchKept))],
    );
    assert!(played.shifts.is_empty(), "no grid at level 3");
    let mut q = pc::engine(&standard, "speed_only", 6000, &x);
    let plain = pc::play_modes(&mut q, 64, &[(0, Req::Level(3))]);
    assert!(cc::same(&played.out, &plain.out));
    // Out of 3 at output 640 (call 10 of 64): the anchor's shift 0 is the first recorded.
    let mut p = pc::engine(&standard, "speed_only", 6000, &x);
    let played = pc::play_modes(
        &mut p,
        64,
        &[
            (0, Req::Level(3)),
            (0, Req::Mode(SpeedMode::PitchKept)),
            (10, Req::Level(1)),
            (30, Req::Level(3)),
        ],
    );
    assert_eq!(played.shifts[0], 0);
    assert!(played.shifts.len() >= 20);
    let (mism, eq, _, _) = pc::against_reference(
        &standard,
        "speed_only",
        &x,
        3,
        SpeedMode::PitchKept,
        &[(640, Req::Level(1)), (1920, Req::Level(3))],
    );
    assert_eq!(mism, 0);
    assert!(eq);
}

/// The one-pass and record streams have no playback speed mode: the control is refused there.
#[test]
fn one_pass_and_record_streams_refuse_the_mode() {
    let mut p = Pipeline::new(Settings::new(48_000)).unwrap();
    for stream in [Stream::OnePass, Stream::Record] {
        assert!(matches!(
            p.schedule_change(stream, 10, MODE, 1.0, 1),
            Err(Error::ControlNotOnStream { control: 30, .. })
        ));
    }
    // The one-pass output is untouched by the mode.
    let x = pc::vowel(0.5);
    let mut y = vec![0.0f32; x.len()];
    let mut p = Pipeline::new(Settings::new(6000)).unwrap();
    let _ = p.process(&x, &mut y).unwrap();
    let mut q = Pipeline::new(Settings::new(6000)).unwrap();
    q.set_playback_speed_mode(SpeedMode::PitchKept);
    let mut z = vec![0.0f32; x.len()];
    let _ = q.process(&x, &mut z).unwrap();
    assert!(cc::same(&y, &z));
}

/// The timeline map (c) in a mode: `place_at_in_mode` with the mode entries equals the engine's
/// own place trace on a schedule of level and mode changes; `place_at` keeps the default-mode
/// reading (it ignores the mode entries).
#[test]
fn timeline_map_c_in_mode() {
    use rr_dr60::timeline::{place_at, place_at_in_mode};
    let x = pc::vowel(0.5);
    for &rate in [6000u32, 48_000].iter() {
        let n = u64::from(rate) / 50;
        let p0 = u64::from(rate) / 8;
        let sched = [
            Change::mode(p0, SpeedMode::PitchKept, 1),
            Change::speed(p0 + n, 5, 2),
            Change::speed(p0 + 3 * n, 3, 3),
            Change::mode(p0 + 5 * n, SpeedMode::PitchWithSpeed, 4),
            Change::speed(p0 + 5 * n, 1, 5),
        ];
        let mut p = pc::engine(&standard, "speed_only", rate, &x);
        p.set_playback_speed(1).unwrap();
        cc::schedule_all(&mut p, &sched);
        p.start_playback(1).unwrap();
        let k = p.recording_length(1).unwrap();
        let mut y = vec![0.0f32; 2048];
        let mut log = Vec::new();
        let mut entries =
            vec![rr_dr60::ChangeEntry::default(); rr_dr60::changes::MAX_CHANGES_PER_CALL];
        let info = p.play_with_changes(&mut y, &mut entries).unwrap();
        log.extend_from_slice(&entries[..info.changes]);
        let trace = rr_dr60::__test_hooks::speed_place_trace(&p);
        assert!(!info.finished && trace.len() >= 2048 * 6000 / rate as usize - 1);
        for (i, st) in trace.iter().enumerate() {
            let want = place_at_in_mode(i as u64, rate, 1, 0, k, &log).unwrap();
            assert_eq!(*st, want, "{rate}: output {i}");
        }
        // `place_at` keeps the default-mode reading: it follows the level entries only, so its
        // level agrees and its blend state differs where the mode's rules differ (the level-5
        // change at p0 + n lands during a mode blend: no blend of its own in the mode reading).
        let last = trace.len() as u64 - 1;
        let plain = place_at(last, rate, 1, k, &log).unwrap();
        let in_mode = place_at_in_mode(last, rate, 1, 0, k, &log).unwrap();
        assert_eq!(plain.level, in_mode.level);
        // Over the schedule's span with the whole playback's log (the first call's log holds no
        // entry at 48 kHz, where p0 lies past its 2048 outputs).
        let mut q = pc::engine(&standard, "speed_only", rate, &x);
        q.set_playback_speed(1).unwrap();
        let full = cc::run_scheduled(&mut q, Source::Playback(1), &sched, Partition::Fixed(2048));
        assert_eq!(full.log.len(), 5, "{rate}: every change logged");
        let span = rr_dr60::changes::speed_landing(p0 + 6 * n, rate).unwrap();
        let differing = (0..=span.max(last))
            .filter(|&i| {
                place_at(i, rate, 1, k, &full.log).unwrap()
                    != place_at_in_mode(i, rate, 1, 0, k, &full.log).unwrap()
            })
            .count();
        assert!(
            differing > 0,
            "{rate}: the readings differ (the mode blend)"
        );
        assert!(matches!(
            place_at_in_mode(0, rate, 1, 2, k, &log),
            Err(Error::InvalidSpeedMode { mode: 2 })
        ));
    }
}

/// Exact silence: every correlation is zero and every shift is 0 by the tie rule; the output is
/// zero.
#[test]
fn exact_silence() {
    let x = vec![0.0f32; 3000];
    for level in pc::LEVELS {
        let mut p = pc::engine(&standard, "speed_only", 6000, &x);
        let played = pc::play_modes(
            &mut p,
            64,
            &[(0, Req::Level(level)), (0, Req::Mode(SpeedMode::PitchKept))],
        );
        assert!(played.shifts.iter().all(|s| *s == 0), "L{level}");
        assert!(
            played.out.iter().all(|v| v.to_bits() == 0.0f32.to_bits()),
            "L{level}"
        );
        assert!(played.shifts.len() > 10);
    }
}
