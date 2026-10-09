# Research: Automatic Gain Control (AGC) on the Record Path

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md) | **Date**: 2026-10-09

Decisions for feature 002. They build on [spec 001's research](../001-pipeline-skeleton/research.md) (cited as 001 R-xx). The numbers marked "prototype" come from a Python float64 simulation of the design below (scipy `remez` detector filter, 8 kHz). The prototype is not committed. The Rust harness re-verifies every number.

## R-01 Where the AGC runs (FR-001, FR-003, FR-010; A-019)

- **Decision**: The AGC runs at the 8 kHz device rate, as the first element of the existing device-rate chain: host → decimator → **AGC** → stage 4 → stage 10 → interpolator → host. `Tap::AfterAgc` runs only the AGC, then goes straight to the interpolator. Bypass skips the AGC with no arithmetic, as for stages 4 and 10 (001 FR-007).
- **Rationale**:
  - A-019 says the AGC responds to the device-band signal (content up to 4 kHz). At the device rate that holds by construction, because the decimator's anti-alias filter (001 R-08) has already removed everything above 4 kHz.
  - The AGC's timing and detector are designed once, for one rate, instead of once per host rate.
  - At the 8 kHz host rate the boundary is the identity, so the harness sees the AGC's gain directly (R-11).
- **Alternatives**: Running at the host rate. Rejected: content above 4 kHz would drive the gain, which contradicts A-019. It would also need six detector designs, and cost up to 12× more at 96 kHz.

## R-02 Level detector (FR-004, FR-008, A-019)

- **Decision**: For each device sample, the detector computes the squared envelope e² from two paths and takes the larger:
  1. **Analytic envelope**: x[n−D]² + h[n]², where h is the output of a 63-tap Hilbert FIR and D = 31 samples (3.875 ms) is its delay. For a steady sine this equals the squared amplitude, with no ripple and no dependence on frequency or phase.
  2. **Peak hold**: the largest x[n−k]² for k = 0 … D. This reacts as soon as a new peak arrives, so a loud onset is not delayed by the Hilbert filter. For a steady sine it never exceeds path 1, so it adds no ripple. It also lets DC and very low frequencies drive the gain, which the Hilbert filter would reject (spec edge cases).

  e² is floored at 10⁻²⁰ (−200 dB). Level in dB is L = (10 / ln 10) · ln(e²), using `detmath::ln`. With the AES17 convention, a sine of amplitude A reads 20·log₁₀ A dBFS, so no crest-factor offset is needed.
- **Rationale**: A-019 calls for a peak-responding detector, and FR-008 requires the level within ±0.5 dB, with THD ≤ 1 %, at 300–3400 Hz for any phase.
  - A plain sample-peak detector misreads the 2000 Hz and 2667 Hz tones by up to 3 dB and 6 dB, because those tones put only 4 and 3 samples in each cycle at 8 kHz. That would fail FR-008.
  - The analytic envelope has no such error. Prototype: the static level is within 0.01 dB of target, and THD is below 0.001 % at 300, 2000, 2667 and 3400 Hz for three phases each.
  - The peak-hold path was added when the prototype measured attack at 11.25 ms without it. With it, attack measures 10.00 ms.
- **Alternatives**:
  - Sample-peak only: fails FR-008, as shown above.
  - True-peak by 8× oversampling: about 0.1 dB error at 3400 Hz, and more work per sample.
  - An RMS detector with a smoothing filter: contradicts A-019, and it either ripples at twice the tone frequency or needs slow smoothing that conflicts with a 1 ms attack.

## R-03 Hilbert filter design (R-02; A-019)

- **Decision**: Equiripple type-III Hilbert FIR, 63 taps, passband 250–3750 Hz at 8 kHz, designed offline with `scipy.signal.remez(type="hilbert")` by a new script, `tools/filter-design/design_hilbert.py`. The script verifies the design and fails if magnitude is outside ±0.01 dB from 300 to 3400 Hz. It writes `crates/rr_dr60/src/stages/agc_hilbert_coeffs.rs`, with each coefficient stored as an exact bit pattern, as 001 R-07 does for the voice-band filter. Only the 32 even-index taps are non-zero (every odd-index tap, including the centre h[31], is zero), so the filter costs 32 multiply-adds per device sample.
- **Sizing** (prototype, worst magnitude error at 300, 500, 1000, 2000, 2667 and 3400 Hz):

  | Taps | Detector delay | Max error |
  |---|---|---|
  | 31 | 1.9 ms | 0.19 dB |
  | 47 | 2.9 ms | 0.03 dB |
  | **63** | **3.9 ms** | **0.006 dB** |
  | 79 | 4.9 ms | 0.001 dB |

  63 taps is the shortest length with a wide margin against FR-008. The delay applies only to the detector, never to the audio (FR-010).
- **Rationale**: Committed bit patterns keep the build independent of Python's floating-point environment (001 R-04). Reusing 001's design-and-verify workflow keeps the coefficients traceable.
- **Alternatives**: Designing the filter at construction with detmath. Rejected: equiripple design needs an iterative exchange algorithm, which is more code for no runtime benefit.

## R-04 Gain computer: the static curve (FR-004, FR-005; A-017)

- **Decision**: Target gain G<sub>t</sub> = clamp(−0.9 · (L − T), −A<sub>max</sub>, +G<sub>max</sub>) dB, where T is the target level. The factor 0.9 = 1 − 1/10 gives the 10:1 slope, since output = T + (L − T)/10 inside the regulated range. The slope is a named constant citing A-017.
- **Rationale**: It is the spec's regulation line exactly, with hard knees at the gain limits. Prototype at defaults: −60 → −20.00, −40 → −13.00, −10 → −10.00, 0 → −9.00, +5 → −8.50 dBFS.
- **Alternatives**: Soft knees. Not specified, harder to test, and the spec already excludes ±3 dB around each knee.

## R-05 Gain smoothing: attack, release, shape, start (FR-006, FR-007, FR-013; A-018, A-019)

- **Decision**: A one-pole smoother on the gain in dB, with two coefficients:
  - G ← G + (G<sub>t</sub> − G) · α<sub>a</sub> when G<sub>t</sub> < G (attack, gain falling), otherwise G ← G + (G<sub>t</sub> − G) · α<sub>r</sub> (release).
  - α = 1 − exp(−1 / (τ · 8000)), computed once at construction with `detmath::exp`.
  - τ<sub>a</sub> = t<sub>attack</sub> / ln 13.5 and τ<sub>r</sub> = (t<sub>release</sub> − D / 8000) / ln 13.5.

  ln 13.5 = ln(27/2) turns the spec's 2/27 settling band into a time constant. The release subtracts the detector delay D, because on a downward step the old peak stays in the detector for D samples. G starts at G<sub>max</sub> on creation, reset and reconfigure. The linear gain applied to the sample is exp(G · ln 10 / 20), via `detmath::exp`, so y = x · g.
- **Rationale**:
  - A single pole in the dB domain *is* the spec's exponential-in-dB shape, for both directions.
  - Silence gives G<sub>t</sub> = G<sub>max</sub>, so the gain rises at the release rate with no hold and no gate (FR-007). Digital silence gives exactly 0, because y = 0 · g.
  - A steady sine gives a constant G<sub>t</sub>, so the gain does not move and adds no distortion.
  - Prototype: attack 10.00 ms and release 1.001 s at defaults. At the extremes, attack 1 ms / release 50 ms measured 1.12 ms / 50.6 ms, attack 100 ms / release 10 s measured 100.12 ms / 10.001 s, and release 3 s measured 3.003 s. The release midpoint (FR-006) reached 47 %, against the 35–65 % window.
- **Alternatives**:
  - Linear-domain RC smoothing: gives a roughly steady dB/s release, which the spec rejects (FR-006).
  - Smoothing the level instead of the gain: equivalent while inside the regulated range, but behaves worse at the gain limits.
  - Applying the gain through a lookup table instead of `exp`: unnecessary at 8000 calls per second.

## R-06 Determinism with transcendental functions on the processing path (FR-013; 001 R-04, R-06)

- **Decision**: The processing path may now call `rr_dr60_detmath::{ln, exp}`. Those functions use only basic IEEE operations and integer bit manipulation, with no loops, so they are bit-identical on every target and have a fixed cost. The clippy `disallowed_methods` ban on the platform's libm (`f64::exp`, `f64::ln`, `powf`, …) stays, and `clippy.toml` needs no change. Each device sample makes one `ln` call and one `exp` call.
  - **Constants**: ln 10 is `core::f64::consts::LN_10`, a compile-time literal. 10 / ln 10 and ln 10 / 20 are computed by plain IEEE division at construction. Settings are converted from f32 to f64, which is exact.
  - **Docs to amend**: 001 R-06 and the `rr_dr60_detmath` crate docs say detmath is used "only at construction". The implementation updates both to say it also runs on the processing path, citing this decision.
- **Rationale**: 001 R-04 restricted the processing path to basic operations so that results don't depend on the platform. detmath already meets that standard; it was limited to construction only because nothing else needed it then. The golden files on all 8 targets prove bit-identity (SC-003).
- **Alternatives**:
  - Lookup tables with interpolation: more code, and their own accuracy questions.
  - Polynomial approximations written into the AGC: they would duplicate detmath.

## R-07 Numerical hygiene (Edge Cases)

- **Decision**:
  - Input is already sanitized: non-finite and subnormal values become 0 (001 R-05).
  - e² has a floor, so `ln` never sees 0.
  - **Gain flush**: after every smoother update, `G = flush_state(G)` (001 R-05, threshold 1e-30). Without it, when the target gain is exactly 0 dB, G decays geometrically toward 0 and becomes subnormal after a few minutes. That happens with max gain = 0 and max attenuation = 0, or when L = T exactly. A host with FTZ/DAZ enabled would then produce different bits.
    - *Correction:* an earlier draft of this plan claimed G could never become subnormal. The plan review found it can.
    - The fix is tested with max attenuation = 0 and a 0 dBFS tone for 10 minutes. The clamp gives G<sub>t</sub> = −0.0, so G decays from max gain toward 0. The test asserts that G flushes to exactly 0.0, that no subnormal appears, and that from then on the output equals the input bit for bit. (With both limits at 0, G starts at 0 and never moves, so that case doesn't exercise the flush.)
  - The Hilbert filter is non-recursive, so it has no decaying tails to flush.
  - The detector's history is a fixed-size array inside the stage. No allocation.
  - **New**: `narrow_out` saturates to ±`f32::MAX` instead of producing ±Inf (`as f32` overflows to Inf). With up to +60 dB of gain, a finite input above about 3.4e35 could otherwise become infinite. This changes output only for such inputs, so the 001 golden files are unaffected.
- **Rationale**: Covers the spec's long-silence, non-finite and overshoot edge cases, and 001's "output never contains NaN or Inf" rule.

## R-08 Rust settings API (FR-011, FR-014; 001 R-11)

- **Decision**:
  - New `#[non_exhaustive] pub struct AgcSettings { enabled: bool, target_dbfs: f32, max_gain_db: f32, max_attenuation_db: f32, attack_ms: f32, release_ms: f32 }`, with `AgcSettings::DEVICE` (a const holding the A-017, A-018 and A-020 defaults) and `Default` returning it.
  - `Settings` gains `pub agc: AgcSettings`. `Tap` gains `AfterAgc`.
  - Validation happens in `Pipeline::new` and `reconfigure`. Ranges are inclusive, and NaN or ±Inf fail. Failure returns the new `Error::InvalidSetting { setting: Setting }`, where `Setting` is a `#[non_exhaustive]` enum whose `Display` gives the field name and its valid range.
  - `Settings` and `AgcSettings` derive `Clone, Copy, Debug, PartialEq`. `Settings` **drops `Eq` and `Hash`**, because floats implement neither.
- **Rationale**: Floats in dB and ms match the spec and are natural in Swift and C. Dropping `Eq`/`Hash` is a breaking change, allowed in a 0.x minor release (0.1 → 0.2). Nothing in the workspace hashes `Settings`. Adding a variant to the `#[non_exhaustive]` `Tap` and `Error` enums is not breaking.
- **Alternatives**: Integer fixed-point units (e.g. hundredths of a dB) to keep `Eq`/`Hash`. Rejected: clumsy for app developers, and the spec gives no resolution. Hand-written `Eq` comparing bit patterns: surprising semantics (−0.0 ≠ 0.0).

## R-09 C API (FR-014; 001 R-12)

- **Decision**:
  - **Settings struct**: `RrDr60Settings` grows at the end with `bool agc_enabled; float agc_target_dbfs; float agc_max_gain_db; float agc_max_attenuation_db; float agc_attack_ms; float agc_release_ms;`. It goes from 24 to 48 bytes. Its derive drops `Eq`, which floats don't implement, and keeps `PartialEq`.
  - **struct_size rule unchanged**: `struct_size` must be ≥ `sizeof(RrDr60Settings)` of this version, as in 0.1. A 24-byte (0.1-layout) struct is rejected with `RR_DR60_STATUS_INVALID_ARGUMENT`.
  - **New values**: tap value `RR_DR60_TAP_AFTER_AGC = 2`, and status `RR_DR60_STATUS_INVALID_SETTING = 5`.
  - **Naming the bad setting**: one new function, `RrDr60Status rr_dr60_settings_validate(const RrDr60Settings *settings, RrDr60SettingField *out_field)`. It returns the status that `rr_dr60_create` would return for these settings, and writes the offending field (`NONE` when valid). `rr_dr60_create` and `rr_dr60_reconfigure` call the same validator, so the status and the named field can never disagree.
    - **Order**: struct size, tap, host rate, then the AGC fields in struct order. This matches the existing `to_settings` → `Pipeline::new` order.
    - **Core side**: the AGC and rate checks come from the core's validator (`Error::InvalidSetting { setting }` / `UnsupportedHostRate`), mapped to `RrDr60SettingField`.
    - **Behavior**: never allocates, and is panic-guarded.
  - **Header**: regenerated with cbindgen, and the version macros go to 0.2.0.
- **Rationale**:
  - The size rule stays because relaxing it to accept older layouts would be unsound as designed. `rr_dr60_create` forms a full-size `&RrDr60Settings`, which is undefined behavior over a 24-byte struct. `rr_dr60_settings_default` also returns the struct by value, so a 0.1 binary would get 48 bytes written into a 24-byte struct anyway.
  - 0.1 was never released (spec Assumptions), so no caller benefits.
  - One validate function backed by the core validator keeps the C surface small and consistent.
- **Alternatives**:
  - Accepting the old sizes and copying `struct_size` bytes: needs raw-pointer reads and a list of allowed sizes, for zero real callers. Rejected (plan review).
  - A separate pure "which field" query: its order could drift from `create`'s. Rejected.
  - A thread-local last-error string: hidden state, and allocates.

## R-10 Keeping spec 001 unchanged (FR-002, FR-017, FR-018)

- **Decision**:
  - The harness's spec-001 configurations get an explicit AGC-bypassed baseline: `configs::settings(name, rate)` sets `agc.enabled = false` for all five 001 configuration names.
  - Every 001 test file (us1, us2, edge cases, timing, determinism, FFI parity, allocation, mutation, golden) uses those settings. Tests that start from `rr_dr60_settings_default` set `agc_enabled = false`.
  - The harness configuration names stay the same. `configs.rs` gets a doc comment saying that `"default"` and the other 001 names now mean "the spec 001 configuration with the AGC bypassed".
  - The tap-mapping helpers in `tests/ffi_parity.rs` and `tests/alloc_free.rs` currently map every tap other than `AfterRecord` to `AfterPlayback`. They become an exhaustive match that includes `AfterAgc`, so the new tap is never silently tested as `AfterPlayback`.
  - No 001 expected value, tolerance or golden hash changes. `golden/golden-v1.json` is not touched. A new harness test, `golden_v1_unchanged`, checks the SHA-256 of the embedded file against a constant recorded in this feature.
  - The AGC has its own named configurations and its own golden file (R-12).
- **Rationale**: This is FR-018 put into practice. Bit-identity of the bypassed path is guaranteed by construction (bypass does no arithmetic, R-01). The unchanged golden file proves it on all 8 targets.

## R-11 Harness: measuring the AGC (FR-004 – FR-012, FR-016)

- **Decision**: A new harness module, `agc_checks`, with one function per requirement, each returning `MeasurementResult`s (001 R-13). Checks cite `002/FR-0xx` and A-IDs.
- **Level convention**: spec levels follow AES17: a sine of amplitude A is 20·log₁₀ A dBFS, so for any signal, level = 20·log₁₀(RMS · √2). Stimuli are generated and outputs measured with this convention. That includes the FR-009 noise: −70 dBFS means RMS = 10^(−70/20) / √2. Using raw RMS dBFS would be off by 3.01 dB and fail the ±2 dB check.
- **Gain trajectory**:
  - **At the 8 kHz host rate**: the exact per-sample gain, y[n] / x[n], wherever |x[n]| ≥ 0.1 × the tone amplitude (engineering target, R-15). The AGC is a pure multiplier and the boundary is the identity, so this gives 0.125 ms resolution.
  - **At other rates**: the **reference ratio**: output ÷ a reference run of the same stimulus with the AGC bypassed (same configuration otherwise), sample by sample, wherever the reference is at least 0.1 × the tone amplitude. Both runs share the rate conversion, so the ratio is the AGC's gain at every host sample and is already latency-aligned. *(Corrected during implementation, T026: the original plan divided an FFT analytic envelope by the input amplitude. A probe showed that envelope reads attack ≈1.6 ms long at every rate, 8 kHz included, because the 1 kHz test tone is too slow a carrier for the 3.7 ms attack time constant. The reference ratio measured 9.88 ms at 8 kHz, identical to the exact y/x, and 10.09–10.12 ms at 16, 48 and 96 kHz.)*
  - **Self-check**: the reference ratio at each host rate must agree with the exact y/x at 8 kHz within 0.25 ms on attack, within 1 % on release, and within 0.1 dB on steady levels (engineering targets). At 8 kHz the two methods are identical by construction. *(Release changed from 0.25 ms during T034: the gain approaches its final value so slowly that 0.25 ms corresponds to about 0.001 dB, finer than any method here can resolve.)* Measured: attack within 0.10–0.19 ms, release within 0.1 %, steady gain within 0.001 dB at every rate.
  - **Timing gate** (T035): timing trajectories keep only samples where the reference is at least 0.5 × the segment's own tone amplitude. When the gain changes within the rate converter's kernel span (a 1 ms attack), output ÷ reference has a small absolute error that becomes a ±2–4 dB spike near the reference's zero crossings. Without this gate the 1 ms attack read 2.04 ms at 48 kHz; with it, 1.27 ms (exact at 8 kHz: 1.25 ms). Samples remain at least every 1/6 cycle (0.17 ms at 1 kHz), finer than FR-006's attack/4.
  - **Release hold** (T034): the final low level is held for 3 × release + 0.5 s (the spec's minimum is 1.5 ×), so the measured "final" value is truly settled. The shorter hold biased release 2–6 % short.
  - **Resolution**: at least min(2 ms, attack / 4) and min(20 ms, release / 20) (FR-006).
- **Settling waits** (runtime): every tone starts from maximum gain, so steady levels settle in the attack direction. Static measurements wait 20 × attack + the detector delay (3.875 ms) + the boundary latency, then measure over a whole number of cycles. For inputs below the regulated range the gain never leaves maximum. The step stimulus for timing is: low level 0.5 s, high level ≥ 20 × attack (minimum 0.2 s), low level ≥ 1.5 × release + 0.5 s. The spec's step definition is amended to match (see spec Overview › Step stimulus).
- **Checks** (each named in the report):
  - **Static curve** (FR-004, FR-005): 1 kHz tones from 10 dB below the regulated range to 10 dB above it, in 5 dB steps. Single-bin DFT level (001 R-13).
  - **Timing and shape** (FR-006, FR-012): settling band = 2/27 of the excursion. Attack and release times are taken from the last sample outside the band. The midpoint is measured at 25 % of the measured release time. Step placement follows FR-012.
  - **US2 examples**: release 3 s (US2 AS3), and target −20 dBFS with a −30 dBFS tone (US2 AS4), as named checks in addition to the min/max matrix.
  - **Frequency and THD+N** (FR-008): tones at 300, 500, 1000, 2000, 2666.67 (= 8000/3, three samples per cycle at the device rate, the spec's "2667 Hz") and 3400 Hz, at −30, −10 and 0 dBFS. 2000 Hz and 8000/3 Hz are each run at 4 starting phases (0, π/4, π/2, 3π/4).
    - Distortion is measured as **THD+N**: a least-squares fit of the known fundamental (sine and cosine terms) over a steady segment, then the power of everything left over, DC included, relative to the fundamental. Limit 1 % (−40 dB), the spec's THD limit applied to THD+N, which is stricter.
    - Plain harmonic bins would miss distortion: at the device rate, harmonics fold. THD+N counts every folded product that lands anywhere other than the fundamental's own frequency, including DC (e.g. the 3rd harmonic of 8000/3 Hz) and Nyquist. A product that folds exactly onto the fundamental (e.g. the 3rd harmonic of 2000 Hz) cannot be separated from it by any measurement; it shows up as a level change, which FR-008's ±0.5 dB level check catches. (Corrected during implementation, T009.)
    - The boundary's alias floor (≥ 60 dB down, 001 FR-005) is well below the limit.
  - **Noise rise** (FR-009): seeded PCG32 noise (001 R-13), band-limited to 300–3400 Hz by a Kaiser-windowed sinc band-pass FIR built with detmath in the harness, then scaled to −70 dBFS by the AES17 convention, plus the 1 kHz bursts. The stimulus never uses the emulator's filters (Principle VII). "First 50 ms after each burst ends" starts after the boundary latency. The "last 1 s of each pause" window ends 10 ms before the next burst (T034): the rate conversion is linear-phase, so the next burst's onset, overshooting by up to the maximum gain, appears a few ms before its nominal latency, and it read the first pause 1.2 dB high.
  - **Silence** (FR-007, US1 AS4): digital silence in gives exactly 0.0 out, and the gain reaches maximum after silence.
  - **Start, reset, reconfigure** (FR-013, Edge Cases):
    - At 8 kHz, the first non-zero sample's gain y/x equals `max_gain_db` within 0.01 dB.
    - After processing a loud tone, `reset()` and `reconfigure(same settings)` each produce output bit-identical to a freshly created pipeline on the same stimulus.
  - **Gain range of zero** (Edge Cases): with max gain = max attenuation = 0, output equals input within ±0.01 dB. In practice it is bit-exact, since the gain is exactly 0 dB and exp(0) = 1. The flush test (R-07) is a separate case.
  - **DC and low frequencies** (Edge Cases):
    - A DC input of amplitude a reads like a sine of peak a: the steady output is a · 10^(G<sub>t</sub>(20·log₁₀ a)/20), within ±1 dB (engineering target), checked at a = 0.1.
    - A 50 Hz tone at −20 dBFS lowers the mean gain by at least 20 dB compared with silence (it drives the AGC; engineering target).
  - **Content above 4 kHz** (Edge Cases): at 48 kHz, a 1 kHz tone at −40 dBFS alone vs. the same tone plus 6 kHz at −10 dBFS. The steady 1 kHz output level must agree within 0.1 dB (engineering target), measured over the last 0.25 s of 6 s. *(T036: switching the 6 kHz tone on abruptly is a click with in-band energy, which the AGC, starting at +40 dB, reacts to; measured after 1 s the gain had not yet recovered (−0.56 dB). The 6 kHz tone itself leaks through the input rate converter at about −104 dBFS. Measured after 6 s: −0.004 dB.)*
  - **Non-finite input** (Edge Cases): NaN, +Inf and −Inf injected mid-stream give output bit-identical to the same stream with 0.0 in their place. No reset is needed.
  - **Overshoot** (Edge Cases): the start-at-maximum peak (+40 dB over the steady level, about +30 dBFS, for a −10 dBFS input at defaults) is passed unclipped and stays finite.
  - **Tap and bypass** (FR-002, FR-003, US2 AS1, AS2): `Tap::AfterAgc` with stages 4 and 10 **on** gives output bit-identical to `agc_only` (stages off). AGC bypassed reproduces 001's golden file (R-10).
- **Settings matrix and rates** (FR-012, SC-002):
  - The defaults, the US2 examples and the minimum and maximum of each setting, with the others at default.
  - In the normal test run (debug), the matrix runs at 8 kHz and 48 kHz, plus the defaults at all six rates.
  - The full matrix at all six rates runs in the existing release-mode `--ignored` CI job. That covers the long release cases: 10 s release is about 16 s of audio per rate.
  - The tasks phase estimates the debug runtime and keeps it under 001's suite budget.
- **Rationale**: The exact sample ratio at 8 kHz makes the 1 ms attack check reliable. The self-check proves the reference-ratio method used at the other rates. THD+N catches distortion wherever it folds.

## R-12 AGC golden file (FR-017, SC-003, SC-008)

- **Decision**: A new file, `crates/rr_dr60_harness/golden/golden-agc-v1.json`, in the same format as 001 R-14. It holds 3 stimuli × 2 configurations × 6 rates = 36 entries.
  - **Stimuli**:
    - `agc_step`: 1 kHz at −40 dBFS for 3 s, −10 dBFS for 1 s, −40 dBFS for 4 s.
    - `agc_noise_burst`: the FR-009 stimulus, 2 cycles (10 s).
    - `agc_start_m10`: 1 kHz at −10 dBFS from a fresh pipeline (0.5 s), which captures the start-at-maximum overshoot.
  - **Configurations**: `agc_only` (tap after AGC) and `default_agc` (AGC on, both stages on).
  - **Blessing**: the bless command takes a file selector. Blessing `golden-v1.json` is disallowed in this feature (FR-017).
- **CI**: `golden_agc` and `golden_v1_unchanged` are added to the `check`, `golden-matrix` and `ios` jobs in `.github/workflows/ci.yml`, and to `scripts/ios-device-golden.sh`, so SC-003's eight targets are covered.
- **Rationale**: Hash-only entries keep the file to about 15 KB. `agc_step` and `agc_noise_burst` change bits when the release time or target changes. SC-008's mutation test needs no hidden hook: the harness's `Make` closure builds a pipeline with altered settings (`release_ms × 1.5`, or `target_dbfs + 3`), and the test checks that both the golden check and the tolerance check fail.

## R-13 Performance and bounded work (SC-004, SC-006; 001 FR-015)

- **Decision**: Per device sample the AGC adds:
  - 32 multiply-adds (Hilbert),
  - 32 compares (peak hold),
  - one `ln`, one `exp`,
  - about 10 other operations.

  That is roughly 150 operations at 8 kHz, about 1.2 M operations per second, compared with about 10 M multiply-adds per second for 001's boundary at 48 kHz. The `op-count` test bound grows by a fixed AGC term. The allocation test (001 R-15) is extended to AGC-on configurations, including extreme settings.
- **Rationale**: SC-006 (≥ 20× real time) keeps a wide margin. Every operation count is fixed regardless of input, so the per-sample bound still holds.

## R-14 Versioning, CHANGELOG and docs (FR-017, Edge Cases; Principle VI)

- **Decision**:
  - Workspace version 0.1.0 → **0.2.0**, and the C macros likewise.
  - CHANGELOG `[Unreleased]` entries:
    - **Added**: the AGC, `Tap::AfterAgc`, the C settings fields and the C query function.
    - **Changed**: default output now includes the AGC; `Settings` no longer `Eq`/`Hash`; C `struct_size` rule relaxed; `narrow_out` saturates.
    - **Assumptions**: A-017 to A-020.
  - **Docs**:
    - rustdoc and README describe the AGC as "modeled on an assumed AGC (A-017–A-020)".
    - They warn about the overshoot (+40 dB at defaults, +60 dB at most) and advise hosts to limit or clip before converting to integer formats.
    - They note that inputs above 0 dBFS are valid.
    - `docs/hardware/signal-chain.md` row 3 links spec 002.
    - **SC-001**: the README gets an "AGC" section with short Rust and C snippets that bypass the AGC and change the release time. The Rust snippet is a doc-test on `AgcSettings`, so it is compiled and run.
  - No EVP claims either way.

## R-15 Traceability of detector and measurement constants (Principle II; spec FR-015)

- **Decision**: These values are **emulator engineering targets**, not device properties. They are labeled that way in code comments and test reports (`// engineering target (002 R-15)`):
  - Hilbert FIR length 63, design band 250–3750 Hz, and the ±0.01 dB design check (R-03).
  - Peak-hold length 32 samples (= the Hilbert delay + 1, R-02).
  - Detector floor e² ≥ 1e-20 (R-02).
  - Release delay compensation, −D / 8000 s (R-05). This is derived from the detector design, not a free parameter.
  - Gain flush threshold 1e-30, reused from 001 R-05 (R-07).
  - Harness: the 0.1 × amplitude gating for y/x and the 0.5 × gating for timing; the self-check tolerances (0.25 ms, 1 %, 0.1 dB); the 20 × attack settling wait; the 3 × release final hold; the FR-009 window ending 10 ms early; the ±1 dB DC check; the 20 dB low-frequency check; the 0.1 dB check for content above 4 kHz and its 6 s duration (R-11).

  The device-level behavior these serve, a peak-responding detector acting on the device band, stays traceable to A-019. No new assumption is needed.
- **Rationale**: Every number in code traces to an A-ID, an S-ID, or a labeled engineering target (SC-007).
