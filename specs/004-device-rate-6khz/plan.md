# Implementation Plan: 6 kHz Device Rate and Codec Filter Templates

**Branch**: `004-device-rate-6khz` | **Date**: 2026-10-11 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/004-device-rate-6khz/spec.md`

## Summary

This feature moves the emulated device from the assumed 8 kHz rate and shared 300–3400 Hz band to the measured 6 kHz rate (A-026) and the codec's two datasheet templates scaled to that clock (A-027). No stage is added, no setting changes, and the AGC and VAS laws are untouched; every figure that depended on the old rate or band is restated once, and all three golden files are re-blessed in one commit.

Technical approach:

- **Device rate** (R-01): `DEVICE_RATE_HZ` becomes 6000. Every millisecond-to-sample conversion in the core already derives from it; a sweep removes the hard-coded 8000s from tests, harness and docs.
- **Host rates** (R-02, R-03): 6 kHz joins the list as the identity rate. The (l, m) table becomes 8 k = 3/4, 16 k = 3/8, 44.1 k = 20/147, 48 k = 1/8, 88.2 k = 10/147, 96 k = 1/16. The boundary prototype moves to a 2550–3000 Hz transition (cutoff 2775 Hz), 70 dB Kaiser as before. Each converter's delay drops from about 5.4 ms to about 4.8 ms, so the default latency is about 9.8 ms (about 471 samples at 48 kHz), and the 44.1 and 88.2 kHz prototypes shrink from about 38 000 taps to about 8 500.
- **Two filters** (R-04, R-05, R-06): the record stage becomes a 3rd-order Butterworth high-pass near 112 Hz plus a 5th-order elliptic low-pass with its edge near 2540 Hz (5 biquads); the playback stage becomes the 5th-order elliptic low-pass alone (3 biquads). Both are designed to the datasheet's typical values with the limits as tolerances, normalized to 0 dB at 765 Hz, minimum-phase. The stage struct becomes generic over its section count; the design script designs and verifies both templates, including group-delay distortion, and emits two coefficient constants.
- **Measurement** (R-07, R-08): the harness's stage check takes a per-stage template, adds group-delay distortion, moves the phase check to the 6 kHz identity rate, measures the anti-alias requirement with tones through the record stage, and reports the stage loss at 2700 and 2900 Hz without a tolerance. Steady tones that probe the VAS's sample-peak detector follow a phase rule derived from the pipeline's own reported latency.
- **Restated figures** (R-09, R-10): the AGC's Hilbert detector coefficients are **byte-identical** at 6 kHz (verified during planning: the design is normalized to the rate); its hard-coded 31/8000 delay in the harness becomes 31/6000, the FR-008 tones and FR-009 noise band move with the band. The VAS keeps W = 32 samples; its device-sample arithmetic in tests moves from 8000/160 to 6000/120.
- **Re-bless and record** (R-11, R-13): the three golden files are regenerated once, with 7 rates (112 + 42 + 63 entries), the two SHA guards updated, `library_version` 0.4.0, one CHANGELOG entry listing every restated figure, the register and sources updated, and a one-line pointer in each earlier spec's header.
- **Version**: 0.4.0 (output of every configuration changes; `SUPPORTED_HOST_RATES` changes type).

## Technical Context

**Language/Version**: Rust, edition 2024, MSRV 1.85 (001 R-01). Python 3.10+ via `uv` for the offline filter design (001 R-07).

**Primary Dependencies**: No new ones. Core: `rr_dr60_detmath`. Harness: `rustfft`, `sha2`, `serde`, `serde_json`. Design tool: numpy, scipy (offline only; coefficients are committed as bit patterns).

**Storage**: N/A. Three golden files regenerated (about 110 KB total, 217 entries).

**Testing**:
- **Core**: rewritten unit tests for the two stages (template and group-delay bounds from the committed coefficients), rate-plan tests for 7 rates, resampler round-trip tests at the new band, AGC and VAS unit tests at 6 kHz.
- **Harness**: `checks.rs` restated (boundary, per-stage templates with GDD, minimum phase at 6 kHz, anti-alias, cascade); `agc_checks.rs` and `vas_checks.rs` restated figures; a stimulus phase helper; all matrices over 7 rates; golden regeneration; mutation fixtures per stage.
- **C**: the smoke test gains a 6 kHz create/process and the new rate in the error-message check.
- **Coverage**: gate stays at 80 %.

**Target Platform**: Unchanged: 8 targets (001 FR-014).

**Project Type**: Library: a Rust crate plus a C ABI.

**Performance Goals**: ≥ 20× real time at 48 kHz with the default settings (SC-006). The per-sample cost falls: 8 biquads instead of 12 per device sample, fewer device samples per second, and shorter polyphase branches (R-03).

**Constraints**:
- Default latency ≤ 20 ms at all 7 rates (FR-004); planned about 9.8 ms.
- Bit-identical on 8 targets and across block partitions, including at 6 kHz.
- No allocation, locks or I/O while processing (001 FR-015).
- No `unsafe` outside `rr_dr60_ffi`.
- All three golden files change exactly once, in one commit (FR-013).
- CI time: the 7th rate adds about one sixth to each matrix; the 44.1/88.2 kHz constructions get about 4.5× cheaper, so the net is expected flat (R-12).

**Scale/Scope**: About 300–400 lines of core changes (rate table, stage generics, constants, tests), about 600–800 lines of harness changes (checks, stimuli, restated figures, 7-rate loops), the design script rewrite (two templates), the C smoke test, docs, register, CHANGELOG, and three regenerated golden files.

There are no open NEEDS CLARIFICATION items. The clarify session resolved the host-rate list (keep 8 and 16 kHz), the earlier-spec edits (one-line header pointer) and the G.712 source. The hardware-emulation review before specification fixed the four design decisions recorded in the spec's Clarifications.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | How this plan complies | Pre-research | Post-design |
|---|---|---|---|
| I. Portable Library First | No new dependencies; the core stays `no_std` + `alloc`. The C API's struct and functions are unchanged; only the documented rate list and the version string change. Header regenerated. | ✅ | ✅ |
| II. Evidence-Based Fidelity | Every template value traces to S-005 scaled per A-027, the DC figure to the new G.712 source (S-007), the rate to A-026/S-002. A-001, A-002 and A-014 are superseded in the register, A-027 corrected, A-016 upgraded, A-019 restated. The CHANGELOG lists every restated figure (R-13). Un-modeled aliasing and imaging are documented as deviations (R-15). | ✅ | ✅ |
| III. Measured, Test-First | Each FR-003 to FR-009 check maps to a harness function (R-07). The design script verifies both templates before emitting coefficients; the Rust tests re-verify from the committed bits. The golden files are re-blessed once, on purpose, with SHA guards and a CHANGELOG entry. The 100 Hz mutation stays caught per stage (R-06). | ✅ | ✅ |
| IV. Deterministic & Real-Time Safe | Coefficients are committed bit patterns; the boundary is designed at construction with detmath as before; no new allocation paths. The phase helper lives in the harness, not the core. | ✅ | ✅ |
| V. Modular, Traceable Signal Chain | Stages 4 and 10 stay independently bypassable and tap-able; they now have their own coefficient sets. Added complexity is recorded below. | ✅ | ✅ |
| VI. Neutral, Honest Communication | README and signal-chain text say the templates are the codec family's datasheet values scaled to the measured clock, medium confidence, with a list of what is not modeled (R-13, R-15). | ✅ | ✅ |
| VII. Independently Testable Features | US1 is checked by the stage measurements alone; US2 by the boundary, identity and latency checks; US3 by re-running the 002 and 003 harnesses. The AGC and VAS checks keep their isolated configurations. | ✅ | ✅ |
| Repo standards | rustdoc on changed public items; SemVer 0.4.0; Conventional Commits; no audio committed. | ✅ | ✅ |

**Gate result**: PASS. Three complexity items are recorded below, as Principle V requires.

## Project Structure

### Documentation (this feature)

```text
specs/004-device-rate-6khz/
├── spec.md
├── plan.md              # this file
├── research.md          # R-01 – R-15
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── rust-api.md      # delta vs 003
│   ├── c-api.md         # delta vs 003
│   └── golden-format.md # re-bless, counts, guards
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks (not created by /speckit-plan)
```

### Source Code (repository root)

```text
crates/rr_dr60/src/
├── settings.rs                   # DEVICE_RATE_HZ = 6000; SUPPORTED_HOST_RATES: [u32; 7]; docs
├── rate.rs                       # 6000 → Identity; new (l, m) table; TRANSITION 450 Hz sizing
├── resample/mod.rs               # CUTOFF_HZ 2775, TRANSITION_HZ 450; tests at the new band
├── resample/{up,down}.rs         # unchanged logic; RATES const and docs
├── pipeline.rs                   # two stage types in DeviceChain; __with_stage_sos(record, playback);
│                                 #   latency formula unchanged (uses DEVICE_RATE_HZ); 6000-based tests
├── lib.rs                        # crate docs (6 kHz, two templates); __test_hooks per stage
├── error.rs                      # Display of UnsupportedHostRate lists 7 rates (no code change if it
│                                 #   formats SUPPORTED_HOST_RATES)
└── stages/
    ├── voiceband.rs              # VoiceBandStage<const N: usize>; record (5) and playback (3) ctors;
    │                             #   group_delay_1k_samples generic; tests per template
    ├── voiceband_coeffs.rs       # REGENERATED: RECORD_SOS [[f64;5];5], PLAYBACK_SOS [[f64;5];3]
    ├── mod.rs                    # test helper at DEVICE_RATE_HZ; frequency list
    ├── agc.rs                    # unchanged logic; unit-test FS and sample counts
    ├── agc_hilbert_coeffs.rs     # UNCHANGED bits; header comment regenerated (6 kHz, 187.5–2812.5 Hz)
    ├── biquad.rs                 # test chirp/tail constants at 6 kHz
    └── vas.rs                    # unchanged logic (W = 32); unit-test H = 6000, O = 120

crates/rr_dr60_ffi/
├── src/lib.rs                    # host_rate_hz doc lists 7 rates; version 0.4.0 via workspace
├── include/rr_dr60.h             # regenerated (doc text only)
└── tests/c/smoke.c               # + 6 kHz pipeline; unsupported-rate message check

crates/rr_dr60_harness/
├── src/
│   ├── checks.rs                 # FR-003 boundary; FR-005/006/008 per-stage templates + GDD; FR-005/008
│   │                             #   minimum phase at 6000; FR-007 anti-alias; FR-009 cascade; report rows
│   ├── configs.rs                # all_rates() → 7
│   ├── stimulus.rs               # bandlimited_noise band 225–2550; tone_at_phase; aligned_phase helper
│   ├── agc_checks.rs             # DETECTOR_DELAY_S = 31/6000; FR-008 tones; exact timing at 6000
│   ├── vas_checks.rs             # DEVICE_RATE 6000; FR-006 frequencies; phase rule; integer-ratio test
│   ├── golden.rs                 # library_version follows VERSION; stimuli unchanged except noise band
│   ├── analysis.rs               # unchanged
│   └── report.rs                 # unchanged
├── golden/
│   ├── golden-v1.json            # RE-BLESSED (112 entries)
│   ├── golden-agc-v1.json        # RE-BLESSED (42 entries)
│   └── golden-vas-v1.json        # RE-BLESSED (63 entries)
└── tests/
    ├── golden_agc.rs             # SHA guards updated to the re-blessed 001 and AGC files
    ├── mutation.rs               # per-stage shifted fixtures (record HP, record LP, playback LP)
    ├── fixtures/                 # shifted_record_*.rs, shifted_playback_*.rs (regenerated)
    ├── latency.rs, response_matrix.rs, us1_voiceband.rs, us2_bypass_tap.rs, edge_cases.rs
    │                             # 8 kHz identity → 6 kHz; new frequency set
    ├── agc_*.rs, us*_agc*.rs     # restated figures (R-09)
    ├── vas_*.rs, us*_vas*.rs     # restated figures (R-10)
    └── determinism.rs, alloc_free.rs, ffi_parity.rs, timing.rs   # 7 rates

tools/filter-design/
├── design_voiceband.py           # two templates; GDD verification; two consts; --shift-hz per stage
├── design_hilbert.py             # FS = 6000, bands scaled; verified byte-identical output
└── README.md                     # updated

Cargo.toml                        # version 0.4.0
.github/workflows/ci.yml          # unchanged steps (7 rates come from the code)
scripts/check-traceability.sh     # unchanged (new labels follow the existing patterns)
docs/hardware/assumptions.md      # A-001/A-002/A-014 superseded; A-016 upgraded; A-019, A-027 corrected
docs/hardware/sources.md          # + S-007 ITU-T G.712
docs/hardware/signal-chain.md     # rows 4, 10 → spec 004; deviations listed
specs/00{1,2,3}-*/spec.md         # one-line header pointer each
CHANGELOG.md, README.md, CLAUDE.md
```

**Structure Decision**: Keep the four-crate workspace. The only structural change is that the voice-band stage is generic over its section count so the record and playback stages can carry different coefficient sets without a second module or a heap-allocated section list (R-06).

## Complexity Tracking

| Added complexity | Why needed | Simpler alternative rejected because |
|---|---|---|
| Two coefficient sets and a stage type generic over section count (R-06) | The datasheet's transmit and receive filters are different filters (band-pass vs low-pass); A-014's shared template is superseded (FR-005, FR-008). | Keeping one shared set cannot meet both templates. A `Vec` of sections would allocate and lose the fixed per-sample bound; padding the playback stage to 5 sections with pass-through biquads costs 2 wasted biquads per sample and muddies the op-count tests. |
| A seventh host rate (R-02) | FR-002: the identity rate must exist for the exact-at-identity checks of specs 001–003 and as the natural raw-device-rate path. | A test-hooks-only device-rate path would hide the one rate app developers may actually want (the device's own) and add a hidden code path to keep in parity with the C API. |
| Harness stimulus phase rule (R-08) | A 1 kHz tone at 6 samples per cycle, sampled off-peak, reads up to 1.25 dB low on the VAS's sample-peak detector, outside 003 FR-006's ±1 dB (spec FR-012). | Changing the reference frequency to 750 Hz would alter spec 003's law; changing the detector is the sensitivity-placement feature's job. The rule needs only the pipeline's reported latency, nothing hidden. |
