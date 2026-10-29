//! Spec 014 User Story 2: the control manifest cannot drift from the stage manifest's controls
//! notes, the C header, the core's setters and its default lines (014 FR-005, FR-007, FR-009,
//! FR-016; SC-003; tasks.md T009). Every check is a function over a table, so each doctored case
//! is a copy of the committed table, or of a committed text, with one change; each asserts the
//! entry it must name. The "missing entry" case runs in the core (`controls::tests`) and the
//! superseded-ID case in `us2_cross_check`.

use rr_dr60::{ControlEntry, ControlValue, controls, manifest};
use rr_dr60_harness::controls_checks::{
    Token, core_sources, defaults, edges, header_text, ids, notes, options_vs_header, setters,
    strings, tokens,
};

fn table() -> Vec<ControlEntry> {
    controls().to_vec()
}

fn index(entries: &[ControlEntry], id: &str) -> usize {
    entries
        .iter()
        .position(|e| e.identifier == id)
        .unwrap_or_else(|| panic!("{id}"))
}

// ---- The committed tree passes -----------------------------------------------------------------

#[test]
fn notes_agree() {
    assert_eq!(notes(controls(), manifest()), Vec::<String>::new());
}

#[test]
fn setters_agree() {
    assert_eq!(
        setters(controls(), &core_sources(), &header_text()),
        Vec::<String>::new()
    );
}

#[test]
fn options_agree_with_the_header() {
    assert_eq!(
        options_vs_header(controls(), &header_text()),
        Vec::<String>::new()
    );
}

#[test]
fn ids_agree_with_the_default_lines() {
    assert_eq!(ids(controls(), &core_sources()), Vec::<String>::new());
}

#[test]
fn tokenizer_reads_the_committed_notes() {
    // 014 research R-05: stage 5 expands to six entries; stage 11 names both live controls;
    // stage 9 names the speed control; option values and other items name nothing.
    let named = |n: usize| -> Vec<String> {
        tokens(manifest().stages()[n - 1].controls, controls())
            .into_iter()
            .filter_map(|t| match t {
                Token::Entry(id) => Some(id),
                _ => None,
            })
            .collect()
    };
    assert_eq!(
        named(5),
        [
            "vas.enabled",
            "vas.mode",
            "vas.sensitivity",
            "vas.threshold_dbfs",
            "vas.hang_ms",
            "vas.onset_ms"
        ]
    );
    assert_eq!(
        named(11),
        [
            "output.volume_amp_enabled",
            "volume",
            "volume",
            "route",
            "route"
        ]
    );
    assert_eq!(named(9), ["speech_codec_enabled", "playback_speed"]);
    assert_eq!(named(12), ["output.speaker_enabled"]);
    assert!(named(6).is_empty());
    assert_eq!(
        tokens("vas.hang; Tap::AfterVas", controls()),
        [
            Token::Unresolved("vas.hang".into()),
            Token::Other("Tap::AfterVas".into())
        ]
    );
}

// ---- SC-003: the six harness doctored cases ----------------------------------------------------

#[test]
fn doctored_moved_bound_is_named() {
    let mut t = table();
    let i = index(&t, "agc.knee_dbfs");
    t[i].value = ControlValue::Continuous {
        min: -40.0,
        max: 3.000_000_2,
        default: -1.0,
    };
    let f = edges(&t);
    assert!(
        f.iter().any(|s| s.starts_with("agc.knee_dbfs:")) && f.len() <= 2,
        "{f:?}"
    );
    let mut t = table();
    let i = index(&t, "vas.sensitivity");
    t[i].value = ControlValue::Integer {
        min: 1,
        max: 6,
        default: 3,
    };
    let f = edges(&t);
    assert!(
        f.iter().all(|s| s.starts_with("vas.sensitivity:")) && !f.is_empty(),
        "{f:?}"
    );
}

#[test]
fn doctored_changed_default_is_named() {
    let mut t = table();
    let i = index(&t, "vas.hang_ms");
    t[i].value = ControlValue::Continuous {
        min: 50.0,
        max: 10_000.0,
        default: 1001.0,
    };
    let f = defaults(&t);
    assert_eq!(f.len(), 1, "{f:?}");
    assert!(f[0].starts_with("vas.hang_ms:"), "{f:?}");
}

#[test]
fn doctored_denylisted_word_is_named() {
    let mut t = table();
    let i = index(&t, "vas.enabled");
    t[i].description = "Switches pausing off when no activity is present.";
    let f = strings(&t);
    assert_eq!(f.len(), 1, "{f:?}");
    assert!(f[0].starts_with("vas.enabled: description"), "{f:?}");
}

#[test]
fn doctored_kind_rule_is_named() {
    let mut t = table();
    let i = index(&t, "volume");
    t[i].sources = &[];
    assert_eq!(
        strings(&t),
        ["volume: a device control must cite S-001 or S-002"]
    );
    let mut t = table();
    let i = index(&t, "agc.knee_dbfs");
    t[i].assumptions = &[];
    assert_eq!(
        strings(&t),
        ["agc.knee_dbfs: a unit condition or model internal must cite an assumption"]
    );
}

#[test]
fn doctored_stage_disagreeing_with_the_notes_is_named() {
    let mut t = table();
    let i = index(&t, "vas.hang_ms");
    t[i].stage = 3;
    let f = notes(&t, manifest());
    assert!(
        f.contains(&"vas.hang_ms: named in stage 5's note but its stage is 3".to_string()),
        "{f:?}"
    );
    assert!(
        f.contains(&"vas.hang_ms: stage 3 but not named in its controls note".to_string()),
        "{f:?}"
    );
}

#[test]
fn doctored_setter_without_entry_is_named() {
    let mut sources = core_sources();
    sources.push((
        "doctored.rs".into(),
        "impl Pipeline {\n    pub fn set_battery(&mut self, v: f32) {}\n}\n".into(),
    ));
    let f = setters(controls(), &sources, &header_text());
    assert_eq!(f, ["doctored.rs: setter set_battery has no live entry"]);
    let header = header_text() + "\nRrDr60Status rr_dr60_set_battery(RrDr60 *e, float v);\n";
    let f = setters(controls(), &core_sources(), &header);
    assert_eq!(f, ["rr_dr60.h: rr_dr60_set_battery has no live entry"]);
}

#[test]
fn doctored_ids_and_options_are_named() {
    let mut t = table();
    let i = index(&t, "agc.release_ms");
    t[i].assumptions = &["A-034"];
    assert_eq!(
        ids(&t, &core_sources()),
        ["agc.release_ms: cites [] [\"A-034\"] but its default's comment cites [] [\"A-035\"]"]
    );
    let header = header_text().replace("RR_DR60_TAP_AFTER_AGC = 2,", "RR_DR60_TAP_AFTER_AGC = 9,");
    let f = options_vs_header(controls(), &header);
    assert_eq!(f.len(), 1, "{f:?}");
    assert!(f[0].starts_with("tap: option after_agc=2"), "{f:?}");
}
