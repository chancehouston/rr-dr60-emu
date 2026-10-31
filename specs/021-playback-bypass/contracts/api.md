# API contract: Live Playback-Stage Bypass (spec 021, version 0.21.0)

## Rust (`rr_dr60`)

The existing scheduling call carries the four switches:

```rust
pipeline.schedule_change(Stream::Playback, position, control, value, tag)?; // control ∈ {5, 22, 23, 24}; value exactly 0.0 or 1.0
pipeline.schedule_change(Stream::OnePass,  position, control, value, tag)?; // control ∈ {22, 23, 24}; 5 refused (ControlNotOnStream)
```

- `value` 1.0 = the stage on, 0.0 = bypassed (the setting's own sense). Any other value, non-finite included → `Error::InvalidControlValue { control, value }`.
- A stage that does not run on the stream (its construction setting off, or the one-pass's tap not reaching it: `AfterVolumeAmp` reaches 11 only, `AfterSpeaker` 11 and 12, `AfterEarphone` 11 and 13, `RouteOutput` all) → `Error::StageNotConfigured { control }` (new), for either value.
- `Stream::Record` → `Error::ControlNotOnStream`.
- Landing: the top of host sample `position` (stage 10: the device sample `changes::speed_landing(position, rate)`); a stream's first produced sample switches at once; logged as a `ChangeEntry` (before/after 0 or 1, the stream word); `settings()` reflects it; a stage-11 – 13 landing on the player is applied by the one-pass at its next call's first sample with the same tag (spec 017 D4), the player adopting at `start_playback` without an entry.
- `MAX_CHANGES_PER_CALL`: 139 → **142** (not an ABI change: the calls take a capacity and return the count; a host that sizes by the constant recompiles; a 0.20 binary's 139-entry buffer may be short by up to three entries in one one-pass call, reported by the count, never overrun).
- `output::stage11_bypass_level_db(route, position) -> f32` (new): stage 11's bypass level change in dB, positive = louder when bypassed, +∞ at position 0, stated below the knee. A deviation from D10's "the status and the bound only", accepted at the plan.
- `controls()`: entries 5, 22, 23, 24 read `ControlTiming::Live`.
- Test hooks (`__test-hooks`): `BypassMutation { weight_inverted, fade_step, fade_linear, stage_cold, count_ungated, phase_frozen_off_route }`; `stage10_trace()` (the half's output per device sample of the last play call, up to 8192).

## C (`rr_dr60_ffi`, `include/rr_dr60.h`)

- `RR_DR60_STATUS_STAGE_NOT_CONFIGURED = 36` (appended), its header comment the `Display` text plus "see the guide, Live stage bypass".
- `RR_DR60_MAX_CHANGES_PER_CALL` 139 → 142.
- `float rr_dr60_stage11_bypass_level_db(RrDr60Route route, float position)` (no handle).
- `rr_dr60_schedule_change` carries the four controls. The header is regenerated and drift-checked; the smoke gains `--bypass` diffed against `examples/bypass_rows.rs`.

## Behavioural contract (the app's)

1. **Construct for A/B**: an engine meant for live A/B is built with every playback stage on (`Settings::new`'s default) and, for the one-pass, a tap that reaches the stages you will switch (the default `RouteOutput` reaches all; `AfterPlayback` reaches none of the chain's).
2. **Presets and restored sessions** apply stage switches as scheduled changes at position 0 of the next playback, never as construction settings. A switch of a stage the engine does not run is **refused (36), not a no-op**: load presets into an all-on engine, or read `settings()` first and skip the switches it cannot run.
3. **`settings()` is a read-out**, not a construction recipe for a live engine: `reconfigure(settings())` while a stage is bypassed builds that stage configured off, after which its switch is refused. To change a construction setting while a stage is bypassed: take `settings()`, set the four switches back to on, `reconfigure`, then re-schedule the bypasses at position 0 (they land on the first sample with no blend).
4. **A finger toggle** passes position 0 (or any passed position): it lands on the next sample the stream produces. The handle is single-threaded: queue the toggle into the audio thread; never call `schedule_change` from the UI thread while a processing call runs.
5. **Stage 11's level**: its bypass changes the level by `stage11_bypass_level_db(route, position)` (earphone +19.2 dB at the reference position, 0 at position 1; speaker −21.5 dB at position 1, 0 at the reference position). **Loudness rule**: an app offering the switch on the earphone route MUST either move the volume to position 1 (the neutral position) first with its own transition, or apply the inverse of the published delta to its own output gain for the bypass's duration, or confirm before the first bypass away from the neutral position; the engine does not compensate (D11). The Workbench default is the first.
6. **While stage 11 is bypassed** the volume position and the battery value act on the warm branch only and change nothing audible: grey or annotate those controls; the settled new value is heard when the stage returns.
7. **Stage 10** is audible mainly at the 6 kHz host rate (at the conversion rates the resampler's roll-off covers most of its band); its audible onset is about the resampler's delay after the position (about 5 ms at 48 kHz), later than the chain's stages; schedule it earlier to hear it together with them.
8. **Lifecycle for the card**: landed values persist across stopped playbacks and `reset`, so a playback can start with a stage bypassed and no blend; read the state at playback start from `settings()` or the log, not from a new entry. Show "bypassed" at once; there is no "blending" state to show (a finger cannot toggle inside 20 ms).
9. **The meter**: `amp_clip_count` is 0 while stage 11 is bypassed by design; the card says "bypassed", never "no clipping".
10. **Order and routes**: the recommended toggle order is 12, 11, 13, 10; grey the inactive route's toggle (12 on the earphone route, 13 on the speaker route): a change there lands and logs but is inaudible until the route changes.
11. **Wording**: a bypassed stage's branch is the previous stage's output, still band-limited, coded and limited; label it "stage N bypassed (emulator)", never "raw", "clean", "unfiltered" or "the true signal".
