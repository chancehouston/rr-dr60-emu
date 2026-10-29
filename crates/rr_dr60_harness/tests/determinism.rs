//! Determinism: block-partition invariance, in-place equivalence, seed independence
//! (001 FR-009, FR-014, SC-003; 001 tasks.md T052), at all seven host rates including the
//! 6 kHz identity rate (spec 004 FR-016; 004 tasks.md T028).
//!
//! Under coverage instrumentation (`cfg(coverage)`) the audio-heavy loops here run at three
//! rates, not seven: see `configs::measurement_rates`.

use rr_dr60::{Pipeline, StopResult, VasEvent};
use rr_dr60_harness::stimulus::Pcg32;
use rr_dr60_harness::{configs, golden};

fn one_block(s: rr_dr60::Settings, x: &[f32]) -> Vec<f32> {
    let mut p = Pipeline::new(s).unwrap();
    let mut y = vec![0.0; x.len()];
    let produced = p.process(x, &mut y).unwrap().produced;
    y.truncate(produced);
    y
}

/// Splits `x` into random blocks of 0..=8192 samples, always including blocks of size 0 and 1.
fn partitioned(s: rr_dr60::Settings, x: &[f32], rng: &mut Pcg32) -> Vec<f32> {
    let mut p = Pipeline::new(s).unwrap();
    let mut out = Vec::with_capacity(x.len());
    let mut pos = 0;
    let mut first = true;
    while pos < x.len() {
        let n = if first { 0 } else { rng.below(8193) as usize }.min(x.len() - pos);
        let n = if pos == 0 && !first { 1 } else { n };
        first = false;
        let mut y = vec![0.0; n];
        let produced = p.process(&x[pos..pos + n], &mut y).unwrap().produced;
        y.truncate(produced);
        out.extend(y);
        pos += n;
    }
    out
}

#[test]
fn hundred_partitions_of_each_golden_stimulus_at_48k() {
    let mut rng = Pcg32::new(0x0D60, 1);
    let s = configs::settings("default", 48_000);
    for stim in golden::STIMULI {
        let x = golden::stimulus(stim, 48_000);
        let want = one_block(s, &x);
        for i in 0..100 {
            assert_eq!(partitioned(s, &x, &mut rng), want, "{stim}, partition {i}");
        }
    }
}

#[test]
fn ten_partitions_for_every_other_config_and_rate() {
    let mut rng = Pcg32::new(0x0D60, 2);
    for &rate in configs::measurement_rates() {
        for config in configs::CONFIGS {
            if rate == 48_000 && config == "default" {
                continue;
            }
            let s = configs::settings(config, rate);
            for stim in golden::STIMULI {
                let x = golden::stimulus(stim, rate);
                let want = one_block(s, &x);
                for i in 0..10 {
                    assert_eq!(
                        partitioned(s, &x, &mut rng),
                        want,
                        "{rate} Hz {config} {stim}, partition {i}"
                    );
                }
            }
        }
    }
}

#[test]
fn in_place_equals_copy() {
    for &rate in configs::measurement_rates() {
        let x = golden::stimulus("sweep_log", rate);
        let mut p = Pipeline::new(configs::settings("default", rate)).unwrap();
        let mut buf = x.clone();
        let produced = p.process_in_place(&mut buf).produced;
        buf.truncate(produced);
        assert_eq!(
            buf,
            one_block(configs::settings("default", rate), &x),
            "{rate} Hz"
        );
    }
}

/// FR-009: with stage 1 off (the spec 001 configuration builds on the 0.11 default, spec 012
/// FR-010) no stage is stochastic, so the seed has no effect.
#[test]
fn seed_does_not_change_output() {
    let x = golden::stimulus("noise_seed_0d60", 44_100);
    let mut a = configs::settings("default", 44_100);
    let mut b = a;
    a.seed = 0;
    b.seed = u64::MAX;
    assert_eq!(one_block(a, &x), one_block(b, &x)); // FR-009
}

/// Spec 005 FR-014, SC-004: the limiter is block-partition invariant. 100 random partitions
/// (including sizes 0 and 1) of each AGC golden stimulus through `agc_only` (the host-rate tap,
/// where the limiter's own output and state are observed) at 48 kHz and at the 6 kHz identity
/// rate, and through `default_agc` at 6 kHz; 10 through `default_agc` at 48 kHz and for the
/// attack-minimum and release-maximum settings (the same code paths through the converters).
/// Under coverage instrumentation 10 and 2 (003 T050 pattern).
#[test]
fn agc_partitions() {
    let mut rng = Pcg32::new(0x0D60, 4);
    let (many, few) = if cfg!(coverage) { (10, 2) } else { (100, 10) };
    let cases = |rate: u32| {
        let mut attack_min = configs::settings("default_agc", rate);
        attack_min.agc.attack_ms = 1.0;
        let mut release_max = configs::settings("default_agc", rate);
        release_max.agc.release_ms = 10_000.0;
        let full = if rate == 6000 { many } else { few };
        [
            ("agc_only", configs::settings("agc_only", rate), many),
            ("default_agc", configs::settings("default_agc", rate), full),
            ("attack min", attack_min, few),
            ("release max", release_max, few),
        ]
    };
    for rate in [6000, 48_000] {
        for (name, s, partitions) in cases(rate) {
            for stim in golden::AGC_STIMULI {
                let x = golden::stimulus(stim, rate);
                let want = one_block(s, &x);
                for i in 0..partitions {
                    assert_eq!(
                        partitioned(s, &x, &mut rng),
                        want,
                        "{rate} Hz {name} {stim}, partition {i}"
                    );
                }
            }
        }
    }
}

/// One block with events: (output, events, paused at end).
fn one_block_events(s: rr_dr60::Settings, x: &[f32]) -> (Vec<f32>, Vec<VasEvent>, bool) {
    let mut p = Pipeline::new(s).unwrap();
    let mut y = vec![0.0; x.len()];
    let mut ev = vec![VasEvent::default(); p.max_events(x.len())];
    let info = p.process_with_events(x, &mut y, &mut ev).unwrap();
    y.truncate(info.produced);
    ev.truncate(info.events);
    (y, ev, info.paused)
}

/// Random blocks of 0..=8192 samples (always including sizes 0 and 1), concatenating the
/// produced samples and the events; the last block's `paused` is the stream's.
fn partitioned_events(
    s: rr_dr60::Settings,
    x: &[f32],
    rng: &mut Pcg32,
) -> (Vec<f32>, Vec<VasEvent>, bool) {
    let mut p = Pipeline::new(s).unwrap();
    let (mut out, mut events) = (Vec::with_capacity(x.len()), Vec::new());
    let mut paused = false;
    let mut pos = 0;
    let mut first = true;
    while pos < x.len() {
        let n = if first { 0 } else { rng.below(8193) as usize }.min(x.len() - pos);
        let n = if pos == 0 && !first { 1 } else { n };
        first = false;
        let mut y = vec![0.0; n];
        let mut ev = vec![VasEvent::default(); p.max_events(n)];
        let info = p
            .process_with_events(&x[pos..pos + n], &mut y, &mut ev)
            .unwrap();
        out.extend_from_slice(&y[..info.produced]);
        events.extend_from_slice(&ev[..info.events]);
        paused = info.paused;
        pos += n;
    }
    (out, events, paused)
}

/// Spec 003 FR-014, SC-003: for every block partition the concatenated output, the events and
/// the final `paused` are bit-identical. 100 seeded partitions of each VAS golden stimulus with
/// `vas_only` at 6 and 48 kHz (the identity rate, spec 004 FR-002, and the integer-ratio host
/// rate); 10 for `vas_mute`, `default_vas` and the other rates. Under
/// coverage instrumentation (unoptimized, `cfg(coverage)`) 10 and 2: the paths are the same and
/// the CI coverage job stays within its budget (003 T050).
#[test]
fn vas_partitions() {
    let mut rng = Pcg32::new(0x0D60, 5);
    let (many, few) = if cfg!(coverage) { (10, 2) } else { (100, 10) };
    for &rate in configs::measurement_rates() {
        for config in configs::VAS_CONFIGS {
            let partitions = if config == "vas_only" && (rate == 6000 || rate == 48_000) {
                many
            } else {
                few
            };
            let s = configs::settings(config, rate);
            for stim in golden::VAS_STIMULI {
                let x = golden::stimulus(stim, rate);
                let want = one_block_events(s, &x);
                assert!(!want.1.is_empty(), "{rate} Hz {config} {stim}: no events");
                for i in 0..partitions {
                    assert_eq!(
                        partitioned_events(s, &x, &mut rng),
                        want,
                        "{rate} Hz {config} {stim}, partition {i}"
                    );
                }
            }
        }
    }
}

/// Record-then-play in one block each: the playback (drain included), the recorder's events
/// (device positions) and the stored length.
fn record_play_one_block(s: rr_dr60::Settings, x: &[f32]) -> (Vec<f32>, Vec<VasEvent>, u64) {
    let mut p = Pipeline::new(configs::memory_capacity(s, 60, 3)).unwrap();
    let mut ev = vec![VasEvent::default(); p.max_record_events(x.len()) + 8];
    p.start_recording().unwrap();
    let info = p.record(x, &mut ev).unwrap();
    let mut events = ev[..info.events].to_vec();
    let out = p.stop_recording(&mut ev).unwrap();
    events.extend_from_slice(&ev[..out.flush.events]);
    let n = match out.result {
        StopResult::Kept { number, .. } => number,
        other => panic!("{other:?}"),
    };
    let length = p.recording_length(n).unwrap();
    let play = rr_dr60_harness::split_checks::play_to_end(&mut p, n, 1 << 20);
    (play, events, length)
}

/// Record-then-play in random blocks of 0..=8192 samples on both paths (always including 0
/// and 1), concatenating the playback and the events.
fn record_play_partitioned(
    s: rr_dr60::Settings,
    x: &[f32],
    rng: &mut Pcg32,
) -> (Vec<f32>, Vec<VasEvent>, u64) {
    let mut p = Pipeline::new(configs::memory_capacity(s, 60, 3)).unwrap();
    let mut ev = vec![VasEvent::default(); p.max_record_events(8192) + 8];
    let mut events = Vec::new();
    p.start_recording().unwrap();
    let mut pos = 0;
    let mut first = true;
    while pos < x.len() {
        let n = if first { 0 } else { rng.below(8193) as usize }.min(x.len() - pos);
        let n = if pos == 0 && !first { 1 } else { n };
        first = false;
        let info = p.record(&x[pos..pos + n], &mut ev).unwrap();
        events.extend_from_slice(&ev[..info.events]);
        pos += n;
    }
    let out = p.stop_recording(&mut ev).unwrap();
    events.extend_from_slice(&ev[..out.flush.events]);
    let n = match out.result {
        StopResult::Kept { number, .. } => number,
        other => panic!("{other:?}"),
    };
    let length = p.recording_length(n).unwrap();
    p.start_playback(n).unwrap();
    let mut play = Vec::new();
    let mut first = true;
    loop {
        let n = if first { 0 } else { rng.below(8193) as usize };
        first = false;
        let mut buf = vec![0.0f32; n];
        let info = p.play(&mut buf).unwrap();
        play.extend_from_slice(&buf[..info.produced]);
        if info.finished {
            break;
        }
    }
    (play, events, length)
}

/// Spec 006 FR-018: for every block partition on both paths the playback, the events and the
/// stored length are bit-identical. 100 seeded partitions of each VAS golden stimulus with
/// `default_vas` at 48 kHz; 10 at the other rates (coverage: 10 and 2; 003 T050).
#[test]
fn record_then_play_partitions() {
    let mut rng = Pcg32::new(0x0D60, 6);
    let (many, few) = if cfg!(coverage) { (10, 2) } else { (100, 10) };
    for &rate in configs::measurement_rates() {
        let partitions = if rate == 48_000 { many } else { few };
        let s = configs::settings("default_vas", rate);
        for stim in golden::VAS_STIMULI {
            let x = golden::stimulus(stim, rate);
            let want = record_play_one_block(s, &x);
            assert!(!want.1.is_empty(), "{rate} Hz {stim}: no events");
            for i in 0..partitions {
                assert_eq!(
                    record_play_partitioned(s, &x, &mut rng),
                    want,
                    "{rate} Hz {stim}, partition {i}"
                );
            }
        }
    }
}

/// Spec 008 FR-016 (T022): the speech codec is block-partition invariant. One-pass: 100 seeded
/// partitions (sizes 0 and 1 included) of the voiced stimulus through `codec_only` at 48 kHz
/// and at the 6 kHz identity rate; 10 at the other rates and for `default_codec`.
/// Record-then-play: the playback, the events, the stored length **and the stored frame bytes**
/// identical over 100 partitions at 48 kHz and 10 at the other rates (`default_codec` with the
/// VAS burst-gap stimulus). Under coverage instrumentation 10 and 2.
#[test]
fn codec_partitions() {
    let mut rng = Pcg32::new(0x0D60, 7);
    let (many, few) = if cfg!(coverage) { (10, 2) } else { (100, 10) };
    for &rate in configs::measurement_rates() {
        let x = rr_dr60_harness::stimulus::voiced(f64::from(rate), 3.0, -20.0, 0x0D60);
        for config in configs::CODEC_CONFIGS {
            let s = configs::settings(config, rate);
            let partitions = if config == "codec_only" && (rate == 6000 || rate == 48_000) {
                many
            } else {
                few
            };
            let want = one_block_events(s, &x);
            for i in 0..partitions {
                assert_eq!(
                    partitioned_events(s, &x, &mut rng),
                    want,
                    "{rate} Hz {config}, partition {i}"
                );
            }
        }
        // Record-then-play with the frames compared byte for byte.
        let s = configs::settings("default_codec", rate);
        let partitions = if rate == 48_000 { many } else { few };
        for stim in ["vas_burst_gap", "tone_1k_m20"] {
            let x = golden::stimulus(stim, rate);
            let want = record_play_one_block_frames(s, &x);
            assert!(want.3.len() >= 10, "{rate} Hz {stim}: no frames");
            assert_eq!(want.3.len() % 10, 0);
            for i in 0..partitions {
                assert_eq!(
                    record_play_partitioned_frames(s, &x, &mut rng),
                    want,
                    "{rate} Hz {stim}, partition {i}"
                );
            }
        }
    }
}

/// `record_play_one_block` plus the stored frame bytes (spec 008 SC-008).
fn record_play_one_block_frames(
    s: rr_dr60::Settings,
    x: &[f32],
) -> (Vec<f32>, Vec<VasEvent>, u64, Vec<u8>) {
    let mut p = Pipeline::new(configs::memory_capacity(s, 60, 3)).unwrap();
    let mut ev = vec![VasEvent::default(); p.max_record_events(x.len()) + 8];
    p.start_recording().unwrap();
    let info = p.record(x, &mut ev).unwrap();
    let mut events = ev[..info.events].to_vec();
    let out = p.stop_recording(&mut ev).unwrap();
    events.extend_from_slice(&ev[..out.flush.events]);
    let n = match out.result {
        StopResult::Kept { number, .. } => number,
        other => panic!("{other:?}"),
    };
    let length = p.recording_length(n).unwrap();
    let frames = rr_dr60::__test_hooks::stored_frames(&p, n).to_vec();
    let play = rr_dr60_harness::split_checks::play_to_end(&mut p, n, 1 << 20);
    (play, events, length, frames)
}

/// `record_play_partitioned` plus the stored frame bytes (spec 008 SC-008).
fn record_play_partitioned_frames(
    s: rr_dr60::Settings,
    x: &[f32],
    rng: &mut Pcg32,
) -> (Vec<f32>, Vec<VasEvent>, u64, Vec<u8>) {
    let mut p = Pipeline::new(configs::memory_capacity(s, 60, 3)).unwrap();
    let mut ev = vec![VasEvent::default(); p.max_record_events(8192) + 8];
    let mut events = Vec::new();
    p.start_recording().unwrap();
    let mut pos = 0;
    let mut first = true;
    while pos < x.len() {
        let n = if first { 0 } else { rng.below(8193) as usize }.min(x.len() - pos);
        let n = if pos == 0 && !first { 1 } else { n };
        first = false;
        let info = p.record(&x[pos..pos + n], &mut ev).unwrap();
        events.extend_from_slice(&ev[..info.events]);
        pos += n;
    }
    let out = p.stop_recording(&mut ev).unwrap();
    events.extend_from_slice(&ev[..out.flush.events]);
    let n = match out.result {
        StopResult::Kept { number, .. } => number,
        other => panic!("{other:?}"),
    };
    let length = p.recording_length(n).unwrap();
    let frames = rr_dr60::__test_hooks::stored_frames(&p, n).to_vec();
    p.start_playback(n).unwrap();
    let mut play = Vec::new();
    let mut first = true;
    loop {
        let n = if first { 0 } else { rng.below(8193) as usize };
        first = false;
        let mut buf = vec![0.0f32; n];
        let info = p.play(&mut buf).unwrap();
        play.extend_from_slice(&buf[..info.produced]);
        if info.finished {
            break;
        }
    }
    (play, events, length, frames)
}

#[test]
fn manifest_canonical_text_is_identical_on_every_target() {
    // Spec 007 SC-004 (research R-09): the manifest's canonical text equals the committed golden
    // text on every CI target (the golden-matrix and ios jobs run this file), and (US1
    // scenario 3) it is unchanged after audio has been processed.
    let path = rr_dr60_harness::manifest_checks::golden_text_path();
    let want = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(
        rr_dr60_harness::manifest_checks::render(rr_dr60::manifest()),
        want
    );
    let mut p = rr_dr60::Pipeline::new(rr_dr60_harness::configs::settings_007(48_000)).unwrap();
    let mut buf = vec![0.25f32; 4800];
    let _ = p.process_in_place(&mut buf);
    assert_eq!(
        rr_dr60_harness::manifest_checks::render(rr_dr60::manifest()),
        want
    );
}

#[test]
fn control_manifest_canonical_text_is_identical_on_every_target() {
    // Spec 014 FR-012, SC-004: the control manifest's canonical text equals the committed
    // golden on every CI target, also after audio has been processed.
    let path = rr_dr60_harness::controls_checks::golden_controls_path();
    let want = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let render = || rr_dr60_harness::controls_checks::render_controls(rr_dr60::controls());
    assert_eq!(render(), want);
    let mut p = rr_dr60::Pipeline::new(rr_dr60::Settings::new(48_000)).unwrap();
    let mut buf = vec![0.25f32; 4800];
    let _ = p.process_in_place(&mut buf);
    assert_eq!(render(), want);
}

/// Spec 009 FR-012 (SC-006; tasks.md T017): the 0.9 default on both routes at two volume
/// positions is partition-invariant (ten random partitions per case; a hundred for the
/// speaker route at the reference position at 48 kHz), and record-then-play with a route
/// change at a fixed play block is the same twice.
#[test]
fn output_chain_routes_and_volumes_are_partition_invariant() {
    use rr_dr60::Route;
    use rr_dr60::output::VOLUME_REFERENCE;
    let mut rng = Pcg32::new(0x0D60, 9);
    let run =
        |s: rr_dr60::Settings, route: Route, volume: f32, x: &[f32], rng: Option<&mut Pcg32>| {
            let mut p = Pipeline::new(s).unwrap();
            p.set_route(route);
            p.set_volume(volume).unwrap();
            let mut out = Vec::with_capacity(x.len());
            let mut pos = 0;
            let mut rng = rng;
            while pos < x.len() {
                let n = match rng.as_deref_mut() {
                    Some(r) => (r.below(8193) as usize).min(x.len() - pos),
                    None => x.len() - pos,
                };
                let mut y = vec![0.0; n];
                let produced = p.process(&x[pos..pos + n], &mut y).unwrap().produced;
                out.extend_from_slice(&y[..produced]);
                pos += n;
            }
            out
        };
    for &rate in configs::measurement_rates() {
        let s = configs::settings("default_output", rate);
        for (route, volume) in [
            (Route::Speaker, VOLUME_REFERENCE),
            (Route::Speaker, 1.0),
            (Route::Earphone, VOLUME_REFERENCE),
            (Route::Earphone, 1.0),
        ] {
            for stim in golden::STIMULI {
                let x = golden::stimulus(stim, rate);
                let want = run(s, route, volume, &x, None);
                let reps =
                    if rate == 48_000 && route == Route::Speaker && volume == VOLUME_REFERENCE {
                        100
                    } else {
                        10
                    };
                for i in 0..reps {
                    assert_eq!(
                        run(s, route, volume, &x, Some(&mut rng)),
                        want,
                        "{rate} Hz {route:?} {volume} {stim}, partition {i}"
                    );
                }
            }
        }
        // Record-then-play with a route change at play block 2 (480-sample blocks): the same
        // twice, and not the same as the run without the change.
        let x = golden::stimulus("tone_1k_m20", rate);
        let play = |switch: bool| {
            let mut p = Pipeline::new(s).unwrap();
            p.set_volume(1.0).unwrap();
            p.start_recording().unwrap();
            let _ = p.record(&x, &mut []).unwrap();
            let StopResult::Kept { .. } = p.stop_recording(&mut []).unwrap().result else {
                unreachable!()
            };
            p.set_route(Route::Speaker);
            p.start_playback(1).unwrap();
            let mut out = Vec::new();
            let mut buf = vec![0.0f32; 480];
            let mut block = 0;
            loop {
                if switch && block == 2 {
                    p.set_route(Route::Earphone);
                }
                let info = p.play(&mut buf).unwrap();
                out.extend_from_slice(&buf[..info.produced]);
                block += 1;
                if info.finished {
                    break;
                }
            }
            out
        };
        let (a, b, c) = (play(true), play(true), play(false));
        assert_eq!(a, b, "{rate} Hz: the route change is deterministic");
        assert_ne!(a, c, "{rate} Hz: the route change changes the sound");
    }
}

/// Spec 010 FR-007 (tasks.md T015; US2 AS4): the six output-clamp configurations are
/// partition-invariant on the full-scale square at their route and position, and
/// record-then-play of the three `default_*` names is the same twice.
#[test]
fn output_clamp_partitions() {
    use rr_dr60_harness::output_clamp_checks::stimulus_by_name;
    let mut rng = Pcg32::new(0x0D60, 10);
    let run = |s: rr_dr60::Settings,
               route: rr_dr60::Route,
               volume: f32,
               x: &[f32],
               rng: Option<&mut Pcg32>| {
        let mut p = Pipeline::new(s).unwrap();
        p.set_route(route);
        p.set_volume(volume).unwrap();
        let mut out = Vec::with_capacity(x.len());
        let mut pos = 0;
        let mut rng = rng;
        while pos < x.len() {
            let n = match rng.as_deref_mut() {
                Some(r) => (r.below(8193) as usize).min(x.len() - pos),
                None => x.len() - pos,
            };
            let mut y = vec![0.0; n];
            let produced = p.process(&x[pos..pos + n], &mut y).unwrap().produced;
            out.extend_from_slice(&y[..produced]);
            pos += n;
        }
        out
    };
    for &rate in configs::measurement_rates() {
        let x = stimulus_by_name("square_1k_fs", rate, 1.0);
        for config in configs::OUTPUT_CLAMP_CONFIGS {
            let s = configs::settings(config, rate);
            let (route, volume) = configs::output_clamp_route_volume(config);
            let want = run(s, route, volume, &x, None);
            for i in 0..10 {
                assert_eq!(
                    run(s, route, volume, &x, Some(&mut rng)),
                    want,
                    "{rate} Hz {config}, partition {i}"
                );
            }
            if config.starts_with("default_") {
                let play = || {
                    let mut p = Pipeline::new(s).unwrap();
                    p.set_route(route);
                    p.set_volume(volume).unwrap();
                    let run = rr_dr60_harness::split_checks::record_then_play(
                        &mut p,
                        &x,
                        rr_dr60_harness::split_checks::RECORD_BLOCK,
                        rr_dr60_harness::split_checks::PLAY_BLOCK,
                    );
                    run.pre_drain
                };
                assert_eq!(play(), play(), "{rate} Hz {config}: record-then-play twice");
            }
        }
    }
}

/// Spec 011 (tasks.md T022; FR-009): a volume change mid-stream gives the same output for ten
/// random partitions (each cut at the change) as for 4096-sample blocks, in the stage-11-only
/// configurations and the default engine on both routes.
#[test]
fn volume_ramp_partitions() {
    use rr_dr60::Route;
    use rr_dr60_harness::checks::standard;
    use rr_dr60_harness::volume_ramp_checks::{self as ramp, Blocks, run_with_changes, same};
    for &rate in configs::measurement_rates() {
        let c = ramp::change_at(rate);
        for (config, route) in [
            ("stage11_only_speaker", Route::Speaker),
            ("stage11_only_earphone", Route::Earphone),
            ("default_output", Route::Speaker),
            ("default_earphone", Route::Earphone),
        ] {
            let s = configs::settings(config, rate);
            let x = rr_dr60_harness::stimulus::voiced(f64::from(rate), 1.5, -20.0, 0x0D60);
            let changes = [(c, 1.0f32), (c + 1024, 0.4)];
            let want =
                run_with_changes(&standard, s, route, 0.1, &changes, &x, Blocks::Fixed(4096));
            for i in 0..10u64 {
                let got =
                    run_with_changes(&standard, s, route, 0.1, &changes, &x, Blocks::Irregular(i));
                assert!(same(&got, &want), "{rate} Hz {config}, partition {i}");
            }
        }
    }
}

/// Spec 012 SC-005 (US1 AS4; tasks.md T031): stages 1 – 2 with the hiss on are block-partition
/// invariant: random partitions (including sizes 0 and 1) of a voiced stimulus followed by
/// silence through every microphone configuration (the 0.12 default with the voice activation
/// bypassed among them) give the one-block output bit for bit, at the measurement rates. Ten
/// partitions each; two under coverage instrumentation.
#[test]
fn mic_partitions() {
    let mut rng = Pcg32::new(0x0D60, 12);
    let partitions = if cfg!(coverage) { 2 } else { 10 };
    for &rate in configs::measurement_rates() {
        let fs = f64::from(rate);
        let mut x = rr_dr60_harness::stimulus::voiced(fs, 0.3, -20.0, 7);
        x.extend(rr_dr60_harness::stimulus::silence(rate as usize / 5));
        for name in configs::MIC_CONFIGS {
            let s = configs::settings(name, rate);
            let want = one_block(s, &x);
            for i in 0..partitions {
                assert_eq!(
                    partitioned(s, &x, &mut rng),
                    want,
                    "{rate} Hz {name}, partition {i}"
                );
            }
        }
    }
}

/// Spec 012 FR-003, FR-009 (A-049): with stage 1 on the seed selects the hiss: the same seed
/// gives the same output, another seed another hiss.
#[test]
fn seed_selects_the_hiss() {
    let x = golden::stimulus("noise_seed_0d60", 44_100);
    let mut a = configs::settings("default_mic", 44_100);
    let mut b = a;
    a.seed = 0;
    b.seed = u64::MAX;
    assert_eq!(one_block(a, &x), one_block(a, &x));
    assert_ne!(one_block(a, &x), one_block(b, &x));
}

/// Spec 013 FR-010 (T020): at a constant level other than 3 a playback is bit-identical across
/// call partitions of 1, 64 and 4096 samples and an irregular one, at 6 and 48 kHz, the codec on
/// and off.
#[test]
fn speed_partitions() {
    for rate in [6000u32, 48_000] {
        for config in ["speed_default", "speed_default_no_codec"] {
            let x = rr_dr60_harness::stimulus::voiced(f64::from(rate), 0.6, -20.0, 13);
            for level in [1u8, 5] {
                let play = |sizes: &mut dyn FnMut() -> usize| {
                    let mut p = Pipeline::new(configs::settings(config, rate)).unwrap();
                    p.start_recording().unwrap();
                    let _ = p.record(&x, &mut []).unwrap();
                    let _ = p.stop_recording(&mut []).unwrap();
                    p.set_playback_speed(level).unwrap();
                    p.start_playback(1).unwrap();
                    let mut out = Vec::new();
                    loop {
                        let mut buf = vec![0.0f32; sizes()];
                        let info = p.play(&mut buf).unwrap();
                        out.extend_from_slice(&buf[..info.produced]);
                        if info.finished {
                            return out;
                        }
                    }
                };
                let reference = play(&mut || 4096);
                for fixed in [1usize, 64] {
                    let got = play(&mut || fixed);
                    assert!(
                        got == reference,
                        "{rate} {config} level {level}: blocks of {fixed}"
                    );
                }
                let mut rng = Pcg32::new(0x013, 1);
                let got = play(&mut || rng.below(3000) as usize);
                assert!(got == reference, "{rate} {config} level {level}: irregular");
            }
        }
    }
}
