# The six app questions (O-1 – O-6): a decision brief

A brief for the owner on the six app questions that were deferred "until engine core development is complete" (brief.md › Open questions). That point came on 2026-11-01, when the engine was declared feature complete at 0.90.0 (now **0.91.0** on `main`). The brief is meant to be decided in one sitting: the summary table first, then one section per question, then the order to take the answers in and the first app spec they make possible.

- **Prepared:** 2026-11-01. Engine facts come from `main` at 0.91.0 (feec054): the control manifest's canonical text (`crates/rr_dr60_harness/golden/control-manifest-v1.txt`, 31 entries), `crates/rr_dr60_ffi/include/rr_dr60.h` and `crates/rr_dr60/src`. Spec 024 was read from branch `024-insertable-component` (held). The app facts come from `~/Code/ic-recorder-app` at c156d38 (`docs/bench-plan.md`, the bench's constitution).
- **Store policy** was checked on 2026-11-01. The excerpts and links are at the end, under Sources.
- **Confidence labels:** *evidenced* means store policy text or the project's own records; *judgement* is the product specialist's experience; *speculative* is a guess that should be tested.
- **Claims policy.** This brief follows `CLAUDE.md`. It describes what the signal chain does to sound, says "modeled on", and makes no statement about paranormal phenomena in either direction. Every piece of draft copy below passes the engine's wording denylist and the product rule.
- **No decision is recorded here.** The owner's answers go into decisions.md as dated rows.

## Summary

| Q | Question | Recommendation | Confidence | Unblocks |
|---|---|---|---|---|
| **O-4** | Release sequencing | **1.0 ships both modes complete and free, with no in-app purchase code at all.** The first paid equipment package comes in a later 1.x, after the component slot (spec 024) and after people are using the app. A TestFlight beta runs before 1.0. Spec 024 is resumed after the first app spec, and the band filter ships in 1.0 only if it is done before the beta freeze. | judgement (high) | The scope of every app spec. With no purchase code there is no 3.1.1 review surface, no restore UI and no paid labels in the metadata. |
| **O-1** | Which controls are free | **Every user-meaningful control is free, through progressive disclosure:** the front carries the device controls; the inside's cards carry the unit conditions, the model internals and the emulator options. Only the three host parameters (rate, seed, output clamp) stay hidden. None of the 31 is paid. Purchases sell components, never knobs. This revises the tier half of the 2026-10-29 four-kinds row; the kinds themselves stay. | judgement (medium-high) | The front's controls, the inside's cards and the preset format. It also resolves a conflict between two recorded decisions (see "Contradictions" below). |
| **O-2** | The paid boundary and its shape | **Paid = equipment packages only**: each one a non-consumable of engine components with example chains and presets, plus a later "all packages" non-consumable. None in 1.0. Every old Workbench item becomes free: library, folders and search, export, review tools, Listen here and the extended memory. | judgement (medium) | The library, export and review-tool specs (all free), and the shape of spec 024's resumption. |
| **O-6** | Subscription | **Confirm: none, at launch or in 1.x.** Revisit only if a catalog grows on a schedule. | evidenced (3.1.2(a)) + judgement (high) | Nothing to build. It closes the "future pack access" thread. |
| **O-3** | The original microphone audio (remaining parts) | Already decided: it is kept for every recording in both modes (2026-10-29). Recommended for the rest: the voice-activation gaps are mapped by the engine's timeline maps, never by trimming the original; the recording's details screen shows "Original microphone audio · 24-bit · N MB" with "Delete original only" (which ends re-render and Listen here for that recording, stated in the dialog); a storage screen; the labels stay "Data Not Collected". | evidenced (privacy definitions) + judgement | The recorder spec's storage model, the deletion UI and the privacy labels. |
| **O-5** | Default mode | **First launch opens Recorder mode's front**, with a one-time card offering Investigation. After that the app reopens the last-used mode. Switching is one persistent control on the front ("Start an investigation") and one on the investigation screen ("Recorder"). | judgement (medium-high) | The first-run flow, the permission timing, the App Review path and the screenshot order. |

## Contradictions found between recorded decisions and notes

These are reported here and not fixed in place. The owner decides which wording wins.

1. **Two rows conflict once 1.0 has no purchase.**
   - The 2026-10-29 row says "everything shipped free in 1.0 stays free for good; later purchases only add components".
   - The 2026-10-29 four-kinds row, the 2026-10-31 spec 021 row and 1.0-feature-set.md § 5 put "model internals and live playback bypasses in the Workbench".
   - If the Workbench is a purchase, it sells knobs, not components, which the first row rules out. If 1.0 ships the knobs without a purchase (the no-revenue constraint makes that likely), they are free for good under the first row anyway.
   - Either way the Workbench line cannot hold as written. O-1 below resolves it.
2. **ui-brief-three-screens.md** (the designer's brief) disagrees with the decisions in five places:
   - "Everything in the three screens below is free" while it shows bypass switches and module knobs. Those are Workbench under the four-kinds row. Under the O-1 recommendation the sentence becomes true.
   - "Changes are heard immediately, on whatever is playing or being recorded" contradicts the 2026-10-29 record-side row: the record-side switches, the speech coder and the memory change only between recordings.
   - "Reorder them" contradicts spec 024 D1: one slot, one point.
   - The review face's scrub, jump-to-mark and loop need a playback start position, which the engine does not have (Engine request R1).
   - The ambient light and sound-level readings are outside the 2026-10-29 Investigation 1.0 scope.
3. **brief.md still lists O-3 and O-15 as open.** Both were decided on 2026-10-29 (decisions.md: the O-3 row and the free-stays-free / Recorder-mode row). The pointers added under O-1 – O-6 send readers here. The question text is unchanged.
4. **Spec 024's status numbers collide with spec 025.**
   - The held draft (D9) assigns C statuses 39 `NO_SUCH_INSERTION_POINT`, 40 `NO_SUCH_COMPONENT` and 41 `COMPONENT_WHILE_ACTIVE`.
   - Spec 025 has since shipped 39 `NOT_A_SETTING` and 40 `NOT_A_PLAIN_LIVE_CONTROL` (`rr_dr60.h`).
   - The resumption must renumber them (to 41 – 43). The draft's control indices 31 – 35 are still free.
5. **The app repo has only a test-bench constitution** (`ic-recorder-app/.specify/memory/constitution.md`, "IC Recorder Test Bench Constitution").
   - Its principle I forbids app-side signal processing on the audio path. That is right for the product too, but it rules out an app-side reverse playback.
   - Its principle II (every C function reachable) does not fit a product.
   - The first product spec needs a constitution amendment first. See "The first app spec".

---

## O-4 · Release sequencing

### Options

1. **Both modes, the full free inside, no purchase code in 1.0;** the first equipment package in a later 1.x. *(Recommended.)*
2. **Both modes with the reduced inside** (the 2026-10-11 plan: meters and badges, no touch controls); the knobs follow in 1.1.
3. **Recorder mode first, Investigation in 1.1.** This contradicts the owner's constraint of both modes at launch.
4. **1.0 with the first paid package at launch.**

### Recommendation: option 1

- **The constraints decide it.** No revenue in year one, no deadline, and "success = an app people use". A purchase at launch earns nothing the owner needs. It does add a review surface:
  - 3.1.1 (in-app purchase) and its restore mechanism;
  - 2.3.2, which requires the description and screenshots to show what is paid;
  - Play's billing and disclosure rules;
  - one more thing to fail on a first submission (O-17).
- **Option 2 makes a weaker first impression and gains nothing.** The four-kinds row already makes the device controls and unit conditions free. The bench (six iterations, all 104 C functions) shows the inside's controls work on the phone. Holding them back only makes 1.0 look thinner to an App Review 4.2 reviewer.
- **Spec 024 (the slot and the band filter) is not on 1.0's critical path.**
  - It matters for the paid direction, not for the table stakes (O-17) or for 4.2.
  - Recommended: resume it after the first app spec, as an engine 0.9x minor. The band filter goes into 1.0 only if it is merged before the beta freeze; otherwise 1.1.
  - The owner's 2026-11-01 request ("include one basic fx plugin for free in the initial release") stays the aim. It is not a gate.
- **A TestFlight beta before 1.0** with 20 – 50 testers from the community. TestFlight needs the paid program, which the 2026-10-29 organization enrolment needs anyway, after a D-U-N-S number. **The D-U-N-S request and the enrolment are the only items with outside lead time:** start them now.
- **Android follows iOS 1.0.** The engine's Android archive and the arm64 golden check (1.0-feature-set.md § 7) go first.

### What changed since the question was opened (2026-10-15)

- The free adjustments now exist and are live:
  - the control manifest (spec 014);
  - the schedule (specs 015 and 017);
  - the battery (018);
  - the live playback bypass (021);
  - pitch-kept speed (023);
  - control access by index (025).
- The "reduced inside" was a hedge against an engine that could not yet expose its controls safely. That hedge is no longer needed.
- Engine and app go 1.0 together (2026-10-31), so the app's spec sequence now sets the engine's 1.0 date.

### Engine work

- None for the sequencing itself.
- **R1** (a playback start position) is needed for the table-stakes review tools; see O-2.
- Resuming spec 024 is a separate engine minor (renumber its statuses, see Contradictions 4).

### Store and compliance

- **4.2 minimum functionality:** both modes plus the flip give a reviewer a full instrument inside 30 seconds (O-17).
- **Review notes:** on the first submission, attach a short note naming what each mode does and that no purchase exists.
- **Age rating:** answer the questionnaire plainly; the app has no user-generated content shared through it and no web access.

---

## O-1 · Which controls are free

### Options

1. **The 2026-10-29 line:** device controls and unit conditions free; model internals and emulator options in a paid Workbench. *(It conflicts with the free-stays-free row; see Contradictions 1.)*
2. **Device controls, unit conditions and model internals free;** the emulator options (stage switches) paid.
3. **Everything user-meaningful free, with progressive disclosure;** host parameters hidden; purchases sell components only. *(Recommended.)*
4. **Everything free and shown flat** (the bench's surface).

### Recommendation: option 3

- **It is the only line consistent with both 2026-10-29 rows** once 1.0 ships without a purchase. Under option 1 the knobs are either absent from 1.0 (and can then never be sold without breaking "purchases only add components") or present (and free for good).
- **Knobs are a weak product to sell and a trust risk in this category.**
  - The engine is MIT, so any competitor can expose the same knobs free.
  - The category's one-star themes are paywalls before a trial and "paid and it still didn't work" (competitors.md, 2026-10-11).
  - A paid "pause threshold" knob invites the reading "pay to make it hear more". That reading drifts toward a claim the project refuses to make.
  - Components (a filter, later an EQ or reverb) are named by what they do to sound and read as equipment.
- **The disclosure keeps the front simple** for the group-hunt and one-time users (personas.md), and gives the tinkerers, who are the likeliest to spread the app by word of mouth, the depth they came for. Recorder mode stays the plain device for every user (the 2026-10-29 O-15 row).
- **The curated-presets rule survives unchanged:** presets may set any value. The Workbench's old "sells the knobs, not the values" becomes "sells components; the knobs are free".

### The 31 controls under the recommendation

**Placement legend:**

- **Front:** on the recorder's front panel.
- **Card:** in its stage's card on the inside, at the card's first level.
- **Card › Model:** inside the card's second level, "Adjust the model". It opens on tap, and its values carry the "modeled, low confidence" badge and the A-IDs.
- **Card › Emulator:** a switch drawn outside the device artwork and labelled "(emulator)".
- **Hidden:** set by the app, never shown as a control.

Every control has "device default" in its card (the 2026-10-11 card rule).

| # | Identifier | Kind (manifest) | Timing | Tier | Placement and notes |
|---|---|---|---|---|---|
| 0 | `route` | device control | live | **free** | Front (speaker / earphone). |
| 1 | `volume` | device control | live | **free** | Front knob; one position per screen frame (A-045's restart rule). |
| 2 | `playback_speed` | device control | live | **free** | Front, five steps, one level per gesture (2026-10-30 row). |
| 3 | `host_rate_hz` | host parameter | host-fixed | **hidden** | The app's audio session sets it (48 kHz recommended, investigation-mode.md § 6). |
| 4 | `record_stage_enabled` | emulator option | idle | **free** | Card › Emulator, stage 4. Applies to the next recording. |
| 5 | `playback_stage_enabled` | emulator option | live | **free** | Card › Emulator, stage 10; live A/B during playback (spec 021). |
| 6 | `adc_clamp_enabled` | emulator option | idle | **free** | Card › Emulator, stage 4. Next recording. |
| 7 | `tap` | emulator option | idle | **free** | Card "Listen here", in the one-pass monitor only: earphones connected, not while recording or playing. A 1.0 candidate, may slip to 1.1. Listen here on a past recording is a re-render (1.1). |
| 8 | `seed` | host parameter | idle | **hidden** | Drawn per engine from the system's random source (2026-10-31 row). Written to the processing record, never shown. |
| 9 | `agc.enabled` | emulator option | idle | **free** | Card › Emulator, stage 3. Next recording. |
| 10 | `agc.knee_dbfs` | model internal | live | **free** | Card › Model, stage 3. |
| 11 | `agc.attack_ms` | model internal | live | **free** | Card › Model, stage 3. |
| 12 | `agc.release_ms` | model internal | live | **free** | Card › Model, stage 3. |
| 13 | `vas.enabled` | emulator option | idle | **free** | Card › Emulator, stage 5, labelled "No voice-activated pause (emulator)"; the device has no such switch (A-008). Also reachable through presets. |
| 14 | `vas.mode` | emulator option | idle | **free** | Card › Emulator, stage 5 (drop / mute). The Investigation default is investigation-mode.md § 6 Q2. |
| 15 | `vas.sensitivity` | device control | live | **free** | Front, five steps. Faithful use is between recordings; live is the emulator's (S-001). |
| 16 | `vas.threshold_dbfs` | model internal | live | **free** | Card › Model, stage 5; also the "units differ" presets (2026-10-29). |
| 17 | `vas.hang_ms` | model internal | live | **free** | Card › Model, stage 5. |
| 18 | `vas.onset_ms` | model internal | live | **free** | Card › Model, stage 5. |
| 19 | `memory.capacity_s` | emulator option | idle, memory-dependent | **free** | Set by mode, not a knob: Recorder mode 3600 s (device); Investigation sessions an extended memory, labelled "(emulator)". See O-2 › Expanded memory. |
| 20 | `memory.max_recordings` | emulator option | idle, memory-dependent | **free** | As row 19: 99 in Recorder mode; the library beyond the memory is the app's (O-2). |
| 21 | `speech_codec_enabled` | emulator option | idle, memory-dependent | **free** | Card › Emulator, stages 7 and 9. It needs an empty memory: the app applies it at the next empty memory, or runs a second engine for "codec bypassed" recordings. The plan decides which. |
| 22 | `output.volume_amp_enabled` | emulator option | live | **free** | Card › Emulator, stage 11. **The loudness guard is mandatory:** move the volume to the neutral position first, or confirm (`stage11_bypass_level_db`); grey out volume and battery while it is bypassed. |
| 23 | `output.speaker_enabled` | emulator option | live | **free** | Card › Emulator, stage 12; live A/B. |
| 24 | `output.earphone_enabled` | emulator option | live | **free** | Card › Emulator, stage 13; live A/B. |
| 25 | `output.clamp_enabled` | emulator option | idle | **hidden** | A host convenience: the app turns it on when it writes integer audio (export). If it is ever shown, it is a count outside the device artwork. |
| 26 | `mic.capsule_enabled` | emulator option | idle | **free** | Card › Emulator, stage 1. Next recording. |
| 27 | `mic.amp_enabled` | emulator option | idle | **free** | Card › Emulator, stage 2. Next recording. |
| 28 | `mic.hiss_offset_db` | unit condition | live | **free** | Card, stage 1, "Hiss level"; presets such as "Noisy capsule (+6 dB hiss)". |
| 29 | `battery_volts` | unit condition | live | **free** | Card, stage 15, with the engine's trajectory presets (`rr_dr60_battery_trajectory_row`). Never driven from the phone. |
| 30 | `playback_speed_mode` | model internal | live | **free** | Front: a "Keep pitch (app option)" switch beside the speed control (the UI brief's request). Card › Model, stage 9, with the labels "Pitch with speed (modeled default)" / "Pitch kept (engine option)". |

**Counts:** 28 free (4 device controls, 2 unit conditions, 7 model internals, 15 emulator options), 3 hidden (2 host parameters, 1 emulator option), **0 paid**.

**Paid, later:** components only. Spec 024's own entries (`insert.before_limiter` and the four `band_filter.*` parameters, indices 31 – 35 in the draft) are free by its D11. A purchased package's components unlock by the app's table, keyed by the component's identifier (024 D11, product M2).

### What changed since the question was opened

- The question asked for "a list the engine does not have". The engine now has one: the control manifest (spec 014: `rr_dr60::controls()`, `rr_dr60_control_count` / `rr_dr60_control`), with a kind and a timing on every entry.
- Spec 025 adds generic access by index (`Pipeline::control_value` / `set_control_value`; C `rr_dr60_control_value`, `rr_dr60_set_control_value`, `rr_dr60_settings_control_value`, `rr_dr60_set_settings_control_value`). The app can therefore build every card from the manifest, as the bench already does, and the tier becomes one table in the app keyed by identifier.

### Engine work

- None. Everything above exists at 0.91.0.
- The live A/B and the loudness rule use `schedule_change` with the indices of rows 5 and 22 – 24 and `stage11_bypass_level_db`.
- The between-recordings path for the idle switches is `reconfigure` (refused while active).
- The presets are values validated against the manifest (`rr_dr60_settings_validate`), never scripts (2.5.2).

### Store and compliance

- **Accurate labels (1.1.6).** Every card that shows a model internal states "modeled, low confidence (A-0xx)". Guideline 1.1.6 rejects "inaccurate device data". This app's controls describe a model of a device, so they must say they are modeled, not that they are the device's.
- **No outcome names (claims policy).** Never name a control or preset by an outcome ("Clear voices", "Sensitive"). The control manifest's sentences already pass the denylist; use them as the card text.

---

## O-2 · The paid boundary and its shape

### Options

1. **Equipment packages:** each a non-consumable of engine components with example chains and presets, plus a later "all packages" non-consumable. The library, export and review tools are free. *(Recommended.)*
2. **One "Workbench" non-consumable** that unlocks the knobs and every future component (the 2026-10-11 shape). It conflicts with the free-stays-free row, and "future access" for a one-time price is a promise without a cost ceiling.
3. **Packages plus paid utilities** (expanded memory, folders, stem export, review tools as paid items).
4. **A tip jar or supporter purchase** that unlocks nothing (a consumable or a non-consumable "supporter" badge).

### Recommendation: option 1, with every old Workbench item free

| Old Workbench item | Recommended home | Reason |
|---|---|---|
| Expanded memory beyond 60 min / 99 | **Free.** Recorder mode keeps the device's 60 min / 99 (its "memory full" moment is faithful). Investigation sessions use an extended memory labelled "(emulator)" (for example 4 h; the plan picks the value from the footprint: about 1.8 MB/h with the coder on, about 86 MB/h bypassed). The app's library holds recordings beyond the memory. | Investigation sessions run an hour or more (investigation-mode.md); a session cut at 60 min breaks that mode's core job. A cap used as a paywall reads as the "usage limits" complaint (competitors.md, GhostTube SLS). |
| Stage taps and "Listen here" | **Free:** the monitor's Listen point (O-1 row 7) in 1.0 or 1.1; Listen here on a past recording through the re-render in 1.1. | It is the inside's demonstration of the positioning: the 4.3 differentiator. |
| Stem export | **Free**, as part of the case file and a recording's share: the processed WAV, the original microphone WAV (opt-in on each share), the stored form and the processing record. | It is table stakes (O-17: export). The 2026-10-31 processing-record row is already free. |
| Folders, search, tags | **Free.** | Table stakes (O-17: "a session library with tags"). |
| Review tools: scrub, trim, A-B loop, jump to mark | **Free**, 1.0. Scrub, jump and loop need R1. | Table stakes; the UI brief's review face. |
| Review tools: spectrogram, reverse playback | **Free**, 1.1. The spectrogram is an analysis display of the app's own copy, calm and labelled in Hz and dB. Reverse playback needs a decision on where it runs: engine-side (an R2 request) or an app exception to the constitution's principle I. | A paid analysis tool in this category reads as "pay to find more", the trust and claims hazard monetization.md names. |
| Band filter (spec 024) | **Free** (024 D11). | The owner's request: one free component in the first release. |
| Further components (EQ, reverb, flanging, unusual processing) | **Paid packages**, from a later 1.x. | The 2026-10-15 direction. Named by what they do. |

- **Package shape:**
  - One non-consumable per package (for example "Equalizer package": the component(s), two or three example chains, presets), with Family Sharing on.
  - When a third package exists, an "All packages" non-consumable covers the packages listed at its purchase. Never "every future package": that is an unbounded promise.
- **Price:** not set here. The category's one-time reference points are $3.99 – $9.99 (competitors.md, checked 2026-10-11; re-check before pricing).
- **Cheapest way to learn more:** ask the beta testers which component they would add first. That one question settles the first package's contents.
- **The open-source consequence** (024 D12): the engine's components are MIT and in the public manifest. A package's value is the app's chains, presets and UX, not the DSP. Packages should be priced and described that way, with no claim of exclusivity of sound.

### What changed since the question was opened

- Spec 024 drafts the mechanism: an insertion-point list, a component manifest, a closed set of engine components, the tier keyed by component identifier in the app, and the processing record's form version 2 carrying the component.
- The re-render (spec 020) makes Listen here on a past recording possible without new engine work.
- The schedule and log (015, 017) make presets and chains plain data.

### Engine work

- **R1: a playback start position** (new; feature description below). It is needed for scrub, jump to mark, loop and the device's own 10 SEC REVIEW (S-002 S11, not modeled).
  - Without it, the app reaches a place by playing silently from the start and discarding the output. That is about 70 ns per host sample, roughly 12 s per hour at 48 kHz on the development machine (judgement from the timing figures in CLAUDE.md): workable but clumsy, and it breaks the live review face.
  - The cheaper alternative is to scrub over a pre-rendered file of the playback, with speed and route fixed at render time. It weakens the review face.
- **Spec 024's resumption**, with the status renumbering (Contradictions 4).
- **R2, only if reverse playback is chosen:** reverse playback as an engine playback option.

### Store and compliance

- **3.1.1:** unlocks go through in-app purchase, with a restore mechanism for non-consumables.
- **2.3.2:** the description and screenshots must show which items need a purchase. That is the reason the store listing should show only free features until a package exists.
- **Play Payments, Section 2 and Section 6:** use Play billing, and show prices and terms clearly.
- **Package presets are data, not code (2.5.2).** Presets and chains in a package are declarative values validated against the manifest. A component is compiled into the engine and unlocked, never downloaded as code.

---

## O-6 · Subscription

### Options

1. **None at launch or in 1.x;** revisit only when a catalog grows on a schedule. *(Recommended.)*
2. **An annual "pack access" plan** beside the one-time packages.
3. **A subscription from 1.0.**

### Recommendation: option 1, confirmed

- **3.1.2(a)** requires "ongoing value" from an auto-renewable subscription. Nothing in the plan delivers value on a schedule: there is no content feed, no cloud and no second device.
- **The category punishes weekly plans and paywalls before a trial**, and tolerates cheap annual plans only where the free mode works (competitors.md, 2026-10-11).
- **No subscription matches the constraints:** the no-revenue year and "success = an app people use".
- **Reversal cost is low.** Adding a subscription later for new content is possible; removing one is not (the 2026-10-11 row).

### What changed

- Packages, not a single unlock, are now the direction. That is what raised "future pack access". The "All packages" non-consumable in O-2 answers it without a subscription.

### Engine work

None.

### Store and compliance

- If a subscription ever comes, 3.1.2(a) applies: a period of at least seven days, available across the user's devices, with ongoing value.

---

## O-3 · The original microphone audio (the remaining parts)

**Already decided (2026-10-29):**

- The original microphone audio is kept for every recording in both modes, at 24-bit lossless, about 250 – 350 MB/h, and deleted with the recording.
- It is free (the free-stays-free row).
- The parts still open are below.

### Options for the remaining parts

**A. How it relates to voice activation:**

1. Keep the original continuous, and map the gaps with the engine's maps. *(Recommended.)*
2. Trim the original to match the kept audio.

**B. Deletion:**

1. With the recording only.
2. Also "Delete original only", per recording and in bulk. *(Recommended.)*

**C. Where it shows:**

1. In the recording's details. *(Recommended.)*
2. As a separate list.

### Recommendation

- **A1: keep the original continuous.**
  - It is the input the re-render needs (spec 020: an engine at the record's counter, fed the original audio through `rr_dr60_samples_from_i24`).
  - Investigation review needs the paused time to stay real time.
  - The engine supplies the maps: `timeline::kept_to_input` / `input_to_kept` (C `rr_dr60_kept_to_input`, `rr_dr60_input_to_kept`), and the processing record's gap rows.
  - Show an alignment as a window (the 2026-10-31 timeline rule).
- **B2: also allow "Delete original only".**
  - The dialog says what is lost: "This recording will keep its processed sound. Without the original microphone audio it can no longer be re-rendered or listened to at another stage." The processing record stays; its reader then reports the re-render as unavailable.
  - Add a **storage screen** ("Original microphone audio: 2.1 GB in 14 recordings"), with "Delete originals older than…" as the bulk path. Phones fill, and 300 MB/h is the largest storage cost the app has.
- **C1: show it in the recording's details.** The line reads "Original microphone audio · 24-bit · 312 MB". Never "raw" (the product denylist).
- **Backup:** keep the originals in the app's backed-up data by default (they are the user's data), and say so on the storage screen. Excluding them from backup risks losing a case file's re-render input on a device restore. *(judgement)*

### What changed

- Spec 020 made the original audio load-bearing: the processing record plus the original reproduce the recording bit for bit.
- The bench's iteration 006 runs that re-render on the phone.

### Engine work

None. The engine keeps no audio (1.0-feature-set.md § 4.11).

### Store and compliance

- **Privacy labels stay "Data Not Collected":**
  - Apple: "Data that is processed only on device is not 'collected'".
  - Google Play: access is not collection when it "occurs solely on the user's device".
  - Play also exempts transfers "based on a specific user-initiated action", which covers the share sheet and the case file.
  - These labels hold only while there is no analytics or crash SDK that sends data off the device (the 2026-10-11 row). Re-check before each submission.
- **Microphone purpose string (5.1.1: "clearly and completely describe your use").** Draft: "Records through the app's model of a 1990s voice recorder. Recordings and the original microphone audio stay on this iPhone unless you share them." It passes the denylist.
- **Motion and barometer** (Investigation): the Motion & Fitness purpose string, when the barometer needs it, is still to confirm (1.0-feature-set.md § 6).

---

## O-5 · Default mode

### Options

1. **Recorder front on first launch;** a one-time card offers Investigation; after that, the last-used mode. *(Recommended.)*
2. **A first-run chooser** ("How will you use it?").
3. **Always the Recorder front.**
4. **Investigation first** (investigation-mode.md's sequencing remark).

### Recommendation: option 1

- **It serves the curious one-time user:** the persona's job is "a first recording within a minute, no account" (personas.md). Asking for the microphone permission on the first Record tap gives the permission its context.
- **It gives an App Review reviewer a working instrument on the first screen** (4.2). Investigation asks for more (a case name, motion permission, the sensor check) before it shows anything.
- **A chooser** (option 2) costs every user a decision before they hear anything, and Investigation users find their mode in one tap anyway.
- **Reopening the last-used mode** respects the investigator who opens the app on site.
- **Switching:**
  - one persistent "Start an investigation" control on the front;
  - "Recorder" on the investigation screen;
  - the flip stays the way to the inside in both modes.
  - A mode switch is refused while recording, with a sentence. It never interrupts a recording (the 2026-10-15 flip rule).
- **Screenshot order** follows: front, inside with the status header (the 2026-10-11 row), Investigation live, Investigation review.

### What changed

- Investigation 1.0 is now the smaller scope (the 2026-10-29 row), so it is a deliberate second step, not a co-equal first screen.

### Engine work

None.

### Store and compliance

- No guideline applies directly. The permission timing serves 5.1.1's minimization: ask for motion only when an investigation starts.

---

## Engine requests

Candidate feature descriptions for `/speckit-specify` in the engine repo, each a 0.9x minor.

**R1: start playback at a place** (serves the free review tools and the device's 10 SEC REVIEW):

> As an app user reviewing a recording, I want playback to start at a chosen place in the recording (and to jump back ten seconds, as the recorder's own review key does), so that I can scrub a waveform, jump to a mark and loop a region without playing silently from the start. The engine should accept a start place (in stored device samples, rounded to the speech coder's frame where the coder is on, the rounding reported) at `start_playback` or as a new call; the output from that place should be specified relative to a playback from the start (bit-identical after a stated warm-up, or a stated settle), the speed schedule, `place_at` / `place_at_in_mode` and the change log's positions should stay consistent, and "10 SEC REVIEW" should be documented as modeled on S-002 control S11 (its exact device behavior an assumption). Free tier: a device feature and the table stakes' scrub.

**R2: reverse playback** (only if O-2's reverse-playback item is chosen; 1.1):

> As an app user, I want to play a recording backwards through the modeled playback path, so that the review tool stays inside the engine's chain (the app processes no audio itself) and is reproducible from the change log. It is an emulator option, not a device behavior; named "Reverse playback (emulator)".

**Spec 024's resumption:** the reviewed draft with its C statuses renumbered after spec 025 (39 and 40 are taken).

---

## The order to take the answers in

1. **O-4 first.** Whether 1.0 carries purchase code decides what "free in 1.0" means for every other answer.
2. **O-1**, now concrete: anything shipped in 1.0 is free for good.
3. **O-2**: where the old Workbench items go, and the package shape.
4. **O-6**: a one-line confirmation once O-2 is settled.
5. **O-3**, the remaining parts: deletion and storage, which the first app spec needs.
6. **O-5**: the first-run flow, also needed by the first app spec.

O-16 (the "entertainment" disclaimer) should be taken in the same sitting. Guideline 1.1.6 now says in its text: "Stating that the app is 'for entertainment purposes' won't overcome this guideline" (checked 2026-11-01). That supports O-16's option 1: keep the rule and write a neutral statement of what the app does. A draft: "This app models how a 1990s voice recorder processes sound. It records and plays back through that model; it does not interpret what a recording contains."

## The first app spec (proposal for the app repo)

**App spec 007, "Recorder mode: the free device front"**, on its own branch in `ic-recorder-app`, preceded by a constitution amendment.

- **The amendment.** It adds product principles beside the bench's:
  - the claims policy;
  - free stays free;
  - no purchase code in 1.0;
  - privacy "Data Not Collected";
  - the progressive-disclosure rule;
  - principle II (every C function reachable) scoped to the Bench target only.
- **The product target.** A second app target, "IC Recorder", beside the Bench, sharing the proven `EngineHost`, the audio session and the ring buffer. Recorder mode's front as the home screen:
  - record / stop / play with the voice-activation indicator ("paused, waiting for sound");
  - the four front controls of O-1 (route, volume, playback speed with the "Keep pitch (app option)" switch, sensitivity), each built from the control manifest;
  - the memory gauge (60 min / 99, with the device's "memory full" stop);
  - the recordings list (rename, share the processed WAV, delete);
  - for every recording, its stored form, its processing record and its original microphone audio, kept as files in the app's library, with the details line and "Delete original only" of O-3 and the storage screen;
  - the first-run flow of O-5 (the microphone permission on the first Record tap; the one-time Investigation card, linking to a placeholder until the Investigation spec);
  - the "Inside" flip button, leading for now to the status header and the flat text list of the stage manifest (the inside's board is spec 008).
- **Simulator tests** drive it through the engine host. The device checklist covers a first recording within a minute of install and a memory-full stop.
- **Next specs.** The inside's board with its cards and the O-1 placement table is spec 008. Investigation is 009. R1 is requested from the engine in parallel for the review face.

## Sources

Store policy, checked 2026-11-01:

- Apple App Review Guidelines, <https://developer.apple.com/app-store/review/guidelines/>:
  - **1.1.6:** "Stating that the app is 'for entertainment purposes' won't overcome this guideline".
  - **2.3.2:** "clearly indicate whether any featured items … require additional purchases".
  - **2.5.2:** "may not … download, install, or execute code which introduces or changes features".
  - **3.1.1:** "you must use in-app purchase"; "a restore mechanism for any restorable in-app purchases".
  - **3.1.2(a):** "you must provide ongoing value … at least seven days".
  - **4.2:** "If your app is not particularly useful, unique, or 'app-like,' it doesn't belong on the App Store".
  - **5.1.1:** "Ensure your purpose strings clearly and completely describe your use of the data".
  - **5.2.1:** "Don't use protected third-party material such as trademarks … without permission".
- Apple App Privacy details, <https://developer.apple.com/app-store/app-privacy-details/>: "Data that is processed only on device is not 'collected' and does not need to be disclosed".
- Google Play Data safety, <https://support.google.com/googleplay/android-developer/answer/10787469>:
  - access is not collection when it "occurs solely on the user's device";
  - the user-initiated transfer exemption.
- Google Play Payments policy, <https://support.google.com/googleplay/android-developer/answer/9858738>:
  - Section 2: Play billing for paid features;
  - Section 6: clear terms and pricing.

Project records:

- decisions.md rows of 2026-10-11 – 2026-11-01;
- brief.md;
- 1.0-feature-set.md;
- controls-discussion.md;
- investigation-mode.md;
- monetization.md;
- personas.md;
- competitors.md (competitor prices checked 2026-10-11; re-check before pricing);
- ui-brief-three-screens.md;
- spec 024's draft on branch `024-insertable-component`;
- `ic-recorder-app` `docs/bench-plan.md` and `.specify/memory/constitution.md`.
