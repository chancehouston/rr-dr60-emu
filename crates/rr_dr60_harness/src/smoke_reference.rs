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

/// Spec 009 US4 (T031, T033): the four route cases' hashes, the stimulus recorded once in the
/// 0.9 default (the chain on) and played on the speaker route at `VOLUME_REFERENCE` and at 1.0
/// (the clipper engaged) and on the earphone route at both. Pinned once from the Rust path
/// (`rust_route_references`); `ffi_parity::smoke_reference_hash_is_pinned` checks them.
pub const SMOKE_SPEAKER_REF_HASH: u64 = 0xe414_787b_3395_1ea4; // measured 2026-10-15 from the Rust path (53 168 samples; 009 T033)
/// See [`SMOKE_SPEAKER_REF_HASH`].
pub const SMOKE_SPEAKER_FULL_HASH: u64 = 0xbf5f_8794_084b_0d08; // measured 2026-10-15 from the Rust path (53 168 samples; 009 T033)
/// See [`SMOKE_SPEAKER_REF_HASH`].
pub const SMOKE_EARPHONE_REF_HASH: u64 = 0xe7a0_621c_54ea_1b7a; // measured 2026-10-15 (Z_EAR 6.4 kΩ, T043 B1) from the Rust path (53 168 samples; 009 T033)
/// See [`SMOKE_SPEAKER_REF_HASH`].
pub const SMOKE_EARPHONE_FULL_HASH: u64 = 0x4a93_f668_93e3_04fb; // measured 2026-10-15 from the Rust path (53 168 samples; 009 T033)
/// The route cases' playback length: the 0.8 length plus the chain's settle at 48 kHz
/// (spec 009 research R-08: 50 404 + 2 764).
pub const SMOKE_ROUTE_PLAYBACK_SAMPLES: usize = 53_168;

/// The four route cases `(route, position, hash, length)` from the Rust path: one recording of
/// the stimulus in the 0.9 default, played four times with the controls set before each
/// playback (spec 009 FR-014, US4 scenario 3).
pub fn rust_route_references() -> [(rr_dr60::Route, f32, u64, usize); 4] {
    use rr_dr60::Route;
    use rr_dr60::output::VOLUME_REFERENCE;
    let mut p = Pipeline::new(crate::configs::settings_011(SMOKE_RATE)).expect("rate");
    p.start_recording().expect("idle");
    for chunk in stimulus().chunks(SMOKE_BLOCK) {
        let info = p.record(chunk, &mut []).expect("recording");
        assert!(info.stopped.is_none());
    }
    let out = p.stop_recording(&mut []).expect("recording");
    let rr_dr60::StopResult::Kept { number, .. } = out.result else {
        unreachable!("the stimulus is recorded")
    };
    let cases = [
        (Route::Speaker, VOLUME_REFERENCE),
        (Route::Speaker, 1.0),
        (Route::Earphone, VOLUME_REFERENCE),
        (Route::Earphone, 1.0),
    ];
    cases.map(|(route, volume)| {
        p.set_route(route);
        p.set_volume(volume).expect("a valid position");
        let play = crate::split_checks::play_to_end(&mut p, number, SMOKE_BLOCK);
        (route, volume, fnv1a64(&play), play.len())
    })
}

/// The Rust path's playback of the smoke stimulus, hashed, with its length (the 006 reading).
pub fn rust_playback_hash() -> (u64, usize) {
    let r = rust_reference();
    (r.playback_hash, r.playback_samples)
}

/// Spec 010 US4 (T027, T031): the two output-clamp cases' hashes, a ±1.0 square wave (24 up,
/// 24 down at 48 kHz; exactly representable) recorded once in the 0.9 default with the option
/// on and played at position 1 on the speaker route and on the earphone route, every sample
/// within [−1, 1]. Pinned once from the Rust path (`rust_clamp_references`);
/// `ffi_parity::smoke_reference_hash_is_pinned` checks them.
pub const SMOKE_SPEAKER_CLAMP_HASH: u64 = 0x5a7f_9b91_a49f_806e; // measured 2026-10-15 from the Rust path (53 168 samples; 010 T031)
/// See [`SMOKE_SPEAKER_CLAMP_HASH`].
pub const SMOKE_EARPHONE_CLAMP_HASH: u64 = 0xfbaf_bb6b_1150_7362; // measured 2026-10-15 from the Rust path (53 168 samples; 010 T031)

/// The clamp cases' stimulus: a full-scale ±1.0 square wave, 24 samples up and 24 down.
pub fn clamp_stimulus() -> Vec<f32> {
    (0..SMOKE_SAMPLES)
        .map(|i| if i % 48 < 24 { 1.0 } else { -1.0 })
        .collect()
}

/// The two clamp cases `(route, hash, length, samples beyond the bound in the option-off
/// playback on the speaker route)` from the Rust path: one recording of the full-scale square
/// in the 0.9 default with the option on, played at position 1 on each route; and the same
/// recording through an option-off engine on the speaker route, whose playback exceeds the
/// bound (so the loop shows the option acting).
pub fn rust_clamp_references() -> ([(rr_dr60::Route, u64, usize); 2], usize) {
    use rr_dr60::Route;
    let record = |clamp: bool| -> (Pipeline, u32) {
        let mut s = crate::configs::settings_011(SMOKE_RATE);
        s.output.clamp_enabled = clamp;
        let mut p = Pipeline::new(s).expect("rate");
        p.start_recording().expect("idle");
        for chunk in clamp_stimulus().chunks(SMOKE_BLOCK) {
            let info = p.record(chunk, &mut []).expect("recording");
            assert!(info.stopped.is_none());
        }
        let out = p.stop_recording(&mut []).expect("recording");
        let rr_dr60::StopResult::Kept { number, .. } = out.result else {
            unreachable!("the stimulus is recorded")
        };
        (p, number)
    };
    let (mut on, number) = record(true);
    let cases = [Route::Speaker, Route::Earphone].map(|route| {
        on.set_route(route);
        on.set_volume(1.0).expect("a valid position");
        let play = crate::split_checks::play_to_end(&mut on, number, SMOKE_BLOCK);
        assert!(play.iter().all(|v| v.abs() <= 1.0), "{route:?}: bounded");
        (route, fnv1a64(&play), play.len())
    });
    let (mut off, number_off) = record(false);
    off.set_route(Route::Speaker);
    off.set_volume(1.0).expect("a valid position");
    let play = crate::split_checks::play_to_end(&mut off, number_off, SMOKE_BLOCK);
    let beyond = play.iter().filter(|v| v.abs() > 1.0).count();
    (cases, beyond)
}

/// Spec 011 T035 (contracts/c-api.md › Smoke loop): the volume-transition case's hash: the
/// full-scale square of the clamp cases recorded once in the default engine (the option off)
/// and played on the speaker route at 0.1, with the position set to 1.0 before play block 10
/// and to 0.4 before block 11 (480-sample blocks: a retarget mid-transition). Pinned once from
/// the Rust path (`rust_ramp_reference`); `ffi_parity::smoke_reference_hash_is_pinned` checks it.
pub const SMOKE_SPEAKER_RAMP_HASH: u64 = 0xcc01_33ff_c64a_c96e; // measured 2026-10-16 from the Rust path (53 168 samples; 011 T035)

/// The play blocks before which the transition case sets the position (contracts/c-api.md).
pub const SMOKE_RAMP_CHANGES: [(usize, f32); 2] = [(10, 1.0), (11, 0.4)]; // engineering target (011 T035)

/// The volume-transition case `(hash, length)` from the Rust path.
pub fn rust_ramp_reference() -> (u64, usize) {
    let mut p = Pipeline::new(crate::configs::settings_011(SMOKE_RATE)).expect("rate");
    p.start_recording().expect("idle");
    for chunk in clamp_stimulus().chunks(SMOKE_BLOCK) {
        let info = p.record(chunk, &mut []).expect("recording");
        assert!(info.stopped.is_none());
    }
    let out = p.stop_recording(&mut []).expect("recording");
    let rr_dr60::StopResult::Kept { number, .. } = out.result else {
        unreachable!("the stimulus is recorded")
    };
    p.set_route(rr_dr60::Route::Speaker);
    p.set_volume(0.1).expect("a valid position");
    p.start_playback(number).expect("idle");
    let mut play = Vec::new();
    let mut buf = vec![0.0f32; SMOKE_BLOCK];
    for block in 0.. {
        for &(at, v) in &SMOKE_RAMP_CHANGES {
            if at == block {
                p.set_volume(v).expect("a valid position");
            }
        }
        let info = p.play(&mut buf).expect("playing");
        play.extend_from_slice(&buf[..info.produced]);
        if info.finished {
            break;
        }
    }
    (fnv1a64(&play), play.len())
}

/// Spec 013 T029 (contracts/c-api.md › Smoke loop): the playback-speed case's hash: the
/// full-scale square of the clamp cases recorded once in the default engine (0.11's settings),
/// played on the speaker route at volume 0.1 at speed level 5, the level set to 1 before play
/// block 10 (480-sample blocks): a live change between two non-3 levels. Pinned once from the
/// Rust path (`rust_speed_reference`); `ffi_parity::smoke_reference_hash_is_pinned` checks it.
pub const SMOKE_SPEED_HASH: u64 = 0x0dd9_0285_502e_dcc3; // measured 2026-10-17 from the Rust path (79 776 samples; 013 T029)

/// The speed case's length in host samples (the level-5 stretch, then level 1 to the end).
pub const SMOKE_SPEED_SAMPLES: usize = 79_776; // measured 2026-10-17 (013 T029)

/// The play block before which the speed case sets level 1 (contracts/c-api.md).
pub const SMOKE_SPEED_CHANGE_BLOCK: usize = 10; // engineering target (013 T029)

/// The speed case `(hash, length, length helper at level 5)` from the Rust path.
pub fn rust_speed_reference() -> (u64, usize, u64) {
    let mut p = Pipeline::new(crate::configs::settings_011(SMOKE_RATE)).expect("rate");
    p.start_recording().expect("idle");
    for chunk in clamp_stimulus().chunks(SMOKE_BLOCK) {
        let info = p.record(chunk, &mut []).expect("recording");
        assert!(info.stopped.is_none());
    }
    let out = p.stop_recording(&mut []).expect("recording");
    let rr_dr60::StopResult::Kept { number, .. } = out.result else {
        unreachable!("the stimulus is recorded")
    };
    p.set_route(rr_dr60::Route::Speaker);
    p.set_volume(0.1).expect("a valid position");
    p.set_playback_speed(5).expect("a valid level");
    let helper = p.playback_length_samples(number, 5).expect("a recording");
    p.start_playback(number).expect("idle");
    let mut play = Vec::new();
    let mut buf = vec![0.0f32; SMOKE_BLOCK];
    for block in 0.. {
        if block == SMOKE_SPEED_CHANGE_BLOCK {
            p.set_playback_speed(1).expect("a valid level");
        }
        let info = p.play(&mut buf).expect("playing");
        play.extend_from_slice(&buf[..info.produced]);
        if info.finished {
            break;
        }
    }
    (fnv1a64(&play), play.len(), helper)
}

/// Spec 012 T048, T049 (contracts/c-api.md › Smoke loop): the microphone cases' hashes, pinned
/// once from the Rust path (`rust_mic_references`); `ffi_parity::smoke_reference_hash_is_pinned`
/// checks them and `smoke.c` embeds them. (a) the 0.12 default one-pass at 48 kHz on the smoke
/// square (1 s) followed by 0.5 s of silence, in 480-sample blocks; (b) two recordings of 1 s of
/// silence in the 0.12 default, a `reset` between them, each played to the end: the hiss of
/// counters 0 and 1.
pub const SMOKE_MIC_DEFAULT_HASH: u64 = 0x533e_9117_7f8c_c1d6; // measured 2026-10-16 from the Rust path (72 000 samples; 012 T049, re-pinned after the review's B1 and B5)
/// The (a) case's output length in host samples.
pub const SMOKE_MIC_DEFAULT_SAMPLES: usize = 72_000; // 1.5 s at 48 kHz: the 0.5 s of silence is shorter than the hang time (A-025), so nothing is dropped
/// The (b) case's first recording's playback hash.
pub const SMOKE_MIC_REC0_HASH: u64 = 0x0ef2_704a_7566_0621; // measured 2026-10-16 from the Rust path (53 168 samples; 012 T049, re-pinned after B5)
/// The (b) case's second recording's playback hash.
pub const SMOKE_MIC_REC1_HASH: u64 = 0x442d_dc90_fb8b_3e11; // measured 2026-10-16 from the Rust path (53 168 samples; 012 T049, re-pinned after B5)
/// The (b) case's playback length (each recording).
pub const SMOKE_MIC_REC_SAMPLES: usize = 53_168; // the recording (one hang time of hiss, frame-padded) plus the drain at 48 kHz (012 T049)

/// The microphone cases from the Rust path: `((hash, length) of (a), [(hash, length); 2] of (b))`.
pub fn rust_mic_references() -> ((u64, usize), [(u64, usize); 2]) {
    let s = rr_dr60::Settings::new(SMOKE_RATE); // the 0.12 default (012 FR-001)
    let mut x = stimulus();
    x.extend(std::iter::repeat_n(0.0f32, SMOKE_SAMPLES / 2));
    let mut p = Pipeline::new(s).expect("rate");
    let mut out = Vec::new();
    let mut buf = vec![0.0f32; SMOKE_BLOCK];
    for chunk in x.chunks(SMOKE_BLOCK) {
        let n = p.process(chunk, &mut buf).expect("idle").produced;
        out.extend_from_slice(&buf[..n]);
    }
    let one_pass = (fnv1a64(&out), out.len());
    let mut p = Pipeline::new(s).expect("rate");
    let zeros = vec![0.0f32; SMOKE_BLOCK];
    let recordings = [0, 1].map(|r| {
        if r == 1 {
            p.reset();
        }
        p.start_recording().expect("idle");
        for _ in 0..SMOKE_SAMPLES / SMOKE_BLOCK {
            let info = p.record(&zeros, &mut []).expect("recording");
            assert!(info.stopped.is_none());
        }
        let rr_dr60::StopResult::Kept { number, .. } =
            p.stop_recording(&mut []).expect("recording").result
        else {
            unreachable!("the hiss is recorded")
        };
        let play = crate::split_checks::play_to_end(&mut p, number, SMOKE_BLOCK);
        (fnv1a64(&play), play.len())
    });
    (one_pass, recordings)
}
