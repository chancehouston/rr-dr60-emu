//! Prints the re-render rows as the C smoke program prints them (`smoke --rerender`; spec 027
//! T010): a 1.2 s ±0.125 square at 48 kHz with a knee change at input 10000, recorded and its
//! record written, then re-rendered faithfully and with the knee at −30 dBFS.
//!
//! Run: `cargo run -q -p rr_dr60_harness --example rerender_rows`

use rr_dr60::changes::{ChangeEntry, MAX_CHANGES_PER_CALL, Stream};
use rr_dr60::record::{RunData, StopFields};
use rr_dr60::rerender::{Plan, Verdict, re_render};
use rr_dr60::{Pipeline, Settings, StopResult, VasEvent};

fn row(record: &[u8], codes: &[i32], knee: bool, name: &str) {
    let mut plan = Plan::from_record(record).unwrap();
    if knee {
        let mut s = *plan.settings();
        s.agc.knee_dbfs = -30.0;
        plan.set_settings(s).unwrap();
    }
    let r = re_render(plan, codes).unwrap();
    let o = r.outcome().unwrap();
    let mut b = vec![0u8; r.record_len().unwrap()];
    let _ = r.record_write(&mut b).unwrap();
    let verdict = match o.verdict {
        Verdict::Identical => 0,
        Verdict::Differs => 1,
        _ => 2,
    };
    println!(
        "rerender {name} verdict {verdict} matches {} edited {} rows {} export {} record_same {} digest {:016x}",
        u8::from(o.matches_original),
        u8::from(o.edited),
        o.rows_landed,
        r.export_len().unwrap(),
        i32::from(b == record),
        o.digest.unwrap()
    );
}

fn main() {
    let rate = 48_000;
    let codes: Vec<i32> = (0..57_600)
        .map(|i| if i % 48 < 24 { 1_048_576 } else { -1_048_576 })
        .collect();
    let x: Vec<f32> = codes.iter().map(|&c| c as f32 / 8_388_608.0).collect();
    let mut p = Pipeline::new(Settings::new(rate)).unwrap();
    p.schedule_change(Stream::Record, 10_000, 10, -30.0, 1)
        .unwrap();
    p.start_recording().unwrap();
    let (mut events, mut changes) = (Vec::new(), Vec::new());
    let mut ev = vec![VasEvent::default(); 256];
    let mut ch = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
    for block in x.chunks(1024) {
        let info = p.record_with_changes(block, &mut ev, &mut ch).unwrap();
        events.extend_from_slice(&ev[..info.events]);
        changes.extend_from_slice(&ch[..info.changes]);
    }
    let out = p.stop_recording_with_changes(&mut ev, &mut ch).unwrap();
    events.extend_from_slice(&ev[..out.flush.events]);
    changes.extend_from_slice(&ch[..out.changes]);
    let StopResult::Kept { number, .. } = out.result else {
        panic!("kept")
    };
    let run = RunData {
        events: &events,
        changes: &changes,
        stop: StopFields::from(&out),
    };
    let mut record = vec![0u8; p.processing_record_len(number, &run).unwrap()];
    let _ = p
        .processing_record_write(number, &run, &mut record)
        .unwrap();
    row(&record, &codes, false, "faithful");
    row(&record, &codes, true, "knee");
}
