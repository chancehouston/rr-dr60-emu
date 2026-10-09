# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Changes to assumed hardware values ([assumption register](docs/hardware/assumptions.md)) alter emulator output and are always listed here.

## [Unreleased]

### Added

- Project bootstrap: Spec-Kit setup, project constitution, community files (README, CONTRIBUTING, Code of Conduct, Security policy), issue and PR templates.
- Hardware research docs: signal-chain inventory, sources, and the initial assumption register (A-001 to A-013).
- Pipeline skeleton MVP ([spec 001](specs/001-pipeline-skeleton/spec.md)): the default record and playback voice-band chain (signal-chain stages 4 and 10) at 8/16/44.1/48/88.2/96 kHz host rates, latency reporting, and a minimal C API (`rr_dr60_ffi`, header `rr_dr60.h`). Assumptions A-014 (G.712-like filter template), A-015 (unity passband gain) and A-016 (minimum-phase codec filters).
- Stage bypass and output tap ("after record") now take effect; latency is reported per configuration; `Pipeline::reconfigure` and `rr_dr60_reconfigure` (FR-007, FR-008).
- `rr_dr60_detmath`: deterministic sin, cos, exp, ln, sqrt and Bessel I0, giving bit-identical results on every platform.
- Measurement harness (`rr_dr60_harness`, not published): deterministic stimuli, response analysis, the full measurement matrix (FR-005, FR-010–FR-013 at all six rates, 212 checks with FR and trace IDs), determinism, allocation-free, C API parity, timing and SC-008 mutation tests.
- Golden files: initial bless (`golden-v1.json`, 96 SHA-256 entries).

### Fixed

- Voice-band biquad state flushing (R-05): the two state values are now flushed together. Flushing them one at a time let the cascade sustain a ~1e-30 limit cycle after loud input, so the tail never reached digital silence. The change affects output only at levels around −600 dBFS.
