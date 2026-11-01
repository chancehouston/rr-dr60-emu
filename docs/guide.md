# rr-dr60-emu user guide

The reference for using the `rr_dr60` library: what the engine does, how to call it, and what each modeled stage of the signal chain does to sound, with its settings, figures and limits. The [README](../README.md) is the short introduction; this guide describes version **0.14.0**. The [CHANGELOG](../CHANGELOG.md) lists what changed in each version, including every change to an assumed hardware value.

**Contents**

- [The engine at a glance](#the-engine-at-a-glance)
- [Rust](#rust) · [C / Swift](#c--swift)
- [Recording and playback](#recording-and-playback)
- [What the engine says about itself](#what-the-engine-says-about-itself) (the stage manifest)
- The stages: [Microphone (1 – 2)](#microphone-signal-chain-stages-1--2) · [AGC (3)](#agc-signal-chain-stage-3) · [VAS (5)](#vas-signal-chain-stage-5) · [Speech codec (7, 9)](#speech-codec-signal-chain-stages-7-and-9) · [Playback speed](#playback-speed-the-speed-control) · [Output chain (11 – 13)](#output-chain-signal-chain-stages-11--13)
- [What the measurement harness checks](#what-the-measurement-harness-checks)

The band-limiting codec stages (4 and 10) are described in [The engine at a glance](#the-engine-at-a-glance) below. Noise reduction (stage 6), the clock (14) and the power supply (15) are not modeled; [docs/hardware/signal-chain.md](hardware/signal-chain.md) explains why for each.

## The engine at a glance

- **Audio in, audio out.** Mono audio at **6, 8, 16, 44.1, 48, 88.2 or 96 kHz** goes in, in blocks of any size, and audio comes out at the same rate. By default the output can be **shorter** than the input, because the VAS drops pauses; each call reports how many samples it produced.
- **The device's rate inside.** The audio is converted to the device's internal **6 kHz** rate (verified from the service manual, [A-026](hardware/assumptions.md); 6 kHz input passes through unconverted) and goes through two band-limiting stages that model the record and playback filters of the recorder's MSM7702 voice-band codec (stages 4 and 10). They follow the codec family datasheet's transmit band-pass and receive low-pass templates scaled to the 6 kHz clock ([A-027](hardware/assumptions.md), medium confidence): flat within about 0.2 dB from 225 to 2250 Hz, 0.31 dB (record) and 0.48 dB (playback) down at 2550 Hz against datasheet typicals of 0.4 and 0.56 dB, the record filter removing DC and the playback filter passing it, both minimum-phase ([A-016](hardware/assumptions.md)). They are modeled on the datasheet scaled to the measured clock, not measured against a real unit; the roll-off between 2550 Hz and 3 kHz is a design choice no source constrains, and the device's own aliasing of 3–3.45 kHz input and its DAC imaging above the band are not modeled (the emulator removes both).
- **The record path**, every stage on by default: the [microphone](#microphone-signal-chain-stages-1--2) (the capsule's hiss and the amplifier's input high-pass near 818 Hz and asymmetric clipper), the [limiter](#agc-signal-chain-stage-3) (the device's automatic gain control) and the codec's full-scale clamp, the [Voice Activated System](#vas-signal-chain-stage-5) (recording pauses while the signal stays below a threshold, and the paused audio is removed), and the [speech encoder](#speech-codec-signal-chain-stages-7-and-9) (4 kbit/s frames from a CELP-class coder of the period, modeled on the class, not on the device's own coder).
- **The memory and the playback path:** a memory of 60 minutes and 99 recordings with numbering, erase, export and import ([Recording and playback](#recording-and-playback)), then the speech decoder, the [playback speed control](#playback-speed-the-speed-control), the playback filter and the [output chain](#output-chain-signal-chain-stages-11--13) (volume control, power amplifier, and the speaker or earphone route). The one-pass processing call runs the record and playback paths back to back, for hosts that only want the sound.
- **Bypasses and taps.** Each modeled stage can be bypassed, and the output can be tapped after the microphone capsule or amplifier, the AGC (the limiter's own output at the host rate), the record stage, the VAS ("what the device recorded", before the codec), the decoder, the playback filter, the volume and amplifier stage, the speaker or the earphone (`Tap`); the default is the route output. The reported latency follows the configuration. `reconfigure` changes settings between streams.
- **What the engine says about itself:** a constant [stage manifest](#what-the-engine-says-about-itself) of the fifteen signal-chain stages, each with its status, confidence, source and assumption IDs, what the engine leaves out, and the version that last changed its model, so an app's "about this model" view is generated from the engine.
- **Latency.** Fixed and reported: 1923 samples (about 40.1 ms) at 48 kHz in the default configuration, 483 (about 10.1 ms) with the speech codec bypassed; 181 and 1 at 6 kHz. It is under 50 ms at every supported rate, and each configuration reports its own; the microphone and limiter taps report 0.
- **Real-time safe.** `process`, `process_in_place`, `process_with_events` and `reset` never allocate, lock or do I/O, and neither do the recording and playback calls (`start_recording`, `record`, `stop_recording`, `start_playback`, `play`, `stop_playback`), the live controls (`set_route`, `set_volume`, `set_playback_speed` and their reads), the scheduled changes (`schedule_change`, `pending_changes`, `stream_position`, `clear_schedule`, the log-taking `process_with_events_and_changes`, `process_in_place_with_events_and_changes` and `play_with_changes`, `recording_counter`, `changes::speed_landing`; since 0.17 `input_position`, `record_with_changes`, `stop_recording_with_changes`, `changes::clock_of` and `changes::record_landing`), the state and figure reads, the meter read-out (`meters`, `rest_meters`), the battery value (`set_battery_volts`, `battery_volts`, `battery::ceiling_factor`, `battery::ceiling_db`; since 0.18), `export_len`, `playback_length_samples` and the latency reads. `erase`, `erase_all` (a bounded move of stored samples), `export` and `import` (a checksum over the recording) are not. `Pipeline::new` and `reconfigure` allocate, so call them outside the audio callback. Since 0.21 the four playback-stage switches through `schedule_change` and `output::stage11_bypass_level_db` (no handle) are real-time safe too.
- **Cost.** Measured on a laptop at 48 kHz: about 60 ns per sample (about 350× real time) with the limiter and the VAS on and the speech codec bypassed; below its knee the limiter costs one exponential per sample and nothing more. With the speech codec on it runs at about 135× real time at 48 kHz (about 155 ns per host sample, 1.2 µs per device sample against the 8 µs budget of spec 008; the worst 64-sample block takes about 0.14 ms of its 1.33 ms) and about 210× at 6 kHz. Recording and playing back through the memory costs about 2 % more than the one-pass call. The output chain adds about 19 % on the speaker route and 6 % on the earphone route to the codec-bypassed one-pass (68.2 and 60.9 against 57.4 ns per host sample; spec 009 SC-007), the microphone path about 4 ns (6 %) more, and the playback speed control about 10 % at the levels other than normal speed (about 1.5 % at normal speed).
- **Deterministic.** The same input, settings and seed give bit-identical output for any block size and on every supported platform. The golden files (one per feature; the speech codec's entries carry a digest of the frame bytes as well as of the audio) are checked on Linux, macOS and Windows (x86-64 and ARM64), in the iOS simulator and on an iOS device.
- **A C API** with a generated header ([`rr_dr60.h`](../crates/rr_dr60_ffi/include/rr_dr60.h)), and a [Swift package](../swift/README.md) that wraps it as an XCFramework.

Not yet: noise reduction (stage 6) and the clock (14); the power supply (15) is modeled only as the [battery's effect on the speaker amplifier's clip ceiling](#battery-state-the-speaker-amplifiers-supply); the stage manifest lists what each modeled stage leaves out.

## Rust

```rust
use rr_dr60::{Pipeline, Settings};

let mut p = Pipeline::new(Settings::new(48_000))?;   // device defaults: every modeled stage on
println!("latency: {} samples", p.latency_samples());
let info = p.process_in_place(&mut buffer);          // in your audio callback; any block length
let out = &buffer[..info.produced];                  // VAS drops pauses: output can be shorter
```

## C / Swift

```c
#include "rr_dr60.h"

if (rr_dr60_abi_check(RR_DR60_ABI_VERSION, sizeof(void *), sizeof(RrDr60Settings), sizeof(RrDr60VasEvent),
                      sizeof(RrDr60ChangeEntry)) != RR_DR60_STATUS_OK) { /* this build of the app is broken */ }
RrDr60Settings s = rr_dr60_settings_default(48000);
RrDr60Pipeline *p = NULL;
if (rr_dr60_create(&s, &p) != RR_DR60_STATUS_OK) { /* handle the error */ }
RrDr60BlockInfo info = { .struct_size = sizeof info };   /* every struct the library fills starts like this (0.22) */
rr_dr60_process(p, in, out, frames, &info);          /* real-time safe; in == out is allowed */
/* only out[0 .. info.produced) is output: the VAS drops pauses by default */
rr_dr60_destroy(p);
```

Build the static library with `cargo build -p rr_dr60_ffi --release`. `crates/rr_dr60_ffi/tests/c/run_smoke.sh` shows a complete build and link. From Swift, use the [Swift package](../swift/README.md) (`scripts/build-xcframework.sh`, then `import RRDR60` for the helpers `.sized` and `RRDR60.checkABI()` and the processing record's re-render; `import RRDR60C` alone is the bare C API).

**The C ABI since 0.22 (the pre-1.0 ABI pass, spec 022).** The C ABI is 64-bit pointer width only (iOS; Android `arm64-v8a` and `x86_64`): the FFI crate does not build for a 32-bit target (a const assertion), and the check's pointer-size argument guards a header/compiler mismatch.

- **Every struct the library fills starts with `uint32_t struct_size`**: the eleven reports and reads that lacked it (`RrDr60BlockInfo` 40 bytes, `RrDr60RecordInfo` 48, `RrDr60StopOutcome` 88, `RrDr60PlayInfo` 48, `RrDr60EngineState` 20, `RrDr60MemoryStatus` 48, `RrDr60SpeechCodecInfo` 28, `RrDr60ManifestHeader` 72, `RrDr60StageEntry` 80, `RrDr60Control` 120, `RrDr60ControlOption` 24) and the four that had it (`RrDr60Meters`, `RrDr60RecordReadout`, `RrDr60InputAlignment`, `RrDr60PlaceState`), under one rule: set it to `sizeof` before the call; below the smallest layout ever published for the struct, or above `RR_DR60_MAX_STRUCT_SIZE` (4096), the call returns `RR_DR60_STATUS_INVALID_ARGUMENT` and writes nothing; a known smaller layout receives exactly its prefix; a larger size receives the whole struct with the tail zeroed; `struct_size` reads back as the size the library filled, so a caller newer than the library sees which fields are meaningful and reads zeros beyond as "unknown to this library"; every padding byte is zero. A report with `struct_size` 0 or uninitialized is refused **at run time, not at compile time**; Swift's `RrDr60BlockInfo()` zero-initializes it, so use `.sized`. In a prefix fill of `RrDr60RecordReadout` the embedded `settings.struct_size` is the settings bytes the prefix holds.
- **The per-call reports** (`rr_dr60_process*`, `rr_dr60_record*`, `rr_dr60_stop_recording*`, `rr_dr60_play*`): a non-NULL report's size is checked first; with a NULL or poisoned handle the handle's status is returned and the idle report written only when the size is valid; otherwise a refused size processes nothing (the stream's position, the memory and the meters are untouched); with an accepted size the report is always written, on every error as before. The handle-taking status reads (`rr_dr60_get_state`, `rr_dr60_memory_status`, `rr_dr60_meters`) check the handle, then NULL, then the size; the constant-data reads check NULL, then the size, then the index.
- **The inputs.** `RrDr60Settings` is validated whole: a `struct_size` different from `sizeof(RrDr60Settings)` is refused at `rr_dr60_create`, `rr_dr60_reconfigure` and `rr_dr60_settings_validate` (before 0.22 a larger one was accepted and its tail ignored). It is the one exception to the growth promise below: appending a setting is a rebuild for earlier callers and moves `RR_DR60_ABI_VERSION`. `RrDr60RunData`, and the read-out as an input: below their size or above the cap refused on read. `RrDr60StopFields`: frozen in layout; its size is written as 40 and ignored on read. `rr_dr60_processing_record_read` checks NULL out, NULL bytes, the size, then the handle.
- **Frozen**: `RrDr60VasEvent` (`RR_DR60_VAS_EVENT_SIZE`, 16) and `RrDr60ChangeEntry` (`RR_DR60_CHANGE_ENTRY_SIZE`, 40; its last field is `stream`, named `reserved` from 0.16 to 0.21: the same offset and meaning). A different layout would be a new type with new functions, never a change to these.
- **The check.** `RR_DR60_ABI_VERSION` (1), `rr_dr60_abi_version()` and `rr_dr60_abi_check(abi_version, pointer_size, settings_size, vas_event_size, change_entry_size)`: `RR_DR60_STATUS_OK` for this library's version, pointer width 8, the settings size equal to the library's and the two frozen sizes, else `RR_DR60_STATUS_ABI_MISMATCH` (37); pure, no handle. Call it once at start as the example does. The library ships inside the app, so a mismatch is a build error of the app (a stale XCFramework, a mixed build), never a user condition: stop in a debug build; in a release build show that the app's build is broken and must be updated, and do not create an engine. The version moves on a change no size field can express (a frozen struct's bytes, a function's signature or removal, an enum value's meaning, a field reorder, a settings growth or a change of its input rule) and never on an appended status, control, tap, define, function or a field appended to a struct the library fills.
- **The promise from 0.22.** A struct the library fills only grows under its size field, the frozen pair does not change, new statuses, controls, taps, defines and functions are appended; `RrDr60Settings` is the one exception. Anything else moves `RR_DR60_ABI_VERSION`, is announced as breaking, and none is planned before 1.0.

**Migrating a 0.21 caller** (0.22 is breaking for every C and Swift caller: rebuild once and initialize every report):

1. Rebuild against `rr_dr60.h` 0.22 (the XCFramework: `scripts/build-xcframework.sh`).
2. Every struct the library fills starts as `{ .struct_size = sizeof x }` (Swift: `.sized`); grep for the eleven names above. The structs that already had a size field are unchanged in use.
3. `outcome.flush.<x>` → `outcome.flush_<x>` (the stop outcome's flush is inline: `flush_stored`, `flush_events`, `flush_paused`, `flush_stopped`, `flush_stop_reason`, `flush_frames`, `flush_frames_total`); `entry.reserved` → `entry.stream`.
4. Arrays of `RrDr60VasEvent` and `RrDr60ChangeEntry` need no change (frozen).
5. Call `rr_dr60_abi_check(...)` once at start (Swift: `try RRDR60.checkABI()`); treat a failure as a build error of your app, not a user condition.
6. A `RrDr60Meters` caller that passed a `struct_size` above 4096 (none known) is now refused; the read-back is the filled size (56 or 64). A settings struct larger than the library's is now refused at `rr_dr60_create`.

## Recording and playback

Since [spec 006](../specs/006-record-playback-split/spec.md) the engine also has the device's structure: a **record path** (the microphone, the limiter, the record filter and clamp, the VAS and the speech encoder) writes into a **memory**, and a **playback path** (the speech decoder, the speed control, the playback filter and the output chain) plays a recording back. The one-pass call above is the two paths back to back.

- **State.** The engine is idle, recording or playing, never two at once (the device mutes its speaker while recording). A call that needs another state is refused with a reason that names the activity; nothing changes on a refusal, and the one-pass call is refused while a recording or playback is in progress.
- **The calls, in order.** `start_recording`, then `record` blocks of any size (each reports how many device samples it stored, its VAS events, and whether the memory filled), then `stop_recording`, which flushes the record path's latency so every fed sample is in the recording and reports the recording's number and length (or that it was empty and discarded, when no input was fed). `start_playback(number)`, then `play` into your buffer until it reports `finished`: the recording, then a short drain of settled silence (`drain_samples`: about 78 ms at 6 kHz and 88 ms at the other rates by default, the output chain's settle included; 20 ms at 6 kHz and about 30 ms at the other rates with the output chain bypassed). `stop_playback` ends it at once. `state`, `memory_status` and `recording_length` read at any time.
- **The memory.** 60 minutes and 99 recordings, counted in stored device time (owner's manual, S-001; [A-038](hardware/assumptions.md)); both are settings, and a capacity above the device's is a configuration the device does not have. Recording stops by itself at the exact sample at which the memory fills and keeps the partial recording ([A-039](hardware/assumptions.md)); afterwards the engine is idle. A start is refused when no time or no slot remains. Paused time costs nothing in drop mode; in mute mode the silence is stored and counts. Recordings are numbered in the order made; `erase` returns a recording's time and renumbers the later ones, `erase_all` empties the memory. The store for the whole capacity is **reserved when the engine is created**, about 1.8 MB with the speech codec on (86 MB when it is bypassed), virtual until recordings are written (a commit charge on Windows), so starting a recording never allocates. Pages once written stay resident until the engine is destroyed or reconfigured to another capacity; erasing does not return them (on iOS they count as dirty memory).
- **Export and import.** `export_len` then `export` give a recording as an opaque byte sequence; `import` puts one back, into the same or another engine of this version at any host rate, with bit-identical playback. The host never parses the bytes. The form is versioned (version 2: the codec's frames, or the record path's 32-bit device-rate output when the codec is bypassed, with the codec model version declared; see [Speech codec](#speech-codec-signal-chain-stages-7-and-9)) and carries a checksum, so a truncated, altered or foreign sequence is refused with a reason, as is one that does not fit the remaining memory or whose encoding is not the engine's.
- **The route.** `set_route` selects the speaker or the earphone at any time, during playback too. Since 0.9 each route has its own sound: see [Output chain](#output-chain-signal-chain-stages-11--13).
- **Same output as the one-pass call.** A recording played back equals what the one-pass call produces for the same input followed by the record latency of silence, bit for bit, at every supported rate; the harness proves it against the committed golden files. Each recording and each playback starts from the path's reset state ([A-037](hardware/assumptions.md): whether a real unit carries the limiter's charge between items is an open question).

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

## What the engine says about itself

Since [spec 007](../specs/007-stage-manifest/spec.md) the library carries a **stage manifest**: constant data describing the fifteen stages of the signal chain ([docs/hardware/signal-chain.md](hardware/signal-chain.md)), one entry per stage under a header. Each entry gives the stage's number and a stable identifier, a name, one sentence on what the stage does to sound **on the device**, its status (not modeled, assumed, sourced, checked against a real unit), which path it sits on, the project's confidence, the source and assumption IDs its model rests on, a note on what the engine does **not** model there, the Rust API items that touch it, and the library version in which its model last changed. The header carries the library version, the 6 kHz device rate, the counts by status, one basis sentence and three links: the hardware-evidence form and the two register pages. Reading the manifest allocates nothing and needs no engine; it is the same on every platform.

Two things the strings are not: they describe the device, not the model (what the model leaves out is in the separate "not modeled here" note, so a card must label both), and they make no claim about what recordings contain. The identifiers (`mic_capsule`, `mic_amp`, `limiter`, `codec_input`, `vas`, `noise_reduction`, `encoder`, `memory`, `decoder`, `codec_output`, `volume_amp`, `speaker`, `earphone`, `clock`, `power`) are the keys for localization; the English text may change in a minor version, the identifiers and numbers may not. The register links are page links; to land near an ID append a text fragment, `#:~:text=A-028`, which scrolls to the ID's first mention on the page.

A harness test keeps the manifest consistent with the registers: every cited ID must exist and not be superseded, every current assumption and source must be cited, the signal-chain table's identifiers and its **Modeled by** marker must agree with each entry, and a wording check enforces the length rules and a list of words the project avoids. The test runs in CI on every change to the registers, the CHANGELOG or this file (`.github/workflows/registers.yml`). The guard binds the manifest to the registers; the code is bound to them by a unit test that touches each modeled stage's bypass control, by the traceability script and by review.

The Rust program below prints the whole manifest in its canonical text form (it is `crates/rr_dr60_harness/examples/manifest.rs`, verbatim):

<!-- manifest-example-rust:start -->
```rust
//! Prints the stage manifest in its canonical text form (spec 007; data-model.md › Canonical
//! rendering). Self-contained: the README's Rust example is this file verbatim.
//!
//! Run: `cargo run -q -p rr_dr60_harness --example manifest`

use rr_dr60::{Confidence, StagePath, StageStatus, manifest};

fn status(s: StageStatus) -> &'static str {
    match s {
        StageStatus::NotModeled => "not_modeled",
        StageStatus::Assumed => "assumed",
        StageStatus::Sourced => "sourced",
        StageStatus::Checked => "checked",
        _ => "unknown",
    }
}

fn path(p: StagePath) -> &'static str {
    match p {
        StagePath::Record => "record",
        StagePath::Playback => "playback",
        StagePath::Both => "both",
        _ => "unknown",
    }
}

fn confidence(c: Confidence) -> &'static str {
    match c {
        Confidence::NotApplicable => "not_applicable",
        Confidence::Low => "low",
        Confidence::Medium => "medium",
        Confidence::High => "high",
        _ => "unknown",
    }
}

/// Prints `key: value`, or `key:` with nothing after the colon when the value is empty.
fn line(key: &str, value: &str) {
    if value.is_empty() {
        println!("{key}:");
    } else {
        println!("{key}: {value}");
    }
}

fn main() {
    let m = manifest();
    let h = &m.header;
    println!("rr_dr60 manifest");
    println!("library_version: {}", h.library_version);
    println!("device_rate_hz: {}", h.device_rate_hz);
    println!("stage_count: {}", h.stage_count);
    println!(
        "counts: not_modeled={} assumed={} sourced={} checked={}",
        h.counts.not_modeled, h.counts.assumed, h.counts.sourced, h.counts.checked
    );
    println!("basis: {}", h.basis);
    println!("evidence_link: {}", h.evidence_link);
    println!("assumptions_link: {}", h.assumptions_link);
    println!("sources_link: {}", h.sources_link);
    for s in m.stages() {
        println!();
        println!("stage {} {}", s.number, s.identifier);
        println!("name: {}", s.name);
        println!("description: {}", s.description);
        println!("status: {}", status(s.status));
        println!("path: {}", path(s.path));
        println!("confidence: {}", confidence(s.confidence));
        line("sources", &s.sources.join(", "));
        line("assumptions", &s.assumptions.join(", "));
        line("coverage", s.coverage);
        line("controls", s.controls);
        line("model_version", s.model_version);
    }
}
```
<!-- manifest-example-rust:end -->

The same from C (the `print_manifest` function of `crates/rr_dr60_ffi/tests/c/smoke.c`, verbatim; `smoke --manifest` prints exactly what the Rust program prints):

<!-- manifest-example-c:start -->
```c
/* The stage manifest (spec 007): the fifteen signal-chain stages and what the engine says
 * about each, printed in the canonical text form. Needs no handle; every string pointer is
 * static for the life of the process. */
static const char *status_word(uint32_t s) {
  switch (s) {
    case RR_DR60_STAGE_STATUS_NOT_MODELED: return "not_modeled";
    case RR_DR60_STAGE_STATUS_ASSUMED: return "assumed";
    case RR_DR60_STAGE_STATUS_SOURCED: return "sourced";
    case RR_DR60_STAGE_STATUS_CHECKED: return "checked";
    default: return "unknown";
  }
}

static const char *path_word(uint32_t p) {
  switch (p) {
    case RR_DR60_STAGE_PATH_RECORD: return "record";
    case RR_DR60_STAGE_PATH_PLAYBACK: return "playback";
    case RR_DR60_STAGE_PATH_BOTH: return "both";
    default: return "unknown";
  }
}

static const char *confidence_word(uint32_t c) {
  switch (c) {
    case RR_DR60_CONFIDENCE_NOT_APPLICABLE: return "not_applicable";
    case RR_DR60_CONFIDENCE_LOW: return "low";
    case RR_DR60_CONFIDENCE_MEDIUM: return "medium";
    case RR_DR60_CONFIDENCE_HIGH: return "high";
    default: return "unknown";
  }
}

/* Prints "key: value", or "key:" with nothing after the colon when the value is empty. */
static void manifest_line(const char *key, const char *value) {
  if (value[0]) printf("%s: %s\n", key, value);
  else printf("%s:\n", key);
}

static void print_manifest(void) {
  RrDr60ManifestHeader h = { .struct_size = sizeof h }; /* every struct the library fills starts like this (0.22) */
  if (rr_dr60_manifest_header(&h) != RR_DR60_STATUS_OK) return;
  printf("rr_dr60 manifest\n");
  manifest_line("library_version", h.library_version);
  printf("device_rate_hz: %u\n", h.device_rate_hz);
  printf("stage_count: %u\n", h.stage_count);
  printf("counts: not_modeled=%u assumed=%u sourced=%u checked=%u\n", h.count_not_modeled, h.count_assumed,
         h.count_sourced, h.count_checked);
  manifest_line("basis", h.basis);
  manifest_line("evidence_link", h.evidence_link);
  manifest_line("assumptions_link", h.assumptions_link);
  manifest_line("sources_link", h.sources_link);
  for (uint32_t n = 1; n <= rr_dr60_manifest_stage_count(); n++) {
    RrDr60StageEntry e = { .struct_size = sizeof e };
    if (rr_dr60_manifest_stage(n, &e) != RR_DR60_STATUS_OK) return;
    printf("\nstage %u %s\n", e.number, e.identifier);
    manifest_line("name", e.name);
    manifest_line("description", e.description);
    manifest_line("status", status_word(e.status));
    manifest_line("path", path_word(e.path));
    manifest_line("confidence", confidence_word(e.confidence));
    printf("sources:");
    for (uint32_t i = 0; i < e.source_count; i++) {
      const char *id = NULL;
      if (rr_dr60_manifest_stage_source(n, i, &id) == RR_DR60_STATUS_OK) printf("%s%s", i ? ", " : " ", id);
    }
    printf("\nassumptions:");
    for (uint32_t i = 0; i < e.assumption_count; i++) {
      const char *id = NULL;
      if (rr_dr60_manifest_stage_assumption(n, i, &id) == RR_DR60_STATUS_OK) printf("%s%s", i ? ", " : " ", id);
    }
    printf("\n");
    manifest_line("coverage", e.coverage);
    manifest_line("controls", e.controls);
    manifest_line("model_version", e.model_version);
  }
}
```
<!-- manifest-example-c:end -->

C names derive from the Rust names: `RrDr60Settings` fields are the Rust field path in snake case with the group as prefix (`agc.knee_dbfs` → `agc_knee_dbfs`, `vas.hang_ms` → `vas_hang_ms`, `memory.capacity_s` → `memory_capacity_s`), top-level fields keep their names (`record_stage_enabled`), enum variants are `RR_DR60_` plus the type and the variant in screaming snake case (`Tap::AfterAgc` → `RR_DR60_TAP_AFTER_AGC`, `Route::Speaker` → `RR_DR60_ROUTE_SPEAKER`), functions are `rr_dr60_` plus the method name, and the value readers `route` and `state` take `get_` (`rr_dr60_get_route`, `rr_dr60_get_state`); `EngineState.speaker_muted` is `RrDr60EngineState.speaker_muted`. The manifest's own C functions are `rr_dr60_manifest_stage_count`, `rr_dr60_manifest_header`, `rr_dr60_manifest_stage`, `rr_dr60_manifest_stage_source` and `rr_dr60_manifest_stage_assumption`; a stage number outside 1 to 15 is refused with `RR_DR60_STATUS_NO_SUCH_STAGE`, and every string pointer stays valid for the life of the process.

## The controls the engine accepts

Since [spec 014](../specs/014-control-manifest/spec.md) the library also carries a **control manifest**: constant data describing every adjustable setting and live control, 31 today (the three live controls, then every field of `Settings` in declaration order, then the battery value appended by spec 018 and the playback speed mode by spec 023). An app builds its sliders, switches and pickers from it instead of copying ranges and defaults by hand. Each entry gives a stable identifier (the setting's path, `vas.sensitivity`, or the live control's name, `volume`; the key for localization), a name, its **kind**, its **timing**, the paths it applies to, its signal-chain stage (0 for none), its value (a switch; an integer with exact `u64` bounds; a continuous `f32` range; or a choice whose options carry the value the engine takes), its default, its unit, the source and assumption IDs it rests on, and one sentence on what changing it does to the sound. Each sentence describes the default engine configuration, every other stage on and the speech coder on, which is what an app user hears; with other stages switched off a control can act differently (with the coder bypassed, for example, switching the converter's clipping off lets loud peaks through).

The **kinds**: a *device control* is one the owner's manual or the service manual gives the user (route, volume, playback speed, microphone sensitivity); a *unit condition* is an audible difference an owner could hear between two units or over one unit's life (the hiss level; component spreads that are inaudible or not directly hearable stay model internals); a *model internal* is a value the device has that its user cannot reach (the limiter's threshold, attack and release; the pause threshold, delay and resume delay) or a choice between candidate models of a mechanism the device has but no document describes (the playback speed mode); an *emulator option* is something the device cannot do (the stage switches, the pause handling, the memory size, the output clamp, the listen point); a *host parameter* is set by the host (the seed, the host sample rate). The **timing classes**: *live* (a `set_*` call while running; a live control's value is kept by `reset` and `reconfigure`), *idle* (applied by `reconfigure` when nothing records or plays), *idle, memory-dependent* (also refused while the stored recordings do not fit: the speech coder switch and the memory figures) and *host-fixed* (the host rate, matched to the audio session).

The app's free and Workbench line is an app rule, not an engine field: the device controls and unit conditions are the free knobs; curated free presets may set any value; the Workbench gives knobs for the rest (the owner's decisions of 2026-10-29). To show a control's evidence status, join it to the stage manifest by stage number; a stage-0 control has none and is labelled by its kind.

Tests keep the control manifest honest: every range, default and option is checked against the engine's validation, `Settings::new` at all seven host rates and the C header; every control with a stage (1 – 15) appears in that stage's controls note above, and every control a note names has that stage; every ID exists, is current and equals the tags on the default's line in the code; the kind rules hold (a device control cites S-001 or S-002, a unit condition or model internal cites an assumption); and a new setting or `set_*` method without an entry fails the build's tests. `cargo run -p rr_dr60_harness --example control_manifest` prints the canonical text (`crates/rr_dr60_harness/golden/control-manifest-v1.txt`).

Rust:

```rust
for c in rr_dr60::controls() {
    println!("{} ({:?}, {:?}): {}", c.identifier, c.kind, c.timing, c.description);
}
let speed = rr_dr60::control("playback_speed").unwrap();
```

C:

```c
for (uint32_t i = 0; i < rr_dr60_control_count(); i++) {
  RrDr60Control c;
  if (rr_dr60_control(i, &c) != RR_DR60_STATUS_OK) break;
  printf("%s: %s\n", c.identifier, c.description);
  for (uint32_t k = 0; k < c.option_count; k++) {
    RrDr60ControlOption o;
    rr_dr60_control_option(i, k, &o); /* o.identifier, o.value */
  }
}
```

An index past any list is refused with `RR_DR60_STATUS_NO_SUCH_CONTROL` and nothing is written. `RrDr60Control` (120 bytes) and `RrDr60ControlOption` (24) start with `struct_size` since 0.22 (the [C / Swift](#c--swift) rule).

**Reading and setting controls by index** (since 0.91). `rr_dr60_get_settings` writes the settings the engine is configured with, landed scheduled values included, so an app shows what is applied rather than what it asked for (a value a stream has not reached yet, such as a playback landing the one-pass takes at its next call, is already in it). `rr_dr60_control_value` returns a control's current value in its manifest type: during any transition, blend or wait (a volume, battery or hiss transition, a stage bypass blend, a speed or mode change waiting for a crossfade) it reads the requested value, not what is sounding. `rr_dr60_settings_control_value` and `rr_dr60_set_settings_control_value` read and write a settings structure's field by the same index. The write checks only that the value fits the control's type (a known option, a finite number, an integer the field can hold; −0.0 is stored as 0.0); ranges are checked once, by `rr_dr60_settings_validate`, which names the field, and `rr_dr60_setting_field_control` turns that field into the control's index. The read refuses only a `bool` byte other than 0 or 1; every other stored value comes back as stored, so a repair screen can show a bad `tap` or host rate, which the write or the validator then refuses. `rr_dr60_set_control_value` sets the five live controls that have a plain setter (route, volume, playback speed, speed mode, battery voltage) through their setters, with their validation and timing (by index −0.0 is stored as 0.0). A generated control surface is a loop over `rr_dr60_control_count()`: read the entry and its value; when idle, edit a copy of the settings by index, validate, `rr_dr60_reconfigure` (refused while recording or playing), then read the settings back; during a recording or playback, change a setting whose manifest timing is live with `rr_dr60_schedule_change` at the stream's position, passing the reading's value as a double (a switch 0 or 1, a choice's engine value, the integer, the float). A read back confirms a `reconfigure` exactly; that a live value took effect is confirmed by its change-log entry, since a read holds what is configured, not what has sounded. A set to the value already applied logs nothing, so compare with `rr_dr60_control_value` before waiting for an entry. The manifest's kind tells an app which controls to present; access by index does not. C callers store 0 or 1 in `bool` fields. Store presets and sessions by control identifier, never by index: indices follow the manifest's order, identifiers are the stable names.

The reading (`RrDr60ControlReading`, 24 bytes, `struct_size` first) carries its `value_type` and the value in `int_value` (a switch 0 or 1, an integer, a choice's engine value) or `float_value` (a continuous value). The refusals, in order and with nothing written: a NULL or poisoned handle; a NULL pointer; a refused size (a settings structure must be exactly `sizeof(RrDr60Settings)`); an index past the count (`RR_DR60_STATUS_NO_SUCH_CONTROL`); the wrong class (`RR_DR60_STATUS_NOT_A_SETTING` 39 for a live control on a settings structure, `RR_DR60_STATUS_NOT_A_PLAIN_LIVE_CONTROL` 40 for a setting on the engine); a value that does not fit (`RR_DR60_STATUS_INVALID_ARGUMENT`); then the setter's own status (`INVALID_VOLUME`, `INVALID_SPEED_LEVEL`, `INVALID_BATTERY_VOLTS`). By index a NaN volume is `INVALID_ARGUMENT`, where `rr_dr60_set_volume` returns `INVALID_VOLUME`. In Rust: `Pipeline::control_value` / `set_control_value` and `control_map::{settings_value, set_settings_value, setting_control}`; in Swift: `ControlReading`, `ControlIndex(identifier:)`, `RRDR60.controlValue`, `RRDR60.setControlValue`, `RRDR60.appliedSettings` and `RrDr60Settings.value(index:)` / `setValue(index:_:)`.

```c
RrDr60Settings edit = rr_dr60_settings_default(48000);
RrDr60ControlReading r = { .struct_size = sizeof r, .value_type = RR_DR60_CONTROL_VALUE_TYPE_CONTINUOUS,
                           .float_value = -6.0f };
rr_dr60_set_settings_control_value(&edit, knee_index, &r);   /* the index from the manifest loop */
RrDr60SettingField field;
if (rr_dr60_settings_validate(&edit, &field) == RR_DR60_STATUS_OK) rr_dr60_reconfigure(p, &edit);
RrDr60Settings applied = { .struct_size = sizeof applied };
rr_dr60_get_settings(p, &applied);                             /* what the engine now runs with */
```

## Microphone (signal-chain stages 1 – 2)

<!-- mic-section:start -->
Since 0.12 the record path starts at the microphone: the capsule and its two-stage transistor amplifier on the service manual's schematic ([S-002](hardware/sources.md)), ahead of the limiter ([spec 012](../specs/012-mic-capsule-amp/spec.md)). Both stages are **on by default**, so the default output changes: the sound is thinner below about 1 kHz, a quiet room carries a steady hiss, a DC offset in the input no longer reaches the limiter, and very loud sounds clip in the amplifier before the limiter acts. Each stage has its own switch (`mic.capsule_enabled`, `mic.amp_enabled`; `mic_capsule_enabled`, `mic_amp_enabled` in C) and its own tap (`Tap::AfterMicCapsule`, `Tap::AfterMicAmp`, both at the host rate with latency 0); `MicSettings::BYPASSED` gives the 0.11 engine. **The figures are modeled on the circuit with typical part values, not measured**: no real unit has been recorded ([A-046 – A-053](hardware/assumptions.md)). From the schematic: the parts, their values and the DC voltages (S-002). Assumed: the capsule's self-noise (a class figure for small electret capsules of the period, A-048, low confidence), the full-scale sound level (A-029, low), the transistor parameters behind the corner's value (A-046, medium) and the clipping's shape (A-051, fitted to a circuit simulation with generic transistor models, low to medium).

- **Input level.** The engine's input reference is unchanged: 0 dBFS (a full-scale sine) at 1 kHz is the level that reaches the codec's full scale, and the two stages are normalized to unity gain at 1 kHz. On the device that corresponds to about **90 dB SPL** at the microphone (80–100 dB SPL, low confidence; [A-029](hardware/assumptions.md)). To feed a calibrated microphone: engine level (dBFS) ≈ sound level (dB SPL) − 90. If your microphone reads M dBFS for a 94 dB SPL calibrator tone at 1 kHz, apply a gain of (94 − 90) − M dB before the engine: one that reads −26 dBFS at 94 dB SPL needs +30 dB. The limiter's knee, the clip points below and the voice-activation thresholds are input-referred one to one **at 1 kHz only**; at other frequencies the input high-pass sits in between.
- **The input high-pass** ([A-046](hardware/assumptions.md)). The capsule's signal reaches the amplifier through a 0.15 µF capacitor that works into the 1 kΩ bias resistor and the amplifier's virtual ground: a first-order high-pass near **818 Hz** (665–985 Hz over the parts' tolerances). Relative to 1 kHz, 300 Hz reads **−7.3 dB** and 2.5 kHz +1.7 dB: voices lose body and gain presence. After the clipper an output coupling near 77 Hz removes rumble and DC, and a roll-off near 12.9 kHz removes what lies above the audio band, both ahead of the limiter ([A-047](hardware/assumptions.md), [A-053](hardware/assumptions.md)).
- **The hiss** ([A-048](hardware/assumptions.md), [A-049](hardware/assumptions.md)). The capsule's self-noise is a steady Gaussian hiss at about **−62 dBFS** (RMS, AES17, in the 300–3000 Hz band after stage 2), shaped by the same band. The credible range is about −66 … −53 dBFS (capsules of −35 … −44 dBV/Pa with a signal-to-noise ratio of 58 … 62 dB through the circuit's level chain); the default corresponds to a capsule of about −42 dBV/Pa. In the default engine silence is no longer silent: a recording of a quiet room carries the hiss, which the emulator's speech codec (a class match, not the device's coder, [A-004](hardware/assumptions.md)) renders about 5 dB lower. `mic.hiss_offset_db` moves it by −18 … +12 dB (`mic::HISS_OFFSET_RANGE_DB`; values outside are refused with the invalid-setting error); `mic.capsule_enabled = false` removes it. **If your host microphone already carries its own floor at or above about −65 dBFS** at the engine's input (RMS, AES17, 300–3000 Hz, after your gain), turn the hiss down or off, or the two floors add (two equal floors read 3 dB above either).
- **The voice activation on the hiss.** The hiss sits well below every voice-activation threshold (an RMS margin of about 17 dB at level 5, the most sensitive, and 29 dB at level 1), so in a silent room the default recording keeps only the **hang time** (1 s) of hiss and then pauses until a sound arrives. Raising the hiss changes that: at level 5 the gate stops pausing from an offset of about **+9 dB** (the engine's bisection in 10 s silent runs reads +8.8 … +9.5 dB by host rate; near that offset a longer run can still pause late, so treat it as a lower estimate; the design script's margin rule of thumb says about +10.5 dB); at level 4 it still pauses at +12 dB at most host rates (at 44.1 and 96 kHz it stops from about +11.9 dB).
- **To record without pauses**, do not rely on the hiss: near the stop offset a long run can still pause. Set `vas.mode = VasMode::Mute` (the paused audio is kept as silence, so the output length equals the input length) or `vas.enabled = false` (the gate is bypassed and every sample is stored as it arrives). The device has no VAS switch (S-001), so both are emulator conveniences, not device configurations.
- **Clipping** ([A-051](hardware/assumptions.md)). A sound loud enough to overdrive the amplifier is limited there, before the limiter can act, and asymmetrically (at 1 kHz at the engine input): **negative samples first and softly, toward about +9.15 dBFS** (Q1's base–collector junction starts to conduct across the feedback resistor, so the compression begins near +6.8 dBFS and creeps toward the limit), positive samples harder, at about **+17.10 dBFS** where Q2 reaches the supply rail (compression from about +16.1 dBFS). The limits and knees are fitted to a circuit simulation of the schematic (ngspice, generic transistor models; within 0.5 dB from 0 to +30 dBFS). The asymmetry adds even harmonics, and the output coupling re-centres the clipped waveform within about 12 ms (after a 50 ms burst at +13 dBFS; longer for heavier overload). The clipper runs at the host rate, so its harmonics fold back at low rates: **for loud material use a host rate of at least 16 kHz** (at 6 and 8 kHz the 2nd harmonic reads up to about 2 dB away from a continuous-time clipper's; at 6 kHz the 3rd falls on Nyquist). Nothing after stage 2 exceeds L₊ + L₋ (about +20 dBFS), so the limiter's range is never exceeded.
- **Hiss streams and the seed.** The hiss is deterministic: the engine's `seed` and a **recording counter** (the recordings started since construction; `reset` and `reconfigure` keep it, a refused start does not advance it) select its stream, so each recording carries its own hiss, and the same seed, history and input give bit-identical output on every platform. The one-pass call uses the next recording's stream. The default seed is 0, so a new engine with the defaults carries the same hiss as any other; with the one-pass call alone the counter stays put, and `reset` restarts the same stream (`reconfigure` with a new seed gives a new one). **Set a different seed per session** (from the session number or the time, outside the engine) when recordings from different sessions must not share hiss; the engine never reads the clock itself.
- **Not modeled:** the capsule's own response (flat, [A-052](hardware/assumptions.md)), handling noise, wind, the capsule's acoustic overload, directivity, the amplifier's distortion below clipping, the output coupling's move to 159 Hz under deep limiting, and the parts' unit-to-unit spread. The amplifier's own noise (a floor near −73 dBFS) is neglected in the hiss figure (it would add about 0.3 dB); the codec's reference noise is unknown ([A-050](hardware/assumptions.md)).

Cost: the two stages add about 4 ns per host sample at 48 kHz on the development machine (about 6 % of the codec-bypassed one-pass); bypassed, they cost nothing that the timing runs can resolve.

```rust
use rr_dr60::{MicSettings, Pipeline, Settings, Tap};

let mut s = Settings::new(48_000);
s.mic.hiss_offset_db = -6.0;          // a quieter capsule
// s.mic.capsule_enabled = false;     // or: no hiss (your microphone brings its own floor)
// s.mic = MicSettings::BYPASSED;     // or: the 0.11 engine, whose input is the amplifier's output
// s.tap = Tap::AfterMicAmp;          // or: hear stages 1–2 alone, at the host rate, latency 0
s.seed = session_seed;                // chosen by the app per session, e.g. from a session number
let mut p = Pipeline::new(s)?;
```

```c
RrDr60Settings s = rr_dr60_settings_default(48000);
s.mic_hiss_offset_db = -6.0f;     /* s.mic_capsule_enabled = false removes the hiss */
s.seed = session_seed;            /* chosen by the app per session, e.g. from a session number */
```
<!-- mic-section:end -->

## AGC (signal-chain stage 3)

The RR-DR60's record path has an automatic gain control, and the service manual's schematic ([S-002](hardware/sources.md)) shows what kind: a **limiter**, not a compressor. It does nothing at all until the signal at the codec input reaches about full scale, then holds the peaks there. The emulator models that circuit ([A-028](hardware/assumptions.md); spec 005):

- **Quiet input passes untouched.** Below the knee the limiter is the identity, bit for bit: quiet passages are not raised, nothing happens in pauses, and background noise does not rise.
- **Loud input is clipped, then held.** A loud onset passes at full level for a few milliseconds (about 3 ms when it is 10 dB over the knee, longer for smaller overshoots) and the codec clips it meanwhile; then the limiter holds the output at a ceiling about 1 dB below full scale, a clean sine, for the rest of the loud sound. Afterwards the gain recovers over about 1.4 s, and for a few seconds the limiter stays "armed": a new loud onset is limited within about half a millisecond (the dead time is back to two thirds of its full value after 10 s).
- **It hears everything the host rate carries.** The limiter runs before the rate conversion, so a loud sound above the device's band (a key click, a close sibilant) still pulls the gain down although the band-limited output contains none of it.

**Input level.** 0 dBFS (a full-scale sine) is the codec's full scale, about 0.196 V peak at the microphone amplifier's output ([A-030](hardware/assumptions.md), [A-029](hardware/assumptions.md)). Where speech sits against it depends on the mic chain's gain, the least certain number in the register: roughly 10–35 dB below full scale at a conversational distance (full scale at about 80–100 dB SPL), so with the default knee the limiter acts on loud sounds (shouting, a clap, a close-talked microphone) and, if the real gain is at the high end of the estimate, on the peaks of ordinary speech. Inputs above 0 dBFS are within its normal operating range. Since 0.12 (stages 1 – 2 on) this reference holds at 1 kHz: the microphone amplifier's input high-pass sits in between at other frequencies, and the amplifier itself limits negative samples from about +7 dBFS (see Microphone). To hear it on everything, move the knee down.

| Setting | Default | Range |
|---|---|---|
| On / bypassed | on (A-020: assumed always active) | — |
| Knee | −1 dBFS, just below full scale (A-028) | −40 to +3 dBFS |
| Attack (the dead time at 10 dB over the knee) | 3 ms (A-034) | 1 to 50 ms |
| Release (to within 1 dB of unity from 10 dB of attenuation) | 1.37 s (A-035) | 0.2 to 10 s |
| ADC clamp | on (A-030) | on / off |

Levels follow the AES17 convention, so a full-scale sine is 0 dBFS. Out-of-range values are rejected when the pipeline is created, and the error names the setting. The spec 002 settings (target level, maximum gain, maximum attenuation) no longer exist; see the CHANGELOG's 0.5.0 entry.

> **Modeled on the circuit, not measured.** The component values are the schematic's; the transistor parameters are estimates, so the dead time is uncertain by about 3× either way, the release by about 40 % and the knee by ±2 dB ([A-028, A-034–A-036](hardware/assumptions.md)). The most audible open question: with the knee modeled 1 dB below full scale, sustained loud input comes out as a clean sine held just below full scale and only the onset clips; **a real unit may instead clip at full scale, flat-topping sustained loud sound for part of every cycle** (the model's own transistor parameters favor the first, which is why the default sits there). The deciding capture is the recorded waveform of a tone held at a fixed loud level; it and the others are listed in the [hardware evidence](https://github.com/chancehouston/rr-dr60-emu/issues/new/choose) issue template.

> **The record path is clamped at full scale; the default output is not, unless you ask.** The ADC clamp bounds the "after record" and "after VAS" taps at ±1.0, but the playback band-limit stage reconstructs a clipped onset with overshoot (the fundamental of a full-scale square wave is 4/π), so the output after stage 10 can exceed ±1.0 by up to about 3 dB (about 1.4 for an onset 12 dB over the knee at 48 kHz), and the 0.11 default's output (stages 1 – 2 bypassed), the route's, reads about 1.96 – 2.06 on the speaker route on a full-scale tone at the top of the volume travel and is never above 1.75 – 2.29 by host rate there (the output section below gives the figures by configuration). By default the engine does not hold its output within ±1.0: **scale it by 0.42 or turn on the output clamp (`output.clamp_enabled`, spec 010) before converting to 16-bit or other integer formats**; see the output section. With the ADC clamp bypassed, nothing bounds the record path either.

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

## VAS (signal-chain stage 5)

The owner's manual ([S-001](hardware/sources.md)) describes the Voice Activated System: "Recording automatically pauses when no sound is detected. This avoids blank portions in recordings. The function works better the lower the microphone sensitivity level." The device has no VAS switch, so the emulator's VAS is **on by default**. Its character comes from what it removes: pauses longer than the hang time vanish from the recording, the first part of each sound after a pause is lost (clipped onsets), and the remaining audio is joined at abrupt splices. It is **modeled on the manual's description and assumed values** ([A-008, A-021–A-025](hardware/assumptions.md)): every number below is a low-confidence assumption until recordings from a real unit exist.

| Setting | Default | Range |
|---|---|---|
| On / bypassed | on (S-001, A-008: the device has no VAS switch) | — |
| Output mode | drop (S-001, A-008) | drop, mute |
| Sensitivity level | 3 (S-001, A-021) | 1 to 5 |
| Threshold at level 3 | −40 dBFS (A-022, revised by spec 005) | −60 to 0 dBFS |
| Hang time | 1 s (A-023) | 50 ms to 10 s |
| Onset time | 20 ms (A-024) | 0 to 200 ms |

> **The output can be shorter than the input.** In drop mode (the default), paused audio is removed, so a block can produce fewer samples than it consumed, or none. Always read `produced` (`BlockInfo::produced` in Rust, `RrDr60BlockInfo.produced` in C) and use only `output[..produced]`. **Mute mode** keeps one output sample per input sample and replaces paused audio with digital silence instead; it is an emulator option for real-time hosts that need a fixed block length, not a device behavior. Both modes make the same pause and resume decisions, and both report where they acted: drop mode reports each splice's output position and the length of input removed there, mute mode each muted region's start and length.

The threshold is compared with the signal's peaks *after* the limiter (stage 3), the record band-limit (stage 4) and the clamp. Below the limiter's knee the record path is unity gain, so the threshold is input-referred one to one (at 1 kHz since 0.12: the microphone amplifier's input high-pass lowers lower frequencies first): −40 dBFS at sensitivity level 3, and −34, −37, −40, −43 and −46 dBFS at levels 1 to 5. On the level scale above, that is about 40 dB below the knee: quiet speech at a conversational distance stays above it, a quiet room's noise floor falls below it, and a louder room keeps recording at level 3 but pauses at level 1 ([A-022](hardware/assumptions.md), revised by spec 005; the placement inherits the level anchor's uncertainty of about ±10 dB, and 6 dB per level is the recorded alternative). In this version the sensitivity level moves only the VAS threshold (3 dB per level; higher levels record quieter sounds). The service manual places the sensitivity setting in the DSP, after the AGC (A-033), so it cannot change the AGC's behavior; whether it also applies a digital gain before the encoder, which would change the recorded level per level, is still open.

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

## Speech codec (signal-chain stages 7 and 9)

<!-- codec-section:start -->
The RR-DR60 compresses what it records: the service manual ([S-002](hardware/sources.md)) specifies a **CELP** speech coder at **4 kbit/s** on the Panasonic MN1931712BB DSP, which is how 16 Mbit of flash hold 60 minutes. Which CELP it is, with what frame length, look-ahead and codebooks, is documented nowhere we have found ([A-004](hardware/assumptions.md)). **So the emulator models the class, not the device's own coder**: a 4 kbit/s CELP-class coder of the period, an ACELP of the G.729 / G.723.1 family adapted to the device's 6 kHz clock, with 20 ms frames of 80 bits and a 10 ms look-ahead ([A-040](hardware/assumptions.md), [A-041](hardware/assumptions.md), both low confidence; [S-008 – S-011](hardware/sources.md) for the class). What it does to sound is what coders of that class do: speech comes back intelligible with the slightly watery, buzzy texture of low-rate CELP, and anything that is not speech (music, noise, tones) is rendered through a speech model.

- **What is modeled.** An 8th-order short-term predictor sent as line spectral pairs, a pitch (adaptive) codebook with fractional lags, G.723.1's 17-bit four-pulse fixed codebook per 10 ms subframe, two gains per subframe with G.729's energy predictor, a perceptual weighting filter in the search and a short-term postfilter with tilt compensation on playback. Every frame is coded the same way: no silence handling, comfort noise or mode switching ([A-041](hardware/assumptions.md)). The VAS, ahead of the coder, is what removes quiet passages; the device's VAS does the same, and whether its coder keeps running through a pause is open ([A-041](hardware/assumptions.md)). The coder is fed the engine's device-rate stream, not the codec's 8-bit µ-law output ([A-003](hardware/assumptions.md)); the µ-law step is far smaller than the coder's own error on speech-level material (near silence the two floors are comparable), so the omission is not expected to be audible. The ITU-T reference software is used by the calibration tool only: it is not part of the library, not a build or run-time dependency, and the library's coder is an independent implementation.
- **Frames.** 120 device samples (20 ms) per frame, 80 bits (10 bytes), 50 frames per second: 4000 bit/s exactly, the payload figure the service manual states (the device's framing overhead is unknown). The codec adds 180 device samples (30 ms: one frame plus the look-ahead) of delay to the record path and to the one-pass call; the playback path adds none. Each `record` and one-pass block reports the frames it completed and the recording's total, each `play` block the frames it decoded and the index of the frame being heard; the read-outs are named "frames".
- **Figures.** The model's figures, measured by the harness ([`us1_codec`](../crates/rr_dr60_harness/tests/us1_codec.rs)) in the codec-only configuration on a synthetic voiced stimulus at −10, −20 and −40 dBFS, beside the same metrics on the ITU-T G.723.1 reference coder at 5.3 kbit/s (the same bit density per sample; [calibration](../specs/008-celp-codec/calibration.md)). They describe the model against a reference coder of its class, **never the device**; the targets are the reference coder's figure plus a margin, and they bound the model, not the device. Spectral figures are computed after resampling both the model's output and its bypassed input reference to 8 kHz, so the model and the reference coder meet the same metric code at the same rate; the level figure is at the host rate. The third-octave figure is a +1 … +2.5 dB lift at 2000 Hz from the postfilter's tilt compensation (the published strength, kept rather than tuned to a synthetic stimulus; the post-implementation review's experiment is in the spec's research notes); the 6 kHz column's stimulus keeps its energy up to 3 kHz where the other hosts' is cut by the rate conversion near 2.7 kHz, so its 2500 Hz band (reported, not a target) reads differently for that reason, not for a rate dependence of the coder.

  | Measure (voiced stimulus) | This model, 6 kHz host | This model, 48 kHz host | G.723.1 reference | Model target |
  |---|---|---|---|---|
  | Output level against the input | −0.4 dB | −0.3 dB | 0.0 dB | ±1.5 dB |
  | Third-octave levels against the input, 315 – 2000 Hz | within 2.0 dB | within 2.1 dB (2.5 dB worst of seven rates) | within 1.2 dB | ±3 dB |
  | Log-spectral distance from the input (LPC envelopes) | 2.2 – 2.3 dB | 1.9 – 2.0 dB | 1.9 – 2.0 dB | ≤ 3 dB |
  | Segmental SNR | 9.8 – 10.2 dB | 11.1 – 12.0 dB | 9.1 dB | ≥ 7 dB |
  | Level tracking, unvoiced to voiced, 100 ms windows | 5.2 – 5.4 dB | 3.8 – 6.3 dB | 5.8 dB | ≤ 6.8 dB |
  | Digital silence after the first frame | −78.2 dBFS | −79.8 dBFS | — | ≤ −60 dBFS |
  | Delay (cross-correlation lag) | 180 device samples | 179.75 device samples | 60 at 8 kHz (the reference's own 7.5 ms look-ahead, not a target) | 180 ± 2 |

- **Latency.** The default one-pass latency is 1923 host samples (40.1 ms) at 48 kHz with the codec and 483 (10.1 ms) without; 181 and 1 at 6 kHz. The record path carries the whole codec delay (1682 at 48 kHz), the playback path is unchanged (241).
- **Bypass.** `speech_codec_enabled = false` switches the encoder and the decoder off together: one control for the pair, because a decoder without its encoder is a combination the device cannot produce, and so is the device without its codec. The codec's output can be tapped before the playback filter (`Tap::AfterDecoder`); the existing "after VAS" tap means "before the codec". **A bypassed engine is not a device configuration.** Label its output as such wherever a person could mistake it for the recorder's sound; the exported form declares the encoding for exactly this purpose. Changing the control needs an empty memory (see below).
- **One memory, one encoding.** A memory holds codec frames with the codec on, or device-rate samples with it bypassed, never both. Two consequences: reconfiguring the codec control while recordings are present is refused ("stored recordings do not fit the new configuration"; erase first), and a recording made with the codec bypassed cannot be imported into a default-configured engine ("encoding differs from the engine's configuration"). Comparing material with and without the codec through the one-pass call needs an empty memory; the taps need no reconfigure.
- **The stored form.** `export` writes form **version 2** ([contract](../specs/008-celp-codec/contracts/recording-form.md)): a header that declares the encoding and, for frames, the **codec model version** that wrote them (0.8.0 today), then the frames verbatim, then a checksum. Version 1 (0.6 and 0.7's device-rate samples, never released) is refused with "unsupported form version". **From the 1.0 release on, every codec model version a released library wrote keeps its decoder in the library**: the stored frames decode to the same device-rate stream, bit for bit, in every later release, so a recording is never orphaned when the codec model improves. Stages after the decoder (the playback filter and, later, the amplifier and speaker models) are not frozen, so what you *hear* can still change with a release; the CHANGELOG lists every such change. Before 1.0 a model version may be dropped with a CHANGELOG entry, and a recording that needs it is refused with "unsupported codec model version".
- **Footprint.** The store reserved at construction is 1 801 600 bytes at the default capacity (180 000 frames of 10 bytes plus the index), about 1.8 MB instead of the 86 MB of device-rate samples when the codec is bypassed.

```rust
use rr_dr60::{Pipeline, Settings, Tap, speech_codec};

let mut s = Settings::new(48_000);
// s.speech_codec_enabled = false;   // bypass the pair (not a device configuration)
// s.tap = Tap::AfterDecoder;        // hear the codec's output before the playback filter
let mut p = Pipeline::new(s)?;
let info = speech_codec::info();     // frame_samples 120, bits_per_frame 80, delay_samples 180, model_version [0, 8, 0]
```

```c
RrDr60Settings s = rr_dr60_settings_default(48000);
s.speech_codec_enabled = false;           /* bypass the pair */
s.tap = RR_DR60_TAP_AFTER_DECODER;        /* or: the codec's output before the playback filter */
```

> **Modeled on the class, not on the device, and not measured against a unit.** The frame length, the look-ahead, the codebooks and the bit allocation are the model's choices from the published coders of the period; a real RR-DR60 may use a different frame length (even a round sample count rather than a round millisecond count), different codebooks, or a frame-granular pause. The deciding captures are listed in [A-040 and A-041](hardware/assumptions.md): a sustained vowel or a click train played back by a real unit (the frame period shows as a recurring excitation pattern), the shortest item a unit keeps, the exact time shown at FULL, and white noise, a tone and a pitch glide through the speaker and earphone outputs.
<!-- codec-section:end -->

## Playback speed (the SPEED CONTROL)

<!-- speed-section:start -->
Since 0.13 a recording can be played at the device's **five playback speeds**. The owner's manual describes a SPEED CONTROL switch and five levels, level 3 normal, level 1 the slowest and level 5 the fastest; Panasonic's 1999 US product page gives the only figure, "Slow/Fast Play (+/-40%)" ([S-001, S-012](hardware/sources.md)). The engine plays the levels at **3/5, 4/5, 1, 6/5 and 7/5** of normal speed (0.6, 0.8, 1.0, 1.2, 1.4: the speed reading of "+/-40%", linear spacing assumed; [A-054](hardware/assumptions.md), low confidence; the other reading, a duration change of ±40 %, is registered beside it). Since 0.23 the engine offers **two playback speed modes**, a second live control beside the level.

- **The pitch moves with the speed** (`playback_speed_mode` 0, `pitch_with_speed`, the default). Level 1 plays about 67 % longer and lower, level 5 shorter and higher. This is the modeled default: a resampling of the decoded stream at the 6 kHz device rate between the speech decoder and the codec's reconstruction filter (A-054's mechanism (b)). How the device itself does it is not documented: the codec cannot be slowed by lowering its clock within its stated range ([S-003](hardware/sources.md)), so the slow levels are a DSP process, and the research of 2026-10-17 leans weakly toward a **pitch-keeping** device (time-scaling the decoder's output). One capture of a real unit settles it (P1 in the [hardware-evidence template](../.github/ISSUE_TEMPLATE/hardware_evidence.yml)). At the fast levels the content that would no longer fit below the device's 3 kHz band is removed first rather than folding back; the emulator's resampler keeps every other product at least 60 dB down (a device of 1998 may have done less). Every output in this mode is bit-identical to 0.22's.
- **The pitch is kept** (`playback_speed_mode` 1, `pitch_kept`; since 0.23, [spec 023](../specs/023-pitch-kept-speed/spec.md)). The same five levels change the duration and keep the pitch: a **time-scaling of the decoded stream at the 6 kHz device rate** at the same point in the chain, by a waveform-similarity overlap-add (frames of 120 device samples, synthesis hops of 60, a search of ±40 samples for the best continuation, a complementary raised-cosine overlap). **An engine convention, not a device behavior:** if the device keeps pitch, its method is undocumented; this mode stands in for an unknown method with artifacts of its own, and a capture (P1) that shows a pitch-keeping device would make a new assumption name the modeled method. What it does to the sound, as the harness reads it on stored −6 dBFS tones at 6 kHz: the fundamental within 0.03 % of the stored one at every level (a 1 kHz tone exactly; the pitch of a steady synthetic vowel within 0.3 % by a pitch tracker whose own error on the stored vowel is 0.2 %; a 90 → 220 Hz glide about 0.5 % in the median and 1.4 % at the 95th percentile, 2.0 % at worst, by a tracker guided to ±20 % of the stored pitch; the glide figure includes up to about 1.5 % from the ±10.7 ms content offset below, so the steady vowel is the pitch figure), the level within 0.07 dB, the spurious products of a 1234.5 Hz tone (off the frame grid) 37 – 46 dB below it in the worst 100 ms window (49 – 52 dB over a whole second; a tone near the band's top, 2345 Hz, reads 12 – 20 dB: the whole-sample shifts leave up to a quarter period of phase at a splice there, roughness in the upper band rather than a pitch error), a 701 + 1901 Hz pair within 0.06 % and 0.002 dB per tone. Its artifacts: every hop is a splice, so a transient can be repeated at the slow levels (a click train of twenty 100 ms-spaced clicks plays as 33 clicks at level 1, 20 at the others) or dropped at the fast ones; the content heard lies within ±64 device samples (10.7 ms; ±52 at levels 2 and 4) of the nominal place the progress reports; a level change between levels 1, 2, 4 and 5 is felt at the next hop (at most 60 device samples, 10 ms) with no blend, the splice reading within 1 dB of a constant level's; a mode change, and a change into level 3 in this mode, blend over 20 ms from the old mechanism's frozen branch (the blends' click energy 30 dB and more below an instant switch's on the 1 kHz tone, spec 019's reader: 46 – 48 dB for a mode change, 52 – 56 dB for a change into level 3). A mode change and a level change at one position land in one order whatever the order they were scheduled in: the mode first, the level riding its blend. Neither mode adds anything to a recording or removes anything from it beyond these artifacts; both are described by what they do to a tone.
- **A live control, playback only.** `set_playback_speed(level)` (1 – 5; `playback_speed()` reads it back, default 3) and `set_playback_speed_mode(mode)` (`speed::SpeedMode::{PitchWithSpeed, PitchKept}`; `playback_speed_mode()` reads it back) work at any time, like the volume and the route, and are kept by `reset` and `reconfigure`. From the first device sample a `play` call computes after the change, the playback continues at the new level from the place it reached: nothing restarts, skips or repeats (a change into level 3 waits until the place is a whole stored sample, at most 4 device samples, 0.7 ms). In the default mode a level change blends (since 0.19): for 20 ms (120 device samples, `speed::CROSSFADE_MS` / `CROSSFADE_SAMPLES`, C `RR_DR60_SPEED_CROSSFADE_MS`) the old level fades out at its own place and pitch while the new level fades in from the change, a smoothstep crossfade, which replaces the instant switch's one-sample step on content above about 1.6 kHz with a 20 ms blend: on stored −6 dBFS tones at 1, 2 and 2.4 kHz the harness reads the crossfade's click energy 46 – 55 dB below the switch's under a 40 ms reader that excludes both tones, every one of spec 013's twenty level-pair rows (the figures of `speed_design.py --crossfade`, pinned within 0.5 dB). In pitch-kept mode a level change between levels 1, 2, 4 and 5 has no blend (the next hop reads the new ratio); a change into level 3 waits for a whole place, then blends for 20 ms; a change out of level 3 anchors the frame grid at that output (its first 60 outputs are the stored samples themselves). A mode change at a level other than 3 is a 20 ms blend between the two mechanisms from the common place; at level 3 it applies at once. **One wait rule for the pair:** a request of either control made during a blend lands when the blend ends, at most 20 ms later (plus at most four device samples for a change into level 3), the latest request per control wins, so a run of requests inside 20 ms plays the last of them after the first's blend and a schedule with requests under 20 ms apart plays longer by the waits; every request is logged at its own position with target values, a superseded one too (it never sounds). The audible onset of a change is its logged position plus the blend's remainder (or, for a level change in pitch-kept mode, up to 60 device samples to the next hop), the deferred wait and `playback_latency_samples()`. A change landing on the playback's first output switches at once (nothing has sounded yet). When the recording's own part and tail end during a blend the old branch's remainder is cut by the drain, so a change within 20 ms of a recording's end can still cut the old level's content in one sample (the harness reports the cut per level pair). **The blends are the engine's conventions, not a device behavior** (A-054): what the device does at the moment of a change is unknown; capture P1 decides. The record path and the one-pass processing ignore both controls, as on the device, where the level changes only during playback.
- **Level 3 is the unchanged output** in both modes: bit-identical to 0.12, every play report included.
- **Length.** A recording of K stored device samples plays its own part in ⌈K ÷ r⌉ device samples, then a short tail (in the default mode the resampler's response, at most 46 device samples; in pitch-kept mode the frame grid's, 60 – 119 device samples; none at level 3), then the unchanged drain: in practice a recording plays about its length ÷ r, so a 10 s recording takes about 16.7 / 12.5 / 10 / 8.3 / 7.1 s at levels 1 – 5, plus the drain. `playback_length_samples(number, level)` returns the whole length in host samples for a constant level **in the default mode**; `playback_length_samples_in_mode(number, level, mode)` is exact in both modes (K is `recording_length(number)`); with level or mode changes, show progress as `place ÷ recording_length` and end on `finished`, never on a precomputed total. `PlayInfo::place` reports the place in the recording, in stored device samples, after every `play` call (it reaches the recording's length at the end), for a progress display; in pitch-kept mode the content heard lies within ±64 device samples of it.
- **The mode as a control.** The control manifest's entry 30, `playback_speed_mode`, a choice of `pitch_with_speed` and `pitch_kept`, kind **model internal** (a choice between candidate models of a mechanism the device has but no document describes), live on the playback stream, schedulable at a position like the level (`schedule_change(Stream::Playback, position, 30, 0.0 | 1.0, tag)`; another value is refused), every change a change entry; the control is refused on the one-pass and record streams. A shared listening is reproduced by replaying the playback change log, this control included; the processing record's map (c) reads it through `timeline::place_at_in_mode`. The engine keeps the mode across playbacks like the level; an app persists it across launches, and its "device as modeled" preset turns it off.

```rust
let (number, level) = (1, 1);                  // recording 1 at SLOW (0.6)
p.set_playback_speed(level)?;
p.set_playback_speed_mode(rr_dr60::speed::SpeedMode::PitchKept); // the pitch kept (an engine convention)
let total = p.playback_length_samples_in_mode(number, level, rr_dr60::speed::SpeedMode::PitchKept)?;
p.start_playback(number)?;
let info = p.play(&mut out)?;                  // info.place: stored samples reached
p.set_playback_speed(5)?;                      // FAST from the next play call, no blend in this mode
```

```c
rr_dr60_set_playback_speed(p, 1);              /* RR_DR60_STATUS_INVALID_SPEED_LEVEL outside 1 - 5 */
rr_dr60_set_playback_speed_mode(p, RR_DR60_SPEED_MODE_PITCH_KEPT); /* RR_DR60_STATUS_INVALID_SPEED_MODE for 2 and up */
uint64_t total = 0;
rr_dr60_playback_length_samples_in_mode(p, 1, 1, RR_DR60_SPEED_MODE_PITCH_KEPT, &total);
/* RrDr60PlayInfo.place since 0.13; the struct starts with struct_size since 0.22 (48 bytes) */
```

> **Modeled on the documents, not measured against a unit.** The five ratios, the pitch behavior and the resampler's quality are assumptions ([A-054](hardware/assumptions.md)); the pitch-kept mode is a convention standing in for an unknown method. A capture of a real unit playing a tone, a sustained vowel and a click train at each level decides which mode, if either, is the device's.
<!-- speed-section:end -->

## Output chain (signal-chain stages 11 – 13)

<!-- output-section:start -->
Since 0.9 the route selected with `set_route` has a sound. Everything after the codec's output filter is modeled on the circuit the service manual's schematic shows ([S-002](hardware/sources.md) sheet 3.1) and the amplifier's datasheet ([S-006](hardware/sources.md)): the volume control and the power amplifier ([A-031](hardware/assumptions.md), adopted; medium confidence), the earphone output ([A-032](hardware/assumptions.md), adopted; low), and the speaker, which is **a template of a small speaker** ([A-010](hardware/assumptions.md), revised; low) with its own bypass, because its acoustic response is documented nowhere and has not been captured from a unit. The three stages are on by default; the one-pass processing and playback both end at the selected route.

- **Stage 11, the volume control and power amplifier.** The 50 kΩ linear control's wiper fraction is the engine's **volume position** (`set_volume`, `volume`, 0 … 1; [A-044](hardware/assumptions.md)): the two routes' gains follow the circuit's loaded law (the wiper's resistance adds to the amplifier's 10 kΩ input resistor), 22.7 dB at position 1 on the speaker route in volts, 9.6 dB at 0.5. Two coupling corners: 32 Hz at the control's input (both routes) and 159 Hz at the amplifier's input (speaker route). The amplifier clips at its **clip ceiling**, about 1.15 V peak at the 2.8 V battery rail the schematic marks (1.0 – 1.4 V registered over the cells' state), through a symmetric knee bracketed by the datasheet's distortion figures. A position change takes 20 ms (see *The volume transition* below).
- **Stage 12, the speaker template.** A second-order resonance at 750 Hz with Q 1.8 and a +4.5 dB rising shelf toward 2.5 kHz, peak gain 1.0 at every host rate, no driver nonlinearity, no enclosure modes, no room. The figures are a choice within the registered ranges, not a capture; switch the stage off (`output.speaker_enabled = false`) to keep only the circuit-sourced stages.
- **Stage 13, the earphone output.** The emitter follower's two coupling corners, 120 Hz and 280 Hz nominal (both follow the transistor's current gain; 65 – 130 and 240 – 320 Hz registered), unity in the passband, **no clipper in the model**; the follower's class-A cut-off on loud material is registered, not modeled: above about 0.4 of the travel on loud material the model is expected to be cleaner than the device ([A-032](hardware/assumptions.md); a later feature if a capture shows it).
- **The output scale** ([A-042](hardware/assumptions.md)). A sample of 1.0 is the amplifier's clip ceiling on the speaker route and the volume control's **full swing** (the codec's full scale at position 1) on the earphone route. The relative loudness of the two transducers is not modeled: a host sets its own playback gain per route. **Headroom:** on the speaker route a steady tone below the knee never exceeds full scale after the template, but the speaker template's resonance rings on transients and on clipped material; the earphone route exceeds full scale on square-edged material (its coupling corners) and, at the top of the travel, on a steady full-scale tone (the level gain of the resampler pair and the speech codec, 1.02 – 1.04 in the 0.11 default at six of the seven host rates). The figures by configuration (spec 010): the output chain alone reads about 1.9 at 48 kHz on a full-scale square or tone at position 1 (+5.7 dB over full scale); the 0.11 default (stages 1 – 2 bypassed; unchanged from 0.9) on the speaker route reads about 1.96 – 2.06 (+5.8 … +6.3 dB) on a full-scale tone at position 1 at 44.1 – 96 kHz; with stage 11 on (its clipper holds it at 1.0), the speaker route is never above the speaker template's impulse-response L1 norm, 1.75 – 2.29 by host rate (≤ +7.2 dB); the earphone route has no fixed bound (1.37 – 1.52 chain-only and 1.04 – 1.07 in the 0.11 default on a full-scale square at position 1; at most 1.07 on the harness's full-scale square and tone at position 1). Even at the default volume position (figures from the 0.11 default), at the reference position the harness's speech-like and noise stimuli stay below full scale at every host rate, but loud material near the speaker template's resonance (about 800 Hz) can exceed it: 800 Hz bursts at +10 dBFS read 0.93 – 0.98, and up to about 1.16 (+1.3 dB) at a level 0.0008 dB lower (the speech codec's discrete choices make such peaks level-sensitive), an 800 Hz full-scale square reads up to about 1.06 (+0.5 dB), and a 1 kHz one 1.006 at 96 kHz. **Two ways to write integer buffers.** The choice that keeps the model's sound unaltered is **headroom**: scale the output by 0.42 (−7.5 dB), which keeps the speaker route under full scale by its bound; the earphone route has no proven bound, so there use the output clamp or check your own material. The convenience is the output clamp, next.
- **The output clamp** (spec 010, **off by default**). `output.clamp_enabled` (`output_clamp_enabled` in C) bounds the **route output**: when on, every sample above +1.0 is replaced by +1.0 and every sample below −1.0 by −1.0, on both routes, also with the output chain bypassed; samples inside the bound pass unchanged. The play calls always return the route output, so with the option on they are always bounded. The per-stage taps (`Tap::AfterVolumeAmp`, `Tap::AfterSpeaker`, `Tap::AfterEarphone`) and the one-pass at `Tap::AfterPlayback` are never bounded; with the option on, the route-output tap differs from the selected route's last per-stage tap wherever a sample was bounded. To read the unbounded route output, select `Tap::AfterSpeaker` (speaker route) or `Tap::AfterEarphone` (earphone route) in the one-pass, or leave the option off for the play calls. Convert to 16-bit with `round(y × 32767)`, so +1.0 maps to 32767; scaling by 32768 overflows at +1.0 unless you saturate it to 32767. It is **an emulator convenience, not a device behavior**: nothing in the device bounds the signal after the speaker or the earphone output (the device's own electrical bound, the amplifier's clipping, A-031, is already modeled in stage 11). On material above the bound the clamp replaces the excess with flat tops, which adds distortion of its own that the device does not have. The bound holds for the samples the engine returns at its host rate; any later resampling, filtering or mixing by the host can exceed full scale again, so run the engine at the output's rate. The clamp has no state, adds no latency, costs about 0.5 ns per host sample at 48 kHz on Apple silicon and about 4 ns (2.5 %) on an x86-64 CI runner, where the NaN-safe min/max takes a few instructions (budget 3 %), and changes nothing stored.
- **The volume default.** The position defaults to the **reference position**, `output::VOLUME_REFERENCE` ≈ 0.1335 (−17.5 dB of travel; [A-043](hardware/assumptions.md)), where a full-scale codec output just reaches the clip ceiling under the loaded law, so the default output does not clip on full-scale material; positions above it **clip on full-scale material** on the speaker route. Consequence: full-scale material at the codec's output reads about 0 dBFS (within −1 … 0 dB, the knee's compression) on the speaker route at the default and about −19.2 dB on the earphone route (the loaded law's earphone figure at the reference position). The device has no default: its knob sits wherever the user left it.
- **The volume transition** (spec 011, since 0.11). A valid `set_volume` change takes **20 ms**, whatever its size and at every host rate (`output::VOLUME_TRANSITION_MS`, `RR_DR60_VOLUME_TRANSITION_MS` in C): from the next block (the next `play` or processing call) the knob moves from where it is to the new position, starting and ending at rest, and the gain follows the loaded law at the knob's position, so a large change no longer puts a step into the waveform. Measured on a 1 kHz tone at the tap after stage 11, the click energy of a change between 0.1 and 1 is about 34 dB below an instantaneous step's on the speaker route (a −30 dBFS tone, below the knee) and about 41 dB below on the earphone route (full scale); about 33 dB at the speaker route's output. A change during a transition starts a new 20 ms movement from where the knob is, starting from rest, so the knob trails a stream of updates: by about 19 ms for a dragged control sending one position per 60 Hz screen frame, about 23 ms at 120 Hz, but about 105 ms if the position changes every 64-sample block at 48 kHz. **Send a position when the control moves, at most once per UI frame; don't make a fade by changing the volume every block.** The position is taken **at once**, with no transition, in the first block that produces output after construction, `reset` and `reconfigure`, at the start of each playback and on a route change, so an engine that sets its volume before processing gives exactly 0.10's output. The one-pass processing ramps a change made mid-stream; the one-pass and the playback each keep their own knob, so after a playback in which the volume changed the one-pass's next block ramps from its own last position. `volume()` returns your last setting, not the knob's position during a transition. The continuity is the device's (the wiper moves continuously, [A-045](hardware/assumptions.md)); the 20 ms and the shape are an **engine convention**, not a measurement of how fast a thumb turns the wheel, and the wiper's contact noise is not modeled.
- **Figures.** Measured by the harness ([`us1_output`](../crates/rr_dr60_harness/tests/us1_output.rs), [`output_matrix`](../crates/rr_dr60_harness/tests/output_matrix.rs)) in the output-chain-only configurations at 48 kHz, against the design's discrete realization of the registered figures. They are the model's figures, never the device's.

  | Measure, 48 kHz | Earphone route | Speaker route | Target |
  |---|---|---|---|
  | Composite response against the design, 20 Hz (earphone) / 100 Hz (speaker) – 2.5 kHz | < 0.001 dB (the Rust against the design's coefficients) | < 0.001 dB (the same) | ±0.5 dB |
  | Level at 1 kHz, position 1 | −0.39 dB | 21.34 dB observed in the route's scale (22.67 dB in volts; the speaker template off) | −0.5 … 0 dB / 21.5 ± 0.5 dB |
  | Passband flatness | 0.33 dB (1 – 2.5 kHz) | 0.16 dB (800 Hz – 2.5 kHz, the template off) | 0.4 dB / 0.2 dB |
  | Speaker template, 1 kHz reading re its peak | — | −0.67 dB | negative, within 1 dB |
  | Group delay at 1 kHz | 0.065 ms | 0.314 ms | ≤ 0.5 ms |
  | Full-scale sine at the reference position | −19.61 dBFS | −0.09 dBFS whole-signal peak (the steady state about −0.3 dB, the knee's compression), 1.24 % THD (the template off) | −19.2 ± 1.5 dB / −1 … 0 dB, ≤ 1.5 % |
  | Distortion, the amplifier path alone | — | 10 % at +2.27 dB over the ceiling, 23.0 % at +6 dB, 0.001 % at −6 dB, 0.34 dB of compression at the ceiling | +1 … +3 dB; 15 – 30 %; ≤ 1 %; ≤ 1 dB |
  | Full-scale sine at position 1, THD+N | below 0.1 % | — | < 0.1 % |

- **Audibility on device-made recordings.** The 32 Hz corner lies far below the codec's 225 Hz record band edge and is a DC blocker on a recording the device made; the 159 Hz corner trims the band's bottom by about 1 dB at 300 Hz; the earphone's corners thin the earphone route's low end. The stages matter most on material that did not come through the recorder's own band-limit.
- **Latency and group delay.** The chain adds no reported latency (its filters are minimum-phase); its group delay at 1 kHz, 0.31 ms on the speaker route and 0.07 ms on the earphone route at 48 kHz, is not counted. A host aligning the default output adds that figure to the reported latency. The playback drain grows by the chain's settle (`output::settle_samples`, about 58 ms).
- **Clipped output at low host rates.** Distortion is measured at 44.1 kHz and above: at 6, 8 and 16 kHz a clipped tone's harmonics alias within the host band, so the clipped speaker route there is band-limited by the host rate and is not the device's. At 6 and 8 kHz the knee's harmonics above the host Nyquist fold back into the band as inharmonic products (a 1 kHz tone reads 29 % THD at 6 kHz against 23 % at 48 kHz at the same drive): use 16 kHz or more for the speaker route above the reference position. The speaker template itself differs by up to about 1 dB between the 6 kHz and 48 kHz hosts near 2.5 kHz (the discrete filter's warp at the low rates; the script reports it).
- **Bypasses and taps.** `output.volume_amp_enabled`, `output.speaker_enabled` and `output.earphone_enabled` switch the stages off one by one (bypassing stage 11 removes the volume control, the corners, the gain and the clipper together: unity gain at the codec's scale, the position retained and ignored). **A bypassed output chain is not a device configuration.** The default tap is the route output (`Tap::RouteOutput`); `Tap::AfterVolumeAmp` taps after stage 11 on either route, `Tap::AfterSpeaker` and `Tap::AfterEarphone` are refused while the other route is selected, and `Tap::AfterPlayback` is 0.8's default output, before the chain: to keep 0.8's output, set it explicitly.
- **What would test the stages.** The captures in [the field-measurement note](hardware/field-measurement.md): the earphone level against knob angle (the loaded law), a 20 Hz – 3 kHz sweep into a 16 Ω load and into a high-impedance input (the two earphone corners), the speaker-terminal peak where distortion passes 10 % (the ceiling and, with the earphone level at the same knob position, the gain), and the speaker-over-earphone ratio (the template).

```rust
use rr_dr60::{Pipeline, Route, Settings, Tap, output};

let mut s = Settings::new(48_000);
// s.output.speaker_enabled = false;  // keep only the circuit-sourced stages
// s.tap = Tap::AfterPlayback;        // 0.8's output, before the chain
// s.output.clamp_enabled = true;    // the output clamp: bound the route output at ±1.0 (spec 010)
let mut p = Pipeline::new(s)?;
p.set_route(Route::Earphone);         // the earphone follower instead of the amplifier and speaker
p.set_volume(output::VOLUME_REFERENCE)?; // the default: full scale just at the ceiling
```

```c
RrDr60Settings s = rr_dr60_settings_default(48000);
s.output_speaker_enabled = false;          /* keep only the circuit-sourced stages */
/* s.tap = RR_DR60_TAP_AFTER_PLAYBACK; */   /* or: 0.8's output, before the chain */
/* s.output_clamp_enabled = true; */       /* the output clamp: the route output bounded at ±1.0 */
```

> **Modeled on the circuit and the datasheet, not measured against a unit.** The clip ceiling, the corners and the volume law are design constants read from the schematic sheet and the datasheet; the speaker is a template. The deciding captures are listed above and in [A-031, A-032, A-042 – A-045](hardware/assumptions.md).
<!-- output-section:end -->

## Battery state (the speaker amplifier's supply)

<!-- battery-section:start -->
The speaker amplifier runs from the two cells directly. The **battery voltage** control sets those cells' voltage; the amplifier's clip ceiling falls with it, so loud playback through the speaker clips sooner and lower, and quiet playback is unchanged. The earphone output and recording are unaffected. Modeled on the circuit and the amplifier's datasheet, not measured ([A-056](hardware/assumptions.md), low to medium confidence). **The value describes the played unit; it is set by the user and never follows the phone's battery, a sensor or a timer.**

**The law.** The amplifier (IC9, an NJM2113) is fed from the cells through a transistor whose drop is about 0.2 V at idle (the sheet's mark; larger under load, not modeled), and its output swing is limited about 1.6 V below its supply ([S-006](hardware/sources.md)), so the clip ceiling scales with the cell pair's voltage V as the **ceiling factor** b = (V − 1.8) ÷ 1.2: exactly 1 at the default 3.0 V, where the ceiling is the model's 1.15 V peak (A-031) and the output scale's 1.0 (A-042). Only the ceiling moves: the clipper's knee (from 0.85 of the ceiling) and its hard ceiling scale by b, and below the knee every sample passes unchanged; a clipped tone comes out lower by 20·log₁₀(b). The law and its data are public: `rr_dr60::battery::{ceiling_factor, ceiling_db, ALKALINE_TRAJECTORY, NIMH_PAIR_VOLTS, BATTERY_VOLTS_RANGE, DEFAULT_BATTERY_VOLTS}` (C: `rr_dr60_battery_ceiling_factor`, `rr_dr60_battery_ceiling_db`, `rr_dr60_battery_trajectory_row`, the `RR_DR60_BATTERY_*` defines).

| Cells | Ceiling factor b | Ceiling re the default |
|---|---|---|
| 3.0 V (the default, the service manual's 2.8 V mark at the amplifier) | 1.000 | 0 dB |
| 2.8 V | 0.833 | −1.6 dB |
| 2.6 V | 0.667 | −3.5 dB |
| 2.45 V (a steady NiMH pair, `NIMH_PAIR_VOLTS`) | 0.542 | −5.3 dB |
| 2.4 V | 0.500 | −6.0 dB |
| 2.2 V (the lowest accepted) | 0.333 | −9.5 dB |

A-056's **alkaline reference trajectory** (`ALKALINE_TRAJECTORY`; the cell datasheet's discharge curves against the owner's manual's recording life, S-017 and S-001) gives the pair's voltage under the device's load after hours of **recording**: 2.96 V at 0 h (−0.3 dB), 2.76 V at 1 h (−1.9), 2.66 V at 2 h (−2.9), 2.52 V at 4 h (−4.4), 2.42 V at 6 h (−5.7), 2.32 V at 8 h (−7.3), 2.22 V at 10 h (−9.1). It is preset data for an app, never a drain: the engine never moves the value on its own. Row 0 is a loaded-cell reading, not a fresher state than the default: an app's "fresh" is `DEFAULT_BATTERY_VOLTS`.

**Setting it.** `set_battery_volts(volts)` / `battery_volts()` (C: `rr_dr60_set_battery_volts`, `rr_dr60_get_battery_volts`; `Error::InvalidBatteryVolts`, C status 30, for a value outside 2.2 … 3.0 V or not finite, the value then unchanged) at any time, including during playback; it survives `reset` and `reconfigure` like the route. A change moves the factor over the **20 ms transition** of the volume control (A-045's convention, spec 011's shape: the landing sample is transition sample 1, the target exact at sample N, monotonic) so a change on clipped material has no step (the output is the splice of the two constant runs after the transition, asserted; the click energy against an instantaneous step is a reported figure of the harness, not a floor: spec 011's reader does not separate the two on a hard-clipped tone); the factor is taken at once on a stream's first sample, while stage 11 is bypassed, on the earphone route and at a route change (a transition in progress ends at the switch). The value is the fourth live control of the [control manifest](#the-controls-the-engine-accepts) (`battery_volts`, a unit condition, index 29, appended so no earlier index moves) and can be [scheduled](#scheduled-changes-and-the-change-log) on the playback and one-pass streams in the output clock; every change, scheduled or plain, is a change entry.

**The meters.** The [read-out](#meters-one-read-out-per-processing-call) gains two figures: `amp_clip_count`, the host samples of the call the amplifier's knee or ceiling changed (above 0.85 of the ceiling at the value in force; meaningful on the speaker route with stage 11 on; 0 on the earphone route), and `battery_ceiling`, the factor in force at the end of the call (a factor: 20·log₁₀ for dB; 1.0 at the default; reported on both routes). In C `RrDr60Meters` grows 56 → 64 bytes under a compatible rule: a `struct_size` of 56 … 63 fills the 0.17 fields and touches nothing else, so a 0.17 binary keeps working.

**Clipped material and the output clamp.** The speaker template rings over the ceiling, so a full-scale square at position 1 reaches about 1.4 – 1.7 at 3.0 V by host rate and less at lower values (figures reported by the harness's `battery_matrix`); the optional [output clamp](#output-chain-signal-chain-stages-11--13) **cannot** engage below about 2.3 – 2.5 V by host rate (the template's worst-case gain, 1.75 – 2.29, times b stays under 1.0: spec 010's bound), so it stays idle at 2.2 V; where it does engage depends on the material: on that square at 3.0 V at every rate, at 2.6 V from 8 kHz up and not at 6 kHz.

**The range.** 2.2 … 3.0 V. Near 2.2 V the device's firmware stops the unit (inferred from the cell data and the recording life, A-056; the manual's own wording for the low-battery mark is not on record); the model plays it. Values above 3.0 V (a lithium primary pair) are outside the amplifier's modeled range and refused.

**Not modeled:** the cells' sag under load on loud passages (0.1 – 0.5 V, A-056), any drain over time, supply-noise coupling, the low-battery indicator and cut-off; the record path and the earphone route run from regulated rails and are modeled as unaffected.

```rust
p.set_battery_volts(2.4)?;                                  // a tired pair: the speaker clips 6 dB lower
let b = rr_dr60::battery::ceiling_factor(2.4);              // 0.5
let (hours, volts) = rr_dr60::battery::ALKALINE_TRAJECTORY[3]; // (4.0, 2.52): a preset's value
let m = p.meters();                                          // m.amp_clip_count, m.battery_ceiling
```

```c
rr_dr60_set_battery_volts(p, 2.4f);
float b = 0.0f; rr_dr60_battery_ceiling_factor(2.4f, &b);   /* 0.5 */
float hours, volts; rr_dr60_battery_trajectory_row(3, &hours, &volts);
```

> **Modeled on the circuit and the amplifier's datasheet, not measured.** The law's slope, the transistor's drop and the trajectory are read from the schematic and the datasheets; a bench supply swept on an opened unit (capture P5) would check the law and decide the knee's shape. See [A-056](hardware/assumptions.md).
<!-- battery-section:end -->

## Meters (one read-out per processing call)

Since 0.15 the engine keeps a **meter read-out**: eleven figures (thirteen since 0.18) computed from the same values the output is computed from, describing the samples of the last non-empty processing call (a one-pass `process*`, `record`, `play` or the stop's flush). Read it after the call with `meters()` (Rust) or `rr_dr60_meters` (C): a plain copy, real-time safe, never changing the engine or its output. Every earlier output is bit-identical with the meters in place; nothing about them is settable, and they add no assumption: each figure reads a stage that is already modeled, and inherits that stage's status and open questions (the IDs in the table).

The figures of a call describe the samples that call **processed**, not the samples it output: the stage 4 and 5 figures, and the recorder's whole group, describe device samples that leave the output up to the path's latency later (the one-pass at 48 kHz by default: 1923 host samples, about 40 ms; 483 with the speech codec off; at other rates `latency_samples()` reports the one-pass figure and `record_latency_samples()` the record path's share; the recorder's samples are stored and heard only when the item is played). A figure whose point the call did not reach keeps its **rest** value; before any call, after `reset` and after `reconfigure` every figure is at rest (`rest_meters()`; `Meters::default()` is the all-zero value, the C zero-struct's twin, whose hang is 0 rather than the configured length). A zero-length call, a skipped flush and the start, stop-playback and erase calls leave the read-out as it was.

| Meter | Field (Rust; C) | Point | Unit | Rest | Lead over the audible output | Rests on |
|---|---|---|---|---|---|---|
| Input peak | `input_peak` | the input after the engine's sanitizing of non-finite samples | linear full-scale, ≥ 0 (20·log₁₀ for dBFS) | 0 | one-pass: the call's own samples leave the output `latency_samples` later; recorder: stored | the full-scale convention (A-030) |
| Limiter-input peak | `limiter_input_peak` | stage 2's output, the level entering the limiter (stage 1's with stage 2 off; the sanitized input with both off) | linear full-scale, ≥ 0; above 1.0 is possible (stage 2 limits softly near +9 and +17 dBFS) | 0 (also at `Tap::AfterMicCapsule`) | as the input peak: the microphone stages add no latency | A-046, A-051, A-030 |
| Gain reduction | `gain_reduction_db` | stage 3: the deepest attenuation the limiter applied to any sample of the call | dB, ≥ 0 (0 = unity gain; exactly 0 below the hold's unity floor) | 0 (also with the limiter off) | as the input peak: the limiter runs at the host rate with latency 0 | A-028, A-034, A-035 (modeled on the circuit with estimated transistor parameters, not measured) |
| Hold charge | `hold_charge` | stage 3: the limiter's hold capacitor at the end of the call, as a fraction of the hold at the model's deepest attenuation | 0 – 1 (0 discharged; landmarks below) | 0 (also with the limiter off) | a state, not a per-call figure: it describes the limiter at the end of the call | A-028, A-034, A-035 |
| Record-filter peak | `record_filter_peak` | stage 4: the largest device sample after the record band-limit, **before** the converter's full-scale clamp | linear full-scale, ≥ 0; above 1.0 means the clamp bounded samples | 0 | device samples: they leave the one-pass output later by the latency less the decimator's delay and the band-limit's own group delay (the remaining stage 10, interpolator and codec delays); in the recorder they are stored | A-027, A-030 |
| Converter-clamp count | `adc_clamp_count` | stage 4: the device samples of the call the full-scale clamp changed, on the 64-bit sample it acts on (a sample exactly at full scale is not counted) | device samples, per call | 0 (also with `adc_clamp_enabled` off) | as the record-filter peak | A-030 |
| Voice-activation state | `vas_paused` (C: 0 or 1) | stage 5: paused or recording at the end of the call, the gate's own state machine (also during the stop flush, which keeps every sample) | — | recording (false; also with the gate off) | describes the end of the call's device samples; a pause lands on the next non-sound device sample, possibly in the next call | S-001, A-022, A-023, A-024 |
| Hang remaining | `hang_remaining_samples` | stage 5: the further non-sound device samples the gate keeps before pausing (the hang length minus the non-sound samples since the last sound) | device samples (÷ 6000 for seconds); the full hang when the last sample was sound or the gate is off; 0 while paused | the configured hang | as the state | A-023 |
| Reconstruction peak | `reconstruction_peak` | stage 10's output after the conversion to the host rate, before the output chain | linear full-scale, ≥ 0 | 0 | the call's own output, ahead of it by the output chain's group delay only (0.31 ms speaker, 0.07 ms earphone at 48 kHz) | A-027 |
| Route peak | `route_peak` | the selected route's output **before** the optional output clamp (also with the output chain bypassed) | linear full-scale, ≥ 0; 1.0 is the amplifier's clip ceiling on the speaker route and the earphone scale on the earphone route; the speaker route reads up to about 2.3 on clipped or transient material | 0 (at `Tap::RouteOutput` only) | none: the call's own output | A-042, A-031, A-010, A-032 |
| Output-clamp count | `output_clamp_count` | the host samples of the call the output clamp bounded, on the 64-bit sample before the output's narrowing | host samples, per call | 0 (also with `output_clamp_enabled` off) | none: the call's own output | A-042; the clamp is an emulator option, not a device behavior (spec 010) |
| Amplifier clip count (since 0.18) | `amp_clip_count` | stage 11 on the speaker route: the host samples of the call the amplifier's knee or ceiling changed (above 0.85 of the ceiling at the battery value in force) | host samples, per call | 0 (also on the earphone route, with stage 11 off and at a tap before it) | none: the call's own output | A-031, A-056 |
| Battery ceiling (since 0.18) | `battery_ceiling` | the [battery value's](#battery-state-the-speaker-amplifiers-supply) ceiling factor in force at the end of the call, moving over the 20 ms transition after a change | a factor, 1.0 at the default (20·log₁₀ for dB) | the factor of the value in force at the last non-empty call (1.0 at the default); `rest_meters()` follows a value set since | a state: reported on both routes | A-056 |

**The hold charge's landmarks** (of the model's hold law, spec 005; the scale is settings-independent): about 0.13 is the unity floor (below it the gain is exactly 1), 0.73 is one decibel of attenuation (the knee's definition), 0.84 ten, 0.90 twenty, 1.0 forty (the model's cap, an engineering target, not a device figure). A silent call reads the decaying hold.

**Which figures a one-pass call fills** depends on the tap: a front tap fills the input peak (and at `AfterMicAmp` the limiter-input peak); `AfterAgc` adds the limiter's two; `AfterRecord` the stage 4 pair; `AfterVas` and `AfterDecoder` the stage 5 pair; `AfterPlayback`, `AfterVolumeAmp`, `AfterSpeaker` and `AfterEarphone` the reconstruction peak; `RouteOutput` everything. The recorder fills the record group (the playback group rests), the player the playback group.

```rust
let info = p.process(&input, &mut output)?;
let m = p.meters();               // the figures of this call
if m.adc_clamp_count > 0 { /* the converter clipped: m.record_filter_peak says how far over */ }
let paused = m.vas_paused;        // == info.paused
```

```c
RrDr60Meters m = { .struct_size = sizeof m };
rr_dr60_process(p, in, out, n, &info);
rr_dr60_meters(p, &m);            /* the same figures, field for field; m.vas_paused is 0 or 1 */
```

The read-out is checked against oracles outside the engine at every host rate (the tap outputs for the peaks, a limiter-bypassed twin for the gain, clamp-bypassed twins for the counts, a reference gate for the voice activation), is bit-identical on every golden target and between Rust and C, and costs a few compare-and-select operations per sample (the figures in the [CHANGELOG](../CHANGELOG.md)). What an app shows with these figures is the app's wording: the product notes ask for "limiter hold" rather than a bare "charge", for "recording / paused" rather than words that imply anything beyond sound level, and for the output-clamp count to be shown as an emulator option.

## Scheduled changes and the change log

Since 0.16 a change of a live control (the route, the volume position, the playback speed level) can carry a **position**: the index of the output sample it is the first to affect, in the stream's own clock. The engine applies it on that sample whatever the host's block size, and reports every change a stream applies, scheduled or set plainly, as a **change entry** with the position it landed on. **Positioning a change on a sample is a feature of the emulator's API, not a device behavior: the recorder's own controls move whenever a hand moves them.** Nothing about the modeled sound changes, no assumption is registered, and with no scheduled change every output is bit-identical to 0.15. It is what a host needs to play a session back the same way at any buffer size, to apply a preset or a scripted sequence of positions, and to show what changed when from the engine's own record rather than from what the host sent.

> **An emulator convention, not a device behavior.** The device has no way to schedule a control; the engine's positions exist so that a host's changes land on the same sample at every block size. A change entry records where the engine applied a change, not where a hand would have moved a knob.

**The two streams and their clocks.** The **playback stream** is the player: its position starts at 0 at `start_playback` and counts every sample the playback produces (the recording's part, the speed control's tail and the drain). The **one-pass stream** is `process*`: its position starts at 0 at construction, `reset` and `reconfigure` and counts the samples produced, the clock the VAS events already use (in drop mode the output is shorter than the input, and positions are the output's). `stream_position(stream)` reads either; the next sample produced has that index. A third stream, the **record stream** (`Stream::Record`), is the recorder; its clock is the **input clock** of the record side (since 0.17, below).

**The record side (since 0.17).** Eight record-side values can be scheduled too: the microphone sensitivity, the pause threshold, the pause delay and the resume delay (stage 5), the limiter's threshold, attack and release (stage 3) and the hiss level (stage 1). They are positioned in the stream's **input clock**: the host input samples the recorder has processed since `start_recording` (the **record stream**, `Stream::Record`), or the one-pass has processed since construction, `reset` or `reconfigure`; `input_position(stream)` reads either. A control whose paths include "record" uses the input clock; one whose paths include "playback" uses the output clock, so the one-pass carries both and each entry's control says which (`changes::clock_of(control)` answers input or output for any control; `changes::record_landing(P, rate)` gives the device sample ⌈P · 6000 ÷ rate⌉ at which a gate value scheduled at input P takes effect, P itself at 6 kHz). The limiter's values land on exactly input sample P with the hold capacitor's charge carried, as a component changed while the circuit runs (the release's resistor, the detector's threshold; a knee change also re-derives the detector's current gain so the configured dead time holds, a model rule); the hiss level moves to its new value over 20 ms from P (the noise itself is unchanged); the sensitivity, threshold and the two delays take effect at the first device sample derived from input sample P, the running counts kept. A landed value shows in `settings()`. The record schedule is kept by `start_recording` and cleared at every end of a recording (the stop after its flush, the self-stop at the memory limit, the skipped flush, `reset`, `reconfigure`), so a change scheduled at position 0 while stopped lands on the first sample of the next recording, and one scheduled during a recording at a position already passed lands on the next input sample. **On the device the sensitivity is set with the MODE SELECT switch while the recorder is stopped (S-001); the manual does not say whether a unit accepts a change during a recording, and the other seven values are the model's constants. Changing any of them while recording is a feature of the emulator's API, not a device behavior:** no assumption is registered, no model version moves, and with no record-side change scheduled every output is bit-identical to 0.16.

Also: read `input_position(Record)` just before stopping: it is the index of the last microphone sample plus one, entries at or past it landed inside the flush (the raw capture has no sample there), and the stop report's final input position (`StopOutcome.input_samples_total`) minus it is the flush's length in host samples; a pending record-side change is not shown by `settings()` until it lands (read `pending_changes(Record)`); a change scheduled while stopped is logged at position 0 of the next recording; a change landed on the recorder reaches the one-pass at the one-pass's first call after the recording ends (the one-pass is refused while a path is active) and is logged there with the landing's tag, where a playback-side change mirrored since 0.16 is logged with tag 0 (spec 017 D4; the two rules align at spec 020's ABI pass); the reverse direction is the adopt: a one-pass landing is in the recorder's stages at the next `start_recording`, with no entry, so a recording's `before` values are the settings at its start. A hiss change during a transition starts a new transition from the gain last applied. A record-side value has no plain setter: the schedule is the only way to change one while the engine runs, and `reconfigure` the way while it is idle.

**Scheduling.** `schedule_change(stream, position, control, value, tag)` (C: `rr_dr60_schedule_change`) puts a change on the stream's schedule: `control` is the control manifest's index of `route`, `volume`, `playback_speed`, (since 0.18) `battery_volts` or (since 0.21) one of the four playback-side stage switches (see [Live stage bypass](#live-stage-bypass)), which an app resolves by identifier at start-up and never assumes; `value` is the control's own value (a route 0 or 1, a volume position or a battery voltage, rounded to single precision, a level 1 … 5); `tag` is any number the entry echoes, **0 being reserved for plain setters, so tag from 1**. The speed is accepted on the playback stream only (its path in the control manifest). Each stream holds up to 64 pending changes (`changes::SCHEDULE_CAPACITY`), applied in position order, changes at one position in submission order (the later wins); a 65th is refused (`ScheduleFull`); `pending_changes(stream)` counts them and `clear_schedule(stream)` drops them. The call is accepted in every engine state and is real-time safe. For a schedule longer than 64 (a replayed session, a long script): keep the list sorted, and after each call, while `pending_changes` is below 64 and the list is not empty, schedule the next entry. The playback schedule is **kept by `start_playback`** (schedule, then start) and **cleared when a playback ends by any path** (`finished`, `stop_playback`, an `erase` of the played item, `erase_all`), so the next item's changes are scheduled only after `finished`; both schedules are cleared by `reset` and `reconfigure`. A change scheduled past a playback's end is dropped at its end without an entry.

**Where a change lands.** A change at position P at or after the stream's position lands so that output sample P is the first it affects: the volume's 20 ms transition has P as its first transition sample (or the position is taken at once in the cases spec 011 lists: a stream's first sample, stage 11 bypassed, an inactive chain), and so has the battery value's (at once in the same cases, on the earphone route and at a route change); the route switches at P with the new branch from rest (a transition in progress ends there); the speed level is requested at P and reaches the device-rate output `changes::speed_landing(P, rate)` = ⌊(P − 1) · 6000 ÷ rate⌋ + 1 (P at 6 kHz; d(0) = 0), the first device sample the playback conversion pulls for output P, under spec 013's whole-place rule and latch. A position before the stream's position lands on the next sample produced and the entry says so. The plain setters keep their meaning: a change lands on the first sample the next processing call of each stream produces; a setter before `start_playback`, `reset` or `reconfigure` is taken by that call and is not an entry, while a `set_route` or `set_volume` made after construction and before the engine's first processing call is a change that call lands and logs (tag 0, position 0) unless the value is the default. One corner differs from 0.15, which reset the chain at every call's entry: a route set and set back before the next producing call never lands and never resets the sections; every other sequence of plain setters is bit-identical. A change scheduled on one stream (or set plainly during a playback) reaches the other stream as a tag-0 change at its next producing call, where its own transition lands. A route change scheduled on the playback stream moves the shared route reading when it lands, so a one-pass configured with the other named-route tap is refused at its next call, as after `set_route`.

**The speed change's onset.** For a change out of level 3 the first output sample that differs bit for bit lies within two device periods after P (a switch *into* level 3 first waits for a whole place, up to four device samples more, spec 013; in pitch-kept mode a change between levels 1, 2, 4 and 5 is felt at the next hop, up to 60 device samples after P, spec 023); the audible change follows after the playback path's latency, `playback_latency_samples()` **for your configuration** (241 host samples at 48 kHz in the default; 238 with stage 10 bypassed: the figure is the path's 1 kHz group delay, spec 006), as the place does. An app aligning a speed change with a volume change subtracts `playback_latency_samples()` from the speed's position. A speed change landing after the recording's latch has no effect on that playback, is logged, and moves the level reading, so it sets the level the next playback starts at.

**The change log.** `process_with_events_and_changes(input, output, events, changes)` and `play_with_changes(output, changes)` (C: `rr_dr60_process_with_events_and_changes`, `rr_dr60_play_with_changes`) write the changes the stream applied in that call to the caller's buffer, the first ones when it is shorter, and report how many there were (`BlockInfo.changes`, `PlayInfo.changes`; in C the count is an out-parameter and the report structures are unchanged); `changes::MAX_CHANGES_PER_CALL` (139 since 0.17: two schedules of 64, the three plain setters and one cross-stream landing per record-side value; 67 before) is always enough; on the record side `record_with_changes(input, events, changes)` and `stop_recording_with_changes(events, changes)` (C: `rr_dr60_record_with_changes`, `rr_dr60_stop_recording_with_changes`) write the record-side entries, the one-pass's log-taking calls carry both sides (the record side's entries first). An entry (`ChangeEntry` / `RrDr60ChangeEntry`, 40 bytes, **frozen** since 0.22: `RR_DR60_CHANGE_ENTRY_SIZE`) carries the position it landed on, the control's index, the value **before** (the value that stream last applied for the control, so entries chain: each one's before is the previous one's after, or the value the stream started with), the value **after**, and the tag. An entry is reported by the call that produced the sample, never by an empty call or a block the voice activation dropped entirely. A scheduled change to the value already applied is a logged no-op (before equals after); a plain setter to the value already applied is not logged. The plain calls (`process*`, `play`) apply the changes, count them and discard the entries.

**Replaying a session.** Re-scheduling a log's entries (position, control, after, tag) reproduces the stream bit for bit **given** the same recording and settings (the playback stream) or the same input, seed and recording counter (the one-pass stream: the microphone hiss is keyed by the counter `recording_counter()` / `rr_dr60_recording_counter` reads, spec 012); the record side's values replay the same way given the same input, seed, recording counter and record schedule.

```rust
use rr_dr60::{ChangeEntry, Stream, changes::MAX_CHANGES_PER_CALL};
let volume = rr_dr60::controls().iter().position(|e| e.identifier == "volume").unwrap() as u32;
p.schedule_change(Stream::Playback, 48_000, volume, 0.5, 1)?;   // output sample 48000 of the playback
p.start_playback(1)?;                                            // the schedule is kept
let mut log = [ChangeEntry::default(); MAX_CHANGES_PER_CALL];
let info = p.play_with_changes(&mut out, &mut log)?;
for e in &log[..info.changes] {
    println!("{} control {} {} -> {} tag {}", e.position, e.control, e.before, e.after, e.tag);
}
```

```c
uint32_t volume = UINT32_MAX;                       /* resolve by identifier, never assume */
for (uint32_t i = 0; i < rr_dr60_control_count(); i++) {
  RrDr60Control c;
  if (rr_dr60_control(i, &c) == RR_DR60_STATUS_OK && strcmp(c.identifier, "volume") == 0) volume = i;
}
rr_dr60_schedule_change(p, RR_DR60_STREAM_PLAYBACK, 48000, volume, 0.5, 1);
rr_dr60_start_playback(p, 1);
RrDr60ChangeEntry log[RR_DR60_MAX_CHANGES_PER_CALL];
size_t count = 0;
rr_dr60_play_with_changes(p, out, frames, &info, log, RR_DR60_MAX_CHANGES_PER_CALL, &count);
```

## Processing record

Since 0.20 the engine can write, for any recording in its memory, a **processing record**: one text document that says how that recording was processed. It is written beside the exported stored form and never inside it (the stored form is unchanged). It carries the engine's identity (the library version and each modeled stage's model version), the host rate, the seed, the recording counter the microphone-noise stream was keyed with, the stored form's version, encoding and unit count, a digest of the stored form, every setting as it was when the recording started, and the run: the record-side change entries with their positions in the input clock, the voice-activation events with the kept position, the removed length and the host-input position each aligns with, the totals the engine counted, the memory's remaining capacity at the start, and how the recording ended (the stop call or the memory filling), with the stop's input position, the padding start and the input samples total. **It contains no time, no place, no device identity, no recording number and nothing of the app's.**

**What the digest shows.** `recording_digest` is a 64-bit FNV-1a over the exported stored form's bytes. Equal digests mean the same bytes up to the 2⁻⁶⁴ coincidence of unrelated forms: a record and a recording belong together. Any change of one byte is detected; a deliberate collision is easy, so it is a consistency check, not a signature, and it says nothing about who made the recording, when, or what reached the microphone: anyone with the library can write a consistent record for any audio. The last line, `record_checksum`, is the same function over the record's own bytes, so an accidentally truncated or altered record is refused.

**The re-render.** An engine built at the record's host rate, settings, seed and counter (`Pipeline::new_at_counter`; C `rr_dr60_create_at_counter`), whose memory is filled **by import** to the record's remaining capacity (a filler recorded in it would consume the counter), that records the same input samples with the record's entries scheduled on the record stream at their positions and stops the same way (after exactly the stop's input position, or by the memory filling) produces a stored form with the record's digest, bit for bit. The harness checks it at every host rate, in sixteen cells (the codec on or bypassed, with and without record-side changes, in drop and mute mode, for the stop call and the memory-full self-stop), and shows the named changes failing it: the seed, the counter, a record-side setting (`mic.hiss_offset_db` +1 dB, `agc.knee_dbfs` to −30 dBFS, `vas.threshold_dbfs` −3 dB, the codec toggled), the memory's capacity in the self-stop cell, a 10 ms burst at −20 dBFS in the input, an entry moved (the hiss entry by one device sample with the codec bypassed, the knee entry by 0.1 s with the codec on), the stop one frame's worth of input earlier; and the playback-side twins leaving it (the volume, the route, the speed, the battery value, the output clamp). The condition is the **library version**: the record names it, and the reader reports when yours differs. The re-render describes how the recording was processed; it does not vouch for what the input contains. The harness example `re_render` (`--demo <dir>` writes a record and a raw file; `<record> <raw>` compares the digests) and the Swift package's `ProcessingRecord.reRender` are the recipe.

**The form.** One field per line, `name = value`, ASCII, LF; the first line `rr_dr60_processing_record_version = 1`; integers decimal; real numbers as the shortest decimal that reads back to the same bits (exact in every correctly rounding parser: Rust, Swift, Python, glibc and macOS); `true`/`false`; a choice as the control manifest's engine value; an unknown value is an absent line. The event and change lines are space-separated fields named by the `event.fields` and `change.fields` lines before them; event positions are device samples (6000 Hz) except the host-input position, which is a host sample; a change row names the control by its identifier and the stream by its word. The form has no comment syntax: a line that is not `name = value` is refused. See `specs/020-session-contract/contracts/record-form.md`.

**Reading a record** (`record::read`, C `rr_dr60_processing_record_read`; with a live handle the host-rate flag is set too) ends in one of three ways:
1. **Refused** (an unknown form version, a malformed line, a missing field, a checksum that does not match the bytes): "This is not a processing record this library can read: <reason>." Nothing else is reported.
2. **Read, and this library is the one that wrote it:** every field is in the read-out; with the recording in the memory, `recording_digest_matches` says whether the record and that recording belong together; with the kept microphone audio, the re-render reproduces the recording.
3. **Read, with differences reported as flags, never as refusals:** "Written by library 0.20.0; this library is 0.21.0" (the re-render may differ; the record still describes the recording); "stage 5's model version differs" (a difference confined to playback-side stages does not touch a stored byte); "host rate differs"; "N setting names this library does not know" (listed by `record_unknown_setting`); "the app collected fewer events or entries than the engine counted (N of M)"; "the record and recording <number> do not belong together" (the digests differ); "`origin = imported`: the recording was brought in from a stored form, so the record names only that form and its digest; there is nothing to re-render."

**The seed and the counter.** Draw the seed once per engine from the system's random source, never from a device, user or app identifier; with the recording counter every recording has its own microphone-noise stream, so a new seed per recording is allowed (`reconfigure` while idle) but not needed. Persist the seed and the counter with the app's state and build the engine at them (`new_at_counter`), so recordings made across launches stay distinct and re-renderable. (A one-pass replay needs the counter as it was at the one-pass's last `reset`, `reconfigure` or construction.)

**The feed point.** Two apps re-rendering the same audio must feed bit-identical samples. Quantize your capture to 24-bit integers at the point you feed the engine, store those losslessly, and convert with `input::samples_from_i24` (x ÷ 2²³, exact for every 24-bit value; C `rr_dr60_samples_from_i24`) on both sides.

**Collecting the run.** The engine keeps no recording's events or entries (the processing path allocates nothing). Append the events and entries of every `record_with_changes` call and of `stop_recording_with_changes` to your own arrays, off the audio thread, sized from `max_record_events` and `MAX_CHANGES_PER_CALL`; pass them with the stop report (`StopFields::from(&outcome)`, or from the self-stop's `RecordInfo`) to `processing_record_len` then `processing_record_write` (both outside the callback; `_len` is one counting pass, call it first; no pre-sizing bound is published). An inconsistent run (entries out of order, at or past the final input position, or from another stream) is refused; a shorter run than the totals is written and flagged on reading.

**The timeline maps** (`timeline::`) are pure functions of the record's data. (a) `kept_to_input`: a kept sample to its **nominal** host-input alignment: the latency convention (the record path's 1 kHz group delay to the encoder's input, `record_latency_pre_encoder_samples`) shifted by every removed gap at or before it, widened to the direct-path window (the decimator's half-length; with the codec on, the model coder's reach around the frame, about −22 … +30 ms re the frame start: A-040/A-041's class match, not the device's coder). It is a convention, not a measured onset, and it states a direct contribution: the gain, the gating and the coder's state depend on earlier input. The stored frames carry no kept-domain shift: the codec's 180 samples are the one-pass's latency alone. (b) `input_to_kept`: a host-input position to the kept sample it reaches, the event that removed it (a burst's first 20 ms, the gate's onset time, read as removed), or in mute mode the stored region that muted it; it does not clamp: compare a result against the stop fields (a kept index at or past the stored length means a gap still open at the stop or the flush); and `onepass_output_to_input`: the one-pass's output clock to its input clock through its latency (`latency_samples`), its events and whether the gate drops. (c) `place_at`: a playback device-output index to the place the speed stage computed there (a replay of its rules over the playback log's speed entries, the recording's unit count and the start level; leading the audible output by `playback_latency_samples()`). The ends are clamped at the stop's input position (the audio the app holds) and reported. The harness reads (a) and (b) within one device sample of a tone burst's onset on the voice-activation-bypassed twin, the splice arithmetic bit for bit, the codec-on case with the decoded output's envelope within 1 ms of the map (the decoder's own delay and the reader's resolution), the one-pass within one device sample, and (c) equal to the stage's own trace on ten schedules.

**Compatibility.** Every later library reads every earlier form version (a later version only adds lines). A recording stays importable and plays bit for bit under its stored form's rules; a re-render from the microphone audio needs the library version the record names, and a record stage whose model version moved is named in the read-out. Everything here is in the free tier of the project's own app (decisions.md, 2026-10-31), and none of it is a device behavior: a real recorder keeps no record of how it processed a recording.

```rust
use rr_dr60::record::{RunData, StopFields};
let out = p.stop_recording_with_changes(&mut events_buf, &mut log)?;      // the host collected every call's rows
let run = RunData { events: &events, changes: &changes, stop: StopFields::from(&out) };
let mut record = vec![0u8; p.processing_record_len(number, &run)?];      // outside the callback
p.processing_record_write(number, &run, &mut record)?;
```

```c
RrDr60RunData run = { .struct_size = sizeof run, .events = events, .event_count = n_events,
                      .changes = changes, .change_count = n_changes,
                      .stop = { .struct_size = sizeof run.stop, .how = RR_DR60_STOP_CALL, /* the outcome's stop_input_position, padding_start_sample, silence_fed_samples; the total fed */ } };
size_t len = 0, written = 0;
rr_dr60_processing_record_len(p, number, &run, &len);
rr_dr60_processing_record_write(p, number, &run, buf, len, &written);
RrDr60RecordReadout r = { .struct_size = sizeof r };
rr_dr60_processing_record_read(p, buf, len, &r);               /* r.flags: the differences */
```

## Live stage bypass

Since 0.21 the four playback-side stage switches can be changed **while a recording plays**: stage 10 (the codec output filter, `playback_stage_enabled`), stage 11 (the volume control and power amplifier, `output.volume_amp_enabled`), stage 12 (the speaker template, `output.speaker_enabled`) and stage 13 (the earphone output, `output.earphone_enabled`). Each is a live value scheduled like the record-side values of 0.17: the setting itself, through `schedule_change` by its control-manifest index (resolve it by identifier), with the value exactly 1 (on) or 0 (bypassed), on the **playback stream** (all four) and the **one-pass stream** (stages 11 – 13; stage 10 is playback-only, like the speed); there is no plain setter; `settings()` reflects a landed value; every landing is logged as a change entry with the stream word.

> **An emulator convention, not a device behavior.** The RR-DR60 has no stage switches. The blend below, the warm stage and the refusal rule are the engine's; no assumption is registered or amended. A bypassed stage's branch is the previous stage's output, still band-limited, coded and limited: label it "stage N bypassed (emulator)".

**The switch is a 20 ms blend.** A stage configured on keeps running while bypassed (its filter state stays warm, the per-sample work is unchanged), and its contribution to the signal fades between its processed output and its input over 20 ms of samples (120 device samples for stage 10, 0.02 × the host rate for stages 11 – 13) along the engine's smoothstep, the one the volume transition, the battery transition and the speed crossfade use; the downstream stages and the per-stage taps receive the blended signal. Outside a blend the stage's output or input is passed as it is, so a bypass equals the same engine configured with that stage off and a return equals the all-on run. A toggle during a blend **reverses from the current phase** (no wait, no restart); two changes of one stage at one position both apply and both log (off-then-on is no blend); a change on a stream's first produced sample switches at once; a change to the value in force is a no-op entry.

**Where the identities are bit-exact.** Where no filter follows the switched stage on the active route (stage 12 on the speaker route, stage 13 on the earphone route, stage 11 alone, stage 10 with the chain off) the bypass equals the configured-off engine and the return equals the all-on run bit for bit from the blend's last sample. In the default engine a downstream filter remembers the blend, so the output converges to the twin's instead: the harness reads it within −100 dBFS of the twin from the blend's end plus the chain's settle at every host rate, and indistinguishable from it on the narrowed output about 10 – 15 ms after the blend's end at every rate with the −6 dBFS stimulus (the 64-bit states still differ, visible once as −313 dBFS in a drain at 88.2 kHz; the time to identity depends on the level); the assertion is the −100 dBFS bound. Stage 10's blend reaches the host output through the rate conversion: at 6 kHz the output is the device stream; at the other rates the identity with the configured-off engine holds from 0 … 27 samples before the bound P + ⌈119 × rate ÷ 6000⌉ + 2D, D the conversion's delay (8 kHz: 4237 of 4239; 48 kHz: 25415 of 25428; 96 kHz: 50829 of 50856, with P = rate ÷ 2), and its audible onset follows the position by about the conversion's delay (about 5 ms at 48 kHz), later than the chain's stages, which land at the position.

**What cannot be switched.** A stage that does not run on the stream cannot be switched live, in either direction: its setting was off at construction, or the one-pass's configured tap stops before it (`AfterVolumeAmp` reaches stage 11 only, `AfterSpeaker` 11 and 12, `AfterEarphone` 11 and 13, `RouteOutput` all three; the taps before the chain none). The call is refused with `Error::StageNotConfigured` (C status 36), whose text names the control and the fix. So the player's reported latency, drain and total length never depend on a live value: a bypassed stage's settle is simply not needed, and a live engine plays the all-on length (longer than a configured-off twin's where the twin runs no stage 10 or no chain stage; the extra samples are the drained input, exactly zero).

**The app's contract.**
1. Build an engine meant for live A/B with every playback stage on (the default) and, for the one-pass, a tap that reaches the stages you will switch (the default `RouteOutput` reaches all).
2. Apply presets and restored sessions as scheduled changes at position 0 of the next playback, never as construction settings. A switch of a stage the engine does not run is refused (36), not a no-op: load presets into an all-on engine, or read `settings()` first and skip the switches it cannot run.
3. `settings()` is a read-out, not a construction recipe for a live engine: `reconfigure(settings())` while a stage is bypassed builds that stage configured off, after which its switch is refused. To change a construction setting while a stage is bypassed: take `settings()`, set the four switches back to on, `reconfigure`, then re-schedule the bypasses at position 0 (they land on the first sample with no blend).
4. A finger toggle passes position 0 (or any passed position): it lands on the next sample the stream produces. The handle is single-threaded: queue the toggle into the audio thread; never call `schedule_change` from the UI thread while a processing call runs.
5. **Loudness.** Stage 11's bypass removes the whole stage, so the bypassed branch is the stage's input at unity: a level change of `output::stage11_bypass_level_db(route, position)` (C `rr_dr60_stage11_bypass_level_db`), positive meaning louder when bypassed, computed from the committed law (A-044, A-031, A-043), not measured, and stated for levels below the amplifier's knee. The **neutral positions**, where a bypass changes no level, are the speaker route's reference position and the earphone route's position 1:

   | Route | Position | Level change when bypassed |
   |---|---|---|
   | Earphone | the reference position (`VOLUME_REFERENCE`) | +19.2 dB |
   | Earphone | 1 | 0.0 dB |
   | Speaker | the reference position | 0.0 dB |
   | Speaker | 1 | −21.5 dB |
   | either | 0 | the on-branch is silent |

   An app offering the switch on the earphone route MUST either move the volume to position 1 first with its own transition (the recommended default), or apply the inverse of the published delta to its own output gain for the bypass's duration, or confirm before the first bypass away from the neutral position; the engine does not compensate.
6. While stage 11 is bypassed the volume position and the battery value act on the warm branch only (with their own 20 ms transitions) and change nothing audible: grey or annotate those controls; the settled new value is heard when the stage returns.
7. Stage 10's bypass is audible mainly at the 6 kHz host rate; at the conversion rates the resampler's own roll-off covers most of the band where it acts (the largest third-octave difference at 48 kHz on voice: 0.85 dB).
8. Landed values persist across stopped playbacks and `reset` (which re-applies `settings()` to every switch at once, the one-pass's stage 10 included: after a playback switched stage 10 off and a `reset`, the one-pass's stage 10 rests bypassed until a playback switches it back and `reset` runs, or `reconfigure`), so a playback can start with a stage bypassed and no blend; read the state at playback start from `settings()` or the log. Show "bypassed" at once; there is no blending state to show (a finger cannot toggle inside 20 ms).
9. `amp_clip_count` is 0 while stage 11 is bypassed by design (the knee is not heard); the card says "bypassed", never "no clipping". The route peak and the reconstruction peak read the blended signal.
10. The recommended toggle order is 12, 11, 13, 10; grey the inactive route's toggle (stage 12 on the earphone route, 13 on the speaker route): a change there lands and logs but is inaudible, and its phase advances so that a later route change finds the blend over. The whole output path is 11 + 12 (the speaker route) or 11 + 13 (the earphone route) scheduled at one position with one tag.
11. Wording: a bypassed stage's branch is the previous stage's output; label it "stage N bypassed (emulator)".

**Figures as the harness reads them** (`us1_bypass`, `bypass_matrix`, 1 kHz stored tones at −6 dBFS, the stage-only configurations at 6 kHz): the blend's click energy below an instantaneous switch's, under the 40 ms reader on the residual against the held run with the tone excluded: 53.2 – 53.3 dB on every asserted row at 6 kHz (stage 10, stage 11 at each neutral position, stages 12 and 13), 46.2 – 56.0 dB on the reported rows (the stages' own bands: stage 10 at 2.6 kHz, stage 12 at 300 Hz, stage 13 at 100 Hz, stage 11's corners at 100 Hz, its level-change rows), 53.2 – 53.6 dB at 48 kHz, and 2.1 – 2.9 dB on voice, where the content fills the band and the reader measures it; a reversal at the blend's midpoint 24.3 – 25.1 dB below a step (the slope's kink; reported); the blend equals y + w·(x − y) of the two constant runs within two f32 steps of the branches' magnitude on the narrowed output (exactly, in the core's 64-bit test), the midpoint weight exactly 0.5 at every rate; the per-sample cost in the all-on state with no landing +0.48 ns (+1.83 %) on the player and +0.45 ns (+0.61 %) on the one-pass against 0.20 (the solo A/B, 45 runs × 3 alternations at 48 kHz; budget 3 %, a 5 % ceiling on the CI runner); a bypassed steady state about +5.5 % and a stage-12 toggle every 200 ms about +6 % over the all-on state (reported; the worst 4096-sample block 157 – 298 µs of 85 333); on voice the blend's energy around a switch reads 0.35 – 2.0 dB below a step's (voice fills the band: the reader measures the content, as in spec 019).

```rust
let speaker = rr_dr60::controls().iter().position(|e| e.identifier == "output.speaker_enabled").unwrap() as u32;
p.schedule_change(Stream::Playback, 0, speaker, 0.0, 1)?;   // bypass stage 12 on the next sample
let delta = rr_dr60::output::stage11_bypass_level_db(Route::Earphone, p.volume()); // the loudness rule
```

## What the measurement harness checks

Spec 015's checks are in `specs/015-timed-changes/quickstart.md` (the block-size-1 plain-setter oracle, the partition identities, the speed reference, the log, the mutants, the golden and the A/B); spec 017's in `specs/017-live-record-values/quickstart.md` (the input-0 and pre-landing identities, the reference gate with the change at k(P), the limiter's carried hold, the hiss transition, the stop, the cross-stream landing, the mutants, the golden and the A/B).

Every modeled stage is built to a written specification in [`specs/`](../specs/), and a measurement harness (`crates/rr_dr60_harness`) checks the build against it at every supported host rate, with the tolerances the specification states: frequency responses, levels, timings, noise figures, codec metrics, output lengths, and bit-exact identities where the specification asks for them (the record and playback paths against the one-pass call, level 3 of the speed control against 0.12, each bypassed option against its earlier output). Golden files guard every feature's output against unintended change. Each feature's quickstart lists its reports and how to run them: [001](../specs/001-pipeline-skeleton/quickstart.md), [003](../specs/003-vas/quickstart.md), [004](../specs/004-device-rate-6khz/quickstart.md), [005](../specs/005-limiter-agc/quickstart.md), [006](../specs/006-record-playback-split/quickstart.md), [007](../specs/007-stage-manifest/quickstart.md), [008](../specs/008-celp-codec/quickstart.md), [009](../specs/009-output-chain/quickstart.md), [010](../specs/010-output-clamp/quickstart.md), [011](../specs/011-volume-ramp/quickstart.md), [012](../specs/012-mic-capsule-amp/quickstart.md), [013](../specs/013-playback-speed/quickstart.md), [016](../specs/016-block-meters/quickstart.md).

What the stages are modeled on differs: the microphone, the limiter and the output chain on the service manual's circuit with typical part values and *estimated* transistor parameters, the clamp on the codec's documented full scale, the VAS on the owner's manual's description plus *assumed* values, the two filter stages on the codec family datasheet's templates scaled to the device's 6 kHz clock (*assumed* scaling), the speech codec on the *class* of 4 kbit/s CELP coders of the period, the speed control on a product page's "+/-40%" (*assumed* ratios and mechanism), and the speaker on a generic small-speaker template. The harness proves that the emulator does what its specifications say. It does **not** prove that the specifications match a real RR-DR60: no stage has been checked against a real unit. See [Accuracy](../README.md#accuracy) in the README.
