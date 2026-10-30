# Records to write at implementation (018)

Wording rules: neutral; "modeled on the circuit and the amplifier's datasheet, not measured" (A-056, low to medium confidence); "behavior"; never "drained/dying/draining"; the value is the played unit's cells, set by the user, never the phone's battery, a sensor or a timer; the default is the nominal mark and is what an app calls "fresh".

## Guide: a section "Battery state (the speaker amplifier's supply)"

Lead: *"The speaker amplifier runs from the two cells directly. The battery voltage control sets those cells' voltage; the amplifier's clip ceiling falls with it, so loud playback through the speaker clips sooner and lower, and quiet playback is unchanged. The earphone output and recording are unaffected. Modeled on the circuit and the amplifier's datasheet, not measured (A-056). The value describes the played unit; it is set by the user and never follows the phone's battery, a sensor or a timer."* Then: the law (b = (V − 1.8) ÷ 1.2, the table of the six points and the alkaline trajectory, the published data), the default as the nominal mark, the 20 ms transition and its at-once cases, the schedule and the log entry, the meters' two fields (the count is the samples the amplifier's knee or ceiling changed, meaningful on the speaker route; `battery_ceiling` is a factor, 20·log10 for dB), the clamp's behavior on clipped material (it engages above about 2.32 – 2.49 V by host rate), the range (2.2 V is where the device would show its low-battery indicator and stop; lithium primaries out of range), what is not modeled (the sag, a drain, supply noise). The real-time list gains the five calls.

## CHANGELOG 0.18.0

An **assumption-status change** entry: A-056 adopted as the static law; stage 15 assumed at 0.18.0; the control appended at index 29 (spec 014's order rule restated: later entries append, indices stable; for the pre-1.0 ABI pass); the transition; the meters' growth (56 → 64 under the compatible prefix-fill rule, spec 016 amended: a 0.17 binary keeps working); `CONTROL_COUNT` 30; the public law, the NiMH constant, the trajectory table (the seven rows with their dB; hours of recording; row 0 not the default); the new golden (259 entries, SHA); the manifest, README and guide re-blesses (the meters golden unmoved); the cost figures; "not a measured figure"; the product rule. Also the header's `RrDr60Meters` comment (56 → 64 and the rule), the README's "What exists today" bullet and the real-time lists in the README, the guide and the C crate doc, CLAUDE.md's status paragraph, and the hardware-evidence issue template's P5 line ("decides the knee shape"). The Swift package does not reference `RrDr60Meters`; its parity test needs only the regenerated header in the XCFramework.

## Registers

A-056: status "adopted by spec 018 (2026-10-30; the static law; the sag not modeled)" with the knee-shape alternative sentence and "the law understates the loss on loud peaks late in the discharge" (S-006's VOH at 75 mA); A-031: the pointer; signal-chain.md rows 11 and 15. The S-001 wording for the low-battery mark is checked against the manual's text before the guide prints "indicator" (product M8).

## Product notes

- controls-discussion.md § 1c row "Amplifier clip ceiling 1.15 V; gain law" and § 1d row "Battery state on the speaker route": the italic status line *"**Modeled by spec 018 (engine 0.18.0, DATE):** the static law as a live unit condition, `battery_volts`, 2.2 – 3.0 V, default 3.0 V (factor 1); the sag stays not modeled."*; § 2b's opening status paragraph "at engine 0.18.0" with the same line on the battery row; § 3's strings row cites the manifest sentence as the engine's own instance of the rule and carries the preset names below; § 4 item 5: an italic status line (built at the recommended scope; the clip meter and the public law added at the product review).
- decisions.md: one row: *"Battery law, alkaline trajectory and the NiMH figure are public engine data and the amplifier's clip count and ceiling factor join the meters; preset names stay in the app; the value is set by the user, never the phone's battery, a sensor or a timer | hard-coded app volts; the route peak as the indicator | 018 product review M1 – M3; spec 018 D8, D10, D11 | low"*.
- brief.md § "1. Inventory the controls": the battery voltage joins the **Live controls** list (which also gains the speed, missing since 0.13), not the playback-side settings; the "Not separated today" paragraph dated as superseded by spec 014's kinds.
- investigation-mode.md example cards: *"**Amplifier:** 'The speaker amplifier ran into its clip knee for 0.6 s at 2.4 V cells, a ceiling 6 dB below nominal. Quiet passages are unchanged. Modeled on the circuit and the amplifier's datasheet, not measured (A-056, A-031).'"* (figures illustrative).
- For spec 020: the session snapshot carries the starting values of the four live controls; the battery is playback-side.

## App preset copy (allowed names, for the app, not the engine; every value an engine constant)

"Fresh (as modeled)" = `DEFAULT_BATTERY_VOLTS`; "Half used (4 h of recording, 2.52 V)" = trajectory row 3; "Nearly flat (10 h, 2.22 V)" = row 6; "NiMH pair (2.45 V)" = `NIMH_PAIR_VOLTS`. No dB in the label; the detail view prints `ceiling_db`. Never "dying", "drained", "draining", or anything tying the value to activity.
