//! Spec 017: the seven-rate report (tasks.md T007; FR-006, SC-001, SC-004). Report:
//! `cargo test -p rr_dr60_harness --test record_matrix -- --nocapture`.

use rr_dr60::changes::{ChangeEntry, MAX_CHANGES_PER_CALL, SCHEDULE_CAPACITY};
use rr_dr60::{MicSettings, Route, Settings, Stream, VasMode};
use rr_dr60_harness::changes_checks::{self as cc, Change, Partition, same};
use rr_dr60_harness::checks::standard;
use rr_dr60_harness::record_checks::{self as rc, Value};
use rr_dr60_harness::{configs, stimulus};

const PARTITIONS: [Partition; 4] = [
    Partition::Fixed(1),
    Partition::Fixed(64),
    Partition::Fixed(4096),
    Partition::Irregular,
];

fn front(rate: u32, mode: VasMode) -> Settings {
    let mut s = configs::settings("default_output", rate);
    s.mic = MicSettings::DEVICE;
    s.vas.mode = mode;
    s.speech_codec_enabled = false;
    s
}

/// SC-001 at the seven rates: per value and stream, the input-0 identity and the partition
/// identity (0 mismatches), the entry count per call at most the bound.
#[test]
fn identities_per_value_and_stream() {
    for rate in configs::all_rates() {
        let x = stimulus::voiced(f64::from(rate), 0.6, -20.0, 0x017);
        let p0 = u64::from(rate / 5) + 11;
        let mut rows = 0usize;
        for mode in [VasMode::Drop, VasMode::Mute] {
            let base = front(rate, mode);
            for v in Value::examples() {
                let constant = rc::constant_settings(&base, v);
                let tag = format!("{rate} Hz {mode:?} {}", v.identifier());
                // The recorder: input 0 against the constant configuration; P at the partitions.
                let a = rc::record_run(
                    &mut standard(base),
                    &x,
                    Partition::Fixed(1000),
                    &[rc::change(Stream::Record, 0, v, 1)],
                );
                let b = rc::record_run(&mut standard(constant), &x, Partition::Fixed(1000), &[]);
                assert_eq!(
                    rc::stored_bytes(&a),
                    rc::stored_bytes(&b),
                    "{tag}: input 0 (recorder)"
                );
                let runs: Vec<_> = PARTITIONS
                    .iter()
                    .map(|&part| {
                        rc::record_run(
                            &mut standard(base),
                            &x,
                            part,
                            &[rc::change(Stream::Record, p0, v, 2)],
                        )
                    })
                    .collect();
                for r in &runs[1..] {
                    assert_eq!(
                        rc::stored_bytes(&runs[0]),
                        rc::stored_bytes(r),
                        "{tag}: partitions (recorder)"
                    );
                    assert_eq!(runs[0].log, r.log, "{tag}: logs (recorder)");
                }
                assert!(runs.iter().all(|r| r.max_per_call <= MAX_CHANGES_PER_CALL));
                // The one-pass.
                let oa = rc::one_pass_run(
                    &mut standard(base),
                    &x,
                    Partition::Fixed(1000),
                    &[rc::change(Stream::OnePass, 0, v, 1)],
                )
                .out;
                let ob =
                    rc::one_pass_run(&mut standard(constant), &x, Partition::Fixed(1000), &[]).out;
                assert!(
                    same(&oa, &ob),
                    "{tag}: input 0 (one-pass) at {:?}",
                    cc::first_diff(&oa, &ob)
                );
                let outs: Vec<_> = PARTITIONS
                    .iter()
                    .map(|&part| {
                        rc::one_pass_run(
                            &mut standard(base),
                            &x,
                            part,
                            &[rc::change(Stream::OnePass, p0, v, 2)],
                        )
                    })
                    .collect();
                for r in &outs[1..] {
                    assert!(
                        same(&outs[0].out, &r.out),
                        "{tag}: partitions (one-pass) at {:?}",
                        cc::first_diff(&outs[0].out, &r.out)
                    );
                    assert_eq!(outs[0].log, r.log, "{tag}: logs (one-pass)");
                }
                rows += 1;
            }
        }
        println!("{rate:>5} Hz: {rows} value × mode rows on both streams: 0 mismatches");
    }
}

/// k(P) pinned by the stored sum (research R-04): one-sample `record` calls with the gate
/// bypassed and the codec off: the device samples stored after P input samples equal
/// `record_landing(P, rate)`, the index of the first device sample a change at P can touch.
#[test]
fn k_of_p_pinned_by_the_stored_sum() {
    for rate in configs::all_rates() {
        let mut s = configs::settings("bypass_all", rate);
        s.speech_codec_enabled = false;
        let mut p = standard(s);
        p.start_recording().expect("start");
        let (l, m) = rr_dr60_harness::split_checks::l_m(rate);
        let mut stored = 0u64;
        for pos in 1..=300u64 {
            let info = p.record(&[0.01], &mut []).expect("record");
            stored += info.stored as u64;
            assert_eq!(stored, rc::k_of(pos, rate), "{rate} Hz P = {pos}");
            assert_eq!(
                stored,
                rc::smallest_k_due(pos, l, m),
                "{rate} Hz P = {pos}: the independent form"
            );
            assert_eq!(info.input_samples_total, pos);
        }
        let _ = p.stop_recording(&mut []).expect("stop");
        println!(
            "{rate:>5} Hz: k(P) = ⌈P · 6000 ÷ {rate}⌉ equals the stored count for P = 1 … 300 (k(300) = {stored})"
        );
    }
}

/// SC-004: the tight bounds. The recorder's largest call is 64 scheduled entries (the adopt
/// rule logs none for the eight values landed on the one-pass before the start); the
/// one-pass's largest is 138: eight values landed on the recorder, 64 record-side and 64
/// playback-side entries due, and a route and a volume set, all in one call.
#[test]
fn the_tight_bounds() {
    for rate in configs::all_rates() {
        let x = stimulus::voiced(f64::from(rate), 0.3, -20.0, 0x017);
        let base = front(rate, VasMode::Drop);
        let mut p = standard(base);
        let mut log = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
        // Eight values landed on the one-pass.
        let eight: Vec<Change> = Value::examples()
            .iter()
            .enumerate()
            .map(|(i, &v)| rc::change(Stream::OnePass, 0, v, i as u64 + 1))
            .collect();
        cc::schedule_all(&mut p, &eight);
        let mut y = vec![0.0f32; 1024];
        let info = p
            .process_with_events_and_changes(&x[..1024], &mut y, &mut [], &mut log)
            .expect("call");
        assert_eq!(info.changes, 8, "{rate} Hz: the eight on the one-pass");
        // The recorder: 64 scheduled inside one block; the start adopts the eight with no entry.
        let hang = Value::Hang(50.0).control();
        for i in 0..SCHEDULE_CAPACITY as u64 {
            p.schedule_change(Stream::Record, 10 + i, hang, 50.0 + i as f64, 100 + i)
                .expect("schedule");
        }
        assert!(
            p.schedule_change(Stream::Record, 5, hang, 60.0, 999)
                .is_err(),
            "the 65th is refused"
        );
        p.start_recording().expect("start");
        let info = p
            .record_with_changes(&x[..1024], &mut [], &mut log)
            .expect("record");
        assert_eq!(
            info.changes, SCHEDULE_CAPACITY,
            "{rate} Hz: the recorder's 64"
        );
        assert_eq!(p.pending_changes(Stream::Record).expect("count"), 0);
        cc::assert_chained(&log[..info.changes], hang, 50.0);
        let _ = p.stop_recording(&mut []).expect("stop");
        // The reverse: eight landed on the recorder, then one one-pass call with everything due.
        let mut q = standard(base);
        let eight_r: Vec<Change> = Value::examples()
            .iter()
            .enumerate()
            .map(|(i, &v)| rc::change(Stream::Record, 0, v, i as u64 + 1))
            .collect();
        cc::schedule_all(&mut q, &eight_r);
        q.start_recording().expect("start");
        let info = q
            .record_with_changes(&x[..1024], &mut [], &mut log)
            .expect("record");
        assert_eq!(info.changes, 8, "{rate} Hz: the eight on the recorder");
        let _ = q.stop_recording(&mut []).expect("stop");
        for i in 0..SCHEDULE_CAPACITY as u64 {
            q.schedule_change(Stream::OnePass, 1 + i, hang, 50.0 + i as f64, 200 + i)
                .expect("record side");
            q.schedule_change(
                Stream::OnePass,
                1 + i,
                cc::volume_control(),
                0.5 + 0.001 * i as f64,
                300 + i,
            )
            .expect("playback side");
        }
        assert_eq!(q.pending_changes(Stream::OnePass).expect("count"), 128);
        q.set_route(Route::Earphone);
        q.set_volume(0.9).expect("valid");
        let info = q
            .process_with_events_and_changes(&x[..1024], &mut y, &mut [], &mut log)
            .expect("call");
        assert_eq!(info.changes, 138, "{rate} Hz: 8 + 64 + 64 + 2");
        assert!(info.changes < MAX_CHANGES_PER_CALL);
        assert_eq!(p.pending_changes(Stream::OnePass).expect("count"), 0);
        // The record side's entries first (the eight cross-stream landings at input 0, then the
        // 64 scheduled), then the playback side's.
        assert!(
            log[..8]
                .iter()
                .all(|e| e.position == 0 && (1..=8).contains(&e.tag))
        );
        assert!(log[8..72].iter().all(|e| e.control == hang));
        assert_eq!(log[72].tag, 0, "the plain route");
        assert_eq!(log[73].tag, 0, "the plain volume");
        assert_eq!(log[74].tag, 300);
        println!(
            "{rate:>5} Hz: the recorder's largest call {} entries (adopt logged none), the one-pass's {} (the bound {MAX_CHANGES_PER_CALL})",
            SCHEDULE_CAPACITY, info.changes
        );
    }
}
