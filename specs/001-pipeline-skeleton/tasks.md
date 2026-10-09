---

description: "Task list for 001 Minimal End-to-End Pipeline Skeleton"
---

# Tasks: Minimal End-to-End Pipeline Skeleton

**Input**: Design documents from `specs/001-pipeline-skeleton/`: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: REQUIRED. Constitution III says measurement tests are written first and must fail before implementation. Every story phase lists its tests before its implementation tasks.

**Organization**: tasks are grouped by user story (US1 = P1 MVP, US2 = P2, US3 = P3), followed by the separate cross-platform CI matrix phase (plan.md) and the polish phase.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: The user story the task belongs to (US1, US2, US3)
- Paths are relative to the repository root. The crates live in `crates/` (plan.md › Project Structure).

## Conventions that apply to every task

- **Traceability**: cite the FR, SC and A-IDs in code comments, rustdoc and test names wherever an assumed or target value is used, e.g. `// A-014: upper −3 dB at 3400 ± 50 Hz`. Engineering targets are commented `// engineering target (FR-018)`.
- **Determinism**: in `rr_dr60` and `rr_dr60_detmath`, use only `+ − × ÷` and comparisons. No `f64::sin`/`cos`/`exp`/`powf`/`mul_add`/`sqrt` and no SIMD intrinsics (R-04). `clippy.toml` enforces this.
- **Real-time safety**: no `Vec` growth, `Box::new` or formatting inside `process`, `process_in_place` or `reset` (FR-015, FR-017).
- **Public items** need rustdoc (`#![deny(missing_docs)]`). `unsafe` is allowed only in `rr_dr60_ffi`, and each block needs a `// SAFETY:` comment.
- **Commits**: use Conventional Commits, committing after each task or logical group.
- **CI and test-first**: tests that fail first stay on local commits or on the feature branch. Squash or merge to `main` only when green. CI triggers on `pull_request` and on `push` to `main`, not on every feature-branch push.
- **Before ticking a task**: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all-features` must pass. A test written to fail first may be the only exception, and only until its implementation task is done.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Workspace skeleton, toolchain configuration, generated filter coefficients and basic CI with the coverage gate (Constitution III: "as soon as the Cargo workspace exists").

- [X] T001 Create the root workspace manifest `Cargo.toml`:
  - `[workspace] resolver = "3"`, `members = ["crates/*"]`.
  - `[workspace.package]`: edition = "2024", rust-version = "1.85", license = "MIT", version = "0.1.0", repository = "https://github.com/chancehouston/rr-dr60-emu".
  - `[workspace.lints.rust]`: `missing_docs = "deny"`, `unsafe_op_in_unsafe_fn = "deny"`.
  - `[workspace.lints.clippy]`: `all = "deny"`.
  - `[profile.release]`: `panic = "unwind"` (R-12), `debug = 1`.
  - Optimized test builds: `[profile.test] opt-level = 3`, plus `[profile.dev.package.rr_dr60]` and `[profile.dev.package.rr_dr60_detmath]` at `opt-level = 3`. The suite budget is that `cargo test --all-features` completes in under 5 minutes on a CI runner.
- [X] T002 [P] Create `rust-toolchain.toml`: channel "stable", components ["rustfmt", "clippy", "llvm-tools-preview"], targets ["aarch64-apple-ios", "aarch64-apple-ios-sim", "x86_64-apple-ios"]. Also create `rustfmt.toml` with `edition = "2024"`.
- [X] T003 [P] Create `clippy.toml` with `disallowed-methods` banning `f64::sin`, `f64::cos`, `f64::tan`, `f64::exp`, `f64::ln`, `f64::log10`, `f64::powf`, `f64::powi`, `f64::mul_add`, `f64::sqrt` and the same `f32` methods. Give each the reason "R-04: not bit-identical across platforms; use rr_dr60_detmath or basic ops".
- [X] T004 [P] Create the crate skeleton `crates/rr_dr60_detmath/Cargo.toml` (workspace package fields, `[lints] workspace = true`, no dependencies) and `crates/rr_dr60_detmath/src/lib.rs` (`#![no_std]`, `#![forbid(unsafe_code)]`, crate doc comment explaining R-06).
- [X] T005 [P] Create the crate skeleton `crates/rr_dr60/Cargo.toml` (depends on `rr_dr60_detmath` by path, with version; features `__test-hooks = []` and `op-count = []`, both documented as test-only) and `crates/rr_dr60/src/lib.rs`:
  - `#![no_std]`, `extern crate alloc;`, `#![forbid(unsafe_code)]`. `no_std` together with `forbid(unsafe_code)` makes clock access, OS randomness, locks and I/O unavailable, which satisfies FR-016 by construction. Say so in the crate docs.
  - Crate docs in neutral, signal-processing-only language (Constitution VI): "modeled on the MSM7702 voice-band codec (assumed, A-002)".
  - Empty modules: `settings`, `error`, `sanitize`, `rate`, `resample/{mod,design,down,up}`, `stages/{mod,biquad,voiceband,voiceband_coeffs}`, `pipeline`.
- [X] T006 [P] Create the crate skeleton `crates/rr_dr60_ffi/Cargo.toml` (`crate-type = ["staticlib", "cdylib", "rlib"]`, depends on `rr_dr60`, feature `ffi-test-panic = []`) and `crates/rr_dr60_ffi/src/lib.rs` (crate docs; the only crate where `unsafe` is allowed).
- [X] T007 [P] Create the crate skeleton `crates/rr_dr60_harness/Cargo.toml`:
  - `publish = false`.
  - Dependencies: `rr_dr60` (with feature `__test-hooks`), `rr_dr60_ffi`, `rr_dr60_detmath`, `rustfft`, `sha2`, `serde` (derive), `serde_json`.
  - `crates/rr_dr60_harness/src/lib.rs` declares modules `stimulus`, `analysis`, `checks`, `configs`, `golden`, `report`.
  - Empty test files: `crates/rr_dr60_harness/tests/{us1_voiceband,us2_bypass_tap,response_matrix,latency,edge_cases,determinism,alloc_free,ffi_parity,golden,timing}.rs`.
- [X] T008 Write the offline design tool `tools/filter-design/design_voiceband.py`, a uv inline-script with numpy and scipy dependencies. It must:
  1. Design the R-07 filter at fs = 8000: Butterworth 5th-order high-pass at 300 Hz plus elliptic 6th-order low-pass (0.1 dB ripple, 40 dB stopband, 3380 Hz), as SOS.
  2. Scale the gain to exactly 0 dB at 1 kHz (A-015).
  3. Verify analytically every FR-010 bound: −3 dB at 300 ± 50 Hz and 3400 ± 50 Hz; ripple ±0.5 dB from 400 to 3200 Hz; ≥ 20 dB at 60 Hz and below; ≥ 40 dB at DC; ≥ 14 dB at 4000 Hz; group delay ≤ 2 ms at 1 kHz; group delay at 400 Hz and 3200 Hz greater than at 1 kHz; every pole and zero with |z| ≤ 1. Exit non-zero on any failure.
  4. Print a margin table. With `--check`, stop after design and verification without writing any file (used by the Phase 1 checkpoint). With `--shift-hz N --emit-fixture PATH`, design with both band edges shifted by N Hz and write a test fixture instead (used by T057).
  5. Emit `crates/rr_dr60/src/stages/voiceband_coeffs.rs`. The header must say "GENERATED — do not edit" and give the design parameters, A-002, A-014, A-015, A-016, and the regeneration command. Each coefficient is `f64::from_bits(0x…)` with a decimal comment, in `pub(crate) const VOICEBAND_SOS: [[f64; 5]; 6]` (b0, b1, b2, a1, a2), with the first-order section padded with b2 = a2 = 0.
- [X] T009 Commit a **placeholder** `crates/rr_dr60/src/stages/voiceband_coeffs.rs` with 6 identity sections (`[1.0, 0.0, 0.0, 0.0, 0.0]`), the same `VOICEBAND_SOS` signature, and the header "PLACEHOLDER — replaced in T029a". The placeholder lets the analytic test T023 fail first (Constitution III). Write `tools/filter-design/README.md` covering the purpose, the command, and the rule that "any change requires golden re-bless + CHANGELOG entry + assumption-register check" (Constitution II).
- [X] T010 [P] Create the basic CI workflow `.github/workflows/ci.yml`, triggered on `pull_request` and on `push` to `main`. Jobs `check` on ubuntu-latest and macos-latest each run:
  - checkout, then the toolchain from `rust-toolchain.toml`
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test --all-features`

  A `coverage` job on ubuntu-latest installs cargo-llvm-cov and runs `cargo llvm-cov --all-features --workspace --fail-under-lines 80` from day one (Constitution III). Because CI runs only on PRs to `main` and pushes to `main`, the gate applies whenever the branch merges, which requires the branch to be green. Add a comment that the cross-platform matrix is added in Phase 6.
- [X] T011 Verify the skeleton: `cargo build --workspace`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --workspace` all succeed (with no tests yet). Commit as `chore: scaffold cargo workspace, design tool, basic CI`.
- [X] T011a Push `001-pipeline-skeleton` to `origin` and open a **draft** PR to `main` titled `feat: 001 pipeline skeleton — US1 (MVP)`. The body links spec.md, plan.md and tasks.md and says the PR merges at the US1 checkpoint (T039a). Draft PRs trigger the `pull_request` CI from T010. Every later push re-runs CI.

**Checkpoint**: The workspace builds, the draft PR exists, the CI `check` jobs are green (the coverage job is expected to stay red until the Phase 2 code and tests land), the design tool runs (`uv run tools/filter-design/design_voiceband.py --check` passes), and the placeholder coefficient file exists.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Deterministic math, settings and errors, input sanitizing, and the minimal harness stimulus and analysis code that US1's own tests need (Principle VII).

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

### Tests (write first, must fail)

- [ ] T012 [P] Write unit tests in `crates/rr_dr60_detmath/src/lib.rs` (`#[cfg(test)]`):
  - `sin`/`cos` against a table of at least 64 high-precision reference values (hard-coded decimal literals from a 50-digit source) for x in [−1e4, 1e4], including multiples of π/2 ± tiny. Tolerance: ≤ 2 ulp.
  - `bessel_i0` at x ∈ {0, 0.5, 1, 2, 5, 6.76, 10, 20} against reference values. Tolerance: relative error ≤ 1e-14.
  - Identities sin² + cos² within 4e-16 over 10,000 points.
  - `exp` and `ln` within 2 ulp of high-precision reference values at 64 points each (exp over [−700, 700], ln over [1e-300, 1e300]).
  - `sqrt` is bit-equal to the correctly rounded `f64::sqrt` at 10,000 seeded points. Test code is allowed to call `f64::sqrt` (`#[allow(clippy::disallowed_methods)]` on the test module only).
- [ ] T013 [P] Write unit tests in `crates/rr_dr60/src/settings.rs` and `crates/rr_dr60/src/error.rs` (`#[cfg(test)]`):
  - `Settings::new(48000)` has defaults `record_stage_enabled: true`, `playback_stage_enabled: true`, `tap: Tap::AfterPlayback`, `seed: 0`.
  - `SUPPORTED_HOST_RATES == [8000, 16000, 44100, 48000, 88200, 96000]`.
  - `DEVICE_RATE_HZ == 8000` (A-001).
  - `Error::UnsupportedHostRate { requested: 22050 }` displays text listing all six supported rates.
  - `Error::LengthMismatch` displays both lengths.
- [ ] T014 [P] Write unit tests in `crates/rr_dr60/src/sanitize.rs` (`#[cfg(test)]`):
  - `sanitize_in`: NaN, +Inf, −Inf and f32 subnormals (e.g. `f32::from_bits(1)`) map to 0.0. Finite normal values, including |x| > 1.0, map to the exact same value as f64.
  - `narrow_out`: f64 values that round to an f32 subnormal map to 0.0; normal values round to nearest.
  - `flush_state(x)`: |x| < 1e-30 gives 0.0, otherwise x is unchanged (R-05).
- [ ] T015 [P] Write unit tests in `crates/rr_dr60_harness/src/stimulus.rs` (`#[cfg(test)]`):
  - The tone has the requested length and its peak is within 1e-12 of the requested amplitude.
  - The impulse is 1.0 at n = 0 and zero elsewhere.
  - Silence and DC have constant values.
  - PCG32 (seed 0x0D60, stream 0): the first 4 `u32` outputs equal the values from the PCG reference algorithm (`pcg32_random_r`), computed in the test from the published constants.
  - Noise lies in [−0.5, 0.5).
  - The log sweep starts at phase 0 and its instantaneous frequency at the start and end is within 1% of the requested start and end frequencies.
- [ ] T016 [P] Write unit tests in `crates/rr_dr60_harness/src/analysis.rs` (`#[cfg(test)]`):
  - The single-bin DFT of a synthetic sinusoid with known amplitude and phase recovers both, within 1e-9 relative and 1e-9 rad.
  - Group delay of a synthetic pure delay of 37 samples is measured as 37.0 ± 0.01.
  - The power ratio of x vs 0.5·x is −6.0206 dB ± 1e-6.
  - `db()`/`from_db()` round-trip.

### Implementation

- [ ] T017 Implement `crates/rr_dr60_detmath/src/lib.rs` (R-06), using only basic operations and no `core::f64` intrinsic methods:
  - `pub fn sin(x: f64) -> f64`, `pub fn cos(x: f64) -> f64`: Cody-Waite reduction by π/2 with a 3-part split of π/2, then minimax polynomials on [−π/4, π/4] (degree 13 for sin and 14 for cos, from the published fdlibm `__kernel_sin`/`__kernel_cos` coefficients, cited in a comment).
  - `pub fn bessel_i0(x: f64) -> f64`: power series, stopping when the term is below 1e-17·sum or after 64 terms.
  - `pub fn exp(x: f64) -> f64`: Cody-Waite reduction by ln 2, then a polynomial (fdlibm `e_exp` coefficients). `pub fn ln(x: f64) -> f64`: reduce the argument to [√½, √2], then an atanh series (fdlibm `e_log` coefficients). `pub fn sqrt(x: f64) -> f64`: Newton iteration from a bit-level initial guess, with a final correction step so it rounds correctly.
  - Keep Sun's fdlibm copyright and permission notice in a comment at the top of `crates/rr_dr60_detmath/src/lib.rs`, and add `THIRD_PARTY.md` at the repository root listing fdlibm and its notice (MIT compatibility).
  - `pub const PI`, `pub const TAU`.
  - Make T012 pass.
- [ ] T018 [P] Implement `crates/rr_dr60/src/settings.rs` and `crates/rr_dr60/src/error.rs` exactly per [contracts/rust-api.md](contracts/rust-api.md). Implement `SUPPORTED_HOST_RATES`, `DEVICE_RATE_HZ` (doc cites A-001), `Tap` (`#[non_exhaustive]`, `#[default] AfterPlayback`), `Settings` (`#[non_exhaustive]`, `const fn new(host_rate_hz)`, defaults citing A-002 for both stages enabled and FR-009 for the seed), and `Error` (`#[non_exhaustive]`, `Display`, `core::error::Error`). Data-model constraint, quoted verbatim: "`host_rate_hz` … Must be one of `SUPPORTED_HOST_RATES = [8000, 16000, 44100, 48000, 88200, 96000]`, otherwise `Error::UnsupportedHostRate`". Validation happens in `Pipeline::new` / `reconfigure`, not in `Settings::new`. Re-export from `crates/rr_dr60/src/lib.rs`, along with `pub const VERSION`. Make T013 pass.
- [ ] T019 [P] Implement `crates/rr_dr60/src/sanitize.rs`: `#[inline] pub(crate) fn sanitize_in(x: f32) -> f64`, `narrow_out(y: f64) -> f32` and `flush_state(x: f64) -> f64`, with threshold constant `STATE_FLUSH: f64 = 1e-30` (commented R-05). Make T014 pass.
- [ ] T020 [P] Implement `crates/rr_dr60_harness/src/stimulus.rs`: `tone(freq_hz, amp, fs, n)` (phase accumulated in f64, `detmath::sin`), `impulse(n)`, `silence(n)`, `dc(level, n)`, `log_sweep(f0, f1, amp, fs, n)` (exponential sweep; phase in closed form using `detmath::{exp, ln, sin}`), `Pcg32::new(seed, stream)` / `next_u32()`, and `noise(seed, n)`. All outputs are `Vec<f32>`. All stimulus math MUST use `rr_dr60_detmath` only, because stimuli feed the golden files (FR-014). `stimulus.rs` stays under the clippy R-04 ban. Make T015 pass.
- [ ] T021 [P] Implement `crates/rr_dr60_harness/src/analysis.rs`:
  - `single_bin(signal, freq, fs, start, cycles) -> Complex`, a DFT over a whole number of cycles.
  - `gain_db_and_phase(input, output, freq, fs, settle)`.
  - `group_delay_samples(measure_fn, freq, fs)`: central difference ±1 Hz, unwrapped.
  - `power_ratio_db(input, output, settle)`.
  - `db` / `from_db`.

  The harness may use std `f64` math (analysis is tolerance-based, not bit-critical), so the R-04 clippy ban is overridden with a documented `#![allow(clippy::disallowed_methods)]` at the top of `analysis.rs` **only**, never crate-wide. `stimulus.rs`, `golden.rs` and `configs.rs` stay under the ban. Make T016 pass.
- [ ] T022 Implement the `MeasurementResult` and `Tolerance` types in `crates/rr_dr60_harness/src/checks.rs`, per [data-model.md](data-model.md) › Measurement result:
  - Fields `requirement`, `property`, `trace`, `host_rate_hz`, `config`, `stimulus`, `measured`, `unit`, `tolerance`, `passed`.
  - `Tolerance::{Range, AtLeast, AtMost, Exact}` with `contains()`.
  - Helper `assert_all(results)` panics with a formatted table of the failures.
  - Confirm locally with `cargo llvm-cov --all-features --workspace --fail-under-lines 80` that coverage is at least 80% once the Foundational code and tests exist.

**Checkpoint**: detmath, settings, sanitize, stimulus and analysis are all unit-tested and green. User stories can begin.

---

## Phase 3: User Story 1 - Hear audio through the RR-DR60 voice band (Priority: P1) 🎯 MVP

**Goal**: A default pipeline at any of the six host rates turns mono audio into band-limited, telephone-style audio (stages 4 and 10), N samples in and N out, with a minimal C API that gives the same result.

**Independent Test**: `cargo test -p rr_dr60_harness --test us1_voiceband` passes. It covers acceptance scenarios AS1–AS5 at 48 kHz and 44.1 kHz, plus N-in/N-out at all 6 rates. It needs only the Foundational harness pieces, not the US3 matrix or golden files.

### Tests for User Story 1 (write first, must fail) ⚠️

- [ ] T023 [P] [US1] Analytic stage tests in `crates/rr_dr60/src/stages/voiceband.rs` (`#[cfg(test)]`). Evaluate H(e^jω) of the committed `VOICEBAND_SOS` with `detmath::{sin,cos}` on a 1 Hz grid from 0 to 4000 Hz, including exactly 4000 Hz (z = −1). Assert every FR-010 bound:
  - |H(1 kHz)| = 0 dB ± 0.1 (A-015).
  - Lower −3 dB at 300 ± 50 Hz; upper −3 dB at 3400 ± 50 Hz.
  - Ripple ±0.5 dB from 400 to 3200 Hz.
  - ≥ 20 dB attenuation from 1 Hz to 60 Hz; ≥ 40 dB at DC; ≥ 14 dB at 4000 Hz.
  - Group delay ≤ 2 ms at 1 kHz; group delay at 400 Hz and at 3200 Hz greater than at 1 kHz.
  - All section poles strictly inside the unit circle and zeros on or inside it (A-016).

  Also assert `group_delay_1k_samples()` matches the grid value within 1e-9.
- [ ] T024 [P] [US1] Biquad tests in `crates/rr_dr60/src/stages/biquad.rs` (`#[cfg(test)]`):
  - The impulse response of a known section (b = [0.5, 0.25, 0], a1 = −0.5, a2 = 0) matches values computed by hand for 8 samples.
  - After input stops, the voiceband cascade output is below −120 dBFS within 4,000 samples (0.5 s at 8 kHz, spec Edge Cases), and exactly 0.0 within 20,000 samples (R-05).
  - `reset()` zeroes the state.
- [ ] T025 [P] [US1] Resampler tests in `crates/rr_dr60/src/resample/mod.rs` (`#[cfg(test)]`) for host rates **48000 and 44100**:
  - `RatePlan` gives L/M = 1/6 and 80/441.
  - The converter delays `d_down` and `d_up` are integers in host samples.
  - Down→up round trip (stages absent) of tones at 50, 1000 and 3600 Hz: gain within ±0.1 dB (FR-005), with the steady-state DFT after a settling time of `d_down + d_up` plus 1000 samples.
  - Tones at 4000, 4600, 8000 and 20000 Hz: total output power ≥ 60 dB below the input (FR-005, engineering target).
  - For 1 kHz in, alias and image energy outside a ±5 Hz band around 1 kHz is ≥ 60 dB down.
  - Design-level: the Kaiser prototype passband ripple is ≤ 0.01 dB up to 3600 Hz and the stopband ≥ 70 dB from 4000 Hz, measured from the taps with a direct DFT.
- [ ] T026 [P] [US1] Extend the resampler tests in `crates/rr_dr60/src/resample/mod.rs` to host rates **16000, 88200 and 96000** (L/M = 1/2, 40/441, 1/12), using the same assertions as T025. Also test that **8000** gives `RatePlan::Identity` with zero delay.
- [ ] T027 [US1] US1 acceptance tests in `crates/rr_dr60_harness/tests/us1_voiceband.rs`, using only `stimulus` and `analysis` from Foundational:
  - **AS1**: default pipeline at 48 kHz, 1 kHz tone at −20 dBFS. The steady-state output level is −20 dBFS ± 0.2 dB (A-015).
  - **AS2**: default pipeline at 44.1 kHz. A 50 Hz tone and a 3800 Hz tone each come out at least 20 dB below the input level (A-002, A-014).
  - **AS3**: at each of the 6 rates, blocks of length 0, 1, 7, 64, 4096 and 48,000 each return exactly that many output samples. `process` with mismatched lengths returns `Err(LengthMismatch)` and leaves the state unchanged (the next output equals that of a fresh pipeline).
  - **AS4**: a fresh pipeline fed 2 s of silence outputs only exact `0.0`.
  - Also: `Pipeline::new(Settings::new(22050))` returns `Err(UnsupportedHostRate { requested: 22050 })`.
- [ ] T028 [US1] US1 AS5 test in `crates/rr_dr60_harness/tests/us1_voiceband.rs`, using the C API through the `rr_dr60_ffi` rlib's `extern "C"` functions:
  - `rr_dr60_settings_default(48000)`, then `rr_dr60_create`, then process a 1 kHz tone with `rr_dr60_process` (copy mode and in-place mode), then `rr_dr60_latency_samples`, `rr_dr60_reset` and `rr_dr60_destroy`.
  - The output is bit-identical to `Pipeline::process` on the same input.
  - The latency equals `Pipeline::latency_samples()`.
  - Unsupported rate 22050 gives `RR_DR60_STATUS_UNSUPPORTED_HOST_RATE` with `*out` untouched; NULL settings gives `RR_DR60_STATUS_NULL_POINTER`. No crash.

### Implementation for User Story 1

- [ ] T029 [P] [US1] Implement `crates/rr_dr60/src/stages/biquad.rs`: `pub(crate) struct Biquad { b0, b1, b2, a1, a2, s1, s2: f64 }`, transposed direct form II (`y = b0·x + s1; s1 = b1·x − a1·y + s2; s2 = b2·x − a2·y`), applying `flush_state` to s1 and s2 after each update (R-05), with `const fn from_sos([f64; 5])` and `reset()`. Make T024 pass.
- [ ] T029a [US1] Run `cargo test -p rr_dr60 stages::voiceband` and confirm T023 **fails** against the placeholder coefficients. Then run `uv run tools/filter-design/design_voiceband.py`, review its margin table, commit the generated `crates/rr_dr60/src/stages/voiceband_coeffs.rs` (A-002, A-014, A-015, A-016), and confirm the analytic part of T023 that tests the coefficient file passes.
- [ ] T030 [US1] Implement `crates/rr_dr60/src/stages/voiceband.rs`: `pub(crate) struct VoiceBandStage { sections: [Biquad; 6] }` built from `VOICEBAND_SOS`. Doc comment: "Models signal-chain stage 4 (record anti-alias + ADC) and stage 10 (DAC + reconstruction) of the MSM7702 voice-band codec — assumed G.712-like (A-002, A-014), unity gain (A-015), minimum-phase (A-016)".
  - `process(x: f64) -> f64` and `reset()`.
  - `pub(crate) fn group_delay_1k_samples() -> f64`: the analytic group delay at 1 kHz summed over the sections, using `detmath`.
  - Under `#[cfg(feature = "__test-hooks")]`, `#[doc(hidden)] pub fn with_sos(sos: [[f64; 5]; 6])` for SC-008 mutation tests.
  - Make T023 pass.
- [ ] T031 [US1] Implement `crates/rr_dr60/src/resample/design.rs` (R-08): `kaiser_beta(atten_db = 70.0)` (0.1102·(A − 8.7)) and `kaiser_lowpass(num_taps, cutoff_norm, beta) -> Vec<f64>` (windowed sinc via `detmath::{sin, bessel_i0, sqrt}`). Also implement `polyphase_split(taps, phases) -> Box<[f64]>`, with each branch normalized so its DC sum equals 1.0 for the down-converter and the interpolation gain is exact for the up-converter.
- [ ] T032 [US1] Implement `crates/rr_dr60/src/rate.rs`:
  - `RatePlan::for_host(host_hz) -> Result<RatePlan, Error>` with L/M from the R-08 table.
  - `num_taps` chosen as the smallest value meeting the 400 Hz transition at A = 70 dB (Kaiser estimate (A − 8)/(2.285·Δω)) and adjusted upward so that (N−1)/2 at the prototype rate is a whole number of host samples. A comment cites FR-005 as an engineering target.
  - `d_down`, `d_up` (host samples).
  - Identity for 8000.
- [ ] T033 [US1] Implement the polyphase decimator `crates/rr_dr60/src/resample/down.rs`. `PolyphaseDown { phases, taps_per_phase, history: ring buffer of f64 with fixed capacity allocated in new, write index, phase_acc: u32 }`. `push(x) -> Option<f64>` emits a device-rate sample when due. It runs per sample, does not allocate, and flushes state with `flush_state`. Implement `reset()`.
- [ ] T034 [US1] Implement the polyphase interpolator `crates/rr_dr60/src/resample/up.rs`. `PolyphaseUp { … }` provides `push_device(d)` and `next_host() -> f64`, which returns exactly one host sample per call (R-09). It is causal and aligned so that `next_host` never needs a device sample that hasn't arrived. Implement `reset()`. Then wire both into `crates/rr_dr60/src/resample/mod.rs`. Make T025 pass (48 k and 44.1 k first, as plan.md says, because 80/441 is the biggest risk), then T026.
- [ ] T035 [US1] Implement `crates/rr_dr60/src/pipeline.rs` for the default configuration, following the per-sample engine in [data-model.md](data-model.md) (steps 1–3, invariants I1–I4):
  - `Pipeline::new(settings)`: validate the rate, build the `RatePlan`, the converters (absent at 8 kHz) and two `VoiceBandStage`s.
  - `process(&mut self, input, output) -> Result<(), Error>`: check the lengths first, then run a per-sample loop.
  - `process_in_place(&mut self, buffer)`.
  - `latency_samples()`: per R-10, `round(d_down + d_up + (τ_rec + τ_play)·host/8000)`, computed in `new` and stored as `u32`.
  - `reset()`: no allocation.
  - `settings()`.
  - Public rustdoc as in contracts/rust-api.md, marking real-time-safe methods. Re-export `Pipeline` from `lib.rs`.
  - Make T027 pass.
- [ ] T036 [US1] Implement the C API core in `crates/rr_dr60_ffi/src/lib.rs`, exactly per [contracts/c-api.md](contracts/c-api.md):
  - `#[repr(i32)] RrDr60Status` (OK = 0, NULL_POINTER = 1, UNSUPPORTED_HOST_RATE = 2, INVALID_ARGUMENT = 3, INTERNAL_ERROR = 4) and `#[repr(u32)] RrDr60Tap`.
  - `#[repr(C)] RrDr60Settings { struct_size: u32, host_rate_hz: u32, record_stage_enabled: bool, playback_stage_enabled: bool, tap: u32, seed: u64 }`.
  - An opaque `RrDr60Pipeline` wrapping `rr_dr60::Pipeline` and a `poisoned: bool`.
  - `#[unsafe(no_mangle)] extern "C"` functions: `rr_dr60_settings_default`, `rr_dr60_create`, `rr_dr60_destroy` (NULL is a no-op), `rr_dr60_process`, `rr_dr60_latency_samples`, `rr_dr60_reset` (clears the poisoned flag), `rr_dr60_version_string` (a static NUL-terminated string).
  - `rr_dr60_process` rules: frames == 0 accepts NULLs; NULL with frames > 0 gives NULL_POINTER; input == output takes the in-place path; any other overlap gives INVALID_ARGUMENT.
  - Validation: `struct_size < size_of::<RrDr60Settings>()` gives INVALID_ARGUMENT; tap > 1 gives INVALID_ARGUMENT.
  - Every body is wrapped in `catch_unwind(AssertUnwindSafe(..))`. A panic gives INTERNAL_ERROR and sets `poisoned`. While poisoned, `process` and `latency` return INTERNAL_ERROR.
  - On error, `*out` parameters are untouched.
  - Every `unsafe` block has a `// SAFETY:` comment.
  - Make T028 pass.
- [ ] T037 [US1] Add `crates/rr_dr60_ffi/cbindgen.toml` (language C, include guard `RR_DR60_H`, `cpp_compat = true`, `RR_DR60_VERSION_{MAJOR,MINOR,PATCH}` defines, enum prefixing `RR_DR60_STATUS_*` / `RR_DR60_TAP_*`, and a doc header with the ownership, threading and real-time notes from contracts/c-api.md Rules 1–5). Install the pinned cbindgen (`cargo install cbindgen --locked --version 0.29.0`, or the latest at implementation time, with the pin recorded in ci.yml later). Generate and commit `crates/rr_dr60_ffi/include/rr_dr60.h`. Check it matches the normative shape in contracts/c-api.md.
- [ ] T038 [US1] Write the C smoke test `crates/rr_dr60_ffi/tests/c/smoke.c`. It creates a 48 kHz default pipeline, processes 0.5 s of a 1 kHz tone at −20 dBFS, checks the RMS level after settling is within ±0.2 dB, checks the latency is non-zero and ≤ 960 (20 ms), exercises every row of the contracts/c-api.md error table that doesn't need reconfigure, destroys the pipeline, prints `smoke: OK` and returns 0. Add a `smoke` step (build the staticlib, `cc … && ./target/smoke`) to the ubuntu and macos jobs in `.github/workflows/ci.yml`.
- [ ] T039 [US1] Coverage hygiene for US1. In `crates/rr_dr60_ffi/src/lib.rs`:
  - Use **no coverage exclusions**. Route every `catch_unwind` panic arm through one small `fn panic_to_status()`. The `ffi-test-panic` tests (T054) execute it under `--all-features`, which the coverage job uses. Add a justification comment (Constitution III) only if a measured arm still shows as uncovered.
  - Behind `#[cfg(feature = "ffi-test-panic")]`, add a `#[doc(hidden)] extern "C" fn rr_dr60__test_force_panic(p)` that makes the next process call panic.
  - Run `cargo llvm-cov --workspace --all-features` locally and confirm the total is ≥ 80%. The panic arm is reached once T054 lands.

- [ ] T039a [US1] Ship the MVP. Once the checkpoint below holds:
  - Add a `CHANGELOG.md` `[Unreleased]` entry: "Added: pipeline skeleton MVP — default record + playback voice-band chain (stages 4 and 10) at 8/16/44.1/48/88.2/96 kHz, latency reporting, minimal C API; assumptions A-014–A-016".
  - Add a short "Status: MVP" note with the Rust and C snippets to `README.md`.
  - Mark the draft PR ready for review, get it green, and merge it to `main` with a merge commit (the repo convention). Afterwards, keep working on `001-pipeline-skeleton`: `git fetch && git merge origin/main`.

**Checkpoint (US1 / MVP)**: `cargo test -p rr_dr60_harness --test us1_voiceband` passes, the C smoke test prints `smoke: OK`, CI is green on ubuntu and macos (coverage ≥ 80% included), and the PR is merged. Stop and demo. This is the MVP.

---

## Phase 4: User Story 2 - Choose stages and tap point (Priority: P2)

**Goal**: Bypass each stage independently, tap after the record stage, report the correct latency per configuration, and reconfigure with the strong guarantee.

**Independent Test**: `cargo test -p rr_dr60_harness --test us2_bypass_tap` passes (AS1–AS4). Per-stage checks run with the other stage bypassed and subtract the bypass-all baseline (FR-010), so they don't depend on the other stage's behavior.

### Tests for User Story 2 (write first, must fail) ⚠️

- [ ] T040 [P] [US2] US2 acceptance tests in `crates/rr_dr60_harness/tests/us2_bypass_tap.rs`, at 48 kHz and 8 kHz:
  - **AS1**: `record_only` (playback bypassed). At 300, 1000, 3200 and 3400 Hz, `R_cfg − R_base` is within FR-010 tolerances of the analytic stage response: ±0.3 dB (engineering target), with the stage's 1 kHz gain at 0 ± 0.1 dB absolute.
  - **AS2**: `tap_after_record` output is bit-identical to `record_only` output for the log sweep, whether `playback_stage_enabled` is true or false.
  - **AS3**: `bypass_all` at 48 kHz is flat within ±0.1 dB from 50 to 3600 Hz. At 8 kHz, `bypass_all` output equals the input exactly (`to_bits` equality), except that injected NaN, Inf and subnormal samples become 0.0 (FR-002).
  - **AS4**: for each of the 5 named configurations, `latency_samples()` equals the measured 1 kHz group delay within ±1 sample (engineering target).
- [ ] T041 [P] [US2] Reconfigure tests in `crates/rr_dr60_harness/tests/us2_bypass_tap.rs`:
  - `reconfigure(s)` followed by input X gives output bit-identical to `Pipeline::new(s)` followed by X, across rate changes (48k → 44.1k → 8k) and flag changes. The latency updates.
  - `reconfigure(Settings::new(22050))` returns `Err(UnsupportedHostRate)` and leaves the settings, latency and subsequent output unchanged (the strong guarantee, data-model.md).
  - C API: `rr_dr60_reconfigure` with tap = 7 gives INVALID_ARGUMENT and the old configuration still processes. With a valid change, the result matches the Rust output.

### Implementation for User Story 2

- [ ] T042 [US2] Extend the per-sample engine in `crates/rr_dr60/src/pipeline.rs`. Honour `record_stage_enabled`, `playback_stage_enabled` and `tap`: a bypassed stage passes the sample through bit-exactly (no arithmetic); with tap `AfterRecord`, stage 10 is not run (data-model.md › Tap). Update the R-10 latency computation: a bypassed stage contributes 0, and `AfterRecord` excludes τ_play. Make T040 pass.
- [ ] T043 [US2] Implement `Pipeline::reconfigure(&mut self, settings) -> Result<(), Error>` in `crates/rr_dr60/src/pipeline.rs`. Build a complete new `Pipeline` first, then swap it in, so that on error nothing changes. Document it as "may allocate; not real-time safe (FR-008)".
- [ ] T044 [US2] Implement `rr_dr60_reconfigure` in `crates/rr_dr60_ffi/src/lib.rs`: the same validation as create, a successful call clears `poisoned`, and `catch_unwind` is used. Regenerate `crates/rr_dr60_ffi/include/rr_dr60.h` with cbindgen. Add the reconfigure rows from the contracts/c-api.md error table to `crates/rr_dr60_ffi/tests/c/smoke.c`. Make T041 pass.

**Checkpoint (US2)**: `us1_voiceband` and `us2_bypass_tap` both pass, and the C smoke test still passes.

---

## Phase 5: User Story 3 - Measurement harness proves the behavior (Priority: P3)

**Goal**: The full measurement matrix (6 rates × 5 configurations), golden-file regression, determinism, allocation-free processing, FFI parity, timing, edge cases and the SC-008 mutation check, each reporting FR and trace IDs (FR-019 – FR-022).

**Independent Test**: `cargo test -p rr_dr60_harness` passes and prints the report. A test-hook build with a band edge shifted by 100 Hz fails both the response check and the golden check (SC-008).

### Tests and harness components for User Story 3 (write first; the checks must fail against a deliberately broken stage) ⚠️

- [ ] T045 [P] [US3] Implement `crates/rr_dr60_harness/src/configs.rs`: the named configurations `default`, `record_only`, `playback_only`, `tap_after_record` and `bypass_all`, exactly as in the data-model.md table, and `all_rates()` returning the 6 supported rates. Unit-test that each config maps to the right `Settings`.
- [ ] T046 [P] [US3] Implement `crates/rr_dr60_harness/src/report.rs`: format `MeasurementResult`s as an aligned table (requirement, property, trace, rate, config, measured, tolerance, PASS/FAIL), as in quickstart.md §2. Print it on success with `--nocapture`, and always on failure. Unit-test the formatting.
- [ ] T047 [US3] Add the minimum-phase reconstruction to `crates/rr_dr60_harness/src/analysis.rs`: FFT (`rustfft`, N = 65536) of an impulse response, a cepstral minimum-phase phase computed from log|H|, and `phase_deviation_deg(measured, minphase, f_lo, f_hi)`. Unit-test it: a known minimum-phase biquad cascade gives ≤ 0.5° deviation, and a known linear-phase FIR gives > 5°.
- [ ] T048 [US3] Implement the check functions in `crates/rr_dr60_harness/src/checks.rs`. Each one returns a `Vec<MeasurementResult>` with the requirement and trace filled in:
  - `check_fr005_boundary(rate)`: on `bypass_all`, flatness ±0.1 dB over 50–3600 Hz, ≥ 60 dB rejection from 4000 Hz up to Nyquist, alias and image products ≥ 60 dB. All values are labeled "engineering target". Checks at or above host Nyquist are skipped; at 8 kHz that means 4000 Hz and above.
  - `check_fr010_stage(rate, stage)`: via `R_cfg − R_base`. The 1 kHz gain is absolute; −3 dB points, ripple, ≤ 60 Hz, DC (≥ 40 dB), 4000 Hz (skipped at 8 kHz), ≥ 4600 Hz total power, group delay at 1 kHz ≤ 2 ms, and group delay at 400 and 3200 Hz greater than at 1 kHz. Trace: A-002, A-014, A-015, A-016.
  - `check_fr010_minphase()`: at the 8 kHz host rate only, per the spec. Impulse response of `record_only` and `playback_only`, phase within ±5° of minimum phase over 400–3200 Hz (A-016; ±5° is an engineering target).
  - `check_fr011_cascade(rate)`: at every test frequency from 100 to 3900 Hz (log grid, ≥ 40 points), `R_default` equals `R_rec + R_play − R_base` within ±0.3 dB wherever the value is above −40 dB; the 1 kHz gain is 0 ± 0.2 dB.
  - `check_fr012_latency(rate, config)`: reported latency equals the measured 1 kHz group delay within ±1 sample.
  - `check_fr013_latency(rate)`: default latency ≤ 20 ms.
- [ ] T049 [US3] Write `crates/rr_dr60_harness/tests/response_matrix.rs`: run `check_fr005_boundary`, `check_fr010_stage` (both stages), `check_fr010_minphase` and `check_fr011_cascade` over all 6 rates, call `report::print`, then `assert_all` (FR-019, FR-020, SC-002).
- [ ] T050 [P] [US3] Write `crates/rr_dr60_harness/tests/latency.rs`: `check_fr012_latency` for 6 rates × 5 configurations, and `check_fr013_latency` for 6 rates (SC-005).
- [ ] T051 [P] [US3] Write `crates/rr_dr60_harness/tests/edge_cases.rs`, one test per spec edge case:
  - A block of size 0 is a no-op: the state is unchanged, so the following output equals the reference.
  - One block of 30 s at 48 kHz is bit-identical to 4096-sample blocks. The 10-minute single-block case lives in `timing.rs` (T058, release mode).
  - NaN and ±Inf inputs are treated as 0.0: the output is all finite, contains no subnormals, and needs no reset.
  - An input of ±4.0 is not clipped: the output peak is above 1.0 at 1 kHz, about 4.0 × gain.
  - 0 dBFS DC in the default configuration gives steady state ≤ −60 dBFS.
  - Tail: after a 0 dBFS sweep stops, every output sample more than 0.5 s later is below −120 dBFS, and eventually exactly 0.0.
  - `reset()` followed by X equals a new pipeline followed by X.
  - Unsupported-rate errors name the supported rates.
- [ ] T052 [P] [US3] Write `crates/rr_dr60_harness/tests/determinism.rs`:
  - One-block output equals the partitioned output, bit for bit (FR-014, SC-003), with partitions from `Pcg32` seed 0x0D60, block sizes 0–8192, always including at least one block of size 0 and one of size 1:
    - **100** random partitions for each of the 4 golden stimuli in `default` at 48 kHz, which is what SC-003 requires.
    - **10** random partitions for every other configuration × rate combination.
  - `process_in_place` equals `process`.
  - `seed = 0` vs `seed = u64::MAX` gives identical output (FR-009).
- [ ] T053 [P] [US3] Write `crates/rr_dr60_harness/tests/alloc_free.rs`. Install a counting `#[global_allocator]` (wrapping `std::alloc::System`, with atomic alloc/dealloc/realloc counters). For each of 6 rates × 5 configurations: create the pipeline, reset the counters, then process ≥ 1000 blocks of seeded random size 0–1024 with `process`, `process_in_place` and `reset`, plus the same through `rr_dr60_process`. Assert all counters are 0 (FR-015, FR-022, SC-004). Pre-allocate the input and output buffers before resetting the counters.
- [ ] T054 [P] [US3] Write `crates/rr_dr60_harness/tests/ffi_parity.rs`:
  - Parity of `rr_dr60_process` with `Pipeline::process` for every configuration × rate on the sweep (FR-023).
  - Every row of the contracts/c-api.md error table, asserting the exact status and that `*out` is untouched (FR-024).
  - `rr_dr60_version_string()` equals `rr_dr60::VERSION`.
  - With feature `ffi-test-panic`: forcing a panic gives INTERNAL_ERROR, and then process and latency also give INTERNAL_ERROR (poisoned). `rr_dr60_reset` restores normal processing, and so does `rr_dr60_reconfigure` in a separate case (FR-024, recoverable).
- [ ] T055 [US3] Implement `crates/rr_dr60_harness/src/golden.rs` per [contracts/golden-format.md](contracts/golden-format.md):
  - The `GoldenFile` and `GoldenEntry` serde types (`format = "rr_dr60-golden"`, `version = 1`, `library_version`, entries sorted by (stimulus, config, host_rate_hz)).
  - `sha256` over each `f32::to_bits().to_le_bytes()`.
  - `head` holds 16 lowercase 8-digit hex values.
  - `rms_dbfs` rounded to 0.01.
  - The 4 stimuli exactly as in the table (`impulse` 0.25 s, `tone_1k_m20` 0.5 s, `sweep_log` 1.0 s 20 Hz → 0.45·fs at amplitude 0.25, `noise_seed_0d60` 0.5 s).
  - `compare()`, which reports the key, the first differing head index and the RMS values old → new, and also fails on missing or extra entries.
  - `bless()`, which writes the file when `RR_DR60_BLESS=1`.
  - Unit-test the hash and head formatting on a 4-sample vector.
- [ ] T056 [US3] Write `crates/rr_dr60_harness/tests/golden.rs`. Generate all 96 entries (4 stimuli × 4 configs × 6 rates), processed in one block. If `RR_DR60_BLESS=1`, bless; otherwise compare against `crates/rr_dr60_harness/golden/golden-v1.json` (FR-021). Run `RR_DR60_BLESS=1 cargo test -p rr_dr60_harness --test golden` once and commit `golden-v1.json`. Check it is under 100 KB.
- [ ] T057 [US3] SC-008 mutation test in `crates/rr_dr60_harness/tests/golden.rs` (or a separate file `crates/rr_dr60_harness/tests/mutation.rs`). Using `rr_dr60` feature `__test-hooks` and `VoiceBandStage::with_sos`, build a pipeline whose stage coefficients are shifted by 100 Hz. Embed a second coefficient set as a test fixture `crates/rr_dr60_harness/tests/fixtures/shifted_100hz_sos.rs`, generated with `design_voiceband.py --shift-hz 100 --emit-fixture` (add that flag to the tool). Expose the hook through a `#[doc(hidden)] Pipeline::__with_stage_sos` under the same feature. Assert that `check_fr010_stage` reports at least one FAIL (an upper −3 dB point outside 3400 ± 50 Hz) and that the golden comparison for `default`/`sweep_log`/48000 fails.
- [ ] T058 [P] [US3] Write `crates/rr_dr60_harness/tests/timing.rs` (`#[ignore]`, release mode only; skip with a message if `cfg!(debug_assertions)`):
  - The median ns/sample over 9 runs at block sizes {1, 64, 4096, 2^20}, plus after a 60 s stream at 48 kHz, has max/min ≤ 3.0 (bounded work, FR-015, FR-022).
  - 60 s of 48 kHz audio processes at ≥ 20× real time (SC-006, engineering target).
  - A single 10-minute block at 48 kHz is bit-identical to 4096-sample blocks (spec Edge Cases, very large blocks).
- [ ] T059 [US3] Add a per-sample operation-count test in `crates/rr_dr60/src/pipeline.rs` (`#[cfg(all(test, feature = "op-count"))]`). An `op-count`-gated counter increments once per multiply-add in the biquad and polyphase inner loops. Assert that for each rate the per-sample count is ≤ `taps_down/M + taps_up/L + 6·5` + a constant, independent of block size (FR-015 bounded work).
- [ ] T060 [US3] Add CI steps to `.github/workflows/ci.yml` in the ubuntu job:
  - A header drift check: install pinned cbindgen, regenerate, then `git diff --exit-code crates/rr_dr60_ffi/include/rr_dr60.h`.
  - `cargo test -p rr_dr60_harness --all-features` (including `ffi-test-panic`).
  - `cargo test -p rr_dr60_harness --release --test timing -- --ignored`.
  - `cargo test -p rr_dr60_harness --test golden` with **default features only**, to prove the shipped configuration (without `op-count` or `__test-hooks`) matches the golden file.

  Confirm the coverage job still passes at ≥ 80%.

**Checkpoint (US3)**: `cargo test --all-features` passes everything, including the golden and mutation tests, and the report prints with FR and trace IDs. All three stories have now been validated independently.

---

## Phase 6: Cross-Platform CI Matrix (separate phase, plan.md; FR-014, SC-003, R-16)

**Purpose**: Prove one golden file is bit-identical on all 8 targets. This phase is build infrastructure: it must not block the DSP stories, but the feature is not done until it passes.

- [ ] T061 Expand `.github/workflows/ci.yml` into a `golden-matrix` job running `cargo test -p rr_dr60_harness --all-features` (which includes golden and determinism) on:
  - ubuntu-latest (x86_64-unknown-linux-gnu)
  - ubuntu-24.04-arm (aarch64-unknown-linux-gnu)
  - macos-latest (aarch64-apple-darwin)
  - macos-latest with target x86_64-apple-darwin under Rosetta 2. If an Intel runner such as `macos-15-intel` is available, prefer it; document the fallback in a comment.
  - windows-latest (x86_64-pc-windows-msvc)
  - windows-11-arm (aarch64-pc-windows-msvc). If that runner is unavailable, treat Windows ARM64 as having **no automated runner**: leave it out of the per-PR matrix, because a `continue-on-error` job does not satisfy FR-014, and move it to the release-time manual gate in T066 alongside the iOS device.
- [ ] T062 [P] Create `scripts/ios-sim-runner.sh` (executable). It boots or reuses an iOS simulator (`xcrun simctl list` → pick the latest iPhone runtime; `xcrun simctl boot`, tolerating already-booted) and runs the test binary via `xcrun simctl spawn booted "$@"`, passing through the exit code. Set `GOLDEN_DIR`-style paths so the test finds `golden-v1.json`: the harness reads the path from `env!("CARGO_MANIFEST_DIR")` at compile time, and the simulator shares the host filesystem. Add `.cargo/config.toml` with `[target.aarch64-apple-ios-sim] runner = "scripts/ios-sim-runner.sh"`.
- [ ] T063 Add an `ios` job on macos-latest to `.github/workflows/ci.yml`:
  - `cargo test -p rr_dr60_harness --target aarch64-apple-ios-sim --test golden --test determinism` through the runner.
  - `cargo build -p rr_dr60_ffi --release --target aarch64-apple-ios` and `--target x86_64-apple-ios` (build only).
- [ ] T064 [P] Create `scripts/ios-device-golden.sh`. It installs or checks `cargo-dinghy`, requires a connected device, runs `cargo dinghy -d <device> test -p rr_dr60_harness --test golden --test determinism`, and prints PASS or FAIL plus the device model and iOS version for the release record.
- [ ] T065 [P] Add `msrv` and `timing` jobs to `.github/workflows/ci.yml`:
  - `msrv` on ubuntu-latest: toolchain 1.85, `cargo test --workspace --all-features`.
  - `timing` on ubuntu-latest: release mode, `--ignored`.

  Pin the cbindgen version in a workflow-level env var.
- [ ] T066 Create `docs/release-checklist.md`. It covers:
  - The CI matrix is green on all 7 automated targets.
  - `scripts/ios-device-golden.sh` passes on a physical device, with the device and OS recorded (FR-014, SC-003, iOS gate "before each release").
  - If Windows ARM64 has no automated runner (T061), run the golden and determinism tests on a Windows ARM64 machine and record the result.
  - The golden file is unchanged since the last bless, or the CHANGELOG explains the change.
  - The assumption register is up to date.
  - Coverage is ≥ 80%.
  - SemVer is bumped for both the Rust and C surfaces.

**Checkpoint**: The matrix is green on every automated target and the iOS device procedure is documented.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [ ] T067 [P] Update `README.md` with a "Using the library" section: Rust and C snippets from quickstart.md §4–5, supported rates, latency, real-time notes, and the neutral accuracy statement ("modeled on… assumed… not yet measured against a real unit", Constitution VI).
- [ ] T068 [P] Update `CHANGELOG.md` `[Unreleased]`:
  - "Added: pipeline skeleton — host-rate I/O through the 8 kHz device domain with record (stage 4) and playback (stage 10) voice-band stages, bypass/tap, latency reporting, C API, measurement harness and golden files"
  - "Added assumptions A-014, A-015, A-016"
  - "Golden files: initial bless (golden-v1)"
- [ ] T069 [P] Update `CLAUDE.md`:
  - Current status: the Cargo workspace exists and basic CI is on.
  - Open decisions resolved: `no_std` = yes with `alloc` (R-02); API shape = fixed pipeline with settings (R-11); MSRV = 1.85 (R-01). iOS distribution is still open.
  - Toolchain: add the `~/.cargo/bin` PATH note, cbindgen and cargo-llvm-cov.
- [ ] T070 [P] Update `.github/pull_request_template.md` with checkboxes for: golden files re-blessed with a CHANGELOG entry explaining the output change; C header regenerated; assumption IDs cited for any new value.
- [ ] T071 [P] Update `docs/hardware/signal-chain.md` rows 4 and 10 so they link to the implemented model ("Modeled in 001: G.712-like min-phase band-pass, A-014/A-015/A-016") and note that DAC imaging residue is not modeled yet.
- [ ] T072 Time a walkthrough of the quickstart (§4 and §5) from a fresh clone and record the result in the PR (SC-001: under 15 minutes). Run the full [quickstart.md](quickstart.md) validation on macOS (§1–§3, §5, §6 simulator, §7 timing) and fix any drift between the docs and the commands. Run `cargo doc --workspace --no-deps` with `RUSTDOCFLAGS="-D warnings"`.
- [ ] T072a Traceability audit (FR-018, SC-007). Add `scripts/check-traceability.sh`, which:
  - Greps `crates/rr_dr60/src/**/*.rs` and `crates/rr_dr60_harness/src/checks.rs` for numeric `const` items and literal tolerances.
  - Fails if any of them has no `A-\d{3}`, `S-\d{3}`, `FR-\d{3}`, `SC-\d{3}` or `engineering target` comment within 2 lines.
  - Exempts `voiceband_coeffs.rs`, because its header cites the IDs.

  Add it as a step in the ubuntu CI job. Fix every hit.
- [ ] T073 Final gate: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features` and `cargo llvm-cov --all-features --workspace --fail-under-lines 80` all pass. Then open the follow-up PR from `001-pipeline-skeleton` to `main` (US2, US3, the CI matrix and polish), titled `feat: 001 pipeline skeleton — US2, US3, CI matrix`. Extend the US1 CHANGELOG entry rather than duplicating it.

---

## Dependencies & Execution Order

### Phase dependencies

- **Phase 1 (Setup)**: no dependencies. T009 commits only a placeholder coefficient file; the real coefficients land in T029a, after T023 has been seen failing (Constitution III).
- **Phase 2 (Foundational)**: depends on Phase 1 and blocks every story. The tests T012–T016 come before the implementations T017–T022.
- **Phase 3 (US1)**: depends on Phase 2.
- **Phase 4 (US2)**: depends on Phase 2 and on the US1 engine (T035) and FFI core (T036), because bypass and tap extend the same per-sample loop. Its tests are still independent: they compare against the bypass-all baseline and analytic stage responses, not against US1's tests.
- **Phase 5 (US3)**: depends on Phase 2, US1 and US2. The harness checks every configuration, and golden entries include record_only, playback_only and bypass_all.
- **Phase 6 (CI matrix)**: depends on US3's golden and determinism tests (T052, T056). It can run alongside Phase 7.
- **Phase 7 (Polish)**: depends on all stories. T072 and T073 come last.

### Within each story

1. Tests are written first and must fail. The US1 analytic test T023 fails against the T009 placeholder until T029a; T027 and T028 fail to compile or fail until T035 and T036.
2. Primitives (biquad, design, rate) come before converters, which come before the pipeline, which comes before the FFI.
3. The checkpoint must pass before the next story starts.

### Task-level dependencies

| Task | Depends on |
|---|---|
| T009 | T008 |
| T017 | T012 |
| T018 | T013 |
| T019 | T014 |
| T020 | T015, T017 |
| T021 | T016 |
| T029 | T024, T019 |
| T029a | T008, T009, T023 |
| T030 | T023, T029, T029a |
| T031 | T017 |
| T032 | T018 |
| T033, T034 | T031, T032, T025, T026 |
| T035 | T027, T030, T033, T034 |
| T036 | T028, T035 |
| T037, T038 | T036 |
| T039 | T036 |
| T039a | T037, T038, T039 |
| T011a | T011 |
| T042 | T040, T035 |
| T043 | T041, T035 |
| T044 | T041, T036, T043 |
| T047 | T021 |
| T048 | T045, T047, T042 |
| T049, T050 | T048 |
| T055 | T020 |
| T056 | T055, T042 |
| T057 | T048, T056, T030 |
| T060 | T037, T049, T056 |
| T061, T063 | T056, T052 |
| T066 | T064 |
| T072a | T048, T035 |

---

## Parallel Examples

### Phase 1

```text
T002 rust-toolchain.toml   |  T003 clippy.toml   |  T004 detmath skeleton
T005 core skeleton         |  T006 ffi skeleton  |  T007 harness skeleton   |  T010 CI workflow
```

### Phase 2 (tests, then implementations)

```text
T012 detmath tests | T013 settings/error tests | T014 sanitize tests | T015 stimulus tests | T016 analysis tests
then: T018 settings/error | T019 sanitize | T021 analysis   (T017 detmath first, then T020 stimulus)
```

### User Story 1

```text
Tests in parallel:  T023 analytic stage | T024 biquad | T025 resampler 48k/44.1k | T026 resampler other rates
Then:               T029 biquad → T029a real coefficients  ||  T031 Kaiser design + T032 RatePlan
Then:               T030 voiceband  ||  T033 down + T034 up
Then:               T035 pipeline → T036 FFI → T037 header || T038 C smoke || T039 coverage
```

### User Story 2

```text
T040 acceptance tests || T041 reconfigure tests → T042 bypass/tap → T043 reconfigure → T044 FFI reconfigure
```

### User Story 3

```text
T045 configs || T046 report || T047 min-phase analysis || T050 latency tests || T051 edge cases
|| T052 determinism || T053 alloc-free || T054 FFI parity || T058 timing
then T048 checks → T049 response matrix;  T055 golden lib → T056 golden test + bless → T057 mutation
```

### Phase 6

```text
T062 sim runner || T064 device script || T065 msrv/timing jobs  → T061/T063 matrix and iOS jobs → T066 release checklist
```

---

## Implementation Strategy

### MVP first (User Story 1 only)

1. Phase 1, Setup: the workspace, the coefficient file, and CI with the coverage gate.
2. Phase 2, Foundational: detmath, settings, sanitize, and harness stimulus and analysis.
3. Phase 3, US1: resolve the 80/441 resampler risk early (T025 → T033/T034 at 44.1 kHz first).
4. **Stop and validate**: `cargo test -p rr_dr60_harness --test us1_voiceband`, plus `smoke: OK`. Demo it to an app developer (SC-001).

### Incremental delivery

There are two PRs from the same branch, `001-pipeline-skeleton`. PR 1 is the draft opened in T011a and merged at T039a (Setup, Foundational and US1). PR 2 is opened in T073 (US2, US3, Phase 6 and Phase 7).


1. US1: telephone-band audio from Rust and C (MVP).
2. US2: bypass, tap, per-configuration latency and reconfigure.
3. US3: the full measured proof, golden files, determinism and real-time safety.
4. Phase 6: bit-identity proven on 8 targets.
5. Phase 7: docs, changelog and PR.

### Risk notes

- **44.1 and 88.2 kHz polyphase (80/441, 40/441)** is the largest technical risk, so it's tested early in T025 and T026. If the per-phase delay alignment proves awkward, the fallback is to choose N so that (N−1)/2 is a multiple of L·M/gcd. That is documented in R-08 and changes only `rate.rs`.
- **detmath accuracy** affects the resampler stopband. T012's ulp bounds plus the T025 design-level stopband test catch any shortfall before the pipeline is integrated.
- **CI runner availability** (Windows ARM64, Intel macOS) is isolated in Phase 6 with documented fallbacks.

---

## Notes

- [P] means a different file with no dependency on an incomplete task.
- Each story is independently testable at its checkpoint (Constitution VII).
- Verify that tests fail before implementing. Commit after each task or logical group, using Conventional Commits.
- Never hand-edit `voiceband_coeffs.rs`, `rr_dr60.h` or `golden-v1.json`. Regenerate them with their tools; for golden files, add a CHANGELOG entry.
