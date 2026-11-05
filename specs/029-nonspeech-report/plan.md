# Implementation Plan: The non-speech reshaping report

**Branch**: `029-nonspeech-report` | **Date**: 2026-11-04 | **Spec**: [spec.md](spec.md) (approved by the owner 2026-11-04, D1 – D9)

**Input**: the signed spec; the hardware review's extended probe of 2026-11-04 (three rates; its outputs in the session scratchpad, not committed) as the expectation for the margins.

## Summary

A harness-only feature. One new checks module computes nine frame-level measures on the output of eight seeded stimuli through eight named configurations, under the harness's deterministic-math rules, and prints a canonical report whose text at three fixed rates is a golden. Six self-consistency checks assert; every other figure is reported. The guide gains a generated section. No library, audio, C, Swift or register change; no version bump.

## Technical context

Rust 2024, MSRV 1.85; the `rr_dr60_harness` crate only (unpublished; `std`). No new dependencies. New module `crates/rr_dr60_harness/src/nonspeech_checks.rs`; new test binaries `golden_nonspeech`, `nonspeech_matrix`, `us1_nonspeech`, `us3_nonspeech`; entries in `mutation`, `determinism`, `alloc_free`, `us2_cross_check`, `us2_abi`, `us1_bypass`; the golden `crates/rr_dr60_harness/golden/nonspeech-v1.txt`; eight named configurations in `configs::settings`; a guide section and a README line. The core crates are untouched (FR-010).

## Constitution check

- **I. Portable library first**: the core is not touched; the feature is measurement and documentation.
- **II. Evidence-based fidelity**: no new modeled behaviour; every stage the report names is cited by its registered ID (A-027 the band, A-004 / A-040 / A-041 the coder, A-022 / A-024 the gate, A-048 the hiss, A-046 the microphone's corner, A-028 the limiter, A-010 the speaker). The report's constants (frame, settle, thresholds, bands) are measurement conventions, stated in its header, not assumptions about the device.
- **III. Measured, test-first**: the reader and the stimuli land with unit pins before the report text; the golden is red until the bless gate, which asserts D5 and D6 at bless time; every check cites its probe figure, bound and margin (spec Assumptions).
- **IV. Deterministic**: the reader, the stimuli and the resampler to 6 kHz use `rr_dr60_detmath` and basic IEEE operations only (R-01, R-02), so the text golden is byte-identical on every CI target; the engine's determinism is spec 001's. Nothing touches the real-time path.
- **V. Modular, traceable chain**: the ladder uses the existing stage switches and taps; the configurations are named in `configs` with their taps (R-03).
- **VI. Neutral communication**: D7, enforced by both word lists and a phrase list in the cross-check (R-10).
- **VII. Independently testable**: each story has its own binary; the golden and the checks need only the harness.
- **Coverage**: the golden is ignored under `cfg(coverage)`; the checks run at `measurement_rates()` on one shared table per binary (R-06).

## Research

- **R-01 The reader's math** (coach 3, hardware B13). The reader lives in `nonspeech_checks` under the workspace's `disallowed-methods` (no `#![allow]`): LPC by autocorrelation and Levinson recursion (basic ops); the LPC envelope on a 10 Hz grid (0 – 3000 Hz) through a **cosine table** `COS[k][g] = detmath::cos(2π·g·10·k ÷ 6000)` for k = 0 … 8, built once per process; dB as `20 · detmath::ln(x) ÷ LN10` with `LN10 = detmath::ln(10.0)` (not `analysis::db`, which uses `std`); RMS via `detmath::sqrt`; the normalised autocorrelation's denominator via `detmath::sqrt`; the Hann window and the envelope DFT through a second table `detmath::cos(2π·m·n ÷ N)` for the 575-frame span (N = 575) and the DFT bins; the prominence and width rules on the grid. Printing at fixed precision through `format!("{:.3}")`, which is exact decimal rounding of the binary value and platform-independent. **Decision**: a self-contained reader; `analysis` is used only for AES17 level and `from_db` where they are not on a golden figure's path (the level column is computed by the reader's own detmath RMS). **Alternative**: reuse `analysis::lpc_lsd`'s `lpc_frame` (private, `std` math; rejected).
- **R-02 The resampler to 6 kHz**. `nonspeech_checks::to_device_rate(x, rate)`: the identity at 6 kHz; otherwise `analysis::resample_to`'s design (a Hamming-windowed sinc over 32 zero crossings each side at the lower rate, cut-off 0.45 × 6000 = 2700 Hz) with `detmath::sin` / `detmath::cos` and the window computed once per fractional phase class (the ratios are rational: 4:3, 8:3, 147:20, 8:1, 147:10, 16:1; a phase table of `rate ÷ gcd` entries). Cost: ≤ 12 s × 6000 × 2 × 32 × 16 ≈ 7 × 10⁷ multiply-adds per cell at 96 kHz (about 0.1 s release). **Decision**: as stated; the FIR's edge at 2700 Hz is why the peak search stops at 2300 Hz (D4) and why measures near the edge are excepted from the cross-rate bound (D5).
- **R-03 The configurations** (D3; hardware B5, coach 11). Named in `configs::settings`, every one built on the existing names, each with its tap stated in its doc comment: `nonspeech_c0` = `bypass_all`; `nonspeech_c1` = `default`; `nonspeech_c2` = `codec_only`; `nonspeech_c3` = `default_codec` with `no_agc`, `no_vas` (the band, the clamp and the coder; `Tap::AfterPlayback`); `nonspeech_c4` = `Settings::new(rate)` with the gate off, the output chain's three stages off, `Tap::AfterPlayback` (the microphone, the limiter, the band, the clamp, the coder); `nonspeech_c5` = `nonspeech_c4` with the gate on at level 3; `nonspeech_c6` = `nonspeech_c5` with `vas.sensitivity = 5` and `mic.hiss_offset_db = 12.0`; `nonspeech_c7` = `Settings::new(rate)` (the default engine at the route output). The limiter-off twin of C4 (`agc.enabled = false`) is a matrix row asserting bit-identity on every stimulus (the stimuli stay below the knee). The report prints each configuration's name, tap and `latency_samples()`.
- **R-04 The span and the alignment** (D4; coach 9, hardware B1, B12). The output is the processing calls' returned samples (4096-sample calls, no drain, no flush); the span in output samples is `[settle + L, min(out_len, in_len + L))` with `settle = 0.5 s` and `L = latency_samples()` of the configuration, then resampled to 6 kHz and framed; in drop mode (C5 – C7 on stimuli where the gate pauses) the output is shorter than `in_len + L`, so the span ends at the output's end and the report says the frames are kept output. The pass-through check (D6 ii) reads the input over `[settle, in_len)` through the same reader: identical sample sets at 6 kHz (L = 0, bit for bit); elsewhere the FIR's phase differs by the latency's fractional part, so the per-measure tolerances are set from the first printed row (R-05).
- **R-05 The cross-rate spread, first estimate** (D5; coach 18). From the hardware review's probe at 6 and 48 kHz (whose noise was not in-band levelled, so its level column is excluded): periodicity C3 0.297 vs 0.292 (0.005), C1 0.239 vs 0.233; peaks C1 1.57 vs 1.56, C3 1.73 vs 1.70; the 2 – 8 Hz share C1 0.103 vs 0.101 on steady noise, 0.937 vs 0.963 on the modulated stimulus (0.026); periodic-frame fraction ≤ 0.02 apart; flux ≤ 0.1 dB apart; active frames equal on the open-gate cells. Expected tolerances by the rule "2 × the printed spread, floor 0.01 or one frame": periodicity 0.01, peaks 0.1, shares 0.05, fractions 0.04, flux 0.2 dB, level 1 dB (after D2's in-band levelling), counts ±1 frame; **recorded in the spec's Clarifications after the first harness print** (a task), asserted in the matrix from then on; C7 keeps a per-rate tolerance; measures reading near 2700 Hz are excepted by construction (the peak search stops at 2300 Hz).
- **R-06 Cost and coverage** (coach 16, hardware B4). Release: about 0.3 s per cell in the reviewer's probe (12 s, 6 kHz, `std` math); with the detmath resampler and the DFT tables about 0.3 – 0.5 s per cell at 48 kHz. The golden: 8 stimuli × 8 configurations × 3 rates = 192 cells ≈ 1 – 1.5 min release; the matrix at seven rates 448 cells ≈ 3 min; the partitions on 2 × 2 cells × 4 block sizes × 2 rates ≈ 32 cells. In the `test` profile (opt-level 3) similar. Under instrumentation the coder costs about 30 ×, so the golden is `#[cfg_attr(coverage, ignore)]` and each check binary computes **one table per rate** at `measurement_rates()` through a `OnceLock`-held `Table` shared by its tests. **The coverage job's minutes are measured at the gate and recorded in the records** (a task); nextest (PR #89) runs the binaries in one pool.
- **R-07 The coder's steady-noise level, a desk check** (hardware B15; spec Assumptions). If `tools/celp-calibrate/ref/` holds the ITU G.723.1 software, the D2 (a, −35) noise at 8 kHz is written by a `codec_stimuli`-style example and run through the 5.3 kbit/s reference, and the reference's level change is **reported** beside the model's (about −5 dB) in the records; if the software is absent, the records say so. No check; no code in the gate depends on it.
- **R-08 The pins a new golden trips** (coach 15; spec 023's lesson). `us1_bypass::us1_as1_*` counts the SHA guards found in the golden tests' sources (`checked == 16`): restated 16 → 17 with a dated comment; `us2_abi`'s untouched-guard list is read from the diff of the golden directory's `*.json` files and the manifest texts (the new `.txt` is listed by name as the one allowed addition); the manifest and control manifest are not re-blessed (no version, string or ID change), so spec 014's and 023's pins do not move. Every guard-scanning and manifest-pinning binary runs before the gate.
- **R-09 The guide region and the README line** (D8). `docs/guide.md` gains a marked region (`<!-- nonspeech:begin -->` … `<!-- nonspeech:end -->`) regenerated by `RR_DR60_BLESS=readme` from the golden's 6 kHz table (rows a (−50), b, d, f; columns level, active frames, periodicity, 2 – 8 Hz share, LPC peaks; configurations C0 (the input's own), C1, C3, C5, C6; each header with its range) and checked by `us2_cross_check guide_nonspeech`; the four effect paragraphs and the caption are hand-written and checked for their figures against the golden (each quoted figure must appear in the table) and for their IDs; the README's "What exists today" gains one hand-written line pointing at the section.
- **R-10 The wording checks** (D7; product M5). `nonspeech_checks::STRINGS` exposes the header lines, the column names and the configuration and stimulus names; `us4_wording` runs `wording::first_hit` and `first_claim_hit` over them; `us2_cross_check guide_nonspeech` runs both lists and a `PHRASES` list (`voice`, `speech-like`, `words`, `hear`, `heard`, `listener`, `sounds like`, `phantom`, `illusion`, `mistaken`, `detect`) over the guide section (case-folded, whole words; "non-speech" in the title is allowed by the whole-word rule; "computed" required, "measured" refused by the claims list).
- **R-11 The mutants** (US3 AS3). Five reader mutants behind `cfg(feature = "__test-hooks")`-free constructor flags on the reader (`ReaderOptions { prominence: bool, pre_emphasis: bool, lag_max: usize, include_flush: bool, reset_state: bool }`, the production reader a constant), each pinned to the D6 check it fails: prominence-and-width off → (v) peaks ≤ 1.5 on noise rows and the resonator row; pre-emphasis off → (v) the reference's lowest-peak median band; `lag_max = 50` → (i) the reference's fraction ≥ 0.9 (the glide's 90 – 220 Hz fundamental needs lags up to 67); flush included → (v) the 2 – 8 Hz share on (b) ≥ 0.8; no state reset → (ii) pass-through on the bursts row.

## Traceability

| Spec | Plan |
|---|---|
| D1, FR-004 | the canonical text (data-model.md), `golden_nonspeech` with the bless gate; R-08 |
| D2, FR-001 | `nonspeech_checks::stimuli` (data-model.md); R-01 |
| D3, FR-002 | R-03; `nonspeech_matrix::configs` (the limiter twin) |
| D4, FR-003 | the reader (R-01, R-02, R-04); `us3_nonspeech::reader_definitions` |
| D5, FR-005 | R-05, R-06; `nonspeech_matrix::{partitions, rates}` |
| D6, FR-006 | `us1_nonspeech` (i – iv), `us3_nonspeech` (ii, v, vi) |
| D7, FR-007 | R-10 |
| D8, FR-008 | R-09 |
| FR-009 | R-11 |
| FR-010 | `us2_abi::spec_029_untouched` (the diff of the three core crates against `main`'s merge base; the seventeen earlier goldens by SHA) |
| FR-011 | `determinism::nonspeech_same_input_same_text`, `alloc_free::nonspeech_processing_allocates_nothing` (the engine's processing calls under the counters; the reader allocates, outside the window) |

## Build order (test-first)

1. The stimuli with unit pins (lengths, levels within 0.01 dB of the target at every rate, the seeds, the bursts' count) and the configurations with their taps (R-03).
2. The reader with unit pins on hand-built frames (a pure tone's periodicity and lag, a two-resonance frame's peaks, the Hann DFT of a 4 Hz envelope, the state reset).
3. `us3_nonspeech` (ii, v, vi) and `us1_nonspeech` (i, iii, iv): red, then green on the first table.
4. The report text, `golden_nonspeech` red; the matrix (partitions asserted, rates reported); the mutants; determinism and allocation; the pins of R-08.
5. The first print: R-05's spread recorded in the spec's Clarifications; R-07 if the reference software is present.
6. The bless (the gate asserts D5 and D6); the guide region and the README line; the cross-checks; CHANGELOG and CLAUDE.md.
7. The full gate; the coverage minutes recorded; the post-implementation coach and hardware reviews; the PR.

## Gate

`cargo fmt --all -- --check`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo test --all-features`; `RUSTFLAGS="-D warnings" cargo build -p rr_dr60_ffi --release`; `cargo llvm-cov nextest --all-features --workspace --fail-under-lines 80` (the minutes recorded); `scripts/check-traceability.sh`; the C smoke and the Swift package unchanged (no FFI change); every manifest-pinning and guard-scanning binary green.

## Project structure

```text
crates/rr_dr60_harness/
├── src/nonspeech_checks.rs        # stimuli, configurations' names, the reader, the report text, STRINGS, ReaderOptions
├── src/configs.rs                 # nonspeech_c0 … c7
├── golden/nonspeech-v1.txt        # the canonical text at 6, 44.1, 48 kHz
├── tests/golden_nonspeech.rs      # byte compare + SHA pin; RR_DR60_BLESS=nonspeech; doctored_hang
├── tests/nonspeech_matrix.rs      # seven rates, partitions, the limiter twin, cross-rate rows
├── tests/us1_nonspeech.rs         # D6 i, iii, iv
├── tests/us3_nonspeech.rs         # D6 ii, v, vi; reader_definitions
├── tests/{mutation,determinism,alloc_free,us2_cross_check,us2_abi,us1_bypass,us4_wording}.rs  # entries
docs/guide.md                      # the generated region and four paragraphs
README.md, CHANGELOG.md, CLAUDE.md # one line, one entry, the status and the commands
specs/029-nonspeech-report/        # plan.md, data-model.md, quickstart.md, tasks.md
```

## Complexity tracking

None: no constitution violation.
