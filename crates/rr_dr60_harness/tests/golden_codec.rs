//! Golden-file check for spec 008, the speech codec (FR-021, SC-008; contracts/golden-format.md;
//! tasks.md T056). The three earlier golden files are untouched by the codec (their
//! configurations run it bypassed) and keep their own guards.

use rr_dr60_harness::checks::standard;
use rr_dr60_harness::golden;
use sha2::{Digest, Sha256};

/// SHA-256 of `golden/golden-codec-v1.json` as blessed once by spec 008 (T056, after the
/// post-implementation hardware review moved the fixed-gain mean to 54 dB). The golden window
/// is closed for the feature: a moved hash is a defect, never a re-bless; a later change to
/// the frame bytes is a model change with a CHANGELOG entry and, after 1.0, a kept decoder
/// (FR-021).
const GOLDEN_CODEC_V1_SHA256: &str =
    "2fed69c7ebbe1ff96a06b604543ceabc41343f8f56f0a47b5bf7af9344af05f1";

#[test]
fn golden_codec_v1_unchanged() {
    let digest = Sha256::digest(include_bytes!("../golden/golden-codec-v1.json"));
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex, GOLDEN_CODEC_V1_SHA256,
        "golden-codec-v1.json changed; it must stay byte-identical to the spec 008 bless"
    );
}

/// Spec 008 FR-021 (contracts/golden-format.md): 84 codec entries (4 stimuli × 3
/// configurations × 7 rates): the one-pass output (with the VAS events for `codec_only_vas`)
/// and the record-then-play recording's frame count and frame-byte digest, bit-exact on every
/// target; the generator also asserts the recording's playback equals the one-pass output
/// (FR-009). Bless on purpose only, with a CHANGELOG entry:
/// `RR_DR60_BLESS=codec cargo test -p rr_dr60_harness --test golden_codec`
#[test]
fn golden_codec_matches() {
    let actual = golden::generate_codec(&standard);
    assert_eq!(actual.entries.len(), 84); // contracts/golden-format.md
    assert!(
        actual
            .entries
            .iter()
            .all(|e| e.frames.is_some() && e.frames_sha256.is_some())
    );
    assert!(
        actual
            .entries
            .iter()
            .all(|e| e.vas.is_some() == (e.config == "codec_only_vas"))
    );
    let path = golden::path_codec();
    if std::env::var("RR_DR60_BLESS").as_deref() == Ok("codec") {
        golden::bless(&path, &actual).unwrap();
        eprintln!("blessed {}", path.display());
        return;
    }
    if let Err(report) = golden::compare(&golden::committed_codec(), &actual) {
        panic!("output differs from {}:\n{report}", path.display());
    }
}
