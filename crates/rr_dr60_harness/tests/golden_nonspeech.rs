//! Spec 029 [TR: D1, D5, D6; FR-004; US2 AS1]: the canonical text golden at the three fixed
//! rates, byte-compared and SHA-pinned; the bless asserts the partition identity and the checks;
//! the doctored-hang teeth. Ignored under coverage (plan P5): the plain build and CI's golden
//! targets are authoritative.

use rr_dr60_harness::nonspeech_checks::{self as ns, Configuration as C, Stimulus as S};
use sha2::{Digest, Sha256};

/// SHA-256 of `golden/nonspeech-v1.txt` as blessed by spec 029 (T017). The scanner of
/// `us1_bypass` maps this constant to `nonspeech-v1.txt` (its text-golden arm).
const GOLDEN_NONSPEECH_V1_SHA256: &str =
    "460fea9dc5b1e71522684c48bcd0e6922015c387fd403567bb156dad8bf2d13d"; // blessed 2026-11-04 (029 T017; re-blessed once after the post-implementation reviews)

fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
#[cfg_attr(coverage, ignore)]
fn golden_nonspeech_v1_unchanged() {
    let bytes = std::fs::read(ns::golden_path())
        .expect("golden/nonspeech-v1.txt; bless with RR_DR60_BLESS=nonspeech");
    assert_eq!(
        sha(&bytes),
        GOLDEN_NONSPEECH_V1_SHA256,
        "nonspeech-v1.txt changed; it must stay byte-identical to the bless"
    );
}

/// 240 rows (3 rates × 10 stimuli × 8 configurations). Bless on purpose only, with a CHANGELOG
/// entry: `RR_DR60_BLESS=nonspeech cargo test -p rr_dr60_harness --test golden_nonspeech`.
#[test]
#[cfg_attr(coverage, ignore)]
fn golden_nonspeech_matches() {
    let bless = std::env::var("RR_DR60_BLESS").as_deref() == Ok("nonspeech");
    let text = ns::canonical_text();
    let rows = text
        .lines()
        .filter(|l| {
            l.split_whitespace()
                .nth(1)
                .is_some_and(|c| c.starts_with('C'))
                && !l.starts_with("cfg ")
        })
        .count();
    assert_eq!(rows, 240, "3 × 80 table rows");
    if bless {
        // D1: the bless gate asserts D5's partition identity (samples and heard splices) and
        // every D6 check at the golden's rates.
        for rate in [6000, 48_000] {
            let cells: Vec<(S, C)> = if rate == 6000 {
                vec![(S::A35, C::C2), (S::G, C::C5), (S::D, C::C5), (S::F, C::C5)]
            } else {
                vec![(S::A35, C::C2), (S::G, C::C5)]
            };
            for (s, c) in cells {
                let x = s.samples(rate);
                let base = ns::process_raw(c.settings(rate), &x, &[4096]);
                let parts: Vec<Vec<usize>> = if rate == 6000 {
                    vec![vec![1usize], vec![64], vec![1, 7, 64, 1000, 4096, 3]]
                } else {
                    vec![vec![64], vec![1, 7, 64, 1000, 4096, 3]]
                };
                for part in parts {
                    let got = ns::process_raw(c.settings(rate), &x, &part);
                    assert_eq!(
                        got.0,
                        base.0,
                        "{} {} at {rate}: partition {part:?} (samples)",
                        s.id(),
                        c.id()
                    );
                    assert_eq!(
                        got.1,
                        base.1,
                        "{} {} at {rate}: partition {part:?} (splices)",
                        s.id(),
                        c.id()
                    );
                }
            }
        }
        let checks = ns::d6_all(&ns::GOLDEN_RATES);
        rr_dr60_harness::report::print(&checks);
        rr_dr60_harness::checks::assert_all(&checks);
        std::fs::write(ns::golden_path(), &text).expect("write the golden");
        eprintln!(
            "blessed {} (sha {})",
            ns::golden_path().display(),
            sha(text.as_bytes())
        );
        return;
    }
    let committed = std::fs::read_to_string(ns::golden_path())
        .expect("golden/nonspeech-v1.txt; bless with RR_DR60_BLESS=nonspeech");
    if committed != text {
        let moved: Vec<&str> = committed
            .lines()
            .zip(text.lines())
            .filter(|(a, b)| a != b)
            .map(|(a, _)| a)
            .collect();
        panic!(
            "the text differs from the golden on {} lines, first: {:?}",
            moved.len(),
            moved.first()
        );
    }
}

/// US2 AS1 (SC-002): the gate's hang doubled (A-023) on the gated rungs moves exactly the C5 and
/// C7 rows where the gate pauses (kept output shorter than the input plus the latency); C6 (its
/// gate never closes) is unmoved, and C4 (the gate off) is the control.
#[test]
#[cfg_attr(coverage, ignore)]
fn doctored_hang() {
    let rate = ns::DEVICE_HZ;
    let doctor = |s: &mut rr_dr60::Settings| s.vas.hang_ms *= 2.0;
    let mut moved = Vec::new();
    for s in ns::STIMULI {
        let c0 = ns::cell(s, C::C0, rate).measures;
        // The gated rungs and one ungated control (C4): the hang cannot reach C0 – C3.
        for c in [C::C4, C::C5, C::C6, C::C7] {
            let prod = ns::cell(s, c, rate);
            let doc = ns::compute_cell_with(s, c, rate, ns::BLOCK, ns::PRODUCTION, &doctor);
            let same = ns::row_text(&prod, &c0) == ns::row_text(&doc, &c0);
            // T010's rule: a row pauses when its kept output is shorter than the input plus
            // the latency.
            let pauses = prod.output_len < 12 * rate as usize + prod.latency as usize;
            match c {
                C::C5 | C::C7 => {
                    if pauses {
                        assert!(!same, "{} {}: a pausing row must move", s.id(), c.id());
                        moved.push(format!("{} {}", s.id(), c.id()));
                    } else {
                        assert!(
                            same,
                            "{} {}: a row where the gate never pauses is unmoved",
                            s.id(),
                            c.id()
                        );
                    }
                }
                _ => assert!(same, "{} {}: unmoved by the hang", s.id(), c.id()),
            }
        }
    }
    println!("doctored hang moved: {moved:?}");
    assert!(
        moved.iter().any(|m| m.starts_with("d ")),
        "the bursts row moved"
    );
    assert!(!moved.is_empty());
}
