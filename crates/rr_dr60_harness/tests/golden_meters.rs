//! Golden-file check for spec 016, the per-block meters (FR-012, SC-004; contracts/api.md ›
//! Golden format; tasks.md T013, T017). The nine earlier golden files are untouched (their
//! generators do not emit `meters`) and keep their own guards.

use rr_dr60_harness::checks::standard;
use rr_dr60_harness::golden;
use sha2::{Digest, Sha256};

/// SHA-256 of `golden/golden-meters-v1.json` as blessed once by spec 016 (T017). The golden
/// window is closed for the feature: a moved hash is a defect, never a re-bless; a later change
/// to a figure is a model change with a CHANGELOG entry (FR-012).
const GOLDEN_METERS_V1_SHA256: &str =
    "1394bbb9d7912ca560bc403e62965a569a08711dbc257ea65997fef43c5d2fe3"; // blessed 2026-10-30 (016 T017)

#[test]
fn golden_meters_v1_unchanged() {
    let digest = Sha256::digest(include_bytes!("../golden/golden-meters-v1.json"));
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex, GOLDEN_METERS_V1_SHA256,
        "golden-meters-v1.json changed; it must stay byte-identical to the spec 016 bless"
    );
}

/// Spec 016 FR-012: 168 entries (4 stimuli × 6 configurations × 7 rates), each with the
/// one-pass output's digest and every call's read-out row, bit-exact on every target;
/// blessing also asserts the tap and count identities and the short-hang rows. Bless on
/// purpose only, with a CHANGELOG entry:
/// `RR_DR60_BLESS=meters cargo test -p rr_dr60_harness --test golden_meters`
#[test]
fn golden_meters_matches() {
    let bless = std::env::var("RR_DR60_BLESS").as_deref() == Ok("meters");
    let actual = golden::generate_meters(&standard, bless);
    assert_eq!(actual.entries.len(), 168); // contracts/api.md › Golden format
    assert!(actual.entries.iter().all(|e| {
        let m = e.meters.as_ref().expect("meters rows");
        e.seed.is_some()
            && e.counter == Some(0)
            && m.block_len == 1024
            && !m.blocks.is_empty()
            && !m.record_blocks.is_empty()
            && !m.play_blocks.is_empty()
            && m.digest.is_none()
            && e.vas.is_none()
            && e.frames.is_none()
            && e.route.is_none()
            && e.n_at_bound.is_none()
    }));
    // The default configuration's entries carry the flush's row and the drain's rows (a
    // configuration whose record latency is 0 at 6 kHz feeds no flush zero, so its flush row is
    // absent by FR-001).
    assert!(
        actual
            .entries
            .iter()
            .filter(|e| e.config == "meters_default")
            .all(|e| {
                let m = e.meters.as_ref().unwrap();
                m.flush.is_some() && !m.drain_blocks.is_empty()
            })
    );
    let path = golden::path_meters();
    if bless {
        golden::bless(&path, &actual).unwrap();
        eprintln!("blessed {}", path.display());
        return;
    }
    let committed = golden::committed_meters();
    assert_eq!(
        committed.entries.len(),
        168,
        "the committed file holds the blessed entries (016 T017)"
    );
    if let Err(report) = golden::compare(&committed, &actual) {
        panic!("output differs from {}:\n{report}", path.display());
    }
    // `compare` reads the audio's SHA; the rows are compared here, row for row.
    for (want, got) in committed.entries.iter().zip(&actual.entries) {
        assert_eq!(
            (&want.stimulus, &want.config, want.host_rate_hz),
            (&got.stimulus, &got.config, got.host_rate_hz)
        );
        assert_eq!(
            want.meters, got.meters,
            "{}/{}/{} Hz: the read-out rows differ",
            got.stimulus, got.config, got.host_rate_hz
        );
    }
}
