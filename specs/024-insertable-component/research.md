# Research: An Insertable Component Slot and the Band Filter (spec 024)

Every figure below is an engineering target the spec states (D1 – D12) or a figure to be measured at implementation and reported; none is a device fact (the slot is an emulator convenience, D1). Every new literal carries an `engineering target (024 Dn)` tag for `scripts/check-traceability.sh` (0.45, 20 ms, ±24, 0.5 / 10 / 0.7071, 20 / 20 000, 16, 1e-30). The reviewers' simulated figures (hardware plan review B4, B5, B10, B12, B16; scripts in the session scratchpad) are reproduced in the harness before any record quotes them (spec 019's lesson).

## R-01 Where the slot sits (D1, D6, D8)

**Decision**: in `RecordPath::front_from` (`paths.rs`), after `self.mic.process(s)` and before `peak_limiter_input` and the limiter:

```text
x = mic.process(s)                      // stages 1 – 2 (f64)
if run_insert { x = insert.process(x) } // the slot; none: not entered, no arithmetic (−0.0 kept)
peak_limiter_input(x); limiter          // the meters read the slot's output (D6)
```

`run_insert: bool` goes into the 7 bytes of padding after `run_limiter` (the next field, `down`, is 8-aligned), so no existing offset moves; `insert: Insert` goes **after** `ctl` so the hot fields and `ctl`'s hot pair keep their offsets (spec 015's lesson); a core test asserts `offset_of!(RecordPath, down)` and `offset_of!(RecordPath, ctl)` unchanged. The empty path does no arithmetic at all (`x + 0.0` would turn −0.0 into +0.0). The stop's flush reaches the slot (`feed_zeros` → `push` → `step` → `front` → `front_from`). `FrontStop::Insert` serves `Tap::AfterInsert` (the one-pass reads stage 2 then the slot, no limiter); at `AfterMicCapsule` and `AfterMicAmp` the slot does not run; `Tap::is_front` includes the new tap; the C value 11 is appended. `RecordPath::reset` calls `Insert::reset` (R-06). The latencies are unchanged with the filter in (FR-013; asserted at seven rates for the record path and the one-pass).

**Rationale**: the host rate, the input clock and the transitions' grid come with the place; no conversion, no latency. One predictable branch is about 0.3 ns of about 70 ns per sample (hardware B13).

**Alternatives**: a `match` on an `Option` per sample (a second load); the slot inside `MicFront` (it would move the stage-2 tap's meaning).

## R-02 The section and its transitions (D3, D4)

**Decision**: the topology-preserving state-variable section (Simper/Zavalishin) in `f64`:

```text
g = sin(π f / r) / cos(π f / r)      (detmath; f the applied frequency, ≤ 0.45 r)
k = 1 / Q;  a1 = 1 / (1 + g (g + k));  a2 = g a1;  a3 = g a2
v3 = x − s2;  v1 = a1 s1 + a2 v3;  v2 = s2 + a2 s1 + a3 v3;  s1 = 2 v1 − s1;  s2 = 2 v2 − s2
b = k v1                              (the band signal at a 0 dB peak)
y = gain · (m0 x + m1 b + m2 v2)      (the type's constant mix triple, R-03)
```

`s1` and `s2` are flushed to 0.0 together when both are below 1e-30 in magnitude (the rule of `biquad.rs` and `first_order.rs`; hardware B14). `detmath` sin and cos are within 2 ulp, so `g` is accurate to about 1e-15 relative from 20 Hz at 96 kHz to the ceiling (cos 0.156): no effect on the oracle. The coefficients are recomputed only while a transition runs; at rest the section reads cached `a1, a2, a3, k, gain`.

**The transition**: a landing at input sample P starts, for its parameter, `n = 1` at P, `w = u²(3 − 2u)` with `u = n / N`, N = 20 ms of host samples (spec 011's `volume_transition_samples`), and sample P + N − 1 carries the target exactly (spec 019's lesson). The scale is the parameter's own: frequency in log of the **applied** frequency (both endpoints clamped to `0.45 × rate` at the landing and at `reconfigure`; the trace and the 1e-9 oracle on applied values; the entries and the record keep the requested value; a move wholly above the ceiling is logged with no audible ramp; hardware B11, coach 11), Q in log, gain in dB (`exp(dB · ln 10 / 20)`). A second landing within 20 ms restarts from the current value (spec 011). A landing applies at once while the slot is empty and on a stream's first input sample (spec 017's hiss rule); `reset`, `reconfigure`, `start_recording` and `set_component` end every transition at its target. The three continuous parameters ramp independently.

**Rationale**: the state-variable form over a direct form (spec D4; the harness's B1 row, R-07). Recompute only while ramping: at worst one sin, one cos and three exp per sample for 20 ms, about 10 µs per 64-sample block of 667 µs at 96 kHz.

**Alternatives**: a direct-form biquad (rejected by B1); coefficient interpolation (stable, but not the log trajectory the spec states); a transition in requested log-frequency (it sits at the ceiling then moves in a compressed tail: a sharper click, an ambiguous oracle).

## R-03 The type blend (D3; plan N2)

**Decision**: one state, two mixes on `(x, b, v2)` with **constant** triples in {−1, 0, 1}: low `(0, 0, 1)`, band `(0, 1, 0)`, high `(1, −1, −1)`, notch `(1, −1, 0)` (k enters once per sample through `b`; hardware B4, coach 10). A type landing at P: `y = gain · (T_old·s + w·(T_new·s − T_old·s))` over N samples with spec 021's smoothstep and both ends selected (the last sample `T_new·s` itself). A type landing during a blend freezes the current mix as a constant triple `T_old + w·(T_new − T_old)` and restarts the phase at 0 toward the newest type over the full 20 ms (the output continuous: hardware B4's failure 1); a landing back to the blend's origin type while it runs reverses from the current phase (spec 021); a landing of the type already targeted changes nothing.

**Rationale**: the state update does not depend on the type, so two sections fed the same input with the same coefficients and state stay **bit-identical**, also during a simultaneous frequency, Q or gain transition (hardware's decision review); the blend computed as above equals the two-section blend exactly. The plan restates every spec sentence that names two sections (plan N2).

**Alternatives**: two sections (the draft; equal signal, twice the work); an instant mix switch (`type_instant`, the mutant and the click reference).

## R-04 The manifests and the controls (D1, D2, D5)

**Decision**: `components.rs` holds two constant tables and their accessors.
- `insertion_points()`: one entry, `before_limiter` (value 0), its name, `paths: RECORD`, the placement sentence, and `control_index` 31 (product M3).
- `components()`: indexed 0 … count − 1 over the real components (product M4), each carrying its `value` (band filter 1; none is 0 and has no entry), identifier, name, description, `latency_samples` 0, `paths: RECORD`, `parameter_offset` (its base in the global store, R-06; band filter 0) and its parameters. Each `ComponentParameter` carries identifier, name, value type (`Choice` with the type options, or `Continuous`), range, unit, default, `scale` (linear / log / decibel: the transition's scale, a slider's hint; product M3) and `control_index` (32 – 35).
- A component fits a point when its paths include the point's path (documented). `band_filter::frequency_ceiling_hz(rate)` = 0.45 × rate.

The control manifest appends entries 31 – 35 (`insert.before_limiter` Choice none/band_filter, emulator option, idle, record, stage 0; the four parameters emulator options, live, record, the input clock); `CONTROL_COUNT` 36; each parameter entry's range, unit and default are **the same constants** as the component manifest's (a core test both ways). The `tap` entry (7) gains the option `after_insert`. The control map (spec 025) gains a third target, `Target::Component`: `control_value(31 … 35)` reads the slot and the store, `set_control_value(31, …)` calls `set_component`, `set_control_value(32 … 35, …)` calls `set_component_parameter` (idle only, 43 while active; product M2), `settings_control_value(31 … 35)` returns `NOT_A_SETTING`; the closure counts 26 settings + 5 live + 5 component = 36. Spec 014's checks learn the classes (coach 16): `us1_controls` checks 31 – 35 against the component manifest and the schedule's validation both ways; `us2_controls`' setter scan maps `set_component` and `set_component_parameter` to entries 31 and 32 – 35; `guide_settings_read` re-checked.

**The sentences** (the denylist applies; product M7): engine strings say "an addition of the emulator, not a part of the original recorder" (the library serves other apps; app copy keeps "an addition of the app"); `band_filter.gain_db`: "Sets the level after the band filter's section; a large gain drives the limiter and can keep the recorder from pausing." The guide states no per-type gate figure until the harness bisection prints one (hardware B12).

## R-05 The record form version 2 (D7)

**Decision**: `record::FORM_VERSION` 2. The engine's slot block (`memory::SlotBlock`) gains `component: u32` and `store: [f32; 16]` (about 68 bytes more per recording slot; reported). The writer emits, after the settings table, `insert.before_limiter = none|band_filter` and one line per parameter of **every** component (the band filter's four in 0.92), by the control manifest's identifiers. The reader accepts 1 and 2 (version 1: an empty slot and the defaults), refuses 0 and 3+, flags an unknown component identifier with `RECORD_FLAG_UNKNOWN_COMPONENT` (the re-render refuses it) and counts unknown parameter identifiers in `unknown_setting_count`. `RecordReadout` (Rust) gains `component` and `store: [f32; 16]`. The re-render (Rust recipe, `examples/re_render.rs`, Swift) calls `set_component` and `set_component_parameter` **before** `start_recording`. A re-render cell on the **second** recording of a session with the filter in (hardware B2). The record golden is re-blessed once (form 2, its version lines normalized): the fifteen audio guards stay unmoved, the record golden's SHA moves once with a comment in `golden_record.rs` and `us2_abi::us3_guards_unmoved` (coach 6).

**C (D9 kept; coach 1)**: the readout is unchanged. A new sized struct `RrDr60RecordComponent { struct_size, component, parameter_count, reserved, store[16] }`, filled by `rr_dr60_processing_record_component(bytes, len, out)`; the pure writer gains `rr_dr60_record_write_with_component(readout, component, out_bytes, …)` (the old `rr_dr60_record_write` writes form 2 with an empty slot and the defaults). The promise script's `--abi` rows are therefore untouched.

## R-06 The store and the component's lifecycle (D3)

**Decision**: the engine owns `ComponentStore { component: u32, values: [f32; 16], tags: [u64; 16] }`, a **global** store: each component's parameters at its published `parameter_offset`, never reused (the band filter at 0 – 3); the components' offsets plus counts ≤ `COMPONENT_PARAMETER_BUDGET` 16 with no overlap (a core test; hardware B7, coach 8; the owner's 2026-11-01 clarification "for all components together"). Created at the defaults; kept through `set_component`, `reset`, `reconfigure`.

- `set_component(point, c)`: 41 / 42 for an unknown value, 43 while recording or playing; equal: no change; otherwise both paths' inserts take it, clear the state and end the ramps; `run_insert = c != none`. On the one-pass the change is immediate (a guide sentence: fade or mute around it; product M6).
- `set_component_parameter(point, index, value)` (index global; hardware B3, coach 2, product M2): idle only (43 while active), validated like the schedule, never a change entry; writes the store, both paths' last-applied values and snaps both inserts.
- `component_parameter_value(point, index)` reads the store (global index; C out-type `float`).
- **The state** is cleared by `RecordPath::reset` (so at every `start_recording`, as stage 2's is), `reconfigure` and `set_component`; the one-pass keeps it across calls (hardware B2, coach 3; a restatement of the spec's edge case).
- **The live parameters** ride spec 017's record control state: `RecordCtl` widens to 8 + 16 slots (slot 8 + i is global store index i; `LIVE_COMPONENT[i]` the control index), masks `u32`, `pending_value`/`pending_tag`/`landed_tag` `[_; 24]`, a new `last_component: [f32; 16]`, `RECORD_LOG` 64 + 8 + 16 = 88. `RecordCtl::new(settings, &store)` and `adopt(settings, &store)` seed slots 8 – 23 from the store and apply them to the insert at once with no entry; `begin_call(settings, &store, &tags24)` makes differing values pending with the store's tags; `slot_of` maps 32 – 35 to 8 – 11; `apply_due` loops 0 … 24; `take_landed` returns the component values, written into the store and its tags by the epilogue (`Pipeline::record_tags` stays for 0 – 7; hardware B8, coach 9). A parameter of a component not in the slot lands in the store and the insert's cached target (in force when the component is set).
- Tests: `reconfigure` and `reset` with a moved store produce no entry; a recorder landing mirrors to the one-pass with its tag; a one-pass landing reaches the recorder through `start_recording` with no entry.

**Alternatives**: a separate control state for the component (a second hot compare per sample); per-component indexing (it aliases the band filter's values once a second component exists); a per-slot budget (product M1: 16 parameters per component rather than in total; recorded for the owner at the merge, not taken because the owner's clarification chose the total).

## R-07 The design script and the oracles (D4)

**Decision**: `tools/band-filter/design_band_filter.py` computes, with the f32-narrowed parameter values the store holds, for the four types at the defaults and at the golden's parameter sets, at the seven rates: the digital response at the oracle's frequencies (20 Hz to 0.45 × rate, third-octave centres plus the corner), the DC and Nyquist values, the coefficients, the impulse length rule and a cross-rate table for corners 1 – 2.5 kHz (hardware B16: the stored sound depends on the host rate near the device band's top; stated in the guide). `--check` asserts the structural zeros and the unity points; `--write` regenerates `crates/rr_dr60_harness/src/band_filter_ref.rs` byte-identical.

- **The response oracle** (oracle configuration: stages 1 – 2 and the limiter bypassed, the tap `AfterInsert`): the impulse response read by `analysis::dtft` at exact frequencies; `|H − H_ref| ≤ max(0.0115·|H_ref|, 1e-6)` (0.1 dB), the phase within 2° where `|H_ref| > −80 dB`; the notch centre ≤ −120 dB, reported (hardware B9, coach 12).
- **The impulse length** from the pole radius: `a2 = (1 − g k + g²) / (1 + g k + g²)`, `L = ⌈2·ln(1e8) / (−ln a2)⌉ + 0.1 × rate` (a −160 dB tail; hardware B5: the spec's `8 Q / (π f) + 0.1 s` leaves −70 dB tails and ignores warping); with it the zeros read ≤ −120 dB and the unity points within 1e-6 dB (SC-001; a restatement of the spec's length formula).
- **The placement identity** (default engine, same seed and counter): a `__test-hooks` f64 trace of the slot's input (`insert_input_trace`, reserved at construction like `stage4_trace`; hardware B1, coach 4); the harness section (detmath, the engine's operation order, the flush) on that trace, narrowed, equals the run's `AfterInsert` bit for bit.
- **The B1 row**: 48 kHz, a 100 Hz tone, Q 0.7071, the notch, band-pass and low-pass moved 21.6 kHz → 20 Hz; the metric the peak of |y| over the move plus 50 ms divided by (amplitude × the largest frozen |H| at the tone along the trajectory); the state-variable form against **transposed direct form II** in the harness (hardware B10: TDF-II reproduces the review's +14 dB, direct form I reads about +8); reported.
- **The click reader** (coach 13): spec 011's reader (0.5 s Hann window, 20 – 2750 Hz, the tone ±250 Hz excluded); the reference the `param_step` mutant at the same P; the floor printed beside every asserted row; asserted ≥ 30 dB on ±12 dB gain steps and one-octave moves at Q 0.7071 (the review's simulation: 45 – 50 dB), the type blend against `type_instant` reported (simulated 47.6 – 48.2 dB), Q 10 and full-range moves reported.
- **D6 and US1 AS5** (coach 14): the meters' limiter-input peak equals the f64 peak of the slot's output at seven rates; the loud row (+24 dB) asserts gain reduction > 0 and an ADC clamp count > 0 with the empty-slot twin at 0; the gate bisection `insert_matrix::gate_offset_bisection` (release-only, per type at levels 3 and 5, a Q 10 row at 0 dB gain, a lower estimate on 10 s runs; spec 012's pattern) prints the figures the guide then quotes (hardware B12).

## R-08 The golden (D10)

**Decision**: `golden-insert-v1.json`, **84 entries**: at the seven rates the four types at the defaults on voice, codec on, record path (28); the low-pass at three parameter sets (Q 0.5 / 300 Hz, Q 3 / 1000 Hz, Q 10 / 3000 Hz clamped where the ceiling applies) (21); the one-pass twin at the defaults (7); two schedules per rate, a frequency move and a type change (14); the tone rows (1 kHz at the four types at 6 and 48 kHz; 8); a second recording of a session with the filter in, at 6, 48 and 96 kHz plus 44.1 (4); and two set-then-schedule rows (2). Each entry `{rate, config, stimulus, schedule, n_out, digest}`. At bless time: the empty-slot twin (the sixteen guards' configurations with the slot empty equal 0.91's), every scheduled change landing before the stimulus ends (spec 013's lesson). Every test name in `golden_insert` and `insert_edge_cases` contains "insert". `RR_DR60_BLESS=insert`.

## R-09 The C surface and the promise (D9)

**Decision**: the functions, defines and statuses of contracts/api.md, four new sized structs (`RrDr60InsertionPoint`, `RrDr60Component`, `RrDr60ComponentParameter`, `RrDr60RecordComponent`) under spec 022's rule, **no existing struct changed** (coach 1; the readout's growth would fail the promise script's `--abi` rows, which print each header's struct sizes). cbindgen: the new structs in `include`. `smoke --insert` against `examples/insert_rows.rs` (a ±0.125 square). The parity rows on the square. ci.yml's promise steps: 12a3c45, f5a0245 and 97bbdc2 (full ids). Miri: the four new fillers. Swift: `ProcessingRecord.reRender` reads `RrDr60RecordComponent` and calls the two setters; `RRDR60Sized` on the new structs. D5's "no smoke case exceeds 142" asserted by a `us2_insert` test that runs every smoke mode's change-collecting calls (coach 17).

## R-10 Cost (D8)

**Decision**: `scripts/ab-timing.sh feec054 insert`: the record path and the one-pass with the slot empty, 45 strictly alternating solo runs, asserted at 3 % (`RR_DR60_INSERT_COST_CEILING` 0.05 on the label job). `InsertMutation` checks live inside `Insert::process` only. `insert_timing.rs` (release, ignored, `--test-threads=1`): the filter's ns per host sample at 48 kHz, a transition every 200 ms (reported), the worst 64-sample block with three parameters ramping at 96 kHz asserted within real time.

## R-11 The bound (D5)

**Decision**: `MAX_CHANGES_PER_CALL = 2 · SCHEDULE_CAPACITY + 3 + 8 + COMPONENT_PARAMETER_BUDGET + 3` = 158; the derivation test asserts the number and the formula. `insert_matrix::the_tight_bounds`: the one-pass's worst call reads **146** (64 + 64 + 3 + 8 + 4 + 3) and its record side 76 of `RECORD_LOG` 88; the recorder's 64 and the player's 69 unchanged; a 142-entry buffer receives the first 142 with the count 146 reported.

**Pins restated** (coach 15; each a dated restatement in its file): `MAX_CHANGES_PER_CALL == 142` (changes.rs, ffi lib.rs, ffi_parity, bypass_matrix, pitch_matrix, us2_bypass, us2_abi's `spec_025_moved_nothing` with `CONTROL_COUNT` 31), `RECORD_LOG == 72`, the 31-control counts (us1_controls, us2_pitch), manifest.rs's exhaustive `Tap` → stage match, control_map's 26 + 5 closure, the live-entry counts, the record golden's SHA. Every manifest-pinning binary runs before the gate (spec 023's lesson).

## The six mutants (`InsertMutation`; coach 5)

| Mutant | Defect | Oracle |
|---|---|---|
| `param_step` | the coefficients jump at P | the click rows (≥ 30 dB) |
| `type_instant` | the mix switches at P | the type click row; the blend identity |
| `freq_linear` | the frequency ramp in Hz, not log | the trace within 1e-9 |
| `restart_from_target` | a second landing restarts from the old target | the trace |
| `state_kept_on_set` | `set_component` keeps the state | the filter → none → filter identity |
| `skip_store_mirror` | a landing never becomes pending on the other stream | US2's cross-stream test |

## Figures to measure at implementation

The response deviation per type and rate; the placement identity's run; the B1 row; the click rows with their floors; the gate bisection per type; the cross-rate table; the slot block's growth; the empty-slot A/B; the filter's ns per sample; the worst block; coverage.
