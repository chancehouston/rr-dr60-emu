# Feature Specification: The non-speech reshaping report

**Feature Branch**: `029-nonspeech-report`

**Created**: 2026-11-04

**Status**: Draft

**Input**: The owner's request of 2026-11-04. A harness-only measurement feature: a report of how the modeled signal chain reshapes **non-speech** input (steady and modulated noise, bursts, silence) stage by stage, stated in figures. The project's records and the guide describe the chain's effect on such input in words (band-limiting, the coder's envelope and periodicity effects, the gate's behaviour on the capsule's hiss, spec 008's silence residue and 2 kHz lift, spec 012's gate margin); app creators who feed the engine room sound have no figures for what comes out. A scratch probe on 2026-11-04 (6 kHz, seeded noise through an ablation ladder) showed the effects are measurable with frame-level measures and that a synthetic voiced control separates from every noise cell by an order of magnitude, so a report with teeth is feasible. **No library change, no audio change, no listening test, no claim about what listeners hear.**

**App consumer**: the Bench tab and the workbench (a reference table for the A/B renders they already make); the guide's readers.

## Decisions for sign-off

| # | Decision | Recommended |
|---|---|---|
| D1 | **A harness report with a text golden.** A new harness binary prints one table per measurement rate: rows are stimulus × configuration cells, columns the measures of D4. The table is pinned as a **canonical text golden** (`golden/nonspeech-v1.txt`, SHA-guarded like the manifest texts; re-blessed on an *intended* change with a CHANGELOG entry), so a later model change that moves the chain's effect on non-speech input shows up in review, as the audio goldens do for audio. The figures are not asserted against stated bounds (the chain's behaviour on noise is not a requirement of the device model); only D6's self-consistency checks assert. | **Report + text golden; figures reported, consistency asserted** |
| D2 | **Stimuli**: fixed, seeded, synthetic, 12 s each, at the host rate: (a) steady uniform white noise at −35, −50 and −55 dBFS RMS; (b) the same noise amplitude-modulated at 4 Hz (50 % depth) at −35 dBFS; (c) the same noise under a slow irregular envelope (three incommensurate sines at 0.7, 1.9 and 3.3 Hz, ±15 % each) at −40 dBFS; (d) sparse bursts: raised-sine envelopes of 10 – 160 ms at −45 … −30 dBFS separated by 0.1 – 1.1 s gaps, from a seeded sequence; (e) digital silence; (f) the control: the harness's synthetic voiced stimulus (spec 008 R-09) at −20 dBFS. Seeds and parameters are constants of the report. | **Six stimuli, the voiced control among them** |
| D3 | **The ladder**: seven configurations differing by one stage group each: C0 pass-through (`bypass_all`); C1 the band filters only (`default`); C2 the coder only (`codec_only`, the decoder tap); C3 band + coder, gate and limiter off; C4 the full default engine (microphone, limiter, gate at level 3, coder, speaker route); C5 the full engine at sensitivity 5 with the hiss offset +9 dB (spec 012's gate margin: the gate stays open on an empty room); C6 the full engine with the gate bypassed. The measurement rates (`configs::measurement_rates`). | **Seven configurations, the measurement rates** |
| D4 | **The measures**, per 20 ms frame on the output resampled to 6 kHz (the harness's resampler; the chain's band lies inside it), after a 0.5 s settle, over frames above −75 dBFS ("active"): (1) formant-like peaks per active frame: maxima of the order-8 LPC envelope (pre-emphasis 0.9) in 150 – 2800 Hz with ≥ 3 dB prominence over ±300 Hz; (2) the mean frame-to-frame movement of the two lowest peaks (Hz); (3) periodicity: the mean of the frame's maximum normalised autocorrelation over lags 20 – 100 samples (300 – 60 Hz); (4) the voiced-frame fraction (that maximum > 0.5); (5) the sustained-pitch fraction (lag within ±2 of the previous frame's at a maximum > 0.35); (6) the syllabic share: the 2 – 8 Hz part of the log-envelope modulation spectrum over 0.5 – 30 Hz (50 Hz frame rate, DC removed); (7) spectral flux: the RMS frame-to-frame difference of the mean-removed LPC envelope (dB); (8) the active-frame count; (9) the output level (RMS dBFS, AES17). Every definition is a constant of the report and printed in its header. | **Nine measures, defined in the report** |
| D5 | **Determinism and rates**: the output of every cell is bit-identical across block sizes (1, 64, 4096 and an irregular partition), so the measures are identical; across the measurement rates the measures of one cell agree within stated tolerances (reported in the first version, asserted once the figures are known: the plan sets them from the printed spread). The report runs under the harness's determinism and allocation suites like every other report. | **Block-size identity asserted; cross-rate agreement reported, then bounded** |
| D6 | **The checks with teeth** (asserted): (i) the control separates: its voiced fraction ≥ 0.9 and its peaks per frame ≥ 2.5 in every configuration up to C3, and every noise cell's voiced fraction ≤ 0.2; (ii) the pass-through cell's measures equal the input's own, computed by the same reader, bit for bit at 6 kHz and within the resampler's tolerance elsewhere; (iii) the silent cells: C0 and C1 have zero active frames; C2 and C3 have zero active frames and a level below −75 dBFS (spec 008's residue); C4 closes the gate (active frames bounded by the hang + onset); C5 and C6 run to the end (active frames ≥ 90 % of the frames); (iv) the directional findings the probe showed, as inequalities with margin: on the 4 Hz stimulus the syllabic share in C3 is at least 1.5 × C1's; the periodicity in C3 exceeds C1's on every noise stimulus; (v) every reported figure is finite where the cell has active frames, and marked "—" where it has none. Each check names the requirement it serves. | **Five checks** |
| D7 | **Wording**: the report's strings and the guide section (D8) pass the project's wording check (`rr_dr60::wording`, the harness denylist): they describe what the chain does to non-speech input, name the stages and their registered sources and assumptions, and make no statement about listeners, hearing, voices, words or interpretation. The measures are named for what they compute ("formant-like spectral peaks", "periodicity"), never for what they might mean. | **Neutral; the denylist extended to the report** |
| D8 | **Records**: the guide gains a short section, "What the chain does to non-speech input", with the 6 kHz table copied from the golden as a generated region (the manifest table's mechanism) and one paragraph per effect pointing at the stage and its IDs; the README's "What exists today" gains one line; CHANGELOG (Unreleased › Added); CLAUDE.md's status and the spec 029 report commands. No manifest, control or register change; no version bump (harness and docs only; the library is untouched, as spec 022's core-untouched guard reads from the diff). | **Guide section from the golden; no version bump** |
| D9 | **Out of scope**: any listening test or perceptual measure; any external speech or voice-activity detector as a judge (the probe found a WebRTC-style detector keys on energy, calling normalised white noise speech 93 % of the time); recorded room sound as a stimulus (synthetic only, so the report stays deterministic and small); a C or Swift surface; the workbench's adoption (its own issue). | **Synthetic, objective, harness-only** |

## User Scenarios & Testing *(mandatory)*

### User Story 1 - An app creator reads what the chain does to room sound (Priority: P1)

An app creator feeding the engine a quiet room wants to know, in figures, what comes out: how much the band, the coder and the gate each change steady noise, modulated noise, short bursts and silence. They open the guide's table (or run the report) and read per cell the level, the periodicity the coder adds, the syllabic modulation it deepens, how many frames the gate keeps, and the silence residue.

**Why this priority**: it is the feature's purpose; the words in the records become figures.

**Independent Test**: run the report at the measurement rates and compare its table with the committed golden; read the guide section.

**Acceptance Scenarios**:

1. **Given** the committed golden, **When** the report runs at 6, 44.1 and 48 kHz, **Then** its canonical text equals the golden byte for byte.
2. **Given** the 4 Hz-modulated noise, **When** read in C1 and C3, **Then** C3's syllabic share is at least 1.5 × C1's and its periodicity exceeds C1's (D6 iv).
3. **Given** digital silence, **When** read in C4, C5 and C6, **Then** C4's active frames are bounded by the gate's hang and onset, and C5 and C6 run to the end at a level the report prints (D6 iii).
4. **Given** the guide, **When** its section is read, **Then** every figure in it is the golden's, every paragraph names a stage and its IDs, and the wording check passes (D7, D8).

---

### User Story 2 - The project pins the chain's effect on non-speech input (Priority: P1)

A later feature changes a stage (a coder constant, the gate's onset, the capsule's hiss). The report's golden moves, the change is seen in review, and the CHANGELOG records the new figures, as the audio goldens do for audio.

**Why this priority**: without a pin the figures drift silently.

**Independent Test**: doctor one constant through a test hook or a settings change the report does not make (the hiss offset, the gate's hang) and see the golden check fail on the right rows.

**Acceptance Scenarios**:

1. **Given** the committed golden, **When** the gate's hang is doubled in the C4 cells by a doctored configuration, **Then** the golden comparison fails on the silence and burst rows of C4 and nowhere else.
2. **Given** the committed golden, **When** the report runs twice with block sizes 64 and 4096, **Then** the two texts are identical (D5).

---

### User Story 3 - The measures have teeth (Priority: P2)

The reader can tell speech-like structure from its absence: the voiced control scores far above every noise cell on voicing and peaks; the pass-through cell reads the input's own measures; silent cells read empty.

**Why this priority**: a report whose measures cannot separate the control from noise would report nothing.

**Independent Test**: D6's checks in the plain build.

**Acceptance Scenarios**:

1. **Given** the control in C0 – C3, **When** read, **Then** its voiced fraction ≥ 0.9 and peaks per frame ≥ 2.5; every noise cell's voiced fraction ≤ 0.2 (D6 i).
2. **Given** any stimulus in C0, **When** read, **Then** the measures equal the input's own (D6 ii).
3. **Given** a mutant reader (the prominence rule removed, the pre-emphasis removed, the lag range halved, the settle skipped), **When** the checks run, **Then** each mutant fails a named check or moves the golden.

---

### Edge Cases

- A cell whose output is all below −75 dBFS has no active frames: every per-frame measure prints "—", the level prints, and the checks that need frames skip with a stated reason.
- The gate in C4 may pause mid-stimulus on the −50 and −55 dBFS noise: the active-frame count reports it; no measure assumes a continuous output.
- The coder's 30 ms delay and the playback half's latency shift the output: the settle skip covers both at every rate; the pass-through comparison (D6 ii) aligns by the configuration's latency.
- At 44.1 kHz the resampling to 6 kHz is a non-integer ratio: the cross-rate tolerance (D5) absorbs it; the block-size identity is exact at every rate.
- The bursts' seeded sequence must not depend on the host rate: the gaps and durations are drawn in seconds and converted.

## Requirements *(mandatory)*

- **FR-001**: D2's six stimuli as seeded constants of the report, bit-reproducible at every measurement rate (`nonspeech_checks`).
- **FR-002**: D3's seven configurations built on the harness's existing ones (`configs`), each differing from its neighbour by one stage group.
- **FR-003**: D4's nine measures, computed by one reader at 6 kHz with the definitions printed in the report's header.
- **FR-004**: D1's canonical text golden, SHA-guarded, re-blessed only by the documented command with a CHANGELOG entry (`golden_nonspeech`).
- **FR-005**: D5's block-size identity, asserted; cross-rate agreement reported in the first version (`nonspeech_matrix`).
- **FR-006**: D6's five checks (`us1_nonspeech`, `us3_nonspeech`), each citing its decision.
- **FR-007**: D7's wording check over the report's strings and the guide section (`us2_cross_check guide_nonspeech`, `us4_wording`).
- **FR-008**: D8's guide section and README line generated or copied from the golden (`us2_cross_check readme`, `RR_DR60_BLESS=readme`).
- **FR-009**: the four mutant readers of US3 AS3 each fail a named test (`mutation nonspeech::`).
- **FR-010**: no library change (the `rr_dr60`, `rr_dr60_detmath` and `rr_dr60_ffi` trees untouched, read from the diff as spec 022's guard does); the seventeen golden files and every smoke hash unmoved.

## Success Criteria *(mandatory)*

- **SC-001**: A reader of the guide can state, in figures, how each of the band, the coder and the gate changes steady noise, modulated noise, bursts and silence, without running anything.
- **SC-002**: A change to any modeled stage that alters the chain's effect on non-speech input moves the golden and is seen in review.
- **SC-003**: The control separates from every noise cell on voicing and peaks by the margins of D6 at every measurement rate.
- **SC-004**: The report is deterministic: identical text across block sizes and runs; the same cells at the measurement rates agree within the tolerances the plan sets.
- **SC-005**: Nothing in the report or the guide makes a statement about listeners; the wording check passes.

## Assumptions

- The engine is at 0.95.0 (main at 4dad714).
- The measures are engineering descriptors of spectral and temporal structure (LPC peaks, autocorrelation, modulation spectrum); the report does not assert that any of them corresponds to what a listener perceives (D9).
- The figures describe the **modeled** chain: the coder is a class match (A-004, A-040, A-041), the speaker a template (A-010); a figure attributed to a stage is a figure of that stage's model.
- The probe's figures (2026-11-04, 6 kHz, uncommitted) are the expectation for D6 iv: syllabic share 0.45 → 0.87 on the 4 Hz stimulus, periodicity 0.23 → 0.30; they are not asserted as values.
- **Alternatives**:
  - **D1**: figures asserted against bounds (rejected: the chain's behaviour on noise is not a device requirement; a text golden pins it without pretending it is); no pin, a report only (rejected: drift).
  - **D2**: recorded room sound (rejected, D9); a pink-noise stimulus (deferred; the band removes most of the difference).
  - **D3**: the gate alone and the limiter alone as rungs (deferred: their effects on these stimuli are the gate's frame count and nothing for the limiter below the knee).
  - **D4**: an external detector as a tenth measure (rejected, D9); a mel-cepstral distance to the control (deferred).
  - **D8**: the table in the README (rejected: the README is the short front page; one line there).
