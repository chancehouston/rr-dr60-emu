# Controls and AI control: a decision brief

A product note on two requests made on 2026-10-15:

- give the app's users more to "play" with;
- let an AI drive the engine's controls.

Prepared 2026-10-18 against engine **0.13.0** on `main` (commit 77d9fa5). Every engine fact below was read from `crates/rr_dr60/src/settings.rs`, `pipeline.rs`, `transport.rs`, the C header, the stage manifest's canonical text and the assumption register on that date. Store-policy text was checked on the same date; links are at the end.

This is a decision brief, not a spec. It creates no engineering work. Each engine item in section 4 becomes work only through `/speckit-specify`. It follows the claims policy in `CLAUDE.md`: it describes what each control does to sound, says "modeled on" until a stage is checked against a real unit, and makes no claim about what recordings contain, either way.

Related notes: [brief.md](brief.md) (O-1, O-9, O-13), [front-and-inside.md](front-and-inside.md), [monetization.md](monetization.md), [decisions.md](decisions.md).

## Summary

- **The engine has 3 live controls and 26 construction-time settings** (counting each field, the tap and the seed). It also has a long list of fixed design constants.
- **Most of what a faithful app needs is "between recordings", not "live".** On the device, the sensitivity, the memory and the stage values are not changed mid-recording. Each recording starts from the path's reset state (A-037), so `reconfigure` between recordings costs no fidelity. What it costs is engineering: it may allocate, it is not real-time safe, and its time on a phone is unmeasured.
- **Four kinds of parameter**, proposed as the engine's own vocabulary:

  | Kind | Meaning |
  |---|---|
  | Device control | A control the owner's manual gives the user. |
  | Unit condition | A physical state of one unit, such as tired batteries or a noisier capsule. |
  | Model internal | A value the device has but the user cannot reach. |
  | Emulator option | Something the device cannot do. |

  The first two would answer O-1 (the "intended user adjustments") by definition.
- **Correction to brief.md O-1:** voice activation on or off is **not** a device control. The device has no VAS switch (S-001, A-008). It is an emulator option.
- **Live controls break the partition guarantee today.** Volume, route and speed changes are applied at a processing-call boundary. Output is partition-independent only when every partition has a boundary at the change (spec 011 US1 AS2). An AI script, a session replay or an investigator's "what was applied when" needs **sample-positioned changes and a change log**. That is the first engine gap for AI control, ahead of any new control.
- **The safe AI surface is structured, not conversational.** The AI chooses values from a machine-readable control manifest and named presets. The engine validates them. The explanation comes from the manifest's own neutral strings. The AI never receives audio, never writes free text about a recording, and never drives a parameter from a phone sensor.
- **Deferral of O-1 – O-6:** in my judgement, the condition is now met for the signal chain. 12 stages are modeled and the other 3 (6, 14, 15) are deliberately not modeled after research. The next engine features cannot be scoped without O-1, O-3 and O-4, so I recommend reopening those three now and keeping O-2, O-5 and O-6 deferred until a phone prototype.

## 1. Inventory of every engine parameter

> **Superseded as the current inventory (2026-10-29):** the engine's control manifest (spec 014, `rr_dr60::controls()`, the guide's "The controls the engine accepts") is the checked, current list of every control with its kind, timing, range, default and IDs. The tables below are the 2026-10-18 brief that fed it.

Timing classes used below:

| Class | Meaning |
|---|---|
| **Live** | Changes while the engine runs, real-time safe, no reset. |
| **Idle (reconfigure)** | Applied by `reconfigure`. It is refused while recording or playing, resets every path's state, keeps the memory and the route, and may allocate. |
| **Fixed** | A constant in the code. Not exposed. |

For "Kind", see the table in the summary. "Perceived" is what a listener notices on default material (speech at about −20 dBFS through the speaker route at the reference volume), as the docs and the harness figures describe it. "Perceived" is judgement where no figure exists.

### 1a. Live controls (3)

| Control | API | Kind | Range / default | Applies to | What a user perceives | Notes |
|---|---|---|---|---|---|---|
| Output route | `set_route` | Device control (the earphone jack, S-001) | Speaker / Earphone; Speaker | Playback and one-pass | Large. The speaker route has the small-speaker resonance near 750 Hz and the amplifier's 159 Hz corner. The earphone route is flatter and about 18.6 dB quieter at the reference position (A-042). | Takes the volume at once on change. Survives `reset` and `reconfigure`. |
| Volume position | `set_volume` | Device control | 0 – 1; 0.1335 (`VOLUME_REFERENCE`, A-043) | Playback and one-pass | Loudness. Above the reference position, full-scale material clips on the speaker route: 10 % THD at +2.3 dB over (A-031). | 20 ms smoothstep transition (A-045). A change during a transition restarts from rest, so a stream of updates lags: about 19 ms at 60 Hz, about 105 ms at one update per 64-sample block (O-13). |
| Playback speed level | `set_playback_speed` | Device control (SPEED CONTROL, S-001) | 1 – 5 = 0.6 / 0.8 / 1.0 / 1.2 / 1.4; 3 | `play` only | Tempo and pitch together (mechanism (b), A-054, low confidence; the research leans weakly toward a device that keeps pitch). A change to or from levels 4 – 5 can tick on content above about 1.6 kHz (spec 013 B2). | Level 3 is bit-identical to 0.12. Kept by `reset` and `reconfigure`. |

### 1b. Construction-time settings (`Settings`, via `new` / `reconfigure`)

| Setting | Kind | Range / default | Stage | What a user perceives when changing it |
|---|---|---|---|---|
| `vas.sensitivity` | **Device control** (MIC SENS, set in the MODE SELECT position, S-001, A-021) | 1 – 5; 3 | 5 | How readily recording pauses: 3 dB of threshold per level (A-022). Whether the device also changes a digital gain is open (A-021). |
| `vas.enabled` | Emulator option (no VAS switch on the device, S-001, A-008) | bool; on | 5 | Large. With it off, silences stay in the recording and no splices occur. This is the device's defining behaviour switched off. |
| `vas.mode` | Emulator option | Drop / Mute; Drop | 5 | Drop removes paused audio, so the recording is shorter. Mute keeps the timeline and inserts digital silence. Investigation sessions may want Mute, or Drop with splice events, for one elapsed timeline. |
| `vas.threshold_dbfs` | Model internal | −60 – 0; −40 (A-022) | 5 | The same effect as sensitivity, in finer steps. |
| `vas.hang_ms` | Model internal | 50 – 10 000; 1000 (A-023) | 5 | How long a silence lasts before recording pauses, so how much pause is kept between phrases. |
| `vas.onset_ms` | Model internal | 0 – 200; 20 (A-024) | 5 | How much of the first sound after a pause is lost (clipped word starts). |
| `agc.enabled` | Emulator option (A-020: always active) | bool; on | 3 | Loud onsets and sustained loud sounds are no longer held below full scale; they clip at the ADC clamp instead. |
| `agc.knee_dbfs` | Model internal | −40 – 3; −1 (A-028) | 3 | At −1 only loud sounds are limited. Lowered, ordinary speech is held at a ceiling and level "pumps" after loud sounds. Very audible. |
| `agc.attack_ms` | Model internal | 1 – 50; 3 (A-034) | 3 | How long a loud onset passes at full level, and so clips, before the limiter acts. Subtle. |
| `agc.release_ms` | Model internal | 200 – 10 000; 1370 (A-035) | 3 | How long the level stays down after a loud sound. Audible as ducking after a bang. |
| `mic.capsule_enabled` | Model internal as a bypass (an emulator option when off) | bool; on | 1 | Removes the capsule's hiss (about −62 dBFS, A-048). Audible in quiet passages and with the volume up. |
| `mic.hiss_offset_db` | **Unit condition** (capsule-to-capsule spread; the research reports unit-to-unit differences in hiss and VAS behaviour) | −18 – +12 dB; 0 | 1 | More or less hiss. From about +9 dB, at sensitivity 5, voice activation stops pausing in a silent room (spec 012). |
| `mic.amp_enabled` | Emulator option when off | bool; on | 2 | Large. With it off, the thin low end (input high-pass near 818 Hz, A-046) and the asymmetric clipping on loud input (A-051) disappear. |
| `record_stage_enabled` | Emulator option when off | bool; on | 4 | With it off, the 225 – 2550 Hz record band opens toward the 3 kHz device Nyquist. Audible. |
| `adc_clamp_enabled` | Emulator option when off | bool; on | 4 | With it off, loud onsets exceed full scale instead of flat-topping. Subtle. |
| `speech_codec_enabled` | Emulator option when off | bool; on | 7, 9 | Large. With it off, the 4 kbit/s CELP-class artifacts disappear. Changing it needs an empty memory, and it changes the latency by 180 device samples. **Never live.** |
| `playback_stage_enabled` | Emulator option when off | bool; on | 10 | With it off, the reconstruction filter is removed. Subtle. |
| `output.volume_amp_enabled` | Emulator option when off | bool; on | 11 | With it off, there is no volume, amplifier gain or clipping (unity at codec scale). |
| `output.speaker_enabled` | Emulator option when off | bool; on | 12 | Large. With it off, the small-speaker template (A-010, low confidence) is removed. |
| `output.earphone_enabled` | Emulator option when off | bool; on | 13 | With it off, the earphone corners at 120 / 280 Hz are removed. Subtle. |
| `output.clamp_enabled` | Emulator option | bool; **off** | after 13 | Only output above ±1.0 changes (flat tops). A host convenience; it adds distortion the device does not have. |
| `memory.capacity_s` | Device limit (S-001: 60 min); above it an emulator option | 1 – 86 400; 3600 | 8 | Noticed only when the memory fills and recording stops. A tier question (O-2). |
| `memory.max_recordings` | Device limit (S-001: 99); above it an emulator option | 1 – 9 999; 99 | 8 | Same as `memory.capacity_s`. |
| `tap` (11 values) | Emulator option | `RouteOutput` default | any | Which stage's output the one-pass returns. **The recorder and `play` ignore it**: `play` always returns the route output. A live "listen here" during playback needs the scope copy (E-B), not the tap. |
| `seed` | Plumbing | u64; 0 | 1 | A different hiss realization. Not perceptible as a character change; it matters for reproducibility. Not a user control. |
| `host_rate_hz` | Plumbing | 7 rates | all | Nothing at 16 kHz and above. At 6 and 8 kHz, the clipped speaker route aliases (spec 009). Fixed by the app's audio session. |

### 1c. Fixed (design constants, not exposed)

| Constant | Basis | Becomes a control? |
|---|---|---|
| Device rate 6 kHz | A-026, sourced | No. The clock offset was struck by the owner on 2026-10-17. |
| Codec filter templates (record band, playback low-pass) | A-027 | No. They are the device. |
| Speech coder (20 ms frames, 80 bits, 30 ms delay) | A-004, A-040, A-041 | No |
| Limiter component values and detector polarity | A-028, A-036 | No. The knee, attack and release are already exposed. |
| Mic amp corners (818 / 77 Hz / 12.9 kHz) and clip limits | A-046, A-047, A-051, A-053 | No. Only as a future "unit condition" if captures show spread. |
| VAS step of 3 dB per sensitivity level | A-022 | No |
| Speaker template (750 Hz, Q 1.8, +4.5 dB shelf); earphone corners | A-010, A-032 | No |
| Amplifier clip ceiling 1.15 V; gain law | A-031, A-044 | **Yes, candidate**: battery state scales the ceiling (A-056). |
| Volume transition 20 ms, smoothstep | A-045 (an engine convention) | No. A programmed fade is a separate candidate (section 4). |
| Speed ratios and mechanism (b) | A-054 | **Yes, candidate**: a pitch-kept mode (c1). |
| Recording counter (hiss stream key) | A-049 | State, not a control |

### 1d. Not modeled, with a control attached

| Candidate | Basis | Status |
|---|---|---|
| Battery state on the speaker route | A-056 (static law: ceiling ∝ V+ − 1.6 V; −0.3 dB fresh to −9.1 dB at 10 h; plus 0.9 – 2.5 dB sag on loud passages) | The owner put it into this discussion on 2026-10-17. Audible only on material that reaches the ceiling: at the reference volume, −20 dBFS speech keeps about 11 dB of headroom even at 10 h. |
| Pitch-kept slow/fast (mechanism c1) | A-054 (the research leans weakly toward it), S-018 | Follow-up to spec 013. |
| Crossfade on a speed-level change | spec 013 B2 (the step at 2 kHz to or from level 5 is −0.7 … −1.9 dB re the tone) | "Revisit after capture P1 or an app request." |
| 10 SEC REVIEW and seek | S-002 control S11 | Not modeled. `play` has no seek. |
| Start-of-item transients | A-057 | Waits for capture P3 (owner, 2026-10-17). |
| Beeps | S-002 (R19/C16) | Out of scope |

## 2. What the app would want live, and the engine work

### 2a. Ranking by user value

User value is my judgement from the personas (personas.md) and the dossier. No interview or review mining has been done on controls.

| Rank | Control | Who notices | Tier (recommended) | Timing the app needs |
|---|---|---|---|---|
| 1 | Volume, route, speed | Everyone | Free (device controls) | Live: **already built** |
| 2 | Sensitivity 1 – 5 | Group-hunt users and investigators. It changes when recording pauses, the behaviour they describe. | Free (device control) | **Between recordings** is faithful (it is set in a menu on the device). Live during recording: device unknown. |
| 3 | Per-stage meters (not a control; E-A) | Everyone who flips | Free | Per block. Without them the inside cannot move honestly. |
| 4 | Hiss offset, battery state ("unit condition") | Recorder-mode users, creators | Free (recommended: a unit condition is a device fact, not a model knob) | Live is pleasant; between recordings is enough for the hiss offset (it is a record-path value). The battery is playback-side and can be live. |
| 5 | Limiter knee, release, attack; VAS threshold, hang, onset | Recorder-mode enthusiasts (a small, loud minority, personas job 5) | Workbench | Between recordings for recordings. Live in a monitoring toy (one-pass). |
| 6 | Stage bypass and A/B ("with / without the speaker") | Enthusiasts, creators | Workbench | Playback-side stages: live with a crossfade. Record-side stages: an offline re-render, not live. |
| 7 | Pitch-kept slow playback | Investigators reviewing recordings | Free if a capture shows the device keeps pitch (then it is the device); otherwise an emulator option, Workbench | Live during playback |
| 8 | Expanded memory | Heavy users | O-2 | Idle |

Never live: the speech codec on/off (it fixes the memory's encoding and the latency), the memory capacity, the host rate, the seed, and any bypass of a stage that carries latency (stage 4 and the rate conversion change `latency_samples`).

### 2b. Engine work, control by control

The common pattern is spec 011's: a setter that is validated, real-time safe and read at a block boundary; per-sample smoothing where a step would click; the test oracle "a change at sample n equals the old constant run before n and the new constant run's steady state after the transition".

| Control | Path | Per block or per sample | State and reset | Determinism | Size |
|---|---|---|---|---|---|
| Sensitivity, threshold, hang, onset, live | Record path and one-pass | The threshold is compared per device sample, so a step is click-free (it moves a decision, not a gain). The hang and onset counters take the new value at the next decision. | Kept across the change. Does `reset` keep or restore the setting? Recommend: a live value lives in `Settings`, and `settings()` reflects it. | Needs sample-positioned changes (2c) | S |
| Hiss offset, live | Record path and one-pass, host rate | A per-sample gain with a 20 ms ramp (a stepped noise level is audible) | The hiss stream is unchanged; only its gain moves | Same | S |
| Limiter knee, attack, release, live | Record path and one-pass, host rate | The derived coefficients are scalars recomputed at the block boundary. The capacitor's state carries over, which is physical: a resistor changed while the circuit runs. | Carried | Same | S – M |
| Battery state (A-056, static law), live | Playback and one-pass, stage 11 | The ceiling scales per block with a 20 ms ramp. At the nominal value it is bit-identical to 0.13. | None | Same | S – M (design script, harness rows) |
| Speed crossfade | `play` | Both levels run for the fade, which doubles the speed stage's work for that time | None | Constant level unchanged; a change has a new oracle (step energy) | S |
| Playback-stage bypass, live (10, 11, 12, 13) | Playback | Both branches computed for a 20 ms crossfade. The filters are minimum-phase with no reported latency, so the branches align. | Each bypassed branch keeps running to stay warm, or restarts from rest under the crossfade | Same | M |
| Record-side "with / without" | Offline | A second pipeline renders the raw capture again with one setting changed. The engine already guarantees this is deterministic. | None | Given | None in the engine. App work, plus O-3 (the raw capture must be kept). Derived cost: about 0.5 min per hour of audio on the development machine (from 1.23 µs per device sample at 48 kHz, spec 008). **Not measured on a phone.** |
| Between-recordings apply | Idle | A real-time-safe apply of the settings that change no allocation (everything except the host rate, the memory capacity and the codec) | Path state resets anyway (A-037) | Bit-identical to `reconfigure` | S. Or first measure `reconfigure` on an iPhone; if it is a few ms, document it and skip the spec. |
| Pitch-kept speed (c1) | `play` | A time-scale modification at 6 kHz in the decoder domain, deterministic | A new mode setting | Constant-level goldens | L |

### 2c. Two engine gaps that matter more than any one control

*Status at engine 0.16.0 (spec 015, 2026-10-30, PR pending): gap 1 is built for the three live controls (the route and the volume on the playback and the one-pass streams, the playback speed on the playback stream), and gap 2's change log is built in the voice-activation events' caller-buffer form: each entry carries the output-sample position the change landed on, the control, the values before and after, and a caller tag (the guide's "Scheduled changes and the change log" section). The record-side values come through the same form in spec 017 (the record stream is reserved for them); the settings snapshot and the session sidecar are spec 020's. Positioning a change on a sample is an emulator convention, not a device behavior: the control manifest's timing classes are unchanged.*

1. **Sample-positioned changes.** Today a change takes effect at the next call boundary: "the controls are read once per call, at the block boundary" (spec 009 FR-009; spec 013 FR-001 is similar for speed). So the same change gives different output under different audio buffer sizes. iOS can change the I/O buffer size under the app, for example on a route change. A control event carrying a sample offset within the block, as audio plug-in hosts do, makes every live change partition-independent. Replays, AI scripts and session logs then reproduce bit for bit. It also removes most of the volume-retarget question (O-13): a scripted fade becomes one event, not a stream.
2. **A change log and a settings snapshot.** The dossier requires "the active chain and settings are saved, and changes during a session are timestamped". Today:
   - the stored form (version 2) records the encoding, but neither the settings nor the controls;
   - the engine reports VAS splices into caller memory, but reports no control changes.

   An engine-written log in the same caller-buffer form as `VasEvent` (sample position, control id, old and new value), plus a versioned settings serialization, gives the app the session contract. It also lets the app show, from the engine's own log, which changes a preset or a script applied and where.

**Threading:** a handle must not be used by two threads at once (C header, rule 4). A slider or an AI runs on another thread than the audio callback, so the app needs a lock-free queue into the callback. Recommend this lives in the Swift package, not in the `no_std` core, and that the core's control event (gap 1) is what the queue carries.

### 2d. Interaction with the record and playback split

- **Record-path controls change what is stored**, permanently. Playback-path controls (route, volume, speed, battery, playback bypasses) change only what is heard. The inside and the session log should show which side a control is on. The control manifest's "path" field (section 3) carries it.
- **The speaker is muted while recording** (S-002). A live monitoring toy in which a user speaks and turns record-side knobs is the one-pass with earphones. It is not a device behaviour. The 2026-10-11 decision (decisions.md) makes live monitoring a labelled Workbench extra; it stands.
- **The tap does not reach `play`.** Every "listen at this stage" during playback needs the scope copy (E-B, still unbuilt).

## 3. Options for an AI-facing control surface

"AI control" can mean three different things. They have different costs and risks:

| Mode | Example | Where it lives | Store and privacy exposure |
|---|---|---|---|
| A. An in-app assistant that turns a request into settings | "Make it sound like a unit with tired batteries" sets the battery and the hiss | App, calling the engine through the control manifest | On-device: no new data collection. Cloud: personal data to third-party AI (below). |
| B. System automation | Shortcuts or Siri through App Intents: "start a recording at sensitivity 4" | App (App Intents) | Low. Each intent is a declared action. |
| C. Developer and agent tooling | An AI coding agent or a script renders files through the library with a preset | Library, CLI, docs | None for the app. Useful to library customers. |

### 3a. Building blocks

| Option | What it is | Pros | Cons |
|---|---|---|---|
| **1. Control manifest** (beside the stage manifest) | Constant data, one entry per setting and live control: stable id, display name, kind (device control / unit condition / model internal / emulator option), timing class (live / idle / construction), path (record / playback / both), type and unit, range, step, device default, stage number, S- and A-IDs, smoothing, and one neutral sentence on what it does to sound. C access like `rr_dr60_manifest_*`. | One source of truth for the app's UI, the inside's cards, the docs and any AI's tool list. Cross-checkable against `validate` (every range boundary probed) and the stage manifest's `controls` notes, the spec 007 pattern. Answers the dossier's engine question 1 and O-1's raw material. Cheap: data only. | Another versioned public surface. The strings must pass the denylist. A range change becomes a semver event (which is right). |
| **2. JSON Schema** of the controls | Generated from option 1 by the harness as a checked-in, golden-guarded file. Not in the core, which is `no_std`. | LLM tool-calling APIs take JSON Schema directly. Apple's on-device framework uses typed Swift (`@Generable`), which the Swift package can generate from the same data. | Two renderings to keep in step. Mitigated by generating both from option 1. |
| **3. Named presets** | A short catalogue of `Settings` plus control values, each with an id, a neutral name and the kinds it touches. The engine carries only device-grounded ones ("device as modeled", "fresh / half / flat batteries" once A-056 is built, "quiet / noisy capsule"). Creative presets live in the app or the packages. | The safest AI target: the AI picks one of N, so it cannot invent a configuration. Good for the free tier and for screenshots. | Preset names are the riskiest strings in the product (below). Engine presets must trace to the registers like everything else. |
| **4. Scheduled control events and a change log** (2c) | Sample-positioned changes in, a change log out | Makes AI changes reproducible and auditable. A script is a list of events. | Engine work (spec 015 in section 4) |
| **5. Settings serialization** | A versioned text form of `Settings` plus the live controls | Sessions, sharing a configuration, AI output that a person can read | A third versioned format next to the stored form and the manifest |
| **6. Free-text assistant** | A chat model that sees the conversation and maybe the audio | Flexible | Claims exposure (it can say anything about a recording), privacy exposure, store review exposure. **Not recommended.** |

**Recommended:** 1 + 3 + 4 first, with 2 generated from 1 and 5 as part of the session contract. The AI's output is a structured change: a preset id, or (control id, value) pairs. The engine validates every value; that is already true of every setter. The app's explanation is assembled from the control manifest's sentences, never generated.

### 3b. Guardrails

Proposed rules for every AI or automation path:

1. **Only declared controls, only declared ranges.** The AI's tool list is the control manifest, filtered by tier and by timing class (an AI cannot request an idle-only setting during a recording).
2. **Rate.** At most one change per control per UI frame. Programmed fades go through a single scheduled event, not a stream. The volume-restart lag (O-13) otherwise grows to about 105 ms.
3. **Hearing.** An AI may not raise the volume above the reference position (A-043) without a person's action. The output clamp is a host option, not a safety limiter.
4. **Visible and reversible.** Every AI change appears in the session log as "set by the assistant", and the inside's "device default" button (decisions.md, 2026-10-11) restores it.
5. **No audio to the AI.** The AI sees the control manifest, the current values and the user's request; never the microphone audio, the recordings or the sensor tracks. That keeps App Privacy and Data safety at "data not collected" (the standing decision) for an on-device model. A cloud model that receives personal data triggers App Store guideline 5.1.2(i) (quoted in 3c).
6. **No sensor-driven parameters.** No mapping from a phone sensor (magnetometer, motion, pressure) to any engine parameter, by an AI or otherwise. Sensors are recorded beside the audio, never fed into it. A competitor markets an "interference modulator" from device sensors (competitors.md, GhostTube EVP, checked 2026-10-11). A sensor-driven sound change produces audio that looks like a response to the environment, which is exactly the claim this project refuses to make.
7. **No search for speech-like output.** Never build an optimiser, AI or otherwise, that tunes parameters to make noise or hiss sound more like words, or that ranks recordings by "activity". It would manufacture the content a listener then hears.
8. **No interpretation.** The AI does not transcribe, label or describe what a recording contains. No "voice detected", "anomaly", "response".

### 3c. Store policy that applies (checked 2026-10-18)

- **App Store Review Guideline 5.1.2(i):** "You must clearly disclose where personal data will be shared with third parties, including with third-party AI, and obtain explicit permission before doing so." A cloud assistant that receives a recording or a transcript needs an in-app disclosure and explicit consent, and it changes the privacy label. Secondary coverage (stora.sh, igeeksblog; not authoritative) says it is unclear whether on-device models are in scope. An on-device model with no audio input sidesteps the question.
- **Guideline 2.5.2:** apps may not "download, install, or execute code which introduces or changes features or functionality". Presets and chains, whether delivered later, generated by an AI or sold in packages, must be **declarative data validated against the control manifest**, never scripts.
- **Guideline 2.3.1(a):** misleading marketing "is grounds for removal". An "AI" label in the store listing invites scrutiny. Describe what the assistant does ("sets the recorder's controls from a request"), never what it might find.
- **Guideline 1.2** names "AI content creation apps" among user-generated-content apps. Sharing AI-made presets publicly would bring the UGC duties (reporting, blocking). Keep sharing out of scope for now.
- **Google Play, AI-Generated Content policy** (Play Console Help, read 2026-10-18): it covers text-to-text chatbots where "the chatbot interaction is a central feature", image generators, and voice or video of real people. It excludes "productivity apps that use AI to improve an existing feature". A settings assistant is plausibly in the exclusion, which is my reading, not a ruling. A chat-first assistant would not be.
- **Apple Foundation Models** (on-device, iOS 26, guided generation into typed Swift values and tool calling): this matches the structured-only design. **Source is third-party write-ups, not Apple's documentation.** Availability depends on the device and Apple Intelligence settings, so the assistant must be optional.

### 3d. What must never be exposed as a claim

These words apply to control names, preset names, AI prompts, AI tool descriptions and store copy. They extend monetization.md › Copy rules.

| Context | Allowed (describes the signal) | Never |
|---|---|---|
| Preset names | "Device as modeled", "Tired batteries (2.4 V)", "Noisy capsule (+6 dB hiss)", "Codec only", "No voice activation (emulator)" | "EVP mode", "Spirit", "Ghost", "Paranormal", "Investigation boost", "Clear voices", "Enhanced", "Sensitive to activity" |
| Control descriptions | "Moves the voice-activation threshold 3 dB per level" | "Picks up more", "listens for", "detects" |
| AI tool description | "Sets the recorder's controls within their ranges" | "Finds the best settings for capturing…" |
| The battery control | "Battery voltage (V) / hours of use; lowers the speaker's clip ceiling" | Any link between battery level and events, presence or "drain". Battery drain is a well-known trope in this category. The control is set by the user only, never by a sensor, a timer or the phone's own battery. |
| Speed | "Plays at 0.6× with the pitch lowered" / "kept" | "Slow down to reveal hidden voices" |

## 4. Recommended sequence of engine specs

Each item below is small and independently testable, with the WHY and the tier it serves. The numbers assume nothing else is queued; the queued earphone cut-off (A-032) stays capture-gated. Each block is a candidate input for `/speckit-specify`.

1. **014: Control manifest** (S, data only; serves free, Workbench, library and AI).
   - *As an app developer and as an AI tool builder, I want the engine to describe every setting and live control (id, kind, timing class, path, unit, range, step, device default, stage, S- and A-IDs, a neutral sentence), so that the app's controls, the inside's cards, the free/paid line and an AI's tool list come from one cross-checked source and cannot drift from what the engine validates.*
   - Oracle: every range boundary equals `validate`'s acceptance boundary. Every control appears in its stage's `controls` note. The denylist passes.
   - Includes a generated JSON Schema file (harness, golden-guarded). The "kind" field needs Q2 below.
2. **015: Sample-positioned control changes and the change log** (M; serves sessions, replay and AI).
   - *As an app, I want to schedule a route, volume or speed change at a sample offset within a block and to receive a log of every applied change, so that the same session reproduces bit for bit at any buffer size and a listener can see what changed when.*
   - Oracle: partition independence for arbitrary change positions; the 011 and 013 splice identities.
   - Optionally includes a programmed volume fade (`over N ms`, an emulator option), which settles O-13 for automation.
3. **016: Per-block meters (E-A)** (M; free; not a control, but the inside cannot show any control's effect honestly without it).
4. **017: Live record-path values** (S – M; free for sensitivity, Workbench for the model internals).
   - Sensitivity, threshold, hang, onset, hiss offset (with a ramp), and the limiter's knee, attack and release become live in the recorder and the one-pass, through 015's events.
   - Oracle: a change at sample 0 equals the constant configuration; partition independence; the hiss-offset ramp energy.
   - If Q3 is answered "between recordings only", this shrinks to a real-time-safe idle apply, or to a measurement of `reconfigure` on an iPhone.
5. **018: Battery state, static law (A-056)** (S – M; a unit condition, free).
   - *As a user, I want to set the unit's battery voltage and hear the speaker route's clip ceiling follow it, so that the recorder can be played as a unit with fresh or tired cells.*
   - Off (nominal 2.8 V V+) by default, bit-identical to 0.13. Live, ramped. The engine only; the app must never drive it from the phone's battery.
   - Envelope sag: a later spec, only if wanted.
6. **019: Speed-change crossfade** (S; free polish).
   - Removes the tick on a level change to or from levels 4 – 5. A constant level stays bit-identical.
   - It could ride with 018 if each keeps its own oracle.
7. **020: Session contract** (M; Investigation mode). Depends on O-3.
   - A versioned settings serialization and the per-recording change log exported beside the stored form (a sidecar, so the device's own memory model stays the device's).
8. **021: Live playback-stage bypass with a crossfade** (M; Workbench A/B for stages 10 – 13).
9. **Later, gated:**
   - pitch-kept speed (c1), L, after capture P1 or an app request;
   - playback seek and 10 SEC REVIEW, S – M, a device feature, free;
   - start-of-item transients, after P3;
   - the clip indicator (spec 010's review), if the inside's design wants it; it fits inside 016.

Critical path for "AI can drive the engine": 014 → 015. For "more to play with": 014 → 017 → 018. The meters (016) can run in parallel with either.

## 5. Has engine core development reached the point where O-1 – O-6 reopen?

**Judgement: yes for the signal chain, as of 0.13.0.**

- The manifest reads 12 assumed, 3 not modeled.
- The 3 unmodeled stages (6 noise reduction, 14 clock, 15 power) were researched on 2026-10-17. The owner decided:
  - not to model stage 6;
  - to strike the clock offset;
  - to wait for capture P3 for the transients;
  - to move the battery state into this discussion.
- Every remaining modeling item is evidence-gated (captures P1, P3, P5, the earphone cut-off) rather than engineering-gated.

What remains is the **app-facing capability surface**: the control manifest, events, meters and the session contract. Its scope depends on product answers: which controls are "intended user adjustments" (O-1), whether raw audio is kept (O-3, which decides 020 and the record-side A/B), and what ships first (O-4, which orders section 4). Deferring those while speccing 014 – 020 means guessing at them in the specs.

Recommendation:

- **Reopen O-1, O-3 and O-4 now.**
- **Keep O-2** (paid shape), **O-5** (default mode) **and O-6** (subscription) deferred until a phone prototype exists. None of them changes an engine contract.

## 6. Decisions for the owner

**Answered 2026-10-29** (rows of that date in [decisions.md](decisions.md)): 1 settled by the decision review's round 2 (O-3 decided, engine first); 2 yes; 3 yes; **4 no AI control for now**; **5 the order 014 → 016 → 015 → 017 → 018 → 019 → 020 → 021**; 6 yes; **7 pitch-kept speed added as an option**, default unchanged. The questions stay below as asked.

Each question gives the recommended answer first.

1. **Is engine core development complete enough to reopen product questions?** Recommended: yes for O-1, O-3 and O-4; keep O-2, O-5 and O-6 deferred until a prototype. Alternative: keep all deferred until 014 – 016 ship, at the risk of specs that encode guesses. Reversal cost: low.
2. **Adopt the four kinds** (device control, unit condition, model internal, emulator option), with "intended user adjustments" (O-1, free) defined as device controls plus unit conditions? Recommended: yes.
   - Model internals and live playback bypasses then sit in the Workbench, whatever its commercial form under O-2.
   - Alternative: every adjustment free, with paid limited to the component packages.
   - Supports the alternative: the engine is MIT, so a competitor's wrapper can give the knobs away free.
   - Supports the recommendation: the internals are this product's unique content, while EQ and reverb packages are commodity.
   - Reversal cost: moving a free feature behind a purchase later is costly (decisions.md, 2026-10-15).
3. **Record-side timing:** "between recordings" first, as faithful and cheapest; live record-side values only through 015's events (017), and in the one-pass monitoring toy as a labelled Workbench extra? Recommended: yes. Never live: the codec, the memory, the host rate, and latency-bearing bypasses.
4. **AI surface:** structured only, meaning the control manifest, presets and scheduled events, with the engine validating every value. On-device model only. No audio, recordings or sensor data to the AI. No free-text interpretation of recordings. No sensor-driven parameters. No speech-likeness optimisation. Recommended: yes to all, written into decisions.md as standing rules. Alternative: a cloud chat assistant, which needs 5.1.2(i) consent, changes the privacy labels and carries open-ended claims risk.
5. **Order:** 014 control manifest → 015 positioned changes and log → (016 meters in parallel) → 017 live record values → 018 battery → 019 speed crossfade → 020 session contract → 021 playback bypass. Recommended: yes. Alternative: 016 meters first if the inside prototype is the next app milestone.
6. **Battery state (A-056):** the static law only, as a live unit condition, nominal by default, user-set only? Recommended: yes; envelope sag later if wanted.
7. **Pitch-kept speed (c1):** wait for capture P1 or an app request, and ship the crossfade (019) first? Recommended: yes. If P1 shows the device keeps pitch, c1 becomes the device default, a free device control, and a CHANGELOG-recorded assumption change.

## Sources (checked 2026-10-18)

- Apple, [App Store Review Guidelines](https://developer.apple.com/app-store/review/guidelines/): 1.2, 2.3, 2.3.1(a), 2.5.2, 2.5.19, 5.1.2(i), read 2026-10-18.
- Google, [Understanding Google Play's AI-Generated Content policy](https://support.google.com/googleplay/android-developer/answer/14094294), read 2026-10-18 (no date on the page).
- Secondary, on the 5.1.2(i) change of November 2025 and its scope: [igeeksblog](https://www.igeeksblog.com/apple-app-store-ai-data-disclosure-guidelines), [stora.sh](https://stora.sh/blog/2026-05-06-apple-ai-consent-rule-5-1-2-i-implementation-guide).
- Secondary, on Apple's Foundation Models framework (guided generation, tool calling, iOS 26, availability): [atelier-socle](https://www.atelier-socle.com/en/articles/foundation-models-api-guide), [infinum](https://infinum.com/blog/apple-on-device-ai/). Confirm against Apple's developer documentation before any spec relies on them.
- Engine facts: this repository at 77d9fa5 (`settings.rs`, `pipeline.rs`, `transport.rs`, `rr_dr60.h`, `golden/manifest-v1.txt`, `docs/hardware/assumptions.md`, `specs/011-volume-ramp/spec.md`, `specs/013-playback-speed/spec.md`, `docs/hardware/research/reports/Noise reduction clock and power.md`).
