# Records: The Settings Read and Control Access by Index (spec 025)

| Record | Change |
|---|---|
| `docs/guide.md` § control manifest | "Reading and setting controls by index" (the product review's paragraph); the C block re-blessed |
| `CHANGELOG.md` | Added, 0.91.0 |
| `README.md` | the C functions list |
| `CLAUDE.md` | the status entry, the reports commands |
| Version 0.91.0 | `Cargo.toml` ×4, `RR_DR60_VERSION_MINOR`, the stage manifest's version line |
| `.github/workflows/ci.yml` | the second promise step (f5a0245), the Miri list |
| ic-recorder-app `specs/001-bench-foundation/spec.md` | points to engine 0.91.0 |
| The guide's paragraph (verbatim) | **Reading and setting controls by index** (since 0.91). `rr_dr60_get_settings` writes the settings the engine is running with, landed scheduled values included, so an app shows what is applied rather than what it asked for. `rr_dr60_control_value` returns a control's current value in its manifest type; during a 20 ms volume or battery transition it reads the target. `rr_dr60_settings_control_value` and `rr_dr60_set_settings_control_value` read and write a settings structure's field by the same index, checking only that the value fits the control's type (a known option, a finite number, an integer the field can hold); ranges are checked once, by `rr_dr60_settings_validate`, which names the field, and `rr_dr60_setting_field_control` turns that field into the control's index. `rr_dr60_set_control_value` sets the five live controls that have a plain setter (route, volume, playback speed, speed mode, battery voltage) exactly as their setters do. A generated control surface is a loop over `rr_dr60_control_count()`: read the entry and its value, edit a copy of the settings by index, validate, `reconfigure`, then read the settings back. The manifest's kind tells an app which controls to present; access by index does not. C callers store 0 or 1 in `bool` fields. Store presets and sessions by control identifier, never by index: indices follow the manifest's order, identifiers are the stable names. |
| A guide wording check | the denylist; "read back" and "confirm" present |
| The traceability audit | the new literals cited |
| The Swift ABI fixture | unchanged (the reading stays out of the frozen rows) |
| The figures | the idle cost of each new call (the C smoke's prints) |
| ic-recorder-app spec 001 | D2/FR-001's accessor table dropped at its plan (access by index replaces it) |
