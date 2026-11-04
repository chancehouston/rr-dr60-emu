# Feature Specification: The one-pass output map

**Feature Branch**: `028-onepass-output-map`

**Created**: 2026-11-04

**Status**: Draft reviewed 2026-11-04 (coach 1 – 17, hardware B1 – B9, product M1 – M8, applied at the defaults below). `/speckit-clarify` next, then the owner's spec sign-off.

**Input**: Engine issue #82, filed by the desktop workbench (rr-dr60-workbench, feature 008, adopting 0.93.0); the owner chose option 2 on 2026-11-04 (a published map, with the existing documentation corrected). `timeline::onepass_output_to_input` maps a one-pass output position to the input position it aligns with, through a latency the caller passes, the voice-activation events and the gate's mode. Its documentation says to pass `latency_samples()` for an output position. With the speech coder running and the gate in drop mode, that counts every splice early by the coder's delay C (180 device samples, 30 ms; 1440 host samples at 48 kHz). The events' positions are on the clock before the coder; the samples inside the coder's delay line when the gate pauses stay there through the pause and come out at the resume, so a splice reaches the output at `event.output_position + C`. The outputs in `[event.output_position, event.output_position + C)` play the hang's last 30 ms, and the documented use maps them into the removed pause's last C host samples, whose last 20 ms (the gate's onset, A-024) are the start of the next sound that the gate consumed. The workbench's tested correction maps an output `o` as `onepass_output_to_input(o − C, latency − C, events, drop)`, with the clamped start for `o < C`.

The reviews found the same offset in the events themselves: with the coder running, a splice (drop mode) or a muted region (mute mode) reaches the output C host samples after the position the event reports, so a host that draws a pause mark at `output_position` is 30 ms early in both modes (coach 1, hardware B3). This feature publishes both reads.

No audio changes. Everything is appended (version 0.95.0); a host built on 0.94 runs unchanged.

**App consumer** (product M8): "Listen here" on a past recording (app 1.1, a one-pass re-render at a tap), the Bench tab and the workbench. App 1.0 is record-then-play (spec 020's `kept_to_input` / `input_to_kept`) and does not need this map.

## Decisions for sign-off

| # | Decision | Recommended |
|---|---|---|
| D1 | **Two engine reads.** `Pipeline::one_pass_input_at_output(position, events) -> OnePassAlignment` (the input behind an output) and `Pipeline::one_pass_splice_output_position(event) -> u64` (where an event reaches the output). Each reads the engine's own latency (`latency_samples()`), the coder's delay C (180 · rate ÷ 6000 host samples when `one_pass_codec_runs()`, else 0) and the gate's mode (drop when `one_pass_vas_runs()` and the mode is drop), so a caller supplies only positions and the events it collected. No pure, handle-less companion (product M7, coach Q5): it would bring back the caller-supplied inputs the issue tripped on. | **Two engine reads; no companion** |
| D2 | **The rule.** An output position `o` maps to the input `o + R(o) − L`, where L is `latency_samples()` and R(o) the host samples removed by every event whose splice has reached the output at or before `o` (`event.output_position + C ≤ o`, in drop mode; 0 in mute mode). For `o + R(o) < L` the result is input 0, clamped. Exact by construction (hardware B4): the delay line steps once per kept device sample after the gate, the playback path receives one device sample per kept sample with or without the coder, so a splice at kept index j reaches output ⌈(j + 180)·m/l⌉ = ⌈j·m/l⌉ + C (180·m/l is a whole number at the seven rates). With C = 0 the rule equals the existing map with `latency_samples()` bit for bit. | **The workbench's rule, C from the engine** |
| D3 | **The results.** `OnePassAlignment { input: u64, clamped: bool }` (`#[non_exhaustive]`), the existing map's shape named; the reads are infallible and callable in any engine state. **No held flag** (hardware B2, coach 2): with the coder off the C outputs before a splice play the same content (the hang's end) and map to the same inputs, so a flag would mark that the coder ran, not anything in the audio; the D2 map is already right on those outputs. A host that marks a pause's end uses `one_pass_splice_output_position`, the same in every cell. The alignment is a point on the 1 kHz group-delay convention, like spec 020's maps; the window a host shows around it is the guide's rule (product M3's cheaper alternative; see Clarifications). | **A point and its clamp; the splice read instead of a flag** |
| D4 | **The existing documentation is corrected; nothing existing changes behaviour.** `timeline::onepass_output_to_input` keeps its signature and behaviour and is not deprecated (it is right on the clock its latency defines; the workbench and the record contract use it). Its rustdoc, the C header's comment on `rr_dr60_onepass_output_to_input` and the guide (§ the timeline maps, § the latency the pause's reports align with) say: to map an event's own position pass `one_pass_report_latency_samples()`; to map an output position use `one_pass_input_at_output` (with the coder off or in mute mode, `latency_samples()` gives the same answer). The `VasEvent` field docs, the C header's `RrDr60VasEvent` comment and the guide state that the one-pass's positions are on the clock before the speech coder, and that with the coder running a splice or a muted region reaches the output C host samples later (`one_pass_splice_output_position`); the recorder's device-sample positions are unaffected. Each new and existing item names its twin. | **Behaviour unchanged; the map's and the events' documentation corrected** |
| D5 | **The events contract** (coach 6). `events` is every event the one-pass reported since the stream's last start (construction, `reset`, `reconfigure`), in report order, as `process_with_events` wrote them (their positions are already cumulative; concatenate the calls' buffers). The work is linear in the events passed, with no allocation: real-time safe for a bounded list; a host mapping many positions maps off the audio thread. Events from before a `reset` or `reconfigure` belong to another stream and must not be passed; the reads use the engine's current settings (the live record-side values do not move the latency). | **The whole stream's list, in order** |
| D6 | **The C and Swift surfaces.** `rr_dr60_one_pass_input_at_output(handle, position, events, count, out)` with a sized `RrDr60OnePassAlignment` (spec 022's `write_sized`), and `rr_dr60_one_pass_splice_output_position(handle, event, out)`; NULL `events` with `count = 0` is valid; a NULL handle or `out`, or NULL `events` with `count > 0`, is `NULL_POINTER`; a poisoned handle keeps its status; a size below the first layout is refused. Named `one_pass_` like spec 026's handle reads and distinct from the existing `onepass_output_to_input` (coach 8, hardware B8). Swift statics on the handle beside `MeterManifest.reached`. Appended: the promise script passes against the five revisions in ci.yml, and a sixth step is added against 0.94's merge (`ad1ba8d401228c91480f12bd8ff3df68ac489e7f`, the full id: a shallow checkout fetches only a full id). | **C on the handle, sized result; Swift statics; a sixth promise step** |
| D7 | **The oracle: a labelled gap** (hardware B1). A burst stimulus whose onsets alone cannot tell the two maps apart (they agree from `pos + C` on, which is why spec 020's codec-off onset check and spec 026's event-position check never saw #82). −20 dBFS 1 kHz or voiced bursts; the gaps filled by sub-threshold tones at about −46 dBFS whose frequency labels the region: f_a = 500 Hz from each burst's end for the hang + 200 ms, f_b = 1500 Hz from there to the next burst; the hang 100 ms, the microphone and output chain bypassed; seven rates, four cells (coder on/off × drop/mute). Checks: (i) each span `[pos, pos + C)` reads f_a at least 12 dB above f_b (a measured check that it plays the hang's end); (ii) every output in it maps to an input labelled f_a, and the existing map with `latency_samples()` maps none there in the coder-on drop cell (the teeth); (iii) every output from `pos + C` to the next such span maps inside the burst's kept part or the following f_a region; (iv) the splice read: in drop mode the burst after a splice starts at or after `one_pass_splice_output_position`, in mute mode the region's f_a energy lasts through `[pos, pos + C)` and the decoded level falls from `pos + C`; (v) an independent exact oracle: record-then-play equals the one-pass skipped by C (spec 008 FR-009), so an output maps through the player's uniform latency and `kept_to_input` with the recorder's events, within one device sample of the method; (vi) the workbench's rule pinned bit-exact as a regression check (algebraically equal to D2, not an oracle). Burst-onset deviations are reported, not asserted (the decoded onset reads up to about 11 ms late on the first burst at 48 – 96 kHz, a coder behaviour). Every figure reproduced in the harness before the records quote it. | **The labelled gap at seven rates, four cells; record-then-play as the exact oracle** |
| D8 | **No audio change**: the seventeen golden files (the sixteen audio guards and the record golden, read from `git diff ce081b8`), every smoke hash and the control manifest's canonical text unmoved; the stage manifest's version line and the guide's copied blocks re-blessed once; the Swift ABI fixture regenerated. Version 0.95.0. | **No audio change; 0.95.0** |

## User Scenarios & Testing *(mandatory)*

Test names carry the token `onepass_map`, so one filter matches every binary (spec 027's lesson).

### User Story 1 - A host finds the input behind an output moment (Priority: P1)

A host analyses a one-pass render with the default engine (the coder on, the gate dropping pauses) and asks, for any output sample, which input sample it came from: for example to play the original microphone audio under a moment the user marked. The answer is right on every output, including the 30 ms before each splice.

**Independent Test**: the labelled-gap oracle and record-then-play at seven rates.

1. **Given** the labelled-gap stimulus at seven rates with the coder on in drop mode, **When** every output in each span `[pos, pos + C)` is mapped, **Then** the span reads f_a at least 12 dB above f_b, every output maps to an f_a-labelled input, and the existing map with `latency_samples()` maps none there (`us1_onepass_map::as1_onepass_map_labelled_gap`).
2. **Given** the same render, **Then** the output `pos + C − 1` maps to the last input before the event's removed run and `pos + C` to the first input after it (`i_e − 1` and `i_e + input_length`, with `i_e = pos + R_prev − L_r`, `L_r` = `one_pass_report_latency_samples()`) (`us1_onepass_map::as2_onepass_map_splice_edges`).
3. **Given** the same render, **Then** every output from `pos + C` to the next span maps inside its burst's kept part or the following f_a region, and the method agrees with record-then-play through `kept_to_input` within one device sample on every output (`us1_onepass_map::as3_onepass_map_record_then_play`).
4. **Given** the same render, **Then** the method equals the workbench's rule on every output, and the existing map with `latency_samples()` differs on every span output that neither map clamps, and nowhere else (`us1_onepass_map::as4_onepass_map_workbench_rule`).
5. **Given** an output before the latency, **Then** the result is input 0, clamped (`us1_onepass_map::as5_onepass_map_clamped`).

### User Story 2 - A host marks where a pause reaches the output (Priority: P1)

A host draws the voice-activation pauses on the output's timeline (a splice in drop mode, a muted region in mute mode) and places each mark where the listener hears it.

**Independent Test**: the splice read against the labelled-gap render in both modes.

1. **Given** the labelled gap with the coder on in drop mode, **Then** each splice's following burst starts at or after `one_pass_splice_output_position(event)` and the span before it reads f_a (`us2_onepass_map::as1_onepass_map_splice_drop`).
2. **Given** mute mode with the coder on, **Then** the f_a energy lasts through `[pos, pos + C)` and the decoded level falls from `pos + C` (the region's zeros reach the output C later) (`us2_onepass_map::as2_onepass_map_splice_mute`).
3. **Given** the coder off (bypassed or a tap before it), **Then** the read is `event.output_position` in both modes (`us2_onepass_map::as3_onepass_map_splice_no_codec`).

### User Story 3 - The new map agrees with the existing one wherever the coder holds nothing (Priority: P1)

**Independent Test**: bit-exact comparisons.

1. **Given** the coder off in drop mode, **Then** the method equals the existing map with `latency_samples()` on every output (`us3_onepass_map::as1_onepass_map_no_codec`).
2. **Given** mute mode with the coder on or off, **Then** nothing is removed and the method equals the existing map with `latency_samples()` (`us3_onepass_map::as2_onepass_map_mute`).
3. **Given** a render with no pause, **Then** the same holds in every cell (`us3_onepass_map::as3_onepass_map_no_pause`).

### User Story 4 - Every surface gives the same answer (Priority: P2)

**Independent Test**: parity, documentation, allocation and mutants.

1. **Given** the four cells at seven rates, **Then** the C functions and the Swift statics return the Rust results on every output and event (`ffi_parity onepass_map`, `swift test OnePassMapTests`).
2. **Given** the rustdoc of both maps and of `VasEvent`, the C header's comments and the guide, **Then** each names the coder's offset and its twin (`us2_cross_check guide_onepass_map`).
3. **Given** the reads, **Then** they allocate nothing (`alloc_free onepass_map`).
4. **Given** five mutants (C omitted, the old rule; C applied twice; the take-while on `output_position` instead of `+ C`; the clamp applied before adding R; C taken whenever the codec setting is on instead of `one_pass_codec_runs()`), **Then** each fails a named test (`mutation onepass_map::`).
5. **Given** the promise script against the six revisions, **Then** every old smoke runs unchanged (`us2_abi spec_028_appended_only`, CI's promise steps).

### Edge Cases

- **Splices are far apart** (hardware B5, coach 9): after a resume the gate keeps at least H + 1 ≥ 301 device samples (the hang's minimum is 50 ms) before it can pause again, and the latency is under 242 device samples at every rate, so the C outputs before a splice never contain an earlier splice and never fall before the latency. Asserted at seven rates with the shortest hang; a synthetic event list closer than C is a unit test of the arithmetic only (`onepass_map_edge_cases::synthetic_close_events`).
- **A render that ends paused** (hardware B7): the hang's last 30 ms stays in the coder, no event reports the pause, and every produced output maps as before (`onepass_map_edge_cases::ends_paused`).
- **Events from another stream**: D5; the documentation says so.
- **A tap before the coder or the gate**: C = 0, or nothing removed (US3).
- **Outputs within the interpolator's half-length of a splice** mix both sides of it; the map is a convention on the 1 kHz group-delay latency, not a measured onset (coach 13).

## Requirements *(mandatory)*

- **FR-001**: D1's two reads, real-time safe and allocation-free (`alloc_free onepass_map`).
- **FR-002**: D2's rule: the labelled gap, the splice edges, record-then-play and the workbench pin (`us1_onepass_map`).
- **FR-003**: D3's results and the splice read in both modes (`us1_onepass_map::as5_onepass_map_clamped`, `us2_onepass_map`).
- **FR-004**: D4's documentation corrections; the existing map's behaviour and tests unchanged (`us2_cross_check guide_onepass_map`).
- **FR-005**: D5's events contract, stated in the rustdoc and the guide (`us2_cross_check guide_onepass_map`).
- **FR-006**: D6's C functions and Swift statics, appended; the six promise steps pass (`ffi_parity onepass_map`, `abi_tests::onepass_map`, `swift test OnePassMapTests`, `us2_abi spec_028_appended_only`).
- **FR-007**: D7's oracle, with the existing map's failure as the teeth and the cells where the maps agree (`us1_onepass_map`, `us3_onepass_map`, `onepass_map_matrix`).
- **FR-008**: every mutant of US4 AS4 fails a named test (`mutation onepass_map::`).
- **FR-009**: D8: no audio change (`us2_abi`'s guard list, the golden SHA guards).

## Success Criteria *(mandatory)*

- **SC-001**: With the default engine at every supported rate, no output in the 30 ms before a splice points past the pause, and every output after a splice points into the sound that follows it.
- **SC-002**: A pause mark placed by the new read sits where the listener hears the splice or the muted region begin, in both modes.
- **SC-003**: Where the coder holds nothing, the new map and the existing map agree on every output.
- **SC-004**: Rust, C and Swift agree on every output and event; no golden file or smoke hash moves; every promise step passes.
- **SC-005** (reported; not a gate of this feature): the workbench can replace its own correction with the method, its figures unchanged.

## Assumptions

- The engine is at 0.94.0 plus the #83 fix (ce081b8).
- The coder's hold is exactly C host samples by construction (D2); corroborated by equal event lists and output lengths with the coder on and off at seven rates (the hardware review's run; reproduced in the harness). The map inherits the existing map's rounding (at most one device sample, spec 020) and adds none.
- **The record path is not affected** (hardware B9): the recorder's events and maps are on the kept clock before the encoder, and the stored frames are decoded in sequence, so nothing is held at a splice (the one-pass's delay line is the only hold; spec 008 FR-009). `place_at` is a playback map and is unchanged.
- **Wording** (product M1): the 30 ms before a splice play the hang's end, the input below the gate's threshold that the gate kept (the capsule's hiss, a quiet tone, speech below the threshold), never "silence"; the documentation this feature adds says so.
- **A marked region across a splice** (product M4): map both ends with the method; the events between them are the removed gaps. A guide paragraph, no range function.
- **Out of scope**: the inverse map (input to one-pass output; the session-clock marks use the record path's `input_to_kept`; reopen if live monitoring ships, product M5); a suffix form of the event list with a `removed_before` argument (coach 6); a map that follows a latency changed by a reconfigure inside a render.
- **Alternatives**:
  - **Option 1** (documentation only): rejected by the owner on 2026-11-04.
  - **D1**: a pure function with both latencies (rejected: the caller-supplied inputs); one read only, the map (rejected: the events carry the same offset).
  - **D2**: a fractional splice modelled on the interpolator's kernel (rejected: the convention is the 1 kHz group delay, like spec 020); held outputs mapped to the removed run's start (rejected: they play the hang).
  - **D3**: a held flag (rejected: it marks that the coder ran, not the content); an alignment window like spec 020's `InputAlignment` (product M3; see Clarifications).
  - **D4**: deprecate the existing map (rejected: it is right on its own clock).
  - **D5**: a suffix form (out of scope).
  - **D6**: a handle-less C function taking the latencies; no Swift statics (rejected: the app is Swift, the inputs are the trap); keep the `one_pass_output_to_input` name beside `onepass_output_to_input` (rejected: one underscore apart).
  - **D7**: onsets of bursts only (rejected: no teeth); the workbench's rule as the oracle (rejected: algebraically equal to D2).
