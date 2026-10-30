//! Golden-file check for spec 015, the sample-timed changes and the change log (FR-009, SC-003;
//! research R-10; tasks.md T016, T017). The ten earlier golden files are untouched (their
//! generators do not emit `changes`) and keep their own guards.

use rr_dr60_harness::checks::standard;
use rr_dr60_harness::golden;
use sha2::{Digest, Sha256};

/// SHA-256 of `golden/golden-changes-v1.json` as blessed once by spec 015 (T017). The golden
/// window is closed for the feature: a moved hash is a defect, never a re-bless; a later change
/// to a landing rule is a model change with a CHANGELOG entry (FR-009).
const GOLDEN_CHANGES_V1_SHA256: &str =
    "e6f1565c425a068b5a51ad39d07d15cad994847318407a5e2e9f8566c94b88de"; // blessed 2026-10-30 (015 T017)

#[test]
fn golden_changes_v1_unchanged() {
    let digest = Sha256::digest(include_bytes!("../golden/golden-changes-v1.json"));
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex, GOLDEN_CHANGES_V1_SHA256,
        "golden-changes-v1.json changed; it must stay byte-identical to the spec 015 bless"
    );
}

/// Spec 015 FR-009: 98 entries (13 scenarios × 7 rates, the speed scenario also with the codec
/// on), each the output digest and the full log at 64-sample blocks, bit-exact on every target;
/// blessing also asserts the partition identity and the block-size-1 setter oracle. Bless on
/// purpose only, with a CHANGELOG entry:
/// `RR_DR60_BLESS=changes cargo test -p rr_dr60_harness --test golden_changes`
#[test]
fn golden_changes_matches() {
    let bless = std::env::var("RR_DR60_BLESS").as_deref() == Ok("changes");
    let actual = golden::generate_changes(&standard, bless);
    assert_eq!(actual.entries.len(), 98);
    assert!(actual.entries.iter().all(|e| {
        let c = e.timed_changes.as_ref().expect("changes rows");
        !c.schedule.is_empty() && c.max_per_call <= 67
    }));
    if bless {
        golden::bless(&golden::path_changes(), &actual).expect("write the golden file");
        eprintln!("blessed {}", golden::path_changes().display());
    }
    let committed = golden::committed_changes();
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
            c.changes, a.changes,
            "{}/{}/{} Hz: the schedule and the log",
            c.stimulus, c.config, c.host_rate_hz
        );
    }
}
