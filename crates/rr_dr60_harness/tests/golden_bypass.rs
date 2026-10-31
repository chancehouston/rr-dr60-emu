//! Golden-file check for spec 021, the live playback-stage bypass (FR-006, SC-001; research
//! R-09; tasks.md T014, T015). The fourteen earlier golden files are untouched (their generators
//! do not emit `bypass`) and keep their own guards.

use rr_dr60_harness::checks::standard;
use rr_dr60_harness::golden;
use sha2::{Digest, Sha256};

/// SHA-256 of `golden/golden-bypass-v1.json` as blessed once by spec 021 (T015). The golden
/// window is closed for the feature: a moved hash is a defect, never a re-bless; a change to
/// the blend is a convention change with a CHANGELOG entry.
const GOLDEN_BYPASS_V1_SHA256: &str = "pending"; // blessed at T015

#[test]
fn golden_bypass_v1_unchanged() {
    let digest = Sha256::digest(include_bytes!("../golden/golden-bypass-v1.json"));
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex, GOLDEN_BYPASS_V1_SHA256,
        "golden-bypass-v1.json changed; it must stay byte-identical to the spec 021 bless"
    );
}

/// Spec 021 FR-006: 20 entries per rate (ten cells × two scenarios, the two whole-path entries),
/// each the output digest, the schedule and the switch entries at 4096-sample blocks, bit-exact
/// on every target; blessing also asserts the endpoint identities (exact where the switched stage
/// is last, the convergence bound elsewhere) and the block-size-1 partition. Bless on purpose
/// only, with a CHANGELOG entry: `RR_DR60_BLESS=bypass cargo test -p rr_dr60_harness --test golden_bypass`
#[test]
fn golden_bypass_matches() {
    let bless = std::env::var("RR_DR60_BLESS").as_deref() == Ok("bypass");
    let actual = golden::generate_bypass(&standard, bless);
    assert_eq!(actual.entries.len(), (10 * 2 + 2) * 7);
    assert!(actual.entries.iter().all(|e| {
        let b = e.bypass.as_ref().expect("bypass rows");
        !b.log.is_empty() && b.produced > 0
    }));
    if bless {
        golden::bless(&golden::path_bypass(), &actual).expect("write the golden file");
        eprintln!("blessed {}", golden::path_bypass().display());
    }
    let committed = golden::committed_bypass();
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
            c.bypass, a.bypass,
            "{}/{}/{} Hz: the schedule and the log",
            c.stimulus, c.config, c.host_rate_hz
        );
    }
}
