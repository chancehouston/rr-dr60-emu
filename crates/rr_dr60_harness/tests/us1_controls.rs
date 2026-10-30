//! Spec 014 User Story 1: the control manifest read from Rust and bound to the engine's own
//! ranges, defaults, units and timing, with no register file (014 FR-001 – FR-008, FR-012,
//! FR-016; tasks.md T005).

use rr_dr60::{ControlKind, ControlTiming, ControlValue, Pipeline, Settings, controls};
use rr_dr60_harness::controls_checks::{
    defaults, edges, golden_controls_path, render_controls, strings, timing_behavior, units,
};

#[test]
fn twenty_nine_entries_in_order() {
    // 014 FR-001: live controls first, then the `pub <field>:` order of `Settings` and its
    // sub-structures as declared in settings.rs (the compiler does not check pattern order).
    let text = std::fs::read_to_string(
        rr_dr60_harness::manifest_checks::repo_root().join("crates/rr_dr60/src/settings.rs"),
    )
    .expect("read settings.rs");
    let fields = |struct_name: &str| -> Vec<String> {
        let from = text
            .find(&format!("pub struct {struct_name} {{"))
            .unwrap_or_else(|| panic!("{struct_name}"));
        let body = &text[from..];
        let body = &body[..body.find("\n}").expect("struct end")];
        body.lines()
            .filter_map(|l| l.trim().strip_prefix("pub "))
            .filter_map(|l| l.split_once(':').map(|(n, _)| n.to_string()))
            .filter(|n| !n.starts_with("struct"))
            .collect()
    };
    let mut want: Vec<String> = ["route", "volume", "playback_speed"]
        .map(String::from)
        .to_vec();
    for field in fields("Settings") {
        let sub = match field.as_str() {
            "agc" => Some("AgcSettings"),
            "vas" => Some("VasSettings"),
            "memory" => Some("MemorySettings"),
            "output" => Some("OutputSettings"),
            "mic" => Some("MicSettings"),
            _ => None,
        };
        match sub {
            Some(s) => want.extend(fields(s).into_iter().map(|f| format!("{field}.{f}"))),
            None => want.push(field),
        }
    }
    let got: Vec<String> = controls()
        .iter()
        .map(|e| e.identifier.to_string())
        .collect();
    assert_eq!(got, want);
    assert_eq!(got.len(), 29);
}

#[test]
fn bounds_match_the_engine() {
    // 014 FR-005, FR-008.
    assert_eq!(edges(controls()), Vec::<String>::new());
}

#[test]
fn defaults_match_the_engine() {
    // 014 FR-005, at all seven host rates.
    assert_eq!(defaults(controls()), Vec::<String>::new());
}

#[test]
fn units_match_the_engine() {
    // 014 FR-016.
    assert_eq!(units(controls()), Vec::<String>::new());
}

#[test]
fn timing_classes_match_the_engine() {
    // 014 FR-004, FR-016: live, idle, memory-dependent behave as classed.
    assert_eq!(timing_behavior(controls()), Vec::<String>::new());
}

#[test]
fn wording_and_kind_rules() {
    // 014 FR-006 (denylist on names and descriptions), FR-016 (kind rules).
    assert_eq!(strings(controls()), Vec::<String>::new());
}

#[test]
fn literal_kinds_and_timing() {
    // Spec 014 Key Entities, D2 – D4: a register-free reading of the decided classes.
    let kind = |id: &str| rr_dr60::control(id).map(|e| e.kind);
    let timing = |id: &str| rr_dr60::control(id).map(|e| e.timing);
    for id in ["route", "volume", "playback_speed", "vas.sensitivity"] {
        assert_eq!(kind(id), Some(ControlKind::DeviceControl), "{id}");
    }
    assert_eq!(kind("mic.hiss_offset_db"), Some(ControlKind::UnitCondition));
    assert_eq!(kind("seed"), Some(ControlKind::HostParameter));
    assert_eq!(kind("host_rate_hz"), Some(ControlKind::HostParameter));
    assert_eq!(kind("memory.capacity_s"), Some(ControlKind::EmulatorOption));
    assert_eq!(timing("host_rate_hz"), Some(ControlTiming::HostFixed));
    for id in [
        "speech_codec_enabled",
        "memory.capacity_s",
        "memory.max_recordings",
    ] {
        assert_eq!(timing(id), Some(ControlTiming::IdleMemoryDependent), "{id}");
    }
    let live: Vec<&str> = controls()
        .iter()
        .filter(|e| e.timing == ControlTiming::Live)
        .map(|e| e.identifier)
        .collect();
    // Spec 017: the eight record-side values are live too (through the schedule).
    assert_eq!(
        live,
        [
            "route",
            "volume",
            "playback_speed",
            "agc.knee_dbfs",
            "agc.attack_ms",
            "agc.release_ms",
            "vas.sensitivity",
            "vas.threshold_dbfs",
            "vas.hang_ms",
            "vas.onset_ms",
            "mic.hiss_offset_db",
        ]
    );
    // Spec 017 FR-010: the eleven live entries are the three output-clock controls and the
    // eight input-clock values (`clock_of` over all 29).
    for (i, e) in controls().iter().enumerate() {
        let clock = rr_dr60::changes::clock_of(i as u32).expect("control");
        let want = if e.paths.contains(rr_dr60::ControlPaths::RECORD) {
            rr_dr60::Clock::Input
        } else {
            rr_dr60::Clock::Output
        };
        assert_eq!(clock, want, "{}", e.identifier);
        if rr_dr60_harness::controls_checks::is_record_side(e.identifier) {
            assert_eq!(clock, rr_dr60::Clock::Input, "{}", e.identifier);
            assert_eq!(e.timing, ControlTiming::Live, "{}", e.identifier);
        }
    }
    assert!(matches!(
        rr_dr60::control("host_rate_hz").map(|e| e.value),
        Some(ControlValue::Choice { default: None, .. })
    ));
}

#[test]
fn read_on_another_thread_while_recording() {
    // 014 US1 AS4: identical reads while an engine records on another thread.
    let before = render_controls(controls());
    let worker = std::thread::spawn(|| {
        let mut p = Pipeline::new(Settings::new(48_000)).expect("pipeline");
        p.start_recording().expect("record");
        let input = vec![0.1_f32; 48_000];
        let mut events = vec![rr_dr60::VasEvent::default(); p.max_record_events(input.len())];
        for _ in 0..4 {
            let _ = p.record(&input, &mut events).expect("record");
        }
    });
    let mut reads = Vec::new();
    while !worker.is_finished() {
        reads.push(render_controls(controls()));
    }
    worker.join().expect("worker");
    assert!(!reads.is_empty());
    assert!(reads.iter().all(|r| *r == before));
}

#[test]
fn canonical() {
    // 014 FR-012: the canonical text equals the committed golden. Bless an intended change
    // with RR_DR60_BLESS=control_manifest (a CHANGELOG entry).
    let text = render_controls(controls());
    let path = golden_controls_path();
    if std::env::var("RR_DR60_BLESS").as_deref() == Ok("control_manifest") {
        std::fs::write(&path, &text).expect("write golden");
    }
    let golden = std::fs::read_to_string(&path).expect("read golden (bless first)");
    assert_eq!(
        text, golden,
        "the control manifest's canonical text changed"
    );
}
