//! Golden-file check for spec 018, the battery value (FR-005, FR-010, SC-003; research R-09;
//! tasks.md T019, T020). The twelve earlier golden files are untouched (their generators do not
//! emit `battery`) and keep their own guards.

use rr_dr60_harness::checks::standard;
use rr_dr60_harness::golden;
use sha2::{Digest, Sha256};

/// SHA-256 of `golden/golden-battery-v1.json` as blessed once by spec 018 (T020). The golden
/// window is closed for the feature: a moved hash is a defect, never a re-bless; a later change
/// to the law or the transition is a model change with a CHANGELOG entry (FR-005).
const GOLDEN_BATTERY_V1_SHA256: &str =
    "ecd1072f7f0f4c1e598fac2ecae4fe9d52face92c567c45da797b6789eaa07db"; // blessed 2026-10-30 (018 T020)

#[test]
fn golden_battery_v1_unchanged() {
    let digest = Sha256::digest(include_bytes!("../golden/golden-battery-v1.json"));
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex, GOLDEN_BATTERY_V1_SHA256,
        "golden-battery-v1.json changed; it must stay byte-identical to the spec 018 bless"
    );
}

/// Spec 018 FR-005: 6 points (five law points and the twin without the setter) × 2 routes × 3
/// stimuli × 7 rates plus one scheduled change per rate: 259 entries, each the output digest,
/// the last call's battery meters and the battery entries of the log at 1024-sample blocks,
/// bit-exact on every target; blessing also asserts the default, below-knee, earphone and
/// no-op twins. Bless on purpose only, with a CHANGELOG entry:
/// `RR_DR60_BLESS=battery cargo test -p rr_dr60_harness --test golden_battery`
#[test]
fn golden_battery_matches() {
    let bless = std::env::var("RR_DR60_BLESS").as_deref() == Ok("battery");
    let actual = golden::generate_battery(&standard, bless);
    assert_eq!(actual.entries.len(), 6 * 2 * 3 * 7 + 7);
    assert!(actual.entries.iter().all(|e| {
        let b = e.battery.as_ref().expect("battery rows");
        !b.battery_row.is_empty() && b.max_per_call <= 139
    }));
    if bless {
        golden::bless(&golden::path_battery(), &actual).expect("write the golden file");
        eprintln!("blessed {}", golden::path_battery().display());
    }
    let committed = golden::committed_battery();
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
            c.battery, a.battery,
            "{}/{}/{} Hz: the battery meters and the log",
            c.stimulus, c.config, c.host_rate_hz
        );
    }
}
