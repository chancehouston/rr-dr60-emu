//! Spec 015 edge cases (tasks.md T010; spec › Edge Cases; research R-01, R-04, R-07).

use rr_dr60::changes::{MAX_CHANGES_PER_CALL, SCHEDULE_CAPACITY};
use rr_dr60::{ChangeEntry, Error, Pipeline, Route, Settings, Stream, Tap, VasEvent};
use rr_dr60_harness::changes_checks::{self as cc, Change, Partition, Source, same};
use rr_dr60_harness::{configs, stimulus};

fn settings(rate: u32) -> Settings {
    configs::settings("default_output", rate)
}

fn recorded(rate: u32, seconds: f64) -> (Pipeline, u32) {
    let x = stimulus::voiced(f64::from(rate), seconds, -20.0, 0x015);
    let mut p = Pipeline::new(settings(rate)).expect("settings");
    p.start_recording().expect("start");
    let _ = p.record(&x, &mut []).expect("record");
    let _ = p.stop_recording(&mut []).expect("stop");
    (p, 1)
}

fn log_buf() -> Vec<ChangeEntry> {
    vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL]
}

/// A change in the drain lands and is logged; one past the end is never logged and is cleared
/// at `finished`.
#[test]
fn drain_and_past_the_end() {
    for &rate in configs::measurement_rates() {
        let (mut p, n) = recorded(rate, 0.3);
        let total = p.playback_length_samples(n, 3).expect("length");
        let drain = u64::from(p.drain_samples());
        let in_drain = total - drain / 2;
        let changes = [
            Change::volume(Stream::Playback, in_drain, 0.9, 1),
            Change::volume(Stream::Playback, total + 100, 0.5, 2),
        ];
        let r = cc::run_scheduled(&mut p, Source::Playback(n), &changes, Partition::Fixed(64));
        assert_eq!(r.out.len() as u64, total);
        assert_eq!(r.log.len(), 1, "{rate}: the drain change only");
        assert_eq!(r.log[0].position, in_drain);
        assert_eq!(
            p.pending_changes(Stream::Playback).expect("stream"),
            0,
            "{rate}: cleared at finished"
        );
        assert_eq!(p.volume(), 0.9);
    }
}

/// Schedule-then-start keeps the changes (they land from the playback's first sample); the
/// end of a playback by any path clears the schedule; a refused start and an erase of another
/// number keep it; recording calls leave the one-pass schedule and clock untouched.
#[test]
fn clearing_rules() {
    let rate = 48_000;
    let (mut p, n) = recorded(rate, 0.3);
    let x = stimulus::voiced(f64::from(rate), 0.1, -20.0, 0x015);
    // Schedule, then start.
    p.schedule_change(Stream::Playback, 0, cc::volume_control(), 0.4, 1)
        .expect("schedule");
    p.schedule_change(Stream::Playback, 10, cc::route_control(), 1.0, 2)
        .expect("schedule");
    assert!(matches!(
        p.start_playback(9),
        Err(Error::NoSuchRecording { .. })
    ));
    assert_eq!(
        p.pending_changes(Stream::Playback).expect("stream"),
        2,
        "a refused start keeps it"
    );
    p.start_playback(n).expect("start");
    assert_eq!(
        p.pending_changes(Stream::Playback).expect("stream"),
        2,
        "start_playback keeps it"
    );
    let mut log = log_buf();
    let mut y = vec![0.0f32; 64];
    let info = p.play_with_changes(&mut y, &mut log).expect("play");
    assert_eq!(info.changes, 2);
    assert_eq!((log[0].position, log[1].position), (0, 10));
    // stop_playback clears.
    p.schedule_change(Stream::Playback, 5000, cc::volume_control(), 0.4, 3)
        .expect("schedule");
    p.stop_playback().expect("stop");
    assert_eq!(p.pending_changes(Stream::Playback).expect("stream"), 0);
    // erase of the played item clears; of another number keeps.
    p.start_recording().expect("start");
    let _ = p.record(&x, &mut []).expect("record");
    let _ = p.stop_recording(&mut []).expect("stop"); // recording 2
    p.start_playback(2).expect("start");
    p.schedule_change(Stream::Playback, 5000, cc::volume_control(), 0.4, 4)
        .expect("schedule");
    p.erase(1).expect("erase a lower number");
    assert_eq!(
        p.pending_changes(Stream::Playback).expect("stream"),
        1,
        "an erase of another item keeps it"
    );
    p.erase(1).expect("erase the played item (renumbered)");
    assert_eq!(
        p.pending_changes(Stream::Playback).expect("stream"),
        0,
        "an erase of the played item clears"
    );
    // erase_all while playing clears.
    let (mut p, n) = recorded(rate, 0.3);
    p.start_playback(n).expect("start");
    p.schedule_change(Stream::Playback, 5000, cc::volume_control(), 0.4, 5)
        .expect("schedule");
    p.erase_all().expect("erase_all");
    assert_eq!(p.pending_changes(Stream::Playback).expect("stream"), 0);
    // Recording calls leave the one-pass schedule and clock.
    let mut p = Pipeline::new(settings(rate)).expect("settings");
    let _ = p
        .process(&x[..1000], &mut vec![0.0f32; 1000])
        .expect("call");
    let pos = p.stream_position(Stream::OnePass).expect("stream");
    p.schedule_change(Stream::OnePass, 9000, cc::volume_control(), 0.4, 6)
        .expect("schedule");
    p.start_recording().expect("start");
    let _ = p.record(&x, &mut []).expect("record");
    // A change on the one-pass stream while a path is active: accepted (the stream is idle).
    p.schedule_change(Stream::OnePass, 9500, cc::route_control(), 1.0, 7)
        .expect("schedule while recording");
    let _ = p.stop_recording(&mut []).expect("stop");
    assert_eq!(p.pending_changes(Stream::OnePass).expect("stream"), 2);
    p.clear_schedule(Stream::OnePass).expect("clear");
    p.schedule_change(Stream::OnePass, 9000, cc::volume_control(), 0.4, 6)
        .expect("schedule");
    assert_eq!(p.pending_changes(Stream::OnePass).expect("stream"), 1);
    assert_eq!(p.stream_position(Stream::OnePass).expect("stream"), pos);
    // reset and reconfigure clear both; clear_schedule clears one.
    p.schedule_change(Stream::Playback, 1, cc::volume_control(), 0.4, 7)
        .expect("schedule");
    p.clear_schedule(Stream::Playback).expect("clear");
    assert_eq!(p.pending_changes(Stream::Playback).expect("stream"), 0);
    assert_eq!(p.pending_changes(Stream::OnePass).expect("stream"), 1);
    p.reset();
    assert_eq!(p.pending_changes(Stream::OnePass).expect("stream"), 0);
    assert_eq!(p.stream_position(Stream::OnePass).expect("stream"), 0);
    p.schedule_change(Stream::OnePass, 1, cc::volume_control(), 0.4, 8)
        .expect("schedule");
    p.reconfigure(settings(rate)).expect("reconfigure");
    assert_eq!(p.pending_changes(Stream::OnePass).expect("stream"), 0);
}

/// Every refusal of US3 AS4 stores nothing.
#[test]
fn refusals_store_nothing() {
    let rate = 48_000;
    let mut p = Pipeline::new(settings(rate)).expect("settings");
    let (r, v, s) = (
        cc::route_control(),
        cc::volume_control(),
        cc::speed_control(),
    );
    assert!(matches!(
        p.schedule_change(Stream::Record, 0, v, 0.5, 1),
        Err(Error::ReservedStream { .. })
    ));
    assert!(matches!(
        p.pending_changes(Stream::Record),
        Err(Error::ReservedStream { .. })
    ));
    assert!(matches!(
        p.stream_position(Stream::Record),
        Err(Error::ReservedStream { .. })
    ));
    assert!(matches!(
        p.clear_schedule(Stream::Record),
        Err(Error::ReservedStream { .. })
    ));
    assert!(matches!(
        p.schedule_change(Stream::OnePass, 0, s, 3.0, 1),
        Err(Error::ControlNotOnStream { .. })
    ));
    assert!(matches!(
        p.schedule_change(Stream::OnePass, 0, 5, 1.0, 1),
        Err(Error::ControlNotOnStream { .. })
    ));
    assert!(matches!(
        p.schedule_change(Stream::Playback, 0, 28, 1.0, 1),
        Err(Error::ControlNotOnStream { .. })
    ));
    assert!(matches!(
        p.schedule_change(Stream::Playback, 0, v, 1.5, 1),
        Err(Error::InvalidVolume { .. })
    ));
    assert!(matches!(
        p.schedule_change(Stream::Playback, 0, v, f64::NAN, 1),
        Err(Error::InvalidVolume { .. })
    ));
    assert!(matches!(
        p.schedule_change(Stream::Playback, 0, s, 6.0, 1),
        Err(Error::InvalidSpeedLevel { .. })
    ));
    assert!(matches!(
        p.schedule_change(Stream::Playback, 0, s, 2.5, 1),
        Err(Error::InvalidSpeedLevel { .. })
    ));
    assert!(matches!(
        p.schedule_change(Stream::Playback, 0, r, 2.0, 1),
        Err(Error::InvalidControlValue { .. })
    ));
    for st in [Stream::Playback, Stream::OnePass] {
        assert_eq!(p.pending_changes(st).expect("stream"), 0);
    }
    // The named-tap refusal on the one-pass stream.
    let mut s = settings(rate);
    s.tap = Tap::AfterSpeaker;
    let mut p = Pipeline::new(s).expect("settings");
    assert!(matches!(
        p.schedule_change(Stream::OnePass, 0, r, 1.0, 1),
        Err(Error::TapNotOnRoute { .. })
    ));
    p.schedule_change(Stream::OnePass, 0, r, 0.0, 1)
        .expect("the tap's route");
    p.schedule_change(Stream::Playback, 0, r, 1.0, 1)
        .expect("the player ignores the tap");
    assert_eq!(p.pending_changes(Stream::OnePass).expect("stream"), 1);
    // The 65th.
    let mut p = Pipeline::new(settings(rate)).expect("settings");
    for i in 0..SCHEDULE_CAPACITY as u64 {
        p.schedule_change(Stream::OnePass, i, v, 0.5, 1)
            .expect("schedule");
    }
    assert!(matches!(
        p.schedule_change(Stream::OnePass, 0, v, 0.5, 1),
        Err(Error::ScheduleFull { .. })
    ));
    assert_eq!(p.pending_changes(Stream::OnePass).expect("stream"), 64);
    assert_eq!(
        p.pending_changes(Stream::Playback).expect("stream"),
        0,
        "the other stream is unaffected"
    );
    // A volume is rounded to single precision; −0.0 normalized.
    let mut p = Pipeline::new(settings(rate)).expect("settings");
    p.schedule_change(Stream::OnePass, 0, v, 0.1, 1)
        .expect("a double 0.1 is accepted");
    p.schedule_change(Stream::OnePass, 0, v, -0.0, 2)
        .expect("-0.0");
    let x = stimulus::voiced(f64::from(rate), 0.1, -20.0, 0x015);
    let mut log = log_buf();
    let info = p
        .process_with_events_and_changes(&x[..256], &mut vec![0.0f32; 256], &mut [], &mut log)
        .expect("call");
    assert_eq!(info.changes, 2);
    assert_eq!(log[0].after, f64::from(0.1f32));
    assert!(log[1].after == 0.0 && log[1].after.is_sign_positive());
}

/// The clock equals the voice-activation events' clock and the produced count at every tap,
/// the front taps included: a route change at a splice's position starts the new branch on
/// the event's first post-splice sample; at `AfterAgc` the changes land, are logged, change no
/// output and move the readings.
#[test]
fn the_clock_and_the_front_taps() {
    for &rate in configs::measurement_rates() {
        // The events' clock (drop mode, a burst then silence then a burst).
        let f = f64::from(rate);
        let mut x = stimulus::tone(1000.0, 0.3, f, (0.15 * f) as usize);
        x.extend(stimulus::silence((1.3 * f) as usize));
        x.extend(stimulus::tone(1000.0, 0.3, f, (0.2 * f) as usize));
        let mut s = settings(rate);
        s.output = rr_dr60::OutputSettings::DEVICE;
        let mut p = Pipeline::new(s).expect("settings");
        let mut events = Vec::new();
        let mut out = Vec::new();
        let mut ev = vec![VasEvent::default(); p.max_events(1024)];
        for chunk in x.chunks(1024) {
            let mut y = vec![0.0f32; chunk.len()];
            let info = p.process_with_events(chunk, &mut y, &mut ev).expect("call");
            out.extend_from_slice(&y[..info.produced]);
            events.extend_from_slice(&ev[..info.events]);
        }
        assert_eq!(
            out.len() as u64,
            p.stream_position(Stream::OnePass).expect("stream"),
            "{rate}: the clock is the produced count"
        );
        let splice = events.first().expect("a splice").output_position;
        let mut q = Pipeline::new(s).expect("settings");
        let r = cc::run_scheduled(
            &mut q,
            Source::OnePass(&x),
            &[Change::route(Stream::OnePass, splice, Route::Earphone, 1)],
            Partition::Fixed(1024),
        );
        assert!(
            same(&r.out[..splice as usize], &out[..splice as usize]),
            "{rate}: before the splice"
        );
        assert!(
            !same(&r.out[splice as usize..], &out[splice as usize..]),
            "{rate}: from the splice"
        );
        assert_eq!(r.log[0].position, splice);
        // The front taps.
        for tap in [Tap::AfterMicAmp, Tap::AfterAgc] {
            let mut s = settings(rate);
            s.tap = tap;
            let mut p = Pipeline::new(s).expect("settings");
            let x = stimulus::voiced(f, 0.2, -20.0, 0x015);
            let plain = cc::run(&mut p, Source::OnePass(&x), Partition::Fixed(64));
            assert_eq!(
                p.stream_position(Stream::OnePass).expect("stream"),
                x.len() as u64,
                "{rate} {tap:?}: the clock counts"
            );
            let mut q = Pipeline::new(s).expect("settings");
            let r = cc::run_scheduled(
                &mut q,
                Source::OnePass(&x),
                &[
                    Change::volume(Stream::OnePass, 777, 0.9, 1),
                    Change::route(Stream::OnePass, 999, Route::Earphone, 2),
                ],
                Partition::Fixed(64),
            );
            assert!(
                same(&r.out, &plain.out),
                "{rate} {tap:?}: the chain is not on the path"
            );
            assert_eq!(r.log.len(), 2);
            assert_eq!((r.log[0].position, r.log[1].position), (777, 999));
            assert_eq!(q.volume(), 0.9);
            assert_eq!(q.route(), Route::Earphone);
        }
    }
}

/// A zero-length call and a wholly dropped block apply and log nothing; the change waits.
#[test]
fn zero_production_calls() {
    let rate = 48_000;
    let mut p = Pipeline::new(settings(rate)).expect("settings");
    p.schedule_change(Stream::OnePass, 0, cc::volume_control(), 0.9, 1)
        .expect("schedule");
    let mut log = log_buf();
    let info = p
        .process_with_events_and_changes(&[], &mut [], &mut [], &mut log)
        .expect("call");
    assert_eq!(info.changes, 0);
    assert_eq!(p.pending_changes(Stream::OnePass).expect("stream"), 1);
    assert_eq!(
        p.volume(),
        rr_dr60::output::VOLUME_REFERENCE,
        "the reading has not moved"
    );
    // A wholly dropped block: silence long past the hang, then a block of silence.
    let mut s = settings(rate);
    s.vas.hang_ms = 50.0;
    let mut p = Pipeline::new(s).expect("settings");
    let f = f64::from(rate);
    let mut x = stimulus::tone(1000.0, 0.3, f, (0.1 * f) as usize);
    x.extend(stimulus::silence((0.6 * f) as usize));
    let mut y = vec![0.0f32; x.len()];
    let info = p
        .process_with_events_and_changes(&x, &mut y, &mut [], &mut log)
        .expect("call");
    assert!(info.paused, "the gate paused");
    let pos = p.stream_position(Stream::OnePass).expect("stream");
    p.schedule_change(Stream::OnePass, pos, cc::volume_control(), 0.9, 2)
        .expect("schedule");
    let silence = stimulus::silence(2048);
    let info = p
        .process_with_events_and_changes(&silence, &mut y[..2048], &mut [], &mut log)
        .expect("call");
    assert_eq!(info.produced, 0, "wholly dropped");
    assert_eq!(info.changes, 0, "nothing landed");
    assert_eq!(
        p.pending_changes(Stream::OnePass).expect("stream"),
        1,
        "the change waits"
    );
    assert_eq!(p.stream_position(Stream::OnePass).expect("stream"), pos);
}

/// The readings after one landed change (it), after two (the second), with one pending beyond
/// the block (unchanged).
#[test]
fn readings_follow_the_landing() {
    let rate = 44_100;
    let x = stimulus::voiced(f64::from(rate), 0.2, -20.0, 0x015);
    let mut p = Pipeline::new(settings(rate)).expect("settings");
    cc::schedule_all(
        &mut p,
        &[
            Change::volume(Stream::OnePass, 100, 0.3, 1),
            Change::volume(Stream::OnePass, 200, 0.6, 2),
            Change::route(Stream::OnePass, 300, Route::Earphone, 3),
            Change::volume(Stream::OnePass, 9_000_000, 0.1, 4),
        ],
    );
    let mut y = vec![0.0f32; 1024];
    let info = p.process(&x[..1024], &mut y).expect("call");
    assert_eq!(info.changes, 3);
    assert_eq!(p.volume(), 0.6, "the second landed volume");
    assert_eq!(p.route(), Route::Earphone);
    assert_eq!(p.pending_changes(Stream::OnePass).expect("stream"), 1);
    // The player's level.
    let (mut p, n) = recorded(rate, 0.3);
    p.schedule_change(Stream::Playback, 50, cc::speed_control(), 2.0, 1)
        .expect("schedule");
    p.schedule_change(Stream::Playback, 60, cc::speed_control(), 5.0, 2)
        .expect("schedule");
    p.start_playback(n).expect("start");
    let info = p.play(&mut y).expect("play");
    assert_eq!(info.changes, 2);
    assert_eq!(p.playback_speed(), 5);
}

/// A route scheduled on the playback stream moves the shared reading at landing, so a
/// one-pass configured with the other named-route tap is refused at its next call, as after a
/// plain `set_route` (research R-04, hardware P10).
#[test]
fn scheduled_route_and_the_named_tap() {
    let rate = 48_000;
    let mut s = settings(rate);
    s.tap = Tap::AfterSpeaker;
    let x = stimulus::voiced(f64::from(rate), 0.3, -20.0, 0x015);
    let mut p = Pipeline::new(s).expect("settings");
    p.start_recording().expect("start");
    let _ = p.record(&x, &mut []).expect("record");
    let _ = p.stop_recording(&mut []).expect("stop");
    let mut y = vec![0.0f32; 1024];
    let _ = p
        .process(&x[..1024], &mut y)
        .expect("the speaker tap on the speaker route");
    p.schedule_change(Stream::Playback, 100, cc::route_control(), 1.0, 1)
        .expect("schedule");
    p.start_playback(1).expect("start");
    let info = p.play(&mut y).expect("play");
    assert_eq!(info.changes, 1);
    assert_eq!(p.route(), Route::Earphone);
    p.stop_playback().expect("stop");
    assert!(matches!(
        p.process(&x[..1024], &mut y),
        Err(Error::TapNotOnRoute { .. })
    ));
}

/// Two engines fed the same calls, schedules and stimuli agree in output and log.
#[test]
fn two_engines_agree() {
    for &rate in configs::measurement_rates() {
        let x = stimulus::voiced(f64::from(rate), 0.3, -20.0, 0x015);
        let changes = [
            Change::volume(Stream::OnePass, 333, 0.4, 1),
            Change::route(Stream::OnePass, 444, Route::Earphone, 2),
        ];
        let mut a = Pipeline::new(settings(rate)).expect("settings");
        let mut b = Pipeline::new(settings(rate)).expect("settings");
        let ra = cc::run_scheduled(&mut a, Source::OnePass(&x), &changes, Partition::Irregular);
        let rb = cc::run_scheduled(&mut b, Source::OnePass(&x), &changes, Partition::Irregular);
        assert!(same(&ra.out, &rb.out), "{rate}");
        assert_eq!(ra.log, rb.log, "{rate}");
    }
}
