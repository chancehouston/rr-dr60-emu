# API contract: Live Playback-Stage Bypass (spec 021, version 0.21.0)

## Rust (`rr_dr60`)

No new public function. The existing scheduling call carries the four switches:

```rust
pipeline.schedule_change(Stream::Playback, position, control, value, tag)?; // control ∈ {5, 22, 23, 24}; value exactly 0.0 or 1.0
pipeline.schedule_change(Stream::OnePass,  position, control, value, tag)?; // control ∈ {22, 23, 24}; 5 refused (ControlNotOnStream)
```

- `value` 1.0 = the stage on, 0.0 = bypassed (the setting's own sense). Any other value, non-finite included → `Error::InvalidControlValue { control, value }`.
- A stage that does not run on the stream (its construction setting off, or the one-pass's configured tap not reaching it) → `Error::StageNotConfigured { control }` (new), for either value.
- `Stream::Record` → `Error::ControlNotOnStream`.
- Landing: the top of host sample `position` (stage 10: the device sample `changes::speed_landing(position, rate)`); logged as a `ChangeEntry` (before/after 0 or 1, the stream word); `settings()` reflects it; a stage-11 – 13 landing becomes pending for the other stream at its next sample (spec 017 D4), the player adopting at `start_playback` without an entry.
- `MAX_CHANGES_PER_CALL`: 139 → **142**.
- `output::SPEAKER_SCALE_DB: f32` (new public constant): the speaker route's scalar beyond the published gain law; with `speaker_gain_db` / `earphone_gain_db` it gives stage 11's bypass level change per route and position.
- `controls()`: entries 5, 22, 23, 24 read `ControlTiming::Live`.
- Test hooks (`__test-hooks`): `BypassMutation { weight_inverted, fade_step, fade_linear, stage_cold, count_ungated, phase_frozen_off_route }`; `stage10_trace()` (the half's output per device sample of the last play call, up to 8192).

## C (`rr_dr60_ffi`, `include/rr_dr60.h`)

- `RR_DR60_STATUS_STAGE_NOT_CONFIGURED = 36` (appended).
- `RR_DR60_MAX_CHANGES_PER_CALL` 139 → 142.
- `RR_DR60_SPEAKER_SCALE_DB` (a double define; cast to float).
- No new function; `rr_dr60_schedule_change` carries the four controls. The header is regenerated and drift-checked; the smoke gains `--bypass` diffed against `examples/bypass_rows.rs`.

## Behavioural contract (the app's)

1. An engine meant for live A/B is constructed with every playback stage on (`Settings::new`'s default).
2. Presets and restored sessions apply stage switches as scheduled changes at position 0 of the next playback, never as construction settings.
3. `settings()` of a live engine is a read-out; `reconfigure(settings())` while a stage is bypassed builds that stage configured off, after which its switch is refused.
4. Stage 11's bypass changes the level by −(speaker_gain_db(p) − SPEAKER_SCALE_DB) dB on the speaker route and −earphone_gain_db(p) dB on the earphone route; the neutral positions are the speaker route's reference position and the earphone route's position 1.
5. Stage 10's audible onset is about the resampler's delay after the position at the conversion rates; an app may schedule it earlier to hear it together with the chain's stages.
