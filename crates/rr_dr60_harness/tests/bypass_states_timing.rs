//! Spec 021 FR-010 (tasks.md T020, head-only): the bypassed steady state and a blend every
//! 200 ms on the player against the all-on state, reported; the worst 4096-sample block of the
//! blending run asserted against the real-time budget (1000 × samples ÷ kHz µs). Release-only.
//! Run: `cargo test -p rr_dr60_harness --release --test bypass_states_timing -- --ignored --nocapture --test-threads=1`

use std::time::Instant;

use rr_dr60::{Pipeline, Route, Settings, Stream};

fn noise(rate: u32, seconds: usize) -> Vec<f32> {
    let mut state = 0x021u64;
    (0..seconds * rate as usize)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            ((state >> 40) as f32 / (1u64 << 23) as f32 - 1.0) * 0.1
        })
        .collect()
}

#[test]
#[ignore = "release-mode timing; run with --ignored"]
fn bypass_states() {
    if cfg!(debug_assertions) {
        eprintln!("timing tests need --release; skipped");
        return;
    }
    let rate = 48_000u32;
    let x = noise(rate, 20);
    let mut s = Settings::new(rate);
    s.speech_codec_enabled = false;
    s.memory.capacity_s = 30;
    let mut p = Pipeline::new(s).unwrap();
    p.set_route(Route::Speaker);
    p.start_recording().unwrap();
    for chunk in x.chunks(4096) {
        let _ = p.record(chunk, &mut []).unwrap();
    }
    let _ = p.stop_recording(&mut []).unwrap();
    let mut out = vec![0.0f32; 4096];
    let mut run = |schedule: &dyn Fn(&mut Pipeline)| -> (f64, f64) {
        let mut best = f64::MAX;
        let mut worst_block = 0.0f64;
        for _ in 0..15 {
            p.start_playback(1).unwrap();
            schedule(&mut p);
            let mut n = 0usize;
            let t = Instant::now();
            loop {
                let b = Instant::now();
                let info = p.play(&mut out).unwrap();
                worst_block = worst_block.max(b.elapsed().as_secs_f64() * 1e6);
                n += info.produced;
                if info.finished {
                    break;
                }
            }
            best = best.min(t.elapsed().as_secs_f64() * 1e9 / n as f64);
        }
        (best, worst_block)
    };
    let (all_on, _) = run(&|_| {});
    let (bypassed, _) = run(&|p| {
        p.schedule_change(Stream::Playback, 0, 22, 0.0, 1).unwrap();
        p.schedule_change(Stream::Playback, 0, 23, 0.0, 1).unwrap();
    });
    let (blending, worst) = run(&|p| {
        let step = u64::from(rate) / 5; // every 200 ms
        for i in 0..64u64 {
            let on = i % 2 == 1;
            p.schedule_change(Stream::Playback, i * step, 23, if on { 1.0 } else { 0.0 }, i + 1)
                .unwrap();
        }
    });
    let budget_us = 1000.0 * 4096.0 / 48.0;
    println!(
        "bypass states, 48 kHz, 4096 blocks, speaker route, codec bypassed: all-on {all_on:.2} ns, bypassed (11 + 12 at rest) {bypassed:.2} ns ({:+.2} %), a stage-12 toggle every 200 ms {blending:.2} ns ({:+.2} %); worst 4096-sample block {worst:.1} us of {budget_us:.0}",
        100.0 * (bypassed / all_on - 1.0),
        100.0 * (blending / all_on - 1.0)
    );
    assert!(worst < budget_us, "the worst block {worst} us exceeds the budget {budget_us} us"); // 021 FR-010
}
