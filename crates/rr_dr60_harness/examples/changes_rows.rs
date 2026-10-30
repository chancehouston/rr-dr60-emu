//! Prints the change entries and output digests of one scheduled run, as the C smoke program
//! prints them (`smoke --changes`; spec 015 FR-011, SC-003, research R-10): a ±0.125 square at
//! 48 kHz through the default engine, a volume change at 700 and a route change at 1500 on the
//! one-pass stream in 1024-sample blocks, then the same input recorded and played with a volume
//! change at 500 and a speed change at 900 on the playback stream.
//!
//! Run: `cargo run -q -p rr_dr60_harness --example changes_rows`

use rr_dr60::changes::MAX_CHANGES_PER_CALL;
use rr_dr60::{ChangeEntry, Pipeline, Route, Settings, Stream};
use rr_dr60_harness::changes_checks::{self as cc, Change, digest};

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

fn main() {
    let rate = 48_000u32;
    let n = rate as usize / 2;
    let x: Vec<f32> = (0..n)
        .map(|i| if i % 48 < 24 { 0.125 } else { -0.125 })
        .collect();
    let mut p = Pipeline::new(Settings::new(rate)).expect("settings");
    cc::schedule_all(
        &mut p,
        &[
            Change::volume(Stream::OnePass, 700, 0.3, 1),
            Change::route(Stream::OnePass, 1500, Route::Earphone, 2),
        ],
    );
    let mut log = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
    let mut out = Vec::new();
    println!("one-pass");
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
    p.start_recording().expect("start");
    let _ = p.record(&x, &mut []).expect("record");
    let _ = p.stop_recording(&mut []).expect("stop");
    cc::schedule_all(
        &mut p,
        &[
            Change::volume(Stream::Playback, 500, 0.8, 3),
            Change::speed(900, 5, 4),
        ],
    );
    p.start_playback(1).expect("start");
    println!("playback");
    let mut out = Vec::new();
    let mut y = vec![0.0f32; 1024];
    loop {
        let info = p.play_with_changes(&mut y, &mut log).expect("play");
        for e in &log[..info.changes] {
            print_entry(e);
        }
        out.extend_from_slice(&y[..info.produced]);
        if info.finished {
            break;
        }
    }
    println!("digest {:016x}", digest(&out));
}
