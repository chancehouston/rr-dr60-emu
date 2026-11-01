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

/// The counters are global: the two tests of this file take turns (spec 016 T014).
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
    let _turn = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
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
    // Spec 014 FR-012, FR-013, SC-005: the control manifest reads.
    control_reads();
}

/// Spec 014 FR-012: reading every control entry, every option and every lookup allocates
/// nothing.
fn control_reads() {
    let before = counts();
    let mut touched = 0usize;
    for e in rr_dr60::controls() {
        touched += e.identifier.len() + e.name.len() + e.description.len() + e.unit.len();
        touched += e.sources.len() + e.assumptions.len();
        if let rr_dr60::ControlValue::Choice { options, .. } = e.value {
            touched += options.iter().map(|o| o.identifier.len()).sum::<usize>();
        }
        touched += usize::from(rr_dr60::control(e.identifier).is_some());
    }
    touched += usize::from(rr_dr60::control("battery").is_none());
    // Spec 014 FR-013: the five C functions over every control, option and ID.
    let mut c = rr_dr60_ffi::RrDr60Control::default();
    let mut o = rr_dr60_ffi::RrDr60ControlOption::default();
    let mut id: *const std::ffi::c_char = std::ptr::null();
    // SAFETY: valid out pointers.
    unsafe {
        for i in 0..=rr_dr60_ffi::rr_dr60_control_count() {
            if rr_dr60_ffi::rr_dr60_control(i, &mut c) != RrDr60Status::Ok {
                continue;
            }
            for k in 0..=c.option_count {
                let _ = rr_dr60_ffi::rr_dr60_control_option(i, k, &mut o);
            }
            for k in 0..=c.source_count {
                let _ = rr_dr60_ffi::rr_dr60_control_source(i, k, &mut id);
            }
            for k in 0..=c.assumption_count {
                let _ = rr_dr60_ffi::rr_dr60_control_assumption(i, k, &mut id);
            }
            touched += c.stage as usize;
        }
    }
    let after = counts();
    assert!(touched > 0);
    assert_eq!(
        after, before,
        "heap activity [allocs, deallocs, reallocs] while reading the control manifest"
    );
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

/// Spec 016 FR-010: reading the meters in Rust and in C allocates nothing: the counting window
/// holds only the processing calls and the reads (the counters are global and the other test
/// runs concurrently, so this thread counts only inside that window, which constructs nothing).
#[test]
fn meters_read_allocates_nothing() {
    use rr_dr60_ffi::{RrDr60Meters, rr_dr60_meters};
    let _turn = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let input = stimulus::noise(0x016, 1024);
    let mut output = vec![0.0f32; 1024];
    for &rate in configs::measurement_rates() {
        let s = rr_dr60::Settings::new(rate);
        let mut p = Pipeline::new(s).unwrap();
        let c = configs::c_settings(&s);
        let mut handle: *mut RrDr60Pipeline = core::ptr::null_mut();
        // SAFETY: a valid settings struct and an out-pointer.
        let st = unsafe { rr_dr60_create(&c, &mut handle) };
        assert_eq!(st, RrDr60Status::Ok);
        let mut cm = RrDr60Meters {
            struct_size: size_of::<RrDr60Meters>() as u32,
            ..Default::default()
        };
        let mut info = RrDr60BlockInfo::default();
        MEASURING.with(|m| m.set(true));
        let before = counts();
        let _ = p.process(&input, &mut output).unwrap();
        let m1 = p.meters();
        let m2 = p.rest_meters();
        // SAFETY: the handle is live; the buffers are 1024 samples; the struct carries its size.
        let st1 = unsafe {
            rr_dr60_process(handle, input.as_ptr(), output.as_mut_ptr(), 1024, &mut info)
        };
        // SAFETY: as above.
        let st2 = unsafe { rr_dr60_meters(handle, &mut cm) };
        let after = counts();
        MEASURING.with(|m| m.set(false));
        assert_eq!(st1, RrDr60Status::Ok);
        assert_eq!(st2, RrDr60Status::Ok);
        assert_eq!(after, before, "{rate} Hz: the reads allocated");
        assert_eq!(m1.input_peak, cm.input_peak);
        assert_ne!(m1, m2);
        // SAFETY: destroying the handle once.
        unsafe { rr_dr60_destroy(handle) };
    }
}

/// Spec 015 FR-009: the schedule call, a log-taking call with a full-length buffer, the clear and
/// the readings allocate nothing, in Rust and in C (the counting window holds only those calls;
/// serialized with the other counting tests).
#[test]
fn changes_allocate_nothing() {
    use rr_dr60::changes::MAX_CHANGES_PER_CALL;
    use rr_dr60::{ChangeEntry, Stream};
    use rr_dr60_ffi::{
        RrDr60ChangeEntry, rr_dr60_clear_schedule, rr_dr60_pending_changes,
        rr_dr60_process_with_events_and_changes, rr_dr60_schedule_change, rr_dr60_stream_position,
    };
    let _turn = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let input = stimulus::noise(0x015, 1024);
    let mut output = vec![0.0f32; 1024];
    let mut log = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
    let mut clog = vec![RrDr60ChangeEntry::default(); MAX_CHANGES_PER_CALL];
    for &rate in configs::measurement_rates() {
        let s = rr_dr60::Settings::new(rate);
        let mut p = Pipeline::new(s).unwrap();
        let c = configs::c_settings(&s);
        let mut handle: *mut RrDr60Pipeline = core::ptr::null_mut();
        // SAFETY: a valid settings struct and an out-pointer.
        let st = unsafe { rr_dr60_create(&c, &mut handle) };
        assert_eq!(st, RrDr60Status::Ok);
        let mut info = RrDr60BlockInfo::default();
        let mut count = 0usize;
        let mut pending = 0u32;
        let mut pos = 0u64;
        MEASURING.with(|m| m.set(true));
        let before = counts();
        for i in 0..64u64 {
            p.schedule_change(Stream::OnePass, 10 + i, 1, 0.5, i + 1)
                .unwrap();
        }
        let rinfo = p
            .process_with_events_and_changes(&input, &mut output, &mut [], &mut log)
            .unwrap();
        let rp = p.stream_position(Stream::OnePass).unwrap();
        p.schedule_change(Stream::OnePass, 1 << 40, 1, 0.5, 99)
            .unwrap();
        let rn = p.pending_changes(Stream::OnePass).unwrap();
        p.clear_schedule(Stream::OnePass).unwrap();
        // SAFETY: the handle is live; the buffers are 1024 samples; the log holds 67 entries.
        let st1 = unsafe {
            for i in 0..64u64 {
                assert_eq!(
                    rr_dr60_schedule_change(handle, 1, 10 + i, 1, 0.5, i + 1),
                    RrDr60Status::Ok
                );
            }
            rr_dr60_process_with_events_and_changes(
                handle,
                input.as_ptr(),
                output.as_mut_ptr(),
                1024,
                &mut info,
                core::ptr::null_mut(),
                0,
                clog.as_mut_ptr(),
                MAX_CHANGES_PER_CALL,
                &mut count,
            )
        };
        // SAFETY: as above.
        let st2 = unsafe { rr_dr60_stream_position(handle, 1, &mut pos) };
        // SAFETY: as above.
        let st3 = unsafe { rr_dr60_pending_changes(handle, 1, &mut pending) };
        // SAFETY: as above.
        let st4 = unsafe { rr_dr60_clear_schedule(handle, 1) };
        let after = counts();
        MEASURING.with(|m| m.set(false));
        assert_eq!(
            (st1, st2, st3, st4),
            (
                RrDr60Status::Ok,
                RrDr60Status::Ok,
                RrDr60Status::Ok,
                RrDr60Status::Ok
            )
        );
        assert_eq!(
            after, before,
            "{rate} Hz: the schedule and the log allocated"
        );
        assert_eq!(rinfo.changes, 64);
        assert_eq!(count, 64);
        assert_eq!(rp, pos);
        assert_eq!(rn, 1);
        assert_eq!(pending, 0);
        // SAFETY: destroying the handle once.
        unsafe { rr_dr60_destroy(handle) };
    }
}

/// Spec 017 FR-008: the record-side schedule call, `record_with_changes` with a full buffer,
/// `stop_recording_with_changes`, `input_position`, `clock_of` and `record_landing` allocate
/// nothing, in Rust and through the C API.
#[test]
fn record_changes_allocate_nothing() {
    use rr_dr60::changes::{MAX_CHANGES_PER_CALL, clock_of, record_landing};
    use rr_dr60::{ChangeEntry, Stream};
    use rr_dr60_ffi::{
        RrDr60ChangeEntry, RrDr60RecordInfo, RrDr60StopOutcome, rr_dr60_clock_of,
        rr_dr60_input_position, rr_dr60_record_landing, rr_dr60_record_with_changes,
        rr_dr60_schedule_change, rr_dr60_start_recording, rr_dr60_stop_recording_with_changes,
    };
    let _turn = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let input = stimulus::noise(0x017, 1024);
    let mut log = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
    let mut clog = vec![RrDr60ChangeEntry::default(); MAX_CHANGES_PER_CALL];
    let hang = rr_dr60::controls()
        .iter()
        .position(|e| e.identifier == "vas.hang_ms")
        .unwrap() as u32;
    for &rate in configs::measurement_rates() {
        let mut s = rr_dr60::Settings::new(rate);
        s.speech_codec_enabled = false;
        let mut p = Pipeline::new(s).unwrap();
        let c = configs::c_settings(&s);
        let mut handle: *mut RrDr60Pipeline = core::ptr::null_mut();
        // SAFETY: a valid settings struct and an out-pointer.
        let st = unsafe { rr_dr60_create(&c, &mut handle) };
        assert_eq!(st, RrDr60Status::Ok);
        let mut info = RrDr60RecordInfo::default();
        let mut outcome = RrDr60StopOutcome::default();
        let (mut count, mut total, mut pos, mut clock, mut k) = (0usize, 0u64, 0u64, 0u32, 0u64);
        MEASURING.with(|m| m.set(true));
        let before = counts();
        for i in 0..64u64 {
            p.schedule_change(Stream::Record, 10 + i, hang, 50.0 + i as f64, i + 1)
                .unwrap();
        }
        p.start_recording().unwrap();
        let rinfo = p.record_with_changes(&input, &mut [], &mut log).unwrap();
        let rp = p.input_position(Stream::Record).unwrap();
        let rstop = p.stop_recording_with_changes(&mut [], &mut log).unwrap();
        let rc = clock_of(hang).unwrap();
        let rk = record_landing(1000, rate).unwrap();
        // SAFETY: the handle is live; the buffers are 1024 samples; the log holds 139 entries.
        let st1 = unsafe {
            for i in 0..64u64 {
                assert_eq!(
                    rr_dr60_schedule_change(handle, 2, 10 + i, hang, 50.0 + i as f64, i + 1),
                    RrDr60Status::Ok
                );
            }
            assert_eq!(rr_dr60_start_recording(handle), RrDr60Status::Ok);
            rr_dr60_record_with_changes(
                handle,
                input.as_ptr(),
                1024,
                &mut info,
                core::ptr::null_mut(),
                0,
                clog.as_mut_ptr(),
                MAX_CHANGES_PER_CALL,
                &mut count,
                &mut total,
            )
        };
        // SAFETY: as above.
        let st2 = unsafe { rr_dr60_input_position(handle, 2, &mut pos) };
        // SAFETY: as above.
        let st3 = unsafe {
            rr_dr60_stop_recording_with_changes(
                handle,
                &mut outcome,
                core::ptr::null_mut(),
                0,
                clog.as_mut_ptr(),
                MAX_CHANGES_PER_CALL,
                &mut count,
                &mut total,
            )
        };
        // SAFETY: valid out-pointers.
        let st4 = unsafe { rr_dr60_clock_of(hang, &mut clock) };
        // SAFETY: valid out-pointers.
        let st5 = unsafe { rr_dr60_record_landing(1000, rate, &mut k) };
        let after = counts();
        MEASURING.with(|m| m.set(false));
        assert_eq!(
            (st1, st2, st3, st4, st5),
            (
                RrDr60Status::Ok,
                RrDr60Status::Ok,
                RrDr60Status::Ok,
                RrDr60Status::Ok,
                RrDr60Status::Ok
            )
        );
        assert_eq!(rinfo.changes, 64);
        assert_eq!(rp, 1024);
        assert_eq!(rstop.changes, 0);
        assert_eq!(rc, rr_dr60::Clock::Input);
        assert_eq!(rk, record_landing(1000, rate).unwrap());
        assert_eq!(pos, 1024);
        assert_eq!(clock, 0);
        assert_eq!(k, rk);
        assert_eq!(
            before, after,
            "{rate} Hz: allocations in the record-side calls"
        );
        // SAFETY: a live handle.
        unsafe { rr_dr60_destroy(handle) };
    }
}

/// Spec 018 FR-007: the battery setter and getter, a scheduled battery change, a producing
/// call, the meters read and the three battery functions allocate nothing, in Rust and through
/// the C API.
#[test]
fn battery_allocate_nothing() {
    use rr_dr60::changes::MAX_CHANGES_PER_CALL;
    use rr_dr60::{ChangeEntry, Stream};
    use rr_dr60_ffi::{
        RrDr60ChangeEntry, RrDr60Meters, rr_dr60_battery_ceiling_db,
        rr_dr60_battery_ceiling_factor, rr_dr60_battery_trajectory_row, rr_dr60_get_battery_volts,
        rr_dr60_meters, rr_dr60_process_with_events_and_changes, rr_dr60_schedule_change,
        rr_dr60_set_battery_volts,
    };
    let _turn = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let input = stimulus::noise(0x018, 1024);
    let mut output = vec![0.0f32; 1024];
    let mut log = vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL];
    let mut clog = vec![RrDr60ChangeEntry::default(); MAX_CHANGES_PER_CALL];
    let control = rr_dr60::controls()
        .iter()
        .position(|e| e.identifier == "battery_volts")
        .unwrap() as u32;
    for &rate in configs::measurement_rates() {
        let mut s = rr_dr60::Settings::new(rate);
        s.speech_codec_enabled = false;
        let mut p = Pipeline::new(s).unwrap();
        let c = configs::c_settings(&s);
        let mut handle: *mut RrDr60Pipeline = core::ptr::null_mut();
        // SAFETY: a valid settings struct and an out-pointer.
        let st = unsafe { rr_dr60_create(&c, &mut handle) };
        assert_eq!(st, RrDr60Status::Ok);
        let mut info = RrDr60BlockInfo::default();
        let mut count = 0usize;
        let mut m = RrDr60Meters {
            struct_size: size_of::<RrDr60Meters>() as u32,
            ..Default::default()
        };
        let (mut volts, mut factor, mut db, mut hours, mut row_volts) =
            (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
        MEASURING.with(|m| m.set(true));
        let before = counts();
        p.set_battery_volts(2.4).unwrap();
        let rv = p.battery_volts();
        p.schedule_change(Stream::OnePass, 500, control, 2.2, 1)
            .unwrap();
        let rinfo = p
            .process_with_events_and_changes(&input, &mut output, &mut [], &mut log)
            .unwrap();
        let rm = p.meters();
        let rf = rr_dr60::battery::ceiling_factor(2.4);
        let rd = rr_dr60::battery::ceiling_db(2.4);
        let rt = rr_dr60::battery::ALKALINE_TRAJECTORY[3];
        // SAFETY: the handle is live; the buffers are 1024 samples; the outs are writable.
        let statuses = unsafe {
            [
                rr_dr60_set_battery_volts(handle, 2.4),
                rr_dr60_get_battery_volts(handle, &mut volts),
                rr_dr60_schedule_change(handle, 1, 500, control, 2.2, 1),
                rr_dr60_process_with_events_and_changes(
                    handle,
                    input.as_ptr(),
                    output.as_mut_ptr(),
                    1024,
                    &mut info,
                    core::ptr::null_mut(),
                    0,
                    clog.as_mut_ptr(),
                    MAX_CHANGES_PER_CALL,
                    &mut count,
                ),
                rr_dr60_meters(handle, &mut m),
                rr_dr60_battery_ceiling_factor(2.4, &mut factor),
                rr_dr60_battery_ceiling_db(2.4, &mut db),
                rr_dr60_battery_trajectory_row(3, &mut hours, &mut row_volts),
            ]
        };
        let after = counts();
        MEASURING.with(|m| m.set(false));
        assert!(
            statuses.iter().all(|&s| s == RrDr60Status::Ok),
            "{statuses:?}"
        );
        assert_eq!(after, before, "{rate} Hz: the battery calls allocated");
        assert_eq!(rv, 2.4);
        assert_eq!(volts, 2.4);
        assert_eq!(rinfo.changes, count);
        assert_eq!(
            rinfo.changes, 2,
            "the plain setter's entry and the scheduled one"
        );
        assert_eq!(m.battery_ceiling.to_bits(), rm.battery_ceiling.to_bits());
        assert_eq!(m.amp_clip_count, rm.amp_clip_count);
        assert_eq!(factor.to_bits(), rf.to_bits());
        assert_eq!(db.to_bits(), rd.to_bits());
        assert_eq!((hours, row_volts), rt);
        // SAFETY: destroying the handle once.
        unsafe { rr_dr60_destroy(handle) };
    }
}

/// Spec 019 FR-009: a level change during a playback (a crossfade starting, a request waiting)
/// and the producing calls allocate nothing, in Rust and through the C API.
#[test]
fn crossfade_allocate_nothing() {
    use rr_dr60_ffi::{
        RrDr60PlayInfo, rr_dr60_play, rr_dr60_set_playback_speed, rr_dr60_start_playback,
    };
    let _turn = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let x = stimulus::voiced(6000.0, 0.5, -20.0, 0x019);
    let mut output = vec![0.0f32; 256];
    for &rate in configs::measurement_rates() {
        let mut s = rr_dr60::Settings::new(rate);
        s.speech_codec_enabled = false;
        let (mut p, _) = configs::stored_recording(&|s| Pipeline::new(s).unwrap(), s, &x);
        let c = configs::c_settings(&s);
        let (q, _) = configs::stored_recording(&|s| Pipeline::new(s).unwrap(), s, &x);
        let mut bytes = vec![0u8; q.export_len(1).unwrap()];
        let _ = q.export(1, &mut bytes).unwrap();
        let mut handle: *mut RrDr60Pipeline = core::ptr::null_mut();
        // SAFETY: a valid settings struct and an out-pointer.
        assert_eq!(unsafe { rr_dr60_create(&c, &mut handle) }, RrDr60Status::Ok);
        // SAFETY: a live handle and the exported bytes.
        assert_eq!(
            unsafe {
                rr_dr60_ffi::rr_dr60_import(
                    handle,
                    bytes.as_ptr(),
                    bytes.len(),
                    core::ptr::null_mut(),
                )
            },
            RrDr60Status::Ok
        );
        p.start_playback(1).unwrap();
        // SAFETY: a live handle.
        assert_eq!(
            unsafe { rr_dr60_start_playback(handle, 1) },
            RrDr60Status::Ok
        );
        let mut info = RrDr60PlayInfo::default();
        MEASURING.with(|m| m.set(true));
        let before = counts();
        let _ = p.play(&mut output).unwrap();
        p.set_playback_speed(5).unwrap(); // a fade starts at the next call
        let _ = p.play(&mut output).unwrap();
        p.set_playback_speed(1).unwrap(); // waits
        let _ = p.play(&mut output).unwrap();
        // SAFETY: the handle is live; the buffer is 256 samples.
        let st = unsafe {
            [
                rr_dr60_play(handle, output.as_mut_ptr(), 256, &mut info),
                rr_dr60_set_playback_speed(handle, 5),
                rr_dr60_play(handle, output.as_mut_ptr(), 256, &mut info),
                rr_dr60_set_playback_speed(handle, 1),
                rr_dr60_play(handle, output.as_mut_ptr(), 256, &mut info),
            ]
        };
        let after = counts();
        MEASURING.with(|m| m.set(false));
        assert!(st.iter().all(|&s| s == RrDr60Status::Ok), "{st:?}");
        assert_eq!(after, before, "{rate} Hz: the crossfade allocated");
        // SAFETY: destroying the handle once.
        unsafe { rr_dr60_destroy(handle) };
    }
}

/// Spec 020 FR-001 – FR-003, FR-006, FR-008 (tasks.md T023): the writer, the reader, the maps
/// and the record path with its block allocate nothing per call.
#[test]
fn record_contract_allocate_nothing() {
    use rr_dr60::record::{self, RecordReadout, RunData, StopFields};
    use rr_dr60::timeline;
    use rr_dr60::{ChangeEntry, Stream};
    use rr_dr60_harness::record_contract_checks::{self as rc, Cell};
    let _turn = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let snapshot = counts;
    let allocations_since = |before: [usize; 3]| {
        let after = counts();
        (after[0] - before[0]) + (after[2] - before[2])
    };
    MEASURING.with(|m| m.set(true));
    for &rate in configs::measurement_rates() {
        let raw = rc::raw_stimulus(rate);
        let x = rc::samples(&raw);
        let cell = Cell {
            codec: false,
            changes: true,
            mute: false,
            self_stop: false,
        };
        let mut p = Pipeline::new(rc::settings(cell, rate)).unwrap();
        for (pos, control, value, tag) in rc::cell_changes(rate) {
            p.schedule_change(Stream::Record, pos, control, value, tag)
                .unwrap();
        }
        let mut ev = vec![VasEvent::default(); p.max_record_events(1024)];
        let mut ch = vec![ChangeEntry::default(); rr_dr60::changes::MAX_CHANGES_PER_CALL];
        let mut events = Vec::with_capacity(64);
        let mut changes = Vec::with_capacity(8);
        p.start_recording().unwrap();
        // The record path with the block and the totals: nothing allocated per call.
        let mut inside = 0usize;
        for block in x.chunks(1024) {
            let before = snapshot();
            let info = p.record_with_changes(block, &mut ev, &mut ch).unwrap();
            inside += allocations_since(before);
            events.extend_from_slice(&ev[..info.events]);
            changes.extend_from_slice(&ch[..info.changes]);
        }
        assert_eq!(inside, 0, "{rate} Hz: the record path");
        let before = snapshot();
        let out = p.stop_recording_with_changes(&mut ev, &mut ch).unwrap();
        assert_eq!(allocations_since(before), 0, "{rate} Hz: the stop");
        events.extend_from_slice(&ev[..out.flush.events]);
        changes.extend_from_slice(&ch[..out.changes]);
        let run = RunData {
            events: &events,
            changes: &changes,
            stop: StopFields::from(&out),
        };
        // The writer: counting and writing into a caller buffer.
        let before = snapshot();
        let n = p.processing_record_len(1, &run).unwrap();
        assert_eq!(allocations_since(before), 0, "{rate} Hz: the length");
        let mut rec = vec![0u8; n];
        let before = snapshot();
        p.processing_record_write(1, &run, &mut rec).unwrap();
        let _ = p.recording_digest(1).unwrap();
        assert_eq!(
            allocations_since(before),
            0,
            "{rate} Hz: the writer and the digest"
        );
        // The reader, the cursors and the batch reads.
        let mut r = RecordReadout::default();
        let mut ev2 = vec![VasEvent::default(); 64];
        let before = snapshot();
        p.processing_record_read(&rec, &mut r).unwrap();
        let _ = record::record_event(&rec, 0);
        let _ = record::record_change(&rec, 0);
        let _ = record::record_events(&rec, 0, &mut ev2);
        let _ = record::record_unknown_setting(&rec, 0);
        let _ = p.recording_digest_matches(1, &mut r).unwrap();
        assert_eq!(allocations_since(before), 0, "{rate} Hz: the reader");
        // The maps (c) allocates its pending list once: measured apart, bounded by one).
        let before = snapshot();
        let _ = timeline::kept_to_input(100, &r.settings, &events, &run.stop).unwrap();
        let _ = timeline::input_to_kept(1000, &r.settings, &events).unwrap();
        let _ = timeline::onepass_output_to_input(1000, 483, &events, true);
        assert_eq!(allocations_since(before), 0, "{rate} Hz: maps (a) and (b)");
        let before = snapshot();
        let _ = timeline::place_at(50, rate, 3, 3000, &[]).unwrap();
        assert!(
            allocations_since(before) <= 1,
            "{rate} Hz: map (c) allocates its request list once"
        );
    }
    MEASURING.with(|m| m.set(false));
}

/// Spec 021 FR-010 (tasks.md T018): scheduling a stage switch, its landing, the blend, the
/// mirror and a refusal allocate nothing in a call, in Rust and through the C API.
#[test]
fn bypass_allocate_nothing() {
    use rr_dr60::Stream;
    use rr_dr60_ffi::{
        RrDr60PlayInfo, rr_dr60_play, rr_dr60_schedule_change, rr_dr60_start_playback,
    };
    use rr_dr60_harness::bypass_checks as bp;
    let _turn = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let x = bp::stored_tone(1000.0, 0.5);
    let mut output = vec![0.0f32; 256];
    for &rate in configs::measurement_rates() {
        let s = bp::settings(bp::DEFAULT_CHAIN, rate);
        let (mut p, _) = configs::stored_recording(&|s| Pipeline::new(s).unwrap(), s, &x);
        let c = configs::c_settings(&s);
        let (q, _) = configs::stored_recording(&|s| Pipeline::new(s).unwrap(), s, &x);
        let mut bytes = vec![0u8; q.export_len(1).unwrap()];
        let _ = q.export(1, &mut bytes).unwrap();
        let mut handle: *mut RrDr60Pipeline = core::ptr::null_mut();
        // SAFETY: a valid settings struct and an out-pointer.
        assert_eq!(unsafe { rr_dr60_create(&c, &mut handle) }, RrDr60Status::Ok);
        // SAFETY: a live handle and the exported bytes.
        assert_eq!(
            unsafe {
                rr_dr60_ffi::rr_dr60_import(
                    handle,
                    bytes.as_ptr(),
                    bytes.len(),
                    core::ptr::null_mut(),
                )
            },
            RrDr60Status::Ok
        );
        let host = bp::host_tone(1000.0, 0.02, rate);
        let mut y = vec![0.0f32; host.len()];
        p.start_playback(1).unwrap();
        // SAFETY: a live handle.
        assert_eq!(
            unsafe { rr_dr60_start_playback(handle, 1) },
            RrDr60Status::Ok
        );
        let mut info = RrDr60PlayInfo::default();
        MEASURING.with(|m| m.set(true));
        let before = counts();
        p.schedule_change(Stream::Playback, 10, bp::CONTROL_STAGE_12, 0.0, 1)
            .unwrap();
        p.schedule_change(Stream::Playback, 100, bp::CONTROL_STAGE_12, 1.0, 2)
            .unwrap(); // a reversal
        p.schedule_change(Stream::Playback, 300, bp::CONTROL_STAGE_10, 0.0, 3)
            .unwrap();
        let refused = p.schedule_change(Stream::OnePass, 0, bp::CONTROL_STAGE_10, 0.0, 4);
        let _ = p.play(&mut output).unwrap();
        let _ = p.play(&mut output).unwrap();
        // SAFETY: the handle is live; the buffer is 256 samples.
        let st = unsafe {
            [
                rr_dr60_schedule_change(handle, 0, 10, 23, 0.0, 1),
                rr_dr60_schedule_change(handle, 0, 100, 23, 1.0, 2),
                rr_dr60_play(handle, output.as_mut_ptr(), 256, &mut info),
                rr_dr60_play(handle, output.as_mut_ptr(), 256, &mut info),
            ]
        };
        let after = counts();
        MEASURING.with(|m| m.set(false));
        assert!(refused.is_err());
        assert!(st.iter().all(|&s| s == RrDr60Status::Ok), "{st:?}");
        assert_eq!(after, before, "{rate} Hz: the bypass allocated (playback)");
        p.stop_playback().unwrap();
        // The one-pass: the mirror landing at the call's first sample and a blend.
        MEASURING.with(|m| m.set(true));
        let before = counts();
        p.schedule_change(Stream::OnePass, 5, bp::CONTROL_STAGE_13, 0.0, 5)
            .unwrap();
        let _ = p.process(&host, &mut y).unwrap();
        let after = counts();
        MEASURING.with(|m| m.set(false));
        assert_eq!(after, before, "{rate} Hz: the bypass allocated (one-pass)");
        // SAFETY: destroying the handle once.
        unsafe { rr_dr60_destroy(handle) };
    }
}

/// Spec 023 FR-007 (tasks.md T014): setting and scheduling the mode, its landing, the blend, a
/// level change in pitch-kept mode (the search) and a refusal allocate nothing in a call, in
/// Rust and through the C API.
#[test]
fn pitch_allocate_nothing() {
    use rr_dr60::Stream;
    use rr_dr60::speed::SpeedMode;
    use rr_dr60_ffi::{
        RrDr60PlayInfo, rr_dr60_play, rr_dr60_schedule_change, rr_dr60_set_playback_speed_mode,
        rr_dr60_start_playback,
    };
    use rr_dr60_harness::pitch_checks as pc;
    let _turn = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let x = pc::vowel(0.5);
    let mut output = vec![0.0f32; 256];
    for &rate in configs::measurement_rates() {
        let s = configs::settings("speed_default_no_codec", rate);
        let (mut p, _) = configs::stored_recording(&|s| Pipeline::new(s).unwrap(), s, &x);
        let c = configs::c_settings(&s);
        let (q, _) = configs::stored_recording(&|s| Pipeline::new(s).unwrap(), s, &x);
        let mut bytes = vec![0u8; q.export_len(1).unwrap()];
        let _ = q.export(1, &mut bytes).unwrap();
        let mut handle: *mut RrDr60Pipeline = core::ptr::null_mut();
        // SAFETY: a valid settings struct and an out-pointer.
        assert_eq!(unsafe { rr_dr60_create(&c, &mut handle) }, RrDr60Status::Ok);
        // SAFETY: a live handle and the exported bytes.
        assert_eq!(
            unsafe {
                rr_dr60_ffi::rr_dr60_import(
                    handle,
                    bytes.as_ptr(),
                    bytes.len(),
                    core::ptr::null_mut(),
                )
            },
            RrDr60Status::Ok
        );
        p.set_playback_speed(1).unwrap();
        p.start_playback(1).unwrap();
        // SAFETY: a live handle.
        unsafe {
            assert_eq!(
                rr_dr60_ffi::rr_dr60_set_playback_speed(handle, 1),
                RrDr60Status::Ok
            );
            assert_eq!(rr_dr60_start_playback(handle, 1), RrDr60Status::Ok);
        }
        let mut info = RrDr60PlayInfo::default();
        MEASURING.with(|m| m.set(true));
        let before = counts();
        p.set_playback_speed_mode(SpeedMode::PitchKept);
        p.schedule_change(Stream::Playback, 300, 2, 5.0, 1).unwrap();
        p.schedule_change(Stream::Playback, 600, 30, 0.0, 2)
            .unwrap();
        p.schedule_change(Stream::Playback, 900, 30, 1.0, 3)
            .unwrap();
        let refused = p.schedule_change(Stream::OnePass, 0, 30, 1.0, 4);
        for _ in 0..6 {
            let _ = p.play(&mut output).unwrap();
        }
        // SAFETY: the handle is live; the buffer is 256 samples.
        let st = unsafe {
            [
                rr_dr60_set_playback_speed_mode(handle, 1),
                rr_dr60_schedule_change(handle, 0, 300, 2, 5.0, 1),
                rr_dr60_schedule_change(handle, 0, 600, 30, 0.0, 2),
                rr_dr60_schedule_change(handle, 0, 900, 30, 1.0, 3),
                rr_dr60_play(handle, output.as_mut_ptr(), 256, &mut info),
                rr_dr60_play(handle, output.as_mut_ptr(), 256, &mut info),
                rr_dr60_play(handle, output.as_mut_ptr(), 256, &mut info),
                rr_dr60_play(handle, output.as_mut_ptr(), 256, &mut info),
                rr_dr60_play(handle, output.as_mut_ptr(), 256, &mut info),
                rr_dr60_play(handle, output.as_mut_ptr(), 256, &mut info),
            ]
        };
        let after = counts();
        MEASURING.with(|m| m.set(false));
        assert!(refused.is_err());
        assert!(st.iter().all(|&s| s == RrDr60Status::Ok), "{st:?}");
        assert_eq!(after, before, "{rate} Hz: the pitch-kept mode allocated");
        p.stop_playback().unwrap();
        // SAFETY: destroying the handle once.
        unsafe { rr_dr60_destroy(handle) };
    }
}
