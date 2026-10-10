//! Release-mode timing checks (FR-015 bounded work, FR-022, SC-006; tasks.md T058).
//! Run with: `cargo test -p rr_dr60_harness --release --test timing -- --ignored --nocapture`

use std::time::Instant;

use rr_dr60::Pipeline;
use rr_dr60_harness::{configs, stimulus};

fn skip_in_debug() -> bool {
    if cfg!(debug_assertions) {
        eprintln!("timing tests need --release; skipped");
        return true;
    }
    false
}

/// Median nanoseconds per sample to process `x` in blocks of `block`, over 9 runs.
fn ns_per_sample(config: &str, x: &[f32], block: usize, warm: Option<&[f32]>) -> f64 {
    let mut runs: Vec<f64> = (0..9)
        .map(|_| {
            let mut p = Pipeline::new(configs::settings(config, 48_000)).unwrap();
            if let Some(w) = warm {
                let _ = p.process_in_place(&mut w.to_vec());
            }
            let mut buf = x.to_vec();
            let t = Instant::now();
            for chunk in buf.chunks_mut(block) {
                let _ = p.process_in_place(chunk);
            }
            t.elapsed().as_nanos() as f64 / x.len() as f64
        })
        .collect();
    runs.sort_by(f64::total_cmp);
    runs[4]
}

#[test]
#[ignore = "release-mode timing; run with --ignored"]
fn per_sample_time_is_flat_and_fast() {
    if skip_in_debug() {
        return;
    }
    let x = stimulus::noise(0x0D60, 1 << 20);
    let long_stream = stimulus::noise(9, 60 * 48_000);
    // The spec 001 configuration (AGC bypassed) and the real 0.2 default with the AGC on
    // (spec 002 T041, SC-006).
    for config in ["default", "default_agc"] {
        println!("{config}:");
        let mut times: Vec<(String, f64)> = [1usize, 64, 4096, 1 << 20]
            .iter()
            .map(|&b| (format!("block {b}"), ns_per_sample(config, &x, b, None)))
            .collect();
        times.push((
            "after 60 s stream".into(),
            ns_per_sample(config, &x, 4096, Some(&long_stream)),
        ));
        for (name, t) in &times {
            println!("{name:>20}: {t:.1} ns/sample");
        }
        let (min, max) = times.iter().fold((f64::MAX, 0.0f64), |(lo, hi), (_, t)| {
            (lo.min(*t), hi.max(*t))
        });
        assert!(
            max / min <= 3.0,
            "{config}: per-sample time varies {:.2}x (FR-015 bounded work)",
            max / min
        );

        // SC-006 (engineering target): 60 s of 48 kHz audio at >= 20x real time.
        let mut p = Pipeline::new(configs::settings(config, 48_000)).unwrap();
        let mut buf = long_stream.clone();
        let t = Instant::now();
        for chunk in buf.chunks_mut(512) {
            let _ = p.process_in_place(chunk);
        }
        let speed = 60.0 / t.elapsed().as_secs_f64();
        println!("speed: {speed:.0}x real time at 48 kHz");
        assert!(
            speed >= 20.0,
            "{config}: only {speed:.1}x real time (SC-006)"
        );
    }
}

#[test]
#[ignore = "release-mode, 10 minutes of audio; run with --ignored"]
fn ten_minute_block_equals_small_blocks() {
    if skip_in_debug() {
        return;
    }
    let x = stimulus::noise(0x0D60, 10 * 60 * 48_000);
    let mut whole = x.clone();
    let _ = Pipeline::new(configs::settings("default", 48_000))
        .unwrap()
        .process_in_place(&mut whole);
    let mut p = Pipeline::new(configs::settings("default", 48_000)).unwrap();
    let mut pieces = x.clone();
    for chunk in pieces.chunks_mut(4096) {
        let _ = p.process_in_place(chunk);
    }
    assert!(
        whole == pieces,
        "10-minute single block differs from 4096-sample blocks (FR-014)"
    );
}
