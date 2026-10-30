# Records to write at implementation (018)

Wording rules: neutral; "modeled on the circuit and the amplifier's datasheet, not measured" (A-056, low to medium confidence); "behavior"; never "drained/dying/draining"; the value is the played unit's cells, set by the user, never the phone's battery, a sensor or a timer; the default is the nominal mark and is what an app calls "fresh".

## Guide: a section "Battery state (the speaker amplifier's supply)"

Lead: *"The speaker amplifier runs from the two cells directly. The battery voltage control sets those cells' voltage; the amplifier's clip ceiling falls with it, so loud playback through the speaker clips sooner and lower, and quiet playback is unchanged. The earphone output and recording are unaffected. Modeled on the circuit and the amplifier's datasheet, not measured (A-056). The value describes the played unit; it is set by the user and never follows the phone's battery, a sensor or a clock."* Then: the law (b = (V − 1.8) ÷ 1.2, the table of the six points and the alkaline trajectory, the published data), the default as the nominal mark, the 20 ms transition and its at-once cases, the schedule and the log entry, the meters' two fields, the clamp's behavior on clipped material, the range (2.2 V is where the device would show its low-battery indicator and stop; lithium primaries out of range), what is not modeled (the sag, a drain, supply noise). The real-time list gains the five calls.

## CHANGELOG 0.18.0

An **assumption-status change** entry: A-056 adopted as the static law; stage 15 assumed at 0.18.0; the control, the transition, the meters' growth (56 → 64; a 0.17 caller's size refused), `CONTROL_COUNT` 30, the public law, the trajectory table (the seven rows with their dB), the new golden (entries, SHA), the meters and manifest re-blesses, the cost figures, "not a measured figure", the product rule.

## Registers

A-056: status "adopted by spec 018 (2026-10-30; the static law; the sag not modeled)" with the knee-shape alternative sentence; A-031: the pointer; signal-chain.md rows 11 and 15.

## Product notes

- controls-discussion.md § 2b battery row: *"**Modeled by spec 018 (engine 0.18.0, DATE):** the static law as a live unit condition, `battery_volts`, 2.2 – 3.0 V, default 3.0 V (factor 1); the sag stays not modeled."*; § 4 item 5: an italic status line (built at the recommended scope; the clip meter and the public law added at the product review).
- decisions.md: one row: *"Battery law and alkaline trajectory are public engine data and the amplifier's clip count and ceiling factor join the meters; preset names stay in the app | hard-coded app volts; the route peak as the indicator | 018 product review M1, M3 | low"*.
- brief.md: "the battery voltage (unit condition, spec 018)" in the "Playback side" inventory; the "Not separated today" line dated.
- investigation-mode.md example cards: *"**Amplifier:** 'The speaker amplifier clipped for 0.6 s at 2.4 V cells, a ceiling 6 dB below nominal. Quiet passages are unchanged. Modeled on the circuit, not measured (A-056, A-031).'"* (figures illustrative).
- For spec 020: the session snapshot carries the starting values of the four live controls; the battery is playback-side.

## App preset copy (allowed names, for the app, not the engine)

"Fresh (default, 3.0 V)", "Half used (2.5 V, about −5 dB)", "Nearly flat (2.2 V, about −9.5 dB)", "NiMH (2.4 V)".
