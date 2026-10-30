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
- **Real-time safe.** `process`, `process_in_place`, `process_with_events` and `reset` never allocate, lock or do I/O, and neither do the recording and playback calls (`start_recording`, `record`, `stop_recording`, `start_playback`, `play`, `stop_playback`), the live controls (`set_route`, `set_volume`, `set_playback_speed` and their reads), the state and figure reads, the meter read-out (`meters`, `rest_meters`), `export_len`, `playback_length_samples` and the latency reads. `erase`, `erase_all` (a bounded move of stored samples), `export` and `import` (a checksum over the recording) are not. `Pipeline::new` and `reconfigure` allocate, so call them outside the audio callback.
- **Cost.** Measured on a laptop at 48 kHz: about 60 ns per sample (about 350× real time) with the limiter and the VAS on and the speech codec bypassed; below its knee the limiter costs one exponential per sample and nothing more. With the speech codec on it runs at about 135× real time at 48 kHz (about 155 ns per host sample, 1.2 µs per device sample against the 8 µs budget of spec 008; the worst 64-sample block takes about 0.14 ms of its 1.33 ms) and about 210× at 6 kHz. Recording and playing back through the memory costs about 2 % more than the one-pass call. The output chain adds about 19 % on the speaker route and 6 % on the earphone route to the codec-bypassed one-pass (68.2 and 60.9 against 57.4 ns per host sample; spec 009 SC-007), the microphone path about 4 ns (6 %) more, and the playback speed control about 10 % at the levels other than normal speed (about 1.5 % at normal speed).
- **Deterministic.** The same input, settings and seed give bit-identical output for any block size and on every supported platform. The golden files (one per feature; the speech codec's entries carry a digest of the frame bytes as well as of the audio) are checked on Linux, macOS and Windows (x86-64 and ARM64), in the iOS simulator and on an iOS device.
- **A C API** with a generated header ([`rr_dr60.h`](../crates/rr_dr60_ffi/include/rr_dr60.h)), and a [Swift package](../swift/README.md) that wraps it as an XCFramework.

Not yet: noise reduction (stage 6), the clock (14) and the power supply (15); the stage manifest lists what each modeled stage leaves out.

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

RrDr60Settings s = rr_dr60_settings_default(48000);
RrDr60Pipeline *p = NULL;
if (rr_dr60_create(&s, &p) != RR_DR60_STATUS_OK) { /* handle the error */ }
RrDr60BlockInfo info;
rr_dr60_process(p, in, out, frames, &info);          /* real-time safe; in == out is allowed */
/* only out[0 .. info.produced) is output: the VAS drops pauses by default */
rr_dr60_destroy(p);
```

Build the static library with `cargo build -p rr_dr60_ffi --release`. `crates/rr_dr60_ffi/tests/c/run_smoke.sh` shows a complete build and link. From Swift, use the [Swift package](../swift/README.md) (`scripts/build-xcframework.sh`, then `import RRDR60C`); it exposes the C API as is, with no Swift-native wrapper yet.

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
  RrDr60ManifestHeader h;
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
    RrDr60StageEntry e;
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

Since [spec 014](../specs/014-control-manifest/spec.md) the library also carries a **control manifest**: constant data describing every adjustable setting and live control, 29 today (the three live controls, then every field of `Settings` in declaration order). An app builds its sliders, switches and pickers from it instead of copying ranges and defaults by hand. Each entry gives a stable identifier (the setting's path, `vas.sensitivity`, or the live control's name, `volume`; the key for localization), a name, its **kind**, its **timing**, the paths it applies to, its signal-chain stage (0 for none), its value (a switch; an integer with exact `u64` bounds; a continuous `f32` range; or a choice whose options carry the value the engine takes), its default, its unit, the source and assumption IDs it rests on, and one sentence on what changing it does to the sound. Each sentence describes the default engine configuration, every other stage on and the speech coder on, which is what an app user hears; with other stages switched off a control can act differently (with the coder bypassed, for example, switching the converter's clipping off lets loud peaks through).

The **kinds**: a *device control* is one the owner's manual or the service manual gives the user (route, volume, playback speed, microphone sensitivity); a *unit condition* is an audible difference an owner could hear between two units or over one unit's life (the hiss level; component spreads that are inaudible or not directly hearable stay model internals); a *model internal* is a value the device has that its user cannot reach (the limiter's threshold, attack and release; the pause threshold, delay and resume delay); an *emulator option* is something the device cannot do (the stage switches, the pause handling, the memory size, the output clamp, the listen point); a *host parameter* is set by the host (the seed, the host sample rate). The **timing classes**: *live* (a `set_*` call while running; a live control's value is kept by `reset` and `reconfigure`), *idle* (applied by `reconfigure` when nothing records or plays), *idle, memory-dependent* (also refused while the stored recordings do not fit: the speech coder switch and the memory figures) and *host-fixed* (the host rate, matched to the audio session).

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

An index past any list is refused with `RR_DR60_STATUS_NO_SUCH_CONTROL` and nothing is written. `RrDr60Control` and `RrDr60ControlOption` have no size field; they join the pre-1.0 ABI pass.

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
Since 0.13 a recording can be played at the device's **five playback speeds**. The owner's manual describes a SPEED CONTROL switch and five levels, level 3 normal, level 1 the slowest and level 5 the fastest; Panasonic's 1999 US product page gives the only figure, "Slow/Fast Play (+/-40%)" ([S-001, S-012](hardware/sources.md)). The engine plays the levels at **3/5, 4/5, 1, 6/5 and 7/5** of normal speed (0.6, 0.8, 1.0, 1.2, 1.4: the speed reading of "+/-40%", linear spacing assumed; [A-054](hardware/assumptions.md), low confidence; the other reading, a duration change of ±40 %, is registered beside it).

- **The pitch moves with the speed.** Level 1 plays about 67 % longer and lower, level 5 shorter and higher. This is the modeled default: a resampling of the decoded stream at the 6 kHz device rate between the speech decoder and the codec's reconstruction filter (A-054's mechanism (b)). How the device itself does it is not documented: the codec cannot be slowed by lowering its clock within its stated range ([S-003](hardware/sources.md)), so the slow levels are a DSP process, and the research of 2026-10-17 leans weakly toward a **pitch-keeping** device (time-scaling the decoder's output). One capture of a real unit settles it (P1 in the [hardware-evidence template](../.github/ISSUE_TEMPLATE/hardware_evidence.yml)); a pitch-keeping mode would be a later feature. At the fast levels the content that would no longer fit below the device's 3 kHz band is removed first rather than folding back; the emulator's resampler keeps every other product at least 60 dB down (a device of 1998 may have done less).
- **A live control, playback only.** `set_playback_speed(level)` (1 – 5; `playback_speed()` reads it back, default 3) works at any time, like the volume and the route, and is kept by `reset` and `reconfigure`. From the first device sample a `play` call computes after the change, the playback continues at the new level from the place it reached: nothing restarts, skips or repeats (a change into level 3 waits until the place is a whole stored sample, at most 4 device samples, 0.7 ms). A change is a step: the new level's band takes over at once, so a change to or from levels 4 – 5 can tick on voice content above about 1.6 kHz. The record path and the one-pass processing ignore it, as on the device, where the level changes only during playback.
- **Level 3 is the unchanged output**: bit-identical to 0.12, every play report included.
- **Length.** A recording of K stored device samples plays its own part in ⌈K ÷ r⌉ device samples, then a short tail of the resampler's response (at most 46 device samples, none at level 3), then the unchanged drain: in practice a recording plays about its length ÷ r, so a 10 s recording takes about 16.7 / 12.5 / 10 / 8.3 / 7.1 s at levels 1 – 5, plus the drain. `playback_length_samples(number, level)` returns the whole length in host samples for a constant level (K is `recording_length(number)`; with level changes, show progress as `place ÷ recording_length`), and `PlayInfo::place` reports the place in the recording, in stored device samples, after every `play` call (it reaches the recording's length at the end), for a progress display.

```rust
let (number, level) = (1, 1);                  // recording 1 at SLOW (0.6, lower pitch)
p.set_playback_speed(level)?;
let total = p.playback_length_samples(number, level)?; // host samples, the drain included
p.start_playback(number)?;
let info = p.play(&mut out)?;                  // info.place: stored samples reached
p.set_playback_speed(5)?;                      // FAST from the next play call
```

```c
rr_dr60_set_playback_speed(p, 1);              /* RR_DR60_STATUS_INVALID_SPEED_LEVEL outside 1 - 5 */
uint64_t total = 0;
rr_dr60_playback_length_samples(p, 1, 1, &total);
/* RrDr60PlayInfo.place since 0.13 (the struct grew 32 -> 40 bytes: rebuild against the header) */
```

> **Modeled on the documents, not measured against a unit.** The five ratios, the pitch behavior and the resampler's quality are assumptions ([A-054](hardware/assumptions.md)); a capture of a real unit playing a tone and a sustained vowel at each level decides them.
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

## Meters (one read-out per processing call)

Since 0.15 the engine keeps a **meter read-out**: eleven figures computed from the same values the output is computed from, describing the samples of the last non-empty processing call (a one-pass `process*`, `record`, `play` or the stop's flush). Read it after the call with `meters()` (Rust) or `rr_dr60_meters` (C): a plain copy, real-time safe, never changing the engine or its output. Every earlier output is bit-identical with the meters in place; nothing about them is settable, and they add no assumption: each figure reads a stage that is already modeled, and inherits that stage's status and open questions (the IDs in the table).

The figures of a call describe the samples that call **processed**, not the samples it output: the stage 4 and 5 figures, and the recorder's whole group, describe device samples that leave the output up to the path's latency later (the one-pass at 48 kHz by default: 1923 host samples, about 40 ms; 483 with the speech codec off; at other rates `latency_samples()` reports the one-pass figure and `record_latency_samples()` the record path's share; the recorder's samples are stored and heard only when the item is played). A figure whose point the call did not reach keeps its **rest** value; before any call, after `reset` and after `reconfigure` every figure is at rest (`rest_meters()`; `Meters::default()` is the all-zero value, the C zero-struct's twin, whose hang is 0 rather than the configured length). A zero-length call, a skipped flush and the start, stop-playback and erase calls leave the read-out as it was.

| Meter | Field (Rust; C) | Point | Unit | Rest | Lead over the audible output | Rests on |
|---|---|---|---|---|---|---|
| Input peak | `input_peak` | the input after the engine's sanitizing of non-finite samples | linear full-scale, ≥ 0 (20·log₁₀ for dBFS) | 0 | one-pass: the call's own samples leave the output `latency_samples` later; recorder: stored | the full-scale convention (A-030) |
| Limiter-input peak | `limiter_input_peak` | stage 2's output, the level entering the limiter (stage 1's with stage 2 off; the sanitized input with both off) | linear full-scale, ≥ 0; above 1.0 is possible (stage 2 limits softly near +9 and +17 dBFS) | 0 (also at `Tap::AfterMicCapsule`) | as the input peak: the microphone stages add no latency | A-046, A-051, A-030 |
| Gain reduction | `gain_reduction_db` | stage 3: the deepest attenuation the limiter applied to any sample of the call | dB, ≥ 0 (0 = unity gain; exactly 0 below the hold's unity floor) | 0 (also with the limiter off) | as the input peak: the limiter runs at the host rate with latency 0 | A-028, A-034, A-035 (modeled on the circuit with estimated transistor parameters, not measured) |
| Hold charge | `hold_charge` | stage 3: the limiter's hold capacitor at the end of the call, as a fraction of the hold at the model's deepest attenuation | 0 – 1 (0 discharged; landmarks below) | 0 (also with the limiter off) | a state, not a per-call figure: it describes the limiter at the end of the call | A-028, A-034, A-035 |
| Record-filter peak | `record_filter_peak` | stage 4: the largest device sample after the record band-limit, **before** the converter's full-scale clamp | linear full-scale, ≥ 0; above 1.0 means the clamp bounded samples | 0 | device samples: they leave the one-pass output later by the latency less the band-limit's own delay; in the recorder they are stored | A-027, A-030 |
| Converter-clamp count | `adc_clamp_count` | stage 4: the device samples of the call the full-scale clamp changed (a sample exactly at full scale is not counted) | device samples, per call | 0 (also with `adc_clamp_enabled` off) | as the record-filter peak | A-030 |
| Voice-activation state | `vas_paused` (C: 0 or 1) | stage 5: paused or recording at the end of the call, the gate's own state machine (also during the stop flush, which keeps every sample) | — | recording (false; also with the gate off) | describes the end of the call's device samples; a pause lands on the next non-sound device sample, possibly in the next call | S-001, A-022, A-023, A-024 |
| Hang remaining | `hang_remaining_samples` | stage 5: the further non-sound device samples the gate keeps before pausing (the hang length minus the non-sound samples since the last sound) | device samples (÷ 6000 for seconds); the full hang when the last sample was sound or the gate is off; 0 while paused | the configured hang | as the state | A-023 |
| Reconstruction peak | `reconstruction_peak` | stage 10's output after the conversion to the host rate, before the output chain | linear full-scale, ≥ 0 | 0 | the call's own output, ahead of it by the output chain's group delay only (0.31 ms speaker, 0.07 ms earphone at 48 kHz) | A-027 |
| Route peak | `route_peak` | the selected route's output **before** the optional output clamp (also with the output chain bypassed) | linear full-scale, ≥ 0; 1.0 is the amplifier's clip ceiling on the speaker route and the earphone scale on the earphone route; the speaker route reads up to about 2.3 on clipped or transient material | 0 (at `Tap::RouteOutput` only) | none: the call's own output | A-042, A-031, A-010, A-032 |
| Output-clamp count | `output_clamp_count` | the host samples of the call the output clamp bounded | host samples, per call | 0 (also with `output_clamp_enabled` off) | none: the call's own output | A-042; the clamp is an emulator option, not a device behavior (spec 010) |

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

The read-out is checked against oracles outside the engine at every host rate (the tap outputs for the peaks, a limiter-bypassed twin for the gain, clamp-bypassed twins for the counts, a reference gate for the voice activation), is bit-identical on every golden target and between Rust and C, and costs a few compare-and-select operations per sample (the figures in the [CHANGELOG](../CHANGELOG.md)). What an app shows with these figures is the app's wording: the product notes ask for "limiter hold" rather than a bare "charge", for "recording / paused" rather than detection words, and for the output-clamp count to be shown as an emulator option.

## What the measurement harness checks

Every modeled stage is built to a written specification in [`specs/`](../specs/), and a measurement harness (`crates/rr_dr60_harness`) checks the build against it at every supported host rate, with the tolerances the specification states: frequency responses, levels, timings, noise figures, codec metrics, output lengths, and bit-exact identities where the specification asks for them (the record and playback paths against the one-pass call, level 3 of the speed control against 0.12, each bypassed option against its earlier output). Golden files guard every feature's output against unintended change. Each feature's quickstart lists its reports and how to run them: [001](../specs/001-pipeline-skeleton/quickstart.md), [003](../specs/003-vas/quickstart.md), [004](../specs/004-device-rate-6khz/quickstart.md), [005](../specs/005-limiter-agc/quickstart.md), [006](../specs/006-record-playback-split/quickstart.md), [007](../specs/007-stage-manifest/quickstart.md), [008](../specs/008-celp-codec/quickstart.md), [009](../specs/009-output-chain/quickstart.md), [010](../specs/010-output-clamp/quickstart.md), [011](../specs/011-volume-ramp/quickstart.md), [012](../specs/012-mic-capsule-amp/quickstart.md), [013](../specs/013-playback-speed/quickstart.md), [016](../specs/016-block-meters/quickstart.md).

What the stages are modeled on differs: the microphone, the limiter and the output chain on the service manual's circuit with typical part values and *estimated* transistor parameters, the clamp on the codec's documented full scale, the VAS on the owner's manual's description plus *assumed* values, the two filter stages on the codec family datasheet's templates scaled to the device's 6 kHz clock (*assumed* scaling), the speech codec on the *class* of 4 kbit/s CELP coders of the period, the speed control on a product page's "+/-40%" (*assumed* ratios and mechanism), and the speaker on a generic small-speaker template. The harness proves that the emulator does what its specifications say. It does **not** prove that the specifications match a real RR-DR60: no stage has been checked against a real unit. See [Accuracy](../README.md#accuracy) in the README.
