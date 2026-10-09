# rr-dr60-emu

**A portable software emulator of the Panasonic RR-DR60 digital voice recorder's complete audio signal chain, from microphone to speaker.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
![Status: pre-alpha](https://img.shields.io/badge/status-pre--alpha-orange)

> ⚠️ **Pre-alpha (MVP).** The first slice works: audio passes through the device's two voice-band
> codec stages (record and playback). The other stages (mic, AGC, VAS, speech codec, speaker) aren't built yet,
> and the API may still change before 1.0.

## What is this?

The Panasonic RR-DR60 is a mid-1990s pocket IC voice recorder. Long after it was discontinued, it became legendary among paranormal investigators as an "EVP recorder", and working units are now sought after on the second-hand market.

`rr-dr60-emu` recreates that recorder's sound in software. Audio goes in, and you get back what an RR-DR60 would have recorded and played back. The emulator models each stage of the device:

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

## Status: MVP

What exists today ([spec 001](specs/001-pipeline-skeleton/spec.md)):

- Mono audio at **8, 16, 44.1, 48, 88.2 or 96 kHz** goes in, in blocks of any size, and the same number of samples comes out at the same rate.
- Inside, the audio is converted to the device's internal 8 kHz rate and passes through two band-limiting stages. They model the record and playback filters of the recorder's MSM7702 voice-band codec, *assumed* to follow a telephone-style 300–3400 Hz band. They're designed to the spec's tolerances but haven't been measured against a real unit ([A-002, A-014–A-016](docs/hardware/assumptions.md)).
- Fixed, reported latency: about 11.3 ms at 48 kHz.
- Deterministic and real-time safe: no allocation, locks or I/O while processing, and bit-identical output for any block size.
- A C API with a generated header ([`rr_dr60.h`](crates/rr_dr60_ffi/include/rr_dr60.h)).

- Each stage can be **bypassed**, the output can be **tapped after the record stage** ("what the device recorded"), and the reported latency follows the configuration. `reconfigure` changes settings between streams.

Not yet: every stage beyond the codec filters.

### Rust

```rust
use rr_dr60::{Pipeline, Settings};

let mut p = Pipeline::new(Settings::new(48_000))?;   // both stages on, output after playback
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
