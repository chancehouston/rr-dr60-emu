# Data Model: Stage Manifest (feature 007)

Constant data only; no state, no transitions. Nothing in the processing data models of specs 001–006 changes.

## Manifest (research R-01)

```
Manifest
├── header: Header
└── stages: [StageEntry; 15]      // number 1..=15 in signal-chain order; index = number − 1
```

| Type | Field | Type | Rule (spec) |
|---|---|---|---|
| `Header` | `library_version` | `&'static str` | `rr_dr60::VERSION` (FR-003) |
| | `device_rate_hz` | `u32` | 6000, `DEVICE_RATE_HZ` (A-026) |
| | `stage_count` | `u32` | 15 |
| | `counts` | `StatusCounts` | computed from the entries at compile time; sums to 15 (FR-003, FR-013 (f)) |
| | `basis` | `&'static str` | FR-012's sentence; ≤ 160 characters |
| | `evidence_link` | `&'static str` | the "Hardware evidence" issue chooser; ≤ 200 |
| | `assumptions_link` | `&'static str` | `docs/hardware/assumptions.md` on the default branch; ≤ 200; an ID appended as `#a-028` is a row anchor (clarify 2026-10-12) |
| | `sources_link` | `&'static str` | `docs/hardware/sources.md` likewise |
| `StatusCounts` | `not_modeled`, `assumed`, `sourced`, `checked` | `u32` | one per `StageStatus` |
| `StageEntry` | `number` | `u8` | 1–15, stable (FR-002) |
| | `identifier` | `&'static str` | `[a-z0-9_]{1,24}`, unique, equals the signal-chain table's Identifier cell (FR-002, FR-013 (d)) |
| | `name` | `&'static str` | ≤ 48 characters |
| | `description` | `&'static str` | one sentence, ≤ 160 characters, ends with `.`; sourced properties only (FR-010); denylist-free (FR-011) |
| | `status` | `StageStatus` | FR-004, derived from the signal-chain marker and the cited assumptions' Status cells |
| | `path` | `StagePath` | `Record`, `Playback`, `Both` |
| | `confidence` | `Confidence` | `NotApplicable` iff `NotModeled`; otherwise ≤ the lowest headline confidence of the cited current assumptions (FR-006) |
| | `sources` | `&'static [&'static str]` | `S-###` strings, ascending, each a register row, none general-only required (FR-005) |
| | `assumptions` | `&'static [&'static str]` | `A-###` strings, ascending, each a current register row (FR-005) |
| | `coverage` | `&'static str` | what the engine leaves out at this stage; ≤ 120; empty when nothing or when not modeled; FR-010 applies |
| | `controls` | `&'static str` | Rust API names verbatim, "splices" for VAS events; ≤ 120; empty when none (FR-007) |
| | `model_version` | `&'static str` | `X.Y.Z` ≤ the library version, appearing in the CHANGELOG; empty iff `NotModeled` (FR-008) |

Enums (`#[non_exhaustive]`, `Copy`, `Debug`, `PartialEq`, `Eq`, `Hash`):

| Enum | Variants (C value) |
|---|---|
| `StageStatus` | `NotModeled` (0), `Assumed` (1), `Sourced` (2), `Checked` (3) |
| `StagePath` | `Record` (0), `Playback` (1), `Both` (2) |
| `Confidence` | `NotApplicable` (0), `Low` (1), `Medium` (2), `High` (3) |

The hidden C twin table (`manifest::__c`) carries the same values with every string as a NUL-terminated `&'static [u8]` and the ID lists as `&'static [&'static [u8]]`; a core unit test asserts twin = string + `\0` for every field.

## The fifteen entries at 0.7.0 (research R-02; spec Key Entities)

| # | identifier | path | status | confidence | sources | assumptions | model version | coverage |
|---|---|---|---|---|---|---|---|---|
| 1 | `mic_capsule` | Record | NotModeled | NotApplicable | S-002 | A-005 | | |
| 2 | `mic_amp` | Record | NotModeled | NotApplicable | S-002 | A-006, A-029 | | |
| 3 | `limiter` | Record | Assumed | Low | S-001, S-002 | A-007, A-019, A-020, A-028, A-034, A-035, A-036 | 0.5.0 | |
| 4 | `codec_input` | Record | Assumed | Medium | S-002, S-003, S-005, S-007 | A-003, A-015, A-016, A-026, A-027, A-030 | 0.5.0 | µ-law quantization, idle noise and the device's own aliasing above 3 kHz |
| 5 | `vas` | Record | Assumed | Low | S-001, S-002 | A-008, A-021, A-022, A-023, A-024, A-025, A-033 | 0.5.0 | a digital gain from the sensitivity setting, if the device applies one |
| 6 | `noise_reduction` | Record | NotModeled | NotApplicable | S-004 | A-009 | | |
| 7 | `encoder` | Record | NotModeled | NotApplicable | S-002, S-004 | A-004 | | |
| 8 | `memory` | Both | Assumed | Low | S-001, S-002 | A-037, A-038, A-039 | 0.6.0 | the stored form is the engine's own samples, not encoded frames |
| 9 | `decoder` | Playback | NotModeled | NotApplicable | S-002 | A-004 | | |
| 10 | `codec_output` | Playback | Assumed | Medium | S-003, S-005, S-007 | A-015, A-016, A-026, A-027 | 0.4.0 | DAC imaging residue above 3 kHz |
| 11 | `volume_amp` | Playback | NotModeled | NotApplicable | S-001, S-002, S-006 | A-031 | | |
| 12 | `speaker` | Playback | NotModeled | NotApplicable | S-002 | A-010 | | |
| 13 | `earphone` | Playback | NotModeled | NotApplicable | S-001, S-002 | A-012, A-032 | | |
| 14 | `clock` | Both | NotModeled | NotApplicable | S-002 | A-011 | | |
| 15 | `power` | Both | NotModeled | NotApplicable | S-002 | A-013 | | |

Names, descriptions and controls notes: the spec's Key Entities table and its "Controls notes (initial)" paragraph, verbatim. Header counts: not modeled 10, assumed 5, sourced 0, checked 0.

## Register rows as the cross-check reads them (research R-04; contracts/cross-check.md)

| Register | Row selector | Cells (in order) | Used for |
|---|---|---|---|
| assumptions.md | first cell `A-\d{3}` | ID, Stage, Assumption, Rationale / sources, Confidence, How to verify, Status | existence (a); class from Status (b, g); adopted/revised flag (c); headline confidence (e) |
| sources.md | first cell `S-\d{3}` | ID, Source, Type, Reliability, Key facts | existence (a); `Type == General` exemption (c) |
| signal-chain.md | first cell an integer | #, Stage, Identifier, Known, Assumed, Character it contributes | row count and identifiers (d); modeled marker `**Modeled by` (g, i) |
| CHANGELOG.md | whole text | | model version appears as a token (g) |

Assumption Status classes: `Superseded` (cell starts with "superseded"), `DocumentVerified` (starts with "verified", no `A-\d{3}` in the cell), `Measured` (starts with "measured"), `Open` (anything else); flag `adopted_or_revised` (starts with "adopted" or "revised"). Derived stage status for a modeled row: `Checked` if every cited assumption is DocumentVerified or Measured and at least one is Measured; `Sourced` if every cited assumption is DocumentVerified or Measured and none Measured; else `Assumed`.

## Findings (research R-04)

`Finding { rule: Rule, stage: Option<u8>, id: Option<String>, detail: String }`, `Rule ∈ { Parse, UnknownId (a), Superseded (b), Uncited (c), TableShape (d), ConfidenceBound (e), Counts (f), StatusDerivation (g), ModelVersion (g), Form (h), Denylist (h) }`. A passing cross-check is an empty list; tests assert on `rule`, `stage` and `id`.

## Canonical rendering (research R-05)

```
rr_dr60 manifest
library_version: 0.7.0
device_rate_hz: 6000
stage_count: 15
counts: not_modeled=10 assumed=5 sourced=0 checked=0
basis: Modeled on the RR-DR60 service manual and the codec family datasheets; no stage has been checked against a real unit.
evidence_link: https://github.com/chancehouston/rr-dr60-emu/issues/new/choose
assumptions_link: https://github.com/chancehouston/rr-dr60-emu/blob/main/docs/hardware/assumptions.md
sources_link: https://github.com/chancehouston/rr-dr60-emu/blob/main/docs/hardware/sources.md

stage 1 mic_capsule
name: Microphone capsule
description: Turns sound pressure into an electrical signal with a small two-terminal electret capsule.
status: not_modeled
path: record
confidence: not_applicable
sources: S-002
assumptions: A-005
coverage:
controls:
model_version:

stage 2 mic_amp
…
```

One blank line between blocks, `\n` line ends, UTF-8, no trailing whitespace; an empty field prints `key:` with nothing after the colon. Committed as `crates/rr_dr60_harness/golden/manifest-v1.txt`; printed identically by the Rust example and by `smoke --manifest`.
