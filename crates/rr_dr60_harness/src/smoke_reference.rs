//! The C smoke test's reference (spec 006 US4 scenario 1; spec 008 US4 scenario 1;
//! contracts/c-api.md › Smoke test): the Rust path's recording and playback of the smoke
//! stimulus with the **0.8 default settings (the speech codec on)**, hashed, so `tests/c/smoke.c`
//! can compare its own frame bytes and playback against the Rust path without a file exchange.
//!
//! The stimulus is a 1 kHz square wave at −18 dBFS (±0.125, 24 samples up and 24 down at
//! 48 kHz): ±0.125 is exactly representable in binary32 (0.1 is not; a C literal need only be
//! nearest-or-adjacent, C11 6.4.4.2), so C and Rust feed identical bits, which a `sin()` from
//! two libms would not guarantee either.

use rr_dr60::Pipeline;

/// The host rate of the smoke loop.
pub const SMOKE_RATE: u32 = 48_000; // engineering target (006 US4)
/// One second of input.
pub const SMOKE_SAMPLES: usize = 48_000; // engineering target (006 US4)
/// The record block the smoke test uses.
pub const SMOKE_BLOCK: usize = 480; // engineering target (006 US4)

/// FNV-1a 64 of the Rust playback (pre-drain and drain) of the smoke stimulus with the default
/// settings at 48 kHz, the speech codec on; `smoke.c` embeds the same value. Re-pin it only with
/// an intended output change (the golden procedure, spec 006 R-09).
pub const SMOKE_REFERENCE_HASH: u64 = 0x971c_70e3_428c_23c7; // measured 2026-10-14 (50404 samples, the ±0.125 stimulus, the codec on; re-pinned once with the fixed-gain mean 54 dB, T053 B1); pinned by `ffi_parity::smoke_reference_hash_is_pinned`
/// FNV-1a 64 of the recording's frame bytes (the exported form's units, 10 bytes per frame).
pub const SMOKE_FRAMES_HASH: u64 = 0x5403_5d54_bd48_af25; // measured 2026-10-14 (51 frames, 510 bytes; re-pinned once with the fixed-gain mean 54 dB, T053 B1); 008 T042
/// The recording's frame count with the default settings.
pub const SMOKE_FRAMES: u64 = 51; // 6031 represented samples → ⌈6031 / 120⌉ frames (008 FR-013)
/// The stop's `silence_fed_samples` with the default settings.
pub const SMOKE_SILENCE_FED: u64 = 149; // 120 × 51 − 6031 + 60 (008 FR-013)
/// The playback's length in host samples (the recording plus the drain).
pub const SMOKE_PLAYBACK_SAMPLES: usize = 50_404; // ⌈6120 × 8⌉ + the 1444-sample drain at 48 kHz (006 FR-007, FR-008)

/// The smoke stimulus.
pub fn stimulus() -> Vec<f32> {
    (0..SMOKE_SAMPLES)
        .map(|i| if i % 48 < 24 { 0.125 } else { -0.125 })
        .collect()
}

/// FNV-1a 64 over the little-endian bytes of `samples`.
pub fn fnv1a64(samples: &[f32]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for s in samples {
        h = fnv1a64_bytes_from(h, &s.to_le_bytes());
    }
    h
}

/// FNV-1a 64 over `bytes`.
pub fn fnv1a64_bytes(bytes: &[u8]) -> u64 {
    fnv1a64_bytes_from(0xcbf2_9ce4_8422_2325u64, bytes)
}

fn fnv1a64_bytes_from(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// What the Rust path makes of the smoke stimulus (default settings, the VAS and the speech
/// codec on, recorded in `SMOKE_BLOCK` blocks and played in the same blocks to `finished`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reference {
    /// FNV-1a 64 of the playback (pre-drain and drain).
    pub playback_hash: u64,
    /// The playback's length in host samples.
    pub playback_samples: usize,
    /// FNV-1a 64 of the recording's frame bytes (the exported form's units).
    pub frames_hash: u64,
    /// The recording's frames (`frames_total` at the stop; the length ÷ 120).
    pub frames: u64,
    /// The stop's `silence_fed_samples`.
    pub silence_fed: u64,
}

/// The Rust path's reference figures.
pub fn rust_reference() -> Reference {
    // Spec 009 FR-016: the 0.8 case keeps 0.8's output (the chain bypassed, the tap after
    // stage 10); the route cases of 009 T031/T033 have their own constants.
    let mut p = Pipeline::new(crate::configs::settings_008(SMOKE_RATE)).expect("rate");
    p.start_recording().expect("idle");
    for chunk in stimulus().chunks(SMOKE_BLOCK) {
        let info = p.record(chunk, &mut []).expect("recording");
        assert!(info.stopped.is_none());
    }
    let out = p.stop_recording(&mut []).expect("recording");
    let rr_dr60::StopResult::Kept { number, length } = out.result else {
        unreachable!("the stimulus is recorded")
    };
    assert_eq!(out.flush.frames_total * 120, length);
    let bytes = crate::split_checks::export_bytes(&p, number);
    let frames_hash = fnv1a64_bytes(&bytes[48..bytes.len() - 4]);
    let play = crate::split_checks::play_to_end(&mut p, number, SMOKE_BLOCK);
    Reference {
        playback_hash: fnv1a64(&play),
        playback_samples: play.len(),
        frames_hash,
        frames: out.flush.frames_total,
        silence_fed: out.silence_fed_samples,
    }
}

/// The Rust path's playback of the smoke stimulus, hashed, with its length (the 006 reading).
pub fn rust_playback_hash() -> (u64, usize) {
    let r = rust_reference();
    (r.playback_hash, r.playback_samples)
}
