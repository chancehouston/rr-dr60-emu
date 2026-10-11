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

  | Host | Taps | Delay per converter | Boundary total | MAC per device sample (down) | MAC per host sample (up) |
  |---|---|---|---|---|---|
  | 8 000 | 235 | 39 samples = 4.88 ms | 9.75 ms | ≈ 58 | ≈ 78 |
  | 16 000 | 463 | 77 = 4.81 ms | 9.62 ms | ≈ 57 | ≈ 154 |
  | 44 100 | 8 481 | 212 = 4.81 ms | 9.61 ms | ≈ 57 | ≈ 424 |
  | 48 000 | 463 | 231 = 4.81 ms | 9.62 ms | ≈ 57 | ≈ 463 |
  | 88 200 | 8 481 | 424 = 4.81 ms | 9.61 ms | ≈ 57 | ≈ 848 |
  | 96 000 | 923 | 461 = 4.80 ms | 9.60 ms | ≈ 57 | ≈ 923 |

  Compared with today: the 44.1 and 88.2 kHz prototypes shrink from about 38 000 taps to about 8 500 (prototype rate 882 kHz instead of 3.528 MHz), so construction time and memory fall by about 4.5×; the per-converter delay falls from about 5.4 to about 4.8 ms.
- **Default latency** (FR-004): 2 × 4.81 ms + (τ_rec + τ_play) at 1 kHz ≈ 9.62 + 0.11 + 0.07 ≈ 9.8 ms. Prototype values in host samples: about **1 at 6 kHz** (identity: only the stages' ≈ 1.0 device sample of group delay), **78 at 8 kHz**, **157 at 16 kHz**, **433 at 44.1 kHz**, **471 at 48 kHz** (was 541), **866 at 88.2 kHz**, **942 at 96 kHz**. The harness measures the final values and the CHANGELOG records them.
- **Harness restatement** (FR-003): `check_fr005_boundary` → `check_fr003_boundary`: flatness 50–2550 Hz within ±0.1 dB; rejection ≥ 60 dB at 3000 Hz, 3450 Hz, 6000 Hz and 20 kHz where below Nyquist; alias/image products ≤ −60 dB on a 1 kHz tone. Skipped at the 6 kHz host rate except the identity assertion.
- **Rationale**: linear-phase boundary keeps the per-stage minimum-phase checks clean (001 R-08). A 300 Hz transition (2700 → 3000 Hz) would cost about 6.0 ms per converter, 12.1 ms boundary, 14 ms default: legal but eats most of the budget for no template benefit, since the templates end at 2550 Hz.
- **Alternatives**: as 001 R-08.

## R-04 Record stage design (FR-005, FR-006; A-027 transmit, A-016)

- **Decision**: a cascade of a **3rd-order Butterworth high-pass** with its −3 dB point at **112.5 Hz** (0.75 × 150 Hz) and a **5th-order elliptic low-pass**, 0.1 dB passband ripple, 35 dB stopband, passband edge tuned near **2540 Hz**, normalized to exactly 0 dB at 765 Hz. Five biquads (one high-pass section is first order, stored as a biquad with b2 = a2 = 0). Minimum-phase by construction.
- **Why this topology** (not a rescale of 001 R-07): S-005's transmit table gives +0.07 dB typical loss at 300 Hz and 26 dB typical at 60 Hz, with group-delay distortion 0.19 ms typical at 500 Hz and 0.11 ms at 600 Hz. A 5th-order 300 Hz Butterworth (001 R-07, scaled to 225 Hz) loses 3 dB at the band edge and fails the +0.2 dB limit by about 2.8 dB; a 3rd-order high-pass near 150 Hz (at 8 kHz) matches both the loss and the group-delay figures. The block diagram's "8th-order BPF" is then 3 high-pass poles plus the same 5th-order low-pass the receive path has.
- **Prototype measurement** (scipy, 6 kHz, high-pass 112.5 Hz + low-pass 0.1 dB / 35 dB / edge 2540 Hz; the script re-verifies):

  | Property (FR-005, FR-006) | Limit | Typical (design goal) | Designed |
  |---|---|---|---|
  | Gain at 765 Hz | 0 ± 0.1 dB | — | 0.000 dB |
  | Loss 225–2250 Hz | −0.15 … +0.2 dB | +0.07 at 225 Hz | −0.02 … +0.13 dB |
  | Loss at 2550 Hz | 0 … 0.8 dB | 0.4 dB | ≈ 0.40 dB (edge-dependent, see risk) |
  | Attenuation at 45 Hz | ≥ 20 dB | 26 dB | 23.9 dB |
  | Attenuation at 20 Hz | ≥ 20 dB | — | 45 dB |
  | Attenuation at DC | ≥ 40 dB | — | ∞ (zeros at z = 1) |
  | Loss at 2700 / 2900 Hz | report only | — | ≈ 35 / 36 dB |
  | GDD at 375 Hz | ≤ 1.0 ms | 0.25 ms | 0.22 ms |
  | GDD at 450 Hz | ≤ 0.47 ms | 0.15 ms | 0.14 ms |
  | GDD 750–1950 Hz | ≤ 0.17 ms | ≤ 0.09 ms | 0.02 … 0.15 ms |
  | GDD at 2100 Hz | ≤ 1.0 ms | 0.09 ms | ≈ 0.24 ms |
  | Group delay at 1 kHz | ≤ 2 ms | — | 0.10 ms |
  | Group delay at 225 / 2550 Hz > at 1 kHz | yes | — | 0.88 / ≈ 4 ms, yes |

- **Risk and rule for the low-pass edge**: with a 5th-order elliptic the loss at 2550 Hz swings about 2 dB for a 20 Hz change of the passband edge (edge 2520 Hz → 2.4 dB, 2540 → 0.40, 2560 → 0.0), and the group-delay distortion at 1950 Hz rises as the edge comes down (0.144 ms at 2560, 0.153 at 2540, 0.163 at 2520, limit 0.17). The design script therefore searches the edge in 1 Hz steps and picks the highest edge whose 2550 Hz loss is within ±0.1 dB of the typical value **and** whose GDD at 1950 Hz leaves at least 0.015 ms (0.09 device samples) of margin; if the two cannot both be met, the GDD margin wins and the 2550 Hz loss is allowed to fall toward 0.2 dB. The chosen edge and the margin table are written into the coefficient file header.
- **Normalization at 765 Hz**: the datasheet's reference is 1020 Hz (× 0.75). Latency still uses the group delay at 1 kHz (001 R-10, FR-004), which is unchanged in meaning.
- **Rationale**: Butterworth high-pass gives a monotonic, ripple-free low end that lands on the datasheet's typical values; elliptic low-pass gives the datasheet's "0.4 dB at the edge, ≥ 30 dB soon after" shape and, with order 5, the group-delay distortion inside the limits. The high-pass order is fixed by the group-delay limits (B2 of the review): a 4th-order high-pass near 112 Hz would exceed 0.47 ms at 450 Hz.
- **Alternatives**: Chebyshev-I low-pass (GDD at 1950 Hz ≥ 0.19 ms at any edge that meets the 2550 Hz loss: fails). Butterworth 5th-order low-pass (passes every tolerance, but its loss at 2700 Hz is only 3–8 dB, far from the family's steep edge, and it under-represents the datasheet's shape between 2550 and 3450 Hz; rejected on plausibility, not on tolerance; the plan reports 2700/2900 Hz so the choice is visible). Reusing the 001 design scaled by 0.75 (fails the +0.2 dB low-end limit, see above).

## R-05 Playback stage design (FR-008; A-027 receive corrected)

- **Decision**: the **5th-order elliptic low-pass alone**: 0.15 dB passband ripple, 35 dB stopband, passband edge tuned near 2540 Hz so the loss at 2550 Hz lands near the receive typical of 0.56 dB (prototype 0.62 dB at 2540; the script applies the same edge-search rule as R-04 with the receive targets: loss within ±0.1 dB of 0.56, GDD at 1950 Hz ≤ 0.155 ms). Three biquads, normalized to 0 dB at 765 Hz. **No high-pass**: DC passes with the passband's loss (prototype −0.02 dB).
- **Why low-pass only** (the review's B1, verified against S-005): the receive table has no 60 Hz row, the 300 Hz loss is −0.03 dB typical (flat), the group-delay distortion is 0.00 ms typical at 500, 600 and 1000 Hz (a 225 Hz high-pass would show about 0.2 ms at 375 Hz), and the block diagram shows "5th LPF" only.
- **Prototype measurement** (edge 2540, 0.15 dB / 35 dB): loss DC–2250 Hz −0.03 … +0.13 dB (limit −0.15 … +0.25); 2550 Hz 0.62 dB (0 … 0.8); GDD 375–1500 Hz ≤ 0.05 ms, 1950 Hz 0.155 ms (≤ 0.17), 2100 Hz 0.24 ms (≤ 1.0); group delay at 1 kHz 0.07 ms; at 2550 Hz about 4 ms (> 1 kHz); loss at 2700 / 2900 Hz ≈ 36 / 36 dB.
- **Rationale**: same family as the record low-pass (the datasheet implies one 5th-order filter design on both sides with slightly different edge loss), different edge loss. DC passing is new emulator behavior; its audibility in the default chain is nil because the record stage has already removed everything below about 100 Hz.
- **Alternatives**: sharing the record stage's low-pass coefficients (would give 0.40 dB instead of 0.56 dB at 2550 Hz; both are inside the limit, so this is a design-to-typical choice, not a tolerance one; rejected to honour the user's "design to typical" decision). Butterworth and Chebyshev as in R-04.

## R-06 Stage structure, coefficients and the design script (FR-005, FR-008, SC-007; 001 R-07 revised)

- **Decision**:
  - `VoiceBandStage<const N: usize>` with `sections: [Biquad; N]`; `VoiceBandStage::<5>::record()` and `VoiceBandStage::<3>::playback()` use `RECORD_SOS: [[f64; 5]; 5]` and `PLAYBACK_SOS: [[f64; 5]; 3]` from the regenerated `voiceband_coeffs.rs`. `process`, `reset`, `ops` and `group_delay_1k_samples` are generic over N.
  - `DeviceChain` holds `record: VoiceBandStage<5>` and `playback: VoiceBandStage<3>`.
  - Test hooks: `Pipeline::__with_stage_sos(settings, record: [[f64; 5]; 5], playback: [[f64; 5]; 3])`; `__test_hooks::record_sos()` and `playback_sos()`.
  - Op-count bound (001 FR-015 tests): 8 biquads per device sample instead of 12; the per-sample constant in the op-count test is recomputed.
  - `design_voiceband.py`: designs both stages (R-04, R-05) with the edge-search rule, verifies every FR-005/FR-006/FR-008 bound analytically (loss table, GDD table relative to the band minimum, pole/zero radii), prints both margin tables, writes one file with both constants and both margin tables in its header, and exits non-zero on any bound. `--shift-hz` becomes `--shift-record-hp-hz`, `--shift-record-lp-hz` and `--shift-playback-lp-hz`; `--emit-fixture PATH` writes `SHIFTED_RECORD_SOS` and `SHIFTED_PLAYBACK_SOS`.
  - Mutation (SC-007): three fixtures, each with one edge shifted by 100 Hz: the record high-pass (112.5 → 212.5 Hz gives 2.3 dB loss at 225 Hz, outside +0.2), the record low-pass (edge −100 Hz gives > 20 dB loss at 2550 Hz, outside 0.8) and the playback low-pass (same). Each must fail the stage's response check and the 001 golden check.
- **Rationale**: const generics keep the per-sample work fixed and allocation-free (Principle IV) with one implementation; the script remains the single source of design truth and the Rust tests re-verify from the committed bits (001 R-07).
- **Alternatives**: two stage modules (duplicated code); `Box<[Biquad]>` (allocation at construction is allowed, but the fixed-size array keeps the op-count reasoning and stack layout as today); padding the playback stage with identity biquads (wasteful, and it hides the structural difference).

## R-07 Measurement methods at the host rate (FR-003, FR-005 – FR-009; SC-002)

- **Decision** (`checks.rs`):
  - **Stage response** as 001 (configuration minus `bypass_all`, from impulse-response spectra, 65 536-point FFT at the host rate). The **test frequency set** (spec definitions) replaces 001's grid: 20, 45, 100, 150, 225, 300, 375, 450, 600, 765, 1000, 1500, 1950, 2100, 2250, 2400, 2550, 2700, 2900 Hz plus DC. Flatness over 225–2250 Hz (record) or DC–2250 Hz (playback) is the max deviation over the spectrum bins in that range, as 001's ripple check did.
  - **Per-stage template**: `check_stage_template(rate, Stage, make)` takes a `Template` (record or playback) with the loss limits, the GDD limits and whether low-frequency rejection applies. GDD: group delay from the stage spectrum's unwrapped phase, minus its minimum over 225–2550 Hz, evaluated at 375, 450, 750, 1000, 1500, 1950 and 2100 Hz. Report-only rows at 2700 and 2900 Hz use a `Tolerance::Report` variant that always passes and prints the value.
  - **DC** for the playback stage: a steady DC input (`stimulus::dc`) through `playback_only` vs `bypass_all`, steady-state ratio; for the record stage, attenuation ≥ 40 dB from the same measurement (the spectrum's DC bin is unreliable for a high-pass with zeros at z = 1).
  - **Minimum phase** (FR-005, FR-008): at the 6 kHz host rate, 225–2550 Hz, ±5°, as 001's check at 8 kHz.
  - **Anti-alias** (FR-007): `record_only` at each rate above 6 kHz; tones from the anti-alias set below Nyquist (3450, 4000, 5000, 7000, 10 000, 15 000, 20 000, 40 000 Hz); total output power including aliases relative to the 765 Hz tone's power; limit ≥ 30 dB. Expected ≥ 60 dB (boundary).
  - **Cascade** (FR-009): as 001 FR-011 over 50–2900 Hz where above −40 dB; 765 Hz gain 0 ± 0.2 dB.
  - **Latency** (FR-004): as 001 FR-012/FR-013 at 7 rates.
- **Rationale**: the datasheet defines GDD relative to the band minimum, so the harness does too; the identity rate makes the phase check exact; tones are the right instrument above the device band because an impulse response at the host rate contains almost nothing there (001 found the same at 4000 Hz).
- **Alternatives**: measuring GDD only at the identity rate (simpler, but FR-006/FR-008 are stated for every host rate and the subtraction of the linear-phase baseline makes it valid everywhere).

## R-08 Stimulus phase rule for sample-peak detectors (spec FR-012; 003 FR-006/FR-007)

- **Problem**: a 1 kHz tone has 6 samples per cycle at the device rate. If no device sample lands on a peak, the VAS's sample-peak detector (003 R-02) reads the tone low by up to 20·log10(cos 30°) = 1.25 dB, outside 003 FR-006's ±1 dB. At 8 samples per cycle the worst case was 0.69 dB and 003 relied on it.
- **Decision**: the harness generates every steady tone used to bisect the VAS threshold (003 FR-006, FR-007, the US2 AS3 bisection) with a starting phase that puts a device sample on the peak. For a tone of frequency f at host rate h, the device grid sees x at times (k / 6000 − D / h) where D is the decimator's delay in host samples (0 at 6 kHz). Device sample k has phase 2πf·k/6000 − 2πf·D/h + φ. Choosing

  φ = (π/2 + 2π f D / h) mod (2π f / 6000)

  puts the phase of some device sample at π/2 exactly, for every host rate and every f that divides 6000 (1000, 1500, 2000, 3000, 750 Hz…). At 6 kHz with f = 1 kHz this is π/6, the spec's example. D is `Pipeline::new(bypass_all).latency_samples() / 2` at that rate (the boundary is two equal linear-phase converters, 001 R-10), so the rule needs nothing hidden. Helper: `stimulus::aligned_phase(f, host, latency_bypass)` and `stimulus::tone_at_phase`.
- **Where it applies**: `vas_checks::tone` (FR-006/FR-007 bisection at 1000 Hz; 225 and 2550 Hz are not phase-locked to the grid and need no alignment); `us2_vas_settings` AS3. **Not** the burst-gap stimuli at threshold + 10 dB (a 1.25 dB read error cannot change a decision there), so the VAS golden stimuli keep phase 0 and their definitions change only through the noise band (R-11). **Not** the AGC's FR-008 tones: its Hilbert-envelope detector is phase-insensitive, and FR-008 sweeps phases on purpose at 1500 and 2000 Hz.
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

## R-13 Documentation, register, sources, CHANGELOG, version (FR-013, FR-014, FR-017, FR-018; SC-001, SC-008)

- **CHANGELOG** (`[Unreleased]` › `### Changed`, one entry): device rate 8 → 6 kHz (A-026 adopted; A-001 superseded); two templates (A-027 adopted and corrected; A-002, A-014 superseded; the record and playback stages differ; the playback stage passes DC); host rate 6 kHz added, 8 and 16 kHz now conversions (`SUPPORTED_HOST_RATES` is `[u32; 7]`); default latency per rate (old → new); every restated figure in a table: 001 FR-002/FR-004/FR-005/FR-010/FR-011 and edge cases, 002 A-019 band, FR-006 identity rate, FR-008 tones, FR-009 band, 003 FR-004/FR-005 exact rates and lead bound, FR-006 frequencies and the phase rule, hang/onset sample counts; golden files re-blessed (counts); the un-modeled aliasing and imaging (R-15); version 0.4.0.
- **Register**: A-001, A-002, A-014 → status `superseded by spec 004` with the date; A-016 → confidence High, status `partially verified (S-005)`, with the 6 kHz GDD figures; A-019 band → 3 kHz; A-027 → receive row rewritten as low-pass only with the FR-008 GDD limits, and the record row with the FR-006 GDD limits, status `assumed (adopted by spec 004)`.
- **Sources**: `S-007` ITU-T G.712 ("Transmission performance characteristics of pulse code modulation channels"), cited for the ≥ 40 dB DC attenuation inherited by FR-005 and for the origin of 001's 4000 Hz point (Assumptions).
- **Signal chain**: rows 4 and 10 point to spec 004; "not modeled" lists the partial aliasing of 3000–3450 Hz, the DAC imaging residue, µ-law and idle noise.
- **README**: rates list (7); "internal 8 kHz" → 6 kHz; the two templates in plain words with the medium-confidence caveat; latency figure; fidelity-table row for the codec band-limiting ("yes, revised: datasheet templates scaled to 6 kHz; not measured against a unit"); performance figures re-measured.
- **Earlier specs**: one line after the `**Status**` line of specs 001, 002 and 003: "**Restated by spec 004** (2026-10-11): the device-rate-dependent figures of this spec are restated in [spec 004](../004-device-rate-6khz/spec.md) FR-010 to FR-012 and in the CHANGELOG; the text below is otherwise unchanged." (clarify Q2).
- **Other prose**: `tools/filter-design/README.md`, `CLAUDE.md` status and "assumed 8 kHz sample rate" lines, crate docs in `lib.rs`, the C header docs (regenerated), `.github/ISSUE_TEMPLATE/hardware_evidence.yml` (the sweep request already reaches 8 kHz; no change needed).
- **Version**: `Cargo.toml` workspace version 0.4.0 (pre-1.0 breaking change: `SUPPORTED_HOST_RATES` type and all output).

## R-14 Engineering-target constants (FR-015; SC-008)

Labeled `// engineering target (004 FR-015)` in code and tests, or with the spec FR they come from: the ±0.1 dB reference-gain and ±0.2 dB cascade tolerances; the test frequency set and the anti-alias set; the −0.15 dB lower bound on the playback template; the report-only frequencies 2700 and 2900 Hz; the ±5° phase tolerance and the 2 ms group delay (A-016); `CUTOFF_HZ`, `TRANSITION_HZ`, `STOPBAND_DB` (FR-003); the ±0.3 dB / −40 dB cascade figures (FR-009); the 20 ms budget and ±1 sample (FR-004); the −60 dBFS DC edge case and the 0.5 s tail; the 100 Hz mutation shifts (SC-007); the edge-search rule's ±0.1 dB and 0.015 ms margins (R-04); the elliptic ripple and stopband parameters (R-04, R-05: design choices inside the template, labeled as such in the coefficient file); the phase rule (R-08) and the 5 minutes in SC-001. Datasheet-derived values cite `A-027` (and `S-005`), the DC attenuation `S-007`, the rate `A-026`.

## R-15 Known deviations not modeled (FR-007, FR-018)

- The device's anti-alias path is only required to reach 30 dB by 3450 Hz (S-005 discrimination row, scaled), so analog content between 3000 and 3450 Hz partially aliases into 2550–3000 Hz on the real unit; the emulator's boundary rejects it by ≥ 60 dB. The device's DAC leaves imaging residue above 3450 Hz at about −35 dBm0 (S-005 out-of-band spurious); the emulator's boundary removes it.
- Both are recorded in `signal-chain.md` (rows 4 and 10) and the README as known deviations that make the emulator slightly cleaner at the top of the band than the device, reversible later as a stage-4/10 realism option. The speech codec (a later feature) will dominate the top of the band in any case.
