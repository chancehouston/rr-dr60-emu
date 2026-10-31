//! Spec 019 edge cases (tasks.md T012; spec › Edge cases): a change to the active level, the
//! first-output rule, consecutive-position requests, two changes at one position, a change at
//! the last own sample and during the tail, a playback stopped during a fade, the one-pass, a
//! change into level 3 during a fade.

use rr_dr60::__test_hooks;
use rr_dr60::{ChangeEntry, Pipeline, Stream};
use rr_dr60_harness::changes_checks::{self as cc, Change, Partition, Source};
use rr_dr60_harness::checks::standard;
use rr_dr60_harness::crossfade_checks as xf;
use rr_dr60_harness::speed_checks as sc;
use rr_dr60_harness::{configs, stimulus};

const RATE: u32 = 6000;

fn tone() -> Vec<f32> {
    xf::stored_tone(2000.0, 4000)
}

fn speed_only() -> rr_dr60::Settings {
    configs::settings("speed_only", RATE)
}

/// A change to the active level: no fade, the entry logged.
#[test]
fn change_to_the_active_level_is_a_no_op() {
    let x = tone();
    let mut p = xf::recorded(&standard, speed_only(), &x);
    let r = cc::run_scheduled(
        &mut p,
        Source::Playback(1),
        &[Change::speed(1500, 3, 1)],
        Partition::Fixed(64),
    );
    let mut q = xf::recorded(&standard, speed_only(), &x);
    let plain = cc::run(&mut q, Source::Playback(1), Partition::Fixed(64));
    assert!(cc::same(&r.out, &plain.out));
    let e = cc::entries_of(&r.log, cc::speed_control());
    assert_eq!(e.len(), 1);
    assert_eq!((e[0].before, e[0].after), (3.0, 3.0));
}

/// The first output (a setter between the start and the first call, or a change scheduled at
/// position 0): a switch without a blend; a level set before the start is the start value.
#[test]
fn first_output_switches_without_a_blend() {
    let x = tone();
    let mut p = xf::recorded(&standard, speed_only(), &x);
    p.set_playback_speed(5).unwrap();
    let early = sc::play(&mut p, 1, 1, &[]).out;
    let mut q = xf::recorded(&standard, speed_only(), &x);
    q.start_playback(1).unwrap();
    q.set_playback_speed(5).unwrap();
    let mut late = Vec::new();
    let mut buf = [0.0f32; 1];
    loop {
        let info = q.play(&mut buf).unwrap();
        late.extend_from_slice(&buf[..info.produced]);
        if info.finished {
            break;
        }
    }
    assert_eq!(
        sc::mismatches(&early, &late),
        0,
        "no blend on the first output"
    );
    let mut s = xf::recorded(&standard, speed_only(), &x);
    let r = cc::run_scheduled(
        &mut s,
        Source::Playback(1),
        &[Change::speed(0, 5, 1)],
        Partition::Fixed(64),
    );
    assert_eq!(sc::mismatches(&early, &r.out), 0, "a change scheduled at 0");
    // Logged as spec 015 logs it (one entry, 3 -> 5); the start-value run has no speed entry.
    let e = cc::entries_of(&r.log, cc::speed_control());
    assert_eq!(e.len(), 1);
    assert_eq!((e[0].position, e[0].before, e[0].after), (0, 3.0, 5.0));
    let mut t = xf::recorded(&standard, speed_only(), &x);
    t.set_playback_speed(5).unwrap();
    let start_value = cc::run(&mut t, Source::Playback(1), Partition::Fixed(64));
    assert!(cc::entries_of(&start_value.log, cc::speed_control()).is_empty());
}

/// Consecutive-position requests: the first starts a blend, the last of the rest waits.
#[test]
fn consecutive_requests_first_starts_last_waits() {
    let x = tone();
    let mut p = xf::recorded(&standard, speed_only(), &x);
    let got = xf::play_6k(&mut p, &[(0, 1), (1500, 5), (1501, 2), (1502, 4)]);
    let xs = xf::stored_stream(&x);
    let (want, _) = sc::reference_output_at(&xs, 1, 1, &[(1500, 5), (1501, 2), (1502, 4)]);
    assert_eq!(sc::mismatches(&got, &want), 0);
    let mut q = xf::recorded(&standard, speed_only(), &x);
    let waited = xf::play_6k(&mut q, &[(0, 1), (1500, 5), (1620, 4)]);
    assert_eq!(
        sc::mismatches(&got, &waited),
        0,
        "the last request lands at the blend's end"
    );
    assert_eq!(p.playback_speed(), 4);
}

/// Two changes scheduled at one position: the later wins, one blend.
#[test]
fn two_changes_at_one_position_later_wins() {
    let x = tone();
    let mut p = xf::recorded(&standard, speed_only(), &x);
    let r = cc::run_scheduled(
        &mut p,
        Source::Playback(1),
        &[Change::speed(1500, 5, 1), Change::speed(1500, 2, 2)],
        Partition::Fixed(64),
    );
    let mut q = xf::recorded(&standard, speed_only(), &x);
    let one = cc::run_scheduled(
        &mut q,
        Source::Playback(1),
        &[Change::speed(1500, 2, 2)],
        Partition::Fixed(64),
    );
    assert!(cc::same(&r.out, &one.out));
    assert_eq!(cc::entries_of(&r.log, cc::speed_control()).len(), 2);
}

/// A change at the last own output and one during the tail: the length rule unchanged (the
/// total equals the stepped build's), the outputs finite, nothing past the drain.
#[test]
fn change_at_the_end_and_during_the_tail() {
    let x = xf::stored_tone(2000.0, 600);
    let own = (5 * 600usize).div_ceil(3); // level 1's own outputs
    for at in [own - 1, own + 5] {
        let mut p = xf::recorded(&standard, speed_only(), &x);
        let fade = xf::play_6k(&mut p, &[(0, 1), (at, 5)]);
        let mut q = xf::recorded(&xf::stepped, speed_only(), &x);
        let step = xf::play_6k(&mut q, &[(0, 1), (at, 5)]);
        assert_eq!(fade.len(), step.len(), "at {at}");
        assert!(fade.iter().all(|v| v.is_finite()));
    }
}

/// A playback stopped during a fade: the next playback starts at the latest request, no blend.
#[test]
fn stop_during_a_fade_then_the_next_playback() {
    let x = tone();
    let mut p = xf::recorded(&standard, speed_only(), &x);
    p.start_playback(1).unwrap();
    let mut buf = vec![0.0f32; 1500];
    let _ = p.play(&mut buf).unwrap();
    p.set_playback_speed(5).unwrap();
    let mut buf = vec![0.0f32; 10];
    let _ = p.play(&mut buf).unwrap(); // inside the fade
    p.set_playback_speed(1).unwrap(); // waited
    p.stop_playback().unwrap();
    assert_eq!(p.playback_speed(), 1);
    let again = sc::play(&mut p, 1, 1, &[]).out;
    let mut q = xf::recorded(&standard, speed_only(), &x);
    let at_1 = sc::play(&mut q, 1, 1, &[(0, 1)]).out;
    assert_eq!(sc::mismatches(&again, &at_1), 0);
}

/// The one-pass never carries the speed: bit-identical to the stepped build in every one-pass
/// configuration (sampled), with a level set.
#[test]
fn one_pass_ignores_the_speed() {
    let x = stimulus::voiced(f64::from(RATE), 0.5, -20.0, 0x019);
    for config in ["default", "default_output", "speed_only"] {
        let s = configs::settings(config, RATE);
        let run = |mut p: Pipeline| {
            p.set_playback_speed(5).unwrap();
            let mut y = vec![0.0f32; x.len()];
            let n = p.process(&x, &mut y).unwrap().produced;
            y.truncate(n);
            y
        };
        let a = run(standard(s));
        let b = run(xf::stepped(s));
        assert_eq!(sc::mismatches(&a, &b), 0, "{config}");
    }
}

/// A change into level 3 requested during a fade: the fade completes, then the deferred wait,
/// then the blend into 3 (the reference models all three).
#[test]
fn into_level_3_during_a_fade() {
    let x = tone();
    let mut p = xf::recorded(&standard, speed_only(), &x);
    let got = xf::play_6k(&mut p, &[(0, 1), (1500, 5), (1560, 3)]);
    let xs = xf::stored_stream(&x);
    let (want, _) = sc::reference_output_at(&xs, 1, 1, &[(1500, 5), (1560, 3)]);
    assert_eq!(sc::mismatches(&got, &want), 0);
    assert_eq!(
        __test_hooks::speed_place_fifths(&p) % 5,
        0,
        "latched at a whole place"
    );
}

/// A scheduled change during a fade is logged at its position with chained target levels.
#[test]
fn log_carries_target_levels() {
    let x = tone();
    let mut p = xf::recorded(&standard, speed_only(), &x);
    let r = cc::run_scheduled(
        &mut p,
        Source::Playback(1),
        &[
            Change::speed(1500, 5, 1),
            Change::speed(1510, 2, 2),
            Change::speed(1520, 4, 3),
        ],
        Partition::Fixed(64),
    );
    let e: Vec<ChangeEntry> = cc::entries_of(&r.log, cc::speed_control());
    assert_eq!(e.len(), 3);
    assert_eq!((e[0].before, e[0].after), (3.0, 5.0));
    assert_eq!((e[1].before, e[1].after), (5.0, 2.0));
    assert_eq!((e[2].before, e[2].after), (2.0, 4.0));
    assert_eq!(
        e.iter().map(|x| x.position).collect::<Vec<_>>(),
        [1500, 1510, 1520]
    );
    let _ = Stream::Playback;
}

/// Convergence T023 (FR-005): a change scheduled inside a blend that runs into the latch never
/// sounds (the output equals the reference, which drops it) but is logged with chained target
/// levels, and the reading follows it.
#[test]
fn waited_request_into_the_latch_is_logged() {
    let x = xf::stored_tone(2000.0, 1200);
    let xs = xf::stored_stream(&x);
    let k = xs.len() as u64; // level 3 plays one output per stored sample: position = output
    let mut p = xf::recorded(&standard, speed_only(), &x);
    let r = cc::run_scheduled(
        &mut p,
        Source::Playback(1),
        &[Change::speed(k - 10, 5, 1), Change::speed(k - 5, 1, 2)],
        Partition::Fixed(64),
    );
    let (want, _) = sc::reference_output_at(&xs, 1, 3, &[(k - 10, 5), (k - 5, 1)]);
    assert_eq!(sc::mismatches(&r.out, &want), 0);
    let e = cc::entries_of(&r.log, cc::speed_control());
    assert_eq!(e.len(), 2, "both requests logged");
    assert_eq!((e[0].before, e[0].after), (3.0, 5.0));
    assert_eq!((e[1].before, e[1].after), (5.0, 1.0));
    assert_eq!(
        p.playback_speed(),
        1,
        "the reading follows the latest request"
    );
}
