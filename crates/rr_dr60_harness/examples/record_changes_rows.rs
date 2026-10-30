//! Prints the change entries, the final input position and the digests of one scheduled
//! recording and one-pass with record-side changes, as the C smoke program prints them
//! (`smoke --record-changes`; spec 017 FR-010, SC-003): a ±0.125 square at 48 kHz through the
//! default engine with the speech codec bypassed; a sensitivity change at input 700 and a hiss
//! change at 1500 on the record stream in 1024-sample blocks, the stop's entries and the
//! recording's exported bytes; then the one-pass with a knee change at input 500 and a route
//! change at output 900.
//!
//! Run: `cargo run -q -p rr_dr60_harness --example record_changes_rows`

use rr_dr60::changes::MAX_CHANGES_PER_CALL;
use rr_dr60::{ChangeEntry, Pipeline, Route, Settings, Stream};
use rr_dr60_harness::changes_checks::{self as cc, Change, digest};
use rr_dr60_harness::record_checks::{self as rc, Value};
use rr_dr60_harness::split_checks;

fn print_entry(e: &ChangeEntry) {
    println!(
        "{} {} {:016x} {:016x} {}",
        e.position,
        e.control,
        e.before.to_bits(),
        e.after.to_bits(),
        e.tag
    );
}

fn bytes_digest(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn main() {
    let rate = 48_000u32;
    let n = rate as usize / 2;
    let x: Vec<f32> = (0..n)
        .map(|i| if i % 48 < 24 { 0.125 } else { -0.125 })
        .collect();
    let mut s = Settings::new(rate);
    s.speech_codec_enabled = false;
    let mut p = Pipeline::new(s).expect("settings");
    cc::schedule_all(
        &mut p,
        &[
            rc::change(Stream::Record, 700, Value::Sensitivity(5), 1),
            rc::change(Stream::Record, 1500, Value::Hiss(6.0), 2),
        ],
    );
    let mut log = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
    p.start_recording().expect("start");
    println!("record");
    for chunk in x.chunks(1024) {
        let info = p
            .record_with_changes(chunk, &mut [], &mut log)
            .expect("record");
        for e in &log[..info.changes] {
            print_entry(e);
        }
    }
    let stop = p
        .stop_recording_with_changes(&mut [], &mut log)
        .expect("stop");
    for e in &log[..stop.changes] {
        print_entry(e);
    }
    println!("final {}", stop.input_samples_total);
    println!(
        "digest {:016x}",
        bytes_digest(&split_checks::export_bytes(&p, 1))
    );
    cc::schedule_all(
        &mut p,
        &[
            rc::change(Stream::OnePass, 500, Value::Knee(-5.0), 3),
            Change::route(Stream::OnePass, 900, Route::Earphone, 4),
        ],
    );
    println!("one-pass");
    let mut out = Vec::new();
    for chunk in x.chunks(1024) {
        let mut y = vec![0.0f32; chunk.len()];
        let info = p
            .process_with_events_and_changes(chunk, &mut y, &mut [], &mut log)
            .expect("call");
        for e in &log[..info.changes] {
            print_entry(e);
        }
        out.extend_from_slice(&y[..info.produced]);
    }
    println!("digest {:016x}", digest(&out));
}
