# Records to write at implementation (019)

Wording rules: neutral; the crossfade is **the engine's convention** for the moment of a level change, never a device behavior (A-054 amended; the device's natural quantum is the 20 ms CELP frame, so a frame-boundary switch is the plausible device behavior; capture P1 decides); "behavior"; no "tick-free" promises beyond the measured figures; the harness reproduces every figure before a record quotes it (the reviewer's 44.7 – 53.8 dB is a simulation until then).

## Guide, docs/guide.md, the playback-speed section (replacing "A change is a step: … above about 1.6 kHz.")

> A change blends: for 20 ms (120 device samples) the old level fades out at its own place and pitch while the new level fades in from the change, a smoothstep crossfade, so a change to or from levels 4 – 5 no longer ticks on content above about 1.6 kHz; a request made during a blend lands when the blend ends, at most 20 ms later (plus at most four device samples for a change into level 3), and every request is logged at its own position with target levels. The blend is the engine's convention (A-054); what the device does at the moment of a change is open (capture P1).

Also: the onset sentence beside spec 015's speed onset (the logged position + the blend's remainder + the deferred wait + `playback_latency_samples()`); the end-of-recording cut; the blend length as a constant beside `VOLUME_TRANSITION_MS` if published. The README needs no change (product M3).

## CHANGELOG 0.19.0

A **convention change visible in schedule outputs, not a device behavior** (not an assumption change): the crossfade, N, the wait rule, the three re-pins (the speed golden's schedule entries, the changes golden's speed scenarios, the C smoke's speed case; every constant-level entry and every other guard unmoved), the figures (the click reduction per pair under the 40 ms reader, the 0.5 s figure, the voiced figure, the worst end-of-recording cut, the cost A/B), "modeled on nothing: an engine convention".

## Registers

A-054: the amendment (the convention; the frame quantum; P1's new capture: a sustained 2.3 kHz tone and a vowel with the dial turned, 48 kHz, read for a step, a fade or a gap and for 20 ms alignment); the issue template's P1 line likewise; stage 9's manifest `controls`/`coverage` notes ("a 20 ms blend at a level change (engine convention)"; "the change's own sound" not modeled); the manifest canonical text, README table and guide blocks re-blessed once.

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

investigation-mode.md's speed card describes a constant level and stays as is.

## Owner question the product review raised (deferred to the app)

Slider or stepper for the app's speed control? Default: stepper (the device's button).
