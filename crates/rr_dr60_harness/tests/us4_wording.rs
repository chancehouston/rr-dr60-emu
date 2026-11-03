//! Spec 026 US4 AS2 – AS4 [TR: D3, FR-003, FR-008]: the wording lists in the core, their
//! matcher against the regex oracle, one source, and the new strings.

use rr_dr60::wording::{self, CLAIMS, ENGINE_STRINGS};
use rr_dr60_harness::manifest_checks as mc;

#[test]
fn as1_lists() {
    // 007's list, moved verbatim (twelve stems, four phrases, seventeen suffixes).
    assert_eq!(ENGINE_STRINGS.stems.len(), 12);
    assert_eq!(ENGINE_STRINGS.phrases.len(), 4);
    assert_eq!(ENGINE_STRINGS.suffixes.len(), 17);
    for w in ["ghost", "spirit", "evp", "detect", "event"] {
        assert!(ENGINE_STRINGS.stems.contains(&w), "{w}");
    }
    for w in [
        "accurate",
        "authentic",
        "reveal",
        "uncover",
        "hidden voice",
        "measured",
    ] {
        assert!(CLAIMS.phrases.contains(&w), "{w}");
    }
    assert_eq!(CLAIMS.exceptions, &[("measured", "not ")]);
}

/// Every string the engine publishes: both manifests and the guide.
fn engine_strings() -> Vec<String> {
    let mut v = Vec::new();
    let m = rr_dr60::manifest();
    for s in m.stages() {
        for t in [s.name, s.description, s.coverage, s.controls] {
            v.push(t.to_owned());
        }
    }
    for e in rr_dr60::controls::controls() {
        v.push(e.name.to_owned());
        v.push(e.description.to_owned());
    }
    let guide = std::fs::read_to_string(mc::repo_root().join("docs/guide.md")).unwrap();
    v.extend(guide.lines().map(str::to_owned));
    v
}

/// The fuzz corpus: tokens from the lists and their pieces, letters in both cases, digits,
/// punctuation and a few non-ASCII letters, from a seeded generator (10⁵ strings).
fn fuzz() -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    for l in [&ENGINE_STRINGS, &CLAIMS] {
        tokens.extend(l.stems.iter().map(|s| (*s).to_owned()));
        tokens.extend(l.phrases.iter().map(|s| (*s).to_owned()));
        tokens.extend(l.suffixes.iter().map(|s| (*s).to_owned()));
    }
    tokens.extend(
        [
            "EVENT", "Ghost", "x", "_", "1", " ", " ", "-", ".", "'", "é", "ü", "ß", "中", "ab",
            "not ",
        ]
        .iter()
        .map(|s| (*s).to_owned()),
    );
    let mut x: u64 = 0x2026_0003_D3D3_0001;
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    (0..100_000)
        .map(|_| {
            let n = 1 + (next() % 6) as usize;
            (0..n)
                .map(|_| tokens[(next() % tokens.len() as u64) as usize].clone())
                .collect::<String>()
        })
        .collect()
}

#[test]
fn as2_differential() {
    let re = mc::denylist_regex();
    let mut checked = 0usize;
    let mut hits = 0usize;
    for s in engine_strings().into_iter().chain(fuzz()) {
        let ours = wording::first_hit(&s).map(|h| (h.start, h.end));
        let theirs = re.find(&s).map(|m| (m.start(), m.end()));
        assert_eq!(ours, theirs, "{s:?}");
        checked += 1;
        hits += usize::from(ours.is_some());
    }
    assert!(
        hits > 1000,
        "the corpus exercises hits: {hits} of {checked}"
    );
    // A documented difference: no non-ASCII case folding (no engine string uses these).
    assert!(wording::first_hit("ſpirit").is_none());
    assert!(wording::first_hit("\u{212A}ghost").is_none());
}

#[test]
fn as3_one_source() {
    assert!(std::ptr::eq(mc::DENYLIST_STEMS, ENGINE_STRINGS.stems));
    assert!(std::ptr::eq(mc::DENYLIST_PHRASES, ENGINE_STRINGS.phrases));
    assert_eq!(mc::denylist_hit("a ghost"), Some("ghost".to_owned()));
}

/// FR-008: the new strings of 026 pass both tiers (the wording module's own data is exempt).
#[test]
fn as4_new_strings_pass() {
    use rr_dr60::{Confidence, StagePath, StageStatus};
    let mut v: Vec<&str> = Vec::new();
    for s in [
        StageStatus::NotModeled,
        StageStatus::Assumed,
        StageStatus::Sourced,
        StageStatus::Checked,
    ] {
        v.extend([s.token(), s.label()]);
    }
    for p in [StagePath::Record, StagePath::Playback, StagePath::Both] {
        v.extend([p.token(), p.label()]);
    }
    for c in [
        Confidence::NotApplicable,
        Confidence::Low,
        Confidence::Medium,
        Confidence::High,
    ] {
        v.extend([c.token(), c.label()]);
    }
    for f in rr_dr60::meters::FIELDS {
        v.extend([f.identifier, f.unit, f.sentence]);
    }
    for s in v {
        assert!(wording::first_hit(s).is_none(), "{s}");
        assert!(wording::first_claim_hit(s).is_none(), "{s}");
    }
}
