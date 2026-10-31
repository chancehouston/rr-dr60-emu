//! Prints the bypass rows of one run, as the C smoke program prints them (`smoke --bypass`;
//! spec 021 FR-011, tasks.md T019): a ±0.125 square (24 up, 24 down; exactly representable,
//! spec 006's lesson for a C-versus-Rust stimulus) for 1.5 s at 48 kHz through the default
//! engine with the codec bypassed on the one-pass stream, the speaker route at the reference
//! position, stage 12 scheduled off at 20000 and on at 40000 and stage 11 off at 60000, in
//! 1024-sample blocks: after each block the entries, then the output digest (FNV-1a over the
//! sample bits); then the level function's four cells as hex bits.
//!
//! Run: `cargo run -q -p rr_dr60_harness --example bypass_rows`

use rr_dr60::changes::MAX_CHANGES_PER_CALL;
use rr_dr60::output::{VOLUME_REFERENCE, stage11_bypass_level_db};
use rr_dr60::{ChangeEntry, Pipeline, Route, Settings, Stream};
use rr_dr60_harness::changes_checks::digest;

fn main() {
    let rate = 48_000u32;
    let n = rate as usize * 3 / 2;
    let x: Vec<f32> = (0..n)
        .map(|i| if i % 48 < 24 { 0.125 } else { -0.125 })
        .collect();
    let mut s = Settings::new(rate);
    s.speech_codec_enabled = false;
    let mut log = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
    println!("bypass");
    let mut p = Pipeline::new(s).expect("settings");
    p.set_route(Route::Speaker);
    p.schedule_change(Stream::OnePass, 20_000, 23, 0.0, 1)
        .expect("schedule");
    p.schedule_change(Stream::OnePass, 40_000, 23, 1.0, 2)
        .expect("schedule");
    p.schedule_change(Stream::OnePass, 60_000, 22, 0.0, 3)
        .expect("schedule");
    let mut out = Vec::new();
    for chunk in x.chunks(1024) {
        let mut y = vec![0.0f32; chunk.len()];
        let info = p
            .process_with_events_and_changes(chunk, &mut y, &mut [], &mut log)
            .expect("call");
        for e in &log[..info.changes] {
            println!(
                "{} {} {:016x} {:016x} {}",
                e.position,
                e.control,
                e.before.to_bits(),
                e.after.to_bits(),
                e.tag
            );
        }
        out.extend_from_slice(&y[..info.produced]);
    }
    println!("digest {:016x}", digest(&out));
    println!("level");
    for (route, pos) in [
        (Route::Earphone, VOLUME_REFERENCE),
        (Route::Earphone, 1.0),
        (Route::Speaker, VOLUME_REFERENCE),
        (Route::Speaker, 1.0),
    ] {
        println!(
            "{:08x} {:08x}",
            pos.to_bits(),
            stage11_bypass_level_db(route, pos).to_bits()
        );
    }
}
