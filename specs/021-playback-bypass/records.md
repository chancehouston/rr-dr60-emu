# Records: Live Playback-Stage Bypass (spec 021)

What the feature writes outside the code, each with its gate.

| Record | Change | Gate |
|---|---|---|
| `golden/control-manifest-v1.txt` | Entries 5, 22, 23, 24: timing `live`; nothing else. | `RR_DR60_BLESS=control_manifest …` once; `us1_controls`/`us2_controls` (the live set, the no-plain-setter rule extended to the four). |
| `golden/manifest-v1.txt`, README table | The version line only (the 010 precedent: an emulator option). | `RR_DR60_BLESS=manifest …` once; `us2_cross_check`. |
| `docs/guide.md` | A "Live stage bypass" section: the four switches and their indices; the 20 ms blend as an engine convention (the device has no such switches); the warm stage; the app contract (construct all-on, presets at position 0, `settings()` a read-out, the `reconfigure(settings())` trap); stage 11's level table and the neutral positions from the public functions and `SPEAKER_SCALE_DB`; stage 10's onset; the whole-output-path recipe (11 + 12 or 11 + 13 at one position with one tag); the inside's "bypassed" card from the log or `settings()`; greying the inactive route's toggle; the measured click reductions. | `us2_cross_check::guide_bypass` (the forbidden-list scan). |
| `README.md` | "What exists today" (spec 021), the real-time list (the four switches live through the schedule). | review |
| `CHANGELOG.md` | Under Added: the feature, no audio change at a constant state, the new status, the bound 142, `SPEAKER_SCALE_DB`, the convention, the golden. | review |
| `docs/product/controls-discussion.md` | § 2a row 6, § 2b row, § 4 item 8: "built as spec 021 …" status lines (the product review's wording). | review |
| `docs/product/brief.md`, `front-and-inside.md`, `decisions.md` | The Workbench A/B bullet; the bypass item and the `reconfigure` question closed; one decisions row (Workbench, an emulator convention, the configured-off meaning, the alternatives). | review |
| `docs/hardware/*` | No change (no assumption). | `scripts/check-traceability.sh` |
| `include/rr_dr60.h` | Status 36, the bound, the define. | the header drift check |
| `CLAUDE.md` | The status entry, the Spec 021 reports commands, the engine order. | review |
| Memory bookmark | The step reached. | — |
