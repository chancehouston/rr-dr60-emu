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
    // The spec 001 configuration (AGC bypassed), the 0.2 default with the AGC on (spec 002
    // T041), and the 0.3 default with the AGC and the VAS on (spec 003 T041, SC-006). The
    // white-noise input never pauses, so the VAS path runs at full cost.
    for config in ["default", "default_agc", "default_vas"] {
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

/// Spec 006 SC-006 (T039): the record path plus the playback path versus the one-pass on the
/// same 60 s stream at 48 kHz with everything on (`default_vas`), five interleaved repeats,
/// the minimum of each; the ratio is printed and must be ≤ 1.25 (engineering target; the
/// expectation is about 1.0–1.05, research R-11).
#[test]
#[ignore = "release-mode timing; run with --ignored"]
fn split_costs_no_headroom() {
    if skip_in_debug() {
        return;
    }
    let x = stimulus::noise(9, 60 * 48_000);
    let settings = configs::memory_capacity(configs::settings("default_vas", 48_000), 61, 1);
    let (mut one_pass, mut split) = (f64::MAX, f64::MAX);
    for _ in 0..5 {
        let mut p = Pipeline::new(settings).unwrap();
        let mut buf = x.clone();
        let t = Instant::now();
        for chunk in buf.chunks_mut(512) {
            let _ = p.process_in_place(chunk);
        }
        one_pass = one_pass.min(t.elapsed().as_nanos() as f64 / x.len() as f64);

        let mut p = Pipeline::new(settings).unwrap();
        let mut out = vec![0.0f32; 512];
        let t = Instant::now();
        p.start_recording().unwrap();
        for chunk in x.chunks(512) {
            let _ = p.record(chunk, &mut []).unwrap();
        }
        let _ = p.stop_recording(&mut []).unwrap();
        p.start_playback(1).unwrap();
        while !p.play(&mut out).unwrap().finished {}
        split = split.min(t.elapsed().as_nanos() as f64 / x.len() as f64);
    }
    let ratio = split / one_pass;
    println!(
        "one-pass {one_pass:.1} ns/sample, record + play {split:.1} ns/sample, ratio {ratio:.3} (SC-006)"
    );
    assert!(
        ratio <= 1.25,
        "record + play costs {ratio:.2}x the one-pass (SC-006)"
    );
}
