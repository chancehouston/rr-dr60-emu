# Contract: Rust public API (`rr_dr60` 0.1.0)

This is a SemVer surface. Breaking changes need a MAJOR bump after 1.0 (a MINOR bump while 0.x). Every public item has rustdoc that cites the relevant FR and A-IDs.

```rust
#![no_std]
extern crate alloc;

/// Host sample rates supported by this version (FR-002).
pub const SUPPORTED_HOST_RATES: [u32; 6] = [8000, 16000, 44100, 48000, 88200, 96000];

/// The emulated device's internal sample rate (A-001).
pub const DEVICE_RATE_HZ: u32 = 8000;

/// Where the pipeline output is taken (FR-007).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Tap {
    /// After signal-chain stage 4 (record band-limit). Stage 10 is not run.
    AfterRecord,
    /// After signal-chain stage 10 (playback band-limit). Default (A-002).
    #[default]
    AfterPlayback,
}

/// Pipeline configuration. Fixed for the life of a configuration (FR-008).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Settings {
    /// Host sample rate in Hz. Must be in `SUPPORTED_HOST_RATES`.
    pub host_rate_hz: u32,
    /// Stage 4 on (true, default, A-002) or bypassed (false).
    pub record_stage_enabled: bool,
    /// Stage 10 on (true, default, A-002) or bypassed (false). Ignored when tap = AfterRecord.
    pub playback_stage_enabled: bool,
    /// Output tap point. Default AfterPlayback.
    pub tap: Tap,
    /// Seed for stochastic stages (FR-009). No effect on output in this version.
    pub seed: u64,
}

impl Settings {
    /// Defaults for the given host rate. Not validated until `Pipeline::new`.
    pub const fn new(host_rate_hz: u32) -> Self;
}

/// Errors from configuration and processing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// `host_rate_hz` is not in `SUPPORTED_HOST_RATES`. Display lists the supported rates.
    UnsupportedHostRate { requested: u32 },
    /// `process`: the input and output slices differ in length.
    LengthMismatch { input: usize, output: usize },
}
impl core::fmt::Display for Error { /* … */ }
impl core::error::Error for Error {}

/// One emulator instance. `Send + Sync` (asserted at compile time); processing takes `&mut self`.
pub struct Pipeline { /* private */ }

impl Pipeline {
    /// Validates the settings and allocates every buffer. Not real-time safe.
    pub fn new(settings: Settings) -> Result<Self, Error>;

    /// Processes one block. `input.len()` must equal `output.len()` (any length, including 0).
    /// Real-time safe: no allocation, locks, I/O or clock (FR-015).
    pub fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<(), Error>;

    /// In-place variant. Infallible.
    pub fn process_in_place(&mut self, buffer: &mut [f32]);

    /// Fixed latency in host-rate samples for the current configuration (FR-012).
    pub fn latency_samples(&self) -> u32;

    /// Returns to the freshly created state. No allocation (FR-017).
    pub fn reset(&mut self);

    /// Applies new settings and resets state. May allocate; not real-time safe (FR-008).
    /// On error the pipeline is unchanged.
    pub fn reconfigure(&mut self, settings: Settings) -> Result<(), Error>;

    /// The current settings.
    pub fn settings(&self) -> &Settings;
}

/// Crate version, e.g. "0.1.0".
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
```

## Behavioral guarantees (tested)

| Guarantee | Requirement | Test |
|---|---|---|
| `process` writes exactly `input.len()` samples; a length mismatch returns an error before any state changes | FR-003 | `us1_voiceband`, `edge_cases` |
| An empty block is a no-op (the state is unchanged) | Edge Cases | `edge_cases` |
| Output for a partitioned input is bit-identical to one-block output | FR-014 | `determinism` |
| `process_in_place(buf)` gives the same result as `process(copy, buf)` | FR-014 | `determinism` |
| `reset()`, then input X, gives the same output as `new(same settings)`, then X | FR-017 | `edge_cases` |
| `reconfigure(s)` gives the same state as `new(s)`; on error nothing changes | FR-008 | `us2_bypass_tap` |
| Different `seed` values give identical output | FR-009 | `determinism` |
| No heap activity inside `process` / `process_in_place` / `reset` | FR-015, FR-017 | `alloc_free` |
| `Pipeline: Send + Sync` | contract | compile-time assertion in `pipeline.rs` |
| Each pipeline retains ≤ 1 MiB of heap | plan: memory constraint | `memory` |
