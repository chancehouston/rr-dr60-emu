//! Golden-file check for spec 003 (FR-019, SC-003; contracts/golden-format.md; tasks.md T038).

use rr_dr60_harness::checks::standard;
use rr_dr60_harness::golden;

/// Spec 003 FR-019 (contracts/golden-format.md): 63 VAS entries (3 stimuli × 3 configurations
/// × 7 rates since spec 004, FR-013; 54 before), covering the produced samples, the events and
/// the final state, bit-exact on every target. Bless on purpose only, with a CHANGELOG entry:
/// `RR_DR60_BLESS=vas cargo test -p rr_dr60_harness --test golden_vas`
#[test]
fn golden_vas_matches() {
    let actual = golden::generate_vas(&standard);
    assert_eq!(actual.entries.len(), 63); // 004 contracts/golden-format.md
    assert!(actual.entries.iter().all(|e| e.vas.is_some()));
    let path = golden::path_vas();
    if std::env::var("RR_DR60_BLESS").as_deref() == Ok("vas") {
        golden::bless(&path, &actual).unwrap();
        eprintln!("blessed {}", path.display());
        return;
    }
    if let Err(report) = golden::compare(&golden::committed_vas(), &actual) {
        panic!("output differs from {}:\n{report}", path.display());
    }
}
