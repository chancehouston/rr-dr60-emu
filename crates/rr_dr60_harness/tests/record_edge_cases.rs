//! Spec 017: the edge cases (tasks.md T010; spec › Edge Cases; research R-01, R-07).

use rr_dr60::changes::{ChangeEntry, Clock, MAX_CHANGES_PER_CALL, clock_of};
use rr_dr60::{Error, MicSettings, Route, Settings, Stream, Tap, VasMode};
use rr_dr60_harness::changes_checks::{self as cc, Partition, same};
use rr_dr60_harness::checks::standard;
use rr_dr60_harness::record_checks::{self as rc, Value};
use rr_dr60_harness::{configs, stimulus};

const RATE: u32 = 48_000;

fn front(mode: VasMode) -> Settings {
    let mut s = configs::settings("default_output", RATE);
    s.mic = MicSettings::DEVICE;
    s.vas.mode = mode;
    s.speech_codec_enabled = false;
    s
}

fn voiced(seconds: f64) -> Vec<f32> {
    stimulus::voiced(f64::from(RATE), seconds, -20.0, 0x017)
}

fn log_buf() -> Vec<ChangeEntry> {
    vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL]
}

/// The two clocks of the one-pass: a route change at output P and a hiss change at input P in
/// one call, each entry in its clock; `clock_of` tells them apart.
#[test]
fn two_clocks_on_the_one_pass() {
    let x = voiced(0.5);
    let mut p = standard(front(VasMode::Drop));
    let latency = p.latency_samples() as u64;
    let pos = 3000u64;
    cc::schedule_all(
        &mut p,
        &[
            cc::Change::route(Stream::OnePass, pos, Route::Earphone, 1),
            rc::change(Stream::OnePass, pos, Value::Hiss(3.0), 2),
        ],
    );
    let mut log = log_buf();
    let mut y = vec![0.0f32; 8192];
    let info = p
        .process_with_events_and_changes(&x[..8192], &mut y, &mut [], &mut log)
        .expect("call");
    assert_eq!(info.changes, 2);
    // The record side first: the hiss at input 3000; then the route at output 3000.
    assert_eq!(
        (log[0].control, log[0].position),
        (Value::Hiss(3.0).control(), pos)
    );
    assert_eq!(
        (log[1].control, log[1].position),
        (cc::route_control(), pos)
    );
    assert_eq!(clock_of(log[0].control).expect("control"), Clock::Input);
    assert_eq!(clock_of(log[1].control).expect("control"), Clock::Output);
    assert_eq!(p.input_position(Stream::OnePass).expect("clock"), 8192);
    assert_eq!(
        p.stream_position(Stream::OnePass).expect("clock"),
        8192 - latency.min(8192) + latency
    ); // the one-pass produces one per input
    assert_eq!(
        p.stream_position(Stream::OnePass).expect("clock"),
        p.input_position(Stream::OnePass).expect("clock")
    );
}

/// The front taps: at `AfterMicCapsule` and `AfterMicAmp` the limiter and the gate do not run,
/// so their values are logged with no effect while the hiss lands; at `AfterAgc` the gate's
/// values are logged with no effect; the two clocks coincide.
#[test]
fn front_taps_log_without_effect() {
    let x = voiced(0.5);
    for tap in [Tap::AfterMicCapsule, Tap::AfterMicAmp, Tap::AfterAgc] {
        let mut s = front(VasMode::Drop);
        s.tap = tap;
        let no_effect: Vec<Value> = if tap == Tap::AfterAgc {
            vec![Value::Sensitivity(5), Value::Hang(50.0)]
        } else {
            vec![Value::Knee(-5.0), Value::Sensitivity(5), Value::Hang(50.0)]
        };
        for v in no_effect {
            let plain = rc::one_pass_run(&mut standard(s), &x, Partition::Fixed(1024), &[]).out;
            let r = rc::one_pass_run(
                &mut standard(s),
                &x,
                Partition::Fixed(1024),
                &[rc::change(Stream::OnePass, 2000, v, 1)],
            );
            assert!(
                same(&plain, &r.out),
                "{tap:?} {}: no effect",
                v.identifier()
            );
            assert_eq!(r.log.len(), 1, "{tap:?}: logged");
            assert_eq!(r.log[0].position, 2000);
        }
        // The hiss lands at every front tap (the capsule is on).
        let plain = rc::one_pass_run(&mut standard(s), &x, Partition::Fixed(1024), &[]).out;
        let r = rc::one_pass_run(
            &mut standard(s),
            &x,
            Partition::Fixed(1024),
            &[rc::change(Stream::OnePass, 2000, Value::Hiss(12.0), 1)],
        );
        assert!(!same(&plain, &r.out), "{tap:?}: the hiss lands");
        assert!(same(&plain[..2000], &r.out[..2000]), "{tap:?}: before P");
        let mut p = standard(s);
        let mut y = vec![0.0f32; 1000];
        let _ = p.process(&x[..1000], &mut y).expect("call");
        assert_eq!(
            p.input_position(Stream::OnePass).expect("clock"),
            p.stream_position(Stream::OnePass).expect("clock")
        );
    }
}

/// The capsule bypassed: a hiss change is accepted, logged, taken at once, no effect. The
/// limiter bypassed: accepted, logged, no effect until a `reconfigure` turns the stage on with
/// the landed value.
#[test]
fn bypassed_stages() {
    let x = voiced(0.5);
    let mut s = front(VasMode::Drop);
    s.mic.capsule_enabled = false;
    let plain = rc::one_pass_run(&mut standard(s), &x, Partition::Fixed(1024), &[]).out;
    let r = rc::one_pass_run(
        &mut standard(s),
        &x,
        Partition::Fixed(1024),
        &[rc::change(Stream::OnePass, 2000, Value::Hiss(12.0), 1)],
    );
    assert!(same(&plain, &r.out));
    assert_eq!(r.log.len(), 1);
    let mut s = front(VasMode::Drop);
    s.agc.enabled = false;
    let loud = stimulus::tone_at_phase(
        1000.0,
        9.0,
        stimulus::peak_phase(1000.0, RATE),
        f64::from(RATE),
        RATE as usize,
    );
    let mut p = standard(s);
    let plain = rc::one_pass_run(&mut standard(s), &loud, Partition::Fixed(1024), &[]).out;
    let r = rc::one_pass_run(
        &mut p,
        &loud,
        Partition::Fixed(1024),
        &[rc::change(Stream::OnePass, 2000, Value::Knee(-20.0), 1)],
    );
    assert!(same(&plain, &r.out), "the limiter off: no effect");
    assert_eq!(r.log.len(), 1);
    assert_eq!(p.settings().agc.knee_dbfs, -20.0, "landed in the settings");
    let mut on = *p.settings();
    on.agc.enabled = true;
    p.reconfigure(on).expect("reconfigure");
    let after = rc::one_pass_run(&mut p, &loud, Partition::Fixed(1024), &[]).out;
    let fresh = rc::one_pass_run(&mut standard(on), &loud, Partition::Fixed(1024), &[]).out;
    assert!(
        same(&after, &fresh),
        "the reconfigured engine equals a fresh one at the landed knee"
    );
    assert!(!same(&after, &plain), "the knee now acts");
}

/// `reset` and `reconfigure`: both one-pass clocks restart, the schedules are cleared, the
/// landed values stay and both record paths adopt them (no entry); a hiss transition in
/// progress ends at once.
#[test]
fn reset_and_reconfigure() {
    let x = voiced(0.5);
    let mut p = standard(front(VasMode::Drop));
    let mut log = log_buf();
    let mut y = vec![0.0f32; 4096];
    cc::schedule_all(
        &mut p,
        &[
            rc::change(Stream::OnePass, 100, Value::Hang(50.0), 1),
            rc::change(Stream::OnePass, 1 << 40, Value::Knee(-5.0), 2),
            rc::change(Stream::Record, 7, Value::Onset(5.0), 3),
        ],
    );
    let info = p
        .process_with_events_and_changes(&x[..4096], &mut y, &mut [], &mut log)
        .expect("call");
    assert_eq!(info.changes, 1);
    assert_eq!(p.pending_changes(Stream::OnePass).expect("count"), 1);
    assert_eq!(p.pending_changes(Stream::Record).expect("count"), 1);
    p.reset();
    assert_eq!(p.input_position(Stream::OnePass).expect("clock"), 0);
    assert_eq!(p.stream_position(Stream::OnePass).expect("clock"), 0);
    assert_eq!(p.pending_changes(Stream::OnePass).expect("count"), 0);
    assert_eq!(p.pending_changes(Stream::Record).expect("count"), 0);
    assert_eq!(p.settings().vas.hang_ms, 50.0, "the landed value stays");
    // Adopted: the next calls log nothing and equal a fresh engine at the landed value.
    let info = p
        .process_with_events_and_changes(&x[..4096], &mut y, &mut [], &mut log)
        .expect("call");
    assert_eq!(info.changes, 0);
    let mut landed = front(VasMode::Drop);
    landed.vas.hang_ms = 50.0;
    let a = rc::one_pass_run(&mut p, &x, Partition::Fixed(1024), &[]).out;
    let mut q = standard(landed);
    let _ = q.process(&x[..4096], &mut y).expect("call");
    let b = rc::one_pass_run(&mut q, &x, Partition::Fixed(1024), &[]).out;
    assert!(
        same(&a, &b),
        "after reset the one-pass equals a fresh engine at the landed value"
    );
    let ra = rc::record_run(&mut p, &x, Partition::Fixed(1024), &[]);
    let rb = rc::record_run(&mut standard(landed), &x, Partition::Fixed(1024), &[]);
    assert_eq!(
        rc::stored_bytes(&ra),
        rc::stored_bytes(&rb),
        "the recorder adopted it"
    );
    assert!(ra.log.is_empty());
    // Reconfigure: the same, through a rebuild.
    cc::schedule_all(
        &mut p,
        &[rc::change(Stream::OnePass, 1 << 40, Value::Knee(-5.0), 2)],
    );
    let settings = *p.settings();
    p.reconfigure(settings).expect("reconfigure");
    assert_eq!(p.pending_changes(Stream::OnePass).expect("count"), 0);
    assert_eq!(p.input_position(Stream::OnePass).expect("clock"), 0);
    // A hiss transition in progress ends at once at reset.
    let silent = stimulus::silence(RATE as usize);
    let mut h = standard(front(VasMode::Drop));
    cc::schedule_all(
        &mut h,
        &[rc::change(Stream::OnePass, 1000, Value::Hiss(12.0), 1)],
    );
    let _ = h.process(&silent[..1100], &mut y[..1100]).expect("call"); // 100 samples into the 960-sample transition
    h.reset();
    let a = rc::one_pass_run(&mut h, &silent, Partition::Fixed(1024), &[]).out;
    let mut twelve = front(VasMode::Drop);
    twelve.mic.hiss_offset_db = 12.0;
    let b = rc::one_pass_run(&mut standard(twelve), &silent, Partition::Fixed(1024), &[]).out;
    assert!(same(&a, &b), "the transition ended at once");
}

/// The skipped-flush stop (nothing fed) clears the schedule and reports 0.
#[test]
fn skipped_flush_stop_clears() {
    let mut p = standard(front(VasMode::Drop));
    cc::schedule_all(
        &mut p,
        &[rc::change(Stream::Record, 0, Value::Hang(50.0), 1)],
    );
    p.start_recording().expect("start");
    assert_eq!(p.pending_changes(Stream::Record).expect("count"), 1);
    let mut log = log_buf();
    let stop = p
        .stop_recording_with_changes(&mut [], &mut log)
        .expect("stop");
    assert_eq!(stop.changes, 0);
    assert_eq!(stop.input_samples_total, 0);
    assert_eq!(p.pending_changes(Stream::Record).expect("count"), 0);
    assert_eq!(p.settings().vas.hang_ms, 1000.0, "nothing landed");
}

/// Changes at one input position: in submission order; for the limiter the last coefficients
/// win, for the hiss one transition toward the last, for the gate the last lengths.
#[test]
fn changes_at_one_position() {
    let x = voiced(0.5);
    let base = front(VasMode::Drop);
    let pos = 3000u64;
    // The limiter: two knees at one position equal one change to the last.
    let two = rc::one_pass_run(
        &mut standard(base),
        &x,
        Partition::Fixed(1024),
        &[
            rc::change(Stream::OnePass, pos, Value::Knee(-10.0), 1),
            rc::change(Stream::OnePass, pos, Value::Knee(-5.0), 2),
        ],
    );
    let one = rc::one_pass_run(
        &mut standard(base),
        &x,
        Partition::Fixed(1024),
        &[rc::change(Stream::OnePass, pos, Value::Knee(-5.0), 2)],
    );
    assert!(same(&two.out, &one.out));
    assert_eq!(two.log.len(), 2);
    assert_eq!(
        (two.log[0].after, two.log[1].before, two.log[1].after),
        (-10.0, -10.0, -5.0),
        "chained"
    );
    // The hiss: one transition toward the last.
    let silent = stimulus::silence(RATE as usize);
    let two = rc::one_pass_run(
        &mut standard(base),
        &silent,
        Partition::Fixed(1024),
        &[
            rc::change(Stream::OnePass, pos, Value::Hiss(-12.0), 1),
            rc::change(Stream::OnePass, pos, Value::Hiss(12.0), 2),
        ],
    );
    let one = rc::one_pass_run(
        &mut standard(base),
        &silent,
        Partition::Fixed(1024),
        &[rc::change(Stream::OnePass, pos, Value::Hiss(12.0), 2)],
    );
    assert!(same(&two.out, &one.out));
    // The gate: the last lengths.
    let two = rc::record_run(
        &mut standard(base),
        &x,
        Partition::Fixed(1024),
        &[
            rc::change(Stream::Record, pos, Value::Hang(50.0), 1),
            rc::change(Stream::Record, pos, Value::Hang(200.0), 2),
        ],
    );
    let one = rc::record_run(
        &mut standard(base),
        &x,
        Partition::Fixed(1024),
        &[rc::change(Stream::Record, pos, Value::Hang(200.0), 2)],
    );
    assert_eq!(rc::stored_bytes(&two), rc::stored_bytes(&one));
    assert_eq!(two.log.len(), 2);
}

/// Mute mode: a landed threshold change moves which samples are stored as zeros, reported by
/// the muted-region events.
#[test]
fn mute_mode_threshold_change() {
    let fs = f64::from(RATE);
    let x = stimulus::tone_at_phase(
        1000.0,
        -43.0,
        stimulus::aligned_phase(1000.0, RATE, 0),
        fs,
        3 * RATE as usize,
    );
    let base = front(VasMode::Mute);
    let plain = rc::record_run(&mut standard(base), &x, Partition::Fixed(1024), &[]);
    let r = rc::record_run(
        &mut standard(base),
        &x,
        Partition::Fixed(1024),
        &[rc::change(
            Stream::Record,
            2 * u64::from(RATE),
            Value::Threshold(-50.0),
            1,
        )],
    );
    let zeros = |s: &[f32]| s.iter().filter(|v| v.to_bits() == 0).count();
    assert!(
        zeros(&plain.samples) > zeros(&r.samples) + 1000,
        "fewer muted samples after the threshold dropped"
    );
    assert_eq!(plain.events.len(), 1, "one region, open to the end");
    assert_eq!(r.events.len(), 1, "one region, closed by the resume");
    assert!(r.events[0].input_length < plain.events[0].input_length);
}

/// Every refusal of US3 AS1 – AS2 stores nothing: the eight ranges, the idle controls, the
/// wrong stream, the clocks.
#[test]
fn refusals_store_nothing() {
    let mut p = standard(front(VasMode::Drop));
    let bad = [
        (Value::Sensitivity(0), 0.0),
        (Value::Sensitivity(0), 6.0),
        (Value::Sensitivity(0), 2.5),
        (Value::Threshold(0.0), 0.5),
        (Value::Hang(0.0), 49.0),
        (Value::Onset(0.0), 201.0),
        (Value::Knee(0.0), 3.5),
        (Value::Attack(0.0), 0.5),
        (Value::Release(0.0), 10_001.0),
        (Value::Hiss(0.0), 12.5),
        (Value::Hiss(0.0), f64::NAN),
    ];
    for (v, value) in bad {
        for stream in [Stream::Record, Stream::OnePass] {
            assert!(
                matches!(
                    p.schedule_change(stream, 0, v.control(), value, 1),
                    Err(Error::InvalidSetting { .. })
                ),
                "{} {value} on {stream:?}",
                v.identifier()
            );
        }
    }
    // The playback stream refuses a record-side control; the record stream a playback control.
    assert!(matches!(
        p.schedule_change(Stream::Playback, 0, Value::Knee(0.0).control(), -5.0, 1),
        Err(Error::ControlNotOnStream { .. })
    ));
    assert!(matches!(
        p.schedule_change(Stream::Record, 0, cc::volume_control(), 0.5, 1),
        Err(Error::ControlNotOnStream { .. })
    ));
    // The idle controls.
    for id in [
        "vas.enabled",
        "vas.mode",
        "agc.enabled",
        "seed",
        "host_rate_hz",
        "memory.capacity_s",
        "mic.capsule_enabled",
        "tap",
    ] {
        let c = cc::control_index(id);
        for stream in [Stream::Record, Stream::OnePass, Stream::Playback] {
            assert!(
                matches!(
                    p.schedule_change(stream, 0, c, 1.0, 1),
                    Err(Error::ControlNotOnStream { .. })
                ),
                "{id} on {stream:?}"
            );
        }
    }
    // The clocks.
    assert!(matches!(
        p.stream_position(Stream::Record),
        Err(Error::NoSuchClock { .. })
    ));
    assert!(matches!(
        p.input_position(Stream::Playback),
        Err(Error::NoSuchClock { .. })
    ));
    assert_eq!(p.input_position(Stream::Record), Ok(0));
    assert!(matches!(
        clock_of(29),
        Err(Error::NoSuchControl { control: 29 })
    ));
    for stream in [Stream::Record, Stream::OnePass, Stream::Playback] {
        assert_eq!(
            p.pending_changes(stream).expect("count"),
            0,
            "{stream:?}: nothing stored"
        );
    }
    // Accepted at the bounds.
    assert_eq!(
        p.schedule_change(Stream::Record, 0, Value::Sensitivity(1).control(), 1.0, 1),
        Ok(())
    );
    assert_eq!(
        p.schedule_change(Stream::Record, 0, Value::Hiss(0.0).control(), -18.0, 1),
        Ok(())
    );
    assert_eq!(p.pending_changes(Stream::Record).expect("count"), 2);
}

/// The adopt rule both ways: a value landed on the one-pass, then `start_recording`, equals a
/// fresh engine at the value with no entry; the reverse through `reset`.
#[test]
fn adopt_rule_both_ways() {
    let x = voiced(0.5);
    let base = front(VasMode::Drop);
    let mut landed = base;
    landed.agc.knee_dbfs = -5.0;
    let loud = stimulus::tone_at_phase(
        1000.0,
        9.0,
        stimulus::peak_phase(1000.0, RATE),
        f64::from(RATE),
        RATE as usize,
    );
    // One-pass → recorder.
    let mut p = standard(base);
    let r = rc::one_pass_run(
        &mut p,
        &x,
        Partition::Fixed(1024),
        &[rc::change(Stream::OnePass, 0, Value::Knee(-5.0), 1)],
    );
    assert_eq!(r.log.len(), 1);
    let a = rc::record_run(&mut p, &loud, Partition::Fixed(1024), &[]);
    let b = rc::record_run(&mut standard(landed), &loud, Partition::Fixed(1024), &[]);
    assert_eq!(rc::stored_bytes(&a), rc::stored_bytes(&b));
    assert!(a.log.is_empty(), "the adopt rule logs none");
    // Recorder → one-pass through reset.
    let mut q = standard(base);
    let r = rc::record_run(
        &mut q,
        &x,
        Partition::Fixed(1024),
        &[rc::change(Stream::Record, 0, Value::Knee(-5.0), 1)],
    );
    assert_eq!(r.log.len(), 1);
    q.reset();
    let a = rc::one_pass_run(&mut q, &loud, Partition::Fixed(1024), &[]);
    // The twin has recorded once too: the one-pass hiss is keyed to the recording counter
    // (spec 012 FR-009), which `reset` keeps.
    let mut t = standard(landed);
    let _ = rc::record_run(&mut t, &x, Partition::Fixed(1024), &[]);
    t.reset();
    let b = rc::one_pass_run(&mut t, &loud, Partition::Fixed(1024), &[]);
    assert!(same(&a.out, &b.out));
    assert!(a.log.is_empty());
}

/// The two sides share the rules: the schedule kept by the start call, cleared by the end,
/// the pending landing at the first sample, chained entries.
#[test]
fn sides_share_the_rules() {
    let x = voiced(0.5);
    let mut p = standard(front(VasMode::Drop));
    // The record side: scheduled before the start, kept by it, landed, cleared by the stop.
    cc::schedule_all(
        &mut p,
        &[
            rc::change(Stream::Record, 100, Value::Hang(50.0), 1),
            rc::change(Stream::Record, 200, Value::Hang(80.0), 2),
            rc::change(Stream::Record, 1 << 40, Value::Hang(90.0), 3),
        ],
    );
    p.start_recording().expect("start");
    assert_eq!(p.pending_changes(Stream::Record).expect("count"), 3);
    let mut log = log_buf();
    let info = p
        .record_with_changes(&x[..4096], &mut [], &mut log)
        .expect("record");
    assert_eq!(info.changes, 2);
    cc::assert_chained(&log[..2], Value::Hang(0.0).control(), 1000.0);
    let stop = p
        .stop_recording_with_changes(&mut [], &mut log)
        .expect("stop");
    assert_eq!(stop.changes, 0);
    assert_eq!(
        p.pending_changes(Stream::Record).expect("count"),
        0,
        "the end clears"
    );
    // The playback side: the same shape (spec 015).
    cc::schedule_all(
        &mut p,
        &[
            cc::Change::volume(Stream::Playback, 100, 0.5, 1),
            cc::Change::volume(Stream::Playback, 1 << 40, 0.6, 2),
        ],
    );
    p.start_playback(1).expect("start");
    assert_eq!(p.pending_changes(Stream::Playback).expect("count"), 2);
    let mut y = vec![0.0f32; 4096];
    let info = p.play_with_changes(&mut y, &mut log).expect("play");
    assert_eq!(info.changes, 1);
    p.stop_playback().expect("stop");
    assert_eq!(p.pending_changes(Stream::Playback).expect("count"), 0);
}

/// A plain record-side change does not exist: the setters are the schedule; `settings()` shows
/// a landed value and not a pending one.
#[test]
fn settings_reflect_landed_not_pending() {
    let x = voiced(0.5);
    let mut p = standard(front(VasMode::Drop));
    cc::schedule_all(
        &mut p,
        &[
            rc::change(Stream::OnePass, 1 << 40, Value::Knee(-5.0), 1),
            rc::change(Stream::OnePass, 10, Value::Attack(10.0), 2),
        ],
    );
    assert_eq!(p.settings().agc.knee_dbfs, -1.0);
    assert_eq!(p.settings().agc.attack_ms, 3.0);
    let mut y = vec![0.0f32; 1024];
    let _ = p.process(&x[..1024], &mut y).expect("call");
    assert_eq!(p.settings().agc.attack_ms, 10.0, "landed");
    assert_eq!(p.settings().agc.knee_dbfs, -1.0, "pending does not show");
}
