//! Prints the battery rows of one run, as the C smoke program prints them (`smoke --battery`;
//! spec 018 FR-011, SC-004, research R-09): a ±1.0 square (24 up, 24 down; exactly
//! representable, spec 006's lesson for a C-versus-Rust stimulus) for 1.5 s at 48 kHz through
//! the default engine with the codec bypassed, the speaker route at position 1 (far over the
//! ceiling at every value), at 3.0, 2.6 and 2.2 V set by the plain setter before each run,
//! processed in 1024-sample blocks: after each block the entries and the battery row (the
//! amplifier clip count and the ceiling factor's bits), then the output digest (FNV-1a over the
//! sample bits); then the law at the six points, the seven trajectory rows and the NiMH figure
//! as hex bits.
//!
//! Run: `cargo run -q -p rr_dr60_harness --example battery_rows`

use rr_dr60::battery::{ALKALINE_TRAJECTORY, NIMH_PAIR_VOLTS, ceiling_db, ceiling_factor};
use rr_dr60::changes::MAX_CHANGES_PER_CALL;
use rr_dr60::{ChangeEntry, Pipeline, Route, Settings};
use rr_dr60_harness::changes_checks::digest;
use rr_dr60_harness::meters_checks::battery_row;

fn main() {
    let rate = 48_000u32;
    let n = rate as usize * 3 / 2;
    let x: Vec<f32> = (0..n)
        .map(|i| if i % 48 < 24 { 1.0 } else { -1.0 })
        .collect();
    let mut s = Settings::new(rate);
    s.speech_codec_enabled = false;
    let mut log = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
    for volts in [3.0f32, 2.6, 2.2] {
        println!("battery {volts}");
        let mut p = Pipeline::new(s).expect("settings");
        p.set_route(Route::Speaker);
        p.set_volume(1.0).expect("position");
        p.set_battery_volts(volts).expect("battery");
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
            println!("{}", battery_row(&p.meters()));
            out.extend_from_slice(&y[..info.produced]);
        }
        println!("digest {:016x}", digest(&out));
    }
    println!("law");
    for v in [3.0f32, 2.8, 2.6, 2.4, 2.2, NIMH_PAIR_VOLTS] {
        println!(
            "{:08x} {:08x} {:08x}",
            v.to_bits(),
            ceiling_factor(v).to_bits(),
            ceiling_db(v).to_bits()
        );
    }
    println!("trajectory");
    for (h, v) in ALKALINE_TRAJECTORY {
        println!(
            "{:08x} {:08x} {:08x}",
            h.to_bits(),
            v.to_bits(),
            ceiling_factor(v).to_bits()
        );
    }
}
