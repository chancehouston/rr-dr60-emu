# Data Model: Optional Output Clamp (feature 010)

The option touches no stored data: the memory, the stored form (version 2) and the export are unchanged (spec FR-006). What follows is the engine's shape after the feature.

## Settings (contracts/rust-api.md; research R-02)

| Item | Type | Default | Meaning |
|---|---|---|---|
| `OutputSettings::clamp_enabled` | `bool` | `false` | The output clamp: the route-output sample held within [−1.0, +1.0] by replacement (FR-001). Not a device stage; an emulator convenience (A-042 amended). |
| `OutputSettings::DEVICE` | | `clamp_enabled: false` | The device: every stage on, no clamp. |
| `OutputSettings::BYPASSED` | | `clamp_enabled: false` | Version 0.8's output; every pre-009 configuration builds on it, so the five golden files hold (FR-002). |

No validation rule: a bool cannot be invalid. No control, no error, no status, no tap.

## `OutputChain` (research R-01)

One new field, set at construction, never mutated:

| Field | Value | Role |
|---|---|---|
| `clamp: bool` | `settings.output.clamp_enabled && tap == Tap::RouteOutput` | The player passes `honour_tap = false`, so its `tap` is `RouteOutput` and `clamp` equals the setting; the one-pass passes the configured tap, so `clamp` is true only when that tap is `RouteOutput`. |

`process(y)`: `let r = inner(y); if self.clamp { r.clamp(-1.0, 1.0) } else { r }`, where `inner` is today's body including the `active` early return and the `AfterVolumeAmp` returns (never clamped, because `clamp` is false for that tap). `reset`, `at_rest`, `runs_any`, `n_out`, `set_controls`, `scalars` are unchanged: the clamp has no state, does not grow the drain and ignores the controls.

Invariant (the identity, FR-005, proven in R-01): for every host-rate output sample, `narrow_out(process_on(y)) == narrow_out(process_off(y)).clamp(-1.0, 1.0)`.

## Taps (FR-003)

Unchanged in number and value. Semantics with the option on:

| Tap | Clamped? |
|---|---|
| `RouteOutput` | **yes** (on both routes; also with every output stage bypassed) |
| `AfterVolumeAmp`, `AfterSpeaker`, `AfterEarphone` | no |
| `AfterPlayback` and every earlier tap | no |

Documented consequence: with the option on, `RouteOutput` and the selected route's last per-stage tap (`AfterSpeaker` or `AfterEarphone`; `AfterPlayback` when the chain is bypassed) are no longer the same signal wherever a sample was bounded.

## C layout (contracts/c-api.md; research R-03)

`RrDr60Settings`: 104 → **108** bytes. Offsets 0 – 98 as 0.9; `reserved_010: uint8_t[5]` at 99 – 103; `output_clamp_enabled: bool` at **104**; `reserved_010_tail: uint8_t[3]` at 105 – 107. `struct_size = 104` refused (`STRUCT_SIZE`).

## Harness configurations (research R-05, R-06)

| Name | Built from | Clamp | Route / position in the golden file | Purpose |
|---|---|---|---|---|
| `clamp_only` | every stage bypassed (`no_agc`, `no_vas`, `no_stages`, ADC clamp off, codec off, `output = BYPASSED`), `tap = RouteOutput` | on | speaker / 1.0 (the position is without effect) | the primary stimulus; the "bypassed chain still bounded" case |
| `output_only_speaker_clamp` | `output_only_speaker` | on | speaker / 1.0 | the speaker template's ringing bounded |
| `output_only_earphone_clamp` | `output_only_earphone` | on | earphone / 1.0 | the coupling corners' overshoot bounded |
| `default_output_clamp` | `default_output` (the 0.9 default) | on | speaker / 1.0 | the whole engine, the clamp on |
| `default_earphone_clamp` | `default_earphone` | on | earphone / 1.0 | the whole engine, earphone |

`configs::CLAMP_CONFIGS` lists the five; `configs::clamp_twin(name)` returns the option-off twin's name (`clamp_only` → an all-bypassed configuration at `RouteOutput`, added as `bypass_route_output` if no existing name matches; `x_clamp` → `x`). Every pre-010 configuration keeps `clamp_enabled = false` through `OutputSettings::BYPASSED` / `DEVICE`.

## Stimuli (research R-05)

| Name | Definition | Where |
|---|---|---|
| `sine_1k_1p5` | `stimulus::tone(1000.0, 1.5, fs, 2 s)`; sample peak 1.299 at 6 kHz (six samples per period), about 1.5 elsewhere | the primary stimulus (US1 AS1, the golden) |
| `square_1k_fs` | `stimulus::square(1000.0, 1.0, fs, 2 s)`: a half-period of round(fs ÷ 2000) samples, starting at +1.0 | the secondary stimuli, both routes (US1 AS2 – AS3, the golden, the smoke loop) |
| `tone_1k_0` | the existing 1 kHz tone at 0 dBFS, 2 s | the speaker route at position 1 (overshoots) and at the reference position (does not; US1 AS4) |

## Expected report (research R-04; `clamp_matrix` prints these, asserting only "exceeds" / "does not exceed")

Option-off peak at the route output (count beyond ±1.0 in parentheses), 1 s stimuli:

| Rate | `clamp_only`, `sine_1k_1p5` | speaker square @ 1 | earphone square @ 1 | speaker tone @ 1 | speaker tone @ reference |
|---|---|---|---|---|---|
| 6 000 | 1.299 (4000) | 1.544 (1999) | 1.522 (1999) | 1.493 (1999) | 0.882 (0) |
| 8 000 | 1.545 (5939) | 1.676 (1979) | 1.391 (1980) | 1.636 (1979) | 0.904 (0) |
| 16 000 | 1.530 (9899) | 1.789 (3963) | 1.379 (7910) | 1.839 (3959) | 0.964 (0) |
| 44 100 | 1.527 (23365) | 1.892 (11906) | 1.370 (15876) | 1.868 (13162) | 0.969 (0) |
| 48 000 | 1.527 (25739) | 1.892 (15832) | 1.371 (17816) | 1.939 (13858) | 0.969 (0) |
| 88 200 | 1.527 (46728) | 1.929 (25791) | 1.371 (33731) | 1.908 (26528) | 0.970 (0) |
| 96 000 | 1.528 (49499) | 1.943 (27716) | 1.370 (37602) | 1.948 (29689) | 0.970 (0) |

## Golden file (contracts/golden-format.md)

`golden/golden-clamp-v1.json`: 3 stimuli × 5 configurations × 7 rates = 105 entries with `route` and `volume`, the one-pass output SHA; the identity against the twin and (for the two `default_*_clamp` configurations) record-then-play equality are asserted by the generator, not stored. Guard `GOLDEN_CLAMP_V1_SHA256` in `tests/golden_clamp.rs`.

## Registers and documents touched

A-042 (amended; status line marked), README output section and the record-path headroom note, CHANGELOG 0.10.0, `golden/manifest-v1.txt` (header version only), CLAUDE.md, the contracts. No source, no new assumption, no signal-chain row.
