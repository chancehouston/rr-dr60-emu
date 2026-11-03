//! Spec 026 US4 AS1 [TR: D6, FR-006]: the stage manifest's tokens and labels from one source.

use rr_dr60::{Confidence, StagePath, StageStatus};

#[test]
fn as1_tokens_labels() {
    let s = [
        (StageStatus::NotModeled, "not_modeled", "not modeled"),
        (StageStatus::Assumed, "assumed", "assumed"),
        (StageStatus::Sourced, "sourced", "sourced"),
        (
            StageStatus::Checked,
            "checked",
            "checked against a real unit",
        ),
    ];
    for (v, t, l) in s {
        assert_eq!((v.token(), v.label()), (t, l));
    }
    let p = [
        (StagePath::Record, "record", "record"),
        (StagePath::Playback, "playback", "playback"),
        (StagePath::Both, "both", "record and playback"),
    ];
    for (v, t, l) in p {
        assert_eq!((v.token(), v.label()), (t, l));
    }
    let c = [
        (
            Confidence::NotApplicable,
            "not_applicable",
            "not applicable",
        ),
        (Confidence::Low, "low", "low"),
        (Confidence::Medium, "medium", "medium"),
        (Confidence::High, "high", "high"),
    ];
    for (v, t, l) in c {
        assert_eq!((v.token(), v.label()), (t, l));
    }
    // Lowercase, and "checked" never alone as a label.
    for l in s
        .iter()
        .map(|x| x.2)
        .chain(p.iter().map(|x| x.2))
        .chain(c.iter().map(|x| x.2))
    {
        assert_eq!(l, l.to_lowercase());
        assert_ne!(l, "checked");
    }
}

/// No copy of the words outside the core: the harness's renderer, the manifest example and the
/// C smoke call the core (the smoke through `rr_dr60_manifest_word`).
#[test]
fn as2_no_copies() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    for f in [
        "rr_dr60_harness/src/manifest_checks.rs",
        "rr_dr60_harness/examples/manifest.rs",
        "rr_dr60_ffi/tests/c/smoke.c",
    ] {
        let src = std::fs::read_to_string(root.join(f)).unwrap();
        for w in [
            "\"not_modeled\"",
            "\"not_applicable\"",
            "\"checked against a real unit\"",
        ] {
            assert!(!src.contains(w), "{f} copies {w}");
        }
    }
}
