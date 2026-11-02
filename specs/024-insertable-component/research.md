# Research: An Insertable Component Slot and the Band Filter (spec 024)

Every figure below is an engineering target the spec states (D1 – D12) or a figure to be measured at implementation and reported; none is a device fact (the slot is an emulator convenience, D1).

## R-01 Where the slot sits (D1, D6, D8)

**Decision**: in `RecordPath::front_from` (`paths.rs`), after `self.mic.process(s)` and before `peak_limiter_input` and the limiter:

```text
x = mic.process(s)                     // stages 1 – 2
if run_insert { x = insert.process(x) } // the slot (none: not entered)
peak_limiter_input(x); limiter          // the meters read the slot's output (D6)
```

`run_insert` is a `bool` beside `run_limiter` in `RecordPath`'s first cache line (spec 017's lesson: the hot flags together), true only while a component is in the slot; with none the branch is never taken and the arithmetic is 0.91's (FR-002). The stop's flush, the one-pass and the recorder all reach the slot through `front_from` (the flush feeds silence through `step`), so "the flush runs through it like the input does" (Edge Cases) needs no code. `FrontStop` gains `Insert` for `Tap::AfterInsert` (the one-pass reads `mic.process_upto(Amp)` then the slot, no limiter); `Tap::is_front` includes the tap; the C value 11 is appended (`RR_DR60_TAP_AFTER_INSERT`).

**Rationale**: the limiter runs at the host rate before the decimator (spec 005 R-01), stage 2 already at the host rate (spec 012), so the slot inherits the host rate, the input clock and the transitions' sample grid with no conversion and no latency (FR-013).

**Alternatives**: a `match` on an `Option<Component>` per sample (a second load; rejected for the empty-slot budget); the slot inside `MicFront` (it would move the stage-2 tap's meaning).

## R-02 The section and its transitions (D3, D4)

**Decision**: the topology-preserving state-variable section (Zavalishin) in `f64`:

```text
g = sin(π f / r) / cos(π f / r)      (detmath; f = min(f_req, 0.45 r))
k = 1 / Q;  a1 = 1 / (1 + g (g + k));  a2 = g a1;  a3 = g a2
v3 = x − s2;  v1 = a1 s1 + a2 v3;  v2 = s2 + a2 s1 + a3 v3;  s1 = 2 v1 − s1;  s2 = 2 v2 − s2
low = v2;  band = k v1 (0 dB peak, D4);  high = x − k v1 − v2;  notch = x − k v1
y = gain · mix_type(x, v1, v2)
```

The mixes are fixed coefficient triples on `(x, v1, v2)` (low `(0, 0, 1)`, band `(0, k, 0)`, high `(1, −k, −1)`, notch `(1, −k, 0)`), so the structural zeros are exact by construction (low at Nyquist, high at DC, band at both) and the unity points within rounding (D4). The coefficients are recomputed **only while a transition runs**; at rest the section reads cached `a1, a2, a3, k, gain`. A transition at P: `u = n / N` with N = 20 ms of host samples (`output::volume_transition_samples`, spec 011's count), `w = u²(3 − 2u)`, the parameter `p(n) = p₀ + w·(p₁ − p₀)` in its own scale (log-frequency via `detmath::ln`/`exp`, log-Q, dB gain via `analysis`'s `from_db` form in the core: `exp(dB · ln10 / 20)`), sample N carrying the target itself (spec 019's lesson: `a + 1.0·(b − a)` is not `b`). A second landing within 20 ms restarts from the current value (spec 011); `reset`, `reconfigure` and `start_recording` end a transition at its target (spec 017's hiss rule). A transition is per parameter (three independent ramps; a type blend runs beside them, R-03).

**Rationale**: the hardware review's simulation (B1): a direct form with per-sample recompute overshoots the frozen response by up to +13.8 dB on a downward move, the state-variable form +3.6 dB; the harness reproduces both as a reported row (R-07). `detmath` has `sin`, `cos`, `exp`, `ln` (no `tan`), so `g` is a quotient; at the ceiling `π·0.45` the cosine is 0.156, far from zero.

**Alternatives**: a direct-form biquad (rejected by B1); coefficient interpolation (stable, but it does not follow the log-frequency trajectory the spec states); recomputing every sample (a fixed cost of a `sin`, a `cos` and three `exp` on every sample with the filter in, rejected for D8's reported cost).

## R-03 The type blend (D3; plan N2)

**Decision**: one state, two mixes. A type landing at P starts a `Fade`-style weight (spec 021's `stages/fade.rs` arithmetic: the smoothstep at `u = k / N`, the ends selected, a reversal from the current phase on a landing during a blend) between `mix_old` and `mix_new`, both read from the one state on every sample of the blend; at the end only `mix_new`. A type landing during a type blend: the blend continues from its current weight toward the newest type, the "old" mix being the blend's current mix frozen as a triple (the mixes are linear, so a blend of mixes is a mix).

**Rationale**: all four outputs are linear in `(x, v1, v2)` of one state; two sections fed the same input from the same state at P stay equal, so the spec's two-section blend and this one compute the same signal up to rounding (stated as a restatement, plan N2). Half the arithmetic during a blend; nothing to copy at P.

**Alternatives**: two sections (the draft; equal signal, twice the work); an instant mix switch (`type_instant`, the mutant and the click reference).

## R-04 The manifests (D1, D2, D5)

**Decision**: `components.rs` holds two constant tables and their accessors: `insertion_points()` (one entry: `before_limiter`, its name, `paths: RECORD`, the placement sentence of the spec's Clarifications) and `components()` (one entry: `band_filter`, its name, the description sentence, `latency_samples: 0`, `paths: RECORD`, and four `ComponentParameter`s: identifier, name, value type (`Choice` with the four type options, or `Continuous`), range, unit, default). `band_filter::frequency_ceiling_hz(rate)` = 0.45 × rate. The control manifest appends entries 31 – 35 (`insert.before_limiter` Choice none/band_filter, emulator option, idle, record, stage 0; the four parameters emulator options, live, record, the input clock), `CONTROL_COUNT` 36; each parameter entry's range, unit and default are **the same constants** as the component manifest's (one source, a core test both ways). The `tap` entry (7) gains the option `after_insert`. The control map (spec 025) gains a third target, `Target::Component(index)`; its exhaustive closure counts 26 settings + 5 live + 5 component = 36.

**Rationale**: spec 014's pattern (constant data bound to the engine both ways); the identifiers are the extension seam for later components (D11), each appending its own entries within the 16-parameter budget.

## R-05 The record form version 2 (D7)

**Decision**: `record::FORM_VERSION` 2. The engine's slot block (`memory::SlotBlock`) gains `component: u32` and `parameters: [f32; 16]` (the store at `start_recording`; about 68 bytes more per slot, the footprint a reported figure). The writer emits, after the settings table, `insert.before_limiter = none|band_filter` and one line per parameter of the component in the slot, or of `band_filter` when empty (D7: "present even when the slot is empty"), in the control manifest's identifiers (`band_filter.type = low_pass`, `band_filter.frequency_hz = 1000`, …). The reader accepts versions 1 and 2 (a version-1 record: an empty slot and the band filter's defaults), refuses 0 and 3+, flags an unknown component identifier with the new `RECORD_FLAG_UNKNOWN_COMPONENT` (the re-render refuses it), and counts unknown parameter identifiers in `unknown_setting_count` (spec 020's rule). `RecordReadout` gains `component` and `component_parameters: [f32; 16]` (the C tail of plan N1). The re-render (Rust, `examples/re_render.rs`, Swift `ProcessingRecord.reRender`) sets the component and the store **before** `start_recording` (B9). The record golden is re-blessed once (form version 2, its version lines normalized).

**Rationale**: spec 020 D2's versioned form; a version-1 reader would call a v2 record malformed rather than newer, which the version line prevents (D7's alternative).

## R-06 The store and the component's lifecycle (D3)

**Decision**: the engine (`Pipeline`) owns `ComponentStore { component: u32, values: [f32; 16] }`, created at the defaults, kept through `set_component`, `reset` and `reconfigure`. Each record path owns an `Insert` (the section, the transitions, the blend) built at construction for the band filter whether or not it is in the slot (no allocation later). `set_component(point, c)`: refused with `Error::NoSuchInsertionPoint` (41) or `NoSuchComponent` (42) for an unknown value, with `ComponentWhileActive` (43) while recording or playing; equal to the current: no change; otherwise both paths' inserts take the component and clear the section state, `run_insert = c != none`. The four parameters ride spec 017's record control state: `RecordCtl` widens from 8 to **8 + 16 slots** (the eight record-side values, then the parameter budget; the band filter's four at slots 8 – 11), the masks `u8` → `u32`, `RECORD_LOG` 64 + 8 → 64 + 8 + 16; `begin_call` compares the store (not the settings) for slots 8 – 23 and makes differing values pending with their tags; a landing routes to `Insert::land(param, value, at_once)`; the call's epilogue writes landed values into the store (the store, not the settings, is the carrier; `settings()` is unchanged). A parameter landed while the slot is empty updates the store and the section's cached target (it starts there when a component is set). A value is validated at the schedule call by the parameter's range after narrowing to `f32` (the type integral 0 – 3), `InvalidControlValue` otherwise; the playback stream refuses with `ControlNotOnStream`. `component_parameter_value(point, index)` reads the store.

**Rationale**: spec 017's machinery already gives the input clock, the landing rule, the cross-stream carrier and the log; widening it keeps one apply point per path and one hot compare.

**Alternatives**: a separate control state for the component (a second hot compare per sample; rejected for D8).

## R-07 The design script and the oracles (D4)

**Decision**: `tools/band-filter/design_band_filter.py` computes, for the four types at the default parameters and at the three parameter sets of the golden, at the seven rates, the digital response of the bilinear-prewarped prototype at the oracle's frequencies (20 Hz to 0.45 × rate, third-octave centres plus the corner), the DC and Nyquist values and the coefficients; `--check` asserts the structural zeros and the unity points; `--write` regenerates `crates/rr_dr60_harness/src/band_filter_ref.rs` (the table the harness compares with), byte-identical. The harness reads the impulse response at `Tap::AfterInsert` in the oracle configuration (stages 1 – 2 and the limiter bypassed; the impulse long enough: 8 Q / (π f) + 0.1 s) by `analysis::dtft` at the exact frequencies, within 0.1 dB and 2°. The **placement identity** (default engine, same seed and counter): the run's `AfterInsert` equals the harness's own `f64` section applied to the empty-slot twin's `AfterMicAmp`, bit for bit. The B1 simulation (the 48 kHz notch moved 21.6 kHz → 20 Hz with a 100 Hz tone; the band-pass and the low-pass) is a reported row beside the direct-form reference in the harness.

## R-08 The golden (D10)

**Decision**: `golden-insert-v1.json`, about 80 entries, each `{rate, config, stimulus, schedule, n_out, digest}`: at the seven rates the four types at the defaults on voice with the codec on (record path; 28); the low-pass at three parameter sets (Q 0.5 / 3 / 10 at 300 / 1000 / 3000 Hz) at the seven rates (21); the one-pass twin at the defaults (7); two schedules per rate, a frequency move and a type change (14); the tone rows at 6 and 48 kHz (about 8). The empty-slot twin is asserted at bless time (the sixteen guards' configurations with the slot empty equal 0.91's), not stored. `RR_DR60_BLESS=insert`.

## R-09 The C surface and the promise (D9; plan N1)

**Decision**: the functions, defines and statuses of D9 (contracts/api.md), three new sized structs (`RrDr60InsertionPoint`, `RrDr60Component`, `RrDr60ComponentParameter`) under spec 022's rule, the readout's appended tail (N1, `READOUT_PUBLISHED` `[376, 448]`, an explicit 4-byte reserved field so the tail has no implicit padding). cbindgen: the new structs in `cbindgen.toml`'s `include`. `smoke --insert` against `examples/insert_rows.rs` (a ±0.125 square, spec 006's lesson). The parity rows (`ffi_parity insert`) on the square: the filter in the slot and across a scheduled change, bit for bit. ci.yml's promise steps: 12a3c45, f5a0245 and 97bbdc2 (full 40-character ids). Miri: the three new fillers and the readout's two layouts. Swift: `ProcessingRecord.reRender` sets the component and the store; `RRDR60Sized` on the three new structs.

## R-10 Cost (D8)

**Decision**: `scripts/ab-timing.sh feec054 insert`: the record path and the one-pass with the slot empty against 0.91.0, 45 strictly alternating solo runs, asserted at 3 % (`RR_DR60_INSERT_COST_CEILING` 0.05 on the label job). `insert_timing.rs` (release, ignored, `--test-threads=1`): the band filter's cost in ns per host sample at 48 kHz (reported), a transition every 200 ms (reported), and the worst 64-sample block with three parameters ramping at 96 kHz asserted within real time (1000 × 64 ÷ 96 µs).

## R-11 The bound (D5)

**Decision**: `MAX_CHANGES_PER_CALL = 2 · SCHEDULE_CAPACITY + 3 + 8 + COMPONENT_PARAMETER_BUDGET + 3` with `COMPONENT_PARAMETER_BUDGET` 16 (a published constant in `components.rs`); the derivation test asserts 158 and the formula. The tight-bound test (`insert_matrix::the_tight_bounds`) builds the one-pass's worst call: 64 one-pass output-clock entries, 64 input-clock entries, three plain changes, the eight record-side mirrors, the four band-filter mirrors and the three stage mirrors, and reads **146** entries; the recorder's 64 and the player's 69 are unchanged. A 142-entry buffer receives the first 142 (spec 015's short-buffer path) and the count 146 is reported. A core test asserts that the components' parameters together are at most the budget (a later component that would exceed it fails the build's test).

## Figures to measure at implementation

The response deviation per type and rate; the placement identity's run length; the B1 row's peak excess (expected +3.6 dB, the direct form +13.8 dB); the click rows (gain steps, one-octave moves at Q 0.7071, asserted ≥ 30 dB; full range and Q 10 reported); the gain at which the gate stops pausing per type at levels 3 and 5; the slot block's growth; the empty-slot A/B; the filter's ns per sample; the worst block; coverage.
