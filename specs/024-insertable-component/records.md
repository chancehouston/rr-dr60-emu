# Records: An Insertable Component Slot and the Band Filter (spec 024)

| Record | Change | Gate |
|---|---|---|
| `docs/guide.md` § Insertable components (new) | The point, the component, the live parameters and their transition, the type blend, the ceiling, the gain's effect on the limiter and the voice gate (the gain read per type at levels 3 and 5 from the harness bisection), the cross-rate table, the one-pass's immediate component change, the idle setter as the restore path, the record form 2, the 158 rule and the parameter budget, the C names, the conventions paragraph of contracts/api.md. | `us2_cross_check::guide_insert` (the denylist) |
| `docs/guide.md` chain diagram, § The controls, § Taps | The slot drawn between stages 2 and 3; entries 31 – 35; `AfterInsert`. | the readme bless |
| `README.md` | "What exists today" (024), the chain line. | the readme bless |
| `CHANGELOG.md` | Under Added (0.92.0): the slot, the band filter, the idle setter, the record form 2 and `RrDr60RecordComponent`, 158 (the short-buffer rule). | review |
| `docs/guide.md` § Changes, § Settings read (spec 025), the meters | 158 and the short-buffer text; `control_value` 31 – 35 and `set_control_value(31 … 35)`; the limiter-input peak reads the slot's output (hardware B15: the rustdoc of `Meters::limiter_input_peak`, the header and guide.md's meter section). | `us2_cross_check` |
| `README.md` "What changed from 0.91", the C crate doc | The slot; the idle-only calls (`set_component`, `set_component_parameter`). | the readme bless; review |
| Pins of earlier specs | The dated restatements of research R-11 (142 → 158, 72 → 88, 31 → 36 controls, the `Tap` match, the control map closure, the live-entry counts, the record golden's SHA). | the binaries |
| `docs/product/decisions.md` | One row: spec 024's engine side built (free for good, the open-crate consequence). | review |
| `docs/product/brief.md` § 3 "Define extension points", `1.0-feature-set.md` § 4.11, `ui-brief-three-screens.md` | The slot as built (cited by section, product M8); the free-for-good sweep is its own docs change on PR #68's branch. | review |
| `docs/hardware/signal-chain.md` | A note under the path summary: the insertion point between rows 2 and 3, an addition of the app (no row). | `us2_cross_check` |
| The manifests | The control manifest (entries 31 – 35, the tap option); the stage manifest's version line. | the re-blesses |
| `scripts/ab-timing.sh`, `.github/workflows/ab-timing.yml` | The `insert` mode. | the label job |
| `.github/workflows/ci.yml` | The third promise step (97bbdc2); the golden lines. | CI |
| `CLAUDE.md` | The status entry, the Spec 024 reports, the engine order (024 done). | review |
| Version 0.92.0 | `Cargo.toml` ×4, `RR_DR60_VERSION_MINOR`, the header, the manifest's version line, the Swift ABI fixture. | `us2_cross_check` |
