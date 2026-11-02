# UI brief: the three screens (for comps)

A plain-language description of the app for making visual comps. It deliberately leaves out engineering terms; the visual direction is the design dossier of 15 October 2026 (`design-dossier-2026-10-15.pdf`, pages 2 – 4): a vintage pocket recorder on the front, a theatrical field instrument for investigations, and the inside, a modular bench of components styled like audio plug-in hardware, all designed for a phone held in one hand.

## What the app is

A phone app that behaves like a small handheld voice recorder from the late 1990s. It records through the phone's microphone, but everything you hear has passed through a software copy of that old recorder's electronics: its microphone, its automatic level control, its voice-activated pausing, its narrow telephone-like sound, its small speaker. Recordings sound the way that recorder made them sound, not the way a modern phone does.

People use it in two ways: as a recorder with a particular character, and as a field instrument for investigations where a recording is kept together with sensor readings, notes and markers so it can be reviewed later.

Everything in the app is free, for good, including every later update: there is no store, no purchase and nothing locked, so the comps show no prices, locks or upgrade prompts (decided 2026-11-01). The three screens below describe the app as it grows. **App 1.0** is the front with its device controls, the investigation screen, and the inside **without controls** (the chain, its meters and its plain descriptions); the inside's switches, knobs and presets, and the place to add a component, arrive in free updates after 1.0, the first of them the effects update. Comps may show the full picture; mark what comes after 1.0.

## Screen 1: the recorder (the front)

This is the home screen and the first thing a user sees. It is the front panel of the recorder.

What it shows:

- A small display with the elapsed time, a recording indicator, the current item number and how much recording time is left (the original holds about 60 minutes, up to 99 items).
- A level meter that moves with the sound.
- A big record/stop button, a play button, a way to reach the list of recordings, and a "more" control.
- The physical controls the original recorder had: playback speed (five steps, slow to fast), microphone sensitivity (five steps), speaker or earphone, and volume.
- A way to turn the device over to see its insides (this leads to screen 3) and a way to go to the investigation screen (screen 2).

How it behaves:

- Press record and it records; the meter shows the level; when the room goes quiet the recorder pauses by itself and resumes when sound returns (a small indicator shows "paused, waiting for sound").
- Press play to hear the last recording through the modeled speaker; the time display counts along.
- The playback speed control works while playing. Slowing down lowers the pitch, like the original machine. A separate switch can keep the pitch the same while the speed changes; label it as the app's own option, not something the original did.
- Recordings can be played, renamed, shared and deleted from the list.

Feel: the vintage hardware look from dossier page 2, left phone. Readable at arm's length, usable with a thumb, the record button always reachable. The front should feel calm and solid; the drama belongs to screen 2.

## Screen 2: the investigation (live, or review)

One screen with two faces. It is either the live investigation or the review of a past session, never both at once; the user flips between them (or the screen turns over). The look is dossier page 3: the theatrical field instrument, with the real readings kept clearly separate from the decoration.

### Live face

What it shows, stacked on one tall screen:

- The audio, recording through the same recorder as screen 1, with its level and elapsed time.
- Live readings from the phone's own sensors: magnetic field, motion and air pressure. *(The ambient light and sound level come after 1.0, decided 2026-11-02.)* Each reading is a labelled instrument with its current value and a small recent-history trace.
- A session clock that everything is stamped against.
- A "mark this moment" button for the user to flag something while it happens, with an optional quick note or a spoken note.
- A record/stop control that is always reachable, and the recorder's own controls (sensitivity, speed) within reach.

How it behaves:

- Starting a session starts the recording and the sensor logging together.
- Marks appear on the audio trace as the session runs.
- When the recorder pauses itself in silence, the session clock keeps running and the pause is visible, so a later reviewer knows there is a gap in the audio but not in the sensor readings.
- Stopping ends the session and offers to go straight to the review face.

### Review face

What it shows:

- The whole session on one shared timeline: the audio waveform on top, the sensor traces underneath, aligned in time, with the marks drawn across all of them.
- A playhead the user can scrub; the readings at the playhead are shown as numbers next to each trace.
- A list of marks with their times and notes; tapping one jumps there.
- Transport controls: play, pause, skip to the previous or next mark, loop a region.
- Playback speed, with the same pitch-kept option as the front, for listening to a passage slowly.
- Export: the audio, the readings and the marks as files, and a short written summary of the session.

How it behaves:

- Playback goes through the modeled recorder, so a slowed passage still sounds like the old machine.
- Changing speed or volume during review is recorded in the session log (so what was heard can be reproduced), but the review never changes the recording itself.

Feel: dossier page 3 for both faces. Brass, glowing elements and dials are welcome as atmosphere; the numbers, traces and marks must stay plainly readable and visibly separate from the decoration. Nothing in the interface should suggest that a reading or a sound means anything beyond what it is; the app describes what it measured and what it recorded, never what it found.

## Screen 3: the inside

The back of the recorder, opened up: the chain of components the sound passes through, as a stack of modules. The look is dossier page 4: audio-plug-in character, large tactile controls, responsive meters, designed for a phone rather than a desktop.

What it shows:

- A compact overview of the chain, top to bottom, in the order the sound travels: the microphone, the level control, the codec's input filter, the voice-activated pause, the speech coder, the memory, the playback filter, the volume and amplifier, the speaker or earphone. Each module has a name and a small meter or trace showing what it is doing right now; after 1.0, an on/off switch where the component can be bypassed.
- Tapping a module expands it: its live readings and a one-sentence plain description of what it does to the sound; after 1.0, its controls (knobs, switches, a few sliders). Each module also says whether it is modeled on documentation of the original recorder or is an addition of the app.
- After 1.0 (the effects update): **one slot** for an added component, between the microphone and the level control, on the recording side. "Add component" offers the band filter, shown as "Band filter (added)" and labelled as an addition of the app, not part of the original recorder; "Remove" empties the slot. The slot holds one component at a time, so there is nothing to reorder. The band filter applies to new recordings (and to a recording in progress), never to recordings already made. Later free updates add more components to choose from.
- After 1.0: a reset to "the recorder as modeled", which puts every component back to the original's behaviour and empties the slot.
- The elapsed time and a stop button stay visible at the top or bottom, because recording or playback may be running while the user explores the inside.

How it behaves:

- Playback-side changes are heard immediately on whatever is playing. Recording-side changes apply to new recordings, and some (such as the sensitivity) also to a recording in progress; a few (the speech coder, the memory) change only between recordings. The module shows which.
- After 1.0: a few adjustments are physical conditions of a unit rather than controls, such as the battery voltage, which changes how the amplifier distorts when loud; these sit in their module as a dial with a sensible default.
- Presets, after 1.0: the original recorder, a few curated variations, and the user's own saved setups. (In 1.0 the only presets are the investigation screen's voice-activation presets, labelled "units differ", beside the factory default.)

Feel: tactile and dense but never cramped; one module expanded at a time; everything reachable with a thumb; the meters move.

## Shared rules for the comps

- Phone first, portrait, one-handed. iPhone sizes first.
- Three screens, reached from each other by flipping the device over (front to inside) and by a clear move to the investigation screen; the user should always know which of the three they are on.
- Recording and playback must never be more than one tap away on any screen, and the running time must be visible on all of them.
- Plain labels. Name each control by what it does ("Playback speed", "Keep pitch", "Sensitivity", "Band filter (added)") and each reading by what it measures ("Magnetic field, µT"). Avoid names that promise an outcome.
- Decoration and data stay separate: atmosphere in the frame, the readings in clean type.
- Numbers in the comps are placeholders, but keep them plausible: a 60-minute recorder, five speed steps, five sensitivity steps, readings in real units.
