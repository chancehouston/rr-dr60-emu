//! Golden-file check for spec 020, the processing record (SC-002; plan review coach 11): one
//! entry per rate, the record's text with its version lines normalized (asserted separately
//! against the library and the manifest) so the file moves only when the record's data moves.
//! The thirteen earlier golden files are untouched and keep their own guards.

use rr_dr60_harness::checks::standard;
use rr_dr60_harness::golden;
use sha2::{Digest, Sha256};

/// SHA-256 of `golden/golden-record-v1.json` as blessed by spec 020 (T015) and re-blessed once
/// by spec 021 (T001's version bump showed the `recording_digest` line follows the library
/// version in the stored form's header, so the golden now normalizes it as
/// `<version-dependent>`; every data line is unchanged). The golden window is closed for the
/// feature: a moved hash is a defect, never a re-bless; a later form version is a new file.
const GOLDEN_RECORD_V1_SHA256: &str =
    "026f1ca3aa98cdcaecef584adf604938550d6eab0fb7e2cde565ea3733e4c8ec"; // re-blessed 2026-10-31 by spec 021 (the digest line normalized; was 0620eb30…61be, blessed 2026-10-31, 020 T015)

#[test]
fn golden_record_v1_unchanged() {
    let digest = Sha256::digest(include_bytes!("../golden/golden-record-v1.json"));
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex, GOLDEN_RECORD_V1_SHA256,
        "golden-record-v1.json changed; it must stay byte-identical to the spec 020 bless"
    );
}

/// Spec 020 SC-002: seven entries (one per rate), each the stored samples' digest and the
/// normalized record, bit-exact on every target. Bless on purpose only, with a CHANGELOG
/// entry: `RR_DR60_BLESS=record cargo test -p rr_dr60_harness --test golden_record`
#[test]
fn golden_record_matches() {
    let bless = std::env::var("RR_DR60_BLESS").as_deref() == Ok("record");
    let actual = golden::generate_record_contract(&standard);
    assert_eq!(actual.entries.len(), 7);
    if bless {
        golden::bless(&golden::path_record_contract(), &actual).expect("write the golden file");
        eprintln!("blessed {}", golden::path_record_contract().display());
    }
    let committed = golden::committed_record_contract();
    assert_eq!(committed.format, "rr_dr60-golden");
    assert_eq!(committed.entries.len(), actual.entries.len());
    for (c, a) in committed.entries.iter().zip(&actual.entries) {
        assert_eq!((&c.config, c.host_rate_hz), (&a.config, a.host_rate_hz));
        assert_eq!(
            c.sha256, a.sha256,
            "{} Hz: the stored samples",
            c.host_rate_hz
        );
        assert_eq!(
            c.record_contract, a.record_contract,
            "{} Hz: the record",
            c.host_rate_hz
        );
    }
}
