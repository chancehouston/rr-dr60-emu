# Contract: Rust API additions (feature 010; research R-01, R-02)

All items are in `rr_dr60`; every public item has rustdoc citing its FR and A-042. Wording: "the output clamp", "an emulator convenience, not a device behavior", "bounds the route output at ±1.0"; "clip" keeps spec 009's meaning (the amplifier).

## Settings

| Item | Contract |
|---|---|
| `OutputSettings::clamp_enabled: bool` | New fourth field (`#[non_exhaustive]` struct, so external literals were never possible). Default `false`. When `true`, every sample at `Tap::RouteOutput` is replaced by +1.0 if above it or by −1.0 if below it; samples within the bound pass unchanged (FR-001). Applies on both routes and when every output stage is bypassed (FR-003). Not a device stage (FR-004). |
| `OutputSettings::DEVICE` | `clamp_enabled: false` (the device has no such bound). |
| `OutputSettings::BYPASSED` | `clamp_enabled: false` (version 0.8's output; the pre-009 configurations and golden files hold, FR-002). |
| `Settings::validate` | unchanged (a bool cannot be invalid). |

## Taps (unchanged set; documentation)

`Tap::RouteOutput`'s docs: "With `output.clamp_enabled`, the only tap that is bounded; the selected route's last per-stage tap (`AfterSpeaker` / `AfterEarphone`, or `AfterPlayback` with the chain bypassed) then differs from it wherever a sample was bounded." `Tap::AfterVolumeAmp`, `AfterSpeaker`, `AfterEarphone`, `AfterPlayback`: "never bounded by the output clamp."

## Pipeline behaviour

| Behaviour | Contract |
|---|---|
| One-pass and record-then-play | Bit-identical under spec 008 FR-009's rule with the option on (FR-006); the two paths share `OutputChain`, which carries the option. |
| Reconfigure | The option is read at `Pipeline::new` / `reconfigure` like the other settings; a change applies from the next processing call, the paths reset (spec 006). |
| Route and volume controls | Unchanged; the bound applies to whichever route's output is selected at each block (FR-006). |
| Latency, drain | Unchanged at every rate (the clamp is stateless: `n_out` and the latency tables untouched, FR-005). |
| Non-finite input to the clamp | Cannot occur (the output is finite, spec 002 R-07); the core function maps ±∞ to ±1.0 and NaN to 0.0 (`f64::clamp` semantics with NaN handled explicitly), pinned by a unit test. |

## Test hook

`__test_hooks::OutputMutation` gains a variant that moves the bound to ±0.5 (feature `__test-hooks` only), so `mutation` can show the identity check fails on a wrong bound.

## Manifest

Unchanged: no row, no count, no entry string (FR-004). `manifest().header.library_version` follows the crate version (0.10.0), which is the one canonical-text re-bless (FR-009).

## Documentation

`lib.rs` crate docs: one paragraph on the option under the output-chain paragraph; `settings.rs`: the field, the presets, the tap sentences; the README's output section (FR-009) with a Rust line `s.output.clamp_enabled = true;` and the ×32767 sentence.
