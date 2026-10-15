//! Golden-file check for spec 010, the output clamp (FR-010, SC-003; contracts/golden-format.md;
//! tasks.md T033). The five earlier golden files are untouched by the option (their
//! configurations keep it off) and keep their own guards.

use rr_dr60_harness::checks::standard;
use rr_dr60_harness::golden;
use sha2::{Digest, Sha256};

/// SHA-256 of `golden/golden-output-clamp-v1.json` as blessed once by spec 010 (T041, after the
/// post-implementation reviews). The golden window is closed for the feature: a moved hash is a
/// defect, never a re-bless; a later change to the bounded output is a model change with a
/// CHANGELOG entry (FR-010).
const GOLDEN_OUTPUT_CLAMP_V1_SHA256: &str =
    "0000000000000000000000000000000000000000000000000000000000000000"; // pinned at 010 T041

#[test]
fn golden_output_clamp_v1_unchanged() {
    let digest = Sha256::digest(include_bytes!("../golden/golden-output-clamp-v1.json"));
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex, GOLDEN_OUTPUT_CLAMP_V1_SHA256,
        "golden-output-clamp-v1.json changed; it must stay byte-identical to the spec 010 bless"
    );
}

/// Spec 010 FR-010 (contracts/golden-format.md): 126 output-clamp entries (3 stimuli × 6
/// configurations × 7 rates), each with its route, position, the one-pass output's SHA and the
/// count of samples at the bound, bit-exact on every target; blessing also asserts the identity
/// against the option-off twins, the reference configuration's zero counts on the tones and
/// record-then-play equals the one-pass for the three default configurations. Bless on purpose
/// only, with a CHANGELOG entry:
/// `RR_DR60_BLESS=output_clamp cargo test -p rr_dr60_harness --test golden_output_clamp`
#[test]
fn golden_output_clamp_matches() {
    let bless = std::env::var("RR_DR60_BLESS").as_deref() == Ok("output_clamp");
    let actual = golden::generate_output_clamp(&standard, bless);
    assert_eq!(actual.entries.len(), 126); // contracts/golden-format.md
    assert!(actual.entries.iter().all(|e| e.route.is_some()
        && e.volume.is_some()
        && e.n_at_bound.is_some()
        && e.vas.is_none()
        && e.frames.is_none()));
    // The reference configuration's entries on the two tones have no sample at the bound; the
    // position-1 entries of the four overshooting cells have some (research R-04; T006).
    for e in &actual.entries {
        let n = e.n_at_bound.unwrap();
        if e.config == "default_output_ref_clamp" && e.stimulus != "square_1k_fs" {
            assert_eq!(n, 0, "{}/{}/{}", e.stimulus, e.config, e.host_rate_hz);
        }
        if e.stimulus == "square_1k_fs" && e.config != "default_output_ref_clamp"
            || e.stimulus == "sine_1k_1p5" && e.config == "output_clamp_only"
        {
            assert!(n > 0, "{}/{}/{}", e.stimulus, e.config, e.host_rate_hz);
        }
    }
    let path = golden::path_output_clamp();
    if bless {
        golden::bless(&path, &actual).unwrap();
        eprintln!("blessed {}", path.display());
        return;
    }
    let committed = golden::committed_output_clamp();
    if let Err(report) = golden::compare(&committed, &actual) {
        panic!("output differs from {}:\n{report}", path.display());
    }
    // `compare` reads the SHA; the count at the bound is compared here.
    for (want, got) in committed.entries.iter().zip(&actual.entries) {
        assert_eq!(
            want.n_at_bound, got.n_at_bound,
            "{}/{}/{} Hz: samples at the bound",
            got.stimulus, got.config, got.host_rate_hz
        );
    }
}
