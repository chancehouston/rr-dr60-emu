# Investigation mode: what is realistic and how to make it engaging

A consult of 2026-10-18 (engine 0.13.0) by the product specialist and the hardware-emulation reviewer, at the owner's request. The subject is Investigation mode from [brief.md](brief.md): audio recorded together with synchronized tracks from the phone's own sensors. It is **not** the shelved real-unit field measurement (`docs/hardware/field-measurement.md`). Working notes, not commitments: nothing here is a spec, and the open decisions at the end wait for the owner.

Both reviews: **sound with changes**. Phone-only sensor sessions are realistic. The experience can be engaging through the session ritual, a review timeline you can scrub, and one feature only an emulator can offer: showing what each modeled stage did at any moment. The main engine gap is a timeline map: the app cannot yet place every recorded sample on the session clock.

## Scope the owner set (2026-10-18)

- **Sensors:** phone only (magnetometer, motion, barometer, location and heading). No Bluetooth meters, no multi-phone team sessions for now.
- **What makes it engaging:**
  - a theatrical instrument;
  - "Inside this moment" (working name "Why did I hear that?");
  - case files and sharing.
  - Not goals, streaks or achievements.
- **Automatic marks:** allowed, but only as neutral measurements in their units, never as "activity", a "response" or a detection.

## 1. Phone sensors: what a reading means

Figures are inferred from public documentation and developer reports. Each is to be confirmed on real phones (see the evidence list in §6).

| Sensor | iOS API | Delivered rate | What a reading means |
|---|---|---|---|
| Magnetometer | `CMMagnetometerData` (raw, carries the phone's own bias); `CMDeviceMotion.magneticField` (calibrated, with an accuracy enum); `CLHeading.x/y/z` | about 50–100 Hz | A steady magnetic flux density, from DC to a few Hz, in µT. Earth's field is 25–65 µT. **Not** a mains-frequency (50/60 Hz) or RF meter. Resolution is about 0.15 µT per step (part class, unverified). |
| Accelerometer and gyro | `CMDeviceMotion` (`userAcceleration` in g, `rotationRate` in rad/s) | about 100 Hz | Handling, steps, setting the phone down |
| Barometer | `CMAltimeter` (`pressure` in **kPa**) | about **1 Hz, fixed** | 0.12 hPa ≈ 1 m of height. Weather drifts it 0.1–1 hPa per hour. |
| Location and heading | `CLLocation` (with accuracy figures); `CLHeading` (`headingAccuracy` < 0 means invalid) | about 1 Hz | Context only; 5–65 m indoors |

- **No public iOS API:** ambient light, temperature (`thermalState` is a four-level enum), humidity.
- **Android:**
  - The uncalibrated magnetometer returns its bias estimate separately.
  - Rates are capped at 200 Hz without `HIGH_SAMPLING_RATE_SENSORS`.
  - A light sensor exists.
  - Many mid-range phones have no barometer.

### The phone's own interference

The phone moves its own magnetometer reading. This is the main honesty issue for the sensor tracks. Sources (inferred; size each on a real phone):

- the haptic motor (a magnet and a coil);
- speaker current during playback;
- charging, display and torch-LED current;
- camera stabilization magnets (speculative);
- MagSafe cases and accessories (tens to hundreds of µT);
- calibration jumps: the calibrated field steps by several µT when the OS re-estimates its bias.

What to do about it:

- Turn haptics off during a session, with no haptic or sound feedback on MARK.
- Log the app's own audio output and haptics as a track.
- Tag every mark made while the speaker plays.
- Default live capture to earphones or muted output.
- **Self-interference check before a session.** With the phone at rest: 10 s quiet, 10 s of pink noise from the speaker, a screen-brightness sweep, one haptic pulse. Store the change and spread of |B| and of pressure for each condition in the case file. It is the only honest basis for a line such as "this phone moves its own reading by X µT when it plays".

**Record with every sample:**
- the raw and calibrated vectors, |B| and the accuracy status;
- the delivered timestamps (not the requested interval);
- charging, speaker and haptics state, and the audio session mode and route;
- device model, OS and app and engine versions.

## 2. Synchronization

- **One host clock.** Audio host time (mach ticks) and Core Motion timestamps (seconds since boot) share the same base. Location timestamps are wall-clock dates: map them through a stored (date, uptime) pair, refreshed every minute.
- **Audio clock drift.** Never convert a sample count to time. Log a (sample time, host time) pair per I/O buffer and fit the pairs piecewise-linearly. A ±20–50 ppm crystal would be ±72–180 ms per hour (a guess). Bluetooth input runs on the headset's clock.
- **Achievable alignment (inferred):**

  | Pair | Alignment |
  |---|---|
  | Audio to host clock | ±1–5 ms |
  | Audio to motion and magnetometer | ±10–20 ms |
  | Barometer | ±0.5–1 s |
  | Location | about ±1 s |

  Show coincidences as windows, never as instants. Don't use "synchronized" in copy until a measured figure exists, and never say "precise".
- **Interruptions and route changes.** Stop the item and start a new one, as the device would; don't feed zeros. A hardware-rate change needs `reconfigure`, which refuses during a recording: either resample to a fixed session rate in the app, or split the item. Capture in the audio session's measurement mode, so the OS applies no gain control or noise suppression.
- **Timeline mapping (engine gap, verified in code).**
  - A stored device sample is a *kept* sample. The 180-sample codec delay is counted in kept samples, after the voice activation (VAS) has removed its pauses. Subtracting the total record latency from an output index and then applying the splices is wrong by up to a splice's length.
  - The latency before the codec is not exposed separately.
  - An output sample depends on an input **interval** (about −30 to +10 ms, from the 20 ms frame, the 10 ms look-ahead and the filters), not an instant.

## 3. The experience

### Session flow

| Phase | What the user does |
|---|---|
| Before | Names the case and sees a sensor check (magnetometer "settling → ready"). Location is opt-in per session. |
| During | Large REC and MARK buttons, an audio scope, three labelled readouts. Automatic marks appear quietly. The flip shows the chain running. |
| After | "Close session" gives a summary card (elapsed, recorded and paused time; marks by kind), then immediate playback through the modeled speaker. |
| Review | One cursor across audio, sensor tracks and marks. A mark opens "Inside this moment". |
| Share | The case file. |

**Moments that make it feel good:**
1. The opening ritual: readings settle to "ready", then a printed slate line.
2. MARK: a large target with a visual response only.
3. Playback right after Stop, with the pauses gone. This is the device's signature behaviour.
4. Scrubbing the cursor and watching every track move together, then flipping into the chain at that moment.
5. The case-file cover: a timeline thumbnail and the counts.

**What makes it feel flat:**
- raw, jittery numbers;
- dozens of automatic marks;
- a review screen that is a list rather than a timeline;
- cards with generic stage text instead of this moment's numbers;
- decoration that animates whatever the state is.
- **Rule:** the atmosphere never animates from sensor values.

### "Inside this moment"

- **Name:** the public name is "Inside this moment". "Why did I hear that?" assumes there was a "that", and the answer then reads as an explanation of it.
- **Card rules:**
  - Say what each stage *did*, with this moment's numbers, a time span and the stage's status and assumption IDs.
  - Never say what the sound *is*. Never use "caused by", "just", "explains" or "real".
  - Sort stages by how much each one changed the signal. Untouched stages go on one line.
  - Offer listening, not verdicts: "As recorded / Original microphone" first, "This stage off" later.

**Example cards:**
- **VAS:** "Voice activation (VAS) paused recording from 12:04.1 to 12:08.9. The input stayed below −40 dBFS (sensitivity 3) for longer than the hang time. In the recording, those 4.8 s are removed, so sounds on either side play back-to-back. Modeled on the device firmware (A-008, A-022)."
- **Limiter:** "The limiter held the peaks at −1 dBFS for 0.6 s, beginning 3 ms after the level crossed it. Gain was reduced by up to 9 dB and returned to unity about 1.4 s later, so quieter sounds in that interval play back lower. Modeled on the circuit (A-028)."
- **Codec on hiss:** "Input this second: −51 dBFS, of which the microphone hiss was −62 dBFS. The speech codec codes every 20 ms frame with the same speech model (a pitch period plus pulses), whatever the input, so noise is given a periodic, buzzy texture. Same class as the device's coder, not its actual coder (A-004, A-040)."
- **Speed:** "Played at level 1 (0.6×). This 5.0 s passage lasted 8.3 s, and every frequency is 0.6× lower: 1 kHz plays as 600 Hz. Modeled on the device's Slow Play (A-054)."

The figures in these cards are illustrations. A shipped card prints the engine's numbers for that moment.

### Case files and sharing

- **Format:** one document file with its own extension, a zip container. It holds:
  - the processed audio as WAV;
  - the original microphone audio, only if the user opts in on each share;
  - `tracks.csv`, `markers.json` and `notes.txt`;
  - `session.json`: settings, seed, recording counter, model versions, event log;
  - a `README.txt` giving the units and the clock.
- **Without the app:** the recipient can unzip it, play the WAV and open the CSV.
- **Later:** a rendered video (waveform, tracks, captioned marks), or a static HTML viewer.
- **No hosted links, accounts or public gallery in the first release.** Hosting changes the privacy labels and brings the store's user-generated-content duties (App Store guideline 1.2).
- **Reproducibility:** "This recording re-renders identically from the included microphone audio with these settings (model 0.x)." It describes how the recording was processed. It does not vouch for what the input contains.
- **Privacy:**
  - Location is off by default, with a per-share toggle that is also off by default.
  - Shared files name the device model only.
  - The notes field reminds the user that notes travel with the file.
  - One neutral note on recording other people, shown before the first session and before the first share. No legal advice. About 11 US states require all-party consent ([RCFP](https://www.rcfp.org/introduction-to-reporters-recording-guide/)).
- **Labels:**
  - **Apple:** data processed only on the device is not "collected" ([App Privacy details](https://developer.apple.com/app-store/app-privacy-details/), read 2026-10-18). An export through the share sheet keeps "data not collected" (judgement).
  - **Google Play:** a user-initiated transfer the user reasonably expects is likely not "sharing" ([Data safety](https://support.google.com/googleplay/android-developer/answer/10787469), read 2026-10-18). Confirm before submission.
- **Store risks:** misleading claims (guideline 2.3.1): any bundled demo case and every screenshot carry mundane notes and neutral marks. Permission strings are needed for the microphone, location (optional) and Motion & Fitness (likely needed for the barometer; to confirm).

### Automatic marks

**Wording rules:**
- Name the sensor, the quantity, the change, the time window and the unit: "Magnetic field magnitude changed by 4.2 µT (phone at rest)".
- Print the context next to each mark: phone motion, sensor accuracy, and whether the speaker or haptics were active.
- The engine's own marks read "Emulated VAS resumed (threshold −40 dBFS)", never "voice".
- One neutral colour. A distinct shape for "Automatic: rule X" versus "Marked by you". No red, no 0–100 scales.
- At most one automatic mark per sensor per 5 s; merge the rest.
- Words never used: activity, spike, anomaly, hit, response, hotspot, presence, energy, detected.

**Starting thresholds** (guesses; tune with the self-interference data, aiming for 0–3 marks per quiet 10 minutes):

| Reading | Rule |
|---|---|
| Magnetic step | Raw \|B\| at rest, low-passed at 2 Hz. Mark when the 1 s medians before and after differ by ≥ max(3 µT, 6σ) and the new level holds ≥ 2 s. Suppress or tag if, within ±2 s, the gyro exceeds 0.1 rad/s, the accuracy status changes, or charging, speaker, torch or haptics change state. A slow drift is reported, never marked. |
| Motion | Intervals, not marks: \|userAcceleration\| > 0.05 g or \|ω\| > 0.2 rad/s. Used to tag other marks "phone moving". |
| Pressure step | ≥ 0.15 hPa within 5 s that holds for 10 s, tagged with the motion state. Most such steps are the phone moved vertically, a door or ventilation. |
| Sound level | On the raw microphone signal: the 100 ms RMS rises ≥ 10 dB above a 30 s running floor for ≥ 200 ms. Shown in dBFS, labelled "uncalibrated". |

**How the category presents the same readings** (App Store listings, read 2026-10-18):
- **Readings:** some list readings in µT. Others rate changes on a five-step scale or show a gauge with a three-axis chart.
- **Alerts:** "spike detection" with hotspot markers and haptic or sound alerts.
- **Disclaimers:** some state that nothing is detected, or that the premise is unproven.

None prints the motion or self-interference context next to a reading. Printing it is a neutral differentiator: it neither claims nor debunks.

### Vibration track (2026-10-21)

The owner's idea, from the 2026-10-21 working notes, and part of Investigation mode's feature set. It is exploratory; the thresholds are open.

- **What it is:** a track of the accelerometer's vibration level, recorded only while the phone lies still. While the phone is handled, the track shows "phone moving" instead, from the motion intervals above.
- **Why it helps the session:** it invites the user to put the phone down. A phone at rest gives quieter audio (no handling noise) and steadier magnetometer readings.
- **What it will pick up:** footsteps, doors, ventilation, traffic, appliances.
- **Wording:** a reading in its unit (g), with a time window, never "detector", "activity" or "presence". Next to an audio mark, it is a context line: "Vibration 0.004 g at 12:04.1 (phone at rest)". The app never says what a coincidence means, either way.
- **Self-interference:** the app stays silent while recording, with no haptics and no speaker output (already the session rule in § 1), so it does not record its own vibration.
- **Open:** the vibration measure (band-limited RMS of `userAcceleration`?), the rest criterion, and whether it produces automatic marks or is a track only.

### "Question asked here" marks (2026-10-21)

A second kind of user mark, from the same notes: one tap marks the moment a question was asked. It has its own shape, is labelled as the user's, and appears on the review timeline like MARK.

**Sensor timing (a note on the working notes):** the notes propose resampling every sensor to one common rate. Keep the delivered timestamps in the session record (§ 1, § 2), and resample only for display and analysis. The barometer delivers about once a second.

## 4. "Inside this moment" by re-rendering

**Verdict: sound, with conditions.** The engine is deterministic: the same input, settings and seed give bit-identical output. So a session that keeps the raw microphone audio, the settings, the seed and a change log can be re-rendered offline to recover any stage's tap and state at any moment.
- The record path has no live controls; its settings change only through `reconfigure`, when idle.
- Playback changes take effect at call boundaries. The replay is exact if the log records the output index of each boundary where a change took effect, and the replay splits its calls there.

| Hazard | Fix |
|---|---|
| The stored raw audio is not the exact samples that were fed (a lossy format, a converter run again) | Quantize to 24-bit at the feed point, feed that, and store it losslessly (about 300 MB/h at 48 kHz) |
| The hiss stream's counter has no public getter or setter | Add a counter getter and a construct-at-counter option |
| A library or model upgrade between recording and review | Store the library version and each stage's model version, plus a digest of the live output. On a mismatch, label the result "re-rendered with version X; differs from the live render". |
| Cross-platform identity | CI's golden jobs cover six desktop targets and the iOS simulator, but no Android target. Add an Android arm64 golden job before shared case files reach Android. |
| Random access | A bit-exact state needs a render from the item's start: about 27 s per hour of audio on the development machine, a guessed 0.5–1 min on a phone. An optional state checkpoint would remove this. |

- **Taps:** the tap is a construction-time setting, so N taps need N renders. Use per-block meters instead.
- **Counterfactual renders** (for example the hiss off, or the VAS bypassed) are exact differences and the most honest form of "This stage off".
- **Recommendation: a hybrid.**
  - Store a per-frame meter track live (about 7 MB/h). It is what the user saw, and it survives version upgrades.
  - Keep re-rendering for deep inspection, checked by the digest.

## 5. Engine work and the proposed order

Both reviewers move meters and the session contract ahead of the control work in [controls-discussion.md](controls-discussion.md) §4:

**016 meters and timeline map → 020 session contract (record side) → 014 control manifest → 015 positioned changes and change log → 017**, with 018, 019 and 021 unchanged. 014 is small and can run in parallel.

Feature descriptions (WHAT/WHY), ready for `/speckit-specify`:

- **016, expanded:** As an Investigation-mode app, I want a per-block read-out of what each modeled stage did, so that at any marked moment the app can show what the chain did there, using only numbers the engine computed. The read-out covers:
  - peak and RMS at the stage boundaries;
  - the hiss level;
  - the limiter's gain reduction and whether it is at the ceiling;
  - clamp hits;
  - the VAS state and events;
  - codec frames and their level;
  - the volume knob position;
  - the speed level and place.

  Each value is tagged with its input-sample and recording-sample position. It changes neither the audio nor determinism, and it is real-time safe.

  It includes a **timeline map**:
  - the latency before the codec and the codec's kept-domain delay, exposed separately;
  - a function from a stored sample and the VAS events to the host input interval it depends on;
  - a function from a playback output sample to the recording place.

  Target accuracy: one device sample. The cost budget is to be set by measurement (a first guess is about 5 % at 48 kHz).
- **020, record side first:** As someone sharing a case file, I want a versioned record of the settings, seed, recording counter, library and per-stage model versions and the event log, plus a digest of the stored form. Re-rendering the original microphone audio from that record must reproduce the recording bit for bit, so a recipient can confirm how it was processed. A model-version mismatch is refused with a clear status. Also wanted:
  - an offline reader for codec frame parameters (pitch lag, gains), with no live cost;
  - optionally, a state checkpoint and restore.
- **015 (built, engine 0.16.0):** each change a stream applies is logged at its output-sample position with the control, the values before and after and a caller tag; the recording counter has a public getter. Moved to 020: the host-input index a change maps to (through the voice-activation events), construct-at-counter, and the map from a playback position to the recording's place under a speed schedule.
- **017 (built, engine 0.17.0):** the sensitivity, the voice activation's threshold, hang and onset, the hiss level and the limiter's knee, attack and release change during a recording through the schedule, positioned in the input clock and logged; a change scheduled while stopped lands on the recording's first sample. The session's setting changes therefore come from the engine's log in microphone time. Still 020's: the entry's stream word, the flush boundary in the stop's report, the host-input index of a voice-activation event.
- **014:** unchanged, plus descriptions of the meter fields, so the app labels them from one source.
- **Chore:** an Android arm64 golden target before case files are shared across platforms.

Everything else is app work:
- sensors, clocks and anchors;
- marks and their suppression;
- raw-audio storage;
- the self-interference check;
- background operation and interruptions.

## 6. Open decisions and evidence

**Decisions for the owner** (the recommendations are the consultants'):

1. **The order:** 016 meters and timeline map, then 020, ahead of 014? Recommended: yes.
2. **Drop or mute mode by default in sessions.**
   - Mute mode keeps the timeline continuous; the hardware reviewer prefers it.
   - Drop mode is the device and gives the playback-after-Stop moment.
   - Main agent's view: drop mode as the default, since 016's timeline map is needed anyway, the speed control included.
3. **Live meter track (the hybrid)?** Recommended: yes.
4. **Original microphone audio in shared case files,** opt-in on each share? Recommended: yes. Touches O-3.
5. **Location in the first release?** Recommended: no.
6. **Feed rate and platforms:** feed the engine at 48 kHz, so the raw audio keeps full bandwidth? Recommended: yes. Reproduce on Android arm64 only.

**Where the deferred product questions bite** (not resolved here):
- **O-3:** raw audio in shared files.
- **O-2:** video export, A/B playback at a mark, review tools.
- **O-4:** this focus implies Investigation-first sequencing.
- **O-5:** the default mode.

**Evidence to gather on real phones** (app-side, about an hour each):
1. The self-interference figures on a given iPhone: speaker, haptics, brightness and charging against |B|.
2. Delivered motion and barometer rates and noise on two or three iPhone models (a 10-minute log at rest).
3. Audio clock against host clock over a 60-minute session (built-in microphone and Bluetooth).
4. Re-render time for one hour of audio on the oldest supported iPhone.
