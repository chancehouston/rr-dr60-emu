# rr-dr60-emu

**A portable software emulator of the Panasonic RR-DR60 digital voice recorder's complete audio signal chain, from microphone to speaker.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
![Status: pre-alpha](https://img.shields.io/badge/status-pre--alpha-orange)

> ⚠️ **Pre-alpha.** The project is in its specification phase. There's no usable code yet.

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
