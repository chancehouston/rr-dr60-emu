# rr-dr60-emu

**A software emulator of the Panasonic RR-DR60 voice recorder's sound, from the microphone to the speaker.** Feed it audio and get back what the recorder would have recorded and played back, as modeled from its service manual and the datasheets of its parts.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
![Version 0.18.0](https://img.shields.io/badge/version-0.18.0-informational)
![Status: pre-alpha](https://img.shields.io/badge/status-pre--alpha-orange)
![Rust 1.85+](https://img.shields.io/badge/rust-1.85%2B-lightgrey)

The RR-DR60 is a pocket digital voice recorder Panasonic sold in the late 1990s. Long after it was discontinued it became well known among paranormal investigators as an "EVP recorder", and working units are now sought after second-hand. This project recreates the recorder's audio signal chain in a portable Rust library with a C API, so apps can use the sound without the hardware.

## Project status

| | |
|---|---|
| **Version** | 0.18.0, pre-alpha. Not yet released or published to crates.io; the API may still change before 1.0. |
| **Signal chain** | 13 of the 15 stages are modeled and on by default (the power supply as the battery's effect on the speaker amplifier's clip ceiling, a user-set value). Not modeled: noise reduction (no evidence the device has it) and the clock. |
| **Accuracy** | Every modeled stage is built from documents (the service manual, the owner's manual, datasheets) plus recorded assumptions. **No stage has been checked against a real unit**; see [Accuracy](#accuracy). |
| **Platforms** | Rust library (`no_std`), C API, and a Swift package for iOS and macOS. Bit-identical output on Linux, macOS, Windows, the iOS simulator and an iOS device. |
| **Not yet** | A published release, a Swift-native API, an app (planned as a separate project). |

The [CHANGELOG](CHANGELOG.md) records every version; the per-stage table under [Accuracy](#accuracy) is generated from the library itself.

## What it does to sound

The emulator runs audio through the recorder's record path, its memory and its playback path:

```text
record    microphone → mic amplifier → limiter (AGC) → codec input (6 kHz) → voice-activated pause (VAS)
          → speech encoder (4 kbit/s) → flash memory
playback  flash memory → speech decoder → speed control → codec output → volume and amplifier → speaker or earphone
```

What you hear, in the model's default configuration:

- **Telephone-like band.** Everything passes through the codec at the device's 6 kHz sample rate: roughly 225 – 2550 Hz survives.
- **Thin low end and a steady hiss.** The microphone amplifier's input coupling rolls off below about 818 Hz, and the capsule adds a quiet hiss (about −62 dBFS).
- **Loud sounds are clipped, then held.** Very loud input clips in the microphone amplifier; the limiter lets an onset through for a few milliseconds, then holds the level just below full scale and recovers over about 1.4 s. Below its threshold the limiter leaves the signal untouched.
- **Pauses are cut out.** The voice-activated system stops recording while the sound stays below a threshold, so the output can be shorter than the input, with clipped onsets and abrupt joins.
- **Low-rate speech coding.** A 4 kbit/s CELP-class coder gives speech a slightly watery, buzzy texture and renders everything else (music, noise, tones) through a speech model. It models the *class* of coder the device used, not the device's own coder, which is undocumented.
- **A small speaker.** Playback goes through the volume control and power amplifier (which clips above its ceiling) to a small-speaker resonance, or to the earphone output. The amplifier runs from the two cells, so its clip ceiling follows a **battery voltage** value: fresh cells by default, lower cells clip loud playback sooner and lower, quiet playback is unchanged. Playback can run at the device's five speeds, 0.6× to 1.4×, with the pitch following.

Each stage can be switched off or tapped on its own, for example to hear the codec alone.

## Quick start

The library is not on crates.io yet; depend on the repository:

```toml
[dependencies]
rr_dr60 = { git = "https://github.com/chancehouston/rr-dr60-emu" }
```

```rust
use rr_dr60::{Pipeline, Settings};

let mut p = Pipeline::new(Settings::new(48_000))?;  // every modeled stage on; allocate outside the audio callback
let info = p.process_in_place(&mut buffer);         // real-time safe; any block length
let out = &buffer[..info.produced];                 // the VAS drops pauses: output can be shorter
```

From C, build the static library with `cargo build -p rr_dr60_ffi --release` and include [`rr_dr60.h`](crates/rr_dr60_ffi/include/rr_dr60.h); from Swift, use the [Swift package](swift/README.md). The [user guide](docs/guide.md#c--swift) has the C version of this example.

Four things to know before you ship:

1. **The output can be shorter than the input.** Use only `produced` samples. Set `vas.mode = VasMode::Mute` to keep the length (pauses become silence).
2. **The output can exceed ±1.0.** The speaker model rings on loud and transient material. Scale by 0.42, or turn on `output.clamp_enabled`, before converting to 16-bit.
3. **There is latency.** 1923 samples (about 40 ms) at 48 kHz by default; `latency_samples()` reports it for any configuration.
4. **The hiss is seeded.** Output is deterministic; set a different `seed` per session if recordings must not share the same hiss.

The engine also models the device's recording memory (60 minutes, 99 recordings, with export and import), its volume control, speaker/earphone routing, playback speed and battery voltage (a unit condition, set by the user, never read from the host device) as live controls, it describes its own stages and its controls through a built-in stage manifest and control manifest, and it offers a per-call **meter read-out** of what each stage did (peaks, the limiter's gain reduction and hold, the clamp counts, the voice activation's state and hang, the amplifier's clip count and the battery's ceiling factor) for apps that draw the chain live. The [user guide](docs/guide.md) covers all of it.

## Documentation

| Read this | For |
|---|---|
| [User guide](docs/guide.md) | Every stage's behavior, settings, figures and limits; recording and playback; the stage manifest and the control manifest (every control with its kind, range and default); real-time and performance notes |
| [C header](crates/rr_dr60_ffi/include/rr_dr60.h) · [Swift package](swift/README.md) | The C API and how to use it from Swift |
| [Signal chain](docs/hardware/signal-chain.md) | The device's fifteen stages, component by component, with what is known and what is assumed |
| [Assumptions](docs/hardware/assumptions.md) · [Sources](docs/hardware/sources.md) | Every modeled value traced to a document (S-###) or a registered assumption (A-###) |
| [CHANGELOG](CHANGELOG.md) | What changed in each version, including every change to an assumed value |
| [specs/](specs/) | The specification, plan and measurement reports behind each feature |
| [CONTRIBUTING](CONTRIBUTING.md) | Building, testing and the spec-driven workflow |

## Who it's for

- **App developers**, including makers of ghost-hunting and EVP apps, who want a sound modeled on the RR-DR60 without sourcing vintage hardware.
- **Audio and vintage-gear enthusiasts** who want to explore the device's character and adjust the model's internals.

This project is **neutral and technical**. It emulates what the recorder's electronics do to sound: band-limiting, noise, limiting and clipping, voice-activated pauses and speech-codec artifacts. It makes no claims, for or against, about paranormal phenomena.

## Accuracy

Exact specifications for the RR-DR60 aren't public. The service manual gives the circuit, the sample rate and the codec type; where a value is unknown the project makes a **reasonable, documented assumption** and records it in the [assumption register](docs/hardware/assumptions.md), with the [sources](docs/hardware/sources.md) behind it. The goal for version 1 is a **modeled character match**: frequency response, noise, limiter and VAS behavior, and codec artifacts within stated tolerances of the model built from those documents. A measurement harness checks the code against those specifications at every supported sample rate. **No stage has been checked against a real unit**, and that check is out of scope until the project has a unit it can open. Bit-exact reproduction isn't a goal.

The table below is the engine's own **stage manifest**, generated from the library (a test fails when it drifts). **Assumed** means at least one of the stage's assumptions is still open; **sourced** would mean every one is confirmed by a document; **not modeled** means the engine passes the stage through. "Not modeled here" names what the engine leaves out at a stage it does model. The basis column lists the source (S-) and assumption (A-) IDs.

<!-- manifest-table:start -->
| # | Stage | Status | Confidence | Basis | Not modeled here | Model version |
|---|---|---|---|---|---|---|
| 1 | Microphone capsule | assumed | low | S-002; A-005, A-048, A-049, A-050, A-052 | directivity; handling noise and wind; acoustic overload; the case's acoustics; the capsule's own response; unit spread | 0.12.0 |
| 2 | Microphone amplifier | assumed | low | S-002; A-006, A-029, A-046, A-047, A-051, A-053 | the output coupling's 159 Hz shift under limiting; below-clip distortion; part and transistor spread | 0.12.0 |
| 3 | Limiter (AGC) | assumed | low | S-001, S-002; A-007, A-019, A-020, A-028, A-030, A-034, A-035, A-036, A-037 | a soft knee, if the circuit has one | 0.5.0 |
| 4 | Codec input: filter, ADC and µ-law | assumed | medium | S-002, S-003, S-005, S-007; A-003, A-015, A-016, A-026, A-027, A-030 | µ-law quantization, idle noise, the input amp's clipping ahead of the filter, and aliasing of input above 3 kHz | 0.5.0 |
| 5 | Voice-activated pause (VAS) | assumed | low | S-001, S-002; A-008, A-021, A-022, A-023, A-024, A-025, A-033, A-037 | a digital gain from the sensitivity setting, if the device applies one | 0.5.0 |
| 6 | Noise reduction | not modeled | — | S-001, S-002, S-004, S-012, S-013; A-009 |  |  |
| 7 | Speech encoder (CELP, 4 kbit/s) | assumed | low | S-002, S-008, S-009, S-010, S-011; A-004, A-040, A-041 | the device's own coder (frame length, look-ahead, codebooks and bit allocation unidentified); 8-bit µ-law input (A-003) | 0.8.0 |
| 8 | Flash memory | assumed | low | S-001, S-002; A-037, A-038, A-039 | item locking (F.LOCK) | 0.8.0 |
| 9 | Speech decoder (CELP) | assumed | low | S-001, S-002, S-003, S-008, S-009, S-010, S-011, S-012, S-018; A-004, A-040, A-041, A-054 | the device's own decoder (frame length, look-ahead, codebooks); its speed method and the sound of a level change (A-054) | 0.19.0 |
| 10 | Codec output: DAC and reconstruction filter | assumed | medium | S-002, S-003, S-005, S-007; A-015, A-016, A-026, A-027, A-030 | µ-law expansion, idle noise on the playback side, DAC imaging residue above 3 kHz | 0.4.0 |
| 11 | Volume control and power amplifier | assumed | medium | S-001, S-002, S-006; A-031, A-043, A-044, A-045 | the 159 Hz corner's knob shift; the clip level's spread; the battery's sag; wiper noise; knob speed; the beeps | 0.11.0 |
| 12 | Speaker | assumed | low | S-001, S-002; A-010, A-042 | this driver's own response (the model is a generic small-speaker template, not a capture); nonlinearity, enclosure, room | 0.9.0 |
| 13 | Earphone output | assumed | low | S-001, S-002; A-012, A-032, A-042 | the follower's clipping cut-off on loud material; its corners' spread with the transistor's gain; loads other than 16 Ω | 0.9.0 |
| 14 | Clock | not modeled | — | S-002, S-003, S-016; A-011 |  |  |
| 15 | Power supply | assumed | low | S-001, S-002, S-006, S-014, S-015, S-017; A-013, A-056, A-057, A-058 | the 3.3 V converter and its rails; the cells' sag under load; supply noise; the low-battery indicator and cut-off | 0.18.0 |
<!-- manifest-table:end -->

Nothing in this table claims the emulator sounds like a real RR-DR60. It claims that each stage is built to a stated, traceable description, and that the harness checks the build against that description.

**Own a working RR-DR60?** Recordings of test signals from a real unit are welcome: open a [Hardware evidence](https://github.com/chancehouston/rr-dr60-emu/issues/new/choose) issue, and any capture that arrives is compared against the model.

## Contributing

Contributions of code, documentation, research and hardware evidence are welcome. The project uses [GitHub Spec-Kit](https://github.com/github/spec-kit): each feature starts as a specification in [`specs/`](specs/), under the principles in the [constitution](.specify/memory/constitution.md). See [CONTRIBUTING.md](CONTRIBUTING.md) to get started.

## License

[MIT](LICENSE) © 2026 Chance Houston

*Panasonic and RR-DR60 are trademarks of their respective owners. This is an independent project, not affiliated with or endorsed by Panasonic.*
