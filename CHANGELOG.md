# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Changes to assumed hardware values ([assumption register](docs/hardware/assumptions.md)) alter emulator output and are always listed here.

## [Unreleased]

### Documentation

- Hardware evidence pass (2026-10-10): the RR-DR60 service manual (S-002) and the OKI codec family datasheet (S-005) were read in full. The register now records the device's **6 kHz sampling rate**, **4 kbit/s CELP** codec and 16 Mbit flash, the µ-law coding law (A-003 verified), the discrete mic amp and the **limiter-type AGC** with a ~10 s release (A-028), the codec's PLL-clocked filters and levels (A-027, A-030), the earphone path that bypasses the power amp (A-032), and that the sensitivity setting and the VAS are DSP firmware functions after the AGC (A-033). A-001 is superseded by A-026; A-014, A-017 and A-018 are marked for revision. Emulator output was unchanged by this pass; the 6 kHz rate and the filter templates were adopted by spec 004 (see **Changed** below, 2026-10-11), which supersedes A-014. The AGC revision (A-017, A-018, A-028) is still pending. README gains a per-stage fidelity table.

### Added

- Project bootstrap: Spec-Kit setup, project constitution, community files (README, CONTRIBUTING, Code of Conduct, Security policy), issue and PR templates.
- Hardware research docs: signal-chain inventory, sources, and the initial assumption register (A-001 to A-013).
- Pipeline skeleton MVP ([spec 001](specs/001-pipeline-skeleton/spec.md)): the default record and playback voice-band chain (signal-chain stages 4 and 10) at 8/16/44.1/48/88.2/96 kHz host rates (8 kHz was the device rate at the time; 6 kHz since spec 004, below), latency reporting, and a minimal C API (`rr_dr60_ffi`, header `rr_dr60.h`). Assumptions A-014 (G.712-like filter template), A-015 (unity passband gain) and A-016 (minimum-phase codec filters).
- Stage bypass and output tap ("after record") now take effect; latency is reported per configuration; `Pipeline::reconfigure` and `rr_dr60_reconfigure` (FR-007, FR-008).
- `rr_dr60_detmath`: deterministic sin, cos, exp, ln, sqrt and Bessel I0, giving bit-identical results on every platform.
- Measurement harness (`rr_dr60_harness`, not published): deterministic stimuli, response analysis, the full measurement matrix (FR-005, FR-010–FR-013 at all six rates, 212 checks with FR and trace IDs), determinism, allocation-free, C API parity, timing and SC-008 mutation tests.
- Golden files: initial bless (`golden-v1.json`, 96 SHA-256 entries).
- CI: fmt, clippy, tests, C smoke test, the 80% coverage gate, C header drift, release timing, a cross-platform golden matrix (Linux, macOS and Windows on x86-64 and ARM64, plus the iOS simulator) and an MSRV (1.85) job. Also a release checklist (`docs/release-checklist.md`) with the manual iOS-device golden gate.
- **Record-path automatic gain control** ([spec 002](specs/002-agc/spec.md)), signal-chain stage 3, **on by default**. It raises quiet input and lowers loud input toward a target level with a 10:1 slope, which produces level "pumping" after loud sounds and background noise that rises during pauses. It runs at the device rate (8 kHz at the time; 6 kHz since spec 004) before stage 4, adds no latency, and is bit-identical on every platform. Modeled on an *assumed* AGC: new assumptions **A-017** (target −10 dBFS, 10:1 slope, +40 dB maximum gain, 20 dB maximum attenuation), **A-018** (attack 10 ms, release 1 s, exponential in dB), **A-019** (peak-responding detector on the device band, no hold, no look-ahead, starts at maximum gain) and **A-020** (always active); A-007 is refined by them.
- Rust API: `AgcSettings` (with `AgcSettings::DEVICE`), `Settings::agc`, `Tap::AfterAgc`, `Error::InvalidSetting` and `Setting`, and `Settings::validate`.
- C API: the `agc_enabled`, `agc_target_dbfs`, `agc_max_gain_db`, `agc_max_attenuation_db`, `agc_attack_ms` and `agc_release_ms` fields, `RR_DR60_TAP_AFTER_AGC`, `RR_DR60_STATUS_INVALID_SETTING`, `RrDr60SettingField` and `rr_dr60_settings_validate` (names the invalid field; real-time safe).
- AGC measurement harness: `agc_checks` and the `agc_matrix` test (1162 checks in the normal suite, 1734 in the release-mode full settings matrix), `agc_edge_cases`, AGC mutation tests, and the AGC golden file `golden-agc-v1.json` (36 entries: 3 AGC stimuli × `agc_only`/`default_agc` × 6 rates; FR-017). Bless only with `RR_DR60_BLESS=agc cargo test -p rr_dr60_harness --test golden_agc`. `golden-v1.json` is unchanged; the spec 001 checks now run with the AGC bypassed (FR-018).
- `tools/filter-design/design_hilbert.py`: designs and verifies the AGC detector's 63-tap Hilbert FIR.
- CI: the AGC golden file on every target, the release-mode AGC matrix and slow edge cases, and the traceability audit extended to the AGC checks.
- **Record-path Voice Activated System (VAS)** ([spec 003](specs/003-vas/spec.md)), signal-chain stage 5, **on by default** in drop mode. While the band-limited, AGC-regulated signal stays below a threshold for longer than the hang time, recording pauses and the paused audio is removed from the output; it resumes after the onset time once sound returns, so the start of each sound after a pause is lost and the remaining audio is joined at abrupt splices. Modeled on the owner's manual (S-001, A-008) and *assumed* values: new assumptions **A-022** (peak-responding detector, threshold −18 dBFS at sensitivity level 3, 3 dB per level), **A-023** (hang time 1.0 s), **A-024** (onset time 20 ms, no pre-roll, abrupt splices) and **A-025** (starts recording; the AGC keeps running while paused). A-021 (five microphone sensitivity levels, S-001) is refined; in this version the level moves only the VAS threshold.
- Rust API: `VasSettings` (with `VasSettings::DEVICE`), `VasMode` (`Drop`, `Mute`), `Settings::vas`, `Tap::AfterVas`, the `Setting::Vas*` variants; `BlockInfo` (`produced`, `events`, `paused`) returned by every processing call; `VasEvent` (`repr(C)`); `Pipeline::process_with_events`, `Pipeline::process_in_place_with_events` and `Pipeline::max_events` (allocation-free event reporting into a caller-provided slice).
- C API: the `vas_enabled`, `vas_mode`, `vas_sensitivity`, `vas_threshold_dbfs`, `vas_hang_ms` and `vas_onset_ms` fields, `RrDr60VasMode`, `RR_DR60_TAP_AFTER_VAS`, the `RR_DR60_SETTING_FIELD_VAS_*` values, `RrDr60BlockInfo`, `RrDr60VasEvent`, `rr_dr60_process_with_events` and `rr_dr60_max_events`. `rr_dr60_create`, `rr_dr60_reconfigure` and `rr_dr60_settings_validate` read `struct_size` before forming a reference to the settings struct.
- VAS measurement harness: `vas_checks` and the `vas_matrix` test (the defaults at all six rates, every sensitivity level and the extremes of each setting at 8 and 48 kHz, the release-mode full matrix, and the US3 AS3 floor-sweep report), `vas_edge_cases`, VAS mutation, determinism and allocation tests, and the VAS golden file `golden-vas-v1.json` (54 entries: 3 stimuli × `default_vas`/`vas_mute`/`vas_only` × 6 rates, covering the produced samples, the events and the final state; FR-019). Bless only with `RR_DR60_BLESS=vas cargo test -p rr_dr60_harness --test golden_vas`. `golden-v1.json` and `golden-agc-v1.json` are unchanged.
- CI: the VAS golden file on every target (desktop matrix, iOS simulator and the iOS-device release script), and the release-mode VAS matrix and edge cases.
- Measurement harness for [spec 004](specs/004-device-rate-6khz/spec.md): per-stage template checks (loss, DC, group-delay distortion and band-edge group delay at exact frequencies), the anti-alias, cascade, minimum-phase, rate-conversion-boundary and latency checks at all seven host rates (413 rows in `response_matrix`), three toward-band mutation fixtures, and the stimulus phase rule (`stimulus::aligned_phase`, `tone_at_phase`, `step_at_phase`) for sample-peak detectors at 6 samples per cycle.

### Changed

- **Device rate 8 → 6 kHz and the codec filter templates** ([spec 004](specs/004-device-rate-6khz/spec.md)). **Assumption change: the output changes at every host rate.** The service manual's specification table (S-002) states a 6 kHz sampling rate, so **A-026 is adopted and A-001 superseded**: the band-limit stages, the AGC and the VAS all run at 6 kHz (Nyquist 3 kHz). The two band-limit stages are now modeled on the codec family datasheet's templates (S-005) scaled by 0.75 to that clock (**A-027 adopted; A-002 and A-014 superseded**), with unity gain at 765 Hz (A-015):
  - the record stage (signal-chain stage 4) is the transmit band-pass: loss within −0.15 … +0.2 dB from 225 to 2250 Hz, 0.31 dB at 2550 Hz (datasheet typical 0.4), at least 20 dB attenuation at 45 Hz and zeros at DC;
  - the playback stage (stage 10) is the receive low-pass only, which **passes DC** (the 0.3 stage rejected it): loss within −0.15 … +0.25 dB from DC to 2250 Hz, 0.48 dB at 2550 Hz (typical 0.56).
  Both are minimum-phase (A-016, now partially verified against S-005's group-delay rows; group delay at 1 kHz 0.09 and 0.05 ms, at 2550 Hz 1.43 and 1.58 ms), implemented as a 5th-order Chebyshev-II low-pass with a 32 dB stopband, plus a 3rd-order Butterworth high-pass at 112.5 Hz in the record stage. The roll-off between 2550 Hz and 3 kHz is a design choice not constrained by any source (reported at 2700 and 2900 Hz: 16 / 33 dB record, 19 / 32 dB playback). Medium confidence: modeled on the datasheet scaled to the measured clock, **not measured against a real unit**. Known deviations, not modeled: the device's partial aliasing of 3000–3450 Hz input into the top of its band, and its DAC imaging residue above 3 kHz; the emulator's rate conversion boundary rejects both by at least 60 dB.
- **Host rates.** 6 kHz is a supported host rate and the identity: with every stage bypassed the output equals the input sample for sample and the latency is 0. 8 and 16 kHz are no longer the identity but 3:4 and 3:8 conversions, so with every stage bypassed their output passes through the rate conversion boundary (flat within ±0.1 dB to 2550 Hz, at least 60 dB rejection from 3000 Hz) instead of being the input itself. **Breaking (0.3 → 0.4):** `SUPPORTED_HOST_RATES` is `[u32; 7]`, `[6000, 8000, 16000, 44100, 48000, 88200, 96000]`; the C API accepts `host_rate_hz = 6000`; any other rate is still rejected. No setting was added or removed.
- **Default latency** (all stages on; reported value, equal to the measured 1 kHz group delay within ±1 sample), old → new:

  | Host rate | 0.3 (samples) | 0.4 (samples) | 0.4 (ms) |
  |---|---|---|---|
  | 6 kHz | — | 1 | 0.17 |
  | 8 kHz | 4 | 81 | 10.12 |
  | 16 kHz | 181 | 162 | 10.12 |
  | 44.1 kHz | 498 | 444 | 10.07 |
  | 48 kHz | 541 | 483 | 10.06 |
  | 88.2 kHz | 993 | 887 | 10.06 |
  | 96 kHz | 1081 | 966 | 10.06 |

- **AGC and VAS laws unchanged** (A-017 – A-019, A-022 – A-025 and every spec 002 and 003 tolerance), re-measured at the new rate. Two consequences of the rate alone, recorded for honesty: the AGC's detector delay is 31 device samples, now 5.17 ms instead of 3.875 ms, so at the 50 ms minimum release setting the gain holds about 5.2 ms before it starts recovering (the total release time is unchanged), and on a signal of arbitrary phase the attack on a 1 kHz step can read up to about 0.33 ms slower than at 8 kHz, inside spec 002's ±2 ms. A-019's detector band is "up to 3 kHz".
- **Restated figures** (spec 004 FR-010 to FR-012; the laws of specs 001–003 are otherwise unchanged):

  | Spec and clause | Was | Now |
  |---|---|---|
  | 001 FR-002 host rates / identity | 6 rates; 8 kHz identity | 7 rates; 6 kHz identity |
  | 001 FR-004 device rate | 8 kHz | 6 kHz |
  | 001 FR-005 boundary | flat to 3600, ≥ 60 dB from 4000 Hz | flat to 2550, ≥ 60 dB from 3000 Hz |
  | 001 FR-006 shape | same nominal shape both stages | record band-pass, playback low-pass |
  | 001 FR-010 stage template | −3 dB at 300/3400, ripple 400–3200, DC/60 Hz both stages, 14 dB at 4000, 25 dB from 4600 | record and playback templates above; DC/45 Hz record only; 4000 Hz point dropped; anti-alias ≥ 30 dB from 3450 |
  | 001 FR-010 phase check rate | 8 kHz | 6 kHz |
  | 001 FR-011 cascade range | 100–3900 Hz, 1 kHz gain | 50–2900 Hz, 765 Hz gain |
  | 001 edge case DC | ≤ −60 dBFS default (two stages) | ≤ −60 dBFS default (record stage alone); playback-only passes DC |
  | 001 latency, default | see the table above | see the table above |
  | 002 A-019 detector band | up to 4 kHz | up to 3 kHz |
  | 002 FR-006 exact-gain method rate | 8 kHz | 6 kHz |
  | 002 FR-006 timing-step stimulus (harness) | 1 kHz step at phase 0; exact y/x gated at 0.5·A | step at the aligned phase at every host rate; exact y/x gated at 0.4·A |
  | 002 FR-006 shape midpoint origin | 25 % of the release time from the step | 25 % of the release time after the detector dead time (31 device samples); window 35–65 % unchanged |
  | 002 FR-008 tones | 300, 500, 1000, 2000, 2667, 3400 Hz; phases at 2000, 2667 | 225, 375, 750, 1000, 1500, 2000, 2550 Hz; phases at 1500, 2000 |
  | 002 FR-009 noise band | 300–3400 Hz | 225–2550 Hz |
  | 002 detector delay (harness) | 31/8000 s | 31/6000 s |
  | 003 FR-004/FR-005 exact rates | 8, 16, 48, 96 kHz | 6, 48, 96 kHz |
  | 003 FR-004 lead bound / settling | ⌈host/8000⌉ (12 at 96 kHz) | ⌈host/6000⌉ (16 at 96 kHz) |
  | 003 FR-006 frequencies | 300, 1000, 3400 Hz | 225, 1000, 2550 Hz, 1 kHz phase-aligned |
  | 003 FR-004/FR-010 bit-identity rate | 8 kHz | 6 kHz |
  | 003 hang / onset in device samples | 8000 / 160 | 6000 / 120 |
  | 003 W (continuity window) | 32 samples = 4 ms | 32 samples = 5.3 ms |
  | Golden entries (001 / AGC / VAS) | 96 / 36 / 54 | 112 / 42 / 63 |

- **Golden files re-blessed once**, in the same commit as this entry: `golden-v1.json` 96 → 112 entries, `golden-agc-v1.json` 36 → 42, `golden-vas-v1.json` 54 → 63 (the seven host rates); the SHA-256 guards now pin the 0.4.0 files. Two stimulus definitions changed, both through the band-limited noise's band (225–2550 Hz): `agc_noise_burst` and `vas_noise_gaps`. No other stimulus changed; the VAS bursts keep phase 0.
- Version 0.4.0 (crates and `RR_DR60_VERSION_*`).
- **Default output now drops pauses** (S-001, A-008, A-022 – A-025), so a block can produce fewer output samples than it consumed, or none: read `BlockInfo::produced` (C: `RrDr60BlockInfo.produced`). Through the AGC's static curve (A-017) the default threshold is about −58 dBFS peak input-referred at sensitivity level 3, so the default pauses only on near-silence; an input floor whose peaks exceed about −58 dBFS keeps recording at levels 2–5. To get the 0.2 sound (AGC and band-limiting, fixed length), set `settings.vas.enabled = false` (C: `vas_enabled = false`).
- **Breaking (0.2 → 0.3):** `process` returns `Result<BlockInfo, Error>` and `process_in_place` returns `BlockInfo` (both `#[must_use]`; existing statement-form calls still compile). `rr_dr60_process` takes a new `out_info` parameter, which is always written when not NULL, including on an error status (contracts/c-api.md rule 2 amendment). `RrDr60Settings` grows from 48 to 72 bytes; code built against the 0.2 header must be rebuilt.
- Version 0.3.0 (crates and `RR_DR60_VERSION_*`).
- **Default output now includes the AGC** (A-020). To get the 0.1 sound (band-limiting only), set `settings.agc.enabled = false` (C: `agc_enabled = false`).
- **Breaking (0.1 → 0.2):** `Settings` and the C `RrDr60Settings` no longer implement `Eq`/`Hash`, because the AGC settings are floating-point. `RrDr60Settings` grows from 24 to 48 bytes; `struct_size` must still be at least `sizeof(RrDr60Settings)`, so code built against the 0.1 header must be rebuilt.
- With the AGC on, output can briefly exceed full scale by up to the maximum gain (+40 dB by default, +60 dB at most) after creation, a reset or a long silence. It is never clipped; limit or clip it before converting to integer PCM. Results beyond the `f32` range now saturate to ±`f32::MAX` instead of becoming ±Inf.
- `rr_dr60_detmath` `ln`/`exp` now also run on the processing path (spec 002 R-06); they stay bit-identical and fixed-cost.
- Version 0.2.0 (crates and `RR_DR60_VERSION_*`).

### Fixed

- Voice-band biquad state flushing (R-05): the two state values are now flushed together. Flushing them one at a time let the cascade sustain a ~1e-30 limit cycle after loud input, so the tail never reached digital silence. The change affects output only at levels around −600 dBFS.
