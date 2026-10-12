//! Golden-file check for spec 003 (FR-019, SC-003; contracts/golden-format.md; tasks.md T038).

use rr_dr60_harness::checks::standard;
use rr_dr60_harness::golden;
use sha2::{Digest, Sha256};

/// SHA-256 of `golden/golden-vas-v1.json` as committed by spec 006 (the one re-bless for the
/// `f32` narrowing of the post-VAS device-rate stream, 006 FR-009, T010; spec 005 re-blessed it
/// for the re-anchored threshold before that). The guard is new in 0.6: the golden window is
/// closed for the feature, so a moved hash is a defect, never a re-bless (006 research R-09).
const GOLDEN_VAS_V1_SHA256: &str =
    "77306208a0bc0a43c78ff1fc8a4062f31a4d29aca0dc14a34460ff3f2f46948b";

#[test]
fn golden_vas_v1_unchanged() {
    let digest = Sha256::digest(include_bytes!("../golden/golden-vas-v1.json"));
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex, GOLDEN_VAS_V1_SHA256,
        "golden-vas-v1.json changed; it must stay byte-identical to the spec 006 re-bless"
    );
}

/// Spec 003 FR-019 (contracts/golden-format.md): 63 VAS entries (3 stimuli × 3 configurations
/// × 7 rates), covering the produced samples, the events and the final state, bit-exact on
/// every target. Re-blessed once by spec 005 (FR-016): the bursts are at −30 dBFS with the
/// −40 dBFS threshold (A-022 revised) and `default_vas` runs the limiter and the clamp; and
/// once by spec 006 (FR-009) for the `f32` narrowing after the VAS. Bless on purpose only, with
/// a CHANGELOG entry:
/// `RR_DR60_BLESS=vas cargo test -p rr_dr60_harness --test golden_vas`
#[test]
fn golden_vas_matches() {
    let actual = golden::generate_vas(&standard);
    assert_eq!(actual.entries.len(), 63); // 005 contracts/golden-format.md
    assert!(actual.entries.iter().all(|e| e.vas.is_some()));
    let path = golden::path_vas();
    if std::env::var("RR_DR60_BLESS").as_deref() == Ok("vas") {
        golden::bless(&path, &actual).unwrap();
        eprintln!("blessed {}", path.display());
        return;
    }
    if let Err(report) = golden::compare(&golden::committed_vas(), &actual) {
        panic!("output differs from {}:\n{report}", path.display());
    }
}
