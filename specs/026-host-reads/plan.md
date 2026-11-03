# Implementation Plan: Host reads

**Branch**: `026-host-reads` | **Date**: 2026-11-03 | **Spec**: [spec.md](spec.md) (approved by the owner 2026-11-03) | **Plan approved by the owner 2026-11-03**

## Summary

Six appended publications, each of a rule or a figure the engine already has inside it:
- D1: the one-pass gate and coder rules, and the report latency;
- D2: a tap-aware one-pass drain bound;
- D3: the wording lists and their matchers in the core;
- D4: a meter manifest with reach per call;
- D5: the Swift slot-store alignment;
- D6: the stage manifest's tokens and labels.

There is no audio change, every C item is appended, and the version is 0.93.0.

## Technical context

Rust 2024, MSRV 1.85, `no_std` core; C API by cbindgen; Swift package. No new dependencies in the core. The harness keeps `regex` for D3's differential test only.

## Constitution check

- **Portable, deterministic, real-time safe.** Every new read is constant-time and allocation-free (D8). D3's matchers allocate nothing: they scan bytes, with word boundaries by a `char` walk.
- **Traceable figures.** D2's settle lengths are pinned to the design scripts' new `--drain` prints (tasks of their own, each recomputed by a test):
  - stage 2's corners: `tools/mic-chain/design_mic_chain.py --drain`;
  - the record band-pass: `tools/filter-design/design_voiceband.py --drain`;
  - the band filter: computed at run time from its applied pole radius, with the formula pinned against `tools/band-filter/design_band_filter.py --drain`.
- **Claims policy.** D3's rustdoc follows product M3's wording. D6's labels never shorten "checked against a real unit".
- **C ABI (spec 022).** One new sized structure (`RrDr60MeterField`, published list `[40]`), statuses 44 – 48, everything else appended. `abi-promise.sh` runs against four revisions (R-08).

## Research

The R-items are mirrored in [research.md](research.md); the types in [data-model.md](data-model.md); the C surface in [contracts/c-api.md](contracts/c-api.md); the commands in [quickstart.md](quickstart.md).

- **R-01 D1.**
  - `Settings::one_pass_vas_runs(&self) -> bool` is the expression in `RecordHalf::new` with `honour_tap = true`, moved into one `const fn` that `RecordHalf::new` calls, so there is one rule.
  - `Settings::one_pass_codec_runs(&self) -> bool` is the `codec_runs` expression in `paths.rs:620`, which `paths.rs` then calls.
  - `Tap::is_front` and `Tap::is_after_stage_10` become `pub const fn`.
  - `Pipeline::one_pass_report_latency_samples()` is `latency_samples()` less `paths::codec_delay_host_samples(rate)` when `one_pass_codec_runs()`. That is exact, because the coder's term is added after the single rounding (`pipeline.rs:1783`).
  - C: three functions taking a settings pointer or a handle, with an `out` and the 022 order of refusals.
- **R-02 D2** (restated by the plan reviews: hardware B1 – B6, coach 1, 6).
  - **The definition**: the host input samples of silence after the input until every kept sample is out and the output is within 10⁻⁵ of the stimulus's own output peak against a silent-input twin (same counter and seed), while the gate stays open.
  - **At the front taps** the figure is the settle of the front stages that run: 0 at `AfterMicCapsule`; stage 2's at `AfterMicAmp`; plus the band filter's at `AfterInsert` and `AfterAgc`. The owner's decision at the plan sign-off, 2026-11-03.
  - **The sum** (host samples):
    - N_front: stage 2's three first-order sections at the host rate. Plus N_band, the band filter's settle, when the slot holds it.
    - Then, where `l ≠ m`: 4·D, both converters' full responses (B1: the player's 2D covers one), plus ⌈(N₄·[record stage] + 180·[coder] + N₁₀·[stage 10] + 1)·m/l⌉. N₄ = 160, N₁₀ = 121 device samples (reproduced); the + 1 is the device grid's phase.
    - Plus `N_OUT` when the chain runs. At 6 kHz, the device terms directly.
  - **N_band** for the TPT SVF:
    - g = tan(πf/fs), k = 1/Q, and the pole radius from ln r = ½(ln1p(g² − gk) − ln1p(g² + gk)) by `detmath`.
    - N_band = ⌈15.3 / −ln r⌉ + 4. That covers the near-repeated poles at Q ≈ 0.5, where ⌈ln 1e-5 / ln r⌉ reads up to 1.44× short (B2). The worst case is about 2.43 s at 96 kHz.
    - During ramps, the box bound kφ_min = min(k)·min(φ(g_cur), φ(g_tgt)), φ = g/(1 + g²), plus the longer remaining ramp (B3).
    - **The band term is computed on read** from the filter's g, k and ramp counters (O(1)); the static terms are cached at construction and `reconfigure` (B4).
  - **The prints** (coach 6): `design_band_filter.py --drain` prints the rule and asserts it against brute-force impulse responses on the grid (7 rates, 20 Hz – 0.45·fs, Q 0.5 – 10, four mixes; worst ratio ≤ 1.0). `design_voiceband.py --drain` prints N₄ and N₁₀. `design_mic_chain.py --drain` prints stage 2's per-rate settle. *(Restated 2026-11-03: no generated file; the core computes the settles from the committed tables and its tests pin the prints. Spec Clarifications.)* A test recomputes each print (008's lesson).
  - **The oracles** (B5):
    - A linear twin's residual against its silent twin, ≤ 10⁻⁵ of its output peak. The stimulus is a tone at the filter's frequency ending on a peak, plus a burst.
    - Rows: the stages-off configuration, the band filter at `AfterInsert`, the record stage, the coder off.
    - A check with teeth: at half the dominant term, the residual exceeds the bound.
    - The default engine's residual is reported, not asserted. The prefix identity is dropped: it holds by causality.
  - C: `rr_dr60_one_pass_drain_samples(handle, *out)`.
- **R-03 D3** (restated: hardware B9, B11, coach 4, 10).
  - `rr_dr60::wording` holds the lists:
    - `ENGINE_STRINGS` (the harness's stems, phrases and suffixes, verbatim);
    - `CLAIMS`, each entry with its explicit forms: "accurate", "accuracy", "authentic", "authenticity", "reveal", "reveals", "revealed", "uncover", "uncovers", "hidden voice", "hidden voices", and "measured" with the "not measured" exception.
  - It has two functions per tier: `first_hit(text) -> Option<Hit>`, with `Hit { entry, start, end }` (the matched byte range), and `first_claim_hit`.
  - **The rule**:
    - words are runs of `char`s in Unicode `\w` (alphabetic, marks, decimal digits, connector punctuation, ZWJ/ZWNJ), the regex crate's class, via a generated table;
    - case folding is ASCII only, as the lists are ASCII;
    - the leftmost hit wins.
  - **The oracle**: the harness's regex rebuilt as `(?i-u:…)` with Unicode `\b`. It is compared by span on every engine string and on 10⁵ seeded fuzz strings drawn from both classes, with the excluded cases pinned as documented differences (the Kelvin sign, long ſ).
  - `CLAIMS` is compared against the four copies in `us2_cross_check.rs`, which then call the core. The guide's substring checks stay: no check is weakened.
  - The regex stays a harness dependency (`changelog_has_version` uses it), and the oracle is permanent. Spec AS2's "before the regex is deleted" is restated.
  - The rustdoc says: "the words the engine's own strings do not use; the engine describes signal processing only; not a store or copy check, and the claims list is a subset of a host's".
  - C: `rr_dr60_wording_first_hit(ptr, len, kind, *out_entry, *out_start, *out_end)`, with `RR_DR60_WORDING_ENGINE_STRINGS` and `RR_DR60_WORDING_CLAIMS`.
  - The functions are bounded (O(text)) and allocation-free.
- **R-04 D4** (restated: hardware B7, B8, product M2, coach 8).
  - `rr_dr60::meters` is a **new** public module; `Meters` stays in `pipeline.rs`.
  - `MeterField { identifier, unit, sentence, kind, side }` and `FIELDS: [MeterField; 13]`.
  - `Pipeline::meter_reached(field, call) -> bool` reads the real gates:
    - `front_tap` and `FrontStop` (`limiter_input_peak` stays at rest at the capsule tap);
    - `run_limiter`;
    - `run_vas`;
    - the point being processed (at a front tap, `record_filter_peak` and `adc_clamp_count` are written as 0 and not reached);
    - `meter_playback`, `meter_route` and `meter_amp`;
    - `run_amp`, plus the **live route** at the call's end for `amp_clip_count`;
    - the clamp option for `output_clamp_count`.
  - `Pipeline::meter_lag_samples(field, call)`:
    - the front points: 0;
    - the device-rate points (stages 4 – 5): round(D + τ₄·rate/6000), or the D1 latency on the one-pass's gate points;
    - `reconstruction_peak` on `Play`: the playback latency; on the one-pass, the latency;
    - `route_peak`: plus the chain's 1 kHz group delay, by route;
    - on `Play` at speed level ≠ 3 the rustdoc points to `timeline::place_at`.
  - The spec's "lag in host samples" in the struct becomes this call.
  - The units: "full scale" (linear peaks, never dBFS), "dB", "fraction of hold", "count (device samples)", "count (host samples)", "device samples", "state", "factor".
  - The 13 sentences (product M2's seeds; each passes both tiers, a test):
    - `input_peak`: "The largest input sample of the call, before the microphone stages."
    - `limiter_input_peak`: "The largest sample entering the limiter during the call."
    - `gain_reduction_db`: "The deepest attenuation the limiter applied during the call."
    - `hold_charge`: "The limiter's hold charge at the call's end, as a fraction of the model's 40 dB hold."
    - `record_filter_peak`: "The largest sample after the record filter, before the converter's clamp, during the call."
    - `adc_clamp_count`: "The device samples the converter's clamp changed during the call."
    - `vas_paused`: "Whether the voice-activated pause is holding at the end of the call."
    - `hang_remaining_samples`: "The device samples left before the voice-activated pause holds, at the call's end."
    - `reconstruction_peak`: "The largest sample after the playback reconstruction during the call."
    - `route_peak`: "The largest sample at the route's output during the call; it can exceed 1.0 on the speaker route."
    - `output_clamp_count`: "The host samples the optional output clamp changed during the call."
    - `amp_clip_count`: "The host samples the amplifier's soft clipper changed during the call."
    - `battery_ceiling`: "The amplifier's ceiling factor for the battery voltage the user set."
  - C: `RrDr60MeterField`, 40 bytes:
    - `struct_size` u32 at 0 and `index` u32 at 4;
    - `identifier`, `unit` and `sentence` pointers at 8, 16 and 24;
    - `kind` u32 at 32 and `side` u32 at 36;
    - published list `[40]`.
  - The functions: `rr_dr60_meter_field_count`, `rr_dr60_meter_field`, `rr_dr60_meter_reached(handle, index, call, *out)` and `rr_dr60_meter_lag_samples(handle, index, call, *out)`.
- **R-05 D5.** In `swift/Sources/RRDR60/ReRender.swift`, the slot loop runs over every component's parameters at `parameter_offset + i` (from the C component list), whatever the component, then calls `set_component`. `fill` floors `missing` to whole frames with the coder on. The test is a record with an empty slot and a non-default store: the digest equals the harness's, and `component_parameter_value` after the re-render equals the store.
- **R-06 D6.**
  - `StageStatus::{token, label}`, `StagePath::{token, label}` and `Confidence::{token, label}`, as `pub const fn`. The labels are lowercase; the host capitalizes:
    - status: "not modeled", "assumed", "sourced", "checked against a real unit";
    - path: "record", "playback", "record and playback" (token `both`);
    - confidence: "not applicable", "low", "medium", "high". The rustdoc says a display adds the noun.
  - The README generator renders the confidence "not applicable" cell as "—" by its own table rule (product M1), so the README stays byte-identical; spec AS1 reads "every non-empty cell".
  - `manifest_checks.rs:906-1000`, `examples/manifest.rs` and `smoke.c` call them; `smoke.c` calls them through `rr_dr60_manifest_word`.
  - A source scan finds no copy left.
  - **The order** (coach 9):
    1. Move the generators to `token()` and `label()` against the unbumped golden, asserted byte-identical.
    2. Bump the version.
    3. Re-bless the manifest, asserting that only its version line differs.
- **R-08 C** (coach 2, 3; hardware B12).
  - The statuses:
    - 44 `NO_SUCH_METER_FIELD`;
    - 45 `NO_SUCH_METER_CALL`;
    - 46 `INVALID_UTF8`;
    - 47 `NO_SUCH_WORDING_KIND`;
    - 48 `NO_SUCH_MANIFEST_WORD`.
  - The refusal order:
    - constant-data reads (`meter_field`, `wording_first_hit`, `manifest_word`): NULL, then size (for the sized struct), then index or kind;
    - handle reads (`one_pass_report_latency_samples`, `one_pass_drain_samples`, `meter_reached`, `meter_lag_samples`): handle, then NULL, then index and call;
    - the settings readers (`rr_dr60_settings_one_pass_vas_runs` and `_codec_runs`), by 025's rule: `struct_size` equal to `SETTINGS_SIZE` (022 D13), then the field bytes at `offset_of!` offsets, never a `&RrDr60Settings`, with a bool byte other than 0 or 1 and a bad tap refused (`INVALID_ARGUMENT`). Miri covers it.
  - Every entry point is panic-guarded, tested as in 022.
  - `abi-promise.sh` runs against 12a3c45, f5a0245, 97bbdc2 **and 2a170d4** (0.92's merge), by full ids.
- **R-07 Records.**
  - The guide gains sections for D1 – D4 and D6.
  - The CHANGELOG lists them under Added.
  - Also: the README's C list, the Swift wrappers (D9), the header, the version bump (`Cargo.toml` ×4, `RR_DR60_VERSION_MINOR` 93), the manifest's version line re-blessed, and the traceability audit.

## Traceability

| Item | R | Tests |
|---|---|---|
| D1, FR-001 | R-01, R-08 | `us1_host_reads::{as1_vas_runs, as1_codec_runs, as1_report_latency}`, `ffi_parity host_reads` |
| D2, FR-002, SC-002 | R-02 | `us2_drain::{as1_emits_and_settles, as2_drop_mode_held, as3_taps_and_band_filter}`, the scripts' `--drain` pins |
| D3, FR-003, FR-008 | R-03 | `us4_wording::{as1_lists, as2_differential, as3_one_source, as4_new_strings_pass}` |
| D4, FR-004 | R-04, R-08 | `us3_meters_manifest::{as1_fields, as2_reach, as3_lag, as4_sentences_pass}`, `abi_tests meter_field`, `ffi_parity meters_manifest` |
| D5, FR-005 | R-05 | `swift test ReRenderTests` |
| D6, FR-006 | R-06 | `us2_cross_check::{canonical, readme}`, `us4_words::{as1_tokens_labels, as2_no_copies}` |
| D7, FR-007, SC-003 | R-07, R-08 | `us2_abi`, the four promise steps, the sixteen golden guards |
| D8 | R-08 | `abi_tests panic_guard_host_reads`, `alloc_free host_reads` |
| D9, SC-001 | R-07 | `cd swift && swift test`, the workbench's later spec |

## Testing

Each acceptance scenario's named test (the spec), plus:
- `ffi_parity host_reads` and `ffi_parity meters_manifest`;
- `abi_tests` for `RrDr60MeterField` (sizes, the published list, painted padding, Miri);
- `alloc_free host_reads`;
- the C smoke's `--host-reads` against `examples/host_reads_rows.rs`;
- the promise steps in `ci.yml` (four revisions; restated 2026-11-03);
- the design scripts' `--drain` prints pinned by a test that recomputes them (008's lesson).

## Project structure

```text
crates/rr_dr60/src/{settings.rs, paths.rs, pipeline.rs}     # D1, D2
crates/rr_dr60/src/wording.rs                               # D3 (new)
crates/rr_dr60/src/meters.rs                                # D4 (new module)
crates/rr_dr60/src/manifest.rs                              # D6
crates/rr_dr60_ffi/src/host_reads.rs                        # the C functions (new)
crates/rr_dr60_harness/tests/{us1_host_reads, us2_drain, us3_meters_manifest, us4_wording}.rs
swift/Sources/RRDR60/ReRender.swift                         # D5
tools/{mic-chain,filter-design,band-filter}/…               # --drain prints
```
