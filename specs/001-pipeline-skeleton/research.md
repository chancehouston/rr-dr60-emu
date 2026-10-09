# Phase 0 Research: Minimal End-to-End Pipeline Skeleton

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md) | **Date**: 2026-10-08

Each entry gives a **Decision**, its **Rationale**, and the **Alternatives considered**. Plan input: Rust workspace, a pure core crate `rr_dr60` plus a C API crate via cbindgen, a block-based real-time-safe engine, cross-platform bit-identity (no FMA, no platform libm in coefficient design), minimum-phase IIR stages at 8 kHz, a boundary resampler meeting FR-005, and a measurement harness with small golden files.

---

## R-01 Language, edition, MSRV

- **Decision**: Rust, edition 2024, `rust-version = "1.85"` (the MSRV). The development toolchain is the current stable (1.99). A CI job builds and tests at the MSRV.
- **Rationale**: Edition 2024 needs 1.85. Pinning the MSRV this low lets older iOS toolchains build the library. This resolves the open decision "Minimum supported Rust version" in CLAUDE.md.
- **Alternatives**: Use latest-stable only (rejected because app developers on pinned toolchains would break). Edition 2021 (rejected because nothing forces it, and 2024's `unsafe extern` / `unsafe_op_in_unsafe_fn` defaults help the FFI layer).

## R-02 `no_std` for the core

- **Decision**: `rr_dr60` is `#![no_std]` with `extern crate alloc`, and has no default features. `core::error::Error` (stable since 1.81) provides the error trait.
- **Rationale**: Construction needs heap buffers. Processing never allocates. `no_std` plus `alloc` costs almost nothing now, is very hard to retrofit later, and makes it physically impossible for the core to do I/O or read the clock (FR-015, FR-016). This resolves the open decision "`no_std` support" in CLAUDE.md.
- **Alternatives**: `std`-only (rejected because it leaves I/O and clock access one `use` away). `no_std` without `alloc` (rejected because it would force fixed-size tables for the largest host rate, about 1 MB of static memory per instance type).

## R-03 Sample format and internal precision

- **Decision**: The public API uses `f32` samples (nominal ±1.0). All internal state, filter memory and coefficients are `f64`. Each sample is widened to `f64` on input and narrowed to `f32` on output with round-to-nearest (`as f32`).
- **Rationale**: `f32` is the native format of AVAudioEngine, Core Audio, JUCE, Web Audio and most hosts. With `f64` internals:
  1. Minimum-phase IIR sections with poles near the unit circle (the 300 Hz high-pass at 8 kHz) stay numerically clean.
  2. Intermediate values never come near the `f64` subnormal range (below 2.2e-308) for any input whose magnitude is at least 1e-30, so a host that sets FTZ/DAZ (flush denormals to zero) on its audio thread cannot change the results (see R-05).
- **Alternatives**: `f32` throughout (rejected because the high-pass poles at 300/8000 lose precision, and f32 subnormals appear in IIR tails, so FTZ-dependent output would break bit-identity). Generic over `f32`/`f64` (rejected under YAGNI, since it doubles the golden files).

## R-04 Bit-identical output across 8 platforms (FR-014)

- **Decision**: Determinism by construction:
  1. **Basic operations only on the processing path.** Only `+ − × ÷` (IEEE-754 correctly rounded) and comparisons. No `mul_add`, no `f64::sin`/`exp`/`powf` (these call the platform libm), no SIMD intrinsics, no `fast-math`. Rust/LLVM never contracts `a*b+c` into an FMA unless asked to, and never reassociates floating-point reductions, so auto-vectorization cannot change results.
  2. **Coefficient design without platform libm.** The voice-band biquad coefficients are precomputed offline and committed as exact bit patterns (`f64::from_bits(0x…)`, R-07). The resampler's Kaiser windowed-sinc taps are computed at construction with the in-house crate `rr_dr60_detmath` (R-06), which also uses basic operations only.
  3. **Lint enforcement.** `clippy::disallowed_methods` in `clippy.toml` bans `f64::{sin,cos,tan,exp,ln,log10,powf,powi,mul_add,sqrt}` and the `f32` equivalents in `rr_dr60` and `rr_dr60_detmath`. `sqrt` is correctly rounded but banned anyway for simplicity; it isn't needed.
  4. **Per-sample state machine.** Every sample goes through the same sequence of operations no matter how the input is split into blocks. Nothing is batched by block length. This gives FR-014's block-partition invariance by construction.
  5. **Supported targets only.** x86-64 always uses SSE2 (never x87), and AArch64 uses its IEEE FPU, so every target in FR-014 has identical basic operations. 32-bit x86 is not supported.
- **Rationale**: Under IEEE-754, the same sequence of basic operations on the same inputs gives the same bits on every conforming FPU. The only real sources of divergence are libm, FMA contraction, reassociation and x87, and each one is excluded above.
- **Alternatives**: Fixed-point arithmetic (rejected because it is invasive, and audio hosts are float-based). Allowing platform libm with a tolerance (rejected because the spec requires one set of golden files and bit-identity). Using the `libm` crate (rejected because some of its paths use arch-specific instructions behind features, and we only need three functions).

## R-05 Non-finite, subnormal and over-range handling

- **Decision**:
  - On input, any non-finite sample becomes `0.0`, and any `f32` subnormal becomes `0.0` (in `sanitize_in`).
  - Each biquad and FIR state value whose magnitude is below `1e-30` is set to `0.0` after each update. This is a deterministic compare-and-store.
  - On output, any `f32` result whose magnitude is below `f32::MIN_POSITIVE` becomes `0.0`.
  - No clipping is applied anywhere.
- **Rationale**: This meets the spec's edge cases and FR-002 (exact pass-through at 8 kHz except for these substitutions). The 1e-30 flush makes the IIR tails reach exact zero, which satisfies the tail rule (below −120 dBFS within 0.5 s, then digital silence) and US1 AS4. It also keeps processing time constant, avoiding subnormal slowdowns.
- **Alternatives**: Relying on CPU FTZ/DAZ (rejected because it is platform- and host-dependent and breaks bit-identity). DC-offset "denormal noise" injection (rejected because it is non-silent and conflicts with US1 AS4).

## R-06 Deterministic math crate `rr_dr60_detmath`

- **Decision**: A small `no_std` crate (MIT, published alongside the core) providing:
  - `sin`/`cos`: Cody-Waite range reduction by π/2 using a 3-part split constant, then minimax polynomials on [−π/4, π/4]. Target error is 1 ulp or less for |x| < 1e6.
  - `bessel_i0`: power series until the term is below 1e-17 of the sum, with at most 64 iterations.
  - `PI` constants.

  It is used only at construction (Kaiser taps), in the harness (stimulus generation), and in tests. Unit tests compare against high-precision reference values with tolerances. The cross-platform golden suite verifies that the results are bit-stable.
- **Rationale**: This removes the last platform-dependent math from anything that affects output bits, including the golden stimuli themselves. A separate crate lets the harness reuse it without widening `rr_dr60`'s public API.
- **Alternatives**: Precomputing all resampler taps as committed tables (rejected because 44.1/88.2 kHz prototypes run to ~35k–70k taps each, multi-MB generated source). Exposing math from `rr_dr60` (rejected because it pollutes the semver surface).

## R-07 Voice-band stage design (stages 4 and 10; A-002, A-014, A-015, A-016)

- **Decision**: The stage is one fixed IIR filter at 8 kHz: a cascade of second-order sections (biquads) in **transposed direct form II** with `f64` coefficients and state.
  - **High-pass**: Butterworth, 5th order, −3 dB at 300 Hz. It has zeros at DC, so DC attenuation is effectively infinite, exceeding FR-010's 40 dB.
  - **Low-pass**: elliptic, 6th order, 0.1 dB passband ripple, 40 dB stopband, edge at 3380 Hz.
  - **Normalization**: the overall gain is scaled to exactly 0 dB at 1 kHz (A-015).
  - **Structure**: 6 biquads in total (one HP section is first order, stored as a biquad with b2 = a2 = 0).
  - **Phase**: minimum-phase by construction. Every pole and zero of the bilinear-transformed Butterworth and elliptic prototypes lies inside or on the unit circle (A-016).

  Both stages use the same coefficients (A-014: same nominal shape).

- **Prototype measurement** (scipy, analytic response, 2026-10-08):

  | Property (FR-010) | Target | Designed |
  |---|---|---|
  | Lower −3 dB | 300 ± 50 Hz | 300 Hz |
  | Upper −3 dB | 3400 ± 50 Hz | 3406 Hz |
  | Ripple 400–3200 Hz | ±0.5 dB | ±0.25 dB |
  | ≤ 60 Hz | ≥ 20 dB | 70 dB |
  | DC | ≥ 40 dB | ∞ (zeros at z = 1) |
  | 4000 Hz | ≥ 14 dB | 39.9 dB |
  | Group delay at 1 kHz | ≤ 2 ms | 0.22 ms |
  | Group delay at 400 / 3200 Hz greater than at 1 kHz | yes | 1.41 / 0.93 ms, yes |

- **Coefficient provenance**: `tools/filter-design/design_voiceband.py` is a uv inline-script using numpy and scipy. It designs the filter, verifies every FR-010 property analytically, and emits `crates/rr_dr60/src/stages/voiceband_coeffs.rs`, with each coefficient as `f64::from_bits(0x…)` and a decimal comment, plus the design parameters and A-IDs in the header. The generated file is committed and is the runtime source of truth. CI does **not** regenerate it, because scipy runs on platform libm. Rust tests independently re-verify every FR-010 property from the committed coefficients.
- **Rationale**: Butterworth gives a ripple-free, monotonic low-frequency edge. Elliptic gives the sharp roll-off between 3400 and 4000 Hz that G.712-like codec filters have. Second-order sections in TDF-II with `f64` are the standard, numerically robust real-time IIR form. Every margin is at least 0.25 dB / 20 Hz, so later fine-tuning to real-unit evidence will not break the tests.
- **Alternatives**: Linear-phase FIR (rejected by the clarification, A-016). Chebyshev-I low-pass (needs a higher order for the same 4 kHz attenuation). Designing at runtime (rejected because it needs elliptic functions and libm, and gains nothing since the design is fixed). Direct form I (equally valid, but TDF-II needs fewer state variables).

## R-08 Rate conversion boundary (FR-004, FR-005)

- **Decision**: Rational **polyphase FIR resamplers**, linear-phase, using Kaiser-windowed sinc, designed at construction with `rr_dr60_detmath`.
  - **Down-converter** (host → 8 kHz): ratio `L/M` with `8000/host = L/M` in lowest terms:
    - 16 k: 1/2
    - 44.1 k: 80/441
    - 48 k: 1/6
    - 88.2 k: 40/441
    - 96 k: 1/12
  - **Up-converter** (8 kHz → host): the inverse ratio.
  - **8 kHz host**: identity. No filter is used and no delay is added (FR-002).
  - **Prototype filter**: cutoff 3800 Hz (the middle of the 3600–4000 Hz transition band), Kaiser window with stopband A = 70 dB (β = 0.1102·(70 − 8.7) ≈ 6.76), length chosen so the 400 Hz transition is met at the prototype rate. Taps are stored per phase, `f64`.
  - **Gain**: each polyphase branch is normalized so its DC gain is exact, so the passband is flat.
  - **Approximate sizes**:
    - 48 k: ≈ 520-tap equivalent at host rate; ≈ 87 MACs per 8 k output and ≈ 87 per host output.
    - 44.1 k: prototype ≈ 38k taps (80 phases × ≈ 478) down, and 441 phases × ≈ 87 up.
    - Memory is under 1 MB per pipeline at any rate.
  - **Delay**: (N−1)/2 at the prototype rate. N is chosen so that each converter's delay is a whole number of host samples, which keeps the latency report exact.
- **Margins**:
  - Kaiser at 70 dB gives δ ≈ 3e-4, about 0.003 dB passband ripple per converter, against FR-005's ±0.1 dB.
  - The stopband is ≥ 70 dB from 4000 Hz, against FR-005's 60 dB.
  - Each converter's delay is about 5.4 ms at every rate (the transition width is fixed in Hz).
- **Rationale**: Linear phase makes the boundary a pure delay plus a flat magnitude. That gives the "fully bypassed baseline" that FR-010 and FR-011 subtract a clean phase, so per-stage minimum-phase checks are not contaminated. The latency budget allows it: 2 × 5.4 + 2 × 0.22 ≈ 11.3 ms, against 20 ms (FR-013).
- **Alternatives**: Minimum-phase resamplers (less latency, but they distort the baseline phase and complicate FR-010's phase test). Arbitrary-ratio interpolation, sinc-table or Farrow (unnecessary for six fixed rates). A third-party crate such as `rubato` (it allocates or uses `f32`/SIMD paths, and its bit-identity is not under our control).

## R-09 Streaming structure and block-size invariance (FR-003, FR-014, FR-015)

- **Decision**: `Pipeline` processes **one host sample at a time** internally:
  1. Sanitize the sample and push it into the down-converter's history ring.
  2. If a device-rate sample is due, compute it, run it through the active stages, apply the tap, and push the result into the up-converter's history ring.
  3. The up-converter emits exactly one host-rate output sample for every host-rate input sample.

  Integer phase accumulators (`u32`) schedule the polyphase branches. Ring buffers have fixed capacities, allocated in `new`/`reconfigure`. `process` is O(N) with a constant per-sample bound of at most `taps_down/M + taps_up/L + 6 biquads`. It contains no branches that depend on block length.
- **Rationale**: Per-sample scheduling makes output independent of how the input is partitioned (bit-exact). Exactly one output per input gives FR-003's N-in/N-out. Fixed rings mean no allocation and no locks.
- **Alternatives**: Block-wise FFT convolution (output would depend on block size, it needs scratch sizing, and the FFT would need libm twiddles). Internal fixed-size chunking with a FIFO (adds latency and complexity).

## R-10 Latency reporting (FR-012, FR-013)

- **Decision**: `latency_samples()` returns `round(D_down + D_up + (τ_rec + τ_play) · host/8000)` as a `u32`, where:
  - `D_down` and `D_up` are the converters' exact linear-phase delays in host samples, integers by construction (R-08).
  - `τ_rec` and `τ_play` are the active stages' group delays at 1 kHz in device samples. They are computed once at construction from the biquad coefficients, using the analytic group-delay formula evaluated with `rr_dr60_detmath::{sin,cos}`. A bypassed stage contributes 0.
  - With tap "after record stage", `τ_play` is not added.
  - At 8 kHz, both `D` terms are 0.

  The value is fixed until `reconfigure`.
- **Expected default values**: 8 kHz: 4 samples (2 × ≈ 1.8 device samples, about 0.44 ms total). 48 kHz: ≈ 540 samples (≈ 11.3 ms). All rates are ≤ 11.4 ms, under 20 ms.
- **Rationale**: Computing the delay analytically makes the report exact up to rounding (±0.5 sample, inside FR-012's ±1). The harness checks it against a measured value independently.
- **Alternatives**: Reporting only the boundary delay (fails FR-012 once stages are on). A fractional latency API (the spec requires a whole number of samples).

## R-11 Public Rust API shape

- **Decision**: A fixed pipeline with a settings struct (not a stage graph): `Settings` (`#[non_exhaustive]`, public fields, `Settings::new(host_rate_hz)` sets the defaults), `Tap` enum, `Pipeline::{new, process, process_in_place, latency_samples, reset, reconfigure, settings}`, and `Error` enum. Details are in [contracts/rust-api.md](contracts/rust-api.md).
- **Rationale**: The spec needs only two stages, two bypass flags and one tap. A stage graph would be speculative (Principle V, YAGNI). `#[non_exhaustive]` lets later specs add stage settings without a breaking change. This resolves the open decision "Exact public API shape" for v0.x: a fixed pipeline with settings. Presets can be layered on top later.
- **Alternatives**: A dynamic stage graph (YAGNI). Builder-only construction (more API surface for no gain).

## R-12 C API (FR-023, FR-024)

- **Decision**: A separate crate `rr_dr60_ffi` (`crate-type = ["staticlib", "cdylib", "rlib"]`) exposing an opaque `RrDr60Pipeline*` handle, a `#[repr(C)] RrDr60Settings`, and a `#[repr(C)] RrDr60Status` (`int32_t` codes).
  - **Panic safety**: every entry point wraps its body in `std::panic::catch_unwind(AssertUnwindSafe(..))` and maps a panic to `RR_DR60_STATUS_INTERNAL_ERROR`. After an internal error the handle is marked poisoned: later process calls return the same error, and only `reset` or `reconfigure` clears it. The release profile keeps `panic = "unwind"` so the guard works.
  - **Header**: generated by the **cbindgen CLI** (pinned version, `cbindgen.toml`) and committed at `crates/rr_dr60_ffi/include/rr_dr60.h`. CI regenerates it and fails if it differs.
  - **iOS packaging** (XCFramework / SwiftPM) is not part of this feature; that belongs to the separate iOS repo per the constitution. CI builds the staticlib for `aarch64-apple-ios`, `aarch64-apple-ios-sim` and `x86_64-apple-ios`.

  Details are in [contracts/c-api.md](contracts/c-api.md).
- **Rationale**: The constitution requires a C ABI, cbindgen, panic safety and no Rust-only types. A committed header gives app developers something to read without running a build. `std` is fine in the FFI crate (`catch_unwind` needs it) while the core stays `no_std`.
- **Alternatives**: cbindgen in `build.rs` writing to OUT_DIR (the header would then be invisible in the repo). UniFFI (generates Swift directly, but it is heavier, allocates per call, and is not a C ABI). `panic = "abort"` (violates FR-024's "never crash the host").

## R-13 Measurement harness (FR-019 – FR-022)

- **Decision**: A workspace crate `rr_dr60_harness` (`publish = false`) containing:
  - `stimulus`:
    - tone (using `detmath::sin`, phase accumulated in `f64`)
    - exponential (log) sine sweep, 20 Hz → 0.45·host
    - unit impulse
    - silence
    - DC
    - seeded white noise: PCG32 (integer-only) mapped to [−0.5, 0.5)
  - `analysis`:
    - single-bin DFT over a whole number of tone cycles after a settling time, giving complex gain (magnitude and phase)
    - group delay by central difference of unwrapped phase (±1 Hz)
    - total-power ratio (RMS out / RMS in) for out-of-band and alias checks
    - FFT of the impulse response (`rustfft`, analysis only) and a cepstral minimum-phase reconstruction for the ±5° phase test
  - `checks`: one function per spec requirement. Each returns a `MeasurementResult` (data-model.md) that carries the requirement ID, the A-/S- IDs, the measured value, the tolerance and pass/fail.
  - `golden`: hash-based golden files (R-14).
  - `report`: a human-readable table printed by `cargo test -p rr_dr60_harness -- --nocapture`.

  Integration tests in `crates/rr_dr60_harness/tests/` run the full matrix: 6 rates × 5 configurations (default, record-only, playback-only, tap-after-record, bypass-all).
- **Per-stage measurement method**:
  - **(a)** A direct analytic evaluation of the committed coefficients in `rr_dr60` unit tests, on a dense grid that includes exactly 4000 Hz (z = −1). This is fast, exact, and covers the cases the spec skips at 8 kHz.
  - **(b)** Measured checks at every host rate via `R_cfg − R_base` (FR-010).
  - **(c)** The minimum-phase ±5° test, on the impulse response at the 8 kHz host rate. There the boundary is the identity, so the stage response is observed directly over 0–4 kHz. The same coefficients run at every rate, which (b) confirms.
- **Rationale**: Each check traces 1:1 to a requirement ID. Stimuli are bit-reproducible (detmath plus an integer PRNG), so they can feed the golden files. `rustfft` is used only for analysis, which has tolerances, so its determinism does not matter.
- **Alternatives**: Putting the harness inside `rr_dr60/tests` (it couldn't be reused by later specs as a library). Python-based analysis (adds a second runtime to CI; the scipy design tool is offline only).

## R-14 Golden-file format (FR-021)

- **Decision**: A single file `crates/rr_dr60_harness/golden/golden-v1.json`. It holds one entry per (stimulus × configuration × rate):
  - `stimulus`, `config`, `host_rate_hz`, `n_samples`
  - `sha256`: SHA-256 of the output's little-endian `f32` bit patterns
  - `head`: the first 16 output samples as hex bit patterns
  - `rms_dbfs`: RMS level, as a diagnostic

  **Stimuli**: impulse (0.25 s), 1 kHz tone at −20 dBFS (0.5 s), log sweep (1 s), seeded noise (0.5 s, seed `0x0D60`). **Configurations**: default, record-only, playback-only, bypass-all. That gives 4 × 4 × 6 = 96 entries, about 40 KB in total.

  - **Bless**: `RR_DR60_BLESS=1 cargo test -p rr_dr60_harness --test golden` rewrites the file. A CHANGELOG entry is required, enforced by the PR template checklist.
  - **On mismatch**: the report names the entry, the first differing `head` index (if any), and the RMS difference.
- **Rationale**: Hashes keep the repo small (no audio committed) while still detecting any single-bit change (SC-008). The `head` and `rms` fields give enough diagnostics to tell "everything shifted" from "a tiny change".
- **Alternatives**: Committing raw `.f32` files (about 40 MB over the matrix, which violates "no large audio"). Comparing against tolerances (not bit-identity, so it fails FR-014).

## R-15 Real-time safety verification (FR-015, FR-022, SC-004)

- **Decision**:
  1. **Allocation**: the harness test binary installs a counting `#[global_allocator]` that wraps `System` and keeps an atomic counter. The test creates the pipeline, resets the counter, processes ≥ 1000 blocks of seeded random sizes (0–8192) in every configuration and rate, and asserts that both the allocation and deallocation counts are 0. The same is done for the C API path.
  2. **Locks and I/O**: the core is `no_std` (R-02), so no std sync or io types exist. The core also has `#![forbid(unsafe_code)]`, so no FFI calls are possible from it. Code review confirms the rest.
  3. **Bounded work**: an `#[ignore]` timing test runs in release mode in CI. It measures the median ns/sample over 9 runs at block sizes {1, 64, 4096, 2^20} and after 60 s of stream, and asserts max/min ≤ 3.0. A separate test asserts the per-sample operation-count bound, using a debug-only counter enabled by the cfg feature `op-count`.
- **Rationale**: Deterministic checks are made wherever possible. Timing checks are loose and run only in release mode, to avoid CI flakiness.
- **Alternatives**: `assert_no_alloc` crate (a fine choice, but a counting allocator needs no dependency). Using only Criterion benchmarks (not a pass/fail gate).

## R-16 Cross-platform CI (FR-014, SC-003)

- **Decision**: `.github/workflows/ci.yml`:

  | Target | Runner | Golden check |
  |---|---|---|
  | x86_64-unknown-linux-gnu | ubuntu-latest | every PR, plus fmt, clippy, coverage gate (`cargo llvm-cov --fail-under-lines 80`), header drift check, C smoke test |
  | aarch64-unknown-linux-gnu | ubuntu-24.04-arm | every PR |
  | aarch64-apple-darwin | macos-latest | every PR, plus the C smoke test |
  | x86_64-apple-darwin | `macos-latest` running the x86_64 binary under Rosetta 2, or an Intel runner if one is available | every PR |
  | x86_64-pc-windows-msvc | windows-latest | every PR |
  | aarch64-pc-windows-msvc | windows-11-arm | every PR |
  | aarch64-apple-ios-sim | macos-latest; tests run in a booted simulator via a cargo `runner` script (`xcrun simctl spawn`) | every PR |
  | aarch64-apple-ios (device) | manual: `scripts/ios-device-golden.sh` (cargo-dinghy) | before each release, recorded on the release checklist |
  | x86_64-apple-ios | build only | — |

  Further jobs: an MSRV (1.85) build and test, and the timing test (release, `--ignored`) on ubuntu-latest.
- **Rationale**: This covers all 8 FR-014 targets with one set of golden files. iOS device gating follows the clarification.
- **Alternatives**: Dropping the Windows ARM64 or Linux ARM64 runners (that would break the clarified platform matrix). QEMU emulation for ARM Linux (slower, and native runners exist).
- **Risk**: hosted runner availability for the Intel macOS and Windows ARM64 runners may change. The fallbacks are documented in the workflow comments: Rosetta for x86_64 macOS, and a self-hosted runner or release-time manual run for Windows ARM64.

## R-17 Dependencies summary

| Crate | Runtime deps | Dev/test deps |
|---|---|---|
| `rr_dr60` | `rr_dr60_detmath` | none |
| `rr_dr60_detmath` | none | none |
| `rr_dr60_ffi` | `rr_dr60` | none (cbindgen CLI is an external tool) |
| `rr_dr60_harness` (publish = false) | `rr_dr60`, `rr_dr60_ffi`, `rr_dr60_detmath`, `rustfft`, `sha2`, `serde`, `serde_json` | none |
| `tools/filter-design` (offline, Python via uv) | numpy, scipy | — |

All NEEDS CLARIFICATION items from the Technical Context are resolved above.
