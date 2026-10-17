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

/// Spec 009 SC-007 (research R-09; T039): the output chain's cost at 48 kHz in 4096-sample
/// blocks, the 0.9 default with the codec bypassed, the chain on against `OutputSettings::BYPASSED`,
/// on both routes, as ns per host sample and as the percentage over the bypassed figure (the
/// budget 25 %); the reported latencies unchanged at the seven rates; the chain's 1 kHz group
/// delay per route from the output checks (≤ 0.5 ms). Quote the figures from a solo run (the
/// timing tests run concurrently under `cargo test`).
#[test]
#[ignore = "release-mode timing; run with --ignored"]
fn output_cost() {
    use rr_dr60::{OutputSettings, Route};
    if skip_in_debug() {
        return;
    }
    let x = stimulus::noise(9, 30 * 48_000);
    let mut on = configs::settings("default_output", 48_000);
    on.speech_codec_enabled = false; // the codec-bypassed one-pass is SC-007's denominator
    let mut off = on;
    off.output = OutputSettings::BYPASSED;
    off.tap = rr_dr60::Tap::AfterPlayback;
    let cost = |s: rr_dr60::Settings, route: Route| {
        let mut best = f64::MAX;
        for _ in 0..5 {
            let mut p = Pipeline::new(s).unwrap();
            p.set_route(route);
            p.set_volume(1.0).unwrap();
            let mut buf = x.clone();
            let t = Instant::now();
            for chunk in buf.chunks_mut(4096) {
                let _ = p.process_in_place(chunk);
            }
            best = best.min(t.elapsed().as_nanos() as f64 / x.len() as f64);
        }
        best
    };
    let bypassed = cost(off, Route::Speaker);
    for route in [Route::Speaker, Route::Earphone] {
        let with = cost(on, route);
        let over = 100.0 * (with / bypassed - 1.0);
        println!(
            "output chain, {route:?} route, 48 kHz, 4096 blocks: {with:.1} ns per host sample with the chain, {bypassed:.1} bypassed, {over:+.1} % (009 SC-007: budget 25 %)"
        );
        assert!(
            over <= 25.0,
            "{route:?}: the chain adds {over:.1} % (009 SC-007)"
        );
    }
    for rate in configs::all_rates() {
        let a = Pipeline::new(configs::settings("default_output", rate)).unwrap();
        let b = Pipeline::new(configs::settings_008(rate)).unwrap();
        assert_eq!(
            a.latency_samples(),
            b.latency_samples(),
            "{rate} Hz one-pass latency"
        );
        assert_eq!(
            a.record_latency_samples(),
            b.record_latency_samples(),
            "{rate} Hz"
        );
        assert_eq!(
            a.playback_latency_samples(),
            b.playback_latency_samples(),
            "{rate} Hz"
        );
    }
    for route in [Route::Speaker, Route::Earphone] {
        let rows = rr_dr60_harness::output_checks::group_delay_1k(
            48_000,
            route,
            &rr_dr60_harness::checks::standard,
        );
        for r in &rows {
            println!("{r}");
            assert!(r.passed, "{r}");
        }
    }
}

/// Spec 010 SC-005 (tasks.md T017; research R-08): the output clamp adds at most 2 ns per host
/// sample to the codec-bypassed speaker route's one-pass cost at 48 kHz on full-scale noise at
/// position 1 (a large share of samples replaced). The budget is SC-005's "about 3 %" of the
/// option-off cost, asserted as a ratio so it means the same on every machine (2 ns was 3 % of
/// the development machine's 68 ns; the x86-64 CI runner is about twice as slow, and its IEEE
/// max / min with NaN handling takes a few instructions where aarch64 takes one). Each side is
/// the fastest of nine alternating runs (the noise-robust estimator of 009's `output_cost`);
/// both routes, the nanoseconds and the bounded share printed; the latencies unchanged. Quote
/// the figures from a solo run.
#[test]
#[ignore = "release-mode timing; run with --ignored"]
fn output_clamp_cost() {
    use rr_dr60::Route;
    if skip_in_debug() {
        return;
    }
    let x: Vec<f32> = stimulus::noise(10, 30 * 48_000)
        .iter()
        .map(|v| (v * 4.0).clamp(-1.0, 1.0))
        .collect();
    let mut off = configs::settings("default_output", 48_000);
    off.speech_codec_enabled = false; // the codec-bypassed one-pass (009 SC-007's denominator)
    let mut on = off;
    on.output.clamp_enabled = true;
    let one = |s: rr_dr60::Settings, route: Route| -> f64 {
        let mut p = Pipeline::new(s).unwrap();
        p.set_route(route);
        p.set_volume(1.0).unwrap();
        let mut buf = x.clone();
        let t = Instant::now();
        for chunk in buf.chunks_mut(4096) {
            let _ = p.process_in_place(chunk);
        }
        t.elapsed().as_nanos() as f64 / x.len() as f64
    };
    for route in [Route::Speaker, Route::Earphone] {
        let (mut m_on, mut m_off) = (f64::MAX, f64::MAX);
        for _ in 0..9 {
            m_on = m_on.min(one(on, route));
            m_off = m_off.min(one(off, route));
        }
        let mut p = Pipeline::new(on).unwrap();
        p.set_route(route);
        p.set_volume(1.0).unwrap();
        let mut buf = x.clone();
        let _ = p.process_in_place(&mut buf);
        let share = 100.0 * buf.iter().filter(|v| v.abs() == 1.0).count() as f64 / buf.len() as f64;
        let over = 100.0 * (m_on / m_off - 1.0);
        println!(
            "output clamp, {route:?} route, 48 kHz, 4096 blocks, full-scale noise at position 1: best of nine {m_on:.1} ns per host sample on, {m_off:.1} off, {:+.2} ns ({over:+.2} %); {share:.1} % of samples at the bound (010 SC-005: budget about 3 % on the speaker route, 2 ns on the development machine)",
            m_on - m_off
        );
        if route == Route::Speaker {
            assert!(
                over <= 3.0,
                "the output clamp adds {over:.2} % ({:.2} ns) (010 SC-005)",
                m_on - m_off
            );
        }
    }
    for rate in configs::all_rates() {
        let a = Pipeline::new(configs::settings("default_output_clamp", rate)).unwrap();
        let b = Pipeline::new(configs::settings("default_output", rate)).unwrap();
        assert_eq!(a.latency_samples(), b.latency_samples(), "{rate} Hz");
        assert_eq!(
            a.record_latency_samples(),
            b.record_latency_samples(),
            "{rate} Hz"
        );
        assert_eq!(
            a.playback_latency_samples(),
            b.playback_latency_samples(),
            "{rate} Hz"
        );
        assert_eq!(a.drain_samples(), b.drain_samples(), "{rate} Hz");
    }
}

/// Spec 011 FR-009, US2 AS7, SC-006 (tasks.md T024; research R-08): the volume transition's
/// worst case, every sample inside a transition (480-sample blocks at 48 kHz with the position
/// alternated between 0.1 and 1 before every block; N = 960), against the `volume_step` hook
/// (0.10's step) at the same partition, codec bypassed, on noise at −20 dB, where the cost does
/// not depend on the position (the knee's upper branches cost about 5 ns at position 1 on
/// full-scale noise; under the alternation the knob cycles between 0.4 and 0.7 while the step
/// spends half its samples at 1). Each side the fastest of nine alternating runs; ≤ 15 % on the
/// speaker route; the constant-position spread (≤ 2 %), the full-scale figure and both routes
/// printed. The idle budget (≤ 3 %) is the build A/B of `scripts/ab-timing.sh`. Quote the
/// figures from a solo run.
#[test]
#[ignore = "release-mode timing; run with --ignored"]
fn volume_ramp_cost() {
    use rr_dr60::__test_hooks::OutputMutation;
    use rr_dr60::Route;
    if skip_in_debug() {
        return;
    }
    let full = stimulus::noise(9, 30 * 48_000);
    let quiet: Vec<f32> = full.iter().map(|v| v * 0.1).collect(); // −20 dB
    let mut s = configs::settings("default_output", 48_000);
    s.speech_codec_enabled = false; // the codec-bypassed one-pass (009 SC-007's denominator)
    let m = OutputMutation {
        volume_step: true,
        ..OutputMutation::default()
    };
    let step = move |s| Pipeline::__with_output_mutation(s, &m).unwrap();
    let standard = |s| Pipeline::new(s).unwrap();
    let alternating = |make: &dyn Fn(rr_dr60::Settings) -> Pipeline, x: &[f32], route: Route| {
        let mut p = make(s);
        p.set_route(route);
        p.set_volume(0.1).unwrap();
        let mut buf = x.to_vec();
        let t = Instant::now();
        for (i, chunk) in buf.chunks_mut(480).enumerate() {
            p.set_volume(if i % 2 == 0 { 1.0 } else { 0.1 }).unwrap();
            let _ = p.process_in_place(chunk);
        }
        t.elapsed().as_nanos() as f64 / x.len() as f64
    };
    let constant = |position: f32, route: Route| {
        let mut best = f64::MAX;
        for _ in 0..1 {
            let mut p = Pipeline::new(s).unwrap();
            p.set_route(route);
            p.set_volume(position).unwrap();
            let mut buf = quiet.clone();
            let t = Instant::now();
            for chunk in buf.chunks_mut(4096) {
                let _ = p.process_in_place(chunk);
            }
            best = best.min(t.elapsed().as_nanos() as f64 / quiet.len() as f64);
        }
        best
    };
    for route in [Route::Speaker, Route::Earphone] {
        // Coach review R7: nine interleaved rounds (the three positions inside each round, the
        // fastest of each kept), so the spread check survives a noisy shared runner.
        let mut at = [f64::MAX; 3];
        for _ in 0..9 {
            for (k, &p) in [0.1f32, 0.55, 1.0].iter().enumerate() {
                at[k] = at[k].min(constant(p, route));
            }
        }
        let lo = at.iter().copied().fold(f64::MAX, f64::min);
        let hi = at.iter().copied().fold(0.0, f64::max);
        let spread = 100.0 * (hi / lo - 1.0);
        let (mut ramp, mut base) = (f64::MAX, f64::MAX);
        let (mut ramp_fs, mut base_fs) = (f64::MAX, f64::MAX);
        for _ in 0..9 {
            ramp = ramp.min(alternating(&standard, &quiet, route));
            base = base.min(alternating(&step, &quiet, route));
            ramp_fs = ramp_fs.min(alternating(&standard, &full, route));
            base_fs = base_fs.min(alternating(&step, &full, route));
        }
        let over = 100.0 * (ramp / base - 1.0);
        println!(
            "volume transition, {route:?} route, 48 kHz, 480 blocks alternating 0.1 / 1 (100 % of samples in a transition; the knob cycles 0.4 – 0.7), noise -20 dB, codec bypassed: best of nine {ramp:.1} ns per host sample, {base:.1} with the step hook, {:+.2} ns ({over:+.2} %); full-scale noise {ramp_fs:.1} vs {base_fs:.1} ns ({:+.2} %, reported); constant positions 0.1 / 0.55 / 1: {:.1} / {:.1} / {:.1} ns (spread {spread:.2} %) (011 FR-009: budget 15 % on the speaker route)",
            ramp - base,
            100.0 * (ramp_fs / base_fs - 1.0),
            at[0],
            at[1],
            at[2],
        );
        assert!(
            spread <= 2.0,
            "{route:?}: the cost depends on the position ({spread:.2} %): the comparison is confounded"
        );
        if route == Route::Speaker {
            assert!(
                over <= 15.0,
                "the volume transition adds {over:.2} % ({:.2} ns) (011 FR-009)",
                ramp - base
            );
        }
    }
}

/// The `mic_cost` ceiling in percent: 10 % (spec 012 FR-011, SC-006) on the development
/// machine; `RR_DR60_MIC_COST_CEILING` overrides it (ci.yml sets 25 on the shared runner, whose
/// x86-64 select sequences cost several times more, spec 010).
fn mic_cost_ceiling() -> f64 {
    std::env::var("RR_DR60_MIC_COST_CEILING")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10.0) // 012 FR-011
}

/// Spec 012 FR-011, SC-006 (tasks.md T040; research R-09): stages 1 – 2's cost in one build.
/// The speaker route at 48 kHz, codec bypassed, 4096-sample blocks, `default_output` (0.11's
/// default, stages 1 – 2 bypassed) against the same with `MicSettings::DEVICE`, on −20 dBFS
/// noise; each side the fastest of 45 strictly alternating runs. At most 10 % (the CI ceiling 25 %);
/// full-scale noise, the offsets −18 / +12 dB and the split between the two stages reported.
/// Quote the figures from a solo run.
#[test]
#[ignore = "release-mode timing; run with --ignored"]
fn mic_cost() {
    use rr_dr60::{MicSettings, Route};
    if skip_in_debug() {
        return;
    }
    let full = stimulus::noise(9, 30 * 48_000);
    let quiet: Vec<f32> = full.iter().map(|v| v * 0.1).collect(); // −20 dB
    let mut off = configs::settings("default_output", 48_000);
    off.speech_codec_enabled = false; // 009 SC-007's denominator: the codec-bypassed one-pass
    let mut on = off;
    on.mic = MicSettings::DEVICE;
    let once = |s: rr_dr60::Settings, x: &[f32]| {
        let mut p = Pipeline::new(s).unwrap();
        p.set_route(Route::Speaker);
        let mut buf = x.to_vec();
        let t = Instant::now();
        for chunk in buf.chunks_mut(4096) {
            let _ = p.process_in_place(chunk);
        }
        t.elapsed().as_nanos() as f64 / x.len() as f64
    };
    // 45 strictly alternating runs per side, the fastest kept (the user's decision of
    // 2026-10-16; research R-09 › As measured): the fastest of nine read +10 – 12 ns on the
    // development machine whatever the code (run-order sensitive), against +3.9 – 4.2 ns here and
    // +4.3 ns in the cross-build A/B.
    let pair = |a: rr_dr60::Settings, b: rr_dr60::Settings, x: &[f32]| {
        let (mut ta, mut tb) = (f64::MAX, f64::MAX);
        for _ in 0..45 {
            ta = ta.min(once(a, x));
            tb = tb.min(once(b, x));
        }
        (ta, tb)
    };
    let (with, without) = pair(on, off, &quiet);
    let over = 100.0 * (with / without - 1.0);
    let (with_fs, without_fs) = pair(on, off, &full);
    let mut low = on;
    low.mic.hiss_offset_db = -18.0;
    let mut high = on;
    high.mic.hiss_offset_db = 12.0;
    let (t_low, t_high) = pair(low, high, &quiet);
    let mut capsule = off;
    capsule.mic.capsule_enabled = true;
    let mut amp = off;
    amp.mic.amp_enabled = true;
    let (t_capsule, t_amp) = pair(capsule, amp, &quiet);
    println!(
        "  split (reported): stage 1 alone {t_capsule:.1} ns ({:+.2}), stage 2 alone {t_amp:.1} ns ({:+.2})",
        t_capsule - without,
        t_amp - without
    );
    let ceiling = mic_cost_ceiling();
    println!(
        "microphone path, speaker route, 48 kHz, 4096 blocks, codec bypassed, noise -20 dB: {with:.1} ns per host sample with stages 1-2, {without:.1} bypassed, {:+.2} ns ({over:+.2} %); full-scale noise {with_fs:.1} vs {without_fs:.1} ns ({:+.2} %, reported); offsets -18 / +12 dB: {t_low:.1} / {t_high:.1} ns (reported) (012 FR-011: budget 10 %, ceiling here {ceiling} %)",
        with - without,
        100.0 * (with_fs / without_fs - 1.0),
    );
    assert!(
        over <= ceiling,
        "stages 1-2 add {over:.2} % ({:.2} ns) (012 FR-011)",
        with - without
    );
}

/// Spec 012 FR-011 (tasks.md T040; research R-09): `output_cost`'s body with stages 1 – 2 on,
/// printing the same line, for `scripts/ab-timing.sh c9f9994 output_cost_mic` (the on-against-
/// 0.11 A/B, reported). Stage 2 lifts the stimulus toward the limiter's knee (RMS 0.240, peak
/// 0.784), so the limiter's state is asserted to be 0.0 at the end of every run: a charged
/// limiter must never pass as microphone cost.
#[test]
#[ignore = "release-mode timing; run with --ignored"]
fn output_cost_mic() {
    use rr_dr60::{MicSettings, OutputSettings, Route};
    if skip_in_debug() {
        return;
    }
    let x = stimulus::noise(9, 30 * 48_000);
    let mut on = configs::settings("default_output", 48_000);
    on.speech_codec_enabled = false; // the codec-bypassed one-pass is SC-007's denominator
    on.mic = MicSettings::DEVICE; // 012 FR-011: stages 1 – 2 on
    let mut off = on;
    off.output = OutputSettings::BYPASSED;
    off.tap = rr_dr60::Tap::AfterPlayback;
    let cost = |s: rr_dr60::Settings, route: Route| {
        let mut best = f64::MAX;
        for _ in 0..5 {
            let mut p = Pipeline::new(s).unwrap();
            p.set_route(route);
            p.set_volume(1.0).unwrap();
            let mut buf = x.clone();
            let t = Instant::now();
            for chunk in buf.chunks_mut(4096) {
                let _ = p.process_in_place(chunk);
            }
            best = best.min(t.elapsed().as_nanos() as f64 / x.len() as f64);
            assert_eq!(
                rr_dr60::__test_hooks::limiter_state(&p),
                0.0,
                "the limiter charged: the figure would not be the microphone path's (012 R-09)"
            );
        }
        best
    };
    let bypassed = cost(off, Route::Speaker);
    for route in [Route::Speaker, Route::Earphone] {
        let with = cost(on, route);
        let over = 100.0 * (with / bypassed - 1.0);
        println!(
            "output chain, {route:?} route, 48 kHz, 4096 blocks: {with:.1} ns per host sample with the chain, {bypassed:.1} bypassed, {over:+.1} % (009 SC-007: budget 25 %; stages 1-2 on, 012 R-09)"
        );
        assert!(
            over <= 25.0,
            "{route:?}: the chain adds {over:.1} % (009 SC-007)"
        );
    }
}
