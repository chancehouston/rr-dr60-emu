# UI brief: the three screens (for comps)

A plain-language description of the app for making visual comps. It deliberately leaves out engineering terms; the visual direction is the design dossier of 15 October 2026 (`design-dossier-2026-10-15.pdf`, pages 2 – 4): a vintage pocket recorder on the front, a theatrical field instrument for investigations, and a modular workbench styled like audio plug-in hardware, all designed for a phone held in one hand.

## What the app is

A phone app that behaves like a small handheld voice recorder from the late 1990s. It records through the phone's microphone, but everything you hear has passed through a software copy of that old recorder's electronics: its microphone, its automatic level control, its voice-activated pausing, its narrow telephone-like sound, its small speaker. Recordings sound the way that recorder made them sound, not the way a modern phone does.

People use it in two ways: as a recorder with a particular character, and as a field instrument for investigations where a recording is kept together with sensor readings, notes and markers so it can be reviewed later.

Everything in the three screens below is free. Extra equipment (additional components for the workbench) may be sold later; the comps do not need to show a store.

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
- Live readings from the phone's own sensors: magnetic field, motion, air pressure, and the ambient light and sound level if we show them. Each reading is a labelled instrument with its current value and a small recent-history trace.
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

## Screen 3: the workbench (the inside)

The back of the recorder, opened up: the chain of components the sound passes through, as a stack of modules. The look is dossier page 4: audio-plug-in character, large tactile controls, responsive meters, designed for a phone rather than a desktop.

What it shows:

- A compact overview of the chain, top to bottom, in the order the sound travels: the microphone, the level control, the voice-activated pause, the band filter, the speech coder, the memory, the playback filter, the volume and amplifier, the speaker or earphone. Each module has a name, a small meter or trace showing what it is doing right now, and an on/off switch where the component can be bypassed.
- Tapping a module expands it: its controls (knobs, switches, a few sliders), its live readings, and a one-sentence plain description of what it does to the sound. Each module also says whether it is modeled on documentation of the original recorder or is an addition of the app.
- A place to add components (the first additions would be things like reverb or an extra filter), reorder them where that makes sense, and remove them.
- A reset to "the recorder as modeled", which puts every component back to the original's behaviour.
- The elapsed time and a stop button stay visible at the top or bottom, because recording or playback may be running while the user explores the inside.

How it behaves:

- Changes are heard immediately, on whatever is playing or being recorded. Some controls apply only to playback, some only to recording; the module shows which.
- A few adjustments are physical conditions of a unit rather than controls, such as the battery voltage, which changes how the amplifier distorts when loud; these sit in their module as a dial with a sensible default.
- Presets: the original recorder, a few curated variations, and the user's own saved setups.

Feel: tactile and dense but never cramped; one module expanded at a time; everything reachable with a thumb; the meters move.

## Shared rules for the comps

- Phone first, portrait, one-handed. iPhone sizes first.
- Three screens, reached from each other by flipping the device over (front to inside) and by a clear move to the investigation screen; the user should always know which of the three they are on.
- Recording and playback must never be more than one tap away on any screen, and the running time must be visible on all of them.
- Plain labels. Name each control by what it does ("Playback speed", "Keep pitch", "Sensitivity", "Band filter") and each reading by what it measures ("Magnetic field, µT"). Avoid names that promise an outcome.
- Decoration and data stay separate: atmosphere in the frame, the readings in clean type.
- Numbers in the comps are placeholders, but keep them plausible: a 60-minute recorder, five speed steps, five sensitivity steps, readings in real units.
