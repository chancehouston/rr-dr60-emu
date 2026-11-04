//! Prints the one-pass map's rows as the C smoke program prints them (`smoke --onepass-map`;
//! spec 028 T007), through the Rust API: at 6 and 48 kHz, three taps, the coder off and on, a
//! fixed report list, each heard point and the alignment at fixed output positions.
//!
//! Run: `cargo run -q -p rr_dr60_harness --example onepass_map_rows`

use rr_dr60::{Pipeline, Settings, Tap, VasEvent};

fn main() {
    let taps = [
        (1u32, Tap::AfterPlayback),
        (5, Tap::RouteOutput),
        (3, Tap::AfterVas),
    ];
    let ev = [VasEvent::new(20_000, 3_000), VasEvent::new(80_000, 1_500)];
    let pos = [
        0u64, 1, 5000, 21000, 21500, 22000, 22500, 25000, 80000, 82000, 83000, 200000,
    ];
    for rate in [6000u32, 48_000] {
        for (t, tap) in taps {
            for c in [false, true] {
                let mut s = Settings::new(rate);
                s.tap = tap;
                s.speech_codec_enabled = c;
                let p = Pipeline::new(s).unwrap();
                for (i, e) in ev.iter().enumerate() {
                    println!(
                        "heard {rate} {t} {} {i} {}",
                        u8::from(c),
                        p.one_pass_splice_output_position(e)
                    );
                }
                for o in pos {
                    let a = p.one_pass_input_at_output(o, &ev);
                    println!(
                        "map {rate} {t} {} {o} {} {} {} {}",
                        u8::from(c),
                        a.centre,
                        a.first,
                        a.last,
                        u32::from(a.clamped)
                    );
                }
            }
        }
    }
}
