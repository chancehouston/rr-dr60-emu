# Contract: Rust API additions (feature 009; research R-05 – R-07)

All items are in `rr_dr60`; every public item has rustdoc citing its FR and S-/A-IDs. Wording: "modeled on the circuit and the datasheet", the speaker "a template of a small speaker" (FR-018).

## Settings

| Item | Contract |
|---|---|
| `Settings::output: OutputSettings` | default `OutputSettings::DEVICE` (all three on) |
| `OutputSettings { volume_amp_enabled, speaker_enabled, earphone_enabled }` | `#[non_exhaustive]`, `Copy`, `Default = DEVICE`; `BYPASSED` (all off) for the harness; bypassing stage 11 removes the volume control, the coupling corner, the gain and the clipper together (unity gain at the codec's scale; the position retained and ignored) |
| `Settings::tap` default | `Tap::RouteOutput` (was `AfterPlayback`); `Settings::default_for(rate)` documents the change |

## Taps (appended; `#[non_exhaustive]`)

`Tap::RouteOutput` (default; the selected route's last stage), `Tap::AfterVolumeAmp` (after stage 11 on either route), `Tap::AfterSpeaker`, `Tap::AfterEarphone` (refused with the other route selected). `Tap::AfterPlayback` keeps its meaning (after stage 10, before the chain) and its docs say "version 0.8's default output".

## Controls

| Call | Contract |
|---|---|
| `Pipeline::set_volume(&mut self, position: f32) -> Result<(), Error>` | finite and 0 ≤ position ≤ 1, else `Error::InvalidVolume { position }` with the position unchanged; takes effect at the next processing call; survives `reset` and `reconfigure`; real-time safe |
| `Pipeline::volume(&self) -> f32` | the position; default `output::VOLUME_REFERENCE` |
| `Pipeline::set_route`, `route` | unchanged signatures; a route change resets the chain's filters at the next processing call |

## Constants (`rr_dr60::output`)

`VOLUME_REFERENCE: f32` (≈ 0.0976; A-043), `CLIP_CEILING_V: f32` (1.327; A-031), `AMP_GAIN_DB: f32` (22.67; A-031), `SPEAKER_SCALE: f32` (`K_SPK`, 10.25; A-042), `VOLUME_CORNER_HZ` (32), `AMP_CORNER_HZ` (159), `EARPHONE_CORNERS_HZ: [f32; 2]` (110, 190 nominal; A-032), `SPEAKER_RESONANCE_HZ`, `SPEAKER_Q`, `SPEAKER_SHELF_DB` (A-010), `SETTLE_SAMPLES(rate) -> u32` (`N_OUT`); all read-only, cited.

## Errors (appended; `#[non_exhaustive]`)

`Error::InvalidVolume { position: f32 }`; `Error::TapNotOnRoute { tap: Tap, route: Route }`.

## Pipeline behaviour

| Call | Change |
|---|---|
| `process`, `process_with_events` | read route and volume at entry; `TapNotOnRoute` if the tap is a named route tap of the other route; the output is at the configured tap (default: the route output) |
| `process_in_place*` | the same check; on refusal produce 0 (no error channel) |
| `start_playback`, `play` | read route and volume at entry; `TapNotOnRoute` cannot occur (the paths ignore the tap; the player's chain always ends at the route output) |
| `latency_samples`, `record_latency_samples`, `playback_latency_samples` | unchanged values (FR-008) |
| `drain_samples` | grows by `N_OUT(rate)` when any output stage runs |
| `reset`, `reconfigure` | the chain's filters reset; route and volume kept |

## Documentation

`lib.rs` crate docs: the output chain, the route's sound, the output-scale rule, the default volume and the clipping above it, the audibility note (the corners sit below the record band edge), the host-rate caveat for clipped output.
