# Data Model: Output Chain (feature 009)

Extends the data models of specs 006 (paths, transport, route) and 008 (the one-pass with the codec). Stages 3 – 10, the memory and the stored form are unchanged.

## The chain in the playback path (research R-01)

```
stored device samples / one-pass device stream → [9 decoder] → [10 stage] → device→host interpolator → f64 y
   → [OutputChain at the host rate]:
        v = p · hpf_vol(y)                          (stage 11, both routes: the 32 Hz coupling corner at the control's input, then the position p)
        speaker route: a = clip( K_SPK · hpf_amp(v) )   (stage 11's amplifier path: the 159 Hz input corner, gain, knee)
                       s = speaker(a)                    (stage 12: resonance biquad + shelf, peak gain 1.0)
        earphone route: e = hpf_out( hpf_in(v) )         (stage 13: the follower's input and output coupling corners, 110 and 190 Hz nominal; no clipper)
   → narrow_out (f32) → host output
Taps: AfterPlayback = y (0.8's default) · AfterVolumeAmp = a (speaker) or v (earphone) · AfterSpeaker = s · AfterEarphone = e · RouteOutput (default) = s or e
Bypasses: volume_amp_enabled=false → v = y and no amplifier path (a = y); speaker_enabled=false → s = a; earphone_enabled=false → e = v.
```

With every output stage bypassed the chain is the identity and the route is a label, as in 0.8.

## Constants (research R-02 – R-04; `rr_dr60::output`; contracts/output-chain-design.md)

| Name | Value (plan; the script's print is authoritative) | Unit | Cited to |
|---|---|---|---|
| `V_CLIP_V` | 1.327 | V peak differential | A-031 (S-006 55 mW / 16 Ω at 10 % THD; S-002 110 mW / 8 Ω) |
| `AMP_GAIN` | 13.6 (22.67 dB) | ratio | A-031 (S-002 R16 10 kΩ input, R18 68 kΩ feedback; S-006 formula 2 Rf / Ri; read from the sheet at plan time) |
| `CODEC_FULL_SCALE_V` | 1.0 | V peak | A-030 |
| `VOLUME_REFERENCE` | 0.0976 (= V_CLIP ÷ (AMP_GAIN × 1.0)) | position | A-043 |
| `K_SPK` | 10.25 (= AMP_GAIN ÷ V_CLIP) | ratio | A-042 |
| `KNEE_START` | 0.85 of the ceiling | ratio | A-031 (the knee's shape, low) |
| `F_VOL_HZ` | 32 | Hz | A-031 (C14 0.1 µF into the 50 kΩ control; both routes) |
| `F_AMP_HZ` | 159 | Hz | A-031 (C8 0.1 µF into R16 10 kΩ at the inverting input) |
| `F_EAR_IN_HZ` | 110 nominal (70 – 145) | Hz | A-032 (C8 + C60 in series into R16 + R65 + the follower's loaded input impedance; β-dependent) |
| `F_EAR_OUT_HZ` | 190 nominal (110 – 300) | Hz | A-032 (C61 10 µF into 16 Ω plus the follower's output resistance; β-dependent) |
| `SPK_F0_HZ`, `SPK_Q` | 750, 1.8 | Hz, ratio | A-010 (template) |
| `SPK_SHELF_DB`, zero / pole | +4.5; 1120 / 1880 Hz | dB, Hz | A-010 (template) |
| `N_OUT` per rate | 500 / 667 / 1 333 / 3 672 / 3 997 / 7 344 / 7 993 | host samples (83.3 ms) | research R-08 (2⁻²⁴ of the 32 Hz corner) |

## Settings (contracts/rust-api.md)

| Item | Type | Default | Meaning |
|---|---|---|---|
| `Settings::output` | `OutputSettings` | `OutputSettings::DEVICE` | the three bypasses |
| `OutputSettings::volume_amp_enabled` | `bool` | `true` | stage 11 (volume control and amplifier path) |
| `OutputSettings::speaker_enabled` | `bool` | `true` | stage 12 |
| `OutputSettings::earphone_enabled` | `bool` | `true` | stage 13 |
| `Settings::tap` default | `Tap::RouteOutput` | | was `Tap::AfterPlayback` (0.8) |

## Controls (engine-level, not settings; spec 006 FR-015 pattern)

| Control | Set / read | Range | Default | Applied |
|---|---|---|---|---|
| route | `set_route`, `route` (unchanged) | `Route::{Speaker, Earphone}` | `Speaker` | at the next processing call (block boundary); the newly selected branch and the one left start from rest |
| volume position | `set_volume(f32) -> Result<(), Error>`, `volume() -> f32` | finite, 0 … 1 | `VOLUME_REFERENCE` | at the next processing call; the multiplier only |

Both survive `reset` and `reconfigure`; both are copied into the one-pass's and the player's `OutputChain` by `process*`, `start_playback` and `play`.

## `OutputChain` state (research R-01, R-03)

| Field | Type | Reset |
|---|---|---|
| `route`, `volume: f64` | controls | kept (controls are not state) |
| `run_amp`, `run_speaker`, `run_earphone` | `bool` | from settings |
| `tap` | `Tap` (the point `process` returns) | from settings |
| `vol_hpf` | `FirstOrder` (32 Hz, both routes) | zero |
| `amp_hpf` | `FirstOrder` (159 Hz) | zero |
| `speaker_res` | `Biquad` | zero |
| `speaker_shelf` | `FirstOrder` | zero |
| `ear_in`, `ear_out` | `FirstOrder` (110, 190 Hz nominal) | zero |

`FirstOrder { b0, b1, a1, s }`: `y = b0·x + s; s ← b1·x − a1·y`, state flushed to exactly 0.0 below 1e-30 (the biquad's rule). `process(y: f64) -> f64` returns the sample at the configured tap; `set_controls(route, volume)` resets the filters when the route changes; `reset()` zeroes the filters; `runs_any()` tells the path whether the drain gains `N_OUT`.

## Taps (revised; contracts/rust-api.md)

| Variant | C value | Meaning | Refused when |
|---|---|---|---|
| `AfterRecord`, `AfterAgc`, `AfterVas`, `AfterDecoder` | 0, 2, 3, 4 | unchanged | |
| `AfterPlayback` | 1 | after stage 10, before the chain (0.8's default output) | |
| `RouteOutput` | 5 | **default**: the selected route's last stage | |
| `AfterVolumeAmp` | 6 | after stage 11 on either route | |
| `AfterSpeaker` | 7 | the speaker route's output | the earphone route is selected (`TapNotOnRoute`) |
| `AfterEarphone` | 8 | the earphone route's output | the speaker route is selected (`TapNotOnRoute`) |

## Errors (appended; `#[non_exhaustive]`)

| Variant | C status | When |
|---|---|---|
| `InvalidVolume { position: f32 }` | 23 `RR_DR60_STATUS_INVALID_VOLUME` | `set_volume` with a non-finite or out-of-range position; the position is unchanged |
| `TapNotOnRoute { tap: Tap, route: Route }` | 24 `RR_DR60_STATUS_TAP_NOT_ON_ROUTE` | a processing call with `AfterSpeaker` / `AfterEarphone` and the other route selected; nothing processed (`process_in_place*` produce 0) |

## Drain (research R-08; spec 006 FR-008 restated)

`drain = ⌈N₁₀ · m / l⌉ + 2D + N_OUT(rate)` when any output stage runs, else spec 006's figure. `drain_samples()` reports it. The playback's total length is `⌈K · m / l⌉ + drain`.

## Per-rate figures (script-printed; pinned by core tests)

| Rate | `N_OUT` | 1 kHz group delay, speaker route (ms) | earphone route (ms) | latency change |
|---|---|---|---|---|
| 6 000 … 96 000 | table above | ≈ 0.3 (template 0.27 + the corners) | ≈ 0.1 | 0 at every rate |

## Reports

Unchanged: `BlockInfo`, `RecordInfo`, `StopOutcome`, `PlayInfo`, `EngineState` keep their fields (the volume has its own getter; the product notes' meters are E-A, out of scope).

## Harness configurations (research R-10)

| Name | Stages on | Tap | Route (set by the check) |
|---|---|---|---|
| `output_only_speaker` | 11, 12 | `RouteOutput` | speaker |
| `output_only_earphone` | 11, 13 | `RouteOutput` | earphone |
| `default_output` | all | `RouteOutput` | speaker |
| `default_earphone` | all | `RouteOutput` | earphone |

Every pre-009 configuration sets the three bypasses off (`output: OutputSettings::BYPASSED`).

## Assumptions touched

A-031 (adopted, corrected, extended), A-032 (adopted, corrected), A-010 (revised: the template's chosen figures), A-042 (output scale), A-043 (volume default), A-044 (volume position as the full-swing fraction); S-006 corrected on the branch.
