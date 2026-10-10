# Research: Voice Activated System (VAS) on the Record Path

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md) | **Date**: 2026-10-09

Decisions for feature 003. They build on [spec 001's research](../001-pipeline-skeleton/research.md) (001 R-xx) and [spec 002's research](../002-agc/research.md) (002 R-xx). Numbers marked "prototype" come from Python simulations run during planning. The prototypes are not committed, and the Rust harness re-verifies every number.

## R-01 Where VAS runs (FR-001, FR-003, FR-011, FR-016)

- **Decision**: VAS runs at the 8 kHz device rate, inside the existing device chain, after stage 4 and before stage 10: host → decimator → AGC → stage 4 → **VAS** → stage 10 → interpolator → host.
  - **Stage gating**:
    - `run_vas = vas.enabled && tap ∈ {AfterVas, AfterPlayback}`;
    - `run_playback = playback_stage_enabled && tap == AfterPlayback`.
    - With `Tap::AfterAgc` or `Tap::AfterRecord`, VAS does not run and the output has fixed length (spec Edge Cases).
  - **What VAS returns per device sample**: *keep* (with the sample), *drop*, or, in mute mode, *mute* (the sample replaced by +0.0).
  - **Bypass**: a bypassed VAS does no arithmetic and keeps every sample.
  - **AGC**: it runs on every device sample whatever the VAS state, because it sits upstream (FR-016, A-025).
- **Rationale**:
  - **Signal-chain order**: matches the device (A-008).
  - **One design**: the decision logic is designed once, at one rate, as with the AGC (002 R-01).
  - **No new delay**: VAS adds no delay of its own, so FR-011's latency is unchanged.
  - **Test access**: at the 8 kHz host rate the boundary is the identity, so the harness can see every keep/drop decision directly.
- **Alternatives**:
  - **Deciding at the host rate**: would need per-rate thresholds and timing, and content above 4 kHz would affect the decision.
  - **Dropping at the host rate after the interpolator**: no simpler, since splices would still need mapping. It would also leave the interpolator's output clock unchanged, so the spliced signal would not be band-limited by the conversion.

## R-02 Detector (FR-006, FR-008, FR-009; A-022)

- **Decision**: a sample-peak comparator on the device-rate VAS input.
  - **Sound sample**: a sample is "sound" when |x| ≥ A<sub>thr</sub>, with A<sub>thr</sub> = 10^(T<sub>s</sub>/20) and T<sub>s</sub> = `threshold_dbfs` + 3 dB · (3 − `sensitivity`). Under AES17, a sine's peak amplitude equals its level, so a sample peak compares directly against the sine-level threshold. A<sub>thr</sub> is computed once at construction with `detmath::exp`.
  - **Pause side (hang)**: counts consecutive non-sound samples.
  - **Resume side (onset)**: counts a *sound run*, which starts at a sound sample and continues while no gap between sound samples exceeds **W = 32 device samples (4 ms)** (engineering target, 003 R-15). W bridges the troughs of every in-band sine: the longest trough of a 300 Hz tone at +3 dB is under half a period, 13.3 samples.
  - **No other path**: there is no Hilbert path, no smoothing and no hysteresis. The hang and onset counters provide the state stability (spec Edge Cases, "level hovering").
- **Rationale**:
  - **Peak-responding** (clarification 2026-10-09, A-019 style).
  - **Exact edge timing**: when a burst stops, the last sound sample is at most one sample before the burst's end. So hang time is measured from a known point, which a ±1 ms tolerance needs. An envelope detector would delay that point by its own decay (up to 63 samples for 002's Hilbert path), and the delay would depend on the signal.
  - **Frequency accuracy is enough**: FR-006 tests 300, 1000 and 3400 Hz within ±1 dB of each other. Sample peaks read 300 and 3400 Hz within 0.1 dB, because their phases sweep across a cycle within a few periods. At 1 kHz (8 samples per cycle) a phase-locked tone reads up to 0.69 dB low in the worst phase, which is still within ±1 dB. The ±3 dB keep/drop margins absorb it.
- **Alternatives**:
  - **002's analytic envelope (Hilbert + peak hold)**: accurate at 2000 and 2667 Hz too, but FR-006 doesn't test those. Its signal-dependent decay delay would eat most of the ±1 ms hang tolerance.
  - **RMS with smoothing**: rejected by the clarification.
  - **Hysteresis (two thresholds)**: unspecified, and the hang and onset times already stop chatter.

## R-03 Default threshold and sensitivity step (FR-006, FR-007; A-022 revised)

- **Decision**: revise A-022 from the first draft's −24 dBFS with 6 dB steps to **−18 dBFS at level 3, with 3 dB steps** (level 1 = −12, level 5 = −24 dBFS). This updates spec FR-006, FR-007, FR-012, US2 AS3 and the register.
- **Rationale** (prototype, spec 002's static curve plus Gaussian peak statistics; the noise's peak-detector reading is about +8.5 dB, 002 FR-009):
  - **Quiet background**: input noise at −70 dBFS comes out of the AGC at −30 dBFS (maximum gain).
    - **With −24 dBFS**: the margin is only 6 dB, so peaks cross the threshold about **38 times per second**. VAS would never pause in a quiet room, which contradicts S-001 ("pauses when no sound is detected").
    - **With −18 dBFS**: the margin is 12 dB, so peaks cross about 1.4 × 10⁻⁴ times per second, and the room pauses.
    - Input noise at −66 dBFS and louder still keeps recording, which is the "VAS works better at lower sensitivity" character.
  - **Sensitivity steps**: a steady sine is recorded when its post-AGC level exceeds the threshold. Through the AGC's static curve, that means an input above:

    | Level | 6 dB steps (first draft) | 3 dB steps (decision) |
    |---|---|---|
    | 1 | +30 dBFS: records nothing | −30 dBFS: "only the louder sounds" (S-001) |
    | 2 | −30 | −55 |
    | 3 | −58 | −58 |
    | 4 | −64 | −61 |
    | 5 | −70 | −64 |

    The AGC compresses the input range 10:1, so the threshold steps must be small to map onto useful input levels.
  - **Speech**: speech regulated by the AGC (about −13 to −9 dBFS) stays above −18 dBFS at levels 2–5. At level 1 only louder speech is recorded.
- **Alternatives**: keep −24 dBFS (fails the quiet-room behavior), or a VAS detector ahead of the AGC (contradicts the signal-chain order and the spec's FR-001).

## R-04 State machine (FR-008, FR-009, edge cases; A-023, A-024, A-025)

- **Decision**: per device sample, with H = round(`hang_ms` · 8) and O = round(`onset_ms` · 8) device samples, both computed at construction:
  - **Recording**:
    - A sound sample resets the silent count. A non-sound sample increments it.
    - While the silent count is ≤ H the sample is **kept**. When it reaches H + 1, the state becomes **Paused**, and that sample is dropped or muted.
    - So a gap of exactly H samples is kept in full and causes no pause (spec Edge Cases).
  - **Paused**:
    - **Tracking the sound run**: a sound sample starts a run, or continues one if the previous sound sample was at most W samples earlier. A gap longer than W ends the run.
    - **Resuming**: when the current sample is a sound sample and the run has lasted at least O + 1 samples, counted from its first sound sample to the current one, the state becomes **Recording** and the current sample is **kept**. Resume happens only on a sound sample: otherwise a burst shorter than the onset time could resume recording up to W samples into the silence after it (plan review, finding 1). It is the first sample after the splice.
    - So the first O samples of the sound are lost. With O = 0 the first sound sample is kept.
    - Every other paused sample is dropped (drop mode) or replaced by +0.0 (mute mode).
  - **Counters**: `since_sound` saturates at W + 1, so long silence can't overflow it (plan review, finding 16).
- **Start, reset and reconfigure**: **Recording**, with a silent count of 0 (A-025). So silence from the start keeps H samples, then pauses (spec US1 AS4).
- **Rationale**:
  - **Spec definitions**: it implements them directly. "Continuously below" is the silent count; "continuously above" is the sound run with W bridging troughs.
  - **Bounded state**: four integers and an enum, with constant work per sample.
  - **Rounding**: hang and onset times are rounded to whole device samples, so the error is at most 62.5 µs, far inside ±1 ms.
- **Alternatives**: timing in host samples (rejected: rate-dependent, R-01); a counter that decays instead of resetting (unspecified, and harder to test).

## R-05 Variable-length output: the emission schedule (FR-004; SC-003)

- **Problem**: today the interpolator emits exactly one host sample per input host sample: output n at input step n, using device samples up to ⌊n·l/m⌋ (001 R-09). In drop mode, dropped device samples never reach the interpolator, so the output clock must slow down without ever producing more output than input, and the result must not depend on block boundaries.
- **Decision**: the interpolator is fed only the device samples that VAS keeps (or mutes). After each host input step, it emits **at most one** host sample: the next sample n, if the kept-sample count K satisfies K ≥ ⌊n·l/m⌋ + 1. That is the number of device samples output n reads. Otherwise it emits nothing for that step.
  - **No drops**: at input step t the decimator has produced at least ⌊t·l/m⌋ + 1 device samples (001 R-09; sometimes + 2 at 44.1 and 88.2 kHz, which is why the interpolator's ring has 2 spare slots). The condition is ≥, so it holds for n = t at every step, so the schedule is **identical** to 001/002's one-in-one-out. Every pre-003 configuration and golden file is unchanged (FR-002, FR-020).
  - **With drops**: output stalls while paused, and catches up at most one sample per step afterwards. The interpolator never reads further ahead than today, so its ring buffer is unchanged.
  - **8 kHz host rate**: identity plan, so each kept sample is emitted immediately and a dropped one produces nothing.
- **Prototype** (200 000 host steps, random pauses, all five non-identity rates):
  - with no drops, the emission times equal 0, 1, 2, … exactly;
  - with drops, input length − output length equals dropped device samples × m/l within 0.95 samples at the end of a stream that ends while recording;
  - **settling** (found while implementing T006): when a pause starts, the output keeps flowing for the rest of the last kept device sample's host-rate slots, so at any moment the difference can be up to one device sample (m/l host samples, ≤ 12). Once recording has continued for ⌈m/l⌉ + 1 host samples after a pause, it is back within ±1 (exact when m/l is an integer). Spec FR-004 and FR-005 state this;
  - output never exceeded input in any step, and the interpolator never needed more look-back than today.
- **Rationale**: it is a per-sample rule on integer counters, so it is independent of block boundaries, allocation-free, and adds constant work per sample. `process_in_place` stays valid, because output index ≤ input index at every step.
- **Alternatives**:
  - **Emit every sample whose device data is available**: up to m/l = 12 outputs per input step at 96 kHz, so output could exceed input in a block.
  - **Resample the kept stream with a separate output buffer**: needs storage proportional to block length, which breaks the no-allocation rule.

## R-06 Splices, muted regions and removed length (FR-004, FR-005)

- **Decision**:
  - **Splice position** (drop mode): when recording resumes at kept device sample j, the splice's output position is the first host output n with ⌊n·l/m⌋ ≥ j. That is the first output whose newest device sample comes after the pause. At 8 kHz, n = j.
  - **Removed length**: let D be the cumulative number of dropped device samples. The splice's removed length is ⌊D<sub>now</sub>·m/l⌋ − ⌊D<sub>previous splice</sub>·m/l⌋, in host input samples.
    - Over a stream that ends while recording, the removed lengths sum to ⌊D·m/l⌋, which matches input − output within ±1 (R-05 prototype).
    - At 8 kHz the sum is exact.
  - **Mute region**: the same decisions. The region starts at the first output whose newest device sample is the first muted one, and ends at the first output whose newest device sample is the resume sample. Its length is in output samples, which equals input samples in mute mode.
    - Mute lengths come from absolute positions, while drop's removed lengths use the cumulative floor. At 44.1 and 88.2 kHz the two can differ by ±1 host sample; they are equal at 8, 16, 48 and 96 kHz, where m/l is an integer (spec FR-005; plan review, finding 2).
  - **When events are reported**: an event is reported by the block whose output contains its position. A splice's output position is emitted in the same step as the resume (R-05), so this is the block in which recording resumes. A muted region is reported when it ends.
  - **Paused at end of block**: each block reports whether VAS is paused at its end. A stream that ends paused has dropped (or muted) audio with no closing event, as spec FR-005 states.
- **Rationale**: positions and lengths come from stream counters, so they are independent of block partition (FR-005, FR-014). The cumulative floor makes the per-splice integers add up with no drift at 44.1 and 88.2 kHz, where one device sample is 5.5125 or 11.025 host samples.
- **Alternatives**: per-splice rounding of D<sub>i</sub>·m/l (drifts by up to 0.5 per splice); reporting removed length in device samples (exact, but in a unit hosts don't use; the clarification asked for input samples).

## R-07 Rust API (FR-004, FR-005, FR-012, FR-014)

- **Decision** (details in [contracts/rust-api.md](contracts/rust-api.md)):
  - **Settings**: `VasSettings { enabled, mode, sensitivity: u8, threshold_dbfs, hang_ms, onset_ms }` with `VasSettings::DEVICE`, a `VasMode { Drop, Mute }` enum, `Settings.vas`, and `Tap::AfterVas`.
  - **`process` and `process_in_place`** return a `BlockInfo { produced, events, paused }`. In drop mode, `output[..produced]` holds the block's output, and the rest of the slice is left unchanged.
  - **`process_with_events`**: a new method that also writes `VasEvent { output_position: u64, input_length: u64 }` values into a caller-provided slice.
    - `BlockInfo.events` is the number the block generated. If the slice was too short, the extra events are counted but not written.
    - Events are written to the slice in order: the first `events.len()` of them.
    - `Pipeline::max_events(frames)` returns a slice length that is always enough for a block of `frames` input samples: ⌊frames·l/m⌋ / (H + 2) + 2, computed in u128 so frames · l can't overflow.
    - **`#[must_use]` sweep**: about 40 existing call sites of `process` and `process_in_place` (harness helpers, tests, FFI, README) must be updated, because `clippy -D warnings` rejects unused results. Helpers that size output by input length must truncate to `produced` before VAS tests reuse them (plan review, finding 15).
  - **Errors**: `Error::InvalidSetting` gains four `Setting` variants: `VasSensitivity`, `VasThresholdDbfs`, `VasHangMs` and `VasOnsetMs`.
- **Rationale**:
  - **Source compatibility**: changing `process`'s success type from `()` to `BlockInfo`, and `process_in_place`'s return from `()` to `BlockInfo`, keeps existing calls compiling. `p.process(x, y)?;` and `p.process_in_place(&mut b);` are both still valid statements. `BlockInfo` is `#[must_use]`, so callers get a warning to read the produced count, which they need now that VAS is on by default.
  - **Real-time safety**: a caller-provided event slice keeps the processing path allocation-free.
  - **Version**: it's a breaking change in behavior (default output length), so the version becomes **0.3.0**. Nothing has been published yet.
- **Alternatives**:
  - **An internal event queue drained by a separate call**: needs a fixed capacity that can overflow on long offline blocks, and adds hidden state.
  - **Returning events by iterator**: allocates, or ties up a borrow of the pipeline.
  - **Separate `process_drop` and `process_mute` methods**: the mode is a setting, as every other stage's behavior is.

## R-08 C API (FR-015)

- **Decision** (details in [contracts/c-api.md](contracts/c-api.md)):
  - **Settings struct**: `RrDr60Settings` grows at the end with `vas_enabled` (bool), `vas_mode` (uint32), `vas_sensitivity` (uint32), `vas_threshold_dbfs`, `vas_hang_ms` and `vas_onset_ms` (float). It is now 72 bytes, and the `struct_size ≥ sizeof` rule is unchanged (002 R-09).
  - **New types**: `RrDr60VasMode`, `RR_DR60_TAP_AFTER_VAS = 3`, four new `RrDr60SettingField` values, `RrDr60BlockInfo { size_t produced; size_t events; bool paused; }` and `RrDr60VasEvent { uint64_t output_position; uint64_t input_length; }`.
  - **`rr_dr60_process`** gains a final `RrDr60BlockInfo *out_info` parameter (NULL allowed). This is a deliberate compile break for C callers: in drop mode they must read `produced`.
  - **New `rr_dr60_process_with_events(…, out_info, events, events_capacity)`** and **`rr_dr60_max_events(pipeline, frames, *out)`**.
  - **Unchanged**: in-place processing (`input == output`) still works, because output index ≤ input index (R-05).
- **Rationale**: a silent ABI-compatible `rr_dr60_process` would let C hosts read stale samples past `produced` without noticing. 0.x is unreleased, so the break costs nothing now.
- **Alternatives**: keep `rr_dr60_process` and add `rr_dr60_process_ex`. Rejected: the old function would silently misbehave with the new default.

## R-09 Mute mode (FR-004, FR-005)

- **Decision**: the VAS state machine is shared with drop mode. A *mute* decision replaces the sample with +0.0 and still feeds it to stage 10 and the interpolator, so the emission schedule stays one-in-one-out. Muted regions are reported as events with `input_length` = the region's length.
- **Exact zeros**:
  - With the tap "after VAS" at 8 kHz, a muted region is exactly zero.
  - At other host rates, a region's reported start is the first output whose newest device sample is muted, but the interpolator's window still holds kept samples for the next 2·delay_host + 1 outputs. So zeros are exact except for those first 2·delay_host + 1 outputs after the start (one pipeline latency plus one sample, spec FR-004), and exact up to the region's end. A region shorter than the window may have no exact zeros (plan review, finding 5).
  - With stage 10 running, its ring-out continues into the region (spec Edge Cases).
- **Rationale**: one decision path for both modes guarantees FR-005's "muted regions cover exactly the spans drop mode removes".

## R-10 Keeping specs 001 and 002 intact (FR-002, FR-020; SC-003)

- **Decision**:
  - **Bypassed configurations**: every harness configuration from 001 and 002 sets `vas.enabled = false`, including 002's `default_agc`, which is built from `Settings::new` and would otherwise pick up VAS.
  - **Unchanged golden files**: `golden-v1.json` and `golden-agc-v1.json` stay byte-identical. `golden_agc.rs` gains a SHA-256 guard for `golden-agc-v1.json`, as 002 did for `golden-v1.json`.
  - **Schedule**: R-05 guarantees one-in-one-out whenever nothing is dropped.
- **Rationale**: the same pattern that kept 001 intact in 002 (002 R-10).

## R-11 Measurement methods (FR-004 – FR-011; SC-002)

- **Decision**: every timing and threshold check is measured by **output length and event positions**, not by envelopes, at every host rate:
  - **Stimulus level**: bursts are 1 kHz at the threshold + 10 dB (spec definitions).
  - **Threshold (FR-006, FR-007)**: steady tones at the threshold ± 3 dB, for 10 s in US1 AS1 and for H + 1 s elsewhere.
    - **+3 dB**: produced = input length and no event.
    - **−3 dB**: produced = H converted to host samples (±1 ms).
    - **Threshold value**: for the ±1 dB checks, a bisection to 0.05 dB finds the lowest level that is kept in full, per frequency and per sensitivity level. The harness reports each rate's measured 1 kHz offset. (A phase-locked 1 kHz tone can read up to 0.69 dB low, and converter ripple adds ±0.1 dB, so a coarser grid could exceed ±1 dB; plan review, finding 9.)
    - **Noise**: the band-limited noise from 002 (harness FIR, not stage 4), at T − 5 dB and T − 15 dB, only for hang times ≥ 0.5 s (spec FR-013; at 50 ms, noise 5 dB below the threshold has sample gaps longer than H and pauses).
  - **Hang (FR-008)**: burst-gap stimuli with gaps of 0.5 H, H − 5 ms, H + 5 ms and 3 H. Kept gap length = output length minus the bursts' kept lengths. The splice count is 0 or 1 as expected.
  - **Onset (FR-009)**: a long gap, then bursts of 0.5 O, O − 2 ms (when O ≥ 3 ms), 1.5 O and 50 O, each followed by silence longer than H. The short bursts must leave no event and no output. The kept part of each burst comes from the output length and the splice position.
  - **Splice integrity (FR-010)**: at 8 kHz, each kept output sample is compared bit for bit with the VAS-bypassed output at the corresponding input index, which follows from the events' removed lengths.
  - **Mute (FR-004, FR-005)**: the same stimuli in mute mode, in the VAS-isolated configuration. Length = input. Region lengths equal drop-mode removed lengths (±1 at 44.1 and 88.2 kHz). Samples outside regions equal drop-mode samples at 8 kHz. Zeros are exact outside the R-09 allowance.
- **Host-rate edge effect**: the decimator smears a burst's edges.
  - **Prototype estimate**: the decimator's tail after a 1 kHz burst falls to a tenth of its amplitude within about 0.3 ms, and to a third (the threshold, for a burst at threshold + 10 dB) well within that. That's inside ±1 ms, and the effect is symmetric, so gap lengths shift by less than that.
  - **Self-test**: the harness reports the measured kept lengths at every rate, plus a self-test comparing each host rate with 8 kHz.
- **Default-pipeline interplay (US3 AS3)**: 1 kHz bursts at −20 dBFS input, 1 s on, then 5 s of −70 dBFS band-limited noise, 3 cycles, through AGC and VAS. The test function is `interplay_report` in `vas_matrix.rs`. The harness reports the kept gap lengths with no tolerance, as documentation of the noise-rise interaction.
- **Rationale**: output length is exactly what the spec constrains, so measuring it directly avoids 002's envelope-timing pitfalls (002 R-11).

## R-12 CI time budget (coach health check, 2026-10-09)

- **Decision**:
  - **Debug budget**: the VAS tests in the normal test suite must add **≤ 60 s** of wall time to the CI `check` job. (`profile.test` is opt-level 3, so the "debug" suite is optimized, about 230× real time.) The budget covers every VAS addition, including `determinism`, `alloc_free`, `mutation` and `golden_vas`.
  - **What the normal run covers**: defaults at all six rates; every sensitivity level and the minimum and maximum of every other setting at 8 and 48 kHz; "kept in full" tone checks of H + 1 s (except US1 AS1's 10 s); threshold searches by bisection.
  - **Partitions**: 100 random partitions of each VAS golden stimulus with `vas_only` at 8 and 48 kHz, and 10 partitions for the other configurations and rates.
  - **Release-mode matrix**: the full matrix (every setting × six rates, including 10 s hang times) runs in the release-mode `--ignored` step, budgeted at ≤ 60 s.
  - **Recording**: a task records the measured times in the test modules' docs, as 002 T035 did.
- **Rationale**: the 002 PR's CI took about 10 minutes. Without a budget, every stage adds a full matrix.
- **Not in this feature**: skipping the Rust jobs for docs-only PRs. That is a separate CI change, needing a stand-in job so required checks don't block.

## R-13 Performance (SC-004, SC-006)

- **Decision**: no new filters.
  - **Per device sample**: VAS adds a compare and a few integer updates, about 10 operations.
  - **Per host sample**: the emission rule adds one integer compare.
  - **Expected effect**: the 002 timing test's ≥ 20× real-time margin (measured about 230×) is essentially unchanged.
  - **Tests**: the `op-count` test gains a fixed VAS term, and the timing test adds the default configuration with VAS on.
- **Rationale**: the work is constant per sample, so it is bounded and independent of block partition (001 FR-015).

## R-14 Documentation and changelog (Principle VI, SC-001)

- **Decision**:
  - **README**: a "VAS (signal-chain stage 5)" section. It says what VAS does, gives the defaults table, and tells hosts to read `produced`. It covers drop vs. mute and when to use each, and has Rust and C snippets for bypass, mute, sensitivity and events. It says VAS is "modeled on the owner's manual and assumed values (A-008, A-021 – A-025)", with no EVP claims either way.
  - **CHANGELOG**: the default output now drops pauses; the API changes; and A-022 – A-025.
  - **Version**: 0.3.0.
- **Rationale**: the default output length changes for every host, so the docs must say so prominently.

## R-15 Engineering-target constants (FR-017; SC-008)

These are emulator design values, not device properties. Each is labeled `// engineering target (003 R-15)` in code:

- **Continuity window**: W = 32 device samples (R-02).
- **Rounding**: hang and onset times are rounded to whole device samples (R-04).
- **Event-slice bound**: the `max_events` formula (R-07).
- **Threshold search**: bisection to 0.05 dB (R-11).
- **Burst level**: the threshold + 10 dB (spec definitions).

The spec's tolerances keep their labels from spec FR-017.
