# Feature Specification: Battery State (the static law)

**Feature Branch**: `018-battery-state`

**Created**: 2026-10-30

**Status**: Draft; awaiting the owner's sign-off on D1 – D9

**Input**: User description: "018 battery state, the static law (A-056). As a user of the app and as an app developer, I want to set the unit's battery voltage as a live unit condition and hear the speaker route's clip ceiling follow it, so that the recorder can be played as a unit with fresh or tired cells; off (the nominal 2.8 V amplifier supply) by default and then bit-identical to 0.17. …" (the full text is the `/speckit-specify` argument of 2026-10-30; its physics, scope, oracles and out-of-scope list are carried into the sections below).

## Decisions for sign-off

The owner reads this section first. Each decision gives the recommendation; the alternative is in the Assumptions section. The three draft reviews (coach, hardware, product) will be applied at their defaults before the sign-off request, as for specs 015 – 017.

| # | Decision | Recommended |
|---|---|---|
| D1 | **One value: the cell pair's voltage, a live unit condition on stage 11.** The battery voltage joins the route, the volume and the speed as an engine control (not a setting): `set_battery_volts` / `battery_volts`, schedulable through spec 015's schedule on the playback and one-pass streams in the **output clock**, kept through `reset` and `reconfigure` like the route, and listed in the control manifest as a **unit condition** (free tier, as the hiss level). Range **2.0 … 3.2 V**; default **3.0 V** (V+ = 2.8 V, the service manual's mark: the factor is exactly 1). | Yes |
| D2 | **The static law only.** The speaker amplifier's supply V+ is the cell pair's voltage less Q9's 0.2 V drop; the clip ceiling scales by **b = (V+ − 1.6) ÷ 1.2** from the model's 1.15 V at V+ = 2.8 V (S-006's swing limits, A-056). The law is a straight line in volts: b = 1 at 3.0 V cells, 0.833 at 2.8 V, 0.667 at 2.6 V (NiMH, −3.5 dB), 0.5 at 2.4 V (−6.0 dB), 0.333 at 2.2 V (−9.5 dB), 0.167 at 2.0 V (−15.6 dB). The dynamic sag under load, any drain over time and supply-noise coupling are **not modeled** and stay registered; the record path and the earphone route are on regulated rails and do not move. | Yes |
| D3 | **Only the ceiling moves.** The factor scales the amplifier's knee and hard ceiling, not the gain: below the knee the output is bit-identical at every battery value; above it the clipped waveform comes out lower by 20·log10(b) because the output scale stays **1.0 = the nominal 1.15 V ceiling** (A-042 unchanged). A full-scale tone at the reference volume position, which sits exactly at the ceiling today, is therefore clipped at a tired battery. | Yes |
| D4 | **A change moves over 20 ms** along the volume's smoothstep (A-045's convention, spec 011's form: the landing sample is transition sample 1, the target exactly at sample N), on the factor b; a change during a transition restarts from the factor last applied; the position is taken at once at the stream's first sample, with stage 11 bypassed and on the earphone route (where it has no effect). An instantaneous step on clipped material would click; below the knee a step is inaudible anyway. | Yes |
| D5 | **Nothing existing moves.** At the default the output is bit-identical to 0.17: the twelve golden files and their SHA guards and every smoke hash are unmoved; the feature's own golden covers the battery values. The oracles: the below-knee identity at every value; the full-scale peak after stage 11 equals b within the knee arithmetic; the register's alkaline trajectory rows (−0.3 … −9.1 dB at 2.96 … 2.22 V) reproduce within 0.05 dB; the 10 % THD point moves with b; the transition is the splice of two constant runs after N samples (the linear mutant caught); partition independence through the schedule; the earphone route and the record path unmoved at every value. | Yes |
| D6 | **Cost budget 3 %** at 48 kHz against the 0.17 merge commit (24ffab9) at the default on the speaker route, a build comparison (45 alternating runs, solo) with a **5 % runner ceiling** through the `ab-timing` label; a non-default value's cost reported. The law costs one multiply on the knee's input and one on its output per sample, or none when folded into the per-block scalars. | Yes |
| D7 | **A-056 moves from "not modeled" to "adopted (the static law)"**; its sag paragraph stays registered as not modeled; A-031's ceiling sentence gains the pointer; stage 11's and stage 15's manifest notes cite the control; a CHANGELOG entry as an assumption-status change with the trajectory table. The factor's line and the trajectory rows come from a design-script print (`tools/output-chain/design_output_chain.py` gains a `--battery` section) pinned by tests, never from the engine's own output. | Yes |
| D8 | **A product rule in the guide and the manifest's sentence: the value is the modeled unit's cells, set by the user; an app must never drive it from the phone's battery.** The device has no battery meter the model could follow (S-001 shows a low-battery indicator only); the number describes a condition of the played unit, as the hiss level does. | Yes |
| D9 | **Not a measured figure.** Everything rests on the schematic (Q9 on the battery line, the 2.8 V mark), the NJM2113 datasheet's swing limits and S-017's discharge curves (low to medium confidence); the guide says "modeled on the circuit, not measured", and the register's bench capture (P5) is the evidence that would adopt or move the law. | Yes |

## Clarifications

(None yet; the draft reviews and the owner's sign-off are recorded here.)

## Overview

The RR-DR60's speaker amplifier runs from the two AAA cells directly, through a switching transistor, while everything else runs from a regulated 3.3 V converter (A-013). Fresh cells and tired cells therefore sound alike until playback reaches the amplifier's clip ceiling, and there a tired pair clips earlier and lower: by about 9 dB at the end of an alkaline pair's ten-hour life. Spec 009 modeled the amplifier at a nominal 2.8 V supply; this feature makes the supply a value the user sets, so a recording can be played as it would sound on a unit with fresh or tired cells. It is a **unit condition**, like the hiss level: a property of the played unit, not a device control a hand moves, and not something the model drains on its own.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Play the unit on tired cells (Priority: P1)

As a user of the app, I set the battery voltage and hear loud playback through the speaker clip earlier and lower, while quiet playback is unchanged; as an app developer, I set it live (a plain setter or a scheduled change) and it moves over 20 ms without a click.

**Why this priority**: it is the feature; the product brief lists battery state among the live unit conditions of the free tier.

**Independent Test**: at each of six battery values and seven host rates, a tone below the knee is bit-identical to the default run; a full-scale tone's peak after stage 11 equals the factor b within the knee arithmetic; a change lands on its position and is the splice of two constant runs after N samples.

**Acceptance Scenarios**:

1. **Given** the default (3.0 V cells), **When** any input is processed on either route, **Then** the output is bit-identical to 0.17.
2. **Given** a tone whose level stays below the knee at the default, **When** the battery is set to any value in range, **Then** stage 11's output is bit-identical to the default run's until the tone reaches the new knee.
3. **Given** a full-scale 1 kHz tone at the reference volume position on the speaker route, **When** the battery is set to 2.96, 2.76, 2.66, 2.52, 2.42, 2.32 and 2.22 V, **Then** the peak after stage 11 reads −0.3, −1.9, −2.9, −4.4, −5.7, −7.3 and −9.1 dB re the default's within 0.05 dB (the register's trajectory), and the 10 % THD point of the input moves with the factor.
4. **Given** a scheduled change at output sample P, **When** the stream runs at block sizes 1, 64, 4096 and the irregular partition, **Then** the outputs and logs are identical, sample P is the first transition sample, and sample P + N − 1 carries the new factor exactly (the splice of the two constant runs from there); a change on the earphone route is logged and has no effect.
5. **Given** the same stimulus on the earphone route or through the record path, **When** the battery is set to any value, **Then** the output is bit-identical to the default's.

### User Story 2 - The condition is described and kept (Priority: P2)

As an app developer, I read the control manifest's entry (a unit condition, live, its range and default, one neutral sentence with the product rule), and the value survives `reset` and `reconfigure` like the route.

**Independent Test**: the manifest entry's bounds equal the setter's acceptance; the value survives the lifecycle calls; the stage manifest's stage 11 and stage 15 notes name the control.

**Acceptance Scenarios**:

1. **Given** a value set, **When** `reset` or `reconfigure` runs, **Then** the value is kept and the next output starts at that factor with no transition.
2. **Given** a value outside 2.0 … 3.2 V or not finite, **When** set or scheduled, **Then** it is refused with a distinct error and nothing changes.
3. **Given** the C API, **When** the setter, the getter and the schedule are used, **Then** the output and the entries equal the Rust path's bit for bit.

### Edge Cases

- **Stage 11 bypassed**: the value is accepted, logged and taken at once; no effect.
- **The earphone route**: no effect; a change is logged; a route switch to the speaker while a transition runs continues it on the speaker (the factor is a property of the amplifier, not of the route).
- **The one-pass's front taps and the taps before stage 11**: no effect, logged.
- **The output clamp** (spec 010) still bounds at ±1.0; a lowered ceiling never reaches it on the speaker route.
- **Two changes at one position**: the later wins, one transition.
- **A change scheduled past a playback's end** is dropped without an entry (spec 015's rule).
- **The meters** (spec 016) read the route peak after the lowered ceiling; the gain-reduction figure is the limiter's and does not move.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The engine MUST hold a battery value in volts of the cell pair, range 2.0 … 3.2 inclusive, default 3.0, set and read by a plain setter and getter and schedulable on the playback and one-pass streams through the existing schedule in the output clock; a value out of range or not finite MUST be refused with a distinct error and leave the value unchanged.
- **FR-002**: The speaker amplifier's clip ceiling MUST scale by b = (V − 0.2 − 1.6) ÷ 1.2 for a battery value V (1 at the default), applied so that only the ceiling moves: stage 11's output below the knee MUST be bit-identical to the default's at every value, and the output scale MUST stay 1.0 = the nominal ceiling.
- **FR-003**: A change MUST move the factor over the volume transition's length and shape from the factor last applied, landing on its position (the landing sample is transition sample 1, the target exact at sample N); it MUST be taken at once at a stream's first sample, with stage 11 bypassed and on the earphone route.
- **FR-004**: The record path, the earphone route and every tap before stage 11 MUST be unaffected at every value.
- **FR-005**: At the default every output MUST be bit-identical to 0.17 (the twelve golden files, their SHA guards, every smoke hash); the feature MUST have its own golden file over the battery values, both routes and the seven rates.
- **FR-006**: The figures MUST come from a design-script print pinned by tests: the factor at the six listed voltages and the register's seven trajectory rows within 0.05 dB; the 10 % THD point's move with b.
- **FR-007**: Deterministic and real-time safe as every spec; the per-sample cost at the default within 3 % of 0.17 on the speaker route (5 % on the runner); a non-default value's cost reported.
- **FR-008**: The control manifest MUST gain one entry (kind unit condition, timing live, stage 11, unit V, A-056 and S-002/S-006 cited) whose sentence states the product rule (set by the user; not the phone's battery); the stage manifest's stage 11 and stage 15 notes MUST cite it; the canonical texts re-blessed once.
- **FR-009**: A-056 MUST move to "adopted (the static law)" with the sag kept as not modeled; A-031 MUST point to it; the CHANGELOG MUST record the status change with the trajectory table; the guide MUST gain a section (the law, the rule, "modeled on the circuit, not measured"); the product notes of controls-discussion.md § 2b and § 4, brief.md and decisions.md updated.
- **FR-010**: Rust and C surfaces: the setter, the getter and the schedule acceptance; the C header regenerated; the Swift package's parity test passes.

### Out of Scope

The dynamic sag under load (a later spec if wanted), any drain or battery meter, supply-noise coupling, the earphone route's follower (regulated), the record path, A-057's start-of-item transients, the beeps, an app-side link to the phone's battery (forbidden by the product rule).

### Key Entities

- **Battery value**: the cell pair's voltage, 2.0 … 3.2 V, default 3.0; an engine control (unit condition), kept through reset and reconfigure.
- **Ceiling factor b**: (V − 1.8) ÷ 1.2, the amplifier's clip ceiling relative to the model's nominal 1.15 V; follows a 20 ms transition on a change.
- **Trajectory table**: A-056's seven alkaline rows, the design script's print and the harness's oracle.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: At the default every golden file and smoke hash is byte-identical; below the knee 0 mismatches at six battery values, seven rates, both streams.
- **SC-002**: The seven trajectory rows reproduce within 0.05 dB; the full-scale peak equals b within the knee arithmetic at every value and rate; the 10 % THD point moves with b within 0.1 dB.
- **SC-003**: The transition identity holds at seven rates (the splice at N; the linear mutant caught); outputs and logs identical at the four partitions.
- **SC-004**: The earphone route and the record path are bit-identical to the default at every value.
- **SC-005**: Cost at the default within 3 % (5 % on the runner); the non-default cost reported.
- **SC-006**: Coverage stays at or above 80 % and within 1 point of the baseline (0.17: 97.32 % locally).

## Assumptions

- **D1 alternative**: a setting applied by `reconfigure` (rejected: the product asks for a live unit condition; the law is a scalar).
- **D2 alternative**: modeling the sag (deferred by the owner's 2026-10-29 decision); a drain over time (rejected: the user sets the value).
- **D3 alternative**: scaling the output with the ceiling so the clipped level stays at 1.0 (rejected: the speaker sees fewer volts; A-042's scale is the nominal ceiling).
- **D4 alternative**: an instantaneous step (rejected: clicks on clipped material).
- **D8 alternative**: following the phone's battery (rejected: a different unit's cells; the product rule).
- The law's confidence is low to medium (A-056); the bench capture P5 is the evidence that would move it.
