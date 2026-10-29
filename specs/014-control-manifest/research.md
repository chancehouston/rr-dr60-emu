# Research: Control Manifest (014)

Decisions first. Each item gives the decision, why, and the alternative.

## R-01 One table macro, a Rust static and a C twin

- **Decision:** `crates/rr_dr60/src/controls.rs` holds one `control_table!` invocation that emits the public `CONTROLS: [ControlEntry; 29]` and a hidden `controls::__c` twin with NUL-terminated strings and the enums as `u32`, exactly as `manifest.rs`'s `stage_table!` does (007 R-01).
- **Why:** one list, so the Rust and C views cannot drift; proven pattern; no `unsafe` in the core.
- **Alternative:** build the C view at runtime: allocates, rejected (FR-012).

## R-02 Value representation

- **Decision:** `ControlValue` is an enum: `Switch { default: bool }`, `Integer { min: u64, max: u64, default: u64 }`, `Continuous { min: f32, max: f32, default: f32 }`, `Choice { options: &[ControlOption], default: Option<u32> }` with `ControlOption { identifier, value: u32 }`. The step is derived (1 for integer, 0 for continuous). The host rate is a choice of seven options with `default: None`.
- **Why:** u64 holds the seed exactly; f32 is the engine's own type for continuous settings; a choice carries the C value an app writes (FR-002). The tap's C values do not follow the Rust order, so each option states its value.
- **Alternative:** one f64 for every number: loses `u64::MAX` and invites float compare on integers.

## R-03 The bounds and defaults oracle (FR-005, FR-008)

- **Decision:** two layers. (1) A **core unit test** compares each continuous and integer entry with `Setting::range()` (crate-private, so the test lives in the core) and the memory ranges, and each default with `Settings::new(rate)` for all seven rates, through an **exhaustive destructuring** of `Settings` and its sub-structs that maps every field to its entry (FR-007; a new field fails to compile). The live defaults come from `output::VOLUME_REFERENCE`, `speed::DEFAULT_LEVEL` and `Route::default()`. (2) A **harness edge table** (`tests/us1_controls.rs`) offers each bound and each just-outside value (spec FR-008's per-type rule) through `Settings::validate` and the live setters, and the choices' out-of-range value through the C API (`INVALID_ARGUMENT`).
- **Why:** the core test catches drift of the range constants; the harness test checks the public behavior an app sees.

## R-04 Live-control completeness (FR-007)

- **Decision:** the harness scans `crates/rr_dr60/src/**/*.rs` outside `#[cfg(test)]` for `pub fn set_` and the C header for `rr_dr60_set_`, and requires each setter's name minus `set_` to be a live entry's identifier. Today: `set_route`, `set_volume`, `set_playback_speed` (three of three).
- **Why:** methods cannot be enumerated by the compiler; the register cross-check already reads files this way.
- **Risk:** a future `set_` helper that is not a control (none today) needs an explicit allowlist entry with a comment, never a broadened pattern.

## R-05 The controls-note tokenizer (FR-009)

- **Decision:** `controls_checks::tokens(note)` implements spec FR-009 literally: split on `;`, then `,`, trim; prefix expansion within a `;` group; classify (a) entry/setting path, (b) `set_<id>`/`<id>` of a live control, (c) `Tap::*`/`Route::*`, (d) other. A token with a settings prefix or equal to a top-level field that resolves under neither (a) nor (b) is an error. The check is two-way per stage; the codec switch may appear under 7 or 9.
- **Checked against the committed notes:** stage 5's `vas.enabled, mode, sensitivity, threshold_dbfs, hang_ms, onset_ms` expands to six entries; stage 11's `set_volume, volume; set_route, route` names both live controls; stage 9's `set_playback_speed` names the speed control; stages 12 and 13's `Route::Speaker`, `Tap::RouteOutput` name nothing. `output.clamp_enabled`, `tap`, `seed` and `host_rate_hz` (stage 0) appear in no note.

## R-06 Cited IDs from the code comments (FR-016)

- **Decision:** the harness reads the default's definition line for each entry (the `DEVICE` blocks and `Settings::new` in `settings.rs`; `VOLUME_REFERENCE` in `output.rs`; `DEFAULT_LEVEL` in `speed.rs`; `Route`'s `#[default]` in `transport.rs`) and collects the `S-###`/`A-###` tags on that line and in the doc comment directly above it; the entry's lists must equal them.
- **Finding:** four comments must be corrected first (comment-only, no audio change): `record_stage_enabled` and `playback_stage_enabled` cite **A-002, superseded** by A-027 (spec 004); `mic.hiss_offset_db` cites no ID though its range and default rest on **A-048**; `Route`'s default variant cites nothing though the earphone/speaker choice is S-001's. Each gains the current ID; the CHANGELOG notes it.
- **Alternative:** author the IDs by hand only: no oracle, rejected by FR-016.

## R-07 Canonical text and the version bump (FR-012, FR-014)

- **Decision:** `controls::canonical()`-style rendering lives in the harness (`render_controls`), golden `crates/rr_dr60_harness/golden/control-manifest-v1.txt`, bless switch `RR_DR60_BLESS=control_manifest`; the `determinism` test reproduces it on every golden target, the example `examples/control_manifest.rs` prints it, and the C smoke program prints it with `--controls` for a byte diff. f32 values use Rust's shortest round-trip `{}`; the C side prints with `%.9g` and the harness compares parsed values, not text, for floats (`printf` has no shortest form).
- The version bump to 0.14.0 re-blesses `manifest-v1.txt`'s `library_version` line and the README's generated regions once; the audio golden files and SHA guards are untouched.

## R-08 C API (FR-013)

- **Decision:** status `RR_DR60_STATUS_NO_SUCH_CONTROL = 26`; functions `rr_dr60_control_count()`, `rr_dr60_control(index, *out)`, `rr_dr60_control_option(index, option, *out)`, `rr_dr60_control_source(index, i, **out)`, `rr_dr60_control_assumption(index, i, **out)`; structs `RrDr60Control` and `RrDr60ControlOption`; enums `RrDr60ControlKind`, `RrDr60ControlTiming`, `RrDr60ControlValueType`; paths as a bit set (`RR_DR60_CONTROL_PATH_RECORD = 1`, `_PLAYBACK = 2`, `_ONE_PASS = 4`). Panic-guarded like the 007 functions; NULL checked before range.
- **Why:** the 007 shape, already consumed by the Swift package.

## R-09 Paths and units

- Paths: `route`, `volume` playback + one-pass; `playback_speed` playback; record-side settings (stages 1 – 5, the seed) record + one-pass; playback-side settings (stages 10 – 13, the output clamp) playback + one-pass; the codec switch and the host rate all three; the memory figures record + playback; the tap one-pass.
- Units from `Setting`'s display text where it names one (dBFS, ms, dB, s); `level` for sensitivity and speed; `Hz` for the host rate; empty for switches, choices, the seed, the recording count and the volume position.
