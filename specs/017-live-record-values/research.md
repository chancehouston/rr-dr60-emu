# Research: Live Record-Side Values (017)

Each entry: decision, rationale, alternatives. Code references are to `main` at the 0.16 merge (`06e40b0`). The spec's reviews (coach 1 – 17, hardware B1 – B11, product M1 – M8) are applied in the spec; the plan reviews are recorded in plan.md when they land.

## R-01: Where the record-side state lives

**Decision.** `RecordPath` (`paths.rs:450`), the object both record-side streams are (the recorder's `recorder` and the one-pass's `one_pass.rec`), gets a `RecordCtl`: the **input clock** (`input: u64`, host samples processed), a `Schedule` (spec 015's, 64 entries), the cached `next_apply`, the stream's **last applied** record-side values (the eight, as `RecordValues { sensitivity, threshold_dbfs, hang_ms, onset_ms, knee_dbfs, attack_ms, release_ms, hiss_offset_db }`), a `fresh` flag (no input sample processed since construction, `reset` or `start_recording`), a `landed` record (the last landed value and tag per control in the call) and a staging log of up to 64 entries (no plain setters on this side). Spec 015's `StreamState` in `PlaybackPath` is unchanged; the one-pass therefore carries two clocks and two schedules, one per path (spec D2). `repr(C)` with the hot fields (`input`, `next_apply`) first and the arrays last (the 015 lesson).

**Rationale.** The record stages (`mic`, `limiter`, `half.vas`) live in the path, the apply point is inside its per-sample `step`, and the recorder, the stop's feed and the one-pass's tick all go through it, so one implementation serves every record-side call, the front taps included (`front_only`).

**Alternatives.** The record-side schedule on the `Pipeline` (rejected: the apply point is in the path's per-input-sample step); a generic per-clock state shared with `StreamState` (rejected for this spec: the two sides' applied values differ in type; a later refactor if 020 wants it).

## R-02: The apply point: the top of input sample P

**Decision.** In `RecordPath::step` and `front_only`, **before** `sanitize_in`/`front`: `if self.ctl.input >= self.ctl.next_apply { self.apply_due() }`, then the sample is processed and `input += 1`. `apply_due` (cold) pops every entry with position ≤ `input`, applies it to the stage it names (R-03 – R-05), logs it (before = the stream's last applied value, after, tag), updates `last`/`landed`, and re-arms `next_apply`. Because the gate processes nothing between host samples, applying the gate's values at the top of input sample P makes the first device sample that sees them exactly k(P) = ⌈P · 6000 ÷ rate⌉ (the decimator emits device k when host ⌊k · rate ÷ 6000⌋ is pushed, `down.rs:53`: k(P) is the smallest k whose due index ≥ P), with no device-side apply point (hardware B6; the mirror of spec 015 R-02's speed argument).

**The cross-stream pending (spec D4).** The `Pipeline` keeps the readings in `self.settings` (the eight fields) and `record_tags: [u64; 8]` (the tag of the last landing per control). At every record-side call's entry (`record`, the stop's feed, `process*`) the path's `begin_call(&settings, &tags)` compares `self.settings`' eight values with the path's `last`: each difference becomes a pending change at the current input position with the landing's tag, applied at the first input sample (so a value landed on the recorder reaches the one-pass at its next input sample and is logged with the same tag; the recorder is reset by `start_recording`, whose `set_start_values(&settings)` makes it start with the settings' values, no entry). `reset` and `reconfigure` set both paths' `last` to the settings' values.

**Alternatives.** Applying inside `RecordHalf::process` at the device rate for the gate's values (rejected: a second apply point and a device counter, the hole 015 found for the speed); a per-stream settings reading (rejected by D4).

## R-03: The limiter's retune

**Decision.** `Limiter::derive(settings: &AgcSettings, host_rate_hz) -> Derived { vbe6, charge_gain, dis_r11, dis_b5, ceiling_fs }` is factored out of `new` (`limiter.rs:188`) and used by `new` and by `retune(&AgcSettings)`, which writes those five fields and leaves `vc`, `vc_max`, `v_floor`, `v_limit`, `ln_k1`, `dt_over_c6` untouched. A knee change moves `vbe6`, `ceiling_fs` and `charge_gain` (through `beta6_for(knee, attack)`); an attack `charge_gain`; a release `dis_r11`, `dis_b5` (hardware B5). The gain law `a(vc)` and spec 016's `charge()` are therefore continuous across a retune; `vc_target` uses the new ceiling. `overdrive_index(KNEE_REF_DBFS)` is a constant computed once. Cost about 5 µs (two 360-point indices); reported as the worst landing (FR-008, FR-009).

**Rationale.** A change at input 0 then equals construction bit for bit (the same arithmetic), and a mid-call retune keeps the 016 meter's running `vc_max`.

## R-04: The gate's values

**Decision.** `VasStage::set_values(settings: &VasSettings)` recomputes `threshold` (the formula of `new`, `vas.rs:82`: `exp(level_db · ln10 / 20)` with `level_db = threshold_dbfs + 3 · (3 − sensitivity)`), `hang` and `onset` (`device_samples`), and keeps `state`, `silent`, `run`, `since_sound`. The next `process` compares the kept counts with the new lengths (hardware B4: a silent count past the new hang pauses at the next non-sound sample, a sound run past the new onset resumes at the next sound sample). `hang_samples()` reads the new hang, so spec 016's `hang_remaining` (`hang.saturating_sub(silent)`) reads 0 while the count exceeds it.

**The event bound** (hardware B3): `RecordPath::max_events` takes `min(current hang, every pending hang change's device_samples)` over the schedule's ≤ 64 entries (at the query, no allocation) in place of `self.half.vas.hang_samples()` (`paths.rs:728`).

## R-05: The hiss transition

**Decision.** `MicCapsule { gain, gain_from, gain_to, ramp_k, ramp_n, inv_n }` with `set_offset(db, at_once: bool)`: `gain_to = σ · 10^(db/20)` (the formula of `new`, `mic.rs:88`); at once → `gain = gain_to`, `ramp_k = ramp_n`; else `gain_from = gain` (the gain at this sample, a retarget included), `ramp_k = 0`. In `process`, before the draw: `if ramp_k < ramp_n { ramp_k += 1; gain = if ramp_k == ramp_n { gain_to } else { gain_from + (gain_to − gain_from) · s(ramp_k · inv_n) } }` with `s(u) = u²(3 − 2u)` (spec 011's `ramp_step` form), N = `volume_transition_samples(rate)`. The idle path is one predictable compare; the draws (`gauss()`) are untouched, so the stream is the same under any gain (hardware B9). The at-once rule: `RecordCtl.fresh` (no input sample processed since construction, `reset`, `reconfigure`, `start_recording`) or the capsule bypassed (`MicFront` has no capsule: the value is logged, nothing moves).

**Rationale.** D3, D9; the 011 convention assigns the target exactly at sample N (coach 1).

## R-06: Validation at the schedule call

**Decision.** `schedule_change` (spec 015, `pipeline.rs`) gains the eight controls: by the control manifest's index (`controls::index_of`), the stream must be `Record` or `OnePass` (else `ControlNotOnStream`); the value is checked against `Setting::range` for the setting (finite, inclusive; the sensitivity also integral) and refused with `InvalidSetting { setting }` as `validate` would; stored as the setting's type (`f32`; the sensitivity `u8`) widened to the entry's `f64`. The eight controls are the only ones with the record path and a numeric value whose timing is live; everything else keeps `ControlNotOnStream`.

## R-07: The stop, the self-stop and the clock

**Decision.** `Transport.fed_input: u64` counts every host sample `feed_one` receives (the input and the stop's zeros); it is the record stream's input position (`input_position(Stream::Record)`; 0 while idle). `stop_recording` applies pending changes during `feed_zeros` as during `record` (the same path code), then clears the schedule; `StopOutcome` gains `input_samples_total: u64` and `changes: usize`; `stop_recording_with_changes(events, changes)` drains the staged entries (a change inside the fed silence is logged; one at or past the total is dropped). The self-stop (`feed_one` returning `Some(stopped)`) clears the schedule; the `record` call's entries that landed before the stopping sample are reported (`RecordInfo.changes`). A refused `start_recording` leaves the schedule; `start_recording` keeps it and resets the clock.

## R-08: The one-pass's two schedules and the log

**Decision.** `schedule_change(Stream::OnePass, …)` routes a record-side control to `one_pass.rec.ctl.schedule` and a playback-side one to `one_pass.play.ctl.schedule`; `pending_changes(OnePass)` is the sum; `clear_schedule(OnePass)` clears both; `input_position(OnePass)` reads `one_pass.rec.ctl.input`. The log-taking one-pass variants drain the record path's staged entries first, then the playback path's (each in landing order); `BlockInfo.changes` is the sum; `MAX_CHANGES_PER_CALL` becomes 64 + 64 + 3 = **131** (the Rust and C constants; the 015 tests that pinned 67 are restated).

## R-09: The clock helper and the landing helper

**Decision.** `changes::Clock { Input, Output }` and `changes::clock_of(control: u32) -> Result<Clock, Error>` (Input when the control's manifest paths include `RECORD`, Output otherwise; `NoSuchControl` past the list); `changes::record_landing(position, host_rate_hz) -> Result<u64, Error>` = ⌈P · 6000 ÷ rate⌉ (`UnsupportedHostRate` otherwise); C `rr_dr60_clock_of` (an enum `RrDr60Clock`) and `rr_dr60_record_landing`.

## R-10: The oracles

| Oracle | Form | Where |
|---|---|---|
| The input-0 identity (every value, both streams, seven rates, both modes) | a fresh stream with the change at 0 against `Pipeline::new(settings with the value)`: bit-identical stored samples / one-pass output | `us1_record` AS1 |
| The pre-landing identity | codec off: every post-VAS device sample before k(P) (read through the recorder's stored samples, or the one-pass at `Tap::AfterVas`) equals the old run's; codec on: every stored frame whose window + 60-sample look-ahead ends before k(P) is byte-identical | `us1_record` AS2 |
| The partition identity | block sizes 1, 64, 4096 and `changes_checks::IRREGULAR` | `us1_record` AS2, `record_matrix` |
| The reference gate | spec 003's data model (the 016 `ReferenceGate`) given the device samples and the new lengths from k(P) | `us1_record` AS4, AS5 |
| The limiter | 2 s after a knee change the attenuation (the AGC-off twin ratio) equals the new constant run's within 0.1 dB; the first dead time / release time after an attack / release change equal the new run's within spec 005 FR-011's tolerances; `limiter_state` continuous at P | `us1_record` AS3 |
| The hiss | at `Tap::AfterMicCapsule`, amp off, silent input: ratio to the old run within 2 ULP of the smoothstep for k = 1 … N − 1; bit-identical to the new run from P + N; whole engine: the band level 1 s after P within 0.1 dB | `us1_record` AS6 |
| Record-then-play equals the one-pass with the same schedule | spec 008 FR-009's rule; the stop's `silence_fed_samples` read after the stop | `us2_record` AS2 |
| The stop | changes inside the fed silence logged in the stop's report; at or past the total dropped; the final position reported; the self-stop clears | `us2_record` AS3 |
| The cross-stream landing | a value landed on the recorder reaches the one-pass at its next input sample with the same tag; `reconfigure(settings())` then equals a fresh engine on both | `us2_record` AS4 |
| The event bound | a 1000 → 50 ms hang landing mid-block with a splicing stimulus; every event written | `us1_record` AS8 |
| k(P) pinned independently | one-sample `record` calls: the device sample count after P calls equals k(P)'s rule (through the recorder's stored length with the gate bypassed) | `record_matrix` |

## R-11: Cost

**Decision.** One `u64` compare per input sample on the record path (the 015 pattern), the hiss's one compare per sample when the capsule is on, nothing per device sample; measured by `tests/record_timing.rs` (the 0.16 API: a 10 s recording through `record` and the one-pass front, 48 kHz, fastest of 45) in a `record` mode of `scripts/ab-timing.sh` (3 %; `RR_DR60_RECORD_COST_CEILING` 0.05 on the label job), and `tests/record_pending_timing.rs` (head-only: one knee change per 4096 block; the worst block) reported.

## R-12: The golden and the C parity

**Decision.** `golden-record-changes-v1.json`: scenarios per value on both streams (a change at 0, a change inside a block), the hang shortening, the stop case (a change in the fed silence), the cross-stream landing, in `default_output` (and the codec-on default for the frame identity), drop and mute, seven rates; per entry the output digest and the full log (f64 as `{:?}`); blessed once after the input-0 and partition identities hold on every entry. The C smoke `--record-changes` against `examples/record_changes_rows.rs`.

## R-13: The records

The control manifest: the eight entries `timing: Live` (`controls.rs`), the canonical re-bless (`RR_DR60_BLESS=control_manifest`), the 014 tests' timing expectations restated; the guide's section gains the record side (records.md's paragraph), the real-time list the new calls; the CHANGELOG 0.17.0 (the 131 change recorded); the product notes of records.md; the stage manifest's version line only.

## R-14: The mutants

`RecordMutation { apply_after_front (the apply after the sample: lands one sample late, caught by the input-0 identity), reset_vc_on_retune (caught by the continuity assert), reset_counts_on_hang (caught by the reference gate), step_hiss (no transition: caught by the hiss ratio), landing_floor (k(P) as ⌊P · 6000 ÷ rate⌋: caught by the k(P) pin), skip_other_stream (no cross-stream pending: caught by US2 AS4) }` behind `__test-hooks`.
