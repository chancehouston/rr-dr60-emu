//! The stage manifest's cross-check against the hardware registers (spec 007 FR-013,
//! FR-011, FR-002, FR-003; contracts/cross-check.md), the wording rules, and the renderers
//! that produce the canonical text and the README's generated regions (research R-04, R-05).
//!
//! The cross-check is a pure function of a manifest value and four texts (`sources.md`,
//! `assumptions.md`, `signal-chain.md`, `CHANGELOG.md`), so tests can substitute doctored
//! copies of any of them. It never panics on malformed input: it reports [`Rule::Parse`].
//!
//! Bless switches: `RR_DR60_BLESS=manifest cargo test -p rr_dr60_harness --test us2_cross_check
//! canonical` rewrites `golden/manifest-v1.txt`; `RR_DR60_BLESS=readme … readme -- --test-threads=1`
//! rewrites the README's three marked regions (single-threaded: two tests write the same file). Both are content changes recorded in the CHANGELOG
//! (007 FR-015).

use std::fmt;
use std::path::PathBuf;
use std::sync::OnceLock;

use regex::Regex;
use rr_dr60::{Confidence, Manifest, StageEntry, StagePath, StageStatus};

// 007 FR-011; docs/product/front-and-inside.md › Wording on the inside. Reword the string,
// never this list. Stems are matched with the suffix set in `denylist_regex`.
/// Word stems no manifest string may contain (007 FR-011).
pub const DENYLIST_STEMS: &[&str] = &[
    "verif",
    "detect",
    "sens",
    "anomal",
    "spik",
    "activit",
    "event",
    "paranormal",
    "evp",
    "ghost",
    "spirit",
    "entit",
];
/// Phrases no manifest string may contain (007 FR-011).
pub const DENYLIST_PHRASES: &[&str] = &[
    "listening for",
    "signal present",
    "voice from",
    "communication",
];

// Length limits in characters; engineering targets (007 FR-002, FR-003, FR-007).
const MAX_IDENTIFIER: usize = 24;
const MAX_NAME: usize = 48;
const MAX_DESCRIPTION: usize = 160;
const MAX_COVERAGE: usize = 120;
const MAX_CONTROLS: usize = 120;
const MAX_BASIS: usize = 160;
const MAX_LINK: usize = 200;
/// The signal-chain table's row count (docs/hardware/signal-chain.md).
const STAGE_COUNT: usize = 15;

/// The four texts the cross-check reads.
#[derive(Clone, Copy, Debug)]
pub struct Registers<'a> {
    /// `docs/hardware/sources.md`.
    pub sources: &'a str,
    /// `docs/hardware/assumptions.md`.
    pub assumptions: &'a str,
    /// `docs/hardware/signal-chain.md`.
    pub signal_chain: &'a str,
    /// `CHANGELOG.md`.
    pub changelog: &'a str,
}

/// The repository root (two levels above this crate's manifest).
pub fn repo_root() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    root.canonicalize().unwrap_or(root)
}

/// The committed golden text, `golden/manifest-v1.txt`.
pub fn golden_text_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("golden/manifest-v1.txt")
}

fn leak(path: &str) -> &'static str {
    let p = repo_root().join(path);
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    Box::leak(text.into_boxed_str())
}

/// The four committed texts, read from the repository at call time.
pub fn committed_registers() -> Registers<'static> {
    Registers {
        sources: leak("docs/hardware/sources.md"),
        assumptions: leak("docs/hardware/assumptions.md"),
        signal_chain: leak("docs/hardware/signal-chain.md"),
        changelog: leak("CHANGELOG.md"),
    }
}

/// One clause of 007 FR-013 (contracts/cross-check.md › Rules).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rule {
    /// A register row has the wrong cell count, or a file has no data rows.
    Parse,
    /// (a) A cited ID has no row.
    UnknownId,
    /// (b) A cited assumption is superseded.
    Superseded,
    /// (c) A current assumption or a non-General source is cited by no stage, or an adopted
    /// or revised assumption by no modeled stage.
    Uncited,
    /// (d) The signal-chain table's row count, numbers or identifiers disagree.
    TableShape,
    /// (e) A confidence above its bound.
    ConfidenceBound,
    /// (f) The header's counts disagree with the entries.
    Counts,
    /// (g) A status, note or confidence disagrees with the row's marker and the cited classes.
    StatusDerivation,
    /// (g) A model version that is empty, malformed, above the library's or absent from the
    /// CHANGELOG.
    ModelVersion,
    /// (h) A length or form rule.
    Form,
    /// (h) A denylisted word.
    Denylist,
}

/// One failure of the cross-check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    /// The rule.
    pub rule: Rule,
    /// The stage number, when one is involved.
    pub stage: Option<u8>,
    /// The ID, when one is involved.
    pub id: Option<String>,
    /// What was found.
    pub detail: String,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} ", self.rule)?;
        match self.stage {
            Some(n) => write!(f, "stage {n} ")?,
            None => write!(f, "- ")?,
        }
        match &self.id {
            Some(id) => write!(f, "{id}: ")?,
            None => write!(f, "-: ")?,
        }
        write!(f, "{}", self.detail)
    }
}

fn finding(rule: Rule, stage: Option<u8>, id: Option<&str>, detail: impl Into<String>) -> Finding {
    Finding {
        rule,
        stage,
        id: id.map(str::to_string),
        detail: detail.into(),
    }
}

// ---- Parsers (contracts/cross-check.md › Register shapes) ---------------------------------------

/// Every line that starts with `|`, as `(line number, cells)`: split on `|`, the empty edge
/// cells dropped, each cell trimmed.
pub fn table_rows(text: &str) -> Vec<(usize, Vec<String>)> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| l.starts_with('|'))
        .map(|(i, l)| {
            let inner = l.trim().trim_start_matches('|').trim_end_matches('|');
            (
                i + 1,
                inner.split('|').map(|c| c.trim().to_string()).collect(),
            )
        })
        .collect()
}

/// How an assumption's Status cell classifies (007 FR-004).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StatusClass {
    /// Starts with "superseded".
    Superseded,
    /// Starts with "verified" and names only `S-###` evidence.
    DocumentVerified,
    /// Starts with "measured".
    Measured,
    /// Anything else.
    Open,
}

/// The first High/Medium/Low word in a Confidence cell (007 FR-006).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Headline {
    /// High.
    High,
    /// Medium.
    Medium,
    /// Low.
    Low,
    /// No level word (a superseded row's "—").
    Unknown,
}

fn rank(h: Headline) -> u8 {
    match h {
        Headline::High => 3,
        Headline::Medium => 2,
        Headline::Low | Headline::Unknown => 1,
    }
}

fn confidence_rank(c: Confidence) -> u8 {
    match c {
        Confidence::High => 3,
        Confidence::Medium => 2,
        _ => 1,
    }
}

/// One row of `assumptions.md`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssumptionRow {
    /// `A-###`.
    pub id: String,
    /// The Stage cell (free text; not matched against anything).
    pub stage: String,
    /// The headline confidence.
    pub headline: Headline,
    /// The status class.
    pub class: StatusClass,
    /// Whether the Status cell starts with "adopted" or "revised".
    pub adopted_or_revised: bool,
}

/// One row of `sources.md`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRow {
    /// `S-###`.
    pub id: String,
    /// Whether the Type cell is "General" (exempt from the citation rule, 007 FR-005).
    pub general: bool,
}

/// One row of `signal-chain.md`'s table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainRow {
    /// The `#` cell.
    pub number: u32,
    /// The Identifier cell, backticks stripped.
    pub identifier: String,
    /// Whether the Known or Assumed cell contains `**Modeled by`.
    pub modeled: bool,
}

fn id_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[AS]-\d{3}$").unwrap())
}

fn a_id_anywhere() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"A-\d{3}").unwrap())
}

/// Classifies a Status cell by its first word (007 FR-004) and flags a cell that says
/// "adopted" or "revised" anywhere (hardware review G2b; the register rules say so).
pub fn classify_status(cell: &str) -> (StatusClass, bool) {
    static ADOPTED: OnceLock<Regex> = OnceLock::new();
    let lower = cell.trim().to_lowercase();
    let flag = ADOPTED
        .get_or_init(|| Regex::new(r"\b(adopted|revised)\b").unwrap())
        .is_match(&lower);
    let class = if lower.starts_with("superseded") {
        StatusClass::Superseded
    } else if lower.starts_with("verified") && !a_id_anywhere().is_match(cell) {
        StatusClass::DocumentVerified
    } else if lower.starts_with("measured") {
        StatusClass::Measured
    } else {
        StatusClass::Open
    };
    (class, flag)
}

/// A Status cell the classifier would read differently from what its author meant (hardware
/// review G2c, G2d): `superseded` anywhere but first (the row would stay current), or a
/// `verified` cell that speaks of a unit, a capture or a measurement (a measurement is
/// `measured`, never `verified`). `None` when the cell is unambiguous.
pub fn status_cell_problem(cell: &str) -> Option<String> {
    static UNIT: OnceLock<Regex> = OnceLock::new();
    let lower = cell.trim().to_lowercase();
    if lower.contains("superseded") && !lower.starts_with("superseded") {
        return Some("'superseded' is not the first word, so the row would stay current".into());
    }
    let unit = UNIT.get_or_init(|| Regex::new(r"\b(unit|capture|measur\w*)\b").unwrap());
    if lower.starts_with("verified") && unit.is_match(&lower) {
        return Some(
            "a 'verified' cell names a unit, capture or measurement; write 'measured'".into(),
        );
    }
    None
}

/// The first High/Medium/Low word in a Confidence cell (007 FR-006).
pub fn headline_confidence(cell: &str) -> Headline {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?i)\b(high|medium|low)\b").unwrap());
    match re.find(cell).map(|m| m.as_str().to_ascii_lowercase()) {
        Some(w) if w == "high" => Headline::High,
        Some(w) if w == "medium" => Headline::Medium,
        Some(_) => Headline::Low,
        None => Headline::Unknown,
    }
}

/// Parses `assumptions.md`: rows whose first cell is `A-###`, seven cells each.
pub fn parse_assumptions(text: &str) -> (Vec<AssumptionRow>, Vec<Finding>) {
    let mut rows = Vec::new();
    let mut findings = Vec::new();
    for (line, cells) in table_rows(text) {
        if !cells
            .first()
            .is_some_and(|c| c.starts_with("A-") && id_regex().is_match(c))
        {
            continue;
        }
        if cells.len() != 7 {
            findings.push(finding(
                Rule::Parse,
                None,
                Some(&cells[0]),
                format!(
                    "assumptions.md line {line}: {} cells, expected 7",
                    cells.len()
                ),
            ));
            continue;
        }
        if let Some(problem) = status_cell_problem(&cells[6]) {
            findings.push(finding(
                Rule::Parse,
                None,
                Some(&cells[0]),
                format!("assumptions.md line {line}: ambiguous Status cell: {problem}"),
            ));
        }
        let (class, adopted_or_revised) = classify_status(&cells[6]);
        rows.push(AssumptionRow {
            id: cells[0].clone(),
            stage: cells[1].clone(),
            headline: headline_confidence(&cells[4]),
            class,
            adopted_or_revised,
        });
    }
    if rows.is_empty() {
        findings.push(finding(
            Rule::Parse,
            None,
            None,
            "assumptions.md: no data rows",
        ));
    }
    (rows, findings)
}

/// Parses `sources.md`: rows whose first cell is `S-###`, five cells each.
pub fn parse_sources(text: &str) -> (Vec<SourceRow>, Vec<Finding>) {
    let mut rows = Vec::new();
    let mut findings = Vec::new();
    for (line, cells) in table_rows(text) {
        if !cells
            .first()
            .is_some_and(|c| c.starts_with("S-") && id_regex().is_match(c))
        {
            continue;
        }
        if cells.len() != 5 {
            findings.push(finding(
                Rule::Parse,
                None,
                Some(&cells[0]),
                format!("sources.md line {line}: {} cells, expected 5", cells.len()),
            ));
            continue;
        }
        rows.push(SourceRow {
            id: cells[0].clone(),
            general: cells[2].eq_ignore_ascii_case("general"),
        });
    }
    if rows.is_empty() {
        findings.push(finding(Rule::Parse, None, None, "sources.md: no data rows"));
    }
    (rows, findings)
}

/// Parses `signal-chain.md`'s table: rows whose first cell is an integer, six cells each.
pub fn parse_signal_chain(text: &str) -> (Vec<ChainRow>, Vec<Finding>) {
    let mut rows = Vec::new();
    let mut findings = Vec::new();
    for (line, cells) in table_rows(text) {
        let Some(number) = cells.first().and_then(|c| c.parse::<u32>().ok()) else {
            continue;
        };
        if cells.len() != 6 {
            findings.push(finding(
                Rule::Parse,
                None,
                None,
                format!(
                    "signal-chain.md line {line}: {} cells, expected 6",
                    cells.len()
                ),
            ));
            continue;
        }
        rows.push(ChainRow {
            number,
            identifier: cells[2].trim_matches('`').to_string(),
            modeled: cells[3].contains("**Modeled by") || cells[4].contains("**Modeled by"),
        });
    }
    if rows.is_empty() {
        findings.push(finding(
            Rule::Parse,
            None,
            None,
            "signal-chain.md: no data rows",
        ));
    }
    (rows, findings)
}

/// Whether `version` (`X.Y.Z`) occurs in the CHANGELOG as a whole token (007 FR-013 (g)).
pub fn changelog_has_version(text: &str, version: &str) -> bool {
    let pattern = format!("(?m)(^|[^0-9.]){}([^0-9.]|$)", regex::escape(version));
    Regex::new(&pattern)
        .map(|re| re.is_match(text))
        .unwrap_or(false)
}

fn semver(s: &str) -> Option<(u64, u64, u64)> {
    let mut it = s.split('.');
    let a = it.next()?.parse().ok()?;
    let b = it.next()?.parse().ok()?;
    let c = it.next()?.parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some((a, b, c))
}

/// `a <= b` for two `X.Y.Z` strings; `None` if either is malformed.
pub fn semver_le(a: &str, b: &str) -> Option<bool> {
    Some(semver(a)? <= semver(b)?)
}

// ---- The denylist (007 FR-011) ------------------------------------------------------------------

fn denylist_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        let stems = DENYLIST_STEMS
            .iter()
            .map(|s| {
                format!(
                    r"\b{}(e|es|s|ed|ied|ies|y|ying|ing|ion|ions|ication|ications|or|ors|ous|ive)?\b",
                    regex::escape(s)
                )
            })
            .collect::<Vec<_>>();
        let phrases = DENYLIST_PHRASES
            .iter()
            .map(|p| format!(r"\b{}\b", regex::escape(p)))
            .collect::<Vec<_>>();
        let all = stems.into_iter().chain(phrases).collect::<Vec<_>>().join("|");
        Regex::new(&format!("(?i)({all})")).unwrap()
    })
}

/// The first denylisted word or phrase in `s`, if any.
pub fn denylist_hit(s: &str) -> Option<String> {
    denylist_regex().find(s).map(|m| m.as_str().to_string())
}

// ---- Wording and form (007 FR-002, FR-003, FR-011; FR-013 (h)) ----------------------------------

fn chars(s: &str) -> usize {
    s.chars().count()
}

fn check_len(out: &mut Vec<Finding>, stage: Option<u8>, field: &str, s: &str, max: usize) {
    if chars(s) > max {
        out.push(finding(
            Rule::Form,
            stage,
            None,
            format!("{field}: {} characters, at most {max}", chars(s)),
        ));
    }
}

fn check_denylist(out: &mut Vec<Finding>, stage: Option<u8>, field: &str, s: &str) {
    if let Some(w) = denylist_hit(s) {
        out.push(finding(
            Rule::Denylist,
            stage,
            None,
            format!("{field} contains {w:?}: {s:?}"),
        ));
    }
}

fn ids_ok(list: &[&str]) -> bool {
    list.iter().all(|id| id_regex().is_match(id)) && list.windows(2).all(|w| w[0] < w[1])
}

/// The form and wording findings of 007 FR-002, FR-003 and FR-011 (rule h).
pub fn wording_findings(m: &Manifest) -> Vec<Finding> {
    let mut out = Vec::new();
    let h = &m.header;
    check_len(&mut out, None, "basis", h.basis, MAX_BASIS);
    for (name, link) in [
        ("evidence_link", h.evidence_link),
        ("assumptions_link", h.assumptions_link),
        ("sources_link", h.sources_link),
    ] {
        check_len(&mut out, None, name, link, MAX_LINK);
        if !link.starts_with("https://") {
            out.push(finding(
                Rule::Form,
                None,
                None,
                format!("{name} is not https: {link}"),
            ));
        }
    }
    for (name, s) in [
        ("library_version", h.library_version),
        ("basis", h.basis),
        ("evidence_link", h.evidence_link),
        ("assumptions_link", h.assumptions_link),
        ("sources_link", h.sources_link),
    ] {
        check_denylist(&mut out, None, name, s);
    }
    let ident_re = Regex::new(r"^[a-z0-9_]{1,24}$").unwrap();
    let mut seen: Vec<&str> = Vec::new();
    for e in m.stages() {
        let n = Some(e.number);
        if !ident_re.is_match(e.identifier) || chars(e.identifier) > MAX_IDENTIFIER {
            out.push(finding(
                Rule::Form,
                n,
                None,
                format!("identifier {:?}", e.identifier),
            ));
        }
        if seen.contains(&e.identifier) {
            out.push(finding(
                Rule::Form,
                n,
                None,
                format!("duplicate identifier {:?}", e.identifier),
            ));
        }
        seen.push(e.identifier);
        check_len(&mut out, n, "name", e.name, MAX_NAME);
        check_len(&mut out, n, "description", e.description, MAX_DESCRIPTION);
        if !e.description.ends_with('.') {
            out.push(finding(
                Rule::Form,
                n,
                None,
                "description does not end with '.'",
            ));
        }
        if e.description.contains(". ") {
            out.push(finding(
                Rule::Form,
                n,
                None,
                "description is more than one sentence",
            ));
        }
        check_len(&mut out, n, "coverage", e.coverage, MAX_COVERAGE);
        check_len(&mut out, n, "controls", e.controls, MAX_CONTROLS);
        if !ids_ok(e.sources) || !e.sources.iter().all(|s| s.starts_with("S-")) {
            out.push(finding(Rule::Form, n, None, "sources not S-### ascending"));
        }
        if !ids_ok(e.assumptions) || !e.assumptions.iter().all(|s| s.starts_with("A-")) {
            out.push(finding(
                Rule::Form,
                n,
                None,
                "assumptions not A-### ascending",
            ));
        }
        for (name, s) in [
            ("identifier", e.identifier),
            ("name", e.name),
            ("description", e.description),
            ("coverage", e.coverage),
            ("controls", e.controls),
            ("model_version", e.model_version),
        ] {
            check_denylist(&mut out, n, name, s);
        }
    }
    out
}

// ---- The cross-check (007 FR-013 (a)–(h)) -------------------------------------------------------

fn modeled(e: &StageEntry) -> bool {
    e.status != StageStatus::NotModeled
}

/// Runs every clause of 007 FR-013 on `m` against the texts in `r`; empty means pass.
pub fn cross_check(m: &Manifest, r: &Registers<'_>) -> Vec<Finding> {
    let mut out = Vec::new();
    let (assumptions, f) = parse_assumptions(r.assumptions);
    out.extend(f);
    let (sources, f) = parse_sources(r.sources);
    out.extend(f);
    let (chain, f) = parse_signal_chain(r.signal_chain);
    out.extend(f);

    let assumption = |id: &str| assumptions.iter().find(|a| a.id == id);
    let source = |id: &str| sources.iter().find(|s| s.id == id);

    // (a), (b)
    for e in m.stages() {
        for id in e.sources {
            if source(id).is_none() {
                out.push(finding(
                    Rule::UnknownId,
                    Some(e.number),
                    Some(id),
                    "no such source",
                ));
            }
        }
        for id in e.assumptions {
            match assumption(id) {
                None => out.push(finding(
                    Rule::UnknownId,
                    Some(e.number),
                    Some(id),
                    "no such assumption",
                )),
                Some(a) if a.class == StatusClass::Superseded => {
                    out.push(finding(
                        Rule::Superseded,
                        Some(e.number),
                        Some(id),
                        "cites a superseded assumption",
                    ));
                }
                Some(_) => {}
            }
        }
    }

    // (c)
    let cited_by = |id: &str, only_modeled: bool| {
        m.stages()
            .iter()
            .filter(|e| !only_modeled || modeled(e))
            .any(|e| e.sources.contains(&id) || e.assumptions.contains(&id))
    };
    for a in assumptions
        .iter()
        .filter(|a| a.class != StatusClass::Superseded)
    {
        if !cited_by(&a.id, false) {
            out.push(finding(
                Rule::Uncited,
                None,
                Some(&a.id),
                "a current assumption cited by no stage",
            ));
        } else if a.adopted_or_revised && !cited_by(&a.id, true) {
            out.push(finding(
                Rule::Uncited,
                None,
                Some(&a.id),
                "adopted or revised but cited by no modeled stage",
            ));
        }
    }
    for s in sources.iter().filter(|s| !s.general) {
        if !cited_by(&s.id, false) {
            out.push(finding(
                Rule::Uncited,
                None,
                Some(&s.id),
                "a source cited by no stage",
            ));
        }
    }

    // (d)
    if chain.len() != STAGE_COUNT {
        out.push(finding(
            Rule::TableShape,
            None,
            None,
            format!(
                "signal-chain.md has {} data rows, expected {STAGE_COUNT}",
                chain.len()
            ),
        ));
    }
    for (i, e) in m.stages().iter().enumerate() {
        match chain.get(i) {
            Some(row) if row.number != u32::from(e.number) => out.push(finding(
                Rule::TableShape,
                Some(e.number),
                None,
                format!("row {} is numbered {}", i + 1, row.number),
            )),
            Some(row) if row.identifier != e.identifier => out.push(finding(
                Rule::TableShape,
                Some(e.number),
                None,
                format!(
                    "table identifier {:?} != manifest {:?}",
                    row.identifier, e.identifier
                ),
            )),
            _ => {}
        }
    }

    // (e)
    for e in m.stages().iter().filter(|e| modeled(e)) {
        let bound = e
            .assumptions
            .iter()
            .filter_map(|id| assumption(id))
            .filter(|a| a.class != StatusClass::Superseded)
            .map(|a| rank(a.headline))
            .min()
            .unwrap_or(3);
        if confidence_rank(e.confidence) > bound {
            out.push(finding(
                Rule::ConfidenceBound,
                Some(e.number),
                None,
                format!(
                    "{:?} exceeds the headline bound (rank {bound})",
                    e.confidence
                ),
            ));
        }
    }

    // (f)
    let mut tally = [0u32; 4];
    for e in m.stages() {
        let i = match e.status {
            StageStatus::NotModeled => 0,
            StageStatus::Assumed => 1,
            StageStatus::Sourced => 2,
            StageStatus::Checked => 3,
            _ => 0,
        };
        tally[i] += 1;
    }
    let c = m.header.counts;
    if [c.not_modeled, c.assumed, c.sourced, c.checked] != tally
        || m.header.stage_count != STAGE_COUNT as u32
    {
        out.push(finding(
            Rule::Counts,
            None,
            None,
            format!("header counts {c:?} vs tally {tally:?}"),
        ));
    }

    // (g)
    for e in m.stages() {
        let n = Some(e.number);
        let Some(row) = chain.iter().find(|r| r.number == u32::from(e.number)) else {
            continue; // reported by (d)
        };
        if !row.modeled {
            if modeled(e) {
                out.push(finding(
                    Rule::StatusDerivation,
                    n,
                    None,
                    "the table row has no Modeled-by marker but the manifest says modeled",
                ));
            }
            if !e.model_version.is_empty()
                || !e.coverage.is_empty()
                || e.confidence != Confidence::NotApplicable
            {
                out.push(finding(
                    Rule::StatusDerivation,
                    n,
                    None,
                    "not modeled, but carries a model version, a coverage note or a confidence",
                ));
            }
            continue;
        }
        if !modeled(e) {
            out.push(finding(
                Rule::StatusDerivation,
                n,
                None,
                "the table row is marked Modeled by but the manifest says not modeled",
            ));
            continue;
        }
        if e.controls.is_empty() {
            out.push(finding(
                Rule::StatusDerivation,
                n,
                None,
                "modeled but no controls note",
            ));
        }
        let classes: Vec<StatusClass> = e
            .assumptions
            .iter()
            .map(|id| assumption(id).map_or(StatusClass::Open, |a| a.class))
            .collect();
        let any_open = classes
            .iter()
            .any(|c| matches!(c, StatusClass::Open | StatusClass::Superseded));
        let any_measured = classes.contains(&StatusClass::Measured);
        let derived = if any_open {
            StageStatus::Assumed
        } else if any_measured {
            StageStatus::Checked
        } else {
            StageStatus::Sourced
        };
        if e.status != derived {
            out.push(finding(
                Rule::StatusDerivation,
                n,
                None,
                format!("status {:?}, derived {derived:?}", e.status),
            ));
        }
        if e.model_version.is_empty() {
            out.push(finding(
                Rule::ModelVersion,
                n,
                None,
                "modeled but no model version",
            ));
        } else {
            match semver_le(e.model_version, rr_dr60::VERSION) {
                None => out.push(finding(
                    Rule::ModelVersion,
                    n,
                    None,
                    format!("malformed {:?}", e.model_version),
                )),
                Some(false) => out.push(finding(
                    Rule::ModelVersion,
                    n,
                    None,
                    format!(
                        "{} is above the library's {}",
                        e.model_version,
                        rr_dr60::VERSION
                    ),
                )),
                Some(true) => {}
            }
            if !changelog_has_version(r.changelog, e.model_version) {
                out.push(finding(
                    Rule::ModelVersion,
                    n,
                    None,
                    format!("{} is not in the CHANGELOG", e.model_version),
                ));
            }
        }
    }

    // (h)
    out.extend(wording_findings(m));
    out
}

// ---- Renderers (research R-05; data-model.md › Canonical rendering) ----------------------------

/// The status as a lowercase snake word, for the canonical text.
pub fn status_word(s: StageStatus) -> &'static str {
    match s {
        StageStatus::NotModeled => "not_modeled",
        StageStatus::Assumed => "assumed",
        StageStatus::Sourced => "sourced",
        StageStatus::Checked => "checked",
        _ => "unknown",
    }
}

/// The path as a lowercase word.
pub fn path_word(p: StagePath) -> &'static str {
    match p {
        StagePath::Record => "record",
        StagePath::Playback => "playback",
        StagePath::Both => "both",
        _ => "unknown",
    }
}

/// The confidence as a lowercase snake word.
pub fn confidence_word(c: Confidence) -> &'static str {
    match c {
        Confidence::NotApplicable => "not_applicable",
        Confidence::Low => "low",
        Confidence::Medium => "medium",
        Confidence::High => "high",
        _ => "unknown",
    }
}

fn line(out: &mut String, key: &str, value: &str) {
    out.push_str(key);
    out.push(':');
    if !value.is_empty() {
        out.push(' ');
        out.push_str(value);
    }
    out.push('\n');
}

/// The canonical text (data-model.md): what `examples/manifest.rs` and `smoke --manifest` print.
pub fn render(m: &Manifest) -> String {
    let h = &m.header;
    let mut out = String::from("rr_dr60 manifest\n");
    line(&mut out, "library_version", h.library_version);
    line(&mut out, "device_rate_hz", &h.device_rate_hz.to_string());
    line(&mut out, "stage_count", &h.stage_count.to_string());
    line(
        &mut out,
        "counts",
        &format!(
            "not_modeled={} assumed={} sourced={} checked={}",
            h.counts.not_modeled, h.counts.assumed, h.counts.sourced, h.counts.checked
        ),
    );
    line(&mut out, "basis", h.basis);
    line(&mut out, "evidence_link", h.evidence_link);
    line(&mut out, "assumptions_link", h.assumptions_link);
    line(&mut out, "sources_link", h.sources_link);
    for s in m.stages() {
        out.push('\n');
        out.push_str(&format!("stage {} {}\n", s.number, s.identifier));
        line(&mut out, "name", s.name);
        line(&mut out, "description", s.description);
        line(&mut out, "status", status_word(s.status));
        line(&mut out, "path", path_word(s.path));
        line(&mut out, "confidence", confidence_word(s.confidence));
        line(&mut out, "sources", &s.sources.join(", "));
        line(&mut out, "assumptions", &s.assumptions.join(", "));
        line(&mut out, "coverage", s.coverage);
        line(&mut out, "controls", s.controls);
        line(&mut out, "model_version", s.model_version);
    }
    out
}

/// The README's generated fidelity table (007 FR-019), in the manifest's vocabulary.
pub fn render_readme_table(m: &Manifest) -> String {
    let mut out = String::from(
        "| # | Stage | Status | Confidence | Basis | Not modeled here | Model version |\n|---|---|---|---|---|---|---|\n",
    );
    for s in m.stages() {
        let status = match s.status {
            StageStatus::NotModeled => "not modeled",
            StageStatus::Assumed => "assumed",
            StageStatus::Sourced => "sourced",
            StageStatus::Checked => "checked against a real unit",
            _ => "unknown",
        };
        let confidence = match s.confidence {
            Confidence::NotApplicable => "—",
            c => confidence_word(c),
        };
        let basis = format!("{}; {}", s.sources.join(", "), s.assumptions.join(", "));
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            s.number, s.name, status, confidence, basis, s.coverage, s.model_version
        ));
    }
    out
}

/// The text between `<!-- name:start -->` and `<!-- name:end -->` lines in a Markdown file
/// (the marker lines excluded), or `None` without both markers.
pub fn readme_region<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let start = format!("<!-- {name}:start -->");
    let end = format!("<!-- {name}:end -->");
    let s = text.find(&start)? + start.len();
    let e = text[s..].find(&end)? + s;
    Some(text[s..e].trim_start_matches('\n'))
}

/// `text` with the region's body replaced by `body` (the markers kept on their own lines).
pub fn replace_readme_region(text: &str, name: &str, body: &str) -> Option<String> {
    let start = format!("<!-- {name}:start -->");
    let end = format!("<!-- {name}:end -->");
    let s = text.find(&start)? + start.len();
    let e = text[s..].find(&end)? + s;
    let mut out = String::with_capacity(text.len() + body.len());
    out.push_str(&text[..s]);
    out.push('\n');
    out.push_str(body.trim_end_matches('\n'));
    out.push('\n');
    out.push_str(&text[e..]);
    Some(out)
}

/// The text between `/* name:start */` and `/* name:end */` lines in a C file.
pub fn c_region<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let start = format!("/* {name}:start */");
    let end = format!("/* {name}:end */");
    let s = text.find(&start)? + start.len();
    let e = text[s..].find(&end)? + s;
    Some(text[s..e].trim_start_matches('\n'))
}

#[cfg(test)]
mod tests {
    //! The branches the story tests do not reach: parser shape failures, every rule's
    //! negative path, the renderers' rarer arms and the region helpers (007 SC-008).

    use super::*;
    use rr_dr60::manifest;

    const LONG: &str = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx.";
    const LONG_LINK: &str = "https://example.invalid/xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";

    fn rules(f: &[Finding]) -> Vec<Rule> {
        f.iter().map(|x| x.rule).collect()
    }

    fn has(f: &[Finding], rule: Rule, stage: Option<u8>) -> bool {
        f.iter().any(|x| x.rule == rule && x.stage == stage)
    }

    /// Spec 008 T033 (US3 scenario 2): a copy of the committed assumption register without
    /// A-040 fails the cross-check naming stage 7 (and stage 9) with the missing ID.
    #[test]
    fn doctored_register_without_a040_names_stage_7() {
        let r = committed_registers();
        let before = r.assumptions.lines().count();
        let assumptions: Vec<&str> = r
            .assumptions
            .lines()
            .filter(|l| !l.starts_with("| A-040 |"))
            .collect();
        assert_eq!(assumptions.len() + 1, before, "exactly one A-040 row");
        let assumptions = assumptions.join("\n");
        let f = cross_check(
            manifest(),
            &Registers {
                assumptions: &assumptions,
                ..r
            },
        );
        for stage in [7u8, 9] {
            assert!(
                f.iter().any(|x| x.rule == Rule::UnknownId
                    && x.stage == Some(stage)
                    && x.id.as_deref() == Some("A-040")),
                "stage {stage}: {f:?}"
            );
        }
        assert!(cross_check(manifest(), &r).is_empty());
    }

    #[test]
    fn finding_display() {
        let a = finding(Rule::Parse, None, None, "no rows");
        assert_eq!(a.to_string(), "Parse - -: no rows");
        let b = finding(
            Rule::UnknownId,
            Some(3),
            Some("A-999"),
            "no such assumption",
        );
        assert_eq!(b.to_string(), "UnknownId stage 3 A-999: no such assumption");
    }

    #[test]
    fn ranks() {
        assert_eq!(rank(Headline::Unknown), 1);
        assert_eq!(rank(Headline::Low), 1);
        assert_eq!(rank(Headline::Medium), 2);
        assert_eq!(rank(Headline::High), 3);
        assert_eq!(confidence_rank(Confidence::NotApplicable), 1);
        assert_eq!(confidence_rank(Confidence::High), 3);
    }

    #[test]
    fn parsers_report_shape_failures() {
        let (rows, f) = parse_assumptions("| A-001 | x | y |\n");
        assert!(rows.is_empty());
        assert_eq!(rules(&f), vec![Rule::Parse, Rule::Parse]);
        assert_eq!(f[0].id.as_deref(), Some("A-001"));
        let (_, f) = parse_sources("| S-001 | x |\n");
        assert_eq!(rules(&f), vec![Rule::Parse, Rule::Parse]);
        let (_, f) = parse_signal_chain("| 1 | x |\n");
        assert_eq!(rules(&f), vec![Rule::Parse, Rule::Parse]);
        let (_, f) = parse_assumptions("nothing here");
        assert_eq!(rules(&f), vec![Rule::Parse]);
    }

    #[test]
    fn semver_edges() {
        assert_eq!(semver_le("1.2.3.4", "1.2.3"), None);
        assert_eq!(semver_le("1.2", "1.2.3"), None);
        assert_eq!(semver_le("a.b.c", "1.2.3"), None);
        assert_eq!(semver_le("1.2.3", "1.2.3"), Some(true));
    }

    #[test]
    fn wording_negatives() {
        let base = *manifest();
        type Mutation = Box<dyn Fn(&mut Manifest)>;
        let cases: Vec<(&str, Mutation)> = vec![
            (
                "identifier",
                Box::new(|m| m.stages[0].identifier = "Bad-Id"),
            ),
            (
                "duplicate identifier",
                Box::new(|m| m.stages[1].identifier = "mic_capsule"),
            ),
            ("name", Box::new(|m| m.stages[0].name = LONG)),
            ("no dot", Box::new(|m| m.stages[0].description = "No dot")),
            (
                "two sentences",
                Box::new(|m| m.stages[0].description = "One. Two."),
            ),
            ("coverage", Box::new(|m| m.stages[2].coverage = LONG)),
            ("controls", Box::new(|m| m.stages[2].controls = LONG)),
            (
                "sources",
                Box::new(|m| m.stages[0].sources = &["S-002", "S-001"]),
            ),
            (
                "assumptions",
                Box::new(|m| m.stages[0].assumptions = &["A-1"]),
            ),
            ("basis", Box::new(|m| m.header.basis = LONG)),
            (
                "link scheme",
                Box::new(|m| m.header.evidence_link = "http://example.invalid/"),
            ),
            (
                "link length",
                Box::new(|m| m.header.sources_link = LONG_LINK),
            ),
        ];
        for (label, mutate) in cases {
            let mut m = base;
            mutate(&mut m);
            let f = wording_findings(&m);
            assert!(f.iter().any(|x| x.rule == Rule::Form), "{label}: {f:?}");
        }
        let mut m = base;
        m.header.basis = "This was verified on a unit.";
        assert!(has(&wording_findings(&m), Rule::Denylist, None));
        let mut m = base;
        m.stages[4].name = "VAS event gate";
        assert!(has(&wording_findings(&m), Rule::Denylist, Some(5)));
    }

    #[test]
    fn cross_check_negatives() {
        let r = committed_registers();
        let base = *manifest();

        let mut m = base;
        m.header.counts.assumed += 1;
        assert!(has(&cross_check(&m, &r), Rule::Counts, None));

        // "0.0.9" relies on that token never appearing in the CHANGELOG.
        for (label, version, detail) in [
            ("empty", "", "no model version"),
            ("malformed", "x", "malformed"),
            ("above", "9.9.9", "above the library's"),
            ("absent", "0.0.9", "not in the CHANGELOG"),
        ] {
            let mut m = base;
            m.stages[2].model_version = version;
            let f = cross_check(&m, &r);
            assert!(
                f.iter().any(|x| x.rule == Rule::ModelVersion
                    && x.stage == Some(3)
                    && x.detail.contains(detail)),
                "{label}: {f:?}"
            );
        }

        let mut m = base;
        m.stages[0].coverage = "x";
        assert!(has(&cross_check(&m, &r), Rule::StatusDerivation, Some(1)));
        let mut m = base;
        m.stages[2].controls = "";
        assert!(has(&cross_check(&m, &r), Rule::StatusDerivation, Some(3)));
        let mut m = base;
        m.stages[2].status = StageStatus::Sourced;
        assert!(has(&cross_check(&m, &r), Rule::StatusDerivation, Some(3)));

        // An adopted or revised assumption cited only by a stage that is not modeled.
        let mut m = base;
        m.stages[2].status = StageStatus::NotModeled;
        let f = cross_check(&m, &r);
        assert!(
            f.iter()
                .any(|x| x.rule == Rule::Uncited && x.id.as_deref() == Some("A-028")),
            "{f:?}"
        );

        // An assumption row removed: UnknownId for the citing stage.
        let assumptions: Vec<&str> = r
            .assumptions
            .lines()
            .filter(|l| !l.starts_with("| A-005 |"))
            .collect();
        let assumptions = assumptions.join("\n");
        let f = cross_check(
            &base,
            &Registers {
                assumptions: &assumptions,
                ..r
            },
        );
        assert!(f.iter().any(|x| x.rule == Rule::UnknownId
            && x.stage == Some(1)
            && x.id.as_deref() == Some("A-005")));

        // A row numbered out of order.
        let chain = r
            .signal_chain
            .replacen("| 2 | Mic amp |", "| 3 | Mic amp |", 1);
        assert_ne!(chain, r.signal_chain);
        assert!(has(
            &cross_check(
                &base,
                &Registers {
                    signal_chain: &chain,
                    ..r
                }
            ),
            Rule::TableShape,
            Some(2)
        ));
    }

    #[test]
    fn readme_table_words_for_every_status() {
        let mut m = *manifest();
        m.stages[2].status = StageStatus::Sourced;
        m.stages[3].status = StageStatus::Checked;
        let table = render_readme_table(&m);
        assert!(table.contains("| sourced |"));
        assert!(table.contains("| checked against a real unit |"));
        assert!(table.contains("| — |"));
        assert_eq!(table.lines().count(), 17);
    }

    #[test]
    fn region_helpers() {
        let text = "a\n<!-- x:start -->\nold\n<!-- x:end -->\nb\n";
        assert_eq!(readme_region(text, "x"), Some("old\n"));
        assert_eq!(readme_region(text, "y"), None);
        let new = replace_readme_region(text, "x", "new\n\n").unwrap();
        assert_eq!(new, "a\n<!-- x:start -->\nnew\n<!-- x:end -->\nb\n");
        assert_eq!(replace_readme_region(text, "y", "new"), None);
        let c = "int a;\n/* r:start */\nvoid f(void) {}\n/* r:end */\n";
        assert_eq!(c_region(c, "r"), Some("void f(void) {}\n"));
        assert_eq!(c_region(c, "q"), None);
        assert_eq!(c_region("no markers", "r"), None);
    }

    #[test]
    fn denylist_phrases_and_words() {
        assert_eq!(
            denylist_hit("we are listening for it").as_deref(),
            Some("listening for")
        );
        assert_eq!(denylist_hit("a quiet room"), None);
        assert_eq!(status_word(StageStatus::Checked), "checked");
        assert_eq!(path_word(StagePath::Both), "both");
        assert_eq!(confidence_word(Confidence::High), "high");
    }

    #[test]
    fn status_cell_problems() {
        // Hardware review G2c, G2d.
        assert!(status_cell_problem("measured (capture 3); superseded by A-050").is_some());
        assert!(status_cell_problem("verified against a unit (capture 2)").is_some());
        assert!(status_cell_problem("verified by measurement").is_some());
        assert!(status_cell_problem("superseded by A-026 (spec 004)").is_none());
        assert!(status_cell_problem("assumed; modeled on the circuit, not measured").is_none());
        assert!(status_cell_problem("verified (S-002); adopted by spec 004").is_none());
        let (_, f) = parse_assumptions(
            "| ID | Stage | Assumption | Rationale | Confidence | How to verify | Status |\n|---|---|---|---|---|---|---|\n| A-900 | X | a | b | Low | c | measured; superseded by A-901 |\n",
        );
        assert_eq!(rules(&f), vec![Rule::Parse]);
        assert_eq!(f[0].id.as_deref(), Some("A-900"));
    }
    /// Spec 009 T022 (FR-015): a copy of the register with A-042 removed fails the cross-check
    /// naming stages 12 and 13 (both cite it), and a copy with A-043's confidence cell reordered
    /// to read "Low" first fails naming stage 11 (the headline rule: a row's confidence may not
    /// exceed the lowest headline of what it cites).
    #[test]
    fn doctored_output_registers() {
        let r = committed_registers();
        let m = manifest();
        let without: String = r
            .assumptions
            .lines()
            .filter(|l| !l.starts_with("| A-042 "))
            .collect::<Vec<_>>()
            .join("\n");
        let f = cross_check(
            m,
            &Registers {
                assumptions: &without,
                ..r
            },
        );
        for n in [12u8, 13] {
            assert!(
                f.iter().any(|x| x.rule == Rule::UnknownId
                    && x.stage == Some(n)
                    && x.id.as_deref() == Some("A-042")),
                "stage {n}: {f:?}"
            );
        }
        let reordered: String = r
            .assumptions
            .lines()
            .map(|l| {
                if l.starts_with("| A-043 ") {
                    l.replace(
                        "Medium (engine convention) / Low (as a typical setting)",
                        "Low (as a typical setting) / Medium (engine convention)",
                    )
                } else {
                    l.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_ne!(
            reordered, r.assumptions,
            "A-043's cell carries the stated order"
        );
        let f = cross_check(
            m,
            &Registers {
                assumptions: &reordered,
                ..r
            },
        );
        assert!(
            f.iter()
                .any(|x| x.rule == Rule::ConfidenceBound && x.stage == Some(11)),
            "{f:?}"
        );
    }
}
