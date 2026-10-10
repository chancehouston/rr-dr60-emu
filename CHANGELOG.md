# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Changes to assumed hardware values ([assumption register](docs/hardware/assumptions.md)) alter emulator output and are always listed here.

## [Unreleased]

### Documentation

- Hardware evidence pass (2026-10-10): the RR-DR60 service manual (S-002) and the OKI codec family datasheet (S-005) were read in full. The register now records the device's **6 kHz sampling rate**, **4 kbit/s CELP** codec and 16 Mbit flash, the µ-law coding law (A-003 verified), the discrete mic amp and the **limiter-type AGC** with a ~10 s release (A-028), the codec's PLL-clocked filters and levels (A-027, A-030), the earphone path that bypasses the power amp (A-032), and that the sensitivity setting and the VAS are DSP firmware functions after the AGC (A-033). A-001 is superseded by A-026; A-014, A-017 and A-018 are marked for revision. **Emulator output is unchanged**: the code still models an 8 kHz, 300–3400 Hz codec and the spec 002 AGC until a feature adopts the new values, and that change will be recorded here. README gains a per-stage fidelity table.

### Added

- Project bootstrap: Spec-Kit setup, project constitution, community files (README, CONTRIBUTING, Code of Conduct, Security policy), issue and PR templates.
- Hardware research docs: signal-chain inventory, sources, and the initial assumption register (A-001 to A-013).
- Pipeline skeleton MVP ([spec 001](specs/001-pipeline-skeleton/spec.md)): the default record and playback voice-band chain (signal-chain stages 4 and 10) at 8/16/44.1/48/88.2/96 kHz host rates, latency reporting, and a minimal C API (`rr_dr60_ffi`, header `rr_dr60.h`). Assumptions A-014 (G.712-like filter template), A-015 (unity passband gain) and A-016 (minimum-phase codec filters).
- Stage bypass and output tap ("after record") now take effect; latency is reported per configuration; `Pipeline::reconfigure` and `rr_dr60_reconfigure` (FR-007, FR-008).
- `rr_dr60_detmath`: deterministic sin, cos, exp, ln, sqrt and Bessel I0, giving bit-identical results on every platform.
- Measurement harness (`rr_dr60_harness`, not published): deterministic stimuli, response analysis, the full measurement matrix (FR-005, FR-010–FR-013 at all six rates, 212 checks with FR and trace IDs), determinism, allocation-free, C API parity, timing and SC-008 mutation tests.
- Golden files: initial bless (`golden-v1.json`, 96 SHA-256 entries).
- CI: fmt, clippy, tests, C smoke test, the 80% coverage gate, C header drift, release timing, a cross-platform golden matrix (Linux, macOS and Windows on x86-64 and ARM64, plus the iOS simulator) and an MSRV (1.85) job. Also a release checklist (`docs/release-checklist.md`) with the manual iOS-device golden gate.
- **Record-path automatic gain control** ([spec 002](specs/002-agc/spec.md)), signal-chain stage 3, **on by default**. It raises quiet input and lowers loud input toward a target level with a 10:1 slope, which produces level "pumping" after loud sounds and background noise that rises during pauses. It runs at the 8 kHz device rate before stage 4, adds no latency, and is bit-identical on every platform. Modeled on an *assumed* AGC: new assumptions **A-017** (target −10 dBFS, 10:1 slope, +40 dB maximum gain, 20 dB maximum attenuation), **A-018** (attack 10 ms, release 1 s, exponential in dB), **A-019** (peak-responding detector on the device band, no hold, no look-ahead, starts at maximum gain) and **A-020** (always active); A-007 is refined by them.
- Rust API: `AgcSettings` (with `AgcSettings::DEVICE`), `Settings::agc`, `Tap::AfterAgc`, `Error::InvalidSetting` and `Setting`, and `Settings::validate`.
- C API: the `agc_enabled`, `agc_target_dbfs`, `agc_max_gain_db`, `agc_max_attenuation_db`, `agc_attack_ms` and `agc_release_ms` fields, `RR_DR60_TAP_AFTER_AGC`, `RR_DR60_STATUS_INVALID_SETTING`, `RrDr60SettingField` and `rr_dr60_settings_validate` (names the invalid field; real-time safe).
- AGC measurement harness: `agc_checks` and the `agc_matrix` test (1162 checks in the normal suite, 1734 in the release-mode full settings matrix), `agc_edge_cases`, AGC mutation tests, and the AGC golden file `golden-agc-v1.json` (36 entries: 3 AGC stimuli × `agc_only`/`default_agc` × 6 rates; FR-017). Bless only with `RR_DR60_BLESS=agc cargo test -p rr_dr60_harness --test golden_agc`. `golden-v1.json` is unchanged; the spec 001 checks now run with the AGC bypassed (FR-018).
- `tools/filter-design/design_hilbert.py`: designs and verifies the AGC detector's 63-tap Hilbert FIR.
- CI: the AGC golden file on every target, the release-mode AGC matrix and slow edge cases, and the traceability audit extended to the AGC checks.
- VAS measurement harness: `vas_checks` and the `vas_matrix` test (the defaults at all six rates, every sensitivity level and the extremes of each setting at 8 and 48 kHz, the release-mode full matrix, and the US3 AS3 floor-sweep report), `vas_edge_cases`, VAS mutation, determinism and allocation tests, and the VAS golden file `golden-vas-v1.json` (54 entries: 3 stimuli × `default_vas`/`vas_mute`/`vas_only` × 6 rates, covering the produced samples, the events and the final state; FR-019). Bless only with `RR_DR60_BLESS=vas cargo test -p rr_dr60_harness --test golden_vas`. `golden-v1.json` and `golden-agc-v1.json` are unchanged.
- **Record-path Voice Activated System (VAS)** ([spec 003](specs/003-vas/spec.md)), signal-chain stage 5, **on by default** in drop mode. While the band-limited, AGC-regulated signal stays below a threshold for longer than the hang time, recording pauses and the paused audio is removed from the output; it resumes after the onset time once sound returns, so the start of each sound after a pause is lost and the remaining audio is joined at abrupt splices. Modeled on the owner's manual (S-001, A-008) and *assumed* values: new assumptions **A-022** (peak-responding detector, threshold −18 dBFS at sensitivity level 3, 3 dB per level), **A-023** (hang time 1.0 s), **A-024** (onset time 20 ms, no pre-roll, abrupt splices) and **A-025** (starts recording; the AGC keeps running while paused). A-021 (five microphone sensitivity levels, S-001) is refined; in this version the level moves only the VAS threshold.

### Changed

- **Default output now drops pauses** (S-001, A-008, A-022 – A-025), so a block can produce fewer output samples than it consumed, or none: read `BlockInfo::produced` (C: `RrDr60BlockInfo.produced`). Through the AGC's static curve (A-017) the default threshold is about −58 dBFS peak input-referred at sensitivity level 3, so the default pauses only on near-silence; an input floor whose peaks exceed about −58 dBFS keeps recording at levels 2–5. To get the 0.2 sound (AGC and band-limiting, fixed length), set `settings.vas.enabled = false` (C: `vas_enabled = false`).
- **Default output now includes the AGC** (A-020). To get the 0.1 sound (band-limiting only), set `settings.agc.enabled = false` (C: `agc_enabled = false`).
- **Breaking (0.1 → 0.2):** `Settings` and the C `RrDr60Settings` no longer implement `Eq`/`Hash`, because the AGC settings are floating-point. `RrDr60Settings` grows from 24 to 48 bytes; `struct_size` must still be at least `sizeof(RrDr60Settings)`, so code built against the 0.1 header must be rebuilt.
- With the AGC on, output can briefly exceed full scale by up to the maximum gain (+40 dB by default, +60 dB at most) after creation, a reset or a long silence. It is never clipped; limit or clip it before converting to integer PCM. Results beyond the `f32` range now saturate to ±`f32::MAX` instead of becoming ±Inf.
- `rr_dr60_detmath` `ln`/`exp` now also run on the processing path (spec 002 R-06); they stay bit-identical and fixed-cost.
- Version 0.2.0 (crates and `RR_DR60_VERSION_*`).

### Fixed

- Voice-band biquad state flushing (R-05): the two state values are now flushed together. Flushing them one at a time let the cascade sustain a ~1e-30 limit cycle after loud input, so the tail never reached digital silence. The change affects output only at levels around −600 dBFS.
