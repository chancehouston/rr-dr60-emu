//! Golden-file check for spec 012, the microphone capsule and amplifier (FR-014, SC-004;
//! contracts/golden-format.md; tasks.md T050, T056). The seven earlier golden files are
//! untouched (every pre-012 configuration is built on a 0.11-default configuration, the stages
//! bypassed) and keep their own guards.

use rr_dr60_harness::checks::standard;
use rr_dr60_harness::golden;
use sha2::{Digest, Sha256};

/// SHA-256 of `golden/golden-mic-v1.json` as blessed once by spec 012 (T056, after the
/// post-implementation reviews). The golden window is closed for the feature: a moved hash is a
/// defect, never a re-bless; a later change to the microphone path is a model change with a
/// CHANGELOG entry (FR-014).
const GOLDEN_MIC_V1_SHA256: &str =
    "70bc609516b80ceb785c2d905f24d8b88ecf64436b30b4092d297397a101f739"; // blessed 2026-10-16 (012 T056)

#[test]
fn golden_mic_v1_unchanged() {
    let digest = Sha256::digest(include_bytes!("../golden/golden-mic-v1.json"));
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex, GOLDEN_MIC_V1_SHA256,
        "golden-mic-v1.json changed; it must stay byte-identical to the spec 012 bless"
    );
}

/// Spec 012 FR-014 (contracts/golden-format.md): 147 microphone entries (4 stimuli × 5
/// configurations × 7 rates, plus the seven second-recording entries), each with its seed,
/// counter and hiss offset, bit-exact on every target; blessing also asserts the quiet zeros,
/// the hiss band, the first recording against the one-pass and the second recording's own
/// stream. Bless on purpose only, with a CHANGELOG entry:
/// `RR_DR60_BLESS=mic cargo test -p rr_dr60_harness --test golden_mic`
#[test]
fn golden_mic_matches() {
    let bless = std::env::var("RR_DR60_BLESS").as_deref() == Ok("mic");
    let actual = golden::generate_mic(&standard, bless);
    assert_eq!(actual.entries.len(), 147); // contracts/golden-format.md
    assert!(actual.entries.iter().all(|e| e.seed.is_some()
        && e.counter.is_some()
        && e.hiss_offset_db.is_some()
        && e.vas.is_none()
        && e.frames.is_none()
        && e.route.is_none()
        && e.n_at_bound.is_none()
        && e.changes.is_none()));
    assert_eq!(
        actual
            .entries
            .iter()
            .filter(|e| e.counter == Some(1))
            .count(),
        7,
        "the default_mic_rec1 family"
    );
    let path = golden::path_mic();
    if bless {
        golden::bless(&path, &actual).unwrap();
        eprintln!("blessed {}", path.display());
        return;
    }
    let committed = golden::committed_mic();
    assert_eq!(
        committed.entries.len(),
        147,
        "the committed file holds the blessed entries (012 T056)"
    );
    if let Err(report) = golden::compare(&committed, &actual) {
        panic!("output differs from {}:\n{report}", path.display());
    }
    // `compare` reads the SHA; the stream keys are compared here.
    for (want, got) in committed.entries.iter().zip(&actual.entries) {
        assert_eq!(
            (want.seed, want.counter, want.hiss_offset_db),
            (got.seed, got.counter, got.hiss_offset_db),
            "{}/{}/{} Hz: the seed, counter and hiss offset",
            got.stimulus,
            got.config,
            got.host_rate_hz
        );
    }
}
