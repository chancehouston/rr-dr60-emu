//! Golden-file check for spec 023, the pitch-kept playback speed mode (D8; research R-12). The
//! fifteen earlier golden files are untouched (the default mode is bit-identical to 0.22) and
//! keep their own guards.

use rr_dr60_harness::checks::standard;
use rr_dr60_harness::golden;
use sha2::{Digest, Sha256};

/// SHA-256 of `golden/golden-pitch-kept-v1.json` as blessed once by spec 023 (tasks.md T013,
/// after the matrix was green). The golden window is closed for the feature: a moved hash is a
/// defect, never a re-bless; a later change to the time-scaler is a model change with a
/// CHANGELOG entry.
const GOLDEN_PITCH_KEPT_V1_SHA256: &str =
    "9406364a82fbb7b5549b5129d2c8ce09d6cac5e22ed1b80a370964708d384c61"; // blessed 2026-10-31 after the hardware review B1 (the same-position rule); the first bless, 3674026f…aaa3, was never committed as the guard

#[test]
fn golden_pitch_kept_v1_unchanged() {
    let hex: String = Sha256::digest(include_bytes!("../golden/golden-pitch-kept-v1.json"))
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        hex, GOLDEN_PITCH_KEPT_V1_SHA256,
        "golden-pitch-kept-v1.json changed; it must stay byte-identical to the spec 023 bless"
    );
}

/// Spec 023 D8: 420 entries, bit-exact on every target. Bless on purpose only, with a CHANGELOG
/// entry: `RR_DR60_BLESS=pitch_kept cargo test -p rr_dr60_harness --test golden_pitch_kept`
/// (asserts the default twins, the lengths, the landings and the reference at bless time).
#[test]
fn golden_pitch_kept_matches() {
    let bless = std::env::var("RR_DR60_BLESS").as_deref() == Ok("pitch_kept");
    let actual = golden::generate_pitch(&standard, bless);
    assert_eq!(actual.entries.len(), 420); // D8: 7 × 4 × 2 × 7 + 4 × 7
    assert!(actual.entries.iter().all(|e| {
        let p = e.pitch.as_ref().expect("the pitch part");
        e.speed_level.is_none() && e.bypass.is_none() && p.produced == u64::from(e.n_samples)
    }));
    let path = golden::path_pitch();
    if bless {
        golden::bless(&path, &actual).unwrap();
        eprintln!("blessed {}", path.display());
        return;
    }
    let committed = golden::committed_pitch();
    assert_eq!(committed.entries.len(), 420);
    golden::compare(&committed, &actual).unwrap();
}
