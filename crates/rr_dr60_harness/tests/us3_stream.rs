//! Spec 027 US3 [TR: D2, D3, D5, D6, FR-002, FR-003, FR-006; SC-002]: streaming, progress,
//! cancel, a short input, the feed edges and an hour at 48 kHz (release, ignored).

use rr_dr60::rerender::{Plan, ReRender, Verdict};
use rr_dr60::{Error, Pipeline, Settings};
use rr_dr60_harness::checks::standard;
use rr_dr60_harness::configs;
use rr_dr60_harness::record_contract_checks::{self as rc, Cell};

const CELL: Cell = Cell {
    codec: true,
    changes: true,
    mute: false,
    self_stop: false,
};

/// AS1: the meters after one 4096-sample feed equal a hand-built engine's after the same input
/// (the session's meters are the engine's last call); `new`'s footprint reported per encoding.
/// The allocation counts are `alloc_free rerender_allocate_nothing`.
#[test]
fn as1_rerender_meters_and_footprint() {
    for &rate in configs::measurement_rates() {
        for codec in [false, true] {
            let raw = rc::raw_stimulus(rate);
            let cell = Cell {
                codec,
                changes: false,
                ..CELL
            };
            let (_, run) = rc::record_run(&standard, rate, cell, &raw);
            let plan = Plan::from_record(&run.record).unwrap();
            let (settings, counter) = (*plan.settings(), plan.counter());
            let remaining = plan.remaining_samples();
            let mut r = ReRender::new(plan).unwrap();
            let _ = r.feed(&raw[..4096]).unwrap();
            let mut p = Pipeline::new_at_counter(settings, counter).unwrap();
            p.fill_to_remaining(remaining).unwrap();
            p.start_recording().unwrap();
            let _ = p.record(&rc::samples(&raw[..4096]), &mut []).unwrap();
            assert_eq!(r.meters(), p.meters(), "{rate} Hz codec {codec}");
            println!(
                "{rate} Hz codec {codec}: session engine footprint {} bytes",
                p.footprint_bytes()
            );
        }
    }
}

/// AS3: dropped half-way, a new session still reads `Identical`; progress rises to the stop.
#[test]
fn as3_rerender_cancel_and_progress() {
    for &rate in configs::measurement_rates() {
        let raw = rc::raw_stimulus(rate);
        let (_, run) = rc::record_run(&standard, rate, CELL, &raw);
        let mut r = ReRender::new(Plan::from_record(&run.record).unwrap()).unwrap();
        let (_, stop) = r.progress();
        assert_eq!(stop, run.stop.stop_input_position);
        let _ = r.feed(&raw[..raw.len() / 2]).unwrap();
        assert_eq!(r.progress().0, (raw.len() / 2) as u64);
        drop(r);
        let mut r = ReRender::new(Plan::from_record(&run.record).unwrap()).unwrap();
        let mut last = 0;
        for block in raw.chunks(5000) {
            let f = r.feed(block).unwrap();
            let (pos, _) = r.progress();
            assert!(pos >= last);
            last = pos;
            if f.reached_stop {
                break;
            }
        }
        assert_eq!(last, stop);
        assert_eq!(r.finish().unwrap().verdict, Verdict::Identical, "{rate} Hz");
    }
}

/// AS4: an input that ends before the stop position: `InputShort` with the samples needed; the
/// session stays feedable and then reads `Identical`.
#[test]
fn as4_rerender_input_short() {
    for &rate in configs::measurement_rates() {
        let raw = rc::raw_stimulus(rate);
        let (_, run) = rc::record_run(&standard, rate, CELL, &raw);
        let mut r = ReRender::new(Plan::from_record(&run.record).unwrap()).unwrap();
        let half = raw.len() / 2;
        let _ = r.feed(&raw[..half]).unwrap();
        let needed = run.stop.stop_input_position - half as u64;
        assert_eq!(r.finish().unwrap_err(), Error::InputShort { needed });
        let _ = r.feed(&raw[half..]).unwrap();
        assert_eq!(r.finish().unwrap().verdict, Verdict::Identical, "{rate} Hz");
        assert_eq!(r.finish().unwrap_err(), Error::ReRenderState);
        assert_eq!(r.feed(&raw[..1]).unwrap_err(), Error::ReRenderState);
    }
}

fn resign(bytes: Vec<u8>) -> Vec<u8> {
    let body_end = bytes[..bytes.len() - 1]
        .iter()
        .rposition(|&b| b == b'\n')
        .unwrap()
        + 1;
    let h = rr_dr60::digest::fnv1a64(&bytes[..body_end]);
    let mut v = bytes[..body_end].to_vec();
    v.extend_from_slice(format!("record_checksum = fnv1a64:{h:016x}\n").as_bytes());
    v
}

/// AS5: the feed edges: an out-of-range code (nothing taken), mixed kinds, input past the stop,
/// `feed_f32` equal to `feed` for the helper's samples, `RunTooLarge` and a digest-only session.
#[test]
fn as5_rerender_feed_edges() {
    let rate = 48_000;
    let raw = rc::raw_stimulus(rate);
    let (_, run) = rc::record_run(&standard, rate, CELL, &raw);
    let plan = || Plan::from_record(&run.record).unwrap();
    let mut r = ReRender::new(plan()).unwrap();
    assert_eq!(r.feed(&[0, 1 << 23]).unwrap_err(), Error::InvalidSample);
    assert_eq!(r.feed(&[-(1 << 23) - 1]).unwrap_err(), Error::InvalidSample);
    assert_eq!(r.progress().0, 0, "nothing taken");
    let _ = r.feed(&raw[..10]).unwrap();
    assert_eq!(r.feed_f32(&[0.0]).unwrap_err(), Error::FeedKindMixed);
    // Input past the stop: only up to the stop is taken.
    let mut long = raw.clone();
    long.extend(std::iter::repeat_n(0, 50_000));
    let f = r.feed(&long[10..]).unwrap();
    assert!(f.reached_stop);
    assert_eq!(f.taken, run.stop.stop_input_position - 10);
    assert_eq!(r.finish().unwrap().verdict, Verdict::Identical);
    // feed_f32 with the helper's samples equals feed.
    let mut r = ReRender::new(plan()).unwrap();
    let _ = r.feed_f32(&rc::samples(&raw)).unwrap();
    assert_eq!(r.finish().unwrap().verdict, Verdict::Identical);
    // RunTooLarge: the stop position edited far out; a digest-only session is built.
    let text = String::from_utf8(run.record.clone()).unwrap();
    let key = "run.stop_input_position = ";
    let i = text.find(key).unwrap() + key.len();
    let j = i + text[i..].find('\n').unwrap();
    let far = resign(format!("{}{}{}", &text[..i], 1u64 << 40, &text[j..]).into_bytes());
    let p = Plan::from_record(&far).unwrap();
    assert!(matches!(
        ReRender::new(p.clone()).unwrap_err(),
        Error::RunTooLarge { .. }
    ));
    let mut digest_only = p;
    digest_only.set_collect_run(false);
    assert!(ReRender::new(digest_only).is_ok());
    // A digest-only session has no record.
    let mut q = plan();
    q.set_collect_run(false);
    let mut r = ReRender::new(q).unwrap();
    let _ = r.feed(&raw).unwrap();
    assert_eq!(r.finish().unwrap().digest, Some(run.digest));
    assert_eq!(r.record_len().unwrap_err(), Error::RunNotCollected);
    assert!(r.export_len().unwrap() > 0);
}

/// AS2 (SC-002, release only): a 60-minute recording at 48 kHz re-renders `Identical`, its feed
/// within 1.5× of the record path's own cost and at least 20× real time; the input generated in
/// blocks, never one vector.
#[test]
#[ignore = "release only: an hour at 48 kHz"]
fn as2_rerender_hour() {
    use std::time::Instant;
    let rate = 48_000u32;
    let block = 4800usize;
    let blocks = 3600 * rate as usize / block;
    let input_block = |k: usize, out: &mut Vec<i32>| {
        out.clear();
        let t0 = k * block;
        for i in 0..block {
            let v = rr_dr60_harness::stimulus::tone_sample(440.0, 0.1, f64::from(rate), t0 + i)
                * if (t0 + i) / rate as usize % 10 < 7 {
                    1.0
                } else {
                    0.0
                };
            out.push((v * 8_388_608.0) as i32);
        }
    };
    let s = Settings::new(rate);
    let mut p = Pipeline::new(s).unwrap();
    p.start_recording().unwrap();
    let mut codes = Vec::with_capacity(block);
    let (mut events, mut changes) = (Vec::new(), Vec::new());
    let mut ev = vec![rr_dr60::VasEvent::default(); p.max_record_events(block)];
    let mut ch = vec![rr_dr60::ChangeEntry::default(); rr_dr60::changes::MAX_CHANGES_PER_CALL];
    let t = Instant::now();
    let mut stopped = None;
    for k in 0..blocks {
        input_block(k, &mut codes);
        let info = p
            .record_with_changes(&rc::samples(&codes), &mut ev, &mut ch)
            .unwrap();
        events.extend_from_slice(&ev[..info.events]);
        changes.extend_from_slice(&ch[..info.changes]);
        if info.stopped.is_some() {
            stopped = Some(rr_dr60::record::StopFields::from(&info));
            break;
        }
    }
    let record_time = t.elapsed();
    let stop = match stopped {
        Some(s) => s,
        None => {
            let mut ev = vec![rr_dr60::VasEvent::default(); p.max_record_events(rate as usize)];
            let out = p.stop_recording_with_changes(&mut ev, &mut ch).unwrap();
            events.extend_from_slice(&ev[..out.flush.events]);
            changes.extend_from_slice(&ch[..out.changes]);
            rr_dr60::record::StopFields::from(&out)
        }
    };
    let n = p.memory_status().count;
    let run = rr_dr60::record::RunData {
        events: &events,
        changes: &changes,
        stop,
    };
    let mut record = vec![0u8; p.processing_record_len(n, &run).unwrap()];
    let _ = p.processing_record_write(n, &run, &mut record).unwrap();
    let t = Instant::now();
    let mut r = ReRender::new(Plan::from_record(&record).unwrap()).unwrap();
    let new_time = t.elapsed();
    let t = Instant::now();
    for k in 0..blocks {
        input_block(k, &mut codes);
        let f = r.feed(&codes).unwrap();
        if f.reached_stop || f.self_stopped {
            break;
        }
    }
    let o = r.finish().unwrap();
    let feed_time = t.elapsed();
    let secs = record_time.as_secs_f64();
    let ratio = feed_time.as_secs_f64() / secs;
    println!(
        "hour at 48 kHz: record {secs:.2} s, new {:.3} s, feed {:.2} s, ratio {ratio:.3}, {:.0}x real time",
        new_time.as_secs_f64(),
        feed_time.as_secs_f64(),
        3600.0 / feed_time.as_secs_f64()
    );
    assert_eq!(o.verdict, Verdict::Identical);
    assert!(ratio <= 1.5, "the feed within 1.5x of the record path");
    assert!(
        3600.0 / feed_time.as_secs_f64() >= 20.0,
        "at least 20x real time"
    );
}
