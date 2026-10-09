# Data Model: Minimal End-to-End Pipeline Skeleton

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md)

This page defines the entities from the spec's Key Entities, plus the internal state that determines behavior. The concrete Rust and C signatures are in [contracts/](contracts/).

## Settings

The caller's configuration. It is fixed for the life of a configuration (FR-008).

| Field | Type | Default | Validation | Trace |
|---|---|---|---|---|
| `host_rate_hz` | u32 | none (required) | Must be one of `SUPPORTED_HOST_RATES = [8000, 16000, 44100, 48000, 88200, 96000]`, otherwise `Error::UnsupportedHostRate` | FR-002 |
| `record_stage_enabled` | bool | `true` | — | FR-007, A-002 |
| `playback_stage_enabled` | bool | `true` | — | FR-007, A-002 |
| `tap` | `Tap` | `AfterPlayback` | C API only: an unknown discriminant gives `INVALID_ARGUMENT` | FR-007 |
| `seed` | u64 | `0` | Any value. It has no effect on output in this slice | FR-009 |

- Rust: `#[non_exhaustive]`, so later specs can add stage settings without a breaking change.
- Construction: `Settings::new(host_rate_hz)`. Validation happens in `Pipeline::new` / `reconfigure`, not in `Settings::new`, so invalid settings can be built and then rejected with a typed error.

### Tap

| Variant | Meaning | Effect |
|---|---|---|
| `AfterRecord` | The output is taken after stage 4 | Stage 10 is not run, and `playback_stage_enabled` is ignored (US2 AS2) |
| `AfterPlayback` (default) | The output is taken after stage 10 | Both stages run according to their flags |

### Named configurations (harness)

| Name | record | playback | tap |
|---|---|---|---|
| `default` | on | on | AfterPlayback |
| `record_only` | on | bypassed | AfterPlayback |
| `playback_only` | bypassed | on | AfterPlayback |
| `tap_after_record` | on | on (ignored) | AfterRecord |
| `bypass_all` | bypassed | bypassed | AfterPlayback |

## Pipeline

One emulator instance.

| Part | Contents | Allocated | Mutated by |
|---|---|---|---|
| `settings` | a copy of `Settings` | construction / reconfigure | reconfigure |
| `rate` | `RatePlan { host_hz, l, m, down_taps, up_taps, d_down, d_up }` | construction / reconfigure | — |
| `down` | `PolyphaseDown { phases: Box<[f64]>, history: Ring<f64>, phase_acc: u32 }` (absent at 8 kHz) | construction / reconfigure | process, reset |
| `record` | `VoiceBandStage { sections: [Biquad; 6] }` | inline | process, reset |
| `playback` | `VoiceBandStage` | inline | process, reset |
| `up` | `PolyphaseUp { phases: Box<[f64]>, history: Ring<f64>, phase_acc: u32 }` (absent at 8 kHz) | construction / reconfigure | process, reset |
| `latency` | `u32`, computed per R-10 | construction / reconfigure | — |
| `poisoned` (FFI wrapper only) | bool | — | set when a panic is caught, cleared by reset or reconfigure |

### Biquad (TDF-II, f64)

| Field | Meaning |
|---|---|
| `b0, b1, b2, a1, a2` | Coefficients (a0 normalized to 1). `const`, from `voiceband_coeffs.rs` |
| `s1, s2` | State. After each update, any value with magnitude below 1e-30 is flushed to 0.0 (R-05) |

### State transitions

```text
              new(settings) ok                      process(block)  (any N ≥ 0, RT-safe)
  (none) ───────────────────────▶ READY ◀──────────────────────────────┐
     │      new(settings) err          │                                │
     └──▶ Error (no pipeline)          ├── process ─────────────────────┘
                                       ├── reset()          → READY (state = freshly created; no alloc)
                                       ├── reconfigure(s) ok → READY (new settings, new latency, fresh state; may alloc)
                                       ├── reconfigure(s) err → READY (unchanged: old settings and state kept)
                                       └── drop / destroy   → (none)

  FFI only: a panic caught in any call → POISONED. process/latency return INTERNAL_ERROR;
            reset / reconfigure (success) → READY; destroy is always allowed.
```

- A failed `reconfigure` leaves the pipeline unchanged. This is the strong exception guarantee: new buffers are built first, then swapped in.
- `reset` restores exactly the freshly created state: all histories and biquad state are zero and the phase accumulators are at their initial values. US3 checks that output after `reset` is bit-identical to a new pipeline's output.

## Per-sample engine (invariants)

For each host input sample `x[n]` (R-09):

1. `x ← sanitize_in(x)`: a non-finite or f32-subnormal sample becomes 0.0. Then widen to f64.
2. Push `x` into the down-converter. If a device sample `d` is due (phase accumulator), compute it:
   - `d ← record.process(d)` if the record stage is enabled.
   - If the tap is `AfterPlayback` and the playback stage is enabled: `d ← playback.process(d)`.
   - Push `d` into the up-converter history.
3. `y[n] ← up.next()`, narrowed to f32. Any |y| below `f32::MIN_POSITIVE` becomes 0.0.

Invariants:
- **I1**: exactly one output per input. Output length always equals input length (FR-003).
- **I2**: the operation sequence for sample `n` depends only on (settings, n, the input history), never on block boundaries (FR-014).
- **I3**: at 8 kHz there are no converters, so step 2 runs for every sample and `y = d` (FR-002).
- **I4**: no allocation, locking or I/O in steps 1–3 (FR-015).

## Measurement result (harness)

| Field | Type | Example |
|---|---|---|
| `requirement` | `&'static str` | `"FR-010"` |
| `property` | `&'static str` | `"upper -3 dB point"` |
| `trace` | `&'static [&'static str]` | `["A-002", "A-014"]` or `["engineering target"]` |
| `host_rate_hz` | u32 | `48000` |
| `config` | named configuration | `record_only` |
| `stimulus` | descriptor | `tone(3400 Hz, -20 dBFS)`, `sweep(20 Hz–21.6 kHz)` |
| `measured` | f64 plus unit | `3406.2 Hz` |
| `tolerance` | `Range` / `AtLeast` / `AtMost` / `Exact` | `3350..=3450 Hz` |
| `passed` | bool | `true` |

The report prints one row per result (FR-020).

## Golden reference

| Field | Type | Notes |
|---|---|---|
| `stimulus` | string | `impulse`, `tone_1k_m20`, `sweep_log`, `noise_seed_0d60` |
| `config` | string | `default`, `record_only`, `playback_only`, `bypass_all` |
| `host_rate_hz` | u32 | one of the 6 supported rates |
| `n_samples` | u32 | stimulus length |
| `sha256` | hex string | SHA-256 over the output's LE `f32` bit patterns |
| `head` | [hex u32; 16] | bit patterns of the first 16 output samples (diagnostic) |
| `rms_dbfs` | f64 | diagnostic, not compared for pass/fail |

Uniqueness: the key (stimulus, config, host_rate_hz) is unique. The full schema is in [contracts/golden-format.md](contracts/golden-format.md).

## Language-neutral interface entities

| Entity | C shape | Notes |
|---|---|---|
| Pipeline handle | `RrDr60Pipeline*` (opaque) | Created by `rr_dr60_create`, freed by `rr_dr60_destroy`. Not thread-safe; it may move between threads but must not be used by two at once |
| Settings | `struct RrDr60Settings` (`repr(C)`) | Mirrors `Settings`, plus `struct_size` for forward-compatible growth |
| Status | `RrDr60Status` (`int32_t` enum) | `OK`, `NULL_POINTER`, `UNSUPPORTED_HOST_RATE`, `INVALID_ARGUMENT`, `INTERNAL_ERROR` |

Details are in [contracts/c-api.md](contracts/c-api.md).
