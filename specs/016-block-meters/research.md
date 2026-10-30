# Research: Per-Block Meters (016)

Each entry: decision, rationale, alternatives. Code references are to `main` at the 0.14 merge (fe82f46).

## R-01: Where the meters are accumulated

**Decision.** One `MeterAcc` (crate-private) per path object: the `RecordPath` owns the record group's accumulators, the `PlaybackPath` the playback group's. Each accumulator is a plain field set (f64 peaks, a u32 count, the limiter's largest entry voltage) that the owning path's per-sample step updates in place. The `Pipeline` holds one public `Meters` value (the read-out) and, at the end of every non-empty processing call, **finalizes** the accumulators into it: narrows the peaks, converts the limiter's voltage to dB and to the charge ratio, reads the VAS state and countdown, and clears the accumulators for the next call.

**Rationale.** The per-sample loops already exist in four places (`OnePass::tick`, `RecordPath::step`/`front`/`front_only`, `PlaybackPath::push_wide`/`try_next_host`, the transport's `feed_one` and the play loop). Putting the accumulators in the path objects means each of the five call sites (one-pass, record, stop flush, play, drain) gets the figures from the same code, and the one-pass and the recorder share nothing but the read-out. Finalizing once per call keeps the transcendental work (`ln`, `exp`) off the per-sample path (spec FR-012, D6).

**Alternatives.** Accumulators in the `Pipeline` updated through the paths' return values (rejected: the points of reading, before the clamps and at the limiter's entry, are inside the paths); a `Meters` filled by each path separately with two read-outs (rejected by D2: one read-out, both groups).

## R-02: The points of reading, per sample

| Meter | Where (code) | Per-sample work |
|---|---|---|
| input peak (a) | `RecordPath::front` and `front_only`, after `sanitize_in(x)` | `abs`, compare-select on an f64 |
| limiter-input peak (b) | `RecordPath::front`, after `self.mic.process(..)` (stage 2's output; with both stages off `MicFront::process` is the identity, so it is the sanitized input, D7). At `Tap::AfterMicCapsule` `front_only` stops before stage 2: the meter stays at rest (`FrontStop::Capsule`); at `Tap::AfterMicAmp` `process_upto(.., MicStop::Amp)` is stage 2's output and the meter is read | as above |
| gain reduction (c) | `Limiter::process` reads `vc` at entry; the gain applied to this sample is `a(vc)`, monotone decreasing in `vc`. The accumulator keeps **the largest entry `vc` of the call**, `vc_max`; the call's finalization computes `a = 1/(1 + exp(vc_max·INV_2VT)·INV_K1)` and `−20·log10(a)` once, through `rr_dr60_detmath` (`exp`, `ln`), exactly as `gain_db` does; below `v_floor` the figure is 0 (the gain is exactly 1.0 there) | one compare-select on an f64 the limiter already holds in a local |
| hold charge (d) | the limiter's `vc` after the call's last sample, `vc / v_limit`, clamped to 1 (it is already clamped to `v_limit` in `process`, so the division is at most 1 by construction); `v_limit` is a field computed from `K1` alone, so the ratio is settings-independent | none per sample; one division at finalization |
| filter peak (e) | `RecordHalf::process`, after `self.record.process(d)` and before `self.clamp.process(d)` | `abs`, compare-select on an f64 |
| converter-clamp count (f) | `RecordHalf::process`: the clamp's output differs from its input (`c != d`; with the clamp bypassed `process` returns `d` bit-exact, so the test is exact and needs no `enabled` branch) | one compare, one conditional increment |
| state (g), hang remaining (h) | finalization: `VasStage::is_paused()`, and a new `VasStage::hang_remaining()` = `hang.saturating_sub(silent)` while recording, 0 while paused; `hang` when the stage does not run (`run_vas` false) | none per sample |
| reconstruction peak (i) | `PlaybackPath::try_next_host`, on `y` (the interpolator's or the pending slot's output, before `self.chain.process(y)`) | `abs`, compare-select |
| route peak (j) | `OutputChain::process`, on `r = self.inner(y)` before the bound, **only when the chain's tap is the route output** (`tap == Tap::RouteOutput`, a construction-time flag the chain already has for the clamp gate; the player's chain is always at the route output). At the other chain taps `r` is a stage's output, not the route's, so the meter stays at rest | `abs`, compare-select |
| output-clamp count (k) | `OutputChain::process`: `bound(r, ..) != r` when `self.clamp` | one compare, one increment, inside the existing `if self.clamp` |

The accumulations are `if a > p { p = a }` on f64 locals (the hardware review B9): no NaN can reach them (`sanitize_in` makes the input finite and every stage is finite on finite input), so the NaN-aware `f64::max` is not needed; on x86-64 it costs several instructions (spec 010's lesson). Each peak is narrowed **once per call** with `narrow_out` (the output's rounding): narrowing is monotone and symmetric, so `narrow(max|xᵢ|) = max|narrow(xᵢ)|` and the figure equals the largest absolute narrowed output exactly (spec FR-005; B6).

**Alternatives.** Reading the peaks at the block level from the output buffer (rejected: the filter peak, the limiter input and the pre-clamp route output are not in any output buffer; and the device-rate stream has no host-rate buffer). Keeping `a_min` instead of `vc_max` (equivalent, but `a` exists only above the floor; `vc` always does).

## R-03: The one-pass and the taps

`OnePass::tick` has three routes: the front tap (`front_only`), the test-only wide route, and the normal route. The meters follow the data: at a front tap only (a) and, at `AfterMicAmp`, (b) are read; `AfterAgc` runs `front` (a, b, c, d) but no `RecordHalf` (`down` is `None` and the half is not called); `AfterRecord` runs the half with `run_vas` false (e, f read; g, h rest); `AfterVas`/`AfterDecoder` the whole record group; the playback group is read when `PlaybackHalf::runs()` or the chain runs. The spec's tap table (Edge Cases) follows from the code paths with no new branching: a meter is at rest exactly when its code is not executed.

**The wide (test-only) route** does not update the meters; `narrowing_delta` does not read them.

## R-04: The transport calls

- `record`: `feed_one` → `recorder.push(x)` accumulates; `record` finalizes at the end of the call when at least one sample was fed.
- `stop_recording`: `feed_zeros` runs the same path; the stop finalizes once after the whole flush (one read-out for the flush, spec FR-001); a skipped flush (`!fed_any`) does not finalize (unchanged read-out, Edge Cases). With `force_open` the half's VAS state machine still runs, so (g, h) report it (B11).
- `play`: the loop pushes device samples and pulls host samples; finalizes when `produced > 0`. The drain pushes zeros through the same path, so the playback group reads its settle.
- `start_recording`, `start_playback`, `stop_playback`, `erase`, `erase_all`: no finalization; the read-out is unchanged. `reset` and `reconfigure` set the read-out to `Meters::default()` (rest) and clear the accumulators.
- The one-pass's `run_block` finalizes when `out.len() > 0`.

**Rule:** a call finalizes iff it processed at least one host sample (the one-pass, record, play) or fed at least one flush zero (stop). The paused state read at finalization is the half's own (`half.vas.is_paused()`), not `RecordPath::paused()` (which is `run_vas && ..`): with `run_vas` false the state is "recording" and the hang the full length by FR-002(h).

## R-05: The read-out types

Rust (`rr_dr60::Meters`, re-exported from `pipeline.rs`): `#[derive(Clone, Copy, Debug, PartialEq, Default)]`, `#[non_exhaustive]`, public fields in the spec's order with f32 peaks, f32 `gain_reduction_db`, f32 `hold_charge`, u32 `converter_clamp_count`, `bool vas_paused`, u64 `hang_remaining_samples`, f32 `reconstruction_peak`, f32 `route_peak`, u32 `output_clamp_count`. `Default` is the rest state except `hang_remaining_samples`, which depends on the settings: `Pipeline::meters()` returns the stored value, which `new`/`reset`/`reconfigure` initialise with the configured hang (`VasStage::hang_samples()`, device samples). `Meters::default()` itself has 0 there and is documented as "the rest state with a 0 hang; the engine's rest carries the configured hang".

**Why f32.** The peaks are narrowed values, exactly representable; the dB and charge figures are casts of deterministic f64 results (bit-identical across targets because the f64 is; `as f32` is a correctly rounded IEEE operation). The C twin then needs no double. The gain reduction's 0.01 dB tolerance is far above f32 resolution.

C (`RrDr60Meters`, `repr(C)`): `uint32_t struct_size` first, then the fields in the same order with `vas_paused` as `uint32_t` (0/1) and explicit padding so every field's offset is asserted (`const _: () = assert!(offset_of!(..))` as spec 010 did): `struct_size` 0, `input_peak` 4, `limiter_input_peak` 8, `gain_reduction_db` 12, `hold_charge` 16, `filter_peak` 20, `converter_clamp_count` 24, `vas_paused` 28, `hang_remaining_samples` 32, `reconstruction_peak` 40, `route_peak` 44, `output_clamp_count` 48, `reserved_016` 52 (u32), size **56**, 8-aligned by the u64. One function, `rr_dr60_meters(const RrDr60Pipeline *, RrDr60Meters *out)`: reads `out->struct_size` through the raw pointer first (the `check_struct_size` pattern, generalised to a `check_out_size` helper or a sibling), refuses `< 56` with `INVALID_ARGUMENT` and nothing written, fills the 56 bytes and zero-fills `[56, struct_size)` with `write_bytes` (D5; B12). A null handle → `NULL_POINTER` (the existing status), a null `out` → `NULL_POINTER`. Panic-caught like every entry point.

**Alternatives.** Growing `RrDr60BlockInfo`/`RecordInfo`/`PlayInfo` (rejected by D5); a double for the peaks (rejected: no precision to carry; the header's other sample figures are float).

## R-06: The filter-peak oracle at rates other than 6 kHz

**Decision.** A `__test-hooks` hook on the pipeline, `stage4_trace(&mut self, x: f32) -> Option<f64>`: pushes one host sample through the one-pass's record path's front and decimator and returns stage 4's output **before the clamp** when a device sample is due, without running the clamp, the VAS or anything after (the `mic_clip_trace` pattern, `paths.rs` 626). The harness runs a twin engine with the hook on the same blocks and takes the largest absolute value per block, narrowed with `narrow_out`; this equals the meter exactly at every rate. At 6 kHz the `AfterRecord` tap with the clamp bypassed is the second, hook-free oracle (spec SC-001).

**Rationale.** The device-rate stream is not in any output buffer at six of seven rates; the hook reads it where the meter does, from the same code, with the stimulus fed in the same blocks. The twin runs the same stages (`RecordHalf::process` up to the clamp), so a mutant that moved the point of reading (after the clamp) is caught by the clamp-on cells, and one that read the wrong stream by the rate cells.

**Alternatives.** A harness-side decimator (rejected: a second implementation of the polyphase filter, and the spec's "the harness's own copy" is satisfied by the hook's per-block capture); comparing with `AfterRecord` through the interpolator with a tolerance (rejected: not exact, and the spec asks for 0 mismatches).

## R-07: The gain-reduction oracle

The harness measures, per block, the ratio of the one-pass output at `Tap::AfterAgc` to a twin's at the same tap with `agc.enabled = false` (stages 1 – 2 bypassed in both, so the limiter's input is the stimulus itself; spec 002's R-11 method), on samples whose reference magnitude is at least −60 dBFS, and takes the deepest ratio in dB; the meter must be within 0.01 dB of it on every block (onset blocks included, B8: the ramp is about 2 dB per host sample at 48 kHz, so a meter reading the post-update state instead of the applied gain misses by up to 2 dB there). The steady figure is compared with `limiter_model.py`'s `steady_db` print (a mean; the meter is the deepest, and the in-cycle ripple at 10 dB over is about 0.05 dB, inside the 0.1 dB tolerance). The stimulus starts at `stimulus::peak_phase` (spec 005's lesson).

**The charge oracle.** `__test_hooks::limiter_state` (the capacitor voltage) divided by 1.275 V (the model's `v_limit` print, `vc_of_gain(10^(−40/20))`) must equal the meter's `hold_charge` within one f32 ulp after every block; the steady ratio on the 10 dB-over tone is pinned to a new print in `limiter_model.py` ("steady charge ratio at +10 dB over: Vc / v_limit") within 1 % (spec SC-002). The landmarks (0.13 floor, 0.73 knee, 0.84 at 10 dB, 0.90 at 20 dB) come from `vc_of_gain` and are printed by the script and asserted by a core unit test from the limiter's own constants (the 008 lesson: a figure the docs quote is pinned by a test that recomputes it).

## R-08: The VAS oracle

A harness `ReferenceGate` (in `meters_checks.rs`): the spec 003 state machine in its simplest form (threshold, hang, onset, the `silent`/`run`/`since_sound` counters as `vas.rs` documents them), fed the harness's own device-rate copy of the post-clamp stream (the `AfterRecord` tap at 6 kHz; at other rates the `stage4_trace` hook's output through the harness's clamp). Per block it yields the state and `hang − silent` (0 while paused); both must equal the meter. Independently, the engine's own reports corroborate: `BlockInfo.paused`/`RecordInfo.paused` equals `vas_paused` after every call, and at each drop-mode resume the splice event's `input_length` equals the device samples the read-out reported as paused since the flip (the harness counts them from the blocks' device-sample counts; at 44.1/88.2 kHz the device count per block is what the conversion produced, read from the twin's hook, not nominal arithmetic: spec 003's lesson).

## R-09: The cost measurement

There is no off switch (D6), so the budget is a **build comparison**: `scripts/ab-timing.sh <0.14 merge> output_cost` (the bypassed chain against the baseline's: both builds run `output_cost`, spec 009's codec-bypassed one-pass with stages 1 – 2 bypassed) and `scripts/ab-timing.sh <0.14 merge> output_cost_mic` (stages 1 – 2 on: the default record front). The spec's "default engine" figure is the latter; the bypassed chain is the former, asserted as **the same absolute nanoseconds** as 5 % of the default's baseline (B9). The script's `rounds` is raised to 45 for these two tests (the 012 protocol); each run's fastest figure is compared. CI: the `ab-timing` label's workflow gains the two steps with the 12 % ceiling (`RR_DR60_METERS_COST_CEILING`, read by the test the script runs). The local figure is quoted from a solo run.

**Expected cost.** About 6.5 compare-selects per host sample at 48 kHz (three host-rate peaks, two device-rate figures at 1/8, the limiter's `vc` compare only above the floor, the chain's count only when clamping) ≈ 1.5 – 3 ns against 73 ns on the speaker route: 2 – 4 % (B9). If the first measurement exceeds 5 %, the per-block alternative for the host-rate peaks (a vectorizable pass over the input and output buffers for (a) and (j) after the loop) is the fallback before any decision.

## R-10: The golden file

`golden-meters-v1.json`, generated by `golden::generate_meters`: entries = stimuli × configurations × 7 rates × paths. Stimuli (0.5 s each, so every block's read-out fits): the voiced −20 dBFS stimulus, a 1 kHz burst-then-silence (the VAS), a +9 dBFS 1 kHz step (the limiter), a full-scale square (the clamps). Configurations: the 0.14 default; the default with the output clamp on; `settings_011` (stages 1 – 2 off) with the limiter and clamp; the chain-only configuration; mute mode. Paths per entry: the one-pass (the per-block read-outs), record-then-play (the record calls' read-outs, the flush's, the play calls' and the drain's). Each `GoldenEntry` gains an optional `meters: Option<MetersGolden>` with per-block arrays of the eleven figures (f32 as 8-digit hex bit patterns, like `head`), the flush's read-out and the drain's last; the entry's existing `sha256` is the one-pass output's, so the entry also proves the audio unchanged. Entries over the size rule (none at 0.5 s; the rule is written for later additions) carry a SHA-256 over the per-block figures instead. The ten earlier files and their guards are untouched (their generators do not emit `meters`).

The C smoke program gains a `--meters` case: the voiced stimulus through the default engine at 48 kHz, printing every block's read-out; the harness `ffi_parity` test compares the C print's figures with the Rust read-outs bit for bit.

## R-11: The stage manifest's notes

The read-out is named by its type, the token `Meters`, in its own `;` group at the end of the controls note of stages 1, 2, 3, 4, 5, 10, 11, 12 and 13 (spec FR-014; `Meters` resolves under 014's FR-009 rule (d), an API item naming no entry, and the control cross-check still passes). Stage 5's note is 118 of 120 characters; its `splices (VasEvent)` becomes `VasEvent` (the 014 tokenizer treats both as an API item; no entry changes), which leaves room. Stages 11 (111) and 9 (114) are not touched beyond 11's `; Meters` (119: fits). The canonical text is re-blessed once, with the library version line; no model version moves (FR-011).

## R-12: Documentation and version

`docs/guide.md` gains a "Meters" section (the table of the spec's Key Entities plus the lead and the assumption IDs per figure, FR-014) with the Rust and C reads; the README's real-time list and the C header's real-time list gain `rr_dr60_meters`; the CHANGELOG records 0.15.0 (added: the read-out, the hook, the golden; no audio change; the manifest re-bless); front-and-inside.md's E-A line says built (0.15.0); CLAUDE.md gets the status entry and the commands at the merge.

No NEEDS CLARIFICATION remains.
