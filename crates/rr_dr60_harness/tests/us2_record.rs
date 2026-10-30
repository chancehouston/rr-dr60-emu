//! Spec 017 User Story 2: the session reproduces (tasks.md T011; spec › US2 AS1 – AS4) at the
//! measurement rates. Report: `cargo test -p rr_dr60_harness --test us2_record -- --nocapture`.

use rr_dr60::changes::{ChangeEntry, MAX_CHANGES_PER_CALL};
use rr_dr60::speech_codec;
use rr_dr60::{MicSettings, Settings, StopResult, Stream, VasMode};
use rr_dr60_harness::changes_checks::{self as cc, Change, Partition, same};
use rr_dr60_harness::checks::standard;
use rr_dr60_harness::record_checks::{self as rc, Value};
use rr_dr60_harness::{configs, split_checks, stimulus};

fn front(rate: u32, mode: VasMode, codec: bool) -> Settings {
    let mut s = configs::settings("default_output", rate);
    s.mic = MicSettings::DEVICE;
    s.vas.mode = mode;
    s.speech_codec_enabled = codec;
    s
}

fn at(check: impl Fn(u32)) {
    for &r in configs::measurement_rates() {
        check(r);
    }
}

fn voiced(rate: u32, seconds: f64) -> Vec<f32> {
    stimulus::voiced(f64::from(rate), seconds, -20.0, 0x017)
}

/// Three record-side changes on `stream`, inside blocks.
fn three(stream: Stream, rate: u32) -> Vec<Change> {
    let r = u64::from(rate);
    vec![
        rc::change(stream, r / 4 + 3, Value::Knee(-5.0), 1),
        rc::change(stream, r / 2 + 5, Value::Sensitivity(5), 2),
        rc::change(stream, 3 * r / 4 + 7, Value::Hiss(6.0), 3),
    ]
}

/// AS1 (FR-008, SC-003): the same input, settings, seed, counter and schedule on two engines
/// give byte-identical recordings and equal logs, at two partitions.
#[test]
fn as1_two_engines() {
    at(|rate| {
        let x = voiced(rate, 1.0);
        for codec in [false, true] {
            let s = front(rate, VasMode::Drop, codec);
            let a = rc::record_run(
                &mut standard(s),
                &x,
                Partition::Fixed(1024),
                &three(Stream::Record, rate),
            );
            let b = rc::record_run(
                &mut standard(s),
                &x,
                Partition::Irregular,
                &three(Stream::Record, rate),
            );
            assert_eq!(
                rc::stored_bytes(&a),
                rc::stored_bytes(&b),
                "{rate} Hz codec {codec}"
            );
            assert_eq!(a.log, b.log);
            assert_eq!(a.log.len(), 3);
            assert_eq!(a.events, b.events);
            let oa = rc::one_pass_run(
                &mut standard(s),
                &x,
                Partition::Fixed(1024),
                &three(Stream::OnePass, rate),
            );
            let ob = rc::one_pass_run(
                &mut standard(s),
                &x,
                Partition::Irregular,
                &three(Stream::OnePass, rate),
            );
            assert!(same(&oa.out, &ob.out));
            assert_eq!(oa.log, ob.log);
        }
        println!("{rate:>5} Hz: AS1 two engines agree, codec off and on");
    });
}

/// AS2 (FR-006, SC-003): record-then-play with a record-side schedule equals the one-pass with
/// the same schedule under spec 008 FR-009's rule, the codec off and on, drop and mute mode.
#[test]
fn as2_record_then_play_equals_the_one_pass() {
    at(|rate| {
        let x = voiced(rate, 1.5);
        let d = if rate == 0 {
            0
        } else {
            (speech_codec::DELAY_SAMPLES * rate / 6000) as usize
        };
        let per_device = split_checks::host_per_device(rate);
        for mode in [VasMode::Drop, VasMode::Mute] {
            for codec in [false, true] {
                let s = front(rate, mode, codec);
                let d = if codec { d } else { 0 };
                let mut p = standard(s);
                let r = rc::record_run(
                    &mut p,
                    &x,
                    Partition::Fixed(4801),
                    &three(Stream::Record, rate),
                );
                let StopResult::Kept { .. } = r.stop.result else {
                    unreachable!()
                };
                let played = split_checks::play_to_end(&mut p, r.number, 997);
                let pre_drain = &played[..played.len() - p.drain_samples() as usize];
                let pre_encoder = p.record_latency_samples() as usize - d;
                let drain =
                    (speech_codec::DELAY_SAMPLES - speech_codec::LOOKAHEAD_SAMPLES) as usize;
                let tail =
                    pre_encoder + (r.stop.silence_fed_samples as usize + drain + 1) * per_device;
                let mut y = x.clone();
                y.extend(std::iter::repeat_n(0.0f32, tail));
                let want = rc::one_pass_run(
                    &mut standard(s),
                    &y,
                    Partition::Fixed(4096),
                    &three(Stream::OnePass, rate),
                );
                let n = pre_drain.len();
                assert!(
                    want.out.len() >= d + n,
                    "{rate} Hz {mode:?} codec {codec}: the one-pass is long enough"
                );
                assert!(
                    same(pre_drain, &want.out[d..d + n]),
                    "{rate} Hz {mode:?} codec {codec}: record-then-play equals the one-pass at {:?}",
                    cc::first_diff(pre_drain, &want.out[d..d + n])
                );
                // The record side's entries agree (the one-pass logs its three at the same
                // input positions).
                let rec: Vec<(u64, u32, u64)> = r
                    .log
                    .iter()
                    .map(|e| (e.position, e.control, e.tag))
                    .collect();
                let one: Vec<(u64, u32, u64)> = want
                    .log
                    .iter()
                    .map(|e| (e.position, e.control, e.tag))
                    .collect();
                assert_eq!(rec, one, "{rate} Hz: the entries");
            }
        }
        println!(
            "{rate:>5} Hz: AS2 record-then-play equals the one-pass with three record-side changes, both modes, codec off and on"
        );
    });
}

/// AS3 (FR-002, FR-005, SC-004): changes before the start land at their positions (0 on the
/// first sample); changes inside the stop's fed silence land and are in the stop's report with
/// the final position; a change at or past the total is dropped without an entry and the
/// schedule is empty after; a self-stop clears the schedule and reports the entries landed
/// before the stopping sample and the final position.
#[test]
fn as3_the_stop() {
    at(|rate| {
        let x = voiced(rate, 1.0);
        let mut s = front(rate, VasMode::Drop, false);
        s.vas.enabled = false;
        let mut p = standard(s);
        let mut log = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
        let latency = u64::from(p.record_latency_samples());
        let total = 1000 + latency;
        // Inside the flush: half-way through the record latency the stop feeds (at 6 kHz the
        // latency is a few samples, so the position is 1000 + latency / 2).
        let inside = 1000 + latency / 2;
        cc::schedule_all(
            &mut p,
            &[
                rc::change(Stream::Record, 0, Value::Knee(-5.0), 1),
                rc::change(Stream::Record, 500, Value::Attack(10.0), 2),
                rc::change(Stream::Record, inside, Value::Release(400.0), 3), // inside the flush
                rc::change(Stream::Record, total, Value::Hiss(6.0), 4), // at the total: dropped
                rc::change(Stream::Record, total + 5, Value::Hiss(9.0), 5), // past it: dropped
            ],
        );
        p.start_recording().expect("start");
        let info = p
            .record_with_changes(&x[..1000], &mut [], &mut log)
            .expect("record");
        assert_eq!(info.changes, 2, "{rate} Hz");
        assert_eq!((log[0].position, log[0].tag), (0, 1));
        assert_eq!((log[1].position, log[1].tag), (500, 2));
        assert_eq!(info.input_samples_total, 1000);
        let stop = p
            .stop_recording_with_changes(&mut [], &mut log)
            .expect("stop");
        assert_eq!(stop.changes, 1, "{rate} Hz: the change inside the flush");
        assert_eq!((log[0].position, log[0].tag), (inside, 3));
        assert_eq!(
            stop.input_samples_total, total,
            "{rate} Hz: the final position"
        );
        assert_eq!(
            p.pending_changes(Stream::Record).expect("count"),
            0,
            "the end clears"
        );
        assert_eq!(
            p.settings().mic.hiss_offset_db,
            0.0,
            "the dropped changes never landed"
        );
        assert_eq!(p.settings().agc.release_ms, 400.0);
        // The self-stop: a 1 s capacity, 2 s of input.
        let small = configs::memory_capacity(s, 1, 2);
        let mut q = standard(small);
        cc::schedule_all(
            &mut q,
            &[
                rc::change(Stream::Record, 100, Value::Knee(-5.0), 1),
                rc::change(Stream::Record, 10 * u64::from(rate), Value::Knee(-10.0), 2),
            ],
        );
        q.start_recording().expect("start");
        let long = voiced(rate, 2.0);
        let mut landed = Vec::new();
        let mut last = None;
        for chunk in long.chunks(4096) {
            let info = q
                .record_with_changes(chunk, &mut [], &mut log)
                .expect("record");
            landed.extend_from_slice(&log[..info.changes]);
            last = Some(info);
            if info.stopped.is_some() {
                break;
            }
        }
        let info = last.expect("a call");
        assert!(info.stopped.is_some(), "{rate} Hz: the memory filled");
        assert_eq!(landed.len(), 1);
        assert_eq!(landed[0].position, 100);
        // The 6000th device sample is produced by the smallest P with k(P) = 6000: ⌊5999 · rate
        // ÷ 6000⌋ + 1 (the conversion yields a device sample on the first host sample of each
        // group).
        let final_position = (5999 * u64::from(rate)) / 6000 + 1;
        assert_eq!(rc::k_of(final_position, rate), 6000);
        assert_eq!(rc::k_of(final_position - 1, rate), 5999);
        assert_eq!(
            info.input_samples_total, final_position,
            "{rate} Hz: the final position of a 1 s capacity"
        );
        assert_eq!(
            q.pending_changes(Stream::Record).expect("count"),
            0,
            "the self-stop clears"
        );
        assert_eq!(q.input_position(Stream::Record).expect("clock"), 0, "idle");
        println!(
            "{rate:>5} Hz: AS3 the stop: entries at 0 and 500, one inside the flush at {inside}, final position {total}; the self-stop at {final_position} with the schedule cleared"
        );
    });
}

/// AS4 (FR-004): a value landed on the recorder shows in `settings()` from the landing; the
/// one-pass's first call after the stop lands it with the same tag; `reconfigure(settings())`
/// then equals a fresh engine on both streams; a pending change does not show.
#[test]
fn as4_cross_stream_and_settings() {
    at(|rate| {
        let x = voiced(rate, 0.5);
        let base = front(rate, VasMode::Drop, false);
        let mut p = standard(base);
        let mut log = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
        cc::schedule_all(
            &mut p,
            &[
                rc::change(Stream::Record, 100, Value::Knee(-5.0), 11),
                rc::change(Stream::Record, 1 << 40, Value::Hang(50.0), 12),
            ],
        );
        p.start_recording().expect("start");
        let info = p
            .record_with_changes(&x[..1000], &mut [], &mut log)
            .expect("record");
        assert_eq!(info.changes, 1);
        assert_eq!(
            p.settings().agc.knee_dbfs,
            -5.0,
            "{rate} Hz: settings() from the landing"
        );
        assert_eq!(
            p.settings().vas.hang_ms,
            1000.0,
            "a pending change does not show"
        );
        let _ = p.stop_recording(&mut []).expect("stop");
        // The one-pass's first call lands it at its input position with the same tag.
        let before = p.input_position(Stream::OnePass).expect("clock");
        let mut y = vec![0.0f32; 1000];
        let info = p
            .process_with_events_and_changes(&x[..1000], &mut y, &mut [], &mut log)
            .expect("call");
        assert_eq!(info.changes, 1, "{rate} Hz: the cross-stream landing");
        assert_eq!(
            (
                log[0].position,
                log[0].control,
                log[0].before,
                log[0].after,
                log[0].tag
            ),
            (before, Value::Knee(0.0).control(), -1.0, -5.0, 11)
        );
        // A second call logs nothing more.
        let info = p
            .process_with_events_and_changes(&x[..1000], &mut y, &mut [], &mut log)
            .expect("call");
        assert_eq!(info.changes, 0);
        // reconfigure(settings()) equals a fresh engine at the landed value on both streams
        // (the twin has recorded once too, for the hiss's counter).
        let settings = *p.settings();
        p.reconfigure(settings).expect("reconfigure");
        let mut landed = base;
        landed.agc.knee_dbfs = -5.0;
        let mut t = standard(landed);
        let _ = rc::record_run(&mut t, &x, Partition::Fixed(1024), &[]);
        t.reset();
        let loud = stimulus::tone_at_phase(
            1000.0,
            9.0,
            stimulus::peak_phase(1000.0, rate),
            f64::from(rate),
            rate as usize,
        );
        let a = rc::one_pass_run(&mut p, &loud, Partition::Fixed(1024), &[]);
        let b = rc::one_pass_run(&mut t, &loud, Partition::Fixed(1024), &[]);
        assert!(
            same(&a.out, &b.out),
            "{rate} Hz: the one-pass after reconfigure"
        );
        assert!(a.log.is_empty() && b.log.is_empty());
        let ra = rc::record_run(&mut p, &loud, Partition::Fixed(1024), &[]);
        let rb = rc::record_run(&mut t, &loud, Partition::Fixed(1024), &[]);
        assert_eq!(
            rc::stored_bytes(&ra),
            rc::stored_bytes(&rb),
            "{rate} Hz: the recorder after reconfigure"
        );
        println!(
            "{rate:>5} Hz: AS4 the cross-stream landing carries tag 11 at input {before}; reconfigure(settings()) equals a fresh engine"
        );
    });
}
