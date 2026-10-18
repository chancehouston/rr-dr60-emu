//! No heap activity while processing (001 FR-015, FR-022, SC-004; 001 tasks.md T053), at all
//! seven host rates including the 6 kHz identity rate (spec 004 FR-016; 004 tasks.md T028).
//!
//! Under coverage instrumentation (`cfg(coverage)`) the audio-heavy loops here run at three
//! rates, not seven: see `configs::measurement_rates`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use rr_dr60::{Pipeline, Route, VasEvent};
use rr_dr60_ffi::{
    RrDr60BlockInfo, RrDr60ManifestHeader, RrDr60Pipeline, RrDr60StageEntry, RrDr60Status,
    RrDr60VasEvent, rr_dr60_create, rr_dr60_destroy, rr_dr60_manifest_header,
    rr_dr60_manifest_stage, rr_dr60_manifest_stage_assumption, rr_dr60_manifest_stage_count,
    rr_dr60_manifest_stage_source, rr_dr60_process, rr_dr60_process_with_events, rr_dr60_reset,
};
use rr_dr60_harness::configs;
use rr_dr60_harness::stimulus::{self, Pcg32};

struct Counting;

/// A labelled limiter setting at one end of its range (005 FR-010).
type Extreme = (&'static str, fn(&mut rr_dr60::AgcSettings));

thread_local! {
    /// Count only allocations made on the measuring thread. Other threads, such as libtest's
    /// main thread printing "has been running for over 60 seconds" in slow coverage builds,
    /// must not count. A `const` thread-local with no destructor never allocates on access.
    static MEASURING: Cell<bool> = const { Cell::new(false) };
}

fn on_measuring_thread() -> bool {
    MEASURING.with(Cell::get)
}

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static DEALLOCS: AtomicUsize = AtomicUsize::new(0);
static REALLOCS: AtomicUsize = AtomicUsize::new(0);

// SAFETY: forwards every call to the system allocator unchanged; only counts them.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if on_measuring_thread() {
            ALLOCS.fetch_add(1, Relaxed);
        }
        // SAFETY: same contract as the caller's.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if on_measuring_thread() {
            DEALLOCS.fetch_add(1, Relaxed);
        }
        // SAFETY: same contract as the caller's.
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if on_measuring_thread() {
            REALLOCS.fetch_add(1, Relaxed);
        }
        // SAFETY: same contract as the caller's.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn counts() -> [usize; 3] {
    [
        ALLOCS.load(Relaxed),
        DEALLOCS.load(Relaxed),
        REALLOCS.load(Relaxed),
    ]
}

/// Everything happens in one test, so no other test thread allocates concurrently.
#[test]
fn processing_never_touches_the_heap() {
    MEASURING.with(|m| m.set(true));
    let input = stimulus::noise(0x0D60, 1024);
    let mut output = vec![0.0f32; 1024];
    let mut in_place = vec![0.0f32; 1024];
    let mut rng = Pcg32::new(0x0D60, 3);
    let sizes: Vec<usize> = (0..1000).map(|_| rng.below(1025) as usize).collect();

    for &rate in configs::measurement_rates() {
        // Spec 001 configurations (limiter bypassed), the spec 005 AGC configurations, the
        // limiter at the extremes of every setting (knee −40 / +3 dBFS, attack 1 / 50 ms,
        // release 200 / 10 000 ms; 005 FR-010) and the clamp off (005 SC-005).
        let mut cases: Vec<(String, rr_dr60::Settings)> = configs::CONFIGS
            .iter()
            .chain(&configs::AGC_CONFIGS)
            .map(|c| (c.to_string(), configs::settings(c, rate)))
            .collect();
        let extremes: [Extreme; 6] = [
            ("knee min", |a| a.knee_dbfs = -40.0),
            ("knee max", |a| a.knee_dbfs = 3.0),
            ("attack min", |a| a.attack_ms = 1.0),
            ("attack max", |a| a.attack_ms = 50.0),
            ("release min", |a| a.release_ms = 200.0),
            ("release max", |a| a.release_ms = 10_000.0),
        ];
        for (label, set) in extremes {
            let mut s = configs::settings("default_agc", rate);
            set(&mut s.agc);
            cases.push((format!("agc {label}"), s));
        }
        let mut clamp_off = configs::settings("default_agc", rate);
        clamp_off.adc_clamp_enabled = false;
        cases.push(("agc clamp off".into(), clamp_off));
        for (config, settings) in cases {
            let mut p = Pipeline::new(settings).unwrap();
            let c_settings = configs::c_settings(&settings);
            let mut handle: *mut RrDr60Pipeline = std::ptr::null_mut();
            // SAFETY: valid pointers.
            assert_eq!(
                unsafe { rr_dr60_create(&c_settings, &mut handle) },
                RrDr60Status::Ok
            );

            let before = counts();
            for &n in &sizes {
                let _ = p.process(&input[..n], &mut output[..n]).unwrap();
                in_place[..n].copy_from_slice(&input[..n]);
                let _ = p.process_in_place(&mut in_place[..n]);
                // SAFETY: live handle; buffers valid for n floats and non-overlapping.
                let status = unsafe {
                    rr_dr60_process(
                        handle,
                        input.as_ptr(),
                        output.as_mut_ptr(),
                        n,
                        std::ptr::null_mut(),
                    )
                };
                assert!(status == RrDr60Status::Ok);
            }
            p.reset();
            // SAFETY: live handle.
            let status = unsafe { rr_dr60_reset(handle) };
            let after = counts();
            assert!(status == RrDr60Status::Ok);
            assert_eq!(
                after, before,
                "{rate} Hz {config}: heap activity [allocs, deallocs, reallocs] while processing"
            );
            // SAFETY: handle came from rr_dr60_create.
            unsafe { rr_dr60_destroy(handle) };
        }

        // Spec 003 SC-004, FR-014: the VAS configurations and a 50 ms hang time, with a
        // burst-gap input so blocks are dropped entirely and others hold several splices,
        // through `process_with_events` and `rr_dr60_process_with_events`. Under coverage
        // instrumentation, 200 blocks instead of 1000 (same paths; 003 T050).
        let vas_sizes = if cfg!(coverage) {
            &sizes[..200]
        } else {
            &sizes[..]
        };
        vas_cases(rate, vas_sizes);

        // Spec 006 FR-017: the record and playback paths' calls and the reads.
        transport_cases(rate, vas_sizes);

        // Spec 008 FR-016 (T022): the speech codec in the one-pass and in the paths.
        codec_cases(rate, vas_sizes);
        output_cases(rate, vas_sizes);
        output_clamp_cases(rate, vas_sizes);
        volume_ramp_cases(rate, vas_sizes);
        mic_cases(rate, vas_sizes);
        speed_cases(rate, vas_sizes);
    }

    // Spec 007 FR-016, FR-017, SC-004: the manifest reads, Rust and C.
    manifest_reads();
}

/// Spec 007 FR-016, FR-017: reading the whole manifest, every lookup and the five C functions
/// over every stage and ID allocates nothing.
fn manifest_reads() {
    let before = counts();
    let m = rr_dr60::manifest();
    let mut touched = 0usize;
    for n in 0..=16u32 {
        if let Some(e) = m.stage(n) {
            touched += e.sources.len() + e.assumptions.len() + e.name.len();
        }
    }
    for e in m.stages() {
        touched += e.description.len() + e.controls.len() + e.coverage.len();
    }
    let mut header = RrDr60ManifestHeader::default();
    let mut entry = RrDr60StageEntry::default();
    let mut id: *const std::ffi::c_char = std::ptr::null();
    // SAFETY: valid out pointers.
    unsafe {
        assert!(rr_dr60_manifest_header(&mut header) == RrDr60Status::Ok);
        for n in 0..=rr_dr60_manifest_stage_count() + 1 {
            let status = rr_dr60_manifest_stage(n, &mut entry);
            if status != RrDr60Status::Ok {
                continue;
            }
            for i in 0..entry.source_count {
                assert!(rr_dr60_manifest_stage_source(n, i, &mut id) == RrDr60Status::Ok);
            }
            for i in 0..entry.assumption_count {
                assert!(rr_dr60_manifest_stage_assumption(n, i, &mut id) == RrDr60Status::Ok);
            }
            assert!(
                rr_dr60_manifest_stage_source(n, entry.source_count, &mut id)
                    == RrDr60Status::InvalidArgument
            );
        }
    }
    let after = counts();
    assert!(touched > 0);
    assert_eq!(
        after, before,
        "heap activity [allocs, deallocs, reallocs] while reading the manifest"
    );
}

/// Spec 006 FR-017: no heap activity in `start_recording`, `record`, `stop_recording`,
/// `start_playback`, `play`, `stop_playback`, `set_route`, the state and figure reads,
/// `export_len`, the latency and drain reads, `footprint_bytes`, `erase` and `erase_all`
/// (bounded memmove, no allocation), `export` into a caller buffer and `import` of a
/// caller buffer, at a 10 s capacity with the VAS configurations and the default with the
/// limiter.
fn transport_cases(rate: u32, sizes: &[usize]) {
    let input = vas_input(rate);
    let mut output = vec![0.0f32; 1024];
    let cases: Vec<(&str, rr_dr60::Settings)> = ["default_vas", "vas_mute", "default_agc"]
        .iter()
        .map(|c| {
            (
                *c,
                // 300 s: the 1000 blocks are up to 85 s of input at 6 kHz, recorded twice and imported once (mute mode stores it all).
                configs::memory_capacity(configs::settings(c, rate), 300, 3),
            )
        })
        .collect();
    for (config, settings) in cases {
        let mut p = Pipeline::new(settings).unwrap();
        let mut events = vec![VasEvent::default(); p.max_record_events(1024) + 8];
        let mut bytes = vec![0u8; 36 + 4 * 600_000];
        let before = counts();
        for round in 0..2 {
            p.start_recording().unwrap();
            for &n in sizes {
                let info = p.record(&input[..n], &mut events).unwrap();
                assert!(
                    info.stopped.is_none(),
                    "{rate} Hz {config}: filled a 300 s memory"
                );
            }
            let out = p.stop_recording(&mut events).unwrap();
            assert!(matches!(out.result, rr_dr60::StopResult::Kept { .. }));
            let number = round + 1;
            p.start_playback(number).unwrap();
            let _ = p.play(&mut output[..512]).unwrap();
            p.set_route(Route::Earphone);
            p.stop_playback().unwrap();
            p.start_playback(number).unwrap();
            loop {
                let n = sizes[(round as usize + 1) % sizes.len()].max(1);
                if p.play(&mut output[..n]).unwrap().finished {
                    break;
                }
            }
            let _ = (p.state(), p.memory_status(), p.route());
            let _ = (
                p.record_latency_samples(),
                p.playback_latency_samples(),
                p.drain_samples(),
            );
            let _ = (p.footprint_bytes(), p.recording_length(number).unwrap());
            let len = p.export_len(number).unwrap();
            assert!(
                len <= bytes.len(),
                "{rate} Hz {config}: the export buffer is too small"
            );
            assert_eq!(p.export(number, &mut bytes[..len]).unwrap(), len);
        }
        assert_eq!(p.import(&bytes[..p.export_len(2).unwrap()]), Ok(3));
        p.erase(1).unwrap();
        p.erase_all().unwrap();
        p.reset();
        let after = counts();
        assert_eq!(
            after, before,
            "{rate} Hz {config}: heap activity [allocs, deallocs, reallocs] in the transport"
        );
    }
}

/// Spec 008 FR-016, FR-017 (T022): no heap activity in the one-pass with the speech codec (the
/// three codec configurations, with events), nor in `start_recording`, `record`,
/// `stop_recording` (the frame-boundary padding included), `start_playback`, `play`,
/// `stop_playback` and the frame read-outs with the codec on; and `speech_codec::info()` and
/// `speech_codec_info()` allocate nothing. The byte form of a frames memory (export, import)
/// arrives with US2 and is covered there.
fn codec_cases(rate: u32, sizes: &[usize]) {
    let input = vas_input(rate);
    let mut output = vec![0.0f32; 1024];
    for config in configs::CODEC_CONFIGS {
        let settings = configs::memory_capacity(configs::settings(config, rate), 300, 3);
        let mut p = Pipeline::new(settings).unwrap();
        let mut events = vec![VasEvent::default(); p.max_events(1024) + 8];
        let before = counts();
        let mut frames = 0;
        for &n in sizes {
            let info = p
                .process_with_events(&input[..n], &mut output[..n], &mut events)
                .unwrap();
            frames += info.frames;
        }
        p.reset();
        for round in 0..2 {
            p.start_recording().unwrap();
            for &n in sizes {
                let info = p.record(&input[..n], &mut events).unwrap();
                frames += info.frames;
                assert!(
                    info.stopped.is_none(),
                    "{rate} Hz {config}: filled a 300 s memory"
                );
            }
            let out = p.stop_recording(&mut events).unwrap();
            assert!(matches!(out.result, rr_dr60::StopResult::Kept { .. }));
            frames += out.flush.frames;
            let number = round + 1;
            p.start_playback(number).unwrap();
            let _ = p.play(&mut output[..512]).unwrap();
            p.stop_playback().unwrap();
            p.start_playback(number).unwrap();
            loop {
                let n = sizes[(round as usize + 1) % sizes.len()].max(1);
                let info = p.play(&mut output[..n]).unwrap();
                frames += info.frames;
                if info.finished {
                    break;
                }
            }
            let _ = (p.recording_length(number).unwrap(), p.encoding());
            let _ = (p.speech_codec_info(), rr_dr60::speech_codec::info());
        }
        p.erase_all().unwrap();
        p.reset();
        let after = counts();
        assert_eq!(
            after, before,
            "{rate} Hz {config}: heap activity [allocs, deallocs, reallocs] with the codec"
        );
        assert!(frames > 0, "{rate} Hz {config}: no frames");
    }
}

/// A 1024-sample block: 900 samples of silence, then 124 samples of a 1 kHz tone at −30 dBFS
/// (the default threshold + 10 dB; −40 dBFS since spec 005, A-022 revised). Blocks replay this
/// buffer's prefix, so short blocks are pure silence that accumulates across blocks into
/// pauses, and full-length blocks end with sound that resumes recording (engineering target,
/// 003 R-15).
fn vas_input(rate: u32) -> Vec<f32> {
    let mut x = vec![0.0f32; 900];
    x.extend(stimulus::tone(
        1000.0,
        stimulus::amplitude(-30.0),
        f64::from(rate),
        124,
    ));
    x
}

fn vas_cases(rate: u32, sizes: &[usize]) {
    let input = vas_input(rate);
    let mut output = vec![0.0f32; 1024];
    let mut hang_50 = configs::settings("vas_only", rate);
    hang_50.vas.hang_ms = 50.0; // the densest splice train (003 R-07)
    hang_50.vas.onset_ms = 0.0; // any sound sample resumes
    let mut cases: Vec<(String, rr_dr60::Settings)> = configs::VAS_CONFIGS
        .iter()
        .map(|c| (c.to_string(), configs::settings(c, rate)))
        .collect();
    cases.push(("vas hang 50 ms".into(), hang_50));
    for (config, settings) in cases {
        let mut p = Pipeline::new(settings).unwrap();
        let mut events = vec![VasEvent::default(); p.max_events(1024)];
        let mut c_events = vec![RrDr60VasEvent::default(); events.len()];
        let c_settings = configs::c_settings(&settings);
        let mut handle: *mut RrDr60Pipeline = std::ptr::null_mut();
        // SAFETY: valid pointers.
        assert_eq!(
            unsafe { rr_dr60_create(&c_settings, &mut handle) },
            RrDr60Status::Ok
        );
        let (mut total_events, mut empty_blocks) = (0usize, 0usize);
        let before = counts();
        for &n in sizes {
            let info = p
                .process_with_events(&input[..n], &mut output[..n], &mut events)
                .unwrap();
            total_events += info.events;
            empty_blocks += usize::from(n > 0 && info.produced == 0);
            let mut c_info = RrDr60BlockInfo::default();
            // SAFETY: live handle; buffers valid for n floats; events valid for its length.
            let status = unsafe {
                rr_dr60_process_with_events(
                    handle,
                    input.as_ptr(),
                    output.as_mut_ptr(),
                    n,
                    &mut c_info,
                    c_events.as_mut_ptr(),
                    c_events.len(),
                )
            };
            assert!(status == RrDr60Status::Ok);
        }
        p.reset();
        let after = counts();
        assert_eq!(
            after, before,
            "{rate} Hz {config}: heap activity [allocs, deallocs, reallocs] while processing"
        );
        if config == "vas hang 50 ms" {
            assert!(total_events > 0, "{rate} Hz {config}: no splices");
            assert!(
                empty_blocks > 0,
                "{rate} Hz {config}: no block was dropped entirely"
            );
        }
        // SAFETY: handle came from rr_dr60_create.
        unsafe { rr_dr60_destroy(handle) };
    }
}

/// Spec 009 FR-012 (tasks.md T017): the output chain's controls and processing never touch the
/// heap: `set_volume`, `volume`, `set_route` and the one-pass on both routes at three positions
/// in the 0.9 default, the gain-law functions, and record / play through the chain with the
/// controls moved between play blocks.
fn output_cases(rate: u32, sizes: &[usize]) {
    use rr_dr60::output::{VOLUME_REFERENCE, earphone_gain_db, speaker_gain_db};
    let input = vas_input(rate);
    let mut output = vec![0.0f32; 1024];
    let settings = configs::memory_capacity(configs::settings("default_output", rate), 300, 3);
    let mut p = Pipeline::new(settings).unwrap();
    let mut events = vec![VasEvent::default(); p.max_record_events(1024) + 8];
    let before = counts();
    let mut db_sum = 0.0f32;
    for (i, &n) in sizes.iter().enumerate() {
        let route = if i % 2 == 0 {
            Route::Speaker
        } else {
            Route::Earphone
        };
        let volume = [VOLUME_REFERENCE, 1.0, 0.0][i % 3];
        p.set_route(route);
        p.set_volume(volume).unwrap();
        assert_eq!(p.volume(), volume);
        db_sum += speaker_gain_db(volume) + earphone_gain_db(volume);
        let _ = p.process(&input[..n], &mut output[..n]).unwrap();
    }
    std::hint::black_box(db_sum);
    p.start_recording().unwrap();
    for &n in &sizes[..200] {
        let info = p.record(&input[..n], &mut events).unwrap();
        assert!(info.stopped.is_none());
    }
    let out = p.stop_recording(&mut events).unwrap();
    assert!(matches!(out.result, rr_dr60::StopResult::Kept { .. }));
    p.start_playback(1).unwrap();
    let mut block = 0usize;
    loop {
        p.set_route(if block % 7 < 4 {
            Route::Speaker
        } else {
            Route::Earphone
        });
        p.set_volume([VOLUME_REFERENCE, 0.5, 1.0][block % 3])
            .unwrap();
        let info = p.play(&mut output[..512]).unwrap();
        block += 1;
        if info.finished {
            break;
        }
    }
    p.reset();
    let after = counts();
    assert_eq!(
        after, before,
        "{rate} Hz default_output: heap activity [allocs, deallocs, reallocs] with the output chain"
    );
}

/// Spec 010 FR-007 (tasks.md T015): the output clamp adds no heap activity: the one-pass in
/// the six clamp configurations at their route and position, and record / play through
/// `default_output_clamp` with the controls moved between play blocks.
fn output_clamp_cases(rate: u32, sizes: &[usize]) {
    use rr_dr60::output::VOLUME_REFERENCE;
    let input = vas_input(rate);
    let mut output = vec![0.0f32; 1024];
    // Construction allocates (the memory's store, spec 006); the count starts after it.
    let mut engines: Vec<(Pipeline, Route, f32)> = configs::OUTPUT_CLAMP_CONFIGS
        .iter()
        .map(|config| {
            let settings = configs::memory_capacity(configs::settings(config, rate), 300, 3);
            let (route, volume) = configs::output_clamp_route_volume(config);
            (Pipeline::new(settings).unwrap(), route, volume)
        })
        .collect();
    let settings =
        configs::memory_capacity(configs::settings("default_output_clamp", rate), 300, 3);
    let mut p = Pipeline::new(settings).unwrap();
    let mut events = vec![VasEvent::default(); p.max_record_events(1024) + 8];
    let before = counts();
    for (p, route, volume) in engines.iter_mut() {
        p.set_route(*route);
        p.set_volume(*volume).unwrap();
        for &n in &sizes[..64] {
            let _ = p.process(&input[..n], &mut output[..n]).unwrap();
        }
    }
    p.set_volume(1.0).unwrap();
    p.start_recording().unwrap();
    for &n in &sizes[..200] {
        let info = p.record(&input[..n], &mut events).unwrap();
        assert!(info.stopped.is_none());
    }
    let out = p.stop_recording(&mut events).unwrap();
    assert!(matches!(out.result, rr_dr60::StopResult::Kept { .. }));
    p.start_playback(1).unwrap();
    let mut block = 0usize;
    loop {
        p.set_route(if block % 7 < 4 {
            Route::Speaker
        } else {
            Route::Earphone
        });
        p.set_volume([VOLUME_REFERENCE, 0.5, 1.0][block % 3])
            .unwrap();
        let info = p.play(&mut output[..512]).unwrap();
        block += 1;
        if info.finished {
            break;
        }
    }
    p.reset();
    let after = counts();
    assert_eq!(
        after, before,
        "{rate} Hz output clamp: heap activity [allocs, deallocs, reallocs]"
    );
}

/// Spec 011 FR-009 (tasks.md T022): volume transitions in every block allocate nothing, in the
/// one-pass (the stage-11-only configurations and the default engine) and in the player, on
/// both routes, with retargets mid-transition.
fn volume_ramp_cases(rate: u32, sizes: &[usize]) {
    let input = vas_input(rate);
    let mut output = vec![0.0f32; 1024];
    let mut engines: Vec<(Pipeline, Route)> = [
        ("stage11_only_speaker", Route::Speaker),
        ("stage11_only_earphone", Route::Earphone),
        ("default_output", Route::Speaker),
        ("default_earphone", Route::Earphone),
    ]
    .iter()
    .map(|&(config, route)| {
        let settings = configs::memory_capacity(configs::settings(config, rate), 300, 3);
        (Pipeline::new(settings).unwrap(), route)
    })
    .collect();
    let settings = configs::memory_capacity(configs::settings("default_output", rate), 300, 3);
    let mut p = Pipeline::new(settings).unwrap();
    let mut events = vec![VasEvent::default(); p.max_record_events(1024) + 8];
    let before = counts();
    for (e, route) in engines.iter_mut() {
        e.set_route(*route);
        for (i, &n) in sizes[..64].iter().enumerate() {
            e.set_volume([0.1, 1.0, 0.4, 0.0][i % 4]).unwrap();
            let _ = e.process(&input[..n], &mut output[..n]).unwrap();
        }
    }
    p.start_recording().unwrap();
    for &n in &sizes[..200] {
        let info = p.record(&input[..n], &mut events).unwrap();
        assert!(info.stopped.is_none());
    }
    let out = p.stop_recording(&mut events).unwrap();
    assert!(matches!(out.result, rr_dr60::StopResult::Kept { .. }));
    for route in [Route::Speaker, Route::Earphone] {
        p.set_route(route);
        p.start_playback(1).unwrap();
        let mut block = 0usize;
        loop {
            p.set_volume([0.1, 1.0, 0.4][block % 3]).unwrap();
            let info = p.play(&mut output[..64]).unwrap();
            block += 1;
            if info.finished {
                break;
            }
        }
    }
    p.reset();
    let after = counts();
    assert_eq!(
        after, before,
        "{rate} Hz volume transitions: heap activity [allocs, deallocs, reallocs]"
    );
}

/// Spec 012 FR-011 (tasks.md T031): stages 1 – 2 allocate nothing per block, in the one-pass
/// (every microphone configuration, Rust and C), in the recorder (a start rekeys the hiss and
/// advances the counter), in the stop's flush, and in `reset` (which rekeys the one-pass).
fn mic_cases(rate: u32, sizes: &[usize]) {
    let input = vas_input(rate);
    let mut output = vec![0.0f32; 1024];
    let mut engines: Vec<(Pipeline, *mut RrDr60Pipeline)> = configs::MIC_CONFIGS
        .iter()
        .map(|&config| {
            let settings = configs::memory_capacity(configs::settings(config, rate), 300, 3);
            let c_settings = configs::c_settings(&settings);
            let mut handle: *mut RrDr60Pipeline = std::ptr::null_mut();
            // SAFETY: valid pointers.
            assert_eq!(
                unsafe { rr_dr60_create(&c_settings, &mut handle) },
                RrDr60Status::Ok
            );
            (Pipeline::new(settings).unwrap(), handle)
        })
        .collect();
    let settings = configs::memory_capacity(configs::settings("default_mic", rate), 300, 3);
    let mut p = Pipeline::new(settings).unwrap();
    let mut events = vec![VasEvent::default(); p.max_record_events(1024) + 8];
    let before = counts();
    for (e, handle) in engines.iter_mut() {
        for &n in &sizes[..64] {
            let _ = e.process(&input[..n], &mut output[..n]).unwrap();
            // SAFETY: live handle; buffers valid for n floats and non-overlapping.
            let status = unsafe {
                rr_dr60_process(
                    *handle,
                    input.as_ptr(),
                    output.as_mut_ptr(),
                    n,
                    std::ptr::null_mut(),
                )
            };
            assert!(status == RrDr60Status::Ok);
        }
        e.reset();
    }
    for _ in 0..2 {
        p.start_recording().unwrap();
        for &n in &sizes[..100] {
            let info = p.record(&input[..n], &mut events).unwrap();
            assert!(info.stopped.is_none());
        }
        let out = p.stop_recording(&mut events).unwrap();
        assert!(matches!(out.result, rr_dr60::StopResult::Kept { .. }));
    }
    p.reset();
    let after = counts();
    assert_eq!(
        after, before,
        "{rate} Hz microphone path: heap activity [allocs, deallocs, reallocs]"
    );
    for (_, handle) in engines {
        // SAFETY: handle came from rr_dr60_create.
        unsafe { rr_dr60_destroy(handle) };
    }
}

/// Spec 013 FR-010 (T020): playbacks at every level and with the level changed before every
/// block (through 3 → 1 → 5 → 2 → 4), the codec on and off, allocate nothing: the speed
/// control's banks and ring exist from construction; its look-ahead and the decoder's frames
/// are filled in place.
fn speed_cases(rate: u32, sizes: &[usize]) {
    let input = vas_input(rate);
    let mut output = vec![0.0f32; 1024];
    let cycle = [3u8, 1, 5, 2, 4];
    for config in ["speed_default", "speed_default_no_codec"] {
        let settings = configs::memory_capacity(configs::settings(config, rate), 300, 3);
        let mut p = Pipeline::new(settings).unwrap();
        p.start_recording().unwrap();
        let _ = p.record(&input, &mut []).unwrap();
        let _ = p.stop_recording(&mut []).unwrap();
        let before = counts();
        for level in 1..=rr_dr60::speed::LEVEL_COUNT {
            p.set_playback_speed(level).unwrap();
            p.start_playback(1).unwrap();
            for &n in &sizes[..32] {
                if p.play(&mut output[..n]).unwrap().finished {
                    break;
                }
            }
            if p.state().state == rr_dr60::State::Playing {
                p.stop_playback().unwrap();
            }
        }
        p.start_playback(1).unwrap();
        let _ = p.playback_length_samples(1, 5).unwrap();
        for (i, &n) in sizes.iter().enumerate() {
            p.set_playback_speed(cycle[i % cycle.len()]).unwrap();
            if p.play(&mut output[..n]).unwrap().finished {
                break;
            }
        }
        p.reset();
        let after = counts();
        assert_eq!(
            after, before,
            "{rate} Hz {config}: the playback speed control, heap activity [allocs, deallocs, reallocs]"
        );
    }
}
