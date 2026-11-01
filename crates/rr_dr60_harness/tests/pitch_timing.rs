//! Spec 023 FR-009 (tasks.md T017): the per-sample cost of the pitch-kept mode, written against
//! **version 0.22's public API only** for the figures the A/B script compares (the default mode
//! at level 3 and level 5), so `scripts/ab-timing.sh <rev> pitch` can copy this file into a
//! 0.22 worktree; the pitch-kept figures use 0.23's setter and are skipped there by the
//! `HAS_MODE` cfg the script sets. Ignored by default (a release-mode timing; never under
//! coverage). Run:
//! `cargo test -p rr_dr60_harness --release --test pitch_timing -- --ignored --nocapture --test-threads=1`

use std::time::Instant;

use rr_dr60::{Pipeline, Route, Settings};

fn noise(rate: u32, seconds: usize) -> Vec<f32> {
    let mut state = 0x023u64;
    (0..seconds * rate as usize)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            ((state >> 40) as f32 / (1u64 << 23) as f32 - 1.0) * 0.1
        })
        .collect()
}

/// An engine at `rate` on the speaker route holding a recording of `x`, the codec `codec`.
fn engine(rate: u32, codec: bool, x: &[f32]) -> Pipeline {
    let mut s = Settings::new(rate);
    s.speech_codec_enabled = codec;
    s.memory.capacity_s = 30;
    let mut p = Pipeline::new(s).unwrap();
    p.set_route(Route::Speaker);
    p.start_recording().unwrap();
    for chunk in x.chunks(4096) {
        let _ = p.record(chunk, &mut []).unwrap();
    }
    let _ = p.stop_recording(&mut []).unwrap();
    p
}

/// One playback of recording 1 at `level` in blocks of `block`: ns per host sample and the
/// worst block in nanoseconds.
fn play_ns(p: &mut Pipeline, level: u8, block: usize) -> (f64, f64) {
    let mut out = vec![0.0f32; block];
    p.set_playback_speed(level).unwrap();
    p.start_playback(1).unwrap();
    let mut n = 0usize;
    let mut worst = 0.0f64;
    let t = Instant::now();
    loop {
        let b = Instant::now();
        let info = p.play(&mut out).unwrap();
        worst = worst.max(b.elapsed().as_nanos() as f64);
        n += info.produced;
        if info.finished {
            break;
        }
    }
    (t.elapsed().as_nanos() as f64 / n as f64, worst)
}

#[cfg(not(no_speed_mode))]
fn set_mode(p: &mut Pipeline, kept: bool) {
    p.set_playback_speed_mode(if kept {
        rr_dr60::speed::SpeedMode::PitchKept
    } else {
        rr_dr60::speed::SpeedMode::PitchWithSpeed
    });
}

#[cfg(no_speed_mode)]
fn set_mode(_p: &mut Pipeline, kept: bool) {
    assert!(!kept, "the baseline has no pitch-kept mode");
}

fn ceiling(name: &str, default: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// The figures the A/B script reads (`level3_ns=`, `level5_ns=`: the default mode, 20 s of
/// −20 dB noise at 48 kHz, the codec bypassed, 4096-sample blocks, the fastest of 45) and, in
/// the working tree, the pitch-kept figures per level: the increment over level 3 against the
/// default mode's increment (FR-009: at most 3 ×; `RR_DR60_PITCH_COST_CEILING`, 4.0 on the CI
/// runner), the 6 kHz figures reported, and the worst 64-sample block at 6 and 48 kHz with the
/// codec on against the block's real-time budget (1000 × block ÷ kHz µs).
#[test]
#[ignore = "release-mode timing; run with --ignored"]
fn pitch_cost() {
    if cfg!(debug_assertions) {
        eprintln!("timing tests need --release; skipped");
        return;
    }
    let rate = 48_000u32;
    let x = noise(rate, 20);
    let mut p = engine(rate, false, &x);
    set_mode(&mut p, false);
    let mut t3 = f64::MAX;
    let mut t5 = f64::MAX;
    for _ in 0..45 {
        t3 = t3.min(play_ns(&mut p, 3, 4096).0);
        t5 = t5.min(play_ns(&mut p, 5, 4096).0);
    }
    println!("level3_ns={t3:.3}");
    println!("level5_ns={t5:.3}");
    if cfg!(no_speed_mode) {
        return;
    }
    let ratio_ceiling = ceiling("RR_DR60_PITCH_COST_CEILING", 3.0); // 023 FR-009
    for rate in [48_000u32, 6000] {
        let x = noise(rate, 20);
        let mut p = engine(rate, false, &x);
        for level in [1u8, 2, 4, 5] {
            let (mut t_default, mut t_kept, mut t3) = (f64::MAX, f64::MAX, f64::MAX);
            for _ in 0..45 {
                set_mode(&mut p, false);
                t_default = t_default.min(play_ns(&mut p, level, 4096).0);
                t3 = t3.min(play_ns(&mut p, 3, 4096).0);
                set_mode(&mut p, true);
                t_kept = t_kept.min(play_ns(&mut p, level, 4096).0);
            }
            set_mode(&mut p, false);
            let inc_default = t_default - t3;
            let inc_kept = t_kept - t3;
            let ratio = inc_kept / inc_default.max(1e-9);
            println!(
                "pitch-kept cost, speaker route, {} kHz, 4096 blocks, codec bypassed: level {level} default {t_default:.2} ns, pitch kept {t_kept:.2} ns, level 3 {t3:.2} ns per host sample; increments {inc_default:+.2} / {inc_kept:+.2} ns, ratio {ratio:.2} (023 FR-009: at most 3 ×, ceiling here {ratio_ceiling}{})",
                rate / 1000,
                if rate == 48_000 { "" } else { "; reported" }
            );
            if rate == 48_000 {
                assert!(
                    ratio <= ratio_ceiling,
                    "level {level}: the pitch-kept increment is {ratio:.2} × the default's (023 FR-009)"
                );
            }
        }
    }
    // The worst 64-sample block with the codec on, the mode kept, the level changed before
    // every block (3 → 1 → 5 → 2 → 4 → 3 …) at 6 and 48 kHz: within the block's real-time
    // budget.
    for rate in [6000u32, 48_000] {
        let x = noise(rate, 10);
        let mut p = engine(rate, true, &x);
        set_mode(&mut p, true);
        let budget_ns = 1000.0 * 64.0 / (rate as f64 / 1000.0) * 1000.0;
        let mut worst = 0.0f64;
        for _ in 0..3 {
            let mut out = vec![0.0f32; 64];
            p.set_playback_speed(3).unwrap();
            p.start_playback(1).unwrap();
            let levels = [1u8, 5, 2, 4, 3];
            let mut i = 0usize;
            loop {
                p.set_playback_speed(levels[i % 5]).unwrap();
                i += 1;
                let b = Instant::now();
                let info = p.play(&mut out).unwrap();
                worst = worst.max(b.elapsed().as_nanos() as f64);
                if info.finished {
                    break;
                }
            }
        }
        println!(
            "pitch-kept worst 64-sample block, {} kHz, codec on, a level change every block: {:.1} µs of {:.1} µs ({:.1} % of real time)",
            rate / 1000,
            worst / 1000.0,
            budget_ns / 1000.0,
            100.0 * worst / budget_ns
        );
        assert!(
            worst < budget_ns,
            "{} kHz: the worst block exceeds real time",
            rate / 1000
        );
    }
}
