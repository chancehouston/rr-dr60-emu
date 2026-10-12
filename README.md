# rr-dr60-emu

**A portable software emulator of the Panasonic RR-DR60 digital voice recorder's complete audio signal chain, from microphone to speaker.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
![Status: pre-alpha](https://img.shields.io/badge/status-pre--alpha-orange)

> ⚠️ **Pre-alpha.** Audio passes through the record-path limiter (the device's AGC), the codec's
> full-scale clamp, the voice-activated recording (VAS) and the device's two voice-band codec
> stages (record and playback). The other stages (mic, preamp, speech codec, speaker) aren't
> built yet, and the API may still change before 1.0.

## What is this?

The Panasonic RR-DR60 is a mid-1990s pocket IC voice recorder. Long after it was discontinued, it became legendary among paranormal investigators as an "EVP recorder", and working units are now sought after on the second-hand market.

`rr-dr60-emu` aims to recreate that recorder's sound in software: audio goes in, and the goal is to get back what an RR-DR60 would have recorded and played back. The emulator models each stage of the device:

microphone → preamp → automatic gain control → voice-band codec (A/D) → voice-activated recording (VAS) → speech compression → storage → decompression → codec (D/A) → amplifier → speaker

The details of each stage are in [docs/hardware/signal-chain.md](docs/hardware/signal-chain.md).

### Who is it for?

- **App developers** (including ghost-hunting and EVP apps) who want a sound modeled on the RR-DR60 without sourcing vintage hardware.
- **Audiophiles and vintage-gear enthusiasts** curious about the device's sonic character.

### A note on claims

This project is **neutral and technical**. It emulates what the recorder's electronics do to sound: band-limiting, noise, limiting and clipping of loud sounds, voice-activation gating, and speech-codec artifacts. It makes no claims, for or against, about paranormal phenomena.

## Accuracy

Exact specifications for the RR-DR60 aren't fully public. Where they're unknown, we make **reasonable, documented assumptions** and record each one in the [assumption register](docs/hardware/assumptions.md) along with the [sources](docs/hardware/sources.md) behind it. The v1 goal is a **measured character match**: frequency response, noise, AGC and VAS behavior, and codec artifacts within stated tolerances. Bit-exact reproduction isn't a goal.

**Own a working RR-DR60?** Recordings of test signals from a real unit are the most valuable contribution you can make. Open a [Hardware evidence](https://github.com/chancehouston/rr-dr60-emu/issues/new/choose) issue.

### Fidelity status by stage

What each modeled stage rests on today. "Documented" means the behavior is taken from the device's own documentation (service manual, datasheets); "assumed" means an educated guess registered in the assumption register; nothing is **verified** against a real unit yet.

| Stage | Built? | Basis | Notes |
|---|---|---|---|
| Codec band-limiting (record and playback filters) | yes ([spec 004](specs/004-device-rate-6khz/spec.md)) | documented rate (A-026, verified from the service manual); datasheet templates scaled to it (A-027, assumed, medium confidence) | Revised by spec 004: the stages run at the device's **6 kHz** rate and follow the codec family datasheet's transmit band-pass and receive low-pass templates scaled to that clock (band about 225–2550 Hz, unity gain at 765 Hz, minimum-phase). Not measured against a real unit; the response between 2550 Hz and 3 kHz is a design choice no source constrains, and the device's own aliasing and DAC imaging above the band are not modeled. |
| AGC (a limiter) | yes ([spec 005](specs/005-limiter-agc/spec.md)) | modeled on the circuit (S-002, A-028); the dead time, release and polarity are estimates (A-034–A-036), low confidence on timing | A peak-detecting limiter just below the codec's full scale: nothing below the knee, a hard ceiling above it after a few milliseconds of clipped onset, recovery over about 1.4 s. Modeled on the service manual's schematic with estimated transistor parameters; not measured against a unit. A real unit may clip at full scale instead of holding just below it. |
| ADC full-scale clamp | yes ([spec 005](specs/005-limiter-agc/spec.md)) | documented (A-030: the codec's full-scale code) | Samples beyond full scale saturate after the record filter; this gives loud onsets their clipped start. Not measured. |
| VAS (voice-activated pausing) | yes ([spec 003](specs/003-vas/spec.md)) | documented behavior (owner's manual), assumed values (A-022–A-025) | Threshold (−40 dBFS at level 3, re-anchored by spec 005), hang and onset times are guesses until captures exist. |
| Microphone and mic amp | no | documented topology (A-005, A-006, A-029) | Gain and noise are estimates from the circuit. |
| µ-law quantization | no | documented (A-003) | |
| CELP speech codec, 4 kbit/s | no | documented bitrate and type (A-004); the exact codec is unknown | The stage most responsible for the device's sound; the hardest to get right. |
| Flash memory (capacity and recordings) | capacity only ([spec 006](specs/006-record-playback-split/spec.md)) | documented figures (60 minutes, 99 recordings, S-001); the accounting assumed (A-038, A-039) | A memory the recordings go into and come out of, with the device's limits: recording stops at the limit, numbering and erase as the owner's manual describes. The stored form is the engine's own 32-bit samples until the codec is modeled, so it says nothing about the device's sound. |
| Speaker amp, speaker, earphone path | no | documented topology (A-031, A-032); speaker acoustics assumed (A-010) | Spec 006 reserves the speaker/earphone **route** as a selectable label (the two routes are exclusive on the device, S-001); it does not change the sound yet. |

Nothing in this table claims the emulator sounds like a real RR-DR60 yet. It claims that each stage is built to a stated, traceable description, and that the harness checks the build against that description.

## Using the library

What exists today ([spec 001](specs/001-pipeline-skeleton/spec.md), [spec 003](specs/003-vas/spec.md), [spec 004](specs/004-device-rate-6khz/spec.md), [spec 005](specs/005-limiter-agc/spec.md), which replaced [spec 002](specs/002-agc/spec.md)'s AGC model, and [spec 006](specs/006-record-playback-split/spec.md)). **What changed from 0.5:** the [CHANGELOG](CHANGELOG.md)'s 0.6.0 entry lists the new recording and playback calls, the one inaudible output change and the C structure change.

- Mono audio at **6, 8, 16, 44.1, 48, 88.2 or 96 kHz** goes in, in blocks of any size, and audio comes out at the same rate. By default the output can be **shorter** than the input, because the VAS drops pauses; each call reports how many samples it produced.
- Inside, the audio is converted to the device's internal **6 kHz** rate (verified from the service manual, [A-026](docs/hardware/assumptions.md); 6 kHz input passes through unconverted) and goes through two band-limiting stages that model the record and playback filters of the recorder's MSM7702 voice-band codec. They follow the codec family datasheet's transmit band-pass and receive low-pass templates scaled to the 6 kHz clock ([A-027](docs/hardware/assumptions.md), medium confidence): flat within about 0.2 dB from 225 to 2250 Hz, 0.31 dB (record) and 0.48 dB (playback) down at 2550 Hz against datasheet typicals of 0.4 and 0.56 dB, the record filter removing DC and the playback filter passing it, both minimum-phase ([A-016](docs/hardware/assumptions.md)). They are modeled on the datasheet scaled to the measured clock, not measured against a real unit; the roll-off between 2550 Hz and 3 kHz is a design choice no source constrains, and the device's own aliasing of 3–3.45 kHz input and its DAC imaging above the band are not modeled (the emulator removes both).
- A record-path **limiter**, the device's automatic gain control (AGC), on by default, and the codec's **full-scale clamp**. See [AGC](#agc-signal-chain-stage-3) below.
- A record-path **Voice Activated System** (VAS), on by default: recording pauses while the signal stays below a threshold, and the paused audio is removed. See [VAS](#vas-signal-chain-stage-5) below.
- **Recording and playback like the device:** a record path into a memory of 60 minutes and 99 recordings, and a playback path out of it, with numbering, erase, export and import. See [Recording and playback](#recording-and-playback) below. The one-pass processing call stays for hosts that only want the sound.
- Fixed, reported latency: 483 samples (about 10.1 ms) at 48 kHz in the default configuration, and 1 sample at 6 kHz. It is under 20 ms at every supported rate, and each configuration reports its own; the "after AGC" tap reports 0.
- **Real-time safe:** `process`, `process_in_place`, `process_with_events` and `reset` never allocate, lock or do I/O, and neither do the recording and playback calls (`start_recording`, `record`, `stop_recording`, `start_playback`, `play`, `stop_playback`, `set_route`, the state and figure reads, `export_len`, the latency reads). `erase`, `erase_all` (a bounded move of stored samples), `export` and `import` (a checksum over the recording) are not. Processing runs at about 360× real time at 48 kHz on a laptop (58 ns per sample) with the limiter and the VAS on, and the same with the limiter alone or with neither: below its knee the limiter costs one exponential per sample and nothing more. `Pipeline::new` and `reconfigure` allocate, so call them outside the audio callback.
- **Deterministic:** the same input and settings give bit-identical output for any block size, and on every supported platform. Three golden files (001, AGC and VAS) are checked on Linux, macOS and Windows (x86-64 and ARM64) and on iOS.
- A C API with a generated header ([`rr_dr60.h`](crates/rr_dr60_ffi/include/rr_dr60.h)).

- Each stage can be **bypassed**, the output can be **tapped after the AGC** (the limiter's own output at the host rate), **after the record stage** or **after the VAS** ("what the device recorded"), and the reported latency follows the configuration. `reconfigure` changes settings between streams.

Not yet: every stage beyond the limiter, the clamp, the VAS, the codec filters and the memory's capacity; the speaker and earphone routes are labels.

### Rust

```rust
use rr_dr60::{Pipeline, Settings};

let mut p = Pipeline::new(Settings::new(48_000))?;   // device defaults: AGC, VAS and both filter stages on
println!("latency: {} samples", p.latency_samples());
let info = p.process_in_place(&mut buffer);          // in your audio callback; any block length
let out = &buffer[..info.produced];                  // VAS drops pauses: output can be shorter
```

### C / Swift

```c
#include "rr_dr60.h"

RrDr60Settings s = rr_dr60_settings_default(48000);
RrDr60Pipeline *p = NULL;
if (rr_dr60_create(&s, &p) != RR_DR60_STATUS_OK) { /* handle the error */ }
RrDr60BlockInfo info;
rr_dr60_process(p, in, out, frames, &info);          /* real-time safe; in == out is allowed */
/* only out[0 .. info.produced) is output: the VAS drops pauses by default */
rr_dr60_destroy(p);
```

Build the static library with `cargo build -p rr_dr60_ffi --release`. `crates/rr_dr60_ffi/tests/c/run_smoke.sh` shows a complete build and link. From Swift, import `rr_dr60.h` through a bridging header or module map.

### Recording and playback

Since [spec 006](specs/006-record-playback-split/spec.md) the engine also works the way the device does: a **record path** (the limiter, the record filter and clamp, the VAS) writes into a **memory**, and a **playback path** (the playback filter) plays a recording back. The one-pass call above is the two paths back to back.

- **State.** The engine is idle, recording or playing, never two at once (the device mutes its speaker while recording). A call that needs another state is refused with a reason that names the activity; nothing changes on a refusal, and the one-pass call is refused while a recording or playback is in progress.
- **The calls, in order.** `start_recording`, then `record` blocks of any size (each reports how many device samples it stored, its VAS events, and whether the memory filled), then `stop_recording`, which flushes the record path's latency so every fed sample is in the recording and reports the recording's number and length (or that it was empty and discarded, when no input was fed). `start_playback(number)`, then `play` into your buffer until it reports `finished`: the recording, then a short drain of settled silence (`drain_samples`). `stop_playback` ends it at once. `state`, `memory_status` and `recording_length` read at any time.
- **The memory.** 60 minutes and 99 recordings, counted in stored device time (owner's manual, S-001; [A-038](docs/hardware/assumptions.md)); both are settings, and a capacity above the device's is a configuration the device does not have. Recording stops by itself at the exact sample at which the memory fills and keeps the partial recording ([A-039](docs/hardware/assumptions.md)); afterwards the engine is idle. A start is refused when no time or no slot remains. Paused time costs nothing in drop mode; in mute mode the silence is stored and counts. Recordings are numbered in the order made; `erase` returns a recording's time and renumbers the later ones, `erase_all` empties the memory. The store for the whole capacity is **reserved when the engine is created**, about 86 MB at the defaults, virtual until recordings are written (a commit charge on Windows), so starting a recording never allocates.
- **Export and import.** `export_len` then `export` give a recording as an opaque byte sequence; `import` puts one back, into the same or another engine of this version at any host rate, with bit-identical playback. The host never parses the bytes. The form is versioned (version 1 is the record path's 32-bit device-rate output; it will become encoded frames when the codec is modeled, [A-004](docs/hardware/assumptions.md)) and carries a checksum, so a truncated, altered or foreign sequence is refused with a reason, as is one that does not fit the remaining memory.
- **The route.** `set_route` selects the speaker or the earphone at any time, during playback too. It is a label in this version: both routes produce the same output until the amplifier, speaker and earphone stages are modeled.
- **Same sound.** A recording played back equals what the one-pass call produces for the same input followed by the record latency of silence, bit for bit, at every supported rate; the harness proves it against the committed golden files. Each recording and each playback starts from the path's reset state ([A-037](docs/hardware/assumptions.md): whether a real unit carries the limiter's charge between items is an open question).

```rust
use rr_dr60::{Pipeline, Settings, StopResult};

let mut p = Pipeline::new(Settings::new(48_000))?;      // reserves the 60-minute store (virtual)
p.start_recording()?;
let info = p.record(&block, &mut events)?;               // in the audio callback; any block length
if info.stopped.is_some() { /* the memory filled: the recording is kept, the engine idle */ }
let outcome = p.stop_recording(&mut events)?;           // flushes the record latency
if let StopResult::Kept { number, length } = outcome.result {
    p.start_playback(number)?;
    while !p.play(&mut out)?.finished { /* out holds the next block */ }
    let mut bytes = vec![0u8; p.export_len(number)?];   // outside the callback
    p.export(number, &mut bytes)?;
}
# Ok::<(), rr_dr60::Error>(())
```

In C the same calls are `rr_dr60_start_recording`, `rr_dr60_record`, `rr_dr60_stop_recording`, `rr_dr60_start_playback`, `rr_dr60_play`, `rr_dr60_get_state`, `rr_dr60_memory_status`, `rr_dr60_erase`, `rr_dr60_export_len`, `rr_dr60_export`, `rr_dr60_import` and `rr_dr60_set_route`, each refusal a distinct status; `crates/rr_dr60_ffi/tests/c/smoke.c` runs the whole loop.

### AGC (signal-chain stage 3)

The RR-DR60's record path has an automatic gain control, and the service manual's schematic ([S-002](docs/hardware/sources.md)) shows what kind: a **limiter**, not a compressor. It does nothing at all until the signal at the codec input reaches about full scale, then holds the peaks there. The emulator models that circuit ([A-028](docs/hardware/assumptions.md); spec 005):

- **Quiet input passes untouched.** Below the knee the limiter is the identity, bit for bit: quiet passages are not raised, nothing happens in pauses, and background noise does not rise.
- **Loud input is clipped, then held.** A loud onset passes at full level for a few milliseconds (about 3 ms when it is 10 dB over the knee, longer for smaller overshoots) and the codec clips it meanwhile; then the limiter holds the output at a ceiling about 1 dB below full scale, a clean sine, for the rest of the loud sound. Afterwards the gain recovers over about 1.4 s, and for a few seconds the limiter stays "armed": a new loud onset is limited within about half a millisecond (the dead time is back to two thirds of its full value after 10 s).
- **It hears everything the host rate carries.** The limiter runs before the rate conversion, so a loud sound above the device's band (a key click, a close sibilant) still pulls the gain down although the band-limited output contains none of it.

**Input level.** 0 dBFS (a full-scale sine) is the codec's full scale, about 0.196 V peak at the microphone amplifier's output ([A-030](docs/hardware/assumptions.md), [A-029](docs/hardware/assumptions.md)). Where speech sits against it depends on the mic chain's gain, the least certain number in the register: roughly 10–35 dB below full scale at a conversational distance (full scale at about 80–100 dB SPL), so with the default knee the limiter acts on loud sounds (shouting, a clap, a close-talked microphone) and, if the real gain is at the high end of the estimate, on the peaks of ordinary speech. Inputs above 0 dBFS are within its normal operating range. To hear it on everything, move the knee down.

| Setting | Default | Range |
|---|---|---|
| On / bypassed | on (A-020: assumed always active) | — |
| Knee | −1 dBFS, just below full scale (A-028) | −40 to +3 dBFS |
| Attack (the dead time at 10 dB over the knee) | 3 ms (A-034) | 1 to 50 ms |
| Release (to within 1 dB of unity from 10 dB of attenuation) | 1.37 s (A-035) | 0.2 to 10 s |
| ADC clamp | on (A-030) | on / off |

Levels follow the AES17 convention, so a full-scale sine is 0 dBFS. Out-of-range values are rejected when the pipeline is created, and the error names the setting. The spec 002 settings (target level, maximum gain, maximum attenuation) no longer exist; see the CHANGELOG's 0.5.0 entry.

> **Modeled on the circuit, not measured.** The component values are the schematic's; the transistor parameters are estimates, so the dead time is uncertain by about 3× either way, the release by about 40 % and the knee by ±2 dB ([A-028, A-034–A-036](docs/hardware/assumptions.md)). The most audible open question: with the knee modeled 1 dB below full scale, sustained loud input comes out as a clean sine held just below full scale and only the onset clips; **a real unit may instead clip at full scale, flat-topping sustained loud sound for part of every cycle** (the model's own transistor parameters favor the first, which is why the default sits there). The deciding capture is the recorded waveform of a tone held at a fixed loud level; it and the others are listed in the [hardware evidence](https://github.com/chancehouston/rr-dr60-emu/issues/new/choose) issue template.

> **The record path is clamped at full scale; the default output is not.** The ADC clamp bounds the "after record" and "after VAS" taps at ±1.0, but the playback band-limit stage reconstructs a clipped onset with overshoot (the fundamental of a full-scale square wave is 4/π), so the default output can exceed ±1.0 by up to about 3 dB there (about 1.4 for an onset 12 dB over the knee at 48 kHz). The emulator never clips its output; **limit or clip it yourself before converting to 16-bit or other integer formats.** With the clamp bypassed, nothing bounds the record path either.

```rust
use rr_dr60::{Pipeline, Settings, Tap};

let mut s = Settings::new(48_000);
s.agc.enabled = false;            // bypass the limiter (the spec 001 sound)
// s.agc.knee_dbfs = -6.0;        // or: move the knee down so ordinary speech reaches it
// s.agc.release_ms = 4000.0;     // or: recover more slowly after loud sounds
// s.adc_clamp_enabled = false;   // or: no full-scale clamp (the un-clipped limiter)
// s.tap = Tap::AfterAgc;         // or: hear the limiter alone, at the host rate, latency 0
let mut p = Pipeline::new(s)?;
```

```c
RrDr60Settings s = rr_dr60_settings_default(48000);
s.agc_knee_dbfs = -6.0f;                  /* lower knee; s.agc_enabled = false bypasses */
s.adc_clamp_enabled = false;              /* no full-scale clamp */
RrDr60SettingField field;
if (rr_dr60_settings_validate(&s, &field) != RR_DR60_STATUS_OK) { /* field names the bad setting */ }
```

### VAS (signal-chain stage 5)

The owner's manual ([S-001](docs/hardware/sources.md)) describes the Voice Activated System: "Recording automatically pauses when no sound is detected. This avoids blank portions in recordings. The function works better the lower the microphone sensitivity level." The device has no VAS switch, so the emulator's VAS is **on by default**. Its character comes from what it removes: pauses longer than the hang time vanish from the recording, the first part of each sound after a pause is lost (clipped onsets), and the remaining audio is joined at abrupt splices. It is **modeled on the manual's description and assumed values** ([A-008, A-021–A-025](docs/hardware/assumptions.md)): every number below is a low-confidence assumption until recordings from a real unit exist.

| Setting | Default | Range |
|---|---|---|
| On / bypassed | on (S-001, A-008: the device has no VAS switch) | — |
| Output mode | drop (S-001, A-008) | drop, mute |
| Sensitivity level | 3 (S-001, A-021) | 1 to 5 |
| Threshold at level 3 | −40 dBFS (A-022, revised by spec 005) | −60 to 0 dBFS |
| Hang time | 1 s (A-023) | 50 ms to 10 s |
| Onset time | 20 ms (A-024) | 0 to 200 ms |

> **The output can be shorter than the input.** In drop mode (the default), paused audio is removed, so a block can produce fewer samples than it consumed, or none. Always read `produced` (`BlockInfo::produced` in Rust, `RrDr60BlockInfo.produced` in C) and use only `output[..produced]`. **Mute mode** keeps one output sample per input sample and replaces paused audio with digital silence instead; it is an emulator option for real-time hosts that need a fixed block length, not a device behavior. Both modes make the same pause and resume decisions, and both report where they acted: drop mode reports each splice's output position and the length of input removed there, mute mode each muted region's start and length.

The threshold is compared with the signal's peaks *after* the limiter (stage 3), the record band-limit (stage 4) and the clamp. Below the limiter's knee the record path is unity gain, so the threshold is input-referred one to one: −40 dBFS at sensitivity level 3, and −34, −37, −40, −43 and −46 dBFS at levels 1 to 5. On the level scale above, that is about 40 dB below the knee: quiet speech at a conversational distance stays above it, a quiet room's noise floor falls below it, and a louder room keeps recording at level 3 but pauses at level 1 ([A-022](docs/hardware/assumptions.md), revised by spec 005; the placement inherits the level anchor's uncertainty of about ±10 dB, and 6 dB per level is the recorded alternative). In this version the sensitivity level moves only the VAS threshold (3 dB per level; higher levels record quieter sounds). The service manual places the sensitivity setting in the DSP, after the AGC (A-033), so it cannot change the AGC's behavior; whether it also applies a digital gain before the encoder, which would change the recorded level per level, is still open.

```rust
use rr_dr60::{Pipeline, Settings, Tap, VasEvent, VasMode};

let mut s = Settings::new(48_000);
s.vas.enabled = false;            // bypass the VAS (limiter and band-limiting only, fixed length)
// s.vas.mode = VasMode::Mute;    // or: keep the length, replacing pauses with silence
// s.vas.sensitivity = 5;         // or: record quieter sounds (1 records only louder sounds)
// s.tap = Tap::AfterVas;         // or: take the output right after the VAS
let mut p = Pipeline::new(s)?;

// Read how much each block produced and where the VAS acted.
let mut events = vec![VasEvent::default(); p.max_events(input.len())];  // outside the callback
let info = p.process_with_events(&input, &mut output, &mut events)?;
let out = &output[..info.produced];
for e in &events[..info.events] {
    // drop mode: a splice at output e.output_position removed e.input_length input samples
    // mute mode: a muted region of e.input_length samples starts at e.output_position
}
```

```c
RrDr60Settings s = rr_dr60_settings_default(48000);
s.vas_mode = RR_DR60_VAS_MODE_MUTE;         /* fixed length; s.vas_enabled = false bypasses */
s.vas_sensitivity = 5;                       /* record quieter sounds */
/* ... rr_dr60_create(&s, &p) ... */
size_t capacity;
rr_dr60_max_events(p, frames, &capacity);    /* always enough; allocate `events` outside the callback */
RrDr60BlockInfo info;
rr_dr60_process_with_events(p, in, out, frames, &info, events, capacity);
/* out[0 .. info.produced) is output; events[0 .. info.events) says where the VAS acted */
```

### Accuracy of this slice

The limiter is **modeled on** the service manual's circuit with *estimated* transistor parameters, the clamp on the codec's documented full scale, the VAS on the owner's manual's description plus *assumed* values, and the two filter stages on the codec family datasheet's templates scaled to the device's measured 6 kHz clock (*assumed* scaling). A measurement harness ([001](specs/001-pipeline-skeleton/quickstart.md), [003](specs/003-vas/quickstart.md), [004](specs/004-device-rate-6khz/quickstart.md), [005](specs/005-limiter-agc/quickstart.md)) checks the filter stages' 413 properties (template loss, DC, group-delay distortion, anti-alias rejection, cascade, minimum phase, boundary flatness and rejection, latency), the limiter's 563 (bit-exact passthrough, the static curve and knee, dead time and arming, release, polarity, bandwidth, the clamp, latency, at seven rates; plus a 290-row settings matrix at 6 and 48 kHz, 1578 rows in the full matrix at seven rates), the three chain-level scenarios of the limiter and the VAS together, the VAS's acceptance scenarios (output lengths, splice positions, threshold, hang and onset times, mute mode), and the record and playback paths' equivalence with the one-pass processing (196 golden entries bit-exact through record-then-play, the length and drain rules, the memory's limits at the exact sample, the export round trip at 7 × 7 rates and an 85-sequence corruption set) against the specs' tolerances at every supported rate. That proves the emulator does what the spec says. It does **not** prove that the spec matches a real RR-DR60, because no real-unit recordings exist yet. See [Accuracy](#accuracy) above for how to help.

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
