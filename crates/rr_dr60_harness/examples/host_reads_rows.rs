//! Prints the host-read rows as the C smoke program prints them (`smoke --host-reads`; spec 026
//! T012), through the Rust API: the meter fields, the stage manifest's words, for every tap at
//! 48 kHz the run rules, the reports' latency, the drain and each field's reach and lag per call,
//! the slot's band filter's drain, and the wording hits on a fixed set of texts.
//!
//! Run: `cargo run -q -p rr_dr60_harness --example host_reads_rows`

use rr_dr60::components::{ComponentId, InsertionPointId::BeforeLimiter};
use rr_dr60::meters::{FIELDS, MeterCall};
use rr_dr60::{Confidence, Pipeline, Settings, StagePath, StageStatus, Tap, wording};

/// The C tap values in order (the C header's `RR_DR60_TAP_*`).
const TAPS: [Tap; 12] = [
    Tap::AfterRecord,
    Tap::AfterPlayback,
    Tap::AfterAgc,
    Tap::AfterVas,
    Tap::AfterDecoder,
    Tap::RouteOutput,
    Tap::AfterVolumeAmp,
    Tap::AfterSpeaker,
    Tap::AfterEarphone,
    Tap::AfterMicCapsule,
    Tap::AfterMicAmp,
    Tap::AfterInsert,
];

/// The wording texts (the smoke's `WORDING_TEXTS`).
const TEXTS: [&str; 6] = [
    "a ghost here",
    "the band filter",
    "Événement: EVENTS detected",
    "not measured, then measured",
    "an accurate hidden voices",
    "",
];

fn main() {
    for (i, f) in FIELDS.iter().enumerate() {
        println!(
            "field {i} {} {} {} {} {}",
            f.identifier, f.unit, f.kind as u32, f.side as u32, f.sentence
        );
    }
    let statuses = [
        StageStatus::NotModeled,
        StageStatus::Assumed,
        StageStatus::Sourced,
        StageStatus::Checked,
    ];
    for (v, s) in statuses.iter().enumerate() {
        println!("word 0 {v} {}|{}", s.token(), s.label());
    }
    for (v, p) in [StagePath::Record, StagePath::Playback, StagePath::Both]
        .iter()
        .enumerate()
    {
        println!("word 1 {v} {}|{}", p.token(), p.label());
    }
    let confidences = [
        Confidence::NotApplicable,
        Confidence::Low,
        Confidence::Medium,
        Confidence::High,
    ];
    for (v, c) in confidences.iter().enumerate() {
        println!("word 2 {v} {}|{}", c.token(), c.label());
    }
    for (t, &tap) in TAPS.iter().enumerate() {
        let mut s = Settings::new(48_000);
        s.tap = tap;
        let p = Pipeline::new(s).unwrap();
        println!(
            "tap {t} {} {} {} {}",
            u8::from(s.one_pass_vas_runs()),
            u8::from(s.one_pass_codec_runs()),
            p.one_pass_report_latency_samples(),
            p.one_pass_drain_samples()
        );
        for (c, call) in [MeterCall::OnePass, MeterCall::Record, MeterCall::Play]
            .into_iter()
            .enumerate()
        {
            let row: String = (0..FIELDS.len())
                .map(|i| match p.meter_lag_samples(i, call) {
                    Some(l) => {
                        assert!(p.meter_reached(i, call));
                        format!(" {l}")
                    }
                    None => " -".to_string(),
                })
                .collect();
            println!("reach {t} {c}{row}");
        }
    }
    let mut s = Settings::new(48_000);
    s.tap = Tap::AfterInsert;
    let mut p = Pipeline::new(s).unwrap();
    p.set_component_parameter(BeforeLimiter, 1, 20.0).unwrap();
    p.set_component_parameter(BeforeLimiter, 2, 10.0).unwrap();
    p.set_component(BeforeLimiter, ComponentId::BandFilter)
        .unwrap();
    println!("slot {}", p.one_pass_drain_samples());
    for (kind, lists) in [wording::ENGINE_STRINGS, wording::CLAIMS]
        .iter()
        .enumerate()
    {
        for (n, text) in TEXTS.iter().enumerate() {
            match wording::first_hit_in(lists, text) {
                Some(h) => println!("hit {kind} {n} {} {} {}", h.entry, h.start, h.end),
                None => println!("hit {kind} {n} - 0 0"),
            }
        }
    }
}
