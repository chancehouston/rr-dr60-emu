# Product brief

**The current product brief for the app built on the `rr_dr60` engine.** Where it disagrees with an older note in this folder, this brief wins; the older notes are marked where they are superseded.

- **Current as of:** 2026-10-16.
- **Sources:**
  - the design dossier *One instrument. Two ways to explore.* (v0.1, 15 October 2026, [design-dossier-2026-10-15.pdf](design-dossier-2026-10-15.pdf)), prepared from the product-design conversation with Chance Houston that day;
  - the product decisions of 2026-10-11 that the dossier does not change (decisions.md);
  - the engine on `main` at version 0.11.0, read on 2026-10-16 for the capability facts below;
  - the owner's working notes of 2026-10-21 (*RR-DR60 Emulator App — Working Notes, Draft 1*, a Claude doc; exploratory), for the items dated 2026-10-21. Its "validated", "chip-level" and "reproduces exactly" wording was not adopted: it conflicts with the claims policy and the 2026-10-18 scope decision.
- **Not a screen specification.** The dossier's renderings are illustrative: their controls, routings and readings are placeholders, and the sensor readings in them are invented for the concept.
- **Claims policy.** Everything here follows the policy in `CLAUDE.md`: describe what the signal chain does to sound, say "modeled on" until a stage is checked against a real unit, and make no claim about paranormal phenomena either way.

Each statement carries one of four labels:

| Label | Meaning |
|---|---|
| **Agreed** | Decided in the 2026-10-15 conversation and recorded in the dossier. |
| **Standing** | Decided 2026-10-11, not changed by the dossier, still in force. |
| **Open** | Not decided. Questions marked ★ need Chance's decision before the dependent work starts. |
| **Needs engine support** | A design requirement or candidate that the engine does not provide today (see "Engine capabilities"). |

## The product in one paragraph

One instrument with two ways to explore it. It is a complete, free recorder with a physical front-and-back relationship:

- **The front** is for recording simply.
- **The back** is what you see when you flip the instrument over: the audio internals, working.

**Recorder mode** is a home for vintage-hardware and audio enthusiasts. **Investigation mode** is a field instrument that records audio together with synchronized tracks from the phone's own sensors. Both modes share the engine and the explorable internals.

The free app is generous: both modes, the visible internals, sensor sessions, and the adjustments the engine intends users to make. Paid expansion arrives as packages of extra components that open a new experience, not as a charge for each control. iPhone first, Android planned.

## Agreed direction (2026-10-15)

### Platform and core

- **Agreed:** phone first, iPhone before Android. Live recording is the core experience.
- **Agreed:** the original front-and-exposed-back concept (dossier concept 01) stays the visual baseline. The app borrows the language of vintage hardware but has its own identity.
- **Agreed:** the internals blend a believable rendering of electronics with a signal path the user can read.

### Two modes

Modes define the task; both share the recording engine and the explorable internals.

- **Agreed, Recorder mode:** the vintage audio experience. Credible test-equipment styling, intuitive live recording, and an interior that makes the sound process visible.
- **Agreed, Investigation mode:** a dedicated field instrument.
  - It records audio together with synchronized phone sensor tracks, with live capture and field readings on one screen, and session review with linked tracks on another.
  - It has its own theatrical visual direction (dossier concept 02: brass, glowing tubes, steampunk cues).
  - Measurements stay clearly labelled and distinct from atmosphere.
  - Investigation mode keeps the flip to the internals.

### The flip

- **Agreed:** the front and the back are two sides of one instrument.
- **Agreed:** flipping reveals the process already running and never interrupts a recording.
- **Agreed:** recording state and Stop stay reachable on the back.
- **Agreed:** large controls and a persistent Stop button take priority over desktop-style density.

### Skins

- **Agreed:** skins are a later appearance layer. A Winamp-inspired skin stays on the future design list.
- **Agreed:** a skin changes materials, colours and meter styling, never function.

### The back as a workbench

Dossier concept 03 shows the back as a workbench of modules in a phone-sized "VST character": distinct modules, tactile controls, responsive meters, inside the removed-back-panel metaphor.

- **Agreed** as a direction.
- **Needs engine support** for the proposed flow:
  - choose an insertion point and add a component;
  - inspect its controls;
  - bypass it for comparison;
  - move or remove it where supported.
- Placement before or after a stage can change the result, so **the supported insertion points must come from the engine contract**, not from the app.
- **Proposed:** a front-panel indicator that shows when a custom chain is active.

### Investigation sessions

A recording becomes a documented session. The whole session design is **Agreed** as a direction, and every item below **needs engine support or platform checks**.

- **One elapsed timeline.** Audio, sensor samples, event markers and setting changes map to one session clock. Voice-activated recording may skip silence; the gaps are kept as explicit timing metadata, not by compressing the session history.
- **Captured and processed sound.**
  - The original microphone audio is kept alongside the emulated or effect-processed result.
  - The active chain and settings are saved, and changes during a session are timestamped, so a later listener knows what processing was applied.
- **Context.**
  - Candidate tracks: magnetic field, motion and pressure, where the phone supports them.
  - *2026-10-21, the owner's idea, part of Investigation mode's feature set:* a **vibration track** from the accelerometer, active only while the phone lies still (investigation-mode.md › Vibration track).
  - Recorded with them: device identity, units, sampling information, sensor accuracy or status where the platform provides it, missing data and interruptions.
- **Review with a shared cursor.** Select a sound and see the phone's readings at that moment, including movement and capture gaps.
- **The interface treats a coincidence as something to examine and never labels it proof of a paranormal cause.**

### Value

- **Agreed principle:** the free app contains the good stuff:
  - both modes;
  - sensor-session functionality;
  - watching the internals;
  - the adjustments the engine intends for users.

  Not all tinkering is paid.
- **Agreed direction:** paid expansion is packages of complementary components (examples discussed: equalization, reverb, flanging, more unusual processing) with example chains and presets.
  - Users can listen, compare, take the chain apart and build their own.
  - One purchase should feel like opening a useful box of equipment.
  - Purchased components appear in the same back-panel language and feel integrated into the workbench.
- **Open:** the products, the prices, the package contents and the exact free/paid line. No subscription model or pricing has been selected.

### Ownership

- **Agreed:** phone sensors, permissions, styling and purchase presentation live in the app layer, unless a shared contract needs otherwise.
- **Agreed:** the engine returns capability facts before the UI or the commercial boundary is locked.

## Standing decisions (2026-10-11, unchanged)

The dossier does not change these. Details and reasoning are in decisions.md.

- **Positioning:** "reproduces the RR-DR60's signal chain stage by stage, from the service manual, and shows which stages are checked against a real unit".
- **Trademark:** no trademark ("Panasonic", "RR-DR60", "DR60") or anything confusingly similar in the app name. The device is named once in the description, with a trademark notice.
- **Privacy:** no analytics or advertising SDK at launch. App Privacy and Data safety both say "data not collected".
  - The sensor tracks and the raw microphone audio are processed and stored on the device. Re-check both labels against the final session design.
- **Ads:** none, and no consumables.
- **Subscriptions:** none at launch. The dossier selects none either; confirmation is question O-6.
- **Evidence program:** *superseded 2026-10-18:* real-unit checks are out of scope until the project has a unit it can open (decisions.md). The status header still shows the truth: no stage checked against a real unit.
- **Engine distribution:** a Swift Package wrapping an XCFramework (later an Android archive), not a source port.
- **Stage strings:** the engine carries short, neutral, English strings per stage (the stage manifest), and apps localize them.
- **Status vocabulary:** product copy says "checked against a real unit", never "verified".
- **The status header and badges on the inside:** sourced, assumed, not modeled, checked against a real unit (front-and-inside.md › Status badges).
- **Honest motion:** a component on the inside animates only from a number the engine returned for the current block. Unbuilt stages are labelled empty sockets.
- **Accessibility:**
  - Reduce Motion replaces the flip with a crossfade.
  - No element flashes faster than about three times per second.
  - Each component is one accessible element, and a flat text list mirrors the board.
- **Copy rules:** describe signal processing only. Never "capture", "reveal", "enhance", "communicate"; never "fake", "debunk", "entertainment only" (monetization.md › Copy rules).

## Superseded

These earlier decisions no longer hold. Each is marked in its own note, and each replacement is labelled Agreed or Open above.

| Earlier decision (date, note) | Now |
|---|---|
| The vintage-gear audience stays in the copy but is not built for until evidence of it appears (2026-10-11, decisions.md, personas.md) | **Agreed:** Recorder mode is built as its home. *2026-10-18:* listeners whose main interest is a checked, authentic sound are no longer targeted (decisions.md); **Open (O-15):** Recorder mode's framing. |
| The free inside has no controls at all; every control is Workbench ("see free, touch paid") (2026-10-11, decisions.md, front-and-inside.md › Rules, › Free and Workbench) | **Agreed:** the engine's intended user adjustments are free. **Open (★ O-1):** which controls those are. |
| The paid tier is one non-consumable called "Workbench", $7.99 (tested between $4.99 and $9.99), with a fixed list: expanded memory, stage taps and mixer, "Listen here", raw-capture retention, bypass and parameters, folders and search, stem export, review tools (2026-10-11, monetization.md) | **Agreed:** paid expansion is component packages with example chains and presets. **Open:** one workbench unlock or several experience packs, the price, and where each old Workbench item now belongs (★ O-2). |
| The raw microphone capture is kept only with the Workbench; the free tier keeps only the device-rate render (2026-10-11, decisions.md, front-and-inside.md › Tap to inspect) | **Agreed:** an Investigation session keeps the original microphone audio, and sensor sessions are free. **Open (★ O-3):** whether raw audio is kept in Recorder mode, whether it is free there, and how the user deletes it. |
| A reduced inside ships in 1.0 with no touch controls; the Workbench purchase follows in 1.1 (2026-10-11, decisions.md) | **Open (★ O-4):** release sequencing for the two modes, the free adjustments and the first package. |

## Open questions

### Needing a decision before the dependent work (★)

- **O-1 ★ Which controls are the free "intended user adjustments"?**
  - The engine has no such list yet; producing one is the dossier's first engine question.
  - The candidates fall into three groups:
    1. the device's own controls: volume, speaker or earphone, voice activation on or off, the sensitivity setting; *(corrected 2026-10-18: voice activation on or off is not a device control, since the device has no VAS switch, S-001, A-008; it is an emulator option. Playback speed, built in 0.13, is a device control. See controls-discussion.md.)*
    2. the model's internal parameters: the limiter's knee and timing, the voice-activation threshold and hang time, a stage's bypass, the speech codec on or off;
    3. effects beyond the device, which the dossier places in paid packages.
  - The question is whether group 2 is free, paid or split.
- **O-2 ★ The paid boundary and its shape.**
  - One workbench unlock or several experience packs; future pack access; store presentation.
  - Where each item of the old Workbench list now sits:
    - expanded memory beyond 60 minutes and 99 recordings;
    - stage taps and "Listen here";
    - stem export;
    - folders and search;
    - review tools (candidates added 2026-10-21: reverse playback and a spectrogram; slow playback is the device's own speed control, free).
- **O-3 ★ Raw microphone audio.**
  - Kept in both modes or only in Investigation sessions; free or paid in Recorder mode.
  - How it interacts with voice activation (the dossier asks this).
  - Where the user sees and deletes it. The privacy labels follow from the answer.
- **O-4 ★ Release sequencing.** What ships first:
  - Recorder mode alone, or both modes;
  - the reduced inside of 2026-10-11, or the free adjustments with it;
  - when the first paid package arrives.
- **O-5 Default mode.** Which mode the app opens in, and how the user chooses or switches.
- **O-6 Subscription.** The 2026-10-11 decision (none at launch) stands unless reopened. Confirm, given that "future pack access" is now an open question.

### Design and product (no decision needed yet)

- **O-7 Skins:** the scope of the skin system.
- **O-8 Navigation:** the exact navigation between front, back and modes. Store placement and screenshots: the 2026-10-11 decision made the inside the second screenshot.
- **O-9 Live editing:** what can change while recording, and what needs a stop. This depends on the engine facts below.
- **O-10 Taglines:** the public wording.
  - "The front is the recorder. The inside is the model." (2026-10-11) concerns the flip.
  - "One instrument. Two ways to explore." (the dossier's title) concerns the modes.
  - Either or both may be used publicly; not decided.
- **O-11 Sensor wording:** each reading is named by what the phone's sensor measures, with its unit and status: magnetic field from the magnetometer in µT, acceleration in g, pressure in hPa.
  - The wording for theatrical labels such as "field" and "live", and any use of category terms such as "EMF", needs a product and store-review read before copy is written. The category's apps sell sensor readouts with strong claims (competitors.md).
- **O-12 Markers:**
  - The inside's scope shows no marks other than the engine's own events (front-and-inside.md › Wording).
  - Investigation review adds the user's own markers (MARK in concept 02).
  - Proposed reconciliation: user markers are allowed in Investigation review and always labelled as the user's. The inside's scope rule stays.
  - *2026-10-21:* a second kind of user mark, "Question asked here", one tap during a session.
- **O-13 Controls and AI control:** the 2026-10-15 request to give the app's users more to "play" with and to allow an AI to drive the engine's controls is not in the dossier. *(2026-10-18: the decision brief is [controls-discussion.md](controls-discussion.md), against engine 0.13.0.)*
  - It joins O-1 and the dossier's engine question 1.
  - It includes how a fast stream of volume changes behaves: each change restarts the 20 ms volume transition, so updates faster than about 60 per second make the knob lag.
- **O-15 Recorder mode's audience (2026-10-18):** with real-unit checks out of scope, the product no longer targets listeners whose main interest is a checked, authentic sound. Does Recorder mode keep its "home for vintage-hardware and audio enthusiasts" framing, become the plain recorder front for every user, or merge into the tinkerer's view? Reconsider the dropped audience if demand appears.
- **O-16 An "entertainment" disclaimer (2026-10-21).** The working notes lean toward "For entertainment purposes" in the store description and the About screen, to cover App Review. The copy rules (2026-10-11) ban "entertainment only" because it reads as a debunk. Options:
  1. keep the rule and write a neutral statement of what the app does instead, in the style competitors use ("measures and reacts to changes in the environment only", competitors.md);
  2. allow "for entertainment" in the store description and About screen only, never in the app's working copy;
  3. decide after the first review: submit with option 1 and add the phrase if App Review asks for it.

  Undecided; the owner is unsure (2026-10-21).
- **O-17 First submission and App Review 4.2 / 4.3 (2026-10-21).** This is the owner's first App Store submission, so review readiness is a design concern.
  - Guideline 4.2 (minimum functionality): a record button with a filter is at risk; the app must feel like a full instrument.
  - Guideline 4.3 (spam): aimed at template clones and lookalikes; a reviewer should see the difference within about 30 seconds of opening the app (the flip to the inside, the stage statuses).
  - Common fixable rejections: crashes, a missing privacy policy, broken links, digital goods sold outside in-app purchase.
  - The category's table stakes, which 1.0 should meet: raw microphone capture, voice activation, instant playback, a waveform to scrub, a session library with tags, trim, export. (Silence removal is the device's own VAS.)
- **O-14 Implementation questions from the dossier:**
  - clock mapping and drift;
  - battery and storage cost;
  - permission flows;
  - background behaviour;
  - synchronization accuracy;
  - export format.

## Engine capabilities (0.11.0) against the dossier's five questions

**This section states capability facts read from the code on `main` (engine 0.11.0) on 2026-10-16. It creates no engineering work: anything the dossier needs that is absent here becomes a spec only through the project's spec workflow.** "To confirm" marks a point that needs checking against the code or a measurement before the app relies on it.

### 1. Inventory the controls

**Live controls** change while the engine runs, without a reset:

- **The volume position.** A change moves the knob over 20 ms (spec 011).
- **The output route:** speaker or earphone.

**Settings** apply only through `reconfigure`. It is refused while a recording or a playback is in progress, it may allocate, and it is not real-time safe. Today no setting can change during a recording. The settings are:

- **Record side:**
  - the limiter: on/off, knee, attack (dead time), release;
  - the codec input filter, and the full-scale clamp;
  - voice activation: on/off, mode (skip or mute), sensitivity 1–5, threshold, hang, onset;
  - the speech codec on/off.
- **Memory:** capacity and maximum number of recordings.
- **Playback side:**
  - the codec output filter;
  - the volume/amplifier, speaker and earphone stages;
  - the optional output clamp.
- **Plumbing:** host sample rate, output tap, noise seed.

**Not separated today:** intended user controls versus calibration or implementation settings. The device's own controls (volume, route, voice activation on/off, sensitivity) are distinguishable from the model's parameters by their sources, but the engine marks no such split.

**Not built:** two device features that the 2026-10-11 free-tier list includes.

- **SLOW/FAST playback speed.** Stage 10's manifest entry names it as not modeled. *(Built since 0.13.0, spec 013: five live levels, pitch moving with speed, A-054.)*
- **10 SEC REVIEW** (S-002 control S11). The playback calls have no seek, so it is not modeled. **To confirm:** whether the app can build it from the existing calls, before promising it.

### 2. Expose meaningful activity

**Per processing call, the engine returns:**

- the samples produced;
- whether voice activation paused the output;
- codec frames;
- output position and input length;
- the voice-activation splice events (or muted regions), written into caller memory without allocating.

**The stage manifest** describes the fifteen stages: name, one-sentence description, status, sources, assumptions, confidence and model version. Today 10 stages are assumed and 5 are not modeled; none is sourced or checked against a real unit.

**Not built: per-stage meters.** These were product request E-A of 2026-10-11:

- input and stage levels;
- the limiter's gain reduction and charge;
- clamp hits;
- the voice-activation state and hang time remaining.

The limiter's internal state is a test-only hook. Also not built: a scope copy at a chosen stage (E-B), limiter ceiling events (E-E), and a clip count.

**Taps:** one output tap is chosen per configuration, from nine. Changing it is a `reconfigure`, and some taps skip stages.

**Animation from engine numbers is limited today to:** the pause state, the splice events, the frame count, the volume, the route, and the memory status.

### 3. Define extension points

**The pipeline is fixed.** Stages can be bypassed through settings, applied by `reconfigure`. There is no insertion, reordering, removal or third-party module interface; that is the API-shape decision of spec 001.

**Effects beyond the device** (reverb, EQ, flanger) do not exist in the engine. They could run in the app around the engine's input or output, or become engine stages. Where they live is an architecture decision for a later spec, not a fact today.

**Latency reporting exists:** total, record path, playback path, and drain.

### 4. Support reproducible sessions

**Deterministic:** the same input, configuration and seed give bit-identical output on every platform.

**Voice-activation timing:**

- In skip mode the recording is shorter than the input, and each splice is reported with its position.
- In mute mode the timeline is kept and the muted regions are reported.
- Either way, a session clock can map the gaps.
- **To confirm:** the exact sample-to-session mapping across the resampler and codec latencies, before the timeline is built.

**Raw microphone audio:** the engine does not keep it. The app holds the input it feeds in.

**Recordings:** export and import as a versioned, checksummed byte sequence (stored form version 2).

**Settings:** a fixed struct with a version-checked size in C. There is no serialization format for settings or for a chain, and no parameter-change log. A versioned session contract does not exist.

### 5. Ownership

- **App:** phone sensors, permissions, styling and purchase presentation.
- **Engine:** no I/O, no clock, no allocation in the audio path.

These match the dossier's split.

## Concept references

The three concept images are on pages 2–4 of [the dossier](design-dossier-2026-10-15.pdf).

| Concept | Shows | Notes |
|---|---|---|
| **01 Recorder: front and exposed back** (accepted visual baseline) | Front: an LCD-style timer and level meter, a large record/stop button, play, recordings, more, and an "Inside" flip button. Back: a stylized circuit board with stage modules (input, limiter with a threshold knob, voice gate, codec, output), each with a waveform and meter, and the recording state and Stop at the bottom. | Stage names and knob settings are placeholders. Per-stage meters need E-A. A limiter knob is a model parameter (O-1). |
| **02 Investigation: a theatrical field instrument** | Live: an audio scope, then magnetic field, motion and pressure read-outs, record, mark, and the flip. Review: linked tracks (audio, magnetic, motion, pressure) under one cursor, a marker card and transport. | A live/review pair, not front/back. Readings are invented for the concept. Wording: O-11, O-12. |
| **03 Component workbench** | A chain of modules (input, recorder core, an added reverb, output) with "Add component", and an expanded reverb module with decay, tone, mix, move and remove. | Add, inspect, bypass and move are proposals pending engine support (question 3). |

## Next review (from the dossier)

1. Map the engine inventory onto the three concepts.
2. Confirm the supported interactions.
3. Prototype the phone layout and the telemetry before treating the renderings as a specification.
