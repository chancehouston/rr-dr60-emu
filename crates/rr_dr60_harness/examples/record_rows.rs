//! Prints the processing-record rows of one run, as the C smoke program prints them (`smoke
//! --record`; spec 020 contracts/api.md): a ±0.125 square (24 up, 24 down) for 0.5 s, 0.4 s of
//! silence (the gate pauses after its 100 ms hang) and 0.3 s of square at 48 kHz through the
//! default engine with the codec bypassed, a knee change to −30 dBFS scheduled at input 10000
//! and a hiss change of +3 dB at 30000; the run collected from every call and the stop; then
//! the record's text verbatim, the read-out's flags and counts, whether the rewrite is
//! byte-identical, the digest check, map (a) of kept sample 0, map (b) of input 24000, map (c)
//! at output 10, and the feed-point rule on three codes.
//!
//! Run: `cargo run -q -p rr_dr60_harness --example record_rows`

use std::io::Write;

use rr_dr60::changes::MAX_CHANGES_PER_CALL;
use rr_dr60::record::{self, RecordReadout, RunData, StopFields};
use rr_dr60::timeline::{self, KeptOrRemoved};
use rr_dr60::{ChangeEntry, Pipeline, Settings, Stream, VasEvent};

fn main() {
    let rate = 48_000u32;
    let n = rate as usize * 12 / 10;
    let x: Vec<f32> = (0..n)
        .map(|i| {
            let on = i < rate as usize / 2 || i >= rate as usize * 9 / 10;
            if on {
                if i % 48 < 24 { 0.125 } else { -0.125 }
            } else {
                0.0
            }
        })
        .collect();
    let knee = rr_dr60::controls()
        .iter()
        .position(|e| e.identifier == "agc.knee_dbfs")
        .unwrap() as u32;
    let hiss = rr_dr60::controls()
        .iter()
        .position(|e| e.identifier == "mic.hiss_offset_db")
        .unwrap() as u32;
    let mut s = Settings::new(rate);
    s.speech_codec_enabled = false;
    s.vas.hang_ms = 100.0;
    let mut p = Pipeline::new(s).expect("settings");
    p.schedule_change(Stream::Record, 10_000, knee, -30.0, 7)
        .expect("schedule");
    p.schedule_change(Stream::Record, 30_000, hiss, 3.0, 8)
        .expect("schedule");
    p.start_recording().expect("start");
    let (mut events, mut changes) = (Vec::new(), Vec::new());
    let mut ev = vec![VasEvent::default(); 64];
    let mut ch = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
    for chunk in x.chunks(1024) {
        let info = p
            .record_with_changes(chunk, &mut ev, &mut ch)
            .expect("record");
        assert!(info.events <= 64 && info.stopped.is_none());
        events.extend_from_slice(&ev[..info.events]);
        changes.extend_from_slice(&ch[..info.changes]);
    }
    let out = p
        .stop_recording_with_changes(&mut ev, &mut ch)
        .expect("stop");
    assert!(StopFields::kept(&out) && out.flush.events <= 64);
    events.extend_from_slice(&ev[..out.flush.events]);
    changes.extend_from_slice(&ch[..out.changes]);
    let number = p.memory_status().count;
    let run = RunData {
        events: &events,
        changes: &changes,
        stop: StopFields::from(&out),
    };
    let len = p.processing_record_len(number, &run).expect("len");
    let mut rec = vec![0u8; len];
    assert_eq!(
        p.processing_record_write(number, &run, &mut rec)
            .expect("write"),
        len
    );
    println!("record {len}");
    std::io::stdout().write_all(&rec).expect("stdout");
    let mut r = RecordReadout::default();
    p.processing_record_read(&rec, &mut r).expect("read");
    println!(
        "flags {} unknown {} events {} changes {} counter {}",
        r.flags, r.unknown_setting_count, r.event_count, r.change_count, r.recording_counter
    );
    let rows: Vec<_> = (0..r.event_count)
        .map(|i| record::record_event(&rec, i).unwrap())
        .collect();
    let mut again = vec![0u8; record::len(&r, &rows, &changes)];
    record::write(&r, &rows, &changes, &mut again).expect("rewrite");
    println!("rewrite_identical {}", u8::from(again == rec));
    let matches = p.recording_digest_matches(number, &mut r).expect("digest");
    println!(
        "matches {} digest {:016x}",
        u8::from(matches),
        p.recording_digest(number).unwrap()
    );
    let a = timeline::kept_to_input(0, &r.settings, &events, &run.stop).expect("rate");
    println!(
        "kept_to_input 0: {} {} {} {}",
        a.centre,
        a.first,
        a.last,
        u8::from(a.clamped)
    );
    let (kept, removed) = match timeline::input_to_kept(24_000, &r.settings, &events).expect("rate")
    {
        KeptOrRemoved::Kept(k) => (k, u64::MAX),
        KeptOrRemoved::Removed { event_index } => (0, event_index),
    };
    println!("input_to_kept 24000: {kept} {removed}");
    let ps = timeline::place_at(10, rate, 3, 1500, &[]).expect("level");
    println!(
        "place_at 10: {} {} {} {} {}",
        ps.place_fifths,
        ps.level,
        u8::from(ps.fading),
        u8::from(ps.latched),
        ps.old_place_fifths
    );
    let codes = [-8_388_608i32, 0, 8_388_607];
    let mut conv = [0.0f32; 3];
    rr_dr60::input::samples_from_i24(&codes, &mut conv);
    for (c, v) in codes.iter().zip(&conv) {
        println!("i24 {c} {:08x}", v.to_bits());
    }
}
