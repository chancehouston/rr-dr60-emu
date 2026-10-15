# Data Model: Output Chain (feature 009)

Extends the data models of specs 006 (paths, transport, route) and 008 (the one-pass with the codec). Stages 3 – 10, the memory and the stored form are unchanged.

## The chain in the playback path (research R-01)

```
stored device samples / one-pass device stream → [9 decoder] → [10 stage] → device→host interpolator → f64 y
   → [OutputChain at the host rate]:
        u = hpf_vol(y)                              (stage 11, both routes: the 32 Hz coupling corner at the control's input)
        speaker route: a = clip( K_SPK(p) · hpf_amp(u) ) (stage 11's amplifier path: the 159 Hz input corner, the loaded gain law for the position p over the ceiling, the knee)
                       s = speaker(a)                    (stage 12: resonance biquad + shelf, peak gain 1.0)
        earphone route: e = hpf_out( hpf_in( K_EAR(p) · u ) ) (stage 13: the earphone route's loaded law for p, then the follower's input and output coupling corners, 120 and 280 Hz nominal; no clipper modeled, see A-032)
   → narrow_out (f32) → host output
Taps: AfterPlayback = y (0.8's default) · AfterVolumeAmp = a (speaker) or v (earphone) · AfterSpeaker = s · AfterEarphone = e · RouteOutput (default) = s or e
Bypasses: volume_amp_enabled=false → u = y, K_SPK = K_EAR = 1 and no amplifier path (a = y); speaker_enabled=false → s = a; earphone_enabled=false → e = K_EAR(p)·u.
K_SPK(p) and K_EAR(p) are per-block scalars from the loaded volume law (A-044); at position 0 both are exactly 0.0.
```

With every output stage bypassed the chain is the identity and the route is a label, as in 0.8.

## Constants (research R-02 – R-04; `rr_dr60::output`; contracts/output-chain-design.md)

| Name | Value (plan; the script's print is authoritative) | Unit | Cited to |
|---|---|---|---|
| `V_CLIP_V` | 1.15 nominal at the 2.8 V rail the schematic marks (S-002) (range 1.0 – 1.4; 1.33 at the datasheet's 3.0 V) | V peak differential | A-031 (S-006 pin swing limits and rated power; S-002 VDD 2.8 V) |
| `AMP_GAIN` | 13.6 (22.67 dB) at position 1; the loaded law `p · 2 Rf / (Ri + 50 kΩ · p (1 − p))` elsewhere | ratio | A-031, A-044 (S-002 R16 10 kΩ input, R18 68 kΩ feedback, VR1 50 kΩ; S-006 formula 2 Rf / Ri) |
| `CODEC_FULL_SCALE_V` | 1.0 | V peak | A-030 |
| `VOLUME_REFERENCE` | 0.13345174 (the position at which the loaded law puts a full-scale codec output at the ceiling; script-solved, `f32`-rounded) | position | A-043 |
| `K_SPK(p)`, `K_EAR(p)` | the two routes' per-block scalars from the loaded law over the route's full swing (unity at P_REF and at position 1 respectively; K_SPK(1) = 11.8, 21.5 dB in the route's scale) | ratio | A-042, A-044 |
| `KNEE_START` | 0.85 of the ceiling | ratio | A-031 (the knee's shape, low) |
| `F_VOL_HZ` | 32 | Hz | A-031 (C14 0.1 µF into the 50 kΩ control; both routes) |
| `F_AMP_HZ` | 159 | Hz | A-031 (C8 0.1 µF into R16 10 kΩ at the inverting input) |
| `F_EAR_IN_HZ` | 120 nominal (65 – 130) | Hz | A-032 (C8 + C60 in series into R16 + R65 + the follower's loaded input impedance; hFE 290 – 460) |
| `F_EAR_OUT_HZ` | 280 nominal (240 – 320) | Hz | A-032 (C61 10 µF into 16 Ω plus the follower's ≈ 40 Ω output resistance; hFE 290 – 460) |
| `Z_EAR_OHM` | 6 400 | Ω | A-044 (the earphone branch's in-band input impedance that loads the control; the 24 kΩ low-frequency value applies below the 280 Hz output corner only) |
| `SPK_F0_HZ`, `SPK_Q` | 750, 1.8 | Hz, ratio | A-010 (template) |
| `SPK_SHELF_DB`, zero / pole | +4.5; 780 / 1310 Hz (the rise complete to within 0.7 dB by 2.5 kHz) | dB, Hz | A-010 (template) |
| `N_OUT` per rate | 346 / 461 / 922 / 2 539 / 2 764 / 5 078 / 5 527 (the script's print, 2026-10-15) | host samples (57.6 ms) | research R-08 (the 32 Hz corner's response below −100 dB of its peak, spec 006 FR-008's rule) |

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
| volume position | `set_volume(f32) -> Result<(), Error>`, `volume() -> f32` | finite, 0 … 1 | `VOLUME_REFERENCE` | at the next processing call; the two routes' scalars recomputed from the loaded law (one divide per route per block) |

Both survive `reset` and `reconfigure`; both are copied into the one-pass's and the player's `OutputChain` by `process*`, `start_playback` and `play`.

## `OutputChain` state (research R-01, R-03)

| Field | Type | Reset |
|---|---|---|
| `route`, `k_spk: f64`, `k_ear: f64` | the route and the two scalars derived from the position per block (the position itself lives in `Pipeline`) | kept (controls are not state) |
| `n_out` | `u32`, the chain's settle at this rate (research R-08) | constant |
| `run_amp`, `run_speaker`, `run_earphone` | `bool` | from settings |
| `tap` | `Tap` (the point `process` returns) | from settings |
| `vol_hpf` | `FirstOrder` (32 Hz, both routes) | zero |
| `amp_hpf` | `FirstOrder` (159 Hz) | zero |
| `speaker_res` | `Biquad` | zero |
| `speaker_shelf` | `FirstOrder` | zero |
| `ear_in`, `ear_out` | `FirstOrder` (120, 280 Hz nominal) | zero |

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

`drain = ⌈N₁₀ · m / l⌉ + 2D + N_OUT(rate)` when any output stage runs, else spec 006's figure; `N_OUT` follows N₁₀'s rule (the slowest section's response below −100 dB of its peak), so spec 006 FR-008's "last 1 ms below −80 dBFS" check extends to the chain. The output is not exactly zero at the drain's end (floating point; the sections flush to exactly 0.0 only below 1e-30, about 350 ms after the input stops), and nothing claims it is. `drain_samples()` reports the figure. The playback's total length is `⌈K · m / l⌉ + drain`.

## Per-rate figures (script-printed; pinned by core tests)

| Rate | `N_OUT` | 1 kHz group delay, speaker route (ms) | earphone route (ms) | latency change |
|---|---|---|---|---|
| 6 000 … 96 000 | table above | 0.31 – 0.33 (the template plus the corners; synced to the script 2026-10-15) | 0.065 – 0.072 | 0 at every rate |

## Reports

Unchanged: `BlockInfo`, `RecordInfo`, `StopOutcome`, `PlayInfo`, `EngineState` keep their fields (the volume has its own getter; the product notes' meters are E-A, out of scope).

## Harness configurations (research R-10)

| Name | Stages on | Tap | Route (set by the check) |
|---|---|---|---|
| `output_only_speaker` | 11, 12 | `RouteOutput` | speaker |
| `output_only_earphone` | 11, 13 | `RouteOutput` | earphone |
| `output_only_amp` | 11 (the speaker template off: the clip and corner checks) | `RouteOutput` | speaker |
| `default_output` | all | `RouteOutput` | speaker |
| `default_earphone` | all | `RouteOutput` | earphone |
| `default_loud` | all | `RouteOutput` | speaker, at `output::VOLUME_6DB_OVER` (the clipper engaged by the volume; the golden) |

Every pre-009 configuration sets the three bypasses off (`output: OutputSettings::BYPASSED`).

## Assumptions touched

A-031 (adopted, corrected, extended), A-032 (adopted, corrected), A-010 (revised: the template's chosen figures), A-042 (output scale), A-043 (volume default), A-044 (volume position as the full-swing fraction); S-006 corrected on the branch.
