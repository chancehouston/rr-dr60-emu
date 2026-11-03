# Feature Specification: Host reads (the workbench's engine issues)

**Feature Branch**: `026-host-reads`

**Created**: 2026-11-03

**Status**: Draft reviewed 2026-11-03 (coach 1 – 10, hardware B1 – B9, product M1 – M7, applied at the defaults below); clarified 2026-11-03 (four questions, every answer the recommended one). **Spec approved by the owner 2026-11-03 (D1 – D9).** Plan reviewed (coach 1 – 11, hardware B1 – B12, product M1 – M6) and **approved by the owner 2026-11-03**, with D2's front taps reporting their own stages' settle. Tasks next.

**Input**: Engine issues #72 – #75, #77 – #79, filed by the desktop workbench (rr-dr60-workbench, features 001 – 006) under its constitution principle I: an engine gap becomes an engine issue, and the workbench does not hide it. Each asks the engine to publish a rule or a read that a host reaches today only by quoting crate-private code or the unpublished harness; #77 is a Swift divergence. **The re-render (#76) is not in this spec**: the draft reviews split it into its own feature (027) because it allocates, takes the i24 feed point, adds errors and a delta API, and the app's Rebuild needs it to return the rebuilt stored form, not a digest (product M1).

**Not on the app's 1.0 path** (product M7): every item here is appended and none is needed by app 1.0, which ships pinned to the engine's v0.92.0 tag. The workbench adopts 0.93.0 afterwards.

## Clarifications

### Session 2026-11-03

- Q: Should the public re-render (#76) move to its own spec, after app 1.0? → A: **Yes, spec 027.** It returns the rebuilt stored form and a new record for the app's Rebuild (product M1). 026 keeps the reads and the Swift slot fix (#77).
- Q: Does app 1.0 ship pinned to v0.92.0, with 026 (0.93.0) landing afterwards for the workbench? → A: **Yes.**
- Q: For the one-pass drain, document the gate-open scope, or add a flush call that holds the gate open? → A: **Document it** (D2 as written; no new call).
- Q: Does the meter manifest give each point's lag in host samples now? → A: **Yes** (D4 as written).

### Session 2026-11-03 (the plan reviews: restatements for the plan sign-off)

- **D2's figure** (coach 1, hardware B1 – B6). It is defined once: the silence until every kept sample is out and the output is within 10⁻⁵ of its own peak against a silent-input twin, while the gate stays open. It counts both converters (4·D) and uses the SVF's corrected settle rule, with the worst case about 2.43 s at 96 kHz rather than 1.83 s. **At the front taps it reports the front stages' settle, not 0** (the owner's decision at the plan sign-off, 2026-11-03; D2's "0 at the front taps" is superseded).
- **D3's matcher** returns the matched span. Its word class is Unicode `\w` with ASCII folding, and the regex oracle is permanent. AS2's "before the regex is deleted" is struck.
- **D4's lag and reach** are calls with the call (and the live route), not struct fields. `RrDr60MeterField` is 40 bytes.
- **D6's labels** are as plan R-06 lists them. The README keeps "—" by its generator's rule, so AS1 covers every non-empty cell.
- **D7's promise** runs against four revisions, 0.92's merge (2a170d4) included.

### Session 2026-11-03 (implementation restatements)

- **D2's settles are computed in the core** from the committed coefficient tables, through the engine's own sections, when the engine is built (`stages::drain`); there is no generated `drain_coeffs.rs` (plan R-02's file). The design scripts' `--drain` prints are the oracle: `design_voiceband.py --drain` prints N₄ = 160 and N₁₀ = 121, `design_mic_chain.py --drain` stage 2's settle per rate (90 … 926 host samples, 498 at 48 kHz), `design_band_filter.py --drain` the settle rule checked against brute-force impulse responses of the closed-form sections on the grid (worst ratio 0.996) and the bound at 20 Hz, Q 10 for the seven rates; the core's tests pin each printed figure.
- **D2's teeth** (AS1): "the figure under twice the silence needed" is asserted on the rows with one dominant term (every stage off but the conversion; the band filter at the slot's tap). On the full chain the bound is a sum of settles and reads about 4× the silence needed at the route output; those rows are reported.
- **D4's lag for the playback group** runs to the output chain's input: the chain's own group delay (0.31 ms on the speaker route and 0.07 ms on the earphone route at 48 kHz) is not included. The guide says so.
- **The C handle reads** take `RrDr60Pipeline *`, as every existing handle read does (contracts/c-api.md's `const` restated).
- **D2's definition is the impulse rule** (hardware post-implementation B1): each kept sample's response settles within 10⁻⁵ of that response's own peak. A rule relative to the output's overall peak cannot be bounded for a high-pass (as the frequency falls, the steady output vanishes while the transient of a cut does not). An input cut abruptly on low-frequency content rings up to about twice the figure at the front taps (stage 2 at 48 kHz: 904 against 498 samples on a 40 Hz tone cut on a peak; 1828 against 926 at 96 kHz); past the front taps the sum covers it. The rustdoc, the guide and the CHANGELOG say so; `us2_drain` asserts on an impulse ending the input and reports the tone cuts. Flagged to the owner for the merge sign-off.
- **D3's word class** is `char::is_alphanumeric` or `_` with ASCII folding, not a generated Unicode `\w` table (plan sign-off). The differences (superscript digits are word characters here, combining marks are not) are documented in the module and pinned by `us4_wording::as5_documented_differences`; on the engine's own strings the two agree (`as2_differential`).
- **D4's lag excludes the output chain's group delay** because it depends on the route at the call's end and is small (about 0.3 ms at 1 kHz, about 1 ms at most in the voice band); the guide states it.
- **US1 AS3** is read through the identity of the gate decisions: with equal events (asserted) the mapped positions with the coder on equal those with it off, whose map is spec 020's (its tone markers within 0.75 device samples, `us3_record_timeline`), so a separate marker reading adds nothing.
- **D5's bug is latent** while the band filter is the only component (it sits at 0 – 3, where the old loop read): the Swift tests show the restore and the digest, and fail on a future component at another offset only.
- **The red phase**: tests and code landed together in this feature's commits; the design scripts' `--drain` prints came after the core's figures and match them.

## Decisions for sign-off

The owner reads this section first. Each decision gives the recommendation; the alternatives are in the Assumptions section.

| # | Decision | Recommended |
|---|---|---|
| D1 | **Where the gate and the coder run on the one-pass** (#72, #78). Three public functions:<br>• `Settings::one_pass_vas_runs()`: `vas.enabled`, and the tap at the gate, the decoder or after stage 10 (`paths.rs`'s rule with the tap honoured).<br>• `Settings::one_pass_codec_runs()`: the coder on, and the tap at or after the decoder.<br>• `Pipeline::one_pass_report_latency_samples()`: the latency the gate's reports align with. That is `latency_samples()` without the coder's 180-sample term: the same single rounding (`pipeline.rs`), so exact.<br>The rustdoc states the recorder's rule (the gate runs whenever it is enabled; the coder whenever it is on: the recorder ignores the tap). `Tap::is_front` and `Tap::is_after_stage_10` become public. The rustdoc of `timeline::onepass_output_to_input` names the new latency as the one to pass. C: `rr_dr60_settings_one_pass_vas_runs`, `rr_dr60_settings_one_pass_codec_runs`, `rr_dr60_one_pass_report_latency_samples`. | **Publish the one-pass rules, named as such** |
| D2 | **A one-pass drain figure** (#73). `Pipeline::one_pass_drain_samples()`: the host input samples of silence after the input that bring every **kept** sample to the output **while the gate stays open**, at the current settings and tap. It is the sum, in host samples, of: the front's settle (stage 2's corners, and the band filter's at its applied values, the remaining ramp included); the conversion both ways; the record stage's settle; the coder's 180-sample delay when it runs; stage 10's settle when it runs; and `N_OUT` when the chain runs. Each settle uses the −100 dB pole-envelope rule `N_OUT` uses, and the sum is a bound for the cascade. At the front taps it is the settle of the front stages that run (0 at the capsule's tap). *(Restated 2026-11-03, see Clarifications.)* **Excluded**, and the rustdoc says so:<br>• the limiter's charge (a state, not a tail: a gain on silence outputs zero);<br>• the hiss (a steady source);<br>• the decoder's idle output (quiet, never zero);<br>• pending schedule entries.<br>**In drop mode** a gate that pauses before the kept samples are out holds them until the next resume. The figure is stated for mute mode, the gate off, or a hang that outlasts it. No new flush call. With a 20 Hz, Q 10 band filter the band term is about 2.44 s at every rate. *(Restated 2026-11-03, see Clarifications.)* The settle is the impulse rule: each kept sample's response within 10⁻⁵ of that response's peak. *(Restated 2026-11-03, see Clarifications.)* `drain_samples()` keeps its meaning, the player's drain. C: `rr_dr60_one_pass_drain_samples`. | **A tap-aware bound, gate-open scope** |
| D3 | **The wording lists in the core, in two tiers** (#74). `rr_dr60::wording` holds:<br>• `ENGINE_STRINGS`: the harness's manifest list unchanged (stems, phrases, suffixes);<br>• `CLAIMS`: "accurate", "authentic", "reveal", "uncover", "hidden voice", and "measured" with its "not measured" exception;<br>• two matchers, `first_hit(text)` and `first_claim_hit(text)`.<br>The matching rule is stated: whole words; ASCII case-insensitive; the leftmost hit in the text wins; word characters as the harness's regex `\b` (Unicode). The context lists of the guide's sections stay in their tests ("events" and "measured" are legitimate in the guide). **The scope** (product M3): the rustdoc says these are "the words the engine's own strings do not use; the engine describes signal processing only; not a store or copy check: hosts layer their own". It never explains a word as a false claim. The `wording` module's own data is exempt from the check. C: `rr_dr60_wording_first_hit(text, len, kind, *entry, *start, *end)`: invalid UTF-8 refused, the entry a static NUL-terminated string or NULL, the span in bytes. *(Restated 2026-11-03, see Clarifications.)* Word characters are `char::is_alphanumeric` or `_`, with the documented differences from the regex's `\w`. *(Restated 2026-11-03, see Clarifications.)* | **Two tiers, scoped, with the harness's rule** |
| D4 | **The meter read-out described** (#75). `rr_dr60::meters::FIELDS`: one constant entry per `Meters` field, in declaration order, with the identifier, the unit, one neutral sentence, the kind (a maximum over the call, a count, or a state at the call's end), the side (input or output); the point's lag is a call, `Pipeline::meter_lag_samples(field, call)`, in host samples, to the output chain's input. *(Restated 2026-11-03, see Clarifications.)* An exhaustive destructure makes a new field fail to compile until it is described. **Reach is per call** (hardware B5, coach 5): `Pipeline::meter_reached(field, MeterCall)`, with `MeterCall` one of `OnePass`, `Record` or `Play`. It is the engine's own rule from its construction-time state, so a live bypass that still runs counts, and the output clamp's setting folds into its count's rule. An unreached field reads its rest value. C: `RrDr60MeterField` (a sized structure under spec 022, 40 bytes: `struct_size`, `index`, three static string pointers, `kind`, `side`), `rr_dr60_meter_field_count`, `rr_dr60_meter_field`, `rr_dr60_meter_reached`, `rr_dr60_meter_lag_samples`. *(Restated 2026-11-03, see Clarifications.)* | **A meter manifest, reach per call** |
| D5 | **The Swift slot store** (#77). The Swift `ReRender` restores every component's stored parameters at `parameter_offset + i`, whatever the component, then the component, as the harness does. Its `fill` floors the missing samples to whole frames with the coder on (hardware B7). It is a Swift-only change, tested by digest and by the engine's state after the re-render. The public re-render is 027's. | **Align Swift now** |
| D6 | **The stage manifest's words, two vocabularies** (#79, product M2, hardware B9). `token()` (the canonical text's: `not_modeled`, `not_applicable`, …) and `label()` (display: "not modeled", "checked against a real unit", never "checked" alone, "not applicable" rather than "—" or empty), on `StageStatus`, `StagePath` and `Confidence`. The canonical text's generator, the manifest example and the C smoke call them, so the four copies today become one. C: `rr_dr60_manifest_word(kind, value, form, *out)`, refused with `NO_SUCH_*` for an unknown value. | **Tokens and labels, one source** |
| D7 | **No audio change, appended only** (coach 7). Every golden guard and smoke hash is unmoved. The control manifest's canonical text is byte-identical, and the stage manifest's moves in its version line only. No existing struct changes. `CONTROL_COUNT`, `MAX_CHANGES_PER_CALL`, `RR_DR60_ABI_VERSION` and every existing struct size are asserted unchanged. The appended C items are the functions of D1 – D4 and D6, `RrDr60MeterField` and the new statuses, numbered from 44, and they stay out of the frozen `--abi` rows (025's precedent). `scripts/abi-promise.sh` runs against 12a3c45, f5a0245 and 97bbdc2 by full id. Version 0.93.0. | **Appended only; 0.93.0** |
| D8 | **Threads and real time**: every new read is constant-time with no allocation. The pure functions are safe on any thread. Every C entry point is panic-guarded. | **As 025** |
| D9 | **The Swift package** wraps the meter manifest and the manifest's words. Afterwards the workbench pins the new commit and replaces its mirrors (its own spec). The app keeps its own lists and its pin for 1.0 (product M3, M7). | **Two wrappers; the hosts follow** |

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A host reads where the gate and the coder run (Priority: P1)

**Independent Test**: the rules against behaviour, at seven rates.

1. **Given** every tap × gate on/off × coder on/off, **Then** `one_pass_vas_runs()` is true exactly when a render of a stimulus with a gap reports a pause in drop mode and a region in mute mode (`us1_host_reads::as1_vas_runs`).
2. **Given** coder on/off twins, **Then** `one_pass_codec_runs()` is true exactly when an impulse's onset moves by 180 · rate / 6000 host samples (`us1_host_reads::as1_codec_runs`). The rows `AfterDecoder` with the coder off and `AfterInsert`/`AfterRecord` are pinned.
3. **Given** tone markers through drop-mode renders with the coder on, **Then** the reports mapped with `one_pass_report_latency_samples()` land within 0.75 device samples of the markers (spec 020's oracle), at seven rates (`us1_host_reads::as1_report_latency`).

### User Story 2 - A host knows how much silence to feed (Priority: P2)

1. **Given** a render in mute mode, and with the gate off, followed by `one_pass_drain_samples()` of silence, **Then** every kept sample is emitted (produced ≥ input + latency). On linear configurations an impulse as the input's last sample has settled within 10⁻⁵ of its response's peak within the figure, which is under twice what it needs on the rows with one dominant term; a low tone cut on a peak is reported (`us2_drain::as1_emits_and_settles`). *(Restated 2026-11-03, see Clarifications.)*
2. **Given** a drop-mode render whose input ends inside the hang, **Then** the samples held at the pause are counted and documented, not emitted (`us2_drain::as2_drop_mode_held`).
3. **Given** the front taps, the figure is the settle of the front stages that run (0 at the capsule's tap). *(Restated 2026-11-03, see Clarifications.)* **Given** a 20 Hz Q 10 band filter, it bounds the ringing at seven rates (`us2_drain::as3_taps_and_band_filter`).

### User Story 3 - A host describes the meters (Priority: P2)

1. **Given** every `Meters` field, **Then** `FIELDS` describes it, in order (`us3_meters_manifest::as1_fields`).
2. **Given** every field × call × configuration on a stress set, **Then** an unreached field never leaves its rest value. A reached field moves on its kind's stimulus, except the fields whose reached value can equal rest (the hang at full, the pause flag false, the battery factor), which are pinned by their rule (`us3_meters_manifest::as2_reach`).

### User Story 4 - Words, from one source (Priority: P3)

1. **Given** every stage, **Then** `token()` reproduces the canonical text byte for byte (`us2_cross_check canonical`), and `label()` the README table (`us2_cross_check readme`). No copy of the words remains in the harness, the example or the C smoke (a source scan).
2. **Given** every engine string (both manifests, the guide's generated blocks) and a fuzz corpus, **Then** the core matchers agree with the harness's regex, which stays as the oracle *(Restated 2026-11-03, see Clarifications.)* (`us4_wording::as2_differential`). The harness's tests use the core lists (`us4_wording::as3_one_source`).
3. **Given** a record with an empty slot and a non-default store, **Then** the Swift `reRender` gives the harness's digest, and the engine's state afterwards holds the store (`swift test ReRenderTests`).

### Edge Cases

- **`AfterDecoder` with the coder off**: the gate runs and the coder does not.
- **A band filter mid-ramp**: the drain uses the smallest k and φ of the current and the target values, plus the remaining ramp. *(Restated 2026-11-03, see Clarifications.)*
- **A live bypass of stage 11 that still runs**: `amp_clip_count` is reached.
- **A word inside a longer word**: not a hit, on whole words.

## Requirements *(mandatory)*

- **FR-001**: D1's three functions and their C functions (`us1_host_reads`, `ffi_parity host_reads`).
- **FR-002**: D2's drain figure, its exclusions and its scope (`us2_drain`, `ffi_parity host_reads`).
- **FR-003**: D3's two tiers and matchers, the harness using them (`us4_wording`).
- **FR-004**: D4's meter manifest, exhaustive, reach per call, with its C structure and functions (`us3_meters_manifest`, `ffi_parity meters_manifest`, `abi_tests`).
- **FR-005**: D5's Swift alignment (`swift test`).
- **FR-006**: D6's tokens and labels from one source (`us2_cross_check`).
- **FR-007**: D7: every audio guard unmoved, the C items appended, the promise script against four revisions *(Restated 2026-11-03, see Clarifications.)* (`us2_abi`, CI's promise steps).
- **FR-008**: Every new string passes D3's `first_hit` (the module's own data exempt).

## Success Criteria *(mandatory)*

- **SC-001**: Each mirror the workbench lists for #72 – #75, #77 – #79 has a named engine item and a test that pins it.
- **SC-002**: The drain figure's three scenarios hold at seven rates.
- **SC-003**: No audio guard moves; the promise script passes against 12a3c45, f5a0245, 97bbdc2 and 2a170d4. *(Restated 2026-11-03, see Clarifications.)*

## Assumptions

- **Alternatives**:
  - **D1**: a single `runs_stage(n)` (rejected: the run rules are not per stage).
  - **D2**: fold it into `drain_samples()` (rejected: it changes a published figure); a one-pass flush call that holds the gate open, like the recorder's stop (deferred: hardware B1's question).
  - **D3**: one global list (rejected: the guide's legitimate words).
  - **D4**: rustdoc only; reach from `Settings` alone (rejected: the call decides).
  - **D6**: one vocabulary (rejected: "checked" alone overstates).
  - **The re-render**: kept here (rejected: its own feature, 027, after app 1.0).
- The engine is at 0.92.0 with spec 024's slot.
