# Data Model: Optional Output Clamp (feature 010)

The option touches no stored data: the memory, the stored form (version 2) and the export are unchanged (spec FR-006; checked by `stored_form_unchanged`). What follows is the engine's shape after the feature.

## Settings (contracts/rust-api.md; research R-02)

| Item | Type | Default | Meaning |
|---|---|---|---|
| `OutputSettings::clamp_enabled` | `bool` | `false` | The output clamp: the route-output sample held within [−1.0, +1.0] by replacement (FR-001). Not a device stage; an emulator convenience (A-042 amended). |
| `OutputSettings::DEVICE` | | `clamp_enabled: false` | The device: every stage on, no clamp. |
| `OutputSettings::BYPASSED` | | `clamp_enabled: false` | Version 0.8's output; every pre-009 configuration builds on it, so the five golden files hold (FR-002). |

No validation rule: a bool cannot be invalid. No control, no error, no status, no tap.

## `OutputChain` (research R-01)

Two new fields, set at construction, mutated only by the test hook:

| Field | Value | Role |
|---|---|---|
| `clamp: bool` | `settings.output.clamp_enabled && tap == Tap::RouteOutput` | The player passes `honour_tap = false`, so its `tap` is `RouteOutput` and `clamp` equals the setting; the one-pass passes the configured tap, so `clamp` is true only when that tap is `RouteOutput`. |
| `bound: f64` | `1.0` (A-042: the route's full scale) | A field only for `__mutate` (`clamp_bound`); production never changes it. |
| `positive_only: bool` | `false` | Test hook only (`clamp_positive_only`), a mutant for the identity check. |

`process(y)`: `let r = inner(y); if self.clamp { bound(r, self.bound) } else { r }`, where `inner` is today's body including the `active` early return and the `AfterVolumeAmp` returns (never bounded, because `clamp` is false for that tap), and

```rust
fn bound(y: f64, b: f64) -> f64 { if y > b { b } else if y < -b { -b } else if y == y { y } else { 0.0 } }
```

(NaN → 0.0; −0.0 passes unchanged; no `f64::clamp`, whose `min <= max` assertion would be a panic site once the bound is a field). `reset`, `at_rest`, `runs_any`, `n_out`, `set_controls`, `scalars` are unchanged: the bound has no state, does not grow the drain and ignores the controls.

Invariant (the identity, FR-005, proven in R-01 and fuzzed): for every host-rate output sample, `narrow_out(process_on(y)) == narrow_out(process_off(y)).clamp(-1.0, 1.0)`.

## Taps (FR-003)

Unchanged in number and value. Semantics with the option on:

| Tap | Bounded? |
|---|---|
| `RouteOutput` | **yes** (on both routes; also with every output stage bypassed) |
| `AfterVolumeAmp`, `AfterSpeaker`, `AfterEarphone` | no (the one-pass at these taps) |
| `AfterPlayback` and every earlier tap | no (the option is inert in the one-pass at `AfterPlayback`) |
| **The play calls** | **always** (the player returns the route output whatever `settings.tap` names) |

Documented consequence: with the option on, `RouteOutput` and the selected route's last per-stage tap (`AfterSpeaker` or `AfterEarphone`; `AfterPlayback` when the chain is bypassed) are no longer the same signal wherever a sample was bounded.

## C layout (contracts/c-api.md; research R-03)

`RrDr60Settings` (alignment 8, from `seed: u64`): 104 → **112** bytes. Offsets 0 – 98 as 0.9; `reserved_010: uint8_t[5]` at 99 – 103; `output_clamp_enabled: bool` at **104**; `reserved_010_tail: uint8_t[7]` at 105 – 111. `struct_size = 104` refused (`INVALID_ARGUMENT`, field `STRUCT_SIZE`). `RR_DR60_VERSION_MINOR = 10`.

## Harness configurations (research R-05, R-06)

| Name | Built from | Clamp | Route / position in the golden file | Option-off twin | Purpose |
|---|---|---|---|---|---|
| `bypass_route_output` | `bypass_all` with `tap = RouteOutput` | off | | | the twin of `output_clamp_only`; the "earlier taps" loop |
| `output_clamp_only` | `bypass_route_output` | on | speaker / 1.0 (the position is without effect) | `bypass_route_output` | the primary stimulus; the "bypassed chain still bounded" case |
| `output_only_speaker_clamp` | `output_only_speaker` | on | speaker / 1.0 | `output_only_speaker` | the speaker template's ringing bounded |
| `output_only_earphone_clamp` | `output_only_earphone` | on | earphone / 1.0 | `output_only_earphone` | the coupling corners' overshoot bounded |
| `default_output_clamp` | `default_output` (the 0.9 default) | on | speaker / 1.0 | `default_output` at 1.0 | the whole engine, the clamp on |
| `default_earphone_clamp` | `default_earphone` | on | earphone / 1.0 | `default_earphone` | the whole engine, earphone |
| `default_output_ref_clamp` | `default_output` | on | speaker / `VOLUME_REFERENCE` | `default_output` at the reference | the non-overshooting case (FR-010; US1 AS4); the user's default |

`configs::OUTPUT_CLAMP_CONFIGS` lists the six clamp configurations; `configs::output_clamp_twin(name)` returns the twin's name; `configs::output_clamp_route_volume` the route and position; `configs::c_settings` maps `clamp_enabled`. Every pre-010 configuration keeps `clamp_enabled = false` through `OutputSettings::BYPASSED` / `DEVICE`.

## Stimuli (research R-05)

| Name | Definition | Where |
|---|---|---|
| `sine_1k_1p5` | `stimulus::tone(1000.0, 1.5, fs, 2 s)`; sample peak 1.299 at 6 kHz (six samples per period), about 1.5 elsewhere | the primary stimulus (US1 AS1, the golden, the earlier-taps loop) |
| `square_1k_fs` | `stimulus::square(1000.0, 1.0, fs, 2 s)`: a half-period of round(fs ÷ 2000) samples, starting at +1.0 | the secondary stimuli, both routes (US1 AS2 – AS3, the golden, the smoke loop, the change-timing identity, the mutants) |
| `tone_1k_0` | the existing 1 kHz tone at 0 dBFS, 2 s | every golden configuration; the reference-position case (US1 AS4) |
| `voiced` −20 dBFS, `noise` | existing | the equivalence (voiced), the stored form (voiced), the quiet-material rows (voiced −1 dBFS, band-limited noise −3 dBFS), the cost (full-scale noise) |

`output_clamp_matrix` uses 1 s stimuli so its counts reproduce the tables below; the golden uses 2 s.

## Expected report (research R-04; `output_clamp_matrix` prints these, asserting "exceeds" / "does not exceed", the L1 bound and the identity)

Option-off peak at the route output (count beyond ±1.0 in parentheses), 1 s stimuli:

| Rate | `output_clamp_only`, `sine_1k_1p5` | chain-only speaker square @ 1 | chain-only earphone square @ 1 | chain-only speaker tone @ 1 | chain-only speaker tone @ reference | default speaker square @ 1 | default earphone square @ 1 | Speaker-route bound (L1) |
|---|---|---|---|---|---|---|---|---|
| 6 000 | 1.299 (4000) | 1.544 (1999) | 1.522 (1999) | 1.493 (1999) | 0.882 (0) | 1.495 (1918) | 1.038 (12) | 1.75 |
| 8 000 | 1.545 (5939) | 1.676 (1979) | 1.391 (1980) | 1.636 (1979) | 0.904 (0) | 1.672 (1917) | 1.035 (11) | 1.88 |
| 16 000 | 1.530 (9899) | 1.789 (3963) | 1.379 (7910) | 1.839 (3959) | 0.964 (0) | 1.803 (3889) | 1.053 (1205) | 2.09 |
| 44 100 | 1.527 (23365) | 1.892 (11906) | 1.370 (15876) | 1.868 (13162) | 0.969 (0) | 1.870 (12728) | 1.041 (843) | 2.24 |
| 48 000 | 1.527 (25739) | 1.892 (15832) | 1.371 (17816) | 1.939 (13858) | 0.969 (0) | 1.852 (14003) | 1.059 (2628) | 2.25 |
| 88 200 | 1.527 (46728) | 1.929 (25791) | 1.371 (33731) | 1.908 (26528) | 0.970 (0) | 1.888 (25548) | 1.047 (1682) | 2.28 |
| 96 000 | 1.528 (49499) | 1.943 (27716) | 1.370 (37602) | 1.948 (29689) | 0.970 (0) | 1.897 (27788) | 1.069 (7057) | 2.29 |

Further rows the matrix prints (the hardware reviewer's figures, to be reproduced): the default speaker route on a full-scale tone at position 1 (2.02 – 2.06 at 44.1 kHz and above); `tone_1k_0` on every golden configuration (every stage bypassed 0.866 (0) at 6 kHz and 1.018 – 1.030 elsewhere; the earphone configurations 0.97 – 1.04); the default at the reference position with `voiced` −1 dBFS (0.43 – 0.47, 0 over), `voiced` 11 dB into the limiter (0.45 – 0.55, 0), band-limited noise −3 dBFS (0.77 – 0.86, 0); a "% of samples bounded" column; the L1 bound recomputed from the coefficients within ±0.02 of the table, every speaker-route peak ≤ it. The `default_output_ref_clamp` twin's counts on the three golden stimuli are measured in Phase 1 (expected 0) and asserted 0 at bless time.

## Golden file (contracts/golden-format.md)

`golden/golden-output-clamp-v1.json`: 3 stimuli × 6 configurations × 7 rates = **126** entries with `route`, `volume`, the one-pass output SHA and `n_at_bound` (the count of output samples exactly ±1.0, recomputed every run). The identity against the twin, the reference configuration's zero counts and (for the three `default_*` configurations) record-then-play equality are asserted at bless time only. Guard `GOLDEN_OUTPUT_CLAMP_V1_SHA256` in `tests/golden_output_clamp.rs`.

## Registers and documents touched

A-042 (amended with the figures by configuration, the L1 bound, the mechanism, the option, the distortion sentence; status line marked), README output section and the record-path headroom paragraph, CHANGELOG 0.10.0, `golden/manifest-v1.txt` (header version only), the `output_edge_cases.rs` doc comment, CLAUDE.md, Cargo.toml (× 4), `RR_DR60_VERSION_MINOR`, the contracts, `ci.yml`, `scripts/ios-device-golden.sh`. No source, no new assumption, no signal-chain row.
