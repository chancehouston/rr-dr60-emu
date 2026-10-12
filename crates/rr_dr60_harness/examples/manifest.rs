//! Prints the stage manifest in its canonical text form (spec 007; data-model.md › Canonical
//! rendering). Self-contained: the README's Rust example is this file verbatim.
//!
//! Run: `cargo run -q -p rr_dr60_harness --example manifest`

use rr_dr60::{Confidence, StagePath, StageStatus, manifest};

fn status(s: StageStatus) -> &'static str {
    match s {
        StageStatus::NotModeled => "not_modeled",
        StageStatus::Assumed => "assumed",
        StageStatus::Sourced => "sourced",
        StageStatus::Checked => "checked",
        _ => "unknown",
    }
}

fn path(p: StagePath) -> &'static str {
    match p {
        StagePath::Record => "record",
        StagePath::Playback => "playback",
        StagePath::Both => "both",
        _ => "unknown",
    }
}

fn confidence(c: Confidence) -> &'static str {
    match c {
        Confidence::NotApplicable => "not_applicable",
        Confidence::Low => "low",
        Confidence::Medium => "medium",
        Confidence::High => "high",
        _ => "unknown",
    }
}

/// Prints `key: value`, or `key:` with nothing after the colon when the value is empty.
fn line(key: &str, value: &str) {
    if value.is_empty() {
        println!("{key}:");
    } else {
        println!("{key}: {value}");
    }
}

fn main() {
    let m = manifest();
    let h = &m.header;
    println!("rr_dr60 manifest");
    println!("library_version: {}", h.library_version);
    println!("device_rate_hz: {}", h.device_rate_hz);
    println!("stage_count: {}", h.stage_count);
    println!(
        "counts: not_modeled={} assumed={} sourced={} checked={}",
        h.counts.not_modeled, h.counts.assumed, h.counts.sourced, h.counts.checked
    );
    println!("basis: {}", h.basis);
    println!("evidence_link: {}", h.evidence_link);
    println!("assumptions_link: {}", h.assumptions_link);
    println!("sources_link: {}", h.sources_link);
    for s in m.stages() {
        println!();
        println!("stage {} {}", s.number, s.identifier);
        println!("name: {}", s.name);
        println!("description: {}", s.description);
        println!("status: {}", status(s.status));
        println!("path: {}", path(s.path));
        println!("confidence: {}", confidence(s.confidence));
        line("sources", &s.sources.join(", "));
        line("assumptions", &s.assumptions.join(", "));
        line("coverage", s.coverage);
        line("controls", s.controls);
        line("model_version", s.model_version);
    }
}
