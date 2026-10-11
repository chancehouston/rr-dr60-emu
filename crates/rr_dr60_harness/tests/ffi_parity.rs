//! C API parity and error contract (FR-023, FR-024; contracts/c-api.md; tasks.md T054).

use rr_dr60::Pipeline;
use rr_dr60_ffi::*;
use rr_dr60_harness::{configs, golden, stimulus};

fn create(s: &RrDr60Settings) -> *mut RrDr60Pipeline {
    let mut p = std::ptr::null_mut();
    // SAFETY: valid pointers.
    assert_eq!(unsafe { rr_dr60_create(s, &mut p) }, RrDr60Status::Ok);
    p
}

#[test]
fn c_api_matches_rust_api_everywhere() {
    for rate in configs::all_rates() {
        let x = golden::stimulus("sweep_log", rate);
        for config in configs::CONFIGS {
            let s = configs::settings(config, rate);
            let mut want = vec![0.0; x.len()];
            let mut rust = Pipeline::new(s).unwrap();
            let produced = rust.process(&x, &mut want).unwrap().produced;
            want.truncate(produced);
            let p = create(&configs::c_settings(&s));
            let mut got = vec![0.0f32; x.len()];
            // SAFETY: live handle; valid non-overlapping buffers.
            assert_eq!(
                unsafe {
                    rr_dr60_process(
                        p,
                        x.as_ptr(),
                        got.as_mut_ptr(),
                        x.len(),
                        std::ptr::null_mut(),
                    )
                },
                RrDr60Status::Ok
            );
            assert_eq!(got, want, "{rate} Hz {config}");
            let mut lat = 0;
            // SAFETY: live handle.
            assert_eq!(
                unsafe { rr_dr60_latency_samples(p, &mut lat) },
                RrDr60Status::Ok
            );
            assert_eq!(lat, rust.latency_samples());
            // SAFETY: from rr_dr60_create.
            unsafe { rr_dr60_destroy(p) };
        }
    }
}

/// Spec 002 FR-014, US2 AS6: C and Rust output are bit-identical with the AGC on, for the
/// AGC configurations and for the minimum and maximum of every AGC setting.
#[test]
fn c_api_matches_rust_api_with_agc() {
    let rate = 48_000;
    let x = stimulus::step(1000.0, &[-40.0, -10.0, -40.0], &[0.1, 0.1, 0.2], 48_000.0);
    let mut cases: Vec<rr_dr60::Settings> = configs::AGC_CONFIGS
        .iter()
        .map(|c| configs::settings(c, rate))
        .collect();
    let extremes: [fn(&mut rr_dr60::AgcSettings, bool); 5] = [
        |a, hi| a.target_dbfs = if hi { 0.0 } else { -30.0 },
        |a, hi| a.max_gain_db = if hi { 60.0 } else { 0.0 },
        |a, hi| a.max_attenuation_db = if hi { 40.0 } else { 0.0 },
        |a, hi| a.attack_ms = if hi { 100.0 } else { 1.0 },
        |a, hi| a.release_ms = if hi { 10_000.0 } else { 50.0 },
    ];
    for set in extremes {
        for hi in [false, true] {
            let mut s = configs::settings("default_agc", rate);
            set(&mut s.agc, hi);
            cases.push(s);
        }
    }
    for s in cases {
        let mut want = vec![0.0; x.len()];
        let produced = Pipeline::new(s)
            .unwrap()
            .process(&x, &mut want)
            .unwrap()
            .produced;
        want.truncate(produced);
        let p = create(&configs::c_settings(&s));
        let mut got = vec![0.0f32; x.len()];
        // SAFETY: live handle; valid non-overlapping buffers of x.len() floats.
        assert_eq!(
            unsafe {
                rr_dr60_process(
                    p,
                    x.as_ptr(),
                    got.as_mut_ptr(),
                    x.len(),
                    std::ptr::null_mut(),
                )
            },
            RrDr60Status::Ok
        );
        // SAFETY: from rr_dr60_create.
        unsafe { rr_dr60_destroy(p) };
        assert_eq!(got, want, "{:?}", s.agc);
    }
}

/// A C settings field and a way to put it out of range.
type BreakCase = (RrDr60SettingField, fn(&mut RrDr60Settings));

/// Validates through the C API and returns (status, field).
fn validate(s: &RrDr60Settings) -> (RrDr60Status, RrDr60SettingField) {
    let mut field = RrDr60SettingField::AgcReleaseMs; // overwritten on every call
    // SAFETY: valid pointers.
    let status = unsafe { rr_dr60_settings_validate(s, &mut field) };
    (status, field)
}

/// Spec 002 US2 AS5, FR-014 (contracts/c-api.md): invalid AGC settings are rejected with
/// INVALID_SETTING, and `rr_dr60_settings_validate` names the field.
#[test]
fn c_api_names_invalid_agc_settings() {
    let cases: [BreakCase; 5] = [
        (RrDr60SettingField::AgcTargetDbfs, |s| {
            s.agc_target_dbfs = 1.0
        }),
        (RrDr60SettingField::AgcMaxGainDb, |s| {
            s.agc_max_gain_db = f32::NAN
        }),
        (RrDr60SettingField::AgcMaxAttenuationDb, |s| {
            s.agc_max_attenuation_db = 41.0
        }),
        (RrDr60SettingField::AgcAttackMs, |s| s.agc_attack_ms = 0.0),
        (RrDr60SettingField::AgcReleaseMs, |s| {
            s.agc_release_ms = f32::INFINITY
        }),
    ];
    let sentinel = std::ptr::NonNull::<RrDr60Pipeline>::dangling().as_ptr();
    for (field, break_it) in cases {
        let mut s = rr_dr60_settings_default(48_000);
        break_it(&mut s);
        assert_eq!(validate(&s), (RrDr60Status::InvalidSetting, field));
        let mut out = sentinel;
        // SAFETY: valid pointers.
        assert_eq!(
            unsafe { rr_dr60_create(&s, &mut out) },
            RrDr60Status::InvalidSetting
        );
        assert_eq!(out, sentinel, "*out touched on error");
    }
    assert_eq!(
        validate(&rr_dr60_settings_default(48_000)),
        (RrDr60Status::Ok, RrDr60SettingField::None)
    );
}

/// Spec 002 contracts/c-api.md: `validate` returns exactly what `create` returns, in the
/// documented order (struct size → tap → host rate → AGC fields), and handles NULL.
#[test]
fn c_api_validate_agrees_with_create() {
    let base = rr_dr60_settings_default(48_000);
    let old_layout = RrDr60Settings {
        struct_size: 24,
        ..base
    };
    let bad_size_and_tap = RrDr60Settings {
        struct_size: 24,
        tap: 9,
        ..base
    };
    let bad_tap_and_rate = RrDr60Settings {
        tap: 9,
        host_rate_hz: 22_050,
        ..base
    };
    let bad_rate_and_attack = RrDr60Settings {
        host_rate_hz: 22_050,
        agc_attack_ms: 0.0,
        ..base
    };
    let bad_target_and_release = RrDr60Settings {
        agc_target_dbfs: 5.0,
        agc_release_ms: 1.0,
        ..base
    };
    let expected = [
        (
            old_layout,
            RrDr60Status::InvalidArgument,
            RrDr60SettingField::StructSize,
        ),
        (
            bad_size_and_tap,
            RrDr60Status::InvalidArgument,
            RrDr60SettingField::StructSize,
        ),
        (
            bad_tap_and_rate,
            RrDr60Status::InvalidArgument,
            RrDr60SettingField::Tap,
        ),
        (
            bad_rate_and_attack,
            RrDr60Status::UnsupportedHostRate,
            RrDr60SettingField::HostRate,
        ),
        (
            bad_target_and_release,
            RrDr60Status::InvalidSetting,
            RrDr60SettingField::AgcTargetDbfs,
        ),
    ];
    for (s, status, field) in expected {
        assert_eq!(validate(&s), (status, field), "{s:?}");
        let mut out = std::ptr::null_mut();
        // SAFETY: valid pointers.
        assert_eq!(unsafe { rr_dr60_create(&s, &mut out) }, status, "{s:?}");
    }
    // SAFETY: NULL settings is reported, never dereferenced; a NULL out_field is allowed.
    unsafe {
        let mut field = RrDr60SettingField::Tap;
        assert_eq!(
            rr_dr60_settings_validate(std::ptr::null(), &mut field),
            RrDr60Status::NullPointer
        );
        assert_eq!(
            rr_dr60_settings_validate(&base, std::ptr::null_mut()),
            RrDr60Status::Ok
        );
    }
}

/// Spec 002 FR-014: an invalid AGC setting in `rr_dr60_reconfigure` leaves the handle unchanged.
#[test]
fn c_api_reconfigure_rejects_invalid_agc_and_keeps_state() {
    let x = stimulus::step(1000.0, &[-30.0], &[0.2], 48_000.0);
    let good = rr_dr60_settings_default(48_000);
    let (p, q) = (create(&good), create(&good));
    let mut scratch = vec![0.0f32; x.len()];
    let mut bad = good;
    bad.agc_attack_ms = 0.0;
    // SAFETY: live handles; valid buffers and settings.
    unsafe {
        assert_eq!(
            rr_dr60_process(
                p,
                x.as_ptr(),
                scratch.as_mut_ptr(),
                x.len(),
                std::ptr::null_mut()
            ),
            RrDr60Status::Ok
        );
        assert_eq!(
            rr_dr60_process(
                q,
                x.as_ptr(),
                scratch.as_mut_ptr(),
                x.len(),
                std::ptr::null_mut()
            ),
            RrDr60Status::Ok
        );
        assert_eq!(rr_dr60_reconfigure(p, &bad), RrDr60Status::InvalidSetting);
        let (mut a, mut b) = (vec![0.0f32; x.len()], vec![0.0f32; x.len()]);
        assert_eq!(
            rr_dr60_process(p, x.as_ptr(), a.as_mut_ptr(), x.len(), std::ptr::null_mut()),
            RrDr60Status::Ok
        );
        assert_eq!(
            rr_dr60_process(q, x.as_ptr(), b.as_mut_ptr(), x.len(), std::ptr::null_mut()),
            RrDr60Status::Ok
        );
        assert_eq!(a, b, "failed reconfigure changed the handle");
        rr_dr60_destroy(p);
        rr_dr60_destroy(q);
    }
}

#[test]
fn error_table() {
    let ok = rr_dr60_settings_default(48_000);
    let sentinel = std::ptr::NonNull::<RrDr60Pipeline>::dangling().as_ptr();
    let mut out = sentinel;
    let status = |s: RrDr60Settings, out: &mut *mut RrDr60Pipeline| {
        // SAFETY: valid pointers.
        unsafe { rr_dr60_create(&s, out) }
    };
    // SAFETY: NULL settings / NULL out-pointer must be rejected without dereferencing.
    assert_eq!(
        unsafe { rr_dr60_create(std::ptr::null(), &mut out) },
        RrDr60Status::NullPointer
    );
    assert_eq!(
        unsafe { rr_dr60_create(&ok, std::ptr::null_mut()) },
        RrDr60Status::NullPointer
    );
    assert_eq!(
        status(rr_dr60_settings_default(22_050), &mut out),
        RrDr60Status::UnsupportedHostRate
    );
    assert_eq!(
        status(RrDr60Settings { tap: 7, ..ok }, &mut out),
        RrDr60Status::InvalidArgument
    );
    assert_eq!(
        status(
            RrDr60Settings {
                struct_size: 4,
                ..ok
            },
            &mut out
        ),
        RrDr60Status::InvalidArgument
    );
    assert_eq!(out, sentinel, "*out touched on error");

    let p = create(&ok);
    let x = vec![0.25f32; 256];
    let mut y = vec![0.0f32; 256];
    // SAFETY (all below): p is live; buffers are valid for their stated lengths.
    unsafe {
        assert_eq!(
            rr_dr60_process(
                std::ptr::null_mut(),
                x.as_ptr(),
                y.as_mut_ptr(),
                64,
                std::ptr::null_mut()
            ),
            RrDr60Status::NullPointer
        );
        assert_eq!(
            rr_dr60_process(
                p,
                std::ptr::null(),
                y.as_mut_ptr(),
                64,
                std::ptr::null_mut()
            ),
            RrDr60Status::NullPointer
        );
        assert_eq!(
            rr_dr60_process(
                p,
                x.as_ptr(),
                std::ptr::null_mut(),
                64,
                std::ptr::null_mut()
            ),
            RrDr60Status::NullPointer
        );
        assert_eq!(
            rr_dr60_process(
                p,
                std::ptr::null(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut()
            ),
            RrDr60Status::Ok
        );
        assert_eq!(
            rr_dr60_process(
                p,
                y.as_ptr(),
                y.as_mut_ptr().add(1),
                64,
                std::ptr::null_mut()
            ),
            RrDr60Status::InvalidArgument
        );
        assert_eq!(
            rr_dr60_process(
                p,
                y.as_ptr().add(1),
                y.as_mut_ptr(),
                64,
                std::ptr::null_mut()
            ),
            RrDr60Status::InvalidArgument
        );
        let mut lat = 7u32;
        assert_eq!(
            rr_dr60_latency_samples(std::ptr::null(), &mut lat),
            RrDr60Status::NullPointer
        );
        assert_eq!(
            rr_dr60_latency_samples(p, std::ptr::null_mut()),
            RrDr60Status::NullPointer
        );
        assert_eq!(lat, 7);
        assert_eq!(
            rr_dr60_reset(std::ptr::null_mut()),
            RrDr60Status::NullPointer
        );
        let unsupported = rr_dr60_settings_default(22_050);
        assert_eq!(
            rr_dr60_reconfigure(p, &unsupported),
            RrDr60Status::UnsupportedHostRate
        );
        assert_eq!(
            rr_dr60_process(p, x.as_ptr(), y.as_mut_ptr(), 256, std::ptr::null_mut()),
            RrDr60Status::Ok,
            "old config must still work"
        );
        rr_dr60_destroy(std::ptr::null_mut());
        rr_dr60_destroy(p);
    }
}

/// One Rust block's result, for comparison with the C side.
type BlockRecord = (RrDr60BlockInfo, Vec<RrDr60VasEvent>);

fn c_event(e: rr_dr60::VasEvent) -> RrDr60VasEvent {
    RrDr60VasEvent {
        output_position: e.output_position,
        input_length: e.input_length,
    }
}

/// Runs `x` through the Rust API in blocks of `block`, with events.
fn rust_blocks(s: rr_dr60::Settings, x: &[f32], block: usize) -> (Vec<f32>, Vec<BlockRecord>) {
    let mut p = Pipeline::new(s).unwrap();
    let (mut out, mut records) = (Vec::new(), Vec::new());
    for chunk in x.chunks(block) {
        let mut y = vec![0.0f32; chunk.len()];
        let mut ev = vec![rr_dr60::VasEvent::default(); p.max_events(chunk.len())];
        let info = p.process_with_events(chunk, &mut y, &mut ev).unwrap();
        out.extend_from_slice(&y[..info.produced]);
        records.push((
            RrDr60BlockInfo {
                produced: info.produced,
                events: info.events,
                paused: info.paused,
            },
            ev[..info.events].iter().copied().map(c_event).collect(),
        ));
    }
    (out, records)
}

/// Runs `x` through the C API in blocks of `block`, with `rr_dr60_process_with_events` and a
/// capacity from `rr_dr60_max_events`.
fn c_blocks(s: &RrDr60Settings, x: &[f32], block: usize) -> (Vec<f32>, Vec<BlockRecord>) {
    let p = create(s);
    let (mut out, mut records) = (Vec::new(), Vec::new());
    for chunk in x.chunks(block) {
        let mut y = vec![0.0f32; chunk.len()];
        let mut cap = usize::MAX;
        // SAFETY: live handle; valid out-pointer.
        assert_eq!(
            unsafe { rr_dr60_max_events(p, chunk.len(), &mut cap) },
            RrDr60Status::Ok
        );
        assert!(cap < usize::MAX);
        let mut ev = vec![RrDr60VasEvent::default(); cap];
        let mut info = RrDr60BlockInfo::default();
        // SAFETY: live handle; valid non-overlapping buffers; `ev` holds `cap` events.
        assert_eq!(
            unsafe {
                rr_dr60_process_with_events(
                    p,
                    chunk.as_ptr(),
                    y.as_mut_ptr(),
                    chunk.len(),
                    &mut info,
                    ev.as_mut_ptr(),
                    cap,
                )
            },
            RrDr60Status::Ok
        );
        assert!(
            info.events <= cap,
            "capacity {cap} < events {}",
            info.events
        );
        out.extend_from_slice(&y[..info.produced]);
        ev.truncate(info.events);
        records.push((info, ev));
    }
    // SAFETY: from rr_dr60_create.
    unsafe { rr_dr60_destroy(p) };
    (out, records)
}

/// Spec 003 US2 AS7, FR-015 (contracts/c-api.md rule 6): C and Rust give bit-identical output,
/// `BlockInfo` sequences and events for the VAS configurations and for the minimum and maximum
/// of every VAS setting.
#[test]
fn c_api_matches_rust_api_with_vas() {
    let rate = 48_000;
    let x = stimulus::burst_gap(
        1000.0,
        -8.0, // threshold + 10 dB (spec 003 definitions)
        &[
            (true, 0.5),
            (false, 2.0),
            (true, 0.3),
            (false, 1.5),
            (true, 0.4),
            (false, 0.5),
        ],
        48_000.0,
    );
    let mut cases: Vec<rr_dr60::Settings> = configs::VAS_CONFIGS
        .iter()
        .map(|c| configs::settings(c, rate))
        .collect();
    let extremes: [fn(&mut rr_dr60::VasSettings, bool); 4] = [
        |v, hi| v.sensitivity = if hi { 5 } else { 1 },
        |v, hi| v.threshold_dbfs = if hi { 0.0 } else { -60.0 },
        |v, hi| v.hang_ms = if hi { 10_000.0 } else { 50.0 },
        |v, hi| v.onset_ms = if hi { 200.0 } else { 0.0 },
    ];
    for set in extremes {
        for hi in [false, true] {
            for mode in [rr_dr60::VasMode::Drop, rr_dr60::VasMode::Mute] {
                let mut s = configs::settings("vas_only", rate);
                s.vas.mode = mode;
                set(&mut s.vas, hi);
                cases.push(s);
            }
        }
    }
    for s in cases {
        for block in [x.len(), 1000, 7] {
            let want = rust_blocks(s, &x, block);
            let got = c_blocks(&configs::c_settings(&s), &x, block);
            assert_eq!(got.0, want.0, "output: {:?} block {block}", s.vas);
            assert_eq!(got.1, want.1, "blocks: {:?} block {block}", s.vas);
        }
    }
}

/// Spec 003 FR-012, FR-015 (contracts/c-api.md › Conversion rules): invalid VAS fields are named,
/// `vas_sensitivity = 259` does not wrap to a valid level, an invalid `vas_mode` is
/// `INVALID_ARGUMENT`, and `validate` agrees with `create`.
#[test]
fn c_api_names_invalid_vas_settings() {
    /// A VAS field put out of range, with the status and field it must report.
    type VasBreak = (RrDr60Status, RrDr60SettingField, fn(&mut RrDr60Settings));
    let cases: [VasBreak; 6] = [
        (
            RrDr60Status::InvalidSetting,
            RrDr60SettingField::VasSensitivity,
            |s| s.vas_sensitivity = 6,
        ),
        (
            RrDr60Status::InvalidSetting,
            RrDr60SettingField::VasSensitivity,
            |s| s.vas_sensitivity = 259,
        ),
        (
            RrDr60Status::InvalidArgument,
            RrDr60SettingField::VasMode,
            |s| s.vas_mode = 7,
        ),
        (
            RrDr60Status::InvalidSetting,
            RrDr60SettingField::VasThresholdDbfs,
            |s| s.vas_threshold_dbfs = 0.5,
        ),
        (
            RrDr60Status::InvalidSetting,
            RrDr60SettingField::VasHangMs,
            |s| s.vas_hang_ms = 49.0,
        ),
        (
            RrDr60Status::InvalidSetting,
            RrDr60SettingField::VasOnsetMs,
            |s| s.vas_onset_ms = f32::NAN,
        ),
    ];
    let sentinel = std::ptr::NonNull::<RrDr60Pipeline>::dangling().as_ptr();
    for (status, field, break_it) in cases {
        let mut s = rr_dr60_settings_default(48_000);
        break_it(&mut s);
        assert_eq!(validate(&s), (status, field), "{s:?}");
        let mut out = sentinel;
        // SAFETY: valid pointers.
        assert_eq!(unsafe { rr_dr60_create(&s, &mut out) }, status, "{s:?}");
        assert_eq!(out, sentinel, "*out touched on error");
    }
    // `vas_mode` is checked after `tap` and before the host rate.
    let s = RrDr60Settings {
        vas_mode: 7,
        host_rate_hz: 22_050,
        ..rr_dr60_settings_default(48_000)
    };
    assert_eq!(
        validate(&s),
        (RrDr60Status::InvalidArgument, RrDr60SettingField::VasMode)
    );
}

/// Spec 003 contracts/c-api.md: `out_info` is always written on an early return (`frames == 0`,
/// NULL buffers, NULL events with a capacity) with `produced = 0`, `events = 0` and the current
/// `paused`; `events = NULL` is accepted with capacity 0; and a 0.2-sized struct is rejected as
/// `STRUCT_SIZE` after reading only its first field.
#[test]
fn c_api_out_info_on_early_returns_and_old_struct_size() {
    let p = create(&configs::c_settings(&configs::settings("vas_only", 8_000)));
    let silence = vec![0.0f32; 16_000]; // two hang times: ends paused
    let mut y = vec![0.0f32; 16_000];
    let stale = RrDr60BlockInfo {
        produced: 77,
        events: 77,
        paused: false,
    };
    let mut info = stale;
    // SAFETY (all below): p is live; buffers are valid for their stated lengths.
    unsafe {
        assert_eq!(
            rr_dr60_process(p, silence.as_ptr(), y.as_mut_ptr(), 16_000, &mut info),
            RrDr60Status::Ok
        );
        assert_eq!(info.produced, 8_000);
        assert!(info.paused);

        let idle = RrDr60BlockInfo {
            produced: 0,
            events: 0,
            paused: true,
        };
        // frames == 0.
        let status = rr_dr60_process(p, std::ptr::null(), std::ptr::null_mut(), 0, &mut info);
        assert_eq!((status, info), (RrDr60Status::Ok, idle));
        info = stale;
        // NULL input.
        let status = rr_dr60_process(p, std::ptr::null(), y.as_mut_ptr(), 64, &mut info);
        assert_eq!((status, info), (RrDr60Status::NullPointer, idle));
        info = stale;
        // NULL events with a non-zero capacity.
        let status = rr_dr60_process_with_events(
            p,
            silence.as_ptr(),
            y.as_mut_ptr(),
            64,
            &mut info,
            std::ptr::null_mut(),
            4,
        );
        assert_eq!((status, info), (RrDr60Status::NullPointer, idle));
        info = stale;
        // NULL events with capacity 0 is fine, and the block is processed.
        assert_eq!(
            rr_dr60_process_with_events(
                p,
                silence.as_ptr(),
                y.as_mut_ptr(),
                64,
                &mut info,
                std::ptr::null_mut(),
                0,
            ),
            RrDr60Status::Ok
        );
        assert_eq!(info.produced, 0, "still paused: nothing produced");
        // NULL handle: paused = false.
        info = stale;
        assert_eq!(
            rr_dr60_process(
                std::ptr::null_mut(),
                silence.as_ptr(),
                y.as_mut_ptr(),
                64,
                &mut info
            ),
            RrDr60Status::NullPointer
        );
        assert_eq!(info, RrDr60BlockInfo::default());
        // max_events: NULL handle / NULL out leave the out-parameter untouched (rule 2).
        let mut cap = 5usize;
        assert_eq!(
            rr_dr60_max_events(std::ptr::null(), 64, &mut cap),
            RrDr60Status::NullPointer
        );
        assert_eq!(cap, 5);
        assert_eq!(
            rr_dr60_max_events(p, 64, std::ptr::null_mut()),
            RrDr60Status::NullPointer
        );
        rr_dr60_destroy(p);
    }

    // A struct from the 0.2 layout (48 bytes, `struct_size = 48`): rejected as STRUCT_SIZE by
    // validate and create, reading only the first field.
    #[repr(C, align(8))]
    struct OldLayout([u8; 48]);
    let mut old = OldLayout([0; 48]);
    old.0[..4].copy_from_slice(&48u32.to_ne_bytes());
    let settings = (&raw const old).cast::<RrDr60Settings>();
    let mut field = RrDr60SettingField::None;
    let mut out = std::ptr::null_mut();
    // SAFETY: the functions read only `struct_size` before rejecting a too-small struct
    // (contracts/c-api.md), and the 48 bytes are readable and aligned.
    unsafe {
        assert_eq!(
            rr_dr60_settings_validate(settings, &mut field),
            RrDr60Status::InvalidArgument
        );
        assert_eq!(field, RrDr60SettingField::StructSize);
        assert_eq!(
            rr_dr60_create(settings, &mut out),
            RrDr60Status::InvalidArgument
        );
        assert!(out.is_null());
        let p = create(&rr_dr60_settings_default(48_000));
        assert_eq!(
            rr_dr60_reconfigure(p, settings),
            RrDr60Status::InvalidArgument
        );
        rr_dr60_destroy(p);
    }
}

#[test]
fn version_string_matches_crate() {
    // SAFETY: static NUL-terminated string.
    let v = unsafe { std::ffi::CStr::from_ptr(rr_dr60_version_string()) };
    assert_eq!(v.to_str().unwrap(), rr_dr60::VERSION);
}

#[cfg(feature = "ffi-test-panic")]
mod panics {
    use super::*;

    fn poison(p: *mut RrDr60Pipeline) {
        let x = [0.1f32; 32];
        let mut y = [0.0f32; 32];
        // SAFETY: live handle; valid buffers.
        unsafe {
            rr_dr60__test_force_panic(p);
            assert_eq!(
                rr_dr60_process(p, x.as_ptr(), y.as_mut_ptr(), 32, std::ptr::null_mut()),
                RrDr60Status::InternalError
            );
            assert_eq!(
                rr_dr60_process(p, x.as_ptr(), y.as_mut_ptr(), 32, std::ptr::null_mut()),
                RrDr60Status::InternalError,
                "not poisoned"
            );
            let mut lat = 0;
            assert_eq!(
                rr_dr60_latency_samples(p, &mut lat),
                RrDr60Status::InternalError
            );
        }
    }

    fn works(p: *mut RrDr60Pipeline) {
        let x = [0.1f32; 32];
        let mut y = [0.0f32; 32];
        // SAFETY: live handle; valid buffers.
        assert_eq!(
            unsafe { rr_dr60_process(p, x.as_ptr(), y.as_mut_ptr(), 32, std::ptr::null_mut()) },
            RrDr60Status::Ok
        );
    }

    #[test]
    fn panic_poisons_and_reset_recovers() {
        let p = create(&rr_dr60_settings_default(48_000));
        poison(p);
        // SAFETY: live handle.
        assert_eq!(unsafe { rr_dr60_reset(p) }, RrDr60Status::Ok);
        works(p);
        // SAFETY: from rr_dr60_create.
        unsafe { rr_dr60_destroy(p) };
    }

    /// Spec 003 contracts/c-api.md: a poisoned handle still writes `out_info`
    /// (`produced = 0`, `events = 0`, the last `paused`).
    #[test]
    fn poisoned_handle_writes_out_info() {
        let p = create(&rr_dr60_settings_default(48_000));
        poison(p);
        let x = [0.1f32; 32];
        let mut y = [0.0f32; 32];
        let mut info = RrDr60BlockInfo {
            produced: 9,
            events: 9,
            paused: true,
        };
        // SAFETY: live (poisoned) handle; valid buffers.
        assert_eq!(
            unsafe { rr_dr60_process(p, x.as_ptr(), y.as_mut_ptr(), 32, &mut info) },
            RrDr60Status::InternalError
        );
        assert_eq!(info, RrDr60BlockInfo::default());
        // SAFETY: from rr_dr60_create.
        unsafe { rr_dr60_destroy(p) };
    }

    #[test]
    fn panic_poisons_and_reconfigure_recovers() {
        let p = create(&rr_dr60_settings_default(48_000));
        poison(p);
        let s = rr_dr60_settings_default(16_000);
        // SAFETY: live handle; valid settings.
        assert_eq!(unsafe { rr_dr60_reconfigure(p, &s) }, RrDr60Status::Ok);
        works(p);
        // SAFETY: from rr_dr60_create.
        unsafe { rr_dr60_destroy(p) };
    }
}
