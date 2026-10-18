//! Golden-file check for spec 013, the playback speed control (FR-013, SC-006;
//! contracts/golden-format.md; tasks.md T023, T035). The eight earlier golden files are
//! untouched (level 3 is the identity) and keep their own guards.
//!
//! Red by design until the bless (T035): the file does not exist yet, and the SHA guard holds a
//! placeholder; the gate runs without this binary until then (010's order).

use rr_dr60_harness::checks::standard;
use rr_dr60_harness::golden;
use sha2::{Digest, Sha256};

/// SHA-256 of `golden/golden-speed-v1.json`, pinned at the bless (T035).
const GOLDEN_SPEED_V1_SHA256: &str = "unblessed"; // 013 T035 pins it

#[test]
fn golden_speed_v1_unchanged() {
    let bytes = std::fs::read(golden::path_speed()).expect("golden-speed-v1.json (bless first)");
    let hex: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        hex, GOLDEN_SPEED_V1_SHA256,
        "golden-speed-v1.json changed; it must stay byte-identical to the spec 013 bless"
    );
}

/// Spec 013 FR-013 (contracts/golden-format.md): 194 entries, bit-exact on every target. Bless
/// on purpose only, with a CHANGELOG entry:
/// `RR_DR60_BLESS=speed cargo test -p rr_dr60_harness --test golden_speed`
#[test]
fn golden_speed_matches() {
    let bless = std::env::var("RR_DR60_BLESS").as_deref() == Ok("speed");
    let actual = golden::generate_speed(&standard, bless);
    assert_eq!(actual.entries.len(), 194); // contracts/golden-format.md
    assert!(actual.entries.iter().all(|e| e.speed_level.is_some()
        && e.place.is_some()
        && e.own_length.is_some()
        && e.vas.is_none()
        && e.n_at_bound.is_none()));
    let path = golden::path_speed();
    if bless {
        golden::bless(&path, &actual).unwrap();
        eprintln!("blessed {}", path.display());
        return;
    }
    let committed = golden::committed_speed();
    assert_eq!(committed.entries.len(), 194);
    golden::compare(&committed, &actual).unwrap();
}
