//! The C smoke test's reference (spec 006 US4 scenario 1; contracts/c-api.md › Smoke test
//! additions): the Rust path's playback of the smoke stimulus, hashed, so `tests/c/smoke.c`
//! can compare its own playback against the Rust path without a file exchange.
//!
//! The stimulus is a 1 kHz square wave at −20 dBFS (±0.1, 24 samples up and 24 down at
//! 48 kHz): every sample is exactly representable, so C and Rust feed identical bits, which a
//! `sin()` from two libms would not guarantee.

use rr_dr60::Pipeline;

/// The host rate of the smoke loop.
pub const SMOKE_RATE: u32 = 48_000; // engineering target (006 US4)
/// One second of input.
pub const SMOKE_SAMPLES: usize = 48_000; // engineering target (006 US4)
/// The record block the smoke test uses.
pub const SMOKE_BLOCK: usize = 480; // engineering target (006 US4)

/// FNV-1a 64 of the Rust playback (pre-drain and drain) of the smoke stimulus with the default
/// settings at 48 kHz; `smoke.c` embeds the same value. Re-pin it only with an intended output
/// change (the golden procedure, spec 006 R-09).
pub const SMOKE_REFERENCE_HASH: u64 = 0xc3e9_0464_3c88_24d7; // measured 2026-10-12 (49 692 samples); pinned by `ffi_parity::smoke_reference_hash_is_pinned`

/// The smoke stimulus.
pub fn stimulus() -> Vec<f32> {
    (0..SMOKE_SAMPLES)
        .map(|i| if i % 48 < 24 { 0.1 } else { -0.1 })
        .collect()
}

/// FNV-1a 64 over the little-endian bytes of `samples`.
pub fn fnv1a64(samples: &[f32]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for s in samples {
        for b in s.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

/// The Rust path's playback of the smoke stimulus (default settings, the VAS on, recorded in
/// `SMOKE_BLOCK` blocks and played in the same blocks to `finished`), hashed, with its length.
pub fn rust_playback_hash() -> (u64, usize) {
    let mut p = Pipeline::new(rr_dr60::Settings::new(SMOKE_RATE)).expect("rate");
    p.start_recording().expect("idle");
    for chunk in stimulus().chunks(SMOKE_BLOCK) {
        let info = p.record(chunk, &mut []).expect("recording");
        assert!(info.stopped.is_none());
    }
    let _ = p.stop_recording(&mut []).expect("recording");
    let out = crate::split_checks::play_to_end(&mut p, 1, SMOKE_BLOCK);
    (fnv1a64(&out), out.len())
}
