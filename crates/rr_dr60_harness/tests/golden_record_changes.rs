//! Golden-file check for spec 017, the live record-side values (FR-008, SC-003; research R-12;
//! tasks.md T017, T018). The eleven earlier golden files are untouched (their generators do not
//! emit `record_changes`) and keep their own guards.

use rr_dr60_harness::checks::standard;
use rr_dr60_harness::golden;
use sha2::{Digest, Sha256};

/// SHA-256 of `golden/golden-record-changes-v1.json` as blessed once by spec 017 (T018). The
/// golden window is closed for the feature: a moved hash is a defect, never a re-bless; a later
/// change to a landing rule is a model change with a CHANGELOG entry (FR-008).
const GOLDEN_RECORD_CHANGES_V1_SHA256: &str =
    "789be307db6581aa8ab6a9678d91d40cad1edf0e798978531c923dfd3e1e99e0"; // blessed 2026-10-30 (017 T018)

#[test]
fn golden_record_changes_v1_unchanged() {
    let digest = Sha256::digest(include_bytes!("../golden/golden-record-changes-v1.json"));
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex, GOLDEN_RECORD_CHANGES_V1_SHA256,
        "golden-record-changes-v1.json changed; it must stay byte-identical to the spec 017 bless"
    );
}

/// Spec 017 FR-008: 8 values × 2 scenarios × 2 streams × 3 configurations × 7 rates, plus the
/// hang shortening (2 streams × 2 modes), the stop case and the cross-stream landing at 7
/// rates: 714 entries, each the output digest and the full log at 64-sample blocks, bit-exact
/// on every target; blessing also asserts the input-0 and partition identities. Bless on
/// purpose only, with a CHANGELOG entry:
/// `RR_DR60_BLESS=record_changes cargo test -p rr_dr60_harness --test golden_record_changes`
#[test]
fn golden_record_changes_matches() {
    let bless = std::env::var("RR_DR60_BLESS").as_deref() == Ok("record_changes");
    let actual = golden::generate_record_changes(&standard, bless);
    assert_eq!(actual.entries.len(), 8 * 2 * 2 * 3 * 7 + 4 * 7 + 2 * 7);
    assert!(actual.entries.iter().all(|e| {
        let c = e.record_changes.as_ref().expect("record-changes rows");
        !c.schedule.is_empty() && !c.log.is_empty() && c.max_per_call <= 139
    }));
    if bless {
        golden::bless(&golden::path_record_changes(), &actual).expect("write the golden file");
        eprintln!("blessed {}", golden::path_record_changes().display());
    }
    let committed = golden::committed_record_changes();
    assert_eq!(committed.format, "rr_dr60-golden");
    assert_eq!(committed.entries.len(), actual.entries.len());
    for (c, a) in committed.entries.iter().zip(&actual.entries) {
        assert_eq!(
            (&c.stimulus, &c.config, c.host_rate_hz),
            (&a.stimulus, &a.config, a.host_rate_hz)
        );
        assert_eq!(
            c.sha256, a.sha256,
            "{}/{}/{} Hz: the output",
            c.stimulus, c.config, c.host_rate_hz
        );
        assert_eq!(
            c.frames_sha256, a.frames_sha256,
            "{}/{}/{} Hz: the frames",
            c.stimulus, c.config, c.host_rate_hz
        );
        assert_eq!(
            c.record_changes, a.record_changes,
            "{}/{}/{} Hz: the schedule, the log and the final position",
            c.stimulus, c.config, c.host_rate_hz
        );
    }
}
