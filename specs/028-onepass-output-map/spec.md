# Feature Specification: The one-pass output map

**Feature Branch**: `028-onepass-output-map`

**Created**: 2026-11-04

**Status**: Draft. Reviews (coach, hardware, product), then `/speckit-clarify`, then the owner's spec sign-off.

**Input**: Engine issue #82, filed by the desktop workbench (rr-dr60-workbench, feature 008, adopting 0.93.0); the owner chose option 2 on 2026-11-04 (a published map, with the existing map's documentation corrected). `timeline::onepass_output_to_input` maps a one-pass output position to the input position it aligns with, through a latency the caller passes, the voice-activation events and the gate's mode. Its documentation says to pass `latency_samples()` for an output position. With the speech coder running and the gate in drop mode, that counts every splice early by the coder's delay C (180 device samples, 30 ms; 1440 host samples at 48 kHz): the events' positions are on the clock before the coder, the samples inside the coder at a pause are held through it and come out at the resume, so a splice reaches the output at `event.output_position + C`. The outputs in `[event.output_position, event.output_position + C)` are the hang's last samples, and the documented use maps them into the content after the removed pause. The workbench's tested correction maps an output `o` as `onepass_output_to_input(o − C, latency − C, events, drop)`, with the clamped start for `o < C`.

No audio changes. Everything is appended (version 0.95.0); a host built on 0.94 runs unchanged.

## Decisions for sign-off

| # | Decision | Recommended |
|---|---|---|
| D1 | **Where the map lives.** A method on the engine, `Pipeline::one_pass_output_to_input(position, events)`, that reads the engine's own latency, the coder's delay (whether the coder runs at the current settings and tap, spec 026's `one_pass_codec_runs`) and the gate's mode, so a caller supplies only the output position and the events it collected. Real-time safe (a walk over the events, no allocation). Alternative: a pure function in `timeline` taking both latencies and the mode (the offline caller has no engine); rejected as the only form because it keeps the three caller-supplied inputs the issue tripped on. A pure companion with the two latencies may be added by the plan if the C or Swift surface needs it. | **An engine method; the caller passes only position and events** |
| D2 | **The rule.** An output position `o` maps to the input `o + R(o) − L`, where L is the one-pass latency and R(o) the host samples removed by every event whose splice has reached the output at or before `o`, the splice of an event at `output_position + C` (C the coder's delay when it runs, else 0). For `o + R(o) < L` the result is the clamped start (input 0, flagged). This equals the workbench's rule and, with C = 0, the existing map with `latency_samples()` bit for bit. | **The workbench's rule, C from the engine** |
| D3 | **The result.** The same shape as the existing map: the input position and whether it was clamped at the start. In addition, whether the output lies in a **held span** (the C outputs before a splice reaches the output, where the coder's held samples play out the hang). These outputs map to the input before the pause (the silence the gate was closing on); the flag lets a host say "this moment is the end of a pause" rather than point at content. | **Input, clamped, held** |
| D4 | **The existing map is unchanged** in behaviour and signature. Its documentation is corrected: it is a map on the clock the caller's latency defines; with the coder running in drop mode, pass the events with `one_pass_report_latency_samples()` only for the events' own positions, and use the new method for output positions. The guide's two paragraphs (the timeline maps, the latency the pause's reports align with) say the same. Not deprecated (the engine is pre-1.0 and the map is correct on its own clock). | **Unchanged; documentation corrected; not deprecated** |
| D5 | **The C and Swift surfaces.** A C function on the handle, `rr_dr60_one_pass_output_to_input(handle, position, events, count, out)`, with a sized result struct (spec 022's rules), and a Swift wrapper on the engine type. Appended; the four ABI promise steps pass. | **C on the handle, sized result, Swift wrapper** |
| D6 | **The oracle.** Measured, not derived: voiced bursts separated by pauses longer than the hang, rendered by the one-pass at seven rates in four cells (coder on/off × drop/mute), with a marker on each burst's onset; every output sample's mapped input must point inside the burst that produced it (the decoded envelope, within spec 020's codec-on tolerance of 1 ms), and every held-span output must map into the silence before its pause. The workbench's rule is a second, independent oracle (bit-exact on every output position of every cell). The existing map with the full latency is shown to fail the held-span check (the test's teeth). | **Bursts at seven rates, four cells, the workbench rule bit-exact** |
| D7 | **No audio change**: the seventeen golden files, every smoke hash and the control manifest unmoved; the stage manifest's version line only. Version 0.95.0. | **No audio change; 0.95.0** |

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A host finds the input behind an output moment (Priority: P1)

A host plays or analyses a one-pass render with the default engine (the coder on, the gate dropping pauses) and asks, for any output sample, which input sample it came from, for example to show the original microphone audio under a moment the user marked. The answer is right on every output, including the last 30 ms before each splice.

**Independent Test**: the method against measured burst onsets and against the workbench's rule, at seven rates.

1. **Given** the default engine at each of seven rates and a voiced-burst stimulus with pauses, **When** every output position of the render is mapped, **Then** each output inside a decoded burst maps inside the input burst that produced it, within 1 ms of the burst's own alignment.
2. **Given** the same render, **Then** every output in a held span maps into the input silence before its pause and is flagged held.
3. **Given** the same render, **Then** the method equals the workbench's rule on every output position, and the existing map with the full latency differs on exactly the held spans.
4. **Given** an output before the latency, **Then** the result is input 0, flagged clamped.

### User Story 2 - The map agrees with the existing one wherever the issue does not apply (Priority: P1)

**Independent Test**: bit-exact comparisons in the cells without the coder's hold.

1. **Given** the coder bypassed (or a tap before it) in drop mode, **Then** the method equals the existing map with `latency_samples()` on every output, and nothing is held.
2. **Given** mute mode with the coder on or off, **Then** nothing is removed, the method equals the existing map with `latency_samples()`, and nothing is held.
3. **Given** a render with no pause, **Then** the same holds in every cell.

### User Story 3 - Every surface gives the same answer (Priority: P2)

**Independent Test**: parity and documentation checks.

1. **Given** the four cells at seven rates, **Then** the C function and the Swift wrapper return the Rust results on every output position.
2. **Given** the existing map's rustdoc, the C header's comment and the guide, **Then** each names the coder's hold and points output positions to the new method (a documentation check).
3. **Given** the method, **Then** it allocates nothing.

### Edge Cases

- **An event list from another engine or another settings state** (the latency or the coder changed after the render): the method reads the engine's current values; the documentation says to map with the engine, or settings, that rendered.
- **Two pauses closer than C** (a burst shorter than the coder's delay between them): each splice reaches the output at its own `output_position + C`; held spans may touch or overlap.
- **A position inside a held span that is also before the latency** (a pause at the very start): clamped wins.
- **Events not in order, or past the position**: events are taken in order up to the first whose splice is past the position, as the existing map does.
- **A tap before the coder** (`one_pass_codec_runs` false): C = 0 (User Story 2).

## Requirements *(mandatory)*

- **FR-001**: D1's engine method, real-time safe and allocation-free (`alloc_free`).
- **FR-002**: D2's rule, equal to the workbench's rule on every output in four cells at seven rates and to the existing map wherever C = 0 or nothing is removed (US1 AS3, US2).
- **FR-003**: D3's result with the held flag (US1 AS2, AS4).
- **FR-004**: D4's documentation corrections in the rustdoc, the C header and the guide; the existing map's behaviour unchanged (US3 AS2; the existing timeline tests unchanged).
- **FR-005**: D5's C function and Swift wrapper, appended; the ABI promise steps pass (US3 AS1).
- **FR-006**: D6's measured oracle on bursts, with the existing map's failure on the held spans as the teeth (US1 AS1 – AS3).
- **FR-007**: D7: no audio change.

## Success Criteria *(mandatory)*

- **SC-001**: With the default engine at every supported rate, every mapped output inside speech points into the input that produced it, and no output in the 30 ms before a splice points into content after the pause.
- **SC-002**: Where the coder does not hold samples, the new map and the existing map agree on every output.
- **SC-003**: Rust, C and Swift agree on every output; no golden file or smoke hash moves; every promise step passes.
- **SC-004**: The workbench can replace its own correction with the method, its figures unchanged (checked in the workbench's adoption feature).

## Assumptions

- The engine is at 0.94.0 plus the #83 fix (ce081b8).
- The coder's hold is exactly C host samples at every rate (180 device samples × rate ÷ 6000, a whole number at the seven rates); the issue measured it at 8 and 48 kHz, and D6's oracle measures it at seven.
- The record path's maps (`kept_to_input`, `input_to_kept`) are on the kept clock before the encoder and are not affected; `place_at` is a playback map. Neither changes (the reviews confirm or extend this).
- **Out of scope**: a map from input to one-pass output (the inverse); a map that follows a latency changed by a reconfigure inside the render.
- **Alternatives**:
  - **Option 1** (documentation only): rejected by the owner on 2026-11-04.
  - **D1**: a pure function with both latencies as the only form (kept as a possible companion).
  - **D3**: no held flag (rejected: a host cannot tell a pause's end from content).
  - **D4**: deprecate the existing map (rejected: it is correct on its own clock and the workbench and the record contract use it).
