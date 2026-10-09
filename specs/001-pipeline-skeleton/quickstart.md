# Quickstart: Minimal End-to-End Pipeline Skeleton

This guide shows how to check that feature 001 works end to end, and how an app developer uses it (SC-001). API details are in [contracts/rust-api.md](contracts/rust-api.md) and [contracts/c-api.md](contracts/c-api.md). The reasons behind the design are in [research.md](research.md).

> The emulator models the RR-DR60's voice-band filtering. That filtering is **assumed** to follow the MSM7702 codec's 300–3400 Hz telephone band (A-002, A-014). It has not yet been measured against a real unit.

## Prerequisites

- Rust stable via rustup, with `~/.cargo/bin` on `PATH`. The MSRV is 1.85.
- iOS targets, needed for the iOS checks only: `rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios`.
- Tools:
  - `cargo install cargo-llvm-cov --locked`, for coverage.
  - `cargo install cbindgen --locked --version <pinned in ci.yml>`, for regenerating the header.
- `uv`, needed only to regenerate the filter coefficients.
- A C compiler (clang or MSVC), for the C smoke test.

## 1. Build and run the full check suite

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo llvm-cov --all-features --workspace --fail-under-lines 80
```

**Expected**: everything passes. `cargo test` includes the harness matrix (6 rates × 5 configurations), golden hashes, block-partition invariance, allocation-free processing, and the C API parity tests.

## 2. See the measurement report (US3)

```sh
cargo test -p rr_dr60_harness --test response_matrix -- --nocapture
```

**Expected**: a table with one row per check, showing the requirement, the trace IDs, the rate, the configuration, the measured value, the tolerance, and PASS. For example:

```text
FR-010 upper -3 dB point   [A-002,A-014]  48000 record_only    3406 Hz   3350..=3450 Hz   PASS
FR-012 latency vs measured [eng. target]  48000 default        540 / 539.6  ±1 sample     PASS
```

## 3. Per-story checkpoints (Principle VII)

| Story | Command | Proves |
|---|---|---|
| US1 (P1) | `cargo test -p rr_dr60_harness --test us1_voiceband` | Default chain: 1 kHz level, rejection at 50 Hz and 3.8 kHz, N-in/N-out, silence in gives silence out, C API parity (AS1–AS5) |
| US2 (P2) | `cargo test -p rr_dr60_harness --test us2_bypass_tap` | Bypass, tap and latency per configuration (AS1–AS4) |
| US3 (P3) | `cargo test -p rr_dr60_harness` | Full matrix, golden files, determinism, allocation-free processing |

## 4. Use it from Rust (app developer, SC-001)

```rust
use rr_dr60::{Pipeline, Settings};

let mut p = Pipeline::new(Settings::new(48_000))?;   // defaults: both stages on, tap after playback
println!("latency: {} samples", p.latency_samples());
// In the audio callback (real-time safe):
p.process_in_place(&mut buffer);                     // any block length
```

## 5. Use it from C / Swift

```sh
cargo build -p rr_dr60_ffi --release                 # target/release/librr_dr60_ffi.{a,dylib,so} / .lib/.dll
cc crates/rr_dr60_ffi/tests/c/smoke.c -Icrates/rr_dr60_ffi/include \
   target/release/librr_dr60_ffi.a -o target/smoke && ./target/smoke
```

**Expected**: `smoke: OK` and exit code 0. The smoke test creates a 48 kHz pipeline, processes a 1 kHz tone, checks the level and latency, exercises the error paths from the contract table, and destroys the pipeline.

Minimal C usage:

```c
RrDr60Settings s = rr_dr60_settings_default(48000);
RrDr60Pipeline *p = NULL;
if (rr_dr60_create(&s, &p) != RR_DR60_STATUS_OK) { /* handle */ }
rr_dr60_process(p, in, out, frames);    /* in the audio callback */
rr_dr60_destroy(p);
```

For Swift, add `rr_dr60.h` to a module map or bridging header and link the static library. XCFramework packaging belongs to the separate iOS repository.

## 6. iOS checks

```sh
cargo build -p rr_dr60_ffi --target aarch64-apple-ios --release        # device library builds
cargo test  -p rr_dr60_harness --target aarch64-apple-ios-sim --test golden   # runs in a booted simulator via the runner
scripts/ios-device-golden.sh                                           # release gate: a connected device (cargo-dinghy)
```

**Expected**: the golden tests pass on the simulator and the device with the same `golden-v1.json` (FR-014).

## 7. Maintenance procedures

- **Regenerate the filter coefficients** (only when the design or an assumption changes):

  ```sh
  uv run tools/filter-design/design_voiceband.py
  ```

  This rewrites `crates/rr_dr60/src/stages/voiceband_coeffs.rs`. Then re-bless the golden files and add a CHANGELOG entry.
- **Re-bless the golden files** (only for an intended output change):

  ```sh
  RR_DR60_BLESS=1 cargo test -p rr_dr60_harness --test golden
  ```

  A CHANGELOG entry is required.
- **Regenerate the C header** after changing the FFI:

  ```sh
  cbindgen --config crates/rr_dr60_ffi/cbindgen.toml --crate rr_dr60_ffi --output crates/rr_dr60_ffi/include/rr_dr60.h
  ```

  CI fails if the committed header is stale.
- **Run the timing checks** (release mode):

  ```sh
  cargo test -p rr_dr60_harness --release --test timing -- --ignored
  ```

  **Expected**: at least 20× real time at 48 kHz, and per-sample time flat across block sizes (max/min ≤ 3).
