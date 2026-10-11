# Research: 6 kHz Device Rate and Codec Filter Templates

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md) | **Date**: 2026-10-11

Decisions for feature 004. They revise [spec 001's research](../001-pipeline-skeleton/research.md) R-07, R-08 and R-10 and restate figures from [spec 002](../002-agc/research.md) and [spec 003](../003-vas/research.md). Numbers marked "prototype" come from scipy probes run during planning (not committed); the design script and the Rust harness re-verify every one. The hardware-emulation review of 2026-10-11 (recorded in the spec's Clarifications) supplied the topology findings that R-04 and R-05 confirm.

## R-01 Device rate (FR-001; A-026)

- **Decision**: `DEVICE_RATE_HZ = 6000`. Every stage that runs at the device rate (stages 4, 10, the AGC, the VAS) reads it from that constant, as today. Nothing else in the core needs a value change for the rate itself:
  - `agc.rs` derives the release compensation from `DEVICE_RATE_HZ` (the detector's 31-sample delay becomes 5.17 ms, was 3.875 ms); the attack path's peak hold is in samples and keeps its 10 ms law through `attack_ms` (002 R-02).
  - `vas.rs` converts `hang_ms` and `onset_ms` with `DEVICE_RATE_HZ` (6000 and 120 samples at the defaults). `W = 32` stays (R-10).
  - `pipeline.rs` computes the latency as `2·D + τ_stages · host / DEVICE_RATE_HZ` (001 R-10), unchanged in form.
- **Sweep**: about 100 hard-coded `8000` / `8_000` / "8 kHz" occurrences remain in tests, harness, FFI docs and prose (plan.md › Source Code). Each is either the identity-rate assumption (→ 6000), a device-sample count (→ scaled: 8000 → 6000, 160 → 120, 4000 → 3000), a frequency above the new Nyquist (→ restated per R-07/R-09/R-10), or prose. The tasks list enumerates them by file; `grep -rn '8000\|8_000\|8 kHz' crates tools docs README.md` must end with only historical mentions (CHANGELOG, specs 001–003, the register's superseded rows).
- **Rationale**: A-026 is verified (S-002 specification table; sync waveform T ≈ 170 µs). Nothing above 3 kHz exists in the device.
- **Alternatives**: Making the device rate a setting (rejected: it is a property of the device, 001 FR-004; and every stage's constants would need a second validation path).

## R-02 Host rates and the identity rate (FR-002; SC-005)

- **Decision**: `SUPPORTED_HOST_RATES: [u32; 7] = [6000, 8000, 16000, 44100, 48000, 88200, 96000]`. `RatePlan::for_host(6000)` is `Identity`. The others convert through a prototype at `host · l`, with `6000 / host = l / m` in lowest terms:

  | Host | l / m | Prototype rate | Integer ratio? |
  |---|---|---|---|
  | 8 000 | 3 / 4 | 24 000 | no |
  | 16 000 | 3 / 8 | 48 000 | no |
  | 44 100 | 20 / 147 | 882 000 | no |
  | 48 000 | 1 / 8 | 48 000 | yes |
  | 88 200 | 10 / 147 | 882 000 | no |
  | 96 000 | 1 / 16 | 96 000 | yes |

  `rate.rs` is table-driven; `up.rs` and `down.rs` are generic in (l, m) and need no change beyond their `RATES` test constant and doc comments. The `up.rs` emission-schedule test (003 R-05) must be re-run at the new table, and 003 R-07's note that 44.1/88.2 kHz can lead by an extra device sample now applies to 8 and 16 kHz too (spec FR-012: lead bound ⌈host / 6000⌉, i.e. 2 at 8 kHz, 3 at 16 kHz, 8 at 44.1 and 48 kHz, 15 at 88.2 kHz, 16 at 96 kHz).
- **Consequences for earlier specs**: everything that said "exact at 8 kHz" moves to 6 kHz (001 FR-002/FR-010 phase check; 002 R-11 exact y/x gain; 003 FR-004/FR-005/FR-010). Lengths are exact at 6, 48 and 96 kHz and within ±1 host sample at the four non-integer rates (spec FR-012). `vas_checks.rs`'s `integer_ratio = rate % 8000 == 0` becomes `rate % 6000 == 0`.
- **Rationale**: the identity rate is where a stage's response, the AGC's sample-by-sample gain and every VAS decision are observed directly; without it, three specs' exactness clauses have no home (user decision, Clarifications). 8 and 16 kHz stay for compatibility (clarify Q1).
- **Alternatives**: a test-hooks-only device-rate path (rejected, Complexity Tracking); dropping 8/16 kHz (rejected by the user).

## R-03 Rate conversion boundary (FR-003, FR-004; 001 R-08 revised)

- **Decision**: same Kaiser polyphase design as 001 R-08 with three constants changed: `CUTOFF_HZ = 2775` (middle of the transition), `TRANSITION_HZ = 450` (2550 → 3000 Hz), `STOPBAND_DB = 70` (unchanged: 10 dB margin over FR-003's 60 dB). Transition choice per the user's decision (Clarifications).
- **Prototype sizes** (Kaiser estimate as `rate.rs` computes it, rounded so each converter's delay is a whole number of host samples):

  | Host | Taps | Delay per converter | Boundary total | MAC per device sample (down, N/l) | MAC per host sample (up, N/m) |
  |---|---|---|---|---|---|
  | 8 000 | 235 | 39 samples = 4.88 ms | 9.75 ms | ≈ 78 | ≈ 58 |
  | 16 000 | 463 | 77 = 4.81 ms | 9.62 ms | ≈ 154 | ≈ 57 |
  | 44 100 | 8 481 | 212 = 4.81 ms | 9.61 ms | ≈ 424 | ≈ 57 |
  | 48 000 | 463 | 231 = 4.81 ms | 9.62 ms | ≈ 463 | ≈ 57 |
  | 88 200 | 8 481 | 424 = 4.81 ms | 9.61 ms | ≈ 848 | ≈ 57 |
  | 96 000 | 923 | 461 = 4.80 ms | 9.60 ms | ≈ 923 | ≈ 57 |

  Compared with today: the 44.1 and 88.2 kHz prototypes shrink from about 38 000 taps to about 8 500 (prototype rate 882 kHz instead of 3.528 MHz), so construction time and memory fall by about 4.5×; the per-converter delay falls from about 5.4 to about 4.8 ms.
- **Default latency** (FR-004): 2 × 4.81 ms + (τ_rec + τ_play) at 1 kHz ≈ 9.62 + 0.11 + 0.07 ≈ 9.8 ms. Prototype values in host samples: about **1 at 6 kHz** (identity: only the stages' ≈ 1.0 device sample of group delay), **78 at 8 kHz**, **157 at 16 kHz**, **433 at 44.1 kHz**, **471 at 48 kHz** (was 541), **866 at 88.2 kHz**, **942 at 96 kHz**. The harness measures the final values and the CHANGELOG records them.
- **0.3 baseline (measured 2026-10-11 at commit 75734b7, T003; the "old" column of the CHANGELOG table)**: default latency 8 kHz **4** samples (0.50 ms; identity), 16 kHz **181** (11.31 ms), 44.1 kHz **498** (11.29 ms), 48 kHz **541** (11.27 ms), 88.2 kHz **993** (11.26 ms), 96 kHz **1081** (11.26 ms).
- **Harness restatement** (FR-003): `check_fr005_boundary` → `check_fr003_boundary`: flatness 50–2550 Hz within ±0.1 dB; rejection ≥ 60 dB at 3000 Hz, 3450 Hz, 6000 Hz and 20 kHz where below Nyquist; alias/image products ≤ −60 dB on a 1 kHz tone. Skipped at the 6 kHz host rate except the identity assertion.
- **Rationale**: linear-phase boundary keeps the per-stage minimum-phase checks clean (001 R-08). A 300 Hz transition (2700 → 3000 Hz) would cost about 6.0 ms per converter, 12.1 ms boundary, 14 ms default: legal but eats most of the budget for no template benefit, since the templates end at 2550 Hz.
- **Alternatives**: as 001 R-08.

## R-04 Record stage design (FR-005, FR-006; A-027 transmit, A-016)

- **Decision**: a cascade of a **3rd-order Butterworth high-pass** with its −3 dB point at **112.5 Hz** (0.75 × 150 Hz) and a **5th-order Chebyshev-II (inverse Chebyshev) low-pass** with **32 dB** stopband attenuation (S-005's typical discrimination; the probe table brackets it with 30 and 35 dB) and its stopband edge tuned near **2740 Hz** so the loss at 2550 Hz lands near the typical 0.4 dB, normalized to exactly 0 dB at 765 Hz. Five biquads (one high-pass section is first order, stored as a biquad with b2 = a2 = 0). Minimum-phase by construction. *(Revised after the plan review: the first draft chose a 5th-order elliptic; see "Why Chebyshev-II".)*
- **Why this topology** (not a rescale of 001 R-07): S-005's transmit table gives +0.07 dB typical loss at 300 Hz and 26 dB typical at 60 Hz, with group-delay distortion 0.19 ms typical at 500 Hz and 0.11 ms at 600 Hz. A 5th-order 300 Hz Butterworth (001 R-07, scaled to 225 Hz) loses 3 dB at the band edge and fails the +0.2 dB limit by about 2.8 dB; a 3rd-order high-pass near 150 Hz (at 8 kHz) matches both the loss and the group-delay figures. The block diagram's "8th-order BPF" is then 3 high-pass poles plus the same 5th-order low-pass the receive path has. A 4th-order high-pass near 112 Hz would exceed the 0.47 ms group-delay limit at 450 Hz, so the order is fixed by the datasheet.
- **Why Chebyshev-II** (plan review finding, user decision in the spec's Clarifications): the datasheet's "≥ 30 dB" row is at 4.6 kHz, 35 % above the 3.4 kHz edge (scaled: 3450 vs 2550 Hz, above the 6 kHz Nyquist). A 5th-order elliptic reaches 35 dB at 2700 Hz, 6 % above the edge, where no source supports it; it misses the typical group-delay distortion by 1.3–1.7× (0.112 ms at 1950 Hz against a typical 0.067) and its group delay peaks at 4.4 ms at 2550 Hz, a ring that every VAS splice would excite through stage 10. The scaled analog 5th-order prototype loses about 1 dB at 2700 Hz and 6 dB at 2900 Hz. A Chebyshev-II of the same order (the zero-passband-ripple limit of the elliptic family, which matches the datasheet's monotonically rising passband) lands near both typicals at once with a 1.4 ms edge delay.
- **Prototype measurement** (scipy, 6 kHz, high-pass 112.5 Hz + Chebyshev-II 35 dB / stopband edge 2750 Hz, bracketing the chosen 32 dB design; the script re-verifies and its table is the record):

  | Property (FR-005, FR-006) | Limit | Typical (design goal) | Prototype |
  |---|---|---|---|
  | Gain at 765 Hz | 0 ± 0.1 dB | — | 0.000 dB |
  | Loss 225–2250 Hz | −0.15 … +0.2 dB | +0.07 at 225 Hz | 0.000 … +0.066 dB |
  | Loss at 2550 Hz | 0 … 0.8 dB | 0.4 dB | 0.30 dB (edge 2750); ≈ 0.4 at edge ≈ 2740 |
  | Attenuation at 45 Hz | ≥ 20 dB | 26 dB | 23.9 dB |
  | Attenuation at 20 Hz | ≥ 20 dB | — | 45 dB |
  | Attenuation at DC | ≥ 40 dB | — | ∞ (zeros at z = 1) |
  | Loss at 2700 / 2900 Hz | report only | — | 14 / 36 dB (elliptic: 35 / 36) |
  | GDD at 375 Hz | ≤ 1.0 ms | 0.25 ms | 0.22 ms |
  | GDD at 450 Hz | ≤ 0.47 ms | 0.15 ms | 0.14 ms |
  | GDD 750–1950 Hz | ≤ 0.17 ms | ≤ 0.067 ms | 0.003 … 0.078 ms |
  | GDD at 2100 Hz | ≤ 1.0 ms | 0.093 ms | 0.13 ms |
  | Group delay at 1 kHz | ≤ 2 ms | — | 0.09 ms |
  | Group delay at 225 Hz | > at 1 kHz | — | 0.87 ms |
  | Group delay at 2550 Hz | > at 1 kHz; ≤ 2 ms (engineering target) | — | 1.4 ms (elliptic: 4.4 ms) |
  | Max pole radius | < 1 | — | 0.943 (elliptic: 0.968) |

  Stopband-edge sensitivity (35 dB design): loss at 2550 Hz is 2.37 dB at edge 2700, 0.91 at 2725, 0.30 at 2750, 0.09 at 2775: about 0.6 dB per 25 Hz, three times gentler than the elliptic's 2 dB per 20 Hz. GDD at 1950 Hz moves 0.103 → 0.066 ms over the same range.
- **Edge-search rule** (design script): with the family, order and stopband attenuation fixed, search the stopband edge in 1 Hz steps and pick the edge whose 2550 Hz loss is within ±0.1 dB of the typical value (0.4 dB) **and** whose GDD at 1950 Hz leaves at least 0.015 ms below the 0.17 ms limit **and** whose 2550 Hz loss is at least +0.02 dB (so the 0 … 0.8 dB tolerance cannot fail on a rounding sign). With Chebyshev-II all three hold with wide margin (GDD ≈ 0.08 ms); the rule stays so a future re-tune to real-unit evidence is mechanical. The 0.015 ms is a design margin against the datasheet limit, not a measurement margin: the harness's GDD bias is ≤ 0.001 ms at every host rate (plan review). The chosen edge and the full margin table are written into the coefficient file header.
- **Normalization at 765 Hz**: the datasheet's reference is 1020 Hz (× 0.75). Latency still uses the group delay at 1 kHz (001 R-10, FR-004), unchanged in meaning.
- **Rationale**: Butterworth high-pass gives a monotonic, ripple-free low end that lands on the datasheet's typical values; Chebyshev-II gives a flat passband, the typical edge loss, group-delay distortion within about 0.01 ms of the typicals, and a stopband that grows gently above the edge like the analog filter it models.
- **Alternatives**: elliptic order 5 (first draft; rejected above on plausibility: unsupported 35 dB at 2700 Hz, 4.4 ms edge ring, GDD 1.7× typical). Chebyshev-I (GDD at 1950 Hz ≥ 0.19 ms at any edge that meets the 2550 Hz loss: fails). Butterworth order 5 (passes every tolerance, but at 2700 Hz it is only 3–8 dB down and its edge loss cannot reach the typical without the passband sagging; rejected on plausibility). Reusing the 001 design scaled by 0.75 (fails the +0.2 dB low-end limit). An impulse-invariant discretization of the scaled analog prototype would also reproduce the device's partial aliasing inside the stage; noted for a later realism option (R-15), not for this feature.

## R-05 Playback stage design (FR-008; A-027 receive corrected)

- **Decision**: the **5th-order Chebyshev-II low-pass alone**, 32 dB stopband attenuation (as R-04), stopband edge tuned near **2715 Hz** so the loss at 2550 Hz lands near the receive typical of 0.56 dB (prototype at 30 dB / 2720 Hz: 0.38 dB; the script's edge search lands on the typical with the same rule as R-04, targets 0.56 dB and GDD at 1950 Hz ≤ 0.155 ms). Three biquads, normalized to 0 dB at 765 Hz. **No high-pass**: DC passes with zero loss (Chebyshev-II has no passband ripple, so the DC gain equals the reference gain to within the normalization).
- **Why low-pass only** (the draft review's B1, verified against S-005): the receive table has no 60 Hz row, the 300 Hz loss is −0.03 dB typical (flat), the group-delay distortion is 0.00 ms typical at 500, 600 and 1000 Hz (a 225 Hz high-pass would show about 0.2 ms at 375 Hz), and the block diagram shows "5th LPF" only.
- **Prototype measurement** (30 dB / edge 2720 Hz): loss DC–2250 Hz 0.000 … +0.001 dB (limit −0.15 … +0.25); 2550 Hz 0.38 dB (0 … 0.8; typical 0.56, reached at edge ≈ 2712); GDD 375–1500 Hz ≤ 0.04 ms, 1950 Hz 0.110 ms (≤ 0.17; typical 0.12), 2100 Hz 0.165 ms (≤ 1.0; typical 0.16); group delay at 1 kHz 0.05 ms, at 2550 Hz 1.5 ms (> 1 kHz; ≤ 2 ms target); loss at 2700 / 2900 Hz 19 / 30 dB; max pole radius 0.92. The 32 dB / 2730 Hz variant: 0.38 dB, 0.112 ms, 0.169 ms, 1.5 ms, 18 / 32 dB.
- **Rationale**: same family and order as the record low-pass (the datasheet implies one 5th-order filter design on both sides with slightly different edge loss), tuned to the receive typical. DC passing is new emulator behavior; its audibility in the default chain is nil because the record stage has already removed everything below about 100 Hz.
- **Alternatives**: sharing the record stage's low-pass coefficients (0.4 dB instead of 0.56 dB at 2550 Hz; both inside the limit, so this is a design-to-typical choice; rejected to honour the user's decision). Elliptic, Chebyshev-I and Butterworth as in R-04.

## R-06 Stage structure, coefficients and the design script (FR-005, FR-008, SC-007; 001 R-07 revised)

- **Decision**:
  - `VoiceBandStage<const N: usize>` with `sections: [Biquad; N]`; `VoiceBandStage::<5>::record()` and `VoiceBandStage::<3>::playback()` use `RECORD_SOS: [[f64; 5]; 5]` and `PLAYBACK_SOS: [[f64; 5]; 3]` from the regenerated `voiceband_coeffs.rs`. `process`, `reset`, `ops` and `group_delay_1k_samples` are generic over N.
  - `DeviceChain` holds `record: VoiceBandStage<5>` and `playback: VoiceBandStage<3>`.
  - Test hooks: `Pipeline::__with_stage_sos(settings, record: [[f64; 5]; 5], playback: [[f64; 5]; 3])`; `__test_hooks::record_sos()` and `playback_sos()`.
  - Op-count bound (001 FR-015 tests): 8 biquads per device sample instead of 12; the per-sample constant in the op-count test is recomputed.
  - `design_voiceband.py`: designs both stages (R-04, R-05) with the edge-search rule, verifies every FR-005/FR-006/FR-008 bound analytically (loss table, GDD table relative to the band minimum, pole/zero radii), prints both margin tables, writes one file with both constants and both margin tables in its header, and exits non-zero on any bound. `--shift-hz` becomes `--shift-record-hp-hz`, `--shift-record-lp-hz` and `--shift-playback-lp-hz` (a negative value moves a stopband edge toward the band); `--emit-fixture PATH` writes `SHIFTED_RECORD_SOS` and `SHIFTED_PLAYBACK_SOS`. A `--family {cheby2,ellip}` switch (default `cheby2`, R-04) keeps the elliptic available for comparison tables only; the committed coefficients are Chebyshev-II.
  - Mutation (SC-007): three fixtures, each with one edge shifted 100 Hz **toward the band**: the record high-pass up (112.5 → 212.5 Hz gives 2.3 dB loss at 225 Hz, outside +0.2), the record low-pass stopband edge down 100 Hz (9.0 dB at 2550 Hz, outside 0.8) and the playback low-pass stopband edge down 100 Hz (10.0 dB). Each must fail the stage's response check and the 001 golden check. An upward shift of an upper edge passes every template row by design (no floor above 2550 Hz) and is caught by the golden check alone (spec Clarifications, plan review).
- **Rationale**: const generics keep the per-sample work fixed and allocation-free (Principle IV) with one implementation; the script remains the single source of design truth and the Rust tests re-verify from the committed bits (001 R-07).
- **Alternatives**: two stage modules (duplicated code); `Box<[Biquad]>` (allocation at construction is allowed, but the fixed-size array keeps the op-count reasoning and stack layout as today); padding the playback stage with identity biquads (wasteful, and it hides the structural difference).

## R-07 Measurement methods at the host rate (FR-003, FR-005 – FR-009; SC-002)

- **Decision** (`checks.rs`):
  - **Stage response** as 001 (configuration minus `bypass_all`, from impulse-response spectra, 65 536-point FFT at the host rate). The **test frequency set** (spec definitions) replaces 001's grid: 20, 45, 100, 150, 225, 300, 375, 450, 600, 765, 1000, 1500, 1950, 2100, 2250, 2400, 2550, 2700, 2900 Hz plus DC. Flatness over 225–2250 Hz (record) or DC–2250 Hz (playback) is the max deviation over the spectrum bins in that range, as 001's ripple check did.
  - **Per-stage template**: `check_stage_template(rate, Stage, make)` takes a `Template` (record or playback) with the loss limits, the GDD limits and whether low-frequency rejection applies. GDD: group delay from the stage spectrum's unwrapped phase, minus its minimum over 225–2550 Hz, evaluated at 375, 450, 750, 1000, 1500, 1950 and 2100 Hz. Report-only rows at 2700 and 2900 Hz use a `Tolerance::Report` variant that always passes and prints the value.
  - **DC** for the playback stage: a steady DC input (`stimulus::dc`) through `playback_only` vs `bypass_all`, steady-state ratio; for the record stage, attenuation ≥ 40 dB from the same measurement (the spectrum's DC bin is unreliable for a high-pass with zeros at z = 1).
  - **Minimum phase** (FR-005, FR-008): at the 6 kHz host rate, 225–2550 Hz, ±5°, as 001's check at 8 kHz.
  - **Anti-alias** (FR-007): `record_only` at each rate above 6 kHz; tones from the anti-alias set below Nyquist (3450, 4000, 5000, 7000, 10 000, 15 000, 20 000, 40 000 Hz); total output power including aliases relative to the 765 Hz tone's power; limit ≥ 30 dB. Expected ≥ 60 dB (boundary).
  - **Group delay at 2550 Hz** (FR-005, FR-008: > at 1 kHz; reported; ≤ 2 ms engineering target): evaluated from the stage spectrum at the exact frequencies 2549 and 2551 Hz with `analysis::dtft`, as `check_fr012_latency` does, not from adjacent FFT bins. At the band edge the group delay changes quickly with frequency, so the nearest-bin offset (≤ 0.37 Hz at 48 kHz) would bias a bin-based reading by several hundredths of a millisecond (plan review).
  - **Asymmetric flatness**: the loss limits are −0.15/+0.2 dB (record) and −0.15/+0.25 dB (playback), so the flatness check tracks the minimum and the maximum loss over the band separately, unlike 001's single max |deviation| ripple figure.
  - **Loss at 2550 Hz ≥ 0**: `Tolerance::Range(0.0, 0.8)` requires a positive loss there; the design script's edge search enforces loss(2550) ≥ 0.02 dB (R-04).
  - **Minimum-phase check fallback**: the cepstral reconstruction (001 R-13) now meets unit-circle zeros about 200 Hz above the band top instead of 400 Hz. If the ±5° check misbehaves near 2550 Hz at the 6 kHz identity rate, the trouble is the reconstruction, not the filter: evaluate 225–2400 Hz and rely on the coefficient zero-radius unit test for the edge (engineering target, labeled).
  - **Cascade** (FR-009): as 001 FR-011 over 50–2900 Hz where above −40 dB; 765 Hz gain 0 ± 0.2 dB.
  - **Latency** (FR-004): as 001 FR-012/FR-013 at 7 rates.
- **Rationale**: the datasheet defines GDD relative to the band minimum, so the harness does too; the identity rate makes the phase check exact; tones are the right instrument above the device band because an impulse response at the host rate contains almost nothing there (001 found the same at 4000 Hz).
- **Alternatives**: measuring GDD only at the identity rate (simpler, but FR-006/FR-008 are stated for every host rate and the subtraction of the linear-phase baseline makes it valid everywhere).

## R-08 Stimulus phase rule for sample-peak detectors (spec FR-012; 003 FR-006/FR-007)

- **Problem**: a 1 kHz tone has 6 samples per cycle at the device rate. If no device sample lands on a peak, the VAS's sample-peak detector (003 R-02) reads the tone low by up to 20·log10(cos 30°) = 1.25 dB, outside 003 FR-006's ±1 dB. At 8 samples per cycle the worst case was 0.69 dB and 003 relied on it.
- **Decision**: the harness generates every steady tone used to bisect the VAS threshold (003 FR-006, FR-007, the US2 AS3 bisection) with a starting phase that puts a device sample on the peak. For a tone of frequency f at host rate h, the device grid sees x at times (k / 6000 − D / h) where D is the decimator's delay in host samples (0 at 6 kHz). Device sample k has phase 2πf·k/6000 − 2πf·D/h + φ. Choosing

  φ = (π/2 + 2π f D / h) mod (2π f / 6000)

  puts the phase of some device sample at π/2 exactly, for every host rate and every f that divides 6000 (1000, 1500, 2000, 3000, 750 Hz…). At 6 kHz with f = 1 kHz this is π/6, the spec's example. D is `Pipeline::new(bypass_all).latency_samples() / 2` at that rate (the boundary is two equal linear-phase converters, 001 R-10), so the rule needs nothing hidden. Helper: `stimulus::aligned_phase(f, host, latency_bypass)` and `stimulus::tone_at_phase`.
- **Where it applies**: `vas_checks::tone` (FR-006/FR-007 bisection at 1000 Hz; 225 and 2550 Hz are 3/80 and 17/40 cycles per sample, the same grid patterns as 300 and 3400 Hz at 8 kHz, so their worst-case sample-peak error is ≤ 0.03 dB and they need no alignment); `us2_vas_settings` AS3; **and the core unit tests** (plan review): the `tone()` helpers in `stages/vas.rs` (`lowest_kept` asserts −18 ± 0.05 dBFS) and `stages/agc.rs` (`static_curve_at_defaults`, ±1 dB) generate 1 kHz at phase 0 and would read 1.25 dB low at 6 samples per cycle. At the identity rate the rule is a half-sample offset: `sin(TAU · ((k % 6) as f64 + 0.5) / 6.0)` puts samples at 30°, 90°, 150° …, so the peak is exact; the `loud()` helper becomes `(k % 6) + 0.5` (|sin| ≥ 0.5, which keeps its existing inequality). **Not** the burst-gap stimuli at threshold + 10 dB (a 1.25 dB read error cannot change a decision there), so the VAS golden stimuli keep phase 0 and their definitions change only through the noise band (R-11). **Not** the AGC's FR-008 tones: its Hilbert-envelope detector is phase-insensitive, and FR-008 sweeps phases on purpose at 1500 and 2000 Hz.
- **Rationale**: keeps spec 003's law (1 kHz reference, ±1 dB) measurable without touching the detector. The spec records that this sensitivity is a model property, not a device one.
- **Alternatives**: 750 Hz reference (changes 003's stated frequencies); an RMS or envelope detector in the VAS (a behavioral change belonging to the sensitivity-placement feature).

## R-09 AGC restatements (spec FR-011; 002 FR-006, FR-008, FR-009; A-019)

- **Hilbert detector**: `design_hilbert.py` designs in normalized frequency. Run with `FS = 6000`, `BAND_HZ = (187.5, 2812.5)` and `CHECK_HZ = (225, 2550)` during planning, it produced **all 63 coefficients bit-identical** to the committed `agc_hilbert_coeffs.rs`. The script is updated to those constants and the generated header comment regenerated; the bit patterns do not change, which the tasks verify by diff. The detector's delay is still 31 samples (now 5.17 ms); `agc.rs` compensates the release from `DEVICE_RATE_HZ` and the attack law is unchanged (002 R-02), so 002 FR-006 is expected to pass unchanged; the harness decides.
- **Harness**: `agc_checks.rs` `DETECTOR_DELAY_S = 31.0 / 6000.0`; `measure_timing(exact)` and `check_ratio_self_test` at 6000; FR-008 tones **225, 375, 750, 1000, 1500, 2000 (6000/3), 2550 Hz** with the 4-phase sweeps at **1500 and 2000 Hz** (4 and 3 samples per cycle), levels and ±0.5 dB / THD+N limits unchanged; FR-009 band-limited noise **225–2550 Hz** (R-11), level and limits unchanged; `stages/mod.rs`'s test frequency list likewise.
- **Register**: A-019's "device-band signal (up to 4 kHz)" → "up to 3 kHz"; S-002's note that the real detector sees the mic amp's full bandwidth stays.
- **Rationale**: the AGC law (A-017, A-018) is not touched by this feature; only the band it is measured over moves with the device.

## R-10 VAS restatements (spec FR-012; 003 FR-004 – FR-010)

- **Core**: no logic change. `W = 32` device samples stays (its invariant is the samples in a half-period of the lowest in-band tone: 225 Hz at 6 kHz and 300 Hz at 8 kHz are both 13.3 samples); the comment is updated (5.3 ms). Unit tests: `FS = 6000`, `H = 6000`, `O = 120`, burst lengths rescaled (e.g. 003's 80/144/159/160/161/240/8000 → 60/108/119/120/121/180/6000 with the same meaning).
- **Pipeline tests**: the 8000-based arithmetic in the VAS integration tests becomes 6000-based (e.g. 6000 + 6000 + 6000 − 120 kept samples).
- **Harness** (`vas_checks.rs`, `us*_vas*.rs`, `vas_edge_cases.rs`): `DEVICE_RATE = 6000`; FR-006 frequencies **225, 1000, 2550 Hz**; the phase rule (R-08) for the 1 kHz bisection; `integer_ratio = rate % 6000 == 0` (true at 6, 48, 96 kHz); the decimator-edge smear term uses `fs / 6000`; the lead bound and settling condition use ⌈host / 6000⌉; noise checks use the 225–2550 Hz band; everything else (threshold, hang, onset, tolerances, floor-sweep report) unchanged.
- **Rationale**: 003's laws are in milliseconds and dB; only their sample counts and band move.

## R-11 Golden files: one re-bless (FR-013, SC-004; 001 R-14)

- **Decision**: the format stays `rr_dr60-golden` version 1; file names stay (`golden-v1.json`, `golden-agc-v1.json`, `golden-vas-v1.json`: "v1" is the format version). `library_version` becomes 0.4.0 (it follows `rr_dr60::VERSION`). Entry counts: **112** (4 stimuli × 4 configs × 7 rates), **42** (3 × 2 × 7), **63** (3 × 3 × 7).
- **Stimuli**: unchanged definitions except where the band moved: `agc_noise_burst` and `vas_noise_gaps` use `bandlimited_noise`, whose band becomes 225–2550 Hz (one pair of constants in `stimulus.rs`, labeled A-027 / 002 FR-009 restated). `sweep_log` (20 Hz to 0.45·host) and the others are rate-relative and unchanged. The VAS bursts keep phase 0 (R-08).
- **Procedure**: one task, one commit: regenerate coefficients → run the full suite → `RR_DR60_BLESS=1`, `=agc`, `=vas` → update `GOLDEN_V1_SHA256` and `GOLDEN_AGC_V1_SHA256` in `golden_agc.rs` to the new files' hashes (the guards keep protecting against later unintended changes) → CHANGELOG entry. Any later golden change inside the feature is a bug, not a re-bless.
- **Rationale**: FR-013 and Principle III. Keeping the guards (rather than deleting them) preserves the "no silent change" property for the rest of the feature and beyond.

## R-12 CI time and performance (SC-006, 001 FR-022)

- **Prototype estimate**: the 7th rate adds about 1/6 to every rate matrix; the 44.1 and 88.2 kHz pipelines construct about 4.5× faster (8 481-tap prototypes instead of about 38 000) and each device sample costs 8 biquads instead of 12 at 6000 instead of 8000 samples per second. Net: the debug suite and the release matrices are expected to be flat or faster; the coverage job (16 min at 003) should not grow. The timing test's `default`/`default_agc`/`default_vas` rows are re-measured and the README figures updated.
- **Decision**: no CI workflow change. Tasks record the before/after suite time in the PR, and the coverage job's time is checked on the first CI run.
- **0.3 baseline (measured 2026-10-11 at commit 75734b7, T003, Apple-silicon laptop)**: `cargo test --all-features` wall time **89 s** (231 tests, warm build); release timing test: `default` 88.5–89.8 ns/sample, **234× real time** at 48 kHz; `default_agc` 91.6–92.6 ns/sample, **227×**; `default_vas` 91.5–92.4 ns/sample, **227×**. CI reference from 003: coverage job 16.2 min.

## R-13 Documentation, register, sources, CHANGELOG, version (FR-013, FR-014, FR-017, FR-018; SC-001, SC-008)

- **CHANGELOG** (`[Unreleased]` › `### Changed`, one entry): device rate 8 → 6 kHz (A-026 adopted; A-001 superseded); two templates (A-027 adopted and corrected; A-002, A-014 superseded; the record and playback stages differ; the playback stage passes DC); host rate 6 kHz added, 8 and 16 kHz now conversions (`SUPPORTED_HOST_RATES` is `[u32; 7]`); default latency per rate (old → new); every restated figure in a table: 001 FR-002/FR-004/FR-005/FR-010/FR-011 and edge cases, 002 A-019 band, FR-006 identity rate, FR-008 tones, FR-009 band, 003 FR-004/FR-005 exact rates and lead bound, FR-006 frequencies and the phase rule, hang/onset sample counts; golden files re-blessed (counts); the un-modeled aliasing and imaging (R-15); version 0.4.0.
- **Register**: A-001, A-002, A-014 → status `superseded by spec 004` with the date; A-016 → confidence High, status `partially verified (S-005, MSM7717 family proxy)`, with the 6 kHz GDD figures; A-019 band → 3 kHz; A-027 → receive row rewritten as low-pass only with the FR-008 GDD limits, and the record row with the FR-006 GDD limits, status `assumed (adopted by spec 004)`.
- **Sources**: `S-007` ITU-T G.712 ("Transmission performance characteristics of pulse code modulation channels"). A task reads its attenuation/frequency-distortion mask (Table 1 / Figure 1; the 1992 or 2001 edition is free from the ITU). It is cited for the origin of 001's 4000 Hz (≥ 14 dB) and 4600 Hz (≥ 25–32 dB) points. It is cited for FR-005's ≥ 40 dB DC attenuation **only if the mask states DC attenuation**; the reviewer could not find such a row (the masks are anchored at 300 and 4600 Hz), in which case the DC figure stays an engineering target whose rationale is "zeros at z = 1; the floor catches a missing high-pass" (spec Clarifications, plan review).
  - **Finding (T002, 2026-10-11): G.712 states no DC attenuation requirement.** Its half-channel mask (Figure 4) starts at 200 Hz, and clause 10.2 recommends no particular value below 300 Hz (a note mentions 20–26 dB at 15–60 Hz, the origin of S-005's 60 Hz row). **FR-005's ≥ 40 dB at DC stays an engineering target.** S-007 is cited for the 4600 Hz / 25 dB image-rejection clause (10.1), the Figure 10 template (about −12.5 dB at 4000 Hz; A-014's 14 dB was a rounding of it), the 15–60 Hz note, and clause 8's "group delay is a design objective". The coefficient-file header, `checks.rs` and the register cite accordingly (T011, T019, T039).
- **Signal chain**: rows 4 and 10 point to spec 004; "not modeled" lists the partial aliasing of 3000–3450 Hz, the DAC imaging residue, µ-law and idle noise.
- **README**: rates list (7); "internal 8 kHz" → 6 kHz; the two templates in plain words with the medium-confidence caveat; latency figure; fidelity-table row for the codec band-limiting ("yes, revised: datasheet templates scaled to 6 kHz; not measured against a unit"); performance figures re-measured.
- **Earlier specs**: one line after the `**Status**` line of specs 001, 002 and 003: "**Restated by spec 004** (2026-10-11): the device-rate-dependent figures of this spec are restated in [spec 004](../004-device-rate-6khz/spec.md) FR-010 to FR-012 and in the CHANGELOG; the text below is otherwise unchanged." (clarify Q2).
- **Other prose**: `tools/filter-design/README.md`, `CLAUDE.md` status and "assumed 8 kHz sample rate" lines, crate docs in `lib.rs`, the C header docs (regenerated), `.github/ISSUE_TEMPLATE/hardware_evidence.yml` (the sweep request already reaches 8 kHz; add a request for 2.7, 2.9 and 3.2 kHz tones against a 1 kHz reference, which decides A-027's top edge and the device's own aliasing, R-15). Two stale comments found by the plan review: `pipeline.rs` `max_events` doc "+2 at 44.1 and 88.2 kHz" → "at the non-integer rates (8, 16, 44.1 and 88.2 kHz)"; `vas.rs` W comment "4 ms … 300 Hz" → "5.3 ms … 225 Hz".
- **Honesty wording** (plan review): "its modeled top edge near 2.5 kHz (A-027)", never "real"; the 2550–3000 Hz region is "not constrained by any source; the emulator's roll-off there is a design choice, reported at 2700 and 2900 Hz", not "not modeled". If the elliptic had stayed, a deviation note would have been required; with Chebyshev-II the roll-off is within the plausible range of the analog filter and needs only the report rows.
- **Version**: `Cargo.toml` workspace version 0.4.0 (pre-1.0 breaking change: `SUPPORTED_HOST_RATES` type and all output).

## R-14 Engineering-target constants (FR-015; SC-008)

Labeled `// engineering target (004 FR-015)` in code and tests, or with the spec FR they come from: the ±0.1 dB reference-gain and ±0.2 dB cascade tolerances; the test frequency set and the anti-alias set; the −0.15 dB lower bound on the playback template; the report-only frequencies 2700 and 2900 Hz; the ±5° phase tolerance and the 2 ms group delay (A-016); `CUTOFF_HZ`, `TRANSITION_HZ`, `STOPBAND_DB` (FR-003); the ±0.3 dB / −40 dB cascade figures (FR-009); the 20 ms budget and ±1 sample (FR-004); the −60 dBFS DC edge case and the 0.5 s tail; the 100 Hz mutation shifts (SC-007); the edge-search rule's ±0.1 dB and 0.015 ms margins (R-04; the 0.015 ms is a design margin against the datasheet limit, not a measurement margin: the harness's GDD bias is ≤ 0.001 ms at every host rate); the 32 dB stopband attenuation (R-04: S-005's typical discrimination, used as a design parameter); the 2 ms group-delay ceiling at 2550 Hz (FR-005, FR-008); the 40 dB DC attenuation if G.712 does not state it (R-13); the Chebyshev-II family and order (R-04, R-05: design choices inside the template, labeled as such in the coefficient file); the phase rule (R-08) and the 5 minutes in SC-001. Datasheet-derived values cite `A-027` (and `S-005`), the DC attenuation `S-007`, the rate `A-026`.

## R-15 Known deviations not modeled (FR-007, FR-018)

- The device's anti-alias path is only required to reach 30 dB by 3450 Hz (S-005 discrimination row, scaled), so analog content between 3000 and 3450 Hz partially aliases into 2550–3000 Hz on the real unit; the emulator's boundary rejects it by ≥ 60 dB. The device's DAC leaves imaging residue above 3450 Hz at about −35 dBm0 (S-005 out-of-band spurious); the emulator's boundary removes it.
- Both are recorded in `signal-chain.md` (rows 4 and 10) and the README as known deviations that make the emulator slightly cleaner at the top of the band than the device, reversible later as a stage-4/10 realism option. The speech codec (a later feature) will dominate the top of the band in any case.
