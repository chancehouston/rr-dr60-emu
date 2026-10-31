# Records to write at implementation (019)

Wording rules: neutral; the crossfade is **the engine's convention** for the moment of a level change, never a device behavior; A-054's amendment reads as a hypothesis: "The emulator's crossfade is a convention, not a model of the device. What the device does at the moment of a level change is unknown: no document describes it. Because the decoder works in 20 ms frames, a switch at a frame boundary is one candidate; an instant step and a brief mute remain candidates. Capture P1 decides." (product M2); "behavior"; never "no longer ticks" or any tick-free promise: say the instant switch's step is replaced by a 20 ms blend and quote the harness's click-energy reduction per pair (product M3, hardware B10); the harness reproduces every figure before a record quotes it (the reviewer's 44.7 – 53.8 dB is a simulation until then).

## Guide, docs/guide.md, the playback-speed section (replacing "A change is a step: … above about 1.6 kHz.")

> A change blends: for 20 ms (120 device samples, `speed::CROSSFADE_MS`, C `RR_DR60_SPEED_CROSSFADE_MS`) the old level fades out at its own place and pitch while the new level fades in from the change, a smoothstep crossfade, which replaces the instant switch's step on content above about 1.6 kHz with a 20 ms blend (the harness reports the click-energy reduction per level pair: FIGURES dB on the 2 and 2.4 kHz tone cases); a request made during a blend lands when the blend ends, at most 20 ms later (plus at most four device samples for a change into level 3), so a schedule with requests under 20 ms apart plays longer by the waits, and every request is logged at its own position with target levels. The blend is the engine's convention (A-054); what the device does at the moment of a change is unknown (capture P1).

Also: the onset sentence beside spec 015's speed onset (the logged position + the blend's remainder + the deferred wait + `playback_latency_samples()`); the end-of-recording cut; the blend length as a constant beside `VOLUME_TRANSITION_MS` if published. The README needs no change (product M3).

## CHANGELOG 0.19.0

Under **Changed** (Keep a Changelog: a change in existing functionality; product M6), the lead: "**The speed-change crossfade** (spec 019, DATE; version 0.19.0; **an audio change at level changes only, not an assumption change**: every constant level is bit-identical to 0.18; A-054 amended in wording, the device's behavior at a change still open). Adds `speed::CROSSFADE_SAMPLES` (120) and `CROSSFADE_MS` / `RR_DR60_SPEED_CROSSFADE_MS` (20)." Then the crossfade, the wait rule (schedules with requests under 20 ms apart play longer), the three re-pins (the speed golden's schedule entries, the changes golden's `speed_inside` entries with their moved counts, the C smoke's speed case; every constant-level entry and every other guard unmoved), the figures (the click reduction per row under the 40 ms reader with the floor, the 0.5 s figures, the voiced energies, the worst end cut, the cost A/B at levels 3 and 5), "an engine convention, modeled on no document".

## Registers

A-054: the amendment as a hypothesis (the wording above, with "engine convention" and "capture P1" pinned by the registers cross-check; P1's new capture: a sustained 2.3 kHz tone and a vowel with the dial turned, 48 kHz, read for a step, a fade or a gap and for 20 ms alignment); the issue template's P1 line likewise; stage 9's manifest **description** ("…at one of five playback speeds (level 3 is normal); a level change blends over 20 ms, an engine convention") and its coverage note rewritten whole within 120 characters keeping the pinned tokens ("the device's own decoder (frame length, look-ahead, codebooks); its speed method and the sound of a level change (A-054)"); the manifest canonical text, README table and guide blocks re-blessed once.

## Product notes (product review M1 – M5, ready to paste)

**controls-discussion.md § 2b, row "Playback speed level", Notes column** (replace "A change to or from levels 4 – 5 can tick on content above about 1.6 kHz (spec 013 B2)."):
> A level change is a 20 ms crossfade of the old and the new level, the engine's convention (spec 019, engine 0.19.0; the device's own behavior at a change is open, capture P1); a constant level is bit-identical to 0.12.

**§ 2 candidate table, row "Crossfade on a speed-level change", Status:**
> Built by spec 019 (engine 0.19.0): a 20 ms smoothstep crossfade, always on, an engine convention; a request during a blend lands at its end.

**§ 2b row "Speed crossfade"**: `Built (spec 019); no setting, no state kept across playbacks.`

**§ 4 item 6:**
> *Built (engine 0.19.0, DATE): a level change crossfades over 20 ms, the engine's convention; a constant level stays bit-identical; a request during a blend lands when it ends. The step-vs-blend figures live in the harness, so the Workbench needs no switch.*

**decisions.md row:**
> | DATE | Spec 019's speed-change crossfade is **always on** with no setting and no control-manifest entry: the tick it removes was the emulator's own instant-switch convention, not a device behavior, so an A/B between two conventions has no Workbench value; the app steps levels one change per gesture (no slider drag) and reads the effective level from the last log entry of a run. The crossfade is an engine convention (A-054 amended); the device's behavior at a change waits for capture P1. Free tier. | an emulator option off by default; completing a running blend at once on a new request; the same-place blend (aliases) | spec 019 D1, D4, D7; spec 013 B2 | low (an option is one bool and a golden re-bless) |

**investigation-mode.md § 5, after the 018 bullet** (product M5):
> - **019 (built, engine 0.19.0):** a playback speed-level change is a 20 ms crossfade of the old and the new level, the engine's convention (A-054 amended; the device's own behavior at a change is open, capture P1); a constant level is bit-identical; a request during a blend lands when it ends and every request is logged at its position. The speed card describes a constant level and is unchanged; the timeline marks a change at its logged position.

**brief.md, the engine-facts list's speed line:**
> - **The playback speed:** five levels, pitch moving with speed (since 0.13.0, spec 013); a level change blends over 20 ms, an engine convention (since 0.19.0, spec 019).

**monetization.md line 14, the stale parenthetical:** *(SLOW/FAST is modeled since 0.13.0, spec 013; the 10-second review is not.)*

**CLAUDE.md**: the status paragraph and the "Spec 019 reports" commands.

investigation-mode.md's speed card describes a constant level and stays as is.

## Owner question the product review raised (deferred to the app)

Slider or stepper for the app's speed control? Default: stepper (the device's button).
