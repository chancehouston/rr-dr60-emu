# Contract: the register cross-check (feature 007, FR-013; research R-04)

The cross-check is a pure function of a manifest value and four texts. It lives in the harness (`rr_dr60_harness::manifest_checks`), never in the core. This contract fixes what the registers must look like for the check to read them, so that a register edit knows the rules.

## Inputs

```text
pub struct Registers<'a> { pub sources: &'a str, pub assumptions: &'a str, pub signal_chain: &'a str, pub changelog: &'a str }
pub fn committed_registers() -> Registers<'static>;            // reads the four files under the repository root
pub fn cross_check(m: &rr_dr60::Manifest, r: &Registers) -> Vec<Finding>;   // empty == pass
pub fn wording_findings(m: &rr_dr60::Manifest) -> Vec<Finding>; // FR-002/FR-003 form, FR-011 denylist (also run by cross_check as rule h)
pub fn render(m: &rr_dr60::Manifest) -> String;                 // the canonical text (data-model.md)
pub fn render_readme_table(m: &rr_dr60::Manifest) -> String;    // the README's generated table
```

## Register shapes the parsers rely on

- A table row is a line that starts with `|`. Cells are split on `|` and trimmed; **no cell may contain a literal `|`** (none does today; a row with an unexpected cell count is a `Parse` finding naming the line).
- **assumptions.md**: data rows have a first cell matching `^A-\d{3}$` and exactly seven cells: ID, Stage, Assumption, Rationale / sources, Confidence, How to verify, Status.
  - Status class: lowercase the cell and trim; starts with `superseded` → Superseded; starts with `verified` and the cell contains no `A-\d{3}` → DocumentVerified; starts with `measured` → Measured; otherwise Open. Flag adopted-or-revised when the cell starts with `adopted` or `revised`.
  - Headline confidence: the first `(?i)\b(high|medium|low)\b` in the Confidence cell ("Low to medium" → Low; "High (topology) / Low (gain)" → High); none → Unknown (only superseded rows today).
- **sources.md**: data rows `^S-\d{3}$` with five cells: ID, Source, Type, Reliability, Key facts. A Type cell equal to `General` (case-insensitive) exempts the source from the citation rule.
- **signal-chain.md**: data rows whose first cell is an integer, with six cells: #, Stage, Identifier, Known, Assumed, Character it contributes. The Identifier cell is the bare identifier, optionally in backticks. A row is **modeled** when the Known or Assumed cell contains the text `**Modeled by`. Exactly fifteen such rows.
- **CHANGELOG.md**: free text; a model version `X.Y.Z` is present when it occurs as a whole token (`(?<![\d.])X\.Y\.Z(?![\d.])`).

## Rules and findings

| Rule | Clause | Fails when | `stage` | `id` |
|---|---|---|---|---|
| `UnknownId` | (a) | a cited `S-`/`A-` has no row | the citing stage | the ID |
| `Superseded` | (b) | a cited assumption's class is Superseded | the citing stage | the ID |
| `Uncited` | (c) | a current assumption, or a source not of Type General, is cited by no stage; or an adopted-or-revised assumption is cited by no modeled stage | — | the ID |
| `TableShape` | (d) | the signal-chain table has ≠ 15 data rows; row *n*'s number ≠ *n*; row *n*'s identifier ≠ entry *n*'s identifier | *n* (or — for the count) | — |
| `ConfidenceBound` | (e) | a modeled stage's confidence > the lowest headline confidence of its cited current assumptions (High > Medium > Low; Unknown counts as Low) | the stage | — |
| `Counts` | (f) | the header's counts ≠ the tally of the entries' statuses, or `stage_count` ≠ 15 | — | — |
| `StatusDerivation` | (g) | a row without the marker has status ≠ NotModeled, a non-empty model version, a non-empty coverage note or confidence ≠ NotApplicable; a row with the marker has NotModeled, or a status ≠ the one derived from its cited assumptions' classes (Checked: all DocumentVerified/Measured and ≥ 1 Measured; Sourced: all DocumentVerified/Measured and none Measured; else Assumed) | the stage | — |
| `ModelVersion` | (g) | a modeled stage's model version is empty, not `X.Y.Z`, greater than `rr_dr60::VERSION`, or absent from the CHANGELOG | the stage | — |
| `Form` | (h) | a length or form rule of FR-002/FR-003 fails (characters, not bytes): identifier `[a-z0-9_]{1,24}` unique; name ≤ 48; description ≤ 160, one sentence ending in `.`; coverage ≤ 120; controls ≤ 120; basis ≤ 160; links ≤ 200; ID lists ascending and well-formed | the stage (— for the header) | — |
| `Denylist` | (h) | any string of the header or an entry matches a denylist stem or phrase | the stage (— for the header) | — (detail names the word and the field) |
| `Parse` | — | a register row has the wrong cell count, or a file has no data rows | — | — |

## The denylist (FR-011; `docs/product/front-and-inside.md` › Wording on the inside)

```text
DENYLIST_STEMS:   verif, detect, sense, anomal, spike, activity, event, paranormal, evp, ghost, spirit, entit
DENYLIST_PHRASES: listening for, signal present, voice from, communication
```

Matcher: `(?i)\b{stem}(s|es|ed|ied|ies|y|ying|ing|ion|ions|or|ors)?\b` for stems and `(?i)\b{phrase}\b` for phrases. The suffix set is a superset of the spec's; "sensitivity" does not match `sense`, "detector" and "verified" do match. The list is one `const` block at the top of `manifest_checks.rs`, the one place the product team edits.

## Tests that exercise the contract (`tests/manifest.rs`)

1. Committed registers: `cross_check` empty, `wording_findings` empty (US2 scenario 1, SC-003).
2. Doctored inputs, one finding each with the stated rule, stage and ID (SC-002, 7 of 7): A-028's Status → "superseded …" (Superseded, stage 3, A-028); a row `| A-040 | … | assumed |` appended (Uncited, A-040); row 3's identifier `limiter` → `agc` (TableShape, stage 3); a sixteenth row appended and, separately, row 15 removed (TableShape, count); row 7 gains `**Modeled by**` (StatusDerivation, stage 7); a manifest copy with stage 3's confidence set to High (ConfidenceBound, stage 3); S-006 removed (UnknownId, stage 11, S-006).
3. Parser units on inline tables: cell count, status classes, headline confidence, the General exemption, the marker, the version token.
4. `render` equals `golden/manifest-v1.txt`; the README's marked region equals `render_readme_table` (bless with `RR_DR60_BLESS=manifest` / `=readme`).
