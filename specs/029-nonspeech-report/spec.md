# Feature Specification: The non-speech reshaping report

**Feature Branch**: `029-nonspeech-report`

**Created**: 2026-11-04

**Status**: Draft reviewed 2026-11-04 (coach 1 – 25, hardware B1 – B15, product M1 – M8, applied at the defaults below); clarified 2026-11-04 (five questions, every answer the recommended default). **Awaiting the owner's spec sign-off (D1 – D9).**

**Input**: The owner's request of 2026-11-04. A harness-only measurement feature: a report of how the modeled signal chain reshapes **noise, bursts and silence** stage by stage, stated in figures. The project's records and the guide describe the chain's effect on such input in words (band-limiting; the coder's level, envelope and periodicity effects; the gate's behaviour on the capsule's hiss; spec 008's silence residue and 2 kHz lift; spec 012's gate margin); app creators who feed the engine room sound have no figures for what comes out. A scratch probe on 2026-11-04 (6 kHz, seeded noise through an ablation ladder; the hardware review re-ran and extended it at three rates) showed the effects are measurable with frame-level measures and that a synthetic harmonic reference reads an order of magnitude higher than every noise cell on the periodicity measures, so the measures respond. Two of the probe's figures turned out to be reader artifacts (the hardware review's B1, B3) and are not carried. **No library change, no audio change, no listening test, no statement, either way, about what a listener perceives.**

**App consumer**: the Bench tab and the workbench (a reference table for the A/B renders they already make); the guide's readers; the app's own copy about the chain's behaviour on noise (product M8: the golden is the source of any such figure).

## Clarifications

### Session 2026-11-04 (the draft reviews, applied at the defaults)

- The golden covers a **fixed rate set** (6, 44.1, 48 kHz), not `measurement_rates()`, which changes under coverage (coach 1, hardware B4); the other four rates are reported by the matrix only.
- The noise stimuli are **generated at the host rate through a deterministic low-pass at 2900 Hz with their level set in band (AES17)**, so every rate carries the same in-band level (coach 2, hardware B3); cross-rate agreement is reported, with tolerances recorded in a later session (coach 18).
- The reader uses **only the harness's deterministic math**, so the text golden is byte-identical on every CI target (coach 3, hardware B13).
- The quiet-room rung uses the **hiss offset +12 dB**, the range maximum, at least 2.5 dB above spec 012's stop estimates (coach 4, hardware B2, product M7).
- The **2 – 8 Hz envelope-modulation inequality is dropped** from the checks: computed over the stimulus span on active frames only, the coder slightly reduces the share (hardware B1); the measure keeps teeth of its own on the pass-through rows, and the C3 ÷ C1 ratio is a reported figure.
- The ladder is restated as **C0 → C1 → C3 → C4 → C5 one stage group each** (C2 the coder alone), the full-engine rungs read **before the output chain**, and **one extra rung at the speaker route** (hardware B5, coach 11).
- The four speech-named measures are **renamed for what they compute** (product M1, coach 17, hardware B14), and the guide's table compares every figure with the pass-through row and omits the harmonic reference row (product M2, M6).
- A **pink-noise stimulus** is added (product answer 5).
- The scratch probes of 2026-11-04 are **not part of the feature** and are not committed (coach 20, product M4); the reader is written fresh under the harness's math rules.

### Session 2026-11-04 (clarify)

- Q: Which host rates should the text golden cover? → A: Three fixed rates, 6, 44.1 and 48 kHz (`NONSPEECH_GOLDEN_RATES`); the other four reported by the matrix only (D1, D3).

- Q: How are the noise stimuli defined so every rate carries the same in-band signal? → A: Uniform noise at the host rate through a deterministic Kaiser low-pass at 2900 Hz, the level set as the in-band AES17 RMS; each rate its own realisation, so the cross-rate rows are a reading (D2).

- Q: What hiss offset does the quiet-room rung C6 use? → A: +12 dB, the range maximum, at least 2.5 dB above spec 012's stop estimates; the app's quiet-room preset is not settled and may quote this rung (D3, D6 iii).

- Q: Does the guide's table show the harmonic reference row? → A: No; it stays in the golden and the checks, and the guide compares every figure with the pass-through row of the same input (D8).

- Q: Is the 2 – 8 Hz envelope-modulation claim dropped from the checks and the guide? → A: Yes; no inequality on the share between configurations; the measure keeps its own teeth on the pass-through rows; the guide prints the C3 ÷ C1 ratio as a figure with no directional sentence (D6 v, D8).

## Decisions for sign-off

| # | Decision | Recommended |
|---|---|---|
| D1 | **A harness report with a text golden.** A new harness binary prints the report: a header (the report's version line without the library version; the stimulus seeds and parameters; the nine definitions with their constants; the pass-through floors of the estimator measures), then one table per rate of the fixed set `NONSPEECH_GOLDEN_RATES = [6000, 44_100, 48_000]`, rows in stimulus order (a – h) × configuration order (C0 – C7), fixed columns, fixed precision (level 0.01 dB, Hz 0.1, fractions and shares 0.001, counts integers), "—" for an undefined figure. The text is pinned as a canonical golden (`golden/nonspeech-v1.txt`): compared byte for byte and its SHA pinned in the test like the audio guards; re-blessed only by `RR_DR60_BLESS=nonspeech cargo test -p rr_dr60_harness --test golden_nonspeech`, which asserts D5's block-size identity and D6's checks at bless time; one CHANGELOG entry per re-bless; a later model change re-blesses it inside that feature's own bless window. No figure is asserted against an absolute bound; D6's relative checks and separation margins are the only assertions. | **Report + text golden at three fixed rates; figures reported, consistency asserted** |
| D2 | **Stimuli**: seeded, synthetic, 12 s each, generated at the host rate; every noise is uniform white noise through a deterministic Kaiser low-pass (the design of the harness's band-limited noise with its lower edge at 0 and its upper edge at 2900 Hz, about 10 ms long), which makes it Gaussian and puts the same share of its power in the device band at every rate; every level is the AES17 RMS of the final stimulus over its whole length. (a) steady noise at −35, −50 and −55 dBFS; (b) the same noise with the envelope 1 + 0.5·sin(2π·4t), at −35 dBFS after modulation; (c) the same noise under a slow irregular envelope (three incommensurate sines at 0.7, 1.9 and 3.3 Hz, ±15 % each) at −40 dBFS; (d) sparse bursts: sin² envelopes of 10 – 160 ms at −45 … −30 dBFS (a burst's level is its RMS over its span), separated by 0.1 – 1.1 s gaps, from a seeded sequence drawn in seconds and rounded to whole samples at 6 kHz before conversion; (e) **pink noise** (a 1/f shaping of the same low-passed noise) at −45 dBFS; (f) digital silence; (g) the harmonic reference: the harness's synthetic glottal-pulse stimulus (spec 008 R-09) at −20 dBFS; (h) the teeth row for the peak measure: steady noise (a, −35) through a static 750 Hz, Q 1.8 resonator generated by the harness. Seeds and parameters are constants of the report. | **Eight stimuli, the harmonic reference and a teeth row among them** |
| D3 | **The ladder**, each configuration named in `configs::settings` with its tap: C0 pass-through (`bypass_all`); C1 the band filters and the ADC clamp (`default`: the coder off, the output chain bypassed at `AfterPlayback`); C2 the coder alone (`codec_only`, `Tap::AfterDecoder`); C3 band + coder, the gate and limiter off, at `AfterPlayback`; C4 C3 + the microphone (capsule and amplifier) + the limiter, the gate off, the output chain bypassed, at `AfterPlayback`; C5 C4 + the gate at level 3 (the default engine before the output chain); C6 C5 at sensitivity 5 with the hiss offset **+12 dB** (the gate stays open on the capsule's hiss alone, a quiet room); C7 C5 at the speaker route output (the default engine as it plays; the only rung with stages 11 – 12; reported, never asserted). C0 → C1 → C3 → C4 → C5 each add one stage group; C6 and C7 are variants of C5. A limiter-off twin of C4 is bit-identical on every stimulus (the stimuli stay below the knee; spec 005's unity below the knee is bit-exact). The checks of D6 run at the measurement rates; the golden at D1's fixed set; the matrix reports the other four rates. | **Eight configurations, the full engine read before the output chain, the route as one rung** |
| D4 | **The measures**, computed by one reader on the output resampled to 6 kHz by a deterministic FIR, in 20 ms frames (120 samples) over the **stimulus span** (from a 0.5 s settle to the stimulus end, both shifted by the configuration's latency; the output is the processing calls' returned samples, no drain; in drop mode the span is counted in kept output), over **active** frames (frame RMS above −75 dBFS); every frame-to-frame state resets on an inactive frame and, in drop mode, at each splice (the events are available): (1) **LPC envelope peaks per frame**: maxima of the order-8 LPC envelope (pre-emphasis 0.9) in 200 – 2300 Hz with ≥ 3 dB prominence over ±300 Hz and a −3 dB width ≤ 300 Hz; (2) **low-peak movement**: the mean, over consecutive active-frame pairs each with ≥ 2 peaks, of the mean absolute change of the two lowest peaks (Hz); (3) **periodicity**: the mean of the frame's maximum of r(τ) = Σ x[n]·x[n−τ] ÷ √(Σ x[n]² · Σ x[n−τ]²) over 120 fixed products from a 220-sample buffer, τ = 20 … 100 (300 – 60 Hz), interior maxima only; (4) **periodic-frame fraction**: frames with that maximum > 0.5; (5) **lag-stable fraction**: consecutive active-frame pairs whose lags agree within ±2 at maxima > 0.35; (6) **2 – 8 Hz envelope-modulation share**: on cells whose span is ≥ 90 % active, the 2 – 8 Hz part of the Hann-windowed log-envelope DFT (50 Hz frame rate, DC removed) over 0.5 – 25 Hz, by DFT bins; "—" otherwise; (7) **spectral flux**: the RMS frame-to-frame difference of the mean-removed LPC envelope (dB); (8) the **active-frame count** (of ⌊11.5 s ÷ 20 ms⌋ = 575 in the span); (9) the **level** (AES17 RMS dBFS over the span). Measures 2, 5 and 7 are estimator figures whose pass-through floor the header prints; each cell prints them beside its C0 value; they enter no check. Every definition is a constant of the report and printed in its header. | **Nine measures, defined in the report, named for what they compute** |
| D5 | **Determinism and rates**: the output of every cell is bit-identical across block sizes, so the text is identical: asserted at 6 and 48 kHz on C2 and C5 with stimuli (a, −35) and (g) at block sizes 1, 64, 4096 and an irregular partition; every other cell runs at 4096. Across the fixed rates the measures of one cell agree within tolerances recorded in a later Clarifications session (the rule: 2 × the printed spread, floor 0.01 or one frame; the route rung C7 keeps a per-rate tolerance; measures reading near the resampler's 2700 Hz edge are excepted); reported in the first version, then asserted. Under `cfg(coverage)` the golden comparison is ignored (the plain build and CI's golden targets are authoritative) and the checks run at the measurement rates; the plan records the measured minutes. | **Block-size identity asserted; cross-rate agreement reported, then bounded; the golden off under coverage** |
| D6 | **The checks** (asserted; each cites its probe figure → bound → margin under Assumptions): (i) **separation**: the harmonic reference's periodic-frame fraction ≥ 0.9 and LPC peaks per frame ≥ 2.5 in C0 – C3 (observed 0.976 – 0.998 and ≥ 2.84); every noise cell's periodic-frame fraction ≤ 0.1 (observed ≤ 0.045; "noise cell" = stimuli a – e in every configuration and f in C6); C4 – C7 report the reference without asserting it (stage 2's 818 Hz high-pass, A-046, removes its fundamental). (ii) **pass-through**: C0's measures equal the input's own, read by the same reader over the same span: bit for bit at 6 kHz; elsewhere within per-measure tolerances set from a printed row (continuous measures) and ±1 frame (counts), recorded in Clarifications. (iii) **silence**: C0 and C1 have zero active frames; C2 and C3 have zero active frames and a level below −75 dBFS (spec 008: ≤ −78 dBFS; margin 3 dB); C5 on silence has at most ⌈(onset + hang + latency) ÷ 20 ms⌉ − 25 active frames (the gate closes); C6 on silence has ≥ 90 % of 575 active frames (the gate stays open; observed 625 / 625 at +12 dB at three rates). (iv) **the coder's periodicity**: C3's mean periodicity ≥ C1's + 0.03 on every noise stimulus with ≥ 50 active frames in both cells (observed +0.05 … +0.06, standard error 0.002). (v) **the measures' own teeth on pass-through rows**: the 2 – 8 Hz share on (b) in C0 ≥ 0.8 and on (a, −35) ≤ 0.35 (observed 0.97 – 0.99 and 0.24 – 0.31); the noise rows' LPC peaks per frame in C0 and C1 ≤ 1.5; the resonator row (h) raises C0's peaks per frame by ≤ 0.2 over (a, −35) (the width rule); the reference's lowest peak's median in C0 within 250 – 800 Hz (the pre-emphasis rule). (vi) every reported figure is finite where the cell has active frames and "—" where it has none. | **Six checks with margins** |
| D7 | **Wording**: the report's strings, the column names and the guide section (D8) pass both of the project's word lists (`rr_dr60::wording`'s engine strings and claims) and a per-section phrase list asserted by the cross-check: no "voice", "speech-like", "words", "hear", "heard", "listener", "sounds like", "phantom", "illusion", "mistaken", "detect". The text describes what the chain does to noise, bursts and silence, names the stages and their registered IDs, says "computed", never "measured", and makes no statement that a figure resembles or fails to resemble speech. The measures are named for what they compute. | **Neutral both ways; both word lists and a phrase list** |
| D8 | **Records**: the guide gains a short section, "What the chain does to noise, bursts and silence": a generated region with the 6 kHz rows for stimuli a (−50), b, d and f in C1, C3, C5 and C6 and the columns level, active frames, periodicity, 2 – 8 Hz share and LPC peaks (each column header with its range; every figure beside the C0 row's; no reference row), regenerated by `RR_DR60_BLESS=readme` and checked by the cross-check; then one paragraph each for the band (A-027), the coder (A-004, A-040, A-041: the level it reads steady noise at, the periodicity it adds, the silence residue), the gate (A-022) on the hiss (A-048), and the route rung (A-010), each with two or three figures from the golden; the caption "The model's figures at engine 0.95.0, not a unit's; the coder is a class match (A-004)". The README's "What exists today" gains one line; CHANGELOG (Unreleased › Added); CLAUDE.md's status and the spec 029 report commands. No manifest, control or register change; no stage manifest re-bless; no version bump (harness and docs only). | **A trimmed guide section from the golden; no version bump** |
| D9 | **Out of scope**: any listening test or perceptual measure; any external voice-activity or speech classifier as a judge (rejected: the one tried keys on energy and does not separate noise from the reference); recorded room sound as a stimulus (determinism, provenance); a C or Swift surface; the workbench's adoption (its own issue; the product review's parked request E-029a, a published reader, after app 1.0); a live or per-moment read-out of these measures in the app (product answer 4); the 2026-11-04 scratch probes (not part of the feature, not committed). | **Synthetic, objective, harness-only** |

## User Scenarios & Testing *(mandatory)*

### User Story 1 - An app creator reads what the chain does to room sound (Priority: P1)

An app creator feeding the engine a quiet room wants to know, in figures, what comes out: how much the band, the coder and the gate each change steady noise, modulated noise, short bursts and silence. They open the guide's table (or run the report) and read per cell the level, the periodicity and the 2 – 8 Hz envelope-modulation share in the coder cells against the band-only cells, how many frames the gate keeps, and the silence residue.

**Why this priority**: it is the feature's purpose; the words in the records become figures.

**Independent Test**: run the report at the fixed rates and compare its text with the committed golden; read the guide section.

**Acceptance Scenarios**:

1. **Given** the committed golden, **When** the report runs at each of 6, 44.1 and 48 kHz, **Then** its canonical text equals the golden byte for byte (`golden_nonspeech`).
2. **Given** the steady noise stimuli, **When** read in C1 and C3, **Then** C3's periodicity is at least C1's + 0.03 on every one, and C3's level reads below C1's by a figure the report prints (D6 iv; the level reported).
3. **Given** digital silence, **When** read in C5 and C6, **Then** C5's active frames are within the gate's hang-and-onset bound and C6's are ≥ 90 % of the span (D6 iii).
4. **Given** the guide, **When** its section is read, **Then** every figure in it is the golden's, every paragraph names a stage and its IDs, and the word lists and the phrase list pass (D7, D8; `us2_cross_check guide_nonspeech`).

---

### User Story 2 - The project pins the chain's effect on noise, bursts and silence (Priority: P1)

A later feature changes a stage (a coder constant, the gate's onset, the capsule's hiss). The report's golden moves, the change is seen in review, and the CHANGELOG records the new figures, as the audio goldens do for audio.

**Why this priority**: without a pin the figures drift silently.

**Independent Test**: doctor one setting the report does not change and see the golden comparison fail on the right rows.

**Acceptance Scenarios**:

1. **Given** the committed golden, **When** the gate's hang is doubled in the C5 cells by a doctored configuration, **Then** the comparison reports a row-level diff in which the C5 rows where the gate pauses (silence, bursts, the −50 and −55 dBFS noise) differ and no C0 – C4 row differs; the test prints the moved rows (`golden_nonspeech doctored_hang`).
2. **Given** the committed golden, **When** the report runs with block sizes 64 and 4096, **Then** the two texts are identical (D5; `nonspeech_matrix partitions`).

---

### User Story 3 - The measures respond (Priority: P2)

The harmonic reference reads high on periodicity and peaks, the pass-through row reads the input's own figures, silent cells read empty, and the measures' own teeth rows hold.

**Why this priority**: a report whose measures could not separate the reference from noise, or that read its own estimator, would report nothing.

**Independent Test**: D6's checks in the plain build at the measurement rates.

**Acceptance Scenarios**:

1. **Given** the reference in C0 – C3, **When** read, **Then** its periodic-frame fraction ≥ 0.9 and peaks per frame ≥ 2.5; every noise cell's periodic-frame fraction ≤ 0.1 (D6 i; `us3_nonspeech separation`).
2. **Given** any stimulus in C0, **When** read, **Then** the measures equal the input's own within D6 ii's tolerances (`us3_nonspeech pass_through`).
3. **Given** a mutant reader, **When** the checks run, **Then** each fails a named D6 check (the golden is not an oracle for mutants): the prominence-and-width rule removed fails (v)'s peaks bound and the resonator row; the pre-emphasis removed fails (v)'s lowest-peak band; the lag range halved fails (i)'s reference fraction; the flush included in the span fails (v)'s 2 – 8 Hz share on (b); the frame-state reset removed fails (ii) on the bursts row (`mutation nonspeech::`).

---

### Edge Cases

- A cell whose output is all below −75 dBFS has no active frames: every per-frame measure prints "—", the level prints, and the checks that need frames skip with a stated reason.
- The gate in C5 pauses on the −50 and −55 dBFS noise and on silence: the active-frame count reports it; no measure assumes a continuous output; the 2 – 8 Hz share prints "—" below 90 % active.
- A 10 ms burst is shorter than a frame and than the gate's 20 ms onset: in C5 it may leave no kept sample; the row reports it.
- In drop mode the 0.5 s settle removes the first 0.5 s of kept output, not of input time; the report states it.
- The coder's 30 ms delay and the playback half's latency shift the output: the span's shift covers both at every rate; the pass-through comparison aligns by the configuration's latency.
- At 44.1 kHz the resampling to 6 kHz is a non-integer ratio and the FIR's edge is near 2700 Hz: the cross-rate tolerance absorbs it; the block-size identity is exact at every rate; the peak search stops at 2300 Hz.
- The bursts' seeded sequence is drawn in seconds, so the gaps and durations do not depend on the host rate.

## Requirements *(mandatory)*

- **FR-001**: D2's eight stimuli as seeded constants of the report, bit-reproducible at every rate (`nonspeech_checks`; `determinism nonspeech`).
- **FR-002**: D3's eight configurations named in `configs::settings` with their taps; the limiter-off twin of C4 bit-identical (`nonspeech_matrix configs`).
- **FR-003**: D4's nine measures by one reader under the harness's deterministic math (no `std` transcendental on the reader's or the resampler's path), the definitions printed in the header equal to the constants (`us3_nonspeech reader_definitions`).
- **FR-004**: D1's canonical text golden at the fixed rates, byte-compared and SHA-pinned, re-blessed only by the documented command with a CHANGELOG entry (`golden_nonspeech`; its guard counted by `us1_bypass`'s scanner, whose pin is restated 16 → 17 with a dated comment).
- **FR-005**: D5's block-size identity asserted; cross-rate agreement reported in the first version, bounded in a later session (`nonspeech_matrix`).
- **FR-006**: D6's six checks, each citing its decision and its margin, D6 ii with per-measure tolerances recorded in Clarifications (`us1_nonspeech`, `us3_nonspeech`).
- **FR-007**: D7's two word lists and the phrase list over the report's strings, the column names and the guide section (`us2_cross_check guide_nonspeech`, `us4_wording`).
- **FR-008**: D8's guide section and README line generated from the golden (`us2_cross_check readme`, `RR_DR60_BLESS=readme`).
- **FR-009**: the five mutant readers of US3 AS3 each fail a named check (`mutation nonspeech::`).
- **FR-010**: no library change: the `rr_dr60`, `rr_dr60_detmath` and `rr_dr60_ffi` trees untouched (read from the diff as spec 022's guard does); the seventeen audio and record golden files and every smoke hash unmoved (`us2_abi spec_029_untouched`).
- **FR-011**: the report runs under the harness's determinism and allocation suites (`determinism nonspeech`, `alloc_free nonspeech`).

## Success Criteria *(mandatory)*

- **SC-001**: The guide's table has a row for each of stimuli a (−50), b, d and f in C1, C3, C5 and C6 at 6 kHz, every figure the golden's (`us2_cross_check guide_nonspeech`).
- **SC-002**: A doctored gate hang moves the golden on the C5 rows where the gate pauses and nowhere else (US2 AS1).
- **SC-003**: The reference separates from every noise cell on the periodic-frame fraction and the peaks per frame by D6 i's margins at every measurement rate (US3 AS1).
- **SC-004**: The report is deterministic: identical text across block sizes and runs (US2 AS2); the same cells at the fixed rates agree within the tolerances recorded in Clarifications (FR-005).
- **SC-005**: Nothing in the report or the guide makes a statement about listeners; both word lists and the phrase list pass (FR-007).

## Assumptions

- The engine is at 0.95.0 (main at 4dad714).
- The measures are engineering descriptors of spectral and temporal structure (LPC peaks, autocorrelation, the envelope's modulation spectrum); the report makes no statement, either way, about what a listener perceives (D9).
- The figures describe the **modeled** chain: the coder is a class match (A-004, A-040, A-041), the speaker a template (A-010), the microphone's corner A-046; a figure attributed to a stage is a figure of that stage's model.
- **Probe figures → bounds → margins** (the hardware review's extended probe of 2026-11-04 at 6, 44.1 and 48 kHz; scratch, std math, to be reproduced in the harness before the records quote them): the reference's periodic-frame fraction 0.976 – 0.998 → ≥ 0.9; its peaks per frame ≥ 2.84 → ≥ 2.5; noise cells' periodic-frame fraction ≤ 0.045 → ≤ 0.1; C3 − C1 periodicity +0.05 … +0.06 (standard error 0.002) → ≥ +0.03; C6 on silence at +12 dB 625 / 625 frames → ≥ 90 % (at +9 dB: 339 / 160 / 533 frames at 6 / 48 / 44.1 kHz, the reason for +12); the 2 – 8 Hz share on (b) in C0 0.97 – 0.99 → ≥ 0.8, on steady noise 0.24 – 0.31 → ≤ 0.35; the silence residue ≤ −78 dBFS → < −75. Two reported figures with no check: the coder reads steady noise about 5 dB below the band-only cell (−33.2 → −38.6 dBFS at 6 kHz, −42.2 → −47.4 at 48 kHz; level-independent; spec 008 calibrated the level on voiced input only; an evidence gap: the G.723.1 reference under `tools/celp-calibrate/` on the D2 noise is the desk check, reported if the reference software is present), and the 2 – 8 Hz share C3 ÷ C1 (about 0.93 on the 4 Hz stimulus).
- **Alternatives**:
  - **D1**: figures asserted against bounds (rejected: the chain's behaviour on noise is not a device requirement); no pin (rejected: drift); all seven rates in the golden with a coverage prefix (the coach's default; rejected for the three fixed rates: the golden's cost and the cross-rate rows belong to the matrix).
  - **D2**: generating every stimulus at 6 kHz and raising it to the host rate (the coach's default; rejected: it needs a deterministic resampler on the stimulus path and makes the cross-rate rows an identity rather than a reading; revisit if the cross-rate spread is large); recorded room sound (rejected, D9); 100 % modulation depth as the probe ran (rejected: no check depends on the ratio now).
  - **D3**: the gate alone and the limiter alone as rungs (deferred: on these stimuli the gate's effect is the frame count and the limiter's is nothing below the knee, which the C4 twin proves).
  - **D4**: an external classifier as a tenth measure (rejected, D9); a mel-cepstral distance to the reference (deferred).
  - **D6**: the 2 – 8 Hz share inequality C3 ≥ 1.5 × C1 (rejected: a reader artifact; the corrected measure reads the other way).
  - **D8**: the table in the README (rejected: the README is the short front page; one line there); the reference row in the guide (rejected: it invites a comparison the report does not make).
