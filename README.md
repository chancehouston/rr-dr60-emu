# rr-dr60-emu

**A portable software emulator of the Panasonic RR-DR60 digital voice recorder's complete audio signal chain, from microphone to speaker.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
![Status: pre-alpha](https://img.shields.io/badge/status-pre--alpha-orange)

> ⚠️ **Pre-alpha.** Audio passes through the record-path AGC and the device's two voice-band
> codec stages (record and playback). The other stages (mic, VAS, speech codec, speaker) aren't built yet,
> and the API may still change before 1.0.

## What is this?

The Panasonic RR-DR60 is a mid-1990s pocket IC voice recorder. Long after it was discontinued, it became legendary among paranormal investigators as an "EVP recorder", and working units are now sought after on the second-hand market.

`rr-dr60-emu` aims to recreate that recorder's sound in software: audio goes in, and the goal is to get back what an RR-DR60 would have recorded and played back. The emulator models each stage of the device:

microphone → preamp → automatic gain control → voice-band codec (A/D) → voice-activated recording (VAS) → speech compression → storage → decompression → codec (D/A) → amplifier → speaker

The details of each stage are in [docs/hardware/signal-chain.md](docs/hardware/signal-chain.md).

### Who is it for?

- **App developers** (including ghost-hunting and EVP apps) who want an authentic RR-DR60 sound without sourcing vintage hardware.
- **Audiophiles and vintage-gear enthusiasts** curious about the device's sonic character.

### A note on claims

This project is **neutral and technical**. It emulates what the recorder's electronics do to sound: band-limiting, noise, gain pumping, voice-activation gating, and speech-codec artifacts. It makes no claims, for or against, about paranormal phenomena.

## Accuracy

Exact specifications for the RR-DR60 aren't fully public. Where they're unknown, we make **reasonable, documented assumptions** and record each one in the [assumption register](docs/hardware/assumptions.md) along with the [sources](docs/hardware/sources.md) behind it. The v1 goal is a **measured character match**: frequency response, noise, AGC and VAS behavior, and codec artifacts within stated tolerances. Bit-exact reproduction isn't a goal.

**Own a working RR-DR60?** Recordings of test signals from a real unit are the most valuable contribution you can make. Open a [Hardware evidence](https://github.com/chancehouston/rr-dr60-emu/issues/new/choose) issue.

### Fidelity status by stage

What each modeled stage rests on today. "Documented" means the behavior is taken from the device's own documentation (service manual, datasheets); "assumed" means an educated guess registered in the assumption register; nothing is **verified** against a real unit yet.

| Stage | Built? | Basis | Notes |
|---|---|---|---|
| Codec band-limiting (record and playback filters) | yes | assumed (A-014), to be revised | The service manual, read 2026-10-10, gives a **6 kHz** sampling rate and the codec family's datasheet gives a flatter, narrower-topped response than the current model (A-026, A-027). The current filters run at 8 kHz with 300–3400 Hz edges. |
| AGC | yes | assumed (A-017–A-019), to be revised | The schematic shows a limiter at the codec's full scale with a ~10 s release, not the wide-range compressor modeled now (A-028). |
| VAS (voice-activated pausing) | in progress ([spec 003](specs/003-vas/spec.md)) | documented behavior (owner's manual), assumed values registered by spec 003 | Threshold, hang and onset times are guesses until captures exist. |
| Microphone and mic amp | no | documented topology (A-005, A-006, A-029) | Gain and noise are estimates from the circuit. |
| µ-law quantization | no | documented (A-003) | |
| CELP speech codec, 4 kbit/s | no | documented bitrate and type (A-004); the exact codec is unknown | The stage most responsible for the device's sound; the hardest to get right. |
| Speaker amp, speaker, earphone path | no | documented topology (A-031, A-032); speaker acoustics assumed (A-010) | |

Nothing in this table claims the emulator sounds like a real RR-DR60 yet. It claims that each stage is built to a stated, traceable description, and that the harness checks the build against that description.

## Using the library

What exists today ([spec 001](specs/001-pipeline-skeleton/spec.md) and [spec 002](specs/002-agc/spec.md)):

- Mono audio at **8, 16, 44.1, 48, 88.2 or 96 kHz** goes in, in blocks of any size, and the same number of samples comes out at the same rate.
- Inside, the audio is converted to the device's internal 8 kHz rate and passes through two band-limiting stages. They model the record and playback filters of the recorder's MSM7702 voice-band codec, *assumed* to follow a telephone-style 300–3400 Hz band. They're designed to the spec's tolerances but haven't been measured against a real unit ([A-002, A-014–A-016](docs/hardware/assumptions.md)).
- A record-path **automatic gain control** (AGC), on by default. See [AGC](#agc-signal-chain-stage-3) below.
- Fixed, reported latency: 541 samples (about 11.3 ms) at 48 kHz in the default configuration. It is under 20 ms at every supported rate, and each configuration reports its own.
- **Real-time safe:** `process`, `process_in_place` and `reset` never allocate, lock or do I/O. Processing runs at about 230× real time at 48 kHz on a laptop with the AGC on (about 245× without it). `Pipeline::new` and `reconfigure` allocate, so call them outside the audio callback.
- **Deterministic:** the same input and settings give bit-identical output for any block size, and on every supported platform. One golden file is checked on Linux, macOS and Windows (x86-64 and ARM64) and on iOS.
- A C API with a generated header ([`rr_dr60.h`](crates/rr_dr60_ffi/include/rr_dr60.h)).

- Each stage can be **bypassed**, the output can be **tapped after the AGC** or **after the record stage** ("what the device recorded"), and the reported latency follows the configuration. `reconfigure` changes settings between streams.

Not yet: every stage beyond the AGC and the codec filters.

### Rust

```rust
use rr_dr60::{Pipeline, Settings};

let mut p = Pipeline::new(Settings::new(48_000))?;   // AGC and both filter stages on, output after playback
println!("latency: {} samples", p.latency_samples());
p.process_in_place(&mut buffer);                     // in your audio callback; any block length
```

### C / Swift

```c
#include "rr_dr60.h"

RrDr60Settings s = rr_dr60_settings_default(48000);
RrDr60Pipeline *p = NULL;
if (rr_dr60_create(&s, &p) != RR_DR60_STATUS_OK) { /* handle the error */ }
rr_dr60_process(p, in, out, frames);                 /* real-time safe; in == out is allowed */
rr_dr60_destroy(p);
```

Build the static library with `cargo build -p rr_dr60_ffi --release`. `crates/rr_dr60_ffi/tests/c/run_smoke.sh` shows a complete build and link. From Swift, import `rr_dr60.h` through a bridging header or module map.

### AGC (signal-chain stage 3)

The RR-DR60 is reported to have automatic gain control ([S-004](docs/hardware/sources.md)), as most dictation recorders do. The emulator's AGC raises quiet input and lowers loud input toward a target level. You can hear its side effects: the level **dips and recovers** after a loud sound ("pumping"), and **background noise rises** during pauses. It is **modeled on an assumed AGC**: every default below is a low-confidence assumption ([A-017–A-020](docs/hardware/assumptions.md)) until recordings from a real unit exist.

| Setting | Default | Range |
|---|---|---|
| On / bypassed | on (A-020: assumed always active) | — |
| Target level | −10 dBFS (A-017) | −30 to 0 dBFS |
| Maximum gain | +40 dB (A-017) | 0 to +60 dB |
| Maximum attenuation | 20 dB (A-017) | 0 to 40 dB |
| Attack time | 10 ms (A-018) | 1 to 100 ms |
| Release time | 1 s (A-018) | 50 ms to 10 s |

Inside its working range the AGC has a fixed 10:1 slope: output rises 1 dB for every 10 dB of input. Levels follow the AES17 convention, so a full-scale sine is 0 dBFS. Out-of-range values are rejected when the pipeline is created, and the error names the setting.

> **Output can exceed full scale.** After the pipeline is created or reset, or after a long silence, the gain sits at its maximum. A loud first sound therefore comes out up to the maximum gain too hot for about one attack time: +40 dB (×100) with the defaults, +60 dB at most. The emulator never clips. **Limit or clip the output yourself before converting to 16-bit or other integer formats.** Inputs above 0 dBFS are valid, because the AGC sits in front of the ADC in the real device.

```rust
use rr_dr60::{Pipeline, Settings, Tap};

let mut s = Settings::new(48_000);
s.agc.enabled = false;            // bypass the AGC (the spec 001 sound)
// s.agc.release_ms = 3000.0;     // or: recover more slowly after loud sounds
// s.tap = Tap::AfterAgc;         // or: hear the AGC alone, without the codec filters
let mut p = Pipeline::new(s)?;
```

```c
RrDr60Settings s = rr_dr60_settings_default(48000);
s.agc_release_ms = 3000.0f;               /* slower recovery; s.agc_enabled = false bypasses */
RrDr60SettingField field;
if (rr_dr60_settings_validate(&s, &field) != RR_DR60_STATUS_OK) { /* field names the bad setting */ }
```

### Accuracy of this slice

The AGC is **modeled on** an *assumed* AGC, and the two filter stages on the MSM7702 codec's *assumed* telephone-band filters. A measurement harness ([001](specs/001-pipeline-skeleton/quickstart.md), [002](specs/002-agc/quickstart.md)) checks the filter stages' 212 properties (band edges, ripple, rejection, latency) and the AGC's 1162 (regulation, gain limits, attack and release, distortion, noise rise, latency; 1734 in the full settings matrix) against the specs' tolerances at every supported rate. That proves the emulator does what the spec says. It does **not** prove that the spec matches a real RR-DR60, because no real-unit recordings exist yet. See [Accuracy](#accuracy) above for how to help.

## Planned features

- Portable **Rust** library (`rr_dr60`) with a stable **C API** for use from Swift/iOS, C/C++, and other languages
- Offline (file) processing and **real-time** streaming from one deterministic engine
- Each stage configurable, bypassable, and tap-able (e.g. "codec only")
- Swift Package / XCFramework distribution for iOS (an iOS app is planned as a separate project)

## Development

This project uses [GitHub Spec-Kit](https://github.com/github/spec-kit) for spec-driven development. Specs live in `specs/`, and the project principles live in [.specify/memory/constitution.md](.specify/memory/constitution.md). See [CONTRIBUTING.md](CONTRIBUTING.md) to get started.

## License

[MIT](LICENSE) © 2026 Chance Houston

*Panasonic and RR-DR60 are trademarks of their respective owners. This is an independent project, not affiliated with or endorsed by Panasonic.*
