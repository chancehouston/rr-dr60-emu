# Records: Pitch-Kept Playback Speed (spec 023)

| Record | Change | Gate |
|---|---|---|
| `docs/guide.md` § Playback speed | Both modes; the convention paragraph of contracts/api.md; the pitch figure (1 %) with the modeled label; the ±7 ms content ambiguity; the audible onset formula with "+ ≤ 60 device samples in pitch-kept mode"; "A change blends (since 0.19)" restated as the default mode's; "a pitch-keeping mode would be a later feature" replaced; the mode-aware length helper; the C names. | `us2_cross_check::readme_speed_statements` (amended) |
| `README.md` | The speed sentence: "with the pitch following, or kept as an option". | the readme bless |
| `CHANGELOG.md` | Under Added: product M8's lead (the promise sentence included). | review |
| `docs/product/decisions.md` | One row: the kind model internal (emulator option the alternative), the tier free via curated values (D11), the convention. | review |
| `docs/product/controls-discussion.md` | Row 7 (kind and tier) and § 3d's Speed row ("Pitch kept (engine option)" allowed). | review |
| `docs/product/brief.md` line 226, `monetization.md` line 14 | The option; the tier note. | review |
| `docs/product/1.0-feature-set.md` | The speed row, line 193, § 5 (the free switch and its labels), § 6, § 7. | review |
| `.github/ISSUE_TEMPLATE/hardware_evidence.yml` P1 | "(the emulator offers both as modes; the capture says which is the device's; a click train tells a time-domain method from a decoder-domain one)". | review |
| `docs/hardware/assumptions.md` A-054 | A dated note: the option, its class, "a convention standing in for an unknown method; if P1 shows the device keeps pitch, a new assumption names the modeled method and its artifacts". | `us2_cross_check` |
| `specs/019-speed-crossfade/spec.md` D7 | A dated note: the crossfade is the default mode's convention for level changes. | review |
| The manifests | The control manifest (entry 30, entry 2's sentence); the stage manifest (stage 9's notes, version 0.23.0). | the re-blesses |
| `scripts/ab-timing.sh`, `.github/workflows/ab-timing.yml` | The `pitch` mode. | the label job |
| `.github/workflows/ci.yml` | `scripts/abi-promise.sh 12a3c45` in the check job. | CI |
| `CLAUDE.md` | The status entry, the Spec 023 reports, the engine order (023 done → 0.90.0). | review |
| Version 0.23.0 | `Cargo.toml` ×4, `RR_DR60_VERSION_MINOR`, the manifest's version line. | `us2_cross_check` |
