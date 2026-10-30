# Research: Battery State (018)

## R-01: The control's plumbing

**Decision.** `battery_volts: f32` is a `Pipeline` field beside `route`, `volume` and `speed_level`, default `battery::DEFAULT_BATTERY_VOLTS` (3.0), kept by `reset` and `reconfigure` (carried into `with_memory`). `set_battery_volts(v)` validates (finite, within `BATTERY_VOLTS_RANGE` = 2.2 … 3.0 inclusive; `Error::InvalidBatteryVolts { volts }`, C status 30) and stores the value; like the volume it lands at the first sample of each stream's next producing call through spec 015's `StreamState::begin_call` (a fourth pending plain value) and is logged with tag 0 when it moved. `schedule_change` accepts `LIVE_BATTERY` (the control manifest's index of `battery_volts`, pinned by a test) on the playback and one-pass streams in the output clock, the value narrowed to f32 (`as f32 + 0.0`) and validated as the setter does; `Apply::Battery(f32)` reaches the chain's `set_battery(b)`. `start_playback`, `reset` and `reconfigure` take it as a start value (no entry). The readings follow landed values (`landed_battery`), so `battery_volts()` reads the most recently landed value on any stream.

**Rationale.** D1; the volume's path is blessed by `golden-changes-v1.json`, so every spec 015 oracle (the block-size-1 setter oracle, the partitions, the chaining) applies unchanged.

**Alternatives.** A setting (rejected: live). A per-stream reading (rejected by 015 D5).

## R-02: The transition on the factor

**Decision.** `OutputChain` gains `batt: f64` (the factor in force), `batt_from`, `batt_to`, `batt_k: u32`, `batt_n: u32` (= `ramp_n`, the volume's N) and `batt_inv_n`. `set_battery(b)`: at once (`batt = batt_from = batt_to = b; batt_k = batt_n`) when the chain is fresh (the stream's first sample, spec 011's rule), when stage 11 is bypassed, when the route is the earphone, and at a route change (`reset_sections` marks the chain fresh, so the next `set_battery` is at once; a transition in progress ends at the switch by `batt = batt_to; batt_k = batt_n`); otherwise `batt_from = batt; batt_to = b; batt_k = 0`. Per speaker-route sample, before the knee: `if batt_k < batt_n { batt_k += 1; batt = if batt_k == batt_n { batt_to } else { batt_from + (batt_to − batt_from) · s(batt_k · batt_inv_n) } }` with s the smoothstep: the landing sample is transition sample 1, sample N exactly `batt_to`, no hold (b is interpolated directly, monotonic). The volume's `ramp_k` is untouched; the two transitions are independent.

**Rationale.** D4; hardware B4 of the spec review (a shared scalar under the running hold could freeze or reverse).

**Alternatives.** Folding 1/b into `k_spk` (rejected, B4). An instantaneous step (rejected, clicks on clipped material).

## R-03: The scaled clipper

**Decision.** `knee_b(x, k, b)`: `u = |x|`; `if u <= k·b { u } else if u >= (2 − k)·b { b } else { d = (2 − k)·b − u; b − d·d / (4·(1 − k)·b) }`, the sign restored. At b = 1.0 every branch reduces to `knee`'s (`k·1.0 = k` exactly, `(2 − k)·1.0` exactly, `4·(1 − k)·1.0` exactly, `1.0 − d²/…` identical), so the default is bit-identical: a core test asserts `knee_b(x, k, 1.0).to_bits() == knee(x, k).to_bits()` over a sweep. The thresholds `k·b` and `(2 − k)·b` and the parabola's denominator are recomputed when `batt` changes (per transition sample, three multiplies), stored in the chain. Below `k·b` the sample passes untouched: FR-002's identity. The speaker branch applies `knee_b(k_spk · amp_hpf(u) + 0.0, k, batt)`; the earphone branch and every earlier stage are untouched (FR-004).

**Rationale.** D3; coach 4 and hardware B4 (scaling the sample breaks the identity).

**Alternatives.** `knee(x / b) · b` (rejected: not bit-identical below the knee; two multiplies per sample). The output scale following the ceiling (rejected by D3).

## R-04: The law and the public data

**Decision.** `pub mod battery` in the core: `pub fn ceiling_factor(volts: f32) -> f32 { (volts − 1.8) / 1.2 }` computed in f64 and narrowed (exactly 1.0 at 3.0: `(3.0 − 1.8) = 1.2` exactly in binary? 3.0 − 1.8 = 1.2000000000000002 in f64; the design must pick the form that is exactly 1.0: compute as `(volts − 1.8) / 1.2` in **f32** → (3.0f32 − 1.8f32) / 1.2f32: 3.0 − 1.8 = 1.2 (f32: 1.2000000476837158 − … must be checked); the core pin decides the form: the research task of the implementation is to find the arithmetic form that gives exactly 1.0 at 3.0 and the design script's figures within 1e-9 elsewhere, candidates `(v − 1.8) / 1.2` in f64, `(v − 1.8) * (1.0 / 1.2)`, `((v − 3.0) / 1.2) + 1.0` (exactly 1.0 at 3.0 by construction: v − 3.0 = 0, 0 / 1.2 = 0, 0 + 1 = 1); the last is recommended as the committed form (it is exactly 1 at the default whatever the rounding elsewhere, and equals the others within 1 ULP); the chain stores the factor as f64), `pub fn ceiling_db(volts) -> f32 = 20·log10(ceiling_factor)` through `rr_dr60_detmath::ln` (deterministic), `pub const ALKALINE_TRAJECTORY: [(f32, f32); 7]` (hours, volts) from A-056, and `BATTERY_VOLTS_RANGE: (f32, f32) = (2.2, 3.0)`, `DEFAULT_BATTERY_VOLTS: f32 = 3.0`. The design script's `--battery` section prints the factor and dB at the six law points and the seven trajectory rows (unrounded) and writes the trajectory constant into `output_coeffs.rs`; the core pins read both. C: `rr_dr60_battery_ceiling_factor(volts, *out)`, `rr_dr60_battery_ceiling_db(volts, *out)` (`INVALID_ARGUMENT` outside the range or not finite), `rr_dr60_battery_trajectory_row(i, *out_hours, *out_volts)` (`INVALID_ARGUMENT` at or past 7), `RR_DR60_BATTERY_TRAJECTORY_ROWS 7`, `RR_DR60_BATTERY_VOLTS_MIN/MAX/DEFAULT` as double defines.

**Rationale.** D2, D11; hardware B2 (the exact-1.0 form), product M3.

## R-05: The meters

**Decision.** `Meters` gains `amp_clip_count: u32` (speaker-route host samples of the call whose knee input `|x| ≥ k·b`, counted where the knee is applied) and `battery_ceiling: f32` (the factor in force at the call's end; rest 1.0); `RrDr60Meters` grows 56 → 64 bytes at the end (`amp_clip_count` at 56, `battery_ceiling` at 60), a 0.17 caller's `struct_size = 56` filled with the first 56 bytes as spec 016's rule says (smaller refused below 56? spec 016: smaller than the library's size is refused; larger zero-filled). **Restated here:** spec 016's rule is "smaller refused, larger zero-filled", which would refuse a 0.17 caller's 56; the plan keeps the rule as spec 016 wrote it (the C struct is pre-1.0 and the ABI pass is queued), and the CHANGELOG says so: a 0.17 caller recompiles. `rest_meters` carries `battery_ceiling: 1.0`. Finalization: `PlaybackPath::finalize_into` reads the chain's accumulators (count cleared, the factor kept). At the default with below-knee material both rest; `golden-meters-v1.json`'s rows are the eleven figures as printed by `row`, so the row format gains the two fields and the meters golden is **re-blessed once** (its digests unchanged; the rows gain two columns) — or the row printer prints the two only when non-rest. **Decision:** append the two columns and re-bless the meters golden once (recorded; the audio digests are unmoved).

**Rationale.** D10, FR-010; product M1.

## R-06: The manifest and the registers

**Decision.** `controls.rs`: `battery_volts` ("Battery voltage", UnitCondition, Live, paths [PLAYBACK, ONE_PASS], stage 15, continuous(2.2, 3.0, 3.0), unit "V", sources ["S-002", "S-006", "S-017"], assumptions ["A-056"], the sentence "Sets the modeled unit's cell voltage, which lowers the speaker amplifier's clip ceiling as the cells run down; set by the user, never read from the phone."); `CONTROL_COUNT` 30; the setters check finds `set_battery_volts` / `rr_dr60_set_battery_volts`. `manifest.rs`: stage 15 status `Assumed`, confidence low, model version 0.18.0, its controls note `battery_volts; set_battery_volts, battery_volts; Meters`, its coverage note the rails' sequencing etc. unchanged plus "the cells' sag under load" ; the header's counts 2 / 13; stage 11's coverage note "the clip level's transistor spread; the battery's sag under load" (the pinned word "battery" kept; the pinning test restated to the new text); S-017 cited by stage 15 (already) and the control. `assumptions.md`: A-056's status "adopted by spec 018 (2026-10-30; the static law)" with the sag paragraph kept as not modeled and the knee-shape alternative added; A-031 gains "the ceiling follows the battery value since spec 018 (A-056)". `signal-chain.md` rows 11 and 15 updated. The canonical texts (`manifest-v1.txt`, `control-manifest-v1.txt`), the README table and the guide's blocks re-blessed once each.

**Rationale.** D7, FR-008, FR-009; coach 2 and 10, hardware B9.

## R-07: The oracles

**Decision.** `battery_checks.rs`: `law_points()` = [3.0, 2.8, 2.6, 2.4, 2.2] plus the default twin; `trajectory()` from `battery::ALKALINE_TRAJECTORY`; `clipped_tone(rate)` a 1 kHz tone at 0 dBFS from `stimulus::peak_phase`, 1.5 s, run at `VOLUME_6DB_OVER` on the speaker route through `output_only_amp`'s configuration at `Tap::AfterVolumeAmp` (stage 11 alone); `peak_after_settle` the AES17 amplitude over the last second (spec 009's lesson); the oracle `peak == b` within 1e-6 and `20·log10(peak)` vs the trajectory's dB within 0.05; `below_knee_identity(rate, volts)`: the −30 dBFS tone and the −20 dBFS voiced at the reference position, the whole default chain, bit-identical to the default run; `knee_table_shift(rate, b)`: spec 009's knee rows at level + 20·log10(b) re-measured, 44.1 kHz and above; `factor_trace` through a hook `__battery_factor_trace` returning the chain's `batt` per produced sample (sample ⌊N/4⌋ = b₀ + 0.15625·Δb; monotonic; exactly b₁ at N); the splice identity (two constant runs); partitions through `changes_checks`; the other-paths identity (earphone route, the record path's stored samples at the six points); the clamp matrix row (route peak and clamp count on a full-scale square at 3.0, 2.6, 2.2 V); the click energy reported (the RMS of the difference between the scheduled run and the new constant run over the transition, on the clipped tone).

**Rationale.** D5; coach 1, 7, 8, 9; hardware B1, B3.

## R-08: Cost

**Decision.** `tests/battery_timing.rs` against the 0.17 API (the default front, codec bypassed, speaker route, 48 kHz, fastest of 45: `speaker_ns=`), a `battery` mode in `scripts/ab-timing.sh` (3 %, `RR_DR60_BATTERY_COST_CEILING` 0.05 on the label job), and `battery_pending_timing.rs` head-only (the chain at 2.4 V on a clipped square: `clipped_ns=`, reported). The per-sample work at the default: the knee's three constants are already in place; `knee_b` reads `k·b` etc. from fields instead of computing `k` and `2 − k` (one load each); the transition compare `batt_k < batt_n` per speaker sample. Expected under 1 %.

**Rationale.** D6, FR-007.

## R-09: The golden, the C parity and the records

**Decision.** `golden-battery-v1.json`: 6 law points × 2 routes × 3 stimuli (the clipped tone at `VOLUME_6DB_OVER`, the −20 dBFS voiced at the reference position, the full-scale square at position 1) × 7 rates = 252 entries, each the one-pass output digest at the default output configuration with the battery set before the stimulus (a plain setter; a scheduled mid-stream change in one extra scenario per rate with its log); at bless time the default entries' digests must equal the twin without the setter and the below-knee entries must equal the default's. The C smoke `--battery`: the clipped tone at 48 kHz at three values, the meters' two new fields, the law at the six points and the seven rows printed, diffed against `examples/battery_rows.rs`. Records: see records.md.

**Rationale.** FR-005, FR-010 – FR-012.
