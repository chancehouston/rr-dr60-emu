---
description: "Tasks for engine feature 026, host reads"
---

# Tasks: Host reads

Test-first. The traceability table is in plan.md.

## Phase 1: D6, the words (the smallest, unbumped first)

- [X] T001 [US4] Test `us4_words::{as1_tokens_labels, as2_no_copies}` (failing): every value's token and label as plan R-06 lists them; a source scan finds no word copies in `manifest_checks.rs`, `examples/manifest.rs`, `smoke.c`.
- [X] T002 [US4] `StageStatus/StagePath/Confidence::{token, label}` in `manifest.rs`; the harness's generator, the example and the smoke call them; `us2_cross_check canonical readme` byte-identical against the unbumped goldens.

## Phase 2: D1, the run rules

- [X] T003 [US1] Tests `us1_host_reads::{as1_vas_runs, as1_codec_runs, as1_report_latency}` (failing).
- [X] T004 [US1] `Settings::{one_pass_vas_runs, one_pass_codec_runs}` (`paths.rs` calls them), `Tap::{is_front, is_after_stage_10}` public, `Pipeline::one_pass_report_latency_samples`, the rustdoc of `onepass_output_to_input`.

## Phase 3: D3, the wording lists

- [X] T005 [US4] Tests `us4_wording::{as1_lists, as2_differential, as3_one_source, as4_new_strings_pass}` (failing; the differential against the regex rebuilt `(?i-u:…)` with Unicode `\b`, 10⁵ fuzz strings, the documented divergences).
- [X] T006 [US4] `rr_dr60::wording` (`Lists`, `Hit`, `ENGINE_STRINGS`, `CLAIMS`, `first_hit`, `first_claim_hit`, the `\w` table); the harness's checks call it; the four CLAIMS copies replaced.

## Phase 4: D4, the meter manifest

- [X] T007 [US3] Tests `us3_meters_manifest::{as1_fields, as2_reach, as3_lag, as4_sentences_pass}` (failing).
- [X] T008 [US3] `rr_dr60::meters` (new): `MeterField`, `MeterKind`, `MeterSide`, `MeterCall`, `FIELDS` (the 13 sentences and units of plan R-04); `Pipeline::{meter_reached, meter_lag_samples}`.

## Phase 5: D2, the drain

- [X] T009 [US2] The design scripts' `--drain` prints (band filter with its brute-force grid assert; voiceband N₄, N₁₀; mic chain's per-rate settle) and `--write` of `stages/drain_coeffs.rs`; a test recomputing each print. *(Restated 2026-11-03: the core computes the settles from the committed tables; the scripts' `--drain` prints are pinned by the core's tests; no generated file. Spec Clarifications, implementation restatements.)*
- [X] T010 [US2] Tests `us2_drain::{as1_emits_and_settles, as2_drop_mode_held, as3_taps_and_band_filter}` (failing; the linear twin, the teeth row, the default engine's residual reported).
- [X] T011 [US2] `Pipeline::one_pass_drain_samples` (the cached static terms, the band term on read).

## Phase 6: C, Swift, records

- [X] T012 C: the functions of contracts/c-api.md, `RrDr60MeterField`, statuses 44 – 48, the defines; `abi_tests` (sizes, published list, painted padding, refusal orders, the settings readers field-local, panic guards), Miri; `ffi_parity host_reads meters_manifest`; `alloc_free host_reads`; the header; `smoke --host-reads` against `examples/host_reads_rows.rs`.
- [X] T013 D5: Swift `ReRender` slot loop and frame-floored fill; `ReRenderTests` (empty slot, non-default store: digest and state); the Swift wrappers of D9 (meter manifest, words).
- [X] T014 `ci.yml`: the fourth promise step (2a170d4, full id); the version 0.93.0 (Cargo ×4, `RR_DR60_VERSION_MINOR`, header, the manifest's version line re-blessed with only that line differing, the Swift fixture if needed); the guide's sections (opening with "optional reads, all appended…"); the CHANGELOG; the README's C list; the traceability audit.
- [ ] T015 The full gate (fmt, clippy, tests, the feature-less ffi build with `-D warnings`, llvm-cov, the MSRV build, the C smoke, the Swift package, the simulator goldens); the post-implementation reviews (coach, hardware).
