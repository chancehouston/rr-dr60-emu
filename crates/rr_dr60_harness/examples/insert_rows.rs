//! Prints the slot's rows as the C smoke program prints them (`smoke --insert`; spec 024
//! research R-09): a ±0.125 square (48 up, 48 down; exactly representable, spec 006's lesson) at
//! 48 kHz for 1 s through the default engine with the codec off, read at the tap after the slot;
//! the band-pass at 800 Hz set while idle, a frequency move to 2400 Hz scheduled on the
//! one-pass at input 12000, 1024-sample blocks: the entries, the output digest (FNV-1a over the
//! sample bits), the store's frequency; then the constant data.
//!
//! Run: `cargo run -q -p rr_dr60_harness --example insert_rows`

use rr_dr60::changes::MAX_CHANGES_PER_CALL;
use rr_dr60::components::{self, ComponentId, InsertionPointId::BeforeLimiter};
use rr_dr60::{ChangeEntry, ControlValue, Pipeline, Settings, Stream, Tap};
use rr_dr60_harness::changes_checks::digest;

fn main() {
    let rate = 48_000u32;
    let x: Vec<f32> = (0..rate as usize)
        .map(|i| if i % 96 < 48 { 0.125 } else { -0.125 })
        .collect();
    let mut s = Settings::new(rate);
    s.speech_codec_enabled = false;
    s.tap = Tap::AfterInsert;
    let mut p = Pipeline::new(s).expect("settings");
    p.set_component_parameter(BeforeLimiter, 0, 2.0)
        .expect("type");
    p.set_component_parameter(BeforeLimiter, 1, 800.0)
        .expect("frequency");
    p.set_component(BeforeLimiter, ComponentId::BandFilter)
        .expect("idle");
    p.schedule_change(Stream::OnePass, 12_000, 33, 2400.0, 3)
        .expect("schedule");
    let mut log = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
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
    let v = p
        .component_parameter_value(BeforeLimiter, 1)
        .expect("value");
    println!("frequency {:08x}", v.to_bits());
    let pt = &components::insertion_points()[0];
    println!(
        "point {} {} {} {}",
        pt.id as u32, pt.identifier, pt.paths, pt.control_index
    );
    let c = &components::components()[0];
    println!(
        "component {} {} {} {} {}",
        c.id as u32,
        c.identifier,
        c.latency_samples,
        c.parameter_offset,
        c.parameters.len()
    );
    for q in c.parameters {
        let (ty, min, max, def) = match q.value {
            ControlValue::Choice { options, default } => (
                3u32, // RR_DR60_CONTROL_VALUE_TYPE_CHOICE
                0.0f32,
                options.last().map_or(0.0, |o| o.value as f32),
                default.unwrap_or(0) as f32,
            ),
            ControlValue::Continuous { min, max, default } => (2u32, min, max, default),
            _ => unreachable!("the band filter's parameters"),
        };
        println!(
            "parameter {} {} {:08x} {:08x} {:08x} {} {}",
            q.identifier,
            ty,
            min.to_bits(),
            max.to_bits(),
            def.to_bits(),
            q.scale as u32,
            q.control_index
        );
    }
    println!(
        "ceiling 6000 {}",
        components::band_filter_frequency_ceiling_hz(6000)
    );
}
