# Contract: Rust API (delta vs spec 003)

The public surface of `rr_dr60` changes in three places. Everything else (`Settings`, `AgcSettings`, `VasSettings`, `Tap`, `VasMode`, `Pipeline` methods, `BlockInfo`, `VasEvent`, `Error`, `Setting`) is unchanged.

```rust
/// CHANGED: seven rates; 6 kHz is the identity rate (spec 004 FR-002).
pub const SUPPORTED_HOST_RATES: [u32; 7] = [6000, 8000, 16000, 44100, 48000, 88200, 96000];

/// CHANGED: the device's measured sample rate (A-026, S-002).
pub const DEVICE_RATE_HZ: u32 = 6000;

/// CHANGED: 0.4.0.
pub const VERSION: &str = "0.4.0";
```

- **Breaking**: `SUPPORTED_HOST_RATES` changes type from `[u32; 6]` to `[u32; 7]`. Callers that destructure or size by it must update. Pre-1.0, recorded in the CHANGELOG.
- `Error::UnsupportedHostRate { requested }`'s `Display` lists the seven rates (it formats the constant; no code change if it already does).
- `Settings::new(6000)` is valid and `Pipeline::new` at 6000 applies no rate conversion (`latency_samples()` is the stages' 1 kHz group delay alone, rounded: about 1 sample with both stages on, 0 with both bypassed).
- Default `latency_samples()` values change at every rate (research R-03); they are reported, not promised, and the CHANGELOG lists them.

## Test hooks (`__test-hooks` feature; unpublished surface)

```rust
impl Pipeline {
    /// CHANGED: one coefficient set per stage (research R-06).
    pub fn __with_stage_sos(
        settings: Settings,
        record: [[f64; 5]; 5],
        playback: [[f64; 5]; 3],
    ) -> Result<Self, Error>;
}

pub mod __test_hooks {
    /// CHANGED: was `voiceband_sos() -> [[f64; 5]; 6]`.
    pub fn record_sos() -> [[f64; 5]; 5];
    pub fn playback_sos() -> [[f64; 5]; 3];
}
```

## Documentation changes (rustdoc)

- Crate docs: "internal rate of 8 kHz (A-001)" → 6 kHz (A-026); the two stages described as the codec's transmit band-pass and receive low-pass templates scaled to the measured clock (A-027), with the medium-confidence caveat.
- `Settings::host_rate_hz`: lists seven rates.
- `AgcSettings`/`VasSettings`: unchanged text (values are in ms and dB).

## Behavior contract

- Output of every configuration changes (new device rate, new boundary, new stages). Recorded once in the CHANGELOG with the golden re-bless (contracts/golden-format.md).
- The AGC and VAS laws are unchanged: same defaults, same ranges, same validation, same error values.
