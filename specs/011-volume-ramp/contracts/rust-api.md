# Contract: Rust API (feature 011; research R-01, R-07)

All items are in `rr_dr60`; every public item has rustdoc citing its FR and A-045. Wording: "the volume transition", "a volume change takes 20 ms"; "ramp" only in identifiers and the feature name; never "accurate", never a claim that the transition protects hearing or equipment.

## Public additions

| Item | Contract |
|---|---|
| `output::VOLUME_TRANSITION_MS: u32 = 20` | The length of a volume transition in milliseconds (A-045; FR-002). The same at every supported host rate. |
| `output::volume_transition_samples(host_rate_hz: u32) -> Option<u32>` | N = 0.02 × host rate for a supported rate (120, 160, 320, 882, 960, 1764, 1920); `None` for an unsupported rate. `const fn`. |

No settings field, no new error, no new `Tap`, no change to `Settings`, `OutputSettings` or their presets (FR-011).

## Pipeline behaviour (documentation of existing items)

| Item | Contract |
|---|---|
| `Pipeline::set_volume(position)` | Unchanged validation and refusals. A valid change takes effect at the next processing call or `play` block as a **20 ms transition**: the knob moves from where it is to `position`, starting and ending at rest, and each route's gain follows the loaded law (A-044) at the knob's position, never passing either end; the new position's gain is reached exactly at the transition's last sample. A change during a transition starts a new one from the knob's current position. The first block that processes at least one sample after `new`, `reset`, `reconfigure` or `start_playback`, and the first block after a route change, take the position at once. With stage 11 bypassed the position has no effect. Real-time safe. |
| `Pipeline::volume()` | The host's last valid setting, not the knob's intermediate position (unchanged). |
| `Pipeline::set_route` | Unchanged; documented: a route change completes a transition in progress at once. |
| `Tap::AfterVolumeAmp` | Documented: the gain at this tap moves over the transition. |

## Test hooks (`__test-hooks` feature)

| Item | Contract |
|---|---|
| `__test_hooks::OutputMutation::volume_step: bool` | Default `false`. `true` restores 0.10's instantaneous step: every change is taken at once at the next block (US1 AS7's reference; R-08's comparison). |
| `__test_hooks::OutputMutation::volume_shape_linear: bool` | Default `false`. `true` moves the knob linearly in time (the mutant that fails FR-004 on the speaker law). |
| `__test_hooks::OutputMutation::volume_ramp_delta: i64` | Default `0`. Added to N (the length mutants). |

Applied through the existing `Pipeline::__with_output_mutation`; `Default` is the committed chain.
