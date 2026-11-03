# Implementation Plan: Host reads

**Branch**: `026-host-reads` | **Date**: 2026-11-03 | **Spec**: [spec.md](spec.md) (approved by the owner 2026-11-03)

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
- **Traceable figures.** D2's settle lengths are pinned to the design scripts' prints:
  - stage 2's corners: `tools/mic-chain/design_mic_chain.py --drain`;
  - the record band-pass: `tools/filter-design/design_voiceband.py --drain`;
  - the band filter: computed at run time from its applied pole radius, with the formula pinned against `tools/band-filter/design_band_filter.py --drain`.
- **Claims policy.** D3's rustdoc follows product M3's wording. D6's labels never shorten "checked against a real unit".
- **C ABI (spec 022).** One new sized structure (`RrDr60MeterField`, published list `[48]`), new statuses from 44, everything else appended. `abi-promise.sh` runs against three revisions.

## Research

- **R-01 D1.**
  - `Settings::one_pass_vas_runs(&self) -> bool` is the expression in `RecordHalf::new` with `honour_tap = true`, moved into one `const fn` that `RecordHalf::new` calls, so there is one rule.
  - `Settings::one_pass_codec_runs(&self) -> bool` is the `codec_runs` expression in `paths.rs:620`, which `paths.rs` then calls.
  - `Tap::is_front` and `Tap::is_after_stage_10` become `pub const fn`.
  - `Pipeline::one_pass_report_latency_samples()` is `latency_samples()` less `paths::codec_delay_host_samples(rate)` when `one_pass_codec_runs()`. That is exact, because the coder's term is added after the single rounding (`pipeline.rs:1783`).
  - C: three functions taking a settings pointer or a handle, with an `out` and the 022 order of refusals.
- **R-02 D2.**
  - `Pipeline::one_pass_drain_samples()` is computed at construction and at `reconfigure`, beside the latency, then refreshed for the band filter when its parameters land.
  - Front taps give 0.
  - Otherwise the sum of:
    - N_front: stage 2's two high-pass corners and the 12.9 kHz roll-off at the −100 dB envelope, when stage 2 runs;
    - the band filter's settle when the slot holds it (D2 below);
    - `2 · delay_host`;
    - ⌈(N₄ + 180·[codec] + N₁₀·[stage 10]) · m / l⌉, where N₄ is the record band-pass's −100 dB settle at 6 kHz, a constant from the design script, when the record stage runs;
    - `N_OUT` when the chain runs.
  - **The band filter**: its pole radius r from the applied (f, Q) at the host rate gives ⌈ln 1e-5 / ln r⌉. During a ramp the larger of the current and the target values is used, plus the ramp's remaining samples.
  - The rustdoc states the exclusions and the drop-mode scope.
  - C: `rr_dr60_one_pass_drain_samples(handle, *out)`.
- **R-03 D3.**
  - `crates/rr_dr60/src/wording.rs`:
    - `pub const ENGINE_STRINGS: Lists`, with `Lists { stems, phrases, suffixes }` and the harness's arrays moved verbatim;
    - `pub const CLAIMS: Lists`;
    - `pub fn first_hit(text: &str) -> Option<&'static str>` and `first_claim_hit`.
  - The matcher:
    - lowercase ASCII, compared per `char` without allocating;
    - a word is a maximal run of `char::is_alphanumeric` or `_` (the regex crate's Unicode `\w`);
    - a stem hits a word equal to the stem or the stem plus a listed suffix;
    - a phrase hits a word-bounded window;
    - the leftmost hit wins.
  - The harness's `denylist_regex` stays in a test module as the differential oracle (`us4_wording::as2_differential`). That test runs over every engine string and 10⁵ fuzzed strings from a seeded generator, and the harness then calls the core.
  - C: `rr_dr60_wording_first_hit(ptr, len, kind, *out_ptr)`, with kind 0 = engine strings and 1 = claims. Invalid UTF-8 is refused; the output is a static NUL-terminated string or NULL.
- **R-04 D4.**
  - `crates/rr_dr60/src/meters.rs` (the module exists for `Meters` reads):
    - `pub struct MeterField { identifier, unit, sentence, kind: MeterKind, side: MeterSide, lag: MeterLag }` (non-exhaustive) and `pub const FIELDS: [MeterField; 13]`.
    - An exhaustive `let Meters { .. } = …` in a const test pins the order.
  - `MeterLag` is an enum (`None`, `Conversion`, `Stored`), resolved to host samples by `Pipeline::meter_lag_samples(field)` from the engine's plan.
  - `Pipeline::meter_reached(field: usize, call: MeterCall) -> bool`, from the construction-time flags the meters already use:
    - the record group's points per call (`front`, `record`, `vas` run flags);
    - the playback group's `play_active` flags;
    - `run_amp` for `amp_clip_count`;
    - the clamp option for `output_clamp_count`.
  - C: `RrDr60MeterField` (48 bytes on 64-bit):
    - `struct_size` u32 at 0 and `index` u32 at 4;
    - `identifier`, `unit` and `sentence`, as `const char*`, at 8, 16 and 24;
    - `kind` u32 at 32, `side` u32 at 36, `lag` u32 at 40, `reserved` u32 at 44.
  - The C functions are `rr_dr60_meter_field_count`, `rr_dr60_meter_field(index, out)`, `rr_dr60_meter_reached(handle, index, call, *out_bool)` and `rr_dr60_meter_lag_samples(handle, index, *out)`.
- **R-05 D5.** In `swift/Sources/RRDR60/ReRender.swift`, the slot loop runs over every component's parameters at `parameter_offset + i` (from the C component list), whatever the component, then calls `set_component`. `fill` floors `missing` to whole frames with the coder on. The test is a record with an empty slot and a non-default store: the digest equals the harness's, and `component_parameter_value` after the re-render equals the store.
- **R-06 D6.**
  - `StageStatus::{token, label}`, `StagePath::{token, label}` and `Confidence::{token, label}`, as `pub const fn`.
  - `manifest_checks.rs:906-1000`, `examples/manifest.rs` and `smoke.c` call them; `smoke.c` calls them through `rr_dr60_manifest_word`.
  - A source scan finds no copy left.
  - The canonical text and the README table are byte-identical; this is asserted, with no re-bless.
- **R-07 Records.**
  - The guide gains sections for D1 – D4 and D6.
  - The CHANGELOG lists them under Added.
  - Also: the README's C list, the Swift wrappers (D9), the header, the version bump (`Cargo.toml` ×4, `RR_DR60_VERSION_MINOR` 93), the manifest's version line re-blessed, and the traceability audit.

## Testing

Each acceptance scenario's named test (the spec), plus:
- `ffi_parity host_reads` and `ffi_parity meters_manifest`;
- `abi_tests` for `RrDr60MeterField` (sizes, the published list, painted padding, Miri);
- `alloc_free host_reads`;
- the C smoke's `--host-reads` against `examples/host_reads_rows.rs`;
- the promise steps in `ci.yml` (three revisions);
- the design scripts' `--drain` prints pinned by a test that recomputes them (008's lesson).

## Project structure

```text
crates/rr_dr60/src/{settings.rs, paths.rs, pipeline.rs}     # D1, D2
crates/rr_dr60/src/wording.rs                               # D3 (new)
crates/rr_dr60/src/meters.rs                                # D4
crates/rr_dr60/src/manifest.rs                              # D6
crates/rr_dr60_ffi/src/host_reads.rs                        # the C functions (new)
crates/rr_dr60_harness/tests/{us1_host_reads, us2_drain, us3_meters_manifest, us4_wording}.rs
swift/Sources/RRDR60/ReRender.swift                         # D5
tools/{mic-chain,filter-design,band-filter}/…               # --drain prints
```
