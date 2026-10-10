//! Golden-file checks for spec 002 (FR-017, FR-018, SC-003; contracts/golden-format.md).

use sha2::{Digest, Sha256};

/// SHA-256 of `golden/golden-v1.json` as committed on `main` (unchanged since spec 001).
/// Spec 002 must never change the 001 golden file (FR-017, FR-018).
const GOLDEN_V1_SHA256: &str = "a02c3ae4a900d3eacfefb2162737f1363541cb0549786054175cf19cf2be0dcc";

#[test]
fn golden_v1_unchanged() {
    let digest = Sha256::digest(include_bytes!("../golden/golden-v1.json"));
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex, GOLDEN_V1_SHA256,
        "golden-v1.json changed; spec 002 must keep the spec 001 golden file byte-identical"
    );
}

/// SHA-256 of `golden/golden-agc-v1.json` as committed on `main` (spec 002).
/// Spec 003 must never change the 002 golden file (003 FR-019, FR-020; 003 research R-10).
const GOLDEN_AGC_V1_SHA256: &str =
    "59f68184ee2dc0988f6f4a47b186a6da4e6c08834271db7a0e6013368029df6d";

#[test]
fn golden_agc_v1_unchanged() {
    let digest = Sha256::digest(include_bytes!("../golden/golden-agc-v1.json"));
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex, GOLDEN_AGC_V1_SHA256,
        "golden-agc-v1.json changed; spec 003 must keep the spec 002 golden file byte-identical"
    );
}

/// Spec 002 FR-017 (contracts/golden-format.md): 36 AGC entries, bit-exact on every target.
/// Bless on purpose only, with a CHANGELOG entry:
/// `RR_DR60_BLESS=agc cargo test -p rr_dr60_harness --test golden_agc`
#[test]
fn golden_agc_matches() {
    use rr_dr60_harness::checks::standard;
    use rr_dr60_harness::golden;

    let actual = golden::generate_agc(&standard);
    assert_eq!(actual.entries.len(), 36);
    let path = golden::path_agc();
    if std::env::var("RR_DR60_BLESS").as_deref() == Ok("agc") {
        golden::bless(&path, &actual).unwrap();
        eprintln!("blessed {}", path.display());
        return;
    }
    if let Err(report) = golden::compare(&golden::committed_agc(), &actual) {
        panic!("output differs from {}:\n{report}", path.display());
    }
}
