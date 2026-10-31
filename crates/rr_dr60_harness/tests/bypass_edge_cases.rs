//! Spec 021 edge cases (tasks.md T016): one test per spec edge case, sixteen in all.

use rr_dr60::__test_hooks::player_bypass_targets;
use rr_dr60::{ChangeEntry, Error, Pipeline, Route, Settings, Stream, Tap, VasEvent};
use rr_dr60_harness::bypass_checks as bp;
use rr_dr60_harness::changes_checks::{self as cc, Change, Partition, Source};
use rr_dr60_harness::checks::standard;

const RATE: u32 = 48_000;

fn quiet() -> Settings {
    bp::settings(bp::DEFAULT_CHAIN, RATE)
}

fn recorded(s: Settings, route: Route) -> Pipeline {
    bp::engine(&standard, s, &bp::stored_tone(1000.0, 1.0), route, 0.5)
}

fn change(position: u64, control: u32, on: bool, tag: u64) -> Change {
    Change {
        stream: Stream::Playback,
        position,
        control,
        value: if on { 1.0 } else { 0.0 },
        tag,
    }
}

fn n12() -> u64 {
    bp::n_fade(12, RATE) as u64
}

/// A change at position 0 of the playback stream lands on the first output at once (no blend).
#[test]
fn position_zero_switches_at_once() {
    let mut p = recorded(quiet(), Route::Speaker);
    let live = cc::run_scheduled(&mut p, Source::Playback(1), &[change(0, 23, false, 1)], Partition::Fixed(256));
    let mut t = recorded(bp::twin_settings(&bp::cells()[8], RATE), Route::Speaker); // stage 12 off
    let twin = cc::run(&mut t, Source::Playback(1), Partition::Fixed(256));
    let n = twin.out.len();
    assert!(cc::same(&live.out[..n], &twin.out), "at once: the twin from sample 0");
    assert_eq!(cc::entries_of(&live.log, 23).len(), 1);
}

/// Two changes of one stage at one position: both applied, both logged, no blend.
#[test]
fn same_position_pair_both_logged_no_blend() {
    let mut p = recorded(quiet(), Route::Speaker);
    let r = cc::run_scheduled(
        &mut p,
        Source::Playback(1),
        &[change(300, 23, false, 1), change(300, 23, true, 2)],
        Partition::Fixed(256),
    );
    let e = cc::entries_of(&r.log, 23);
    assert_eq!(e.len(), 2);
    assert_eq!((e[0].before, e[0].after, e[1].before, e[1].after), (1.0, 0.0, 0.0, 1.0));
    let mut q = recorded(quiet(), Route::Speaker);
    let plain = cc::run(&mut q, Source::Playback(1), Partition::Fixed(256));
    assert!(cc::same(&r.out, &plain.out));
}

/// Changes of two stages at one position: both land, two independent blends.
#[test]
fn two_stages_same_position_two_blends() {
    let mut p = recorded(quiet(), Route::Speaker);
    let r = cc::run_scheduled(
        &mut p,
        Source::Playback(1),
        &[change(400, 22, false, 1), change(400, 23, false, 1)],
        Partition::Fixed(256),
    );
    assert_eq!(cc::entries_of(&r.log, 22).len(), 1);
    assert_eq!(cc::entries_of(&r.log, 23).len(), 1);
    assert_eq!(player_bypass_targets(&p).0, [true, true, false]);
    // Each alone differs from both together inside the blend.
    let mut a = recorded(quiet(), Route::Speaker);
    let only = cc::run_scheduled(&mut a, Source::Playback(1), &[change(400, 22, false, 1)], Partition::Fixed(256));
    assert!(!cc::same(&r.out, &only.out));
}

/// A toggle back exactly at the blend's last sample: a return from rest.
#[test]
fn toggle_at_last_blend_sample() {
    let n = n12();
    let mut p = recorded(quiet(), Route::Speaker);
    let r = cc::run_scheduled(
        &mut p,
        Source::Playback(1),
        &[change(400, 23, false, 1), change(400 + n, 23, true, 2)],
        Partition::Fixed(256),
    );
    // Sample 400 + n − 1 selected the input; the return's first sample computes k = N − 1; from
    // 400 + 2n − 1 the output is the all-on run's.
    let mut q = recorded(quiet(), Route::Speaker);
    let plain = cc::run(&mut q, Source::Playback(1), Partition::Fixed(256));
    let from = (400 + 2 * n - 1) as usize;
    assert!(cc::same(&r.out[from..], &plain.out[from..]));
    assert!(!cc::same(&r.out[400..from], &plain.out[400..from]));
}

/// Several toggles within one blend: each reverses; the output stays between the branches.
#[test]
fn several_toggles_reverse_each_time() {
    let n = n12();
    let mut p = recorded(quiet(), Route::Speaker);
    let sched = [
        change(400, 23, false, 1),
        change(400 + n / 4, 23, true, 2),
        change(400 + n / 4 + n / 8, 23, false, 3),
        change(400 + n / 2, 23, true, 4),
    ];
    let r = cc::run_scheduled(&mut p, Source::Playback(1), &sched, Partition::Fixed(256));
    assert_eq!(cc::entries_of(&r.log, 23).len(), 4);
    let mut a = recorded(quiet(), Route::Speaker);
    let all_on = cc::run(&mut a, Source::Playback(1), Partition::Fixed(256));
    let mut t = recorded(bp::twin_settings(&bp::cells()[8], RATE), Route::Speaker);
    let twin = cc::run(&mut t, Source::Playback(1), Partition::Fixed(256));
    for i in 400..(400 + n) as usize {
        let (y, x) = (f64::from(all_on.out[i]), f64::from(twin.out[i]));
        let v = f64::from(r.out[i]);
        let tol = y.abs().max(x.abs()) * f64::from(f32::EPSILON) * 2.0;
        assert!(v >= y.min(x) - tol && v <= y.max(x) + tol, "sample {i}");
    }
    // Back at rest on well before 400 + 2n: the last toggle at n/2 toward on from phase ≤ n/2.
    let from = (400 + n + n / 2) as usize;
    assert!(cc::same(&r.out[from..], &all_on.out[from..]));
}

/// A playback stopped during a blend: the next playback starts at the landed values with no
/// blend; the schedule is cleared with the playback.
#[test]
fn stopped_during_blend_restarts_at_rest() {
    let mut p = recorded(quiet(), Route::Speaker);
    p.schedule_change(Stream::Playback, 100, 23, 0.0, 1).unwrap();
    p.schedule_change(Stream::Playback, 100_000, 24, 0.0, 2).unwrap();
    p.start_playback(1).unwrap();
    let mut y = vec![0.0f32; 100 + n12() as usize / 2];
    let _ = p.play(&mut y).unwrap();
    p.stop_playback().unwrap();
    assert_eq!(p.pending_changes(Stream::Playback).unwrap(), 0, "cleared with the playback");
    assert_eq!(player_bypass_targets(&p).0, [false, true, false]);
    let r = cc::run(&mut p, Source::Playback(1), Partition::Fixed(256));
    let mut t = recorded(bp::twin_settings(&bp::cells()[8], RATE), Route::Speaker);
    let twin = cc::run(&mut t, Source::Playback(1), Partition::Fixed(256));
    assert!(cc::same(&r.out[..twin.out.len()], &twin.out), "no blend: the twin from sample 0");
    assert!(cc::entries_of(&r.log, 23).is_empty());
}

/// `reset` re-applies the landed values at once and clears the schedule and the log;
/// `reconfigure(s)` takes `s`.
#[test]
fn reset_and_reconfigure_rules() {
    let mut p = recorded(quiet(), Route::Speaker);
    cc::run_scheduled(&mut p, Source::Playback(1), &[change(100, 24, false, 1)], Partition::Fixed(256));
    p.schedule_change(Stream::Playback, 5, 23, 0.0, 9).unwrap();
    p.reset();
    assert_eq!(p.pending_changes(Stream::Playback).unwrap(), 0);
    assert_eq!(player_bypass_targets(&p).0, [false, false, true]);
    assert!(!p.settings().output.earphone_enabled);
    let mut s = *p.settings();
    s.output.earphone_enabled = true;
    p.reconfigure(s).unwrap();
    assert_eq!(player_bypass_targets(&p).0, [false, false, false]);
    assert!(p.settings().output.earphone_enabled);
}

/// A bypass landing during the drain: the blend runs on the drain's samples; nothing is added.
#[test]
fn landing_during_the_drain() {
    let mut p = recorded(quiet(), Route::Speaker);
    let own = (6000 * RATE as usize) / 6000; // the 1 s recording in host samples
    let pos = own as u64 + 100;
    let r = cc::run_scheduled(&mut p, Source::Playback(1), &[change(pos, 23, false, 1)], Partition::Fixed(256));
    let mut q = recorded(quiet(), Route::Speaker);
    let plain = cc::run(&mut q, Source::Playback(1), Partition::Fixed(256));
    assert_eq!(r.out.len(), plain.out.len());
    assert_eq!(cc::entries_of(&r.log, 23).len(), 1);
    assert_eq!(cc::entries_of(&r.log, 23)[0].position, pos);
}

/// A stage not on the active route: the change lands, logs, is inaudible, and its phase
/// advances so that a later route change finds the blend over.
#[test]
fn inactive_route_stage_lands_and_phase_advances() {
    let mut p = recorded(quiet(), Route::Earphone);
    let n = n12();
    let sched = [
        change(200, 23, false, 1), // stage 12 on the earphone route: inaudible
        Change::route(Stream::Playback, 200 + 3 * n, Route::Speaker, 2),
    ];
    let r = cc::run_scheduled(&mut p, Source::Playback(1), &sched, Partition::Fixed(256));
    assert_eq!(cc::entries_of(&r.log, 23).len(), 1);
    // Before the route change: equal to the plain earphone run.
    let mut q = recorded(quiet(), Route::Earphone);
    let plain = cc::run_scheduled(&mut q, Source::Playback(1), &[Change::route(Stream::Playback, 200 + 3 * n, Route::Speaker, 2)], Partition::Fixed(256));
    let at = (200 + 3 * n) as usize;
    assert!(cc::same(&r.out[..at], &plain.out[..at]), "inaudible on the earphone route");
    // After the route change the blend is over: the speaker route carries stage 12 bypassed
    // (the twin with stage 12 off, the same route change), bit for bit.
    let mut t = recorded(bp::twin_settings(&bp::cells()[8], RATE), Route::Earphone);
    let twin = cc::run_scheduled(&mut t, Source::Playback(1), &[Change::route(Stream::Playback, 200 + 3 * n, Route::Speaker, 2)], Partition::Fixed(256));
    let end = r.out.len().min(twin.out.len());
    assert!(cc::same(&r.out[at..end], &twin.out[at..end]), "the blend was over when the route came");
}

/// A route change during a blend keeps the weight and direction (the sections reset).
#[test]
fn route_change_mid_blend_keeps_the_weight() {
    let n = n12();
    let mut p = recorded(quiet(), Route::Speaker);
    // Stage 11 fades on both routes; a route switch halfway through.
    let sched = [
        change(300, 22, false, 1),
        Change::route(Stream::Playback, 300 + n / 2, Route::Earphone, 2),
    ];
    let r = cc::run_scheduled(&mut p, Source::Playback(1), &sched, Partition::Fixed(256));
    // After the blend: the twin with stage 11 off and the same route change, bit for bit on
    // the earphone route (stage 13 follows, so within the convergence bound).
    let mut t = recorded(bp::twin_settings(&bp::cells()[10], RATE), Route::Speaker);
    let twin = cc::run_scheduled(&mut t, Source::Playback(1), &[Change::route(Stream::Playback, 300 + n / 2, Route::Earphone, 2)], Partition::Fixed(256));
    let settle = rr_dr60::output::settle_samples(RATE).unwrap() as usize;
    let from = (300 + n) as usize + settle;
    let end = r.out.len().min(twin.out.len());
    let worst = (from..end).map(|i| (f64::from(r.out[i]) - f64::from(twin.out[i])).abs()).fold(0.0, f64::max);
    assert!(worst < 1e-5, "{worst}");
    // The phase continued: at the switch the fade was at N/2, and the run differs from a run
    // whose bypass lands at the switch (a frozen or restarted fade).
    let mut f = recorded(quiet(), Route::Speaker);
    let restarted = cc::run_scheduled(&mut f, Source::Playback(1), &[change(300 + n / 2, 22, false, 1), Change::route(Stream::Playback, 300 + n / 2, Route::Earphone, 2)], Partition::Fixed(256));
    let mid = (300 + n / 2) as usize;
    assert!(!cc::same(&r.out[mid..(300 + n) as usize], &restarted.out[mid..(300 + n) as usize]));
}

/// Stage 11 bypassed: the volume and the battery act on the warm branch with their own
/// transitions; the return hears the settled new value.
#[test]
fn stage11_bypassed_volume_and_battery_on_the_warm_branch() {
    let n = n12();
    let mut p = recorded(quiet(), Route::Speaker);
    let sched = [
        change(300, 22, false, 1),
        Change::volume(Stream::Playback, 300 + 2 * n, 0.8, 2),
        Change::battery(Stream::Playback, 300 + 2 * n, 2.5, 3),
        change(300 + 6 * n, 22, true, 4),
    ];
    let r = cc::run_scheduled(&mut p, Source::Playback(1), &sched, Partition::Fixed(256));
    // While bypassed the volume/battery changes are inaudible: equal to the twin (stage 11 off,
    // the chain's remaining stage 12 running) over the bypassed span.
    let mut t = recorded(bp::twin_settings(&bp::cells()[10], RATE), Route::Speaker);
    let twin = cc::run_scheduled(&mut t, Source::Playback(1), &[Change::volume(Stream::Playback, 300 + 2 * n, 0.8, 2), Change::battery(Stream::Playback, 300 + 2 * n, 2.5, 3)], Partition::Fixed(256));
    let settle = rr_dr60::output::settle_samples(RATE).unwrap() as usize;
    let (a, b) = ((300 + n) as usize + settle, (300 + 6 * n) as usize);
    let worst = (a..b).map(|i| (f64::from(r.out[i]) - f64::from(twin.out[i])).abs()).fold(0.0, f64::max);
    assert!(worst < 1e-5, "inaudible while bypassed: {worst}");
    // After the return: the all-on run with the same volume and battery schedule, whose
    // transitions ran on the warm branch (bit for bit once stage 12 has settled).
    let mut q = recorded(quiet(), Route::Speaker);
    let all_on = cc::run_scheduled(&mut q, Source::Playback(1), &[Change::volume(Stream::Playback, 300 + 2 * n, 0.8, 2), Change::battery(Stream::Playback, 300 + 2 * n, 2.5, 3)], Partition::Fixed(256));
    let from = (300 + 7 * n) as usize + settle;
    let end = r.out.len().min(all_on.out.len());
    let worst = (from..end).map(|i| (f64::from(r.out[i]) - f64::from(all_on.out[i])).abs()).fold(0.0, f64::max);
    assert!(worst < 1e-5, "the settled new value heard after the return: {worst}");
    assert_eq!(cc::entries_of(&r.log, 1).len(), 1);
    assert_eq!(cc::entries_of(&r.log, 29).len(), 1);
}

/// Stage 10 at a conversion rate: the first differing and first identical host samples are
/// measured (no pre-quoted onset); the identity holds by the bound with the chain off.
#[test]
fn stage10_conversion_rate_onset_measured() {
    let rows = bp::stage10_trace(&standard, RATE);
    rr_dr60_harness::report::print(&rows);
    rr_dr60_harness::checks::assert_all(&rows);
}

/// The one-pass with the codec on: stages 11 – 13 land; stage 10 is refused there.
#[test]
fn one_pass_codec_on_lands_11_to_13() {
    let mut s = quiet();
    s.speech_codec_enabled = true;
    let mut p = Pipeline::new(s).unwrap();
    assert!(matches!(
        p.schedule_change(Stream::OnePass, 0, 5, 0.0, 1),
        Err(Error::ControlNotOnStream { .. })
    ));
    let x = bp::host_tone(1000.0, 0.5, RATE);
    let sched = [
        Change { stream: Stream::OnePass, position: 1000, control: 22, value: 0.0, tag: 1 },
        Change { stream: Stream::OnePass, position: 1000, control: 23, value: 0.0, tag: 2 },
        Change { stream: Stream::OnePass, position: 1000, control: 24, value: 0.0, tag: 3 },
    ];
    let r = cc::run_scheduled(&mut p, Source::OnePass(&x), &sched, Partition::Fixed(512));
    for c in [22, 23, 24] {
        assert_eq!(cc::entries_of(&r.log, c).len(), 1, "{c}");
    }
    assert!(!p.settings().output.volume_amp_enabled);
}

/// A one-pass at a tap before the chain or before stage 10 refuses every stage it does not
/// reach.
#[test]
fn one_pass_front_tap_refuses_all() {
    for tap in [Tap::AfterAgc, Tap::AfterRecord, Tap::AfterVas, Tap::AfterPlayback] {
        let mut s = quiet();
        s.tap = tap;
        let mut p = Pipeline::new(s).unwrap();
        for c in [22u32, 23, 24] {
            assert!(
                matches!(p.schedule_change(Stream::OnePass, 0, c, 0.0, 1), Err(Error::StageNotConfigured { .. })),
                "{tap:?} {c}"
            );
        }
        // The player still runs every stage.
        p.schedule_change(Stream::Playback, 0, 23, 0.0, 1).unwrap();
    }
}

/// The record stream refuses all four.
#[test]
fn record_stream_refuses() {
    let mut p = Pipeline::new(quiet()).unwrap();
    p.start_recording().unwrap();
    for c in [5u32, 22, 23, 24] {
        assert!(matches!(
            p.schedule_change(Stream::Record, 0, c, 0.0, 1),
            Err(Error::ControlNotOnStream { .. })
        ));
    }
    let mut ev = [VasEvent::default(); 4];
    let _ = p.stop_recording(&mut ev).unwrap();
}

/// A landing of the value a fading stage is already heading to is a no-op entry, the fade
/// untouched.
#[test]
fn landing_same_value_during_fade_is_a_noop() {
    let n = n12();
    let mut p = recorded(quiet(), Route::Speaker);
    let r = cc::run_scheduled(
        &mut p,
        Source::Playback(1),
        &[change(300, 23, false, 1), change(300 + n / 3, 23, false, 2)],
        Partition::Fixed(256),
    );
    let e = cc::entries_of(&r.log, 23);
    assert_eq!(e.len(), 2);
    assert_eq!((e[1].before, e[1].after), (0.0, 0.0));
    let mut q = recorded(quiet(), Route::Speaker);
    let one = cc::run_scheduled(&mut q, Source::Playback(1), &[change(300, 23, false, 1)], Partition::Fixed(256));
    assert!(cc::same(&r.out, &one.out), "the fade untouched");
    let _: Vec<ChangeEntry> = e;
}
