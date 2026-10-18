//! Spec 007 User Story 1: the stage manifest read from Rust, with no engine and no register
//! file (007 FR-001 – FR-008, FR-012, FR-016; SC-006; tasks.md T003).
//!
//! Everything here is checked against the spec's own statements of the content (spec 007 ›
//! Key Entities; data-model.md › The fifteen entries), not against the registers: that is
//! User Story 2's job (`us2_cross_check.rs`).

use rr_dr60::{Confidence, StageEntry, StagePath, StageStatus, manifest};

/// data-model.md › The fifteen entries: the identifiers in signal-chain order.
const IDENTIFIERS: [&str; 15] = [
    "mic_capsule",
    "mic_amp",
    "limiter",
    "codec_input",
    "vas",
    "noise_reduction",
    "encoder",
    "memory",
    "decoder",
    "codec_output",
    "volume_amp",
    "speaker",
    "earphone",
    "clock",
    "power",
];

/// 007 FR-012: the basis sentence while no stage is checked against a real unit.
const BASIS: &str = "Modeled on the RR-DR60 owner's and service manuals and the codec family datasheets; no stage has been checked against a real unit.";

fn modeled(e: &StageEntry) -> bool {
    e.status != StageStatus::NotModeled
}

#[test]
fn fifteen_entries_in_signal_chain_order() {
    // 007 FR-001, FR-002: fifteen entries numbered 1..=15, index = number - 1.
    let m = manifest();
    assert_eq!(m.stages().len(), 15);
    for (i, e) in m.stages().iter().enumerate() {
        assert_eq!(
            usize::from(e.number),
            i + 1,
            "entry {i} has number {}",
            e.number
        );
        assert_eq!(
            e.identifier, IDENTIFIERS[i],
            "entry {} identifier",
            e.number
        );
        assert!(
            !e.name.is_empty() && !e.description.is_empty(),
            "stage {}",
            e.number
        );
    }
}

#[test]
fn header_figures() {
    // SC-006; 007 FR-003: the header's figures today.
    let h = manifest().header();
    assert_eq!(h.stage_count, 15);
    assert_eq!(h.device_rate_hz, 6000); // A-026
    assert_eq!(h.library_version, rr_dr60::VERSION);
    assert_eq!(h.counts.not_modeled, 3); // spec 012 US4 AS1: stages 1 – 2 modeled
    assert_eq!(h.counts.assumed, 12);
    assert_eq!(h.counts.sourced, 0);
    assert_eq!(h.counts.checked, 0);
    assert_eq!(
        h.counts.not_modeled + h.counts.assumed + h.counts.sourced + h.counts.checked,
        h.stage_count
    );
    assert_eq!(h.basis, BASIS);
    for link in [h.evidence_link, h.assumptions_link, h.sources_link] {
        assert!(
            link.starts_with("https://github.com/chancehouston/rr-dr60-emu/"),
            "link {link}"
        );
    }
}

#[test]
fn literal_statuses() {
    // The register-free reading of US1 scenario 5 (007 FR-004): stages 1 – 5 and 7 – 13 are
    // modeled on open assumptions (7 and 9 since spec 008; 11 – 13 since spec 009; 1 and 2
    // since spec 012), every other stage (6, 14, 15) is not modeled.
    for e in manifest().stages() {
        let want = match e.number {
            1..=5 | 7..=13 => StageStatus::Assumed,
            _ => StageStatus::NotModeled,
        };
        assert_eq!(e.status, want, "stage {} status", e.number);
    }
}

/// Spec 008 US3 scenario 1 (FR-020): stages 7 and 9 read assumed / low / 0.8.0, their coverage
/// notes name the device's unidentified frame length, look-ahead and codebooks, their controls
/// notes name the bypass control, the decoder tap and the frame-count report fields; stage 8
/// sits at 0.8.0 and its coverage note no longer says the store holds the engine's samples.
#[test]
fn codec_stages_us3_scenario_1() {
    let m = manifest();
    for n in [7u32, 9] {
        let e = m.stage(n).unwrap();
        assert_eq!(e.status, StageStatus::Assumed, "stage {n}");
        assert_eq!(e.confidence, Confidence::Low, "stage {n}");
        // Restated 2026-10-17 by spec 013 (/speckit-analyze F2): stage 9 carries the playback
        // speed control (A-054) at model version 0.13.0; stage 7 stays at 0.8.0.
        let version = if n == 9 { "0.13.0" } else { "0.8.0" };
        assert_eq!(e.model_version, version, "stage {n}");
        assert!(
            !e.coverage.is_empty() && !e.controls.is_empty(),
            "stage {n}"
        );
        assert!(e.controls.contains("speech_codec_enabled"), "stage {n}");
        assert!(e.assumptions.contains(&"A-004"), "stage {n}");
        assert!(e.assumptions.contains(&"A-040"), "stage {n}");
        assert!(e.assumptions.contains(&"A-041"), "stage {n}");
    }
    // Both coverage notes name the three unknowns (US3 string review, T039).
    for n in [7u32, 9] {
        let e = m.stage(n).unwrap();
        for word in ["frame length", "look-ahead", "codebooks"] {
            assert!(
                e.coverage.contains(word),
                "stage {n} coverage lacks {word:?}"
            );
        }
    }
    let enc = m.stage(7).unwrap();
    for item in [
        "RecordInfo.frames",
        "RecordInfo.frames_total",
        "BlockInfo.frames",
    ] {
        assert!(
            enc.controls.contains(item),
            "stage 7 controls lack {item:?}"
        );
    }
    let dec = m.stage(9).unwrap();
    assert!(
        dec.assumptions.contains(&"A-054"),
        "stage 9 cites A-054 (spec 013 FR-011)"
    );
    for item in [
        "Tap::AfterDecoder",
        "PlayInfo.frames",
        "PlayInfo.frame_index",
        "set_playback_speed", // spec 013 FR-011
        "PlayInfo.place",     // spec 013 FR-009
    ] {
        assert!(
            dec.controls.contains(item),
            "stage 9 controls lack {item:?}"
        );
    }
    let mem = m.stage(8).unwrap();
    assert_eq!(mem.model_version, "0.8.0");
    assert!(
        !mem.coverage.contains("samples"),
        "stage 8's coverage note still speaks of the engine's samples"
    );
    let c = m.header().counts;
    assert_eq!(
        (c.not_modeled, c.assumed, c.sourced, c.checked),
        (3, 12, 0, 0) // spec 012 US4 AS1: stages 1 – 2 modeled too
    );
}

#[test]
fn not_modeled_invariants() {
    // 007 FR-006, FR-008, FR-002 (coverage note): a stage that is not modeled carries no
    // confidence, no model version and no coverage note.
    for e in manifest().stages().iter().filter(|e| !modeled(e)) {
        assert_eq!(
            e.confidence,
            Confidence::NotApplicable,
            "stage {}",
            e.number
        );
        assert!(
            e.model_version.is_empty(),
            "stage {} model version",
            e.number
        );
        assert!(e.coverage.is_empty(), "stage {} coverage", e.number);
    }
}

#[test]
fn modeled_invariants() {
    // 007 FR-006, FR-007, FR-008, FR-013 (g): a modeled stage has a model version, a controls
    // note and a confidence level.
    for e in manifest().stages().iter().filter(|e| modeled(e)) {
        assert!(
            !e.model_version.is_empty(),
            "stage {} model version",
            e.number
        );
        assert!(!e.controls.is_empty(), "stage {} controls", e.number);
        assert_ne!(
            e.confidence,
            Confidence::NotApplicable,
            "stage {}",
            e.number
        );
    }
}

#[test]
fn paths() {
    // 007 FR-002: record 1–7, playback 9–13, both for the memory, the clock and the power.
    for e in manifest().stages() {
        let want = match e.number {
            1..=7 => StagePath::Record,
            9..=13 => StagePath::Playback,
            _ => StagePath::Both,
        };
        assert_eq!(e.path, want, "stage {} path", e.number);
    }
}

#[test]
fn lookup() {
    // 007 FR-016: lookup by stage number; nothing for 0 or above 15.
    let m = manifest();
    assert!(m.stage(0).is_none());
    assert!(m.stage(16).is_none());
    assert!(m.stage(u32::MAX).is_none());
    for n in 1..=15u32 {
        assert_eq!(u32::from(m.stage(n).unwrap().number), n);
    }
}

#[test]
fn identical_across_reads() {
    // 007 FR-016, SC-004: the same data, at the same address, every time.
    let a = manifest();
    let b = manifest();
    assert!(core::ptr::eq(a, b));
    assert_eq!(a, b);
    assert_eq!(*a, *b);
}

#[test]
fn wording_is_clean() {
    // US1 scenario 4; 007 FR-002, FR-003, FR-011; SC-003, register-free (the renderer module
    // reads no file for this). FR-010, sourced properties only, is judged by review (T029).
    let f = rr_dr60_harness::manifest_checks::wording_findings(manifest());
    for x in &f {
        println!("{x}");
    }
    assert!(f.is_empty(), "{} findings", f.len());
}

#[test]
fn id_lists_ascending_and_well_formed() {
    // 007 FR-002, FR-005: IDs are the registers' strings verbatim, in ascending order.
    fn well_formed(id: &str, prefix: u8) -> bool {
        let b = id.as_bytes();
        b.len() == 5 && b[0] == prefix && b[1] == b'-' && b[2..].iter().all(|c| c.is_ascii_digit())
    }
    for e in manifest().stages() {
        for id in e.sources {
            assert!(well_formed(id, b'S'), "stage {} source {id}", e.number);
        }
        for id in e.assumptions {
            assert!(well_formed(id, b'A'), "stage {} assumption {id}", e.number);
        }
        assert!(
            e.sources.windows(2).all(|w| w[0] < w[1]),
            "stage {} sources",
            e.number
        );
        assert!(
            e.assumptions.windows(2).all(|w| w[0] < w[1]),
            "stage {} assumptions",
            e.number
        );
    }
}

/// Spec 009 US3 scenario 1 (FR-015): stages 11, 12 and 13 read assumed with medium / low / low
/// confidence and model versions 0.11.0 / 0.9.0 / 0.9.0 (stage 11 moved with the volume
/// transition, spec 011 FR-010), their coverage notes name what each leaves out, their
/// controls notes name the route, the volume control, the bypass settings and the taps, and
/// the header counts read 5 not modeled / 10 assumed.
#[test]
fn output_stages_us3_scenario_1() {
    let m = manifest();
    for (n, confidence, model_version) in [
        (11u32, Confidence::Medium, "0.11.0"), // spec 011 FR-010: the volume transition
        (12, Confidence::Low, "0.9.0"),
        (13, Confidence::Low, "0.9.0"),
    ] {
        let e = m.stage(n).unwrap();
        assert_eq!(e.status, StageStatus::Assumed, "stage {n}");
        assert_eq!(e.confidence, confidence, "stage {n}");
        assert_eq!(e.model_version, model_version, "stage {n}");
        assert!(
            !e.coverage.is_empty() && !e.controls.is_empty(),
            "stage {n}"
        );
        assert!(
            e.sources.contains(&"S-002"),
            "stage {n} cites the schematic"
        );
    }
    let amp = m.stage(11).unwrap();
    for id in ["A-031", "A-043", "A-044", "A-045"] {
        assert!(amp.assumptions.contains(&id), "stage 11 cites {id}");
    }
    assert!(
        amp.sources.contains(&"S-006"),
        "stage 11 cites the datasheet"
    );
    // Spec 011 FR-010: the wiper's noise and the knob's speed are not modeled.
    for word in ["knob", "battery", "beep", "wiper", "speed"] {
        assert!(
            amp.coverage.contains(word),
            "stage 11 coverage lacks {word:?}"
        );
    }
    for item in [
        "output.volume_amp_enabled",
        "set_volume",
        "set_route",
        "Tap::AfterVolumeAmp",
        "EngineState.speaker_muted",
    ] {
        assert!(
            amp.controls.contains(item),
            "stage 11 controls lack {item:?}"
        );
    }
    let spk = m.stage(12).unwrap();
    for id in ["A-010", "A-042"] {
        assert!(spk.assumptions.contains(&id), "stage 12 cites {id}");
    }
    // The description states the device (spec 007's convention); the "not modeled here" note
    // says the model is a template and not a capture (009 T028, product review).
    assert!(
        spk.description.contains("2.8 cm"),
        "stage 12 describes the device's speaker"
    );
    assert!(
        !spk.description.contains("template"),
        "the template belongs in the coverage note"
    );
    for word in [
        "template",
        "not a capture",
        "nonlinearity",
        "enclosure",
        "room",
    ] {
        assert!(
            spk.coverage.contains(word),
            "stage 12 coverage lacks {word:?}"
        );
    }
    for item in [
        "output.speaker_enabled",
        "Route::Speaker",
        "Tap::AfterSpeaker",
        "Tap::RouteOutput",
    ] {
        assert!(
            spk.controls.contains(item),
            "stage 12 controls lack {item:?}"
        );
    }
    let ear = m.stage(13).unwrap();
    for id in ["A-012", "A-032", "A-042"] {
        assert!(ear.assumptions.contains(&id), "stage 13 cites {id}");
    }
    for word in ["cut-off", "transistor's gain", "loads"] {
        assert!(
            ear.coverage.contains(word),
            "stage 13 coverage lacks {word:?}"
        );
    }
    for item in [
        "output.earphone_enabled",
        "Route::Earphone",
        "Tap::AfterEarphone",
        "Tap::RouteOutput",
    ] {
        assert!(
            ear.controls.contains(item),
            "stage 13 controls lack {item:?}"
        );
    }
    let c = m.header().counts;
    assert_eq!(
        (c.not_modeled, c.assumed, c.sourced, c.checked),
        (3, 12, 0, 0) // spec 012 US4 AS1
    );
}

/// Spec 012 US4 AS1 (FR-013): stages 1 and 2 read assumed / low / 0.12.0, cite S-002 and their
/// assumptions, name what they do not model, and their controls notes name the switches, the
/// hiss offset and the taps after them.
#[test]
fn mic_stages_us4_scenario_1() {
    let m = manifest();
    for (n, cites, controls) in [
        (
            1u32,
            &["A-005", "A-048", "A-049", "A-050", "A-052"][..],
            &[
                "mic.capsule_enabled",
                "mic.hiss_offset_db",
                "Tap::AfterMicCapsule",
            ][..],
        ),
        (
            2,
            &["A-006", "A-029", "A-046", "A-047", "A-051", "A-053"][..],
            &["mic.amp_enabled", "Tap::AfterMicAmp"][..],
        ),
    ] {
        let e = m.stage(n).unwrap();
        assert_eq!(e.status, StageStatus::Assumed, "stage {n}");
        assert_eq!(e.confidence, Confidence::Low, "stage {n}");
        assert_eq!(e.model_version, "0.12.0", "stage {n}");
        assert!(e.sources.contains(&"S-002"), "stage {n}");
        for a in cites {
            assert!(e.assumptions.contains(a), "stage {n} lacks {a}");
        }
        for c in controls {
            assert!(e.controls.contains(c), "stage {n} controls lack {c:?}");
        }
        assert!(!e.coverage.is_empty(), "stage {n}");
    }
    assert!(m.stage(1).unwrap().description.contains("hiss"));
    assert!(m.stage(2).unwrap().description.contains("clip"));
}
