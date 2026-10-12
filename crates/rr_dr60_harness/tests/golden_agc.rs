//! Golden-file guards and the AGC golden check (spec 005 FR-016, SC-004;
//! contracts/golden-format.md; spec 002 FR-017, FR-018 before it).

use sha2::{Digest, Sha256};

/// SHA-256 of `golden/golden-v1.json` as committed by spec 006 (the one re-bless for the `f32`
/// narrowing of the post-VAS device-rate stream, 006 FR-009, T010; spec 004 re-blessed it for
/// the 6 kHz device rate, and specs 001–003 and 005 left it byte-identical). No later feature
/// may change it without a CHANGELOG-recorded re-bless (002 FR-017, FR-018; 006 FR-009).
const GOLDEN_V1_SHA256: &str = "c8ba66679bd074b955768be43c54553a3d4ffb5ab81048a6b34df26532c8ea78";

#[test]
fn golden_v1_unchanged() {
    let digest = Sha256::digest(include_bytes!("../golden/golden-v1.json"));
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex, GOLDEN_V1_SHA256,
        "golden-v1.json changed; it must stay byte-identical to the spec 006 re-bless"
    );
}

/// SHA-256 of `golden/golden-agc-v1.json` as committed by spec 006 (the one re-bless for the
/// `f32` narrowing, 006 FR-009, T010; spec 005 re-blessed it for the limiter and spec 004 for
/// the 6 kHz rate before that). No later feature may change it without a CHANGELOG-recorded
/// re-bless (005 FR-016; 006 FR-009).
const GOLDEN_AGC_V1_SHA256: &str =
    "009cc127e695d252ac4fd85abeacdacc53543f76030a0d5ceb4836362f51a17e";

#[test]
fn golden_agc_v1_unchanged() {
    let digest = Sha256::digest(include_bytes!("../golden/golden-agc-v1.json"));
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex, GOLDEN_AGC_V1_SHA256,
        "golden-agc-v1.json changed; it must stay byte-identical to the spec 006 re-bless"
    );
}

/// Spec 005 FR-016 (contracts/golden-format.md): 42 AGC entries (3 stimuli × 2 configurations
/// × 7 rates), bit-exact on every target; `agc_only` is the host-rate "after AGC" tap.
/// Bless on purpose only, with a CHANGELOG entry:
/// `RR_DR60_BLESS=agc cargo test -p rr_dr60_harness --test golden_agc`
#[test]
fn golden_agc_matches() {
    use rr_dr60_harness::checks::standard;
    use rr_dr60_harness::golden;

    let actual = golden::generate_agc(&standard);
    assert_eq!(actual.entries.len(), 42); // 005 contracts/golden-format.md
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
