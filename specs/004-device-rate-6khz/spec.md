# Feature Specification: 6 kHz Device Rate and Codec Filter Templates

**Feature Branch**: `004-device-rate-6khz`

**Created**: 2026-10-11

**Status**: Draft

**Input**: User description: "Adopt the RR-DR60's measured 6 kHz device sample rate and the codec's PLL-scaled filter templates for the record and playback band-limit stages (signal-chain stages 4 and 10; S-002, S-005, A-026, A-027), replacing the assumed 8 kHz rate and the 300–3400 Hz G.712-like band shared by both stages (A-001, A-002, A-014, to be marked superseded; A-016 minimum phase is kept and upgraded, since S-005's non-zero, edge-rising group-delay distortion supports it). This is an assumption-change feature with no new stage: the emulator keeps the same stages, controls, taps, latency reporting and interfaces, but every signal now passes through a 6 kHz device rate (Nyquist 3 kHz), and the two band-limit stages become different filters, each designed to the datasheet's typical values with the datasheet limits (scaled by 0.75) as measurable tolerances. Record stage (transmit template, a band-pass): loss relative to 765 Hz within −0.15/+0.2 dB from 225 to 2250 Hz, between 0 and 0.8 dB at 2550 Hz (typical 0.4 dB), at least 20 dB at and below 45 Hz, at least 40 dB at DC; group-delay distortion relative to its minimum over the band at most 1.0 ms at 375 Hz, 0.47 ms at 450 Hz, 0.17 ms from 750 to 1950 Hz and 1.0 ms at 2100 Hz (typical 0.25 ms at 375 Hz). Playback stage (receive template, a low-pass only: the datasheet's receive filter has no low-frequency rejection and zero group-delay distortion below 1 kHz, so stage 10 passes DC): loss within −0.15/+0.25 dB from DC to 2250 Hz, between 0 and 0.8 dB at 2550 Hz (typical 0.56 dB), group-delay distortion at most 0.17 ms from 375 to 1950 Hz and 1.0 ms at 2100 Hz. The datasheet's 30 dB "discrimination" above 3450 Hz is an anti-alias requirement measured at the host rate through the record stage: any input from 3450 Hz up to the host Nyquist frequency must be at least 30 dB down in total output power including alias products; the device's partial aliasing of 3000–3450 Hz content and the playback DAC's imaging residue are out of scope and documented as such. Host rates: 6 kHz is added as a supported host rate (the new identity rate), alongside 8, 16, 44.1, 48, 88.2 and 96 kHz, so every clause in specs 001–003 that relies on "the 8 kHz host rate (identity)" is restated at 6 kHz; 8 and 16 kHz become non-integer conversions. The rate-conversion boundary is restated (spec 001 FR-005): flat within ±0.1 dB from 50 to 2550 Hz, 2550–3000 Hz may be attenuated, at least 60 dB rejection from 3000 Hz up including alias and image products, with the default latency budget of spec 001 unchanged. Behaviors to restate with the laws themselves unchanged: spec 001 FR-010's per-stage tolerances and measurement frequencies (the 4000 Hz point becomes 3000 Hz; the DC and 60 Hz rejection become record-stage-only); spec 002's AGC detector band (A-019, 4 kHz to 3 kHz), FR-008 tones (225, 375, 750, 1500, 2000 and 2550 Hz, phase sweeps at 1500 and 2000 Hz) and FR-009 noise band (225–2550 Hz), with the detector coefficients expected to be unchanged because the design is normalized; spec 003's VAS figures in device samples at 6 kHz, its FR-006 frequency set (225, 1000 and 2550 Hz) with the 1 kHz stimulus phase-aligned so a sample-peak detector reads the true peak at 6 samples per cycle, its length clauses (exact at 6, 48 and 96 kHz, within ±1 host sample elsewhere, transient lead bound ⌈host rate / 6000⌉), and the continuity window kept at 32 device samples. Policy for this assumption change: one CHANGELOG entry listing every restated figure; all three golden files and their stimulus definitions re-blessed in one commit with the SHA guards updated; A-001, A-002 and A-014 marked superseded and A-027's receive row corrected in the register. Success is measured by the harness: both stage templates and the anti-alias requirement pass at every supported host rate, output is bit-identical across platforms and block partitions, the spec 002 and 003 checks pass with only the explicitly restated figures changed, and a deliberate shift of either stage's band edge is caught. Out of scope: the AGC limiter revision (A-028), sensitivity-setting placement (A-033), µ-law quantization (A-003), idle noise and level anchoring (A-030), the CELP codec, the device's own aliasing and imaging residue, power amp, speaker and earphone modeling."

## Overview

This feature changes no stage's job and adds no stage. It replaces two assumed values that every earlier spec stands on with the values the service manual (S-002) and the codec family datasheet (S-005) give, read in full on 2026-10-10:

- The device rate becomes **6 kHz** (A-026, verified), not the 8 kHz assumed in spec 001 (A-001). Nothing above 3 kHz exists inside the device.
- The two band-limit stages become **two different filters** that follow the codec's templates scaled to the 6 kHz clock (A-027): the **record stage** (stage 4, the codec's transmit path) is a band-pass that passes about 225 to 2550 Hz, and the **playback stage** (stage 10, the codec's receive path) is a low-pass only, with no low-frequency rejection. Spec 001 modeled both as one 300–3400 Hz band-pass (A-002, A-014).

Everything downstream is measured in device samples or in the device band, so this is the right moment to change: the AGC (spec 002) and the VAS (spec 003) are built, and the speech codec, whose frame timing derives from the device rate, is not. The AGC and VAS **laws are unchanged**; only the figures that depend on the device rate or the device band are restated, and this spec lists every one of them. Every golden reference changes, once, in one commit.

There is still no physical unit and no reference recording. The template values are the datasheet's, scaled (A-027, medium confidence); the filter behavior between 2550 Hz and the 3 kHz Nyquist frequency is not given by any source and is reported, not toleranced.

Terms used in this spec (others are defined in specs 001 to 003):

- **Device rate**: the sample rate inside the emulated device, 6 kHz (A-026). Its Nyquist frequency is 3 kHz.
- **Host rate**: the sample rate the caller uses. The **identity rate** is the host rate equal to the device rate, where the rate conversion boundary does nothing and a stage's response is observed directly. It was 8 kHz; it is now 6 kHz.
- **Record template** / **playback template**: the loss and group-delay limits each stage must meet, taken from the datasheet's transmit and receive tables (S-005) with every frequency multiplied by 0.75 (A-027). Loss is measured relative to the stage's gain at the **reference frequency**, 765 Hz (0.75 × the datasheet's 1020 Hz).
- **Group-delay distortion**: a stage's group delay at a frequency minus its smallest group delay anywhere in the band 225–2550 Hz, as the datasheet defines it.
- **Anti-alias requirement**: how much of the host input above the device band may reach the output, measured at the host rate through the record stage, counting the output's total power including alias products.
- **Restated figure**: a number in specs 001, 002 or 003 that depended on the 8 kHz device rate or the 300–3400 Hz band and is replaced by this spec. The law it belongs to (what is measured, how, and to which tolerance) does not change.
- **Test frequency set**: the frequencies at which stage responses are measured: 20, 45, 100, 150, 225, 300, 375, 450, 600, 765, 1000, 1500, 1950, 2100, 2250, 2400, 2550, 2700 and 2900 Hz, plus DC (engineering target). The **anti-alias set**: 3450, 4000, 5000, 7000, 10 000, 15 000, 20 000 and 40 000 Hz, each used only below the host Nyquist frequency (engineering target).

## Clarifications

### Session 2026-10-11 (hardware-emulation review, before specification)

The hardware-emulation reviewer read the draft description against S-005 and the code before this spec was written. Its findings shaped the description; four decisions were put to the user and are recorded here so `/speckit-clarify` need not ask them again.

- Q: Should 6 kHz become a public supported host rate, or only an internal test path? → A: **Public.** It is the device's native rate, it costs nothing to support, and it gives every "exact at the identity rate" check in specs 001–003 a home (FR-002).
- Q: Fold µ-law quantization (A-003, verified) into this feature to save a later golden re-bless? → A: **No.** Quantization forces a clipping decision that only makes sense once the AGC keeps peaks at full scale (A-028, the next feature). This feature stays a pure rate and band change.
- Q: Rate conversion boundary transition band: 2550–3000 Hz (default latency about 10 ms) or 2700–3000 Hz (about 15 ms)? → A: **2550–3000 Hz** (FR-003). The template's top edge is 2550 Hz, and the lower latency keeps spec 001's 20 ms budget comfortable.
- Q: Design the stages to the datasheet's typical values, with the datasheet limits as tolerances? → A: **Yes.** The typical values (0.4 dB record and 0.56 dB playback loss at 2550 Hz, 0.25 ms group-delay distortion at 375 Hz) are design goals; the limits are the pass/fail tolerances (FR-005, FR-008).
- Finding without a decision: the datasheet's receive filter is a low-pass only (no 60 Hz row, zero group-delay distortion below 1 kHz, a 5th-order low-pass in the block diagram), so the playback stage passes DC (FR-008), and A-027's receive row is corrected (FR-014).
- Finding without a decision: the datasheet's "30 dB from 4.6 kHz up" is rejection of analog input above the band before sampling, so at 6 kHz it is an anti-alias requirement at the host rate (FR-007), not a point on the stage's response.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Hear the device's actual band (Priority: P1)

An app developer processes speech through the default pipeline and hears the RR-DR60's real bandwidth: nothing above 3 kHz, a top edge near 2.5 kHz, and a low end that rolls off gently below about 225 Hz, instead of spec 001's telephone band with its −3 dB points at 300 and 3400 Hz. A maintainer runs the harness and sees each band-limit stage measured against its own datasheet template, record and playback separately.

**Why this priority**: The device rate is the coordinate system for every stage. The speech codec's frame length, the AGC's and VAS's sample counts and every golden file depend on it. Changing it now, with three stages built, costs one re-bless; every stage built before the change would add another. The band edges are also the device's most audible fixed property.

**Independent Test**: Measure each stage alone (the other stage, the AGC and the VAS bypassed) at every host rate with the test frequency set, and check FR-005 to FR-008. This needs nothing from Stories 2 and 3 beyond the host rates spec 001 already supports.

**Acceptance Scenarios**:

1. **Given** the record stage alone at the 48 kHz host rate, **When** steady tones from the test frequency set are processed, **Then** the loss relative to 765 Hz is within −0.15/+0.2 dB from 225 to 2250 Hz, between 0 and 0.8 dB at 2550 Hz, at least 20 dB at 45 Hz and at 20 Hz, and at least 40 dB at DC.
2. **Given** the playback stage alone at 48 kHz, **When** the same tones and a steady DC input are processed, **Then** the loss relative to 765 Hz is within −0.15/+0.25 dB from DC to 2250 Hz (DC and 20 Hz included) and between 0 and 0.8 dB at 2550 Hz.
3. **Given** either stage alone at the 6 kHz host rate, **When** its group delay is measured across the band, **Then** the group-delay distortion is within the stage's limits (FR-006, FR-008), its phase is within ±5° of the minimum-phase response computed from its measured magnitude, and its group delay at 2550 Hz exceeds that at 1 kHz.
4. **Given** the record stage alone at any host rate above 6 kHz, **When** tones from the anti-alias set below the host Nyquist frequency are processed, **Then** each tone's total output power, including alias products, is at least 30 dB below the 765 Hz gain.
5. **Given** the default pipeline at 48 kHz, **When** a logarithmic sweep from 20 Hz to 20 kHz is processed, **Then** the output contains nothing above 3 kHz within 60 dB of the sweep level, and the −1 dB and −3 dB points of the combined response lie between 2.4 and 3.0 kHz (engineering target).

---

### User Story 2 - Use any host rate, including the device's own (Priority: P2)

An app developer whose app runs at 44.1 or 48 kHz keeps working with no API change: the pipeline still converts to and from the device rate at the boundary, with the same latency budget. A developer or test author who wants to see the device's samples directly creates the pipeline at 6 kHz, where the boundary is the identity and a bypassed pipeline returns its input unchanged. A developer at 8 or 16 kHz still gets correct output; those rates are now conversions rather than the identity.

**Why this priority**: Supporting the device rate as a host rate is what makes every "exact at the identity rate" check in specs 001–003 possible after the change, and it is the cheapest way for a host to get the raw device-rate stream. The boundary's restated response is what keeps the host-rate output honest about the new band.

**Independent Test**: Create pipelines at each of the seven host rates with both stages bypassed; check the boundary's response (FR-003), the identity at 6 kHz (FR-002), the rejection of an unsupported rate, and the latency (FR-004).

**Acceptance Scenarios**:

1. **Given** a pipeline at the 6 kHz host rate with every stage bypassed, **When** any finite input is processed, **Then** the output equals the input exactly, sample for sample, with zero latency, except that non-finite and subnormal input samples become 0.0 (spec 001 edge cases).
2. **Given** a pipeline at 8, 16, 44.1, 48, 88.2 or 96 kHz with every stage bypassed, **When** tones from 50 Hz to 2550 Hz are processed, **Then** the response is flat within ±0.1 dB, and any input from 3000 Hz up to the host Nyquist frequency is attenuated by at least 60 dB, including alias and image products.
3. **Given** a request for a 7 kHz, 11.025 kHz or 32 kHz host rate, **When** the pipeline is created, **Then** it is rejected with an error that names the seven supported rates, and no partially working pipeline is returned.
4. **Given** the default settings at every supported host rate, **When** latency is queried and measured, **Then** the reported latency is at most 20 ms and matches the measured 1 kHz group delay within ±1 sample.
5. **Given** a host that uses only the language-neutral interface, **When** it creates a pipeline at 6 kHz and processes a stimulus, **Then** its output is bit-identical to the native interface's.

---

### User Story 3 - Keep the AGC and VAS character while the ground moves (Priority: P3)

A maintainer re-runs the spec 002 and spec 003 harnesses after the rate change. Every check passes with only the figures this spec lists changed: the AGC still regulates to the same levels with the same attack and release, the VAS still pauses after the same hang time and clips the same onset, and nothing else about their behavior moved. The three golden files are re-blessed once, together, with a CHANGELOG entry that lists every restated figure, and the assumption register shows which assumptions this feature superseded, corrected or upgraded.

**Why this priority**: Principle II makes an assumption change a user-visible, recorded event, and Principle III forbids silent golden changes. The value of doing the rate change alone is being able to say "only the rate and the band moved"; this story proves it.

**Independent Test**: Run the spec 002 and spec 003 harnesses and golden checks with the restated figures (FR-011, FR-012); diff the CHANGELOG and the register against FR-013 and FR-014.

**Acceptance Scenarios**:

1. **Given** the spec 002 AGC checks with the restated tone frequencies and noise band, **When** the harness runs at every host rate, **Then** every check passes with the spec 002 levels, times and tolerances unchanged.
2. **Given** the spec 003 VAS checks with the restated frequency set and the phase-aligned 1 kHz stimulus, **When** the harness runs at every host rate, **Then** every check passes with the spec 003 threshold, hang time, onset time and tolerances unchanged, and lengths are exact at 6, 48 and 96 kHz and within ±1 host sample at the other rates.
3. **Given** the three golden files, **When** the feature is complete, **Then** each was re-blessed exactly once, in one commit with the restated stimulus definitions and the updated guards, and the CHANGELOG lists every restated figure, the new default latency and the superseded assumptions.
4. **Given** the assumption register, **When** the feature is complete, **Then** A-001, A-002 and A-014 are marked superseded with a pointer to this spec, A-027's receive row says the playback filter is a low-pass only, A-016 cites S-005's group-delay figures, and A-019's detector band reads 3 kHz.
5. **Given** a deliberate 100 Hz shift of the record stage's lower edge, or of either stage's upper edge, **When** the harness runs, **Then** both the response check and the golden check fail.

---

### Edge Cases

- **6 kHz host rate**: the identity rate. Checks at or above 3 kHz do not apply (there is no such content), as spec 001 said of 4 kHz at the old identity rate. The anti-alias requirement (FR-007) and the boundary's rejection clause (FR-003) are checked only at the other six rates.
- **8 and 16 kHz host rates**: no longer the identity. With every stage bypassed, the output is the input passed through the rate conversion boundary (FR-003), not the input itself. Content between 3000 Hz and the host Nyquist frequency is removed. Existing hosts at these rates see this as a change of output, recorded in the CHANGELOG.
- **DC input, default configuration**: a constant 0 dBFS DC input still gives steady-state output at or below −60 dBFS (engineering target, as spec 001), now from the record stage alone (FR-005): the playback stage passes DC.
- **DC input, playback stage only**: passes, within the playback template's tolerance (FR-008). This is new behavior; the device's receive filter does not block DC (S-005), and the low-frequency cut a listener hears comes from the record stage and, later, from the output coupling (A-031, A-032; out of scope).
- **Content between 2550 Hz and 3 kHz**: no source constrains either stage there. The harness reports each stage's loss at 2700 and 2900 Hz without a tolerance (FR-005, FR-008). The boundary may attenuate this band at host rates above 6 kHz (FR-003).
- **Content between 3000 and 3450 Hz in the host input**: the real device partially aliases it into the top of its band; the emulator removes it (FR-003). This deviation is documented, not modeled (Assumptions).
- **Tones whose period is a whole number of device samples** (3000, 2000, 1500, 1200, 1000, 750, 600, 500 … Hz) stay phase-locked to the device grid at every integer-ratio host rate. A sample-peak detector reads such a tone low by up to 1.25 dB at 6 samples per cycle (1 kHz) and 6 dB at 3 samples per cycle (2 kHz) when a sample never lands on the peak. Stimuli for sample-peak detectors are generated so that a device sample lands on the peak (FR-012); stimuli meant to probe phase sensitivity sweep the phase on purpose (FR-011).
- **Figures in device samples**: hang time 1.0 s is 6000 device samples (was 8000), onset time 20 ms is 120 (was 160), and each "one device sample" of slack in spec 003 is now 1/6000 s. The VAS continuity window stays 32 device samples (now 5.3 ms, was 4 ms): its invariant is the number of samples in a half-period of the lowest in-band tone (225 Hz at 6 kHz, 300 Hz at 8 kHz, both 13.3 samples), not a time.
- **End of stream / tail**: spec 001's 0.5 s flush still holds; the new stages' slowest mode decays faster than the old 300 Hz high-pass.
- **Reconfigure between host rates**, including to or from 6 kHz: as spec 001 FR-008, a reconfigured pipeline equals a fresh one with the new settings, with the new reported latency.
- **Block sizes 0 and 1, very large blocks, non-finite input, input beyond ±1.0, reset**: unchanged from spec 001.

## Requirements *(mandatory)*

### Functional Requirements

**Device rate and host rates**

- **FR-001**: Every stage that runs at the device rate (the two band-limit stages, the AGC and the VAS) MUST run at **6 kHz** (A-026, S-002). The rate conversion boundary MUST always be applied, even when every stage is bypassed, because the device rate is a property of the device. There is no setting to change the device rate. This replaces spec 001 FR-004 (A-001, superseded).
- **FR-002**: The pipeline MUST support exactly these host rates: **6 kHz**, 8 kHz, 16 kHz, 44.1 kHz, 48 kHz, 88.2 kHz and 96 kHz. Every supported rate MUST pass the full measurement harness. Any other rate MUST be rejected when the pipeline is created, with an error that names the supported rates. At the 6 kHz host rate the rate conversion boundary is the identity (no conversion), so with every stage bypassed the output equals the input exactly, except that non-finite and subnormal input samples become 0.0 (spec 001 edge cases). Wherever specs 001, 002 or 003 say "at the 8 kHz host rate, where the rate conversion is the identity" (001 FR-002, FR-010; 002 FR-006; 003 FR-004, FR-005, FR-010), they now mean the 6 kHz host rate. This replaces spec 001 FR-002.
- **FR-003**: The rate conversion boundary MUST NOT noticeably color the result: with every stage bypassed, the response from 50 Hz to 2550 Hz MUST be flat within ±0.1 dB; content between 2550 Hz and 3000 Hz MAY be attenuated; any input from 3000 Hz up to the host Nyquist frequency MUST be attenuated by at least 60 dB, and any alias or image products MUST be at least 60 dB below the stimulus. (Engineering targets, as spec 001 FR-005.) Checks at or above the host Nyquist frequency do not apply, so at the 6 kHz host rate the rejection clause is not checked. This replaces spec 001 FR-005.
- **FR-004**: Spec 001's latency requirements are unchanged in form: the reported latency MUST equal the measured group delay at 1 kHz within ±1 host sample and be constant for a configuration (001 FR-012), and with default settings the total latency MUST NOT exceed 20 ms at any supported host rate (001 FR-013). The reported values change with the new boundary and stages, and the new default latency at each host rate MUST be recorded in the CHANGELOG.

**Record stage: transmit template (A-027, S-005; the other stage, the AGC and the VAS bypassed; every supported host rate)**

- **FR-005**: The record stage (stage 4) MUST meet these tolerances. Its gain at the reference frequency, 765 Hz, is absolute (output level / input level); every other magnitude limit is relative to it. The stage's own response is the configuration's measured response minus the fully bypassed configuration's measured response (magnitude in dB, phase and group delay), which removes the rate conversion boundary, as in spec 001 FR-010.
  - Gain at 765 Hz: 0 dB ± 0.1 dB (A-015; the ±0.1 dB is an engineering target).
  - Loss from 225 Hz to 2250 Hz: within −0.15 dB and +0.2 dB (S-005 transmit limits at 300 and 3000 Hz, scaled; A-027).
  - Loss at 2550 Hz: between 0 and 0.8 dB (S-005 transmit limit at 3400 Hz, scaled). Design goal, not a tolerance: the typical value, 0.4 dB.
  - Attenuation at 45 Hz and at every lower test frequency above DC: at least 20 dB (S-005 "60 Hz ≥ 20 dB", scaled; typical 26 dB).
  - Attenuation at DC: at least 40 dB (carried from A-014's G.712-derived figure; S-005 gives no DC row. Engineering target).
  - Loss at 2700 Hz and 2900 Hz: reported, no tolerance. No source constrains the response between 2550 Hz and the 3 kHz Nyquist frequency (Assumptions).
  - Group delay at 1 kHz: at most 2 ms (A-016).
  - Group delay at 225 Hz and at 2550 Hz: each greater than the group delay at 1 kHz (A-016: minimum-phase, delay rising toward the edges).
  - Phase from 225 Hz to 2550 Hz: within ±5° of the minimum-phase response computed from the stage's measured magnitude (A-016; ±5° is an engineering target). Performed at the 6 kHz host rate, where the stage is observed directly; the coefficients are the same at every host rate, which the other checks confirm.
  - This replaces spec 001 FR-010 for the record stage (A-002, A-014 superseded).
- **FR-006**: **Record stage group-delay distortion** (group delay minus its minimum over 225–2550 Hz; S-005 transmit maxima scaled by 1/0.75; A-027): at most 1.0 ms at 375 Hz, 0.47 ms at 450 Hz, 0.17 ms at every test frequency from 750 to 1950 Hz, and 1.0 ms at 2100 Hz. Design goals, not tolerances: the typical values, 0.25 ms at 375 Hz, 0.15 ms at 450 Hz, about 0.03 ms at 750 Hz, 0.07 ms at 1950 Hz and 0.09 ms at 2100 Hz. These limits are what constrain how steep the stage's low end may be: the datasheet's low end is far gentler than spec 001's 300 Hz −3 dB point (Assumptions).
- **FR-007**: **Anti-alias requirement.** Measured at every host rate above 6 kHz through the record stage alone: for each tone of the anti-alias set below the host Nyquist frequency, the output's total power, including every alias product, MUST be at least 30 dB below the stage's 765 Hz gain (S-005 "discrimination, 4.6 to 72 kHz, ≥ 30 dB", scaled to 3450 Hz; A-027). The rate conversion boundary's 60 dB rejection (FR-003) already exceeds this; the check exists because it is the device requirement the boundary stands in for. The device's partial aliasing of 3000–3450 Hz content into the top of its band, and the receive path's imaging residue above 3450 Hz (S-005 "out-of-band spurious ≤ −35 dBm0"), are NOT modeled; the emulator rejects both by at least 60 dB, and the deviation is documented in the signal-chain inventory (FR-018).

**Playback stage: receive template (A-027 corrected, S-005; the other stage, the AGC and the VAS bypassed; every supported host rate)**

- **FR-008**: The playback stage (stage 10) MUST be a low-pass only, with no low-frequency rejection: the datasheet's receive filter is a 5th-order low-pass, its response table has no low-frequency rejection row, and its group-delay distortion is zero below 1 kHz (S-005). It MUST meet these tolerances, measured as in FR-005:
  - Gain at 765 Hz: 0 dB ± 0.1 dB (A-015; engineering target).
  - Loss from DC to 2250 Hz: within −0.15 dB and +0.25 dB (S-005 receive limit at 3000 Hz, scaled, and the typical −0.03 dB at 300 Hz; the −0.15 dB lower bound is carried from the transmit row as an engineering target). DC is checked with a steady DC input and the lowest test frequencies (20, 45 and 100 Hz) with tones.
  - Loss at 2550 Hz: between 0 and 0.8 dB (S-005 receive limit at 3400 Hz, scaled). Design goal: the typical value, 0.56 dB.
  - Loss at 2700 Hz and 2900 Hz: reported, no tolerance.
  - Group-delay distortion (over 225–2550 Hz): at most 0.17 ms at every test frequency from 375 to 1950 Hz and 1.0 ms at 2100 Hz (S-005 receive maxima scaled). Design goals: typical 0.00 ms up to 1000 Hz, 0.12 ms at 1950 Hz, 0.16 ms at 2100 Hz.
  - Group delay at 1 kHz: at most 2 ms (A-016). Group delay at 2550 Hz: greater than at 1 kHz (A-016).
  - Phase from 225 Hz to 2550 Hz: within ±5° of the minimum-phase response computed from the measured magnitude (A-016), at the 6 kHz host rate.
  - This replaces spec 001 FR-010 for the playback stage and spec 001 FR-006's "same nominal shape on both sides" (A-014 superseded).
- **FR-009**: **Both stages on.** At every test frequency from 50 Hz to 2900 Hz where the value is above −40 dB, the measured response MUST equal R_rec + R_play − R_base within ±0.3 dB, where R_rec and R_play are the single-stage responses and R_base the fully bypassed response, all in dB (as spec 001 FR-011; ±0.3 dB and −40 dB are engineering targets). The gain at 765 Hz MUST be 0 dB ± 0.2 dB (A-015). This replaces spec 001 FR-011.

**Restated figures in specs 001 to 003 (laws unchanged)**

- **FR-010**: **Spec 001.** FR-002, FR-004, FR-005, FR-006 (shape clause), FR-010 and FR-011 are replaced by FR-001 to FR-009 above. Spec 001's edge cases are restated as in this spec's Edge Cases (DC in the default configuration: at or below −60 dBFS, from the record stage alone). Every other spec 001 requirement, acceptance scenario and edge case is unchanged and MUST still pass at all seven host rates, including the determinism, real-time safety, reset, interface and harness requirements (001 FR-003, FR-007 to FR-009, FR-012 to FR-024).
- **FR-011**: **Spec 002 (AGC).** The AGC law is unchanged: A-017's levels, A-018's times and shape, A-019's behaviors and every spec 002 tolerance stay as written. Restated figures only:
  - A-019's detector band: "up to 3 kHz" (the device band), not 4 kHz. Spec 002's edge case "content above 4 kHz does not affect the AGC gain" becomes "above 3 kHz".
  - 002 FR-008 tones: 225, 375, 750, 1000, 1500, 2000 (exactly 6000/3) and 2550 Hz, replacing 300, 500, 1000, 2000, 2667 (exactly 8000/3) and 3400 Hz. The starting-phase sweeps (4 or more phases) move to 1500 Hz (4 samples per cycle) and 2000 Hz (3 samples per cycle), the frequencies where a peak detector is most phase-sensitive at the device rate. The ±0.5 dB level tolerance is relative to the 1 kHz value, as before; 1 kHz stays in the set as the reference and is phase-aligned per FR-012.
  - 002 FR-009 noise band: 225–2550 Hz, replacing 300–3400 Hz. Its level, bursts and ±2 dB / 20 dB / 50 ms limits are unchanged.
  - 002 FR-006's method note "at the 8 kHz host rate, where the rate conversion boundary is the identity" now reads 6 kHz (FR-002).
  - Every spec 002 check MUST pass with these restatements and nothing else changed.
- **FR-012**: **Spec 003 (VAS).** The VAS law is unchanged: A-022's threshold and 3 dB per level, A-023's hang time, A-024's onset time and abrupt splices, A-025's start state, and every spec 003 tolerance stay as written. Restated figures only:
  - 003 FR-006 frequencies: 225, 1000 and 2550 Hz, replacing 300, 1000 and 3400 Hz, with the threshold the same within ±1 dB at each. Its noise checks use the 225–2550 Hz band of FR-011.
  - **Stimulus phase rule**: a steady tone whose period is a whole number of device samples, used to measure a sample-peak detector (the VAS; 003 FR-006, FR-007 and the burst-gap stimulus at 1 kHz), MUST be generated so that a device sample lands on the tone's peak; at 6 kHz a 1 kHz tone starting at phase π/6 does this. Otherwise the detector reads the tone up to 1.25 dB low (cos 30°), outside the ±1 dB tolerance. This is a property of the emulator's detector, not of the device (Assumptions).
  - 003 FR-004 and FR-005 lengths: exact at the 6, 48 and 96 kHz host rates (integer conversion ratios), within ±1 host sample at 8, 16, 44.1 and 88.2 kHz; the transient lead bound becomes ⌈host rate / 6000⌉ host samples (16 at 96 kHz), and the settling condition ⌈host rate / 6000⌉ + 1 input samples. The exact-zero rule of 003 FR-004 and the bit-identity rules of 003 FR-004 and FR-010 apply at the 6 kHz host rate.
  - Hang and onset times stay 1.0 s and 20 ms; in device samples they are 6000 and 120. Each "one device sample" of slack in spec 003 is one sample at 6 kHz. The continuity window stays 32 device samples.
  - Every spec 003 check MUST pass with these restatements and nothing else changed.
- **FR-013**: **Golden references.** All three golden files (spec 001, AGC, VAS) MUST be re-blessed exactly once, in a single commit, together with the restated stimulus definitions (every golden stimulus whose frequency or band moved: the FR-009 noise band, the AGC tone list, the VAS burst tone and its phase) and the updated guards that pin the earlier files. The CHANGELOG MUST record, in one entry: the device rate change (A-026 adopted, A-001 superseded), the two templates (A-027 adopted, A-002 and A-014 superseded), the new host rate, the new default latency at each host rate, and every restated figure of FR-010 to FR-012. No other golden change is permitted in this feature.
- **FR-014**: **Assumption register.** A-001, A-002 and A-014 MUST be marked superseded with a pointer to this spec. A-027's receive row MUST be corrected to a low-pass-only template with the group-delay limits of FR-008, citing the S-005 rows. A-016 MUST be upgraded (status partially verified by S-005; confidence High) with the datasheet's group-delay figures at 6 kHz. A-019's band MUST read 3 kHz. The signal-chain inventory rows for stages 4 and 10 MUST point to this spec and list the un-modeled aliasing and imaging residue (FR-007) as known deviations.
- **FR-015**: **Traceability.** Every tolerance and default MUST cite its S-/A- ID in the spec, code and tests. The following are emulator engineering targets, labeled as such: the ±0.1 dB reference-gain tolerance and the ±0.2 dB combined tolerance (FR-005, FR-008, FR-009); the test frequency set and the anti-alias set (definitions); the 40 dB DC attenuation (FR-005); the −0.15 dB lower bound on the playback template (FR-008); the report-only frequencies 2700 and 2900 Hz; the ±5° phase tolerance and the 2 ms group delay (A-016); the boundary's ±0.1 dB / 60 dB figures and 2550–3000 Hz transition (FR-003); the ±0.3 dB / −40 dB combined-response figures (FR-009); the 20 ms latency budget and ±1 sample (FR-004); the −60 dBFS DC edge case and 0.5 s tail; the 100 Hz mutation shift (SC-007); the stimulus phase rule's π/6 (FR-012); and the 5 minutes in SC-001.
- **FR-016**: **Harness.** The measurement harness MUST check FR-002 to FR-009 and FR-011 to FR-012 at every supported host rate, with each result reporting its measured value, tolerance, requirement and A-/S- IDs (as 001 FR-020). Spec 001's determinism, real-time safety and reset requirements (001 FR-014 to FR-017, FR-022) MUST hold at all seven host rates; the 6 kHz host rate joins every matrix, golden and partition check.
- **FR-017**: **Interface.** No setting is added or removed. The supported host rate list grows by 6 kHz, in both the native and the language-neutral interface (001 FR-023, FR-024), with bit-identical output between them at every rate. The reported latency values change. The public interfaces are versioned surfaces under Semantic Versioning; the output change and the rate-list change are recorded in the CHANGELOG.
- **FR-018**: **Documentation.** The README's fidelity table rows for the codec band-limiting and its description of the band ("telephone-band", 8 kHz, 300–3400 Hz) MUST be updated to 6 kHz and the two templates, stating that the templates are the codec family's datasheet values scaled to the measured clock (S-005, A-027, medium confidence) and that the band between 2550 Hz and 3 kHz, the device's aliasing and the DAC imaging are not modeled. Tone stays neutral and technical (Principle VI).

### Out of Scope

The AGC limiter revision (A-028); the sensitivity setting's placement (A-033); µ-law quantization (A-003) and clipping; codec idle noise and the absolute level anchor (A-030); the speech codec (stages 7, 9); the device's own aliasing of 3000–3450 Hz content and the DAC's imaging residue (FR-007); noise reduction (stage 6); flash storage (stage 8); volume, power amp, speaker and earphone path (stages 11–13, A-031, A-032); the mic capsule and preamp (stages 1, 2); changing the device rate or the templates through settings.

### Key Entities

- **Device rate**: the fixed internal sample rate, 6 kHz (A-026). Not a setting.
- **Host rate**: one of seven supported rates; 6 kHz is the identity rate.
- **Record template** and **playback template**: the loss, group-delay-distortion and phase limits for stages 4 and 10 (A-027 corrected), each with its design goals (typical values) and tolerances (limits).
- **Rate conversion boundary**: the host-to-device and device-to-host conversion, with its restated response (FR-003).
- **Restated figure**: a spec 001/002/003 number replaced by this spec, enumerated in FR-010 to FR-012 and the CHANGELOG.
- **Assumption status change**: superseded (A-001, A-002, A-014), corrected (A-027 receive row), upgraded (A-016), restated (A-019 band).
- **Measurement result** and **golden reference**: as specs 001–003, with the new frequency sets and the 6 kHz host rate.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An app developer can find the device rate, the two stages' band edges and what changed from version 0.3 in under 5 minutes using only the project's documentation (CHANGELOG and README).
- **SC-002**: 100 % of the tolerance checks in FR-003 to FR-009 pass at all seven supported host rates, in every stage configuration.
- **SC-003**: 100 % of the spec 002 and spec 003 checks pass at all seven host rates with only the figures enumerated in FR-011 and FR-012 changed; the AGC's levels and times and the VAS's threshold and times are unchanged.
- **SC-004**: The three golden files are each re-blessed exactly once in the feature, in one commit; afterwards they pass unchanged on all eight platform targets from 001 FR-014 and across at least 100 random block partitions of each golden stimulus.
- **SC-005**: Default-configuration latency is 20 ms or less at all seven host rates, and reported latency matches measured latency within ±1 sample in 100 % of configurations and rates.
- **SC-006**: Zero memory allocations are observed while processing 1000+ blocks of varying size at every host rate, including 6 kHz, and processing runs at least 20× faster than real time (engineering target) at 48 kHz with the default settings.
- **SC-007**: A deliberate 100 Hz shift of the record stage's lower band edge, or of either stage's upper band edge, is caught by the harness: both the response check and the golden check fail.
- **SC-008**: A review finds zero untraceable numbers in this feature, and the CHANGELOG entry lists every restated figure of FR-010 to FR-012: a reader can reconstruct the old and new value of each from the entry alone.

## Assumptions

- **A-026** (existing, verified by S-002): the device rate is 6 kHz. Adopted by this feature; A-001 superseded.
- **A-027** (existing, medium confidence), adopted and **corrected**: the codec's switched-capacitor filters are clocked by a PLL locked to the 6 kHz sync, so every frequency of the datasheet templates scales by 0.75, and group-delay figures by 1/0.75. Correction: the receive (playback) template is a low-pass only; A-027's current receive row implies a band-pass. The datasheet guarantees its characteristics only at 8 kHz and allows 6 kHz operation "not guaranteed" (S-005), which is why the scaled template is an assumption, not a measurement; a swept-sine capture through a real unit, looking for the −0.8 and −3 dB points near 2.5–2.8 kHz, is the single most informative piece of evidence (see also sources.md's open lead for the MSM7702's own pages, S-003).
- **A-016** (existing), kept and upgraded: minimum-phase stages. S-005's group-delay distortion is non-zero and rises toward the band edges, with an absolute delay under 0.6 ms; a linear-phase filter would show zero distortion everywhere and need more delay. Confidence High; status partially verified (S-005).
- **A-015** (existing): unity passband gain at the reference frequency. The level anchor (A-030) stays out of scope.
- The two stages are modeled on the datasheet's **typical** values with its **limits** as tolerances (user decision, Clarifications). Where the datasheet gives a limit but no typical, the limit alone applies.
- The response between 2550 Hz and the 3 kHz Nyquist frequency is not constrained by any source. The datasheet's next point above 3400 Hz (scaled 2550 Hz) is the 4.6 kHz (scaled 3450 Hz) discrimination row, which at 6 kHz lies above Nyquist and becomes the anti-alias requirement (FR-007). Spec 001's "≥ 14 dB at 4000 Hz" came from G.712 through A-014; its scaled equivalent, 3000 Hz, is the device's Nyquist frequency and cannot be observed through the boundary, so it is dropped; its purpose, alias protection, is FR-007. The stages' loss at 2700 and 2900 Hz is reported so the choice the plan makes is visible.
- The real device partially aliases host content between 3000 and 3450 Hz into the top of its band (the analog anti-alias filter is only required to reach 30 dB by 3450 Hz), and its DAC leaves imaging residue above 3450 Hz at about −35 dBm0. The emulator's boundary removes both by at least 60 dB, so the emulator is slightly "cleaner" at the top of the band than the device. This is a known, documented deviation (FR-007, FR-018), reversible later as a stage-4/10 realism option; the speech codec will dominate the top of the band in any case.
- A 6 kHz host rate is an emulator convenience and the natural identity rate; the device has no host rate. 8 and 16 kHz stay supported for compatibility and become ordinary conversions.
- The VAS detector's sensitivity to the phase of a tone with a whole number of device samples per period is a property of the emulator's sample-peak detector (003 R-02), not of the device, whose VAS is DSP firmware (A-033) of unknown method. The stimulus phase rule (FR-012) keeps the spec 003 checks meaningful; the detector itself may be revisited by the sensitivity-placement feature.
- The AGC detector's design is normalized to the device rate, so its behavior at 6 kHz is the same law over a band that scales with the rate; whether its coefficients stay byte-identical is a plan-time verification, not a requirement.
- The default pipeline's output, the output of every configuration, and the reported latency all change. No release has been published, so this is acceptable; the CHANGELOG records it (Principle II).
- Specs 001 to 003 are not edited. This spec restates their figures by reference (FR-010 to FR-012), the CHANGELOG lists each one, and their status lines note the restatement. The three specs remain the record of the laws; this spec is the record of the ground they stand on.
