# Research: Playback Speed (spec 013)

Every figure here comes from `tools/playback-speed/speed_design.py` (written at plan time, 2026-10-17; `uv run tools/playback-speed/speed_design.py --check`) unless marked as an architecture fact read from the code.

## R-01: Placement and structure

**Decision**: a new stage `stages/speed.rs::SpeedStage`, owned by `Pipeline` beside the player's decoder (`Pipeline { speed: SpeedStage, speed_level: u8 }`), run only by `Pipeline::play()`. The player's inner pull loop (`transport.rs`, `play()`: "Pull the next stored device sample (decoding a frame when the codec is on), or a zero of the drain") is refactored into a **source** that returns `Option<f32>`: the next stored device sample, or `None` once the recording's K stored samples are exhausted. The speed control takes the source as a closure, `SpeedStage::next(&mut self, src: &mut impl FnMut() -> Option<f32>) -> SpeedOut`, and the transport pushes each `SpeedOut` into `PlaybackPath` (a new `push_device_f64`, the existing `push_wide`).

**Rationale**: architecture facts: playback is pull-based (the interpolator asks for a device sample when it needs one, `PlaybackPath::try_next_host`), the source (memory, decoder, cursor) is owned by `Pipeline`/`Transport`, and `PlaybackPath` is shared with the one-pass processing, which has no stored source. A pull interface lets the speed control read stored samples ahead of its place without adding latency (R-03), and keeps stage 10, the interpolator and the output chain unchanged (FR-003).

**Alternatives considered**: inside `PlaybackPath` (would need the memory and decoder, or a push interface that cannot read ahead); a push-model resampler with a fixed delay (adds latency, so the place at the first output would not be the start, and the level-3 identity would need the delay at level 3 too); the codec decoder producing speed-adjusted output (ties the control to the codec, but it must also work with the codec bypassed).

## R-02: The place in fifths, the level and the switch rules

**Decision**: the place is a `u64` count of fifths of a stored sample, `p5`, starting at 0. Each device output is computed at the current place with the active level k (k = 3 … 7), then `p5 += k`. At a place p5 = 5i + ph (i whole, ph = 0 … 4):
- level 3 (k = 5, ph is always 0): the output is the stored sample x[i] widened exactly (`f64::from`);
- levels 1, 2, 4, 5: the output is Σ_{m=0}^{56} c_k[ph][m] · x[i + 28 − m] (R-03), x[j] = 0 for j < 0 and j ≥ K.

The host's level (`Pipeline::speed_level`, set any time) is copied into the stage at the start of each `play()` call (like the route and the volume, spec 009 FR-009), as the **requested** level; the stage applies it before computing its next output: a change to a non-3 level applies at once (the next output uses the new bank at the current place, the place then advancing by the new k); a change into level 3 is **deferred** until p5 ≡ 0 (mod 5) (at most 4 outputs at the old ratio, because gcd(k, 5) = 1 for k = 3, 4, 6, 7: the script prints "at most 4" for every level), and a different request during the wait applies at once and drops the pending switch (the spec's clarification). Once the place reaches K (p5 ≥ 5K) the level is **latched**: later requests do not change the current playback.

**Rationale**: integer fifths make the place exact (no accumulated rounding, FR-005) and the five ratios exact; the duration rule follows directly: at a constant level the outputs with place < K number ⌈5K ÷ k⌉ = ⌈K ÷ r⌉ (the script's table: K = 120 gives 200 / 150 / 120 / 100 / 86). Because each output depends only on (place, level, stored samples), the output after a change equals a constant-level playback from the same place bit for bit (the continuity oracle, FR-007). The deferred switch avoids the phase step of a snap (D5: up to 0.8 sample, 96° at 2 kHz).

**Alternatives considered**: a fractional `f64` place (accumulates rounding; not exact); snapping into level 3 (a click); applying a change at the next whole sample for every level (needless; the banks work at every phase).

## R-03: The filter

**Decision**: per non-3 level, a linear-phase Kaiser-windowed sinc prototype of **281 taps** (10·D + 1, D = 28) at the 30 kHz prototype rate (5 × 6 kHz), built at construction by the existing `resample::design::kaiser_lowpass(281, fc ÷ 30000, kaiser_beta(70))` and split by the existing `polyphase_split(h, 5)` into 5 phases of 57 taps, each normalized to sum to exactly 1 (unit DC per phase, D10). Band edges, in the stored stream's frequencies:

| Level | k | r | Passband (Hz) | Stopband (Hz) | Cutoff (Hz) | Passband deviation (dB) | Worst spurious 20 – 3000 Hz (dB, at stored f) | Tail (device samples) |
|---|---|---|---|---|---|---|---|---|
| 1 | 3 | 0.6 | 2250 | 3100 | 2675.0 | 0.0017 | −76.1 (2828 Hz) | 46 |
| 2 | 4 | 0.8 | 2250 | 3100 | 2675.0 | 0.0017 | −76.2 (2828 Hz) | 34 |
| 4 | 6 | 1.2 | 1875 | 2500 | 2187.5 | 0.0019 | −72.8 (2503 Hz) | 23 |
| 5 | 7 | 1.4 | 1607.1 | 2142.9 | 1875.0 | 0.0025 | −74.4 (2203 Hz) | 20 |

The passband is min(2250, 2250 ÷ r); the slow levels' stopband is 3100 Hz (the image 6000 − f of a stored 2900 Hz tone, FR-004 (b)); the fast levels' is 3000 ÷ r (the device Nyquist frequency after the move, FR-004 (b), (c)). The look-ahead is D = 28 stored samples (4.67 ms). Figures: a least-squares fit at r × f for the passband (tones every 50 Hz from 103 Hz), a Kaiser-windowed (β 20) spectrum of the residual between 20 and 3000 Hz for the spurious (tones every 25 Hz from 103 to 2903 Hz; for r × f above 3000 Hz the whole output is the residual, FR-004 (c)).

The Rust banks are designed by the same formula; the core tests pin the Rust figures to the script's within 0.5 dB (spurious) and 0.001 dB (passband), and the bank's DC sums to 1 per phase within 1e-15 (the script cannot be bit-identical across languages: numpy's `i0` and `sin` against detmath's).

**Rationale**: the spec's prototype (`proto3.py`) showed 241 taps leaves level 5 at −66.9 dB and 281 taps at −71.7 dB; the per-level edges in the table above (the stopband exactly at the spec's limits, the cutoff halfway) give at least 12.8 dB of margin over the 60 dB floor at 281 taps. Designing at construction with the existing deterministic designer follows the host converters (004 research R-03, determinism across platforms by detmath), and costs 4 × 285 f64 ≈ 9 KB.

**Alternatives considered**: committed coefficients generated by the script as bit patterns (the hardware reviewer's D10; the host converters' runtime design is the closer precedent, the script's `--check` pins the figures, and a committed table of 1140 values would need its own byte-identical `--write`); per-level lengths (fewer multiplies at the slow levels, 201 taps reach −73 dB there; one length keeps the ring, the look-ahead and the tail rule uniform); a 60 dB design (131 taps; 5 dB margin, too thin for measurement leakage, D8); linear interpolation (images 25 – 40 dB down; the spec chose a clean resampler).

## R-04: The ring, the look-ahead and the tail

**Decision**: the stage keeps the stored samples in a ring of 128 `f32` (a power of two above the 57-tap window plus two samples per output), `pulled` (stored samples pulled so far) and `end: Option<u64>` (K, once the source returned `None`). Reading x[j] for j ≥ pulled pulls from the source until `pulled > j` or the source ends; x[j] = 0 for j ≥ K. At level 3 only x[i] is needed (no look-ahead, FR-006); every pulled sample is written to the ring, so the history is there when the level leaves 3, and leaving level 3 at place i pulls up to x[i + 28] at once (at most 29 stored samples, at most one extra frame decoded in that call).

`SpeedOut` is `Own(f64)` while the place is before K, `Tail(f64)` while the kernel still reaches a stored sample (the lowest index read, i − 28 for ph = 0 and i − 27 for ph > 0, is at most K − 1), then `Silence` (+0.0). At level 3 there is no tail (the first output at place K is `Silence`, +0.0, exactly 0.12's drain zero). The tail is at most **46** device samples (level 1; 34 / 23 / 20 at levels 2, 4, 5; the script's worst case over K mod k), well inside the drain's ⌈N₁₀ · m ÷ l⌉ + 2D host samples (N₁₀ = 121 device samples), so the drain's length is unchanged; a const assertion pins `MAX_TAIL_SAMPLES` (46) < N₁₀. Stage 10 then settles 121 device samples after the tail's last sample instead of after K: spec 009's last-millisecond bound (−80 dBFS on the drain's tail, 70 dB of margin, 009 lessons) is re-checked at every level (US1 AS6).

**Rationale**: the pull model makes the look-ahead free in latency; playing the tail avoids the truncation click of stopping at K (D3); the drain's zeros past K were already pulled as device samples in 0.12, so replacing the first ones changes no length.

**Alternatives considered**: stopping at K (cuts the last samples' response, D3); extending the drain at levels other than 3 (changes the drain length and the length helper per level).

## R-05: Level-3 identity and the dynamic total

**Decision**: `Cursor::total` (host samples the playback produces in all) becomes **dynamic**: `u64::MAX` at `start_playback`, set to ⌈K′ · m ÷ l⌉ + drain the first time the speed control returns a non-`Own` output, K′ being the `Own` outputs so far. The `place` reported by `play()` is min(⌊p5_last ÷ 5⌋, K), p5_last the place of the last output computed (0 before any); `frame_index` is min(⌊place ÷ 120⌋, F − 1) with the codec on and 0 with it bypassed.

**Proof of the level-3 identity** (architecture facts from `transport.rs` and `resample/up.rs`): at level 3 the stage pulls exactly one stored sample per output, in the same order and at the same moments as 0.12's loop (the interpolator's demand is unchanged), so every pushed device sample, every decode, `frames` and the output are identical. The first request for a device sample at place K happens when host sample n₀ = ⌈K · m ÷ l⌉ needs device sample K (`PolyphaseUp::try_next_host` reads device samples up to ⌊n · l ÷ m⌋; at the identity rate the pending sample, n₀ = K); at that moment `produced = n₀`, so the dynamic total ⌈K · m ÷ l⌉ + drain equals 0.12's precomputed one and is never below `produced`. 0.12's `frame_index` is the index of the last decoded frame, which is the frame of the last pulled sample ⌊i ÷ 120⌋ and F − 1 in the drain; the new rule gives the same. The existing tests `playback_produces_ceil_k_m_over_l_then_drain` and `play_reports_frames_and_index` (`transport.rs`) must pass unchanged, and the eight golden files are the outer guard.

**Rationale**: a precomputed total cannot hold once the level changes mid-playback (C4); the dynamic one is exact for every schedule and reduces to 0.12's at level 3.

**Alternatives considered**: recomputing the total at every level change (needs the future schedule); counting device samples instead of host samples for `finished` (changes 0.12's `produced` accounting).

## R-06: The API

**Decision** (contracts/rust-api.md, contracts/c-api.md): Rust `Pipeline::set_playback_speed(level: u8) -> Result<(), Error>` (`Error::InvalidSpeedLevel { level }` for 0 or above 5, the level unchanged), `Pipeline::playback_speed() -> u8`, `Pipeline::playback_length_samples(number: u32, level: u8) -> Result<u64, Error>` (level checked first, then the number; ⌈⌈5K ÷ k⌉ · m ÷ l⌉ + drain); `PlayInfo::place: u64`; the module `rr_dr60::speed` with `LEVEL_COUNT = 5`, `DEFAULT_LEVEL = 3`, `RATIO_DENOMINATOR = 5`, `RATIO_NUMERATORS = [3, 4, 5, 6, 7]`, `ratio(level) -> Option<f64>`, `LOOKAHEAD_SAMPLES = 28`, `MAX_TAIL_SAMPLES = 46`. C: status 25 `INVALID_SPEED_LEVEL`, `rr_dr60_set_playback_speed`, `rr_dr60_get_playback_speed`, `rr_dr60_playback_length_samples`, defines `RR_DR60_SPEED_LEVEL_COUNT`, `RR_DR60_SPEED_DEFAULT_LEVEL`, `RR_DR60_SPEED_RATIO_DENOMINATOR`, the numerators as five defines `RR_DR60_SPEED_RATIO_NUMERATOR_1 … _5` (cbindgen emits a define only for a literal const, 009 lesson), and `place` at the end of `RrDr60PlayInfo` (32 → 40 bytes on 64-bit targets; a compile break, as 008's growth). The level is a `u8` like a C `uint8_t`; the control mirrors the volume's (spec 009 FR-003): a setter that refuses invalid values with the value unchanged, a getter returning the last valid setting, kept by `reset` and `reconfigure`.

**Rationale**: the control surface of the volume is the established pattern; an integer level matches the device's detented dial; exact fifths let hosts compute lengths exactly.

**Alternatives considered**: an enum `SpeedLevel` (stronger typing, but a C mapping and five variants for a 1 – 5 integer; the volume's validated-scalar pattern is simpler); a speed setting in `Settings` (needs a reconfigure, which resets the paths, spec 006: not live).

## R-07: Cost and timing protocol

**Decision**: three timing measurements, quoted from solo release runs with `--test-threads=1`:
- `timing::speed_cost` (in one build): the codec-bypassed playback of 60 s of −20 dB noise recorded with stages 1 – 5 bypassed, at 48 kHz on the speaker route in 4096-sample blocks, levels 1, 2, 4, 5 against level 3, each side the fastest of 45 strictly alternating runs; asserted ≤ 15 % on the development machine with a runner ceiling `RR_DR60_SPEED_COST_CEILING` (25 %) in ci.yml; nanoseconds per host sample printed. The cost per host sample counts the recording's own part and the drain.
- `playback_timing` (a new test binary using only 0.12's API): the same playback at level 3 (the default), printed in `ab-timing.sh`'s parsable form; `scripts/ab-timing.sh` gains a playback mode that copies `tests/playback_timing.rs` into the baseline worktree (`3a4eefb`, 0.12's merge) before building, then alternates the two builds 45 times; ≤ 3 % on the development machine, ≤ 5 % on the CI runner (the `ab-timing` label).
- `timing::speed_codec_worst_block`: codec on, 6 and 48 kHz, blocks of 64 and 4096 samples, the level changed before every block through the cycle 3 → 1 → 5 → 2 → 4 → 3 …; the worst block against 1000 × block ÷ kHz µs (008's lesson: a block budget in µs); and the per-host-sample cost at a constant level 5 against level 3 (≤ 1.5×).

Expected: 57 multiply-adds per device output at the non-3 levels, about 7 per host sample at 48 kHz (the script prints both): +3 … +6 % on the development machine; the decoder's cost scales with r at level 5 (1.4 frames per output second): about +40 % of the decoder share with the codec on, inside 1.5×.

**Rationale**: the 2026-10-17 chore's lesson (in-build budgets need the 45-run estimator and a runner ceiling); 011's A/B lesson (a budget below the timing noise needs a build A/B); the baseline cannot have a test that does not exist in it, so the A/B overlays a test written against the old API.

**Alternatives considered**: the existing `split_costs_no_headroom` for the A/B (record and play together, so the playback's share is diluted, and its print is not in the parsable form); fastest-of-9 (run-order sensitive on this machine, 012's lesson).

## R-08: Oracles and measurement

**Decision**: `speed_checks.rs` with:
- **lengths**: the recording's own part from `play()`'s `produced` minus `drain_samples()` at a constant level equals ⌈⌈5K ÷ k⌉ · m ÷ l⌉ and `playback_length_samples`; `finished` once.
- **fit** (FR-004 (a)): at 6 kHz in `configs::speed_only` (codec, stage 10 and output chain bypassed), an imported stored tone; `analysis::sine_fit` at exactly r × f over the steady middle; amplitude ±0.1 dB, residual's spectral peak ≤ −60 dB; at the other rates the level against level 3's playback of a stored tone at r × f within ±0.1 dB (the host converter's ripple common to both).
- **spurious** (FR-004 (b), (c)): `analysis::spurious_peak_db` (Kaiser β 20 window, 20 – 3000 Hz) of the fit's residual (or the whole output for r × f > 3000 Hz); asserted ≤ −60 dB at 6 kHz; pinned to the script's table within 0.5 dB at the table's tones; reported at the other rates.
- **impulse** (FR-008): an imported impulse at stored sample 600: the output peak at device output round(600 ÷ r) (1000, 750, 600, 500, 429) at 6 kHz; symmetric within 1e-12 where 5 · 600 ÷ k is whole.
- **completeness and tail**: a recording ending on a full-scale sample: the device outputs after the own part equal the hooked constant-level computation's tail and are followed by exact zeros at stage 10's input (`configs::speed_only` at 6 kHz reads the device stream directly).
- **continuity** (FR-007): a test hook `__test_hooks::start_playback_at(p, number, place_fifths)` starts a playback at a place at the current level; for every ordered pair (a, b ≠ 3) and two change points, the output after the change (from the first device sample computed after it, located through the reported place and the speed-only configuration at 6 kHz) is bit-identical to the hooked constant-b playback from the reported place, the prefix to the constant-a playback, and the hook `speed_place_fifths` reads the exact place sequence; into level 3, the old ratio for at most 4 outputs then bit-identical to a level-3 playback from the whole place.
- **level 3**: every play report field against 0.12's rules (R-05), the record-then-play ≡ one-pass checks unchanged, the eight goldens.
- **scope**: the one-pass, the record path, the stored bytes and the export at every level against level 3 (bit-identical).

FR-004 is asserted at 6 kHz only, where `play()`'s output is the device stream (C3); the stimuli are imported (the samples form) or recorded with stages 1 – 5 bypassed, never the default record path (its hiss, spec 012).

**Rationale**: constant-level runs and the stimulus itself are bit-exact oracles (011's lesson); the place hook makes the continuity oracle exact (D6); a least-squares fit resolves 0.01 % where an FFT bin cannot (D1).

**Alternatives considered**: a tolerance-based continuity oracle (looser than the structure permits); FFT frequency estimates.

## R-09: Golden file

**Decision** (contracts/golden-format.md): `golden-speed-v1.json`, generated through the public API: (a) the default engine (0.13's default, the codec on) recording `voiced_m20` and `tone_1k_m20` with stages 1 – 5 at their defaults, played at levels 1, 2, 4, 5 on both routes at the reference volume, at the seven rates (2 stimuli × 4 levels × 2 routes × 7 = 112); (b) the codec bypassed, the speaker route (2 × 4 × 7 = 56); (c) two change schedules (A: 3 → 1 → 3 → 5 → 4; B: 2 → 5 → 1, plus a change after the place reached the end) on the default engine's speaker route (2 × 7 = 14); (d) the speed-only configuration at 6 kHz on an imported `tone_1k_m20` and `voiced_m20` at levels 1, 2, 4, 5 and the two schedules (2 × 6 = 12). 194 entries. Each entry carries the existing digest fields plus `speed_level` (the constant level, or the schedule's first), `speed_changes` (`[play-call index, level]`), `place` (final), `own_length` (host samples before the drain) and `frames` (decoded, codec on). The bless asserts FR-005's counts, the length helper for the constant-level entries, and that no entry's level is 3 throughout (the eight earlier files cover level 3).

**Rationale**: the default engine is the point (app use); the speed-only set isolates the stage; schedules guard the live change.

**Alternatives considered**: level 3 entries (duplicates of the existing files); every stimulus of 009's set (no new coverage for the speed control).

## R-10: Records and wording

**Decision**:
- **A-054** (new row; cited by stage 9): the spec's Assumptions text, status "assumed (spec 013)", confidence "Low (ratios, mechanism, resampler quality) / High (the control exists and is firmware)"; the verification capture P1 with the corrected decision rule (the 2.3 kHz tone about 54 dB down at level 4 under (b) as modeled; absent at level 5).
- **S-018** (new row; cited by stage 9): OKI EP0714089B1 (priority 1994, CELP decoder speed change by deleting or repeating pitch cycles, aimed at IC-memory answering machines) and Lucent US5717823A (excitation-domain time scaling and its artifacts), Type "Patent", Reliability "Secondary (for the class; nothing names the device)".
- **Manifest** (each string checked against the denylist and the length limits before the re-bless): stage 9 description "Rebuilds the voice-band stream from the stored CELP frames for playback, at one of five playback speeds (level 3 is normal)." (124 characters); sources S-001, S-002, S-003, S-008, S-009, S-010, S-011, S-012, S-018; assumptions A-004, A-040, A-041, A-054; controls "speech_codec_enabled; set_playback_speed; Tap::AfterDecoder; PlayInfo.frames, PlayInfo.frame_index, PlayInfo.place" (114); coverage "the device's own decoder and postfilter (A-040, A-041); its speed mechanism (A-054); pitch-kept speeds" (102); model version 0.13.0. Stage 10 coverage "µ-law expansion, idle noise on the playback side, DAC imaging residue above 3 kHz" (the speed clause removed). The FR-011 "level 3 is its identity" sits in the description ("level 3 is normal"), because the controls note lists API items verbatim.
- **Signal-chain row 9**: "**Modeled by** spec 013 (2026-10-17): five playback speeds, 0.6 … 1.4 (A-054), a resampling at the 6 kHz device rate between the decoder and stage 10, pitch moving with speed; modeled on the documents, not measured".
- **README**: a "Playback speed" section (the levels and ratios, pitch, the modeled default and the weak lean toward a pitch-keeping device, live and playback-only, the length rule and helper, level 3 unchanged, the C names); `us2_cross_check::readme_speed_statements` checks the key statements.
- **CHANGELOG**: 0.13.0 regions (Added, Changed: the C play report's growth, Documentation: A-054, S-018, the manifest re-bless).

Wording: "the speed control", "playback speed", "level"; "ramp" and "glide" never; "modeled on", never "measured" or "accurate"; the denylist's stems (`verif`, `detect`, `sens`, `event`, …) avoided in manifest strings.

## R-11: Capture P1's figures under the model

**Decision**: A-054 quotes the script's figures: a stored 2300 Hz tone at level 4 plays at 2760 Hz, the speed control −20.0 dB and stage 10 −33.5 dB, **−53.6 dB** in all; at level 5 it moves to 3220 Hz (above Nyquist) and the largest residue is −81.4 dB. Under (a) (a codec clock change) the tone plays at 2.76 / 3.22 kHz at about 0 dB, the receive filter scaling with the clock. These replace the review's −37 dB, computed for an earlier filter (spec Clarifications).

## R-12: Risks

- **Level-3 identity broken by the refactor** (the most likely defect): guarded by the existing transport tests unchanged, the eight goldens checked in Phase 2 right after the core change, and a level-3 row in `speed_checks`.
- **Off-by-one in the tail or the latch**: the completeness check against the hooked constant-level computation; a mutant without the tail; a mutant with the look-ahead 27.
- **The drain's tail bound**: the tail moves stage 10's settling start by up to 46 device samples; re-checked at every level; if it fails, the drain gains ⌈46 · m ÷ l⌉ host samples at every level including 3 (a behavior change; the plan would stop and ask).
- **Cost on the runner**: the x86-64 runner reads 2 – 3× the development machine's in-build ratios for some code (010's clamp); the 25 % runner ceiling for `speed_cost` and 5 % for the A/B follow the 2026-10-17 chore.
- **Codec decode at level 5 in a 64-sample block**: at most one frame per 120 stored samples; at 48 kHz a 64-sample block computes at most 8 device outputs (11.2 stored samples), so at most one decode per block; leaving level 3 adds at most 29 pulls (one more decode at most).
