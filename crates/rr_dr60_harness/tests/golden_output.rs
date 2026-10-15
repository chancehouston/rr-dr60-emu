//! Golden-file check for spec 009, the output chain (FR-016, SC-006; contracts/golden-format.md;
//! tasks.md T046). The four earlier golden files are untouched by the chain (their
//! configurations run it bypassed) and keep their own guards.

use rr_dr60_harness::checks::standard;
use rr_dr60_harness::golden;
use sha2::{Digest, Sha256};

/// SHA-256 of `golden/golden-output-v1.json` as blessed once by spec 009 (T046, after the S-003
/// read and the post-implementation hardware review). The golden window is closed for the
/// feature: a moved hash is a defect, never a re-bless; a later change to the chain's output is
/// a model change with a CHANGELOG entry (FR-016).
const GOLDEN_OUTPUT_V1_SHA256: &str =
    "3b64ae4429cf679b8d4c39e49b7bb2d333035b8d23306b80bc4146d6c805c2f4"; // blessed 2026-10-15 (009 T046)

#[test]
fn golden_output_v1_unchanged() {
    let digest = Sha256::digest(include_bytes!("../golden/golden-output-v1.json"));
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex, GOLDEN_OUTPUT_V1_SHA256,
        "golden-output-v1.json changed; it must stay byte-identical to the spec 009 bless"
    );
}

/// Spec 009 FR-016 (contracts/golden-format.md): 140 output entries (4 stimuli × 5
/// configurations × 7 rates), each with its route and volume position and the one-pass
/// output's SHA, bit-exact on every target; the generator also asserts record-then-play equals
/// the one-pass for the three default configurations (FR-010). Bless on purpose only, with a
/// CHANGELOG entry: `RR_DR60_BLESS=output cargo test -p rr_dr60_harness --test golden_output`
#[test]
fn golden_output_matches() {
    let actual = golden::generate_output(&standard);
    assert_eq!(actual.entries.len(), 140); // contracts/golden-format.md
    assert!(
        actual.entries.iter().all(|e| e.route.is_some()
            && e.volume.is_some()
            && e.vas.is_none()
            && e.frames.is_none())
    );
    let path = golden::path_output();
    if std::env::var("RR_DR60_BLESS").as_deref() == Ok("output") {
        golden::bless(&path, &actual).unwrap();
        eprintln!("blessed {}", path.display());
        return;
    }
    if let Err(report) = golden::compare(&golden::committed_output(), &actual) {
        panic!("output differs from {}:\n{report}", path.display());
    }
}
