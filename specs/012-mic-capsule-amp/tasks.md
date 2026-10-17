---

description: "Task list for 012 Microphone Capsule and Amplifier (signal-chain stages 1–2)"
---

# Tasks: Microphone Capsule and Amplifier (signal-chain stages 1–2)

**Input**: Design documents from `specs/012-mic-capsule-amp/`:
- [plan.md](plan.md) (› "Task order")
- [spec.md](spec.md), with the specify, spec-review and plan-review Clarifications
- [research.md](research.md) (R-01 – R-12)
- [data-model.md](data-model.md)
- [contracts/](contracts/) (rust-api, c-api, golden-format)
- [quickstart.md](quickstart.md)
- the design script `tools/mic-chain/design_mic_chain.py`

**Tests**: REQUIRED. Constitution III requires tests to be written first and to fail before the implementation exists. Every story phase lists its tests before its implementation tasks and runs them red:
- **US1:** against the stages bypassed;
- **US2:** against a linear stand-in for the clipper;
- **US3:** against the stages before its wiring;
- **C:** with placeholder hashes.

**Organization**:
1. Setup: version 0.12.0, the design script and its figures, the register skeletons, the manifest entries and the one re-bless.
2. Foundational: the settings, the taps and the C fields, all inert; the 0.11-default configuration and the rebase.
3. US1 (P1, MVP): the band and the hiss; the default flips on; the bit-identity proof.
4. US2 (P2): clipping.
5. US3 (P2): the whole engine and the cost.
6. US4 (P3): the records.
7. The C surface.
8. Gates, the ngspice gate, the reviews, the bless and the PR.

The plan's task order names these phases 1, 1 (foundational part), 2, 3, 4a, 4b, 5 and 6; here they are numbered 1–8.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task).
- **[Story]**: the user story the task belongs to (US1 – US4).
- Paths are relative to the repository root.

## Conventions that apply to every task

- **Traceability.** Cite spec 012's FR, SC and assumption IDs in code comments, rustdoc and test names, for example `// A-046: the input high-pass (spec 012 FR-005)` or `// 012 FR-009: the counter never rewinds`. Every literal tolerance line carries its citation (the traceability scan).
- **The figures come from the design script.**
  - Every expected value in a test is a constant the script prints, copied into `mic_checks.rs` with the date of the print (spec 005's lesson). Tests never pin to the engine's own output.
  - `--write` regenerates `crates/rr_dr60/src/stages/mic_coeffs.rs` byte-identically. `--check` asserts every bound and constraint.
- **The seven golden files do not move** (FR-010): `golden`, `golden_agc`, `golden_vas`, `golden_codec`, `golden_output`, `golden_output_clamp` and `golden_volume_ramp`, with their SHA guards.
  - Every pre-012 harness configuration is built on `configs::settings_011` (stages 1–2 bypassed).
  - The guards run at the end of every phase. **The bit-identity proof is at the default flip** (T030).
- **The oracles** (research R-02 – R-08):
  - **(a) bypass identity:** stages off equals 0.11, bit for bit;
  - **(b) the prototype:** read by impulse and DTFT at exact third-octave centres (spec 004's lesson: never adjacent FFT bins);
  - **(c) the clipper's static curve:** read through the engine's own probe;
  - **(d) the clipper-off twin:** for the recovery tail;
  - **(e) limiter ratios:** two seeds within 4 f32 ULP;
  - **(f) bit-for-bit equivalence:** the first recording after construction, reset or reconfigure equals the one-pass.
- **Levels.**
  - dBFS is AES17: a full-scale sine reads 0 dBFS, so RMS dBFS = 20·log10(RMS·√2).
  - Power ratios use `analysis::db(·) / 2`; `f64::log10` is banned.
  - Spectral checks on the codec's output use 8 kHz after resampling both sides (spec 008's lesson).
- **Phase-aligned tones.** A 1 kHz tone that must hit the clipper's input on a peak starts on the phase the script prints, which compensates the input high-pass. A 1 kHz tone at 6 kHz has six samples per period, so sample peaks read phase, not level.
- **Floating point.** f64 in the per-sample path; basic IEEE operations only; the ziggurat's tail through `rr_dr60_detmath::{exp, ln}`; selects instead of `f64::clamp`.
- **Wording** (R-11):
  - "the hiss", "the microphone's self-noise", "modeled on the circuit";
  - never "accurate", "authentic", "measured" or "verified" (the denylist stem `verif`);
  - "noise floor" only with a level attached.
- **Story boundaries** (Principle VII):
  - US1 and US2 use the stages-only configurations (`mic_only`, `mic_only_quiet`, `mic_capsule_only`) and synthetic input;
  - US3 reuses the whole-engine harnesses;
  - US4 uses the cross-check and the audit.
- **Coverage time.** Audio-heavy loops use `configs::measurement_rates()`. The voice-activation runs read at `Tap::AfterVas` (no codec); the offset bisection runs at 6 kHz only under `cfg(coverage)`; the 600 s, 60-minute and 10⁸-draw checks are release-only (`#[ignore]`).
- **Before ticking a task**, these must pass (inside a story's test-first window, that story's new tests are the expected red):
  - `cargo fmt --all -- --check`;
  - `cargo clippy --all-targets --all-features -- -D warnings`;
  - `cargo test --all-features`.

  The MSRV gate is `RUSTFLAGS="-D warnings" cargo +1.85 test --workspace --all-features --no-run`.
- **Commits.** Conventional Commits on branch `012-mic-capsule-amp`, after each task or logical group. `git add` by file. Merge to `main` only through a green PR, and only after the user says so.

---

## Phase 1: Setup (plan › Task order 1; research R-01 – R-04, R-11)

**Purpose**: version 0.12.0, the design script and every figure, the register skeletons, the manifest entries and the one re-bless. Nothing audible changes in this phase.

- [x] T001 Bump the workspace version to 0.12.0:
  - `version = "0.12.0"` in `[workspace.package]` and the three internal dependency entries of `Cargo.toml`;
  - `RR_DR60_VERSION_MINOR = 12` in `crates/rr_dr60_ffi/src/lib.rs`;
  - regenerate `crates/rr_dr60_ffi/include/rr_dr60.h` (`cbindgen --config crates/rr_dr60_ffi/cbindgen.toml --crate rr_dr60_ffi --output crates/rr_dr60_ffi/include/rr_dr60.h`);
  - fix the version strings the smoke loop and `ffi_parity` pin.
- [x] T002 Create `tools/mic-chain/design_mic_chain.py` (uv inline metadata: numpy, scipy), part 1, the circuit (research R-01):
  - **The model.** The hybrid-π nodal network of S-002 sheet 3.1, stated in the module docstring: M, B, E, C3, O, the load C4ser C5 + 23 kΩ, the codec input amplifier R15 ∥ C10 to GSX. Operating points 104 / 6 µA from the DC voltages.
  - **The fit.** Solve the transfer from the capsule's current to GSX, then a joint fit of three first-order poles (residual < 0.001 dB). Print the corners, the −3 dB point below the analytic plateau, the responses at 300 Hz and 2550 Hz re 1 kHz, and the transimpedance at 1 kHz.
  - **The grid.** C1 ±10 %, hFE 150–460, VA 20–100 V, I3 5–7 µA. Print the range of each corner, the reviewer's closed form and the upper bound 1/(2π·C1·R2′).
  - **The DC check.** An Ebers–Moll consistency check (ΔVbe ≈ 74 mV against the meter; Q2's Veb).
  - **The level chain.** Full scale in µA and in dB SPL for capsules of −35 / −40 / −44 dBV/Pa.
  - **Checkpoint:** compare the printed nominal and grid with research R-01's table (818.4 / 76.8 / 12 939 Hz; input corner 665–985 Hz). Stop and report any difference above 1 %.
- [x] T003 Add `--spice` to the design script and `tools/mic-chain/rr_dr60_mic.cir` (R-01 › Cross-checks):
  - write the netlist of sheet 3.1 with generic 2SD1819A / 2SB1218A models (parameters stated in the file);
  - run `ngspice -b`, parse the AC response at GSX, and print the corners next to the nodal figures;
  - exit non-zero when the input corner disagrees by more than 10 % (the bless gate);
  - run it once now and record the figures in a comment in the script's output section.
- [x] T004 The design script, part 2, the realization (R-02, R-03):
  - **The fit.** The per-rate constrained fit of HP_in (with the 1 kHz normalization in its numerator; zero at DC, pole in [0, 1)), HP_out (bilinear form, gain (1 + p)/2) and LP (b0, b1 ≥ 0, pole in [0, 1), unity DC gain). Iterated least squares, power 2 → 16, at the third-octave centres from 20 Hz to min(20 kHz, 0.45·fs) where the prototype is above −40 dB.
  - **Print per rate:** the coefficients, the worst error (expect ≤ 0.075 dB; the ±0.25 dB above-4-kHz fallback is printed only if needed), each section's −3 dB point and the 1 kHz group delay.
  - **The clipper.** The limits L₋ and L₊ from the DC voltages (0 dBFS = 0.196 V peak). k = 0.85, the onsets and the levels at which the output reaches L, in clipper units and at the engine input (× |HP_out·LP|(1 kHz)). The phase of HP_in at 1 kHz and the start phase that puts a host sample on the clipper-input peak, per rate.
  - **Bound, settling, harmonics.** The bound L₊ + L₋ and its premises (the impulse responses of LP′ = 1 − HP_out and of LP non-negative, each summing to 1). The settle time within ±0.1 dB for the stimulus 1 kHz at +13 dBFS for 50 ms, then −20 dBFS. H2 and H3 of a tone 6 dB above the first onset from the discrete model at each rate, with the continuous figures.
- [x] T005 The design script, part 3, the hiss (R-04, R-07):
  - **The anchor.** 34 dB SPL(A), the A-weighted-to-band offset through the full-network response, full scale 90 dB SPL → the band level (expect −62.0 dBFS AES17), the 1 kHz third-octave and the third-octave shape at 315–2500 Hz.
  - **σ_r per rate.** Integrate through the **realized** stage-2 response.
  - **Floors.** The amplifier and R2 floor (≈ −73 dBFS) and the credible range (about −70 … −54).
  - **The generator.** The ziggurat tables (128 layers: x, f, and the integer thresholds) as `f64` bit patterns; the constants `MIC_STREAM` and `MIC_COUNTER_SALT`; the retry cap 8 and its per-sample probability.
  - **The voice activation.** Its margins per sensitivity level at offset 0, using the reviewer's state-machine model, and the offsets at which levels 4 and 5 stop pausing.
- [x] T006 The design script, part 4:
  - **`--check`:** asserts every constraint and bound of T004 and T005; exit 0 or a list of failures.
  - **`--write`:** emits `crates/rr_dr60/src/stages/mic_coeffs.rs`:
    - a generated-file header;
    - per-rate `[f64; 3]` sections and σ_r as exact bits;
    - the clipper's L, 1/L and k·L;
    - the ziggurat tables, the stream constants and the retry cap;
    - every constant with its assumption ID in a comment.
  - **The writer's rules:** a second `--write` is byte-identical; add `stages/mic_coeffs.rs` to `crates/rr_dr60/src/stages/mod.rs` (`pub(crate)`, `#[allow(clippy::unreadable_literal)]` as in `output_coeffs.rs`).
  - **`tools/mic-chain/README.md`:** usage and the gate meaning of `--spice`.
- [x] T007 [P] Register the new assumptions in `docs/hardware/assumptions.md` as rows after A-045 (research R-11), each with statement, rationale, confidence and a capture protocol, and the figures from T002–T005's print:
  - **A-046:** the input high-pass, 818 Hz, 665–985 Hz, bounded above by about 1.0 kHz. High confidence that it exists, medium in the value.
  - **A-047:** the output coupling (≈ 77 Hz; C4/C5 Y5V +80/−20 %, so 40–95 Hz per unit; the 159 Hz shift under limiting not modeled) and the 12.9 kHz roll-off.
  - **A-048:** the hiss anchor, shape and credible range.
  - **A-049:** the generator, keying, retry cap and the never-rewinding counter (an engine convention).
  - **A-050:** the amplifier and R2 floor, and the unknown SG reference noise (22 nV/√Hz equivalence; S-003 the lead).
  - **A-051:** the clip limits, k = 0.85 and its justification (Q1's saturation curvature at a high loop gain), and the first-clipping polarity.
  - **A-052:** the capsule's flat response.
  - **A-053:** A-047's sections lumped ahead of the limiter.

  Mark each "assumed (spec 012)". The rationale text is completed in T045.
- [x] T008 Stages 1–2's manifest entries in `crates/rr_dr60/src/manifest.rs` (FR-013; R-11):
  - **Status and versions:** status `assumed`, model version `"0.12.0"`.
  - **Sources and assumptions:** S-002; stage 1 cites A-005, A-048, A-049, A-052; stage 2 cites A-006, A-029, A-046, A-047, A-051, A-053.
  - **Descriptions:** one neutral sentence each, run through the denylist first (`cargo test -p rr_dr60_harness --test us2_cross_check denylist`).
  - **Coverage notes** (≤ 120 characters each):
    - stage 1: directivity, handling noise, wind, acoustic overload, case acoustics, unit spread;
    - stage 2: the coupling's shift under limiting, below-clip distortion, part tolerance.
  - **Controls:** `Settings::mic`, `MicSettings::capsule_enabled`, `MicSettings::amp_enabled`, `MicSettings::hiss_offset_db`, `Tap::AfterMicCapsule`, `Tap::AfterMicAmp`. These are verbatim paths, so **T008 and T010 run at the end of Phase 2, after T012–T019**. Their numbers keep them with the records they belong to.
  - **Check:** the manifest counts become 12 assumed and 3 not modeled.
- [x] T009 Open the three `<!-- 0.12.0:start -->` / `<!-- 0.12.0:end -->` regions in `CHANGELOG.md` under `[Unreleased]` › Added, Changed and Documentation as a skeleton.
  - **Changed:** "**output change**: stages 1–2 on by default (the input high-pass, the hiss, the clipper); a DC offset no longer reaches the limiter; C structure 112 → 120".
  - **Documentation:** the one re-bless.
- [x] T010 Re-bless **once**:
  - run `RR_DR60_BLESS=manifest cargo test -p rr_dr60_harness --test us2_cross_check canonical` and `RR_DR60_BLESS=readme cargo test -p rr_dr60_harness --test us2_cross_check readme -- --test-threads=1`;
  - check that `git diff crates/rr_dr60_harness/golden/manifest-v1.txt README.md` shows exactly the version, stages 1–2's entries and the counts;
  - run `cargo test -p rr_dr60_harness --test us1_manifest --test us2_cross_check` and `scripts/check-traceability.sh`, all green.

**Checkpoint**: the script prints every figure. `--check` passes, `--write` is byte-identical, and `--spice` agrees within 10 %. The registers have their skeleton rows and the CHANGELOG has its skeleton. T008 and T010 follow at the end of Phase 2.

*(As built: the core and the C layer are green. Three cross-checks are **red by design until T020**, because the records run ahead of the code: `canonical_text_matches_golden` (the library version), `committed_registers_pass` (A-046 – A-053 not yet cited by a manifest entry) and spec 010's `readme_output_clamp_statements` (it pins the version and the manifest counts 5 / 10, which become 3 / 12). The grid over every transistor rank reaches 637 Hz, so the registered range is 665–985 Hz for ranks R/S (the probable S parts). ngspice 47 gives an operating point of 1.71 / 1.08 / 2.60 V against S-002's 1.7 / 1.1 / 2.6 V, and an input corner of 824.2 Hz, +0.7 %.)* Commit: `chore(012): version 0.12.0, the mic-chain design script and A-046 – A-053 (records ahead of the code)`.

---

## Phase 2: Foundational (plan › Task order 1; research R-05, R-06, R-10)

**Purpose**: the settings, taps and C fields exist but are **inert**: `Settings::new` keeps the stages bypassed. The 0.11-default configuration and the harness rebase are in place. **Blocks every story.**

- [x] T011 [P] `crates/rr_dr60/src/mic.rs` (a new public module, `pub mod mic` in `lib.rs`): `pub const HISS_OFFSET_RANGE_DB: (f32, f32) = (-18.0, 12.0);` with rustdoc (A-048, FR-003, for UI sliders). No SPL constant.
- [x] T012 `crates/rr_dr60/src/settings.rs` (contracts/rust-api.md):
  - **`MicSettings`:**
    - `#[non_exhaustive]`, fields `capsule_enabled`, `amp_enabled`, `hiss_offset_db: f32`;
    - `DEVICE` (true, true, 0.0) and `BYPASSED` (false, false, 0.0);
    - rustdoc stating the level reference (0 dBFS at 1 kHz = the codec's full scale ≈ 90 dB SPL, 80–100, low, A-029) and the hiss figure.
  - **`Settings::mic`.** `Settings::new` sets **`MicSettings::BYPASSED` for now** (flipped in T029).
  - **`Tap::AfterMicCapsule` and `Tap::AfterMicAmp`**, appended, with rustdoc: host-rate, latency 0, ignored by the recorder.
  - **`Setting::MicHissOffsetDb`**, appended, with `range()` −18 … +12.
- [x] T013 `crates/rr_dr60/src/validate.rs`: `hiss_offset_db` must be finite and in −18.0 ≤ x ≤ +12.0, **checked whether or not the capsule is on**, else `Error::InvalidSetting(Setting::MicHissOffsetDb)`. Unit tests cover −18, +12, −18.001, 12.001, NaN and ±∞, with the capsule both on and off.
- [x] T014 `crates/rr_dr60/src/paths.rs`, `crates/rr_dr60/src/pipeline.rs`: the taps routed inert.
  - `Tap::AfterMicCapsule` and `Tap::AfterMicAmp` take the `AfterAgc`-like path (no conversion, no voice activation, no chain, no events, latency 0).
  - Until US1 the stages are identities, so both taps equal the sanitized input.
  - `max_events` is 0 at them, as at `AfterAgc`.
  - A unit test checks the latencies and the zero event bound.
- [x] T015 `crates/rr_dr60_ffi/src/lib.rs` (contracts/c-api.md), the C fields **inert**:
  - **Fields:** `mic_capsule_enabled` at 112, `mic_amp_enabled` at 113, `reserved_012 [u8; 2]` at 114, `mic_hiss_offset_db: f32` at 116.
  - **Assertions:** `size_of == 120`, `align_of == 8`, every new offset, and 116 + 4 == size.
  - **Mapping:** `to_settings` and `rr_dr60_settings_default` follow `Settings::new`.
  - **Refusals:** `struct_size = 112` refused with the invalid-argument status and the struct-size field.
  - **Enums:** `RrDr60SettingField::MicHissOffsetDb = 16`, taps `AfterMicCapsule = 9` and `AfterMicAmp = 10`.
  - **Header:** regenerated; the header drift check clean.
- [x] T016 `crates/rr_dr60_ffi/tests/c/smoke.c`: every existing case sets `s.mic_capsule_enabled = false; s.mic_amp_enabled = false;` after `rr_dr60_settings_default` (inert now; it keeps the hashes after the flip). Run `crates/rr_dr60_ffi/tests/c/run_smoke.sh` and `cargo test -p rr_dr60_harness --test ffi_parity`, both green.
- [x] T017 [P] `crates/rr_dr60_harness/src/configs.rs` (data-model.md › Harness configurations):
  - **The base.** `pub fn settings_011(rate)` is `Settings::new` with `mic = MicSettings::BYPASSED`.
  - **The rebase.** `settings_008` builds on it, and `"default_output" | "default_earphone" | "default_loud"` use it.
  - **New names:**
    - `"mic_only"`: `bypass_all` with mic `DEVICE`, tap `AfterMicAmp`;
    - `"mic_only_quiet"`: the same with the capsule off;
    - `"mic_capsule_only"`: capsule on at −12 dB, amp off, tap `AfterMicCapsule`;
    - `"default_mic"`: `Settings::new` with the voice activation off;
    - `"default_mic_quiet"`: the same with the capsule off;
    - `MIC_CONFIGS`.
  - **Explicit DEVICE.** Until T029 the mic names set `mic = MicSettings::DEVICE` explicitly, so they don't depend on the default.
  - **Unit tests:** every pre-012 named configuration has `mic == BYPASSED`.
- [x] T018 [P] Move the direct `Settings::new` uses in `crates/rr_dr60_harness/src/output_checks.rs`, `crates/rr_dr60_harness/src/smoke_reference.rs` and `crates/rr_dr60_harness/tests/output_edge_cases.rs` to `configs::settings_011` (R-06).
- [x] T019 [P] `crates/rr_dr60_harness/src/analysis.rs`:
  - `band_level_aes17(y, rate, lo, hi) -> f64`;
  - `THIRD_OCTAVE_CENTRES_20_20K`;
  - `prototype_deviation(impulse_response, rate, prototype_db: &[(f64, f64)]) -> Vec<(f64, f64)>`, by DTFT at the exact centres;
  - `exceedance_count(x, threshold) -> usize`;
  - `norm_xcorr_max(a, b, max_lag) -> (f64, f64)` (lag 0 and the maximum over ±max_lag).

  Each has a unit test against a closed form: a full-scale sine reads 0 dBFS; white noise's band level; an impulse's flat response; a shifted copy's correlation of 1 at its lag.
- [x] T020 Run T008 (stages 1–2's manifest entries) and T010 (the one re-bless) now that the controls exist. Restate spec 010's `readme_output_clamp_statements` in `crates/rr_dr60_harness/tests/us2_cross_check.rs`: it pins the manifest counts 5 / 10, which become 3 / 12 (stages 1–2 are assumed). Then run the seven golden tests, the C loop, `ffi_parity` and `cargo test --all-features`, all green.

**Checkpoint**: the settings, taps and C fields exist and the manifest is re-blessed once; nothing audible has changed.

*(As built:*
- *`Tap::is_front()` generalizes the one-pass's "after AGC" path to the three host-rate front taps (`RecordPath::front_only`, a `FrontStop`), so the flag `tap_after_agc` is now `front_tap`.*
- *The manifest's `bypass_control_anchor` destructures `mic` but asserts stages 1–2's controls only from T029, when the default turns them on.*
- *The three "not modeled" rule tests moved from stage 1 to stage 6.*
- *Stage 3's "not modeled here" note drops "the DC block and 13 kHz roll-off ahead of the peak circuit", now stage 2's.*
- *`us1_manifest` and spec 010's `readme_output_clamp_statements` are restated to 3 / 12; `mic_stages_us4_scenario_1` is new.*
- *The C smoke loop's 23 settings sites use a `settings_011()` helper; `configs::c_default_008` turns the two switches off; the C layer's, `ffi_parity`'s and `us2_bypass_tap`'s layout and tap pins are restated to 120 bytes and taps 9 / 10, with 11 refused.*
- *The registers cross-check needed two things as soon as the manifest called stages 1–2 modeled:*
  - *signal-chain rows 1–2's "Modeled by" marker (both rows rewritten with the corrected topology now, ahead of T045);*
  - *A-050 cited, by stage 1, whose hiss figure folds the amplifier's floor in.*
- *`manifest_checks::cross_check_negatives` also moved its not-modeled example to stage 6.*
- *Full suite (`--no-fail-fast`), the C loop, the header drift check and traceability are green.)* Commit: `feat(012): MicSettings, the taps and the C fields (inert), the 0.11-default configuration, stages 1–2's manifest entries`.

---

## Phase 3: User Story 1: the device's microphone sound, its band and its hiss (Priority: P1) 🎯 MVP

**Goal**: the input high-pass, the output coupling and the roll-off (with the clipper still linear), the deterministic hiss with its offset and per-recording streams, on by default.

**Independent Test**: `cargo test -p rr_dr60_harness --test us1_mic -- --nocapture` in the stages-only configurations, plus the core's `stages::mic` tests.

### Tests for User Story 1 (write first; expected red until T026–T029)

- [x] T021 [US1] Create `crates/rr_dr60_harness/src/mic_checks.rs` (`pub mod mic_checks` in `lib.rs`), the US1 checks, with constants copied from T004 and T005's print (dated):
  - **`band(rate)`:** an impulse through `mic_only_quiet`; `analysis::prototype_deviation` within ±0.1 dB at every centre above −40 dB (US1 AS1, SC-002). Each section's −3 dB point and the 1 kHz group delay are printed.
  - **`hiss_level(rate, offset)`:** 10 s of silence through `mic_only`; the band level within ±0.5 dB of the figure (+ offset) and the third-octave shape at 315–2500 Hz within ±1 dB (US1 AS2, AS3, SC-001).
  - **`streams(rate)`:** at `mic_capsule_only` over 480 000 samples, the cross-correlation for seeds 0 / 1 and for counters 0 / 1 (by recording twice) below 5/√N at lag 0 and 6/√N over ±64 (US1 AS4).
  - **`determinism(rate)`:** the same seed, counter and input in four partitions (1, 64, 4096 and an irregular one) are bit-identical.

  Each check returns `MeasurementResult` rows.
- [x] T022 [P] [US1] Create `crates/rr_dr60_harness/tests/us1_mic.rs`: the T021 checks at `configs::measurement_rates()`, plus an offset sweep (−18, −6, 0, +12) and the refused offsets. Run it: **red** (the stages are identities).
- [x] T023 [P] [US1] Core tests in `crates/rr_dr60/src/stages/mic.rs`, written before the implementation:
  - the keying (`key(seed, counter)` matches the script's two-bijection formula on fixed vectors; seed 0 gives a non-zero state);
  - the retry cap through a `force_retry_cap` hook returns a deterministic value;
  - release-only `#[ignore]`: on 10⁸ draws, the 4σ count within ±5 % of 6 334 and the 5σ count within ±40 % of 57 (US1 AS5);
  - each fitted section's −3 dB point equals the script's print;
  - the bypass identity.
- [x] T024 [P] [US1] Counter tests in `crates/rr_dr60/src/transport.rs` for **every row of research R-05's table**:
  - construction 0; `reset` and an accepted `reconfigure` keep it; a successful start rekeys and then increments;
  - a refused start (memory full, no slots, recording in progress) does not advance;
  - an empty recording discarded at the stop has consumed its stream;
  - a memory-full self-stop, erase, import, playback and one-pass calls leave it unchanged;
  - a zero-length call advances nothing.

  Run them: **red**.

### Implementation for User Story 1

- [x] T025 [US1] `crates/rr_dr60/src/stages/mic.rs`, `MicCapsule`:
  - **State.** xoshiro256++ state; `key(seed, counter)` = mix64(seed ^ MIC_STREAM) ^ mix64(counter ^ MIC_COUNTER_SALT), then four SplitMix64 outputs (A-049).
  - **The ziggurat.** Layer and sign from bits 0–7, mantissa from bits 11–63, tail uniforms `(r >> 11) + 1`, `detmath::{exp, ln}` in the tail and the wedge, each rejection loop capped at `RETRY_CAP` (8) returning the last candidate.
  - **The gain.** σ_r·`detmath::exp(offset·ln(10)/20)`, computed at construction.
  - **Methods:** `process(x)`, `rekey(counter)`, and `reset` (rekeys to the stored counter).
- [x] T026 [US1] `crates/rr_dr60/src/stages/mic.rs`, `MicAmp` without the clipper: HP_in (normalization folded in) → **identity in place of the clipper (US2 adds it)** → HP_out → LP, from `mic_coeffs` per rate, using `stages::first_order::FirstOrder` (state flushed below 1e-30), with `reset`. `MicFront { capsule: Option<MicCapsule>, amp: Option<MicAmp> }` is decided at construction from the switches.
- [x] T027 [US1] `crates/rr_dr60/src/paths.rs`:
  - `RecordPath::limited` becomes `front`: `sanitize_in` → capsule → amp → limiter, each step skipped when off;
  - `front_only(upto)` serves the two taps and `AfterAgc`;
  - `RecordPath::rekey(counter)`;
  - `reset` puts the sections at rest and rekeys to the stored counter.
- [x] T028 [US1] `crates/rr_dr60/src/transport.rs` and `pipeline.rs`:
  - **The counter** (`u64`, 0 at construction, carried through `reconfigure`).
  - **`start_recording`.** After `memory.open()` succeeds: `recorder.reset()`, `recorder.rekey(counter)`, then `counter += 1`.
  - **The one-pass record path.** Keyed to the counter's current value at construction, `reset` and `reconfigure`.
  - **Docs.** `StopOutcome::silence_fed_samples` rustdoc: zero-valued input; with stage 1 on the recording carries hiss there.
  - **Run:** T023 and T024 green.
- [x] T029 [US1] **Flip the default:** `Settings::new` uses `MicSettings::DEVICE`.
  - Remove T017's explicit `DEVICE` from the mic configurations.
  - **The audit** (R-06):
    - add `test_util::settings_011` to `crates/rr_dr60/src/test_util.rs`;
    - audit every `Settings::new(` in `crates/rr_dr60/src` (about 70), `crates/rr_dr60_ffi/src/lib.rs` (5) and the doctests in `pipeline.rs` and `settings.rs`;
    - rebase each one that checks pre-012 behaviour (for example `pipeline.rs`'s "the first 180 outputs == 0.0");
    - keep and restate each one that checks the default.
  - **The seed's docs.** Update `Settings::seed`'s rustdoc: it seeds the capsule's hiss, keyed per recording.
- [x] T030 [US1] **The bit-identity proof at the flip** (FR-010, SC-004, US3 AS1):
  - the seven golden tests, `crates/rr_dr60_ffi/tests/c/run_smoke.sh`, `ffi_parity` and `cargo test --all-features` are green with the default on;
  - `us1_mic` is green (T022);
  - the core tests are green, including the release-only `cargo test -p rr_dr60 --release stages::mic -- --ignored`.
- [x] T031 [US1] Early whole-engine checks (US3 AS1–AS3; the hiss is on by default from here), added to `mic_checks.rs` and `us1_mic.rs`:
  - **`equivalence(rate)`:** the first recording after construction, after `reset` and after `reconfigure` equals the one-pass under spec 008's coordinates, with the stop's flush carrying the hiss (SC-008);
  - **`rest_and_rekey(rate)`:** after `reset` the sections are at rest and the hiss restarts at the current counter's stream;
  - **`taps(rate)`:** `AfterMicCapsule` is the input plus white hiss and `AfterMicAmp` the shaped signal, both latency 0; every earlier tap's latency is unchanged.

  Also: `crates/rr_dr60_harness/tests/determinism.rs` gains `mic_partitions`, and `alloc_free.rs` gains the mic configurations.
- [x] T032 [P] [US1] Create `crates/rr_dr60_harness/tests/mic_matrix.rs`: the seven-rate report of the US1 rows (band deviations, −3 dB points, group delay, hiss level and shape, offsets, cross-correlations). Release-only `#[ignore]`: the 60-minute 10 s-window stability at 48 kHz (US1 AS5).

**Checkpoint**: SC-001, SC-002, SC-004, SC-005, SC-008. The default carries the band and the hiss; the seven guards are unmoved. Commit: `feat(012): stages 1–2's band and hiss, on by default (US1)`.

---

## Phase 4: User Story 2: very loud sounds overload the microphone amplifier (Priority: P2)

**Goal**: the memoryless asymmetric soft clipper between HP_in and HP_out, with the bound after stage 2.

**Independent Test**: `cargo test -p rr_dr60_harness --test us2_mic -- --nocapture` in `mic_only_quiet`.

### Tests for User Story 2 (write first; red against the linear stand-in)

- [x] T033 [US2] Hooks in `crates/rr_dr60/src/lib.rs` (`__test_hooks`), `pipeline.rs` and `stages/mic.rs`:
  - `MicMutation { no_input_hp, symmetric_clip, swapped_polarity, shared_stream, hiss_db_delta, clipper_off, force_retry_cap }`;
  - `Pipeline::__with_mic_mutation`;
  - `Pipeline::__mic_clip_trace(&mut self, input: &[f32], trace: &mut [(f64, f64)])`, which writes the engine's own clipper input and output per sample at the tap after stage 2.

  Inert apart from what US1 used.
- [x] T034 [US2] US2 checks in `crates/rr_dr60_harness/src/mic_checks.rs`, with the figures from T004's print:
  - **`static_curve`:** through the trace, the output equals the input exactly below k·L, never exceeds L, and the onsets and limits equal the script's (US2 AS1);
  - **`engine_onsets(rate)`:** 1 kHz tones at the printed start phase, levels stepped; onsets and limits within ±0.5 dB at the engine input, whatever `agc.knee_dbfs`;
  - **`bound(rate)`:** +40 dBFS tones, steps, a square toggling between ±L and random full-range input; every sample after stage 2 within L₊ + L₋, rounded up to f32 (US2 AS2, SC-003);
  - **`harmonics(rate)`:** a tone 6 dB above the first onset; H2 and H3 within ±0.5 dB of the discrete model, with the alias level reported (US2 AS3);
  - **`recovery(rate)`:** +13 dBFS for 50 ms then −20 dBFS against the `clipper_off` twin; the difference is exactly the decaying tail, and within ±0.1 dB by the printed settle time (US2 AS4).
- [x] T035 [P] [US2] Create `crates/rr_dr60_harness/tests/us2_mic.rs` with T034's checks at `measurement_rates()`. Run it: **red** (the clipper is the identity).

### Implementation for User Story 2

- [x] T036 [US2] The clipper in `crates/rr_dr60/src/stages/mic.rs` (FR-006; R-03):
  - `y = v` when −k·L₋ ≤ v ≤ k·L₊; otherwise `L₊·knee(v·inv₊, k)` for v > 0 and `−L₋·knee(−v·inv₋, k)` for v < 0, as branchless selects (reuse `stages::output::knee`, already `pub(crate)`);
  - the limits from `mic_coeffs`;
  - the mutants wired;
  - core unit tests: exact linearity, memorylessness, symmetric and swapped mutants.

  Then `us2_mic` green, and `mic_matrix` gains the US2 rows.

**Checkpoint**: SC-003. Commit: `feat(012): the microphone amplifier's asymmetric soft clipper (US2)`.

---

## Phase 5: User Story 3: the microphone path in the whole engine (Priority: P2)

**Goal**: the voice-activation margin, the limiter and codec interactions, the mutants and the cost.

**Independent Test**: `cargo test -p rr_dr60_harness --test mic_matrix vas_margin -- --nocapture`, `--test mutation mic`, and the solo `timing` runs.

### Tests for User Story 3 (write first)

- [x] T037 [US3] `crates/rr_dr60_harness/src/mic_checks.rs`:
  - **`vas_margin(rate, level, seconds)`:** the 0.12 default with the voice activation on (drop mode), read at `Tap::AfterVas`. Silence for 60 s. From the events: exactly the hang time kept, one pause, never resumed. The RMS margin at the voice-activation input (AES17) and the exceedance count are printed. The level-5 margin is at least 10 dB (US3 AS6, SC-010).
  - **`vas_offset_bisection(rate, level)`:** the offset at which the gate stops pausing, at 6 kHz only under `cfg(coverage)`.
  - **`limiter_ratio(rate)`:** twin engines (limiter on and bypassed) and two seeds; a +9 dBFS 1 kHz burst then 3 s of silence. Over the silence the ratio is at most 1 and non-decreasing, and the two seeds' trajectories agree within 4 f32 ULP (US3 AS8).
  - **`decoded_hiss(rate)`:** `default_mic` with the voice activation bypassed; record and play 10 s of silence; the decoded band level relative to the input's, at 8 kHz after resampling, is reported. With the voice activation on, the recording length is reported (US3 AS7).

  Add them to `us1_mic.rs` (named `whole_engine_*`) and to `mic_matrix.rs` (all rates; 600 s release-only). Run them: **red** where the wiring is missing, otherwise green; record which.
- [x] T038 [P] [US3] `crates/rr_dr60_harness/tests/mutation.rs`, `mod mic`: the five mutants each fail their row.
  - `no_input_hp` fails the band;
  - `symmetric_clip` and `swapped_polarity` fail the static curve;
  - `shared_stream` fails the counter cross-correlation;
  - `hiss_db_delta = 3.0` fails the hiss level.
- [x] T039 [P] [US3] `crates/rr_dr60_harness/tests/mic_edge_cases.rs`: every spec edge case.
  - silence gives the hiss, and silence with stage 1 off gives zeros;
  - DC is blocked (a DC step decays to zero after stage 2);
  - +40 dBFS is bounded;
  - NaN and subnormal input becomes 0.0 carrying the hiss alone;
  - loud input then silence with the hiss off reaches exact zeros with no subnormal;
  - the sensitivity setting leaves stages 1–2 unchanged;
  - the offset at +12 is accepted, and the voice-activation behaviour at it is reported;
  - a reset between recordings gives new hiss;
  - the limiter bypassed with the hiss on reaches the clamp.

### Implementation for User Story 3

- [x] T040 [US3] `crates/rr_dr60_harness/tests/timing.rs`:
  - **`mic_cost`** (`#[ignore]`, release): the speaker route at 48 kHz, codec bypassed, 4096 blocks, `default_output` with the stages on against bypassed, on −20 dBFS noise; a full-scale spread and an offset spread reported; the fastest of nine alternating runs. Assert ≤ 10 % on the development machine; the CI ceiling is 25 % (read an env var set by `ci.yml`'s timing step).
  - **`output_cost_mic`:** `output_cost`'s body with stages 1–2 on, printing the same line format; it asserts `limiter_state() == 0.0` at the end (R-09).
- [x] T041 [US3] Timing scripts and workflow:
  - `scripts/ab-timing.sh`: an optional second argument (the head build's test name, default `output_cost`); the default baseline becomes `c9f9994`; the comment says what each mode measures;
  - `.github/workflows/ab-timing.yml`: baseline `c9f9994` and a second step `scripts/ab-timing.sh c9f9994 output_cost_mic`;
  - `.github/workflows/ci.yml`: the release timing step sets the environment variable that `mic_cost` reads, so the 25 % CI ceiling applies there (spec FR-011; the analysis finding U1).
- [x] T042 [US3] Run the cost measurements solo on the development machine:
  - `cargo test -p rr_dr60_harness --release --test timing mic_cost -- --ignored --nocapture --test-threads=1`;
  - `scripts/ab-timing.sh` (bypassed against 0.11: ≤ 3 %);
  - `scripts/ab-timing.sh c9f9994 output_cost_mic` (on against 0.11: reported).

  Record the figures against the budgets in research R-09 › As measured. If `mic_cost` exceeds 10 %, stop and take R-09's fallback to review.

  *Done 2026-10-16:* the specified fastest-of-nine protocol read +15 – 18 % (run-order sensitive); R-09's fallback was not taken; by the user's decision `mic_cost` uses 45 strictly alternating runs (+5.4 – 5.9 %) and the cross-build "on" figure (+5.8 %) corroborates it. Figures in research R-09 › As measured.

**Checkpoint**: SC-006, SC-010; all five mutants caught. Commit: `test(012): the microphone path in the whole engine, the mutants and the cost (US3)`.

---

## Phase 6: User Story 4: the microphone path is documented and traceable (Priority: P3)

**Goal**: the README, rustdoc, registers, signal chain, sources, issue template and CHANGELOG state the behaviour, its figures and its confidence.

**Independent Test**: `cargo test -p rr_dr60_harness --test us1_manifest --test us2_cross_check` and `scripts/check-traceability.sh`.

- [x] T043 [US4] `crates/rr_dr60_harness/tests/us2_cross_check.rs`: `readme_mic_statements`, written first. The README's microphone section must state every item of US4 AS2, including the per-session seed. It must not contain the denylisted words or "accurate", "authentic", "measured". A-046 – A-053 must contain their key phrases. Run it: **red**.
- [x] T044 [US4] `README.md`: the microphone section (US4 AS2), covering:
  - the level reference and the host-gain arithmetic;
  - the input high-pass and how it thins the sound;
  - the hiss, its offset and switch, and the advice for hosts whose microphone already carries a floor (about −65 dBFS);
  - the default voice activation keeping only the hang time of hiss, and the offsets at which it stops pausing;
  - the clip limits, and a host rate of at least 16 kHz for loud material;
  - the per-session seed;
  - "modeled on the circuit with typical part values, not measured".

  Then grep the README, `docs/hardware/assumptions.md` and CLAUDE.md for "default" figures and restate each or label it "0.11 default" (R-06): A-042's peaks, the earphone 1.04–1.07, spec 011's route-output figures, spec 008's silence floor, spec 009's quiet decoder.
- [x] T045 [US4] The registers:
  - **New rows:** complete A-046 – A-053 (rationale, capture protocols).
  - **Amendments:**
    - A-005;
    - A-006: the topology is a two-stage transimpedance amplifier with a high loop gain;
    - A-019, A-028: the detector no longer sees host DC;
    - A-022: re-derived (the hiss's peaks about −51 dBFS over 600 s; room-noise placements tilted by A-046);
    - A-029: the corners corrected, the level chain closed, the self-noise figure; never "low loop gain";
    - A-036: the first-clipping polarity.
  - **Signal chain:** `docs/hardware/signal-chain.md` rows 1–2 (Identifier, Modeled by, figures).
  - **Sources:** `docs/hardware/sources.md`'s S-002 summary (R6 in the mic amplifier; R1 is the record lamp's resistor) and a candidate secondary source for the 2SD1819A ranks.
- [x] T046 [P] [US4] `.github/ISSUE_TEMPLATE/hardware_evidence.yml`: the captures from the reviews:
  - the 300 Hz / 1 kHz / 2.5 kHz tones at equal SPL;
  - the quiet room with the voice activation off;
  - the silent closet at sensitivity levels 1–5;
  - the hiss against a limited tone;
  - the clip asymmetry;
  - the capsule marking;
  - the board check of C1 and R2.
- [x] T047 [US4] Rustdoc for every new public item and for `Settings::seed`, `Tap::AfterAgc` and `StopOutcome::silence_fed_samples`, plus one paragraph in the crate docs.
  - Fill the CHANGELOG 0.12.0 regions (Added, Changed, Documentation), with the default-output change called out.
  - Run `readme_mic_statements` green, `us1_manifest`, `us2_cross_check` and `scripts/check-traceability.sh`.

**Checkpoint**: SC-007. No second canonical re-bless. Commit: `docs(012): the microphone path's records (US4)`.

---

## Phase 7: The C surface (FR-012; contracts/c-api.md)

- [x] T048 New cases in `crates/rr_dr60_ffi/tests/c/smoke.c`, written red with placeholder hashes:
  - (a) the 0.12 default one-pass at 48 kHz on a ±0.125 square wave plus silence;
  - (b) two recordings of silence with the hiss on and a `rr_dr60_reset` between them; the hashes differ;
  - (c) `struct_size = 112` refused;
  - (d) offsets 13.0 and NaN refused.
- [x] T049 Compute the hashes in `crates/rr_dr60_harness/src/smoke_reference.rs` (`SMOKE_MIC_DEFAULT_HASH`, `SMOKE_MIC_REC0_HASH`, `SMOKE_MIC_REC1_HASH`) from the Rust path. Pin them in `smoke.c` and in `ffi_parity`'s `smoke_reference_hash_is_pinned`. `crates/rr_dr60_harness/tests/ffi_parity.rs` gains `c_api_mic_matches_rust_at_all_rates`: the five mic configurations at the seven rates on an exactly representable stimulus. The C loop and `ffi_parity` are green.

**Checkpoint**: SC-009. Commit: `feat(ffi): the microphone fields' smoke and parity cases (012)`.

---

## Phase 8: The golden file, gates, reviews, the bless, the PR (plan › Task order, Phase 6)

- [ ] T050 The golden generator in `crates/rr_dr60_harness/src/golden.rs` (contracts/golden-format.md):
  - `MIC_STIMULI` (`silence_1s`, `voiced_1s`, `loud_tone`, `very_loud_tone`), `GOLDEN_MIC_CONFIGS`, the `default_mic_rec1` family (counter 1), `generate_mic`, `committed_mic`, `path_mic`;
  - the entries' `seed`, `counter` and `hiss_offset_db` fields;
  - the bless-time assertions;
  - `crates/rr_dr60_harness/tests/golden_mic.rs` with an empty placeholder file and a SHA placeholder, **red by design until T056**.
- [ ] T051 The gate **without** `golden_mic`:
  - `cargo test --all-features -- --skip golden_mic`, fmt, clippy;
  - the MSRV test build with `-D warnings`;
  - the header drift check;
  - `uv run tools/mic-chain/design_mic_chain.py --check` and a byte-identical `--write`;
  - `scripts/check-traceability.sh`.
- [ ] T052 **The ngspice gate**: `uv run tools/mic-chain/design_mic_chain.py --spice`, with the input corner within 10 % of the nodal model. Record the figures in research R-01 › Cross-checks (as run).
- [ ] T053 The solo release timing runs (`mic_cost`, `output_cost_mic`, and spec 008's worst block with the stages on), quoted in research R-09.
- [ ] T054 **The post-implementation hardware review** (`hw-emulation-reviewer`, the session's model): the as-built stages against the spec and the registers. Apply its findings before the bless. One review-driven manifest re-bless is allowed.
- [ ] T055 **The coach review** (`speckit-coach`, the session's model): the branch against spec, plan and tasks, including SC-007's reader test. Apply its findings.
- [ ] T056 **The bless, once, after the reviews**: `RR_DR60_BLESS=mic cargo test -p rr_dr60_harness --test golden_mic` (the bless-time assertions pass).
  - Pin the SHA in `tests/golden_mic.rs`.
  - In the same commit, add `golden_mic` to `.github/workflows/ci.yml` (the check job's golden step, the golden matrix, the iOS simulator) and to `scripts/ios-device-golden.sh`.
  - Add `--test mic_matrix` to ci.yml's release `--ignored` step.
  - Add the CHANGELOG's Added golden line.
- [ ] T057 The full gate with coverage: `cargo llvm-cov --all-features --workspace --fail-under-lines 80`, within 1 point of 0.11's 98.02 %. Quote the figure.
- [ ] T058 CLAUDE.md:
  - the 012 status line (what was built, the figures as the script and harness printed them, the lessons) and the spec 012 report commands;
  - the open decisions (stage 6 and stages 14–15 to be researched; the controls discussion; the product questions deferred until engine core development is complete).

  The spec's Status becomes "implemented on the branch".
- [ ] T059 **Ask the user**, then:
  - push the branch and open a draft PR `feat: the microphone capsule and amplifier (012, v0.12.0)` with the figures;
  - when CI is green (11 jobs plus `registers`), add the `ab-timing` label and quote the runner's figures (the bypassed A/B against the 3 % budget, the on A/B reported, `mic_cost` under the 25 % ceiling);
  - mark the PR ready; **ask before merging** (`gh pr merge --merge --delete-branch`).
- [ ] T060 After the merge: the spec's Status "merged" and CLAUDE.md's 012 line "complete on main" with the PR and the merge commit; then `/speckit-converge`.

---

## Dependencies & Execution Order

- **Setup (T001–T007, T009)** comes first. T002 → T003 → T004 → T005 → T006 in order (one script). T007 [P] needs T002–T005's print. **T008 and T010 run inside T020**, after the Foundational items exist.
- **Foundational (T011–T020)** depends on Setup and **blocks every story**. T011, T017, T018 and T019 are [P]. T015 → T016.
- **US1 (T021–T032)**: tests T021–T024 first (red), then T025 → T026 → T027 → T028 → T029 → T030 → T031. T032 [P] after T030.
- **US2 (T033–T036)** depends on US1 (the amplifier exists). T033 → T034 → T035 (red) → T036.
- **US3 (T037–T042)** depends on US1 and US2. T037 first; T038 and T039 [P]; T040 → T041 → T042.
- **US4 (T043–T047)** depends on the figures (Setup) and the behaviour (US1–US3). T043 first (red); T046 [P].
- **C (T048–T049)** depends on US1–US2 (the default output exists).
- **Phase 8** depends on everything. T050 → T051 → T052 → T053 → T054 → T055 → T056 → T057 → T058 → T059 → T060.

## Parallel examples

- **Setup:** T007 (registers) beside T008 (manifest), once the script prints.
- **Foundational:** T011, T017, T018 and T019 together (four files).
- **US1:** T022, T023 and T024 (tests in three files) after T021.
- **US3:** T038 (mutation) beside T039 (edge cases).

## Implementation strategy

- **MVP = US1.** The band and the hiss, on by default, with the bit-identity proof (T030) and the equivalence (T031). That is the audible core of the feature.
- **Then US2.** The clipper, a self-contained addition between two existing sections.
- **Then US3 and US4.** They need the figures and behaviour of US1–US2.
- **Then the C surface, then the gates, reviews and the single bless.**

The phase checkpoints are commit points. The user is asked before the push (T059) and before the merge.
