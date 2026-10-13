# Implementation Plan: Speech Codec (CELP, 4 kbit/s)

**Branch**: `008-celp-codec` | **Date**: 2026-10-12 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/008-celp-codec/spec.md` (clarified; coach, hardware-review and clarify decisions in its Clarifications section).

## Summary

The record path gains a **speech encoder** after the VAS and the playback path a **speech decoder** before the codec output filter: an **algebraic CELP coder of the G.729 / G.723.1 family at 20 ms frames of 80 bits** (exactly 4 kbit/s at the 6 kHz device rate), adapted to 6 kHz with every constant re-derived and every quantizer table analytic, written in deterministic fixed-point integer arithmetic. Recordings become whole 10-byte frames in a store of 1.8 MB; the exported byte form moves to version 2; the one-pass processing runs the pair in line with an exact 180-sample delay (a whole number of host samples at every rate), so record-then-play stays bit-identical to it; the codec is on by default with one bypass and a new tap after the decoder; the earlier specs' checks run with it bypassed, so no existing golden file moves and the codec gets its own, with frame-byte digests.

Technical approach:

- **Design** (R-01, R-02; contracts/codec-design.md): 120-sample frames, two 60-sample subframes, 10 ms look-ahead (delay 180 samples, 30 ms), order-8 LPC as LSPs (22 companded scalar bits), pitch 7 + 5 bits over lags 15 – 120 with half-sample resolution, G.723.1's 17-bit four-pulse codebook per subframe, 3 + 3 gain bits per subframe with a moving-average energy predictor (G.729's coefficients) whose range is anchored to the project's levels, a weighting filter in the searches, a short-term postfilter with tilt and gain scaling. The design tool `tools/celp-design/design_celp.py` prints every figure and generates `codec/tables.rs`.
- **Arithmetic** (R-03): Q15 fixed point with saturating operators written in the core; no floating point and no `detmath` in the codec; explicit overflow semantics on every operation so debug and release agree; ties to the lowest index; the cross-platform goldens compare frame bytes.
- **Integration** (R-06): the transport owns the recorder's encoder and appends frames to the memory; `OnePass::tick` runs encoder → decoder → a 180-sample FIFO, so the one-pass delay is exactly the codec's; playback decodes frame by frame; the stop feeds host silence through the record path to the frame boundary **with the VAS gate held open in drop mode** (so a paused gate cannot prevent the last frame) and reports the silence fed, look-ahead included; the record and one-pass latencies grow by 180 device samples, the playback latency does not; the spec's FR-009 and FR-013 are amended for these two rules in its plan-review Clarifications.
- **Memory and form** (R-07): a byte store in one of two encodings (frames or samples), spans in units, capacity in units, 1 800 000 bytes reserved at the default with the codec on; form version 2 with the encoding and the codec model version; version 1 refused; one encoding per memory; decoders frozen per released model version from 1.0.
- **Interface** (R-08; contracts/): `Settings::speech_codec_enabled`, `Tap::AfterDecoder`, `rr_dr60::speech_codec` constants, frame fields in the block reports, two new errors; the C struct 88 → 96, statuses 21 and 22, `rr_dr60_speech_codec_info`, `rr_dr60_get_encoding`; version 0.8.0.
- **Measurement** (R-09): two speech-like stimuli, the codec-only configuration at the decoder tap, third-octave levels, LPC log-spectral distance, gated segmental SNR, cross-correlation lag, the look-ahead probe, the one-pass equivalence with the stop's silence appended and the output skipped by the delay; the calibration tool runs the same stimuli through the **ITU-T reference implementation** of G.723.1 at 5.3 kbit/s (ffmpeg's encoder has no 5.3 kbit/s mode) and the harness measures both with the same code, **before any codec code is written**; `golden-codec-v1.json` (84 entries) blessed once, last.
- **Records** (R-10, R-11): manifest rows 7, 8, 9 (counts 8 / 7), A-004 adopted, A-040 and A-041 registered, S-008 – S-011 added, the signal-chain markers, README section, CHANGELOG 0.8.0.

## Technical Context

**Language/Version**: Rust, edition 2024, MSRV 1.85 (001 R-01). Python 3 for the two offline tools (`uv run`; `numpy` for the design tool; a C compiler and the ITU-T G.723.1 reference software, downloaded by the user, for the calibration tool, with `ffmpeg` optional for a decode cross-check; neither tool runs in CI or at build time).

**Primary Dependencies**: none new in the core (the codec is in-crate fixed-point integer code; the CRC exists). The harness keeps `sha2`, `serde_json`, `rustfft`.

**Storage**: the in-memory recording store becomes `Vec<u8>` in one of two encodings (R-07): 1 800 000 bytes at the default with the codec on, 86 400 000 when bypassed. No disk I/O in the core. The harness example `codec_stimuli` writes raw files for the calibration tool (the harness is unpublished).

**Testing**:
- **Core**: unit tests for the fixed-point operators (every saturating branch), the LPC analysis against a floating-point reference computed in the test (tolerance one Q15 step), the LPC ↔ LSP round trip, every quantizer's code ↔ value table, the pitch code map (127 codes, 127 unused), the codebook decode (every 17-bit code places ≤ 4 pulses inside the subframe), the bit packer round trip for every allocation field, encoder frame causality (push 180 samples → one frame on the 180th; the look-ahead boundary), decoder reset, silence output, the memory in both encodings (units, capacity rounding through a test hook with a 7-sample unit, the reconfigure refusal with recordings present), form version 2 (round trip, each refusal in order, version 1 refused), the one-pass FIFO delay (the first 180 outputs are exactly 0.0; the alignment itself is FR-009's check), the stop padding (lengths ≡ 0 mod 120; the VAS-paused case), latencies at the seven rates, settings and FFI layouts.
- **Harness**: `codec_checks.rs` (R-09) and the tests `us1_codec`, `us2_frames_memory`, `codec_matrix` (seven rates, `--nocapture` report, the `--ignored` `calibration` reader), `codec_edge_cases`, `golden_codec`; `ffi_parity`, `alloc_free`, `determinism` (a record-then-play partition case with the codec; frame bytes compared), `timing` (SC-006), `mutation` (a one-bit change in a frame changes the output; a one-sample input change after the look-ahead leaves the frame unchanged) extended; `us1_manifest` and `us2_cross_check` goldens re-blessed; the C smoke test extended.
- **Golden**: the three existing files untouched (SHA guards); `golden-codec-v1.json` blessed once at the end (`RR_DR60_BLESS=codec`).
- **Coverage**: gate 80 %; the codec's search code is reached by the stimuli; the unused-code and clamp branches by the form and decoder unit tests.

**Target Platform**: unchanged: 8 targets (001 FR-014). Fixed-point integer code is identical everywhere by construction; the golden matrix proves it on six.

**Project Type**: library: a Rust crate plus a C ABI; two offline Python tools.

**Performance Goals**: ≤ 8 µs of codec work per device sample (≈ 2 µs expected on the CI runner; 0.8 – 2.5 µs on a 2018-class phone core), ≤ 1000 ns per 48 kHz host sample for the one-pass with the codec, the worst 64-sample block at 48 kHz well under 1.33 ms (≈ 0.1 ms expected), record-plus-play within 25 % of the one-pass (SC-006).

**Constraints**:
- Bit-identity (FR-009, FR-016) at every rate, block partition and platform: integer arithmetic only in the codec; the same `SpeechEncoder`/`SpeechDecoder` code in the one-pass and the paths; the FIFO delay exact.
- No allocation in per-block calls, start, stop or the figure reads; the store, the span table, the FIFO and the frame buffers are fixed-size or reserved at construction/reconfigure; export and import use caller buffers.
- The three existing golden files do not change (FR-021); the codec's golden file is blessed exactly once.
- No `unsafe` outside `rr_dr60_ffi`; panics never cross the C boundary; status codes only appended; the C struct grows at the end.
- Every string follows the claims policy and spec 007's denylist (FR-023); the codec is "modeled on a 4 kbit/s CELP-class coder of the period".

**Scale/Scope**: about 3 200 lines of core (`codec/`: `fixed.rs` ≈ 250, `lpc.rs` ≈ 300, `lsp.rs` ≈ 450, `pitch.rs` ≈ 350, `acelp.rs` ≈ 350, `gains.rs` ≈ 150, `postfilter.rs` ≈ 150, `bits.rs` ≈ 150, `tables.rs` generated ≈ 300, `mod.rs` ≈ 400 with the encoder/decoder drivers and tests; `memory.rs` reshaped ≈ 200 changed, `form.rs` ≈ 250 changed, `transport.rs` ≈ 250 changed, `pipeline.rs` ≈ 120 changed, settings/error/validate/manifest ≈ 150), about 400 lines of FFI, about 1 500 lines of harness (`codec_checks.rs`, two stimuli, four metrics, five test files, extensions, the example), two Python tools (≈ 400 and ≈ 150 lines), docs, registers, CHANGELOG, README, one new golden file, the manifest goldens re-blessed.

There are no open NEEDS CLARIFICATION items. The plan session fixed the design (R-01, R-02) and the plan reviews amended three spec rules, recorded in the spec's Clarifications ("coach and hardware-emulation reviews of the plan"): FR-013 (the stop feed holds the VAS gate open in drop mode; `silence_fed_samples` includes the look-ahead; nothing is fed when no sample was kept), FR-009 (the equivalence holds when the VAS is bypassed or open at the end, with the one-pass output skipped by the codec delay) and FR-001 (the lag window is conserved in hertz; the weighting and postfilter factors are kept as published).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | How this plan complies | Pre-research | Post-design |
|---|---|---|---|
| I. Portable Library First | No new dependencies; the core stays `no_std` + `alloc` (fixed-point integer code, generated tables); no I/O (the harness example and the Python tools live outside the core); the C struct grows at the end (88 → 96), statuses appended (21, 22), the header regenerated and drift-checked, the smoke test covers the codec loop (R-08). | ✅ | ✅ |
| II. Evidence-Based Fidelity | The class, rate and bit rate trace to S-002; the design to S-008 – S-011 as **secondary sources for the class** with A-004 adopted as a class match and A-040 / A-041 registering every unknown at low confidence with a verification method (R-10); the codec on by default is a user-visible change recorded in the CHANGELOG; every design constant is a tool-printed figure pinned by a test, never called measured; the one borrowed constant set (the gain predictor's coefficients, S-009) is named as borrowed. | ✅ | ✅ |
| III. Measured, Test-First Verification | Each FR maps to a `codec_checks` function or a core unit test (R-09), written and seen to fail first; the calibration (stimuli, metrics, tool, `calibration.md`) is built before any codec code so the tolerances exist when the checks are written, with the spec's floors written first and tightened once; the goldens guard the earlier specs unchanged and the codec with frame-byte digests; mutation: a one-bit frame change and a post-look-ahead input change. | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | Integer arithmetic only, ties to the lowest index, no `detmath`, no clock; fixed trip counts; frames, FIFO and buffers fixed-size; the store reserved once; `alloc_free` extended (R-03, R-04, R-12). | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | Stages 7 and 9 are one module with one bypass (the pair, as the spec decides) and a tap after the decoder; the earlier taps keep their meaning; the memory's encoding follows the bypass (Complexity Tracking rows 1, 2). | ✅ | ✅ |
| VI. Neutral, Honest Communication | "Modeled on a 4 kbit/s CELP-class coder of the period"; the manifest's coverage notes name the unknowns; the README states the calibration basis and that the device's coder is unidentified; nothing about what recordings contain (R-10, R-11). | ✅ | ✅ |
| VII. Independently Testable Features | US1 by the codec-only configuration and the one-pass equivalence with the memory at default capacity; US2 by the memory alone at a small capacity and the form's corruption set; US3 by the cross-check; US4 by C parity and the smoke test; specs 001 – 006 keep their isolated configurations with the codec bypassed. | ✅ | ✅ |
| Repo standards | rustdoc on every new public item; SemVer 0.8.0 (breaking: C struct size and report structs, the default output, form version 2); Conventional Commits; no audio committed (stimuli generated; the calibration outputs are git-ignored, the report is text). | ✅ | ✅ |

**Gate result**: PASS. Five complexity items are recorded below.

## Project Structure

### Documentation (this feature)

```text
specs/008-celp-codec/
├── plan.md              # This file
├── research.md          # Phase 0: decisions R-01 – R-13
├── data-model.md        # Phase 1: frame, codec state, memory encodings, reports, form v2, per-rate table
├── quickstart.md        # Phase 1: validation walk-through
├── calibration.md       # written by the calibration step (R-09); committed with the figures
├── contracts/
│   ├── codec-design.md  # the 6 kHz constants and the 80-bit allocation (R-02); the frame's bit order
│   ├── recording-form.md# the exported byte form, version 2; the frozen-decoder rule
│   ├── rust-api.md      # new settings, tap, constants, report fields, errors, Encoding
│   ├── c-api.md         # RrDr60Settings 88 → 96, statuses 21–22, report structs, two functions
│   └── golden-format.md # golden-codec-v1.json: entries with frame digests; the unchanged three
└── tasks.md             # Phase 2 (/speckit-tasks)

.gitignore                       # + tools/celp-calibrate/out/, tools/celp-calibrate/ref/
.github/workflows/ci.yml         # golden_codec added to the golden-matrix and test jobs (the three lines that list golden tests)
```

### Source Code (repository root)

```text
crates/rr_dr60/src/
├── lib.rs                      # crate docs (the codec, the frames, form v2); pub mod speech_codec (re-exports)
├── settings.rs                 # Settings::speech_codec_enabled (default true); Tap::AfterDecoder
├── error.rs                    # Error::EncodingDiffers, Error::UnsupportedCodecModelVersion { major, minor, patch }
├── codec/                      # NEW (R-02 – R-05): the speech codec, fixed point
│   ├── mod.rs                  #   constants, MODEL_VERSION, Frame = [u8; 10], SpeechEncoder, SpeechDecoder, SpeechCodecInfo
│   ├── fixed.rs                #   saturating Q15/Q31 operators
│   ├── lpc.rs                  #   window, autocorrelation, lag window, Levinson–Durbin
│   ├── lsp.rs                  #   LPC ↔ LSP (Chebyshev root search), quantizer, interpolation
│   ├── pitch.rs                #   open-loop estimate, adaptive codebook, fractional interpolation, lag codes
│   ├── acelp.rs                #   the 17-bit four-pulse codebook: search and decode
│   ├── gains.rs                #   adaptive and fixed gain quantizers, the energy predictor
│   ├── postfilter.rs           #   short-term postfilter, tilt, gain scaling
│   ├── bits.rs                 #   frame packer/unpacker from the allocation table
│   └── tables.rs               #   GENERATED by tools/celp-design/design_celp.py (windows, interpolation taps, quantizer tables)
├── paths.rs                    # RecordHalf::force_open (the stop feed); latency helpers take the codec delay
├── memory.rs                   # Memory { store: Vec<u8>, encoding: Encoding, spans }, units, append_frame/append_sample, frames(span)
├── form.rs                     # version 2: header 48 bytes, encoding, codec model version; version 1 refused
├── transport.rs                # recorder_codec: Option<SpeechEncoder>; player_codec: Option<SpeechDecoder>; frames in reports;
│                               # the stop feed to the frame boundary; silence_fed_samples; Cursor in frames; encoding()
├── pipeline.rs                 # OnePass { codec: Option<OnePassCodec { enc, dec, fifo }> }; tick with the 150-sample delay;
│                               # latency_samples with the codec delay; BlockInfo.frames
├── manifest.rs                 # stages 7, 8, 9; bypass_control_anchor arms for speech_codec_enabled and Tap::AfterDecoder
└── stages/                     # unchanged

crates/rr_dr60_ffi/
├── src/lib.rs                  # RrDr60Settings + speech_codec_enabled (88), 96 bytes; RR_DR60_TAP_AFTER_DECODER; statuses 21, 22;
│                               # RrDr60BlockInfo/RecordInfo/StopOutcome/PlayInfo frame fields; RrDr60SpeechCodecInfo, RrDr60Encoding;
│                               # rr_dr60_speech_codec_info, rr_dr60_get_encoding; layout tests; version 0.8.0
├── cbindgen.toml               # export include: RrDr60Encoding; the real-time-safe list
├── include/rr_dr60.h           # regenerated
└── tests/c/smoke.c             # record the ±0.125 square wave through the codec → export → import → play; frame bytes memcmp

crates/rr_dr60_harness/
├── src/stimulus.rs             # voiced(), unvoiced_voiced() (R-09)
├── src/analysis.rs             # third_octave_levels, lpc_lsd, segmental_snr, xcorr_lag, level_track
├── src/codec_checks.rs         # NEW (R-09): the checks listed in research
├── src/configs.rs              # codec_only(rate), codec_only_vas(rate), default_codec(rate); codec bypass on every earlier config
├── src/golden.rs               # CODEC_STIMULI, CODEC_CONFIGS, frames + frames_sha256 fields, generate_codec, committed_codec
├── golden/golden-codec-v1.json # NEW, blessed once (84 entries)
├── golden/manifest-v1.txt      # re-blessed once (rows 7, 8, 9; counts)
├── examples/codec_stimuli.rs   # NEW: writes raw f32le stimuli at a given rate for the calibration tool
├── tests/us1_codec.rs          # NEW (US1)
├── tests/us2_frames_memory.rs  # NEW (US2)
├── tests/codec_matrix.rs       # NEW: the seven-rate matrix; the --ignored calibration reader
├── tests/codec_edge_cases.rs   # NEW
├── tests/golden_codec.rs       # NEW
└── tests/ffi_parity.rs, alloc_free.rs, determinism.rs, timing.rs, mutation.rs, us2_cross_check.rs  # extended

tools/celp-design/design_celp.py       # NEW: prints every design constant, the budget (--check), generates codec/tables.rs
tools/celp-calibrate/calibrate.py      # NEW: builds the ITU-T G.723.1 reference software (user-downloaded) and runs it at 5.3 kbit/s on the harness's 8 kHz stimuli; writes decoded references
tools/celp-calibrate/README.md         # the download, build and run steps; the ffmpeg decoder cross-check; what to do without the software
docs/hardware/assumptions.md (A-004 adopted; A-040, A-041 new; A-038, A-039 one sentence), sources.md (S-008 – S-011; leads),
signal-chain.md (rows 7, 8, 9; diagram); CHANGELOG.md (0.8.0); README.md (codec section; generated table); CLAUDE.md
```

**Structure Decision**: the existing three-crate workspace. The codec is one new module directory in the core with a single public surface (`speech_codec`), because the encoder and decoder share tables, arithmetic and the allocation and are bypassed together; the memory, form and transport modules of 006 are extended in place.

## Task order (for /speckit-tasks; coach Q8)

1. **Phase 1, Setup and calibration**: the design tool and `tables.rs`; the harness stimuli, metrics and the `codec_stimuli` example; the calibration tool, its README and `calibration.md` with the figures and the stimulus digests (a non-ignored digest test); `configs::settings` sets `speech_codec_enabled = false` for every pre-008 configuration; the memory gains its `Frames` / `Samples` encoding with the codec bypassed everywhere. **Checkpoint: the whole existing suite and the three SHA guards green before any codec code exists.**
2. **Phase 2, US1**: `codec_checks` and the core unit tests written red → `fixed.rs` → `lpc`, `lsp`, `pitch`, `acelp`, `gains`, `postfilter`, `bits` → the encoder and decoder drivers → transport, the one-pass FIFO, the stop rule, the latencies, the settings and the tap. Checkpoint: `us1_codec`, `codec_matrix`, `determinism`, `alloc_free`, `mutation`.
3. **Phase 3, US2**: form version 2, the capacity scenarios, the corruption set, the reconfigure rule. Checkpoint: `us2_frames_memory`.
4. **Phase 4, US3**: manifest rows 7, 8, 9; the registers (A-004, A-040, A-041, A-038, A-039, S-008 – S-011); the signal chain; the README section. **The manifest and README re-bless sits here** (strings only). Checkpoint: `us1_manifest`, `us2_cross_check`, the traceability audit.
5. **Phase 5, US4**: FFI (struct, statuses, report structs, two functions), the header, the smoke test, `ffi_parity`. Checkpoint: the C loop.
6. **Phase 6, Records and gates**: timing, coverage, CHANGELOG, `ci.yml` (`golden_codec` in the golden-matrix and test jobs), `.gitignore`, CLAUDE.md; the post-implementation hardware review; **the codec golden bless is the last task, after that review** (R-13: the LSP tables may still change). Then the PR (ask first).

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| One bypass for two stages (7 and 9) | A decoder without its encoder, or the reverse, is a combination the device cannot produce (spec FR-002); the memory's encoding must be known at construction (FR-012) | Separate bypasses: a recording of frames played without the decoder has no meaning, and the store could not be sized |
| The memory's encoding follows a setting and a reconfigure that changes it is refused while recordings exist | FR-012 (clarify Q1): a 1.8 MB store cannot hold sample recordings, and a store sized for both gives up the footprint | A store sized for both encodings (86 MB); per-recording encoding (every call branches on it, and the capacity accounting in time would hide the byte limit) |
| The stop feed holds the VAS open | Without it a VAS paused at the stop drops the padding and the last frame can never complete (R-06); the device has no partial frame to complete at all | Zero-filling the frame (rejected by the hardware review: breaks the one-pass equivalence); leaving the last partial frame unstored (loses up to 20 ms the host fed) |
| A second numeric domain in the core (fixed-point integer, `codec/fixed.rs`) | Bit-identity across platforms for argmax searches (R-03): floating point would need every platform to agree to the last ulp in thousands of comparisons per frame; the period's DSP was 16-bit integer anyway | `f64` with `detmath`: a one-ulp difference flips an index and every later frame |
| Generated tables (`codec/tables.rs`) from a Python tool | Windows, interpolation taps and quantizer tables are irrational constants that the `no_std` core cannot compute at build time without `detmath` sin/cos at start-up; the tool prints and documents every figure (the 005 pattern); the generated file is checked byte-identical by `git diff --exit-code` after `--write` | Computing tables at construction with `detmath` (slower start, a second source of truth); hand-typed constants (untraceable) |
| `Memory::append_sample` panics in debug when the encoding is `Frames` (and the reverse) | The two encodings are selected at construction and the transport calls the matching method by construction; the branch is unreachable and carries the constitution's justification comment (`// coverage: unreachable by construction (Constitution III)`) | A `Result` on the hot path for a programming error (cost on every sample, and nothing a host could do with it) |
