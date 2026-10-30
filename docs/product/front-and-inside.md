# The front and the inside

A product note on the app's two views of the emulated recorder. Part 1 is Chance's brainstorm of 2026-10-11, lightly edited. Part 2 is the product read and the decisions it led to, each marked "Decided 2026-10-11". The note is public and follows the claims policy in `CLAUDE.md`: it describes what the signal chain does to sound, says "modeled on" until a stage is checked against a real unit, and makes no claim about what recordings contain.

Public wording: **"The front is the recorder. The inside is the model."** (Decided 2026-10-11.)

> **Partly superseded 2026-10-15.** The current brief is [brief.md](brief.md). The flip, the honesty of motion, the empty sockets, the status badges and header, the wording rules and the accessibility rules stand. Superseded: "the free inside has no controls" and "see free, touch paid" (the engine's intended user adjustments are free, O-1), and the raw capture as Workbench-only (O-3). The "now" column and the engine-request status describe engine 0.5; the 0.11.0 facts are in brief.md › Engine capabilities. The app now has two modes (Recorder and Investigation) that both use this front and inside.

## Part 1: the brief (Chance, 2026-10-11)

The `rr_dr60` library is the machine on the bench: the stages, components and behaviours that together recreate the recorder. The app shows that machine from two sides.

**The front** is the simple, approachable view: the controls and the information someone needs to make and play a recording. It should feel like using a piece of audio hardware, not operating an engineering tool.

**The inside** is what you see when you flip the recorder over, take the back off, and can somehow watch it work. Not a settings page and not a diagnostics screen: the signal, moving through the components that make up the emulator.

The inside should feel alive. As audio passes through, parts of the chain wake up. Levels move. The limiter grabs and lets go. Voice activation opens and closes. Encoding ticks over. Storage fills. On playback a different path lights up. The user follows the signal's journey with their eyes.

The visual language is a beautiful, understandable rendering of electronics rather than a literal schematic or a wall of controls: PCB traces, IC-like modules, small meters, LEDs, scopes, cues borrowed from late-1990s audio gear and lab equipment. The feeling to aim for: "I'm not looking at diagnostics. I'm looking inside the recorder while it's running."

Components invite exploration. A curious user taps a stage and sees what is happening at that point in the chain, without the whole experience turning into a configuration panel.

The transition is part of the personality: rather than switching tabs, the device physically flips over and reveals what is underneath.

## Part 2: product read

### Who it serves

- **Serious investigators**, the likeliest critics and the likeliest source of test recordings: "which stages are checked against a real unit" becomes something they look at, not a page they read.
- **Content creators**: the flip is a short on-camera moment and the most shareable thing in the app.
- **Group-hunt and one-time users** flip once and go back to the front. The front must stand alone.
- **Vintage-gear enthusiasts and library developers**: the inside is a living demonstration of the engine's taps and meters, so it shapes the API for everyone without a dedicated mode (see the vintage-gear decision in decisions.md).

The inside supports the positioning in decisions.md ("stage by stage, from the service manual, shows which stages are checked against a real unit") better than copy can: no competitor can render a chain it has not modeled (competitors.md).

### Rules that keep it honest

1. **Honesty of motion.** A component animates only from a number the engine returned for the current block. Stages that are not yet modeled render as labelled empty sockets ("not yet modeled"), with their source and assumption IDs. The sockets are a feature: they are the visible work list and the appeal for captures. Nothing on the inside moves for effect.
2. **No global controls.** *(Partly superseded 2026-10-15: controls are no longer all paid, brief.md O-1; the card, dot and "device default" rules stand.)* The free inside has no controls at all. Every Workbench control lives in the card of the component it belongs to; a dot on the component marks it as off the device default; each card has one "device default" button. (Decided 2026-10-11.)
3. **The front is the default.** The app opens on the front, recording starts from the front, and after a period without touch the inside drops to a low frame rate. Investigation sessions run an hour or more on battery.

### Scope: the first cut

**Decided 2026-10-11:** a reduced inside ships in 1.0: the built stages live, labelled empty sockets for the rest, status badges and the status header, no touch controls. It replaces the separate "About this model" screen; a flat text list of the same information remains for assistive technology and for the store review notes. Rendering is procedural (one style sheet, no commissioned illustration), to be revisited after the first reviews. The inside is the second App Store screenshot at launch, with the status header visible.

### What each stage can show

Stage numbers follow `docs/hardware/signal-chain.md`. "Now" means the engine computes it today (0.5); "later" names the engine feature that would add it. Badge states are defined below.

| # | Stage | Honest now | Later | Badge |
|---|---|---|---|---|
| 1–2 | Mic capsule, mic amp | input level (measured by the app) | | Sourced parts (S-002); gain assumed (A-029) |
| 3 | Limiter | gain-reduction meter; hold-capacitor charge bar (the stage's real state, which also shows how "armed" it is); knee mark | ceiling-hold markers (E-E) | Modeled on the circuit (A-028, A-034–A-036), low confidence |
| 4 | Codec input filter and clamp | pass-band window 225–2550 Hz (app-side spectrum of a tap); clip indicator from clamp hits | µ-law quantization and idle noise (A-030) | Datasheet template scaled to the clock (A-027), medium confidence |
| 5 | VAS | recording / paused indicator; hang-time countdown; count of pauses removed and seconds removed | | Manual plus assumed values (A-008, A-022–A-025) |
| 6 | Noise reduction | empty socket: "claimed only" | | S-004, A-009 |
| 7, 9 | CELP encoder, decoder | empty socket: "4 kbit/s CELP on the MN1931712BB" | frame tick at the codec's frame rate; a constant 4 kbit/s read-out | A-004 |
| 8 | Flash | gauge: minutes of 60, recordings of 99 (app-side until E-C) | from the engine (E-C) | S-002 |
| 10 | Codec output filter | output level | | Datasheet template (A-027) |
| 11–13 | Volume, power amp, speaker, earphone | route indicator; the speaker greyed and labelled "muted while recording" (S-002) | amp clip indicator (A-031); speaker response (A-010) | Sourced parts; response assumed |
| 14–15 | Clock, power | static labels: 8 MHz resonator, 6 kHz sync | | S-002; power not modeled (A-013) |

### Record and playback paths

- **Recording:** the record path lights left to right into the flash; the playback path is dark; the speaker is visibly muted, which the service manual documents (S-002).
- **Playback:** flash, decoder, codec output filter, volume, then the selected output (speaker or earphone) light; the record path is dark.
- Until the record/playback split (decisions.md; E-C below) ships, the engine runs one pass from microphone to speaker and the inside can light only that single path.

### Tap to inspect

Tapping a component raises a card; dismissing it returns to the board. Nothing else on the board responds to touch. Card contents:

1. One neutral sentence on what the stage does to sound.
2. The live read-out, large.
3. Source and assumption IDs, confidence, and the engine version that last changed the stage.
4. Workbench only: "Listen here" (plays the current recording as heard at this stage), bypass, and the stage's parameters, with the "device default" button.

"Listen here" needs the recording's raw microphone capture. *(Superseded 2026-10-15: an Investigation session keeps the original microphone audio and sessions are free; Recorder mode is open, brief.md O-3.)* **Decided 2026-10-11:** the Workbench keeps the raw capture per recording; it is visible in the recording's details with a delete option, and the app's privacy label lists it. The free tier keeps only the device-rate render.

### Status badges and the header

Three badge states, distinguished by shape and label, not colour alone:

- **Sourced**: the value comes from a document (S-###).
- **Assumed**: a registered assumption (A-###) with its confidence.
- **Not modeled**: the stage is an empty socket.

A fourth state, **Checked against a real unit**, applies to no stage today. The board header says so plainly: *"Modeled on the service manual and codec datasheets. 0 of 15 stages checked against a real unit. Own one? Help check it."* The link opens the capture kit from the evidence program. The header counts stages, not features, and updates from the engine's stage manifest (E-D). The word "verified" is avoided in the interface because, next to a recording, it can be read as a claim about the recording.

### The flip, motion and accessibility

- The device turns over and the back cover slides aside; board indicators wake as audio arrives. Period cues live in typography and meter ballistics, not in drawn screws.
- The flip is never automatic and never required: a labelled button and a drag gesture both work, and it completes in well under a second.
- Reduce Motion (iOS) and Remove animations (Android) replace the flip with a crossfade. No element flashes faster than about three times per second (the WCAG 2.3.1 threshold; confirm the current text before writing an accessibility statement). Meters move with ballistics; indicators latch rather than blink.
- Each component is one accessible element with its name, badge and a throttled value; cards follow the system text size; the flat text list mirrors the board.

### Free and Workbench: see free, touch paid

*Superseded 2026-10-15: the engine's intended user adjustments are free and paid expansion is component packages (brief.md › Value, O-1, O-2). Kept as the 2026-10-11 record.*

Consistent with the decision that the free tier is the whole device and the Workbench is only what the device cannot do (decisions.md, monetization.md).

| | Free | Workbench |
|---|---|---|
| Flip and look: meters, indicators, paths, badges, card read-outs and sources | yes | |
| "Listen here" at a stage | | yes (the stage taps and mixer) |
| Bypass and parameters in the card | | yes |
| Storage gauge | 60 min / 99 recordings | the expanded memory on the same gauge |

### Wording on the inside

Say what the stage does to sound, with its status:

- "Limiter: holds loud sounds just below full scale. Modeled on the circuit (A-028); not checked against a unit."
- "VAS: recording. 3 pauses removed, 12 s."
- "Codec input: 225–2550 Hz at 6 kHz. Datasheet template (A-027)."
- "Speaker: muted while recording (S-002)."

Avoid: an "activity" indicator (name it "encoding" or "frames"), "detects", "senses", "listening for", "signal present", "anomaly", "spike", "event" as a label (use "splice" and "ceiling"), and "verified" (use "checked against a real unit"). The meters (spec 016) add three labels: the hold charge is "limiter hold" with a capacitor glyph, never a bare "charge" or "energy" ("how far the limiter's hold capacitor is charged: 0 resting, 1 the model's deepest hold; it rises on loud sound and drains over about ten seconds"); the voice activation reads "recording / paused", never "voice detected"; the output-clamp count is an emulator option (spec 010), shown outside the device drawing or only when the option is on, labelled as such, never as a part of the device; a route peak above 1.0 is "above the amplifier's clip ceiling (modeled)", not "overload". The scope is the riskiest element, because a waveform in this category is read as evidence: keep it, labelled with level and time, calm, with no marks other than the engine's own splice and ceiling events. Store screenshots of the inside carry the status header.

### Engine requests

*Status at engine 0.16.0 (2026-10-30, PR pending): E-A is built (spec 016, the per-block meters: eleven figures per call, read after the call; the guide's "Meters" section), E-C is built (spec 006, the record/playback split and the memory) and E-D is built (spec 007, the stage manifest); E-B and E-E are not built. Later requests from the 016 product review: **E-F, an amplifier limit indicator** (the deepest compression stage 2 applied in the call, dB ≥ 0, the form of the limiter's; rests on A-051) and **a timeline map** of each figure to input and recording sample positions (spec 020). Spec 015 (engine 0.16.0) defines the **two clocks** that map is built on: the playback stream's position (host output samples since `start_playback`, the drain included) and the one-pass stream's (host output samples since construction, `reset` or `reconfigure`, the clock the voice-activation events already use); every change a stream applies is reported at its position in that clock, so the timeline's change marks come from the engine's log, not from what the app sent. The record stream's clock joins with spec 017. See brief.md › Engine capabilities.*

Already in the engine (0.5): `BlockInfo.paused`, VAS events through `process_with_events` and `max_events`, per-stage bypass flags, one output `Tap`, `latency_samples`, `version_string`. The limiter's state is a test-only hook and its gain read-out is test-only. Each request below is phrased for `/speckit-specify`.

- **E-A: per-stage meters (new; the inside depends on it).** *As an app showing the chain live, I want one read-out per processed block of the input peak, the limiter's gain reduction (block minimum, dB) and hold-capacitor charge (0–1), the post-filter peak, the number of clamp hits, the VAS state and hang time remaining, and the post-playback peak, so that every moving element on screen is a number the engine computed.* A pull API that fills a caller-provided plain struct during processing; no allocation; the meters never change the output or its determinism. Tier: free.
- **E-B: a scope copy at a chosen stage (new).** *I want a read-only copy of one stage's output per block, written into my buffer, independent of the output tap, so a waveform at that stage can be shown or played without the tap change altering which stages run, the latency or the VAS behaviour.* Today `Tap::AfterAgc` skips the rate conversion and the VAS, so switching the tap live changes the chain. Caller buffer, block granularity. Tier: free to view, Workbench to listen.
- **E-C: the record/playback split with the flash model (already decided; see decisions.md).** Adds storage used, the output route (speaker or earphone) and mute-while-recording as readable state. Tier: free.
- **E-D: a stage manifest (new; small, high value).** *I want the library to export, for each of the 15 stages, its name, a one-sentence neutral description of what it does to sound, its status (sourced, assumed, not modeled, checked against a unit), its source and assumption IDs, its confidence, and the version that last changed it, as static strings, so that the About view is generated from the engine and cannot drift from the registers.* **Decided 2026-10-11:** the engine may carry these short, neutral, English strings; apps localize on their side. Not a real-time call; read at start-up. Benefits every library customer. Tier: free.
- **E-E: limiter ceiling events (later; Workbench review tools).** *I want the position and duration of each ceiling hold, in the same caller-buffer form as the VAS events, so review tools can mark where the limiter acted.*

Real-time safety: E-A, E-B and E-E are plain writes into caller memory per block and respect the no-allocation, no-I/O rule. Callbacks and spectrum analysis stay out of the core; the band display's spectrum is the app's job. Whether `reconfigure` preserves stage state must be checked before any live bypass toggle is built.
