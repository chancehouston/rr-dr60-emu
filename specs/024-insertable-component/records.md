# Records: An Insertable Component Slot and the Band Filter (spec 024)

| Record | Change | Gate |
|---|---|---|
| `docs/guide.md` § Insertable components (new) | The point, the component, the live parameters and their transition, the type blend, the ceiling, the gain's effect on the limiter and the voice gate (the measured gain per type at levels 3 and 5), the record form 2, the 158 rule and the parameter budget, the C names, the conventions paragraph of contracts/api.md. | `us2_cross_check::guide_insert` (the denylist) |
| `docs/guide.md` chain diagram, § The controls, § Taps | The slot drawn between stages 2 and 3; entries 31 – 35; `AfterInsert`. | the readme bless |
| `README.md` | "What exists today" (024), the chain line. | the readme bless |
| `CHANGELOG.md` | Under Added (0.92.0): the slot, the band filter, the record form 2, 158; under Changed: the readout's tail (plan N1). | review |
| `docs/product/decisions.md` | One row: spec 024's engine side built (free for good, the open-crate consequence). | review |
| `docs/product/brief.md` 277 – 279, `1.0-feature-set.md` 192 and § 5, `ui-brief-three-screens.md`, `controls-discussion.md` | The slot as built; the app's update after 1.0; stage 4 named "the codec's input filter" in the UI brief so only the component is "Band filter (added)". | review |
| `docs/hardware/signal-chain.md` | A note under the path summary: the insertion point between rows 2 and 3, an addition of the app (no row). | `us2_cross_check` |
| The manifests | The control manifest (entries 31 – 35, the tap option); the stage manifest's version line. | the re-blesses |
| `scripts/ab-timing.sh`, `.github/workflows/ab-timing.yml` | The `insert` mode. | the label job |
| `.github/workflows/ci.yml` | The third promise step (97bbdc2); the golden lines. | CI |
| `CLAUDE.md` | The status entry, the Spec 024 reports, the engine order (024 done). | review |
| Version 0.92.0 | `Cargo.toml` ×4, `RR_DR60_VERSION_MINOR`, the header, the manifest's version line, the Swift ABI fixture. | `us2_cross_check` |
