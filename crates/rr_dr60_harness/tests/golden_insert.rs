//! Golden-file check for spec 024, the slot and the band filter (D10, SC-003; research R-08).
//! The fifteen audio golden files are untouched (the slot is empty in their configurations) and
//! keep their own guards; the record golden moved once (form 2).

// The default Q is the spec's literal 0.7071 (024 D4), not 1/√2.
#![allow(clippy::approx_constant)]

use rr_dr60_harness::insert_checks as ic;
use sha2::{Digest, Sha256};

/// SHA-256 of `golden/golden-insert-v1.json` as blessed once by spec 024 (T020).
const GOLDEN_INSERT_V1_SHA256: &str =
    "8d0af1c671aa382d5ddcc92e7ae4f58c79d3aac00c978a2b57efe240b22ae4e7"; // blessed 2026-11-01 (024 T020)

#[test]
fn golden_insert_v1_unchanged() {
    let digest = Sha256::digest(include_bytes!("../golden/golden-insert-v1.json"));
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex, GOLDEN_INSERT_V1_SHA256,
        "golden-insert-v1.json changed; it must stay byte-identical to the spec 024 bless"
    );
}

/// 84 entries (research R-08): the four types and three low-pass sets on voice through the
/// recorder with the codec on at the seven rates, the one-pass at the defaults, two record
/// schedules per rate, the tone rows, the second recording of a session, two set-then-schedule
/// rows. At bless time every scheduled change landed. Bless on purpose only, with a CHANGELOG
/// entry: `RR_DR60_BLESS=insert cargo test -p rr_dr60_harness --test golden_insert`
#[test]
fn golden_insert_matches() {
    let bless = std::env::var("RR_DR60_BLESS").as_deref() == Ok("insert");
    let actual = ic::generate_golden();
    assert_eq!(actual.entries.len(), 84);
    if bless {
        for rate in rr_dr60_harness::configs::all_rates() {
            assert!(
                ic::empty_twin_holds(rate),
                "the empty-slot twin at {rate} (D10)"
            );
        }
        for e in &actual.entries {
            let want = u64::from(e.name.starts_with("schedule") || e.name == "set_then_schedule");
            assert_eq!(
                e.changes, want,
                "{} at {}: every scheduled change landed",
                e.name, e.host_rate_hz
            );
        }
        std::fs::write(
            ic::golden_path(),
            serde_json::to_string_pretty(&actual).unwrap() + "\n",
        )
        .expect("write the golden file");
        eprintln!("blessed {}", ic::golden_path().display());
        return;
    }
    let committed = ic::committed_golden();
    assert_eq!(committed.entries.len(), actual.entries.len());
    for (c, a) in committed.entries.iter().zip(&actual.entries) {
        assert_eq!(c, a, "{} at {} Hz", a.name, a.host_rate_hz);
    }
}
