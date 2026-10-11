# Research: Limiter-Type AGC, ADC Clamp, VAS Threshold (feature 005)

Decisions for feature 005. They revise [spec 002's research](../002-agc/research.md) (R-01 placement, R-02/R-03 detector, R-05 smoother, R-11 measurement method) and [spec 003's research](../003-vas/research.md) R-03 (the input-referred threshold). Every number below comes from the committed reference model `tools/agc-model/limiter_model.py` (R-04), run on 2026-10-11, unless a source is cited. The transistor-level parameters are estimates (A-028, A-034, A-035, A-036); the component values are the schematic's (S-002).

## R-01 Placement: the limiter runs at the host rate, before the decimator (FR-001; spec Clarifications)

- **Decision**: `Pipeline::process` applies the limiter to each host sample before `down.push(x)` (and before the identity path's direct copy). With `Tap::AfterAgc`, the output is the limiter's output sample for sample, the rate converters are not run, `latency_samples()` returns 0 and `max_events` returns 0. `DeviceChain` loses the AGC; its order becomes record → clamp → VAS → playback. The limiter runs whether or not later stages run; with `agc.enabled = false` it is skipped without arithmetic (bit-exact bypass).
- **Consequences**: spec 002 R-01 ("the AGC runs at the device rate") is reversed. The detector sees everything the host rate carries; the device's 72 Hz–13 kHz mic-amp bandwidth is not modeled (R-14). The sample-peak reading at the identity rate (004 R-08) is a model property; the harness uses aligned-phase tones.
- **Rationale**: the reviewer's three reasons (pre-filter detector; 6 kHz loop over-attenuation; exact gain observable everywhere) and the user's decision.
- **Alternatives**: device-rate placement (rejected: Clarifications); both (rejected: one tap).

## R-02 The model: the circuit's control loop in the control-voltage domain (FR-003 – FR-007)

- **Decision**: one state variable, the hold-capacitor voltage `vc` (volts), and these per-sample steps for host sample `x` (full-scale units, 1.0 = `V_FS` = 0.7 V peak at GSX, A-030):
  1. **Shunt gain** `a(vc)`: `ib4 = IS · exp(vc / (2 · VT))` (Q5 and Q4 base–emitter drops in series), `g = (BETA_R + 1) · ib4 / VT` (saturated-transistor conductance), `r = 1 / g`, `zn = r · R10 / (r + R10)`, `a = (zn / (R7 + zn)) / (R10 / R_IN)` (the divider R7 → node → R10 into the codec amp, relative to its unloaded value). `a` is exactly 1.0 when `vc` is below the turn-on floor (R-03).
  2. **Output** `y = a · x` (no multiply when `a == 1.0`).
  3. **Detector view**: `gsx = clamp(y · V_FS, ±V_RAIL)` (the codec input amp's swing limit), sensed polarity `pol` (A-036: +1), `over = KDIV · pol · gsx − VBE6` in volts.
  4. **Charge**: if `over > 0`: `ic = min(BETA6 · over / RTH, (V_REF − vc) / R12)` (current-gain limited, then collector-resistor limited), else 0.
  5. **Discharge**: `idis = vc / R11 + ib4 / BETA5` (R11, plus the follower's base current).
  6. **Integrate** `vc += (ic − idis) · dt / C6`, floor at 0, then the **cap** (R-03).
- **Constants** (data-model.md has the table with traces): `C6` 1 µF, `R11` 10 MΩ, `R12` 390 Ω, `R13` 100 kΩ, `R17` 330 kΩ (so `RTH` = 76.7 kΩ, `KDIV` = 0.767), `R7` 22 kΩ, `R10` 1 kΩ, `R_IN` 23 kΩ, `V_REF` 1.6 V: S-002. `VT` 26 mV, `IS` 1e-14 A, `BETA_R` 5, `BETA5` 300: transistor estimates (A-028). `V_FS` 0.7 V: A-030. `V_RAIL` 1.2 V and `VBE6` 0.6 V: estimates (A-028; the rail limits the detector's overdrive and makes the dead time saturate above about +5 dB; the Vbe sets the knee). `BETA6` is derived from the attack setting (R-05).
- **Why the dynamics come out as the spec says**: the exponential law maps only about 0.1–0.25 V of `vc` to gain (0.05 dB/mV near −3 dB, 0.16 dB/mV below −25 dB), so (a) `vc` must climb through about 0.93 V (two Vbe's) before `a` drops 1 dB: the **dead time**; (b) while `vc` sits between 0.5 and 1.1 V the loop is **armed** and R11's 10 s decay keeps it there for seconds; (c) R11's 0.1 µA plus Q5's base current discharge the 0.1–0.25 V that matter in about 1–2 s: the **release**, fast at first and slower near unity because the law flattens as the shunt's resistance passes R10. The **ceiling** is where the detector stops conducting: output peak = `VBE6 / KDIV` = 0.782 V = +0.97 dBFS ≈ the knee setting.
- **Alternatives**: a dB-domain one-pole smoother (spec 002 R-05): cannot produce a dead zone or arming, and its release is exponential in dB (rejected, reviewer D2). A peak-hold-plus-ramp "limiter" with tuned dead time and release constants: four tuned numbers with no circuit trace (rejected: Principle II).

## R-03 Discretization at the host rate: the charge cap, exact passthrough, flush-to-zero (FR-003, FR-005, FR-014)

- **Problem**: at 6 kHz, `dt` = 167 µs and the current-limited charge can raise `vc` by up to 0.17 V in one sample, which on the exponential law is 8–26 dB of attenuation: a plain Euler step over-attenuates (the reviewer measured −33 dB on a +10 dB step at 6 kHz; correct at 48 kHz).
- **Decision**: after the Euler step, when the sample charged (`ic > 0`), cap `vc` at the voltage at which this sample's amplitude would no longer exceed the threshold: `a_t = (VBE6 / KDIV) / (|x| · V_FS)` (clamped to ≤ 1), then the closed-form inverse of `a(vc)`: `zn_t = a_t · (R10 / R_IN) · R7 / (1 − a_t · (R10 / R_IN))`, `r_t = zn_t · R10 / (R10 − zn_t)`, `ib4_t = VT / ((BETA_R + 1) · r_t)`, `vc_t = 2 · VT · ln(ib4_t / IS)`; `vc = min(vc, vc_t)`. This is what the continuous loop does within a sample (conduction stops when the output falls to the threshold), and since the peak sample's target is the highest of any sample's, `vc` never exceeds the static value for the largest peak seen: **no over-attenuation at any host rate** (the FR-005 guard). One `ln` per charging sample.
- **Exact passthrough**: `a(vc)` is set to exactly 1.0, and the multiply skipped, whenever `vc < V_FLOOR`, where `V_FLOOR` is the voltage at which the shunt's effect is below 1e-9 (about 0.55 V; a derived constant, computed at construction with detmath). Below the knee the detector never conducts, so `vc` stays 0 and the output is the input bit for bit (FR-003). After a loud sound `vc` decays exponentially; once below `V_FLOOR` the gain is exactly 1.0 again, and once below `V_FLUSH` (1e-6 V, engineering target) `vc` is set to 0.0, so a pipeline that has recovered is bit-identical in state to a fresh one (reset and partition tests; 001 R-05's flush rule).
- **Grid effects** (model properties, documented): at 6 samples per cycle the detector sees 1–2 above-threshold samples per cycle, so the dead time is quantized to about ±0.17 ms and depends on phase; the harness uses the 004 R-08 aligned phase. At 48 kHz and above the model reproduces the 192 kHz reference within 0.1 ms.
- **Determinism**: `exp` and `ln` from `rr_dr60_detmath`; all other operations are basic IEEE. The state is one `f64`. Same input, settings and host rate ⇒ bit-identical on every target.

## R-04 The reference model and the numbers (A-028, A-034, A-035, A-036)

- **Decision**: `tools/agc-model/limiter_model.py` (pure Python, 192 kHz Euler, the same equations as R-02) is committed and prints the design figures. A core unit test pins the Rust model's figures to it within the spec tolerances. Output on 2026-10-11:
  - **Static curve**: gain 0.00 dB up to +1 dBFS input; −1.02 dB at +2, −2.02 at +3, −5.01 at +6, −11.01 at +12, −19.00 at +20 dBFS; output peak +0.98 … +1.00 dBFS throughout (ratio effectively unlimited); **knee (−1 dB point) +1.98 dBFS**. Spec FR-004: +2 ± 2 dBFS, ceiling ±0.5 dB.
  - **Dead time** from a discharged state, step from −20 dBFS: at `BETA6` 300: 50.5 ms (+2 dB), 3.55 ms (+5), **2.37 ms (+10)**, 1.58 ms (+18); at 150: 116 / 7.5 / 4.41 / 3.43 ms; at 600: 24.5 / 1.61 / 1.36 / 0.68 ms. **Armed** (`vc` = 1.0 V): 0.01 ms. The attenuation reaches its steady value within the same cycle and never exceeds it. Spec FR-005: 3 ± 1.5 ms at +10 dB; +2 dB ≥ 5× that; armed ≤ 0.5 ms.
  - **Release** after a 1 s burst: from 10 dB of attenuation, −1 dB after **1.37 s** (−9.2 dB at 0.05 s, −7.3 at 0.2, −6.2 at 0.3, −4.4 at 0.5, −2.9 at 0.75, −1.9 at 1.0); from 17 dB, 1.69 s. Spec FR-006: 1.3 s ± 40 %, −5.5 ± 2 dB at 0.3 s, −2.6 ± 1.5 dB at 0.75 s.
  - **Arming decay**: `vc` 1.1 → 0.5 V in 7.9 s (R11 × C6 = 10 s).
  - **Polarity**: the asymmetric stimulus with its larger peak at +4 dBFS gives −3.01 dB steady gain when the larger peak is on the sensed polarity and 0.00 dB when it is not: a **3.0 dB** difference, because the smaller peak (+1.5 dBFS) lies below the knee. Spec FR-007 said 2.5 ± 0.5 dB (the peak asymmetry); the plan corrects it to **3 ± 0.5 dB** (recorded in the spec's plan-review Clarifications).
- **Rationale**: Principle II: the numbers must be reproducible from the repo; the reviewer's scratch simulation is not committed.
- **Alternatives**: citing the reviewer's figures without a script (rejected); a Rust-only derivation (the Rust model is the implementation under test, so an independent reference is needed).

## R-05 Settings mapping (FR-010, FR-011)

| Setting | Model parameter | Mapping | Default → parameter |
|---|---|---|---|
| `knee_dbfs` | detector threshold `VBE6` | `VBE6 = KDIV · V_FS · 10^(knee / 20)` (the conduction onset) | +1.0 dBFS → 0.602 V (0.6 V nominal, A-028) |
| `attack_ms` (dead time at 10 dB over the knee) | detector current gain `BETA6` | `BETA6 = 300 · 2.37 / attack_ms` (dead time ∝ 1/β to within 15 % over 150–600, R-04) | 3.0 ms → 237 |
| `release_ms` (to −1 dB from 10 dB) | discharge scale `k_dis` on both discharge terms | `k_dis = 1300 / release_ms` (release ∝ 1/discharge current) | 1300 ms → 1.0 (R11 = 10 MΩ exactly; realized release 1.37 s, inside ±40 %) |
| `enabled` | — | skip the stage | on (A-020) |

- Ranges: knee −40 … +6 dBFS (below −40 the ceiling is near the VAS threshold and the model's rail limit no longer applies; above +6 the clamp dominates), attack 0.5 … 50 ms, release 200 … 10 000 ms. Validation as spec 002 FR-011 (finite, in range, named setting).
- The arming memory scales with `release_ms` (both discharge terms share `k_dis`), as FR-011 asks. The attenuation limit of 40 dB is a clamp on `vc` (the voltage giving −40 dB, derived at construction).
- **Alternatives**: calibrating `BETA6` by simulating the step at construction (deterministic but complex; rejected: the ±50 % FR-011 tolerance does not need it); expressing the attack as a charge rate (rejected: Clarifications).

## R-06 ADC full-scale clamp (FR-008; spec Clarifications)

- **Decision**: `stages/clamp.rs`, a stage with `process(d) = d.clamp(-1.0, 1.0)` when enabled (bit-exact pass-through when bypassed; `f64::clamp` is a basic operation and NaN never reaches it, spec 001 Edge Cases). It runs in `DeviceChain` after the record stage and before the VAS, under `Settings::adc_clamp_enabled` (default true). `bypass_all` and the spec 001 configurations: `bypass_all` sets it false; every other configuration leaves it true.
- **Golden-v1 verification (2026-10-11)**: from the committed `RECORD_SOS`, the record stage's impulse response peaks at 0.570 at the identity rate and its in-band gain is ≤ 1.0000036, so none of the spec 001 golden stimuli (impulse 1.0, tone −20 dBFS, sweep 0.25, noise ±0.5) exceeds 1.0 after stage 4 at any host rate (the decimator attenuates the impulse further). `golden-v1.json` stays byte-identical with the clamp on (FR-016); the SHA guard proves it.
- **Measurement**: max |y| of a +10 dBFS tone through `record_only` at 6 kHz equals 1.0 exactly; the first 3 ms of a +10 dBFS onset through the default pipeline contain samples at ±1.0; with `adc_clamp_enabled = false` the same tone's peak exceeds 1.0.
- **Not modeled**: the codec input amp's rail clip before the filter (about +4.7 dBFS, the `V_RAIL` estimate) in the signal path; it is used only in the detector's view (R-02). Documented in R-14.

## R-07 VAS threshold re-anchored (FR-012; A-022 revised; 003 R-03 revised)

- **Decision**: `VasSettings::DEVICE.threshold_dbfs = -40.0`; the 3 dB per step, hang, onset and all VAS logic unchanged. Harness: `BURST_ABOVE_DB` stays 10, so the burst-gap stimuli are at −30 dBFS; the VAS golden stimuli follow (R-10). `vas_checks::input_referred_threshold_dbfs` becomes the identity below the knee (the limiter's static curve is unity there), and the interplay report's text and floor sweep are rewritten: floors −70 … −35 dBFS RMS in 5 dB steps through the default pipeline at level 3, reporting how much of each gap is kept.
- **Chain scenarios** (FR-012; a new `tests/chain.rs`): (i) the speech-like pattern (three groups of four 300 ms bursts at −30 dBFS peak, 150 ms apart, 5 s gaps; the 003 interplay stimulus re-leveled) over a −60 dBFS RMS floor at level 3: no splice inside a group (events' positions outside the groups), each 5 s gap cut to the hang time ± 1 ms; (ii) over a −45 dBFS floor: no event at level 3; at level 1 (threshold −34 dBFS) the gaps pause (noise peaks ≈ −36.5 dBFS, 003 R-03's 8.5 dB peak factor); (iii) 30 s of a −60 dBFS floor then a +6 dBFS burst of 1 s: one event, resume after the onset, the burst's samples at the ceiling after the dead time (measured at the "after record" tap with the clamp: peak exactly 1.0).
- **Rationale**: spec Clarifications; A-029's level scale.
- **Alternatives**: 6 dB per step (recorded, not adopted).

## R-08 Measurement methods (FR-018; retiring 002 R-11)

- **Exact gain at the tap**: the `agc_only` configuration is `Tap::AfterAgc` with stages, clamp and VAS off; `y[n] / x[n]` where `|x[n]| ≥ 0.1 · A` (002 R-15's gate) at every host rate, no reference run, no boundary. `agc_checks::measure_timing` and the reference-ratio method are deleted; `check_ratio_self_test` is deleted.
- **Checks** (each a `MeasurementResult` with trace IDs):
  - `check_fr003_passthrough(rate)`: 1 kHz at −20 dBFS, speech-like bursts at −30, noise ±0.5, and a 5 s tone at knee − 1.5 dB: output bit-identical to input (differing samples == 0).
  - `check_fr004_static(rate)`: the static-curve set (−60 … +20 dBFS in 2 dB steps, 1 dB from −4 to +6), 5 s tones at the aligned phase, last second: output peak vs ceiling ±0.5 dB above knee + 1.5 dB; report rows within ±1.5 dB; `knee_bisection` to 0.1 dB: +2 ± 2 dBFS.
  - `check_fr005_attack(rate)`: step −20 → +10 dBFS after 20 s (the 20 s at −20 dBFS is replaced by a fresh pipeline, which is the same state by R-03): dead time = time of the first sample with gain < −1 dB, 3 ± 1.5 ms; ramp from −1 dB to within 1 dB of the final value ≤ 1 ms; max over-attenuation vs the final value ≤ 1 dB; +2 dB step dead time ≥ 5× the +10 dB value; armed: +10 dBFS burst 0.2 s, −20 dBFS 2 s, +10 dBFS: dead time ≤ 0.5 ms.
  - `check_fr006_release(rate)`: +11 dBFS 1 s then −20 dBFS 3 s: time to −1 dB 1.3 s ± 40 %; gain at 0.3 s and 0.75 s; the 17 dB case reported.
  - `check_fr007_polarity(rate)` (3 ± 0.5 dB, R-04) and `check_fr007_bandwidth(rate)` (rates > 12 kHz: 6 kHz tone at +6 dBFS plus a 1 kHz probe at −20 dBFS, probe level drop 5 ± 1 dB at the default output, and the default output's 6 kHz content ≤ −60 dB).
  - `check_fr008_clamp(rate)` (R-06), `check_fr009_thdn(rate)` (report rows at +6 and +12 dBFS), `check_fr011_settings(rate, case)` (knee −10 and +4 dBFS; attack 0.5, 10 and 50 ms; release 0.2, 4 and 10 s; the matrix at 6 and 48 kHz, the full matrix at seven rates in release mode), `check_fr001_latency(rate)` (AfterAgc 0; others equal with AGC on/off).
- **Phase rule**: every 1 kHz stimulus that feeds the detector uses `stimulus::aligned_phase` at the host rate with the bypass latency 0 at the tap (the tap has no decimator, so the rule reduces to π/6 at 6 kHz and to 0 … π/2 elsewhere; `aligned_phase(f, host, 0)`).
- **Mutation** (SC-007): `__with_limiter_params` test hook or settings-level mutants: knee +3 dB (static-curve ceiling row and golden), release × 2 (release row and golden), VAS threshold +3 dB (bisection row and golden-vas).
- **Retired**: the Hilbert FIR and its design script, `stages/mod.rs`'s Hilbert tests, 002's settings-matrix cases, `agc_checks::DETECTOR_DELAY_S`, the 004 R-09 "step at the aligned phase for the peak-hold branch" finding (no longer relevant; kept in the 004 research as history).

## R-09 Interface (FR-010, FR-015; contracts/)

- **Rust**: `AgcSettings { enabled: bool, knee_dbfs: f32, attack_ms: f32, release_ms: f32 }` with `DEVICE = { true, 1.0, 3.0, 1300.0 }`; `Settings::adc_clamp_enabled: bool` (default true, after `playback_stage_enabled` in declaration order; `#[non_exhaustive]` so adding a field is allowed); `Setting::AgcKneeDbfs` replaces `AgcTargetDbfs`; `AgcMaxGainDb` and `AgcMaxAttenuationDb` are removed; `AgcAttackMs` and `AgcReleaseMs` keep their names with the new meanings and ranges. `Tap::AfterAgc` docs: host rate, latency 0.
- **C**: `RrDr60Settings` keeps every offset: `agc_target_dbfs` (offset 36) becomes `agc_knee_dbfs`; `agc_max_gain_db` (40) and `agc_max_attenuation_db` (44) become `reserved_agc_1` and `reserved_agc_2` (ignored; `rr_dr60_settings_default` writes 0); `agc_attack_ms` (48) and `agc_release_ms` (52) keep their names; `adc_clamp_enabled` is appended at offset 72; `struct_size` = 80. `RrDr60SettingField`: `AgcTargetDbfs = 4` becomes `AgcKneeDbfs = 4`; 5 and 6 are retired (never returned; the C header keeps a comment); 7 and 8 keep their meaning. Validation ignores the reserved fields. A 72-byte struct is rejected as `STRUCT_SIZE` (as 003 did for 48). Version string 0.5.0.
- **Rationale**: Complexity Tracking row 3.

## R-10 Golden files (FR-016; contracts/golden-format.md)

- **AGC stimuli** (phase 0, 1 kHz, bit-reproducible): `agc_onset` (−20 dBFS 0.1 s, +10 dBFS 0.4 s, −20 dBFS 2.0 s), `agc_rearm` (+10 dBFS 0.2 s, −20 dBFS 2.0 s, +10 dBFS 0.2 s, −20 dBFS 0.6 s), `agc_staircase` (−20, 0, +2, +4, +8, +14 dBFS, 0.4 s each). Configurations `agc_only` (tap AfterAgc; host-rate, no conversion) and `default_agc` (all stages and the clamp on, VAS off, tap AfterPlayback). 3 × 2 × 7 = 42 entries.
- **VAS stimuli**: definitions unchanged except the burst level, now the threshold + 10 dB = −30 dBFS, and `vas_noise_gaps`' floor unchanged at −70 dBFS RMS. 63 entries. The `default_vas` configuration now includes the limiter and the clamp.
- **Procedure**: one commit with the CHANGELOG entry: `RR_DR60_BLESS=agc`, `RR_DR60_BLESS=vas`, update `GOLDEN_AGC_V1_SHA256` in `golden_agc.rs`; `GOLDEN_V1_SHA256` unchanged (R-06).

## R-11 Retirement and records (FR-013, FR-019)

- Register: A-007 → `verified in circuit (S-002); behavior in A-028`; A-017, A-018 → `superseded by A-028 / A-034 / A-035 (spec 005)`; A-019 → revised row (half-wave on positive samples, no look-ahead, host-rate bandwidth in the emulator, starts at unity); A-022 → revised (−40 dBFS at level 3; rationale from A-029; the 6 dB alternative); A-028 → adopted, reinterpreted (knee +1 dBFS ± 2 dB; R12 × C6 limits the peak current; R11 × C6 is the arming memory; the rail estimate; temperature); A-029 → refined (mic amp clips 7–16 dB above the knee; the SPL anchor); new A-034 (dead time), A-035 (release), A-036 (polarity); the 40 dB limit and the 13 kHz / 7 Hz omissions in A-028's row.
- Spec 002 header: "**Replaced by spec 005** (2026-10-xx): the compressor model of this spec (A-017 – A-019) is replaced by the limiter of spec 005; the text below is kept as the record of the earlier model." Spec 003 header: a note that the threshold default is restated by spec 005 (A-022 revised).
- CHANGELOG `### Changed`: the limiter (A-028 adopted; A-017/A-018 superseded), what a user hears, the level anchor, the clipped onset, the clamp, the changed tap, the removed settings and renamed C fields (breaking 0.4 → 0.5), the VAS threshold, the re-blessed files, the un-modeled items; `### Removed`: the Hilbert detector and script. README: the AGC section rewritten (what it does, the knee at about full scale, how loud input must be, the clamp, bypass, honesty caveat); fidelity table rows for the AGC and the ADC clamp. Signal-chain rows 3 and 4. `tools/filter-design/README.md` loses the Hilbert section; `tools/agc-model/` is described. CLAUDE.md status and commands. Issue template per FR-019.

## R-12 Performance and CI (SC-005, 001 FR-022)

- Per host sample with the limiter on: one `exp`, about 20 flops, one `ln` on charging samples only; the Hilbert FIR's 32 MACs per device sample disappear. Expected: faster than 0.4 (368× at 48 kHz). The `op-count` test's per-sample bound is restated. No CI workflow change; the 42 + 63 golden entries keep their sizes. Coverage: the new modules are fully exercised by the unit tests; the retired Hilbert tests are deleted.

## R-13 Engineering-target constants (FR-017; SC-008)

Labeled `// engineering target (005 FR-017)` or with the FR: the 1.5 dB knee margins; the ±0.5 dB ceiling and ±2 dB knee tolerances and the 0.1 dB bisection; the 40 dB attenuation limit; the FR-005 figures (±1.5 ms, ≤ 1 ms, 1 dB, 5×, 0.5 ms) and the 20 s / 2 s stimulus timings; the FR-006 figures (±40 %, ±2 dB, ±1.5 dB) and the 1 s burst; the 3 ± 0.5 dB polarity difference and the asymmetric stimulus; the 6 kHz / +6 dBFS bandwidth stimulus and 5 ± 1 dB; the static-curve set; the setting ranges; the FR-011 tolerances; `V_FLOOR` (1e-9 effect) and `V_FLUSH` (1e-6 V); the `BETA6` and `k_dis` mappings' reference points (2.37 ms, 1300 ms); the FR-012 chain scenarios' levels, timings and ±1 dB / ±1 ms; the mutation sizes; the golden stimulus levels and durations; the 5 minutes in SC-001. Circuit-derived constants cite S-002 (component values) or A-028 / A-034 / A-035 / A-036 (estimates); the level scale A-030 / A-029; the threshold A-022.

## R-14 Known deviations not modeled (FR-007, FR-019)

The detector's 13 kHz roll-off (C10/R15) and 7 Hz DC block (C5/23 kΩ): the emulator's detector sees the host input's full band and any DC offset. The codec input amp's rail clip in the signal path (about +4.7 dBFS): only its effect on the detector is modeled; the signal path is clamped at full scale after the filter instead. The knee's temperature drift (−0.3 dB per 10 °C). The mic amp's own clipping 7–16 dB above the knee (stage 2, out of scope). The transistor parameters are estimates: the dead time is uncertain by about 3× either way, the release by about 40 %, the knee by ±2 dB. Captures that settle them are listed in FR-019 and the issue template.

## R-15 Risks

- **The spec's figures are the model's**: if the Rust implementation and the reference script disagree beyond the tolerances, the plan is wrong, not the spec; stop and re-derive (the pinning unit test).
- **6 kHz grid**: the dead time is quantized to about ±0.17 ms and phase-dependent; the FR-005 tolerance (±1.5 ms) covers it with aligned-phase tones; arbitrary-phase input at the identity rate is a documented model property.
- **Chain scenario (ii)**: the −45 dBFS floor's peaks sit about 2.5 dB above the level-1 threshold; if the pause does not occur at level 1 within the 5 s gaps, the scenario's floor is re-leveled to −43 dBFS (engineering target), not the threshold.
- **VAS mute-mode and the clamp**: a muted region's ring-out passes through the clamp unchanged (it never exceeds 1.0); no interaction expected.
- **C callers of 0.4**: a 72-byte struct is rejected; the renamed field fails to compile. Both intended; recorded.
