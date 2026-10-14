//! Spec 007 User Story 2: the manifest cannot drift from the registers (007 FR-013, FR-011,
//! FR-002, FR-003; SC-002, SC-003; contracts/cross-check.md; tasks.md T014).
//!
//! The cross-check is a function of a manifest value and four texts, so every doctored input
//! here is a copy of the committed text with one change, or a copy of the manifest with one
//! field changed. Each doctored case asserts the rule, the stage and the ID the contract names.

use std::path::Path;
use std::process::Command;

use rr_dr60::{Confidence, manifest};
use rr_dr60_harness::manifest_checks::{
    self as mc, Finding, Headline, Registers, Rule, StatusClass, committed_registers, cross_check,
    render, render_readme_table, wording_findings,
};

fn print(findings: &[Finding]) {
    for f in findings {
        println!("{f}");
    }
}

fn has(findings: &[Finding], rule: Rule, stage: Option<u8>, id: Option<&str>) -> bool {
    findings
        .iter()
        .any(|f| f.rule == rule && f.stage == stage && f.id.as_deref() == id)
}

/// The committed row that starts with `| <id> |`, with its Status (last) cell replaced.
fn with_status(assumptions: &str, id: &str, status: &str) -> String {
    let prefix = format!("| {id} |");
    let mut changed = false;
    let out: Vec<String> = assumptions
        .lines()
        .map(|l| {
            if l.starts_with(&prefix) {
                let mut cells: Vec<&str> = l.trim().trim_matches('|').split('|').collect();
                let n = cells.len();
                cells[n - 1] = status;
                changed = true;
                format!("|{}|", cells.join("|"))
            } else {
                l.to_string()
            }
        })
        .collect();
    assert!(changed, "{id} row not found");
    out.join("\n")
}

fn without_line_starting(text: &str, prefix: &str) -> String {
    let before = text.lines().count();
    let out: Vec<&str> = text.lines().filter(|l| !l.starts_with(prefix)).collect();
    assert_eq!(
        out.len() + 1,
        before,
        "exactly one line starts with {prefix:?}"
    );
    out.join("\n")
}

fn with_cell_appended(text: &str, row_prefix: &str, cell: usize, suffix: &str) -> String {
    let mut changed = false;
    let out: Vec<String> = text
        .lines()
        .map(|l| {
            if l.starts_with(row_prefix) {
                let mut cells: Vec<String> = l
                    .trim()
                    .trim_matches('|')
                    .split('|')
                    .map(str::to_string)
                    .collect();
                cells[cell].push_str(suffix);
                changed = true;
                format!("|{}|", cells.join("|"))
            } else {
                l.to_string()
            }
        })
        .collect();
    assert!(changed, "row {row_prefix:?} not found");
    out.join("\n")
}

// ---- US2 scenario 1; SC-003 --------------------------------------------------------------------

#[test]
fn committed_registers_pass() {
    // 007 FR-013 (a)–(h) on the committed registers: zero findings.
    let f = cross_check(manifest(), &committed_registers());
    print(&f);
    assert!(f.is_empty(), "{} findings", f.len());
}

#[test]
fn wording_is_clean() {
    // 007 FR-002, FR-003, FR-011; SC-003.
    let f = wording_findings(manifest());
    print(&f);
    assert!(f.is_empty(), "{} findings", f.len());
}

// ---- US2 scenarios 2–7; SC-002 (7 of 7) --------------------------------------------------------

#[test]
fn doctored_superseded() {
    // Scenario 2: a cited assumption becomes superseded → Superseded, stage 3, A-028.
    let r = committed_registers();
    let assumptions = with_status(r.assumptions, "A-028", " superseded (test) ");
    let f = cross_check(
        manifest(),
        &Registers {
            assumptions: &assumptions,
            ..r
        },
    );
    print(&f);
    assert!(has(&f, Rule::Superseded, Some(3), Some("A-028")));
}

#[test]
fn doctored_added_assumption() {
    // Scenario 3: a new current assumption nobody cites → Uncited, A-099 (A-040 is registered
    // and cited since spec 008).
    let r = committed_registers();
    let assumptions = format!(
        "{}\n| A-099 | Test | A test assumption. | Test. | Low | Test. | assumed |",
        r.assumptions.trim_end()
    );
    let f = cross_check(
        manifest(),
        &Registers {
            assumptions: &assumptions,
            ..r
        },
    );
    print(&f);
    assert!(has(&f, Rule::Uncited, None, Some("A-099")));
}

// ---- Spec 008 US3 scenarios 2–3 (FR-020, FR-023) -----------------------------------------------

/// Spec 008 US3 scenario 2: the committed registers carry A-004 as adopted, A-040 and A-041
/// registered at low confidence, S-008 – S-011 registered and not General, and stages 7 and 9
/// cite them all; the cross-check passes (`committed_registers_pass`).
#[test]
fn codec_rows_registered_and_cited() {
    let r = committed_registers();
    let (assumptions, af) = mc::parse_assumptions(r.assumptions);
    let (sources, sf) = mc::parse_sources(r.sources);
    assert!(af.is_empty() && sf.is_empty(), "{af:?} {sf:?}");
    let a004 = assumptions.iter().find(|a| a.id == "A-004").expect("A-004");
    assert!(a004.adopted_or_revised, "A-004 is adopted by spec 008");
    assert_eq!(a004.class, StatusClass::Open);
    for id in ["A-040", "A-041"] {
        let a = assumptions
            .iter()
            .find(|a| a.id == id)
            .unwrap_or_else(|| panic!("{id}"));
        assert_eq!(a.headline, Headline::Low, "{id}");
        assert_eq!(a.class, StatusClass::Open, "{id}");
    }
    for id in ["S-008", "S-009", "S-010", "S-011"] {
        let s = sources
            .iter()
            .find(|s| s.id == id)
            .unwrap_or_else(|| panic!("{id}"));
        assert!(!s.general, "{id} bears on stages 7 and 9");
    }
    let m = manifest();
    for n in [7u32, 9] {
        let e = m.stage(n).unwrap();
        for id in ["A-004", "A-040", "A-041"] {
            assert!(e.assumptions.contains(&id), "stage {n} cites {id}");
        }
        for id in ["S-002", "S-008", "S-009", "S-010", "S-011"] {
            assert!(e.sources.contains(&id), "stage {n} cites {id}");
        }
    }
    // The signal-chain rows 7 and 9 carry the marker.
    let (chain, cf) = mc::parse_signal_chain(r.signal_chain);
    assert!(cf.is_empty(), "{cf:?}");
    for n in [7usize, 9] {
        assert!(chain[n - 1].modeled, "row {n} says Modeled by");
    }
}

/// Spec 008 US3 scenario 3 (FR-023): the README's codec section and the CHANGELOG's 0.8.0
/// entry pass the manifest's denylist.
#[test]
fn readme_codec_section_and_changelog_entry_pass_the_denylist() {
    let text = readme();
    let section =
        mc::readme_region(&text, "codec-section").expect("README has the codec-section markers");
    assert!(section.len() > 500, "the codec section is written");
    assert_eq!(mc::denylist_hit(section), None, "README codec section");
    let changelog = std::fs::read_to_string(mc::repo_root().join("CHANGELOG.md")).unwrap();
    let entry = mc::readme_region(&changelog, "0.8.0").expect("CHANGELOG has the 0.8.0 markers");
    assert!(entry.contains("0.8.0"));
    assert_eq!(mc::denylist_hit(entry), None, "CHANGELOG 0.8.0 entry");
}

#[test]
fn doctored_identifier() {
    // Scenario 4: row 3's identifier changed → TableShape, stage 3.
    let r = committed_registers();
    assert_eq!(r.signal_chain.matches("| `limiter` |").count(), 1);
    let chain = r.signal_chain.replace("| `limiter` |", "| `agc` |");
    let f = cross_check(
        manifest(),
        &Registers {
            signal_chain: &chain,
            ..r
        },
    );
    print(&f);
    assert!(has(&f, Rule::TableShape, Some(3), None));
}

#[test]
fn doctored_row_count() {
    // Scenario 5: a sixteenth row, and separately a removed row → TableShape, no stage.
    let r = committed_registers();
    let with_16 = format!(
        "{}\n| 16 | Test | `test` | — | — | — |",
        r.signal_chain.trim_end()
    );
    // The appended row lands after the table's last row only if the table ends the file;
    // it does not, so insert it right after row 15 instead.
    let with_16 = if with_16.contains("\n| 15 |") {
        let mut out = String::new();
        for l in r.signal_chain.lines() {
            out.push_str(l);
            out.push('\n');
            if l.starts_with("| 15 |") {
                out.push_str("| 16 | Test | `test` | — | — | — |\n");
            }
        }
        out
    } else {
        with_16
    };
    let f = cross_check(
        manifest(),
        &Registers {
            signal_chain: &with_16,
            ..r
        },
    );
    print(&f);
    assert!(has(&f, Rule::TableShape, None, None), "sixteenth row");

    let without_15 = without_line_starting(r.signal_chain, "| 15 |");
    let f = cross_check(
        manifest(),
        &Registers {
            signal_chain: &without_15,
            ..r
        },
    );
    print(&f);
    assert!(has(&f, Rule::TableShape, None, None), "removed row");
}

#[test]
fn doctored_marker() {
    // Scenario 6: row 6 gains the marker while the manifest says not modeled →
    // StatusDerivation, stage 6 (row 7 is modeled since spec 008).
    let r = committed_registers();
    // Cells after the edge cells are dropped: 0 #, 1 Stage, 2 Identifier, 3 Known, ...
    let chain = with_cell_appended(r.signal_chain, "| 6 |", 3, " **Modeled by** test");
    let f = cross_check(
        manifest(),
        &Registers {
            signal_chain: &chain,
            ..r
        },
    );
    print(&f);
    assert!(has(&f, Rule::StatusDerivation, Some(6), None));
}

#[test]
fn doctored_confidence() {
    // Scenario 7: a confidence above its bound → ConfidenceBound, stage 3.
    let mut m = *manifest();
    m.stages[2].confidence = Confidence::High;
    let f = cross_check(&m, &committed_registers());
    print(&f);
    assert!(has(&f, Rule::ConfidenceBound, Some(3), None));
}

#[test]
fn doctored_removed_source() {
    // The removed-ID case: S-006 gone → UnknownId, stage 11, S-006.
    let r = committed_registers();
    let sources = without_line_starting(r.sources, "| S-006 |");
    let f = cross_check(
        manifest(),
        &Registers {
            sources: &sources,
            ..r
        },
    );
    print(&f);
    assert!(has(&f, Rule::UnknownId, Some(11), Some("S-006")));
}

// ---- Parser units (contracts/cross-check.md › Register shapes) ---------------------------------

#[test]
fn cells_and_edge_cells() {
    let rows = mc::table_rows("| a | b |\n|---|---|\n| 1 | two |\nnot a row\n");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[2].1, vec!["1".to_string(), "two".to_string()]);
}

#[test]
fn status_classes() {
    // 007 FR-004: the first-word rule, with the "S-### only" condition for verified.
    let c = |s: &str| mc::classify_status(s);
    // "adopted"/"revised" anywhere in the cell flag the row (hardware review G2b).
    assert_eq!(
        c("verified (S-002); adopted by spec 004"),
        (StatusClass::DocumentVerified, true)
    );
    assert_eq!(
        c("verified (topology, S-002); values in A-029"),
        (StatusClass::Open, false)
    );
    assert_eq!(
        c("superseded by A-026 (spec 004, 2026-10-11)"),
        (StatusClass::Superseded, false)
    );
    assert_eq!(c("measured (capture 3)"), (StatusClass::Measured, false));
    assert_eq!(
        c("revised (spec 005, 2026-10-11); partially verified (S-002)"),
        (StatusClass::Open, true)
    );
    assert_eq!(
        c("adopted ([spec 005](x), 2026-10-11)"),
        (StatusClass::Open, true)
    );
    assert_eq!(
        c("assumed (adopted by spec 004, 2026-10-11)"),
        (StatusClass::Open, true)
    );
    assert_eq!(
        c("assumed; refined by spec 005"),
        (StatusClass::Open, false)
    );
    assert_eq!(c("partially verified (S-002)"), (StatusClass::Open, false));
}

#[test]
fn headline_confidence() {
    // 007 FR-006: the first High/Medium/Low word.
    assert_eq!(mc::headline_confidence("Low to medium"), Headline::Low);
    assert_eq!(
        mc::headline_confidence("High (topology) / Low (gain)"),
        Headline::High
    );
    assert_eq!(
        mc::headline_confidence("Medium (proxy datasheet)"),
        Headline::Medium
    );
    assert_eq!(mc::headline_confidence("—"), Headline::Unknown);
}

#[test]
fn general_source_exempt() {
    let (rows, findings) = mc::parse_sources(
        "| ID | Source | Type | Reliability | Key facts extracted |\n|----|---|---|---|---|\n| S-900 | X | General | Secondary | none |\n| S-901 | Y | Article | Secondary | some |\n",
    );
    assert!(findings.is_empty());
    assert_eq!(rows.len(), 2);
    assert!(rows[0].general);
    assert!(!rows[1].general);
}

#[test]
fn marker_in_known_or_assumed_cell_only() {
    let text = "| # | Stage | Identifier | Known | Assumed | Character it contributes |\n|---|---|---|---|---|---|\n| 1 | A | `a` | x | **Modeled by** spec | — |\n| 2 | B | `b` | x | y | **Modeled by** nothing |\n";
    let (rows, findings) = mc::parse_signal_chain(text);
    assert!(findings.is_empty());
    assert!(rows[0].modeled);
    assert!(!rows[1].modeled, "the Character cell does not count");
    assert_eq!(rows[1].identifier, "b");
}

#[test]
fn version_token() {
    // 007 FR-013 (g): a whole token, no look-around needed.
    assert!(mc::changelog_has_version(
        "version **0.5.0** (crates)",
        "0.5.0"
    ));
    assert!(mc::changelog_has_version("- Version 0.5.0\n", "0.5.0"));
    assert!(!mc::changelog_has_version("version 10.5.0 only", "0.5.0"));
    assert!(!mc::changelog_has_version("version 0.5.01 only", "0.5.0"));
    assert_eq!(mc::semver_le("0.4.0", "0.7.0"), Some(true));
    assert_eq!(mc::semver_le("0.7.1", "0.7.0"), Some(false));
    assert_eq!(mc::semver_le("x", "0.7.0"), None);
}

#[test]
fn denylist_word_lists() {
    // 007 FR-011; contracts/cross-check.md › The denylist.
    let positives = [
        "verified",
        "verify",
        "verifies",
        "verifying",
        "verification",
        "detect",
        "detects",
        "detected",
        "detection",
        "detector",
        "sense",
        "senses",
        "sensed",
        "sensing",
        "sensor",
        "anomaly",
        "anomalies",
        "anomalous",
        "spike",
        "spikes",
        "spiking",
        "activity",
        "activities",
        "event",
        "events",
        "entity",
        "entities",
        "ghost",
        "spirits",
        "EVP",
        "paranormal",
        "listening for",
        "signal present",
        "voice from",
        "communication",
    ];
    for w in positives {
        assert!(
            mc::denylist_hit(&format!("a {w} here")).is_some(),
            "{w} should be caught"
        );
    }
    let negatives = [
        "sensitivity",
        "Voice-activated",
        "splice",
        "ceiling",
        "checked against a real unit",
        "VasEvent",
        "evpx",
    ];
    for w in negatives {
        assert!(
            mc::denylist_hit(&format!("a {w} here")).is_none(),
            "{w} should be clean"
        );
    }
}

// ---- The canonical text, the example and the README (research R-05) ----------------------------

#[test]
fn canonical_text_matches_golden() {
    // SC-004; FR-015: the committed rendering; `RR_DR60_BLESS=manifest` rewrites it.
    let path = mc::golden_text_path();
    let want = render(manifest());
    if std::env::var("RR_DR60_BLESS").as_deref() == Ok("manifest") {
        std::fs::write(&path, &want).unwrap();
        println!("blessed {}", path.display());
    }
    let got = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}; bless with RR_DR60_BLESS=manifest", path.display()));
    assert_eq!(got, want, "canonical text differs from {}", path.display());
}

#[test]
fn example_prints_canonical_text() {
    // SC-001: the README's Rust program prints the canonical text.
    let out = Command::new(env!("CARGO"))
        .args([
            "run",
            "-q",
            "-p",
            "rr_dr60_harness",
            "--example",
            "manifest",
        ])
        .current_dir(mc::repo_root())
        .output()
        .expect("cargo run --example manifest");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8(out.stdout).unwrap(), render(manifest()));
}

fn readme() -> String {
    std::fs::read_to_string(mc::repo_root().join("README.md")).unwrap()
}

/// Compares a README region with `want`, blessing it with `RR_DR60_BLESS=readme`.
fn check_readme_region(name: &str, want: &str) {
    let text = readme();
    if std::env::var("RR_DR60_BLESS").as_deref() == Ok("readme") {
        let new = mc::replace_readme_region(&text, name, want)
            .unwrap_or_else(|| panic!("README has no {name} markers"));
        std::fs::write(mc::repo_root().join("README.md"), new).unwrap();
        println!("blessed README region {name}");
    }
    let text = readme();
    let got =
        mc::readme_region(&text, name).unwrap_or_else(|| panic!("README has no {name} markers"));
    assert_eq!(got.trim_end(), want.trim_end(), "README region {name}");
}

#[test]
fn readme_table_matches() {
    // 007 FR-019: the generated fidelity table.
    check_readme_region("manifest-table", &render_readme_table(manifest()));
}

#[test]
fn readme_code_blocks_match() {
    // SC-001: the README's Rust block is the example file, its C block the smoke function.
    let root = mc::repo_root();
    let rust =
        std::fs::read_to_string(root.join("crates/rr_dr60_harness/examples/manifest.rs")).unwrap();
    check_readme_region("manifest-example-rust", &format!("```rust\n{}```", rust));
    let smoke = std::fs::read_to_string(root.join("crates/rr_dr60_ffi/tests/c/smoke.c")).unwrap();
    let c = mc::c_region(&smoke, "manifest-example-c")
        .expect("smoke.c has the manifest-example-c markers");
    check_readme_region("manifest-example-c", &format!("```c\n{}```", c));
}

#[test]
fn repo_root_is_the_workspace() {
    assert!(Path::new(&mc::repo_root()).join("Cargo.toml").exists());
    assert!(
        mc::repo_root()
            .join("docs/hardware/assumptions.md")
            .exists()
    );
}
