//! Golden-file regression (FR-021, SC-003; contracts/golden-format.md; tasks.md T056).
//!
//! Re-bless only on purpose, with a CHANGELOG entry:
//! `RR_DR60_BLESS=1 cargo test -p rr_dr60_harness --test golden`

use rr_dr60_harness::checks::standard;
use rr_dr60_harness::golden;

#[test]
fn output_matches_golden_file() {
    let actual = golden::generate(&standard);
    assert_eq!(actual.entries.len(), 96);
    let path = golden::path();
    if std::env::var("RR_DR60_BLESS").as_deref() == Ok("1") {
        golden::bless(&path, &actual).unwrap();
        eprintln!("blessed {}", path.display());
        return;
    }
    let expected = golden::committed();
    assert_eq!(expected.format, "rr_dr60-golden");
    assert_eq!(expected.version, 1);
    if let Err(report) = golden::compare(&expected, &actual) {
        panic!("output differs from {}:\n{report}", path.display());
    }
}
