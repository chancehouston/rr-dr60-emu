//! Golden-file guards and the AGC golden check (spec 005 FR-016, SC-004;
//! contracts/golden-format.md; spec 002 FR-017, FR-018 before it).

use sha2::{Digest, Sha256};

/// SHA-256 of `golden/golden-v1.json` as committed by spec 004 (the one re-bless for the 6 kHz
/// device rate, 004 FR-013; it was unchanged from spec 001 through spec 003, and spec 005 left
/// it byte-identical, 005 FR-016). No later feature may change it without a CHANGELOG-recorded
/// re-bless (002 FR-017, FR-018).
const GOLDEN_V1_SHA256: &str = "ae921d771a43903a638bd547143da2d0000c173679a7a2ec7849dab654ba86a5";

#[test]
fn golden_v1_unchanged() {
    let digest = Sha256::digest(include_bytes!("../golden/golden-v1.json"));
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex, GOLDEN_V1_SHA256,
        "golden-v1.json changed; it must stay byte-identical to the spec 004 re-bless"
    );
}

/// SHA-256 of `golden/golden-agc-v1.json` as committed by spec 005 (the one re-bless for the
/// limiter, 005 FR-016, T044; spec 004 re-blessed it before for the 6 kHz rate). No later
/// feature may change it without a CHANGELOG-recorded re-bless (005 FR-016; research R-10).
const GOLDEN_AGC_V1_SHA256: &str =
    "03d937200e53f16e3152a86e0e9bf28bf505712e80eb9938c8fda9901797beb2";

#[test]
fn golden_agc_v1_unchanged() {
    let digest = Sha256::digest(include_bytes!("../golden/golden-agc-v1.json"));
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex, GOLDEN_AGC_V1_SHA256,
        "golden-agc-v1.json changed; it must stay byte-identical to the spec 005 re-bless"
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
