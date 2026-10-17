# Field measurement through the app (feasibility note)

Status: **idea under evaluation** (2026-10-13). Not a spec. Nothing here is implemented, and nothing here changes the emulator's behavior. When this becomes a feature it goes through Spec-Kit like everything else; this page records what the loop can and cannot measure so the spec starts from the physics rather than from hope.

## The idea

The emulator has no real-unit evidence yet: every modeled stage in the [stage manifest](signal-chain.md) is *assumed* or *sourced*, none is *checked against a real unit*. Owners of working RR-DR60s exist, but asking them to run a capture protocol with an audio interface (the [hardware-evidence issue template](../../.github/ISSUE_TEMPLATE/hardware_evidence.yml)) has produced nothing so far.

The proposal is to make the app the measurement rig:

1. The app plays a fixed stimulus sequence through the phone's speaker.
2. The user records it on their RR-DR60.
3. The user plays the recording back into the app, through the phone's microphone or, preferably, a cable from the 2.5 mm earphone jack.
4. The app analyses the return on the device and derives a set of figures (a *device profile*).
5. With consent, the user submits the profile: figures and metadata, **never audio**.

Many profiles, aggregated, give what one borrowed unit could not: the typical values *and* the spread across 25-to-30-year-old units.

## What the loop contains

```text
 acoustic leg (unavoidable)                     the device under test                       return leg
 phone speaker → room → [1 mic] → [2 amp] → [3 AGC] → [4 codec in] → [5 VAS] → [7 CELP] → [8] → [9] → [10 codec out] → [11 volume]
                                                                                                        ├─▶ [12 speaker] → room → phone mic   (acoustic return)
                                                                                                        └─▶ [13 earphone] → cable → phone input (electrical return)
```

Unknowns that are *not* the device: the phone speaker, the room (twice on an acoustic return), the phone microphone, and the phone's own audio processing. Two choices shrink them:

- **Electrical return.** The earphone path is an emitter follower fed from the power amplifier's input node while the amplifier is shut down, bypassing the amplifier and speaker (S-002 sheet 3.1, A-032). A 2.5 mm to 3.5 mm or Lightning/USB-C adapter removes stages 12, the second room pass and the phone mic. The follower drives the jack through a 10 µF capacitor, so its output corner depends on the load: about 110–300 Hz into a 16 Ω earphone, a few hertz into a high-impedance recording input; the electrical return therefore reads the earphone branch's input corner (≈ 70–145 Hz) alone, and a 16 Ω load across the jack would be needed to measure the device's own earphone sound. The record side has to stay acoustic: the device has no microphone input (S-001, S-002).
- **A reference pass without the device.** The app plays the same stimulus and records it straight back in the same room. Dividing the two passes cancels most of the phone speaker and room. It does not cancel what differs between passes: the device mic is not the phone mic, and the two sit at different positions. That residual *is* mostly stage 1 and 2, which is what we want.

The phone must record in a measurement mode (on iOS, `AVAudioSession` measurement mode with voice processing off); otherwise the phone's own AGC, noise suppression and echo cancellation contaminate every figure. A profile must carry the phone model and OS version so a contaminated batch can be excluded later.

## What survives unknown transducers

The robust figures are **self-referenced**: a ratio in time or a ratio in level within the same return, so the transducer's shape divides out.

| Figure | Register IDs | Probe | How it survives | Confidence |
|--------|--------------|-------|-----------------|------------|
| Band edge and sample rate | A-026, A-027 | Band-limited noise 100 Hz – 6 kHz; a slow sweep as a cross-check | The 2550 Hz cutoff is by far the sharpest feature in the chain, and phone speakers extend well past it. The edge frequency is read from the return's spectrum, not its absolute level. A residue at 3000–3450 Hz folded into the top of the band would show aliasing the emulator does not model (signal-chain row 4). | High |
| Clock accuracy and playback speed | A-011, S-001 speed control, S-012 | A recorded 2.2–2.4 kHz tone played back at speed level 3 through the earphone return, the volume up and the phone's input processing off | A unit records and plays on the same clock, so the played-back tone's shift against the phone's crystal reads the **record-to-play clock ratio and the speed setting** (about zero on any steady unit at level 3), not the clock error (corrected 2026-10-17; this row said the shift "gives the device clock error directly"). The clock is read from the codec's DAC image in the same return: each decoded component at f has an image at fs − f, and the earphone follower has no low-pass, so **f_tone + f_image = fs**, to about 20 ppm from a 10 s FFT and robust to the coder's own frequency errors. A 2.2–2.4 kHz tone's image (3.6–3.8 kHz) is the strongest; a 1 kHz tone's image at 5 kHz lies deep in the receive filter's stopband. One unit cannot show a common design offset (it lies inside the per-unit scatter); the spread across units replaces A-011's tolerance. At the other speed levels the same sum shows whether the codec clock changes: it tracks the speed ratio if it does and stays at fs if it does not (the [2026-10-17 research](<research/reports/Noise reduction clock and power.md>), capture P1). The profile should record the SPEED CONTROL position and level. | High (ratio and speed) / Medium (the image read) |
| AGC knee, dead time, release | A-028, A-034, A-035 | Level steps over about 20 dB around full scale; a loud burst then a quiet tone at 0.5, 2 and 10 s | All ratios of return level over time against the stimulus level. Frequency response is irrelevant. The absolute level that *is* full scale at the mic is unknown (A-029), so the staircase must bracket it and the knee is found as the step where the ratio stops being constant. | Medium: the phone speaker must reach the knee without gross distortion, see below |
| VAS threshold and hang | A-022 – A-025, A-033 | 1 s tone bursts with gaps of 0.3, 1, 2 and 5 s at three levels; a quiet floor sweep | Measured by output length and gaps, the spec 003 lesson. Independent of response. Also answers whether MIC SENS moves the threshold or a digital gain (A-033) if the user repeats at two settings. | High |
| Codec structure | A-004 (and spec 008's A-040 draft) | Tones, chirps, voiced and unvoiced speech-like bursts | Frame length appears as spectral or temporal periodicity; tone rendering, noise fill and the onset of codec failure on non-speech are present on any content and compared against the emulator's own return of the same stimulus. | Medium |
| Relative noise floor | A-029, A-030 | 20 s of silence from the phone; the return's noise relative to the 1 kHz reference tone | Level relative to the tone, and the floor's spectral shape. The absolute floor is not recoverable without an SPL anchor. | Medium |
| Earphone and speaker corners | A-010, A-031, A-032; A-042–A-044 (spec 009) | Noise and a 20 Hz – 3 kHz sweep, both returns; the earphone level against knob angle; a 1 kHz tone with rising volume at the speaker terminals | Speaker return divided by earphone return gives stage 11–12 relative to 13 for *that* unit, with the room residual; the sweep gives the earphone branch's input corner into a high-impedance input (add a 16 Ω load for the output corner); the knob sweep gives the loaded volume law; the speaker-terminal peak where distortion passes 10 % gives the clip ceiling and, with the earphone level at the same knob position, the amplifier's gain. | Low–medium |

## What does not survive

- **Absolute frequency response below about 500 Hz.** Phone speakers roll off there and differ by model. The low corners of the mic amp (≈ 72 Hz, A-006) and the earphone branch's input coupling (≈ 110 Hz, A-032) are mostly below where a phone speaker can speak.
- **The SPL anchor (A-029).** Full scale is somewhere in 80–100 dB SPL at the mic. A phone cannot calibrate itself; only a user with an SPL meter gives an anchor, and a profile field should allow one.
- **Distortion.** Phone speakers distort at the levels needed to drive the limiter past its knee, so harmonic figures from the loop describe the phone, not the device.
- **Anything per-unit the aggregate hides**: a profile from one unit is one sample of a 30-year-old population with aged electrolytics and speaker cones. The spread is the finding, which is why many submissions matter and why a single profile must never be reported as "the device".

## The codec changes the stimulus design

Every return has passed through a 4 kbit/s CELP coder (S-002). A speech coder at this rate does not preserve a sine sweep: the excitation and pitch stages render steady tones with their own artifacts, and the "response" read off a sweep is a codec behavior, not a filter. Two consequences:

- **Response from noise, not sweeps.** CELP transmits the LPC spectral envelope every frame, so the long-term spectrum of band-limited noise survives the coder and the band edge can be read from it. The harness already generates this stimulus (`stimulus::bandlimited_noise`) and reads it in third-octave bands (`analysis::third_octave_levels`) and as a log-spectral distance (`analysis::lpc_lsd`).
- **Tones probe the codec, not the filters.** Steady tones, chirps and the voiced / unvoiced bursts (`stimulus::voiced`, `stimulus::unvoiced_voiced`) are the right probes for A-004 and are compared against the emulator's own rendering of the same stimulus.

The VAS also constrains the sequence: every quiet gap in a stimulus that is not meant to probe the VAS must stay shorter than its hang time, or above its threshold, or the recording is spliced and every later timing figure is off. The spec 003 reports give the hang figures to design against.

## Reuse from the harness

Feature 008 Phase 1 committed the stimuli, metrics and a calibration harness for the codec. The field stimuli are a subset of `rr_dr60_harness::stimulus`, and most derived figures are `rr_dr60_harness::analysis` functions applied to a return aligned with `analysis::xcorr_lag`. The harness is a dev-dependency crate today. Shipping analysis in the app means either a `measure` module in the core (it would be the first non-DSP code there, and it is not real-time) or a small separate crate behind the C API. That choice belongs in the plan, not here.

## Bootstrap before the app exists

None of this needs the app to start. The same stimulus sequence can be published as WAV files with a desktop analysis script under `tools/`, and the hardware-evidence issue template can point at it. Anyone who already owns a unit could contribute a profile now, and the stimuli would be debugged on real units before they are baked into an app release.

## Open questions for the product side

These are for the product specialist (see `docs/product/`), not for this page:

- Consent wording and the App Store privacy label for a derived, non-audio submission.
- Whether the feature must be free-tier, since the value is in the number of submissions.
- What to do with profiles from recorders that are not RR-DR60s (a bonus dataset or a filter).
- How the aggregated result is presented without overclaiming: "checked against N real units" must mean exactly the figures in the table above, nothing more, under the claims policy in CLAUDE.md.
- Whether a user's own profile also personalizes their emulation ("my unit"), which is a different feature from the aggregate submission and should not be conflated with it.

## Honest summary

The loop can verify the band edge, the clock, the AGC timing, the VAS behavior and the codec's structure, which covers most of what is currently assumed in stages 3, 4, 5 and 7. It cannot verify absolute response, absolute level or distortion. That is a large fraction of the register for a feature that needs no hardware beyond a phone and a cheap adapter.
