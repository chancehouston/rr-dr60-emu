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
