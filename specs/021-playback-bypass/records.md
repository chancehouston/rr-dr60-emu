# Records: Live Playback-Stage Bypass (spec 021)

What the feature writes outside the code, each with its gate.

| Record | Change | Gate |
|---|---|---|
| `golden/control-manifest-v1.txt` | Entries 5, 22, 23, 24: timing `live`; nothing else. | `RR_DR60_BLESS=control_manifest …` once; `us1_controls`/`us2_controls` (the live set, the no-plain-setter rule extended to the four). |
| `golden/manifest-v1.txt`, README table | The version line only (the 010 precedent: an emulator option). | `RR_DR60_BLESS=manifest …` once; `us2_cross_check`. |
| `docs/guide.md` | A "Live stage bypass" section: the four switches and their indices; the 20 ms blend as an engine convention (the device has no such switches); the warm stage; the eleven-point app contract of contracts/api.md (construct all-on with a reaching tap, presets at position 0 and refused rather than no-ops, `settings()` a read-out and the trap's recipe, the finger toggle on the audio thread, the loudness rule for stage 11 on the earphone route, the inert volume and battery, stage 10's audibility and onset, the card's lifecycle and meter rules, the toggle order 12, 11, 13, 10 and the greyed inactive route, the wording rule); stage 11's level table from `stage11_bypass_level_db` and the neutral positions ("computed from the committed law (A-044, A-031, A-043), not measured"); the whole-output-path recipe (11 + 12 or 11 + 13 at one position with one tag); where the identities are exact and the measured convergence figures; the measured click reductions and the stage-10 figures. | `us2_cross_check::guide_bypass` (the forbidden-list scan, with the stems raw / clean / unfiltered / true signal added for this section). |
| `README.md` | "What exists today" (spec 021), the real-time list (the four switches live through the schedule). | review |
| `CHANGELOG.md` | Under Added, the lead: "no audio change for any configuration without a scheduled switch (the thirteen audio golden guards, the record golden and every smoke hash unmoved); a scheduled switch is new behaviour, an emulator convention: the device has no stage switches and no assumption is registered or amended"; the four switches and their streams; stage 11's level change by route and position with the app's loudness rule; status 36; the bound 139 → 142 (not an ABI change; a host that sizes by the constant recompiles; a stale 139-entry buffer may be short by up to three entries, reported by the count); `stage11_bypass_level_db` (a D10 deviation accepted at the plan); the golden. | review |
| `docs/product/controls-discussion.md` | § 2a row 6, § 2b row, § 4 item 8: "built as spec 021 …" status lines (the product review's wording). | review |
| `docs/product/brief.md`, `front-and-inside.md`, `decisions.md` | The product review's draft copy (scratchpad `021-product-plan-review.md`, the agent report of 2026-10-31): brief.md's Workbench bullet and the Settings paragraph; front-and-inside.md's badges row, the `reconfigure` question closed, the Avoid list's wording sentence; one decisions row (Workbench, an emulator convention, the configured-off meaning, the alternatives). | review |
| `docs/hardware/*` | No change (no assumption). | `scripts/check-traceability.sh` |
| `include/rr_dr60.h` | Status 36 with its comment (the `Display` text plus "see the guide, Live stage bypass"), the bound 142, `rr_dr60_stage11_bypass_level_db`. | the header drift check |
| Swift package | No status list to update (`swift/README.md` has a "Status codes" note only); nothing. | — |
| Version 0.21.0 | `Cargo.toml` ×4, the header's version, the manifest's version line, `CHANGELOG.md`. | `us2_cross_check`, the header drift check |
| `tools/output-chain/design_output_chain.py` | No change: superseded at implementation (R-07): `stage11_bypass_level_db` is −20·log10 of the chain's own scalar, so no constant exists. | — |
| `CLAUDE.md` | The status entry, the Spec 021 reports commands, the engine order. | review |
| Memory bookmark | The step reached. | — |
