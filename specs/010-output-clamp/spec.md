# Feature Specification: Optional Output Clamp (a host option at the route output)

**Feature Branch**: `010-output-clamp`

**Created**: 2026-10-15

**Status**: Draft

**Input**: User description (abridged; the full text is in the specify session's transcript): "An optional output clamp at the end of the playback chain (feature 010, queued by spec 009 on 2026-10-15). Since spec 009 the engine's default output is the selected route's output. The speaker route's stage 11 is bounded at full scale by the amplifier's clipper, and the speaker template's gain peaks at 1.0, but the template's resonance rings on transients and on clipped material: a full-scale square wave, or a full-scale tone at volume position 1, reads about 1.9 (+5.7 dB over full scale) after the template at 48 kHz (A-042). The earphone route passes the playback filter's reconstruction overshoot scaled by the position. Spec 009 kept this documented, not clamped, and the README tells hosts to limit or clip before converting to integer formats. A host-selectable clamp that bounds the route output to ±1.0 on both routes, after the route's last stage, OFF by default: the default output of 0.9 unchanged bit for bit, every golden file unmoved, no manifest row (an emulator convenience, not a device behavior). Applies at the route-output tap only; the per-stage taps stay unclamped. No state, no latency, no allocation; deterministic. A settings field in Rust and C; version 0.10.0."

## Overview

Since [spec 009](../009-output-chain/spec.md) the emulator's default output is the selected route's output: the signal after the speaker template on the speaker route, or after the earphone output on the earphone route. Each route's output is scaled so that a sample of 1.0 is that route's electrical full swing ([A-042](../../docs/hardware/assumptions.md)). The output is not bounded at that figure. On the speaker route the amplifier's clipper bounds stage 11 at full scale and the speaker template's frequency-response gain peaks at 1.0, so a steady tone below the knee never exceeds full scale, but the template's resonance rings on transients and on clipped material: a full-scale square wave, or a full-scale tone at volume position 1, reads about 1.9 after the template at 48 kHz (+5.7 dB over full scale). On the earphone route the output follows the playback filter's reconstruction overshoot of a clipped onset, scaled by the volume position, and can exceed full scale near the top of the travel. Spec 009 decided to keep this **documented, not clamped** (its US1 coach review): the overshoot is the model's own ringing, as a real driver's cone overshoots in its own way, and the README tells hosts to leave headroom or clamp before converting to 16-bit or other integer formats.

That leaves every integer-buffer host writing the same clamp, and the engine's own C smoke loop and parity checks cannot show a bounded output. This feature adds the clamp **as a host option**: when a host turns it on, no route-output sample leaves the engine above +1.0 or below −1.0; a sample inside that range passes unchanged, so everything spec 009 measured below the knee is bit-identical with the option on or off.

The option is **off by default** and is **not a stage of the device**. Nothing in the RR-DR60 bounds its acoustic output at a number: the speaker's cone and the earphone's diaphragm move past any such figure in their own ways, and the model's overshoot stands in for that. So the default output of version 0.9 is unchanged bit for bit, the five existing golden files and their guards stay unmoved, the stage manifest's fifteen device stages gain no row, and the option is described everywhere as an **emulator convenience**: the words "clip", "clamp" and "bound" apply to the engine's output numbers, never to the device, and the register entry A-042 names the option as the host's alternative to leaving headroom.

Two things the option is not: it is not a limiter (no gain control, no time constants, no knee; a sample is either inside the bound or replaced by it), and it is not a change to any tap before the route output. The points after stage 11, after stage 12 and after stage 13 keep reporting the unbounded signal, so the overshoot stays measurable and a host that wants the raw figure selects that tap. Three choices the description left open are recorded under Clarifications with the proposed default and marked for the clarify step.

## Clarifications

### Session 2026-10-15 (specify: the choices the description left open, proposed defaults)

- Q: Is the option a settings field (fixed at configuration) or an engine control turned at any time, like the volume and the route? → A: [NEEDS CLARIFICATION: a settings field in the output settings group is proposed, because the choice is a static property of the host's output format, not something turned during playback; a control would let a host toggle it mid-playback without a reconfigure, at the cost of a second kind of surface for a boolean]
- Q: A hard bound at exactly ±1.0, or a host-set ceiling, or a soft knee? → A: [NEEDS CLARIFICATION: a hard bound at exactly ±1.0 is proposed, because full scale is the integer-buffer boundary the hosts need, a host that wants headroom scales its own output, and anything softer is a second limiter with no device basis; a host-set ceiling (a number) would be a float field with its own refusals]
- Q: Does the bound also apply to the per-stage taps after stages 11, 12 and 13, and to the after-playback tap when the chain is bypassed? → A: [NEEDS CLARIFICATION: no is proposed on both counts: only the route-output tap is bounded, so the unbounded figures stay measurable at the per-stage taps, and a host that bypasses the three stages and wants the bound selects the route output, where the option still applies (the chain's bypass does not switch the option off)]
- Q: Is the default on or off? → A: **Off.** The device has no such bound; on by default would change 0.9's default output and every host's audio without a host decision, and the register could not describe it as a device behavior. Alternative rejected: on by default "because integer hosts need it", because the audiophile audience and the measurement harness want the unbounded figure and because the five golden files would all move.
- Q: A manifest row or a count? → A: **No row, no count change.** The manifest describes the device's fifteen stages; the option is engine behavior, documented in the manifest's "not modeled here" notes of stages 12 and 13 only if a string change is worth a re-bless, otherwise in the README, the register (A-042) and the settings documentation.
- Q: Which golden file? → A: **A new golden file for the option on**, at the seven host rates with the stimuli that overshoot today, blessed once; the five existing files and their SHA guards are unmoved by rule.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Bounded output for integer-buffer hosts (Priority: P1)

An app developer writes the engine's output into 16-bit buffers for playback or for a file. With the option on, the developer converts every output sample without a clamp of their own and without leaving headroom, on either route and at any volume position, and the clipped and transient material that read about 1.9 on the speaker route in 0.9 now reads at most 1.0. With the option off, the developer gets exactly 0.9's output and the README's headroom advice. An audiophile keeps the option off, or reads the per-stage tap, to hear and measure the model's own overshoot.

**Why this priority**: this is the whole feature: the one output guarantee the integer-buffer hosts asked for since spec 009, delivered as an option the device-faithful default does not pay for.

**Independent Test**: can be fully tested with a fresh engine in the **output-chain-only configuration** of spec 009 (every earlier stage bypassed) at each supported host rate, feeding the three stimuli that overshoot today (a full-scale square wave at position 1 on the speaker route, a full-scale 1 kHz tone at position 1 on the speaker route, a full-scale square wave at position 1 on the earphone route) and one that does not (a full-scale 1 kHz tone at the reference position on the speaker route), reading the sample peak at the route output with the option on and off and at the per-stage taps. The memory, the export and the C API are not needed. P1 alone is a usable engine with the bounded output.

**Acceptance Scenarios**:

1. **Given** the output-chain-only configuration at 48 kHz, the speaker route, position 1 and the option **on**, **When** a full-scale 1 kHz square wave is played, **Then** no route-output sample exceeds +1.0 or falls below −1.0, and at least one sample equals a bound; **When** the same is played with the option **off**, **Then** the route output's sample peak reads about 1.9 (within ±0.1 of the figure spec 009 reports), unchanged from 0.9.
2. **Given** the same configuration at each of the seven host rates and both routes, **When** the three overshooting stimuli are played with the option on, **Then** no route-output sample exceeds the bound at any rate or route; **When** the option is off, **Then** each stimulus exceeds the bound at the route output at every rate (the overshoot is still there to bound).
3. **Given** the option on, the speaker route at the reference position, **When** a full-scale 1 kHz tone is played (below the knee; it never exceeds full scale after the template), **Then** the route output is **bit-identical** to the option off, sample for sample, at the seven host rates.
4. **Given** the option on and a stimulus that overshoots, **When** the output is read at the tap after stage 11, after stage 12 (speaker route) or after stage 13 (earphone route), **Then** each reads the unbounded signal, bit-identical to the option off; only the route-output tap is bounded.
5. **Given** the option on and every output-chain stage bypassed, **When** a full-scale square wave is played and the route output is read, **Then** it is bounded; **When** the after-playback tap is read instead, **Then** it is 0.8's output, unbounded.
6. **Given** a bounded sample, **When** the host inspects it, **Then** it is exactly +1.0 or exactly −1.0 (the bound replaces the sample; no scaling, no knee, no time behavior: a sample inside the bound is never altered, and the output has no new latency).

---

### User Story 2 - The option through the whole engine (Priority: P2)

The developer who already uses the engine finds that the option behaves the same in the one-pass processing and in record-then-play, that it survives a route change and a volume change, that the default output of 0.9 is unchanged bit for bit when the option is off, that every earlier check still passes, and that the option costs nothing measurable.

**Why this priority**: the guarantees of specs 006 – 009 (the one-pass equals record-then-play; the taps; the controls at the next block; the golden files) must hold with the option on, or the option is not usable in the structured engine. It is separable from P1: P1 proves the bound at the route output in one configuration, P2 proves it does not disturb the rest.

**Independent Test**: can be tested by running the existing equivalence, determinism, allocation-free and golden checks with the option on, and by a timing comparison with the option on versus off, without any new signal measurement.

**Acceptance Scenarios**:

1. **Given** the option on, both routes, two volume positions and the seven host rates, **When** record-then-play is compared with the one-pass processing under spec 008's equivalence rule, **Then** they are bit-identical.
2. **Given** the option on, **When** the route or the volume changes mid-playback, **Then** the change takes effect at the next block as in spec 009 and the route output stays bounded through the change.
3. **Given** the option **off** (the default), **When** every existing golden file is checked, **Then** all five are byte-identical and their SHA guards are unchanged; **When** the option is on, **Then** the feature's own golden file reproduces on every golden-matrix platform and the frame bytes of the stored form are unchanged (the option is after the memory; it changes nothing stored).
4. **Given** the option on, **When** the block partition is varied and the same input is processed, **Then** the output is bit-identical (determinism); **When** the per-block processing is run, **Then** it allocates nothing.
5. **Given** the release timing job, **When** the one-pass cost per host sample at 48 kHz is measured with the option on versus off on the speaker route, **Then** the option adds at most 3 % and the reported latencies are unchanged at all seven rates.

---

### User Story 3 - The option is explained and is not a device claim (Priority: P3)

A reader of the README, the register and the settings documentation learns what the option does, that it is an emulator convenience and not a device behavior, that the default is off, which tap it applies to, and that the unbounded figure stays available; the register's A-042 names the option as the host's alternative to leaving headroom; the manifest's device stages are unchanged in number and status; the wording passes the claims policy.

**Why this priority**: the project's honesty rule: an engine option that bounds the output must never read as "the device clips here" or as a fidelity improvement. Separable: the strings and the records can be checked without the audio.

**Independent Test**: the manifest cross-check, the wording tests and the traceability audit run against the updated register, README and CHANGELOG; a reader test on the README.

**Acceptance Scenarios**:

1. **Given** the README, **When** the output-scale paragraph is read, **Then** it states the option, its default (off), what it bounds (the route output at ±1.0), what it does not (the per-stage taps), and that it is an emulator convenience, not a device behavior, in under five minutes of reading.
2. **Given** the register, **When** A-042 is read, **Then** its headroom text names the option and keeps its hardware statement ("a real driver's cone overshoots too") unchanged; **When** the CHANGELOG is read, **Then** the entry records the option, its default and the version.
3. **Given** the manifest, **When** its header and rows are read, **Then** the counts by status are unchanged (10 not modeled, 5 assumed) and no row describes the option as a stage; a string change in a "not modeled here" note, if any, is a recorded re-bless.
4. **Given** every string this feature adds, **When** the wording tests run, **Then** they pass spec 007's denylist and the claims policy: "an emulator convenience, not a device behavior", never "accurate", "protects", "safe" or "the device clips".

---

### User Story 4 - The same from C (Priority: P4)

The iOS and C hosts set the option through the C settings structure and get the same bounded output as the Rust path, sample for sample.

**Why this priority**: the integer-buffer hosts the feature serves are mostly C and Swift callers; without the C field the feature does not reach them. Separable: it is a field and a parity check on top of P1.

**Independent Test**: the C smoke loop with one bounded case per route, the C-versus-Rust parity check with the option on at the seven rates, the structure size and offset assertions.

**Acceptance Scenarios**:

1. **Given** a C caller setting the option on, **When** the overshooting square wave is processed on each route, **Then** no output sample exceeds the bound and the output matches the Rust path's pinned hash.
2. **Given** a C caller of the previous version's structure size, **When** it calls the new library, **Then** it is refused as spec 009's rule requires (the structure grew or the new field occupies a reserved slot with the size unchanged; either way the caller's behavior is defined and documented).
3. **Given** the C header, **When** it is regenerated, **Then** the field is present with its documentation and the drift check is clean.

---

### Edge Cases

- **Samples exactly at the bound.** +1.0 and −1.0 pass unchanged; the bound is inclusive.
- **Not-a-number and infinities.** Cannot reach the route output (spec 001 sanitizes the input and the chain is finite); if one did, an infinity becomes the bound of its sign and a not-a-number is replaced by 0.0 (stated, so the behavior is defined; no test depends on producing one).
- **Volume position 0.** The output is exactly zero from the next block as in spec 009; the option changes nothing (0.0 is inside the bound; bit-identical with the option on or off).
- **Output chain bypassed.** The option still bounds the route-output tap (the bypass is a measurement configuration; the bound is a host decision); the after-playback tap is never bounded.
- **The per-stage taps.** Never bounded (after stage 11, 12, 13 and every earlier tap), with the option on or off.
- **Route change mid-playback.** The option is route-independent; the bound applies to whichever route's output is selected at each block.
- **Reconfigure.** The option is read at configuration like the other settings; a change applies from the next processing call after the reconfigure, with the paths reset as spec 006 states (a settings field) **or** at the next block without a reset (a control), per the first clarification.
- **Low host rates.** At 6, 8 and 16 kHz the overshoot is smaller (the template's ringing and the reconstruction overshoot are band-limited by the host rate) but still above the bound on the three stimuli; the figure per rate is reported, not bounded by the spec.
- **Recording.** Nothing plays; the option has no effect on the record path, the memory or the stored form.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: **The option.** The host MUST be able to turn on an **output bound** that replaces every route-output sample above +1.0 by +1.0 and every sample below −1.0 by −1.0, on both routes; a sample inside the bound MUST pass unchanged. The bound is inclusive.
- **FR-002**: **Default off.** The option MUST be off by default; with it off the route output MUST be bit-identical to version 0.9's at every tap, route, volume position and host rate (the five existing golden files and their guards unmoved).
- **FR-003**: **Where it applies.** The bound MUST apply at the route-output tap only, after the selected route's last stage (after the speaker template on the speaker route, after the earphone output on the earphone route) and after any output-chain bypass; it MUST NOT apply to the taps after stages 11, 12 and 13, to the after-playback tap or to any earlier tap, so the unbounded signal stays readable.
- **FR-004**: **Not a stage.** The option MUST NOT add a row to the stage manifest or change its counts; every string describing it MUST call it an emulator convenience or a host option, never a device behavior, and MUST pass the claims policy and spec 007's denylist.
- **FR-005**: **No time behavior.** The bound MUST have no state, add no latency and no reported latency change, use no gain control and no knee: a bounded sample is exactly the bound, and a sample inside the bound is never altered by a neighboring sample's bounding.
- **FR-006**: **Whole-engine behavior.** With the option on, record-then-play MUST equal the one-pass processing bit for bit under spec 008's equivalence rule on both routes and at two volume positions at the seven host rates; route and volume changes MUST take effect at the next block as in spec 009; the stored form and the frame bytes MUST be unchanged.
- **FR-007**: **Determinism and real-time safety.** The bound MUST be bit-identical on every supported platform and block partition, allocation-free per block, and MUST add at most 3 % to the speaker route's one-pass cost per host sample at 48 kHz.
- **FR-008**: **The surface.** The option MUST be exposed in the Rust public API and in the C API; the C settings structure's change MUST be compile-visible and the previous size refused (or the slot reserved and documented) per spec 009's rule; the header regenerated and the drift check clean. [NEEDS CLARIFICATION: settings field or control; see Clarifications]
- **FR-009**: **Records.** A-042 MUST be amended to name the option as the host's alternative to leaving headroom with its hardware statement unchanged; the README's output-scale paragraph MUST state the option, its default, what it bounds and what it does not; the CHANGELOG MUST record the option, its default and version 0.10.0; the manifest canonical text and README table MUST be re-blessed only for a string change recorded in the CHANGELOG.
- **FR-010**: **Its own golden file.** The option on MUST have its own golden file at the seven host rates with the stimuli that overshoot and one that does not, blessed once, reproducing on every golden-matrix platform.
- **FR-011**: **Verification of the overshoot.** The feature's checks MUST show, with the option off, that each of the three stimuli exceeds the bound at the route output at every host rate (so the option is bounding something) and report the per-rate peak figures; with the option on, that no sample exceeds it and that at least one equals it.

### Out of Scope

A host-set ceiling other than full scale, a soft knee or a look-ahead limiter (a second limiter without a device basis); any change to the overshoot itself (the speaker template, A-010, and the earphone corners, A-032, are untouched); the fixed-length volume ramp and the earphone follower's class-A cut-off (queued separately by spec 009); any bound on the record path beyond the codec's clamp of spec 005; dithering or integer conversion (the host's).

### Key Entities

- **Output bound (the option)**: a boolean host option, off by default; when on, the route-output sample is held within [−1.0, +1.0] by replacement.
- **Route output**: the selected route's output after its last stage (spec 009), the only point the bound applies to.
- **Per-stage taps**: the points after stages 11, 12 and 13 and every earlier tap; never bounded.
- **Overshoot figures**: the per-rate sample peaks of the three overshooting stimuli with the option off (about 1.9 on the speaker route at 48 kHz), reported by the checks and documented, not bounded by this spec.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: With the option on, 100 % of route-output samples of the three overshooting stimuli at the seven host rates on both routes lie within [−1.0, +1.0], and each run contains at least one sample equal to a bound; with the option off, each stimulus's route-output peak exceeds 1.0 at every rate and the 48 kHz speaker-route figures agree with spec 009's within ±0.1 (about 1.9).
- **SC-002**: With the option on, a full-scale 1 kHz tone at the reference position on the speaker route is bit-identical to the option off at the seven host rates, and the per-stage taps are bit-identical to the option off on every stimulus.
- **SC-003**: The five existing golden files are byte-identical and their SHA guards unchanged; the feature's golden file is blessed once and reproduces on every golden-matrix platform; the stored form's frame bytes are unchanged.
- **SC-004**: Record-then-play equals the one-pass bit for bit with the option on, on both routes, at two volume positions, at the seven host rates; 100 % of the determinism and allocation-free cases pass with the option on; the reported latencies are unchanged at the seven rates.
- **SC-005**: The option adds at most 3 % to the speaker route's one-pass cost per host sample at 48 kHz in the release timing job (reported for both routes).
- **SC-006**: 100 % of the C smoke loop's bounded cases match the Rust path's pinned hashes; the C/Rust parity holds with the option on at the seven rates; the structure's size and offsets are asserted; a previous-version caller is refused or served as documented.
- **SC-007**: The manifest cross-check, the wording tests and the traceability audit pass; the manifest's counts are unchanged (10 not modeled, 5 assumed, 0 sourced, 0 checked); A-042, the README and the CHANGELOG carry the stated text; any re-bless is recorded.
- **SC-008**: A developer can find, from the README and the CHANGELOG alone, what the option does, its default, which tap it bounds, that it is an emulator convenience and not a device behavior, and how to read the unbounded signal, in under 5 minutes.

## Assumptions

- **A-042** (amended, not revised in substance): the headroom text names the output bound as the host's alternative to leaving headroom; the output scale, the overshoot figures and the hardware statement ("a real driver's cone overshoots too, in its own way") are unchanged. No new assumption ID: the option is engine behavior with no hardware claim, so there is nothing to register beyond the amendment; if the plan's reviewers want an ID for traceability of the "not a device behavior" statement, the plan registers one at the engine-convention confidence of A-042.
- The option is a boolean; the bound is exactly ±1.0 on each route's own scale (A-042); it is applied by replacement (no scaling), inclusive, stateless, and after every bypass.
- The option's default is off; version 0.9's default output is therefore unchanged and the five golden files and the stored form hold by rule.
- The three overshooting stimuli (a full-scale square wave at position 1 on each route, a full-scale 1 kHz tone at position 1 on the speaker route) exceed the bound at every host rate; the per-rate figures are reported, not specified (the low rates band-limit the ringing).
- The checks run in spec 009's output-chain-only configuration; the whole-engine checks reuse the existing equivalence, determinism and allocation-free harnesses with the option on.
- The version is 0.10.0 (a C settings-structure change, compile-visible); no release has been published, so the frozen-decoder rule of spec 008 is untouched.
- The manifest's counts and rows do not change; a "not modeled here" string change for stages 12 and 13 is optional and, if made, a recorded re-bless.
