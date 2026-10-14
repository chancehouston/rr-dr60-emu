//! Release-mode timing checks (FR-015 bounded work, FR-022, SC-006; tasks.md T058; spec 008
//! SC-006, T049: the speech codec's cost and its release-profile frame-byte digest).
//! Run with: `cargo test -p rr_dr60_harness --release --test timing -- --ignored --nocapture`

use std::time::Instant;

use rr_dr60::Pipeline;
use rr_dr60_harness::{configs, golden, stimulus};

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

/// Spec 008 SC-006 (research R-12; T049): the one-pass with the codec on (`default_codec`, the
/// 0.8 default) at 6 and 48 kHz, reported per host sample and per device sample against the
/// 8 µs-per-device-sample budget (engineering target, 008 SC-006); the worst single 64-sample
/// block at 48 kHz against its 1.33 ms real-time budget, the margin printed; and record plus
/// play through the codec within 25 % of the one-pass (spec 006 SC-006's rule). White noise
/// never pauses the VAS, so the codec runs on every sample.
#[test]
#[ignore = "release-mode timing; run with --ignored"]
fn codec_cost_per_sample_and_worst_block() {
    if skip_in_debug() {
        return;
    }
    const BUDGET_US_PER_DEVICE_SAMPLE: f64 = 8.0; // 008 SC-006
    println!("default_codec (the 0.8 default, the speech codec on):");
    for rate in [6000u32, 48_000] {
        let x = stimulus::noise(0x0D60, 20 * rate as usize);
        let mut runs: Vec<f64> = (0..9)
            .map(|_| {
                let mut p = Pipeline::new(configs::settings("default_codec", rate)).unwrap();
                let mut buf = x.clone();
                let t = Instant::now();
                for chunk in buf.chunks_mut(4096) {
                    let _ = p.process_in_place(chunk);
                }
                t.elapsed().as_nanos() as f64 / x.len() as f64
            })
            .collect();
        runs.sort_by(f64::total_cmp);
        let ns_host = runs[4];
        let us_device = ns_host * f64::from(rate) / 6000.0 / 1000.0;
        println!(
            "  {rate} Hz, blocks of 4096: {ns_host:.1} ns per host sample, {us_device:.3} µs per device sample (budget {BUDGET_US_PER_DEVICE_SAMPLE} µs), {:.0}x real time",
            1e9 / (ns_host * f64::from(rate))
        );
        assert!(
            us_device <= BUDGET_US_PER_DEVICE_SAMPLE,
            "{rate} Hz: {us_device:.3} µs per device sample (008 SC-006)"
        );
    }

    // The worst 64-sample block at 48 kHz: a frame completes in one block of every 15 (120
    // device samples = 960 host samples), so the maximum is the block that encodes and
    // decodes a frame.
    const BLOCK: usize = 64; // 008 SC-006
    let budget_us = BLOCK as f64 * 1000.0 / 48.0; // 1333.3 µs of audio per block
    let x = stimulus::noise(0x0D60, 10 * 48_000);
    let mut worst = 0.0f64;
    let mut blocks: Vec<f64> = Vec::with_capacity(x.len() / BLOCK);
    for _ in 0..3 {
        let mut p = Pipeline::new(configs::settings("default_codec", 48_000)).unwrap();
        let mut buf = x.clone();
        for chunk in buf.chunks_mut(BLOCK) {
            let t = Instant::now();
            let _ = p.process_in_place(chunk);
            let us = t.elapsed().as_nanos() as f64 / 1000.0;
            blocks.push(us);
            worst = worst.max(us);
        }
    }
    blocks.sort_by(f64::total_cmp);
    let median = blocks[blocks.len() / 2];
    println!(
        "  worst 64-sample block at 48 kHz: {worst:.1} µs (median {median:.1} µs; budget {budget_us:.1} µs, margin {:.0}x)",
        budget_us / worst
    );
    assert!(
        worst < budget_us,
        "worst 64-sample block {worst:.1} µs exceeds {budget_us:.1} µs (008 SC-006)"
    );

    // Record plus play through the codec against the one-pass (spec 006 SC-006's rule).
    let x = stimulus::noise(9, 60 * 48_000);
    let settings = configs::memory_capacity(configs::settings("default_codec", 48_000), 61, 1);
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
        "  one-pass {one_pass:.1} ns/sample, record + play {split:.1} ns/sample, ratio {ratio:.3} (008 SC-006)"
    );
    assert!(
        ratio <= 1.25,
        "record + play through the codec costs {ratio:.2}x the one-pass (008 SC-006)"
    );
}

/// The frame-byte digest of the golden `voiced_m20` stimulus recorded at 48 kHz in
/// `default_codec`, pinned to the blessed `golden-codec-v1.json` entry (101 frames; T056); the release timing job runs this test, so a wrapped overflow that only a release
/// build would take cannot hide behind debug-only tests (008 research R-03; plan review P8).
/// Not profile-gated: it runs in whichever profile `--ignored` is given.
const VOICED_M20_48K_FRAMES_SHA256: Option<&str> =
    Some("840e8196b1fae38009c9caeab5c01b9c4847119b23afb19023a934f7244a87bc"); // golden-codec-v1.json, T056

#[test]
#[ignore = "release-profile digest check; run with --ignored"]
fn voiced_m20_frame_bytes_digest_in_this_profile() {
    let x = golden::stimulus("voiced_m20", 48_000);
    let mut p = Pipeline::new(configs::settings("default_codec", 48_000)).unwrap();
    p.start_recording().unwrap();
    for chunk in x.chunks(4096) {
        let _ = p.record(chunk, &mut []).unwrap();
    }
    let number = match p.stop_recording(&mut []).unwrap().result {
        rr_dr60::StopResult::Kept { number, .. } => number,
        other => panic!("{other:?}"),
    };
    let frames = rr_dr60::__test_hooks::stored_frames(&p, number);
    let digest = golden::sha256_hex(frames);
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    println!(
        "voiced_m20 at 48 kHz in default_codec ({profile} profile): {} frames, frame-byte SHA-256 {digest}",
        frames.len() / rr_dr60::speech_codec::FRAME_BYTES as usize
    );
    if let Some(want) = VOICED_M20_48K_FRAMES_SHA256 {
        assert_eq!(
            digest, want,
            "frame bytes differ in the {profile} profile (008 R-03)"
        );
    }
}
