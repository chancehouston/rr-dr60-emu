//! Golden-file check for spec 011, the volume transition (FR-012, SC-004; contracts/
//! golden-format.md; tasks.md T037, T042). The six earlier golden files are untouched (their
//! generators set the volume before the first block) and keep their own guards.

use rr_dr60_harness::checks::standard;
use rr_dr60_harness::golden;
use sha2::{Digest, Sha256};

/// SHA-256 of `golden/golden-volume-ramp-v1.json` as blessed once by spec 011 (T042, after the
/// post-implementation reviews). Until the bless the committed file is an empty placeholder
/// and this test is red by design (the gate before the bless skips it). The golden window then
/// closes for the feature: a moved hash is a defect, never a re-bless; a later change to the
/// transition is a model change with a CHANGELOG entry (FR-012).
const GOLDEN_VOLUME_RAMP_V1_SHA256: &str = "placeholder until the 011 T042 bless";

#[test]
fn golden_volume_ramp_v1_unchanged() {
    let digest = Sha256::digest(include_bytes!("../golden/golden-volume-ramp-v1.json"));
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex, GOLDEN_VOLUME_RAMP_V1_SHA256,
        "golden-volume-ramp-v1.json changed; it must stay byte-identical to the spec 011 bless"
    );
}

/// Spec 011 FR-012 (contracts/golden-format.md): 140 volume-ramp entries (5 scenarios × 4
/// configurations × 7 rates), each with its route, starting position and changes, bit-exact on
/// every target; blessing also asserts the splice identity for the stage-11-only entries. Bless
/// on purpose only, with a CHANGELOG entry:
/// `RR_DR60_BLESS=volume_ramp cargo test -p rr_dr60_harness --test golden_volume_ramp`
#[test]
fn golden_volume_ramp_matches() {
    let bless = std::env::var("RR_DR60_BLESS").as_deref() == Ok("volume_ramp");
    let actual = golden::generate_volume_ramp(&standard, bless);
    assert_eq!(actual.entries.len(), 140); // contracts/golden-format.md
    assert!(actual.entries.iter().all(|e| e.route.is_some()
        && e.volume.is_some()
        && e.changes.as_ref().is_some_and(|c| !c.is_empty())
        && e.vas.is_none()
        && e.frames.is_none()
        && e.n_at_bound.is_none()));
    let path = golden::path_volume_ramp();
    if bless {
        golden::bless(&path, &actual).unwrap();
        eprintln!("blessed {}", path.display());
        return;
    }
    let committed = golden::committed_volume_ramp();
    assert_eq!(
        committed.entries.len(),
        140,
        "the committed file holds the blessed entries (an empty placeholder until 011 T042)"
    );
    if let Err(report) = golden::compare(&committed, &actual) {
        panic!("output differs from {}:\n{report}", path.display());
    }
    // `compare` reads the SHA; the changes are compared here.
    for (want, got) in committed.entries.iter().zip(&actual.entries) {
        assert_eq!(
            want.changes, got.changes,
            "{}/{}/{} Hz: the scenario's changes",
            got.stimulus, got.config, got.host_rate_hz
        );
    }
}
