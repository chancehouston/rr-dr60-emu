//! Spec 015 User Story 2: a log of what changed when (tasks.md T011; spec › US2 AS1 – AS6).
//! Report: `cargo test -p rr_dr60_harness --test us2_changes -- --nocapture`.

use rr_dr60::changes::MAX_CHANGES_PER_CALL;
use rr_dr60::output::VOLUME_REFERENCE;
use rr_dr60::{ChangeEntry, Pipeline, Route, Settings, Stream};
use rr_dr60_harness::changes_checks::{self as cc, Change, Partition, Source, same};
use rr_dr60_harness::{configs, stimulus};

fn settings(rate: u32) -> Settings {
    configs::settings("default_output", rate)
}

fn recorded(rate: u32) -> (Pipeline, u32) {
    let x = stimulus::voiced(f64::from(rate), 0.5, -20.0, 0x015);
    let mut p = Pipeline::new(settings(rate)).expect("settings");
    p.start_recording().expect("start");
    let _ = p.record(&x, &mut []).expect("record");
    let _ = p.stop_recording(&mut []).expect("stop");
    (p, 1)
}

fn at(check: impl Fn(u32)) {
    for &r in configs::measurement_rates() {
        check(r);
    }
}

/// AS1 (FR-005): each change once, in the call that produced its sample, with the landed
/// position, the control, the chained values and the tag; identical concatenations at every
/// partition.
#[test]
fn as1_log_matches_the_schedule() {
    at(|rate| {
        let x = stimulus::voiced(f64::from(rate), 1.0, -20.0, 0x015);
        let r = u64::from(rate);
        let changes = [
            Change::volume(Stream::OnePass, r / 8 + 7, 0.3, 11),
            Change::route(Stream::OnePass, r / 4 + 7, Route::Earphone, 12),
            Change::volume(Stream::OnePass, r / 4 + 7, 0.6, 13),
            Change::volume(Stream::OnePass, r / 2 + 7, 0.9, 14),
        ];
        let mut logs = Vec::new();
        for part in [
            Partition::Fixed(1),
            Partition::Fixed(64),
            Partition::Fixed(4096),
            Partition::Irregular,
        ] {
            let mut p = Pipeline::new(settings(rate)).expect("settings");
            let r = cc::run_scheduled(&mut p, Source::OnePass(&x), &changes, part);
            // The positions are the output clock's (the gate may drop input); each entry is
            // reported by the call that produced its sample.
            assert_eq!(r.log.len(), 4, "{rate} {part:?}");
            for (e, c) in r.log.iter().zip(&changes) {
                assert_eq!(
                    (e.position, e.control, e.after, e.tag),
                    (c.position, c.control, c.value, c.tag),
                    "{rate} {part:?}"
                );
            }
            cc::assert_chained(&r.log, cc::volume_control(), f64::from(VOLUME_REFERENCE));
            cc::assert_chained(&r.log, cc::route_control(), 0.0);
            logs.push(r.log);
        }
        for l in &logs[1..] {
            assert_eq!(l, &logs[0], "{rate}: the logs over the partitions");
        }
        println!(
            "{rate:>5} Hz: {:?}",
            logs[0]
                .iter()
                .map(|e| (e.position, e.control, e.before, e.after, e.tag))
                .collect::<Vec<_>>()
        );
    });
}

/// AS2 (FR-004, FR-005): a plain setter between two calls logs one entry at the next call's
/// first sample with tag 0; several setter calls of one control log only the last; a setter
/// before `start_playback`, `reset` or `reconfigure` logs nothing.
#[test]
fn as2_plain_setters_and_lifecycle() {
    at(|rate| {
        let x = stimulus::voiced(f64::from(rate), 0.4, -20.0, 0x015);
        let mut p = Pipeline::new(settings(rate)).expect("settings");
        let mut log = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
        let n = 1024usize;
        let mut y = vec![0.0f32; n];
        let info = p
            .process_with_events_and_changes(&x[..n], &mut y, &mut [], &mut log)
            .expect("call");
        assert_eq!(info.changes, 0, "{rate}: nothing changed");
        let pos = p.stream_position(Stream::OnePass).expect("stream");
        p.set_volume(0.3).expect("valid");
        p.set_volume(0.5).expect("valid");
        p.set_volume(0.7).expect("valid");
        p.set_route(Route::Earphone);
        let info = p
            .process_with_events_and_changes(&x[n..2 * n], &mut y, &mut [], &mut log)
            .expect("call");
        assert_eq!(info.changes, 2, "{rate}: the route and the last volume");
        let route = log[..2]
            .iter()
            .find(|e| e.control == cc::route_control())
            .expect("route");
        let vol = log[..2]
            .iter()
            .find(|e| e.control == cc::volume_control())
            .expect("volume");
        assert_eq!(
            (route.position, route.tag, route.before, route.after),
            (pos, 0, 0.0, 1.0)
        );
        assert_eq!(
            (vol.position, vol.tag, vol.after),
            (pos, 0, f64::from(0.7f32))
        );
        assert_eq!(vol.before, f64::from(VOLUME_REFERENCE));
        // Before start_playback: consumed, no entry.
        let (mut p, number) = recorded(rate);
        p.set_volume(0.2).expect("valid");
        p.set_playback_speed(4).expect("valid");
        p.start_playback(number).expect("start");
        let info = p.play_with_changes(&mut y, &mut log).expect("play");
        assert_eq!(
            info.changes, 0,
            "{rate}: a setter before start_playback logs nothing"
        );
        // Between two play calls: logged.
        p.set_volume(0.6).expect("valid");
        let pos = p.stream_position(Stream::Playback).expect("stream");
        let info = p.play_with_changes(&mut y, &mut log).expect("play");
        assert_eq!(info.changes, 1);
        assert_eq!(
            (log[0].position, log[0].tag, log[0].before, log[0].after),
            (pos, 0, f64::from(0.2f32), f64::from(0.6f32))
        );
        // Before reset / reconfigure: consumed.
        let mut p = Pipeline::new(settings(rate)).expect("settings");
        p.set_volume(0.33).expect("valid");
        p.reset();
        let info = p
            .process_with_events_and_changes(&x[..n], &mut y, &mut [], &mut log)
            .expect("call");
        assert_eq!(
            info.changes, 0,
            "{rate}: a setter before reset logs nothing"
        );
        p.set_route(Route::Earphone);
        p.reconfigure(settings(rate)).expect("reconfigure");
        let info = p
            .process_with_events_and_changes(&x[..n], &mut y, &mut [], &mut log)
            .expect("call");
        assert_eq!(
            info.changes, 0,
            "{rate}: a setter before reconfigure logs nothing"
        );
        assert_eq!(p.route(), Route::Earphone);
    });
}

/// AS3 (FR-005): a short buffer: every change applied, the first entries written, the count
/// reported; the published length is always enough.
#[test]
fn as3_short_buffer() {
    at(|rate| {
        let x = stimulus::voiced(f64::from(rate), 1.0, -20.0, 0x015);
        let mut p = Pipeline::new(settings(rate)).expect("settings");
        let changes: Vec<Change> = (0..5u64)
            .map(|i| Change::volume(Stream::OnePass, 100 + i, 0.1 + 0.1 * i as f32, i + 1))
            .collect();
        cc::schedule_all(&mut p, &changes);
        let mut short = vec![ChangeEntry::default(); 2];
        let mut y = vec![0.0f32; 4096];
        let info = p
            .process_with_events_and_changes(&x[..4096], &mut y, &mut [], &mut short)
            .expect("call");
        assert_eq!(info.changes, 5, "{rate}");
        assert_eq!((short[0].tag, short[1].tag), (1, 2));
        assert_eq!(p.pending_changes(Stream::OnePass).expect("stream"), 0);
        assert!(info.changes <= MAX_CHANGES_PER_CALL);
        // The same schedule on a fresh engine with a full buffer: the five, chained.
        let mut q = Pipeline::new(settings(rate)).expect("settings");
        let r = cc::run_scheduled(
            &mut q,
            Source::OnePass(&x[..4096]),
            &changes,
            Partition::Fixed(4096),
        );
        assert_eq!(r.log.len(), 5);
        cc::assert_chained(&r.log, cc::volume_control(), f64::from(VOLUME_REFERENCE));
        assert!(same(&y[..info.produced], &r.out));
    });
}

/// AS4 (FR-005): the plain variants apply the changes and count them; no buffer written; the
/// outputs equal the log-taking variants'.
#[test]
fn as4_plain_variants_discard() {
    at(|rate| {
        let x = stimulus::voiced(f64::from(rate), 1.0, -20.0, 0x015);
        let changes = [
            Change::volume(Stream::OnePass, 500, 0.9, 1),
            Change::route(Stream::OnePass, 900, Route::Earphone, 2),
        ];
        let mut p = Pipeline::new(settings(rate)).expect("settings");
        cc::schedule_all(&mut p, &changes);
        let mut y = vec![0.0f32; 4096];
        let info = p.process(&x[..4096], &mut y).expect("call");
        assert_eq!(info.changes, 2, "{rate}: the plain call counts");
        assert_eq!(p.pending_changes(Stream::OnePass).expect("stream"), 0);
        assert_eq!(p.route(), Route::Earphone);
        let mut q = Pipeline::new(settings(rate)).expect("settings");
        let r = cc::run_scheduled(
            &mut q,
            Source::OnePass(&x[..4096]),
            &changes,
            Partition::Fixed(4096),
        );
        assert!(same(&y[..info.produced], &r.out), "{rate}");
        // The player's plain `play`.
        let (mut p, n) = recorded(rate);
        p.schedule_change(Stream::Playback, 300, cc::speed_control(), 5.0, 7)
            .expect("schedule");
        p.start_playback(n).expect("start");
        let info = p.play(&mut y).expect("play");
        assert_eq!(info.changes, 1);
        assert_eq!(p.playback_speed(), 5);
    });
}

/// AS5 (FR-006): a scheduled change to the applied value is a logged no-op; a plain setter to
/// the applied value is not logged.
#[test]
fn as5_same_value() {
    at(|rate| {
        let x = stimulus::voiced(f64::from(rate), 0.3, -20.0, 0x015);
        let mut p = Pipeline::new(settings(rate)).expect("settings");
        let plain = cc::run(&mut p, Source::OnePass(&x), Partition::Fixed(64)).out;
        let mut q = Pipeline::new(settings(rate)).expect("settings");
        let r = cc::run_scheduled(
            &mut q,
            Source::OnePass(&x),
            &[
                Change::volume(Stream::OnePass, 777, VOLUME_REFERENCE, 5),
                Change::route(Stream::OnePass, 999, Route::Speaker, 6),
            ],
            Partition::Fixed(64),
        );
        assert!(same(&r.out, &plain), "{rate}: a no-op changes nothing");
        assert_eq!(r.log.len(), 2);
        assert_eq!(
            (r.log[0].before, r.log[0].after, r.log[0].tag),
            (f64::from(VOLUME_REFERENCE), f64::from(VOLUME_REFERENCE), 5)
        );
        assert_eq!(
            (r.log[1].before, r.log[1].after, r.log[1].tag),
            (0.0, 0.0, 6)
        );
        let mut p = Pipeline::new(settings(rate)).expect("settings");
        p.set_volume(VOLUME_REFERENCE).expect("valid");
        p.set_route(Route::Speaker);
        let mut log = vec![ChangeEntry::default(); 4];
        let mut y = vec![0.0f32; 1024];
        let info = p
            .process_with_events_and_changes(&x[..1024], &mut y, &mut [], &mut log)
            .expect("call");
        assert_eq!(
            info.changes, 0,
            "{rate}: a plain same-value setter is not logged"
        );
    });
}

/// AS6 (FR-005; spec B2): a value set or scheduled during a playback reaches the one-pass
/// stream as a tag-0 entry at its next producing call, where its own transition lands.
#[test]
fn as6_cross_stream() {
    at(|rate| {
        // A tone in two 100 ms blocks (the voiced stimulus opens quietly, and the path's latency
        // at 88.2 kHz is about 10 ms).
        let len = rate as usize / 10;
        let x = stimulus::tone(1000.0, stimulus::amplitude(-20.0), f64::from(rate), 2 * len);
        for origin in ["none", "setter", "scheduled"] {
            let (mut p, n) = recorded(rate);
            let mut y = vec![0.0f32; len];
            let mut log = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
            // The one-pass runs once so its stream has its own last applied values.
            let _ = p
                .process_with_events_and_changes(&x[..len], &mut y, &mut [], &mut log)
                .expect("call");
            let one_pass_pos = p.stream_position(Stream::OnePass).expect("stream");
            p.start_playback(n).expect("start");
            let _ = p.play_with_changes(&mut y, &mut log).expect("play");
            match origin {
                "none" => {}
                "setter" => {
                    p.set_volume(0.6).expect("valid");
                    p.set_route(Route::Earphone);
                }
                _ => {
                    let at = len as u64 + 10; // inside the second play call
                    p.schedule_change(Stream::Playback, at, cc::volume_control(), 0.6, 21)
                        .expect("schedule");
                    p.schedule_change(Stream::Playback, at + 20, cc::route_control(), 1.0, 22)
                        .expect("schedule");
                }
            }
            let info = p.play_with_changes(&mut y, &mut log).expect("play");
            let want = usize::from(origin != "none") * 2;
            assert_eq!(info.changes, want, "{rate} {origin}: logged by the player");
            p.stop_playback().expect("stop");
            // The one-pass's next call: the two land as tag-0 entries at its own position.
            let info = p
                .process_with_events_and_changes(&x[len..2 * len], &mut y, &mut [], &mut log)
                .expect("call");
            assert_eq!(
                info.changes, want,
                "{rate} {origin}: the one-pass logs them again"
            );
            // The comparison engine: the two one-pass blocks with no playback between them.
            let mut q = Pipeline::new(settings(rate)).expect("settings");
            let mut scratch = vec![0.0f32; len];
            let _ = q.process(&x[..len], &mut scratch).expect("call");
            let mut y2 = vec![0.0f32; len];
            let info2 = q.process(&x[len..2 * len], &mut y2).expect("call");
            if origin == "none" {
                assert!(
                    same(&y[..info.produced], &y2[..info2.produced]),
                    "{rate}: a playback between two one-pass calls leaves the one-pass unchanged"
                );
                continue;
            }
            for e in &log[..2] {
                assert_eq!(e.tag, 0);
                assert_eq!(e.position, one_pass_pos);
            }
            let vol = log[..2]
                .iter()
                .find(|e| e.control == cc::volume_control())
                .expect("volume");
            assert_eq!(
                (vol.before, vol.after),
                (f64::from(VOLUME_REFERENCE), f64::from(0.6f32))
            );
            // The transition is visible: the output differs from the run that kept the values.
            assert!(
                !same(&y[..info.produced], &y2[..info2.produced]),
                "{rate} {origin}: the transition landed"
            );
        }
    });
}
