# Research: Control Manifest (014)

Decisions first. Each item gives the decision, why, and the alternative. Revised 2026-10-29 after the coach (1 – 11), hardware (B1 – B13) and product string reviews of the plan.

## R-01 One table macro, a Rust static and a C twin

- **Decision:** `crates/rr_dr60/src/controls.rs` holds one `control_table!` invocation that emits the public `CONTROLS: [ControlEntry; 29]` and a hidden `controls::__c` twin with NUL-terminated strings and the enums as `u32`, as `manifest.rs`'s `stage_table!` does (007 R-01).
- **Why:** one list, so the Rust and C views cannot drift; no `unsafe` in the core.

## R-02 Value representation

- **Decision:** `ControlValue` = `Switch { default: bool }` | `Integer { min: u64, max: u64, default: u64 }` | `Continuous { min: f32, max: f32, default: f32 }` | `Choice { options: &[ControlOption], default: Option<u32> }`, `ControlOption { identifier, value: u32 }`. The host rate is a choice of seven options (`hz_6000` …) with `default: None`; the tap's options are listed in C-value order.
- **Why:** u64 holds the seed exactly; f32 is the engine's type; a choice carries the C value an app writes.

## R-03 The oracles (FR-005, FR-007, FR-008), as functions over a table

Every check is a function over `&[ControlEntry]`, so a doctored copy can be passed in (SC-003).

- **Core** (`controls::tests`, the only place with access to `#[non_exhaustive]` `Settings` and `pub(crate)` `Setting::range`):
  - `missing_or_extra(entries)`: an exhaustive destructuring of `Settings` and every sub-structure maps each field to its entry; the mapped identifiers plus the three live ones must equal the table's 29 exactly. A new field fails to compile; an extra or missing entry fails the test (SC-003 case "missing entry").
  - Ranges against `Setting::range()` and the memory ranges; defaults against `Settings::new(rate)` at all seven rates, `output::VOLUME_REFERENCE`, `speed::DEFAULT_LEVEL`, `Route::default()`.
  - An exhaustive `match` maps every `Tap`, `VasMode` and `Route` variant to its option, so a new variant fails to compile.
- **Harness** (`controls_checks`): `edges(entries)` offers each bound and each just-outside value through `Settings::validate` and the live setters; `defaults(entries)`; `options_vs_header(entries, header_text)` requires `RR_DR60_<TAP|VAS_MODE|ROUTE>_<IDENTIFIER uppercased> = <value>` in `rr_dr60.h` for every option.
- **Just outside a bound:** for f32, ±1 on the bits, except at a 0.0 bound: below it `f32::from_bits(0x8000_0001)`, above it `f32::from_bits(1)` (`-0.0` equals `0.0` and is accepted; `f32::next_down` needs Rust 1.86, above the MSRV). Integers bound ∓ 1; through C, `vas_sensitivity` = 261 must be refused by `rr_dr60_settings_validate` with `INVALID_SETTING` (it saturates to `u8::MAX`, never wraps). Host rate: each option ± 1 and 0. Choices: the first value past the last C value, through the C API (`INVALID_ARGUMENT`). None for switches and the seed.
- **Never** construct a pipeline at `memory.capacity_s` = 86 400 with the codec bypassed (about 2 GB reserved): the memory edges go through `validate` only.

## R-04 Live-control completeness (FR-007)

- **Decision:** `setters(entries, sources)` scans `crates/rr_dr60/src/**/*.rs` outside `#[cfg(test)]` with `pub(\s+const)?\s+fn\s+set_\w+` (which excludes `pub(crate) fn set_controls`) and `rr_dr60.h` with `^RrDr60Status rr_dr60_set_(\w+)\(` (declarations only; four comment lines also name setters). Each setter minus `set_` must be a live entry's identifier, and every live entry must have a setter in both. Today: `route`, `volume`, `playback_speed`.
- **Risk:** a future non-control `set_` gets an explicit allowlist entry with a comment, never a broadened pattern.

## R-05 The controls-note tokenizer (FR-009)

- **Decision:** `notes(entries, manifest)` implements spec FR-009 literally. Checked on all fifteen committed notes: stage 5 expands to six entries; stage 11's `set_volume, volume; set_route, route` names both live controls; stage 9's `set_playback_speed` names the speed control; `Route::*`, `Tap::*`, `BlockInfo.paused` and `splices (VasEvent)` name nothing; stage 0 entries appear in no note.

## R-06 Cited IDs from the code comments (FR-016), and the comment work

- **Reader:** the trailing `//` comment on the default's line only, matching `\b[AS]-\d{3}\b`; doc comments are not read. Default lines: the field's line in its sub-structure's `impl XSettings { pub const DEVICE` block (scoped to that block: `enabled` occurs in two, and `BYPASSED` blocks repeat fields), or in `Settings::new` for top-level fields; `volume` → `VOLUME_REFERENCE` (output.rs); `playback_speed` → `DEFAULT_LEVEL` (speed.rs); `route` → the `Speaker` variant (settings.rs); `host_rate_hz` → `SUPPORTED_HOST_RATES` (settings.rs). The entry's ID lists must equal the collected tags (ascending).
- **Comment work** (owner decision 3, 2026-10-29: each control cites what its description rests on; comment-only, no code or audio change, one commit, a CHANGELOG note):
  - superseded **A-002 → A-027** on `record_stage_enabled` and `playback_stage_enabled`, and everywhere else it appears in `settings.rs` (the two field docs, the test comments, the test name `defaults_cite_a002_and_fr009`);
  - added: `hiss_offset_db` A-048; `capsule_enabled` A-052; `Speaker` S-001, A-032, A-042; `VOLUME_REFERENCE` S-002, A-044, A-045; `DEFAULT_LEVEL` S-001, S-012, A-054; `SUPPORTED_HOST_RATES` A-026; `seed` A-049; `clamp_enabled` A-042; `capacity_s` A-038; `speech_codec_enabled` A-040, A-041; `sensitivity` A-022, A-033.
- The register cross-check (spec 007 FR-013) gains the control manifest's IDs: each exists and is not superseded (FR-010), in `us2_cross_check` via `manifest_checks`.

## R-07 Canonical text and the version bump (FR-012, FR-014)

- **Decision:** the harness renders the canonical text (`render_controls`); golden `crates/rr_dr60_harness/golden/control-manifest-v1.txt`; bless `RR_DR60_BLESS=control_manifest`, the test in `us1_controls` (US1 owns it); `determinism` reproduces it on every golden target; `examples/control_manifest.rs` prints it.
- **Floats:** Rust prints f32 with `{}` (shortest round trip). The C smoke program prints each f32 with `%.*f` using the fewest decimals (0 – 9) whose `strtof` equals the value: the same text, never an exponent. `run_smoke.sh` diffs `smoke --controls` against the example byte for byte.
- The bump to 0.14.0 re-blesses `manifest-v1.txt`'s `library_version` line and the README's generated regions once; audio golden files untouched.

## R-08 C API (FR-013)

- **Decision:** the 007 shape. Status variant `RR_DR60_STATUS_NO_SUCH_CONTROL = 26` in `RrDr60Status`; functions `rr_dr60_control_count`, `rr_dr60_control`, `rr_dr60_control_option`, `rr_dr60_control_source`, `rr_dr60_control_assumption`; enums for kind, timing and value type; `#define RR_DR60_CONTROL_PATH_*` bits. A NULL out pointer returns `RR_DR60_STATUS_NULL_POINTER`, checked first, as in 007. Deliberately unlike 007: an option or ID index past its list returns `NO_SUCH_CONTROL` (spec FR-013). An explicit `reserved_014` field fills the padding before the first `uint64_t`, with `offset_of` const assertions (spec 010's lesson).
- **Growth** (owner decision 4): no size field and no "grows at the end" promise; `RrDr60Control` and `RrDr60ControlOption` join `RrDr60PlayInfo` and `RrDr60StageEntry` on the pre-1.0 ABI-pass list.

## R-09 Paths and units

- Paths: `route`, `volume` playback + one-pass; `playback_speed` playback; record-side settings (stages 1 – 5, the seed) record + one-pass; playback-side settings (stages 10 – 13, the output clamp) playback + one-pass; the codec switch and the host rate all three; the memory figures record + playback; the tap one-pass (the split paths ignore it).
- Units from `Setting`'s display text (dBFS, ms, dB, s); `level` for sensitivity and speed; `Hz` for the host rate; empty otherwise.

## R-10 Kind rules (owner decision 3)

- `strings(entries)` also requires: a device control cites S-001 or S-002 (Panasonic's own documents; S-001's row does not name the volume wheel, S-002's schematic does); a unit condition or a model internal cites at least one A-###.

## R-11 App rules for the guide (FR-015; owner decisions 1 and 2)

- Free/Workbench (D1): free knobs are the device controls and unit conditions; free curated presets may set any value; the Workbench gives knobs for the rest.
- Unit condition: an audible difference an owner could hear between two units or over one unit's life; component spreads that are inaudible or not directly hearable stay model internals.
- The app joins a control to its stage's status and confidence by stage number; stage 0 controls have none and are labelled by kind.
