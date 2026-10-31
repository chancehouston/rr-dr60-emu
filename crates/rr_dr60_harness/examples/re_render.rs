//! The re-render recipe (spec 020 D6, D12; research R-06): builds an engine from a processing
//! record, fills its memory by import to the record's remaining capacity, schedules the record's
//! entries on the record stream (refilling past 64 pending), feeds the kept microphone audio
//! through the feed-point rule up to the stop position, stops the same way, and compares the
//! stored form's digest with the record's.
//!
//! Run: `cargo run -p rr_dr60_harness --example re_render -- --demo <dir>` writes `record.txt`
//! and `input.i24` (little-endian `int32` samples, 24-bit values) at 48 kHz; then
//! `cargo run -p rr_dr60_harness --example re_render -- <dir>/record.txt <dir>/input.i24`
//! prints both digests.

use rr_dr60_harness::checks::standard;
use rr_dr60_harness::record_contract_checks::{self as rc, Cell, Delta};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("--demo") => {
            let dir = std::path::Path::new(args.get(2).map_or(".", String::as_str));
            std::fs::create_dir_all(dir).expect("the directory");
            let rate = 48_000;
            let raw = rc::raw_stimulus(rate);
            let cell = Cell {
                codec: true,
                changes: true,
                mute: false,
                self_stop: false,
            };
            let (_, run) = rc::record_run(&standard, rate, cell, &raw);
            std::fs::write(dir.join("record.txt"), &run.record).expect("write");
            let bytes: Vec<u8> = raw.iter().flat_map(|v| v.to_le_bytes()).collect();
            std::fs::write(dir.join("input.i24"), bytes).expect("write");
            println!(
                "wrote {} and {} (digest {:016x})",
                dir.join("record.txt").display(),
                dir.join("input.i24").display(),
                run.digest
            );
        }
        Some(record_path) => {
            let raw_path = args.get(2).expect("usage: re_render <record> <raw.i24>");
            let record = std::fs::read(record_path).expect("the record");
            let bytes = std::fs::read(raw_path).expect("the raw audio");
            let raw: Vec<i32> = bytes
                .chunks_exact(4)
                .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            let r = rc::readout(&record);
            let got = rc::re_render(&record, &raw, 4096, Delta::None);
            println!("record  fnv1a64:{:016x}", r.recording_digest);
            println!("render  fnv1a64:{got:016x}");
            println!(
                "{}",
                if got == r.recording_digest {
                    "equal: the record and the recording belong together"
                } else {
                    "differ"
                }
            );
        }
        None => eprintln!("usage: re_render --demo <dir> | re_render <record.txt> <input.i24>"),
    }
}
